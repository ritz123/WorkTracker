//! Phosphor icon font (PUA glyphs) for toolbar and day-panel actions.
//!
//! Font bytes are vendored from the Phosphor set (MIT); we register the family on the
//! egui context at startup so `Proportional` text can render these code points without
//! pulling in a second `egui` version via `egui-phosphor`.

use egui::{Color32, Context, FontData, FontFamily, RichText};
use epaint::text::{FontInsert, FontPriority, InsertFontFamily};

/// Registers Phosphor as a proportional fallback so PUA icon code points resolve without
/// replacing the default UI font.
pub fn install_phosphor_font(ctx: &Context) {
    ctx.add_font(FontInsert {
        name: "phosphor".into(),
        data: FontData::from_static(include_bytes!("../../assets/fonts/Phosphor.ttf")),
        families: vec![InsertFontFamily {
            family: FontFamily::Proportional,
            priority: FontPriority::Lowest,
        }],
    });
}

// Glyphs match Phosphor Regular (same code points as egui-phosphor `regular` module).
pub const TIMER: &str = "\u{E492}";
pub const CARET_LEFT: &str = "\u{E138}";
pub const CARET_RIGHT: &str = "\u{E13A}";
pub const CALENDAR_BLANK: &str = "\u{E10A}";
pub const EXPORT: &str = "\u{EAF0}";
pub const GEAR: &str = "\u{E270}";
pub const PALETTE: &str = "\u{E6C8}";
pub const PLUS: &str = "\u{E3D4}";
pub const PUSH_PIN: &str = "\u{E3E2}";
pub const PUSH_PIN_SLASH: &str = "\u{E3E4}";
pub const COPY: &str = "\u{E1CA}";
pub const TRASH: &str = "\u{E4A6}";
pub const PENCIL: &str = "\u{E3B4}";

/// Icon-only label sized for compact toolbar chips.
pub fn icon_text(icon: &str, size: f32, color: Color32) -> RichText {
    RichText::new(icon).size(size).color(color)
}

/// Icon plus short caption (single-line toolbar / chip).
pub fn icon_label(icon: &str, label: &str, size: f32, color: Color32) -> RichText {
    RichText::new(format!("{icon}  {label}"))
        .size(size)
        .color(color)
}

/// Small icon button on day cards; defers painting to [`crate::ui::chrome::card_icon_button`].
pub fn small_icon_button(
    ui: &mut egui::Ui,
    palette: crate::theme::Palette,
    icon: &str,
    tooltip: &str,
) -> egui::Response {
    crate::ui::chrome::card_icon_button(ui, palette, icon_text(icon, 16.0, palette.primary), tooltip)
}