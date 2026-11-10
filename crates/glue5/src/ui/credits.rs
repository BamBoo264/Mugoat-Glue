//! The credits: what Glue 5 is, whose game it runs, and the open-source
//! projects it is built on, with their licenses and websites.
use super::pages::sheet;
use super::theme;
use super::widgets::{Control, action_button, setting};
use egui::{Color32, RichText};

/// A project: its name, what it does here, its license, its website.
const PROJECTS: &[(&str, &str, &str, &str)] = &[
    ("WABT", "wasm2c: chuyển mã trò chơi sang C", "Apache-2.0", "https://github.com/WebAssembly/wabt"),
    ("wgpu", "Đồ họa trên Metal và Vulkan", "MIT / Apache-2.0", "https://wgpu.rs"),
    ("naga", "Bộ chuyển đổi shader trò chơi", "MIT / Apache-2.0", "https://github.com/gfx-rs/wgpu"),
    ("SDL", "Cửa sổ, điều khiển, âm thanh và tay cầm", "Zlib", "https://libsdl.org"),
    ("sdl3-rs", "Dùng SDL từ Rust", "MIT", "https://github.com/vhspace/sdl3-rs"),
    ("egui", "Giao diện ứng dụng", "MIT / Apache-2.0", "https://github.com/emilk/egui"),
    ("Barlow", "Phông chữ của Jeremy Tribby", "SIL OFL 1.1", "https://github.com/jpt/barlow"),
    (
        "game-icons.net",
        "Biểu tượng cảm ứng của Lorc, Delapouite, John Colburn và Guard13007",
        "CC BY 3.0",
        "https://game-icons.net",
    ),
    ("Phosphor Icons", "Biểu tượng cảm ứng", "MIT", "https://phosphoricons.com"),
    ("Font Awesome Free", "Biểu tượng cảm ứng", "CC BY 4.0", "https://fontawesome.com"),
    ("Material Symbols", "Biểu tượng cảm ứng của Google", "Apache-2.0", "https://fonts.google.com/icons"),
    ("Lucide", "Biểu tượng cảm ứng", "ISC", "https://lucide.dev"),
    ("Tabler Icons", "Biểu tượng cảm ứng", "MIT", "https://tabler.io/icons"),
    ("MingCute", "Biểu tượng cảm ứng", "Apache-2.0", "https://www.mingcute.com"),
    ("etcpak", "Texture cho GPU thiếu hỗ trợ BC", "BSD-3-Clause", "https://github.com/wolfpld/etcpak"),
    ("bcdec_rs", "Giải mã texture", "MIT", "https://github.com/ScanMountGoat/image_dds"),
    ("snow", "Giao thức Noise: mã hóa kênh bạn bè", "Apache-2.0 / MIT", "https://github.com/mcginty/snow"),
    ("Zstandard", "Đóng gói tệp nhỏ để truyền qua mạng", "BSD-3-Clause", "https://facebook.github.io/zstd"),
    #[cfg(target_os = "android")]
    (
        "libadrenotools",
        "Trình điều khiển Adreno bổ sung, cùng liblinkernsbypass của Billy Laws",
        "BSD-2-Clause",
        "https://github.com/bylaws/libadrenotools",
    ),
    #[cfg(target_vendor = "apple")]
    ("objc2", "Dùng framework của Apple từ Rust", "MIT", "https://github.com/madsmtm/objc2"),
    #[cfg(windows)]
    ("mimalloc", "Bộ cấp phát bộ nhớ", "MIT", "https://github.com/microsoft/mimalloc"),
    ("Rust", "Ngôn ngữ, trình biên dịch và thư viện", "MIT / Apache-2.0", "https://www.rust-lang.org"),
];

/// True when closed.
pub(super) fn credits_page(ctx: &egui::Context) -> bool {
    sheet(ctx, "credits-window", "THÔNG TIN DỰ ÁN", |ui| {
        let scale = theme::density(ui);
        let text = |ui: &mut egui::Ui, text: &str| {
            ui.label(RichText::new(text).size(12.5 * scale).color(theme::MUTED));
        };
        egui::Frame::new()
            .fill(Color32::from_gray(28))
            .corner_radius((14.0 * scale) as u8)
            .inner_margin((16.0 * scale) as i8)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 6.0 * scale;
                ui.horizontal(|ui| {
                    ui.label(theme::heading("MUGOAT GLUE", 26.0 * scale));
                    ui.label(RichText::new(env!("CARGO_PKG_VERSION")).size(12.0 * scale).color(theme::MUTED));
                });
                text(
                    ui,
                    "GTA V's WebAssembly build as a native application for phones, tablets and computers: \
                     the game's code recompiled for this device, with its graphics, sound, controls and \
                     online play. Free software, under the GNU General Public License, version 3 or later.",
                );
            });
        text(
            ui,
            "Grand Theft Auto V is © Rockstar Games and Take-Two Interactive. Glue 5 is an independent \
             project, neither affiliated with nor endorsed by them, and includes none of the game's files: \
             it runs your own copy.",
        );
        ui.add_space(4.0 * scale);
        ui.label(theme::heading("MÃ NGUỒN MỞ", 17.0 * scale).color(theme::MUTED));
        for &(name, what, license, url) in PROJECTS {
            setting(ui, name, Control::Action, Some(&format!("{what} · {license}")), |ui| {
                if action_button(ui, "Trang web").on_hover_text(url).clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                }
            });
        }
        text(
            ui,
            "With the libraries these use, each under its own license (MIT, Apache-2.0, BSD, Zlib, \
             Unicode, …). GTA Online's players and friends find each other through the BitTorrent DHT, \
             kept by the people who run it.",
        );
    })
}
