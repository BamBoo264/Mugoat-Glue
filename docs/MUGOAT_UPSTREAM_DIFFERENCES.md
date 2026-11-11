# Mugoat Glue upstream differences

The local history begins at import commit `bc3b61c`, which already contains localization. The following comparisons use that import as a baseline. It is not a verified upstream SHA.

- `glue5` owns the launcher, presets, controls, and app lifecycle. The fork adds Vietnamese UI, touch editing, and graphics presets.
- `glue5-runtime` owns WASM memory, threads, and game settings. The fork writes graphics selections to the engine settings XML. Preset effects need to be compared at identical settings.
- `glue5-game` generates the game runtime and supplies engine patches. `glue5-gpu` translates rendering commands to wgpu and owns texture creation, uploads, pipelines, and GPU submission. The local Git comparison shows no changes in `glue5-gpu` between the import and baseline `45a37c4`.
- iOS packaging uses `platform/ios` and `.github/workflows/build.yml`. The fork adds memory entitlements and CI crate tests. An entitlement request does not establish a process limit.
- Patched wgpu and sp-fidelity are prepared by `.mise/tasks/setup`. The Cargo manifest uses those local dependency paths.

## Confirmed source issue

`glue5-gpu/src/exec.rs` hands destroyed textures to `Recycler::keep_texture`. Creation may take one back via `Recycler::texture`. The former age queue matched entries by descriptor and destruction epoch. A reused object left a stale record that could match another object of the same descriptor destroyed in the same epoch. `Recycler::trim` now checks a unique ticket, removing stale head records without evicting live entries that happen to share the epoch.

This cache holds GPU allocations the engine already destroyed. It is distinct from the engine texture pool. The code fix does not establish the cause of grey roads or missing geometry.

## Runtime verification still needed

Keep device, game data, preset, resolution and FPS cap fixed. Record static city, walking, normal driving, fast driving, entering downtown and repeating the same route. Run the fast route for 30 minutes. Capture visual failures alongside frame-time p50/p95/p99, process footprint, Metal bytes, live texture bytes, decode waits, and pipeline-pending/failed draw counts. Existing GPU statistics already provide several of these measurements. Actual engine pool occupancy, eviction and reload rate need engine-side instrumentation before attributing the failure to texture pressure.

Reference projects: [GLUE 5](https://github.com/mempage4k/glue5), [GTA V iOS Port](https://github.com/kevinerikjs/gta5-ios-port), [GTA V Native](https://github.com/kalki-kgp/gtav-native), [GTAV Recomp](https://github.com/andrewnakas/gtav-recomp).
