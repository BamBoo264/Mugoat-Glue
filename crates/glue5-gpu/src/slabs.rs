//! Small vertex and index buffers, many to a GPU buffer. The engine makes
//! tens of thousands of them (a model's meshes), most under 16 KiB, and on
//! iOS each GPU buffer of its own takes 7-16 KiB of memory however small it
//! is: 20,000 buffers of 512 bytes took 288 MB on an iPhone 13 Pro. Here they
//! are slots of 4 MiB GPU buffers instead. On Vulkan too, where the
//! allocator places small buffers in large blocks anyway, but finds room by
//! scanning a block's free list: with tens of thousands of buffers streamed
//! in and out, that scan took 7 % of the command thread on a Pixel 8a, and
//! slots took a sixth off its recording time (its slabs keep 60-90 MiB of
//! freed slots for their next buffers).
//!
//! Slots come in size classes (four per power of two, so at most a fifth
//! is lost to rounding) and go back to their class's free list once the GPU
//! has finished the batch they were freed in. Buffers bound as storage or
//! indirect arguments keep buffers of their own (alignment, and usage
//! tracking per buffer).

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use wgpu::BufferUsages;

const CHUNK: u64 = 4 << 20;
/// Largest size given a slot.
pub const MAX: u64 = 64 << 10;

/// Size classes: 256 bytes, then four per power of two up to `MAX`. All
/// multiples of 64 bytes, which is enough alignment for vertex and index
/// buffers and for copies.
const CLASSES: [u64; 33] = {
    let mut sizes = [0; 33];
    sizes[0] = 256;
    let (mut i, mut p) = (1, 256);
    while i < 33 {
        sizes[i] = p + p / 4 * ((i as u64 - 1) % 4 + 1);
        if (i - 1) % 4 == 3 {
            p *= 2;
        }
        i += 1;
    }
    sizes
};

/// Where a buffer lives in the slabs.
#[derive(Clone, Copy, Debug)]
pub struct Slot {
    chunk: u32,
    class: u8,
    pub offset: u64,
}

struct Chunk<B> {
    buffer: B,
    /// Slots currently allocated OR still in `pending` awaiting GPU completion.
    active_slots: u32,
}

pub struct SlabPool<B: Clone> {
    chunks: Vec<Option<Chunk<B>>>,
    /// Index of the chunk currently serving bump allocations from `tail`.
    tail_chunk: Option<usize>,
    /// Next byte of `tail_chunk` never handed out.
    tail: u64,
    /// Free slots by class.
    free: Vec<Vec<Slot>>,
    /// Slots freed in a batch the GPU may not have finished: (epoch, slot).
    pending: VecDeque<(u64, Slot)>,
    /// Batches (epochs) the GPU has finished, from the submit thread.
    finished: Arc<AtomicU64>,
    /// Bytes in slots handed out (class sizes).
    pub used: u64,
}

impl<B: Clone> SlabPool<B> {
    pub fn new(finished: Arc<AtomicU64>) -> Self {
        SlabPool {
            chunks: Vec::new(),
            tail_chunk: None,
            tail: CHUNK,
            free: vec![Vec::new(); CLASSES.len()],
            pending: VecDeque::new(),
            finished,
            used: 0,
        }
    }

    pub fn alloc_with(&mut self, bytes: u64, make_chunk: impl FnOnce() -> B) -> (Slot, B) {
        self.reclaim();
        let class = CLASSES.partition_point(|&c| c < bytes);
        let size = CLASSES[class];
        self.used += size;
        if let Some(slot) = self.free[class].pop() {
            let chunk = self.chunks[slot.chunk as usize].as_mut().expect("live slab chunk");
            chunk.active_slots += 1;
            return (slot, chunk.buffer.clone());
        }
        if self.tail_chunk.is_none() || self.tail + size > CHUNK {
            let buffer = make_chunk();
            let idx = match self.chunks.iter().position(Option::is_none) {
                Some(i) => {
                    self.chunks[i] = Some(Chunk { buffer, active_slots: 0 });
                    i
                }
                None => {
                    self.chunks.push(Some(Chunk { buffer, active_slots: 0 }));
                    self.chunks.len() - 1
                }
            };
            self.tail_chunk = Some(idx);
            self.tail = 0;
        }
        let idx = self.tail_chunk.expect("active tail chunk");
        let chunk = self.chunks[idx].as_mut().expect("tail slab chunk present");
        chunk.active_slots += 1;
        let slot = Slot { chunk: idx as u32, class: class as u8, offset: self.tail };
        self.tail += size;
        (slot, chunk.buffer.clone())
    }

    /// Frees a slot the commands of `epoch` may still use.
    pub fn free(&mut self, epoch: u64, slot: Slot) {
        self.used -= CLASSES[slot.class as usize];
        self.pending.push_back((epoch, slot));
    }

    /// Slots freed in batches the GPU has finished, back to their classes.
    fn reclaim(&mut self) {
        let finished = self.finished.load(Ordering::Acquire);
        while let Some(&(epoch, slot)) = self.pending.front() {
            if epoch > finished {
                break;
            }
            self.pending.pop_front();
            if let Some(chunk) = self.chunks.get_mut(slot.chunk as usize).and_then(Option::as_mut) {
                chunk.active_slots = chunk.active_slots.saturating_sub(1);
                self.free[slot.class as usize].push(slot);
            }
        }
    }

    /// Drops completely empty 4 MiB chunks (all slots freed and finished on GPU),
    /// keeping at most `keep_spare` empty chunks to avoid immediate churn.
    /// Returns bytes released.
    pub fn reclaim_empty(&mut self, keep_spare: usize) -> u64 {
        self.reclaim();
        let mut empty_kept = 0usize;
        let mut freed_chunks = 0u64;
        for idx in (0..self.chunks.len()).rev() {
            let is_empty = self.chunks[idx].as_ref().is_some_and(|c| c.active_slots == 0);
            if !is_empty {
                continue;
            }
            if empty_kept < keep_spare {
                empty_kept += 1;
                continue;
            }
            let target = idx as u32;
            for list in &mut self.free {
                list.retain(|s| s.chunk != target);
            }
            if self.tail_chunk == Some(idx) {
                self.tail_chunk = None;
                self.tail = CHUNK;
            }
            self.chunks[idx] = None;
            freed_chunks += 1;
        }
        while self.chunks.last().is_some_and(Option::is_none) {
            self.chunks.pop();
        }
        freed_chunks * CHUNK
    }

    /// The GPU memory currently held by live chunks, in bytes.
    pub fn bytes(&self) -> u64 {
        self.chunks.iter().filter_map(Option::as_ref).count() as u64 * CHUNK
    }
}

pub struct Slabs {
    device: wgpu::Device,
    pool: SlabPool<wgpu::Buffer>,
    pub used: u64,
}

impl Slabs {
    pub fn new(device: wgpu::Device, finished: Arc<AtomicU64>) -> Slabs {
        Slabs { device, pool: SlabPool::new(finished), used: 0 }
    }

    /// Whether a buffer of `bytes` and `usage` goes in a slot.
    pub fn takes(bytes: u64, usage: BufferUsages) -> bool {
        let allowed = BufferUsages::VERTEX | BufferUsages::INDEX | BufferUsages::COPY_SRC | BufferUsages::COPY_DST;
        bytes <= MAX && allowed.contains(usage)
    }

    /// A slot of at least `bytes` (at most `MAX`), and the GPU buffer it is in.
    pub fn alloc(&mut self, bytes: u64) -> (Slot, wgpu::Buffer) {
        let device = &self.device;
        let out = self.pool.alloc_with(bytes, || {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("slabs"),
                size: CHUNK,
                usage: BufferUsages::VERTEX | BufferUsages::INDEX | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        self.used = self.pool.used;
        out
    }

    /// Frees a slot the commands of `epoch` may still use.
    pub fn free(&mut self, epoch: u64, slot: Slot) {
        self.pool.free(epoch, slot);
        self.used = self.pool.used;
    }

    /// Drops finished empty chunks, returning bytes freed.
    pub fn reclaim_empty(&mut self, keep_spare: usize) -> u64 {
        self.pool.reclaim_empty(keep_spare)
    }

    /// The GPU memory taken, in bytes.
    pub fn bytes(&self) -> u64 {
        self.pool.bytes()
    }
}

#[test]
fn classes() {
    assert_eq!(&CLASSES[..6], &[256, 320, 384, 448, 512, 640]);
    assert_eq!(CLASSES[CLASSES.len() - 1], MAX);
    assert!(CLASSES.windows(2).all(|w| w[0] < w[1] && w[1] % 64 == 0 && w[1] * 4 <= w[0] * 5 + 1));
}

#[test]
fn slab_reclaims_empty_chunks_and_reuses_slots() {
    let finished = Arc::new(AtomicU64::new(0));
    let mut pool = SlabPool::new(finished.clone());
    let mut next_id = 0usize;
    let mut alloc_max = |p: &mut SlabPool<usize>| -> Slot {
        let (slot, _) = p.alloc_with(MAX, || {
            let id = next_id;
            next_id += 1;
            id
        });
        slot
    };

    // Each chunk is 4 MiB = 64 * 64 KiB slots. Fill 2 chunks cleanly (128 slots).
    let mut chunk0_slots = Vec::new();
    let mut chunk1_slots = Vec::new();
    for _ in 0..64 {
        chunk0_slots.push(alloc_max(&mut pool));
    }
    for _ in 0..64 {
        chunk1_slots.push(alloc_max(&mut pool));
    }
    assert_eq!(pool.bytes(), 2 * CHUNK);
    assert_eq!(pool.used, 2 * CHUNK);

    // Free chunk0 slots at epoch 1, keep chunk1 live.
    for s in chunk0_slots {
        pool.free(1, s);
    }
    assert_eq!(pool.used, CHUNK);
    // Before GPU finishes epoch 1, chunk0 must NOT be dropped yet.
    assert_eq!(pool.reclaim_empty(0), 0);
    assert_eq!(pool.bytes(), 2 * CHUNK);

    // Once GPU finishes epoch 1, chunk0 becomes reclaimable (keep_spare=0 drops it, keep_spare=1 retains 1 spare).
    finished.store(1, Ordering::Release);
    assert_eq!(pool.reclaim_empty(1), 0);
    assert_eq!(pool.reclaim_empty(0), CHUNK);
    assert_eq!(pool.bytes(), CHUNK);

    // Reallocating after reclaim reuses freed chunk slot 0 cleanly without leaking chunk indices.
    let reused = alloc_max(&mut pool);
    assert_eq!(pool.bytes(), 2 * CHUNK);
    assert_eq!(reused.chunk, 0);
    for s in chunk1_slots {
        pool.free(2, s);
    }
    pool.free(2, reused);
    finished.store(2, Ordering::Release);
    assert_eq!(pool.reclaim_empty(0), 2 * CHUNK);
    assert_eq!(pool.bytes(), 0);
}
