use egui::{
    self, Align2, Button, Color32, CornerRadius, FontId, Frame, Margin, Order, Painter, Rect,
    Response, RichText, Sense, Stroke, StrokeKind, TextStyle, TextWrapMode, Ui, Vec2, WidgetText,
};

use crate::theme::Palette;

/// Ledger Pro spacing scale (logical pixels).
pub const SPACE_XS: f32 = 4.0;
pub const SPACE_SM: f32 = 8.0;
pub const SPACE_MD: f32 = 16.0;
pub const SPACE_LG: f32 = 24.0;

pub const CAL_WEEK_COL_WIDTH: f32 = 32.0;
pub const CAL_DAY_CELL: Vec2 = Vec2::new(44.0, 48.0);
/// Smallest day-cell height before the month grid must scroll (very short windows).
pub const CAL_DAY_CELL_MIN_HEIGHT: f32 = 28.0;
/// Minimum width of the calendar column in the main split (Ledger Pro density).
pub const CAL_COLUMN_MIN_WIDTH: f32 = 420.0;
/// Minimum width reserved for the day-detail / form column.
pub const DETAIL_COLUMN_MIN_WIDTH: f32 = 360.0;
/// Calendar column grows with the window but never beyond this fraction of inner width.
pub const MAIN_SPLIT_CAL_MAX_FRAC: f32 = 0.45;
/// Smallest client size that still honors [`CAL_COLUMN_MIN_WIDTH`] + [`DETAIL_COLUMN_MIN_WIDTH`] and one month grid at [`CAL_DAY_CELL_MIN_HEIGHT`].
pub const MIN_WINDOW_WIDTH: f32 =
    CAL_COLUMN_MIN_WIDTH + DETAIL_COLUMN_MIN_WIDTH + SPACE_MD + 1.0;
/// Toolbar + footer + six week rows at min cell height + calendar chrome (title, weekday row).
pub const MIN_WINDOW_HEIGHT: f32 = 480.0;
pub const CARD_RADIUS: u8 = 10;
pub const CALENDAR_OUTER_RADIUS: u8 = 12;

/// Width of the calendar column given total central-panel width and the gutter between panes.
pub fn calendar_column_width(total_width: f32, pane_gap: f32) -> f32 {
    let divider = 1.0;
    let cal_cap = total_width * MAIN_SPLIT_CAL_MAX_FRAC;
    let cal_max = (total_width - DETAIL_COLUMN_MIN_WIDTH - pane_gap - divider).max(CAL_COLUMN_MIN_WIDTH);
    cal_cap.clamp(CAL_COLUMN_MIN_WIDTH, cal_max)
}

/// Day cell size from column width and the vertical space reserved for the week grid.
///
/// Row height shrinks below [`CAL_DAY_CELL`] when needed so every week row fits in
/// `scroll_area_height` without scrolling; only hits [`CAL_DAY_CELL_MIN_HEIGHT`] on
/// very short viewports (then the caller's scroll area may scroll).
pub fn cal_day_cell_for_layout(column_width: f32, week_rows: usize, scroll_area_height: f32) -> Vec2 {
    let gap = SPACE_SM;
    let frame_margin = SPACE_SM * 2.0;
    let inner_w = (column_width - frame_margin).max(CAL_WEEK_COL_WIDTH + gap);
    let day_w = ((inner_w - CAL_WEEK_COL_WIDTH - 7.0 * gap) / 7.0).clamp(32.0, 56.0);
    let rows = week_rows.max(1);
    let row_gaps = gap * (rows.saturating_sub(1)) as f32;
    let available_for_rows = (scroll_area_height - frame_margin - row_gaps).max(0.0);
    let row_h = (available_for_rows / rows as f32)
        .clamp(CAL_DAY_CELL_MIN_HEIGHT, 56.0_f32.max(CAL_DAY_CELL.y));
    Vec2::new(day_w, row_h)
}

/// Total height of the bordered month grid for `week_rows` rows at `day_cell` (matches `calendar_outer_frame` margins).
pub fn cal_month_grid_content_height(week_rows: usize, day_cell: Vec2) -> f32 {
    let gap = SPACE_SM;
    let frame_margin = SPACE_SM * 2.0;
    let rows = week_rows.max(1);
    frame_margin + rows as f32 * day_cell.y + gap * (rows.saturating_sub(1)) as f32
}

/// Detail pane background (muted surface) distinct from the app background.
pub fn detail_column_frame(palette: Palette) -> Frame {
    Frame::new()
        .fill(palette.surface)
        .inner_margin(Margin::same(SPACE_MD as i8))
}

/// Surface card used for day-detail rows and similar list items.
pub fn card_frame(palette: Palette) -> Frame {
    Frame::new()
        .fill(palette.surface)
        .stroke(Stroke::new(1.0, palette.border))
        .corner_radius(CornerRadius::same(CARD_RADIUS))
        .inner_margin(Margin::same(SPACE_MD as i8))
}

/// Fills the ISO week column (header “Wk” and each week number) with the shaded week-column tokens.
pub fn paint_week_column_cell(
    painter: &Painter,
    rect: Rect,
    radius: CornerRadius,
    palette: Palette,
    current_week: bool,
    label: &str,
) {
    let (bg, fg) = if current_week {
        (palette.cal_wk_active_bg, palette.cal_wk_active_fg)
    } else {
        (palette.cal_wk_inactive_bg, palette.cal_wk_inactive_fg)
    };
    painter.rect_filled(rect, radius, bg);
    let edge = if current_week {
        palette.cal_wk_active_bg
    } else {
        palette.border
    };
    painter.rect_stroke(rect, radius, Stroke::new(1.0, edge), StrokeKind::Inside);
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(11.0),
        fg,
    );
}

/// Outer shell around the scrollable month grid.
pub fn calendar_outer_frame(palette: Palette) -> Frame {
    Frame::new()
        .stroke(Stroke::new(1.0, palette.border))
        .corner_radius(CornerRadius::same(CALENDAR_OUTER_RADIUS))
        .inner_margin(Margin::same(SPACE_SM as i8))
}

/// Vertical brand gradient for toolbar and splash (software “dark-band” pattern).
pub fn paint_vertical_gradient_band(
    painter: &Painter,
    rect: Rect,
    top: Color32,
    bottom: Color32,
) {
    let mut mesh = egui::Mesh::default();
    let vi = mesh.vertices.len() as u32;
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), bottom);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.colored_vertex(rect.left_bottom(), top);
    mesh.add_triangle(vi, vi + 1, vi + 2);
    mesh.add_triangle(vi, vi + 2, vi + 3);
    painter.add(egui::Shape::mesh(mesh));
}

pub fn toolbar_gradient(palette: Palette) -> (Color32, Color32) {
    (palette.toolbar_bg, palette.toolbar_accent)
}

/// Hairline on the primary toolbar (visible on saturated fills).
pub fn toolbar_divider(ui: &mut Ui, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, height.max(20.0)), Sense::hover());
    ui.painter().line_segment(
        [rect.left_top(), rect.left_bottom()],
        Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(255, 255, 255, 96),
        ),
    );
}

/// Top app bar: thick primary band with a darker accent stroke along the bottom edge.
pub fn toolbar_panel_frame(palette: Palette) -> Frame {
    Frame::new()
        .fill(palette.toolbar_bg)
        .inner_margin(Margin::symmetric(SPACE_LG as i8, SPACE_MD as i8))
        .stroke(Stroke::new(4.0, palette.toolbar_accent))
}

/// Padded body inside modal windows (settings, export, copy).
pub fn modal_content_frame() -> Frame {
    Frame::new().inner_margin(Margin::same(SPACE_LG as i8))
}

/// Semi-transparent scrim behind modal windows. Must stay on [`Order::Background`]; a
/// foreground scrim would paint above [`egui::Window`] (middle layer) and block all input.
pub fn paint_modal_backdrop(
    ctx: &egui::Context,
    backdrop_id: egui::Id,
) -> Response {
    let screen = ctx.input(|i| i.screen_rect);
    egui::Area::new(backdrop_id)
        .order(Order::Background)
        .fixed_pos(screen.min)
        .interactable(true)
        .show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter()
                .rect_filled(rect, 0, Color32::from_black_alpha(102));
            response
        })
        .inner
}

/// `egui::Button` always paints its label with `visuals.text_color()`, ignoring `RichText::color`.
/// Repaint the galley with an explicit foreground so icons stay readable on custom fills.
fn button_with_fg(
    ui: &mut Ui,
    label: RichText,
    fill: Color32,
    fg: Color32,
    stroke: Stroke,
    min_size: Vec2,
) -> Response {
    let response = ui.add(
        Button::new(label.clone())
            .fill(fill)
            .stroke(stroke)
            .min_size(min_size),
    );
    if ui.is_rect_visible(response.rect) {
        let pad = ui.spacing().button_padding;
        let galley = WidgetText::RichText(label).into_galley(
            ui,
            Some(TextWrapMode::Extend),
            (response.rect.width() - 2.0 * pad.x).max(0.0),
            TextStyle::Button,
        );
        let text_pos = ui
            .layout()
            .align_size_within_rect(galley.size(), response.rect.shrink2(pad))
            .min;
        ui.painter().galley(text_pos, galley, fg);
    }
    response
}

pub fn primary_button(ui: &mut Ui, palette: Palette, label: impl Into<WidgetText>) -> Response {
    let label = label.into();
    let rich = rich_from_widget_text(label);
    button_with_fg(
        ui,
        rich,
        palette.primary,
        Color32::WHITE,
        Stroke::new(1.0, palette.primary),
        Vec2::ZERO,
    )
}

pub fn secondary_button(ui: &mut Ui, palette: Palette, label: impl Into<WidgetText>) -> Response {
    let label = label.into();
    let rich = rich_from_widget_text(label);
    button_with_fg(
        ui,
        rich,
        palette.surface,
        palette.text,
        Stroke::new(1.0, palette.border),
        Vec2::ZERO,
    )
}

/// Best-effort `RichText` wrapper when callers pass plain strings or pre-colored labels.
fn rich_from_widget_text(text: WidgetText) -> RichText {
    match text {
        WidgetText::RichText(rich) => rich,
        WidgetText::LayoutJob(job) => RichText::new(&job.text),
        WidgetText::Galley(galley) => RichText::new(galley.text()),
    }
}

/// Compact control for month navigation on the colored toolbar.
pub fn toolbar_text_button(ui: &mut Ui, palette: Palette, label: RichText) -> Response {
    button_with_fg(
        ui,
        label,
        Color32::from_rgba_unmultiplied(255, 255, 255, 28),
        palette.toolbar_fg,
        Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(255, 255, 255, 90),
        ),
        Vec2::ZERO,
    )
}

/// Export / Settings on the toolbar (light chip on the primary bar).
pub fn toolbar_action_button(ui: &mut Ui, palette: Palette, label: RichText) -> Response {
    button_with_fg(
        ui,
        label,
        palette.surface,
        palette.primary,
        Stroke::new(1.0, palette.border),
        Vec2::ZERO,
    )
}

/// Text/icon control with explicit fill and foreground (day header pin, etc.).
pub fn pill_button(
    ui: &mut Ui,
    label: RichText,
    fill: Color32,
    fg: Color32,
    stroke: Stroke,
) -> Response {
    button_with_fg(ui, label, fill, fg, stroke, Vec2::ZERO)
}

/// Icon-only or icon-primary control on day cards (neutral surface, primary glyph).
pub fn card_icon_button(ui: &mut Ui, palette: Palette, icon: RichText, tooltip: &str) -> Response {
    button_with_fg(
        ui,
        icon,
        palette.surface,
        palette.primary,
        Stroke::new(1.0, palette.border),
        Vec2::splat(30.0),
    )
    .on_hover_text(tooltip)
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

/// Splash fullscreen gradient endpoints derived from the theme primary.
#[cfg(test)]
mod tests {
    use super::*;

    /// Six-row months must fit in a typical central-panel viewport without exceeding the budget used for sizing.
    #[test]
    fn month_grid_six_rows_fits_viewport_budget() {
        let scroll_h = 320.0;
        let cell = cal_day_cell_for_layout(420.0, 6, scroll_h);
        let total = cal_month_grid_content_height(6, cell);
        assert!(
            total <= scroll_h + 0.5,
            "grid height {total} should fit in scroll budget {scroll_h}"
        );
    }

    #[test]
    fn month_grid_prefers_shrink_over_min_preferred_height() {
        let cell = cal_day_cell_for_layout(420.0, 6, 260.0);
        assert!(
            cell.y < CAL_DAY_CELL.y,
            "expected shrink below preferred {} got {}",
            CAL_DAY_CELL.y,
            cell.y
        );
        assert!(cell.y >= CAL_DAY_CELL_MIN_HEIGHT);
    }
}

pub fn splash_gradient(palette: Palette) -> (Color32, Color32) {
    toolbar_gradient(palette)
}