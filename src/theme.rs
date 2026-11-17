use egui::{Color32, Stroke, Visuals};

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
            ThemeId::Default => "Indigo/violet (matches Electron default)",
            ThemeId::Dark => "Low-light slate and violet",
            ThemeId::Warm => "Cream and amber",
            ThemeId::Forest => "Emerald greens",
            ThemeId::Ocean => "Sky blues and cyan",
            ThemeId::Rose => "Soft pinks",
        }
    }
}

/// Semantic colors for custom painting (calendar, cards) beyond egui widgets.
#[derive(Clone, Copy)]
pub struct Palette {
    pub background: Color32,
    pub surface: Color32,
    pub primary: Color32,
    pub accent: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub border: Color32,
    /// Solid fill for the top app bar (primary brand strip).
    pub toolbar_bg: Color32,
    /// Labels and nav controls on the toolbar (high contrast on `toolbar_bg`).
    pub toolbar_fg: Color32,
    /// Thick accent line under the toolbar.
    pub toolbar_accent: Color32,
    pub cal_week_row: Color32,
    /// Shaded fill behind inactive ISO week numbers (left column).
    pub cal_wk_inactive_bg: Color32,
    pub cal_wk_inactive_fg: Color32,
    /// Current calendar week’s week-number cell (matches Electron `--cal-wk-active-*`).
    pub cal_wk_active_bg: Color32,
    pub cal_wk_active_fg: Color32,
    pub cal_today: Color32,
    pub cal_focus: Color32,
    pub cal_hover: Color32,
    /// Activity dots when 1 / 2 / 3+ items on a day (`--cal-dot-*`).
    pub cal_dot_1: Color32,
    pub cal_dot_2: Color32,
    pub cal_dot_3: Color32,
    pub cal_cell_out_bg: Color32,
    pub cal_cell_out_text: Color32,
    pub cal_cell_in_text: Color32,
    pub cal_day_today_bg: Color32,
    pub cal_day_today_fg: Color32,
    pub day_header_today_start: Color32,
    pub day_header_today_end: Color32,
    pub achievement: Color32,
}

impl Palette {
    pub fn for_theme(theme: ThemeId) -> Self {
        match theme {
            ThemeId::Default => Palette {
                background: hex(0xF8FAFC),
                surface: hex(0xFFFFFF),
                primary: hex(0x4F46E5),
                accent: hex(0x6366F1),
                text: hex(0x1E293B),
                text_muted: hex(0x64748B),
                border: hex(0xE2E8F0),
                toolbar_bg: hex(0x4F46E5),
                toolbar_fg: Color32::WHITE,
                toolbar_accent: hex(0x7C3AED),
                cal_week_row: Color32::from_rgba_unmultiplied(238, 242, 255, 102),
                cal_wk_inactive_bg: hex(0xF8FAFC),
                cal_wk_inactive_fg: hex(0x94A3B8),
                cal_wk_active_bg: hex(0x6366F1),
                cal_wk_active_fg: Color32::WHITE,
                cal_today: Color32::from_rgba_unmultiplied(238, 242, 255, 179),
                cal_focus: hex(0xE0E7FF),
                cal_hover: hex(0xEEF2FF),
                cal_dot_1: hex(0xA5B4FC),
                cal_dot_2: hex(0x818CF8),
                cal_dot_3: hex(0x6366F1),
                cal_cell_out_bg: Color32::from_rgba_unmultiplied(248, 250, 252, 128),
                cal_cell_out_text: hex(0xCBD5E1),
                cal_cell_in_text: hex(0x334155),
                cal_day_today_bg: hex(0x6366F1),
                cal_day_today_fg: Color32::WHITE,
                day_header_today_start: hex(0x6366F1),
                day_header_today_end: hex(0x7C3AED),
                achievement: hex(0xF59E0B),
            },
            ThemeId::Dark => Palette {
                background: hex(0x0F172A),
                surface: hex(0x1E293B),
                primary: hex(0x818CF8),
                accent: hex(0xA5B4FC),
                text: hex(0xF1F5F9),
                text_muted: hex(0x94A3B8),
                border: hex(0x334155),
                toolbar_bg: hex(0x3730A3),
                toolbar_fg: Color32::WHITE,
                toolbar_accent: hex(0x5B21B6),
                cal_week_row: Color32::from_rgba_unmultiplied(49, 46, 129, 64),
                cal_wk_inactive_bg: hex(0x1E293B),
                cal_wk_inactive_fg: hex(0x64748B),
                cal_wk_active_bg: hex(0x6366F1),
                cal_wk_active_fg: Color32::WHITE,
                cal_today: Color32::from_rgba_unmultiplied(99, 102, 241, 56),
                cal_focus: Color32::from_rgba_unmultiplied(99, 102, 241, 71),
                cal_hover: Color32::from_rgba_unmultiplied(99, 102, 241, 51),
                cal_dot_1: hex(0xA5B4FC),
                cal_dot_2: hex(0x818CF8),
                cal_dot_3: hex(0x6366F1),
                cal_cell_out_bg: Color32::from_rgba_unmultiplied(15, 23, 42, 153),
                cal_cell_out_text: hex(0x475569),
                cal_cell_in_text: hex(0xE2E8F0),
                cal_day_today_bg: hex(0x6366F1),
                cal_day_today_fg: Color32::WHITE,
                day_header_today_start: hex(0x6366F1),
                day_header_today_end: hex(0x7C3AED),
                achievement: hex(0xFBBF24),
            },
            ThemeId::Warm => Palette {
                background: hex(0xFFFBEB),
                surface: hex(0xFFFFFF),
                primary: hex(0xD97706),
                accent: hex(0x059669),
                text: hex(0x292524),
                text_muted: hex(0x78716C),
                border: hex(0xFDE68A),
                toolbar_bg: hex(0xD97706),
                toolbar_fg: Color32::WHITE,
                toolbar_accent: hex(0xB45309),
                cal_week_row: Color32::from_rgba_unmultiplied(217, 119, 6, 18),
                cal_wk_inactive_bg: hex(0xE7E5E4),
                cal_wk_inactive_fg: hex(0x78716C),
                cal_wk_active_bg: hex(0xEA580C),
                cal_wk_active_fg: Color32::WHITE,
                cal_today: Color32::from_rgba_unmultiplied(217, 119, 6, 35),
                cal_focus: Color32::from_rgba_unmultiplied(217, 119, 6, 55),
                cal_hover: Color32::from_rgba_unmultiplied(217, 119, 6, 25),
                cal_dot_1: hex(0xFCD34D),
                cal_dot_2: hex(0xF59E0B),
                cal_dot_3: hex(0xD97706),
                cal_cell_out_bg: Color32::from_rgba_unmultiplied(255, 251, 235, 128),
                cal_cell_out_text: hex(0xD6D3D1),
                cal_cell_in_text: hex(0x44403C),
                cal_day_today_bg: hex(0xD97706),
                cal_day_today_fg: Color32::WHITE,
                day_header_today_start: hex(0xD97706),
                day_header_today_end: hex(0xEA580C),
                achievement: hex(0xF59E0B),
            },
            ThemeId::Forest => Palette {
                background: hex(0xECFDF5),
                surface: hex(0xFFFFFF),
                primary: hex(0x059669),
                accent: hex(0x2563EB),
                text: hex(0x064E3B),
                text_muted: hex(0x047857),
                border: hex(0xA7F3D0),
                toolbar_bg: hex(0x059669),
                toolbar_fg: Color32::WHITE,
                toolbar_accent: hex(0x047857),
                cal_week_row: Color32::from_rgba_unmultiplied(5, 150, 105, 18),
                cal_wk_inactive_bg: hex(0xA7F3D0),
                cal_wk_inactive_fg: hex(0x047857),
                cal_wk_active_bg: hex(0x059669),
                cal_wk_active_fg: Color32::WHITE,
                cal_today: Color32::from_rgba_unmultiplied(5, 150, 105, 35),
                cal_focus: Color32::from_rgba_unmultiplied(5, 150, 105, 55),
                cal_hover: Color32::from_rgba_unmultiplied(5, 150, 105, 25),
                cal_dot_1: hex(0x6EE7B7),
                cal_dot_2: hex(0x34D399),
                cal_dot_3: hex(0x059669),
                cal_cell_out_bg: Color32::from_rgba_unmultiplied(236, 253, 245, 128),
                cal_cell_out_text: hex(0xA7F3D0),
                cal_cell_in_text: hex(0x064E3B),
                cal_day_today_bg: hex(0x059669),
                cal_day_today_fg: Color32::WHITE,
                day_header_today_start: hex(0x059669),
                day_header_today_end: hex(0x047857),
                achievement: hex(0xF59E0B),
            },
            ThemeId::Ocean => Palette {
                background: hex(0xF0F9FF),
                surface: hex(0xFFFFFF),
                primary: hex(0x0284C7),
                accent: hex(0x059669),
                text: hex(0x0C4A6E),
                text_muted: hex(0x0369A1),
                border: hex(0xBAE6FD),
                toolbar_bg: hex(0x0284C7),
                toolbar_fg: Color32::WHITE,
                toolbar_accent: hex(0x0369A1),
                cal_week_row: Color32::from_rgba_unmultiplied(2, 132, 199, 18),
                cal_wk_inactive_bg: hex(0xBAE6FD),
                cal_wk_inactive_fg: hex(0x0369A1),
                cal_wk_active_bg: hex(0x0284C7),
                cal_wk_active_fg: Color32::WHITE,
                cal_today: Color32::from_rgba_unmultiplied(2, 132, 199, 35),
                cal_focus: Color32::from_rgba_unmultiplied(2, 132, 199, 55),
                cal_hover: Color32::from_rgba_unmultiplied(2, 132, 199, 25),
                cal_dot_1: hex(0x7DD3FC),
                cal_dot_2: hex(0x38BDF8),
                cal_dot_3: hex(0x0284C7),
                cal_cell_out_bg: Color32::from_rgba_unmultiplied(240, 249, 255, 128),
                cal_cell_out_text: hex(0xBAE6FD),
                cal_cell_in_text: hex(0x0C4A6E),
                cal_day_today_bg: hex(0x0284C7),
                cal_day_today_fg: Color32::WHITE,
                day_header_today_start: hex(0x0284C7),
                day_header_today_end: hex(0x0369A1),
                achievement: hex(0xF59E0B),
            },
            ThemeId::Rose => Palette {
                background: hex(0xFFF1F2),
                surface: hex(0xFFFFFF),
                primary: hex(0xE11D48),
                accent: hex(0x2563EB),
                text: hex(0x881337),
                text_muted: hex(0xBE123C),
                border: hex(0xFECDD3),
                toolbar_bg: hex(0xE11D48),
                toolbar_fg: Color32::WHITE,
                toolbar_accent: hex(0xBE123C),
                cal_week_row: Color32::from_rgba_unmultiplied(225, 29, 72, 18),
                cal_wk_inactive_bg: hex(0xFECDD3),
                cal_wk_inactive_fg: hex(0x9F1239),
                cal_wk_active_bg: hex(0xE11D48),
                cal_wk_active_fg: Color32::WHITE,
                cal_today: Color32::from_rgba_unmultiplied(225, 29, 72, 35),
                cal_focus: Color32::from_rgba_unmultiplied(225, 29, 72, 55),
                cal_hover: Color32::from_rgba_unmultiplied(225, 29, 72, 25),
                cal_dot_1: hex(0xFDA4AF),
                cal_dot_2: hex(0xFB7185),
                cal_dot_3: hex(0xE11D48),
                cal_cell_out_bg: Color32::from_rgba_unmultiplied(255, 241, 242, 128),
                cal_cell_out_text: hex(0xFECDD3),
                cal_cell_in_text: hex(0x881337),
                cal_day_today_bg: hex(0xE11D48),
                cal_day_today_fg: Color32::WHITE,
                day_header_today_start: hex(0xE11D48),
                day_header_today_end: hex(0xBE123C),
                achievement: hex(0xF59E0B),
            },
        }
    }
}

fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb(
        ((rgb >> 16) & 0xFF) as u8,
        ((rgb >> 8) & 0xFF) as u8,
        (rgb & 0xFF) as u8,
    )
}

/// Maps the palette onto egui `Visuals` so controls inherit contrast-safe tokens.
pub fn apply_visuals(ctx: &egui::Context, theme: ThemeId) {
    let p = Palette::for_theme(theme);
    let mut visuals = if theme == ThemeId::Dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    visuals.panel_fill = p.background;
    visuals.window_fill = p.surface;
    visuals.extreme_bg_color = p.background;
    visuals.faint_bg_color = p.cal_week_row;
    visuals.widgets.noninteractive.bg_fill = p.surface;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.inactive.bg_fill = p.surface;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, p.border);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.inactive.expansion = 0.0;
    visuals.widgets.hovered.bg_fill = p.cal_hover;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, p.primary);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.active.bg_fill = p.primary;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, p.primary);
    visuals.widgets.open.bg_fill = p.cal_focus;
    visuals.selection.bg_fill = p.cal_focus;
    visuals.selection.stroke = Stroke::new(1.0, p.primary);
    visuals.hyperlink_color = p.primary;
    visuals.warn_fg_color = p.achievement;
    ctx.set_visuals(visuals);
}