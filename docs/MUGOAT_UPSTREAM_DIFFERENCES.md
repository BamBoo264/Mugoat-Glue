# Mugoat Glue vs. Upstream GLUE 5: Audit, Module Map & Cross-Repository Findings

## 1. Baseline & Lineage

- **Fork repository (`BamBoo264/Mugoat-Glue`)**:
  - Local import baseline commit: `bc3b61c` (`Việt hóa Mugoat Glue cho iOS`)
  - `main` HEAD prior to subsystem work: `45a37c4` (`Chạy test 5 crate cốt lõi trước khi build IPA trên CI`)
  - First fix commit (`fix/gpu-recycler-order`): `30f56ca` (`Fix GPU recycler ordering for reused resources`)
- **Upstream reference (`mempage4k/glue5`)**:
  - Audited against commit `76e6770` (`Merge branch 'next'`).
- **Additional study repositories**:
  - `kevinerikjs/gta5-ios-port` (studied at `9357c18`, v1.3.2–v1.5.2 changelog, memory budget, streaming health, mesh release, BC->ETC/ASTC texture paths, Metal renderer).
  - `andrewnakas/gtav-recomp` (studied at `2a76184`, shader/pipeline warmup, failed retry, zero-initialized targets, GPU resource lifetime).
  - `kalki-kgp/gtav-native`: `https://api.github.com/repos/kalki-kgp/gtav-native` returned HTTP **404 Not Found** at study time (unavailable publicly; ARM64 memory ordering and locking patterns were audited directly in `Mugoat-Glue` instead).

> [!IMPORTANT]
> Comparing `mempage4k/glue5@76e6770` directly against `Mugoat-Glue@45a37c4` shows that `glue5-game`, `patches/`, `Cargo.toml`, `Cargo.lock`, and `glue5-gpu` were **100% identical to upstream** until `30f56ca` (`crates/glue5-gpu/src/recycle.rs`). All earlier fork changes lived strictly in UI/localization, iOS entitlements/metadata, graphics preset wiring, touch customization, and CI workflow steps.

---

## 2. Dependency & Workspace Module Map

Workspace root `Cargo.toml` (Rust edition `2024`, MSRV `1.90`):

```text
crates/glue5          (Launcher, egui UI, SDL3 window, touch controls, iOS lifecycle, PerfHud)
  ├── crates/glue5-runtime   (WASM memory/buddy heap, engine hooks runtime, threads, waits, game settings XML)
  │     └── crates/glue5-game (Builds game.wasm via wasm2c + clang/cc + ARM64 ordering patch + RAGE symbol hooks)
  ├── crates/glue5-gpu       (Command ring consumer, wgpu/Metal backend, BC/ETC/ASTC transcode, pipelines, Perf)
  ├── crates/glue5-mods      (Luau mod runtime, native DB bindings, Studio server)
  ├── crates/glue5-stream    (HTTP/cache streaming client for network game data)
  └── crates/glue5-qos       (Apple QoS / thread priority bindings)

Third-party patched crates (prepared by `.mise/tasks/setup` from `patches/`):
  - third_party/wgpu         (30.0.1 + patches/wgpu)
  - third_party/wgpu-core    (30.0.1 + patches/wgpu-core, snatch locks)
  - third_party/wgpu-hal     (30.0.1 + patches/wgpu-hal, Metal command buffer completion & Adreno fixes)
  - third_party/sp-fidelity  (0.2.0 + patches/sp-fidelity)
```

---

## 3. Existing Fork Differences (Before Subsystem Fixes)

### A. Build, Entitlements & iOS Packaging
- `platform/ios/project.yml`:
  - Deployment target: iOS `16.4`, `ARCHS: arm64`, `TARGETED_DEVICE_FAMILY: "1,2"`.
  - App identity: `CFBundleDisplayName: Mugoat Glue`, `CFBundleName: Mugoat Glue`, `CFBundleDevelopmentRegion: vi`, `CFBundleLocalizations: [vi]`.
  - File sharing & frame pacing flags: `CADisableMinimumFrameDurationOnPhone: true`, `UIFileSharingEnabled: true`, `LSSupportsOpeningDocumentsInPlace: true`, `GCSupportsGameMode: true`, `LSSupportsGameMode: true`.
- `platform/ios/Glue5.entitlements`:
  - Adds `com.apple.developer.kernel.increased-memory-limit = true` and `com.apple.developer.kernel.extended-virtual-addressing = true`.
  - Note: Entitlements permit iOS to grant higher Jetsam limits when available on supported devices, but do not raise or guarantee pool limits on lower-RAM/free-cert devices; runtime memory budgeting must query actual process headroom (`os_proc_available_memory` / physical footprint).
- `.github/workflows/build.yml` & `.mise/tasks/ios/ipa`:
  - Added `Test` step before `Build` running `cargo test -p glue5-game -p glue5-gpu -p glue5-mods -p glue5-runtime -p glue5 --lib`.
  - Unsigned IPA renamed to `Mugoat-Glue-ios.ipa` (`codesign --force --sign - --entitlements platform/ios/Glue5.entitlements --generate-entitlement-der`).

### B. Launcher, Graphics Presets & Controls
- `crates/glue5/src/paths.rs`:
  - Added `prepare_documents()` on iOS to create `Documents/game/` and `HuongDan_ChepGame.txt` so the iOS Files app exposes `On My iPhone > Mugoat Glue > game` immediately on first launch.
- `crates/glue5-runtime/src/game_settings.rs`:
  - Added `GraphicsPreset` (`UltraLow`, `Low`, `Medium`, `High`, `Ultra`, `Custom`) and `CustomGraphics`, mapping presets to GTA V `settings.xml`.
- `crates/glue5/src/settings.rs`:
  - Added `graphics_preset` (default `UltraLow`), `custom_graphics`, `graphics_applied`, `touch_buttons: BTreeMap<String, ButtonOverride>`, `apply_graphics_preset()`, and `recommended_scale()` (mobile default line heights: `480` UltraLow/Custom, `576` Low, `720` Medium, `900` High, native `1.0` Ultra).
  - Issue noted in `apply_graphics_preset()` (`settings.rs:373-374`): uses raw `sdl3::cpuinfo::system_ram() < 7000` (`LOW_MEMORY_RAM_MB`) alongside preset name rather than actual iOS process Jetsam headroom.
- `crates/glue5/src/ui/pages.rs`, `touch.rs`, `touch_editor.rs`, `theme.rs`, `widgets.rs`, and `mods/glue-trainer/**`:
  - Complete Vietnamese localization, redesigned dark theme & icon assets (`platform/shared/Glue5.icon`), `ĐỒ HOẠ` settings tab, and interactive `TouchEditor`.

### C. Upstream Capabilities Already Present in GLUE 5 Base
Before applying Mugoat Glue fixes, upstream GLUE 5 (`76e6770`) already implements:
- **ARM64 memory ordering**: `crates/glue5-game/build/ordering.rs` patches wasm2c loads/stores for Apple Silicon TSO-vs-weak ordering around game physics/locks.
- **Buddy heap & `MADV_FREE_REUSABLE`**: `crates/glue5-runtime/src/memory.rs` hooks `grow_heap`, `memory_available`, `largest_block`, and `buddy_free`, plus virtual reserve fallback when reserves fail.
- **Pre-seeded shader pipeline warm-up**: `crates/glue5-gpu/src/prepare.rs` + `seed/pipelines.jsonl` (2375 pipeline recipes) + wgpu `PipelineCache`.
- **4 MiB buffer slabs & Upload staging chunks**: `crates/glue5-gpu/src/slabs.rs`, `crates/glue5-gpu/src/uploads.rs`.
- **MetalFX upscaling & `presentDrawable:afterMinimumDuration:`**: `crates/glue5-gpu/src/metalfx.rs`, `present.rs`.
- **Frame pacing & Audio backpressure**: `crates/glue5-runtime/src/frame_pace.rs`, `audio_update.rs`, `waits.rs`.

---

## 4. Verified Root Causes & Implemented Fixes per Subsystem

### 1) GPU Recycler Stale-Entry Ordering (`fix/gpu-recycler-order`, commit `30f56ca`)
- **Files / Symbols**: `crates/glue5-gpu/src/recycle.rs` (`Recycler::keep_texture`, `Recycler::texture`, `Recycler::trim`), `crates/glue5-gpu/src/exec.rs`.
- **Root Cause & Fix**: Age queue matched recycled entries by descriptor + destruction epoch; reused handles left stale head records that could evict live entries sharing the epoch. Introduced unique monotonically increasing tickets checked by `Recycler::trim`.

### 2) Memory Guard, Heap Accounting & Slab Retention (`fix/memory-guard`)
- **Files / Symbols**:
  - `crates/glue5-runtime/src/imports.rs` (`wasm_module_int_js` -> `gameResourceMB`, `budget_resource_heap`)
  - `crates/glue5-runtime/src/memory.rs` (`MemoryProfile`, `MemoryGuard`, `GpuMemoryStats`, `memory_available`, `largest_block`, `grow_heap`, `buddy_free`, `give_back`, `note_low_memory_warning`)
  - `crates/glue5-gpu/src/slabs.rs` (`SlabPool`, `Slabs::reclaim_empty`), `crates/glue5-gpu/src/uploads.rs` (`staging_keep_limit`), `crates/glue5-gpu/src/exec.rs` (`Exec::trim`, `Exec::sync_memory_telemetry`), `crates/glue5-gpu/src/present.rs`, `crates/glue5/src/settings.rs`, `crates/glue5/src/app.rs`
- **Root Causes & Fixes**:
  - Original guard only enforced virtual `RESOURCE_BUDGET` without checking live iOS `os_proc_available_memory()`, Metal allocation size, or GPU texture pool occupancy, and lacked hysteresis (causing eviction/allocation oscillation).
  - Implemented `MemoryProfile::detect()` (physical RAM via `sysctlbyname` / `/proc/meminfo` + live process limit + env overrides `GLUE5_MEMORY_PROFILE` / `GLUE5_RESOURCE_BUDGET_MB` / `GLUE5_GUARD_RESERVE_MB` / `GLUE5_GUARD_BAND_MB` / `GLUE5_TEX_POOL_BUDGET_MB`), two-threshold hysteresis `MemoryGuard` (`reserve` + `band` + hold nudge + trim request), and lock-free `GpuMemoryStats` telemetry bridge.
  - Extended `buddy_free` to return multi-page blocks inside `RESOURCE_HEAP` and `PHYSICAL_HEAP` via `MADV_FREE_REUSABLE` during elevated/critical pressure while keeping nominal path fast.
  - Added per-chunk `active_slots` tracking in `Slabs` so completely empty finished 4 MiB chunks are released in `Exec::trim()`.
  - Updated `Settings::apply_graphics_preset` to check `MemoryProfile::detect().recommends_low_memory()` alongside raw RAM, and wired `Event::AppLowMemory` to `note_low_memory_warning()`.

### 3) Texture Streaming, Cancellation & Eviction/Reload Thrashing (`fix/texture-streaming`)
- **Files / Symbols**: `crates/glue5-gpu/src/exec.rs` (`Exec::create_texture`, `compute_mip_drop`, `Exec::upload_texture`, `Exec::finish_decodes`, `Exec::destroy`, `Decode`), `crates/glue5-gpu/src/recycle.rs` (`ReloadDetector`, `TexSignature`, `effective_max_bytes`).
- **Root Causes & Fixes**:
  - `create_texture` statically dropped only `1` mip level on `!BC` `>= 512` 2D textures; `destroy()` removed `Decode` from `self.decodes` without cancelling in-flight decode work, and all decodes shared one FIFO queue (`urgent`) without separating large top mips from small/urgent mips.
  - Added cooperative cancellation (`Arc<AtomicBool>`), split small/lower mips (`urgent`) vs large top mips (`background`), implemented dynamic `compute_mip_drop()` (up to 2 mips on large sampled world textures under `Compact` tier / pressure while preserving UI `< 512` and all render/depth/UAV targets), bounded `create_texture` failure recovery via recycler flush + 1-step mip backoff, and `ReloadDetector` reload/eviction/hit-rate tracking.

### 4) Texture Compression & CPU Conversion Path (`perf/texture-upload`)
- **Files / Symbols**: `crates/glue5-gpu/src/lib.rs` (`WANTED_FEATURES`), `crates/glue5-gpu/src/format.rs` (`decoded`, `TexFormat::uncompressed`), `crates/glue5-gpu/src/bc.rs` (`transcode_astc`, `decode_to_etc2_rgba`, `decode_cancellable`), `crates/glue5-gpu/src/exec.rs` (`Upload::Bc`, `Upload::A8`, `Upload::Unorm16`).
- **Root Causes & Fixes**:
  - Requested `wgpu::Features::TEXTURE_COMPRESSION_ASTC` (with `GLUE5_NO_ASTC` debug toggle).
  - Implemented zero-intermediate direct block bit-transcoding `BC1 -> ASTC 4x4` and `BC3 -> ASTC 4x4` when ASTC is available (`bc1_block_to_astc`, `bc3_block_to_astc`), eliminating multi-MB RGBA intermediates, R/B swizzle passes, and encoder searches.
  - Replaced full-image RGBA buffering and nested `std::thread::scope` thread churn for remaining `BC2`/`BC7` ETC2 targets with strip-by-strip decoding/compression (`decode_to_etc2_rgba`), and streamlined `Upload::A8` and `Upload::Unorm16` row writes.

### 5) Shader & Pipeline Compilation, Retry & Target Initialization (`fix/shader-pipeline`)
- **Files / Symbols**: `crates/glue5-gpu/src/draw.rs` (`Draws::pipeline_for`, `Draws::draw`), `crates/glue5-gpu/src/pipeline.rs` (`Cell`, `CellPoll`, `parse_known_lines`), `crates/glue5-gpu/src/prepare.rs`, `crates/glue5-gpu/src/exec.rs` (`Exec::create_texture`).
- **Root Causes & Fixes**:
  - Removed `Qos::Utility` worker downgrade in `Draws::pipeline_for` so background pipeline compiles retain `Qos::Initiated` and finish promptly without leaving invisible roads/buildings during traversal.
  - Replaced permanent one-shot `OnceLock<Option<Pipe>>` failure blacklisting with `pipeline::Cell` (`OnceLock<Pipe>` fast path + up to `MAX_PIPELINE_ATTEMPTS = 3` bounded retries with epoch backoff) and added `pending`, `hits`, `misses`, `retries` counters.
  - Queued deterministic zero clears (`Color::TRANSPARENT` / `depth = 1.0` / `stencil = 0`) on newly created/recycled 2D `RENDER_ATTACHMENT` textures in `create_texture` (merged seamlessly by `ensure_pass` / `CLEAR_RT`), preventing stale Metal heap contents or NaNs when early passes are skipped while pipelines compile.
  - Added corrupt-line counting/isolation in `known_recipes`.

### 6) Indirect Draw Validation & Fallback (`fix/indirect-draw-fallback`)
- **Files / Symbols**: `crates/glue5-gpu/src/draw.rs` (`validate_indirect_args`, `parse_indirect_args`, `Draws::draw`), `crates/glue5-gpu/src/exec.rs` (`Exec::create_buffer`).
- **Root Causes & Fixes**:
  - Validated `BufferUsages::INDIRECT`, 4-byte offset alignment, and buffer bounds (`16` bytes non-indexed / `20` bytes indexed) plus index alignment before recording indirect draws, preventing malformed draws from poisoning the entire Metal command encoder.
  - Retained CPU `shadow` copies on `MISC_DRAWINDIRECT_ARGS` buffers when `!caps.indirect` and added direct `parse_indirect_args` fallback execution instead of discarding indirect geometry.

### 7) Thread Scheduling, GPU Queries & Logging Overhead (`perf/thread-scheduling`)
- **Files / Symbols**: `crates/glue5-gpu/src/exec.rs` (`Exec::idle`), `crates/glue5-runtime/src/gamelog.rs` (`Repeats`), `crates/glue5-gpu/src/lib.rs` (`log_rate_limited`).
- **Root Causes & Fixes**:
  - Flushed pending GPU queries (`occlusion.pending` / `Work::Query` / `Work::Store`) immediately as `"idle (query)"` when the command ring drains empty instead of waiting out `IDLE_BATCH`.
  - Replaced whole-map `self.0.clear()` in `gamelog::Repeats` with a 2-generation rotating map preserving hot spam suppression counts across generation boundaries.

### 8) Thermal Scaling, Frame Pacing & Developer Diagnostics (`perf/frame-pacing`)
- **Files / Symbols**: `crates/glue5-runtime/src/frame_pace.rs` (`ThermalPolicy`, `set_thermal_cap`), `crates/glue5/src/app.rs` (`App::watch_thermal`, `App::effective_max_fps`), `crates/glue5-gpu/src/perf.rs` (`Diagnostics`, `Snapshot::percentiles`), `crates/glue5-gpu/src/present.rs`, `crates/glue5/src/ui/perf.rs` (`PerfHud`).
- **Root Causes & Fixes**:
  - Upgraded `App::watch_thermal()` with hysteresis `ThermalPolicy` (`Serious >= 3s -> 40 FPS`, `Critical -> 30 FPS + trim()`, 15s cooldown recovery) feeding both `Presentation::set_frame_rate` and host `frame_pace` without mutating user settings.
  - Extended `OverlayDetail::Full` in `PerfHud` with `p50/p95/p99` & hitch counts, texture pool occupancy/hit%/reloads, decode wait/staging MB, pending/skipped/failed pipelines, thermal tier, memory pressure tier, and Metal allocated MB.

---

## 5. Texture Format Support & Upload Conversion Matrix

| Source (DXGI / Game) | Native BC Devices (`TEXTURE_COMPRESSION_BC`, A17 Pro+) | `!BC` + ASTC + ETC2 Devices (iPhone 11–15, Metal) | `!BC` + ETC2 Only (`!ASTC`) | Fallback (`!BC`, `!ETC2`, `!ASTC`, 3D, or `% 4 != 0`) | Memory Multiplier (`!BC` ASTC/ETC2 vs BC) | CPU Decode / Transcode Cost |
|---|---|---|---|---|---|---|
| `BC1` / `BC1_sRGB` (0.5 BPP) | `Bc1RgbaUnorm[_sRGB]` (`Upload::Direct`) | `Astc { B4x4, Unorm[_sRGB] }` (`transcode_astc`) | `Etc2Rgba8Unorm[_sRGB]` (strip) | `Rgba8Unorm[_sRGB]` | **2.0x** (16 B/blk vs 8 B/blk); **1.0x** on native BC; mip backoff saves 4x–16x on `>= 512` | Very low (`bc1_block_to_astc`, zero RGBA buffer) |
| `BC2` / `BC2_sRGB` (1.0 BPP) | `Bc2RgbaUnorm[_sRGB]` (`Upload::Direct`) | `Etc2Rgba8Unorm[_sRGB]` (`decode_to_etc2_rgba`) | `Etc2Rgba8Unorm[_sRGB]` (strip) | `Rgba8Unorm[_sRGB]` | **1.0x** (16 B/blk) | Medium (4-row L1 strip decode + compress) |
| `BC3` / `BC3_sRGB` (1.0 BPP) | `Bc3RgbaUnorm[_sRGB]` (`Upload::Direct`) | `Astc { B4x4, Unorm[_sRGB] }` (`transcode_astc`) | `Etc2Rgba8Unorm[_sRGB]` (strip) | `Rgba8Unorm[_sRGB]` | **1.0x** (16 B/blk) | Low (`bc3_block_to_astc` fast flat path, zero RGBA buffer) |
| `BC4` (0.5 BPP) | `Bc4RUnorm` (`Upload::Direct`) | `R8Unorm` (`decode_cancellable`) | `R8Unorm` | `R8Unorm` | **2.0x** (lossless normal/height masks) | Low (4x4 BC4 decode, cancellable) |
| `BC5` (1.0 BPP) | `Bc5RgUnorm` (`Upload::Direct`) | `Rg8Unorm` (`decode_cancellable`) | `Rg8Unorm` | `Rg8Unorm` | **2.0x** (lossless tangent normals) | Low (BC5 decode, cancellable) |
| `BC6H` (1.0 BPP HDR) | `Bc6hRgbUfloat` (`Upload::Direct`) | `Rgba16Float` (`decode_cancellable`) | `Rgba16Float` | `Rgba16Float` | **8.0x** (float HDR) | Medium (`bc6h_half`, cancellable) |
| `BC7` / `BC7_sRGB` (1.0 BPP) | `Bc7RgbaUnorm[_sRGB]` (`Upload::Direct`) | `Etc2Rgba8Unorm[_sRGB]` (`decode_to_etc2_rgba`) | `Etc2Rgba8Unorm[_sRGB]` (strip) | `Rgba8Unorm[_sRGB]` | **1.0x** (16 B/blk) | Medium-High (4-row strip buffer, no nested threads) |
| `A8` / `Unorm16` | Direct or expanded row conversion (`Upload::A8` / `Unorm16`) | `Rgba8Unorm` / `Rgba16Float` (or `16BIT_NORM`) | Same | Same | 4x (`A8`) / 1x (`Unorm16`) | Low (pre-sized slice loops) |

