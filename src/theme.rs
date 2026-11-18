//! "Quiet Precision" color architecture, brand hero tonal palettes, semantic category badge
//! styles, and `egui` style setup.
//!
//! All six [`ThemeId`] variants (`Default` "Light", `Dark`, `Warm`, `Forest`, `Ocean`, `Rose`)
//! preserve their serialized `prefs.json` keys while pairing layered architectural neutrals
//! (`background` canvas, elevated `surface`, subtle `surface_muted` tracks/inputs, `1px` `border`
//! hairlines) with a curated monochromatic tonal Hero Banner (`hero_bg_start -> hero_bg_end`) and
//! a single high-contrast primary accent per theme.

use egui::{Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle, Visuals};
use epaint::Shadow;

use crate::ui::icons::font_bold;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeId {
    Default,
    Dark,
    Warm,
    Forest,
    Ocean,
    Rose,
}

impl ThemeId {
    pub const ALL: [ThemeId; 6] = [
        ThemeId::Default,
        ThemeId::Dark,
        ThemeId::Warm,
        ThemeId::Forest,
        ThemeId::Ocean,
        ThemeId::Rose,
    ];

    /// Parses a persisted `prefs.json` theme string, falling back to [`ThemeId::Default`]
    /// so unknown or legacy values never fail startup.
    pub fn from_pref(s: &str) -> Self {
        match s {
            "dark" => ThemeId::Dark,
            "warm" => ThemeId::Warm,
            "forest" => ThemeId::Forest,
            "ocean" => ThemeId::Ocean,
            "rose" => ThemeId::Rose,
            _ => ThemeId::Default,
        }
    }

    /// Returns the stable `prefs.json` key shared with `~/.weekly-tracker/prefs.json`.
    pub fn as_pref(self) -> &'static str {
        match self {
            ThemeId::Default => "default",
            ThemeId::Dark => "dark",
            ThemeId::Warm => "warm",
            ThemeId::Forest => "forest",
            ThemeId::Ocean => "ocean",
            ThemeId::Rose => "rose",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ThemeId::Default => "Light",
            ThemeId::Dark => "Dark",
            ThemeId::Warm => "Warm",
            ThemeId::Forest => "Forest",
            ThemeId::Ocean => "Ocean",
            ThemeId::Rose => "Rose",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            ThemeId::Default => "Studio Slate · crisp neutral & cobalt",
            ThemeId::Dark => "Obsidian · deep slate & sapphire",
            ThemeId::Warm => "Paper & Amber · warm stone & burnt amber",
            ThemeId::Forest => "Nordic Sage · mineral pine & emerald",
            ThemeId::Ocean => "Fjord · coastal slate & deep ocean",
            ThemeId::Rose => "Terracotta · editorial stone & crimson",
        }
    }

    pub fn is_dark(self) -> bool {
        matches!(self, ThemeId::Dark)
    }
}

/// Semantic pill colors for work-item category chips (`Development`, `Code Review`, etc.).
#[derive(Clone, Copy, Debug)]
pub struct CategoryBadgeStyle {
    pub bg: Color32,
    pub fg: Color32,
    pub dot: Color32,
    pub border: Color32,
}

/// Maps a category identifier (`development`, `code-review`, `meetings`, `devops`,
/// `documentation`, `other`) to a calm, WCAG-legible tinted pill palette tuned separately
/// for light surfaces and dark `Obsidian` surfaces.
///
/// Unknown custom category IDs safely fall back to the neutral slate `"other"` style so
/// externally edited `data.json` files still render cleanly.
pub fn category_style(category_id: &str, is_dark: bool) -> CategoryBadgeStyle {
    if is_dark {
        match category_id {
            "development" => CategoryBadgeStyle {
                bg: Color32::from_rgba_unmultiplied(59, 130, 246, 34),
                fg: hex(0x93C5FD),
                dot: hex(0x60A5FA),
                border: Color32::from_rgba_unmultiplied(59, 130, 246, 84),
            },
            "code-review" => CategoryBadgeStyle {
                bg: Color32::from_rgba_unmultiplied(20, 184, 166, 34),
                fg: hex(0x5EEAD4),
                dot: hex(0x2DD4BF),
                border: Color32::from_rgba_unmultiplied(20, 184, 166, 84),
            },
            "meetings" => CategoryBadgeStyle {
                bg: Color32::from_rgba_unmultiplied(245, 158, 11, 34),
                fg: hex(0xFCD34D),
                dot: hex(0xFBBF24),
                border: Color32::from_rgba_unmultiplied(245, 158, 11, 84),
            },
            "devops" => CategoryBadgeStyle {
                bg: Color32::from_rgba_unmultiplied(16, 185, 129, 34),
                fg: hex(0x6EE7B7),
                dot: hex(0x34D399),
                border: Color32::from_rgba_unmultiplied(16, 185, 129, 84),
            },
            "documentation" => CategoryBadgeStyle {
                bg: Color32::from_rgba_unmultiplied(14, 165, 233, 34),
                fg: hex(0x7DD3FC),
                dot: hex(0x38BDF8),
                border: Color32::from_rgba_unmultiplied(14, 165, 233, 84),
            },
            _ => CategoryBadgeStyle {
                bg: Color32::from_rgba_unmultiplied(148, 163, 184, 28),
                fg: hex(0xCBD5E1),
                dot: hex(0x94A3B8),
                border: Color32::from_rgba_unmultiplied(148, 163, 184, 68),
            },
        }
    } else {
        match category_id {
            "development" => CategoryBadgeStyle {
                bg: hex(0xEFF6FF),
                fg: hex(0x1D4ED8),
                dot: hex(0x2563EB),
                border: hex(0xDBEAFE),
            },
            "code-review" => CategoryBadgeStyle {
                bg: hex(0xF0FDFA),
                fg: hex(0x0F766E),
                dot: hex(0x0D9488),
                border: hex(0xCCFBF1),
            },
            "meetings" => CategoryBadgeStyle {
                bg: hex(0xFFFBEB),
                fg: hex(0xB45309),
                dot: hex(0xD97706),
                border: hex(0xFDE68A),
            },
            "devops" => CategoryBadgeStyle {
                bg: hex(0xECFDF5),
                fg: hex(0x047857),
                dot: hex(0x10B981),
                border: hex(0xD1FAE5),
            },
            "documentation" => CategoryBadgeStyle {
                bg: hex(0xF0F9FF),
                fg: hex(0x0369A1),
                dot: hex(0x0284C7),
                border: hex(0xE0F2FE),
            },
            _ => CategoryBadgeStyle {
                bg: hex(0xF1F5F9),
                fg: hex(0x475569),
                dot: hex(0x64748B),
                border: hex(0xE2E8F0),
            },
        }
    }
}

/// Semantic surface, border, typography, button, and calendar tokens for custom `egui` painting.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub is_dark: bool,
    /// App canvas behind elevated cards (`CentralPanel`). The title bar and footer use
    /// [`crate::ui::chrome::chrome_bar_fill`] instead, so this stays the page ground.
    pub background: Color32,
    /// Primary elevated card surface (top bar, calendar card, detail pane, modals).
    pub surface: Color32,
    /// Subtle sunken/grouped neutral fill (segmented tracks, input rest ground, week column).
    pub surface_muted: Color32,
    /// Interactive hover step above `surface_muted` for neutral/ghost controls.
    pub surface_hover: Color32,
    /// `1px` structural hairline border across cards, buttons, and panels.
    pub border: Color32,
    /// Slightly higher-contrast border for hovered inputs, secondary buttons, and cards.
    pub border_strong: Color32,
    /// Soft internal section separator inside cards and modals.
    pub divider: Color32,

    /// Primary ink for titles, numerals, and active controls.
    pub text: Color32,
    /// Secondary ink for descriptions, subtitles, and idle icons.
    pub text_muted: Color32,
    /// Quiet tertiary ink for out-of-month calendar dates and input hints.
    pub text_faint: Color32,

    /// Solid primary CTA fill, focus rings, and Today numeral badge.
    pub primary: Color32,
    pub primary_hover: Color32,
    pub primary_active: Color32,
    pub on_primary: Color32,

    /// Soft accent surface for active pills, Today tint, and selected calendar cells.
    pub accent_soft: Color32,
    pub accent_soft_hover: Color32,
    pub accent_soft_border: Color32,
    pub accent_soft_fg: Color32,

    /// Calendar ISO week-number strip and day-cell tokens.
    pub cal_wk_inactive_bg: Color32,
    pub cal_wk_inactive_fg: Color32,
    pub cal_wk_active_bg: Color32,
    pub cal_wk_active_fg: Color32,
    pub cal_week_row: Color32,
    pub cal_today_bg: Color32,
    pub cal_today_badge_bg: Color32,
    pub cal_today_badge_fg: Color32,
    pub cal_focus_bg: Color32,
    pub cal_hover_bg: Color32,
    pub cal_dot: Color32,

    /// Warm gold achievement tokens and red destructive-action tokens.
    pub achievement_fg: Color32,
    pub achievement_bg: Color32,
    pub achievement_border: Color32,
    pub danger_fg: Color32,
    pub danger_hover_bg: Color32,
    pub shadow_sm: Color32,

    /// Brand Hero Banner tokens: deep editorial monochromatic tonal wash (`hero_bg_start` ->
    /// `hero_bg_end` within each theme's single color family), crisp ivory typography, frosted
    /// translucent stat-pill glass (`hero_card_bg` / `hero_card_border`), sculpted brand emblem
    /// (`hero_badge_bg` / `hero_badge_fg`), luminous gold win accent (`hero_gold`), and subtle
    /// clipped horology dial watermark (`hero_watermark`).
    pub hero_bg_start: Color32,
    pub hero_bg_end: Color32,
    pub hero_fg: Color32,
    pub hero_muted: Color32,
    pub hero_card_bg: Color32,
    pub hero_card_border: Color32,
    pub hero_badge_bg: Color32,
    pub hero_badge_fg: Color32,
    pub hero_watermark: Color32,
    pub hero_gold: Color32,
}

impl Palette {
    pub fn for_theme(theme: ThemeId) -> Self {
        match theme {
            ThemeId::Default => Palette {
                is_dark: false,
                background: hex(0xF6F8FA),
                surface: hex(0xFFFFFF),
                surface_muted: hex(0xF1F4F8),
                surface_hover: hex(0xE6ECF2),
                border: hex(0xE2E8F0),
                border_strong: hex(0xCBD5E1),
                divider: hex(0xEDF2F7),
                text: hex(0x0F172A),
                text_muted: hex(0x64748B),
                text_faint: hex(0x94A3B8),
                primary: hex(0x1D4ED8),
                primary_hover: hex(0x1E40AF),
                primary_active: hex(0x1E3A8A),
                on_primary: Color32::WHITE,
                accent_soft: hex(0xEFF6FF),
                accent_soft_hover: hex(0xDBEAFE),
                accent_soft_border: hex(0xBFDBFE),
                accent_soft_fg: hex(0x1D4ED8),
                cal_wk_inactive_bg: hex(0xF1F4F8),
                cal_wk_inactive_fg: hex(0x64748B),
                cal_wk_active_bg: hex(0xEFF6FF),
                cal_wk_active_fg: hex(0x1D4ED8),
                cal_week_row: Color32::from_rgba_unmultiplied(239, 246, 255, 140),
                cal_today_bg: hex(0xEFF6FF),
                cal_today_badge_bg: hex(0x1D4ED8),
                cal_today_badge_fg: Color32::WHITE,
                cal_focus_bg: hex(0xEFF6FF),
                cal_hover_bg: hex(0xF1F4F8),
                cal_dot: hex(0x2563EB),
                achievement_fg: hex(0xB45309),
                achievement_bg: hex(0xFEF3C7),
                achievement_border: hex(0xFDE68A),
                danger_fg: hex(0xDC2626),
                danger_hover_bg: hex(0xFEF2F2),
                shadow_sm: Color32::from_rgba_unmultiplied(15, 23, 42, 16),
                hero_bg_start: hex(0x0F172A),
                hero_bg_end: hex(0x1E3A8A),
                hero_fg: hex(0xF8FAFC),
                hero_muted: hex(0xCBD5E1),
                hero_card_bg: Color32::from_white_alpha(20),
                hero_card_border: Color32::from_white_alpha(36),
                hero_badge_bg: Color32::from_rgba_unmultiplied(59, 130, 246, 52),
                hero_badge_fg: hex(0x93C5FD),
                hero_watermark: Color32::from_white_alpha(14),
                hero_gold: hex(0xFCD34D),
            },
            ThemeId::Dark => Palette {
                is_dark: true,
                background: hex(0x0B0F17),
                surface: hex(0x111827),
                surface_muted: hex(0x1A2234),
                surface_hover: hex(0x232E45),
                border: hex(0x243045),
                border_strong: hex(0x364663),
                divider: hex(0x1C2638),
                text: hex(0xF8FAFC),
                text_muted: hex(0x94A3B8),
                text_faint: hex(0x475569),
                primary: hex(0x3B82F6),
                primary_hover: hex(0x2563EB),
                primary_active: hex(0x1D4ED8),
                on_primary: Color32::WHITE,
                accent_soft: Color32::from_rgba_unmultiplied(59, 130, 246, 36),
                accent_soft_hover: Color32::from_rgba_unmultiplied(59, 130, 246, 58),
                accent_soft_border: Color32::from_rgba_unmultiplied(59, 130, 246, 112),
                accent_soft_fg: hex(0x60A5FA),
                cal_wk_inactive_bg: hex(0x172032),
                cal_wk_inactive_fg: hex(0x64748B),
                cal_wk_active_bg: Color32::from_rgba_unmultiplied(59, 130, 246, 42),
                cal_wk_active_fg: hex(0x60A5FA),
                cal_week_row: Color32::from_rgba_unmultiplied(59, 130, 246, 18),
                cal_today_bg: Color32::from_rgba_unmultiplied(59, 130, 246, 32),
                cal_today_badge_bg: hex(0x3B82F6),
                cal_today_badge_fg: Color32::WHITE,
                cal_focus_bg: Color32::from_rgba_unmultiplied(59, 130, 246, 44),
                cal_hover_bg: hex(0x1A2234),
                cal_dot: hex(0x60A5FA),
                achievement_fg: hex(0xFBBF24),
                achievement_bg: Color32::from_rgba_unmultiplied(245, 158, 11, 36),
                achievement_border: Color32::from_rgba_unmultiplied(245, 158, 11, 92),
                danger_fg: hex(0xF87171),
                danger_hover_bg: Color32::from_rgba_unmultiplied(220, 38, 38, 44),
                shadow_sm: Color32::from_rgba_unmultiplied(0, 0, 0, 84),
                hero_bg_start: hex(0x111827),
                hero_bg_end: hex(0x172554),
                hero_fg: hex(0xF8FAFC),
                hero_muted: hex(0xCBD5E1),
                hero_card_bg: Color32::from_white_alpha(22),
                hero_card_border: Color32::from_white_alpha(42),
                hero_badge_bg: Color32::from_rgba_unmultiplied(96, 165, 250, 48),
                hero_badge_fg: hex(0x93C5FD),
                hero_watermark: Color32::from_white_alpha(14),
                hero_gold: hex(0xFCD34D),
            },
            ThemeId::Warm => Palette {
                is_dark: false,
                background: hex(0xFAF8F5),
                surface: hex(0xFFFFFF),
                surface_muted: hex(0xF3EFEA),
                surface_hover: hex(0xEAE4DC),
                border: hex(0xE6E0D6),
                border_strong: hex(0xD6CEC2),
                divider: hex(0xEFECE6),
                text: hex(0x1C1917),
                text_muted: hex(0x78716C),
                text_faint: hex(0xA8A29E),
                primary: hex(0xB45309),
                primary_hover: hex(0x92400E),
                primary_active: hex(0x78350F),
                on_primary: Color32::WHITE,
                accent_soft: hex(0xFEF3C7),
                accent_soft_hover: hex(0xFDE68A),
                accent_soft_border: hex(0xFCD34D),
                accent_soft_fg: hex(0x92400E),
                cal_wk_inactive_bg: hex(0xF3EFEA),
                cal_wk_inactive_fg: hex(0x78716C),
                cal_wk_active_bg: hex(0xFEF3C7),
                cal_wk_active_fg: hex(0x92400E),
                cal_week_row: Color32::from_rgba_unmultiplied(254, 243, 199, 110),
                cal_today_bg: hex(0xFFFBEB),
                cal_today_badge_bg: hex(0xB45309),
                cal_today_badge_fg: Color32::WHITE,
                cal_focus_bg: hex(0xFEF3C7),
                cal_hover_bg: hex(0xF3EFEA),
                cal_dot: hex(0xD97706),
                achievement_fg: hex(0x92400E),
                achievement_bg: hex(0xFEF3C7),
                achievement_border: hex(0xFCD34D),
                danger_fg: hex(0xDC2626),
                danger_hover_bg: hex(0xFEF2F2),
                shadow_sm: Color32::from_rgba_unmultiplied(28, 25, 23, 16),
                hero_bg_start: hex(0x291D14),
                hero_bg_end: hex(0x78350F),
                hero_fg: hex(0xF8FAFC),
                hero_muted: hex(0xEAD8C7),
                hero_card_bg: Color32::from_white_alpha(20),
                hero_card_border: Color32::from_white_alpha(36),
                hero_badge_bg: Color32::from_rgba_unmultiplied(245, 158, 11, 50),
                hero_badge_fg: hex(0xFDE68A),
                hero_watermark: Color32::from_white_alpha(14),
                hero_gold: hex(0xFDE68A),
            },
            ThemeId::Forest => Palette {
                is_dark: false,
                background: hex(0xF4F7F5),
                surface: hex(0xFFFFFF),
                surface_muted: hex(0xEBF1ED),
                surface_hover: hex(0xDFEAE3),
                border: hex(0xD8E4DC),
                border_strong: hex(0xC2D4C8),
                divider: hex(0xE7EFEA),
                text: hex(0x11221A),
                text_muted: hex(0x526B5E),
                text_faint: hex(0x8FA599),
                primary: hex(0x047857),
                primary_hover: hex(0x065F46),
                primary_active: hex(0x064E3B),
                on_primary: Color32::WHITE,
                accent_soft: hex(0xECFDF5),
                accent_soft_hover: hex(0xD1FAE5),
                accent_soft_border: hex(0xA7F3D0),
                accent_soft_fg: hex(0x047857),
                cal_wk_inactive_bg: hex(0xEBF1ED),
                cal_wk_inactive_fg: hex(0x526B5E),
                cal_wk_active_bg: hex(0xD1FAE5),
                cal_wk_active_fg: hex(0x047857),
                cal_week_row: Color32::from_rgba_unmultiplied(209, 250, 229, 110),
                cal_today_bg: hex(0xECFDF5),
                cal_today_badge_bg: hex(0x047857),
                cal_today_badge_fg: Color32::WHITE,
                cal_focus_bg: hex(0xECFDF5),
                cal_hover_bg: hex(0xEBF1ED),
                cal_dot: hex(0x059669),
                achievement_fg: hex(0xB45309),
                achievement_bg: hex(0xFEF3C7),
                achievement_border: hex(0xFDE68A),
                danger_fg: hex(0xDC2626),
                danger_hover_bg: hex(0xFEF2F2),
                shadow_sm: Color32::from_rgba_unmultiplied(17, 34, 26, 16),
                hero_bg_start: hex(0x09221A),
                hero_bg_end: hex(0x064E3B),
                hero_fg: hex(0xF8FAFC),
                hero_muted: hex(0xC6E2D6),
                hero_card_bg: Color32::from_white_alpha(20),
                hero_card_border: Color32::from_white_alpha(36),
                hero_badge_bg: Color32::from_rgba_unmultiplied(16, 185, 129, 48),
                hero_badge_fg: hex(0x6EE7B7),
                hero_watermark: Color32::from_white_alpha(14),
                hero_gold: hex(0xFCD34D),
            },
            ThemeId::Ocean => Palette {
                is_dark: false,
                background: hex(0xF4F8FA),
                surface: hex(0xFFFFFF),
                surface_muted: hex(0xEAF1F6),
                surface_hover: hex(0xDDE8F0),
                border: hex(0xD5E3EC),
                border_strong: hex(0xBDD2E0),
                divider: hex(0xE6EFF5),
                text: hex(0x0C2434),
                text_muted: hex(0x526D82),
                text_faint: hex(0x8CA3B5),
                primary: hex(0x0369A1),
                primary_hover: hex(0x075985),
                primary_active: hex(0x0C4A6E),
                on_primary: Color32::WHITE,
                accent_soft: hex(0xF0F9FF),
                accent_soft_hover: hex(0xE0F2FE),
                accent_soft_border: hex(0xBAE6FD),
                accent_soft_fg: hex(0x0369A1),
                cal_wk_inactive_bg: hex(0xEAF1F6),
                cal_wk_inactive_fg: hex(0x526D82),
                cal_wk_active_bg: hex(0xE0F2FE),
                cal_wk_active_fg: hex(0x0369A1),
                cal_week_row: Color32::from_rgba_unmultiplied(224, 242, 254, 115),
                cal_today_bg: hex(0xF0F9FF),
                cal_today_badge_bg: hex(0x0369A1),
                cal_today_badge_fg: Color32::WHITE,
                cal_focus_bg: hex(0xE0F2FE),
                cal_hover_bg: hex(0xEAF1F6),
                cal_dot: hex(0x0284C7),
                achievement_fg: hex(0xB45309),
                achievement_bg: hex(0xFEF3C7),
                achievement_border: hex(0xFDE68A),
                danger_fg: hex(0xDC2626),
                danger_hover_bg: hex(0xFEF2F2),
                shadow_sm: Color32::from_rgba_unmultiplied(12, 36, 52, 16),
                hero_bg_start: hex(0x082032),
                hero_bg_end: hex(0x0C4A6E),
                hero_fg: hex(0xF8FAFC),
                hero_muted: hex(0xC5DFEE),
                hero_card_bg: Color32::from_white_alpha(20),
                hero_card_border: Color32::from_white_alpha(36),
                hero_badge_bg: Color32::from_rgba_unmultiplied(56, 189, 248, 46),
                hero_badge_fg: hex(0x7DD3FC),
                hero_watermark: Color32::from_white_alpha(14),
                hero_gold: hex(0xFCD34D),
            },
            ThemeId::Rose => Palette {
                is_dark: false,
                background: hex(0xFAF6F6),
                surface: hex(0xFFFFFF),
                surface_muted: hex(0xF4ECEE),
                surface_hover: hex(0xEBE0E3),
                border: hex(0xE8D8DC),
                border_strong: hex(0xD8C3C8),
                divider: hex(0xF0E6E8),
                text: hex(0x2A1218),
                text_muted: hex(0x7A5660),
                text_faint: hex(0xAE929A),
                primary: hex(0xBE123C),
                primary_hover: hex(0x9F1239),
                primary_active: hex(0x881337),
                on_primary: Color32::WHITE,
                accent_soft: hex(0xFFF1F2),
                accent_soft_hover: hex(0xFFE4E6),
                accent_soft_border: hex(0xFECDD3),
                accent_soft_fg: hex(0xBE123C),
                cal_wk_inactive_bg: hex(0xF4ECEE),
                cal_wk_inactive_fg: hex(0x7A5660),
                cal_wk_active_bg: hex(0xFFE4E6),
                cal_wk_active_fg: hex(0xBE123C),
                cal_week_row: Color32::from_rgba_unmultiplied(255, 228, 230, 115),
                cal_today_bg: hex(0xFFF1F2),
                cal_today_badge_bg: hex(0xBE123C),
                cal_today_badge_fg: Color32::WHITE,
                cal_focus_bg: hex(0xFFE4E6),
                cal_hover_bg: hex(0xF4ECEE),
                cal_dot: hex(0xE11D48),
                achievement_fg: hex(0xB45309),
                achievement_bg: hex(0xFEF3C7),
                achievement_border: hex(0xFDE68A),
                danger_fg: hex(0xDC2626),
                danger_hover_bg: hex(0xFEF2F2),
                shadow_sm: Color32::from_rgba_unmultiplied(42, 18, 24, 16),
                hero_bg_start: hex(0x2A0E17),
                hero_bg_end: hex(0x881337),
                hero_fg: hex(0xF8FAFC),
                hero_muted: hex(0xECCBD3),
                hero_card_bg: Color32::from_white_alpha(20),
                hero_card_border: Color32::from_white_alpha(36),
                hero_badge_bg: Color32::from_rgba_unmultiplied(251, 113, 133, 46),
                hero_badge_fg: hex(0xFDA4AF),
                hero_watermark: Color32::from_white_alpha(14),
                hero_gold: hex(0xFCD34D),
            },
        }
    }
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb(
        ((rgb >> 16) & 0xFF) as u8,
        ((rgb >> 8) & 0xFF) as u8,
        (rgb & 0xFF) as u8,
    )
}

/// Applies the typographic scale, spacing rhythm, and `egui::Visuals` for `theme` onto `ctx`.
///
/// Responsibility & invariants:
/// - Updates `text_styles`, `spacing`, and `visuals` inside a single `ctx.style_mut` transaction
///   so native `egui` widgets (`ComboBox` popups, checkboxes, scrollbars, tooltips) inherit the
///   exact same `7px` corner radius, `1px` hairline borders, and vertical rhythm (~34px control
///   height) as custom-painted controls in [`crate::ui::chrome`].
/// - Requires [`crate::ui::icons::install_fonts`] to have registered [`crate::ui::icons::bold_family`]
///   first because `TextStyle::Heading` binds directly to the `700`-weight `NotoSans-Bold` family.
pub fn apply_visuals(ctx: &egui::Context, theme: ThemeId) {
    let p = Palette::for_theme(theme);
    let mut visuals = if theme.is_dark() {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    let radius = CornerRadius::same(7);
    visuals.panel_fill = p.background;
    visuals.window_fill = p.surface;
    visuals.window_stroke = Stroke::new(1.0, p.border);
    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.window_shadow = Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: if p.is_dark {
            Color32::from_black_alpha(130)
        } else {
            Color32::from_rgba_unmultiplied(15, 23, 42, 28)
        },
    };
    visuals.popup_shadow = Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: if p.is_dark {
            Color32::from_black_alpha(120)
        } else {
            Color32::from_rgba_unmultiplied(15, 23, 42, 24)
        },
    };
    visuals.menu_corner_radius = CornerRadius::same(9);
    visuals.extreme_bg_color = p.surface;
    visuals.faint_bg_color = p.surface_muted;
    visuals.override_text_color = None;

    visuals.widgets.noninteractive.bg_fill = p.surface;
    visuals.widgets.noninteractive.weak_bg_fill = p.surface_muted;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.noninteractive.corner_radius = radius;

    visuals.widgets.inactive.bg_fill = p.surface;
    visuals.widgets.inactive.weak_bg_fill = p.surface;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, p.border);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.inactive.corner_radius = radius;
    visuals.widgets.inactive.expansion = 0.0;

    visuals.widgets.hovered.bg_fill = p.surface_muted;
    visuals.widgets.hovered.weak_bg_fill = p.surface_muted;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, p.border_strong);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.hovered.corner_radius = radius;
    visuals.widgets.hovered.expansion = 0.0;

    visuals.widgets.active.bg_fill = p.primary;
    visuals.widgets.active.weak_bg_fill = p.primary;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, p.primary);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, p.on_primary);
    visuals.widgets.active.corner_radius = radius;
    visuals.widgets.active.expansion = 0.0;

    visuals.widgets.open.bg_fill = p.accent_soft;
    visuals.widgets.open.weak_bg_fill = p.accent_soft;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, p.accent_soft_border);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.open.corner_radius = radius;
    visuals.widgets.open.expansion = 0.0;

    visuals.selection.bg_fill = p.accent_soft_hover;
    visuals.selection.stroke = Stroke::new(1.0, p.primary);
    visuals.hyperlink_color = p.primary;
    visuals.warn_fg_color = p.achievement_fg;
    visuals.error_fg_color = p.danger_fg;

    ctx.style_mut(|style| {
        style.text_styles.insert(TextStyle::Heading, font_bold(18.0));
        style.text_styles.insert(
            TextStyle::Body,
            FontId::new(13.5, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(13.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Small,
            FontId::new(11.5, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Monospace,
            FontId::new(12.5, FontFamily::Monospace),
        );

        style.spacing.button_padding = egui::Vec2::new(12.0, 7.0);
        style.spacing.item_spacing = egui::Vec2::new(8.0, 6.0);
        style.spacing.menu_margin = Margin::same(6);
        style.visuals = visuals;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_pref_roundtrip_preserves_all_six_variants() {
        for theme in ThemeId::ALL {
            assert_eq!(ThemeId::from_pref(theme.as_pref()), theme);
        }
        assert_eq!(ThemeId::from_pref("unknown-legacy"), ThemeId::Default);
    }

    #[test]
    fn category_styles_resolve_for_known_and_custom_categories() {
        for dark in [false, true] {
            for id in [
                "development",
                "code-review",
                "meetings",
                "devops",
                "documentation",
                "other",
                "custom-tag",
            ] {
                let s = category_style(id, dark);
                assert_ne!(s.fg, Color32::TRANSPARENT);
                assert_ne!(s.dot, Color32::TRANSPARENT);
            }
        }
    }

    #[test]
    fn hero_tokens_preserve_tonal_depth_and_legible_contrast_across_all_themes() {
        fn rel_luma(c: Color32) -> f32 {
            let [r, g, b, _] = c.to_srgba_unmultiplied();
            let chan = |v: u8| {
                let s = v as f32 / 255.0;
                if s <= 0.03928 {
                    s / 12.92
                } else {
                    ((s + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * chan(r) + 0.7152 * chan(g) + 0.0722 * chan(b)
        }

        for theme in ThemeId::ALL {
            let p = Palette::for_theme(theme);
            assert_ne!(
                p.hero_bg_start, p.hero_bg_end,
                "{theme:?} hero must have a tonal wash between distinct stops"
            );
            assert_ne!(
                p.hero_bg_start, p.background,
                "{theme:?} hero start must stand apart from canvas background"
            );
            let fg_luma = rel_luma(p.hero_fg);
            let muted_luma = rel_luma(p.hero_muted);
            for bg in [p.hero_bg_start, p.hero_bg_end] {
                let bg_luma = rel_luma(bg);
                let fg_contrast = (fg_luma + 0.05) / (bg_luma + 0.05);
                let muted_contrast = (muted_luma + 0.05) / (bg_luma + 0.05);
                assert!(
                    fg_contrast >= 7.0,
                    "{theme:?} hero_fg WCAG AAA contrast too low: {fg_contrast:.2}"
                );
                assert!(
                    muted_contrast >= 4.5,
                    "{theme:?} hero_muted WCAG AA contrast too low: {muted_contrast:.2}"
                );
            }
        }
    }

    #[test]
    fn fonts_and_all_themes_run_headless_frame_without_panic() {
        let ctx = egui::Context::default();
        crate::ui::icons::install_fonts(&ctx);
        for theme in ThemeId::ALL {
            apply_visuals(&ctx, theme);
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.heading("Work Tracker");
                    ui.label("Body copy check");
                });
            });
        }
    }
}
