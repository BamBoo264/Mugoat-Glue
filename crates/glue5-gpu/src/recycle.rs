//! Buffers and textures the engine destroyed, kept for its next ones of the
//! same description once the GPU is done with them. Creating one is a call
//! into the kernel driver on the command thread (about a tenth of its time
//! on an iPhone 13 Pro while the world streams in), and the engine destroys
//! and creates the same sizes over and over: while loading, 95% of new
//! buffers matched one destroyed earlier.
//!
//! An object is reused once the GPU has finished the batch it was destroyed
//! in (the submit thread counts finished batches). What is kept is bounded
//! in bytes and in age; the rest is destroyed as before (the graveyard).

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use rustc_hash::FxHashMap;
use wgpu::{BufferUsages, Extent3d, TextureDimension, TextureFormat, TextureUsages};

use crate::submit::Grave;

/// Kept at most, in bytes.
const MAX_BYTES: u64 = 32 << 20;
/// Kept at most for this many batches (a few seconds).
const MAX_AGE: u64 = 1000;
/// Number of recently destroyed sampled textures remembered to spot eviction/reload thrashing.
const RECENT_DESTROYED_CAP: usize = 256;
/// Window (in epochs) inside which recreating a recently destroyed texture counts as a reload.
const RELOAD_WINDOW_EPOCHS: u64 = 240;

fn effective_max_bytes() -> u64 {
    use glue5_runtime::memory::{GuardPressure, MemoryTier, MemoryProfile, pressure_level};
    match pressure_level() {
        GuardPressure::Critical => 0,
        GuardPressure::High => 8 << 20,
        GuardPressure::Elevated => 16 << 20,
        GuardPressure::Nominal => match MemoryProfile::detect().tier {
            MemoryTier::Compact => 16 << 20,
            _ => MAX_BYTES,
        },
    }
}

/// Lightweight fingerprint of a texture descriptor to detect destroy -> recreate thrashing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TexSignature {
    pub format: TextureFormat,
    pub dim: TextureDimension,
    pub width: u32,
    pub height: u32,
    pub layers: u32,
    pub mips: u32,
    pub usage: TextureUsages,
}

/// Sliding window of recently destroyed sampled textures to detect reload thrashing.
#[derive(Default)]
pub struct ReloadDetector {
    recent: VecDeque<(TexSignature, u64)>,
    pub reloads: u64,
}

impl ReloadDetector {
    pub fn record_destroy(&mut self, sig: TexSignature, epoch: u64) {
        while self.recent.len() >= RECENT_DESTROYED_CAP {
            self.recent.pop_front();
        }
        self.recent.push_back((sig, epoch));
    }

    /// Returns true when a matching texture was destroyed recently within `RELOAD_WINDOW_EPOCHS`.
    pub fn note_create(&mut self, sig: TexSignature, epoch: u64) -> bool {
        while let Some(&(_, at)) = self.recent.front() {
            if epoch.saturating_sub(at) <= RELOAD_WINDOW_EPOCHS {
                break;
            }
            self.recent.pop_front();
        }
        if let Some(pos) = self.recent.iter().rposition(|&(s, _)| s == sig) {
            self.recent.remove(pos);
            self.reloads += 1;
            true
        } else {
            false
        }
    }
}

/// What a texture is made of; equal descriptions are interchangeable.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TexKey {
    pub format: TextureFormat,
    pub view: Option<TextureFormat>,
    pub dim: TextureDimension,
    pub size: Extent3d,
    pub mips: u32,
    pub samples: u32,
    pub usage: TextureUsages,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Key {
    Buffer(u64, BufferUsages),
    Texture(TexKey),
}

enum Object {
    Buffer(wgpu::Buffer),
    Texture(wgpu::Texture),
}

impl Object {
    fn grave(self) -> Grave {
        match self {
            Object::Buffer(b) => Grave::Buffer(b),
            Object::Texture(t) => Grave::Texture(t),
        }
    }
}

#[derive(Default)]
pub struct Recycler {
    /// Batches (epochs) the GPU has finished, from the submit thread.
    pub finished: Arc<AtomicU64>,
    /// Kept objects by description, oldest first: (ticket, epoch destroyed, object, bytes).
    kept: FxHashMap<Key, VecDeque<(u64, u64, Object, u64)>>,
    /// Everything kept, in the order it came (some taken since).
    order: VecDeque<(u64, u64, Key)>,
    next_ticket: u64,
    bytes: u64,
    pub reused: u64,
    pub tex_hits: u64,
    pub tex_misses: u64,
    pub tex_evicted: u64,
    pub reload_detector: ReloadDetector,
}

impl Recycler {
    /// A buffer of `bytes` and `usage` destroyed in an epoch the GPU has finished.
    pub fn buffer(&mut self, bytes: u64, usage: BufferUsages) -> Option<wgpu::Buffer> {
        match self.take(&Key::Buffer(bytes, usage))? {
            Object::Buffer(b) => Some(b),
            Object::Texture(_) => None,
        }
    }

    pub fn texture(&mut self, key: &TexKey) -> Option<wgpu::Texture> {
        let hit = match self.take(&Key::Texture(key.clone())) {
            Some(Object::Texture(t)) => Some(t),
            _ => None,
        };
        if hit.is_some() {
            self.tex_hits += 1;
        } else {
            self.tex_misses += 1;
        }
        hit
    }

    /// Keeps a buffer the commands of `epoch` may still use.
    pub fn keep_buffer(&mut self, epoch: u64, buffer: wgpu::Buffer) {
        let (bytes, usage) = (buffer.size(), buffer.usage());
        self.keep(epoch, Key::Buffer(bytes, usage), Object::Buffer(buffer), bytes);
    }

    pub fn keep_texture(&mut self, epoch: u64, key: TexKey, texture: wgpu::Texture, bytes: u64) {
        self.keep(epoch, Key::Texture(key), Object::Texture(texture), bytes);
    }

    /// Hands what is too old, or over the budget, to the graveyard.
    pub fn trim(&mut self, epoch: u64, graveyard: &mut Vec<Grave>) {
        let max_bytes = effective_max_bytes();
        while let Some((ticket, at, key)) = self.order.front() {
            let live = self.kept.get(key).and_then(|list| list.front()).is_some_and(|entry| entry.0 == *ticket);
            if live && self.bytes <= max_bytes && epoch.saturating_sub(*at) <= MAX_AGE {
                break;
            }
            let (_, _, key) = self.order.pop_front().unwrap();
            if !live {
                continue;
            }
            let list = self.kept.get_mut(&key).unwrap();
            let (_, _, object, bytes) = list.pop_front().unwrap();
            if list.is_empty() {
                self.kept.remove(&key);
            }
            self.bytes -= bytes;
            if matches!(key, Key::Texture(_)) {
                self.tex_evicted += 1;
            }
            graveyard.push(object.grave());
        }
    }

    /// Hands everything kept to the graveyard (memory is short); returns
    /// its bytes.
    pub fn release(&mut self, graveyard: &mut Vec<Grave>) -> u64 {
        self.order.clear();
        for (key, list) in self.kept.drain() {
            if matches!(key, Key::Texture(_)) {
                self.tex_evicted += list.len() as u64;
            }
            graveyard.extend(list.into_iter().map(|(_, _, object, _)| object.grave()));
        }
        std::mem::take(&mut self.bytes)
    }

    fn take(&mut self, key: &Key) -> Option<Object> {
        let finished = self.finished.load(Ordering::Acquire);
        let list = self.kept.get_mut(key)?;
        if list.front().is_none_or(|(_, epoch, ..)| *epoch > finished) {
            return None;
        }
        let (_, _, object, bytes) = list.pop_front().unwrap();
        if list.is_empty() {
            self.kept.remove(key);
        }
        self.bytes -= bytes;
        self.reused += 1;
        Some(object)
    }

    fn keep(&mut self, epoch: u64, key: Key, object: Object, bytes: u64) {
        let ticket = self.next_ticket;
        self.next_ticket = self.next_ticket.wrapping_add(1);
        self.kept.entry(key.clone()).or_default().push_back((ticket, epoch, object, bytes));
        self.order.push_back((ticket, epoch, key));
        self.bytes += bytes;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reload_detector_flags_rapid_recreation_and_expires_old_entries() {
        let mut detector = ReloadDetector::default();
        let sig = TexSignature {
            format: TextureFormat::Rgba8Unorm,
            dim: TextureDimension::D2,
            width: 1024,
            height: 1024,
            layers: 1,
            mips: 11,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        };
        let other = TexSignature { width: 512, ..sig };

        detector.record_destroy(sig, 10);
        assert!(!detector.note_create(other, 20));
        assert!(detector.note_create(sig, 50));
        assert_eq!(detector.reloads, 1);

        // Consuming once does not double-count without another destroy.
        assert!(!detector.note_create(sig, 55));

        // Expired past RELOAD_WINDOW_EPOCHS no longer counts as thrashing.
        detector.record_destroy(sig, 100);
        assert!(!detector.note_create(sig, 100 + RELOAD_WINDOW_EPOCHS + 1));
        assert_eq!(detector.reloads, 1);
    }
}
