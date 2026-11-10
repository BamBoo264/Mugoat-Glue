//! Servers: where the game's files come from when they stream. The server
//! in use and how it is doing; the computers sharing the game on this
//! network, each asked whether it serves this game; one by its address;
//! what this device keeps of the game.
use std::time::{Duration, Instant};

use egui::{Align, Color32, Layout, RichText, Sense, Stroke, pos2, vec2};
use glue5_stream::Check;

use super::pages::{bytes, card_title, is_server, server_name, sheet, small, stream_progress, stream_state};
use super::widgets::{Control, action_button, focus, setting, text_field, toggle};
use super::{Action, Status, TextKind, WARN, theme};
use crate::paths;
use crate::settings::{Settings, Source};

/// While the page is open the network is searched this often: computers
/// that start or stop sharing come and go by themselves.
const SEARCH_EVERY: Duration = Duration::from_secs(5);

/// What the page keeps between frames.
#[derive(Default)]
pub struct ServersPage {
    /// An address being typed.
    address: String,
    /// When this page last started a search; none: just opened.
    searched: Option<Instant>,
    /// Delete was pressed once: the next press deletes the copy.
    deleting: bool,
}

impl ServersPage {
    pub(super) fn opened(&mut self) {
        self.searched = None;
        self.deleting = false;
    }
}

/// True when closed.
pub(super) fn servers_page(
    ctx: &egui::Context,
    page: &mut ServersPage,
    settings: &mut Settings,
    status: &Status,
    actions: &mut Vec<Action>,
) -> bool {
    if !status.searching && page.searched.is_none_or(|t| t.elapsed() >= SEARCH_EVERY) {
        actions.push(Action::FindServers);
        page.searched = Some(Instant::now());
    }
    sheet(ctx, "servers-window", "MÁY CHỦ", |ui| {
        in_use(ui, settings, status);
        network(ui, settings, status, actions);
        by_address(ui, page, actions);
        this_device(ui, page, settings, status, actions);
        ui.add_space(4.0 * theme::density(ui));
        muted(
            ui,
            "The game's files come from the server as the game reads them, and stay on this device: later starts \
             ask it only for what is not here yet.",
        );
    })
}

/// The server in use: its name, address and state, what it is doing.
fn in_use(ui: &mut egui::Ui, settings: &Settings, status: &Status) {
    let server = settings.server.trim();
    panel(ui, |ui| {
        if server.is_empty() {
            card_title(ui, "Chưa có máy chủ", Some(("Chưa chọn", WARN)));
            small(ui, "Chọn máy trong mạng này hoặc nhập địa chỉ.", theme::MUTED);
            return;
        }
        let stream = status.stream.filter(|(s, _)| *s == server).map(|(_, s)| s);
        let name = server_name(status, server);
        card_title(ui, &name, stream.map(stream_state));
        if name != server {
            small(ui, server, theme::MUTED);
        }
        match stream {
            _ if settings.source != Source::Server => {
                small(ui, "Đang đọc tệp từ thư mục trên thiết bị (Cài đặt › Trò chơi › Tệp trò chơi).", WARN)
            }
            Some(s) => {
                if s.cache_bytes > 0 {
                    small(ui, &format!("{} trên thiết bị này", bytes(s.cache_bytes)), theme::MUTED);
                }
                stream_progress(ui, s);
            }
            // The game runs from another server, or from the folder.
            None if status.started.is_some() => small(ui, "Áp dụng từ lần mở sau. Phiên chơi hiện tại vẫn tiếp tục.", WARN),
            None => small(ui, "Đang kết nối…", theme::MUTED),
        }
    });
}

/// The computers sharing the game on this network, searched all the while.
fn network(ui: &mut egui::Ui, settings: &Settings, status: &Status, actions: &mut Vec<Action>) {
    section(ui, "TRONG MẠNG NÀY", scanning);
    let current = settings.server.trim();
    for server in status.servers {
        let (found, check) = server;
        let using = settings.source == Source::Server && !current.is_empty() && is_server(server, current);
        let (what, usable) = match check {
            None => (format!("{} · đang hỏi…", found.address), true),
            Some(Check::Ready { ms: 0, .. }) => (format!("{} · dưới 1 ms", found.address), true),
            Some(Check::Ready { ms, .. }) => (format!("{} · {ms} ms", found.address), true),
            // Another game build, another version of the app.
            Some(Check::Unusable(why)) => (why.clone(), false),
            Some(Check::Unreachable(_)) => (format!("{} · không phản hồi", found.address), true),
        };
        ui.push_id(&found.address, |ui| {
            setting(ui, &found.name, Control::Action, Some(&what), |ui| {
                if using {
                    ui.add_enabled_ui(false, |ui| action_button(ui, "Đang dùng"));
                } else if ui.add_enabled_ui(usable, |ui| action_button(ui, "Dùng")).inner.clicked() {
                    actions.push(Action::UseServer(found.address.clone()));
                }
            });
        });
    }
    if status.servers.is_empty() {
        muted(
            ui,
            "Looking for computers sharing the game. On the one with the game: Glue 5 › Settings › Game › Share on \
             the network, or glue5-serve; both on the same network.",
        );
        if cfg!(target_os = "ios") {
            muted(ui, "Nếu không thấy máy nào, cấp quyền mạng nội bộ trong Cài đặt › Quyền riêng tư & Bảo mật.");
        }
    }
}

/// An arc turning: the network is searched. Ten frames a second, not every
/// frame (the page draws only when something moves).
fn scanning(ui: &mut egui::Ui) {
    let scale = theme::density(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(40.0, 40.0) * scale, Sense::hover());
    let turn = (ui.input(|i| i.time) * 10.0).floor() as f32 / 10.0 * std::f32::consts::TAU * 0.8;
    let (c, r) = (rect.center(), 7.0 * scale);
    let points = (0..=16).map(|i| {
        let a = turn + i as f32 / 16.0 * std::f32::consts::TAU * 0.7;
        pos2(c.x + r * a.cos(), c.y + r * a.sin())
    });
    ui.painter().add(egui::Shape::line(points.collect(), Stroke::new(2.0 * scale, theme::MUTED)));
    ui.ctx().request_repaint_after(Duration::from_millis(100));
}

/// A server by its address: typed, then Connect (or Enter).
fn by_address(ui: &mut egui::Ui, page: &mut ServersPage, actions: &mut Vec<Action>) {
    let scale = theme::density(ui);
    section(ui, "NHẬP ĐỊA CHỈ", |_| {});
    ui.horizontal(|ui| {
        let button = 104.0 * scale;
        let field = ui.available_width() - button - ui.spacing().item_spacing.x;
        let typed = ui
            .allocate_ui(vec2(field, 44.0 * scale), |ui| {
                text_field(ui, "", TextKind::Address, &mut page.address, "192.168.1.20 hoặc ten-may.local", 120)
            })
            .inner;
        let enter = typed.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let ready = !page.address.trim().is_empty();
        let connect = egui::Button::new(RichText::new("Kết nối").size(14.0 * scale))
            .corner_radius((12.0 * scale) as u8)
            .min_size(vec2(button, 44.0 * scale));
        let clicked = focus(ui.add_enabled(ready, connect)).clicked();
        if ready && (clicked || enter) {
            actions.push(Action::UseServer(page.address.trim().to_owned()));
            page.address.clear();
        }
    });
    muted(ui, "Dùng khi máy chủ ở mạng khác hoặc qua VPN. Cổng mặc định là 47100.");
}

/// What this device keeps: the whole game or what was read, and deleting it.
fn this_device(
    ui: &mut egui::Ui,
    page: &mut ServersPage,
    settings: &mut Settings,
    status: &Status,
    actions: &mut Vec<Action>,
) {
    section(ui, "TRÊN THIẾT BỊ", |_| {});
    let stream = status.stream.map(|(_, s)| s);
    if paths::can_download_all() {
        let mut shown = match stream {
            Some(s) if s.complete => "Đã tải đủ để chơi ngoại tuyến".to_owned(),
            Some(s) if s.no_room => format!("Đang chờ dung lượng: {} trên {}", bytes(s.cache_bytes), bytes(s.total_bytes)),
            Some(s) if s.filling => format!("{} trên {}", bytes(s.cache_bytes), bytes(s.total_bytes)),
            Some(s) if s.total_bytes > 0 => format!("{} để chơi ngoại tuyến", bytes(s.total_bytes)),
            _ => "Để chơi ngoại tuyến".to_owned(),
        };
        // The stream follows the setting when the game no longer runs on it.
        if status.started.is_some() && stream.is_some_and(|s| (s.filling || s.complete) != settings.download_all) {
            shown.push_str(" · lần mở sau");
        }
        setting(ui, "Tải toàn bộ trò chơi", Control::Toggle, Some(&shown), |ui| toggle(ui, &mut settings.download_all))
            .on_hover_text("Ưu tiên thấp nhất. Các tệp trò chơi đang cần sẽ được tải trước.");
    }
    let copy = stream.map_or(0, |s| s.cache_bytes);
    let what = match copy {
        0 => "Chưa có dữ liệu".to_owned(),
        n if stream.is_some_and(|s| s.complete) => format!("{}: toàn bộ dữ liệu", bytes(n)),
        n => format!("{}, lưu cho những lần mở sau", bytes(n)),
    };
    page.deleting &= copy > 0 && status.started.is_none();
    setting(ui, "Bản sao trên thiết bị", Control::Action, Some(&what), |ui| {
        ui.add_enabled_ui(status.started.is_none() && copy > 0, |ui| {
            if action_button(ui, if page.deleting { "Xác nhận?" } else { "Xóa" }).clicked() {
                if page.deleting {
                    actions.push(Action::ClearCopy);
                }
                page.deleting = !page.deleting;
            }
        })
        .response
    })
    .on_hover_text("Dữ liệu từ máy chủ. Có thể tải lại khi cần.");
}

/// A section's title, and on its right what `right` adds (a button).
fn section(ui: &mut egui::Ui, title: &str, right: impl FnOnce(&mut egui::Ui)) {
    let scale = theme::density(ui);
    ui.add_space(6.0 * scale);
    ui.horizontal(|ui| {
        ui.label(theme::heading(title, 17.0 * scale).color(theme::MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), right);
    });
}

/// The server in use's box: the settings rows' look.
fn panel(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    let scale = theme::density(ui);
    egui::Frame::new()
        .fill(Color32::from_gray(28))
        .corner_radius((14.0 * scale) as u8)
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
