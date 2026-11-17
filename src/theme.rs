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
            ThemeId::Default => "Slate surfaces, blue primary (design system)",
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
    pub toolbar_top: Color32,
    pub toolbar_bottom: Color32,
    pub cal_week_row: Color32,
    pub cal_today: Color32,
    pub cal_focus: Color32,
    pub cal_hover: Color32,
    pub achievement: Color32,
}

impl Palette {
    pub fn for_theme(theme: ThemeId) -> Self {
        match theme {
            ThemeId::Default => Palette {
                background: hex(0xF8FAFC),
                surface: hex(0xFFFFFF),
                primary: hex(0x2563EB),
                accent: hex(0x059669),
                text: hex(0x0F172A),
                text_muted: hex(0x64748B),
                border: hex(0xE4ECFC),
                toolbar_top: hex(0xFFFFFF),
                toolbar_bottom: hex(0xFFFFFF),
                cal_week_row: Color32::from_rgba_unmultiplied(37, 99, 235, 18),
                cal_today: Color32::from_rgba_unmultiplied(37, 99, 235, 35),
                cal_focus: Color32::from_rgba_unmultiplied(37, 99, 235, 55),
                cal_hover: Color32::from_rgba_unmultiplied(37, 99, 235, 25),
                achievement: hex(0xF59E0B),
            },
            ThemeId::Dark => Palette {
                background: hex(0x0F172A),
                surface: hex(0x1E293B),
                primary: hex(0x818CF8),
                accent: hex(0x34D399),
                text: hex(0xF1F5F9),
                text_muted: hex(0x94A3B8),
                border: hex(0x334155),
                toolbar_top: hex(0x1E293B),
                toolbar_bottom: hex(0x1E293B),
                cal_week_row: Color32::from_rgba_unmultiplied(129, 140, 248, 22),
                cal_today: Color32::from_rgba_unmultiplied(129, 140, 248, 40),
                cal_focus: Color32::from_rgba_unmultiplied(129, 140, 248, 60),
                cal_hover: Color32::from_rgba_unmultiplied(129, 140, 248, 30),
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
                toolbar_top: hex(0xFFFFFF),
                toolbar_bottom: hex(0xFFFFFF),
                cal_week_row: Color32::from_rgba_unmultiplied(217, 119, 6, 18),
                cal_today: Color32::from_rgba_unmultiplied(217, 119, 6, 35),
                cal_focus: Color32::from_rgba_unmultiplied(217, 119, 6, 55),
                cal_hover: Color32::from_rgba_unmultiplied(217, 119, 6, 25),
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
                toolbar_top: hex(0xFFFFFF),
                toolbar_bottom: hex(0xFFFFFF),
                cal_week_row: Color32::from_rgba_unmultiplied(5, 150, 105, 18),
                cal_today: Color32::from_rgba_unmultiplied(5, 150, 105, 35),
                cal_focus: Color32::from_rgba_unmultiplied(5, 150, 105, 55),
                cal_hover: Color32::from_rgba_unmultiplied(5, 150, 105, 25),
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
                toolbar_top: hex(0xFFFFFF),
                toolbar_bottom: hex(0xFFFFFF),
                cal_week_row: Color32::from_rgba_unmultiplied(2, 132, 199, 18),
                cal_today: Color32::from_rgba_unmultiplied(2, 132, 199, 35),
                cal_focus: Color32::from_rgba_unmultiplied(2, 132, 199, 55),
                cal_hover: Color32::from_rgba_unmultiplied(2, 132, 199, 25),
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
                toolbar_top: hex(0xFFFFFF),
                toolbar_bottom: hex(0xFFFFFF),
                cal_week_row: Color32::from_rgba_unmultiplied(225, 29, 72, 18),
                cal_today: Color32::from_rgba_unmultiplied(225, 29, 72, 35),
                cal_focus: Color32::from_rgba_unmultiplied(225, 29, 72, 55),
                cal_hover: Color32::from_rgba_unmultiplied(225, 29, 72, 25),
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