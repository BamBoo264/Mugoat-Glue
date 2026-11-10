//! The engine's graphics settings: `settings.xml` in the user directory,
//! which its pause menu edits and saves (Settings › Graphics). Changes apply
//! at once, or, for those that need it, at a restart the engine asks for
//! (see [`crate::exit`]).
//!
//! The host never overrides them on the command line, which would undo the
//! menu's choices at every start: it only writes the low-memory preset over
//! them, once, and puts them back to the engine's defaults when asked.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

/// Launcher graphics preset tiers (also shared with the UI settings page).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GraphicsPreset {
    #[default]
    UltraLow,
    Low,
    Medium,
    High,
    Ultra,
    Custom,
}

impl GraphicsPreset {
    /// Presets selectable by the user, in display order.
    pub const ALL: [Self; 6] = [
        Self::UltraLow,
        Self::Low,
        Self::Medium,
        Self::High,
        Self::Ultra,
        Self::Custom,
    ];

    /// Five fixed tiers (excluding `Custom`).
    pub const TIERS: [Self; 5] = [
        Self::UltraLow,
        Self::Low,
        Self::Medium,
        Self::High,
        Self::Ultra,
    ];

    /// Vietnamese display label for this preset.
    pub fn label(self) -> &'static str {
        match self {
            Self::UltraLow => "Cực Thấp",
            Self::Low => "Thấp",
            Self::Medium => "Trung Bình",
            Self::High => "Cao",
            Self::Ultra => "Cực Cao",
            Self::Custom => "Tuỳ Chỉnh",
        }
    }

    /// Balanced engine settings corresponding to this tier (`Custom` returns `UltraLow` baseline).
    pub fn defaults(self) -> CustomGraphics {
        match self {
            Self::UltraLow | Self::Custom => CustomGraphics {
                texture_quality: 0,
                shader_quality: 0,
                shadow_quality: 1,
                soft_shadows: 0,
                reflection_quality: 0,
                reflection_msaa: 0,
                particle_quality: 0,
                water_quality: 0,
                grass_quality: 0,
                postfx: 0,
                ssao: 0,
                msaa: 0,
                anisotropic: 0,
                fxaa: false,
                tessellation: 0,
                city_density: 0.0,
                ped_variety: 0.0,
                vehicle_variety: 0.0,
                lod_scale: 0.0,
                max_lod_scale: 0.0,
                shadow_distance: 0.5,
                motion_blur: 0.0,
                long_shadows: false,
                ultra_shadows: false,
                dof: false,
                hd_flight: false,
                fog_volumes: false,
            },
            Self::Low => CustomGraphics {
                texture_quality: 0,
                shader_quality: 0,
                shadow_quality: 1,
                soft_shadows: 0,
                reflection_quality: 0,
                reflection_msaa: 0,
                particle_quality: 0,
                water_quality: 0,
                grass_quality: 0,
                postfx: 0,
                ssao: 0,
                msaa: 0,
                anisotropic: 2,
                fxaa: true,
                tessellation: 0,
                city_density: 0.25,
                ped_variety: 0.25,
                vehicle_variety: 0.25,
                lod_scale: 0.25,
                max_lod_scale: 0.0,
                shadow_distance: 0.75,
                motion_blur: 0.0,
                long_shadows: false,
                ultra_shadows: false,
                dof: false,
                hd_flight: false,
                fog_volumes: false,
            },
            Self::Medium => CustomGraphics {
                texture_quality: 1,
                shader_quality: 1,
                shadow_quality: 2,
                soft_shadows: 1,
                reflection_quality: 1,
                reflection_msaa: 0,
                particle_quality: 1,
                water_quality: 1,
                grass_quality: 1,
                postfx: 1,
                ssao: 1,
                msaa: 0,
                anisotropic: 4,
                fxaa: true,
                tessellation: 1,
                city_density: 0.50,
                ped_variety: 0.50,
                vehicle_variety: 0.50,
                lod_scale: 0.50,
                max_lod_scale: 0.25,
                shadow_distance: 1.0,
                motion_blur: 0.0,
                long_shadows: false,
                ultra_shadows: false,
                dof: false,
                hd_flight: false,
                fog_volumes: true,
            },
            Self::High => CustomGraphics {
                texture_quality: 2,
                shader_quality: 2,
                shadow_quality: 2,
                soft_shadows: 2,
                reflection_quality: 2,
                reflection_msaa: 2,
                particle_quality: 1,
                water_quality: 2,
                grass_quality: 2,
                postfx: 2,
                ssao: 1,
                msaa: 2,
                anisotropic: 8,
                fxaa: true,
                tessellation: 2,
                city_density: 0.80,
                ped_variety: 0.80,
                vehicle_variety: 0.80,
                lod_scale: 0.80,
                max_lod_scale: 0.60,
                shadow_distance: 1.25,
                motion_blur: 0.15,
                long_shadows: true,
                ultra_shadows: false,
                dof: true,
                hd_flight: false,
                fog_volumes: true,
            },
            Self::Ultra => CustomGraphics {
                texture_quality: 2,
                shader_quality: 2,
                shadow_quality: 3,
                soft_shadows: 3,
                reflection_quality: 3,
                reflection_msaa: 4,
                particle_quality: 2,
                water_quality: 2,
                grass_quality: 3,
                postfx: 3,
                ssao: 2,
                msaa: 4,
                anisotropic: 16,
                fxaa: true,
                tessellation: 3,
                city_density: 1.0,
                ped_variety: 1.0,
                vehicle_variety: 1.0,
                lod_scale: 1.0,
                max_lod_scale: 1.0,
                shadow_distance: 1.50,
                motion_blur: 0.25,
                long_shadows: true,
                ultra_shadows: true,
                dof: true,
                hd_flight: true,
                fog_volumes: true,
            },
        }
    }

    /// Recommended target vertical resolution in lines (mobile) for each tier.
    pub fn recommended_mobile_lines(self) -> Option<u32> {
        match self {
            Self::UltraLow => Some(480),
            Self::Low => Some(576),
            Self::Medium => Some(720),
            Self::High => Some(900),
            Self::Ultra => Some(1080),
            Self::Custom => None,
        }
    }

    /// Whether low-memory engine streaming pools should be active for this tier.
    pub fn recommends_low_memory(self) -> Option<bool> {
        match self {
            Self::UltraLow | Self::Low => Some(true),
            Self::Medium | Self::High | Self::Ultra => Some(false),
            Self::Custom => None,
        }
    }
}

/// Detailed graphics configuration written into the engine's `settings.xml`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomGraphics {
    pub texture_quality: u8,
    pub shader_quality: u8,
    pub shadow_quality: u8,
    pub soft_shadows: u8,
    pub reflection_quality: u8,
    pub reflection_msaa: u8,
    pub particle_quality: u8,
    pub water_quality: u8,
    pub grass_quality: u8,
    pub postfx: u8,
    pub ssao: u8,
    pub msaa: u8,
    pub anisotropic: u8,
    pub fxaa: bool,
    pub tessellation: u8,
    pub city_density: f32,
    pub ped_variety: f32,
    pub vehicle_variety: f32,
    pub lod_scale: f32,
    pub max_lod_scale: f32,
    pub shadow_distance: f32,
    pub motion_blur: f32,
    pub long_shadows: bool,
    pub ultra_shadows: bool,
    pub dof: bool,
    pub hd_flight: bool,
    pub fog_volumes: bool,
}

impl Default for CustomGraphics {
    fn default() -> Self {
        GraphicsPreset::UltraLow.defaults()
    }
}

impl CustomGraphics {
    /// Serializes these settings into `(xml_tag, value)` pairs for `settings.xml`.
    pub fn to_xml_pairs(&self) -> Vec<(&'static str, String)> {
        let f = |v: f32| format!("{v:.6}");
        let b = |v: bool| if v { "true" } else { "false" }.to_string();
        let lod = self.lod_scale.clamp(0.0, 1.0);
        let mip_blur = self.reflection_quality > 0;
        let particle_shadows = self.shadow_quality >= 2 && self.particle_quality >= 1;
        vec![
            ("TextureQuality", self.texture_quality.min(2).to_string()),
            ("ShaderQuality", self.shader_quality.min(2).to_string()),
            ("ShadowQuality", self.shadow_quality.min(3).to_string()),
            ("Shadow_SoftShadows", self.soft_shadows.min(5).to_string()),
            ("ReflectionQuality", self.reflection_quality.min(3).to_string()),
            ("ReflectionMSAA", snap_msaa(self.reflection_msaa).to_string()),
            ("ParticleQuality", self.particle_quality.min(2).to_string()),
            ("WaterQuality", self.water_quality.min(2).to_string()),
            ("GrassQuality", self.grass_quality.min(3).to_string()),
            ("PostFX", self.postfx.min(3).to_string()),
            ("SSAO", self.ssao.min(2).to_string()),
            ("MSAA", snap_msaa(self.msaa).to_string()),
            ("AnisotropicFiltering", snap_af(self.anisotropic).to_string()),
            ("FXAA_Enabled", b(self.fxaa)),
            ("Tessellation", self.tessellation.min(3).to_string()),
            ("CityDensity", f(self.city_density.clamp(0.0, 1.0))),
            ("PedVarietyMultiplier", f(self.ped_variety.clamp(0.0, 1.0))),
            ("VehicleVarietyMultiplier", f(self.vehicle_variety.clamp(0.0, 1.0))),
            ("LodScale", f(lod)),
            ("PedLodBias", f(lod)),
            ("VehicleLodBias", f(lod)),
            ("MaxLodScale", f(self.max_lod_scale.clamp(0.0, 1.0))),
            ("Shadow_Distance", f(self.shadow_distance.clamp(0.5, 2.0))),
            ("MotionBlurStrength", f(self.motion_blur.clamp(0.0, 1.0))),
            ("Shadow_LongShadows", b(self.long_shadows)),
            ("UltraShadows_Enabled", b(self.ultra_shadows)),
            ("DoF", b(self.dof)),
            ("HdStreamingInFlight", b(self.hd_flight)),
            ("Lighting_FogVolumes", b(self.fog_volumes)),
            ("Reflection_MipBlur", b(mip_blur)),
            ("Shadow_ParticleShadows", b(particle_shadows)),
        ]
    }
}

fn snap_msaa(v: u8) -> u8 {
    match v {
        0..=1 => 0,
        2..=3 => 2,
        4..=7 => 4,
        _ => 8,
    }
}

fn snap_af(v: u8) -> u8 {
    match v {
        0..=1 => 0,
        2..=3 => 2,
        4..=7 => 4,
        8..=15 => 8,
        _ => 16,
    }
}

fn path(user_dir: &Path) -> PathBuf {
    user_dir.join("Documents/Rockstar Games/GTA V/settings.xml")
}

/// The low-memory preset (the browser build's `?low=1`): the lowest texture,
/// reflection, particle and grass quality, shadows at the lowest level,
/// minimum ped and vehicle variety, city density and level of detail.
/// Measured in a Node run at the world: GPU memory 1.16 GB -> 0.5 GB,
/// streamed assets 1.38 GB -> 0.95 GB.
const LOW: &[(&str, &str)] = &[
    ("TextureQuality", "0"),
    ("ShadowQuality", "1"),
    ("ReflectionQuality", "0"),
    ("ParticleQuality", "0"),
    ("GrassQuality", "0"),
    ("CityDensity", "0.000000"),
    ("PedVarietyMultiplier", "0.000000"),
    ("VehicleVarietyMultiplier", "0.000000"),
    ("LodScale", "0.000000"),
    ("PedLodBias", "0.000000"),
    ("VehicleLodBias", "0.000000"),
];

/// What the engine writes when it first starts without settings (its
/// defaults for the adapter it is given, the same everywhere).
const DEFAULTS: &str = include_str!("settings.xml");

/// Writes the low-memory preset over the current settings (or the engine's
/// defaults, before its first start).
pub fn apply_low_preset(user_dir: &Path) -> io::Result<()> {
    apply(user_dir, LOW)
}

/// Writes a [`CustomGraphics`] configuration over the current settings (or the
/// engine's defaults, before its first start).
pub fn apply_custom(user_dir: &Path, cfg: &CustomGraphics) -> io::Result<()> {
    apply(user_dir, &cfg.to_xml_pairs())
}

/// Writes `settings` (name, value) over the current settings (or the
/// engine's defaults, before its first start). Names the file lacks are
/// skipped, with a warning.
pub fn apply<N: AsRef<str>, V: AsRef<str>>(user_dir: &Path, settings: &[(N, V)]) -> io::Result<()> {
    let path = path(user_dir);
    let mut xml = std::fs::read_to_string(&path).unwrap_or_else(|_| DEFAULTS.to_string());
    for (name, value) in settings {
        let name = name.as_ref();
        if !xml.contains(&format!("<{name} value=\"")) {
            log::warn!("game settings: no {name} in settings.xml");
        }
        xml = set(&xml, name, value.as_ref());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, xml)
}

/// Puts the graphics settings back to the engine's defaults, keeping the
/// file as it was in `settings.xml.bak` (the rest of it, and the saves,
/// stay): the way out of a setting the engine cannot run with, which stops
/// it before its menu can change it back.
pub fn reset_graphics(user_dir: &Path) -> io::Result<()> {
    let path = path(user_dir);
    let Ok(xml) = std::fs::read_to_string(&path) else {
        return Ok(()); // none: the engine starts from its defaults
    };
    std::fs::write(path.with_extension("xml.bak"), &xml)?;
    std::fs::write(&path, with_graphics(&xml, DEFAULTS))
}

/// `xml` with the `<graphics>` section of `from` (all of `from` if either
/// lacks one).
fn with_graphics(xml: &str, from: &str) -> String {
    const END: &str = "</graphics>";
    let section = |s: &str| Some(s.find("<graphics>")?..s.find(END)? + END.len());
    match (section(xml), section(from)) {
        (Some(mine), Some(theirs)) => format!("{}{}{}", &xml[..mine.start], &from[theirs], &xml[mine.end..]),
        _ => from.to_string(),
    }
}

/// `xml` with `<name value="..." />` set to `value` (unchanged if absent).
fn set(xml: &str, name: &str, value: &str) -> String {
    let tag = format!("<{name} value=\"");
    let Some(start) = xml.find(&tag).map(|i| i + tag.len()) else { return xml.to_string() };
    let Some(len) = xml[start..].find('"') else { return xml.to_string() };
    format!("{}{value}{}", &xml[..start], &xml[start + len..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_values_by_exact_name() {
        let xml = "<LodScale value=\"1.000000\" />\n<MaxLodScale value=\"1.000000\" />";
        assert_eq!(
            set(xml, "LodScale", "0.000000"),
            "<LodScale value=\"0.000000\" />\n<MaxLodScale value=\"1.000000\" />"
        );
        assert_eq!(set(xml, "Missing", "0"), xml);
    }

    #[test]
    fn graphics_only_go_back_to_the_defaults() {
        let xml = "<Settings>\n  <graphics>\n    <MSAA value=\"2\" />\n  </graphics>\n  <video>\n    <VSync value=\"0\" />\n  </video>\n</Settings>";
        let reset = with_graphics(xml, DEFAULTS);
        assert!(reset.contains("<MSAA value=\"0\" />"));
        assert!(reset.contains("<VSync value=\"0\" />\n  </video>\n</Settings>"));
        assert_eq!(reset.matches("<graphics>").count(), 1);
        assert_eq!(with_graphics("broken", DEFAULTS), DEFAULTS);
    }

    #[test]
    fn preset_names_are_in_the_defaults() {
        for (name, _) in LOW {
            assert!(DEFAULTS.contains(&format!("<{name} value=\"")), "{name}");
        }
        for preset in GraphicsPreset::ALL {
            for (name, _) in preset.defaults().to_xml_pairs() {
                assert!(DEFAULTS.contains(&format!("<{name} value=\"")), "{name}");
            }
        }
    }

    #[test]
    fn apply_custom_preserves_other_sections_and_orders_tiers() {
        let dir = std::env::temp_dir().join(format!("glue5_gs_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ultra_low = GraphicsPreset::UltraLow.defaults();
        let ultra = GraphicsPreset::Ultra.defaults();
        assert!(ultra_low.texture_quality <= ultra.texture_quality);
        assert!(ultra_low.city_density < ultra.city_density);
        assert!(ultra_low.msaa < ultra.msaa);

        apply_custom(&dir, &ultra_low).unwrap();
        let written = std::fs::read_to_string(path(&dir)).unwrap();
        assert!(written.contains("<TextureQuality value=\"0\" />"));
        assert!(written.contains("<CityDensity value=\"0.000000\" />"));
        assert!(written.contains("<audio>"));

        apply_custom(&dir, &ultra).unwrap();
        let updated = std::fs::read_to_string(path(&dir)).unwrap();
        assert!(updated.contains("<TextureQuality value=\"2\" />"));
        assert!(updated.contains("<MSAA value=\"4\" />"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
