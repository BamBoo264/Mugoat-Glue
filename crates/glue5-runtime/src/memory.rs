//! The module's linear memory: reserved once (so it never moves while other
//! threads run) and accessed by host code through [`Mem`].

use std::ffi::CStr;
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};
use std::time::Instant;

use anyhow::{Result, bail};
use glue5_game::{Env, MEMORY_INITIAL_PAGES, MEMORY_MAX_PAGES, Memory, PAGE_SIZE, orig};

pub struct WasmMemory {
    pub desc: *mut Memory,
    base: *mut u8,
    max_pages: u64,
}

unsafe impl Send for WasmMemory {}
unsafe impl Sync for WasmMemory {}

impl WasmMemory {
    /// Reserves address space for the largest memory the module may grow to
    /// (or the most the OS grants, but at least the initial size). Pages count
    /// once written; on Windows, which does not overcommit, the memory's
    /// current size is committed too ([`commit`]).
    pub fn reserve() -> Result<Self> {
        let mut max_pages = MEMORY_MAX_PAGES;
        loop {
            let bytes = (max_pages * PAGE_SIZE) as usize;
            if let Some(base) = map(bytes) {
                if !commit(base, 0, MEMORY_INITIAL_PAGES * PAGE_SIZE) {
                    bail!("không thể cấp phát {} MiB cho bộ nhớ wasm", (MEMORY_INITIAL_PAGES * PAGE_SIZE) >> 20);
                }
                log::info!("wasm memory: {} MiB reserved at {base:p}", bytes >> 20);
                let desc = unsafe { Memory::new(base, MEMORY_INITIAL_PAGES, max_pages) };
                return Ok(WasmMemory { desc, base, max_pages });
            }
            if max_pages / 2 < MEMORY_INITIAL_PAGES {
                bail!("không thể giữ trước {} MiB cho bộ nhớ wasm", (MEMORY_INITIAL_PAGES * PAGE_SIZE) >> 20);
            }
            max_pages /= 2;
        }
    }

    pub fn mem(&self) -> Mem {
        Mem { base: self.base }
    }

    pub fn max_bytes(&self) -> u64 {
        self.max_pages * PAGE_SIZE
    }

    pub fn size(&self) -> u64 {
        unsafe { (*self.desc).size }
    }

    /// `emscripten_resize_heap`: memory is already reserved, growing only
    /// publishes the new size (and commits it, on Windows).
    pub fn grow_to(&self, bytes: u64) -> bool {
        if bytes > self.max_bytes() {
            return false;
        }
        let pages = bytes.div_ceil(PAGE_SIZE);
        // Committed before it is published: committing pages that are
        // already is harmless, so racing growers need no lock.
        if !commit(self.base, self.size(), pages * PAGE_SIZE) {
            return false;
        }
        unsafe {
            if pages > (*self.desc).pages {
                (*self.desc).pages = pages;
                (*self.desc).size = pages * PAGE_SIZE;
            }
        }
        true
    }
}

#[cfg(unix)]
fn map(bytes: usize) -> Option<*mut u8> {
    let ptr = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            bytes,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANON | libc::MAP_NORESERVE,
            -1,
            0,
        )
    };
    (ptr != libc::MAP_FAILED).then_some(ptr as *mut u8)
}

/// Makes `[from, to)` of the mapping usable. Unix systems overcommit: the
/// whole mapping is, already.
#[cfg(unix)]
fn commit(_base: *mut u8, _from: u64, _to: u64) -> bool {
    true
}

/// Address space only: committed memory counts against the system's commit
/// limit (RAM and page file), however little of it is touched.
#[cfg(windows)]
fn map(bytes: usize) -> Option<*mut u8> {
    use windows_sys::Win32::System::Memory::{MEM_RESERVE, PAGE_NOACCESS, VirtualAlloc};
    let ptr = unsafe { VirtualAlloc(std::ptr::null(), bytes, MEM_RESERVE, PAGE_NOACCESS) };
    (!ptr.is_null()).then_some(ptr as *mut u8)
}

/// Commits `[from, to)` of the reservation, as the memory grows: its only
/// growth is `emscripten_resize_heap` ([`WasmMemory::grow_to`]). Committed
/// pages read as zeros, as wasm's do.
#[cfg(windows)]
fn commit(base: *mut u8, from: u64, to: u64) -> bool {
    use windows_sys::Win32::System::Memory::{MEM_COMMIT, PAGE_READWRITE, VirtualAlloc};
    if to <= from {
        return true;
    }
    let ptr = unsafe { VirtualAlloc(base.add(from as usize).cast(), (to - from) as usize, MEM_COMMIT, PAGE_READWRITE) };
    !ptr.is_null()
}

/// A view of wasm memory for host code. Addresses are wasm (linear memory)
/// addresses. The memory is shared with running wasm threads; plain accessors
/// are unordered, like non-atomic wasm accesses.
#[derive(Clone, Copy)]
pub struct Mem {
    base: *mut u8,
}

unsafe impl Send for Mem {}
unsafe impl Sync for Mem {}

impl Mem {
    /// # Safety
    /// `env` must be a live instance's environment.
    pub unsafe fn of(env: *mut Env) -> Mem {
        Mem { base: unsafe { (*(*env).memory).data } }
    }

    /// A view of `buf` as if it were wasm memory (tests).
    #[cfg(test)]
    pub(crate) fn over(buf: &mut [u8]) -> Mem {
        Mem { base: buf.as_mut_ptr() }
    }

    pub fn ptr(self, addr: u64) -> *mut u8 {
        unsafe { self.base.add(addr as usize) }
    }

    pub fn u8(self, addr: u64) -> u8 {
        unsafe { *self.ptr(addr) }
    }
    pub fn u16(self, addr: u64) -> u16 {
        unsafe { self.ptr(addr).cast::<u16>().read_unaligned() }
    }
    pub fn u32(self, addr: u64) -> u32 {
        unsafe { self.ptr(addr).cast::<u32>().read_unaligned() }
    }
    pub fn i32(self, addr: u64) -> i32 {
        self.u32(addr) as i32
    }
    pub fn u64(self, addr: u64) -> u64 {
        unsafe { self.ptr(addr).cast::<u64>().read_unaligned() }
    }
    pub fn f64(self, addr: u64) -> f64 {
        f64::from_bits(self.u64(addr))
    }

    pub fn set_u8(self, addr: u64, v: u8) {
        unsafe { *self.ptr(addr) = v }
    }
    pub fn set_u16(self, addr: u64, v: u16) {
        unsafe { self.ptr(addr).cast::<u16>().write_unaligned(v) }
    }
    pub fn set_u32(self, addr: u64, v: u32) {
        unsafe { self.ptr(addr).cast::<u32>().write_unaligned(v) }
    }
    pub fn set_i32(self, addr: u64, v: i32) {
        self.set_u32(addr, v as u32)
    }
    pub fn set_u64(self, addr: u64, v: u64) {
        unsafe { self.ptr(addr).cast::<u64>().write_unaligned(v) }
    }
    pub fn set_i64(self, addr: u64, v: i64) {
        self.set_u64(addr, v as u64)
    }

    pub fn bytes<'a>(self, addr: u64, len: usize) -> &'a [u8] {
        unsafe { std::slice::from_raw_parts(self.ptr(addr), len) }
    }
    #[allow(clippy::mut_from_ref)]
    pub fn bytes_mut<'a>(self, addr: u64, len: usize) -> &'a mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr(addr), len) }
    }

    /// NUL-terminated string at `addr` (lossy UTF-8).
    pub fn str(self, addr: u64) -> String {
        if addr == 0 {
            return String::new();
        }
        unsafe { CStr::from_ptr(self.ptr(addr) as *const _) }.to_string_lossy().into_owned()
    }

    /// Emscripten's `stringToUTF8`: writes at most `cap - 1` bytes plus a NUL.
    /// Returns the bytes written, without the NUL.
    pub fn write_str(self, addr: u64, s: &str, cap: usize) -> usize {
        if cap == 0 {
            return 0;
        }
        let mut n = s.len().min(cap - 1);
        while !s.is_char_boundary(n) {
            n -= 1;
        }
        self.bytes_mut(addr, n).copy_from_slice(&s.as_bytes()[..n]);
        self.set_u8(addr + n as u64, 0);
        n
    }

    pub fn atomic_u32<'a>(self, addr: u64) -> &'a AtomicU32 {
        unsafe { &*(self.ptr(addr) as *const AtomicU32) }
    }
    pub fn atomic_u64<'a>(self, addr: u64) -> &'a AtomicU64 {
        unsafe { &*(self.ptr(addr) as *const AtomicU64) }
    }

    /// `Atomics.store` + `Atomics.notify` on a 32-bit word.
    pub fn store_notify(self, addr: u64, v: u32) {
        let word = self.atomic_u32(addr);
        word.store(v, Ordering::SeqCst);
        glue5_game::futex::notify(word, u32::MAX);
    }
}

/// The process's memory footprint (what iOS limits; the resident set on
/// Android and Linux) and, on iOS and tvOS, how much more it may take before
/// the system terminates it, in bytes.
pub fn footprint() -> (Option<u64>, Option<u64>) {
    #[cfg(target_vendor = "apple")]
    let used = unsafe {
        // libproc: in the system on iOS too, though its header is macOS-only.
        unsafe extern "C" {
            fn proc_pid_rusage(pid: libc::c_int, flavor: libc::c_int, buffer: *mut libc::c_void) -> libc::c_int;
        }
        let mut info: libc::rusage_info_v4 = std::mem::zeroed();
        let ok = proc_pid_rusage(libc::getpid(), libc::RUSAGE_INFO_V4, &mut info as *mut _ as *mut libc::c_void) == 0;
        ok.then_some(info.ri_phys_footprint)
    };
    #[cfg(any(target_os = "android", target_os = "linux"))]
    let used = std::fs::read_to_string("/proc/self/statm").ok().and_then(|s| {
        let pages: u64 = s.split_whitespace().nth(1)?.parse().ok()?;
        Some(pages * unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64)
    });
    #[cfg(not(any(target_vendor = "apple", target_os = "android", target_os = "linux")))]
    let used = None;
    #[cfg(any(target_os = "ios", target_os = "tvos"))]
    let available = {
        unsafe extern "C" {
            fn os_proc_available_memory() -> usize;
        }
        Some(unsafe { os_proc_available_memory() } as u64)
    };
    #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
    let available = None;
    (used, available)
}

/// Physical RAM installed on the device, in bytes, when the OS reports it.
pub fn physical_memory_bytes() -> Option<u64> {
    static BYTES: OnceLock<Option<u64>> = OnceLock::new();
    *BYTES.get_or_init(|| {
        #[cfg(target_vendor = "apple")]
        {
            let mut size: u64 = 0;
            let mut len = std::mem::size_of::<u64>();
            let ok = unsafe {
                libc::sysctlbyname(
                    c"hw.memsize".as_ptr(),
                    (&mut size as *mut u64).cast(),
                    &mut len,
                    std::ptr::null_mut(),
                    0,
                )
            } == 0;
            (ok && size > 0).then_some(size)
        }
        #[cfg(any(target_os = "android", target_os = "linux"))]
        {
            let info = std::fs::read_to_string("/proc/meminfo").ok()?;
            for line in info.lines() {
                if let Some(rest) = line.strip_prefix("MemTotal:")
                    && let Some(kb) = rest.split_whitespace().next().and_then(|s| s.parse::<u64>().ok())
                {
                    return Some(kb << 10);
                }
            }
            None
        }
        #[cfg(not(any(target_vendor = "apple", target_os = "android", target_os = "linux")))]
        {
            None
        }
    })
}

/// Device/process memory tier used to size budgets and safety margins cleanly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryTier {
    /// <= 4 GiB physical RAM or <= 3.6 GiB usable Jetsam limit.
    Compact,
    /// ~6 GiB physical RAM or ~3.6–6.0 GiB Jetsam limit (typical base/Pro iPhones).
    Standard,
    /// >= 8 GiB physical RAM with ample headroom, or desktop hosts.
    High,
}

/// Configurable device-aware memory guard and pool budget profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryProfile {
    pub tier: MemoryTier,
    pub physical_bytes: Option<u64>,
    pub estimated_limit_bytes: Option<u64>,
    pub resource_budget_mb: u32,
    pub guard_reserve_bytes: u64,
    pub guard_band_bytes: u64,
    pub critical_floor_bytes: u64,
    pub tex_pool_budget_bytes: u64,
    pub staging_keep_bytes: u64,
}

impl MemoryProfile {
    /// Builds a profile from known physical RAM and optional process limit
    /// (`phys_footprint + os_proc_available_memory` on iOS/tvOS), applying any
    /// explicit environment overrides.
    pub fn from_sizes(physical_bytes: Option<u64>, process_limit_bytes: Option<u64>, desktop: bool) -> Self {
        let mib = |m: u64| m << 20;
        let effective = match (physical_bytes, process_limit_bytes) {
            (Some(p), Some(l)) => Some(p.min(l)),
            (None, Some(l)) => Some(l),
            (Some(p), None) => Some(p),
            (None, None) => None,
        };
        let mut tier = if let Some(bytes) = effective {
            if bytes <= mib(3700) {
                MemoryTier::Compact
            } else if bytes <= mib(6400) || (desktop && bytes <= mib(7168)) {
                MemoryTier::Standard
            } else {
                MemoryTier::High
            }
        } else if desktop {
            MemoryTier::High
        } else {
            MemoryTier::Standard
        };

        if let Ok(env_tier) = std::env::var("GLUE5_MEMORY_PROFILE") {
            match env_tier.trim().to_ascii_lowercase().as_str() {
                "compact" | "low" => tier = MemoryTier::Compact,
                "standard" | "medium" | "mid" => tier = MemoryTier::Standard,
                "high" => tier = MemoryTier::High,
                _ => {}
            }
        }

        let (mut res_mb, mut reserve_mb, mut band_mb, mut crit_mb, mut tex_mb, staging_mb) = match tier {
            MemoryTier::Compact => (512u32, 384u64, 128u64, 224u64, 640u64, 16u64),
            MemoryTier::Standard => (768, 448, 160, 256, 1024, 24),
            MemoryTier::High => (1024, 512, 192, 288, 1536, 32),
        };

        // Keep reserve + band sane if iOS grants a very tight Jetsam budget (e.g., < 2.2 GiB).
        if let Some(limit) = process_limit_bytes {
            let limit_mb = limit >> 20;
            if limit_mb > 0 && limit_mb <= 2600 {
                res_mb = res_mb.min(448);
                reserve_mb = reserve_mb.min(limit_mb / 6).max(192);
                band_mb = band_mb.min(limit_mb / 12).max(96);
                crit_mb = crit_mb.min(reserve_mb.saturating_sub(64)).max(128);
                tex_mb = tex_mb.min(limit_mb / 4).max(384);
            }
        }

        let env_u64 = |k: &str| std::env::var(k).ok().and_then(|v| v.trim().parse::<u64>().ok());
        if let Some(v) = env_u64("GLUE5_RESOURCE_BUDGET_MB") {
            res_mb = v.clamp(256, 4096) as u32;
        }
        if let Some(v) = env_u64("GLUE5_GUARD_RESERVE_MB") {
            reserve_mb = v.clamp(64, 2048);
        }
        if let Some(v) = env_u64("GLUE5_GUARD_BAND_MB") {
            band_mb = v.clamp(32, 1024);
        }
        if let Some(v) = env_u64("GLUE5_TEX_POOL_BUDGET_MB") {
            tex_mb = v.clamp(128, 8192);
        }

        Self {
            tier,
            physical_bytes,
            estimated_limit_bytes: process_limit_bytes,
            resource_budget_mb: res_mb,
            guard_reserve_bytes: mib(reserve_mb),
            guard_band_bytes: mib(band_mb),
            critical_floor_bytes: mib(crit_mb.min(reserve_mb.saturating_sub(16).max(64))),
            tex_pool_budget_bytes: mib(tex_mb),
            staging_keep_bytes: mib(staging_mb),
        }
    }

    /// Detects the active device/process memory profile cleanly (cached once).
    pub fn detect() -> Self {
        static PROFILE: OnceLock<MemoryProfile> = OnceLock::new();
        *PROFILE.get_or_init(|| {
            let phys = physical_memory_bytes();
            let (used, avail) = footprint();
            let proc_limit = used.zip(avail).map(|(u, a)| u.saturating_add(a));
            let desktop = !cfg!(any(target_os = "ios", target_os = "android", target_os = "tvos"));
            Self::from_sizes(phys, proc_limit, desktop)
        })
    }

    /// Whether tight process/physical memory justifies enabling the low-memory
    /// engine pool cap on mobile devices.
    pub fn recommends_low_memory(self) -> bool {
        !matches!(self.tier, MemoryTier::High)
    }
}

/// GPU/Metal live allocation counters published by `glue5-gpu` for memory guard decisions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuMemoryStats {
    pub textures_bytes: u64,
    pub buffers_bytes: u64,
    pub metal_allocated_bytes: u64,
    pub slabs_bytes: u64,
    pub pending_decode_bytes: u64,
    pub staging_bytes: u64,
    pub tex_pool_budget_bytes: u64,
}

static GPU_TEX_BYTES: AtomicU64 = AtomicU64::new(0);
static GPU_BUF_BYTES: AtomicU64 = AtomicU64::new(0);
static GPU_METAL_BYTES: AtomicU64 = AtomicU64::new(0);
static GPU_SLAB_BYTES: AtomicU64 = AtomicU64::new(0);
static GPU_PENDING_DECODE_BYTES: AtomicU64 = AtomicU64::new(0);
static GPU_STAGING_BYTES: AtomicU64 = AtomicU64::new(0);
static GPU_TEX_BUDGET_BYTES: AtomicU64 = AtomicU64::new(0);
static GPU_TRIM_REQUESTED: AtomicBool = AtomicBool::new(false);
static GUARD_PRESSURE_LEVEL: AtomicU8 = AtomicU8::new(0);
static LOW_MEM_WARNING_UNTIL_MS: AtomicU64 = AtomicU64::new(0);

fn monotonic_ms() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// Publishes live GPU allocation counts from `glue5-gpu` (lock-free).
pub fn update_gpu_stats(stats: GpuMemoryStats) {
    GPU_TEX_BYTES.store(stats.textures_bytes, Ordering::Relaxed);
    GPU_BUF_BYTES.store(stats.buffers_bytes, Ordering::Relaxed);
    GPU_METAL_BYTES.store(stats.metal_allocated_bytes, Ordering::Relaxed);
    GPU_SLAB_BYTES.store(stats.slabs_bytes, Ordering::Relaxed);
    GPU_PENDING_DECODE_BYTES.store(stats.pending_decode_bytes, Ordering::Relaxed);
    GPU_STAGING_BYTES.store(stats.staging_bytes, Ordering::Relaxed);
    GPU_TEX_BUDGET_BYTES.store(stats.tex_pool_budget_bytes, Ordering::Release);
}

/// Reads the most recently published GPU memory stats.
pub fn gpu_stats() -> GpuMemoryStats {
    GpuMemoryStats {
        textures_bytes: GPU_TEX_BYTES.load(Ordering::Relaxed),
        buffers_bytes: GPU_BUF_BYTES.load(Ordering::Relaxed),
        metal_allocated_bytes: GPU_METAL_BYTES.load(Ordering::Relaxed),
        slabs_bytes: GPU_SLAB_BYTES.load(Ordering::Relaxed),
        pending_decode_bytes: GPU_PENDING_DECODE_BYTES.load(Ordering::Relaxed),
        staging_bytes: GPU_STAGING_BYTES.load(Ordering::Relaxed),
        tex_pool_budget_bytes: GPU_TEX_BUDGET_BYTES.load(Ordering::Acquire),
    }
}

/// Requests `glue5-gpu` to trim cached/recyclable GPU buffers, slabs and staging memory.
pub fn request_gpu_trim() {
    GPU_TRIM_REQUESTED.store(true, Ordering::Release);
}

/// Consumes a pending GPU trim request (called from `glue5-gpu`).
pub fn take_gpu_trim_request() -> bool {
    GPU_TRIM_REQUESTED.swap(false, Ordering::AcqRel)
}

/// Records an OS memory warning (`Event::AppLowMemory` / Jetsam warning) so
/// both the runtime memory guard and GPU caches enter recovery mode immediately.
pub fn note_low_memory_warning() {
    let until = monotonic_ms().saturating_add(5_000);
    LOW_MEM_WARNING_UNTIL_MS.store(until, Ordering::Release);
    GUARD_PRESSURE_LEVEL.store(GuardPressure::Critical as u8, Ordering::Release);
    request_gpu_trim();
}

/// Current memory guard pressure tier (`0 = Nominal .. 3 = Critical`).
pub fn pressure_level() -> GuardPressure {
    match GUARD_PRESSURE_LEVEL.load(Ordering::Acquire) {
        1 => GuardPressure::Elevated,
        2 => GuardPressure::High,
        3.. => GuardPressure::Critical,
        _ => GuardPressure::Nominal,
    }
}

/// Hysteresis-based memory pressure tier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum GuardPressure {
    #[default]
    Nominal = 0,
    Elevated = 1,
    High = 2,
    Critical = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuardSample {
    pub proc_used: Option<u64>,
    pub proc_available: Option<u64>,
    pub tex_used: u64,
    pub tex_budget: u64,
    pub low_warning_active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuardDecision {
    pub pressure: GuardPressure,
    pub latched: bool,
    pub block_speculative_grow: bool,
    pub available_cap: Option<u64>,
    pub trigger_trim: bool,
}

/// Stateful hysteresis evaluator inspired by `gta5-ios-port`'s two-threshold guard:
/// latches once free headroom drops below `reserve` (or GPU pool saturates),
/// holds through the `[reserve, reserve + band)` band to avoid eviction/reload thrashing,
/// and applies cooldown nudges if headroom stays constrained continuously.
pub struct MemoryGuard {
    reserve: u64,
    band: u64,
    critical_floor: u64,
    hold_ms: u64,
    cooldown_ms: u64,
    latched: bool,
    latched_since_ms: u64,
    last_nudge_ms: u64,
    last_trim_ms: u64,
    nudge_steps: u32,
}

impl MemoryGuard {
    pub fn from_profile(profile: MemoryProfile) -> Self {
        Self {
            reserve: profile.guard_reserve_bytes,
            band: profile.guard_band_bytes,
            critical_floor: profile.critical_floor_bytes,
            hold_ms: 8_000,
            cooldown_ms: 8_000,
            latched: false,
            latched_since_ms: 0,
            last_nudge_ms: 0,
            last_trim_ms: 0,
            nudge_steps: 0,
        }
    }

    pub fn evaluate(&mut self, sample: GuardSample, now_ms: u64) -> GuardDecision {
        let gpu_budget = sample.tex_budget;
        let gpu_high = gpu_budget > 0 && sample.tex_used >= gpu_budget;
        let gpu_elevated = gpu_budget > 0 && sample.tex_used >= gpu_budget * 85 / 100;
        let gpu_critical = gpu_budget > 0 && sample.tex_used >= gpu_budget * 115 / 100;
        let proc_below_reserve = sample.proc_available.is_some_and(|a| a < self.reserve);
        let proc_below_release = sample.proc_available.is_some_and(|a| a < self.reserve.saturating_add(self.band));
        let proc_critical = sample.proc_available.is_some_and(|a| a < self.critical_floor);

        let should_latch = proc_below_reserve || gpu_high || sample.low_warning_active;
        let can_unlatch = !proc_below_release && !gpu_high && !sample.low_warning_active;

        if !self.latched && should_latch {
            self.latched = true;
            self.latched_since_ms = now_ms;
            self.last_nudge_ms = now_ms;
            self.nudge_steps = 0;
        } else if self.latched && can_unlatch {
            self.latched = false;
            self.nudge_steps = 0;
        }

        let pressure = if proc_critical || gpu_critical || sample.low_warning_active {
            GuardPressure::Critical
        } else if self.latched {
            GuardPressure::High
        } else if gpu_elevated || proc_below_release {
            GuardPressure::Elevated
        } else {
            GuardPressure::Nominal
        };

        let mut trigger_trim = false;
        if pressure >= GuardPressure::High && now_ms.saturating_sub(self.last_trim_ms) >= self.cooldown_ms {
            self.last_trim_ms = now_ms;
            trigger_trim = true;
        }

        if self.latched
            && now_ms.saturating_sub(self.latched_since_ms) >= self.hold_ms
            && now_ms.saturating_sub(self.last_nudge_ms) >= self.cooldown_ms
        {
            self.last_nudge_ms = now_ms;
            self.nudge_steps = self.nudge_steps.saturating_add(1).min(4);
            trigger_trim = true;
        }

        let available_cap = match pressure {
            GuardPressure::Nominal => None,
            GuardPressure::Elevated => Some(96 << 20),
            GuardPressure::High => {
                // Step down gradually over long holds instead of sudden 0 MiB eviction storms.
                let base = 36u64 << 20;
                let step = (self.nudge_steps as u64) * (6u64 << 20);
                Some(base.saturating_sub(step).max(8 << 20))
            }
            GuardPressure::Critical => Some(4 << 20),
        };

        GuardDecision {
            pressure,
            latched: self.latched,
            block_speculative_grow: self.latched || pressure >= GuardPressure::High,
            available_cap,
            trigger_trim,
        }
    }
}

static GUARD_STATE: Mutex<Option<(MemoryGuard, u64, GuardDecision)>> = Mutex::new(None);
static OS_SAMPLE_CACHE: Mutex<(u64, (Option<u64>, Option<u64>))> = Mutex::new((0, (None, None)));

fn cached_footprint(now_ms: u64) -> (Option<u64>, Option<u64>) {
    let mut cache = OS_SAMPLE_CACHE.lock().unwrap();
    if cache.0 == 0 || now_ms.saturating_sub(cache.0) >= 16 {
        cache.0 = now_ms;
        cache.1 = footprint();
    }
    cache.1
}

fn current_guard_decision() -> GuardDecision {
    let now_ms = monotonic_ms();
    let (proc_used, proc_available) = cached_footprint(now_ms);
    let profile = MemoryProfile::detect();
    let gpu = gpu_stats();
    let tex_budget = if gpu.tex_pool_budget_bytes != 0 {
        gpu.tex_pool_budget_bytes
    } else if cfg!(any(target_os = "ios", target_os = "android", target_os = "tvos")) {
        profile.tex_pool_budget_bytes
    } else {
        0
    };
    let sample = GuardSample {
        proc_used,
        proc_available,
        tex_used: gpu.textures_bytes,
        tex_budget,
        low_warning_active: now_ms < LOW_MEM_WARNING_UNTIL_MS.load(Ordering::Acquire),
    };

    let decision = {
        let mut slot = GUARD_STATE.lock().unwrap();
        let (guard, last_eval_ms, prev) = slot.get_or_insert_with(|| {
            (
                MemoryGuard::from_profile(profile),
                0,
                GuardDecision {
                    pressure: GuardPressure::Nominal,
                    latched: false,
                    block_speculative_grow: false,
                    available_cap: None,
                    trigger_trim: false,
                },
            )
        });
        if *last_eval_ms != 0 && now_ms.saturating_sub(*last_eval_ms) < 8 && !sample.low_warning_active {
            *prev
        } else {
            let dec = guard.evaluate(sample, now_ms);
            *last_eval_ms = now_ms;
            *prev = dec;
            dec
        }
    };
    GUARD_PRESSURE_LEVEL.store(decision.pressure as u8, Ordering::Release);
    if decision.trigger_trim {
        request_gpu_trim();
    }
    decision
}

// ---- the engine's resource heap ----------------------------------------------

/// The resource heap's budget, in bytes (0: none).
static RESOURCE_BUDGET: AtomicU64 = AtomicU64::new(0);

thread_local! {
    /// The streaming engine is asking how much memory is left.
    static ASKING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Budgets the resource heap at `mb` MiB (the engine's `gameResourceMB`).
pub(crate) fn budget_resource_heap(mb: u32) {
    RESOURCE_BUDGET.store((mb as u64) << 20, Ordering::Release);
}

/// Hook for `rage::sysMemGrowBuddyAllocator::GrowHeap()`. The PC build's
/// resource heap starts at `gameResourceMB` and grows by up to 256 MiB, for
/// an allocation and also whenever the streaming engine asks how much memory
/// is left (answering grows it), so it holds whatever the player has passed
/// by: on a phone, past what the system allows. With a budget or active memory
/// guard, answering no longer grows it further; real allocations (`!ASKING`)
/// still grow as needed so streaming never fails a mandatory read. -1: no growth.
pub(crate) unsafe extern "C" fn grow_heap(env: *mut Env, this: u64) -> u32 {
    // Heaps come a little short of whole MiB (255, 511, ...): within the
    // smallest growth step of the budget counts as there.
    const STEP: u64 = 16 << 20;
    let budget = RESOURCE_BUDGET.load(Ordering::Acquire);
    if ASKING.get() {
        let over_cap = budget != 0 && unsafe { orig::heap_size(env, this) } + STEP > budget;
        let guarded = current_guard_decision().block_speculative_grow;
        if over_cap || guarded {
            return u32::MAX;
        }
    }
    unsafe { orig::grow_heap(env, this) }
}

/// Effective headroom exposed to the streaming engine from `orig::memory_available`
/// combined with heap budget overrun and hysteresis guard cap.
fn apply_headroom_guard(raw_available: u64, over_budget: u64, guard: GuardDecision) -> u64 {
    let budgeted = raw_available.saturating_sub(over_budget);
    match guard.available_cap {
        Some(cap) => budgeted.min(cap),
        None => budgeted,
    }
}

/// Hook for `rage::sysMemGrowBuddyAllocator::GetMemoryAvailable()`: past
/// the budget (grown by allocations), what lies beyond it does not count as
/// free, and active memory pressure gently caps free headroom so streaming
/// evicts smoothly instead of overflowing device limits.
pub(crate) unsafe extern "C" fn memory_available(env: *mut Env, this: u64) -> u64 {
    // The two ask each other: inner calls are answered as they are.
    if ASKING.get() {
        return unsafe { orig::memory_available(env, this) };
    }
    let available = asking(|| unsafe { orig::memory_available(env, this) });
    let guard = current_guard_decision();
    apply_headroom_guard(available, unsafe { over_budget(env, this) }, guard)
}

/// Hook for `rage::sysMemGrowBuddyAllocator::GetLargestAvailableBlock()`
/// (no larger than what [`memory_available`] says is free).
pub(crate) unsafe extern "C" fn largest_block(env: *mut Env, this: u64) -> u64 {
    if ASKING.get() {
        return unsafe { orig::largest_block(env, this) };
    }
    let largest = asking(|| unsafe { orig::largest_block(env, this) });
    let available = asking(|| unsafe { orig::memory_available(env, this) });
    let guard = current_guard_decision();
    let effective = apply_headroom_guard(available, unsafe { over_budget(env, this) }, guard);
    largest.min(effective)
}

/// How far the heap has grown past the budget, in bytes.
unsafe fn over_budget(env: *mut Env, this: u64) -> u64 {
    match RESOURCE_BUDGET.load(Ordering::Acquire) {
        0 => 0,
        budget => unsafe { orig::heap_size(env, this) }.saturating_sub(budget),
    }
}

fn asking<R>(f: impl FnOnce() -> R) -> R {
    ASKING.set(true);
    let r = f();
    ASKING.set(false);
    r
}

// ---- what the engine's heaps hold, on the 10 s stats -------------------------

/// The engine's heaps (addresses in this build, from `InitGameHeap`): the
/// game heap and the replay's (`sysMemSimpleAllocator`); the resource heap
/// and the one for "physical" memory, little used here
/// (`sysMemGrowBuddyAllocator`s).
const GAME_HEAP: u64 = 8017912;
const REPLAY_HEAP: u64 = 8060376;
const RESOURCE_HEAP: u64 = 8043728;
const PHYSICAL_HEAP: u64 = 8026992;

/// When the heaps were last reported (`Clock` ms).
static REPORTED: AtomicU64 = AtomicU64::new(0);

/// Logs what the engine's heaps hold, every 10 s (from the render thread):
/// how the wasm memory, which never shrinks, is made up.
pub(crate) fn report_heaps(mem: Mem) {
    let now = crate::rt().clock.now_ms() as u64;
    if now < REPORTED.load(Ordering::Relaxed) + 10_000 {
        return;
    }
    REPORTED.store(now, Ordering::Relaxed);
    let mb = |b: u64| b >> 20;
    let (game, replay) = (simple_heap(mem, GAME_HEAP), simple_heap(mem, REPLAY_HEAP));
    let (resources, physical) = (buddy_heap(mem, RESOURCE_HEAP), buddy_heap(mem, PHYSICAL_HEAP));
    let gpu = gpu_stats();
    let profile = MemoryProfile::detect();
    let pressure = pressure_level();
    log::info!(
        "memory: wasm memory {} MiB; game heap {}/{} MiB ({} at most), resource heap {}/{} MiB, physical heap {}/{} MiB, replay heap {}/{} MiB; {} MiB freed given back in all ({} MiB taken back); guard {:?} ({:?}, gpu tex {}/{} MiB)",
        mb(crate::rt().memory.size()),
        mb(game.0),
        mb(game.1),
        mb(game.2),
        mb(resources.0),
        mb(resources.1),
        mb(physical.0),
        mb(physical.1),
        mb(replay.0),
        mb(replay.1),
        mb(GIVEN.load(Ordering::Relaxed)),
        mb(TAKEN_BACK.load(Ordering::Relaxed)),
        pressure,
        profile.tier,
        mb(gpu.textures_bytes),
        mb(if gpu.tex_pool_budget_bytes != 0 { gpu.tex_pool_budget_bytes } else { profile.tex_pool_budget_bytes }),
    );
}

/// A `sysMemSimpleAllocator`'s bytes in use, size and most in use so far
/// (its size less the least it had free).
fn simple_heap(mem: Mem, at: u64) -> (u64, u64, u64) {
    let size = mem.u64(at + 280);
    (mem.u64(at + 296), size, size.saturating_sub(mem.u64(at + 568)))
}

/// A `sysMemGrowBuddyAllocator`'s bytes in use and size.
fn buddy_heap(mem: Mem, at: u64) -> (u64, u64) {
    segments(mem, at).fold((0, 0), |(used, size), s| (used + s.used, size + s.size))
}

/// A segment of a `sysMemGrowBuddyAllocator` (a `sysMemBuddyAllocator`).
struct Segment {
    base: u64,
    size: u64,
    /// Leaves in use times the leaf size.
    used: u64,
}

/// A `sysMemGrowBuddyAllocator`'s segments, in the order it grew.
fn segments(mem: Mem, at: u64) -> impl Iterator<Item = Segment> {
    (0..mem.i32(at + 508).max(0) as u64).filter_map(move |i| {
        let base = mem.u64(at + 16384 + i * 8);
        let segment = at + 512 + i * 488;
        let used = mem.u64(segment + 32) * mem.u64(segment + 56);
        (base != 0).then(|| Segment { base, size: mem.u64(segment + 24), used })
    })
}

// ---- freed resource memory back to the system, past the budget -----------------

/// Hook for `rage::sysMemGrowBuddyAllocator::Free()`. Past the resource
/// heap's budget lie segments grown for allocations: the streaming engine
/// keeps what it uses under the budget, but spread over every segment, so
/// the heap holds hundreds of MiB it has written and freed (the low-memory
/// benchmark freed 450 MiB past the budget, of which a third was taken
/// again). A block freed there goes back to the system, its whole pages,
/// before it is freed (until then it is the caller's alone; the allocator
/// keeps nothing in free blocks). Not inside the budget: that is the cache
/// the streaming engine is meant to keep.
pub(crate) unsafe extern "C" fn buddy_free(env: *mut Env, this: u64, ptr: u64) {
    let budget = RESOURCE_BUDGET.load(Ordering::Acquire);
    let pressure = pressure_level();
    let under_pressure = pressure >= GuardPressure::High;
    if ptr != 0 && ((this == RESOURCE_HEAP && (budget != 0 || under_pressure)) || (this == PHYSICAL_HEAP && under_pressure)) {
        let mem = unsafe { Mem::of(env) };
        let min_pressure_block = if pressure >= GuardPressure::Critical { 16 << 10 } else { 64 << 10 };
        if this == RESOURCE_HEAP {
            // Segments come a little short of whole MiB, as in `grow_heap`.
            let mut start = 0;
            for s in segments(mem, this) {
                if (s.base..s.base + s.size).contains(&ptr) {
                    let past_budget = budget != 0 && start + (16 << 20) > budget;
                    if past_budget || under_pressure {
                        let block_size = unsafe { orig::buddy_block_size(env, this, ptr) };
                        if past_budget || block_size >= min_pressure_block {
                            give_back(mem, ptr, block_size);
                        }
                    }
                    break;
                }
                start += s.size;
            }
        } else {
            let block_size = unsafe { orig::buddy_block_size(env, this, ptr) };
            if block_size >= min_pressure_block {
                give_back(mem, ptr, block_size);
            }
        }
    }
    unsafe { orig::buddy_free(env, this, ptr) }
}

/// Hook for `rage::sysMemGrowBuddyAllocator::TryAllocate()` (Apple): pages
/// given back come back into the process's footprint before the block is
/// handed out, as with the system's malloc.
#[cfg(target_vendor = "apple")]
pub(crate) unsafe extern "C" fn buddy_try_allocate(env: *mut Env, this: u64, size: u64, align: u64, bucket: u32) -> u64 {
    let ptr = unsafe { orig::buddy_try_allocate(env, this, size, align, bucket) };
    if ptr != 0
        && let Some(given) = GIVEN_BACK.get()
    {
        let size = unsafe { orig::buddy_block_size(env, this, ptr) }.max(size);
        let (first, end) = (ptr / page_size(), (ptr + size).div_ceil(page_size()));
        let mut taken = 0;
        for (word, mask) in page_bits(first, end) {
            if given[word].load(Ordering::Acquire) & mask != 0 {
                taken += (given[word].fetch_and(!mask, Ordering::AcqRel) & mask).count_ones() as u64;
            }
        }
        if taken != 0 {
            TAKEN_BACK.fetch_add(taken * page_size(), Ordering::Relaxed);
            let mem = unsafe { Mem::of(env) };
            let len = (end - first) * page_size();
            unsafe { libc::madvise(mem.ptr(first * page_size()).cast(), len as usize, libc::MADV_FREE_REUSE) };
        }
    }
    ptr
}

/// Pages given back (Apple: a bit each, by wasm address), which allocations
/// take back.
#[cfg(target_vendor = "apple")]
static GIVEN_BACK: OnceLock<Box<[AtomicU64]>> = OnceLock::new();

/// Bytes given back, and taken back (Apple), in all.
static GIVEN: AtomicU64 = AtomicU64::new(0);
static TAKEN_BACK: AtomicU64 = AtomicU64::new(0);

/// The bits of pages `[first, end)`: (word, mask) pairs.
#[cfg(target_vendor = "apple")]
fn page_bits(first: u64, end: u64) -> impl Iterator<Item = (usize, u64)> {
    (first / 64..end.div_ceil(64)).map(move |word| {
        let (lo, hi) = (first.max(word * 64) - word * 64, end.min(word * 64 + 64) - word * 64);
        (word as usize, (u64::MAX >> (64 - (hi - lo))) << lo)
    })
}

/// Gives the whole pages of `[at, at + len)` back to the system. Apple:
/// out of the footprint at once, though the pages stay until the system
/// needs them, and taken back by [`buddy_try_allocate`]. Elsewhere (Unix):
/// dropped, zero when touched again.
fn give_back(mem: Mem, at: u64, len: u64) {
    let (first, end) = (at.div_ceil(page_size()), (at + len) / page_size());
    if end <= first || cfg!(not(unix)) {
        return;
    }
    let ptr = mem.ptr(first * page_size()).cast::<std::ffi::c_void>();
    let len = ((end - first) * page_size()) as usize;
    GIVEN.fetch_add(len as u64, Ordering::Relaxed);
    #[cfg(target_vendor = "apple")]
    {
        let words = (crate::rt().memory.max_bytes() / page_size()).div_ceil(64) as usize;
        let given = GIVEN_BACK.get_or_init(|| (0..words).map(|_| AtomicU64::new(0)).collect());
        // Marked first: the block is freed only after this returns.
        for (word, mask) in page_bits(first, end) {
            given[word].fetch_or(mask, Ordering::AcqRel);
        }
        unsafe { libc::madvise(ptr, len, libc::MADV_FREE_REUSABLE) };
    }
    #[cfg(all(unix, not(target_vendor = "apple")))]
    unsafe {
        libc::madvise(ptr, len, libc::MADV_DONTNEED)
    };
    #[cfg(windows)]
    let _ = ptr;
}

/// The system's page size.
fn page_size() -> u64 {
    static SIZE: OnceLock<u64> = OnceLock::new();
    *SIZE.get_or_init(|| {
        #[cfg(unix)]
        let size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;
        #[cfg(windows)]
        let size = 4096;
        size
    })
}

/// Hook for libc++'s `operator_new_impl` (its nothrow `operator new`, which
/// std::stable_sort takes its buffers from: a cutscene's load events, the
/// HUD's markers, past 128 of them): rage's `operator delete` frees them,
/// into the thread's rage heap, which owns no dlmalloc block (it stops the
/// game, or takes the block as its own). Rage's `operator new` makes them,
/// as zlib's default allocator does.
pub(crate) unsafe extern "C" fn nothrow_new(env: *mut Env, size: u64) -> u64 {
    match u32::try_from(size.max(1)) {
        Ok(size) => unsafe { orig::zcalloc(env, 0, 1, size) },
        Err(_) => 0,
    }
}

/// This module's hooks.
pub(crate) fn hooks(hooks: &mut glue5_game::Hooks) {
    hooks.nothrow_new = Some(nothrow_new);
    hooks.grow_heap = Some(grow_heap);
    hooks.memory_available = Some(memory_available);
    hooks.largest_block = Some(largest_block);
    hooks.buddy_free = Some(buddy_free);
    #[cfg(target_vendor = "apple")]
    {
        hooks.buddy_try_allocate = Some(buddy_try_allocate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_vendor = "apple")]
    #[test]
    fn page_bits_cover_the_pages() {
        let bits: Vec<_> = page_bits(3, 130).collect();
        assert_eq!(bits, [(0, u64::MAX << 3), (1, u64::MAX), (2, 0b11)]);
        assert_eq!(page_bits(64, 128).collect::<Vec<_>>(), [(1, u64::MAX)]);
        assert_eq!(page_bits(5, 6).collect::<Vec<_>>(), [(0, 1 << 5)]);
    }

    #[test]
    fn memory_profile_tier_mapping() {
        let gib = |g: u64| g << 30;
        let compact = MemoryProfile::from_sizes(Some(gib(4)), Some(3200 << 20), false);
        assert_eq!(compact.tier, MemoryTier::Compact);
        assert!(compact.recommends_low_memory());

        // 6 GiB physical iPhone whose Jetsam entitlement grants ~3.8 GiB still sits in Standard (low-memory recommended).
        let standard = MemoryProfile::from_sizes(Some(gib(6)), Some(3900 << 20), false);
        assert_eq!(standard.tier, MemoryTier::Standard);
        assert!(standard.recommends_low_memory());

        let high = MemoryProfile::from_sizes(Some(gib(8)), Some(gib(7)), false);
        assert_eq!(high.tier, MemoryTier::High);
        assert!(!high.recommends_low_memory());
    }

    #[test]
    fn memory_guard_hysteresis_and_nudge() {
        let profile = MemoryProfile::from_sizes(Some(6 << 30), Some(4 << 30), false);
        let mut guard = MemoryGuard::from_profile(profile);
        let mib = |m: u64| m << 20;

        // Comfortable headroom stays nominal and unthrottled.
        let dec = guard.evaluate(
            GuardSample {
                proc_used: Some(mib(1500)),
                proc_available: Some(mib(900)),
                tex_used: mib(300),
                tex_budget: mib(1024),
                low_warning_active: false,
            },
            1000,
        );
        assert_eq!(dec.pressure, GuardPressure::Nominal);
        assert!(!dec.latched);
        assert_eq!(dec.available_cap, None);

        // Dropping below reserve (448 MiB) latches high pressure guard & triggers trim.
        let dec = guard.evaluate(
            GuardSample {
                proc_used: Some(mib(2200)),
                proc_available: Some(mib(400)),
                tex_used: mib(500),
                tex_budget: mib(1024),
                low_warning_active: false,
            },
            9_000,
        );
        assert_eq!(dec.pressure, GuardPressure::High);
        assert!(dec.latched);
        assert!(dec.block_speculative_grow);
        assert!(dec.trigger_trim);
        assert_eq!(dec.available_cap, Some(mib(36)));

        // Slight recovery (500 MiB) inside [reserve, reserve + band) keeps guard latched (hysteresis prevents thrashing).
        let dec = guard.evaluate(
            GuardSample {
                proc_used: Some(mib(2100)),
                proc_available: Some(mib(500)),
                tex_used: mib(500),
                tex_budget: mib(1024),
                low_warning_active: false,
            },
            12_000,
        );
        assert!(dec.latched);
        assert_eq!(dec.pressure, GuardPressure::High);

        // Holding > 8s steps nudge down gently instead of stalling forever.
        let dec = guard.evaluate(
            GuardSample {
                proc_used: Some(mib(2150)),
                proc_available: Some(mib(430)),
                tex_used: mib(500),
                tex_budget: mib(1024),
                low_warning_active: false,
            },
            18_000,
        );
        assert_eq!(dec.available_cap, Some(mib(30)));

        // Recovery past reserve + band (448 + 160 = 608 MiB) releases latch cleanly.
        let dec = guard.evaluate(
            GuardSample {
                proc_used: Some(mib(1800)),
                proc_available: Some(mib(700)),
                tex_used: mib(500),
                tex_budget: mib(1024),
                low_warning_active: false,
            },
            20_000,
        );
        assert!(!dec.latched);
        assert_eq!(dec.pressure, GuardPressure::Nominal);
        assert_eq!(dec.available_cap, None);
    }

    #[test]
    fn memory_guard_gpu_pool_pressure_and_warning_window() {
        let profile = MemoryProfile::from_sizes(Some(6 << 30), Some(4 << 30), false);
        let mut guard = MemoryGuard::from_profile(profile);
        let mib = |m: u64| m << 20;

        // GPU texture pool exceeding budget triggers High guard even when proc headroom is unavailable (desktop/unreported).
        let dec = guard.evaluate(
            GuardSample {
                proc_used: None,
                proc_available: None,
                tex_used: mib(1040),
                tex_budget: mib(1024),
                low_warning_active: false,
            },
            10_000,
        );
        assert!(dec.latched);
        assert_eq!(dec.pressure, GuardPressure::High);
        assert_eq!(apply_headroom_guard(mib(200), 0, dec), mib(36));

        // Active OS low-memory warning elevates straight to Critical cap.
        let dec = guard.evaluate(
            GuardSample {
                proc_used: None,
                proc_available: None,
                tex_used: mib(400),
                tex_budget: mib(1024),
                low_warning_active: true,
            },
            11_000,
        );
        assert_eq!(dec.pressure, GuardPressure::Critical);
        assert_eq!(dec.available_cap, Some(mib(4)));
        assert_eq!(apply_headroom_guard(mib(120), 0, dec), mib(4));
    }
}
