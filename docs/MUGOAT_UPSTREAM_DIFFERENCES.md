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

## 4. Verified Root Causes & Planned Fixes per Subsystem

### 1) GPU Recycler Stale-Entry Ordering (`fix/gpu-recycler-order`, commit `30f56ca`)
- **Files / Symbols**: `crates/glue5-gpu/src/recycle.rs` (`Recycler::keep_texture`, `Recycler::texture`, `Recycler::trim`), `crates/glue5-gpu/src/exec.rs`.
- **Finding**: Age queue matched recycled entries by descriptor + destruction epoch; reused handles left stale head entries that could evict live entries sharing the epoch. `30f56ca` introduced unique tickets in `Recycler::trim`.

### 2) Memory Guard, Heap Accounting & Slab Retention (`fix/memory-guard`)
- **Files / Symbols**:
  - `crates/glue5-runtime/src/imports.rs` (`wasm_module_int_js` -> `gameResourceMB`, `budget_resource_heap`)
  - `crates/glue5-runtime/src/memory.rs` (`RESOURCE_BUDGET`, `over_budget`, `memory_available`, `largest_block`, `grow_heap`, `buddy_free`, `give_back`)
  - `crates/glue5-gpu/src/slabs.rs` (`Slabs`)
  - `crates/glue5/src/settings.rs` (`Settings::apply_graphics_preset`, `LOW_MEMORY_RAM_MB`)
- **Root Causes**:
  - `memory_available` / `largest_block` / `grow_heap` only enforce `RESOURCE_BUDGET` (`gameResourceMB`, 768 MiB on `low_memory`), ignoring real iOS process memory availability (`os_proc_available_memory()` / `task_vm_info.phys_footprint`) and live Metal/wgpu GPU pool consumption.
  - No two-threshold hysteresis guard (`reserve` + `band` + cooldown nudge like `gta5-ios-port`'s `NativeMemoryBudget.h`), causing abrupt cliff behavior and thrashing between evictions and allocations.
  - `buddy_free` only returns pages in segments above `budget`, leaving freed blocks inside lower segments un-reclaimed under pressure; `Slabs` (`slabs.rs`) holds 4 MiB chunks indefinitely even when completely empty.
  - `settings.rs` uses raw `sdl3::cpuinfo::system_ram() < 7000`, misclassifying 6 GB iPhones whose usable Jetsam cap is ~3.1–3.8 GB under Medium+ presets.

### 3) Texture Streaming, Cancellation & Eviction/Reload Thrashing (`fix/texture-streaming`)
- **Files / Symbols**: `crates/glue5-gpu/src/exec.rs` (`Exec::create_texture`, `Exec::upload_texture`, `Exec::finish_decodes`, `Exec::destroy`, `Decode`, `Tex`).
- **Root Causes**:
  - `create_texture` drops only `1` mip level statically on `!BC` `>= 512` 2D textures; there is no dynamic 2-mip backoff on large sampled world textures (`>= 512` / `>= 1024`) when memory pressure rises.
  - `destroy` removes `Decode` entries matching `id` from `self.decodes`, but already-enqueued closures on `Pool("gpu decode")` lack a cancellation token and still decode/encode dead textures, burning CPU cores and temporary memory.
  - No in-flight byte budget or priority split between small/lower mip chains and huge top mips, allowing burst uploads to stall visible nearby road/ground mips.
  - Zero telemetry for texture pool used/budget/free bytes, eviction count, reload count, or cache hit rate.

### 4) Texture Compression & CPU Conversion Path (`perf/texture-upload`)
- **Files / Symbols**: `crates/glue5-gpu/src/lib.rs` (`WANTED_FEATURES`), `crates/glue5-gpu/src/format.rs` (`decoded`), `crates/glue5-gpu/src/bc.rs` (`compress`, `decode_etc2`, `BC4`/`BC5`), `crates/glue5-gpu/src/exec.rs` (`Upload::A8`, `Upload::Unorm16`).
- **Root Causes**:
  - `WANTED_FEATURES` omits `wgpu::Features::TEXTURE_COMPRESSION_ASTC` (natively supported across Apple2+ iOS Metal GPUs).
  - On pre-A17 iPhones (`!TEXTURE_COMPRESSION_BC`), `format.rs` maps `BC4 -> R8Unorm` (2x VRAM), `BC5 -> Rg8Unorm` (2x VRAM), and `BC1/2/3/7 -> Etc2Rgba8Unorm[_sRGB]` (2x VRAM for opaque/1-bit `BC1`), inflating VRAM and decode time.
  - `bc.rs` allocates full-image `Vec<u8>` RGBA intermediates (`w * h * 4`), performs full-buffer R/B swizzle, AND spawns nested `std::thread::scope` OS threads per mip inside the 2-worker `Pool("gpu decode")` — creating avoidable allocations and thread contention while `ASTC 4x4` bit-transcoding (for `BC1`/`BC3`) and `EAC R11`/`RG11` (for `BC4`/`BC5`) / strip processing avoid full-image RGBA buffers.

### 5) Shader & Pipeline Compilation, Retry & Target Initialization (`fix/shader-pipeline`)
- **Files / Symbols**: `crates/glue5-gpu/src/draw.rs` (`Draws::prepare`, `Draws::draw`), `crates/glue5-gpu/src/pipeline.rs` (`Cell`), `crates/glue5-gpu/src/prepare.rs`, `crates/glue5-gpu/src/exec.rs` (`Exec::create_texture`).
- **Root Causes**:
  - `draw.rs:558-561` explicitly drops background pipeline compile workers to `Qos::Utility` (efficiency cores on Apple Silicon), stalling unseeded world/road shader compilations while `draw.rs:1164` drops draws unconditionally (`pending_skips`), causing invisible roads and grey buildings while driving.
  - `pipeline::Cell` uses `OnceLock<Option<Pipe>>`, permanently blacklisting any pipeline after a single failure (`"pipeline failed"`) with no bounded retry.
  - Newly created `RENDER_ATTACHMENT` / `DEPTH_STENCIL` / `STORAGE_BINDING` textures in `exec.rs` are not deterministically zero-initialized, exposing stale Metal heap contents or NaNs if early passes are skipped while pipelines compile (same failure class as `gta5-ios-port` `GTA-77`).
  - `pipelines.jsonl` / binary cache loading lacks corrupted-line isolation, versioning checks, and periodic saving of newly encountered recipes mid-session.

### 6) Indirect Draw Validation & Fallback (`fix/indirect-draw-fallback`)
- **Files / Symbols**: `crates/glue5-gpu/src/draw.rs` (`Draws::draw`, `Draws::dispatch`), `crates/glue5-gpu/src/exec.rs` (`Exec::create_buffer`).
- **Root Causes**:
  - `draw.rs:1269-1274` / `1407-1409` submits indirect buffers directly into `wgpu::RenderPass` without validating `BufferUsages::INDIRECT`, 4-byte offset alignment, or `offset + args_bytes <= buf.bytes`, allowing a malformed indirect draw to fail validation for the entire render pass.
  - Devices with `!caps.indirect` discard `kind >= 4` indirect draws outright (`skip("indirect draw (unsupported by the device)")`) rather than reading mirrored/CPU-shadowed draw args.

### 7) Thread Scheduling, GPU Queries & Logging Overhead (`perf/thread-scheduling`)
- **Files / Symbols**: `crates/glue5-gpu/src/exec.rs` (`Exec::idle`), `crates/glue5-runtime/src/gamelog.rs` (`Repeats`), `crates/glue5-runtime/src/memory.rs`.
- **Root Causes**:
  - `exec.rs::idle()` waits up to `IDLE_BATCH` before flushing pending occlusion/query work unless readbacks are waiting, introducing avoidable query completion stalls.
  - `gamelog::Repeats` calls `self.0.clear()` on the entire 50,000-entry map when full, creating periodic burst log spikes and repeated heap churn.

### 8) Thermal Scaling, Frame Pacing & Developer Diagnostics (`perf/frame-pacing`)
- **Files / Symbols**: `crates/glue5/src/app.rs` (`App::watch_thermal`, `App::set_frame_rate`), `crates/glue5-gpu/src/perf.rs` (`Snapshot`, `Live`), `crates/glue5/src/ui/perf.rs` (`PerfHud`, `Figures`).
- **Root Causes**:
  - `App::watch_thermal()` polls `NSProcessInfo.processInfo().thermalState()` every second and only logs transitions; it never steps FPS caps or triggers pressure relief during sustained `.serious` / `.critical` thermals.
  - `perf::Snapshot` & `PerfHud` omit frame-time percentiles (`p50`/`p95`/`p99`), hitch counts, process/engine/Metal memory breakdown, texture pool utilization, eviction/reload counts, decode/upload wait times, `pending_skips`, and thermal state.
