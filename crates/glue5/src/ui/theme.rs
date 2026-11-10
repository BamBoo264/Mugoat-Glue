//! Tài nguyên đồ hoạ dùng chung và giao diện nền Obsidian-Gold cho Mugoat Glue.
use egui::*;

/// Nền tối sâu chủ đạo khớp với biểu tượng & màn hình thành phố về đêm.
pub(super) const INK: Color32 = Color32::from_rgb(9, 11, 16);
/// Bề mặt kính tối cho cửa sổ / trang cài đặt.
pub(super) const SURFACE: Color32 = Color32::from_rgb(15, 19, 28);
/// Nền thẻ nâng (cài đặt, chế độ chơi, bảng trạng thái).
pub(super) const CARD: Color32 = Color32::from_rgb(24, 29, 41);
/// Nền thẻ khi di chuột / chọn.
pub(super) const CARD_HOVER: Color32 = Color32::from_rgb(33, 40, 56);
/// Nền hốc điều khiển (thanh trượt, công tắc tắt, nhãn phụ).
pub(super) const CONTROL: Color32 = Color32::from_rgb(40, 48, 66);
/// Viền kính mảnh.
pub(super) const BORDER: Color32 = Color32::from_rgba_premultiplied(26, 26, 26, 26);
/// Viền nổi bật hơn khi chọn / di chuột.
pub(super) const BORDER_STRONG: Color32 = Color32::from_rgba_premultiplied(62, 62, 62, 62);
/// Màu nhấn Vàng Champagne chủ đạo đồng bộ với huy hiệu logo mới.
pub(super) const ACCENT: Color32 = Color32::from_rgb(244, 186, 82);
/// Lớp phủ vàng mờ cho thẻ đang chọn và tab hoạt động.
pub(super) const ACCENT_SOFT: Color32 = Color32::from_rgba_premultiplied(33, 25, 11, 34);
/// Màu chữ tương phản trên nút vàng.
pub(super) const ACCENT_INK: Color32 = Color32::from_rgb(18, 15, 10);
/// Màu nhấn Cyan điện cho chế độ Online & kết nối mạng.
pub(super) const CYAN: Color32 = Color32::from_rgb(76, 214, 224);
/// Màu chữ chính sáng rõ.
pub(super) const TEXT: Color32 = Color32::from_rgb(246, 248, 252);
/// Màu chữ phụ dịu mắt.
pub(super) const MUTED: Color32 = Color32::from_rgb(142, 153, 176);

pub(super) fn heading(text: &str, size: f32) -> RichText {
    RichText::new(text).font(FontId::new(size, FontFamily::Name("condensed".into())))
}

pub(super) fn install(ctx: &Context) {
    ctx.set_theme(Theme::Dark);
    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [
        ("ui", include_bytes!("../../assets/ui/Barlow-SemiBold.ttf").as_slice()),
        ("condensed", include_bytes!("../../assets/ui/BarlowCondensed-Bold.ttf").as_slice()),
    ] {
        fonts.font_data.insert(name.into(), FontData::from_static(bytes).into());
    }
    fonts.families.get_mut(&FontFamily::Proportional).unwrap().insert(0, "ui".into());
    let mut headings = vec!["condensed".into()];
    headings.extend(fonts.families[&FontFamily::Proportional].clone());
    fonts.families.insert(FontFamily::Name("condensed".into()), headings);
    ctx.set_fonts(fonts);
    ctx.all_styles_mut(|s| {
        s.text_styles.insert(TextStyle::Body, FontId::proportional(14.0));
        s.text_styles.insert(TextStyle::Button, FontId::proportional(14.0));
        s.text_styles.insert(TextStyle::Small, FontId::proportional(11.0));
        s.spacing.item_spacing = vec2(10.0, 10.0);
        s.spacing.button_padding = vec2(12.0, 8.0);
        s.spacing.interact_size.y = 44.0;
        s.visuals.override_text_color = Some(TEXT);
        s.visuals.panel_fill = INK;
        s.visuals.window_fill = Color32::from_rgba_unmultiplied(13, 17, 25, 244);
        s.visuals.window_corner_radius = 20.into();
        s.visuals.window_stroke = Stroke::new(1.0, BORDER_STRONG);
        s.visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(244, 186, 82, 72);
        s.visuals.selection.stroke = Stroke::new(1.5, ACCENT);
        for w in [&mut s.visuals.widgets.inactive, &mut s.visuals.widgets.hovered, &mut s.visuals.widgets.active] {
            w.corner_radius = 12.into();
            w.bg_fill = Color32::from_rgb(36, 44, 62);
            w.weak_bg_fill = Color32::from_rgb(30, 37, 52);
            w.bg_stroke = Stroke::new(1.0, BORDER);
            w.fg_stroke = Stroke::new(1.5, TEXT);
        }
        s.visuals.widgets.hovered.bg_fill = CARD_HOVER;
        s.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER_STRONG);
        s.visuals.widgets.active.bg_stroke = Stroke::new(2.0, ACCENT);
    });
}

/// Mật độ giao diện tự co giãn theo không gian hiển thị (kể cả điện thoại xoay ngang).
pub(super) fn layout_scale(rect: Rect) -> f32 {
    (rect.height() / 560.0).min(rect.width() / 780.0).clamp(0.75, 1.0)
}

pub(super) fn density(ui: &Ui) -> f32 {
    ui.style().text_styles[&TextStyle::Body].size / 14.0
}

pub(super) fn apply_density(ui: &mut Ui, scale: f32) {
    let style = ui.style_mut();
    for font in style.text_styles.values_mut() {
        font.size *= scale;
    }
    style.spacing.item_spacing *= scale;
    style.spacing.button_padding *= scale;
    style.spacing.interact_size *= scale;
}

pub(super) fn background(ctx: &Context) -> Option<TextureHandle> {
    let decode = || -> Result<ColorImage, png::DecodingError> {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(include_bytes!("../../assets/ui/home.png")));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info()?;
        let mut pixels = vec![0; reader.output_buffer_size().unwrap_or(0)];
        let info = reader.next_frame(&mut pixels)?;
        let size = [info.width as usize, info.height as usize];
        Ok(match info.color_type {
            png::ColorType::Rgb => ColorImage::from_rgb(size, &pixels[..info.buffer_size()]),
            png::ColorType::Rgba => ColorImage::from_rgba_unmultiplied(size, &pixels[..info.buffer_size()]),
            _ => return Err(png::DecodingError::IoError(std::io::Error::other("định dạng ảnh nền không hỗ trợ"))),
        })
    };
    match decode() {
        Ok(image) => Some(ctx.load_texture("home-background", image, TextureOptions::LINEAR)),
        Err(error) => {
            log::warn!("Không thể tải ảnh nền trang chính: {error}");
            None
        }
    }
}

pub(super) fn backdrop(ctx: &Context, texture: Option<&TextureHandle>) {
    let r = ctx.viewport_rect();
    let mut p = ctx.layer_painter(LayerId::background());
    p.set_clip_rect(r);
    p.rect_filled(r, 0, INK);
    if let Some(texture) = texture {
        let image_aspect = texture.size_vec2().x / texture.size_vec2().y;
        let aspect = r.width() / r.height().max(1.0);
        let uv = if aspect > image_aspect {
            let h = image_aspect / aspect;
            Rect::from_min_max(pos2(0.0, (1.0 - h) * 0.5), pos2(1.0, (1.0 + h) * 0.5))
        } else {
            let w = aspect / image_aspect;
            Rect::from_min_max(pos2((1.0 - w) * 0.65, 0.0), pos2((1.0 - w) * 0.65 + w, 1.0))
        };
        p.image(texture.id(), r, uv, Color32::from_gray(208));
    }
    // Lớp phủ chuyển sắc sâu dần về bên trái & cạnh dưới để các thẻ kính nổi bật.
    let gradients = [
        [232, 36, 48, 240],
        [40, 0, 245, 255],
    ];
    for alphas in gradients {
        let mut mesh = Mesh::default();
        for (pos, a) in [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()].into_iter().zip(alphas) {
            mesh.colored_vertex(pos, Color32::from_rgba_unmultiplied(INK.r(), INK.g(), INK.b(), a));
        }
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        p.add(mesh);
    }
    // Điểm sáng viền vàng-lam nhẹ góc trên trái tạo chiều sâu kính.
    let mut glow = Mesh::default();
    let colors = [
        Color32::from_rgba_unmultiplied(ACCENT.r(), ACCENT.g(), ACCENT.b(), 18),
        Color32::from_rgba_unmultiplied(CYAN.r(), CYAN.g(), CYAN.b(), 10),
        Color32::TRANSPARENT,
        Color32::TRANSPARENT,
    ];
    for (pos, col) in [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()].into_iter().zip(colors) {
        glow.colored_vertex(pos, col);
    }
    glow.add_triangle(0, 1, 2);
    glow.add_triangle(0, 2, 3);
    p.add(glow);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backdrop_is_not_clipped_to_the_safe_area() {
        let ctx = Context::default();
        let viewport = Rect::from_min_size(Pos2::ZERO, vec2(874.0, 402.0));
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(viewport),
                safe_area_insets: Some(SafeAreaInsets(egui::epaint::MarginF32 {
                    left: 62.0,
                    right: 62.0,
                    top: 0.0,
                    bottom: 21.0,
                })),
                ..Default::default()
            },
            |ui| backdrop(ui.ctx(), None),
        );
        output.textures_delta.clear();
        let background = output.shapes.iter().find(|s| matches!(&s.shape, Shape::Rect(r) if r.fill == INK)).unwrap();
        assert_eq!(background.clip_rect, viewport);
        output.textures_delta.clear();
    }
}
