use egui::{
    self, Button, Color32, CornerRadius, Frame, Margin, Order, Response, RichText, Sense, Stroke,
    Ui, Vec2, WidgetText,
};

use crate::theme::Palette;

/// Ledger Pro spacing scale (logical pixels).
pub const SPACE_XS: f32 = 4.0;
pub const SPACE_SM: f32 = 8.0;
pub const SPACE_MD: f32 = 16.0;
pub const SPACE_LG: f32 = 24.0;

pub const CAL_WEEK_COL_WIDTH: f32 = 32.0;
pub const CAL_DAY_CELL: Vec2 = Vec2::new(44.0, 48.0);
pub const CARD_RADIUS: u8 = 10;
pub const CALENDAR_OUTER_RADIUS: u8 = 12;

/// Surface card used for day-detail rows and similar list items.
pub fn card_frame(palette: Palette) -> Frame {
    Frame::new()
        .fill(palette.surface)
        .stroke(Stroke::new(1.0, palette.border))
        .corner_radius(CornerRadius::same(CARD_RADIUS))
        .inner_margin(Margin::same(SPACE_MD as i8))
}

/// Outer shell around the scrollable month grid.
pub fn calendar_outer_frame(palette: Palette) -> Frame {
    Frame::new()
        .stroke(Stroke::new(1.0, palette.border))
        .corner_radius(CornerRadius::same(CALENDAR_OUTER_RADIUS))
        .inner_margin(Margin::same(SPACE_SM as i8))
}

/// Top app bar: neutral surface with a single hairline along the bottom edge.
pub fn toolbar_panel_frame(palette: Palette) -> Frame {
    Frame::new()
        .fill(palette.surface)
        .inner_margin(Margin::symmetric(SPACE_MD as i8, SPACE_SM as i8))
        .stroke(Stroke::new(1.0, palette.border))
}

/// Padded body inside modal windows (settings, export, copy).
pub fn modal_content_frame() -> Frame {
    Frame::new().inner_margin(Margin::same(SPACE_LG as i8))
}

/// Semi-transparent scrim behind modal windows so focus stays on the dialog.
pub fn paint_modal_backdrop(ctx: &egui::Context, palette: Palette) {
    let screen = ctx.input(|i| i.screen_rect);
    egui::Area::new(egui::Id::new("modal_backdrop"))
        .order(Order::Foreground)
        .fixed_pos(screen.min)
        .interactable(true)
        .show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(screen.size(), Sense::click());
            let mut dim = palette.text;
            dim = Color32::from_rgba_unmultiplied(dim.r(), dim.g(), dim.b(), 96);
            ui.painter().rect_filled(rect, 0, dim);
            let _ = response;
        });
}

pub fn primary_button(ui: &mut Ui, palette: Palette, label: impl Into<WidgetText>) -> Response {
    ui.add(
        Button::new(label)
            .fill(palette.primary)
            .stroke(Stroke::new(1.0, palette.primary)),
    )
}

pub fn secondary_button(ui: &mut Ui, palette: Palette, label: impl Into<WidgetText>) -> Response {
    ui.add(
        Button::new(label)
            .fill(palette.surface)
            .stroke(Stroke::new(1.0, palette.border)),
    )
}

/// Compact text-style control for toolbar navigation (month prev/next, etc.).
pub fn toolbar_text_button(ui: &mut Ui, palette: Palette, label: &str) -> Response {
    secondary_button(
        ui,
        palette,
        RichText::new(label).color(palette.text),
    )
}

/// Painted gold marker for achievements (no Unicode symbols).
pub fn achievement_indicator(ui: &mut Ui, palette: Palette) -> Response {
    let size = Vec2::new(22.0, 18.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    painter.circle_filled(
        rect.left_center() + Vec2::new(7.0, 0.0),
        5.0,
        palette.achievement,
    );
    painter.text(
        rect.left_center() + Vec2::new(14.0, 0.0),
        egui::Align2::LEFT_CENTER,
        "Ach",
        egui::FontId::proportional(10.0),
        palette.achievement,
    );
    response
}

/// Splash fullscreen gradient endpoints derived from the theme primary (toolbar stays neutral).
pub fn splash_gradient(palette: Palette) -> (Color32, Color32) {
    let top = palette.primary;
    let bottom = Color32::from_rgb(
        (top.r() as f32 * 0.82) as u8,
        (top.g() as f32 * 0.82) as u8,
        (top.b() as f32 * 0.82) as u8,
    );
    (top, bottom)
}