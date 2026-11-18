//! "Quiet Precision" UI chrome: layout geometry, tactile flat-modern buttons, segmented
//! controls, semantic category/achievement pills, focus-ring inputs, and frameless modal shells.
//!
//! Eliminates multi-hue mesh gradients in favor of solid surface elevation, `1px` neutral
//! hairlines, micro-bevel inner highlights on primary CTAs, and deterministic galley-first
//! widget sizing (`button_desired_size`).

use std::f32::consts::FRAC_PI_2;

use egui::{
    self, Align, Align2, Color32, CornerRadius, CursorIcon, FontFamily, FontId, Frame, Layout,
    Margin, Order, Painter, Pos2, Rect, Response, RichText, Sense, Stroke, StrokeKind, TextStyle,
    TextWrapMode, Ui, Vec2, WidgetText,
};
use epaint::Shadow;

use crate::theme::{category_style, Palette, ThemeId};
use crate::ui::icons::{
    font_bold, icon_label_bold, rich_bold, CARET_DOWN, CHECK, PLUS, STAR, X,
};

/// Spacing scale in logical pixels (4px base grid).
pub const SPACE_XS: f32 = 4.0;
pub const SPACE_SM: f32 = 8.0;
pub const SPACE_MD: f32 = 16.0;
pub const SPACE_LG: f32 = 24.0;

pub const CAL_WEEK_COL_WIDTH: f32 = 32.0;
pub const CAL_DAY_CELL: Vec2 = Vec2::new(44.0, 48.0);
/// Smallest day-cell height before the month grid scrolls on ultra-short viewports.
pub const CAL_DAY_CELL_MIN_HEIGHT: f32 = 28.0;
/// Minimum width of the left calendar pane in the main split.
pub const CAL_COLUMN_MIN_WIDTH: f32 = 420.0;
/// Minimum width reserved for the right day-detail / form pane.
///
/// The day-detail header is a single non-wrapping row. Its widest case is
/// `Wednesday, May 10` (longest English weekday, widest `%b` abbreviation in
/// Noto Sans Bold; digits are tabular so every two-digit day is the same width)
/// plus the `Today` and `Pinned` pills, the 34px pin button, and `+ Add Item`.
/// Laid out in Noto Sans Bold that row is 455.7px. [`detail_column_frame`] insets
/// 20px on each side, so the pane is 496px (`ceil(455.7 + 40)`). At 495px the row
/// is 0.7px wider than the content box and the pin button overlaps the `Pinned` pill.
/// The edit-form footer (`Copy to another day…` + Cancel + Update) is narrower than this row.
pub const DETAIL_COLUMN_MIN_WIDTH: f32 = 496.0;
/// Calendar pane grows with the viewport up to 45% of total width so the detail list gets generous room.
pub const MAIN_SPLIT_CAL_MAX_FRAC: f32 = 0.45;
/// Smallest client width that keeps the calendar at [`CAL_COLUMN_MIN_WIDTH`] and the
/// day-detail header inside [`DETAIL_COLUMN_MIN_WIDTH`], plus the 16px pane gap and the
/// 1px divider between them.
///
/// The widest toolbar (`September 2026` + `Theme · Forest` + Import/Export/Close, about
/// 773px) is inside this width. Extra window width grows the detail pane; the calendar
/// stays at its 420px floor because [`calendar_column_width`] caps it at 45% of the row.
pub const MIN_WINDOW_WIDTH: f32 =
    CAL_COLUMN_MIN_WIDTH + DETAIL_COLUMN_MIN_WIDTH + SPACE_MD + 1.0;
/// Shortest client height that keeps the full 86px hero and six day rows at the preferred
/// [`CAL_DAY_CELL`] height (`48px`). One pixel shorter and the live layout drops the rows
/// below that preferred height. This is both the launch size and the resize floor.
pub const MIN_WINDOW_HEIGHT: f32 = 650.0;
/// Launch width. Equal to [`MIN_WINDOW_WIDTH`]: wider windows only add empty space between
/// the calendar and the day detail.
pub const DEFAULT_WINDOW_WIDTH: f32 = MIN_WINDOW_WIDTH;
/// Launch height. Equal to [`MIN_WINDOW_HEIGHT`].
pub const DEFAULT_WINDOW_HEIGHT: f32 = MIN_WINDOW_HEIGHT;

/// Standard height of the Brand Hero Card on desktop viewports (`>= 620px` window height).
pub const HERO_HEIGHT: f32 = 86.0;
/// Auto-compacted Brand Hero Card height on short viewports (`< 620px` window height) so all 6
/// calendar week rows continue to fit without vertical scrolling.
pub const HERO_COMPACT_HEIGHT: f32 = 64.0;
pub const HERO_RADIUS: u8 = 14;

pub const CARD_RADIUS: u8 = 10;
pub const CALENDAR_OUTER_RADIUS: u8 = 12;
pub const MODAL_RADIUS: u8 = 14;

/// Shared corner radius for standard buttons and inputs (`7px` architectural crispness).
const BTN_RADIUS_PX: u8 = 7;
/// Full stadium pill radius for compact date-preset chips.
const PRESET_RADIUS_PX: u8 = 99;
/// Standard button padding and `34px` minimum height across primary and secondary actions.
const BTN_PADDING: Vec2 = Vec2::new(14.0, 7.0);
const BTN_MIN_SIZE: Vec2 = Vec2::new(0.0, 34.0);
/// Opacity multiplier applied to disabled buttons so disabled state is unmistakable without vanishing.
const DISABLED_ALPHA: f32 = 0.45;

/// Computes the width of the left calendar column from `total_width` and `pane_gap`.
///
/// Protects the dual invariant that the calendar column never shrinks below
/// [`CAL_COLUMN_MIN_WIDTH`] (`420px`, enough for `Wk` + 7 day columns) while never starving
/// the right detail pane below [`DETAIL_COLUMN_MIN_WIDTH`].
pub fn calendar_column_width(total_width: f32, pane_gap: f32) -> f32 {
    let divider = 1.0;
    let cal_cap = total_width * MAIN_SPLIT_CAL_MAX_FRAC;
    let cal_max =
        (total_width - DETAIL_COLUMN_MIN_WIDTH - pane_gap - divider).max(CAL_COLUMN_MIN_WIDTH);
    cal_cap.clamp(CAL_COLUMN_MIN_WIDTH, cal_max)
}

/// Computes `(day_width, day_height)` for a calendar cell from `column_width`, `week_rows`
/// (4–6 ISO weeks), and `scroll_area_height` reserved for the month grid.
///
/// Responsibility & invariants:
/// - Shrinks cell height smoothly below `CAL_DAY_CELL.y` (`48px`) down to
///   [`CAL_DAY_CELL_MIN_HEIGHT`] (`28px`) so all 4–6 week rows remain simultaneously visible
///   without vertical scrolling down to [`MIN_WINDOW_HEIGHT`].
/// - Accounts for `frame_margin = SPACE_SM * 2.0` (`16px` total vertical inset) matching
///   [`calendar_outer_frame`] and [`cal_month_grid_content_height`] so layout budget and painted
///   content height agree to sub-pixel precision.
pub fn cal_day_cell_for_layout(
    column_width: f32,
    week_rows: usize,
    scroll_area_height: f32,
) -> Vec2 {
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

/// Exact total height of the padded month grid for `week_rows` rows at `day_cell`.
///
/// Must stay locked in step with [`cal_day_cell_for_layout`] and [`calendar_outer_frame`]
/// (`SPACE_SM` top + `SPACE_SM` bottom margin + `SPACE_SM` inter-row spacing).
pub fn cal_month_grid_content_height(week_rows: usize, day_cell: Vec2) -> f32 {
    let gap = SPACE_SM;
    let frame_margin = SPACE_SM * 2.0;
    let rows = week_rows.max(1);
    frame_margin + rows as f32 * day_cell.y + gap * (rows.saturating_sub(1)) as f32
}

/// Elevated neutral top header bar shell (`palette.surface` fill, zero outer margin).
pub fn toolbar_panel_frame(palette: Palette) -> Frame {
    Frame::new()
        .fill(palette.surface)
        .inner_margin(Margin::ZERO)
        .stroke(Stroke::NONE)
}

/// Paints a `1px` vertical hairline divider of `height` logical pixels in `color`.
pub fn vertical_divider(ui: &mut Ui, height: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, height.max(12.0)), Sense::hover());
    ui.painter()
        .line_segment([rect.left_top(), rect.left_bottom()], Stroke::new(1.0, color));
}

/// Paints a full-width `1px` horizontal hairline divider across `ui.available_width()`.
pub fn horizontal_divider(ui: &mut Ui, color: Color32) {
    let width = ui.available_width().max(1.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), Sense::hover());
    ui.painter()
        .line_segment([rect.left_top(), rect.right_top()], Stroke::new(1.0, color));
}

/// Elevated card shell wrapping the left calendar header, weekday strip, and week rows.
pub fn calendar_card_frame(palette: Palette) -> Frame {
    Frame::new()
        .fill(palette.surface)
        .stroke(Stroke::new(1.0, palette.border))
        .corner_radius(CornerRadius::same(CALENDAR_OUTER_RADIUS))
        .inner_margin(Margin::symmetric(12, 10))
        .shadow(if palette.is_dark {
            Shadow::NONE
        } else {
            Shadow {
                offset: [0, 1],
                blur: 3,
                spread: 0,
                color: palette.shadow_sm,
            }
        })
}

/// Inner `SPACE_SM`-padded frame around the week-row grid whose `16px` vertical margin matches
/// [`cal_day_cell_for_layout`] and [`cal_month_grid_content_height`].
pub fn calendar_outer_frame(_palette: Palette) -> Frame {
    Frame::new()
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::NONE)
        .inner_margin(Margin::same(SPACE_SM as i8))
}

/// Right-hand detail/form pane shell (`palette.surface` fill with generous `20px` horizontal margin).
pub fn detail_column_frame(palette: Palette) -> Frame {
    Frame::new()
        .fill(palette.surface)
        .inner_margin(Margin::symmetric(20, SPACE_MD as i8))
}

/// Surface card shell for individual work items in `DayDetailPanel`.
pub fn card_frame(palette: Palette) -> Frame {
    Frame::new()
        .fill(palette.surface)
        .stroke(Stroke::new(1.0, palette.border))
        .corner_radius(CornerRadius::same(CARD_RADIUS))
        .inner_margin(Margin::symmetric(14, 12))
}

/// Paints a cell in the left ISO week-number strip (`Wk` header and week numbers `1..=53`).
///
/// Design & invariants:
/// - Inactive weeks receive a quiet `cal_wk_inactive_bg` (`surface_muted`) fill without heavy
///   competing strokes so the shaded week column reads as a cohesive vertical reference gutter.
/// - The current calendar week receives `cal_wk_active_bg` (`accent_soft`), a `1px`
///   `accent_soft_border` inside hairline, and a bold `cal_wk_active_fg` numeral so the user can
///   orient to the active ISO week at a glance without a harsh saturated block.
pub fn paint_week_column_cell(
    painter: &Painter,
    rect: Rect,
    radius: CornerRadius,
    palette: Palette,
    current_week: bool,
    label: &str,
) {
    let (bg, fg, font) = if current_week {
        (
            palette.cal_wk_active_bg,
            palette.cal_wk_active_fg,
            font_bold(11.0),
        )
    } else {
        (
            palette.cal_wk_inactive_bg,
            palette.cal_wk_inactive_fg,
            FontId::new(11.0, FontFamily::Proportional),
        )
    };
    painter.rect_filled(rect, radius, bg);
    if current_week {
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, palette.accent_soft_border),
            StrokeKind::Inside,
        );
    }
    painter.text(rect.center(), Align2::CENTER_CENTER, label, font, fg);
}

/// Paints a dashed rounded-rectangle border strictly inside `rect` using `egui::Shape::dashed_line`.
///
/// Insets the perimeter by `0.5 * stroke.width` so `1px` dashes align cleanly with `rect_filled`
/// corners on the `"+ Add another item"` bar at the bottom of a day's work-item list.
pub fn paint_dashed_rect(
    painter: &Painter,
    rect: Rect,
    radius: u8,
    stroke: Stroke,
    dash_length: f32,
    gap_length: f32,
) {
    let inset = rect.shrink(stroke.width * 0.5);
    if !inset.is_positive() || stroke.is_empty() {
        return;
    }
    let r = (radius as f32 - stroke.width * 0.5)
        .max(0.0)
        .min(0.5 * inset.width().min(inset.height()));

    const ARC_STEPS: usize = 5;
    let corners = [
        (Pos2::new(inset.right() - r, inset.top() + r), -FRAC_PI_2),
        (Pos2::new(inset.right() - r, inset.bottom() - r), 0.0),
        (Pos2::new(inset.left() + r, inset.bottom() - r), FRAC_PI_2),
        (Pos2::new(inset.left() + r, inset.top() + r), std::f32::consts::PI),
    ];

    let mut path = Vec::with_capacity(corners.len() * (ARC_STEPS + 1) + 1);
    for &(corner_center, start_angle) in &corners {
        for step in 0..=ARC_STEPS {
            let theta = start_angle + FRAC_PI_2 * (step as f32 / ARC_STEPS as f32);
            path.push(corner_center + Vec2::new(theta.cos(), theta.sin()) * r);
        }
    }
    if let Some(&first) = path.first() {
        path.push(first);
    }

    painter.extend(egui::Shape::dashed_line(&path, stroke, dash_length, gap_length));
}

/// Semi-transparent backdrop scrim painted on [`Order::Background`] behind modal windows.
///
/// Keeping the scrim on `Order::Background` ensures `egui::Window` (which lives on `Middle`)
/// receives pointer/keyboard events while clicks outside the modal hit this scrim and dismiss cleanly.
pub fn paint_modal_backdrop(
    ctx: &egui::Context,
    palette: Palette,
    backdrop_id: egui::Id,
) -> Response {
    let screen = ctx.input(|i| i.screen_rect);
    let scrim = if palette.is_dark {
        Color32::from_black_alpha(150)
    } else {
        Color32::from_rgba_unmultiplied(15, 23, 42, 110)
    };
    egui::Area::new(backdrop_id)
        .order(Order::Background)
        .fixed_pos(screen.min)
        .interactable(true)
        .show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(rect, 0, scrim);
            response
        })
        .inner
}

/// Frameless designer modal window shell (`14px` corner radius, `1px` hairline border, zero
/// outer margin so header/footer dividers span edge-to-edge, and soft elevated drop shadow).
pub fn modal_window_frame(palette: Palette) -> Frame {
    Frame::new()
        .fill(palette.surface)
        .stroke(Stroke::new(1.0, palette.border))
        .corner_radius(CornerRadius::same(MODAL_RADIUS))
        .inner_margin(Margin::ZERO)
        .shadow(Shadow {
            offset: [0, 12],
            blur: 32,
            spread: 0,
            color: if palette.is_dark {
                Color32::from_black_alpha(160)
            } else {
                Color32::from_rgba_unmultiplied(15, 23, 42, 38)
            },
        })
}

/// Renders a frameless modal header (bold title, optional muted subtitle, top-right ghost `X`
/// close button, and full-bleed `1px` bottom divider). Returns `true` if the user clicked `X`.
pub fn modal_header(
    ui: &mut Ui,
    palette: Palette,
    title: &str,
    subtitle: Option<&str>,
) -> bool {
    let mut close_clicked = false;
    Frame::new()
        .inner_margin(Margin::symmetric(SPACE_LG as i8, 16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.set_width(ui.available_width());
                ui.vertical(|ui| {
                    ui.label(rich_bold(title, 16.5).color(palette.text));
                    if let Some(sub) = subtitle {
                        ui.add_space(2.0);
                        ui.label(RichText::new(sub).size(12.5).color(palette.text_muted));
                    }
                });
                ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                    if ghost_icon_button(ui, palette, X, 15.0, "Close (Esc)").clicked() {
                        close_clicked = true;
                    }
                });
            });
        });
    horizontal_divider(ui, palette.divider);
    close_clicked
}

/// Computes the exact layout allocation size for a custom button from its laid-out text `galley_size`,
/// symmetric `padding`, and caller `min_size` floor.
///
/// Protects the invariant `result.x >= galley_size.x + 2.0 * padding.x` and
/// `result.y >= galley_size.y + 2.0 * padding.y` while honoring square icon-button or full-width
/// bar floors in `min_size`. Callers must measure the real label galley before allocating space.
pub fn button_desired_size(galley_size: Vec2, padding: Vec2, min_size: Vec2) -> Vec2 {
    (galley_size + 2.0 * padding).max(min_size)
}

struct CustomButtonStyle {
    idle_bg: Color32,
    hover_bg: Color32,
    active_bg: Color32,
    fg_idle: Color32,
    fg_hover: Color32,
    stroke_idle: Stroke,
    stroke_hover: Stroke,
    shadow: Option<Shadow>,
    top_bevel: bool,
    padding: Vec2,
    min_size: Vec2,
    radius: u8,
}

/// Deterministic galley-first custom button renderer shared across all four button tiers.
///
/// Flow & invariants:
/// 1. Converts `label` into an unwrapped button `Galley` *before* `allocate_exact_size` so
///    `response.rect` always encloses the full icon+caption width plus `2.0 * style.padding`.
/// 2. Respects both `enabled` and `ui.is_enabled()`: disabled buttons switch to `Sense::hover()`,
///    omit shadows/bevels and `PointingHand` cursor, and scale fill/stroke/text alpha by
///    [`DISABLED_ALPHA`].
/// 3. When `style.top_bevel` is set (primary CTAs) and the button is enabled and unpressed, paints
///    a subtle `1px` `rgba(255, 255, 255, 36)` top inner highlight inset inside the rounded corners,
///    giving solid flat buttons tactile depth without horizontal hue gradients.
/// 4. Draws text via `Shape::galley_with_override_text_color` so pre-colored Phosphor `RichText`
///    helpers still transition smoothly between idle, hover, and disabled foreground colors.
fn custom_button(
    ui: &mut Ui,
    label: RichText,
    style: CustomButtonStyle,
    enabled: bool,
) -> Response {
    let galley = WidgetText::RichText(label).into_galley(
        ui,
        Some(TextWrapMode::Extend),
        f32::INFINITY,
        TextStyle::Button,
    );
    let desired_size = button_desired_size(galley.size(), style.padding, style.min_size);
    let effective_enabled = enabled && ui.is_enabled();
    let sense = if effective_enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(desired_size, sense);

    let interacted_hover = effective_enabled && response.hovered();
    let interacted_active = effective_enabled && response.is_pointer_button_down_on();
    if interacted_hover {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    if ui.is_rect_visible(rect) {
        let corner = CornerRadius::same(style.radius);
        let alpha = if effective_enabled {
            1.0
        } else {
            DISABLED_ALPHA
        };

        if effective_enabled && !interacted_active {
            if let Some(shadow) = style.shadow {
                ui.painter().add(shadow.as_shape(rect, corner));
            }
        }

        let raw_bg = if interacted_active {
            style.active_bg
        } else if interacted_hover {
            style.hover_bg
        } else {
            style.idle_bg
        };
        let bg = raw_bg.gamma_multiply(alpha);
        if bg != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, corner, bg);
        }

        if style.top_bevel && effective_enabled && !interacted_active {
            let inset_x = (style.radius as f32).max(2.0);
            if rect.width() > inset_x * 2.0 {
                ui.painter().line_segment(
                    [
                        Pos2::new(rect.left() + inset_x, rect.top() + 1.0),
                        Pos2::new(rect.right() - inset_x, rect.top() + 1.0),
                    ],
                    Stroke::new(1.0, Color32::from_white_alpha(38)),
                );
            }
        }

        let raw_stroke = if interacted_hover || interacted_active {
            style.stroke_hover
        } else {
            style.stroke_idle
        };
        if !raw_stroke.is_empty() {
            let stroke = Stroke::new(raw_stroke.width, raw_stroke.color.gamma_multiply(alpha));
            ui.painter()
                .rect_stroke(rect, corner, stroke, StrokeKind::Inside);
        }

        let raw_fg = if interacted_hover || interacted_active {
            style.fg_hover
        } else {
            style.fg_idle
        };
        let fg = raw_fg.gamma_multiply(alpha);
        let text_pos = Align2::CENTER_CENTER
            .anchor_size(rect.center(), galley.size())
            .min;
        ui.painter()
            .add(egui::Shape::galley_with_override_text_color(text_pos, galley, fg));
    }

    response
}

fn bold_from_widget_text(text: WidgetText, size: f32) -> RichText {
    match text {
        WidgetText::RichText(rich) => rich.font(font_bold(size)),
        WidgetText::LayoutJob(job) => rich_bold(&job.text, size),
        WidgetText::Galley(galley) => rich_bold(galley.text(), size),
    }
}

fn rich_from_widget_text(text: WidgetText) -> RichText {
    match text {
        WidgetText::RichText(rich) => rich,
        WidgetText::LayoutJob(job) => RichText::new(&job.text),
        WidgetText::Galley(galley) => RichText::new(galley.text()),
    }
}

/// Primary solid CTA button (`+ Add Item`, `Update`, `Download .md`, `Copy item`).
pub fn primary_button(ui: &mut Ui, palette: Palette, label: impl Into<WidgetText>) -> Response {
    primary_button_enabled(ui, palette, label, true)
}

/// Primary solid CTA button with explicit `enabled` control, `1px` drop shadow, and `1px` top
/// inner highlight micro-bevel.
pub fn primary_button_enabled(
    ui: &mut Ui,
    palette: Palette,
    label: impl Into<WidgetText>,
    enabled: bool,
) -> Response {
    let rich = bold_from_widget_text(label.into(), 13.0);
    custom_button(
        ui,
        rich,
        CustomButtonStyle {
            idle_bg: palette.primary,
            hover_bg: palette.primary_hover,
            active_bg: palette.primary_active,
            fg_idle: palette.on_primary,
            fg_hover: palette.on_primary,
            stroke_idle: Stroke::NONE,
            stroke_hover: Stroke::NONE,
            shadow: Some(Shadow {
                offset: [0, 1],
                blur: 2,
                spread: 0,
                color: palette.shadow_sm,
            }),
            top_bevel: true,
            padding: BTN_PADDING,
            min_size: BTN_MIN_SIZE,
            radius: BTN_RADIUS_PX,
        },
        enabled,
    )
}

/// Compact `28px`-tall primary CTA button for notification banners (`"Open in browser"`).
pub fn primary_button_compact(
    ui: &mut Ui,
    palette: Palette,
    label: impl Into<WidgetText>,
) -> Response {
    let rich = bold_from_widget_text(label.into(), 12.0);
    custom_button(
        ui,
        rich,
        CustomButtonStyle {
            idle_bg: palette.primary,
            hover_bg: palette.primary_hover,
            active_bg: palette.primary_active,
            fg_idle: palette.on_primary,
            fg_hover: palette.on_primary,
            stroke_idle: Stroke::NONE,
            stroke_hover: Stroke::NONE,
            shadow: None,
            top_bevel: true,
            padding: Vec2::new(11.0, 4.0),
            min_size: Vec2::new(0.0, 28.0),
            radius: 6,
        },
        true,
    )
}

/// Tactile secondary outline button (`Export`, `Cancel`, `Close`, GitHub links) with elevated
/// `palette.surface` fill, `1px` hairline border, and subtle `1px` light-mode shadow.
pub fn secondary_button(ui: &mut Ui, palette: Palette, label: impl Into<WidgetText>) -> Response {
    let rich = rich_from_widget_text(label.into()).size(13.0);
    custom_button(
        ui,
        rich,
        CustomButtonStyle {
            idle_bg: palette.surface,
            hover_bg: palette.surface_muted,
            active_bg: palette.surface_hover,
            fg_idle: palette.text,
            fg_hover: palette.text,
            stroke_idle: Stroke::new(1.0, palette.border),
            stroke_hover: Stroke::new(1.0, palette.border_strong),
            shadow: if palette.is_dark {
                None
            } else {
                Some(Shadow {
                    offset: [0, 1],
                    blur: 2,
                    spread: 0,
                    color: palette.shadow_sm,
                })
            },
            top_bevel: false,
            padding: BTN_PADDING,
            min_size: BTN_MIN_SIZE,
            radius: BTN_RADIUS_PX,
        },
        true,
    )
}

/// Compact `28px`-tall borderless ghost button (`"Dismiss"` on the release banner).
pub fn ghost_button_compact(ui: &mut Ui, palette: Palette, label: &str) -> Response {
    custom_button(
        ui,
        RichText::new(label).size(12.0),
        CustomButtonStyle {
            idle_bg: Color32::TRANSPARENT,
            hover_bg: palette.surface_hover,
            active_bg: palette.surface_hover,
            fg_idle: palette.text_muted,
            fg_hover: palette.text,
            stroke_idle: Stroke::NONE,
            stroke_hover: Stroke::NONE,
            shadow: None,
            top_bevel: false,
            padding: Vec2::new(10.0, 4.0),
            min_size: Vec2::new(0.0, 28.0),
            radius: 6,
        },
        true,
    )
}

/// Square `32x32` borderless ghost icon button used for top-bar `Settings` and modal/form `Close`.
pub fn ghost_icon_button(
    ui: &mut Ui,
    palette: Palette,
    icon: &str,
    icon_size: f32,
    tooltip: &str,
) -> Response {
    custom_button(
        ui,
        RichText::new(icon).size(icon_size),
        CustomButtonStyle {
            idle_bg: Color32::TRANSPARENT,
            hover_bg: palette.surface_muted,
            active_bg: palette.surface_hover,
            fg_idle: palette.text_muted,
            fg_hover: palette.text,
            stroke_idle: Stroke::NONE,
            stroke_hover: Stroke::NONE,
            shadow: None,
            top_bevel: false,
            padding: Vec2::ZERO,
            min_size: Vec2::splat(32.0),
            radius: BTN_RADIUS_PX,
        },
        true,
    )
    .on_hover_text(tooltip)
}

/// Square `34x34` toggle icon button used for the Day Detail `Pin` control.
///
/// Idle unpinned state matches a quiet secondary outline icon button; active pinned state
/// switches to `palette.accent_soft` fill + `accent_soft_border` + `palette.primary` glyph so
/// pinned mode is immediately legible.
pub fn toggle_icon_button(
    ui: &mut Ui,
    palette: Palette,
    icon: &str,
    active: bool,
    tooltip: &str,
) -> Response {
    let style = if active {
        CustomButtonStyle {
            idle_bg: palette.accent_soft,
            hover_bg: palette.accent_soft_hover,
            active_bg: palette.accent_soft_hover,
            fg_idle: palette.accent_soft_fg,
            fg_hover: palette.primary,
            stroke_idle: Stroke::new(1.0, palette.accent_soft_border),
            stroke_hover: Stroke::new(1.0, palette.primary),
            shadow: None,
            top_bevel: false,
            padding: Vec2::ZERO,
            min_size: Vec2::splat(34.0),
            radius: BTN_RADIUS_PX,
        }
    } else {
        CustomButtonStyle {
            idle_bg: palette.surface,
            hover_bg: palette.surface_muted,
            active_bg: palette.surface_hover,
            fg_idle: palette.text_muted,
            fg_hover: palette.text,
            stroke_idle: Stroke::new(1.0, palette.border),
            stroke_hover: Stroke::new(1.0, palette.border_strong),
            shadow: None,
            top_bevel: false,
            padding: Vec2::ZERO,
            min_size: Vec2::splat(34.0),
            radius: BTN_RADIUS_PX,
        }
    };
    custom_button(ui, RichText::new(icon).size(15.5), style, true).on_hover_text(tooltip)
}

/// Compact `28x28` icon segment (`<` / `>`) inside the top-bar `< | Today | >` month scrubber.
pub fn scrubber_icon_button(
    ui: &mut Ui,
    palette: Palette,
    icon: &str,
    tooltip: &str,
) -> Response {
    custom_button(
        ui,
        RichText::new(icon).size(14.5),
        CustomButtonStyle {
            idle_bg: Color32::TRANSPARENT,
            hover_bg: palette.surface,
            active_bg: palette.surface_hover,
            fg_idle: palette.text_muted,
            fg_hover: palette.text,
            stroke_idle: Stroke::NONE,
            stroke_hover: Stroke::new(1.0, palette.border),
            shadow: None,
            top_bevel: false,
            padding: Vec2::ZERO,
            min_size: Vec2::splat(28.0),
            radius: 6,
        },
        true,
    )
    .on_hover_text(tooltip)
}

/// Center `"Today"` segment inside the top-bar `< | Today | >` month scrubber.
///
/// Highlights subtly when viewing a non-current month (`!at_current_month`) so jumping back to
/// Today stands out as an actionable anchor, while staying calm when already on the current month.
pub fn scrubber_today_button(
    ui: &mut Ui,
    palette: Palette,
    at_current_month: bool,
) -> Response {
    let label = if at_current_month {
        RichText::new("Today").size(12.5)
    } else {
        rich_bold("Today", 12.5)
    };
    custom_button(
        ui,
        label,
        CustomButtonStyle {
            idle_bg: if at_current_month {
                Color32::TRANSPARENT
            } else {
                palette.surface
            },
            hover_bg: palette.surface,
            active_bg: palette.surface_hover,
            fg_idle: if at_current_month {
                palette.text
            } else {
                palette.accent_soft_fg
            },
            fg_hover: palette.primary,
            stroke_idle: if at_current_month {
                Stroke::NONE
            } else {
                Stroke::new(1.0, palette.border)
            },
            stroke_hover: Stroke::new(1.0, palette.border),
            shadow: None,
            top_bevel: false,
            padding: Vec2::new(10.0, 4.0),
            min_size: Vec2::new(58.0, 28.0),
            radius: 6,
        },
        true,
    )
    .on_hover_text("Jump to current month")
}

/// Top-bar Theme dropdown trigger displaying a dual-tone swatch dot (`hero_bg_end` ring +
/// `primary` core) alongside the active theme label (`"Theme · <Label>"`) and down-caret.
pub fn theme_swatch_button(ui: &mut Ui, palette: Palette, theme: ThemeId) -> Response {
    let caption = format!("Theme · {}  {CARET_DOWN}", theme.label());
    let galley = WidgetText::RichText(RichText::new(caption).size(12.5)).into_galley(
        ui,
        Some(TextWrapMode::Extend),
        f32::INFINITY,
        TextStyle::Button,
    );
    let dot_slot = 17.0;
    let pad = Vec2::new(11.0, 5.0);
    let desired = Vec2::new(
        galley.size().x + dot_slot + 2.0 * pad.x,
        (galley.size().y + 2.0 * pad.y).max(30.0),
    );
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click());
    let hovered = response.hovered();
    let active = response.is_pointer_button_down_on();
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    if ui.is_rect_visible(rect) {
        let corner = CornerRadius::same(BTN_RADIUS_PX);
        let bg = if active {
            palette.surface_hover
        } else if hovered {
            palette.surface_muted
        } else {
            palette.surface
        };
        let border = if hovered || active {
            palette.border_strong
        } else {
            palette.border
        };
        ui.painter().rect_filled(rect, corner, bg);
        ui.painter()
            .rect_stroke(rect, corner, Stroke::new(1.0, border), StrokeKind::Inside);

        let dot_center = Pos2::new(rect.left() + pad.x + 5.0, rect.center().y);
        ui.painter()
            .circle_filled(dot_center, 5.0, palette.hero_bg_end);
        ui.painter().circle_filled(dot_center, 2.8, palette.primary);
        ui.painter().circle_stroke(
            dot_center,
            5.0,
            Stroke::new(1.0, Color32::from_black_alpha(32)),
        );

        let text_pos = Pos2::new(
            rect.left() + pad.x + dot_slot,
            rect.center().y - galley.size().y * 0.5,
        );
        let fg = if hovered {
            palette.text
        } else {
            palette.text_muted
        };
        ui.painter()
            .add(egui::Shape::galley_with_override_text_color(text_pos, galley, fg));
    }

    response.on_hover_text("Switch brand color theme")
}

/// Custom-painted row inside the Theme picker popup showing `target_theme`'s hero-and-accent
/// swatch, bold label, muted palette description, and an active `CHECK` glyph.
pub fn theme_menu_row(
    ui: &mut Ui,
    current_palette: Palette,
    target_theme: ThemeId,
    selected: bool,
) -> Response {
    let target_palette = Palette::for_theme(target_theme);
    let desired = Vec2::new(268.0, 38.0);
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    if ui.is_rect_visible(rect) {
        let corner = CornerRadius::same(6);
        if selected {
            ui.painter()
                .rect_filled(rect, corner, current_palette.accent_soft);
        } else if response.hovered() {
            ui.painter()
                .rect_filled(rect, corner, current_palette.surface_muted);
        }

        let swatch_center = Pos2::new(rect.left() + 16.0, rect.center().y);
        ui.painter()
            .circle_filled(swatch_center, 6.5, target_palette.hero_bg_end);
        ui.painter()
            .circle_filled(swatch_center, 3.6, target_palette.primary);
        ui.painter().circle_stroke(
            swatch_center,
            6.5,
            Stroke::new(1.0, Color32::from_black_alpha(36)),
        );

        let title_pos = Pos2::new(rect.left() + 32.0, rect.top() + 5.0);
        ui.painter().text(
            title_pos,
            Align2::LEFT_TOP,
            target_theme.label(),
            font_bold(12.5),
            current_palette.text,
        );
        let desc_pos = Pos2::new(rect.left() + 32.0, rect.top() + 21.0);
        ui.painter().text(
            desc_pos,
            Align2::LEFT_TOP,
            target_theme.description(),
            FontId::new(11.0, FontFamily::Proportional),
            current_palette.text_muted,
        );

        if selected {
            ui.painter().text(
                Pos2::new(rect.right() - 14.0, rect.center().y),
                Align2::CENTER_CENTER,
                CHECK,
                FontId::new(14.0, FontFamily::Proportional),
                current_palette.primary,
            );
        }
    }

    response
}

/// Borderless accent text button (`"Copy to another day…"` on the left side of `ItemForm`).
pub fn accent_ghost_button_enabled(
    ui: &mut Ui,
    palette: Palette,
    label: &str,
    enabled: bool,
) -> Response {
    custom_button(
        ui,
        RichText::new(label).size(13.0),
        CustomButtonStyle {
            idle_bg: Color32::TRANSPARENT,
            hover_bg: palette.accent_soft,
            active_bg: palette.accent_soft_hover,
            fg_idle: palette.accent_soft_fg,
            fg_hover: palette.primary,
            stroke_idle: Stroke::NONE,
            stroke_hover: Stroke::NONE,
            shadow: None,
            top_bevel: false,
            padding: Vec2::new(10.0, 6.0),
            min_size: Vec2::new(0.0, 34.0),
            radius: BTN_RADIUS_PX,
        },
        enabled,
    )
}

/// Soft accent button used for the empty-state `"+ Add Work Item"` action in `DayDetailPanel`.
pub fn soft_accent_button(ui: &mut Ui, palette: Palette, label: RichText) -> Response {
    soft_accent_button_enabled(ui, palette, label, true)
}

/// Soft accent button with explicit `enabled` control (used for `"Download .md"` alongside the
/// primary `"Download PDF"` CTA in the Export modal footer so disabled ranges dim consistently).
pub fn soft_accent_button_enabled(
    ui: &mut Ui,
    palette: Palette,
    label: RichText,
    enabled: bool,
) -> Response {
    custom_button(
        ui,
        label.font(font_bold(13.0)),
        CustomButtonStyle {
            idle_bg: palette.accent_soft,
            hover_bg: palette.accent_soft_hover,
            active_bg: palette.accent_soft_hover,
            fg_idle: palette.accent_soft_fg,
            fg_hover: palette.primary,
            stroke_idle: Stroke::new(1.0, palette.accent_soft_border),
            stroke_hover: Stroke::new(1.0, palette.primary),
            shadow: None,
            top_bevel: false,
            padding: BTN_PADDING,
            min_size: BTN_MIN_SIZE,
            radius: BTN_RADIUS_PX,
        },
        enabled,
    )
}

/// Full-width `38px` dashed `"+ Add another item"` bar at the bottom of a non-empty Day Detail list.
///
/// Stays quiet at rest (`transparent` ground, neutral `border_strong` dashes, `text_muted` label)
/// so existing work-item cards command visual priority, then lights up in `accent_soft` on hover.
pub fn dashed_add_bar_button(ui: &mut Ui, palette: Palette, label: RichText) -> Response {
    let width = ui.available_width().max(120.0);
    let galley = WidgetText::RichText(label.font(font_bold(12.5))).into_galley(
        ui,
        Some(TextWrapMode::Extend),
        f32::INFINITY,
        TextStyle::Button,
    );
    let desired = button_desired_size(galley.size(), Vec2::new(14.0, 8.0), Vec2::new(width, 38.0));
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click());
    let hovered = response.hovered() || response.is_pointer_button_down_on();
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if ui.is_rect_visible(rect) {
        let (bg, dash_color, fg) = if hovered {
            (
                palette.accent_soft,
                palette.accent_soft_border,
                palette.accent_soft_fg,
            )
        } else {
            (
                Color32::TRANSPARENT,
                palette.border_strong,
                palette.text_muted,
            )
        };
        if bg != Color32::TRANSPARENT {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(CARD_RADIUS), bg);
        }
        paint_dashed_rect(
            ui.painter(),
            rect,
            CARD_RADIUS,
            Stroke::new(1.0, dash_color),
            5.0,
            4.0,
        );
        let text_pos = Align2::CENTER_CENTER
            .anchor_size(rect.center(), galley.size())
            .min;
        ui.painter()
            .add(egui::Shape::galley_with_override_text_color(text_pos, galley, fg));
    }
    response
}

/// Stadium-pill date preset chip (`"This week"`, `"Last 7 days"`, etc.) with active-range highlight.
pub fn preset_button(ui: &mut Ui, palette: Palette, label: &str, active: bool) -> Response {
    let text = if active {
        rich_bold(label, 12.0)
    } else {
        RichText::new(label).size(12.0)
    };
    custom_button(
        ui,
        text,
        CustomButtonStyle {
            idle_bg: if active {
                palette.accent_soft
            } else {
                palette.surface_muted
            },
            hover_bg: if active {
                palette.accent_soft_hover
            } else {
                palette.surface_hover
            },
            active_bg: palette.accent_soft_hover,
            fg_idle: if active {
                palette.accent_soft_fg
            } else {
                palette.text_muted
            },
            fg_hover: if active {
                palette.primary
            } else {
                palette.text
            },
            stroke_idle: Stroke::new(
                1.0,
                if active {
                    palette.accent_soft_border
                } else {
                    palette.border
                },
            ),
            stroke_hover: Stroke::new(
                1.0,
                if active {
                    palette.primary
                } else {
                    palette.border_strong
                },
            ),
            shadow: None,
            top_bevel: false,
            padding: Vec2::new(12.0, 4.0),
            min_size: Vec2::new(0.0, 26.0),
            radius: PRESET_RADIUS_PX,
        },
        true,
    )
}

/// Borderless `28x28` ghost icon button on day-detail cards for `Copy` and `Edit`.
pub fn card_icon_button(ui: &mut Ui, palette: Palette, icon: RichText, tooltip: &str) -> Response {
    custom_button(
        ui,
        icon,
        CustomButtonStyle {
            idle_bg: Color32::TRANSPARENT,
            hover_bg: palette.surface_muted,
            active_bg: palette.surface_hover,
            fg_idle: palette.text_muted,
            fg_hover: palette.text,
            stroke_idle: Stroke::NONE,
            stroke_hover: Stroke::NONE,
            shadow: None,
            top_bevel: false,
            padding: Vec2::ZERO,
            min_size: Vec2::splat(28.0),
            radius: 6,
        },
        true,
    )
    .on_hover_text(tooltip)
}

/// Borderless `28x28` ghost icon button on day-detail cards for destructive `Delete` actions.
pub fn card_danger_icon_button(
    ui: &mut Ui,
    palette: Palette,
    icon: RichText,
    tooltip: &str,
) -> Response {
    custom_button(
        ui,
        icon,
        CustomButtonStyle {
            idle_bg: Color32::TRANSPARENT,
            hover_bg: palette.danger_hover_bg,
            active_bg: palette.danger_hover_bg,
            fg_idle: palette.text_muted,
            fg_hover: palette.danger_fg,
            stroke_idle: Stroke::NONE,
            stroke_hover: Stroke::NONE,
            shadow: None,
            top_bevel: false,
            padding: Vec2::ZERO,
            min_size: Vec2::splat(28.0),
            radius: 6,
        },
        true,
    )
    .on_hover_text(tooltip)
}

/// Segmented control tab button inside `SettingsModal`'s `surface_muted` track.
///
/// Active tab renders as an elevated `palette.surface` rounded-6 pill with `1px` `border` and
/// bold ink; inactive tabs sit transparently on the track until hovered.
pub fn settings_tab_button(ui: &mut Ui, palette: Palette, label: &str, active: bool) -> Response {
    let text = if active {
        rich_bold(label, 12.5)
    } else {
        RichText::new(label).size(12.5)
    };
    custom_button(
        ui,
        text,
        CustomButtonStyle {
            idle_bg: if active {
                palette.surface
            } else {
                Color32::TRANSPARENT
            },
            hover_bg: if active {
                palette.surface
            } else {
                palette.surface_hover
            },
            active_bg: palette.surface,
            fg_idle: if active {
                palette.text
            } else {
                palette.text_muted
            },
            fg_hover: palette.text,
            stroke_idle: if active {
                Stroke::new(1.0, palette.border)
            } else {
                Stroke::NONE
            },
            stroke_hover: if active {
                Stroke::new(1.0, palette.border)
            } else {
                Stroke::NONE
            },
            shadow: if active && !palette.is_dark {
                Some(Shadow {
                    offset: [0, 1],
                    blur: 2,
                    spread: 0,
                    color: palette.shadow_sm,
                })
            } else {
                None
            },
            top_bevel: false,
            padding: Vec2::new(14.0, 5.0),
            min_size: Vec2::new(88.0, 28.0),
            radius: 6,
        },
        true,
    )
}

/// Non-interactive compact status pill (`"Today"`, `"Pinned"`) in the Day Detail header.
pub fn status_pill(ui: &mut Ui, bg: Color32, border: Color32, fg: Color32, label: &str) {
    let galley = WidgetText::RichText(rich_bold(label, 11.0)).into_galley(
        ui,
        Some(TextWrapMode::Extend),
        f32::INFINITY,
        TextStyle::Small,
    );
    let pad = Vec2::new(8.0, 2.5);
    let (rect, _) = ui.allocate_exact_size(galley.size() + 2.0 * pad, Sense::hover());
    if ui.is_rect_visible(rect) {
        let corner = CornerRadius::same(99);
        ui.painter().rect_filled(rect, corner, bg);
        ui.painter()
            .rect_stroke(rect, corner, Stroke::new(1.0, border), StrokeKind::Inside);
        let pos = Align2::CENTER_CENTER
            .anchor_size(rect.center(), galley.size())
            .min;
        ui.painter()
            .add(egui::Shape::galley_with_override_text_color(pos, galley, fg));
    }
}

/// Warm gold achievement pill badge (`STAR` Phosphor glyph + `"Achievement"`) on work-item cards.
pub fn achievement_badge(ui: &mut Ui, palette: Palette) -> Response {
    let text = format!("{STAR} Achievement");
    let galley = WidgetText::RichText(rich_bold(text, 11.0)).into_galley(
        ui,
        Some(TextWrapMode::Extend),
        f32::INFINITY,
        TextStyle::Small,
    );
    let pad = Vec2::new(7.5, 2.5);
    let (rect, response) = ui.allocate_exact_size(galley.size() + 2.0 * pad, Sense::hover());
    if ui.is_rect_visible(rect) {
        let corner = CornerRadius::same(6);
        ui.painter()
            .rect_filled(rect, corner, palette.achievement_bg);
        ui.painter().rect_stroke(
            rect,
            corner,
            Stroke::new(1.0, palette.achievement_border),
            StrokeKind::Inside,
        );
        let pos = Align2::CENTER_CENTER
            .anchor_size(rect.center(), galley.size())
            .min;
        ui.painter().add(egui::Shape::galley_with_override_text_color(
            pos,
            galley,
            palette.achievement_fg,
        ));
    }
    response
}

/// Semantic tinted category pill (`Development`, `Code Review`, `Meetings`, `DevOps`,
/// `Documentation`, `Other`) featuring a `3px`-radius color dot and high-contrast label.
pub fn category_pill(ui: &mut Ui, category_id: &str, label: &str, is_dark: bool) {
    let style = category_style(category_id, is_dark);
    let galley = WidgetText::RichText(rich_bold(label, 11.0)).into_galley(
        ui,
        Some(TextWrapMode::Extend),
        f32::INFINITY,
        TextStyle::Small,
    );
    let dot_space = 11.0;
    let pad = Vec2::new(8.0, 2.5);
    let size = Vec2::new(
        galley.size().x + dot_space + 2.0 * pad.x,
        (galley.size().y + 2.0 * pad.y).max(20.0),
    );
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    if ui.is_rect_visible(rect) {
        let corner = CornerRadius::same(6);
        ui.painter().rect_filled(rect, corner, style.bg);
        ui.painter()
            .rect_stroke(rect, corner, Stroke::new(1.0, style.border), StrokeKind::Inside);
        let dot_center = Pos2::new(rect.left() + pad.x + 3.0, rect.center().y);
        ui.painter().circle_filled(dot_center, 3.0, style.dot);
        let text_pos = Pos2::new(
            rect.left() + pad.x + dot_space,
            rect.center().y - galley.size().y * 0.5,
        );
        ui.painter().add(egui::Shape::galley_with_override_text_color(
            text_pos,
            galley,
            style.fg,
        ));
    }
}

/// Tactile gold toggle chip replacing raw `egui::Checkbox` for `"Mark as Achievement"` in `ItemForm`.
pub fn achievement_toggle_chip(ui: &mut Ui, palette: Palette, active: &mut bool) -> Response {
    let caption = if *active {
        format!("{STAR}  Achievement  {CHECK}")
    } else {
        format!("{STAR}  Mark as Achievement")
    };
    let text = if *active {
        rich_bold(caption, 12.5)
    } else {
        RichText::new(caption).size(12.5)
    };
    let resp = custom_button(
        ui,
        text,
        CustomButtonStyle {
            idle_bg: if *active {
                palette.achievement_bg
            } else {
                palette.surface
            },
            hover_bg: if *active {
                palette.achievement_bg
            } else {
                palette.surface_muted
            },
            active_bg: palette.achievement_bg,
            fg_idle: if *active {
                palette.achievement_fg
            } else {
                palette.text_muted
            },
            fg_hover: if *active {
                palette.achievement_fg
            } else {
                palette.text
            },
            stroke_idle: Stroke::new(
                1.0,
                if *active {
                    palette.achievement_border
                } else {
                    palette.border
                },
            ),
            stroke_hover: Stroke::new(
                1.0,
                if *active {
                    palette.achievement_fg
                } else {
                    palette.border_strong
                },
            ),
            shadow: None,
            top_bevel: false,
            padding: Vec2::new(12.0, 6.0),
            min_size: Vec2::new(0.0, 34.0),
            radius: BTN_RADIUS_PX,
        },
        true,
    );
    if resp.clicked() {
        *active = !*active;
    }
    resp
}

/// Padded single-line text input (`10x8` inner margin, `8px` radius) with a crisp `1.5px`
/// `palette.primary` focus ring and soft outer halo when focused.
///
/// Renders `egui::TextEdit::singleline` with `.frame(false)` inside an explicit `egui::Frame`
/// so text baselines sit comfortably inside the box instead of hugging raw sunken borders,
/// then over-strokes the outer frame rectangle in the same immediate-mode pass when focused.
pub fn styled_singleline_input(
    ui: &mut Ui,
    palette: Palette,
    buf: &mut String,
    hint: &str,
) -> Response {
    let radius = CornerRadius::same(8);
    let inner = Frame::new()
        .fill(palette.surface)
        .stroke(Stroke::new(1.0, palette.border))
        .corner_radius(radius)
        .inner_margin(Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(buf)
                    .frame(false)
                    .desired_width(f32::INFINITY)
                    .hint_text(RichText::new(hint).color(palette.text_faint)),
            )
        });
    paint_input_focus_state(ui.painter(), inner.response.rect, radius, palette, &inner.inner);
    inner.inner
}

/// Padded multi-line text input (`10x8` inner margin, `8px` radius) with the same focus-ring
/// treatment as [`styled_singleline_input`].
pub fn styled_multiline_input(
    ui: &mut Ui,
    palette: Palette,
    buf: &mut String,
    rows: usize,
    hint: &str,
) -> Response {
    let radius = CornerRadius::same(8);
    let inner = Frame::new()
        .fill(palette.surface)
        .stroke(Stroke::new(1.0, palette.border))
        .corner_radius(radius)
        .inner_margin(Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(buf)
                    .frame(false)
                    .desired_width(f32::INFINITY)
                    .desired_rows(rows)
                    .hint_text(RichText::new(hint).color(palette.text_faint)),
            )
        });
    paint_input_focus_state(ui.painter(), inner.response.rect, radius, palette, &inner.inner);
    inner.inner
}

fn paint_input_focus_state(
    painter: &Painter,
    rect: Rect,
    radius: CornerRadius,
    palette: Palette,
    text_response: &Response,
) {
    if text_response.has_focus() {
        painter.rect_stroke(
            rect.expand(1.5),
            CornerRadius::same(radius.nw.saturating_add(2)),
            Stroke::new(2.0, palette.accent_soft_border),
            StrokeKind::Outside,
        );
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(1.5, palette.primary),
            StrokeKind::Inside,
        );
    } else if text_response.hovered() {
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, palette.border_strong),
            StrokeKind::Inside,
        );
    }
}

/// Live brand story and workspace telemetry rendered by [`paint_brand_hero_banner`].
pub struct HeroContent<'a> {
    pub app_name: &'a str,
    pub tagline: &'a str,
    pub viewing_context: &'a str,
    pub this_week_count: usize,
    pub month_label: &'a str,
    pub month_count: usize,
    pub month_achievements: usize,
}

fn lerp_color32(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let [ar, ag, ab, aa] = a.to_srgba_unmultiplied();
    let [br, bg, bb, ba] = b.to_srgba_unmultiplied();
    let mix = |x: u8, y: u8| -> u8 {
        (x as f32 + (y as f32 - x as f32) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Color32::from_rgba_unmultiplied(mix(ar, br), mix(ag, bg), mix(ab, bb), mix(aa, ba))
}

/// Paints a rounded rectangle filled with a smooth left-to-right tonal wash (`left_color` ->
/// `right_color`) as a single GPU triangle-fan [`egui::Mesh`].
///
/// Responsibility & invariants:
/// - Because horizontal color interpolation `t(x) = (x - rect.left()) / rect.width()` is affine
///   in screen-space `(x, y)`, GPU barycentric interpolation across the corner-arc triangle fan
///   reproduces the exact linear wash at every interior pixel without vertical strip seams.
/// - Callers pair this fill with an anti-aliased `rect_stroke` on the outer rect so rounded
///   corners stay feather-crisp at any DPI scale.
fn paint_horizontal_gradient_rounded_rect(
    painter: &Painter,
    rect: Rect,
    radius: f32,
    left_color: Color32,
    right_color: Color32,
) {
    if !rect.is_positive() {
        return;
    }
    let r = radius
        .max(0.0)
        .min(0.5 * rect.width().min(rect.height()));
    let width = rect.width().max(1.0);
    let color_at = |p: Pos2| lerp_color32(left_color, right_color, (p.x - rect.left()) / width);

    const ARC_STEPS: usize = 8;
    let corners = [
        (Pos2::new(rect.right() - r, rect.top() + r), -FRAC_PI_2),
        (Pos2::new(rect.right() - r, rect.bottom() - r), 0.0),
        (Pos2::new(rect.left() + r, rect.bottom() - r), FRAC_PI_2),
        (
            Pos2::new(rect.left() + r, rect.top() + r),
            std::f32::consts::PI,
        ),
    ];

    let mut mesh = egui::Mesh::default();
    let center = rect.center();
    mesh.colored_vertex(center, color_at(center));

    for &(corner_center, start_angle) in &corners {
        for step in 0..=ARC_STEPS {
            let theta = start_angle + FRAC_PI_2 * (step as f32 / ARC_STEPS as f32);
            let pos = corner_center + Vec2::new(theta.cos(), theta.sin()) * r;
            mesh.colored_vertex(pos, color_at(pos));
        }
    }

    let perimeter_count = (mesh.vertices.len() - 1) as u32;
    for i in 0..perimeter_count {
        let current = 1 + i;
        let next = 1 + ((i + 1) % perimeter_count);
        mesh.add_triangle(0, current, next);
    }

    painter.add(egui::Shape::mesh(mesh));
}

/// Paints a whisper-subtle architectural horology watermark (concentric watch-dial arcs, 24 radial
/// hour/half-hour graduations, and `10:10` sweep axes) strictly clipped inside `hero_rect`.
///
/// Clipping to `hero_rect.shrink(1.0)` guarantees the oversized dial rings never bleed outside
/// the `14px` rounded hero perimeter on any viewport size.
fn paint_hero_horology_watermark(painter: &Painter, hero_rect: Rect, color: Color32) {
    let inner = hero_rect.shrink(1.0);
    if !inner.is_positive() {
        return;
    }
    let clipped = painter.with_clip_rect(inner);
    let center = Pos2::new(
        hero_rect.right() - hero_rect.width().min(960.0) * 0.24,
        hero_rect.center().y + 10.0,
    );

    let thin = Stroke::new(1.0, color);
    clipped.circle_stroke(center, 126.0, thin);
    clipped.circle_stroke(center, 88.0, thin);
    clipped.circle_stroke(center, 48.0, thin);
    clipped.circle_stroke(center, 3.5, thin);

    for idx in 0..24 {
        let angle = (idx as f32) * (std::f32::consts::TAU / 24.0);
        let dir = Vec2::new(angle.cos(), angle.sin());
        let (r_in, stroke_w) = if idx % 6 == 0 {
            (92.0, 1.25)
        } else if idx % 2 == 0 {
            (98.0, 1.0)
        } else {
            (108.0, 1.0)
        };
        clipped.line_segment(
            [center + dir * r_in, center + dir * 120.0],
            Stroke::new(stroke_w, color),
        );
    }

    let min_dir = Vec2::new((-34.0_f32).to_radians().cos(), (-34.0_f32).to_radians().sin());
    let hr_dir = Vec2::new(
        (-138.0_f32).to_radians().cos(),
        (-138.0_f32).to_radians().sin(),
    );
    clipped.line_segment([center + min_dir * 5.0, center + min_dir * 112.0], thin);
    clipped.line_segment([center + hr_dir * 5.0, center + hr_dir * 74.0], thin);
}

/// Paints the sculpted rounded-10 horological Brand Emblem on the left side of the Hero Banner.
fn paint_hero_brand_emblem(painter: &Painter, rect: Rect, palette: Palette) {
    let corner = CornerRadius::same(10);
    painter.rect_filled(rect, corner, palette.hero_badge_bg);
    painter.rect_stroke(
        rect,
        corner,
        Stroke::new(1.0, palette.hero_card_border),
        StrokeKind::Inside,
    );
    if rect.width() > 16.0 {
        painter.line_segment(
            [
                Pos2::new(rect.left() + 8.0, rect.top() + 1.0),
                Pos2::new(rect.right() - 8.0, rect.top() + 1.0),
            ],
            Stroke::new(1.0, Color32::from_white_alpha(52)),
        );
    }

    let c = rect.center() + Vec2::new(0.0, 1.2);
    let r = if rect.width() >= 40.0 { 11.2 } else { 9.8 };
    painter.circle_stroke(c, r, Stroke::new(1.6, palette.hero_badge_fg));
    painter.line_segment(
        [Pos2::new(c.x, c.y - r - 3.0), Pos2::new(c.x, c.y - r)],
        Stroke::new(1.5, palette.hero_badge_fg),
    );
    painter.line_segment(
        [
            Pos2::new(c.x - 2.8, c.y - r - 3.0),
            Pos2::new(c.x + 2.8, c.y - r - 3.0),
        ],
        Stroke::new(1.6, palette.hero_fg),
    );

    let min_ang = (-38.0_f32).to_radians();
    let hr_ang = (-142.0_f32).to_radians();
    painter.line_segment(
        [
            c,
            c + Vec2::new(min_ang.cos(), min_ang.sin()) * (r * 0.64),
        ],
        Stroke::new(1.7, palette.hero_fg),
    );
    painter.line_segment(
        [c, c + Vec2::new(hr_ang.cos(), hr_ang.sin()) * (r * 0.46)],
        Stroke::new(1.7, palette.hero_fg),
    );
    painter.circle_filled(c, 1.8, palette.hero_gold);
}

/// High-contrast solid ivory CTA button (`+ Log Today`) anchored on the right of the Hero Banner.
fn hero_cta_button(ui: &mut Ui, palette: Palette, compact_height: bool) -> Response {
    let font_sz = if compact_height { 12.0 } else { 12.5 };
    let min_h = if compact_height { 32.0 } else { 36.0 };
    let pad = if compact_height {
        Vec2::new(11.0, 5.0)
    } else {
        Vec2::new(13.0, 7.0)
    };
    let label = icon_label_bold(PLUS, "Log Today", font_sz, palette.hero_bg_start);
    custom_button(
        ui,
        label,
        CustomButtonStyle {
            idle_bg: palette.hero_fg,
            hover_bg: Color32::WHITE,
            active_bg: Color32::from_rgb(226, 232, 240),
            fg_idle: palette.hero_bg_start,
            fg_hover: palette.hero_bg_start,
            stroke_idle: Stroke::NONE,
            stroke_hover: Stroke::NONE,
            shadow: Some(Shadow {
                offset: [0, 2],
                blur: 6,
                spread: 0,
                color: Color32::from_black_alpha(48),
            }),
            top_bevel: false,
            padding: pad,
            min_size: Vec2::new(0.0, min_h),
            radius: 8,
        },
        true,
    )
    .on_hover_text("Jump to today and open the Add Work Item form")
}

/// Translucent frosted-glass telemetry pill on the right side of [`paint_brand_hero_banner`].
fn paint_hero_stat_pill(
    ui: &mut Ui,
    palette: Palette,
    count: usize,
    caption: &str,
    gold_star: bool,
    compact_height: bool,
    narrow_width: bool,
    tooltip: &str,
) {
    let num_size = if compact_height { 14.5 } else { 16.0 };
    let cap_size = if compact_height { 10.5 } else { 11.0 };
    let pill_h = if compact_height { 42.0 } else { 48.0 };
    let pad_x = if narrow_width { 10.0 } else { 13.0 };
    let min_w = if narrow_width { 66.0 } else { 78.0 };

    let num_galley = WidgetText::RichText(rich_bold(format!("{count}"), num_size)).into_galley(
        ui,
        Some(TextWrapMode::Extend),
        f32::INFINITY,
        TextStyle::Body,
    );
    let cap_galley = WidgetText::RichText(RichText::new(caption).size(cap_size)).into_galley(
        ui,
        Some(TextWrapMode::Extend),
        f32::INFINITY,
        TextStyle::Small,
    );
    let star_galley = gold_star.then(|| {
        WidgetText::RichText(RichText::new(STAR).size(num_size - 1.5)).into_galley(
            ui,
            Some(TextWrapMode::Extend),
            f32::INFINITY,
            TextStyle::Body,
        )
    });

    let star_advance = star_galley
        .as_ref()
        .map_or(0.0, |g| g.size().x + 4.5);
    let top_w = star_advance + num_galley.size().x;
    let pill_w = (top_w.max(cap_galley.size().x) + 2.0 * pad_x).max(min_w);

    let (rect, response) = ui.allocate_exact_size(Vec2::new(pill_w, pill_h), Sense::hover());
    if ui.is_rect_visible(rect) {
        let corner = CornerRadius::same(10);
        let fill = if response.hovered() {
            Color32::from_white_alpha(28)
        } else {
            palette.hero_card_bg
        };
        ui.painter().rect_filled(rect, corner, fill);
        ui.painter().rect_stroke(
            rect,
            corner,
            Stroke::new(1.0, palette.hero_card_border),
            StrokeKind::Inside,
        );

        let gap_y = if compact_height { 1.0 } else { 2.0 };
        let stack_h = num_galley.size().y + gap_y + cap_galley.size().y;
        let top_y = rect.center().y - stack_h * 0.5;
        let left_x = rect.left() + pad_x;

        if let Some(sg) = star_galley {
            let star_pos = Pos2::new(
                left_x,
                top_y + (num_galley.size().y - sg.size().y) * 0.5,
            );
            ui.painter().add(egui::Shape::galley_with_override_text_color(
                star_pos,
                sg,
                palette.hero_gold,
            ));
        }

        let num_fg = if gold_star && count > 0 {
            palette.hero_gold
        } else {
            palette.hero_fg
        };
        ui.painter().add(egui::Shape::galley_with_override_text_color(
            Pos2::new(left_x + star_advance, top_y),
            num_galley,
            num_fg,
        ));
        ui.painter().add(egui::Shape::galley_with_override_text_color(
            Pos2::new(left_x, top_y + stack_h - cap_galley.size().y),
            cap_galley,
            palette.hero_muted,
        ));
    }
    let _ = response.on_hover_text(tooltip);
}

/// Renders the full-width Brand Hero Card at the top of the workspace and returns `true` when
/// the user clicks the `+ Log Today` quick-action button.
///
/// Responsibility & invariants:
/// 1. Allocates a deterministic fixed height ([`HERO_HEIGHT`] `86px`, or [`HERO_COMPACT_HEIGHT`]
///    `64px` when `compact_height` is set on short `< 620px` viewports) so font line metrics can
///    never steal vertical budget from the 6-row Month Calendar below.
/// 2. Paints the shadow, smooth monochromatic tonal mesh (`hero_bg_start -> hero_bg_end`),
///    clipped horology dial watermark, and `1px` specular glass border *before* laying out child
///    controls so z-ordering is exact in a single pass.
/// 3. Measures the laid-out left edge of the right-hand telemetry pills + `+ Log Today` CTA first
///    and clips the left-hand brand story strictly to `right_controls_left - gap`, guaranteeing
///    zero text collision even down to [`MIN_WINDOW_WIDTH`].
pub fn paint_brand_hero_banner(
    ui: &mut Ui,
    palette: Palette,
    theme: ThemeId,
    content: HeroContent<'_>,
    compact_height: bool,
) -> bool {
    let width = ui.available_width().max(320.0);
    let height = if compact_height {
        HERO_COMPACT_HEIGHT
    } else {
        HERO_HEIGHT
    };
    let narrow_width = width < 880.0;
    let (hero_rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());

    if ui.is_rect_visible(hero_rect) {
        let corner = CornerRadius::same(HERO_RADIUS);
        let shadow = Shadow {
            offset: [0, 3],
            blur: 10,
            spread: 0,
            color: if palette.is_dark {
                Color32::from_black_alpha(110)
            } else {
                Color32::from_rgba_unmultiplied(15, 23, 42, 26)
            },
        };
        ui.painter().add(shadow.as_shape(hero_rect, corner));

        paint_horizontal_gradient_rounded_rect(
            ui.painter(),
            hero_rect.shrink(0.25),
            HERO_RADIUS as f32,
            palette.hero_bg_start,
            palette.hero_bg_end,
        );
        paint_hero_horology_watermark(ui.painter(), hero_rect, palette.hero_watermark);

        ui.painter().rect_stroke(
            hero_rect,
            corner,
            Stroke::new(1.0, palette.hero_card_border),
            StrokeKind::Inside,
        );
        let bevel_inset = HERO_RADIUS as f32;
        if hero_rect.width() > bevel_inset * 2.0 {
            ui.painter().line_segment(
                [
                    Pos2::new(hero_rect.left() + bevel_inset, hero_rect.top() + 1.0),
                    Pos2::new(hero_rect.right() - bevel_inset, hero_rect.top() + 1.0),
                ],
                Stroke::new(1.0, Color32::from_white_alpha(46)),
            );
        }
    }

    let pad_x = if narrow_width { 14.0 } else { 20.0 };
    let inner_rect = hero_rect.shrink2(Vec2::new(pad_x, 0.0));
    let mut log_today_clicked = false;
    let right_controls_left = {
        // Spawn an overlaid child `Ui` inside the already-allocated `hero_rect` rather than
        // `scope_builder` so parent `ui.cursor()` advances strictly once by `height`.
        let mut right_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(inner_rect)
                .layout(Layout::right_to_left(Align::Center)),
        );
        right_ui.spacing_mut().item_spacing.x = if narrow_width { 6.0 } else { 8.0 };
        if hero_cta_button(&mut right_ui, palette, compact_height).clicked() {
            log_today_clicked = true;
        }
        right_ui.add_space(if narrow_width { 2.0 } else { 4.0 });

        let ach_caption = if narrow_width { "Wins" } else { "Achievements" };
        paint_hero_stat_pill(
            &mut right_ui,
            palette,
            content.month_achievements,
            ach_caption,
            true,
            compact_height,
            narrow_width,
            "Starred achievements recorded in the viewed month",
        );
        paint_hero_stat_pill(
            &mut right_ui,
            palette,
            content.month_count,
            content.month_label,
            false,
            compact_height,
            narrow_width,
            "Work items logged in the currently viewed calendar month",
        );
        paint_hero_stat_pill(
            &mut right_ui,
            palette,
            content.this_week_count,
            "This Week",
            false,
            compact_height,
            narrow_width,
            "Work items logged during the current ISO week (Mon–Sun)",
        );
        right_ui.min_rect().left()
    };

    if ui.is_rect_visible(hero_rect) {
        let safe_gap = if narrow_width { 10.0 } else { 16.0 };
        let left_max_x = (right_controls_left - safe_gap).max(inner_rect.left() + 64.0);
        let left_rect =
            Rect::from_min_max(inner_rect.left_top(), Pos2::new(left_max_x, inner_rect.bottom()));
        let left_painter = ui.painter().with_clip_rect(left_rect);

        let emblem_sz = if compact_height { 38.0 } else { 42.0 };
        let emblem_rect = Rect::from_min_size(
            Pos2::new(left_rect.left(), left_rect.center().y - emblem_sz * 0.5),
            Vec2::splat(emblem_sz),
        );
        paint_hero_brand_emblem(&left_painter, emblem_rect, palette);

        let text_x = emblem_rect.right() + if narrow_width { 11.0 } else { 14.0 };
        let eyebrow_text = format!(
            "{}  ·  {} EDITION",
            content.app_name.to_uppercase(),
            theme.label().to_uppercase()
        );

        if compact_height {
            let top_line = format!("{eyebrow_text}  ·  {}", content.viewing_context);
            let eye_galley = WidgetText::RichText(rich_bold(top_line, 10.5)).into_galley(
                ui,
                Some(TextWrapMode::Extend),
                f32::INFINITY,
                TextStyle::Small,
            );
            let title_sz = if narrow_width { 15.0 } else { 16.0 };
            let title_galley =
                WidgetText::RichText(rich_bold(content.tagline, title_sz)).into_galley(
                    ui,
                    Some(TextWrapMode::Extend),
                    f32::INFINITY,
                    TextStyle::Heading,
                );
            let stack_h = eye_galley.size().y + 3.0 + title_galley.size().y;
            let y0 = hero_rect.center().y - stack_h * 0.5;
            let dot_c = Pos2::new(text_x + 3.0, y0 + eye_galley.size().y * 0.5);
            left_painter.circle_filled(dot_c, 2.8, palette.hero_badge_fg);
            left_painter.add(egui::Shape::galley_with_override_text_color(
                Pos2::new(text_x + 10.0, y0),
                eye_galley,
                palette.hero_badge_fg,
            ));
            left_painter.add(egui::Shape::galley_with_override_text_color(
                Pos2::new(text_x, y0 + stack_h - title_galley.size().y),
                title_galley,
                palette.hero_fg,
            ));
        } else {
            let eye_galley = WidgetText::RichText(rich_bold(eyebrow_text, 10.5)).into_galley(
                ui,
                Some(TextWrapMode::Extend),
                f32::INFINITY,
                TextStyle::Small,
            );
            let title_sz = if narrow_width { 16.0 } else { 18.0 };
            let title_galley =
                WidgetText::RichText(rich_bold(content.tagline, title_sz)).into_galley(
                    ui,
                    Some(TextWrapMode::Extend),
                    f32::INFINITY,
                    TextStyle::Heading,
                );
            let ctx_galley =
                WidgetText::RichText(RichText::new(content.viewing_context).size(12.0))
                    .into_galley(
                        ui,
                        Some(TextWrapMode::Extend),
                        f32::INFINITY,
                        TextStyle::Body,
                    );

            let gap = 3.0;
            let eye_h = eye_galley.size().y;
            let title_h = title_galley.size().y;
            let stack_h = eye_h + gap + title_h + gap + ctx_galley.size().y;
            let y0 = hero_rect.center().y - stack_h * 0.5;
            let dot_c = Pos2::new(text_x + 3.0, y0 + eye_h * 0.5);
            left_painter.circle_filled(dot_c, 2.8, palette.hero_badge_fg);
            left_painter.add(egui::Shape::galley_with_override_text_color(
                Pos2::new(text_x + 10.0, y0),
                eye_galley,
                palette.hero_badge_fg,
            ));
            let y1 = y0 + eye_h + gap;
            left_painter.add(egui::Shape::galley_with_override_text_color(
                Pos2::new(text_x, y1),
                title_galley,
                palette.hero_fg,
            ));
            let y2 = y1 + title_h + gap;
            left_painter.add(egui::Shape::galley_with_override_text_color(
                Pos2::new(text_x, y2),
                ctx_galley,
                palette.hero_muted,
            ));
        }
    }

    log_today_clicked
}

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

    /// Regression test for the `" "` ghost-button width bug: button allocation width must grow
    /// monotonically with the measured label galley width plus `2.0 * padding`, while still
    /// honoring explicit `min_size` floors for square icon buttons and full-width bars.
    #[test]
    fn button_desired_size_grows_with_galley_and_respects_min_size() {
        let space_galley = Vec2::new(4.0, 16.0);
        let export_galley = Vec2::new(68.0, 16.0);
        let download_galley = Vec2::new(96.0, 16.0);

        let space_btn = button_desired_size(space_galley, BTN_PADDING, BTN_MIN_SIZE);
        let export_btn = button_desired_size(export_galley, BTN_PADDING, BTN_MIN_SIZE);
        let download_btn = button_desired_size(download_galley, BTN_PADDING, BTN_MIN_SIZE);

        assert_eq!(export_btn.x, export_galley.x + 2.0 * BTN_PADDING.x);
        assert_eq!(export_btn.y, BTN_MIN_SIZE.y);
        assert!(export_btn.x > space_btn.x);
        assert!(download_btn.x > export_btn.x);

        // Square 34x34 icon-only buttons and full-width 38px bars must honor their min_size floor.
        let icon_btn = button_desired_size(Vec2::splat(16.0), Vec2::ZERO, Vec2::splat(34.0));
        assert_eq!(icon_btn, Vec2::splat(34.0));

        let full_bar = button_desired_size(
            Vec2::new(110.0, 16.0),
            Vec2::new(14.0, 8.0),
            Vec2::new(380.0, 38.0),
        );
        assert_eq!(full_bar, Vec2::new(380.0, 38.0));
    }

    /// Verifies that the Brand Hero Banner renders cleanly across all six themes in both standard
    /// (`86px`) and compact (`64px`) height modes without panics or unexpected height drift.
    #[test]
    fn brand_hero_banner_renders_exact_heights_across_all_six_themes() {
        let ctx = egui::Context::default();
        crate::ui::icons::install_fonts(&ctx);
        for theme in ThemeId::ALL {
            crate::theme::apply_visuals(&ctx, theme);
            let p = Palette::for_theme(theme);
            for (compact, win_w, win_h, expected_h) in [
                (false, DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT, HERO_HEIGHT),
                (true, MIN_WINDOW_WIDTH, 600.0, HERO_COMPACT_HEIGHT),
            ] {
                let raw = egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(win_w, win_h))),
                    ..Default::default()
                };
                let _ = ctx.run(raw, |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let before_y = ui.cursor().top();
                        let clicked = paint_brand_hero_banner(
                            ui,
                            p,
                            theme,
                            HeroContent {
                                app_name: "Time Tracker",
                                tagline: "Track your work. Celebrate your wins.",
                                viewing_context: "Viewing November 2026",
                                this_week_count: 5,
                                month_label: "November",
                                month_count: 18,
                                month_achievements: 4,
                            },
                            compact,
                        );
                        assert!(!clicked);
                        let used_h = ui.cursor().top() - before_y;
                        assert!(
                            (used_h - expected_h).abs() <= 0.01,
                            "expected hero height {expected_h} got {used_h}"
                        );
                    });
                });
            }
        }
    }

    /// Six-row months must fit without scrolling at the launch size and in the shorter
    /// compact-hero band the layout still knows how to draw.
    #[test]
    fn six_row_calendar_fits_below_hero_banner_from_default_down_to_short_windows() {
        // Remaining scroll_h after toolbar, footer, hero band, and calendar card chrome (70px).
        for (win_h, hero_band_h, wrapper_pad_v) in [
            (DEFAULT_WINDOW_HEIGHT, HERO_HEIGHT + 21.0, 24.0),
            (600.0, HERO_COMPACT_HEIGHT + 17.0, 20.0),
            (540.0, HERO_COMPACT_HEIGHT + 13.0, 16.0),
        ] {
            let split_h = win_h - 46.0 - 26.0 - hero_band_h;
            let inner_h = split_h - wrapper_pad_v;
            let scroll_h = inner_h - 70.0;
            let cell = cal_day_cell_for_layout(CAL_COLUMN_MIN_WIDTH, 6, scroll_h);
            let total = cal_month_grid_content_height(6, cell);
            assert!(
                total <= scroll_h + 0.5,
                "at window height {win_h}, 6-row grid {total} exceeded scroll budget {scroll_h}"
            );
            assert!(cell.y >= CAL_DAY_CELL_MIN_HEIGHT);
        }
    }
}
