//! Friends, as Social Club's overlay had them: the friends with what they
//! play (Join, Invite), the friend requests, the game invites, and finding
//! players (the ones met in sessions, or by friend code); and its
//! notifications.
use glue5_runtime::friends::{self, AddError, FriendInfo, Notice, Playing, SessionKind, Snapshot};

use super::pages::sheet;
use super::widgets::{Control, action_button, action_buttons, focus, segmented, setting, text_field};
use super::{Action, TextKind, WARN, theme};
use egui::{Color32, RichText, Stroke};

/// The page's tabs, Social Club's: Friends, Friend Requests, Game Invites,
/// Find Players.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Friends,
    Requests,
    Invites,
    Find,
}

/// A notification: title, the friend's name, a line; the tab it opens.
#[derive(Clone, Debug)]
pub struct Toast {
    pub title: &'static str,
    pub name: String,
    pub text: String,
    pub tab: Tab,
}

/// A friends' notification, worded as Social Club's overlay did.
pub fn toast(notice: &Notice) -> Toast {
    let toast = |title, name: &str, text: &str, tab| Toast { title, name: name.to_owned(), text: text.to_owned(), tab };
    match notice {
        Notice::Summary { online, requests } => {
            let text = match (*online, *requests) {
                (0, r) => format!("{r} yêu cầu"),
                (o, 0) => format!("{o} trực tuyến"),
                (o, r) => format!("{o} trực tuyến · {r} yêu cầu"),
            };
            let tab = if *requests > 0 { Tab::Requests } else { Tab::Friends };
            Toast { title: "Bạn bè", name: String::new(), text, tab }
        }
        Notice::Online(name) => toast("Bạn bè trực tuyến", name, "Vừa trực tuyến.", Tab::Friends),
        Notice::Offline(name) => toast("Bạn bè ngoại tuyến", name, "Vừa ngoại tuyến.", Tab::Friends),
        Notice::Request(name) => toast("Lời mời kết bạn", name, "Muốn kết bạn với bạn.", Tab::Requests),
        Notice::Accepted(name) => toast("Lời mời kết bạn", name, "Đã chấp nhận lời mời kết bạn của bạn.", Tab::Friends),
        Notice::Invite(name) => toast("Lời mời chơi chung", name, "Đã gửi lời mời chơi chung.", Tab::Invites),
    }
}

/// A notification's card, `width` wide.
pub(super) fn draw_toast(ui: &mut egui::Ui, t: &Toast, width: f32) {
    let scale = theme::density(ui);
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(18, 18, 20, 240))
        .stroke(Stroke::new(1.0, Color32::from_gray(60)))
        .corner_radius((14.0 * scale) as u8)
        .inner_margin(egui::Margin::symmetric((14.0 * scale) as i8, (10.0 * scale) as i8))
        .show(ui, |ui| {
            ui.set_width(width - 28.0 * scale);
            ui.horizontal_top(|ui| {
                avatar(ui, &t.name, 34.0 * scale);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0 * scale;
                    ui.label(theme::heading(t.title, 15.0 * scale).color(Color32::WHITE));
                    if !t.name.is_empty() {
                        ui.label(RichText::new(&t.name).size(13.0 * scale).strong().color(Color32::WHITE));
                    }
                    ui.label(RichText::new(&t.text).size(12.0 * scale).color(theme::MUTED));
                });
            });
        });
}

/// A round badge with a name's initial (Social Club showed avatars).
fn avatar(ui: &mut egui::Ui, name: &str, size: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let initial = name.chars().next().map_or_else(|| "♡".to_owned(), |c| c.to_uppercase().collect());
    // A colour of its own per name.
    let hue = name.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b))) % 360;
    let color = egui::ecolor::Hsva::new(hue as f32 / 360.0, 0.45, 0.55, 1.0);
    ui.painter().circle_filled(rect.center(), size / 2.0, color);
    let font = egui::FontId::new(size * 0.5, egui::FontFamily::Name("condensed".into()));
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, initial, font, Color32::WHITE);
}

/// What the page keeps between frames.
#[derive(Default)]
pub struct FriendsPage {
    pub(super) tab: Tab,
    /// The code being typed.
    code: String,
    /// What came of the last thing done (and whether it went wrong).
    message: Option<(String, bool)>,
    /// The friend whose removal waits for a second press.
    removing: Option<String>,
}

/// What the game lets the page do: `started`, a game runs; `online`, it
/// reaches GTA Online (the story too: friends are joined from it; invited
/// to its session in GTA Online).
pub struct Context {
    pub started: bool,
    pub online: bool,
}

/// True when closed.
pub(super) fn friends_page(ctx: &egui::Context, page: &mut FriendsPage, game: Context, actions: &mut Vec<Action>) -> bool {
    // What the friends do comes from the network: the page follows it.
    ctx.request_repaint_after(std::time::Duration::from_millis(500));
    let snapshot = friends::snapshot();
    sheet(ctx, "friends-window", "BẠN BÈ", |ui| {
        let scale = theme::density(ui);
        let count = |label: &str, n: usize| if n > 0 { format!("{label} {n}") } else { label.to_owned() };
        let invites = snapshot.friends.iter().filter(|f| f.invited).count();
        let tabs = [
            (Tab::Friends, "BẠN BÈ".to_owned()),
            (Tab::Requests, count("YÊU CẦU", snapshot.requests.len())),
            (Tab::Invites, count("LỜI MỜI", invites)),
            (Tab::Find, "TÌM".to_owned()),
        ];
        let tabs: Vec<(Tab, &str)> = tabs.iter().map(|(t, l)| (*t, l.as_str())).collect();
        if segmented(ui, &mut page.tab, &tabs, 40.0 * scale) {
            page.message = None;
        }
        ui.add_space(10.0 * scale);
        if let Some((text, wrong)) = &page.message {
            ui.label(RichText::new(text).size(12.0 * scale).color(if *wrong { WARN } else { theme::MUTED }));
        }
        match page.tab {
            Tab::Friends => friends_tab(ui, &snapshot, page, &game, actions),
            Tab::Requests => requests_tab(ui, &snapshot, page),
            Tab::Invites => invites_tab(ui, &snapshot, &game, actions),
            Tab::Find => find_tab(ui, &snapshot, page),
        }
    })
}

fn muted(ui: &mut egui::Ui, text: &str) {
    let scale = theme::density(ui);
    ui.label(RichText::new(text).size(12.0 * scale).color(theme::MUTED));
}

fn heading(ui: &mut egui::Ui, text: &str) {
    let scale = theme::density(ui);
    ui.add_space(4.0 * scale);
    ui.label(theme::heading(text, 17.0 * scale).color(theme::MUTED));
}

fn friends_tab(ui: &mut egui::Ui, snapshot: &Snapshot, page: &mut FriendsPage, game: &Context, actions: &mut Vec<Action>) {
    let online = snapshot.friends.iter().filter(|f| f.playing.is_some()).count();
    heading(ui, &format!("TRỰC TUYẾN {online} / {}", snapshot.friends.len()));
    if snapshot.friends.is_empty() {
        muted(ui, "Chưa có bạn bè. Bạn có thể tìm người đã gặp trong phiên hoặc nhập mã bạn bè.");
    }
    for f in &snapshot.friends {
        friend_row(ui, f, page, game, actions);
    }
    ui.add_space(4.0);
    muted(
        ui,
        "Friends find each other directly, with no server: they see when you play, invite you and join you. \
         What you say to each other is encrypted; like any player you meet online, a friend sees your Internet \
         address.",
    );
}

fn friend_row(ui: &mut egui::Ui, f: &FriendInfo, page: &mut FriendsPage, game: &Context, actions: &mut Vec<Action>) {
    let status = match (f.playing, f.kind) {
        (None, _) => "Ngoại tuyến",
        (Some(Playing::Menu), _) => "Trực tuyến",
        (Some(Playing::Story), _) => "Đang chơi cốt truyện GTA V",
        (Some(Playing::Online), Some(SessionKind::Friends)) => "Đang chơi GTA Online, phiên riêng cho bạn bè",
        (Some(Playing::Online), Some(SessionKind::Closed)) => "Đang chơi GTA Online, phiên chỉ có khách mời",
        (Some(Playing::Online), Some(SessionKind::Solo)) => "Đang chơi GTA Online, phiên một người",
        (Some(Playing::Online), _) => "Đang chơi GTA Online, phiên công khai",
    };
    // The font has no round dot: its bullet.
    let dot = if f.playing.is_some() { "• " } else { "" };
    let sure = page.removing.as_deref() == Some(f.key.as_str());
    // Its session, when friends may join it (or it invited); an invite to
    // this player's, playing GTA Online.
    let can_join = f.lobby.is_some() && f.playing == Some(Playing::Online) && (!game.started || game.online);
    let can_invite = game.online && friends::in_gta_online() && f.playing.is_some();
    ui.push_id(&f.key, |ui| {
        setting(ui, &format!("{dot}{}", f.name), Control::Actions, Some(status), |ui| {
            // A phone's row has room for short words.
            let short = ui.available_width() < 250.0 * theme::density(ui);
            let mut labels = vec![if sure { "Gỡ?" } else { "🗙" }];
            labels.extend(can_join.then_some(if short { "Tham gia" } else { "Tham gia trò chơi" }));
            labels.extend(can_invite.then_some(if short { "Mời" } else { "Mời chơi" }));
            let mut buttons = action_buttons(ui, &labels).into_iter();
            if buttons.next().is_some_and(|b| b.on_hover_text("Gỡ bạn bè").clicked()) {
                if sure {
                    friends::remove(&f.key);
                    page.message = Some((format!("Đã gỡ {} khỏi danh sách bạn bè.", f.name), false));
                    page.removing = None;
                } else {
                    page.removing = Some(f.key.clone());
                }
            }
            if can_join && buttons.next().is_some_and(|b| b.clicked()) {
                join(f, game, actions);
                page.message = Some((format!("Đang tham gia cùng {}…", f.name), false));
            }
            if can_invite && buttons.next().is_some_and(|b| b.clicked()) {
                friends::invite(&f.key);
                page.message = Some((format!("Đã mời {} chơi", f.name), false));
            }
        });
    });
}

fn requests_tab(ui: &mut egui::Ui, snapshot: &Snapshot, page: &mut FriendsPage) {
    heading(ui, "LỜI MỜI KẾT BẠN");
    if snapshot.requests.is_empty() {
        muted(ui, "Không có lời mời đang chờ.");
    }
    for (key, name) in &snapshot.requests {
        ui.push_id(key, |ui| {
            setting(ui, name, Control::Actions, Some("Muốn kết bạn với bạn."), |ui| {
                let [decline, accept] = <[_; 2]>::try_from(action_buttons(ui, &["Từ chối", "Chấp nhận"])).unwrap();
                if decline.clicked() {
                    friends::answer(key, false);
                    page.message = Some((format!("Đã từ chối lời mời của {name}."), false));
                }
                if accept.clicked() {
                    friends::answer(key, true);
                    page.message = Some((format!("Đã chấp nhận lời mời của {name}."), false));
                }
            });
        });
    }
    if !snapshot.asked.is_empty() {
        heading(ui, "ĐÃ GỬI");
        for a in &snapshot.asked {
            let state = match (a.declined, a.reached) {
                (true, _) => "Đã từ chối",
                (false, true) => "Lời mời kết bạn đang chờ",
                (false, false) => "Đang tìm. Người đó cần mở Mugoat Glue.",
            };
            ui.push_id(&a.id, |ui| {
                setting(ui, &a.label, Control::Action, Some(state), |ui| {
                    if action_button(ui, if a.declined { "Bỏ qua" } else { "Hủy" }).clicked() {
                        friends::cancel(&a.id);
                    }
                });
            });
        }
    }
}

fn invites_tab(ui: &mut egui::Ui, snapshot: &Snapshot, game: &Context, actions: &mut Vec<Action>) {
    let invites: Vec<&FriendInfo> = snapshot.friends.iter().filter(|f| f.invited).collect();
    heading(ui, "LỜI MỜI CHƠI");
    if invites.is_empty() {
        muted(ui, "Không có lời mời chơi đang chờ.");
    }
    for f in invites {
        ui.push_id(("invite", &f.key), |ui| {
            setting(ui, &f.name, Control::Actions, Some("Đã gửi lời mời chơi chung."), |ui| {
                // Accepted from the start page or a game that reaches GTA
                // Online (not the benchmark).
                let can_accept = !game.started || game.online;
                let labels: &[&str] = if can_accept { &["Từ chối", "Chấp nhận"] } else { &["Từ chối"] };
                let buttons = action_buttons(ui, labels);
                if buttons[0].clicked() {
                    friends::ignore_invite(&f.key);
                }
                if buttons.get(1).is_some_and(|b| b.clicked()) {
                    join(f, game, actions);
                }
            });
        });
    }
}

fn find_tab(ui: &mut egui::Ui, snapshot: &Snapshot, page: &mut FriendsPage) {
    heading(ui, "NGƯỜI ĐÃ GẶP");
    if snapshot.recent.is_empty() {
        muted(ui, "Những người bạn gặp trong GTA Online hiện ở đây. Bạn có thể gửi lời mời kết bạn.");
    }
    for r in snapshot.recent.iter().take(30) {
        let state = match (r.asked, r.here) {
            (true, _) => "Lời mời kết bạn đang chờ",
            (false, true) => "Trong phiên của bạn",
            (false, false) => "Đã chơi cùng",
        };
        ui.push_id(("recent", &r.key), |ui| {
            setting(ui, &r.name, Control::Action, Some(state), |ui| {
                // Asked while it plays in one of this player's lobbies.
                if !r.asked && r.here && action_button(ui, "Gửi lời mời kết bạn").clicked() {
                    friends::ask_player(&r.key);
                    page.message = Some((format!("Đã gửi lời mời tới {}", r.name), false));
                }
            });
        });
    }
    heading(ui, "MÃ BẠN BÈ");
    your_code(ui, snapshot, page);
    ask(ui, page);
}

/// Joins a friend's session: from the start page into its lobby first;
/// from the game, it asks whether to leave this session, or the story.
fn join(f: &FriendInfo, game: &Context, actions: &mut Vec<Action>) {
    if game.started {
        friends::join(&f.key);
        actions.push(Action::CloseFriends);
    } else if let Some(lobby) = &f.lobby {
        actions.push(Action::JoinFriend { key: f.key.clone(), lobby: lobby.clone() });
    }
}

fn your_code(ui: &mut egui::Ui, snapshot: &Snapshot, page: &mut FriendsPage) {
    let code = snapshot.code.clone().or_else(friends::my_code).unwrap_or_default();
    let what = "Người khác có thể dùng mã này để gửi lời mời kết bạn.";
    setting(ui, &code, Control::Actions, Some(what), |ui| {
        let [new, copy] = <[_; 2]>::try_from(action_buttons(ui, &["Mới", "Sao chép"])).unwrap();
        if new.on_hover_text("Tạo mã mới sẽ làm mã cũ hết hiệu lực.").clicked() {
            friends::new_code();
            page.message = Some(("Đã tạo mã mới. Mã cũ hết hiệu lực.".into(), false));
        }
        if copy.clicked() {
            ui.ctx().copy_text(code.clone());
            page.message = Some(("Đã sao chép mã.".into(), false));
        }
    });
}

fn ask(ui: &mut egui::Ui, page: &mut FriendsPage) {
    let scale = theme::density(ui);
    ui.horizontal(|ui| {
        let button = 92.0 * scale;
        let field = ui.available_width() - button - ui.spacing().item_spacing.x;
        let typed = ui
            .allocate_ui(egui::vec2(field, 44.0 * scale), |ui| {
                text_field(ui, "Mã", TextKind::Code, &mut page.code, "Mã của bạn bè", 24)
            })
            .inner;
        let enter = typed.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let send = egui::Button::new(RichText::new("Gửi").size(14.0 * scale))
            .corner_radius((12.0 * scale) as u8)
            .min_size(egui::vec2(button, 44.0 * scale));
        let sent = focus(ui.add_enabled(!page.code.trim().is_empty(), send));
        if sent.clicked() || enter && !page.code.trim().is_empty() {
            page.message = Some(match friends::ask(&page.code) {
                Ok(()) => {
                    page.code.clear();
                    ("Đã gửi lời mời. Họ sẽ thấy khi mở Mugoat Glue.".into(), false)
                }
                Err(AddError::Invalid) => ("Mã bạn bè không hợp lệ. Hãy kiểm tra lại.".into(), true),
                Err(AddError::Own) => ("Đây là mã của bạn.".into(), true),
                Err(AddError::Asked) => ("Đã có lời mời đang chờ.".into(), true),
            });
        }
    });
}
