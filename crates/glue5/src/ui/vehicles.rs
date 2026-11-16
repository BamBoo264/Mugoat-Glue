//! Giao diện quản lý mod xe cho GLUE 5:
//! Thêm mod xe (.zip, .rar, .7z, .oiv), tự động nhận diện xe Add-on & Replace,
//! bật/tắt hoặc xóa mod, hướng dẫn gọi xe trong game qua F4/F8.

use egui::{Color32, RichText};
use glue5_runtime::vehicle_mods::{self, VehicleMod, VehicleModKind};

use super::pages::{card_title, sheet, small};
use super::widgets::{Control, action_button, setting, toggle};
use super::{Action, Status, theme};

#[derive(Default)]
pub struct VehiclesPage {
    pub message: Option<(String, bool)>, // (nội dung, là lỗi)
    pub importing: bool,
}

impl VehiclesPage {
    pub fn set_status(&mut self, text: String, is_err: bool) {
        self.message = Some((text, is_err));
        self.importing = false;
    }
}

/// Trả về true khi nhấn đóng (X) hoặc Esc.
pub(super) fn vehicles_page(
    ctx: &egui::Context,
    page: &mut VehiclesPage,
    status: &Status,
    actions: &mut Vec<Action>,
) -> bool {
    sheet(ctx, "vehicles-window", "QUẢN LÝ MOD XE", |ui| {
        let scale = theme::density(ui);
        let db = vehicle_mods::load_db(status.user_dir);
        let mods = db.mods;

        // Thẻ hành động chính: Nhập mod
        ui.horizontal(|ui| {
            if page.importing {
                ui.label(RichText::new("⏳ Đang giải nén và phân tích mod xe...").color(Color32::from_rgb(240, 200, 100)));
            } else if action_button(ui, "➕ Nhập mod xe (.zip, .rar, .7z, .oiv)").clicked() {
                page.importing = true;
                page.message = None;
                actions.push(Action::ImportVehicleMod);
            }
        });

        if let Some((msg, is_err)) = &page.message {
            let color = if *is_err {
                Color32::from_rgb(240, 120, 120)
            } else {
                Color32::from_rgb(120, 210, 140)
            };
            ui.add_space(4.0 * scale);
            ui.label(RichText::new(msg).color(color));
        }

        ui.add_space(6.0 * scale);
        small(
            ui,
            "💡 Hướng dẫn: Nhập trực tiếp file nén .zip, .rar, .7z hoặc .oiv tải từ gta5-mods.com.\n\
             Trong trò chơi: Nhấn F4 hoặc F8 để mở menu và triệu hồi xe bất cứ lúc nào!",
            theme::MUTED,
        );

        ui.add_space(10.0 * scale);

        // --- PHẦN 1: XE BỔ SUNG (ADD-ON) ---
        let addons: Vec<&VehicleMod> = mods.iter().filter(|m| m.kind == VehicleModKind::Addon).collect();
        let addon_title = format!("XE BỔ SUNG (ADD-ON) · {} xe", addons.len());
        card_title(ui, &addon_title, None);

        if addons.is_empty() {
            small(ui, "Chưa có xe bổ sung nào. Nhấn 'Nhập mod xe' để cài xe mới.", theme::MUTED);
        } else {
            for m in addons {
                let id = m.id.clone();
                let mut on = m.enabled;
                let pack = m.pack_name.as_deref().unwrap_or(&m.id);
                let models_str = if m.models.is_empty() {
                    pack.to_string()
                } else {
                    m.models.join(", ")
                };
                let note = format!("Gói: {} · Mã gọi xe: {}", pack, models_str);

                setting(ui, &m.title, Control::Toggle, Some(&note), |ui| {
                    if toggle(ui, &mut on).changed() {
                        actions.push(Action::ToggleVehicleMod { id: id.clone(), enabled: on });
                    }
                    if action_button(ui, "Xóa").clicked() {
                        actions.push(Action::DeleteVehicleMod { id: id.clone() });
                    }
                });
            }
        }

        ui.add_space(14.0 * scale);

        // --- PHẦN 2: XE THAY THẾ (REPLACE) ---
        let replaces: Vec<&VehicleMod> = mods.iter().filter(|m| m.kind == VehicleModKind::Replace).collect();
        let replace_title = format!("XE THAY THẾ (REPLACE) · {} xe", replaces.len());
        card_title(ui, &replace_title, None);

        if replaces.is_empty() {
            small(ui, "Chưa có xe thay thế nào.", theme::MUTED);
        } else {
            for m in replaces {
                let id = m.id.clone();
                let mut on = m.enabled;
                let rep_str = if m.replaces.is_empty() {
                    m.id.clone()
                } else {
                    m.replaces.join(", ")
                };
                let note = format!("Thay thế phương tiện gốc: {}", rep_str);

                setting(ui, &m.title, Control::Toggle, Some(&note), |ui| {
                    if toggle(ui, &mut on).changed() {
                        actions.push(Action::ToggleVehicleMod { id: id.clone(), enabled: on });
                    }
                    if action_button(ui, "Xóa").clicked() {
                        actions.push(Action::DeleteVehicleMod { id: id.clone() });
                    }
                });
            }
        }

        // --- PHẦN 3: GÓI MOD OPENIV (.OIV) KHÁC NẾU CÓ ---
        let packages: Vec<&VehicleMod> = mods.iter().filter(|m| m.kind == VehicleModKind::Package).collect();
        if !packages.is_empty() {
            ui.add_space(14.0 * scale);
            card_title(ui, &format!("GÓI MOD KHÁC · {} gói", packages.len()), None);
            for m in packages {
                let id = m.id.clone();
                let mut on = m.enabled;
                setting(ui, &m.title, Control::Toggle, Some("Gói tài nguyên OpenIV"), |ui| {
                    if toggle(ui, &mut on).changed() {
                        actions.push(Action::ToggleVehicleMod { id: id.clone(), enabled: on });
                    }
                    if action_button(ui, "Xóa").clicked() {
                        actions.push(Action::DeleteVehicleMod { id: id.clone() });
                    }
                });
            }
        }
    })
}
