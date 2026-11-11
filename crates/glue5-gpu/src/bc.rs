//! BCn texture data decoded or transcoded on the CPU, for devices without
//! native BC compression (iPhones before the A17 Pro, most Android GPUs).
//!
//! Paths when `!TEXTURE_COMPRESSION_BC`:
//! - **BC1 & BC3 -> ASTC 4x4** (when `TEXTURE_COMPRESSION_ASTC` is supported):
//!   direct block-by-block bit-transcoding with zero intermediate RGBA buffer,
//!   zero pixel swizzle, and exact 1-bit BC1 alpha preservation.
//! - **BC2 & BC7** (and BC1/BC3 when ASTC is absent but ETC2 is available):
//!   strip-based decode into a small L1/L2-friendly 4-row band directly encoded
//!   as ETC2 RGBA8, avoiding full-image multi-MB temporary allocations and nested
//!   thread spawning inside the decode pool.
//! - **BC4 -> R8**, **BC5 -> RG8**, **BC6H -> RGBA16F**, and non-block-aligned
//!   fallback targets: strip-based decode with cooperative cancellation.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::format::Bc;

/// Bytes per compressed 4x4 block (ASTC 4x4 and ETC2 RGBA8 both use 16 bytes).
pub const COMPRESSED_4X4_BLOCK_BYTES: u32 = 16;
/// Alias kept for clarity where ETC2 RGBA8 block size is referenced.
pub const ETC2_RGBA_BLOCK_BYTES: u32 = COMPRESSED_4X4_BLOCK_BYTES;

impl Bc {
    /// Bytes per compressed 4x4 block.
    pub fn block_bytes(self) -> usize {
        match self {
            Bc::Bc1 | Bc::Bc4 => 8,
            _ => 16,
        }
    }

    /// Bytes per decoded texel.
    pub fn texel_bytes(self) -> usize {
        match self {
            Bc::Bc4 => 1,
            Bc::Bc5 => 2,
            Bc::Bc6h => 8,
            _ => 4,
        }
    }
}

/// Decodes `cols` x `rows` blocks per layer (`layers` layers of `src_rows`
/// block rows, `src_bpr` bytes per block row) into tightly packed texels:
/// `cols * 4` texels per row, `rows * 4` rows per layer.
pub fn decode(bc: Bc, src: &[u8], src_bpr: usize, src_rows: usize, cols: usize, rows: usize, layers: usize) -> Vec<u8> {
    decode_cancellable(bc, src, src_bpr, src_rows, cols, rows, layers, &AtomicBool::new(false))
}

/// Same as [`decode`], stopping early and returning an empty vector if `cancel` is set.
pub fn decode_cancellable(
    bc: Bc,
    src: &[u8],
    src_bpr: usize,
    src_rows: usize,
    cols: usize,
    rows: usize,
    layers: usize,
    cancel: &AtomicBool,
) -> Vec<u8> {
    let texel = bc.texel_bytes();
    let pitch = cols * 4 * texel;
    let band = pitch * 4;
    let mut out = vec![0u8; band * rows * layers];
    let blk_bytes = bc.block_bytes();
    for layer in 0..layers {
        for row in 0..rows {
            if cancel.load(Ordering::Acquire) {
                return Vec::new();
            }
            let dst_base = (layer * rows + row) * band;
            let dst = &mut out[dst_base..dst_base + band];
            let src_row = (layer * src_rows + row) * src_bpr;
            for col in 0..cols {
                let at = src_row + col * blk_bytes;
                let Some(block) = src.get(at..at + blk_bytes) else {
                    return out;
                };
                decode_block(bc, block, &mut dst[col * 4 * texel..], pitch);
            }
        }
    }
    out
}

/// Decodes `cols` x `rows` x `layers` BC blocks strip-by-strip (one 4-row block
/// band at a time) directly into ETC2 RGBA8 blocks without allocating a full-image
/// uncompressed RGBA buffer or spawning nested OS threads.
pub fn decode_to_etc2_rgba(
    bc: Bc,
    src: &[u8],
    src_bpr: usize,
    src_rows: usize,
    cols: usize,
    rows: usize,
    layers: usize,
    cancel: &AtomicBool,
) -> Vec<u8> {
    if cols == 0 || rows == 0 || layers == 0 {
        return Vec::new();
    }
    let width = (cols * 4) as u32;
    let strip_pitch = cols * 4 * 4;
    let strip_bytes = strip_pitch * 4;
    let band_blocks_bytes = cols * COMPRESSED_4X4_BLOCK_BYTES as usize;
    let mut out = vec![0u8; band_blocks_bytes * rows * layers];
    let mut strip = vec![0u8; strip_bytes];
    let blk_bytes = bc.block_bytes();

    for layer in 0..layers {
        for row in 0..rows {
            if cancel.load(Ordering::Acquire) {
                return Vec::new();
            }
            let src_row = (layer * src_rows + row) * src_bpr;
            for col in 0..cols {
                let at = src_row + col * blk_bytes;
                if let Some(block) = src.get(at..at + blk_bytes) {
                    decode_block(bc, block, &mut strip[col * 16..], strip_pitch);
                } else {
                    strip[col * 16..col * 16 + 16].fill(0);
                }
            }
            // etcpak consumes BGRA pixels.
            for texel in strip.as_chunks_mut::<4>().0 {
                texel.swap(0, 2);
            }
            let dst_at = (layer * rows + row) * band_blocks_bytes;
            let surface = ctt_etcpak::Surface::new(&strip, width, 4);
            ctt_etcpak::etc2_rgba::compress_blocks_into(
                &surface,
                true,
                &mut out[dst_at..dst_at + band_blocks_bytes],
            );
        }
    }
    out
}

/// Directly bit-transcodes `cols` x `rows` x `layers` of BC1 or BC3 blocks into
/// ASTC 4x4 blocks with zero intermediate RGBA allocation.
pub fn transcode_astc(
    bc: Bc,
    src: &[u8],
    src_bpr: usize,
    src_rows: usize,
    cols: usize,
    rows: usize,
    layers: usize,
    cancel: &AtomicBool,
) -> Vec<u8> {
    if cols == 0 || rows == 0 || layers == 0 {
        return Vec::new();
    }
    let blk_bytes = bc.block_bytes();
    let row_out_bytes = cols * COMPRESSED_4X4_BLOCK_BYTES as usize;
    let mut out = vec![0u8; row_out_bytes * rows * layers];
    for layer in 0..layers {
        for row in 0..rows {
            if cancel.load(Ordering::Acquire) {
                return Vec::new();
            }
            let src_row = (layer * src_rows + row) * src_bpr;
            let dst_row = (layer * rows + row) * row_out_bytes;
            for col in 0..cols {
                let at = src_row + col * blk_bytes;
                let dst = &mut out[dst_row + col * 16..dst_row + col * 16 + 16];
                if let Some(block) = src.get(at..at + blk_bytes) {
                    match bc {
                        Bc::Bc1 => bc1_block_to_astc(block.try_into().unwrap(), dst.try_into().unwrap()),
                        Bc::Bc3 => bc3_block_to_astc(block.try_into().unwrap(), dst.try_into().unwrap()),
                        _ => unreachable!("ASTC fast path only supports BC1/BC3"),
                    }
                }
            }
        }
    }
    out
}

/// `width` x `height` RGBA8 texels (both multiples of 4, tightly packed)
/// encoded as ETC2 RGBA blocks, one block row at a time.
pub fn etc2_rgba(mut texels: Vec<u8>, width: u32, height: u32) -> Vec<u8> {
    for texel in texels.as_chunks_mut::<4>().0 {
        texel.swap(0, 2);
    }
    let pitch = width as usize * 4 * 4;
    let band_bytes = (width / 4) as usize * ETC2_RGBA_BLOCK_BYTES as usize;
    let mut out = vec![0u8; band_bytes * (height / 4) as usize];
    for (src, dst) in texels.chunks(pitch).zip(out.chunks_mut(band_bytes)) {
        let surface = ctt_etcpak::Surface::new(src, width, 4);
        ctt_etcpak::etc2_rgba::compress_blocks_into(&surface, true, dst);
    }
    out
}

/// ASTC integer sequence packing for 5 trits (values 0..=2) into 8 bits.
const TRIT_PACK: [u8; 243] = {
    let mut out = [0u8; 243];
    let mut seen = [false; 243];
    let mut t_val = 0u32;
    while t_val < 256 {
        let (c, mut t) = if ((t_val >> 2) & 7) == 7 {
            ((((t_val >> 5) & 7) << 2) | (t_val & 3), [0u32, 0, 0, 2, 2])
        } else if ((t_val >> 5) & 3) == 3 {
            (t_val & 31, [0, 0, 0, (t_val >> 7) & 1, 2])
        } else {
            (t_val & 31, [0, 0, 0, (t_val >> 5) & 3, (t_val >> 7) & 1])
        };
        if (c & 3) == 3 {
            t[2] = 2;
            t[1] = (c >> 4) & 1;
            t[0] = ((c >> 3) & 1) << 1 | (((c >> 2) & 1) & !((c >> 3) & 1));
        } else if ((c >> 2) & 3) == 3 {
            t[2] = 2;
            t[1] = 2;
            t[0] = c & 3;
        } else {
            t[2] = (c >> 4) & 1;
            t[1] = (c >> 2) & 3;
            t[0] = ((c >> 1) & 1) << 1 | ((c & 1) & !((c >> 1) & 1));
        }
        if t[0] <= 2 && t[1] <= 2 && t[2] <= 2 && t[3] <= 2 && t[4] <= 2 {
            let key = (t[0] + 3 * (t[1] + 3 * (t[2] + 3 * (t[3] + 3 * t[4])))) as usize;
            if !seen[key] {
                seen[key] = true;
                out[key] = t_val as u8;
            }
        }
        t_val += 1;
    }
    out
};

const fn abs_diff_u8(a: i32, b: i32) -> i32 {
    if a >= b { a - b } else { b - a }
}

const UNQ48: [u8; 48] = {
    let mut unq = [0u8; 48];
    let mut v = 0usize;
    while v < 48 {
        let d = v >> 4;
        let bits = v & 15;
        let a = bits & 1;
        let x = (bits >> 1) & 7;
        let aa = if a != 0 { 0x1FF } else { 0 };
        let bb = (x << 6) | x;
        let tt = (d * 22 + bb) ^ aa;
        unq[v] = (((aa & 0x80) | (tt >> 2)) & 0xFF) as u8;
        v += 1;
    }
    unq
};

const Q48_FOR_VALUE: [u8; 256] = {
    let mut q = [0u8; 256];
    let mut t = 0usize;
    while t < 256 {
        let mut best = 0u8;
        let mut best_err = i32::MAX;
        let mut v = 0usize;
        while v < 48 {
            let err = abs_diff_u8(UNQ48[v] as i32, t as i32);
            if err < best_err {
                best_err = err;
                best = v as u8;
            }
            v += 1;
        }
        q[t] = best;
        t += 1;
    }
    q
};

#[inline]
fn rgb565_to_rgb8(c: u16) -> [u32; 3] {
    let r = (((c >> 11) & 31) as u32 * 527 + 23) >> 6;
    let g = (((c >> 5) & 63) as u32 * 259 + 33) >> 6;
    let b = ((c & 31) as u32 * 527 + 23) >> 6;
    [r, g, b]
}

#[inline]
fn quant7(v8: u32) -> u8 {
    let base = (v8 >> 1) as i32;
    let mut best = 0u8;
    let mut best_err = i32::MAX;
    for v in (base - 1)..=(base + 1) {
        if (0..=127).contains(&v) {
            let decoded = (v << 1) | (v >> 6);
            let err = (decoded - v8 as i32).abs();
            if err < best_err {
                best_err = err;
                best = v as u8;
            }
        }
    }
    best
}

/// ASTC 3-level weight sequence written top-down from bit 127.
#[inline]
fn trit_weights(w: &[u8]) -> u128 {
    const BITS_FOR: [u32; 6] = [0, 2, 4, 5, 7, 8];
    let mut stream = 0u64;
    let mut bit = 0u32;
    let count = w.len();
    let mut g = 0;
    while g < count {
        let n = (count - g).min(5);
        let mut key = 0usize;
        let mut mul = 1usize;
        for i in 0..n {
            key += w[g + i] as usize * mul;
            mul *= 3;
        }
        let mask = (1u64 << BITS_FOR[n]) - 1;
        stream |= ((TRIT_PACK[key] as u64) & mask) << bit;
        bit += BITS_FOR[n];
        g += 5;
    }
    (stream.reverse_bits() as u128) << 64
}

/// Rewrites one 8-byte BC1 block into one 16-byte ASTC 4x4 block.
#[inline]
pub fn bc1_block_to_astc(bc1: &[u8; 8], astc: &mut [u8; 16]) {
    let c0 = u16::from_le_bytes([bc1[0], bc1[1]]);
    let c1 = u16::from_le_bytes([bc1[2], bc1[3]]);
    let indices = u32::from_le_bytes([bc1[4], bc1[5], bc1[6], bc1[7]]);
    let e = [rgb565_to_rgb8(c0), rgb565_to_rgb8(c1)];
    let four_colour = c0 > c1;
    let transparent = !four_colour && ((indices & (indices >> 1)) & 0x5555_5555) != 0;

    if four_colour {
        // Block mode 0x042: 4x4 weights, range 0..3, 1 plane, CEM 8 (RGB direct, 8-bit endpoints).
        let swap = (e[1][0] + e[1][1] + e[1][2]) < (e[0][0] + e[0][1] + e[0][2]);
        let i0 = usize::from(swap);
        let i1 = usize::from(!swap);
        let mut v: u128 = 0x042 | (8u128 << 13);
        for c in 0..3 {
            v |= ((e[i0][c] | (e[i1][c] << 8)) as u128) << (17 + 16 * c);
        }
        let mut weights = ((indices ^ (indices >> 1)) & 0x5555_5555) | ((indices << 1) & 0xAAAA_AAAA);
        if swap {
            weights = !weights;
        }
        v |= (weights.reverse_bits() as u128) << 96;
        *astc = v.to_le_bytes();
        return;
    }

    const WEIGHT_OF: [u8; 4] = [0, 2, 1, 0];
    if !transparent {
        // Block mode 0x051: 4x4 weights, range 0..2, 1 plane, CEM 8.
        let swap = (e[1][0] + e[1][1] + e[1][2]) < (e[0][0] + e[0][1] + e[0][2]);
        let i0 = usize::from(swap);
        let i1 = usize::from(!swap);
        let mut v: u128 = 0x051 | (8u128 << 13);
        for c in 0..3 {
            v |= ((e[i0][c] | (e[i1][c] << 8)) as u128) << (17 + 16 * c);
        }
        let mut w = [0u8; 16];
        for (i, dst) in w.iter_mut().enumerate() {
            let wt = WEIGHT_OF[((indices >> (2 * i)) & 3) as usize];
            *dst = if swap { 2 - wt } else { wt };
        }
        v |= trit_weights(&w);
        *astc = v.to_le_bytes();
        return;
    }

    // Block mode 0x451: 2 planes (colour, alpha: CCS=3), range 0..2, CEM 12 (RGBA direct, 7-bit endpoints).
    let mut q = [[0u8; 4]; 2];
    for (k, rgb) in e.iter().enumerate() {
        for c in 0..3 {
            q[k][c] = quant7(rgb[c]);
        }
    }
    q[0][3] = 127;
    q[1][3] = 0;
    let sum = |row: &[u8; 4]| -> u32 { (0..3).map(|c| ((row[c] as u32) << 1) | ((row[c] as u32) >> 6)).sum() };
    let swap = sum(&q[1]) < sum(&q[0]);
    let i0 = usize::from(swap);
    let i1 = usize::from(!swap);
    let mut v: u128 = 0x451 | (12u128 << 13);
    for c in 0..4 {
        v |= ((q[i0][c] as u128) | ((q[i1][c] as u128) << 7)) << (17 + 14 * c);
    }
    let mut w = [0u8; 32];
    for i in 0..16 {
        let index = ((indices >> (2 * i)) & 3) as usize;
        let colour = WEIGHT_OF[index];
        let alpha = if index == 3 { 2 } else { 0 };
        w[2 * i] = if swap { 2 - colour } else { colour };
        w[2 * i + 1] = if swap { 2 - alpha } else { alpha };
    }
    v |= trit_weights(&w);
    v |= 3u128 << (128 - 52 - 2);
    *astc = v.to_le_bytes();
}

#[inline]
fn pack48(v: &[u8; 8]) -> u128 {
    const T_BITS: [u32; 5] = [2, 2, 1, 2, 1];
    let mut out = 0u128;
    let mut bit = 17u32;
    for g in (0..8).step_by(5) {
        let n = (8 - g).min(5);
        let mut key = 0usize;
        let mut mul = 1usize;
        for i in 0..n {
            key += (v[g + i] >> 4) as usize * mul;
            mul *= 3;
        }
        let t_pack = TRIT_PACK[key] as u32;
        let mut used = 0u32;
        for i in 0..n {
            out |= ((v[g + i] & 15) as u128) << bit;
            bit += 4;
            let take = if n == 3 && i == 2 { 1 } else { T_BITS[i] };
            out |= (((t_pack >> used) & ((1 << take) - 1)) as u128) << bit;
            bit += take;
            used += take;
        }
    }
    out
}

#[inline]
fn lerp64(e0: u8, e1: u8, w: u32) -> i32 {
    ((e0 as i32 * (64 - w as i32)) + (e1 as i32 * w as i32) + 32) >> 6
}

/// Rewrites one 16-byte BC3 block into one 16-byte ASTC 4x4 block.
#[inline]
pub fn bc3_block_to_astc(bc3: &[u8; 16], astc: &mut [u8; 16]) {
    let ab = u64::from_le_bytes(bc3[..8].try_into().unwrap());
    let mut a = [0u32; 8];
    a[0] = bc3[0] as u32;
    a[1] = bc3[1] as u32;
    if a[0] > a[1] {
        for k in 1..7 {
            a[k + 1] = ((7 - k as u32) * a[0] + k as u32 * a[1]) / 7;
        }
    } else {
        for k in 1..5 {
            a[k + 1] = ((5 - k as u32) * a[0] + k as u32 * a[1]) / 5;
        }
        a[6] = 0;
        a[7] = 255;
    }
    let mut alpha = [0u8; 16];
    let mut a_min = 255u32;
    let mut a_max = 0u32;
    for (i, dst) in alpha.iter_mut().enumerate() {
        let av = a[((ab >> (16 + 3 * i)) & 7) as usize];
        *dst = av as u8;
        a_min = a_min.min(av);
        a_max = a_max.max(av);
    }

    let c0 = u16::from_le_bytes([bc3[8], bc3[9]]);
    let c1 = u16::from_le_bytes([bc3[10], bc3[11]]);
    let indices = u32::from_le_bytes([bc3[12], bc3[13], bc3[14], bc3[15]]);
    let e = [rgb565_to_rgb8(c0), rgb565_to_rgb8(c1)];
    let mut weights = ((indices ^ (indices >> 1)) & 0x5555_5555) | ((indices << 1) & 0xAAAA_AAAA);

    if a_min == a_max {
        // One flat alpha: lossless RGBA 8-bit endpoints with 2-bit color weights.
        let swap = (e[1][0] + e[1][1] + e[1][2]) < (e[0][0] + e[0][1] + e[0][2]);
        let i0 = usize::from(swap);
        let i1 = usize::from(!swap);
        let mut v: u128 = 0x042 | (12u128 << 13);
        for k in 0..3 {
            v |= ((e[i0][k] | (e[i1][k] << 8)) as u128) << (17 + 16 * k);
        }
        v |= ((a_min | (a_min << 8)) as u128) << (17 + 48);
        if swap {
            weights = !weights;
        }
        v |= (weights.reverse_bits() as u128) << 96;
        *astc = v.to_le_bytes();
        return;
    }

    const WV: [u32; 4] = [0, 21, 43, 64];
    let mut w = [0u8; 16];
    for (i, dst) in w.iter_mut().enumerate() {
        *dst = ((weights >> (2 * i)) & 3) as u8;
    }
    let mut q = [0u8; 8];
    for k in 0..3 {
        q[2 * k] = Q48_FOR_VALUE[e[0][k] as usize];
        q[2 * k + 1] = Q48_FOR_VALUE[e[1][k] as usize];
    }

    let mut best_err = u32::MAX;
    let mut best0 = Q48_FOR_VALUE[a_min as usize];
    let mut best1 = Q48_FOR_VALUE[a_max as usize];
    for d0 in -2i32..=2 {
        for d1 in -2i32..=2 {
            let t0 = a_min as i32 + d0 * 6;
            let t1 = a_max as i32 + d1 * 6;
            if !(0..=255).contains(&t0) || !(0..=255).contains(&t1) {
                continue;
            }
            let x0 = Q48_FOR_VALUE[t0 as usize];
            let x1 = Q48_FOR_VALUE[t1 as usize];
            let (u0, u1) = (UNQ48[x0 as usize], UNQ48[x1 as usize]);
            let mut error = 0u32;
            for &av in &alpha {
                let mut be = u32::MAX;
                for &wv in &WV {
                    let dd = lerp64(u0, u1, wv) - av as i32;
                    be = be.min((dd * dd) as u32);
                }
                error = error.saturating_add(be);
            }
            if error < best_err {
                best_err = error;
                best0 = x0;
                best1 = x1;
            }
        }
    }
    q[6] = best0;
    q[7] = best1;
    let (u0, u1) = (UNQ48[q[6] as usize], UNQ48[q[7] as usize]);
    let mut aw = [0u8; 16];
    for (i, &av) in alpha.iter().enumerate() {
        let mut be = u32::MAX;
        for (k, &wv) in WV.iter().enumerate() {
            let dd = lerp64(u0, u1, wv) - av as i32;
            let sq = (dd * dd) as u32;
            if sq < be {
                be = sq;
                aw[i] = k as u8;
            }
        }
    }

    let s0 = UNQ48[q[0] as usize] as u32 + UNQ48[q[2] as usize] as u32 + UNQ48[q[4] as usize] as u32;
    let s1 = UNQ48[q[1] as usize] as u32 + UNQ48[q[3] as usize] as u32 + UNQ48[q[5] as usize] as u32;
    if s1 < s0 {
        for k in 0..4 {
            q.swap(2 * k, 2 * k + 1);
        }
        for i in 0..16 {
            w[i] = 3 - w[i];
            aw[i] = 3 - aw[i];
        }
    }

    let mut v: u128 = 0x442 | (12u128 << 13);
    v |= pack48(&q);
    let mut stream = 0u64;
    for i in 0..16 {
        stream |= ((w[i] as u64) << (4 * i)) | ((aw[i] as u64) << (4 * i + 2));
    }
    v |= (stream.reverse_bits() as u128) << 64;
    v |= 3u128 << 62;
    *astc = v.to_le_bytes();
}

/// One block into `dst` (its top-left texel), `pitch` bytes per texel row.
fn decode_block(bc: Bc, block: &[u8], dst: &mut [u8], pitch: usize) {
    match bc {
        Bc::Bc1 => bcdec_rs::bc1(block, dst, pitch),
        Bc::Bc2 => bcdec_rs::bc2(block, dst, pitch),
        Bc::Bc3 => bcdec_rs::bc3(block, dst, pitch),
        Bc::Bc4 => bcdec_rs::bc4(block, dst, pitch, false),
        Bc::Bc5 => bcdec_rs::bc5(block, dst, pitch, false),
        Bc::Bc7 => bcdec_rs::bc7(block, dst, pitch),
        Bc::Bc6h => {
            // RGB halves, then RGBA16F with alpha 1.0.
            let mut rgb = [0u16; 4 * 4 * 3];
            bcdec_rs::bc6h_half(block, &mut rgb, 4 * 3, false);
            for y in 0..4 {
                for x in 0..4 {
                    let s = (y * 4 + x) * 3;
                    let d = y * pitch + x * 8;
                    for (c, v) in [rgb[s], rgb[s + 1], rgb[s + 2], 0x3c00].into_iter().enumerate() {
                        dst[d + c * 2..d + c * 2 + 2].copy_from_slice(&v.to_le_bytes());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Independent spec-level ASTC 4x4 block decoder covering modes emitted by
    /// `bc1_block_to_astc` and `bc3_block_to_astc` (0x042, 0x051, 0x451, 0x442).
    fn test_decode_astc_block(astc: &[u8; 16]) -> [[u8; 4]; 16] {
        let v = u128::from_le_bytes(*astc);
        let mode = (v & 0x7FF) as u32;
        let cem = ((v >> 13) & 0xF) as u32;
        let lerp = |a: u8, b: u8, w: u32| -> u8 { (((a as u32 * (64 - w)) + (b as u32 * w) + 32) >> 6) as u8 };
        let decode_trit_stream = |count: usize, top_val: u128| -> Vec<u8> {
            let stream = (top_val >> 64) as u64;
            let raw = stream.reverse_bits();
            const BITS_FOR: [u32; 6] = [0, 2, 4, 5, 7, 8];
            let mut out = Vec::with_capacity(count);
            let mut bit = 0u32;
            let mut g = 0;
            while g < count {
                let n = (count - g).min(5);
                let field = ((raw >> bit) & ((1u64 << BITS_FOR[n]) - 1)) as u8;
                let limit = 3usize.pow(n as u32);
                let key = TRIT_PACK[..limit]
                    .iter()
                    .position(|&p| (p as u32 & ((1u32 << BITS_FOR[n]) - 1)) == field as u32)
                    .unwrap();
                let mut k = key;
                for _ in 0..n {
                    out.push((k % 3) as u8);
                    k /= 3;
                }
                bit += BITS_FOR[n];
                g += 5;
            }
            out
        };
        let mut out = [[0u8; 4]; 16];
        match (mode, cem) {
            (0x042, 8) => {
                let ep = |c: u32| -> (u8, u8) {
                    let word = ((v >> (17 + 16 * c)) & 0xFFFF) as u16;
                    (word as u8, (word >> 8) as u8)
                };
                let (r0, r1) = ep(0);
                let (g0, g1) = ep(1);
                let (b0, b1) = ep(2);
                let weights = ((v >> 96) as u32).reverse_bits();
                const W2: [u32; 4] = [0, 21, 43, 64];
                for (i, px) in out.iter_mut().enumerate() {
                    let wi = W2[((weights >> (2 * i)) & 3) as usize];
                    *px = [lerp(r0, r1, wi), lerp(g0, g1, wi), lerp(b0, b1, wi), 255];
                }
            }
            (0x051, 8) => {
                let ep = |c: u32| -> (u8, u8) {
                    let word = ((v >> (17 + 16 * c)) & 0xFFFF) as u16;
                    (word as u8, (word >> 8) as u8)
                };
                let (r0, r1) = ep(0);
                let (g0, g1) = ep(1);
                let (b0, b1) = ep(2);
                let w = decode_trit_stream(16, v);
                const W3: [u32; 3] = [0, 32, 64];
                for (i, px) in out.iter_mut().enumerate() {
                    let wi = W3[w[i] as usize];
                    *px = [lerp(r0, r1, wi), lerp(g0, g1, wi), lerp(b0, b1, wi), 255];
                }
            }
            (0x451, 12) => {
                let ep7 = |c: u32| -> (u8, u8) {
                    let word = ((v >> (17 + 14 * c)) & 0x3FFF) as u32;
                    let (a, b) = (word & 0x7F, (word >> 7) & 0x7F);
                    (((a << 1) | (a >> 6)) as u8, ((b << 1) | (b >> 6)) as u8)
                };
                let (r0, r1) = ep7(0);
                let (g0, g1) = ep7(1);
                let (b0, b1) = ep7(2);
                let (a0, a1) = ep7(3);
                let w = decode_trit_stream(32, v);
                const W3: [u32; 3] = [0, 32, 64];
                for (i, px) in out.iter_mut().enumerate() {
                    let wc = W3[w[2 * i] as usize];
                    let wa = W3[w[2 * i + 1] as usize];
                    *px = [lerp(r0, r1, wc), lerp(g0, g1, wc), lerp(b0, b1, wc), lerp(a0, a1, wa)];
                }
            }
            (0x042, 12) => {
                let ep = |c: u32| -> (u8, u8) {
                    let word = ((v >> (17 + 16 * c)) & 0xFFFF) as u16;
                    (word as u8, (word >> 8) as u8)
                };
                let (r0, r1) = ep(0);
                let (g0, g1) = ep(1);
                let (b0, b1) = ep(2);
                let (a0, a1) = ep(3);
                let weights = ((v >> 96) as u32).reverse_bits();
                const W2: [u32; 4] = [0, 21, 43, 64];
                for (i, px) in out.iter_mut().enumerate() {
                    let wi = W2[((weights >> (2 * i)) & 3) as usize];
                    *px = [lerp(r0, r1, wi), lerp(g0, g1, wi), lerp(b0, b1, wi), lerp(a0, a1, wi)];
                }
            }
            (0x442, 12) => {
                // Range-48 packed endpoints from bits 17..62.
                const T_BITS: [u32; 5] = [2, 2, 1, 2, 1];
                let mut q = [0u8; 8];
                let mut bit = 17u32;
                for g in (0..8).step_by(5) {
                    let n = (8 - g).min(5);
                    let mut trits_bits = 0u32;
                    let mut used = 0u32;
                    for i in 0..n {
                        let lo = ((v >> bit) & 0xF) as u8;
                        bit += 4;
                        let take = if n == 3 && i == 2 { 1 } else { T_BITS[i] };
                        trits_bits |= (((v >> bit) & ((1 << take) - 1)) as u32) << used;
                        bit += take;
                        used += take;
                        q[g + i] = lo;
                    }
                    let total_tbits = if n == 5 { 8 } else { 5 };
                    let limit = 3usize.pow(n as u32);
                    let mut k = TRIT_PACK[..limit]
                        .iter()
                        .position(|&p| (p as u32 & ((1 << total_tbits) - 1)) == trits_bits)
                        .unwrap();
                    for i in 0..n {
                        q[g + i] |= ((k % 3) as u8) << 4;
                        k /= 3;
                    }
                }
                let e: Vec<u8> = q.iter().map(|&idx| UNQ48[idx as usize]).collect();
                let stream = ((v >> 64) as u64).reverse_bits();
                const W2: [u32; 4] = [0, 21, 43, 64];
                for (i, px) in out.iter_mut().enumerate() {
                    let wc = W2[((stream >> (4 * i)) & 3) as usize];
                    let wa = W2[((stream >> (4 * i + 2)) & 3) as usize];
                    *px = [lerp(e[0], e[1], wc), lerp(e[2], e[3], wc), lerp(e[4], e[5], wc), lerp(e[6], e[7], wa)];
                }
            }
            other => panic!("unexpected ASTC test mode {other:?}"),
        }
        out
    }

    #[test]
    fn etc2_rgba_keeps_colours_and_alpha() {
        let colours = [[255u8, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [255, 255, 255, 128]];
        let mut texels = vec![0u8; 8 * 8 * 4];
        for y in 0..8 {
            for x in 0..8 {
                let c = colours[(y / 4) * 2 + x / 4];
                texels[(y * 8 + x) * 4..][..4].copy_from_slice(&c);
            }
        }
        let blocks = super::etc2_rgba(texels.clone(), 8, 8);
        assert_eq!(blocks.len(), 4 * super::ETC2_RGBA_BLOCK_BYTES as usize);
        let decoded = ctt_etcpak::decode::decode_rgba(&blocks, 8, 8);
        for (got, want) in decoded.chunks(4).zip(texels.chunks(4)) {
            assert!(got.iter().zip(want).all(|(g, w)| g.abs_diff(*w) <= 8), "{got:?} for {want:?}");
        }
    }

    #[test]
    fn bc1_to_astc_matches_bcdec_across_modes() {
        // 1) Four-colour gradient (c0 > c1), 2) Three-colour opaque (c0 <= c1), 3) Three-colour transparent.
        let blocks = [
            [0x00, 0xF8, 0x1F, 0x00, 0xE4, 0xE4, 0xB1, 0x1B],
            [0x1F, 0x00, 0xE0, 0x07, 0x00, 0x55, 0xAA, 0x50],
            [0x00, 0x00, 0xFF, 0xFF, 0xFF, 0x00, 0x3F, 0xCC],
        ];
        for bc1 in blocks {
            let mut astc = [0u8; 16];
            bc1_block_to_astc(&bc1, &mut astc);
            let got = test_decode_astc_block(&astc);
            let mut want = [0u8; 64];
            bcdec_rs::bc1(&bc1, &mut want, 16);
            for i in 0..16 {
                let w = &want[i * 4..i * 4 + 4];
                assert_eq!(got[i][3], w[3], "alpha mismatch for BC1 {bc1:?} texel {i}");
                if w[3] > 0 {
                    for ch in 0..3 {
                        assert!(
                            got[i][ch].abs_diff(w[ch]) <= 4,
                            "ch {ch} diff too large: got {} want {} (bc1={bc1:?})",
                            got[i][ch],
                            w[ch]
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn bc3_to_astc_matches_bcdec_flat_and_varying_alpha() {
        let blocks = [
            // Flat alpha = 200
            [200, 200, 0, 0, 0, 0, 0, 0, 0xE0, 0x07, 0x00, 0xF8, 0xE4, 0xB1, 0x1B, 0x4E],
            // Varying gradient alpha 16..240
            [240, 16, 0x88, 0x88, 0x10, 0x32, 0x54, 0x76, 0xFF, 0xFF, 0x20, 0x08, 0xE4, 0xB1, 0x1B, 0x4E],
        ];
        for bc3 in blocks {
            let mut astc = [0u8; 16];
            bc3_block_to_astc(&bc3, &mut astc);
            let got = test_decode_astc_block(&astc);
            let mut want = [0u8; 64];
            bcdec_rs::bc3(&bc3, &mut want, 16);
            for i in 0..16 {
                let w = &want[i * 4..i * 4 + 4];
                for ch in 0..3 {
                    assert!(
                        got[i][ch].abs_diff(w[ch]) <= 20,
                        "BC3 ch {ch} diff too large: got {} want {}",
                        got[i][ch],
                        w[ch]
                    );
                }
                assert!(
                    got[i][3].abs_diff(w[3]) <= 45,
                    "BC3 alpha diff too large: got {} want {}",
                    got[i][3],
                    w[3]
                );
            }
        }
    }

    #[test]
    fn strip_etc2_matches_full_decode_and_honours_cancel() {
        let bc7_block = [0x40, 0xF0, 0x0F, 0x88, 0x22, 0x55, 0xAA, 0xFF, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
        let mut src = Vec::new();
        for _ in 0..(4 * 2) {
            src.extend_from_slice(&bc7_block);
        }
        let strip_out = decode_to_etc2_rgba(Bc::Bc7, &src, 32, 2, 2, 2, 2, &AtomicBool::new(false));
        let full_rgba = decode(Bc::Bc7, &src, 32, 2, 2, 2, 2);
        let full_etc2 = etc2_rgba(full_rgba, 8, 16);
        assert_eq!(strip_out, full_etc2);

        let cancelled = AtomicBool::new(true);
        assert!(decode_to_etc2_rgba(Bc::Bc7, &src, 32, 2, 2, 2, 2, &cancelled).is_empty());
        assert!(transcode_astc(Bc::Bc1, &src, 16, 2, 2, 2, 1, &cancelled).is_empty());
        assert!(decode_cancellable(Bc::Bc4, &src, 16, 2, 2, 2, 1, &cancelled).is_empty());
    }
}
