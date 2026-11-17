use std::sync::mpsc;
use std::time::{Duration as StdDuration, Instant};

use chrono::{Datelike, Duration, Utc};
use egui::{
    self, Align, Align2, Button, Color32, CornerRadius, FontId, Layout, Order, RichText,
    ScrollArea, Sense, Stroke, StrokeKind, Vec2,
};
use uuid::Uuid;

use crate::categories::{category_label, CATEGORIES};
use crate::dates::{
    end_of_month, end_of_week, format_date_key, format_day_full, format_local_time,
    format_month_year, get_month_grid, is_in_current_week, is_same_month, is_today,
    iso_weekday_labels, month_offset_for_date, parse_date_key, start_of_month, start_of_week,
    today_local,
};
use crate::markdown::generate_markdown_for_range;
use crate::models::{
    APP_CONTACT_EMAIL, APP_LICENSE, APP_NAME, APP_REPO_URL, APP_RELEASES_URL, APP_TAGLINE,
    APP_VERSION, Prefs, WorkItem,
};
use crate::storage::{load_items, load_prefs, save_items, save_prefs, storage_dir_display};
use crate::theme::{apply_visuals, Palette, ThemeId};
use crate::ui::chrome::{
    achievement_indicator, calendar_outer_frame, card_frame, modal_content_frame,
    paint_modal_backdrop, primary_button, secondary_button, splash_gradient,
    toolbar_panel_frame, toolbar_text_button, CAL_DAY_CELL, CAL_WEEK_COL_WIDTH, SPACE_MD, SPACE_SM,
};
use crate::updates::{self, UpdateCheckResult};

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    Storage,
    Updates,
    About,
    Contact,
}

enum FormMode {
    Add,
    Edit,
}

#[derive(Clone)]
struct UpdateBanner {
    title: String,
    url: String,
}

#[derive(Clone)]
enum UpdateNotice {
    Error(String),
    UpToDate(String),
    Newer { text: String, url: String },
    NoReleases(String),
}

enum UpdatePollKind {
    Startup,
    Settings,
}

pub struct WeeklyTrackerApp {
    items: Vec<WorkItem>,
    month_offset: i32,
    pinned_date: Option<chrono::NaiveDate>,
    hover_date: Option<chrono::NaiveDate>,
    theme: ThemeId,
    prefs: Prefs,
    show_export: bool,
    show_settings: bool,
    settings_tab: SettingsTab,

    form_mode: Option<FormMode>,
    form_item_id: Option<String>,
    form_date: Option<chrono::NaiveDate>,
    form_title: String,
    form_description: String,
    form_category: String,
    form_achievement: bool,

    export_start: String,
    export_end: String,

    copy_source_id: Option<String>,
    copy_target_date: String,

    splash_active: bool,
    splash_started: Instant,

    update_rx: Option<mpsc::Receiver<UpdateCheckResult>>,
    update_poll_kind: Option<UpdatePollKind>,
    update_busy: bool,
    update_banner: Option<UpdateBanner>,
    settings_update_notice: Option<UpdateNotice>,
}

impl WeeklyTrackerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let prefs = load_prefs();
        let theme = ThemeId::from_pref(&prefs.theme);
        apply_visuals(&cc.egui_ctx, theme);

        let items = load_items().unwrap_or_default();
        let today = today_local();
        let export_start = format_date_key(start_of_month(today));
        let export_end = format_date_key(end_of_month(today));

        Self {
            items,
            month_offset: 0,
            pinned_date: None,
            hover_date: None,
            theme,
            prefs,
            show_export: false,
            show_settings: false,
            settings_tab: SettingsTab::Storage,
            form_mode: None,
            form_item_id: None,
            form_date: None,
            form_title: String::new(),
            form_description: String::new(),
            form_category: "development".into(),
            form_achievement: false,
            export_start,
            export_end,
            copy_source_id: None,
            copy_target_date: format_date_key(today + Duration::days(1)),
            splash_active: true,
            splash_started: Instant::now(),
            update_rx: None,
            update_poll_kind: None,
            update_busy: false,
            update_banner: None,
            settings_update_notice: None,
        }
    }

    fn palette(&self) -> Palette {
        Palette::for_theme(self.theme)
    }

    fn display_date(&self) -> chrono::NaiveDate {
        self.pinned_date
            .or(self.hover_date)
            .unwrap_or_else(today_local)
    }

    fn persist_items(&mut self) {
        if let Err(e) = save_items(&self.items) {
            eprintln!("save items: {e}");
        }
    }

    fn persist_prefs(&self) {
        if let Err(e) = save_prefs(&self.prefs) {
            eprintln!("save prefs: {e}");
        }
    }

    fn set_theme(&mut self, ctx: &egui::Context, theme: ThemeId) {
        self.theme = theme;
        self.prefs.theme = theme.as_pref().into();
        apply_visuals(ctx, theme);
        self.persist_prefs();
    }

    fn open_add_form(&mut self, date: chrono::NaiveDate) {
        self.form_mode = Some(FormMode::Add);
        self.form_item_id = None;
        self.form_date = Some(date);
        self.form_title.clear();
        self.form_description.clear();
        self.form_category = "development".into();
        self.form_achievement = false;
    }

    fn open_edit_form(&mut self, item: &WorkItem) {
        self.form_mode = Some(FormMode::Edit);
        self.form_item_id = Some(item.id.clone());
        self.form_date = parse_date_key(&item.date);
        self.form_title = item.title.clone();
        self.form_description = item.description.clone();
        self.form_category = item.category.clone();
        self.form_achievement = item.is_achievement;
    }

    fn cancel_form(&mut self) {
        self.form_mode = None;
        self.form_item_id = None;
        self.form_date = None;
    }

    fn submit_form(&mut self) {
        let title = self.form_title.trim();
        if title.is_empty() {
            return;
        }
        let now = Utc::now();
        match self.form_mode {
            Some(FormMode::Add) => {
                let date = self.form_date.unwrap_or_else(today_local);
                self.items.push(WorkItem {
                    id: Uuid::new_v4().to_string(),
                    date: format_date_key(date),
                    title: title.into(),
                    description: self.form_description.trim().into(),
                    category: self.form_category.clone(),
                    is_achievement: self.form_achievement,
                    created_at: now,
                    updated_at: now,
                });
            }
            Some(FormMode::Edit) => {
                if let Some(id) = &self.form_item_id {
                    if let Some(item) = self.items.iter_mut().find(|i| &i.id == id) {
                        item.title = title.into();
                        item.description = self.form_description.trim().into();
                        item.category = self.form_category.clone();
                        item.is_achievement = self.form_achievement;
                        item.updated_at = now;
                    }
                }
            }
            None => {}
        }
        self.cancel_form();
        self.persist_items();
    }

    fn delete_item(&mut self, id: &str) {
        self.items.retain(|i| i.id != id);
        self.persist_items();
    }

    fn copy_item_to_date(&mut self, source_id: &str, target_key: &str) {
        let Some(source) = self.items.iter().find(|i| i.id == source_id).cloned() else {
            return;
        };
        let now = Utc::now();
        self.items.push(WorkItem {
            id: Uuid::new_v4().to_string(),
            date: target_key.into(),
            title: source.title,
            description: source.description,
            category: source.category,
            is_achievement: source.is_achievement,
            created_at: now,
            updated_at: now,
        });
        if let Some(d) = parse_date_key(target_key) {
            self.month_offset = month_offset_for_date(d);
            self.pinned_date = Some(d);
        }
        self.copy_source_id = None;
        self.persist_items();
    }

    fn item_counts(&self) -> std::collections::HashMap<String, usize> {
        let mut m = std::collections::HashMap::new();
        for item in &self.items {
            *m.entry(item.date.clone()).or_default() += 1;
        }
        m
    }

    fn achievement_dates(&self) -> std::collections::HashSet<String> {
        self.items
            .iter()
            .filter(|i| i.is_achievement)
            .map(|i| i.date.clone())
            .collect()
    }

    fn open_github(url: &str) {
        if url.starts_with("https://github.com/") {
            let _ = open::that(url);
        }
    }

    fn spawn_update_check(&mut self, ctx: &egui::Context, kind: UpdatePollKind) {
        if self.update_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.update_rx = Some(rx);
        self.update_poll_kind = Some(kind);
        self.update_busy = true;
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = updates::check_for_updates();
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }

    fn notice_from_result(result: &UpdateCheckResult) -> UpdateNotice {
        if !result.ok {
            return UpdateNotice::Error(
                result
                    .error_message
                    .clone()
                    .unwrap_or_else(|| "Update check failed.".into()),
            );
        }
        if result.is_newer {
            let ver = result.remote_version.as_deref().unwrap_or("unknown");
            let text = format!(
                "A newer release is available: {} ({ver}).",
                result.name.as_deref().unwrap_or(ver)
            );
            let url = result
                .html_url
                .clone()
                .unwrap_or_else(|| APP_RELEASES_URL.to_string());
            return UpdateNotice::Newer { text, url };
        }
        if let Some(remote) = &result.remote_version {
            return UpdateNotice::UpToDate(format!(
                "You are up to date. Latest GitHub release is {remote} (this app is {}).",
                result.current_version
            ));
        }
        UpdateNotice::NoReleases(
            "No published releases were found on GitHub. You can still browse the releases page."
                .into(),
        )
    }

    fn apply_update_result(&mut self, result: UpdateCheckResult) {
        let kind = self.update_poll_kind.take();
        self.update_rx = None;
        self.update_busy = false;

        if matches!(kind, Some(UpdatePollKind::Settings)) {
            self.settings_update_notice = Some(Self::notice_from_result(&result));
        }

        if result.ok && result.is_newer {
            if let Some(url) = result.html_url.clone() {
                let title = result
                    .name
                    .clone()
                    .or(result.remote_version.clone())
                    .unwrap_or_else(|| "New release".into());
                self.update_banner = Some(UpdateBanner { title, url });
            }
        }
    }

    fn poll_update_check(&mut self) {
        let Some(rx) = &self.update_rx else {
            return;
        };
        if let Ok(result) = rx.try_recv() {
            self.apply_update_result(result);
        }
    }

    fn dismiss_splash(&mut self, ctx: &egui::Context) {
        if !self.splash_active {
            return;
        }
        self.splash_active = false;
        if self.prefs.check_updates_on_startup {
            self.spawn_update_check(ctx, UpdatePollKind::Startup);
        }
    }

    fn finish_splash_if_due(&mut self, ctx: &egui::Context) {
        if !self.splash_active {
            return;
        }
        let elapsed = self.splash_started.elapsed();
        if elapsed >= StdDuration::from_secs(5) {
            self.dismiss_splash(ctx);
            return;
        }
        ctx.request_repaint_after(StdDuration::from_millis(80));
    }

    fn splash_overlay(&mut self, ctx: &egui::Context, palette: Palette) {
        let screen = ctx.input(|i| i.screen_rect);
        let fade_start = StdDuration::from_millis(4200);
        let elapsed = self.splash_started.elapsed();
        let alpha = if elapsed >= fade_start {
            let t = (elapsed - fade_start).as_secs_f32() / 0.8;
            ((1.0 - t).clamp(0.0, 1.0) * 255.0) as u8
        } else {
            255
        };

        egui::Area::new(egui::Id::new("splash"))
            .order(Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let size = screen.size();
                let (rect, response) = ui.allocate_exact_size(size, Sense::click());
                if response.clicked() {
                    self.dismiss_splash(ctx);
                }
                let (top, bottom) = splash_gradient(palette);
                let mut mesh = egui::Mesh::default();
                let vi = mesh.vertices.len() as u32;
                mesh.colored_vertex(rect.left_top(), Color32::from_rgba_unmultiplied(top.r(), top.g(), top.b(), alpha));
                mesh.colored_vertex(rect.right_top(), Color32::from_rgba_unmultiplied(bottom.r(), bottom.g(), bottom.b(), alpha));
                mesh.colored_vertex(rect.right_bottom(), Color32::from_rgba_unmultiplied(bottom.r(), bottom.g(), bottom.b(), alpha));
                mesh.colored_vertex(rect.left_bottom(), Color32::from_rgba_unmultiplied(top.r(), top.g(), top.b(), alpha));
                mesh.add_triangle(vi, vi + 1, vi + 2);
                mesh.add_triangle(vi, vi + 2, vi + 3);
                ui.painter().add(egui::Shape::mesh(mesh));

                let icon_center = rect.center() - Vec2::new(0.0, 48.0);
                ui.painter().circle_stroke(
                    icon_center,
                    28.0,
                    Stroke::new(2.0, Color32::from_rgba_unmultiplied(255, 255, 255, alpha)),
                );
                ui.painter().line_segment(
                    [
                        icon_center,
                        icon_center + Vec2::new(0.0, -14.0),
                    ],
                    Stroke::new(2.0, Color32::from_rgba_unmultiplied(255, 255, 255, alpha)),
                );
                ui.painter().line_segment(
                    [
                        icon_center,
                        icon_center + Vec2::new(12.0, 8.0),
                    ],
                    Stroke::new(2.0, Color32::from_rgba_unmultiplied(255, 255, 255, alpha)),
                );
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    APP_NAME,
                    FontId::proportional(28.0),
                    Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
                );
                ui.painter().text(
                    rect.center() + Vec2::new(0.0, 36.0),
                    Align2::CENTER_CENTER,
                    APP_TAGLINE,
                    FontId::proportional(16.0),
                    Color32::from_rgba_unmultiplied(230, 240, 255, alpha),
                );
                ui.painter().text(
                    rect.center() + Vec2::new(0.0, 120.0),
                    Align2::CENTER_CENTER,
                    format!("v{APP_VERSION}"),
                    FontId::proportional(12.0),
                    Color32::from_rgba_unmultiplied(200, 220, 255, alpha),
                );
            });
    }
}

impl eframe::App for WeeklyTrackerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let palette = self.palette();
        self.poll_update_check();
        self.finish_splash_if_due(ctx);

        if self.splash_active {
            self.splash_overlay(ctx, palette);
            self.finish_splash_if_due(ctx);
            if self.splash_active {
                return;
            }
        }

        egui::TopBottomPanel::top("toolbar")
            .frame(toolbar_panel_frame(palette))
            .show(ctx, |ui| {
                self.toolbar(ui, palette);
            });

        if let Some(banner) = self.update_banner.clone() {
            egui::TopBottomPanel::top("update_banner")
                .frame(egui::Frame::NONE.fill(palette.cal_week_row))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("New release: {}", banner.title)).strong());
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("Dismiss").clicked() {
                                self.update_banner = None;
                            }
                            if ui.button("Open in browser").clicked() {
                                Self::open_github(&banner.url);
                            }
                        });
                    });
                });
        }

        egui::TopBottomPanel::bottom("footer").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "© {} Biplab Sarkar · {}",
                        today_local().year(),
                        APP_LICENSE
                    ))
                    .small()
                    .color(palette.text_muted),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{APP_NAME} v{APP_VERSION}"))
                            .small()
                            .color(palette.text_muted),
                    );
                });
            });
        });

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(palette.background))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.x = SPACE_MD;
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_max_width(360.0);
                        self.month_calendar(ui, palette);
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.set_min_width(320.0);
                        if self.form_mode.is_some() {
                            self.item_form(ui, palette);
                        } else {
                            self.day_detail(ui, palette);
                        }
                    });
                });
            });

        if self.show_export {
            self.export_modal(ctx, palette);
        }
        if self.show_settings {
            self.settings_modal(ctx, palette);
        }
        if self.copy_source_id.is_some() {
            self.copy_modal(ctx, palette);
        }
    }
}

impl WeeklyTrackerApp {
    fn toolbar(&mut self, ui: &mut egui::Ui, palette: Palette) {
        let grid = get_month_grid(self.month_offset);
        ui.spacing_mut().item_spacing.x = SPACE_SM;
        ui.horizontal(|ui| {
            ui.label(RichText::new(APP_NAME).strong().color(palette.text));
            if toolbar_text_button(ui, palette, "◀").clicked() {
                self.month_offset -= 1;
            }
            if toolbar_text_button(ui, palette, "Today").clicked() {
                self.month_offset = 0;
                self.pinned_date = None;
            }
            if toolbar_text_button(ui, palette, "▶").clicked() {
                self.month_offset += 1;
            }
            ui.add_space(SPACE_SM);
            ui.separator();
            ui.add_space(SPACE_SM);
            ui.label(
                RichText::new(format_month_year(grid.month))
                    .size(18.0)
                    .strong()
                    .color(palette.text),
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = SPACE_SM;
                if secondary_button(ui, palette, "Export").clicked() {
                    self.show_export = true;
                }
                if secondary_button(ui, palette, "Settings").clicked() {
                    self.show_settings = true;
                }

                ui.menu_button(
                    RichText::new("Theme").color(palette.text),
                    |ui| {
                    for t in ThemeId::ALL {
                        if ui
                            .selectable_label(self.theme == t, format!("{} — {}", t.label(), t.description()))
                            .clicked()
                        {
                            self.set_theme(ui.ctx(), t);
                            ui.close_menu();
                        }
                    }
                });
            });
        });
    }

    fn month_calendar(&mut self, ui: &mut egui::Ui, palette: Palette) {
        let grid = get_month_grid(self.month_offset);
        let counts = self.item_counts();
        let achievements = self.achievement_dates();
        let radius = CornerRadius::same(6);
        let cell_gap = SPACE_SM;

        ui.label(
            RichText::new("Calendar")
                .small()
                .strong()
                .color(palette.text_muted),
        );
        ui.add_space(SPACE_SM);

        egui::Grid::new("cal_header")
            .num_columns(8)
            .spacing([cell_gap, cell_gap])
            .show(ui, |ui| {
                let labels: Vec<&str> = std::iter::once("Wk")
                    .chain(iso_weekday_labels())
                    .collect();
                for (i, label) in labels.iter().enumerate() {
                    let width = if i == 0 {
                        CAL_WEEK_COL_WIDTH
                    } else {
                        CAL_DAY_CELL.x
                    };
                    ui.allocate_ui_with_layout(
                        Vec2::new(width, 18.0),
                        Layout::top_down(Align::Center),
                        |ui| {
                            ui.label(RichText::new(*label).small().color(palette.text_muted));
                        },
                    );
                }
                ui.end_row();
            });

        ScrollArea::vertical().show(ui, |ui| {
            calendar_outer_frame(palette).show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(cell_gap, cell_gap);
                for week in &grid.weeks {
                    let current_week = week.days.iter().any(|&d| is_in_current_week(d));
                    ui.horizontal(|ui| {
                        let wk_size = Vec2::new(CAL_WEEK_COL_WIDTH, CAL_DAY_CELL.y);
                        let (wk_rect, _) = ui.allocate_exact_size(wk_size, Sense::hover());
                            let wk_bg = if current_week {
                                palette.cal_focus
                            } else {
                                palette.background
                            };
                            ui.painter()
                                .rect_filled(wk_rect, radius, wk_bg);
                            ui.painter().text(
                                wk_rect.center(),
                                Align2::CENTER_CENTER,
                                format!("{}", week.week_number),
                                FontId::proportional(11.0),
                                if current_week {
                                    palette.primary
                                } else {
                                    palette.text_muted
                                },
                            );

                            for &date in &week.days {
                                let key = format_date_key(date);
                                let in_month = is_same_month(date, grid.month);
                                let today = is_today(date);
                                let focused = self.pinned_date == Some(date);
                                let hovered = self.hover_date == Some(date);
                                let count = counts.get(&key).copied().unwrap_or(0);
                                let has_ach = achievements.contains(&key);

                                let (rect, response) =
                                    ui.allocate_exact_size(CAL_DAY_CELL, Sense::click());

                                let mut bg = if in_month {
                                    palette.surface
                                } else {
                                    palette.background
                                };
                                if current_week && !focused && !hovered && !today {
                                    bg = palette.cal_week_row;
                                }
                                if focused {
                                    bg = palette.cal_focus;
                                } else if hovered {
                                    bg = palette.cal_hover;
                                } else if today && !focused {
                                    bg = palette.cal_today;
                                }

                                ui.painter().rect_filled(rect, radius, bg);
                                if focused {
                                    ui.painter().rect_stroke(
                                        rect,
                                        radius,
                                        Stroke::new(2.0, palette.primary),
                                        StrokeKind::Inside,
                                    );
                                }

                                let day_color = if in_month {
                                    palette.text
                                } else {
                                    palette.text_muted
                                };
                                let mut day_center = rect.center();
                                if count > 0 || has_ach {
                                    day_center.y -= 4.0;
                                }
                                ui.painter().text(
                                    day_center,
                                    Align2::CENTER_CENTER,
                                    format!("{}", date.day()),
                                    FontId::proportional(13.0),
                                    if today {
                                        palette.accent
                                    } else {
                                        day_color
                                    },
                                );

                                if count > 0 || has_ach {
                                    let dot_color = if count >= 3 {
                                        palette.primary
                                    } else if count >= 2 {
                                        palette.accent
                                    } else {
                                        palette.text_muted
                                    };
                                    ui.painter().circle_filled(
                                        rect.center() + Vec2::new(-4.0, 14.0),
                                        3.0,
                                        dot_color,
                                    );
                                    if has_ach {
                                        ui.painter().circle_filled(
                                            rect.center() + Vec2::new(6.0, 14.0),
                                            2.5,
                                            palette.achievement,
                                        );
                                    }
                                    if count > 1 {
                                        ui.painter().text(
                                            rect.center() + Vec2::new(14.0, 14.0),
                                            Align2::CENTER_CENTER,
                                            format!("{count}"),
                                            FontId::proportional(8.0),
                                            palette.text_muted,
                                        );
                                    }
                                }

                                if response.hovered() {
                                    self.hover_date = Some(date);
                                }
                                if response.clicked() {
                                    self.pinned_date = if self.pinned_date == Some(date) {
                                        None
                                    } else {
                                        Some(date)
                                    };
                                }
                            }
                        });
                    }
                });
        });
    }

    fn day_detail(&mut self, ui: &mut egui::Ui, palette: Palette) {
        let date = self.display_date();
        let day_items: Vec<WorkItem> = self
            .items
            .iter()
            .filter(|i| i.date == format_date_key(date))
            .cloned()
            .collect();
        let today = is_today(date);
        let pinned = self.pinned_date == Some(date);

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.heading(format_day_full(date));
                ui.label(
                    RichText::new(format!(
                        "{} {}",
                        day_items.len(),
                        if day_items.len() == 1 {
                            "item"
                        } else {
                            "items"
                        }
                    ))
                    .small()
                    .color(palette.text_muted),
                );
                if today {
                    ui.label(RichText::new("· Today").small().color(palette.accent));
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .button(if pinned { "Unpin" } else { "Pin day" })
                    .clicked()
                {
                    self.pinned_date = if pinned { None } else { Some(date) };
                }
                if primary_button(ui, palette, "+ Add").clicked() {
                    self.open_add_form(date);
                }
            });
        });
        ui.add_space(SPACE_SM);
        ui.separator();

        if day_items.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(24.0);
                ui.label(RichText::new("No items for this day").color(palette.text_muted));
                if primary_button(ui, palette, "+ Add Work Item").clicked() {
                    self.open_add_form(date);
                }
            });
        } else {
            ScrollArea::vertical().show(ui, |ui| {
                for item in &day_items {
                    let id = item.id.clone();
                    card_frame(palette).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if item.is_achievement {
                                achievement_indicator(ui, palette);
                            }
                            ui.vertical(|ui| {
                                ui.strong(&item.title);
                                if !item.description.is_empty() {
                                    ui.label(
                                        RichText::new(&item.description).color(palette.text_muted),
                                    );
                                }
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(category_label(&item.category))
                                            .small()
                                            .color(palette.primary),
                                    );
                                    ui.label(
                                        RichText::new(format_local_time(item.updated_at))
                                            .small()
                                            .color(palette.text_muted),
                                    );
                                });
                            });
                            ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                                if ui.small_button("Copy").clicked() {
                                    self.copy_source_id = Some(id.clone());
                                    if let Some(d) = parse_date_key(&item.date) {
                                        self.copy_target_date =
                                            format_date_key(d + Duration::days(1));
                                    }
                                }
                                if ui.small_button("Delete").clicked() {
                                    self.delete_item(&id);
                                }
                                if ui.small_button("Edit").clicked() {
                                    if let Some(full) = self.items.iter().find(|i| i.id == id).cloned() {
                                        self.open_edit_form(&full);
                                    }
                                }
                            });
                        });
                    });
                    ui.add_space(SPACE_SM);
                }
                if primary_button(ui, palette, "Add Work Item").clicked() {
                    self.open_add_form(date);
                }
            });
        }
    }

    fn item_form(&mut self, ui: &mut egui::Ui, palette: Palette) {
        let heading = match self.form_mode {
            Some(FormMode::Add) => "Add Work Item",
            Some(FormMode::Edit) => "Edit Work Item",
            None => "Work Item",
        };
        ui.heading(heading);
        if let Some(d) = self.form_date {
            ui.label(
                RichText::new(format_day_full(d))
                    .small()
                    .color(palette.text_muted),
            );
        }

        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("category")
                .selected_text(category_label(&self.form_category))
                .show_ui(ui, |ui| {
                    for cat in CATEGORIES {
                        ui.selectable_value(&mut self.form_category, cat.id.to_string(), cat.label);
                    }
                });
            ui.checkbox(&mut self.form_achievement, "Mark as Achievement");
        });

        ui.label("Title");
        ui.text_edit_singleline(&mut self.form_title);
        ui.label("Details (optional)");
        ui.text_edit_multiline(&mut self.form_description);

        ui.horizontal(|ui| {
            if matches!(self.form_mode, Some(FormMode::Edit)) {
                if ui.button("Copy to another day…").clicked() {
                    if let Some(id) = &self.form_item_id {
                        self.copy_source_id = Some(id.clone());
                    }
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let can_save = !self.form_title.trim().is_empty();
                if ui
                    .add_enabled(
                        can_save,
                        Button::new("Save")
                            .fill(palette.primary)
                            .stroke(Stroke::new(1.0, palette.primary)),
                    )
                    .clicked()
                {
                    self.submit_form();
                }
                if secondary_button(ui, palette, "Cancel").clicked() {
                    self.cancel_form();
                }
            });
        });
    }

    fn export_modal(&mut self, ctx: &egui::Context, palette: Palette) {
        paint_modal_backdrop(ctx, palette);
        egui::Window::new("Export Report")
            .collapsible(false)
            .resizable(true)
            .default_width(520.0)
            .show(ctx, |ui| {
                modal_content_frame().show(ui, |ui| {
                    ui.label(
                        RichText::new("Export Report")
                            .size(20.0)
                            .strong()
                            .color(palette.text),
                    );
                    ui.add_space(SPACE_SM);
                    ui.label("Date range");
                ui.horizontal(|ui| {
                    ui.label("From");
                    ui.text_edit_singleline(&mut self.export_start);
                    ui.label("To");
                    ui.text_edit_singleline(&mut self.export_end);
                });

                ui.horizontal_wrapped(|ui| {
                    if ui.button("This week").clicked() {
                        let t = today_local();
                        self.export_start = format_date_key(start_of_week(t));
                        self.export_end = format_date_key(end_of_week(t));
                    }
                    if ui.button("Last 7 days").clicked() {
                        let t = today_local();
                        self.export_start = format_date_key(t - Duration::days(6));
                        self.export_end = format_date_key(t);
                    }
                    if ui.button("Last 30 days").clicked() {
                        let t = today_local();
                        self.export_start = format_date_key(t - Duration::days(29));
                        self.export_end = format_date_key(t);
                    }
                    if ui.button("This month").clicked() {
                        let t = today_local();
                        self.export_start = format_date_key(start_of_month(t));
                        self.export_end = format_date_key(end_of_month(t));
                    }
                });

                let valid = match (
                    parse_date_key(&self.export_start),
                    parse_date_key(&self.export_end),
                ) {
                    (Some(a), Some(b)) => a <= b,
                    _ => false,
                };

                if !valid {
                    ui.colored_label(Color32::RED, "Enter a valid range (start ≤ end).");
                } else if let (Some(start), Some(end)) = (
                    parse_date_key(&self.export_start),
                    parse_date_key(&self.export_end),
                ) {
                    let md = generate_markdown_for_range(&self.items, start, end);
                    ui.separator();
                    ui.label(RichText::new("Preview").small().strong());
                    ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                        ui.monospace(&md);
                    });

                    if primary_button(ui, palette, "Download .md").clicked() {
                        let filename =
                            format!("report-{}-to-{}.md", self.export_start, self.export_end);
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Time Tracker — Export Report")
                            .set_file_name(&filename)
                            .add_filter("Markdown", &["md"])
                            .save_file()
                        {
                            if std::fs::write(&path, &md).is_ok() {
                                let _ = open::that(path);
                            }
                        }
                    }
                }

                    if secondary_button(ui, palette, "Close").clicked() {
                        self.show_export = false;
                    }
                });
            });
    }

    fn settings_modal(&mut self, ctx: &egui::Context, palette: Palette) {
        paint_modal_backdrop(ctx, palette);
        egui::Window::new("Settings")
            .collapsible(false)
            .default_width(460.0)
            .show(ctx, |ui| {
                modal_content_frame().show(ui, |ui| {
                    ui.label(
                        RichText::new("Settings")
                            .size(20.0)
                            .strong()
                            .color(palette.text),
                    );
                    ui.add_space(SPACE_SM);
                ui.horizontal(|ui| {
                    for (tab, label) in [
                        (SettingsTab::Storage, "Storage"),
                        (SettingsTab::Updates, "Updates"),
                        (SettingsTab::About, "About"),
                        (SettingsTab::Contact, "Contact"),
                    ] {
                        if ui
                            .selectable_label(self.settings_tab == tab, label)
                            .clicked()
                        {
                            self.settings_tab = tab;
                        }
                    }
                });
                ui.separator();

                match self.settings_tab {
                    SettingsTab::Storage => {
                        ui.label("Your entries are stored on this device only.");
                        ui.label(format!("Data directory: {}", storage_dir_display()));
                        ui.label("(Compatible with the Electron app’s ~/.weekly-tracker/ layout.)");
                    }
                    SettingsTab::Updates => {
                        ui.label(format!("Installed version: {APP_VERSION}"));
                        ui.checkbox(
                            &mut self.prefs.check_updates_on_startup,
                            "Check for updates when the app opens",
                        );
                        if primary_button(ui, palette, "Save preferences").clicked() {
                            self.persist_prefs();
                        }
                        ui.horizontal(|ui| {
                            let label = if self.update_busy {
                                "Checking…"
                            } else {
                                "Check for updates now"
                            };
                            if ui
                                .add_enabled(!self.update_busy, egui::Button::new(label))
                                .clicked()
                            {
                                self.settings_update_notice = None;
                                self.spawn_update_check(ui.ctx(), UpdatePollKind::Settings);
                            }
                            if ui.button("View releases on GitHub").clicked() {
                                Self::open_github(APP_RELEASES_URL);
                            }
                        });
                        if let Some(notice) = &self.settings_update_notice {
                            ui.separator();
                            match notice {
                                UpdateNotice::Error(text) => {
                                    ui.colored_label(Color32::RED, text);
                                }
                                UpdateNotice::UpToDate(text) | UpdateNotice::NoReleases(text) => {
                                    ui.label(text);
                                }
                                UpdateNotice::Newer { text, url } => {
                                    ui.label(text);
                                    if ui.button("Open release page").clicked() {
                                        Self::open_github(url);
                                    }
                                }
                            }
                        } else {
                            ui.label(
                                RichText::new(
                                    "Compares this install to the latest GitHub release (network required).",
                                )
                                .small()
                                .color(palette.text_muted),
                            );
                        }
                    }
                    SettingsTab::About => {
                        ui.heading(APP_NAME);
                        ui.label(APP_TAGLINE);
                        ui.label(format!("Version {APP_VERSION}"));
                        ui.label(
                            "A desktop application for tracking daily work items, achievements, and generating reports.",
                        );
                        ui.label(APP_LICENSE);
                    }
                    SettingsTab::Contact => {
                        ui.label(format!("Email: {APP_CONTACT_EMAIL}"));
                        if ui.button("Open GitHub repository").clicked() {
                            Self::open_github(APP_REPO_URL);
                        }
                    }
                }

                    if secondary_button(ui, palette, "Close").clicked() {
                        self.show_settings = false;
                    }
                });
            });
    }

    fn copy_modal(&mut self, ctx: &egui::Context, palette: Palette) {
        let source_title = self
            .copy_source_id
            .as_ref()
            .and_then(|id| self.items.iter().find(|i| &i.id == id))
            .map(|i| i.title.clone())
            .unwrap_or_default();

        let source_id = self.copy_source_id.clone();
        paint_modal_backdrop(ctx, palette);
        egui::Window::new("Copy to another day")
            .collapsible(false)
            .show(ctx, |ui| {
                modal_content_frame().show(ui, |ui| {
                    ui.label(
                        RichText::new("Copy to another day")
                            .size(18.0)
                            .strong()
                            .color(palette.text),
                    );
                    ui.add_space(SPACE_SM);
                    ui.label(&source_title);
                    ui.label("Target date (yyyy-MM-dd)");
                    ui.text_edit_singleline(&mut self.copy_target_date);
                    ui.horizontal(|ui| {
                        if secondary_button(ui, palette, "Cancel").clicked() {
                            self.copy_source_id = None;
                        }
                        if primary_button(ui, palette, "Copy item").clicked() {
                            if let Some(id) = source_id.clone() {
                                let target = self.copy_target_date.clone();
                                self.copy_item_to_date(&id, &target);
                            }
                        }
                    });
                });
            });
    }
}