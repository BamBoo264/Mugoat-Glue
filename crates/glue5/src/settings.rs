//! The application's own settings, in `glue5.json` in the user directory.
//! Graphics quality is the game's (its menu, Settings › Graphics, saved in
//! its `settings.xml`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use glue5_runtime::game_settings::{CustomGraphics, GraphicsPreset};
use serde::{Deserialize, Serialize};

use crate::paths::{MOBILE, TV};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The game directory, once found or chosen.
    pub game_dir: Option<PathBuf>,
    /// Where the game's files come from: a folder on this device, or a
    /// server (another device sharing its folder, or glue5-serve).
    pub source: Source,
    /// The server: `host` or `host:port`.
    pub server: String,
    /// This computer shares its game folder on the network (desktop).
    pub share: bool,
    /// The whole game comes from the server to this device (in its game
    /// folder), to play without it.
    pub download_all: bool,
    /// Smaller engine pools, and the game's graphics settings lowered once
    /// (the browser build's `?low=1`).
    pub low_memory: bool,
    /// Whether the low-memory preset is in the game's settings (written at
    /// the first start with `low_memory`; the game's menu changes it after).
    pub low_preset_written: bool,
    /// Mức cấu hình đồ hoạ được chọn từ launcher (`Cực Thấp` mặc định khi mới cài).
    pub graphics_preset: GraphicsPreset,
    /// Các thông số đồ hoạ chi tiết khi chọn `Tuỳ Chỉnh` (hoặc giá trị tương ứng mức preset).
    pub custom_graphics: CustomGraphics,
    /// Đã ghi cấu hình đồ hoạ hiện tại xuống `settings.xml` hay chưa.
    pub graphics_applied: bool,
    /// The game's resolution as a fraction of the window's, per side (the
    /// back buffer follows the window). Chosen for the screen at the first
    /// start ([`Settings::for_screen`]).
    pub render_scale: Option<f32>,
    pub vsync: bool,
    /// At most this many frames per second; 0: no limit of the engine's
    /// (the display's refresh rate with VSync).
    pub max_fps: u32,
    /// How the game's frames are scaled up to the window (what the GPU has
    /// of it; the bicubic filter otherwise).
    pub upscaler: Upscaling,
    /// How much FSR sharpens its image, 0 to 1.
    pub sharpness: f32,
    /// A frame made between each two of the game's (MetalFX Temporal, on
    /// macOS and iOS 26): twice the frames shown, half a frame more of
    /// latency.
    pub frame_generation: bool,
    /// The GPU driver the user added that the renderer opens at launch
    /// (Android, Adreno GPUs: drivers.rs); none: the phone's own.
    pub gpu_driver: Option<String>,
    /// The performance overlay over the game, and how the application's
    /// own shows.
    pub overlay: PerfOverlay,
    pub overlay_detail: OverlayDetail,
    pub overlay_corner: Corner,
    pub touch: TouchMode,
    pub touch_opacity: f32,
    pub touch_size: f32,
    /// How fast a finger's drag turns the camera.
    pub touch_camera: f32,
    /// The aim assist while playing by touch (the game's Targeting Mode,
    /// in place of its menu's).
    pub touch_aim: TouchAim,
    /// The Taptic Engine taps under pressed touch controls and plays the game's rumble (iPhone).
    pub haptics: bool,
    /// Tuỳ chỉnh vị trí, kích cỡ và độ mờ riêng cho từng nút cảm ứng (theo mã nút).
    pub touch_buttons: BTreeMap<String, ButtonOverride>,
    /// The game starts in the sandbox (the map, no story) rather than the
    /// story: the start page's last choice.
    pub sandbox: bool,
    /// GTA Online was the start page's last choice.
    pub online: bool,
    /// The player's name online (made up when GTA Online is first opened).
    pub online_name: String,
    /// Friends' notifications show (Social Club's "Hide Notifications").
    pub friend_notices: bool,
    /// The mods in the user directory's `mods` folder can run (glue5-mods).
    pub mods: bool,
    /// With mods, the game serves GLUE 5 Studio.
    pub studio: bool,
    /// Other devices on the network may open the Studio (with its code).
    pub studio_lan: bool,
    /// The Studio's pairing code: kept, so that a browser paired once stays
    /// paired when the game starts again (the Mods page makes a new one).
    pub studio_code: String,
    /// Settings saved before the render scale: the back buffer's height.
    #[serde(skip_serializing)]
    render_height: Option<u32>,
    /// Settings saved before the choice of upscalers: MetalFX's spatial one
    /// on or off.
    #[serde(skip_serializing)]
    metalfx: Option<bool>,
    /// Settings saved before the choice of overlays: Apple's HUD on or off.
    #[serde(skip_serializing)]
    metal_hud: Option<bool>,
}

/// Where the game's files come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    Folder,
    Server,
}

/// The performance overlay over the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PerfOverlay {
    Off,
    /// Apple's, drawn by Metal (shown from the launch it is on at).
    MetalHud,
    /// The application's (ui/perf.rs), on every platform.
    Glue5,
}

/// How much the application's overlay shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverlayDetail {
    /// One line: frame rate, frame time, GPU time.
    Minimal,
    /// The frame times as a graph, and where they go.
    Standard,
    /// The busiest threads, the renderer's counts, resolution and memory too.
    Full,
}

/// A corner of the safe area.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// How the game's frames are scaled up to the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Upscaling {
    Bicubic,
    /// MetalFX's spatial upscaler (Apple).
    #[serde(rename = "metalfx")]
    MetalFx,
    /// MetalFX's temporal upscaler (Apple).
    #[serde(rename = "metalfx-temporal")]
    MetalFxTemporal,
    /// AMD's FSR 2 upscaler (temporal, on GPUs with the features it needs).
    Fsr,
}

impl Upscaling {
    pub fn upscaler(self) -> glue5_gpu::Upscaler {
        match self {
            Upscaling::Bicubic => glue5_gpu::Upscaler::Bicubic,
            Upscaling::MetalFx => glue5_gpu::Upscaler::MetalFx,
            Upscaling::MetalFxTemporal => glue5_gpu::Upscaler::MetalFxTemporal,
            Upscaling::Fsr => glue5_gpu::Upscaler::Fsr,
        }
    }
}

/// When the on-screen controls show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TouchMode {
    /// On touch screens, while no controller (or keyboard) is in use.
    Auto,
    Always,
    Off,
}

/// The aim assist while playing by touch: one of the game's targeting modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TouchAim {
    /// Aiming locks on to the nearest target and follows it (Assisted Aim -
    /// Full).
    LockOn,
    /// Aiming snaps to the target, then the finger aims (Assisted Aim -
    /// Partial).
    Snap,
    /// The camera slows down over targets (Free Aim - Assisted).
    SlowDown,
    /// Free Aim.
    Off,
}

impl TouchAim {
    pub fn targeting(self) -> glue5_runtime::input::Targeting {
        use glue5_runtime::input::Targeting;
        match self {
            TouchAim::LockOn => Targeting::Full,
            TouchAim::Snap => Targeting::Partial,
            TouchAim::SlowDown => Targeting::FreeAssisted,
            TouchAim::Off => Targeting::Free,
        }
    }
}

/// Phones and tablets with less RAM start with the low-memory profile (the
/// game takes about 4 GB at the normal one).
const LOW_MEMORY_RAM_MB: i32 = 7000;

/// The render scale's range, in steps of 5%.
pub const RENDER_SCALES: std::ops::RangeInclusive<f32> = 0.25..=1.0;

/// Tuỳ chỉnh riêng cho từng nút thao tác cảm ứng.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ButtonOverride {
    /// Độ lệch so với vị trí gốc (tính theo tỉ lệ rộng/cao của vùng an toàn).
    pub offset: [f32; 2],
    /// Hệ số kích cỡ (`0.5..=2.0`), nhân với kích cỡ nút chung.
    pub size: f32,
    /// Hệ số độ mờ (`0.15..=1.0`), nhân với độ mờ nút chung.
    pub opacity: f32,
}

impl Default for ButtonOverride {
    fn default() -> Self {
        Self { offset: [0.0, 0.0], size: 1.0, opacity: 1.0 }
    }
}

impl ButtonOverride {
    pub fn clamped(mut self) -> Self {
        self.offset[0] = self.offset[0].clamp(-0.95, 0.95);
        self.offset[1] = self.offset[1].clamp(-0.95, 0.95);
        self.size = self.size.clamp(0.5, 2.0);
        self.opacity = self.opacity.clamp(0.15, 1.0);
        self
    }

    pub fn is_default(&self) -> bool {
        self.offset[0].abs() < 1e-4
            && self.offset[1].abs() < 1e-4
            && (self.size - 1.0).abs() < 1e-4
            && (self.opacity - 1.0).abs() < 1e-4
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            game_dir: None,
            source: if TV { Source::Server } else { Source::Folder },
            server: String::new(),
            share: false,
            download_all: false,
            low_memory: MOBILE,
            low_preset_written: false,
            graphics_preset: GraphicsPreset::UltraLow,
            custom_graphics: GraphicsPreset::UltraLow.defaults(),
            graphics_applied: false,
            render_scale: None,
            vsync: true,
            // Phones hold a steady 60 for longer than they hold 120 (heat).
            max_fps: if MOBILE { 60 } else { 0 },
            upscaler: Upscaling::MetalFx,
            sharpness: 0.5,
            frame_generation: false,
            gpu_driver: None,
            overlay: PerfOverlay::Off,
            overlay_detail: OverlayDetail::Standard,
            overlay_corner: Corner::TopLeft,
            touch: TouchMode::Auto,
            touch_opacity: 0.6,
            touch_size: 1.0,
            touch_camera: 1.0,
            touch_aim: TouchAim::LockOn,
            haptics: true,
            touch_buttons: BTreeMap::new(),
            sandbox: false,
            online: false,
            online_name: String::new(),
            friend_notices: true,
            mods: false,
            studio: true,
            // A phone's own browser is no place to write mods.
            studio_lan: crate::paths::MOBILE,
            studio_code: String::new(),
            render_height: None,
            metalfx: None,
            metal_hud: None,
        }
    }
}

/// Tên mặc định cho người chơi Online mới: "NguoiChoi" kèm 4 chữ số.
pub fn default_online_name() -> String {
    format!("NguoiChoi{:04}", random() % 10000)
}

fn random() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    std::collections::hash_map::RandomState::new().build_hasher().finish()
}

impl Settings {
    fn file(user_dir: &Path) -> PathBuf {
        user_dir.join("glue5.json")
    }

    pub fn load(user_dir: &Path) -> Settings {
        let file = Self::file(user_dir);
        let mut settings = match std::fs::read(&file) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                // Kept aside (the defaults are saved over it soon), not lost.
                let aside = file.with_extension("json.bad");
                match std::fs::rename(&file, &aside) {
                    Ok(()) => log::warn!("settings unreadable ({e}): kept as {}, the defaults instead", aside.display()),
                    Err(r) => log::warn!("settings unreadable ({e}), not kept aside ({r}): the defaults instead"),
                }
                Settings::default()
            }),
            Err(_) => Settings::default(),
        };
        if settings.metalfx.take() == Some(false) {
            settings.upscaler = Upscaling::Bicubic;
        }
        if settings.metal_hud.take() == Some(true) {
            settings.overlay = PerfOverlay::MetalHud;
        }
        if TV {
            settings.source = Source::Server;
        }
        settings
    }

    pub fn save(&self, user_dir: &Path) {
        let json = serde_json::to_vec_pretty(self).expect("settings serialise");
        // Through a copy: a write cut short (disk full, the app killed)
        // leaves the old file, not one that loads as the defaults.
        let file = Self::file(user_dir);
        let tmp = file.with_extension("json.tmp");
        if let Err(e) = std::fs::write(&tmp, json).and_then(|()| std::fs::rename(&tmp, &file)) {
            log::warn!("cannot save settings: {e}");
        }
    }

    /// Picks the render scale, the first time, for a window `height` pixels
    /// tall: the height an older settings file had, else about 1080 lines on
    /// desktop and 720 on phones and tablets (what the game ran at before:
    /// its GPU time grows with the pixels, and phone screens have many).
    pub fn for_screen(&mut self, height: u32) {
        if self.render_scale.is_none() {
            if let Some(lines) = self.render_height.take() {
                self.render_scale = Some(round_scale(lines as f32 / height.max(1) as f32));
            } else {
                self.render_scale = Some(recommended_scale(self.graphics_preset, height));
            }
        }
    }

    /// Chuyển preset đồ hoạ, đồng bộ cấu hình chi tiết & độ phân giải tương ứng.
    pub fn apply_graphics_preset(&mut self, preset: GraphicsPreset, window_height: u32) {
        self.graphics_preset = preset;
        if preset != GraphicsPreset::Custom {
            self.custom_graphics = preset.defaults();
            self.render_scale = Some(recommended_scale(preset, window_height));
            let tight_device = glue5_runtime::memory::MemoryProfile::detect().recommends_low_memory()
                || sdl3::cpuinfo::system_ram() < LOW_MEMORY_RAM_MB;
            let small_ram = MOBILE && tight_device;
            self.low_memory = small_ram || (MOBILE && matches!(preset, GraphicsPreset::UltraLow | GraphicsPreset::Low));
        }
        self.graphics_applied = false;
    }

    pub fn render_scale(&self) -> f32 {
        self.render_scale.unwrap_or(1.0)
    }
}

/// Tỉ lệ phân giải khuyến nghị theo từng mức đồ hoạ và chiều cao màn hình.
pub fn recommended_scale(preset: GraphicsPreset, height: u32) -> f32 {
    let h = height.max(1) as f32;
    let lines = match preset {
        GraphicsPreset::UltraLow | GraphicsPreset::Custom => if MOBILE { 480 } else { 540 },
        GraphicsPreset::Low => if MOBILE { 576 } else { 720 },
        GraphicsPreset::Medium => if MOBILE { 720 } else { 900 },
        GraphicsPreset::High => if MOBILE { 900 } else { 1080 },
        GraphicsPreset::Ultra => return 1.0,
    };
    round_scale(lines as f32 / h)
}

/// A render scale in the range, to its 5% step.
pub fn round_scale(scale: f32) -> f32 {
    ((scale * 20.0).round() / 20.0).clamp(*RENDER_SCALES.start(), *RENDER_SCALES.end())
}

/// The game's back buffer for a window of `pixels` at `scale`, as the
/// browser build sizes it: even sides (the engine halves them for its
/// post-processing), at least the 320x240 the engine takes, at most 4K
/// (1440p with low memory: render targets the size of the screen are most
/// of the GPU's memory).
pub fn render_size((w, h): (u32, u32), scale: f32, low_memory: bool) -> (u32, u32) {
    let cap = if low_memory { (2560.0, 1440.0) } else { (3840.0, 2160.0) };
    let (w, h) = (w as f32 * scale, h as f32 * scale);
    let k = (cap.0 / w).min(cap.1 / h).min(1.0);
    let side = |v: f32, min: u32| ((v * k / 2.0).round() as u32 * 2).max(min);
    (side(w, 320), side(h, 240))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_sizes() {
        assert_eq!(render_size((2560, 1440), 0.75, false), (1920, 1080));
        assert_eq!(render_size((2532, 1170), 0.6, false), (1520, 702));
        assert_eq!(render_size((300, 100), 0.5, false), (320, 240));
        assert_eq!(render_size((6016, 3384), 1.0, false), (3840, 2160));
        assert_eq!(render_size((6016, 3384), 1.0, true), (2560, 1440));
    }

    #[test]
    fn upscaler_and_overlay_from_older_settings() {
        let dir = std::env::temp_dir().join(format!("glue5-settings-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("glue5.json"), r#"{"metalfx": false}"#).unwrap();
        assert_eq!(Settings::load(&dir).upscaler, Upscaling::Bicubic);
        std::fs::write(dir.join("glue5.json"), r#"{"metalfx": true}"#).unwrap();
        assert_eq!(Settings::load(&dir).upscaler, Upscaling::MetalFx);
        std::fs::write(dir.join("glue5.json"), r#"{"upscaler": "metalfx-temporal"}"#).unwrap();
        assert_eq!(Settings::load(&dir).upscaler, Upscaling::MetalFxTemporal);
        std::fs::write(dir.join("glue5.json"), r#"{"metal_hud": true}"#).unwrap();
        assert_eq!(Settings::load(&dir).overlay, PerfOverlay::MetalHud);
        std::fs::write(dir.join("glue5.json"), r#"{"metal_hud": false}"#).unwrap();
        assert_eq!(Settings::load(&dir).overlay, PerfOverlay::Off);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Settings that cannot be read are kept aside, not saved over.
    #[test]
    fn unreadable_settings_kept_aside() {
        let dir = std::env::temp_dir().join(format!("glue5-settings-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("glue5.json"), b"{\"online\": tru").unwrap();
        let s = Settings::load(&dir);
        assert_eq!(s.online, Settings::default().online);
        s.save(&dir);
        assert_eq!(std::fs::read(dir.join("glue5.json.bad")).unwrap(), b"{\"online\": tru");
        assert!(serde_json::from_slice::<Settings>(&std::fs::read(dir.join("glue5.json")).unwrap()).is_ok());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn first_scale() {
        let mut s = Settings { render_height: Some(720), ..Settings::default() };
        s.for_screen(1440);
        assert_eq!(s.render_scale, Some(0.5));
        s.for_screen(720);
        assert_eq!(s.render_scale, Some(0.5), "chosen once");
        let mut s = Settings::default();
        assert_eq!(s.graphics_preset, GraphicsPreset::UltraLow);
        s.for_screen(1170);
        assert_eq!(s.render_scale, Some(if MOBILE { 0.4 } else { 0.45 }));
    }

    #[test]
    fn graphics_presets_and_touch_overrides_roundtrip() {
        let mut s: Settings = serde_json::from_str(r#"{"online": true}"#).expect("old config loads");
        assert_eq!(s.graphics_preset, GraphicsPreset::UltraLow);
        assert!(s.touch_buttons.is_empty());

        s.apply_graphics_preset(GraphicsPreset::High, 1170);
        assert_eq!(s.graphics_preset, GraphicsPreset::High);
        assert_eq!(s.custom_graphics, GraphicsPreset::High.defaults());
        assert_eq!(s.render_scale, Some(recommended_scale(GraphicsPreset::High, 1170)));
        assert!(!s.graphics_applied);

        s.touch_buttons.insert(
            "foot:fire".into(),
            ButtonOverride { offset: [0.2, -0.1], size: 1.5, opacity: 0.75 }.clamped(),
        );
        let json = serde_json::to_vec(&s).unwrap();
        let back: Settings = serde_json::from_slice(&json).unwrap();
        assert_eq!(back.graphics_preset, GraphicsPreset::High);
        assert_eq!(back.touch_buttons.get("foot:fire").unwrap().size, 1.5);
        assert_eq!(back.touch_buttons.get("foot:fire").unwrap().opacity, 0.75);
    }
}
