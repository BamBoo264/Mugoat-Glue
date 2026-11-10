//! Trình chỉnh sửa trực quan bố cục nút thao tác cảm ứng (vị trí, kích cỡ, độ mờ).

use egui::{
    Align, Align2, Area, Color32, Context, FontFamily, FontId, Frame, Id, Key, Layout, Order, Pos2, Rect, RichText,
    Sense, Stroke, StrokeKind, Vec2, pos2, vec2,
};

use super::theme::{ACCENT, ACCENT_INK, BORDER, BORDER_STRONG, CARD, CARD_HOVER, MUTED, SURFACE, TEXT, apply_density, heading, layout_scale};
use super::widgets;
use crate::settings::{ButtonOverride, Settings};
use crate::touch::{EditorLayout, Touch, control_scale};

#[derive(Default)]
pub(super) struct TouchEditorState {
    pub layout: EditorLayout,
    pub selected_id: Option<String>,
    pub dragging_id: Option<String>,
    pub panel_bottom: bool,
}

/// Vẽ trình chỉnh sửa vị trí, kích cỡ và độ mờ cho từng nút thao tác.
/// Trả về `true` khi người dùng bấm "Xong" để quay lại Cài đặt.
pub(super) fn touch_editor_page(
    ctx: &Context,
    settings: &mut Settings,
    started: bool,
    touch_preview: &mut Touch,
    state: &mut TouchEditorState,
) -> bool {
    let screen = ctx.viewport_rect();
    let safe = ctx.content_rect();
    let scale = layout_scale(safe);

    if state.selected_id.is_none() {
        state.selected_id = Some("move_stick".into());
    }

    let cscale = control_scale(screen.size(), settings.touch_size);
    touch_preview.layout(screen.size(), safe, cscale);
    touch_preview.set_overrides(settings.touch_buttons.clone());

    let mut controls = touch_preview.editable_controls(state.layout);
    if !controls.iter().any(|c| state.selected_id.as_deref() == Some(c.id.as_str())) {
        state.selected_id = controls.first().map(|c| c.id.clone());
    }

    // Lớp nền + tất cả nút thao tác (tầng giữa, bên dưới thanh công cụ).
    Area::new(Id::new("touch-editor-canvas"))
        .order(Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
            let painter = ui.painter();

            // Nền lưới tối dịu giúp nhìn rõ nút kể cả khi độ mờ thấp.
            let backdrop_alpha = if started { 155 } else { 205 };
            painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(8, 11, 17, backdrop_alpha));
            paint_grid(painter, rect, safe, scale);

            // Xử lý chọn và kéo nút trực tiếp trên màn hình.
            let interact_pos = response.interact_pointer_pos();
            if (response.drag_started() || response.clicked())
                && let Some(pos) = interact_pos
            {
                let picked = controls
                    .iter()
                    .filter_map(|c| {
                        let d = c.at.distance(pos);
                        (d <= c.radius * 1.3 + 8.0).then_some((d, c.id.as_str()))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, id)| id.to_string());
                if let Some(id) = picked {
                    state.selected_id = Some(id.clone());
                    if response.drag_started() {
                        state.dragging_id = Some(id);
                    }
                }
            }

            if response.dragged()
                && let Some(id) = state.dragging_id.clone()
            {
                let delta = ui.input(|i| i.pointer.delta());
                if delta != Vec2::ZERO
                    && let Some(ctrl) = controls.iter().find(|c| c.id == id)
                {
                    let ov = settings.touch_buttons.entry(id.clone()).or_default();
                    touch_preview.drag_control(ctrl, ov, delta);
                    if ov.is_default() {
                        settings.touch_buttons.remove(&id);
                    }
                    touch_preview.set_overrides(settings.touch_buttons.clone());
                    controls = touch_preview.editable_controls(state.layout);
                }
            }
            if !response.dragged() {
                state.dragging_id = None;
            }

            touch_preview.draw_editor_preview(
                painter,
                settings.touch_opacity,
                state.layout,
                state.selected_id.as_deref(),
            );

            if let Some(sel) = controls.iter().find(|c| state.selected_id.as_deref() == Some(c.id.as_str())) {
                paint_selected_badge(painter, safe, sel, scale);
            }
        });

    // Bảng điều khiển gọn nổi trên/dưới vùng an toàn.
    let mut done = false;
    let bar_w = (safe.width() - 16.0 * scale).clamp(240.0, 560.0 * scale);
    let anchor = if state.panel_bottom {
        (Align2::CENTER_BOTTOM, pos2(safe.center().x, safe.bottom() - 8.0 * scale))
    } else {
        (Align2::CENTER_TOP, pos2(safe.center().x, safe.top() + 8.0 * scale))
    };

    Area::new(Id::new("touch-editor-bar"))
        .order(Order::Foreground)
        .anchor(anchor.0, Vec2::ZERO)
        .fixed_pos(anchor.1)
        .show(ctx, |ui| {
            apply_density(ui, scale);
            let frame = Frame::new()
                .fill(Color32::from_rgba_unmultiplied(SURFACE.r(), SURFACE.g(), SURFACE.b(), 238))
                .stroke(Stroke::new(1.0, BORDER_STRONG))
                .corner_radius((16.0 * scale) as u8)
                .inner_margin((10.0 * scale) as i8);
            frame.show(ui, |ui| {
                ui.set_width(bar_w - 20.0 * scale);
                ui.spacing_mut().item_spacing = vec2(6.0 * scale, 6.0 * scale);

                let selected_ctrl = controls
                    .iter()
                    .find(|c| state.selected_id.as_deref() == Some(c.id.as_str()))
                    .cloned();

                // Hàng 1: Tiêu đề nút đang chọn + nút chuyển vị trí thanh + Đặt lại hết + Xong.
                ui.horizontal(|ui| {
                    let (bar, _) = ui.allocate_exact_size(vec2(3.5 * scale, 18.0 * scale), Sense::hover());
                    ui.painter().rect_filled(bar, 2.0, ACCENT);
                    let title = selected_ctrl
                        .as_ref()
                        .map(|c| c.label)
                        .unwrap_or("TÙY CHỈNH NÚT THAO TÁC");
                    ui.label(heading(title, 17.0 * scale).color(TEXT));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if done_pill(ui, "Xong", scale).clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                            done = true;
                        }
                        if compact_pill(ui, "Mặc định", false, scale).clicked() {
                            settings.touch_buttons.clear();
                        }
                        let flip_label = if state.panel_bottom { "Đưa lên" } else { "Đưa xuống" };
                        if compact_pill(ui, flip_label, false, scale).clicked() {
                            state.panel_bottom = !state.panel_bottom;
                        }
                    });
                });

                // Hàng 2: Chọn chế độ/bố cục điều khiển xem trước.
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(5.0 * scale, 5.0 * scale);
                    for layout in EditorLayout::ALL {
                        let active = state.layout == layout;
                        if compact_pill(ui, layout.label(), active, scale).clicked() {
                            state.layout = layout;
                        }
                    }
                });

                // Hàng 3: Kích cỡ & Độ mờ của nút đang chọn.
                if let Some(ctrl) = selected_ctrl {
                    let mut ov = settings.touch_buttons.get(&ctrl.id).copied().unwrap_or_default();
                    let prev = ov;
                    let slider_w = (ui.available_width() * 0.42).clamp(86.0 * scale, 165.0 * scale);
                    ui.spacing_mut().slider_width = slider_w;
                    ui.spacing_mut().interact_size.y = 32.0 * scale;

                    ui.horizontal_wrapped(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Cỡ").size(13.0 * scale).color(MUTED));
                            widgets::slider(ui, &mut ov.size, 0.5..=2.0, 0.05, "×", 2);
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Độ mờ").size(13.0 * scale).color(MUTED));
                            widgets::slider(ui, &mut ov.opacity, 0.15..=1.0, 0.05, "%", 0);
                        });
                        if settings.touch_buttons.contains_key(&ctrl.id)
                            && compact_pill(ui, "Đặt lại nút", false, scale).clicked()
                        {
                            ov = ButtonOverride::default();
                        }
                    });

                    if ov != prev {
                        let clamped = ov.clamped();
                        if clamped.is_default() {
                            settings.touch_buttons.remove(&ctrl.id);
                        } else {
                            settings.touch_buttons.insert(ctrl.id, clamped);
                        }
                    }
                } else {
                    ui.label(
                        RichText::new("Chạm vào nút bất kỳ bên ngoài rồi kéo để đặt lại vị trí.")
                            .size(12.5 * scale)
                            .color(MUTED),
                    );
                }
            });
        });

    done
}

fn paint_grid(painter: &egui::Painter, rect: Rect, safe: Rect, scale: f32) {
    let step = 40.0 * scale;
    let line_color = Color32::from_white_alpha(10);
    let mut x = safe.left();
    while x <= safe.right() {
        painter.vline(x, safe.y_range(), Stroke::new(1.0, line_color));
        x += step;
    }
    let mut y = safe.top();
    while y <= safe.bottom() {
        painter.hline(safe.x_range(), y, Stroke::new(1.0, line_color));
        y += step;
    }
    painter.vline(safe.center().x, rect.y_range(), Stroke::new(1.0, Color32::from_rgba_unmultiplied(244, 186, 82, 28)));
    painter.rect_stroke(
        safe,
        12.0 * scale,
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(244, 186, 82, 55)),
        StrokeKind::Inside,
    );
}

fn paint_selected_badge(painter: &egui::Painter, safe: Rect, sel: &crate::touch::EditableControl, scale: f32) {
    let font = FontId::new(12.0 * scale, FontFamily::Proportional);
    let galley = painter.layout_no_wrap(sel.label.to_string(), font, ACCENT_INK);
    let pad = vec2(8.0 * scale, 4.0 * scale);
    let box_size = galley.size() + pad * 2.0;
    let above_y = sel.at.y - sel.radius - 12.0 * scale - box_size.y / 2.0;
    let y = if above_y - box_size.y / 2.0 >= safe.top() + 4.0 {
        above_y
    } else {
        (sel.at.y + sel.radius + 12.0 * scale + box_size.y / 2.0).min(safe.bottom() - box_size.y / 2.0 - 4.0)
    };
    let x = sel
        .at
        .x
        .clamp(safe.left() + box_size.x / 2.0 + 4.0, safe.right() - box_size.x / 2.0 - 4.0);
    let r = Rect::from_center_size(pos2(x, y), box_size);
    painter.rect_filled(r, 6.0 * scale, ACCENT);
    painter.galley(r.min + pad, galley, ACCENT_INK);
}

fn compact_pill(ui: &mut egui::Ui, label: &str, active: bool, scale: f32) -> egui::Response {
    let font = FontId::new(12.5 * scale, FontFamily::Proportional);
    let pad = vec2(10.0 * scale, 6.0 * scale);
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, if active { ACCENT_INK } else { TEXT });
    let size = galley.size() + pad * 2.0;
    let (rect, response) = ui.allocate_exact_size(vec2(size.x, size.y.max(28.0 * scale)), Sense::click());
    let fill = if active {
        ACCENT
    } else if response.hovered() {
        CARD_HOVER
    } else {
        CARD
    };
    let stroke = if active {
        Stroke::new(1.0, ACCENT)
    } else {
        Stroke::new(1.0, BORDER)
    };
    ui.painter().rect(rect, 9.0 * scale, fill, stroke, StrokeKind::Inside);
    ui.painter()
        .galley(pos2(rect.left() + pad.x, rect.center().y - galley.size().y / 2.0), galley, TEXT);
    widgets::focus(response)
}

fn done_pill(ui: &mut egui::Ui, label: &str, scale: f32) -> egui::Response {
    let font = FontId::new(13.0 * scale, FontFamily::Name("condensed".into()));
    let pad = vec2(14.0 * scale, 6.0 * scale);
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, ACCENT_INK);
    let size = galley.size() + pad * 2.0;
    let (rect, response) = ui.allocate_exact_size(vec2(size.x, size.y.max(28.0 * scale)), Sense::click());
    ui.painter().rect_filled(rect, 9.0 * scale, ACCENT);
    ui.painter()
        .galley(pos2(rect.left() + pad.x, rect.center().y - galley.size().y / 2.0), galley, ACCENT_INK);
    widgets::focus(response)
}
