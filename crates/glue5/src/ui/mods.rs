//! Mods: switched on or off here; the mods in the user directory's `mods`
//! folder, started with the game or during it; their menus while it runs;
//! GLUE 5 Studio (the web IDE the app serves) and its control page (the
//! mods' menus, for a phone) with their addresses, the pairing code, a QR
//! code and the paired browsers and agents; the mods' log.

use egui::{Color32, CornerRadius, RichText, Sense, Stroke, vec2};
use glue5_mods::ModState;

use super::menus::{MenusView, Out};
use super::pages::{card_title, sheet, small};
use super::widgets::{Control, action_button, action_buttons, inline_button, segmented, setting, toggle};
use super::{Status, WARN, theme};
use crate::settings::Settings;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    Menus,
    #[default]
    Mods,
    Studio,
    Log,
}

/// What the page keeps between frames.
#[derive(Default)]
pub struct ModsPage {
    tab: Tab,
    menus: MenusView,
}

impl ModsPage {
    /// Opens on the mods' menus (where the player was in them).
    pub fn show_menus(&mut self) {
        self.tab = Tab::Menus;
    }

    /// Opens at menu `menu` of mod `module` (`menu:show()`). False when
    /// there is no such menu.
    pub fn show_menu(&mut self, module: &str, menu: u32) -> bool {
        let shown = self.menus.open_at(&glue5_mods::menus(), module, menu);
        if shown {
            self.tab = Tab::Menus;
        }
        shown
    }

    /// Back (B, Esc) inside the page: false when the page should close.
    pub fn back(&mut self, ctx: &egui::Context) -> bool {
        self.tab == Tab::Menus && self.menus.back(ctx)
    }

    /// The page closed: its menus are no longer looked at.
    pub fn closed(&mut self) {
        send(self.menus.hidden());
    }
}

/// What the player did with the menus, to the runtime.
fn send(outs: Vec<Out>) {
    for out in outs {
        match out {
            Out::Input { module, item, input } => {
                if let Err(e) = glue5_mods::menu_input(&module, item, input) {
                    log::debug!("mods: menu input: {e}");
                }
            }
            Out::Viewer { module, menu, open } => glue5_mods::menu_viewer(&module, menu, open),
        }
    }
}

/// The Studio as it serves, for the page.
pub struct StudioInfo {
    /// The Studio's address.
    pub url: String,
    /// The control page's address.
    pub control: String,
    /// The pairing code as people type it.
    pub code: String,
    pub lan: bool,
}

impl StudioInfo {
    /// `url` with the code to pair: the page opens paired (this device's
    /// browser, a phone's camera).
    pub fn paired(&self, url: &str) -> String {
        format!("{url}?pair={}", self.code)
    }
}

const ERR: Color32 = Color32::from_rgb(240, 149, 149);

/// The mods in a line, for Settings: off, on, what runs.
pub(super) fn summary(settings: &Settings, status: &Status) -> String {
    match status.started.as_ref() {
        Some(s) if s.mods != settings.mods => {
            return if settings.mods { "Bật từ lần mở sau".into() } else { "Tắt từ lần mở sau".into() };
        }
        _ => {}
    }
    if !settings.mods {
        return "Đã tắt · script thay đổi trò chơi".into();
    }
    let mods = glue5_mods::mods();
    let mut parts = vec!["Bật".to_owned()];
    match mods.len() {
        0 => {}
        1 => parts.push("1 bản mod".into()),
        n => parts.push(format!("{n} bản mod")),
    }
    let running = mods.iter().filter(|m| m.state == ModState::Running).count();
    if running > 0 {
        parts.push(format!("{running} đang chạy"));
    }
    if status.studio.is_some() {
        parts.push("Studio đang bật".into());
    }
    parts.join(" · ")
}

/// The start page's card while mods are on: what there is, and the way
/// into the Studio.
pub(super) fn home_card(ui: &mut egui::Ui, settings: &Settings, status: &Status, go: &mut Option<Option<super::Page>>) {
    let state = match status.started.as_ref() {
        Some(s) if !s.mods => ("Lần mở sau", WARN),
        _ => ("Bật", super::pages::READY),
    };
    card_title(ui, "BẢN MOD", Some(state));
    let mods = glue5_mods::mods();
    let running = mods.iter().filter(|m| m.state == ModState::Running).count();
    let mut line = match mods.len() {
        0 => "Chưa có bản mod".to_owned(),
        1 => "1 bản mod".to_owned(),
        n => format!("{n} bản mod"),
    };
    if running > 0 {
        line.push_str(&format!(" · {running} đang chạy"));
    }
    match &status.studio {
        Some(info) => {
            line.push_str(if info.lan { " · Studio trên mạng này" } else { " · Studio trên thiết bị" });
            small(ui, &line, theme::MUTED);
            let address = info.url.trim_end_matches('/').to_owned();
            ui.label(RichText::new(format!("{address}  ·  mã {}", info.code)).monospace().color(Color32::WHITE));
            let [open, control, copy, page] = <[_; 4]>::try_from(
                ui.horizontal(|ui| action_buttons(ui, &["Mở Studio", "Điều khiển", "Sao chép địa chỉ", "Bản mod"])).inner,
            )
            .unwrap();
            if open.on_hover_text("Trong trình duyệt của thiết bị").clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(info.paired(&info.url)));
            }
            if control.on_hover_text("Menu mod trong trình duyệt của thiết bị").clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(info.paired(&info.control)));
            }
            if copy.clicked() {
                ui.ctx().copy_text(info.url.clone());
            }
            if page.clicked() {
                *go = Some(Some(super::Page::Mods));
            }
        }
        None => {
            if !settings.studio {
                line.push_str(" · Studio đã tắt");
            }
            small(ui, &line, theme::MUTED);
            if ui.horizontal(|ui| action_button(ui, "Bản mod")).inner.clicked() {
                *go = Some(Some(super::Page::Mods));
            }
        }
    }
}

/// True when closed.
pub(super) fn mods_page(ctx: &egui::Context, page: &mut ModsPage, settings: &mut Settings, status: &Status) -> bool {
    // Mods change by themselves (they end, fail, the Studio starts them).
    ctx.request_repaint_after(std::time::Duration::from_millis(500));
    sheet(ctx, "mods-window", "BẢN MOD", |ui| {
        let scale = theme::density(ui);
        // Inside a menu, its nav bar takes the place of the switch and the
        // tabs.
        let menus = (status.mods && page.tab == Tab::Menus).then(glue5_mods::menus);
        match &menus {
            Some(menus) => page.menus.prune(menus),
            None => send(page.menus.hidden()),
        }
        if let Some(menus) = menus.as_ref().filter(|_| page.menus.inside()) {
            menus_tab(ui, &mut page.menus, menus);
            return;
        }
        let running_with = status.started.as_ref().map(|s| s.mods);
        let note = match running_with {
            Some(with) if with != settings.mods => {
                Some(if settings.mods { "Bật khi mở trò chơi lần sau" } else { "Tắt từ lần mở sau. Mod đang chạy tiếp tục hoạt động đến lúc đó." })
            }
            _ => None,
        };
        setting(ui, "Bản mod", Control::Toggle, Some(note.unwrap_or("Script Luau chạy cùng và thay đổi trò chơi")), |ui| {
            toggle(ui, &mut settings.mods);
        });
        if !settings.mods && !status.mods {
            ui.add_space(4.0 * scale);
            panel(ui, |ui| {
                card_title(ui, "VỀ BẢN MOD", None);
                small(
                    ui,
                    "Mod là thư mục chứa các tệp mã nguồn Luau gọi trực tiếp các hàm của trò chơi: tạo xe, thay đổi \
                     thời tiết, thêm menu tùy chỉnh. Hãy bật Bản mod rồi biên tập bằng Mugoat Glue Studio trên trình \
                     duyệt máy tính/máy tính bảng.",
                    theme::MUTED,
                );
            });
            return;
        }
        ui.add_space(6.0 * scale);
        // The menus are there while the game runs with mods.
        let mut tabs = vec![(Tab::Mods, "BẢN MOD"), (Tab::Studio, "STUDIO"), (Tab::Log, "NHẬT KÝ")];
        if status.mods {
            tabs.insert(0, (Tab::Menus, "MENU"));
        } else if page.tab == Tab::Menus {
            page.tab = Tab::Mods;
        }
        segmented(ui, &mut page.tab, &tabs, 40.0 * scale);
        ui.add_space(10.0 * scale);
        match page.tab {
            Tab::Menus => menus_tab(ui, &mut page.menus, menus.as_deref().map_or(&[], |m| m.as_slice())),
            Tab::Mods => mods_tab(ui, page, status),
            Tab::Studio => studio_tab(ui, settings, status),
            Tab::Log => log_tab(ui),
        }
    })
}

/// The mods' menus: the list of them, or the level open in one.
fn menus_tab(ui: &mut egui::Ui, view: &mut MenusView, menus: &[glue5_mods::Menu]) {
    let mods = glue5_mods::mods();
    let names: Vec<(&str, &str)> = mods.iter().map(|m| (m.name.as_str(), m.info.title.as_str())).collect();
    send(view.view(ui, menus, &names));
}

fn mods_tab(ui: &mut egui::Ui, page: &mut ModsPage, status: &Status) {
    let held = glue5_mods::held_back();
    if !held.is_empty() {
        small(
            ui,
            &format!("{} không tự chạy vì còn hoạt động khi trò chơi dừng lần trước.", held.join(", ")),
            WARN,
        );
    }
    let mods = glue5_mods::mods();
    if mods.is_empty() {
        panel(ui, |ui| {
            card_title(ui, "CHƯA CÓ BẢN MOD", None);
            small(ui, "Viết mod trong Mugoat Glue Studio bằng trình duyệt trên mạng này.", theme::MUTED);
            if let Some(dir) = glue5_mods::dir().filter(|_| !crate::paths::MOBILE) {
                small(ui, &format!("Hoặc đặt thư mục chứa main.luau vào {}", dir.display()), theme::MUTED);
            }
            ui.add_space(4.0 * theme::density(ui));
            if inline_button(ui, "Mở Studio").clicked() {
                page.tab = Tab::Studio;
            }
        });
        return;
    }
    // Before the game: which start with it.
    if !status.mods {
        for m in &mods {
            let mut on = m.info.autostart;
            let note = if on { "Chạy cùng trò chơi" } else { "Không chạy cùng trò chơi" };
            setting(ui, &m.info.title, Control::Toggle, Some(note), |ui| {
                if toggle(ui, &mut on).changed()
                    && let Err(e) = glue5_mods::set_autostart(&m.name, on)
                {
                    log::warn!("mods: {e}");
                }
            });
        }
        return;
    }
    for m in &mods {
        let subtitle = match m.state {
            ModState::Running if m.waiting => "Đang chờ".to_owned(),
            ModState::Running => format!("Đang chạy · {:.1} ms mỗi khung", m.cpu_ms),
            ModState::Error => format!("Lỗi · {}", m.error.as_deref().unwrap_or("")),
            ModState::Stopped if m.info.autostart => "Đã dừng · tự chạy cùng trò chơi".to_owned(),
            ModState::Stopped => "Đã dừng".to_owned(),
        };
        let dot = match m.state {
            ModState::Running => "• ",
            _ => "",
        };
        setting(ui, &format!("{dot}{}", m.info.title), Control::Action, Some(&subtitle), |ui| {
            let running = m.state == ModState::Running;
            if action_button(ui, if running { "Dừng" } else { "Bắt đầu" }).clicked() {
                if running {
                    glue5_mods::stop(&m.name);
                } else {
                    glue5_mods::start(&m.name);
                }
            }
        });
    }
    let running: Vec<_> = mods.iter().filter(|m| m.state == ModState::Running).collect();
    if !running.is_empty() {
        let total: f32 = running.iter().map(|m| m.cpu_ms).sum();
        setting(ui, &format!("{} đang chạy", running.len()), Control::Action, Some(&format!("{total:.1} ms mỗi khung")), |ui| {
            if action_button(ui, "Dừng tất cả").clicked() {
                for m in &running {
                    glue5_mods::stop(&m.name);
                }
            }
        });
    }
    if glue5_mods::tainted() {
        small(ui, "Một hàm của game gặp lỗi. Hãy mở lại trò chơi.", WARN);
    }
}

fn studio_tab(ui: &mut egui::Ui, settings: &mut Settings, status: &Status) {
    setting(ui, "Mugoat Glue Studio", Control::Toggle, Some("Viết và chạy mod từ trình duyệt, điều khiển từ điện thoại."), |ui| {
        toggle(ui, &mut settings.studio);
    });
    let others = if settings.studio_lan { "Trình duyệt trên mạng này có thể ghép nối bằng mã" } else { "Chỉ trình duyệt trên thiết bị này" };
    setting(ui, "Thiết bị khác", Control::Toggle, Some(others), |ui| {
        toggle(ui, &mut settings.studio_lan);
    });
    match &status.studio {
        Some(info) => {
            let place = if info.lan { "trên mạng này" } else { "trên thiết bị này" };
            for (url, title, note) in [
                (&info.url, "Studio", format!("Trình chỉnh sửa trong trình duyệt {place}")),
                (&info.control, "Trang điều khiển", format!("Mod và menu cho điện thoại hoặc máy tính bảng {place}")),
            ] {
                let address = format!("{title} · {}", url.trim_end_matches('/'));
                setting(ui, &address, Control::Actions, Some(&note), |ui| {
                    let [go, copy] = <[_; 2]>::try_from(action_buttons(ui, &["Mở", "Sao chép"])).unwrap();
                    if go.on_hover_text("Trình duyệt của thiết bị, đã ghép nối").clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(info.paired(url)));
                    }
                    if copy.clicked() {
                        ui.ctx().copy_text(url.clone());
                    }
                });
            }
            setting(ui, &info.code, Control::Actions, Some("Mã ghép nối cho trình duyệt hoặc tác nhân"), |ui| {
                let [new, copy] = <[_; 2]>::try_from(action_buttons(ui, &["Mới", "Sao chép"])).unwrap();
                if new.on_hover_text("Tạo mã mới giữ nguyên các kết nối hiện có").clicked() {
                    settings.studio_code.clear();
                }
                if copy.clicked() {
                    ui.ctx().copy_text(info.code.clone());
                }
            });
            if info.lan {
                qr_row(ui, &info.paired(&info.control));
            }
            paired(ui);
        }
        None if settings.studio => small(ui, "Đang mở…", theme::MUTED),
        None => {}
    }
    muted(
        ui,
        "Bất kỳ ai có mã đều có thể ghép nối và chạy mã trong trò chơi qua Studio: hãy tạo mã mới nếu muốn ngắt \
         người khác và gỡ các kết nối không còn dùng. Kết nối không hoạt động quá 90 ngày sẽ tự xóa.",
    );
}

/// Các trình duyệt và tác nhân AI đã ghép nối với Studio.
fn paired(ui: &mut egui::Ui) {
    let pairings = glue5_mods::studio::pairing::list();
    if pairings.is_empty() {
        return;
    }
    let scale = theme::density(ui);
    ui.add_space(6.0 * scale);
    ui.label(theme::heading("ĐÃ GHÉP NỐI", 15.0 * scale).color(theme::ACCENT));
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    for p in &pairings {
        let what = match p.scope {
            glue5_mods::studio::Scope::Studio => "Studio",
            glue5_mods::studio::Scope::Control => "Trang điều khiển",
        };
        let note = format!("{what} · dùng {}", ago(now.saturating_sub(p.last_used)));
        setting(ui, &p.name, Control::Actions, Some(&note), |ui| {
            if action_button(ui, "Gỡ").on_hover_text("Thiết bị này cần ghép nối lại bằng mã").clicked() {
                glue5_mods::studio::pairing::forget(&p.id);
            }
        });
    }
    if pairings.len() > 1 {
        setting(ui, "Mọi kết nối", Control::Actions, Some("Mọi trình duyệt và tác nhân cần ghép nối lại"), |ui| {
            if action_button(ui, "Gỡ tất cả").clicked() {
                glue5_mods::studio::pairing::forget_all();
            }
        });
    }
}

/// Thời gian tương đối bằng tiếng Việt.
fn ago(seconds: u64) -> String {
    match seconds {
        0..60 => "vừa xong".into(),
        60..3600 => format!("{} phút trước", seconds / 60),
        3600..86400 => format!("{} giờ trước", seconds / 3600),
        _ => match seconds / 86400 {
            1 => "hôm qua".into(),
            d => format!("{d} ngày trước"),
        },
    }
}

/// Mã QR mở trang điều khiển mod trên điện thoại.
fn qr_row(ui: &mut egui::Ui, url: &str) {
    let scale = theme::density(ui);
    egui::Frame::new()
        .fill(theme::CARD)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .corner_radius((16.0 * scale) as u8)
        .inner_margin((12.0 * scale) as i8)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                qr(ui, url, 112.0 * scale);
                ui.add_space(8.0 * scale);
                ui.vertical(|ui| {
                    ui.label(RichText::new("Quét để mở").size(14.0 * scale).color(Color32::WHITE));
                    ui.label(
                        RichText::new("Dùng camera điện thoại hoặc máy tính bảng để mở và ghép nối trang điều khiển mod.")
                            .size(11.0 * scale)
                            .color(theme::MUTED),
                    );
                });
            });
        });
}

fn log_tab(ui: &mut egui::Ui) {
    let scale = theme::density(ui);
    // The last lines only (a copy of the whole log each frame held the lock
    // the mods' update takes).
    let lines = glue5_mods::recent_logs(80);
    if lines.is_empty() {
        muted(ui, "Chưa có dữ liệu.");
    }
    panel(ui, |ui| {
        for l in &lines {
            let color = if l.level == glue5_mods::LogLevel::Error { ERR } else { Color32::from_gray(213) };
            ui.label(RichText::new(format!("{} · {}", l.module, l.text)).monospace().size(12.0 * scale).color(color));
        }
    });
}

/// `text` as a QR code, dark on white.
fn qr(ui: &mut egui::Ui, text: &str, size: f32) {
    // Encoded once per address (the page is drawn every frame).
    thread_local! {
        static CODE: std::cell::RefCell<Option<(String, usize, Vec<qrcode::Color>)>> = const { std::cell::RefCell::new(None) };
    }
    let code = CODE.with_borrow_mut(|cached| {
        if cached.as_ref().is_none_or(|(t, ..)| t != text) {
            *cached = qrcode::QrCode::new(text.as_bytes()).ok().map(|c| (text.to_string(), c.width(), c.to_colors()));
        }
        cached.as_ref().map(|(_, modules, colors)| (*modules, colors.clone()))
    });
    let Some((modules, colors)) = code else { return };
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::same((10.0 * theme::density(ui)) as u8), Color32::WHITE);
    let quiet = 2.0;
    let cell = size / (modules as f32 + 2.0 * quiet);
    for (i, c) in colors.iter().enumerate() {
        if *c == qrcode::Color::Dark {
            let (x, y) = ((i % modules) as f32, (i / modules) as f32);
            let min = rect.min + vec2((x + quiet) * cell, (y + quiet) * cell);
            painter.rect_filled(egui::Rect::from_min_size(min, vec2(cell + 0.5, cell + 0.5)), 0, theme::INK);
        }
    }
}

fn panel(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    let scale = theme::density(ui);
    egui::Frame::new()
        .fill(theme::CARD)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .corner_radius((16.0 * scale) as u8)
        .inner_margin((14.0 * scale) as i8)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 6.0 * scale;
            add(ui);
        });
}

fn muted(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(12.0 * theme::density(ui)).color(theme::MUTED));
}
