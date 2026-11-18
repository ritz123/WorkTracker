//! Typography and Phosphor PUA iconography registration for the `egui` context.
//!
//! Replaces `egui`'s spindly built-in `Ubuntu-Light` proportional font with vendored
//! `NotoSans-Regular` (weight 400) for crisp UI body copy and `NotoSans-Bold` (weight 700)
//! bound to a dedicated `"bold"` [`FontFamily`], while keeping `Phosphor.ttf` (MIT) as a
//! lowest-priority fallback on both families so Private Use Area (PUA) icon glyphs render
//! without missing-glyph tofu in either regular or bold labels.

use std::sync::Arc;

use egui::{Color32, Context, FontData, FontDefinitions, FontFamily, FontId, RichText};

/// Named [`FontFamily`] key backed by `NotoSans-Bold` (`usWeightClass = 700`).
///
/// `egui` does not synthesize bold weights from `RichText::strong()`—`.strong()` only alters
/// foreground color. Binding a real bold face to `FontFamily::Name("bold")` gives headings,
/// date titles, active tabs, and primary CTAs genuine typographic hierarchy.
pub const BOLD_FAMILY_NAME: &str = "bold";

/// Returns the [`FontFamily`] handle for bold (`700`-weight) UI typography.
pub fn bold_family() -> FontFamily {
    FontFamily::Name(BOLD_FAMILY_NAME.into())
}

/// Constructs a [`FontId`] rendered in `NotoSans-Bold` at `size` logical pixels.
pub fn font_bold(size: f32) -> FontId {
    FontId::new(size, bold_family())
}

/// Constructs [`RichText`] rendered in `NotoSans-Bold` at `size` logical pixels.
pub fn rich_bold(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).font(font_bold(size))
}

/// Installs the application font stack (`NotoSans-Regular`, `NotoSans-Bold`, `Hack` monospace,
/// built-in emoji fallbacks, and `Phosphor` PUA icons) onto `ctx`.
///
/// Responsibility & invariants:
/// - Called once during [`crate::app::WeeklyTrackerApp::new`] before [`crate::theme::apply_visuals`]
///   assigns `TextStyle::Heading` to [`bold_family`]. Calling `apply_visuals` without first
///   registering `"bold"` in `FontDefinitions::families` would panic in `epaint` when laying out
///   headings.
/// - Strips `Ubuntu-Light` from `FontFamily::Proportional` while preserving `egui`'s bundled
///   symbol/emoji fallbacks (verified against `fonts.font_data` so internal `egui` font renames
///   can never trigger a missing-font panic).
/// - Appends `"phosphor"` to *both* `Proportional` and `"bold"` so strings that mix Phosphor PUA
///   code points with bold captions (such as `"+  Add Item"` or `"Export"`) resolve every glyph.
pub fn install_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "noto_sans_regular".into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/NotoSans-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "noto_sans_bold".into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/NotoSans-Bold.ttf"
        ))),
    );
    fonts.font_data.insert(
        "phosphor".into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/Phosphor.ttf"
        ))),
    );

    let existing_fallbacks: Vec<String> = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|name| name != "Ubuntu-Light" && fonts.font_data.contains_key(name))
        .collect();

    let mut proportional_chain = vec!["noto_sans_regular".to_string()];
    proportional_chain.extend(existing_fallbacks.iter().cloned());
    proportional_chain.push("phosphor".to_string());
    fonts
        .families
        .insert(FontFamily::Proportional, proportional_chain);

    let mut bold_chain = vec![
        "noto_sans_bold".to_string(),
        "noto_sans_regular".to_string(),
    ];
    bold_chain.extend(existing_fallbacks);
    bold_chain.push("phosphor".to_string());
    fonts.families.insert(bold_family(), bold_chain);

    if let Some(mono) = fonts.families.get_mut(&FontFamily::Monospace) {
        if !mono.iter().any(|n| n == "phosphor") {
            mono.push("phosphor".to_string());
        }
    }

    ctx.set_fonts(fonts);
}

// Glyphs match Phosphor Regular PUA code points verified against `assets/fonts/Phosphor.ttf`.
pub const TIMER: &str = "\u{E492}";
pub const CARET_DOWN: &str = "\u{E136}";
pub const CARET_LEFT: &str = "\u{E138}";
pub const CARET_RIGHT: &str = "\u{E13A}";
pub const CALENDAR_BLANK: &str = "\u{E10A}";
pub const CHECK: &str = "\u{E182}";
pub const COPY: &str = "\u{E1CA}";
pub const DOWNLOAD: &str = "\u{E20A}";
pub const EXPORT: &str = "\u{EAF0}";
pub const GEAR: &str = "\u{E270}";
pub const PENCIL: &str = "\u{E3B4}";
pub const PLUS: &str = "\u{E3D4}";
pub const PUSH_PIN: &str = "\u{E3E2}";
pub const PUSH_PIN_SLASH: &str = "\u{E3E4}";
pub const STAR: &str = "\u{E46A}";
pub const TRASH: &str = "\u{E4A6}";
pub const X: &str = "\u{E4F6}";

/// Icon-only [`RichText`] at `size` logical pixels and `color`.
pub fn icon_text(icon: &str, size: f32, color: Color32) -> RichText {
    RichText::new(icon).size(size).color(color)
}

/// Regular-weight icon plus short single-line caption (`"{icon}  {label}"`).
pub fn icon_label(icon: &str, label: &str, size: f32, color: Color32) -> RichText {
    RichText::new(format!("{icon}  {label}"))
        .size(size)
        .color(color)
}

/// Bold-weight icon plus short single-line caption (`"{icon}  {label}"`) for primary CTAs.
pub fn icon_label_bold(icon: &str, label: &str, size: f32, color: Color32) -> RichText {
    rich_bold(format!("{icon}  {label}"), size).color(color)
}

/// Ghost icon button on day-detail item cards for neutral actions (`Copy`, `Edit`).
///
/// Passes a 15px Phosphor glyph to [`crate::ui::chrome::card_icon_button`], which paints a
/// borderless `28x28` slot (`text_muted` idle, `surface_muted` fill + `text` glyph on hover)
/// and attaches `tooltip` on hover.
pub fn small_icon_button(
    ui: &mut egui::Ui,
    palette: crate::theme::Palette,
    icon: &str,
    tooltip: &str,
) -> egui::Response {
    crate::ui::chrome::card_icon_button(
        ui,
        palette,
        icon_text(icon, 15.0, palette.text_muted),
        tooltip,
    )
}

/// Ghost icon button on day-detail item cards for destructive actions (`Delete`).
///
/// Renders identically to [`small_icon_button`] when idle (`transparent` fill, `text_muted` glyph)
/// so cards stay calm at rest, then transitions to `palette.danger_hover_bg` and `palette.danger_fg`
/// on hover/press to signal a destructive action before click confirmation.
pub fn small_danger_icon_button(
    ui: &mut egui::Ui,
    palette: crate::theme::Palette,
    icon: &str,
    tooltip: &str,
) -> egui::Response {
    crate::ui::chrome::card_danger_icon_button(
        ui,
        palette,
        icon_text(icon, 15.0, palette.text_muted),
        tooltip,
    )
}
