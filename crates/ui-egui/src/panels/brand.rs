//! Original ALEJANDRO Creative Academy crown-and-A mark, generated in Rust.
// Copyright 2026 Alejandro Creative Academy. MIT OR Apache-2.0.
use egui::{Color32, Context, Id, Rect, TextureHandle, TextureOptions, Ui, pos2};
fn decode() -> egui::ColorImage {
    let mut image = egui::ColorImage::filled([128, 128], Color32::TRANSPARENT);
    for (i, pixel) in image.pixels.iter_mut().enumerate() {
        let x = (i % 128) as i32;
        let y = (i / 128) as i32;
        let dx = x - x.clamp(22, 105);
        let dy = y - y.clamp(22, 105);
        if dx * dx + dy * dy > 22 * 22 {
            continue;
        }
        *pixel = Color32::from_rgb(40, 23, 56);
        let crown_tip = 30 + (x - 64).abs() / 2;
        if (27..=100).contains(&x) && (crown_tip..=57).contains(&y) {
            *pixel = Color32::from_rgb(217, 182, 88);
        }
        let half = (y - 63) / 2;
        if (66..=106).contains(&y) && (x - 64).abs() <= half && ((x - 64).abs() >= half - 7 || (89..=96).contains(&y)) {
            *pixel = Color32::from_rgb(248, 236, 208);
        }
    }
    image
}
fn texture(ctx: &Context) -> TextureHandle {
    let id = Id::new("aca-brand-mark");
    if let Some(texture) = ctx.data(|data| data.get_temp::<TextureHandle>(id)) {
        return texture;
    }
    let options = TextureOptions { mipmap_mode: Some(egui::TextureFilter::Linear), ..TextureOptions::LINEAR };
    let texture = ctx.load_texture("aca-brand-mark", decode(), options);
    ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
    texture
}
pub fn paint_mark(ui: &Ui, rect: Rect) {
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    ui.painter().image(texture(ui.ctx()).id(), rect, uv, Color32::WHITE);
    ui.ctx().data_mut(|data| data.insert_temp(Id::new("aca-brand-mark-rect"), rect));
}
#[cfg(test)]
pub fn mark_rect(ctx: &Context) -> Option<Rect> {
    ctx.data(|data| data.get_temp(Id::new("aca-brand-mark-rect")))
}
#[cfg(test)]
mod academy_tests {
    use super::*;
    #[test]
    fn original_mark_is_bounded_transparent_at_corners_and_opaque_at_center() {
        let image = decode();
        assert_eq!(image.size, [128, 128]);
        assert_eq!(image.pixels.first().map(|p| p.a()), Some(0));
        assert_eq!(image.pixels.get(64 * 128 + 64).map(|p| p.a()), Some(255));
        assert!(image.pixels.iter().any(|p| *p == Color32::from_rgb(217, 182, 88)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_icon_decodes_to_a_rounded_128_px_tile() {
        let img = decode();
        assert_eq!(img.size, [128, 128]);
        // Rounded corners: the corner pixel is transparent, the centre opaque.
        assert_eq!(img.pixels[0].a(), 0);
        assert_eq!(img.pixels[64 * 128 + 64].a(), 255);
    }

    #[test]
    fn the_title_bar_paints_the_icon_from_one_cached_texture() {
        let ctx = Context::default();
        crate::PhotocraftApp::setup_context(&ctx, crate::theme::ThemeKind::ALL[0]);
        let mut app = crate::PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        // The system title bar hides the in-app mark (the OS draws its own icon), so paint the
        // app-drawn bar here.
        app.custom_titlebar = true;
        let mut frame = || {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| crate::panels::title_bar(&mut app, ui));
            let uploads = out.textures_delta.set.values().flat_map(|d| d.iter()).filter(|d| d.image.size() == [128, 128]).count();
            out.textures_delta.clear();
            uploads
        };
        assert_eq!(frame(), 1, "uploaded on first use");
        assert_eq!(frame(), 0, "then reused");
        assert!(mark_rect(&ctx).is_some());
    }
}
