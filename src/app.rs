use std::sync::mpsc;
use std::time::{Duration as StdDuration, Instant};

use chrono::{Datelike, Duration, Utc};
use egui::{
    self, Align, Align2, Color32, CornerRadius, FontFamily, FontId, Frame, Key, Layout, Margin,
    Order, Pos2, RichText, ScrollArea, Sense, Stroke, StrokeKind, Vec2,
};
use uuid::Uuid;

use crate::categories::{category_label, CATEGORIES};
use crate::dates::{
    end_of_month, end_of_week, format_date_key, format_day_full, format_local_time,
    format_month_year, get_month_grid, is_in_current_week, is_same_month, is_today,
    iso_weekday_labels, items_for_day, month_offset_for_date, parse_date_key, start_of_month,
    start_of_week, today_local,
};
use crate::markdown::{generate_markdown_for_range, parse_markdown_report, ParsedMarkdownReport};
use crate::models::{
    APP_CONTACT_EMAIL, APP_LICENSE, APP_NAME, APP_REPO_URL, APP_RELEASES_URL, APP_TAGLINE,
    APP_VERSION, Prefs, WorkItem,
};
use crate::pdf::{generate_pdf_for_range, range_report_stats};
use crate::storage::{load_items, load_prefs, save_items, save_prefs, storage_dir_display};
use crate::theme::{apply_visuals, Palette, ThemeId};
use crate::ui::chrome::{
    accent_ghost_button_enabled, achievement_badge, achievement_toggle_chip,
    cal_day_cell_for_layout, cal_month_grid_content_height, calendar_card_frame,
    calendar_column_width, calendar_outer_frame, card_frame, category_pill, dashed_add_bar_button,
    detail_column_frame, ghost_button_compact, ghost_icon_button, horizontal_divider, modal_header,
    modal_window_frame, paint_brand_hero_banner, paint_modal_backdrop, paint_week_column_cell,
    preset_button, primary_button, primary_button_compact, primary_button_enabled,
    scrubber_icon_button, scrubber_today_button, secondary_button, settings_tab_button,
    soft_accent_button, soft_accent_button_enabled, status_pill, styled_multiline_input,
    styled_singleline_input, theme_menu_row, theme_swatch_button,
    toggle_icon_button, toolbar_panel_frame, vertical_divider, HeroContent, CAL_WEEK_COL_WIDTH,
    CARD_RADIUS, DETAIL_COLUMN_MIN_WIDTH, SPACE_LG, SPACE_MD, SPACE_SM, SPACE_XS,
};
use crate::ui::icons::{
    self, font_bold, icon_label, icon_label_bold, rich_bold, CALENDAR_BLANK, CARET_LEFT,
    CARET_RIGHT, CHECK_CIRCLE, COPY, CORNERS_IN, DOWNLOAD, EXPORT, FILE_TEXT, FOLDER_OPEN, GEAR,
    MINUS, PENCIL, PLUS, PUSH_PIN, PUSH_PIN_SLASH, SQUARE, TIMER, TRASH, UPLOAD_SIMPLE,
    WARNING_CIRCLE, X,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ImportMode {
    MergeSkipDuplicates,
    ReplaceDateRange,
}

#[derive(Debug, Clone)]
struct ImportPlan {
    /// Per-item flag aligned 1:1 with `report.items`: `true` if `MergeSkipDuplicates` skips it.
    duplicate_flags: Vec<bool>,
    duplicate_count: usize,
    new_merge_count: usize,
    achievements_count: usize,
    replace_range: Option<(chrono::NaiveDate, chrono::NaiveDate)>,
    existing_in_replace_range: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ImportOutcome {
    imported_count: usize,
    latest_date: Option<chrono::NaiveDate>,
}

fn import_dedup_key(item: &WorkItem) -> (String, String, String) {
    (
        item.date.trim().to_string(),
        item.title.trim().to_lowercase(),
        item.category.trim().to_lowercase(),
    )
}

/// Computes preview telemetry and per-item duplicate flags for `report` against `existing_items`.
///
/// Responsibility & invariants:
/// - Defines duplicate identity in [`ImportMode::MergeSkipDuplicates`] as matching `(date,
///   trimmed lowercase title, lowercase category)`, tracking seen keys across both `existing_items`
///   and earlier entries inside `report.items` so intra-file duplicates are also surfaced cleanly.
/// - Computes `replace_range` from `report.start_date..=report.end_date` (falling back to the min
///   and max item dates when needed) and counts how many `existing_items` lie inside that window so
///   [`ImportMode::ReplaceDateRange`] discloses its exact blast radius before confirmation.
fn compute_import_plan(existing_items: &[WorkItem], report: &ParsedMarkdownReport) -> ImportPlan {
    let mut seen: std::collections::HashSet<(String, String, String)> =
        existing_items.iter().map(import_dedup_key).collect();

    let mut duplicate_flags = Vec::with_capacity(report.items.len());
    let mut duplicate_count = 0;
    let mut achievements_count = 0;

    let mut min_item_date: Option<chrono::NaiveDate> = None;
    let mut max_item_date: Option<chrono::NaiveDate> = None;

    for item in &report.items {
        if item.is_achievement {
            achievements_count += 1;
        }
        if let Some(d) = parse_date_key(&item.date) {
            min_item_date = Some(min_item_date.map_or(d, |prev| prev.min(d)));
            max_item_date = Some(max_item_date.map_or(d, |prev| prev.max(d)));
        }
        let is_dup = !seen.insert(import_dedup_key(item));
        if is_dup {
            duplicate_count += 1;
        }
        duplicate_flags.push(is_dup);
    }

    let replace_range = match (report.start_date.or(min_item_date), report.end_date.or(max_item_date))
    {
        (Some(s), Some(e)) if s <= e => Some((s, e)),
        _ => None,
    };

    let existing_in_replace_range = match replace_range {
        Some((start, end)) => existing_items
            .iter()
            .filter(|i| parse_date_key(&i.date).is_some_and(|d| d >= start && d <= end))
            .count(),
        None => 0,
    };

    let new_merge_count = report.items.len().saturating_sub(duplicate_count);

    ImportPlan {
        duplicate_flags,
        duplicate_count,
        new_merge_count,
        achievements_count,
        replace_range,
        existing_in_replace_range,
    }
}

/// Applies `report` onto `items` according to `mode` and returns the count of imported items plus
/// the most recent imported [`chrono::NaiveDate`] so the UI can jump directly to it.
///
/// Invariants:
/// - In [`ImportMode::MergeSkipDuplicates`], existing items are never mutated or removed; only
///   items whose `(date, lowercase trimmed title, lowercase category)` is not yet present are
///   appended.
/// - In [`ImportMode::ReplaceDateRange`], if `report.items` is empty this function is a safe no-op
///   (protecting against accidental range wipes from empty reports); otherwise only existing items
///   with valid dates inside `replace_range` are replaced while all dates outside the range stay
///   untouched.
fn apply_markdown_import(
    items: &mut Vec<WorkItem>,
    report: &ParsedMarkdownReport,
    mode: ImportMode,
) -> ImportOutcome {
    let plan = compute_import_plan(items, report);
    let mut latest_date: Option<chrono::NaiveDate> = None;
    let mut imported_count = 0;

    match mode {
        ImportMode::MergeSkipDuplicates => {
            for (item, &is_dup) in report.items.iter().zip(plan.duplicate_flags.iter()) {
                if is_dup {
                    continue;
                }
                if let Some(d) = parse_date_key(&item.date) {
                    latest_date = Some(latest_date.map_or(d, |prev| prev.max(d)));
                }
                items.push(item.clone());
                imported_count += 1;
            }
        }
        ImportMode::ReplaceDateRange => {
            if report.items.is_empty() {
                return ImportOutcome {
                    imported_count: 0,
                    latest_date: None,
                };
            }
            if let Some((start, end)) = plan.replace_range {
                items.retain(|existing| {
                    parse_date_key(&existing.date).is_none_or(|d| d < start || d > end)
                });
            }
            for item in &report.items {
                if let Some(d) = parse_date_key(&item.date) {
                    latest_date = Some(latest_date.map_or(d, |prev| prev.max(d)));
                }
                items.push(item.clone());
                imported_count += 1;
            }
        }
    }

    ImportOutcome {
        imported_count,
        latest_date,
    }
}

/// Blocking open/save dialog for Markdown import and PDF/Markdown export.
///
/// `save_name` is the suggested file name for a save dialog. `None` opens an existing file.
/// Cancel, an empty selection, and a missing dialog binary all return `None` so the caller
/// leaves the workspace unchanged.
///
/// Linux must not call `rfd::FileDialog`. rfd 0.15's default backend sends the request to the
/// XDG desktop portal through ashpd and zbus 5.19. On this XFCE session that segfaults the
/// process: the `zbus::Connection` thread dies with `SIGSEGV` inside
/// `PropertiesCache::update_cache` while iterating `invalidated_properties`
/// (`IntoIterator for &[T]`). The UI thread is blocked in `pollster::block_on` waiting for
/// the portal, so rfd's zenity fallback — which runs only after the portal returns `Err` —
/// never executes. A fatal signal is not that `Err`. Zenity is a child process; its cancel
/// or crash cannot take this process down. Other platforms keep `rfd`'s native dialog.
fn choose_report_file(
    title: &str,
    filter_name: &str,
    extensions: &[&str],
    save_name: Option<&str>,
) -> Option<std::path::PathBuf> {
    #[cfg(target_os = "linux")]
    {
        let mut cmd = report_file_dialog_command(title, filter_name, extensions, save_name);
        match cmd.output() {
            Ok(output) => {
                let Ok(text) = String::from_utf8(output.stdout) else {
                    return None;
                };
                let path = text.trim();
                if path.is_empty() {
                    None
                } else {
                    Some(std::path::PathBuf::from(path))
                }
            }
            Err(err) => {
                eprintln!("file dialog failed ({title}): {err}");
                None
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let mut dialog = rfd::FileDialog::new()
            .set_title(title)
            .add_filter(filter_name, extensions);
        if let Some(name) = save_name {
            dialog = dialog.set_file_name(name);
            dialog.save_file()
        } else {
            dialog.pick_file()
        }
    }
}

/// Builds the Linux zenity invocation used by [`choose_report_file`].
///
/// Separated from process execution so tests can assert the dialog never goes through the
/// XDG portal. `--no-markup` keeps titles with `&` or `<` from being parsed as Pango.
/// `--confirm-overwrite` is save-only: an open dialog must not prompt to replace a file
/// the user is only selecting.
#[cfg(target_os = "linux")]
fn report_file_dialog_command(
    title: &str,
    filter_name: &str,
    extensions: &[&str],
    save_name: Option<&str>,
) -> std::process::Command {
    let mut cmd = std::process::Command::new("zenity");
    cmd.arg("--no-markup");
    cmd.arg("--file-selection");
    cmd.args(["--title", title]);
    if let Some(name) = save_name {
        cmd.args(["--save", "--confirm-overwrite", "--filename", name]);
    }
    let patterns: Vec<String> = extensions.iter().map(|ext| format!("*.{ext}")).collect();
    cmd.arg("--file-filter");
    cmd.arg(format!("{filter_name} | {}", patterns.join(" ")));
    cmd
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HeroMetrics {
    this_week_count: usize,
    month_count: usize,
    month_achievements: usize,
}

impl HeroMetrics {
    /// Aggregates live workspace pulse counts from `items` relative to `today`'s ISO week
    /// (`Mon..=Sun`) and the calendar's `viewed_month`.
    ///
    /// Malformed external `date` strings in `data.json` are skipped safely rather than panicking.
    fn compute(
        items: &[WorkItem],
        viewed_month: chrono::NaiveDate,
        today: chrono::NaiveDate,
    ) -> Self {
        let week_start = start_of_week(today);
        let week_end = end_of_week(today);
        let mut this_week_count = 0;
        let mut month_count = 0;
        let mut month_achievements = 0;

        for item in items {
            let Some(d) = parse_date_key(&item.date) else {
                continue;
            };
            if d >= week_start && d <= week_end {
                this_week_count += 1;
            }
            if is_same_month(d, viewed_month) {
                month_count += 1;
                if item.is_achievement {
                    month_achievements += 1;
                }
            }
        }

        Self {
            this_week_count,
            month_count,
            month_achievements,
        }
    }
}

pub struct WeeklyTrackerApp {
    items: Vec<WorkItem>,
    month_offset: i32,
    pinned_date: Option<chrono::NaiveDate>,
    hover_date: Option<chrono::NaiveDate>,
    theme: ThemeId,
    prefs: Prefs,
    show_import: bool,
    show_export: bool,
    show_settings: bool,
    settings_tab: SettingsTab,

    import_mode: ImportMode,
    import_file_name: Option<String>,
    import_parsed: Option<Result<ParsedMarkdownReport, String>>,
    import_banner: Option<String>,

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
        icons::install_fonts(&cc.egui_ctx);
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
            show_import: false,
            show_export: false,
            show_settings: false,
            settings_tab: SettingsTab::Storage,
            import_mode: ImportMode::MergeSkipDuplicates,
            import_file_name: None,
            import_parsed: None,
            import_banner: None,
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

    /// Jumps the calendar to the current month, pins today's date so pointer movement across the
    /// calendar grid cannot shift selection on the way to the form, and opens the Add Work Item
    /// form anchored on `today_local()`.
    fn log_today(&mut self) {
        let today = today_local();
        self.month_offset = 0;
        self.pinned_date = Some(today);
        self.open_add_form(today);
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

    fn open_import_modal(&mut self) {
        self.show_import = true;
        self.import_mode = ImportMode::MergeSkipDuplicates;
        self.import_file_name = None;
        self.import_parsed = None;
    }

    fn load_import_markdown_content(&mut self, file_name: String, content: Result<String, String>) {
        self.import_file_name = Some(file_name);
        self.import_parsed = Some(content.and_then(|raw| parse_markdown_report(&raw)));
    }

    fn confirm_markdown_import(&mut self) {
        let Some(Ok(report)) = self.import_parsed.clone() else {
            return;
        };
        let outcome = apply_markdown_import(&mut self.items, &report, self.import_mode);
        if outcome.imported_count == 0 {
            return;
        }
        self.persist_items();
        if let Some(latest) = outcome.latest_date {
            self.month_offset = month_offset_for_date(latest);
            self.pinned_date = Some(latest);
        }
        let item_word = if outcome.imported_count == 1 {
            "item"
        } else {
            "items"
        };
        let source = self
            .import_file_name
            .as_deref()
            .unwrap_or("Markdown report");
        self.import_banner = Some(format!(
            "Imported {} work {item_word} from {source}",
            outcome.imported_count
        ));
        self.show_import = false;
        self.import_file_name = None;
        self.import_parsed = None;
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
        let skip_key = ctx.input(|i| {
            i.key_pressed(Key::Escape) || i.key_pressed(Key::Space) || i.key_pressed(Key::Enter)
        });
        let elapsed = self.splash_started.elapsed();
        if skip_key || elapsed >= StdDuration::from_millis(1000) {
            self.dismiss_splash(ctx);
            return;
        }
        ctx.request_repaint_after(StdDuration::from_millis(16));
    }

    /// Renders a calm, skippable ~1.0s architectural-neutral launch surface that fades cleanly
    /// into the workspace without jarring full-screen gradients. A click that does not turn into
    /// a drag, or pressing `Esc`, `Space`, or `Enter`, dismisses it on the current frame.
    ///
    /// Dragging the splash moves the undecorated window. The OS frame is disabled in `main`, so
    /// this overlay is the only surface on screen for the first second and must itself be a drag
    /// handle. A drag is not treated as a dismiss click.
    fn splash_overlay(&mut self, ctx: &egui::Context, palette: Palette) {
        let screen = ctx.input(|i| i.screen_rect);
        let fade_start = StdDuration::from_millis(650);
        let elapsed = self.splash_started.elapsed();
        let alpha = if elapsed >= fade_start {
            let t = (elapsed - fade_start).as_secs_f32() / 0.35;
            (1.0 - t).clamp(0.0, 1.0)
        } else {
            1.0
        };

        egui::Area::new(egui::Id::new("splash"))
            .order(Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
                if response.drag_started_by(egui::PointerButton::Primary) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                } else if response.clicked() {
                    self.dismiss_splash(ctx);
                }
                let painter = ui.painter();
                painter.rect_filled(rect, 0, palette.background.gamma_multiply(alpha));

                let card_size = Vec2::new(320.0, 196.0);
                let card_rect = egui::Rect::from_center_size(rect.center(), card_size);
                painter.rect_filled(
                    card_rect,
                    CornerRadius::same(16),
                    palette.surface.gamma_multiply(alpha),
                );
                painter.rect_stroke(
                    card_rect,
                    CornerRadius::same(16),
                    Stroke::new(1.0, palette.border.gamma_multiply(alpha)),
                    StrokeKind::Inside,
                );

                let badge_rect = egui::Rect::from_center_size(
                    card_rect.center() - Vec2::new(0.0, 42.0),
                    Vec2::splat(44.0),
                );
                painter.rect_filled(
                    badge_rect,
                    CornerRadius::same(11),
                    palette.accent_soft.gamma_multiply(alpha),
                );
                painter.rect_stroke(
                    badge_rect,
                    CornerRadius::same(11),
                    Stroke::new(1.0, palette.accent_soft_border.gamma_multiply(alpha)),
                    StrokeKind::Inside,
                );
                painter.text(
                    badge_rect.center(),
                    Align2::CENTER_CENTER,
                    TIMER,
                    FontId::new(22.0, FontFamily::Proportional),
                    palette.primary.gamma_multiply(alpha),
                );

                painter.text(
                    card_rect.center() + Vec2::new(0.0, 6.0),
                    Align2::CENTER_CENTER,
                    APP_NAME,
                    font_bold(20.0),
                    palette.text.gamma_multiply(alpha),
                );
                painter.text(
                    card_rect.center() + Vec2::new(0.0, 32.0),
                    Align2::CENTER_CENTER,
                    APP_TAGLINE,
                    FontId::new(12.5, FontFamily::Proportional),
                    palette.text_muted.gamma_multiply(alpha),
                );
                painter.text(
                    card_rect.center() + Vec2::new(0.0, 62.0),
                    Align2::CENTER_CENTER,
                    format!("v{APP_VERSION}"),
                    FontId::new(11.0, FontFamily::Proportional),
                    palette.text_faint.gamma_multiply(alpha),
                );
            });
    }
}

/// Restores edge resizing after the OS window frame is removed in `main`.
///
/// A 5px band inside each screen edge maps to [`egui::ResizeDirection`]. The band is narrower
/// than the toolbar's 16px horizontal inset, so it does not cover Close, Import, or Export.
/// On the press that starts inside the band, [`egui::ViewportCommand::BeginResize`] asks the
/// window manager to run the resize grab. While the window is maximized there is no edge to
/// drag, so the function returns without changing the cursor.
///
/// The cursor icon matches the edge so the pointer shows a resize arrow before the press.
/// Without this, an undecorated X11 window has no frame grip and cannot be resized at all.
fn undecorated_window_edges(ctx: &egui::Context) {
    const BORDER: f32 = 5.0;
    let (pointer, rect, maximized, pressed) = ctx.input(|input| {
        (
            input.pointer.interact_pos(),
            input.screen_rect(),
            input.viewport().maximized.unwrap_or(false),
            input.pointer.primary_pressed(),
        )
    });
    if maximized {
        return;
    }
    let Some(pointer) = pointer else {
        return;
    };
    if !rect.contains(pointer) {
        return;
    }

    let west = pointer.x <= rect.left() + BORDER;
    let east = pointer.x >= rect.right() - BORDER;
    let north = pointer.y <= rect.top() + BORDER;
    let south = pointer.y >= rect.bottom() - BORDER;
    let direction = match (west, east, north, south) {
        (true, _, true, _) => Some(egui::ResizeDirection::NorthWest),
        (_, true, true, _) => Some(egui::ResizeDirection::NorthEast),
        (true, _, _, true) => Some(egui::ResizeDirection::SouthWest),
        (_, true, _, true) => Some(egui::ResizeDirection::SouthEast),
        (true, _, _, _) => Some(egui::ResizeDirection::West),
        (_, true, _, _) => Some(egui::ResizeDirection::East),
        (_, _, true, _) => Some(egui::ResizeDirection::North),
        (_, _, _, true) => Some(egui::ResizeDirection::South),
        _ => None,
    };
    let Some(direction) = direction else {
        return;
    };

    ctx.set_cursor_icon(match direction {
        egui::ResizeDirection::North | egui::ResizeDirection::South => {
            egui::CursorIcon::ResizeVertical
        }
        egui::ResizeDirection::East | egui::ResizeDirection::West => {
            egui::CursorIcon::ResizeHorizontal
        }
        egui::ResizeDirection::NorthEast | egui::ResizeDirection::SouthWest => {
            egui::CursorIcon::ResizeNeSw
        }
        egui::ResizeDirection::NorthWest | egui::ResizeDirection::SouthEast => {
            egui::CursorIcon::ResizeNwSe
        }
    });
    if pressed {
        ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
    }
}

impl eframe::App for WeeklyTrackerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        undecorated_window_edges(ctx);
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

        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            if self.copy_source_id.is_some() {
                self.copy_source_id = None;
            } else if self.show_import {
                self.show_import = false;
            } else if self.show_export {
                self.show_export = false;
            } else if self.show_settings {
                self.show_settings = false;
            } else if self.form_mode.is_some() {
                self.cancel_form();
            }
        }

        if let Some(msg) = self.import_banner.clone() {
            egui::TopBottomPanel::top("import_banner")
                .frame(
                    Frame::new()
                        .fill(palette.accent_soft)
                        .inner_margin(Margin::symmetric(SPACE_MD as i8, 6))
                        .stroke(Stroke::NONE),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            icon_label_bold(CHECK_CIRCLE, &msg, 12.5, palette.accent_soft_fg),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ghost_button_compact(ui, palette, "Dismiss").clicked() {
                                self.import_banner = None;
                            }
                        });
                    });
                });
            egui::TopBottomPanel::top("import_banner_divider")
                .frame(Frame::new().fill(palette.accent_soft_border).inner_margin(Margin::ZERO))
                .exact_height(1.0)
                .show(ctx, |_ui| {});
        }

        if let Some(banner) = self.update_banner.clone() {
            egui::TopBottomPanel::top("update_banner")
                .frame(
                    Frame::new()
                        .fill(palette.accent_soft)
                        .inner_margin(Margin::symmetric(SPACE_MD as i8, 6))
                        .stroke(Stroke::NONE),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            rich_bold(format!("Update available · {}", banner.title), 12.5)
                                .color(palette.accent_soft_fg),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_XS;
                            if ghost_button_compact(ui, palette, "Dismiss").clicked() {
                                self.update_banner = None;
                            }
                            if primary_button_compact(ui, palette, "Open in browser").clicked() {
                                Self::open_github(&banner.url);
                            }
                        });
                    });
                });
        }

        egui::TopBottomPanel::top("toolbar")
            .frame(toolbar_panel_frame(palette))
            .show(ctx, |ui| {
                self.toolbar(ui, palette);
            });

        egui::TopBottomPanel::bottom("footer")
            .frame(toolbar_panel_frame(palette))
            .show(ctx, |ui| {
                horizontal_divider(ui, palette.accent_soft_border);
                Frame::new()
                    .inner_margin(Margin::symmetric(SPACE_MD as i8, 5))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "© {} Biplab Sarkar · {}",
                                    today_local().year(),
                                    APP_LICENSE
                                ))
                                .size(11.0)
                                .color(palette.text),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(format!("{APP_NAME} v{APP_VERSION}"))
                                        .size(11.0)
                                        .color(palette.text_muted),
                                );
                            });
                        });
                    });
            });

        egui::CentralPanel::default()
            .frame(Frame::new().fill(palette.background).inner_margin(Margin::ZERO))
            .show(ctx, |ui| {
                let screen_h = ctx.input(|i| i.screen_rect.height());
                let compact_hero = screen_h < 620.0;
                let ultra_short = screen_h < 540.0;
                let hero_pad_v: i8 = if ultra_short {
                    6
                } else if compact_hero {
                    8
                } else {
                    10
                };
                let cal_pad_v: i8 = if ultra_short {
                    8
                } else if compact_hero {
                    10
                } else {
                    12
                };

                // Zero vertical auto-spacing between the Hero showcase band, the 1px blueprint
                // seam, and the Calendar | Day Detail split so `ui.available_height()` gives the
                // exact pixel budget remaining for all 4–6 calendar week rows.
                ui.spacing_mut().item_spacing.y = 0.0;

                Frame::new()
                    .inner_margin(Margin::symmetric(12, hero_pad_v))
                    .show(ui, |ui| {
                        self.brand_hero(ui, palette, compact_hero);
                    });

                horizontal_divider(ui, palette.border);

                let total_w = ui.available_width();
                let split_h = ui.available_height().max(0.0);
                let cal_w = calendar_column_width(total_w, SPACE_MD);
                let divider_w = 1.0;
                let detail_w =
                    (total_w - cal_w - SPACE_MD - divider_w).max(DETAIL_COLUMN_MIN_WIDTH);

                ui.horizontal(|ui| {
                    ui.set_width(total_w);
                    ui.spacing_mut().item_spacing.x = 0.0;

                    ui.allocate_ui_with_layout(
                        Vec2::new(cal_w + SPACE_MD, split_h),
                        Layout::top_down(Align::LEFT),
                        |ui| {
                            Frame::new()
                                .inner_margin(Margin {
                                    left: 12,
                                    right: 12,
                                    top: cal_pad_v,
                                    bottom: cal_pad_v,
                                })
                                .show(ui, |ui| {
                                    let inner_w = (cal_w + SPACE_MD - 24.0).max(0.0);
                                    let inner_h =
                                        (split_h - (cal_pad_v as f32) * 2.0).max(0.0);
                                    ui.set_width(inner_w);
                                    ui.set_min_height(inner_h);
                                    self.month_calendar(ui, palette, inner_h);
                                });
                        },
                    );

                    let (div_rect, _) =
                        ui.allocate_exact_size(Vec2::new(divider_w, split_h), Sense::hover());
                    ui.painter().rect_filled(div_rect, 0, palette.border);

                    ui.allocate_ui_with_layout(
                        Vec2::new(detail_w, split_h),
                        Layout::top_down(Align::LEFT),
                        |ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(8.0, 6.0);
                            ui.set_width(detail_w);
                            ui.set_min_height(split_h);
                            detail_column_frame(palette).show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.set_min_height((split_h - SPACE_MD * 2.0).max(0.0));
                                if self.form_mode.is_some() {
                                    self.item_form(ui, palette);
                                } else {
                                    self.day_detail(ui, palette);
                                }
                            });
                        },
                    );
                });
            });

        if self.show_import {
            self.import_modal(ctx, palette);
        }
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
    /// Renders the compact ~45px top utility bar (`chrome_bar_fill` + `palette.accent_soft_border` seam)
    /// dedicated to navigation and workspace controls (`< | Today | >` month scrubber, active
    /// `Month Year`, dual-tone Theme picker, Settings icon, and Export button) so it pairs cleanly
    /// with [`Self::brand_hero`] directly below without competing duplicate brand headers.
    ///
    /// The bar is filled with [`chrome_bar_fill`], the theme's light accent tint
    /// (`palette.accent_soft_hover`).
    ///
    /// The OS window frame is disabled, so this bar is the title-bar replacement. Empty space in
    /// the bar (not the buttons) starts a window drag via [`egui::ViewportCommand::StartDrag`].
    /// `Minimize`, `Maximize`/`Restore`, and `Close` sit at the right edge, in that order.
    /// They replace the removed title-bar buttons: minimize sends
    /// [`egui::ViewportCommand::Minimized`], maximize toggles
    /// [`egui::ViewportCommand::Maximized`], and close sends
    /// [`egui::ViewportCommand::Close`].
    fn toolbar(&mut self, ui: &mut egui::Ui, palette: Palette) {
        let drag = ui.interact(ui.max_rect(), ui.id().with("window_drag"), Sense::drag());
        if drag.drag_started_by(egui::PointerButton::Primary) {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        let grid = get_month_grid(self.month_offset);
        let at_current_month = self.month_offset == 0 && self.pinned_date.is_none();

        Frame::new()
            .inner_margin(Margin::symmetric(SPACE_MD as i8, 6))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.x = SPACE_SM;

                    Frame::new()
                        .fill(palette.surface_muted)
                        .stroke(Stroke::new(1.0, palette.border))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(Margin::same(2))
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            if scrubber_icon_button(ui, palette, CARET_LEFT, "Previous month")
                                .clicked()
                            {
                                self.month_offset -= 1;
                            }
                            if scrubber_today_button(ui, palette, at_current_month).clicked() {
                                self.month_offset = 0;
                                self.pinned_date = None;
                            }
                            if scrubber_icon_button(ui, palette, CARET_RIGHT, "Next month")
                                .clicked()
                            {
                                self.month_offset += 1;
                            }
                        });

                    ui.add_space(SPACE_XS);
                    ui.label(
                        rich_bold(format_month_year(grid.month), 15.5).color(palette.text),
                    );
                    if self.month_offset == 0 {
                        status_pill(
                            ui,
                            palette.surface_muted,
                            palette.border,
                            palette.text_muted,
                            &format!("W{}", today_local().iso_week().week()),
                        );
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = SPACE_SM;
                        if ghost_icon_button(ui, palette, X, 14.0, "Close").clicked() {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                        let (zoom_icon, zoom_tip) = if maximized {
                            (CORNERS_IN, "Restore")
                        } else {
                            (SQUARE, "Maximize")
                        };
                        if ghost_icon_button(ui, palette, zoom_icon, 14.0, zoom_tip).clicked() {
                            ui.ctx()
                                .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                        }
                        if ghost_icon_button(ui, palette, MINUS, 14.0, "Minimize").clicked() {
                            ui.ctx()
                                .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                        if secondary_button(
                            ui,
                            palette,
                            icon_label(EXPORT, "Export", 13.0, palette.text),
                        )
                        .on_hover_text("Export PDF or Markdown report")
                        .clicked()
                        {
                            self.show_export = true;
                        }

                        if secondary_button(
                            ui,
                            palette,
                            icon_label(UPLOAD_SIMPLE, "Import", 13.0, palette.text),
                        )
                        .on_hover_text("Import work items from an exported Markdown (.md) report")
                        .clicked()
                        {
                            self.open_import_modal();
                        }

                        if ghost_icon_button(ui, palette, GEAR, 16.5, "Settings").clicked() {
                            self.show_settings = true;
                        }

                        vertical_divider(ui, 18.0, palette.divider);

                        let theme_btn = theme_swatch_button(ui, palette, self.theme);
                        let bar_id = ui.id().with("toolbar_theme_bar");
                        let mut bar_state = egui::menu::BarState::load(ui.ctx(), bar_id);
                        let _ = bar_state.bar_menu(&theme_btn, |ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            for t in ThemeId::ALL {
                                if theme_menu_row(ui, palette, t, self.theme == t).clicked() {
                                    self.set_theme(ui.ctx(), t);
                                    ui.close_menu();
                                }
                            }
                        });
                        bar_state.store(ui.ctx(), bar_id);
                    });
                });
            });

        horizontal_divider(ui, palette.accent_soft_border);
    }

    /// Renders the brand-themed Hero Banner (`Work Tracker — Track your work. Celebrate your wins.`)
    /// with live ISO-week, viewed-month, and achievement telemetry pills plus the `+ Log Today`
    /// one-click CTA.
    fn brand_hero(&mut self, ui: &mut egui::Ui, palette: Palette, compact_height: bool) {
        let today = today_local();
        let grid = get_month_grid(self.month_offset);
        let metrics = HeroMetrics::compute(&self.items, grid.month, today);
        let wide_banner = ui.available_width() >= 920.0;

        let month_label = if grid.month.year() != today.year() {
            grid.month.format("%b '%y").to_string()
        } else if wide_banner {
            grid.month.format("%B").to_string()
        } else {
            grid.month.format("%b").to_string()
        };

        let viewing_context = match self.pinned_date {
            Some(pinned) => format!(
                "Viewing {}  ·  Pinned {}",
                format_month_year(grid.month),
                pinned.format("%b %-d")
            ),
            None => format!(
                "Viewing {}  ·  ISO Week {}",
                format_month_year(grid.month),
                today.iso_week().week()
            ),
        };

        if paint_brand_hero_banner(
            ui,
            palette,
            self.theme,
            HeroContent {
                app_name: APP_NAME,
                tagline: APP_TAGLINE,
                viewing_context: &viewing_context,
                this_week_count: metrics.this_week_count,
                month_label: &month_label,
                month_count: metrics.month_count,
                month_achievements: metrics.month_achievements,
            },
            compact_height,
        ) {
            self.log_today();
        }
    }

    /// Renders the left-hand architectural month calendar card inside `card_max_h` vertical space.
    ///
    /// Protects two layout invariants:
    /// 1. Sizes day cells via [`cal_day_cell_for_layout`] against `card_max_h` minus card margins
    ///    and header height so all 4–6 ISO week rows remain simultaneously visible without vertical
    ///    clipping across standard desktop heights (`>= MIN_WINDOW_HEIGHT`).
    /// 2. Applies identical `SPACE_SM` horizontal margins to both the `Wk / Mo..Su` header strip
    ///    and [`calendar_outer_frame`] so weekday captions align pixel-for-pixel with day columns.
    fn month_calendar(&mut self, ui: &mut egui::Ui, palette: Palette, card_max_h: f32) {
        let grid = get_month_grid(self.month_offset);
        let counts = self.item_counts();
        let achievements = self.achievement_dates();
        let radius = CornerRadius::same(7);
        let cell_gap = SPACE_SM;

        let month_total: usize = grid
            .weeks
            .iter()
            .flat_map(|w| w.days.iter())
            .filter(|&&d| is_same_month(d, grid.month))
            .map(|d| counts.get(&format_date_key(*d)).copied().unwrap_or(0))
            .sum();

        calendar_card_frame(palette).show(ui, |ui| {
            let col_w = ui.available_width();
            // `calendar_card_frame` has 10px top + 10px bottom inner margin = 20px total.
            let inner_avail_h = (card_max_h - 20.0).max(0.0);
            // Title row (20px) + 6px gap + weekday strip (22px) + 2px gap = 50px fixed header.
            let header_reserve = 50.0;
            let scroll_h = (inner_avail_h - header_reserve).max(0.0);
            let day_cell = cal_day_cell_for_layout(col_w, grid.weeks.len(), scroll_h);
            let grid_content_h = cal_month_grid_content_height(grid.weeks.len(), day_cell);
            let scroll_max_h = grid_content_h.min(scroll_h);

            ui.spacing_mut().item_spacing.y = 0.0;

            ui.allocate_ui_with_layout(
                Vec2::new(col_w, 20.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.set_width(col_w);
                    Frame::new()
                        .inner_margin(Margin::symmetric(SPACE_SM as i8, 0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.set_width(ui.available_width());
                                ui.label(rich_bold("Calendar", 12.0).color(palette.text_muted));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let summary = if month_total == 1 {
                                        "1 entry this month".to_string()
                                    } else {
                                        format!("{month_total} entries this month")
                                    };
                                    ui.label(
                                        RichText::new(summary).size(11.0).color(palette.text_faint),
                                    );
                                });
                            });
                        });
                },
            );

            ui.add_space(6.0);

            // Match `calendar_outer_frame`'s horizontal `SPACE_SM` inset so `Wk` and all 7
            // weekday labels align column-for-column with the week rows below.
            Frame::new()
                .inner_margin(Margin::symmetric(SPACE_SM as i8, 0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = cell_gap;
                        let (wk_rect, _) =
                            ui.allocate_exact_size(Vec2::new(CAL_WEEK_COL_WIDTH, 22.0), Sense::hover());
                        paint_week_column_cell(
                            ui.painter(),
                            wk_rect,
                            CornerRadius::same(5),
                            palette,
                            false,
                            "Wk",
                        );
                        for label in iso_weekday_labels() {
                            let (rect, _) =
                                ui.allocate_exact_size(Vec2::new(day_cell.x, 22.0), Sense::hover());
                            ui.painter().text(
                                rect.center(),
                                Align2::CENTER_CENTER,
                                label,
                                font_bold(11.0),
                                palette.text_muted,
                            );
                        }
                    });
                });

            ui.add_space(2.0);

            ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height(scroll_max_h)
                .show(ui, |ui| {
                    ui.set_width(col_w);
                    calendar_outer_frame(palette).show(ui, |ui| {
                        ui.set_width((col_w - SPACE_SM * 2.0).max(0.0));
                        ui.spacing_mut().item_spacing = Vec2::new(cell_gap, cell_gap);
                        for week in &grid.weeks {
                            let current_week = week.days.iter().any(|&d| is_in_current_week(d));
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = cell_gap;
                                let wk_size = Vec2::new(CAL_WEEK_COL_WIDTH, day_cell.y);
                                let (wk_rect, _) =
                                    ui.allocate_exact_size(wk_size, Sense::hover());
                                paint_week_column_cell(
                                    ui.painter(),
                                    wk_rect,
                                    radius,
                                    palette,
                                    current_week,
                                    &format!("{}", week.week_number),
                                );

                                for &date in &week.days {
                                    let key = format_date_key(date);
                                    let in_month = is_same_month(date, grid.month);
                                    let today = is_today(date);
                                    let focused = self.pinned_date == Some(date);
                                    let cell_resp =
                                        ui.allocate_exact_size(day_cell, Sense::click());
                                    let (rect, response) = cell_resp;
                                    let ptr_hover = response.hovered();
                                    if ptr_hover {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    // Keep the unpinned hover-selected day subtly tinted even after
                                    // the pointer moves into the right-hand Day Detail pane so the
                                    // calendar visually echoes `self.display_date()`.
                                    let inspected_hover = ptr_hover
                                        || (self.pinned_date.is_none()
                                            && self.hover_date == Some(date));

                                    let mut bg = if focused {
                                        palette.cal_focus_bg
                                    } else if inspected_hover {
                                        palette.cal_hover_bg
                                    } else if today {
                                        palette.cal_today_bg
                                    } else if current_week && in_month {
                                        palette.cal_week_row
                                    } else {
                                        Color32::TRANSPARENT
                                    };
                                    if !in_month && !focused && !inspected_hover && !today {
                                        bg = Color32::TRANSPARENT;
                                    }

                                    if bg != Color32::TRANSPARENT {
                                        ui.painter().rect_filled(rect, radius, bg);
                                    }
                                    if focused {
                                        ui.painter().rect_stroke(
                                            rect,
                                            radius,
                                            Stroke::new(1.5, palette.primary),
                                            StrokeKind::Inside,
                                        );
                                    } else if today {
                                        ui.painter().rect_stroke(
                                            rect,
                                            radius,
                                            Stroke::new(1.0, palette.accent_soft_border),
                                            StrokeKind::Inside,
                                        );
                                    } else if inspected_hover {
                                        ui.painter().rect_stroke(
                                            rect,
                                            radius,
                                            Stroke::new(1.0, palette.border),
                                            StrokeKind::Inside,
                                        );
                                    }

                                    let count = counts.get(&key).copied().unwrap_or(0);
                                    let has_ach = achievements.contains(&key);
                                    let roomy = day_cell.y >= 38.0;

                                    let mut day_center = rect.center();
                                    if roomy && (count > 0 || has_ach) {
                                        day_center.y -= 4.0;
                                    }

                                    if today {
                                        let badge_r = if roomy { 11.0 } else { 9.5 };
                                        ui.painter().circle_filled(
                                            day_center,
                                            badge_r,
                                            palette.cal_today_badge_bg,
                                        );
                                    }

                                    let day_font = if today || focused {
                                        font_bold(12.5)
                                    } else {
                                        FontId::new(12.5, FontFamily::Proportional)
                                    };
                                    let day_fg = if today {
                                        palette.cal_today_badge_fg
                                    } else if focused {
                                        palette.accent_soft_fg
                                    } else if in_month {
                                        palette.text
                                    } else {
                                        palette.text_faint
                                    };
                                    ui.painter().text(
                                        day_center,
                                        Align2::CENTER_CENTER,
                                        format!("{}", date.day()),
                                        day_font,
                                        day_fg,
                                    );

                                    if count > 0 || has_ach {
                                        if roomy {
                                            let indicator_y = rect.bottom() - 6.5;
                                            let dots_shown = count.min(3);
                                            let total_dots =
                                                dots_shown + usize::from(has_ach);
                                            let spacing = 6.0;
                                            let start_x = rect.center().x
                                                - (total_dots.saturating_sub(1) as f32 * spacing)
                                                    * 0.5;
                                            for idx in 0..dots_shown {
                                                ui.painter().circle_filled(
                                                    Pos2::new(
                                                        start_x + idx as f32 * spacing,
                                                        indicator_y,
                                                    ),
                                                    2.3,
                                                    palette.cal_dot,
                                                );
                                            }
                                            if has_ach {
                                                ui.painter().circle_filled(
                                                    Pos2::new(
                                                        start_x + dots_shown as f32 * spacing,
                                                        indicator_y,
                                                    ),
                                                    2.5,
                                                    palette.achievement_fg,
                                                );
                                            }
                                        } else {
                                            let dot_pos =
                                                Pos2::new(rect.right() - 5.0, rect.bottom() - 5.0);
                                            let dot_col = if has_ach {
                                                palette.achievement_fg
                                            } else {
                                                palette.cal_dot
                                            };
                                            ui.painter().circle_filled(dot_pos, 2.2, dot_col);
                                        }
                                    }

                                    if ptr_hover {
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
        });
    }

    /// Renders the editorial right-hand Day Detail view for [`Self::display_date`], including
    /// status pills (`Today`, `Pinned`), `Pin` toggle, `+ Add Item` CTA, calm empty state,
    /// clickable work-item cards with quiet `Copy`/`Edit`/`Delete` icon actions, and the bottom
    /// dashed `+ Add another item` bar.
    fn day_detail(&mut self, ui: &mut egui::Ui, palette: Palette) {
        let date = self.display_date();
        let day_items: Vec<WorkItem> = items_for_day(&self.items, date)
            .into_iter()
            .cloned()
            .collect();
        let today = is_today(date);
        let pinned = self.pinned_date == Some(date);
        let achievement_count = day_items.iter().filter(|i| i.is_achievement).count();

        ui.horizontal(|ui| {
            ui.set_width(ui.available_width());
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = SPACE_SM;
                    ui.label(rich_bold(format_day_full(date), 18.0).color(palette.text));
                    if today {
                        status_pill(
                            ui,
                            palette.accent_soft,
                            palette.accent_soft_border,
                            palette.accent_soft_fg,
                            "Today",
                        );
                    }
                    if pinned {
                        status_pill(
                            ui,
                            palette.surface_muted,
                            palette.border_strong,
                            palette.text_muted,
                            "Pinned",
                        );
                    }
                });
                ui.add_space(2.0);
                let item_word = if day_items.len() == 1 {
                    "item"
                } else {
                    "items"
                };
                let subtitle = if achievement_count > 0 {
                    let ach_word = if achievement_count == 1 {
                        "achievement"
                    } else {
                        "achievements"
                    };
                    format!(
                        "{} {item_word} · {achievement_count} {ach_word}",
                        day_items.len()
                    )
                } else {
                    format!("{} {item_word} logged", day_items.len())
                };
                ui.label(RichText::new(subtitle).size(12.5).color(palette.text_muted));
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = SPACE_SM;
                if primary_button(
                    ui,
                    palette,
                    icon_label_bold(PLUS, "Add Item", 13.0, palette.on_primary),
                )
                .on_hover_text("Log a work item for this day")
                .clicked()
                {
                    self.open_add_form(date);
                }

                let pin_icon = if pinned { PUSH_PIN_SLASH } else { PUSH_PIN };
                let pin_tooltip = if pinned {
                    "Unpin day (follow calendar hover)"
                } else {
                    "Pin this day while browsing calendar"
                };
                if toggle_icon_button(ui, palette, pin_icon, pinned, pin_tooltip).clicked() {
                    self.pinned_date = if pinned { None } else { Some(date) };
                }
            });
        });

        ui.add_space(12.0);
        horizontal_divider(ui, palette.divider);
        ui.add_space(SPACE_MD);

        if day_items.is_empty() {
            let avail_h = ui.available_height().max(180.0);
            Frame::new()
                .fill(palette.background)
                .stroke(Stroke::new(1.0, palette.border))
                .corner_radius(CornerRadius::same(12))
                .inner_margin(Margin::symmetric(SPACE_LG as i8, SPACE_LG as i8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let inner_h = (avail_h - SPACE_LG * 2.0 - 8.0).max(132.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), inner_h),
                        Layout::top_down(Align::Center),
                        |ui| {
                            ui.add_space((inner_h * 0.16).max(8.0));
                            let (icon_rect, _) =
                                ui.allocate_exact_size(Vec2::splat(42.0), Sense::hover());
                            ui.painter().rect_filled(
                                icon_rect,
                                CornerRadius::same(10),
                                palette.surface,
                            );
                            ui.painter().rect_stroke(
                                icon_rect,
                                CornerRadius::same(10),
                                Stroke::new(1.0, palette.border),
                                StrokeKind::Inside,
                            );
                            ui.painter().text(
                                icon_rect.center(),
                                Align2::CENTER_CENTER,
                                CALENDAR_BLANK,
                                FontId::new(20.0, FontFamily::Proportional),
                                palette.text_muted,
                            );

                            ui.add_space(12.0);
                            ui.label(
                                rich_bold("No items logged for this day", 14.0)
                                    .color(palette.text),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(
                                    "Capture engineering progress, reviews, meetings, or key wins.",
                                )
                                .size(12.5)
                                .color(palette.text_muted),
                            );
                            ui.add_space(SPACE_MD);
                            if soft_accent_button(
                                ui,
                                palette,
                                icon_label_bold(PLUS, "Add Work Item", 13.0, palette.accent_soft_fg),
                            )
                            .clicked()
                            {
                                self.open_add_form(date);
                            }
                        },
                    );
                });
        } else {
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let mut pending_edit: Option<WorkItem> = None;
                    let mut pending_delete: Option<String> = None;

                    for item in &day_items {
                        let id = item.id.clone();
                        let inner = card_frame(palette).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            let mut action_interacted = false;

                            ui.horizontal(|ui| {
                                ui.set_width(ui.available_width());
                                let actions_reserve = 3.0 * 28.0 + 2.0 * 4.0 + SPACE_MD;
                                let text_w = (ui.available_width() - actions_reserve).max(100.0);

                                ui.allocate_ui_with_layout(
                                    Vec2::new(text_w, 0.0),
                                    Layout::top_down(Align::LEFT),
                                    |ui| {
                                        ui.label(rich_bold(&item.title, 14.0).color(palette.text));
                                        if !item.description.is_empty() {
                                            ui.add_space(4.0);
                                            ui.label(
                                                RichText::new(&item.description)
                                                    .size(12.5)
                                                    .color(palette.text_muted),
                                            );
                                        }
                                        ui.add_space(SPACE_SM);
                                        ui.horizontal_wrapped(|ui| {
                                            ui.spacing_mut().item_spacing.x = 6.0;
                                            category_pill(
                                                ui,
                                                &item.category,
                                                category_label(&item.category),
                                                palette.is_dark,
                                            );
                                            if item.is_achievement {
                                                let _ = achievement_badge(ui, palette);
                                            }
                                            ui.label(
                                                RichText::new(format!(
                                                    "· {}",
                                                    format_local_time(item.updated_at)
                                                ))
                                                .size(11.5)
                                                .color(palette.text_faint),
                                            );
                                        });
                                    },
                                );

                                ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                                    ui.spacing_mut().item_spacing.x = 2.0;

                                    let del_resp = icons::small_danger_icon_button(
                                        ui,
                                        palette,
                                        TRASH,
                                        "Delete item",
                                    );
                                    if del_resp.hovered() || del_resp.clicked() {
                                        action_interacted = true;
                                    }
                                    if del_resp.clicked() {
                                        pending_delete = Some(id.clone());
                                    }

                                    let edit_resp = icons::small_icon_button(
                                        ui,
                                        palette,
                                        PENCIL,
                                        "Edit item",
                                    );
                                    if edit_resp.hovered() || edit_resp.clicked() {
                                        action_interacted = true;
                                    }
                                    if edit_resp.clicked() {
                                        pending_edit = Some(item.clone());
                                    }

                                    let copy_resp = icons::small_icon_button(
                                        ui,
                                        palette,
                                        COPY,
                                        "Copy to another day",
                                    );
                                    if copy_resp.hovered() || copy_resp.clicked() {
                                        action_interacted = true;
                                    }
                                    if copy_resp.clicked() {
                                        self.copy_source_id = Some(id.clone());
                                        if let Some(d) = parse_date_key(&item.date) {
                                            self.copy_target_date =
                                                format_date_key(d + Duration::days(1));
                                        }
                                    }
                                });
                            });

                            action_interacted
                        });

                        let card_rect = inner.response.rect;
                        let action_interacted = inner.inner;
                        let card_hover =
                            ui.interact(card_rect, ui.id().with(("day_card", &id)), Sense::hover());
                        if card_hover.hovered() {
                            if !action_interacted {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            ui.painter().rect_stroke(
                                card_rect,
                                CornerRadius::same(CARD_RADIUS),
                                Stroke::new(1.0, palette.border_strong),
                                StrokeKind::Inside,
                            );
                        }
                        let card_clicked = card_hover.hovered()
                            && !action_interacted
                            && ui.input(|i| {
                                i.pointer.primary_clicked()
                                    && i.pointer
                                        .press_origin()
                                        .is_some_and(|origin| card_rect.contains(origin))
                            });
                        if card_clicked {
                            pending_edit = Some(item.clone());
                        }

                        ui.add_space(SPACE_SM);
                    }

                    if let Some(id) = pending_delete {
                        self.delete_item(&id);
                    } else if let Some(edit_item) = pending_edit {
                        self.open_edit_form(&edit_item);
                    }

                    ui.add_space(SPACE_XS);
                    if dashed_add_bar_button(
                        ui,
                        palette,
                        icon_label_bold(PLUS, "Add another item", 12.5, palette.text_muted),
                    )
                    .clicked()
                    {
                        self.open_add_form(date);
                    }
                });
        }
    }

    /// Renders the inline Add / Edit work-item form inside the right detail column with category
    /// selector, gold achievement toggle chip, focus-ring inputs, `Enter`-to-submit on title, and
    /// `Esc` / `X` cancellation.
    fn item_form(&mut self, ui: &mut egui::Ui, palette: Palette) {
        let heading = match self.form_mode {
            Some(FormMode::Add) => "Add Work Item",
            Some(FormMode::Edit) => "Edit Work Item",
            None => "Work Item",
        };

        let mut close_clicked = false;
        ui.horizontal(|ui| {
            ui.set_width(ui.available_width());
            ui.vertical(|ui| {
                ui.label(rich_bold(heading, 18.0).color(palette.text));
                if let Some(d) = self.form_date {
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(format_day_full(d))
                            .size(12.5)
                            .color(palette.text_muted),
                    );
                }
            });
            ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                if ghost_icon_button(ui, palette, crate::ui::icons::X, 15.0, "Cancel (Esc)")
                    .clicked()
                {
                    close_clicked = true;
                }
            });
        });
        if close_clicked {
            self.cancel_form();
            return;
        }

        ui.add_space(12.0);
        horizontal_divider(ui, palette.divider);
        ui.add_space(SPACE_MD);

        ui.label(rich_bold("Category & Impact", 12.0).color(palette.text_muted));
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = SPACE_SM;
            egui::ComboBox::from_id_salt("category")
                .height(240.0)
                .selected_text(
                    RichText::new(category_label(&self.form_category))
                        .size(13.0)
                        .color(palette.text),
                )
                .show_ui(ui, |ui| {
                    for cat in CATEGORIES {
                        ui.selectable_value(
                            &mut self.form_category,
                            cat.id.to_string(),
                            cat.label,
                        );
                    }
                });
            achievement_toggle_chip(ui, palette, &mut self.form_achievement);
        });

        ui.add_space(SPACE_MD);
        ui.label(rich_bold("Title", 12.0).color(palette.text_muted));
        ui.add_space(6.0);
        let title_resp = styled_singleline_input(
            ui,
            palette,
            &mut self.form_title,
            "What did you ship, review, or advance?",
        );

        ui.add_space(SPACE_MD);
        ui.horizontal(|ui| {
            ui.label(rich_bold("Details", 12.0).color(palette.text_muted));
            ui.label(RichText::new("(optional)").size(11.5).color(palette.text_faint));
        });
        ui.add_space(6.0);
        styled_multiline_input(
            ui,
            palette,
            &mut self.form_description,
            5,
            "Notes, PR links, design decisions, or measurable impact…",
        );

        ui.add_space(SPACE_LG);
        horizontal_divider(ui, palette.divider);
        ui.add_space(12.0);

        let can_save = !self.form_title.trim().is_empty();
        if title_resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) && can_save {
            self.submit_form();
            return;
        }

        let save_label = match self.form_mode {
            Some(FormMode::Edit) => "Update",
            _ => "Add Item",
        };

        ui.horizontal(|ui| {
            ui.set_width(ui.available_width());
            if matches!(self.form_mode, Some(FormMode::Edit))
                && accent_ghost_button_enabled(ui, palette, "Copy to another day…", can_save)
                    .clicked()
            {
                if let Some(id) = &self.form_item_id {
                    self.copy_source_id = Some(id.clone());
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = SPACE_SM;
                if primary_button_enabled(ui, palette, save_label, can_save).clicked() {
                    self.submit_form();
                }
                if secondary_button(ui, palette, "Cancel").clicked() {
                    self.cancel_form();
                }
            });
        });
    }

    /// Renders the frameless Export modal (`Export Report`) with scrim/Esc/X dismissal,
    /// active-highlighted date range presets, a live active-day telemetry strip, monospace
    /// Markdown preview, and both `.md` and active-days-only `.pdf` save dialogs.
    ///
    /// Responsibility & invariants:
    /// - Reuses [`range_report_stats`] for the modal's summary strip so the reported item and
    ///   active-day counts match [`generate_pdf_for_range`] identically, making clear before
    ///   saving that PDF output omits dates with zero saved entries.
    /// - Disables both `Download .md` and `Download PDF` identically whenever `export_start` or
    ///   `export_end` fails `YYYY-MM-DD` parsing or `start > end`.
    fn export_modal(&mut self, ctx: &egui::Context, palette: Palette) {
        let backdrop = paint_modal_backdrop(ctx, palette, egui::Id::new("export_modal_backdrop"));
        if backdrop.clicked() {
            self.show_export = false;
            return;
        }

        let today = today_local();
        let presets = [
            (
                "This week",
                format_date_key(start_of_week(today)),
                format_date_key(end_of_week(today)),
            ),
            (
                "Last 7 days",
                format_date_key(today - Duration::days(6)),
                format_date_key(today),
            ),
            (
                "Last 30 days",
                format_date_key(today - Duration::days(29)),
                format_date_key(today),
            ),
            (
                "This month",
                format_date_key(start_of_month(today)),
                format_date_key(end_of_month(today)),
            ),
        ];

        let mut close_requested = false;
        egui::Window::new("export_report_modal")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_size([540.0, 0.0])
            .frame(modal_window_frame(palette))
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                if modal_header(
                    ui,
                    palette,
                    "Export Report",
                    Some("Download a studio-grade PDF (active days only) or Markdown summary"),
                ) {
                    close_requested = true;
                }

                Frame::new()
                    .inner_margin(Margin::symmetric(SPACE_LG as i8, 16))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let half_w = ((ui.available_width() - SPACE_MD) * 0.5).max(120.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_MD;
                            ui.allocate_ui_with_layout(
                                Vec2::new(half_w, 0.0),
                                Layout::top_down(Align::LEFT),
                                |ui| {
                                    ui.label(
                                        rich_bold("Start date", 12.0).color(palette.text_muted),
                                    );
                                    ui.add_space(4.0);
                                    styled_singleline_input(
                                        ui,
                                        palette,
                                        &mut self.export_start,
                                        "YYYY-MM-DD",
                                    );
                                },
                            );
                            ui.allocate_ui_with_layout(
                                Vec2::new(half_w, 0.0),
                                Layout::top_down(Align::LEFT),
                                |ui| {
                                    ui.label(rich_bold("End date", 12.0).color(palette.text_muted));
                                    ui.add_space(4.0);
                                    styled_singleline_input(
                                        ui,
                                        palette,
                                        &mut self.export_end,
                                        "YYYY-MM-DD",
                                    );
                                },
                            );
                        });

                        ui.add_space(10.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(SPACE_SM, 6.0);
                            for (label, start_key, end_key) in &presets {
                                let active = &self.export_start == start_key
                                    && &self.export_end == end_key;
                                if preset_button(ui, palette, label, active).clicked() {
                                    self.export_start = start_key.clone();
                                    self.export_end = end_key.clone();
                                }
                            }
                        });

                        let range = match (
                            parse_date_key(&self.export_start),
                            parse_date_key(&self.export_end),
                        ) {
                            (Some(start), Some(end)) if start <= end => Some((start, end)),
                            _ => None,
                        };

                        if let Some((start, end)) = range {
                            let stats = range_report_stats(&self.items, start, end);
                            let item_word = if stats.total_items == 1 {
                                "item"
                            } else {
                                "items"
                            };
                            let day_word = if stats.active_days == 1 {
                                "day"
                            } else {
                                "days"
                            };
                            ui.add_space(10.0);
                            Frame::new()
                                .fill(palette.surface_muted)
                                .stroke(Stroke::new(1.0, palette.border))
                                .corner_radius(CornerRadius::same(7))
                                .inner_margin(Margin::symmetric(10, 6))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.label(
                                        RichText::new(format!(
                                            "{} {item_word} across {} active {day_word} · PDF omits empty days",
                                            stats.total_items, stats.active_days
                                        ))
                                        .size(12.0)
                                        .color(palette.text_muted),
                                    );
                                });
                        } else if !self.export_start.is_empty() && !self.export_end.is_empty() {
                            ui.add_space(SPACE_SM);
                            ui.label(
                                RichText::new(
                                    "Enter valid YYYY-MM-DD dates with start date on or before end date.",
                                )
                                .size(12.0)
                                .color(palette.danger_fg),
                            );
                        }

                        ui.add_space(SPACE_MD);
                        ui.label(rich_bold("Markdown Preview", 12.0).color(palette.text_muted));
                        ui.add_space(6.0);

                        Frame::new()
                            .fill(palette.background)
                            .stroke(Stroke::new(1.0, palette.border))
                            .corner_radius(CornerRadius::same(8))
                            .inner_margin(Margin::same(12))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ScrollArea::vertical()
                                    .id_salt("export_preview")
                                    .auto_shrink([false, false])
                                    .max_height(200.0)
                                    .show(ui, |ui| {
                                        if let Some((start, end)) = range {
                                            let md =
                                                generate_markdown_for_range(&self.items, start, end);
                                            ui.label(
                                                RichText::new(md)
                                                    .monospace()
                                                    .size(12.0)
                                                    .color(palette.text),
                                            );
                                        } else {
                                            ui.label(
                                                RichText::new(
                                                    "Select a valid date range above to preview the Markdown report.",
                                                )
                                                .size(12.5)
                                                .color(palette.text_muted),
                                            );
                                        }
                                    });
                            });
                    });

                let range = match (
                    parse_date_key(&self.export_start),
                    parse_date_key(&self.export_end),
                ) {
                    (Some(start), Some(end)) if start <= end => Some((start, end)),
                    _ => None,
                };

                horizontal_divider(ui, palette.divider);
                Frame::new()
                    .inner_margin(Margin::symmetric(SPACE_LG as i8, 12))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.set_width(ui.available_width());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.spacing_mut().item_spacing.x = SPACE_SM;
                                let valid = range.is_some();

                                let pdf_label = icon_label_bold(
                                    DOWNLOAD,
                                    "Download PDF",
                                    13.0,
                                    palette.on_primary,
                                );
                                if primary_button_enabled(ui, palette, pdf_label, valid).clicked() {
                                    if let Some((start, end)) = range {
                                        let pdf = generate_pdf_for_range(&self.items, start, end);
                                        let filename = format!(
                                            "report-{}-to-{}.pdf",
                                            self.export_start, self.export_end
                                        );
                                        if let Some(path) = choose_report_file(
                                            "Work Tracker — Export PDF Report",
                                            "PDF",
                                            &["pdf"],
                                            Some(&filename),
                                        ) {
                                            if std::fs::write(&path, &pdf).is_ok() {
                                                let _ = open::that(path);
                                            }
                                        }
                                    }
                                }

                                let md_label = icon_label_bold(
                                    DOWNLOAD,
                                    "Download .md",
                                    13.0,
                                    palette.accent_soft_fg,
                                );
                                if soft_accent_button_enabled(ui, palette, md_label, valid)
                                    .clicked()
                                {
                                    if let Some((start, end)) = range {
                                        let md =
                                            generate_markdown_for_range(&self.items, start, end);
                                        let filename = format!(
                                            "report-{}-to-{}.md",
                                            self.export_start, self.export_end
                                        );
                                        if let Some(path) = choose_report_file(
                                            "Work Tracker — Export Report",
                                            "Markdown",
                                            &["md"],
                                            Some(&filename),
                                        ) {
                                            if std::fs::write(&path, &md).is_ok() {
                                                let _ = open::that(path);
                                            }
                                        }
                                    }
                                }

                                if secondary_button(ui, palette, "Close").clicked() {
                                    close_requested = true;
                                }
                            });
                        });
                    });
            });

        if close_requested {
            self.show_export = false;
        }
    }

    /// Renders the frameless `Import Markdown Report` modal (`import_markdown_modal`) with
    /// scrim/Esc/X dismissal, instant file-load validation, summary telemetry strip, segmented
    /// `Merge (skip duplicates) | Replace date range` mode selector, scrollable item preview, and
    /// explicit disabled CTA clarity when zero importable items are ready.
    ///
    /// Flow & invariants:
    /// - Validates `.md` files immediately on selection via [`parse_markdown_report`] rather than
    ///   waiting until submit, displaying either a clear error callout or a rich item-by-item
    ///   preview with duplicate badges.
    /// - Disables `Import <N> Items` whenever no file is loaded, parsing failed, or `N == 0`
    ///   (either all parsed entries already exist in Merge mode or the report contained zero work
    ///   items), preventing empty or duplicate-only commits.
    /// - Upon confirmation, persists `self.items`, jumps `month_offset` and `pinned_date` to the
    ///   most recent imported date, closes the modal, and surfaces `self.import_banner`.
    fn import_modal(&mut self, ctx: &egui::Context, palette: Palette) {
        let backdrop = paint_modal_backdrop(ctx, palette, egui::Id::new("import_modal_backdrop"));
        if backdrop.clicked() {
            self.show_import = false;
            return;
        }

        let plan_opt = match &self.import_parsed {
            Some(Ok(report)) => Some(compute_import_plan(&self.items, report)),
            _ => None,
        };

        let mut close_requested = false;
        let mut trigger_import = false;
        let mut pick_file_clicked = false;

        egui::Window::new("import_markdown_modal")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_size([560.0, 0.0])
            .frame(modal_window_frame(palette))
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                if modal_header(
                    ui,
                    palette,
                    "Import Markdown Report",
                    Some("Load work items from an exported Work Tracker .md report"),
                ) {
                    close_requested = true;
                }

                Frame::new()
                    .inner_margin(Margin::symmetric(SPACE_LG as i8, 16))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());

                        // 1. File Picker Bar
                        Frame::new()
                            .fill(palette.surface_muted)
                            .stroke(Stroke::new(1.0, palette.border))
                            .corner_radius(CornerRadius::same(8))
                            .inner_margin(Margin::symmetric(12, 10))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.set_width(ui.available_width());
                                    ui.spacing_mut().item_spacing.x = SPACE_SM;

                                    ui.label(
                                        RichText::new(FILE_TEXT)
                                            .size(18.0)
                                            .color(if self.import_file_name.is_some() {
                                                palette.primary
                                            } else {
                                                palette.text_muted
                                            }),
                                    );

                                    let btn_reserve = 162.0;
                                    let text_w = (ui.available_width() - btn_reserve).max(120.0);
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(text_w, 0.0),
                                        Layout::top_down(Align::LEFT),
                                        |ui| {
                                            if let Some(name) = &self.import_file_name {
                                                ui.label(
                                                    rich_bold(name, 13.0).color(palette.text),
                                                );
                                                if let Some(Ok(report)) = &self.import_parsed {
                                                    if let (Some(s), Some(e)) =
                                                        (report.start_date, report.end_date)
                                                    {
                                                        ui.label(
                                                            RichText::new(format!(
                                                                "Range: {} to {}",
                                                                format_date_key(s),
                                                                format_date_key(e)
                                                            ))
                                                            .size(11.5)
                                                            .color(palette.text_muted),
                                                        );
                                                    }
                                                }
                                            } else {
                                                ui.label(
                                                    rich_bold("No file selected", 13.0)
                                                        .color(palette.text_muted),
                                                );
                                                ui.label(
                                                    RichText::new(
                                                        "Choose an exported .md or .markdown file",
                                                    )
                                                    .size(11.5)
                                                    .color(palette.text_faint),
                                                );
                                            }
                                        },
                                    );

                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if secondary_button(
                                            ui,
                                            palette,
                                            icon_label(
                                                FOLDER_OPEN,
                                                "Choose .md File…",
                                                13.0,
                                                palette.text,
                                            ),
                                        )
                                        .clicked()
                                        {
                                            pick_file_clicked = true;
                                        }
                                    });
                                });
                            });

                        ui.add_space(SPACE_MD);

                        // 2. Empty Prompt / Parse Error / Parsed Preview State
                        match (&self.import_parsed, &plan_opt) {
                            (None, _) => {
                                Frame::new()
                                    .fill(palette.background)
                                    .stroke(Stroke::new(1.0, palette.border))
                                    .corner_radius(CornerRadius::same(10))
                                    .inner_margin(Margin::symmetric(SPACE_LG as i8, 22))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.vertical_centered(|ui| {
                                            let (icon_rect, _) = ui.allocate_exact_size(
                                                Vec2::splat(40.0),
                                                Sense::hover(),
                                            );
                                            ui.painter().rect_filled(
                                                icon_rect,
                                                CornerRadius::same(10),
                                                palette.surface,
                                            );
                                            ui.painter().rect_stroke(
                                                icon_rect,
                                                CornerRadius::same(10),
                                                Stroke::new(1.0, palette.border),
                                                StrokeKind::Inside,
                                            );
                                            ui.painter().text(
                                                icon_rect.center(),
                                                Align2::CENTER_CENTER,
                                                UPLOAD_SIMPLE,
                                                FontId::new(19.0, FontFamily::Proportional),
                                                palette.text_muted,
                                            );
                                            ui.add_space(10.0);
                                            ui.label(
                                                rich_bold(
                                                    "Select a Work Tracker Markdown report",
                                                    13.5,
                                                )
                                                .color(palette.text),
                                            );
                                            ui.add_space(4.0);
                                            ui.label(
                                                RichText::new(
                                                    "Parses day sections, categories, descriptions, and starred achievements with duplicate detection before saving.",
                                                )
                                                .size(12.0)
                                                .color(palette.text_muted),
                                            );
                                        });
                                    });
                            }
                            (Some(Err(err_msg)), _) => {
                                Frame::new()
                                    .fill(palette.danger_hover_bg)
                                    .stroke(Stroke::new(
                                        1.0,
                                        palette.danger_fg.gamma_multiply(0.45),
                                    ))
                                    .corner_radius(CornerRadius::same(8))
                                    .inner_margin(Margin::symmetric(14, 12))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = SPACE_SM;
                                            ui.label(
                                                RichText::new(WARNING_CIRCLE)
                                                    .size(16.0)
                                                    .color(palette.danger_fg),
                                            );
                                            ui.label(
                                                rich_bold(
                                                    "Could not import Markdown file",
                                                    13.0,
                                                )
                                                .color(palette.danger_fg),
                                            );
                                        });
                                        ui.add_space(6.0);
                                        ui.label(
                                            RichText::new(err_msg)
                                                .size(12.5)
                                                .color(palette.text),
                                        );
                                    });
                            }
                            (Some(Ok(report)), Some(plan)) => {
                                let total = report.items.len();
                                let item_word = if total == 1 { "item" } else { "items" };
                                let day_word = if report.active_days == 1 {
                                    "day"
                                } else {
                                    "days"
                                };
                                let ach_word = if plan.achievements_count == 1 {
                                    "achievement"
                                } else {
                                    "achievements"
                                };

                                Frame::new()
                                    .fill(palette.surface_muted)
                                    .stroke(Stroke::new(1.0, palette.border))
                                    .corner_radius(CornerRadius::same(7))
                                    .inner_margin(Margin::symmetric(12, 8))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        if total == 0 {
                                            ui.label(
                                                RichText::new(
                                                    "0 work items found in this report (all day sections were empty).",
                                                )
                                                .size(12.0)
                                                .color(palette.text_muted),
                                            );
                                        } else {
                                            let dup_note = if plan.duplicate_count == 0 {
                                                "0 already exist".to_string()
                                            } else {
                                                format!("{} already exist", plan.duplicate_count)
                                            };
                                            ui.label(
                                                RichText::new(format!(
                                                    "{total} {item_word} parsed · {} active {day_word} · {} {ach_word} · {dup_note}",
                                                    report.active_days, plan.achievements_count
                                                ))
                                                .size(12.0)
                                                .color(palette.text_muted),
                                            );
                                        }
                                    });

                                ui.add_space(12.0);
                                ui.label(
                                    rich_bold("Import Mode", 12.0).color(palette.text_muted),
                                );
                                ui.add_space(6.0);

                                Frame::new()
                                    .fill(palette.surface_muted)
                                    .stroke(Stroke::new(1.0, palette.border))
                                    .corner_radius(CornerRadius::same(8))
                                    .inner_margin(Margin::same(3))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = 3.0;
                                            if settings_tab_button(
                                                ui,
                                                palette,
                                                "Merge (skip duplicates)",
                                                self.import_mode
                                                    == ImportMode::MergeSkipDuplicates,
                                            )
                                            .clicked()
                                            {
                                                self.import_mode = ImportMode::MergeSkipDuplicates;
                                            }
                                            if settings_tab_button(
                                                ui,
                                                palette,
                                                "Replace date range",
                                                self.import_mode == ImportMode::ReplaceDateRange,
                                            )
                                            .clicked()
                                            {
                                                self.import_mode = ImportMode::ReplaceDateRange;
                                            }
                                        });
                                    });

                                ui.add_space(6.0);
                                let mode_hint = match self.import_mode {
                                    ImportMode::MergeSkipDuplicates => {
                                        if total > 0 && plan.new_merge_count == 0 {
                                            "All parsed items already exist on their respective dates. Switch to Replace date range to overwrite.".to_string()
                                        } else if plan.duplicate_count > 0 {
                                            format!(
                                                "Adds {} new entries and skips {} matching entries already on the same date.",
                                                plan.new_merge_count, plan.duplicate_count
                                            )
                                        } else {
                                            "Keeps all existing entries and adds new items (skips matching date + title + category).".to_string()
                                        }
                                    }
                                    ImportMode::ReplaceDateRange => match plan.replace_range {
                                        Some((s, e)) => format!(
                                            "Replaces {} existing entries from {} to {}; dates outside this range stay untouched.",
                                            plan.existing_in_replace_range,
                                            format_date_key(s),
                                            format_date_key(e)
                                        ),
                                        None => {
                                            "Replaces existing entries inside the report's date range.".to_string()
                                        }
                                    },
                                };
                                ui.label(
                                    RichText::new(mode_hint)
                                        .size(11.5)
                                        .color(palette.text_muted),
                                );

                                ui.add_space(12.0);
                                ui.label(
                                    rich_bold("Preview", 12.0).color(palette.text_muted),
                                );
                                ui.add_space(6.0);

                                Frame::new()
                                    .fill(palette.background)
                                    .stroke(Stroke::new(1.0, palette.border))
                                    .corner_radius(CornerRadius::same(8))
                                    .inner_margin(Margin::same(10))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ScrollArea::vertical()
                                            .id_salt("import_preview_scroll")
                                            .auto_shrink([false, false])
                                            .max_height(190.0)
                                            .show(ui, |ui| {
                                                ui.set_width(ui.available_width());
                                                if report.items.is_empty() {
                                                    ui.label(
                                                        RichText::new(
                                                            "No work items to preview in this report.",
                                                        )
                                                        .size(12.5)
                                                        .color(palette.text_muted),
                                                    );
                                                } else {
                                                    for (idx, (item, &is_dup)) in report
                                                        .items
                                                        .iter()
                                                        .zip(plan.duplicate_flags.iter())
                                                        .enumerate()
                                                    {
                                                        if idx > 0 {
                                                            ui.add_space(6.0);
                                                        }
                                                        let skipped = self.import_mode
                                                            == ImportMode::MergeSkipDuplicates
                                                            && is_dup;
                                                        Frame::new()
                                                            .fill(palette.surface)
                                                            .stroke(Stroke::new(
                                                                1.0,
                                                                palette.border,
                                                            ))
                                                            .corner_radius(CornerRadius::same(7))
                                                            .inner_margin(Margin::symmetric(10, 8))
                                                            .show(ui, |ui| {
                                                                ui.set_width(ui.available_width());
                                                                ui.horizontal_wrapped(|ui| {
                                                                    ui.spacing_mut()
                                                                        .item_spacing
                                                                        .x = 6.0;
                                                                    ui.label(
                                                                        RichText::new(&item.date)
                                                                            .monospace()
                                                                            .size(11.5)
                                                                            .color(
                                                                                palette.text_muted,
                                                                            ),
                                                                    );
                                                                    category_pill(
                                                                        ui,
                                                                        &item.category,
                                                                        category_label(
                                                                            &item.category,
                                                                        ),
                                                                        palette.is_dark,
                                                                    );
                                                                    if item.is_achievement {
                                                                        let _ = achievement_badge(
                                                                            ui, palette,
                                                                        );
                                                                    }
                                                                    if skipped {
                                                                        status_pill(
                                                                            ui,
                                                                            palette.surface_muted,
                                                                            palette.border_strong,
                                                                            palette.text_muted,
                                                                            "Exists · skip",
                                                                        );
                                                                    }
                                                                });
                                                                ui.add_space(3.0);
                                                                let title_col = if skipped {
                                                                    palette.text_muted
                                                                } else {
                                                                    palette.text
                                                                };
                                                                ui.label(
                                                                    rich_bold(&item.title, 13.0)
                                                                        .color(title_col),
                                                                );
                                                                if !item.description.is_empty() {
                                                                    let first_line = item
                                                                        .description
                                                                        .lines()
                                                                        .next()
                                                                        .unwrap_or_default();
                                                                    ui.label(
                                                                        RichText::new(first_line)
                                                                            .size(11.5)
                                                                            .color(
                                                                                palette.text_muted,
                                                                            ),
                                                                    );
                                                                }
                                                            });
                                                    }
                                                }
                                            });
                                    });
                            }
                            _ => {}
                        }
                    });

                let (import_count, can_import) = match (&self.import_parsed, &plan_opt) {
                    (Some(Ok(report)), Some(plan)) => match self.import_mode {
                        ImportMode::MergeSkipDuplicates => {
                            (plan.new_merge_count, plan.new_merge_count > 0)
                        }
                        ImportMode::ReplaceDateRange => {
                            (report.items.len(), !report.items.is_empty())
                        }
                    },
                    _ => (0, false),
                };

                let cta_caption = match (&self.import_parsed, import_count) {
                    (None | Some(Err(_)), _) => "Import Items".to_string(),
                    (_, 1) => "Import 1 Item".to_string(),
                    (_, n) => format!("Import {n} Items"),
                };

                horizontal_divider(ui, palette.divider);
                Frame::new()
                    .inner_margin(Margin::symmetric(SPACE_LG as i8, 12))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_SM;
                            let cta_label = icon_label_bold(
                                UPLOAD_SIMPLE,
                                &cta_caption,
                                13.0,
                                palette.on_primary,
                            );
                            if primary_button_enabled(ui, palette, cta_label, can_import).clicked()
                            {
                                trigger_import = true;
                            }
                            if secondary_button(ui, palette, "Cancel").clicked() {
                                close_requested = true;
                            }
                        });
                    });
            });

        if pick_file_clicked {
            if let Some(path) = choose_report_file(
                "Work Tracker — Import Markdown Report",
                "Markdown",
                &["md", "markdown"],
                None,
            ) {
                let file_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string());
                let read_res = std::fs::read_to_string(&path)
                    .map_err(|e| format!("Failed to read '{}': {e}", path.display()));
                self.load_import_markdown_content(file_name, read_res);
            }
        }

        if trigger_import {
            self.confirm_markdown_import();
        } else if close_requested {
            self.show_import = false;
        }
    }

    /// Renders the frameless Settings modal with a segmented `Storage | Updates | About | Contact`
    /// tab bar and scrim/Esc/X dismissal.
    fn settings_modal(&mut self, ctx: &egui::Context, palette: Palette) {
        let backdrop =
            paint_modal_backdrop(ctx, palette, egui::Id::new("settings_modal_backdrop"));
        if backdrop.clicked() {
            self.show_settings = false;
            return;
        }

        let mut close_requested = false;
        egui::Window::new("settings_modal_window")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_size([496.0, 0.0])
            .frame(modal_window_frame(palette))
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                if modal_header(
                    ui,
                    palette,
                    "Settings",
                    Some("Local data storage, release checks, and application details"),
                ) {
                    close_requested = true;
                }

                Frame::new()
                    .inner_margin(Margin::symmetric(SPACE_LG as i8, 16))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());

                        Frame::new()
                            .fill(palette.surface_muted)
                            .stroke(Stroke::new(1.0, palette.border))
                            .corner_radius(CornerRadius::same(8))
                            .inner_margin(Margin::same(3))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 3.0;
                                    for (tab, label) in [
                                        (SettingsTab::Storage, "Storage"),
                                        (SettingsTab::Updates, "Updates"),
                                        (SettingsTab::About, "About"),
                                        (SettingsTab::Contact, "Contact"),
                                    ] {
                                        if settings_tab_button(
                                            ui,
                                            palette,
                                            label,
                                            self.settings_tab == tab,
                                        )
                                        .clicked()
                                        {
                                            self.settings_tab = tab;
                                        }
                                    }
                                });
                            });

                        ui.add_space(SPACE_MD);

                        Frame::new()
                            .fill(palette.background)
                            .stroke(Stroke::new(1.0, palette.border))
                            .corner_radius(CornerRadius::same(10))
                            .inner_margin(Margin::same(14))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                match self.settings_tab {
                                    SettingsTab::Storage => {
                                        ui.label(
                                            rich_bold("Local-first JSON storage", 13.5)
                                                .color(palette.text),
                                        );
                                        ui.add_space(4.0);
                                        ui.label(
                                            RichText::new(
                                                "Your work entries and preferences stay strictly on this machine.",
                                            )
                                            .size(12.5)
                                            .color(palette.text_muted),
                                        );
                                        ui.add_space(10.0);
                                        Frame::new()
                                            .fill(palette.surface)
                                            .stroke(Stroke::new(1.0, palette.border))
                                            .corner_radius(CornerRadius::same(7))
                                            .inner_margin(Margin::symmetric(10, 8))
                                            .show(ui, |ui| {
                                                ui.set_width(ui.available_width());
                                                ui.label(
                                                    RichText::new(storage_dir_display())
                                                        .monospace()
                                                        .size(12.0)
                                                        .color(palette.text),
                                                );
                                            });
                                        ui.add_space(6.0);
                                        ui.label(
                                            RichText::new(
                                                "Compatible with ~/.weekly-tracker/data.json & prefs.json.",
                                            )
                                            .size(11.5)
                                            .color(palette.text_faint),
                                        );
                                        ui.add_space(12.0);
                                        horizontal_divider(ui, palette.divider);
                                        ui.add_space(12.0);
                                        ui.label(
                                            rich_bold("Import from Markdown report", 13.0)
                                                .color(palette.text),
                                        );
                                        ui.add_space(4.0);
                                        ui.label(
                                            RichText::new(
                                                "Merge or restore work items from an exported Work Tracker .md file.",
                                            )
                                            .size(12.0)
                                            .color(palette.text_muted),
                                        );
                                        ui.add_space(10.0);
                                        if secondary_button(
                                            ui,
                                            palette,
                                            icon_label(
                                                UPLOAD_SIMPLE,
                                                "Import from Markdown (.md)…",
                                                13.0,
                                                palette.text,
                                            ),
                                        )
                                        .clicked()
                                        {
                                            close_requested = true;
                                            self.open_import_modal();
                                        }
                                    }
                                    SettingsTab::Updates => {
                                        ui.label(
                                            rich_bold(
                                                format!("Installed version · v{APP_VERSION}"),
                                                13.5,
                                            )
                                            .color(palette.text),
                                        );
                                        ui.add_space(8.0);
                                        if ui
                                            .checkbox(
                                                &mut self.prefs.check_updates_on_startup,
                                                "Check for new releases automatically on startup",
                                            )
                                            .changed()
                                        {
                                            self.persist_prefs();
                                        }
                                        ui.add_space(12.0);
                                        ui.horizontal_wrapped(|ui| {
                                            ui.spacing_mut().item_spacing =
                                                Vec2::new(SPACE_SM, SPACE_SM);
                                            let label = if self.update_busy {
                                                "Checking…"
                                            } else {
                                                "Check for updates"
                                            };
                                            if primary_button_enabled(
                                                ui,
                                                palette,
                                                label,
                                                !self.update_busy,
                                            )
                                            .clicked()
                                            {
                                                self.settings_update_notice = None;
                                                self.spawn_update_check(
                                                    ui.ctx(),
                                                    UpdatePollKind::Settings,
                                                );
                                            }
                                            if secondary_button(
                                                ui,
                                                palette,
                                                "View GitHub releases",
                                            )
                                            .clicked()
                                            {
                                                Self::open_github(APP_RELEASES_URL);
                                            }
                                        });
                                        ui.add_space(10.0);
                                        if let Some(notice) = &self.settings_update_notice {
                                            horizontal_divider(ui, palette.divider);
                                            ui.add_space(8.0);
                                            match notice {
                                                UpdateNotice::Error(text) => {
                                                    ui.colored_label(palette.danger_fg, text);
                                                }
                                                UpdateNotice::UpToDate(text)
                                                | UpdateNotice::NoReleases(text) => {
                                                    ui.label(
                                                        RichText::new(text)
                                                            .size(12.5)
                                                            .color(palette.text_muted),
                                                    );
                                                }
                                                UpdateNotice::Newer { text, url } => {
                                                    ui.label(
                                                        rich_bold(text, 12.5)
                                                            .color(palette.accent_soft_fg),
                                                    );
                                                    ui.add_space(6.0);
                                                    if secondary_button(
                                                        ui,
                                                        palette,
                                                        "Open release page",
                                                    )
                                                    .clicked()
                                                    {
                                                        Self::open_github(url);
                                                    }
                                                }
                                            }
                                        } else {
                                            ui.label(
                                                RichText::new(
                                                    "Queries the latest GitHub release tag over HTTPS.",
                                                )
                                                .size(11.5)
                                                .color(palette.text_faint),
                                            );
                                        }
                                    }
                                    SettingsTab::About => {
                                        ui.label(rich_bold(APP_NAME, 15.5).color(palette.text));
                                        ui.add_space(2.0);
                                        ui.label(
                                            RichText::new(APP_TAGLINE)
                                                .size(12.5)
                                                .color(palette.text_muted),
                                        );
                                        ui.add_space(8.0);
                                        ui.label(
                                            RichText::new(
                                                "A fast native Rust + egui desktop logbook for tracking daily engineering work, key achievements, and clean Markdown reports.",
                                            )
                                            .size(12.5)
                                            .color(palette.text),
                                        );
                                        ui.add_space(8.0);
                                        ui.label(
                                            RichText::new(format!(
                                                "Version {APP_VERSION} · {APP_LICENSE}"
                                            ))
                                            .size(11.5)
                                            .color(palette.text_faint),
                                        );
                                    }
                                    SettingsTab::Contact => {
                                        ui.label(
                                            rich_bold("Author & Repository", 13.5)
                                                .color(palette.text),
                                        );
                                        ui.add_space(4.0);
                                        ui.label(
                                            RichText::new(format!("Email: {APP_CONTACT_EMAIL}"))
                                                .size(12.5)
                                                .color(palette.text_muted),
                                        );
                                        ui.add_space(12.0);
                                        if secondary_button(ui, palette, "Open GitHub repository")
                                            .clicked()
                                        {
                                            Self::open_github(APP_REPO_URL);
                                        }
                                    }
                                }
                            });
                    });

                horizontal_divider(ui, palette.divider);
                Frame::new()
                    .inner_margin(Margin::symmetric(SPACE_LG as i8, 12))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if secondary_button(ui, palette, "Close").clicked() {
                                close_requested = true;
                            }
                        });
                    });
            });

        if close_requested {
            self.show_settings = false;
        }
    }

    /// Renders the frameless `Copy to Another Day` modal for duplicating a work item onto a
    /// validated `YYYY-MM-DD` target date (`Today` / `Tomorrow` quick chips, `Enter` submit).
    fn copy_modal(&mut self, ctx: &egui::Context, palette: Palette) {
        let backdrop = paint_modal_backdrop(ctx, palette, egui::Id::new("copy_modal_backdrop"));
        if backdrop.clicked() {
            self.copy_source_id = None;
            return;
        }

        let source_title = self
            .copy_source_id
            .as_ref()
            .and_then(|id| self.items.iter().find(|i| &i.id == id))
            .map(|i| i.title.clone())
            .unwrap_or_default();
        let source_id = self.copy_source_id.clone();

        let today = today_local();
        let tomorrow_key = format_date_key(today + Duration::days(1));
        let today_key = format_date_key(today);

        let mut close_requested = false;
        egui::Window::new("copy_item_modal")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_size([420.0, 0.0])
            .frame(modal_window_frame(palette))
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                if modal_header(
                    ui,
                    palette,
                    "Copy to Another Day",
                    Some("Duplicate this work entry onto a target date"),
                ) {
                    close_requested = true;
                }

                let mut submit_enter = false;
                Frame::new()
                    .inner_margin(Margin::symmetric(SPACE_LG as i8, 16))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());

                        Frame::new()
                            .fill(palette.background)
                            .stroke(Stroke::new(1.0, palette.border))
                            .corner_radius(CornerRadius::same(8))
                            .inner_margin(Margin::symmetric(12, 9))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.label(
                                    RichText::new("Source item")
                                        .size(11.0)
                                        .color(palette.text_faint),
                                );
                                ui.add_space(2.0);
                                ui.label(rich_bold(&source_title, 13.0).color(palette.text));
                            });

                        ui.add_space(SPACE_MD);
                        ui.label(
                            rich_bold("Target date (YYYY-MM-DD)", 12.0).color(palette.text_muted),
                        );
                        ui.add_space(6.0);
                        let input_resp = styled_singleline_input(
                            ui,
                            palette,
                            &mut self.copy_target_date,
                            "YYYY-MM-DD",
                        );
                        if input_resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                            submit_enter = true;
                        }

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_SM;
                            if preset_button(
                                ui,
                                palette,
                                "Today",
                                self.copy_target_date == today_key,
                            )
                            .clicked()
                            {
                                self.copy_target_date = today_key.clone();
                            }
                            if preset_button(
                                ui,
                                palette,
                                "Tomorrow",
                                self.copy_target_date == tomorrow_key,
                            )
                            .clicked()
                            {
                                self.copy_target_date = tomorrow_key.clone();
                            }
                        });
                    });

                let valid_target = parse_date_key(&self.copy_target_date).is_some();
                if submit_enter && valid_target {
                    if let Some(id) = source_id.clone() {
                        let target = self.copy_target_date.clone();
                        self.copy_item_to_date(&id, &target);
                        return;
                    }
                }

                horizontal_divider(ui, palette.divider);
                Frame::new()
                    .inner_margin(Margin::symmetric(SPACE_LG as i8, 12))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_SM;
                            if primary_button_enabled(ui, palette, "Copy Item", valid_target)
                                .clicked()
                            {
                                if let Some(id) = source_id.clone() {
                                    let target = self.copy_target_date.clone();
                                    self.copy_item_to_date(&id, &target);
                                }
                            }
                            if secondary_button(ui, palette, "Cancel").clicked() {
                                close_requested = true;
                            }
                        });
                    });
            });

        if close_requested {
            self.copy_source_id = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::chrome::{
        DEFAULT_WINDOW_HEIGHT, DEFAULT_WINDOW_WIDTH, MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH,
    };

    #[test]
    fn hero_metrics_aggregate_iso_week_viewed_month_and_achievements_accurately() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 11, 18).unwrap(); // Wednesday W47
        let nov = chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        let oct = chrono::NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let now = Utc::now();

        let mk = |id: &str, date: &str, ach: bool| WorkItem {
            id: id.into(),
            date: date.into(),
            title: id.into(),
            description: String::new(),
            category: "development".into(),
            is_achievement: ach,
            created_at: now,
            updated_at: now,
        };

        let items = vec![
            mk("mon-w47", "2026-11-16", true),
            mk("sun-w47", "2026-11-22", false),
            mk("early-nov", "2026-11-03", true),
            mk("oct-win", "2026-10-28", true),
            mk("malformed", "not-a-date", true),
        ];

        let nov_metrics = HeroMetrics::compute(&items, nov, today);
        assert_eq!(nov_metrics.this_week_count, 2);
        assert_eq!(nov_metrics.month_count, 3);
        assert_eq!(nov_metrics.month_achievements, 2);

        let oct_metrics = HeroMetrics::compute(&items, oct, today);
        assert_eq!(oct_metrics.this_week_count, 2);
        assert_eq!(oct_metrics.month_count, 1);
        assert_eq!(oct_metrics.month_achievements, 1);
    }

    /// The launch window is the smallest size that still shows the full hero, six preferred-height
    /// day rows, the widest toolbar (September + Theme · Forest), and the widest day-detail header
    /// (`Wednesday, May 10` + Today + Pinned + pin + Add Item) without those controls overlapping.
    ///
    /// One pixel shorter than [`MIN_WINDOW_HEIGHT`] drops the day rows below [`CAL_DAY_CELL`].y.
    /// [`DETAIL_COLUMN_MIN_WIDTH`] is the ceil of that header row plus the detail frame's 40px of
    /// horizontal margin. One pixel narrower and the pin button overlaps the `Pinned` pill.
    #[test]
    fn launch_window_is_the_smallest_size_that_keeps_the_layout_intact() {
        let ctx = egui::Context::default();
        icons::install_fonts(&ctx);
        let today = today_local();
        let mut app = WeeklyTrackerApp {
            items: Vec::new(),
            month_offset: 0,
            pinned_date: None,
            hover_date: None,
            theme: ThemeId::Default,
            prefs: Prefs::default(),
            show_import: false,
            show_export: false,
            show_settings: false,
            settings_tab: SettingsTab::Storage,
            import_mode: ImportMode::MergeSkipDuplicates,
            import_file_name: None,
            import_parsed: None,
            import_banner: None,
            form_mode: None,
            form_item_id: None,
            form_date: None,
            form_title: String::new(),
            form_description: String::new(),
            form_category: "development".into(),
            form_achievement: false,
            export_start: format_date_key(start_of_month(today)),
            export_end: format_date_key(end_of_month(today)),
            copy_source_id: None,
            copy_target_date: format_date_key(today + Duration::days(1)),
            splash_active: false,
            splash_started: Instant::now(),
            update_rx: None,
            update_poll_kind: None,
            update_busy: false,
            update_banner: None,
            settings_update_notice: None,
        };
        apply_visuals(&ctx, app.theme);
        let p = app.palette();
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 800.0))),
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let left = ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = SPACE_SM;
                    Frame::new()
                        .fill(p.surface_muted)
                        .stroke(Stroke::new(1.0, p.border))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(Margin::same(2))
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            let _ = scrubber_icon_button(ui, p, CARET_LEFT, "Previous month");
                            let _ = scrubber_today_button(ui, p, true);
                            let _ = scrubber_icon_button(ui, p, CARET_RIGHT, "Next month");
                        });
                    ui.add_space(SPACE_XS);
                    ui.label(rich_bold("September 2026", 15.5).color(p.text));
                    status_pill(ui, p.surface_muted, p.border, p.text_muted, "W47");
                });
                let right = ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = SPACE_SM;
                    let _ = ghost_icon_button(ui, p, X, 14.0, "Close");
                    let _ = ghost_icon_button(ui, p, SQUARE, 14.0, "Maximize");
                    let _ = ghost_icon_button(ui, p, MINUS, 14.0, "Minimize");
                    let _ = secondary_button(ui, p, icon_label(EXPORT, "Export", 13.0, p.text));
                    let _ = secondary_button(ui, p, icon_label(UPLOAD_SIMPLE, "Import", 13.0, p.text));
                    let _ = ghost_icon_button(ui, p, GEAR, 16.5, "Settings");
                    vertical_divider(ui, 18.0, p.divider);
                    let _ = theme_swatch_button(ui, p, ThemeId::Forest);
                });
                let needed = left.response.rect.width()
                    + right.response.rect.width()
                    + 32.0
                    + SPACE_SM;
                assert!(
                    needed <= MIN_WINDOW_WIDTH,
                    "widest toolbar needs {needed}px, window is {MIN_WINDOW_WIDTH}px"
                );
                assert_eq!(DEFAULT_WINDOW_WIDTH, MIN_WINDOW_WIDTH);
                assert_eq!(DEFAULT_WINDOW_HEIGHT, MIN_WINDOW_HEIGHT);

                // Same spacing as `day_detail`: the outer row keeps the detail pane's 8px
                // item spacing; the date/pills row and the button row set `SPACE_SM`.
                let outer_gap = ui.spacing().item_spacing.x;
                let date_row = ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = SPACE_SM;
                    ui.label(rich_bold("Wednesday, May 10", 18.0).color(p.text));
                    status_pill(
                        ui,
                        p.accent_soft,
                        p.accent_soft_border,
                        p.accent_soft_fg,
                        "Today",
                    );
                    status_pill(ui, p.surface_muted, p.border_strong, p.text_muted, "Pinned");
                });
                let actions = ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = SPACE_SM;
                    let _ = primary_button(
                        ui,
                        p,
                        icon_label_bold(PLUS, "Add Item", 13.0, p.on_primary),
                    );
                    let _ = toggle_icon_button(ui, p, PUSH_PIN_SLASH, true, "Unpin day");
                });
                let header = date_row.response.rect.width()
                    + outer_gap
                    + actions.response.rect.width();
                // `detail_column_frame` is Margin::symmetric(20, …): 40px of horizontal inset.
                let needed_detail = (header + 40.0).ceil();
                assert_eq!(
                    DETAIL_COLUMN_MIN_WIDTH, needed_detail,
                    "header row is {header}px (date {:.1} + actions {:.1}); pane floor must be {needed_detail}",
                    date_row.response.rect.width(),
                    actions.response.rect.width()
                );

                let copy_btn = ui.horizontal(|ui| {
                    let _ = accent_ghost_button_enabled(ui, p, "Copy to another day…", true);
                });
                let form_actions = ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = SPACE_SM;
                    let _ = primary_button_enabled(ui, p, "Update", true);
                    let _ = secondary_button(ui, p, "Cancel");
                });
                let form_row = copy_btn.response.rect.width()
                    + outer_gap
                    + form_actions.response.rect.width();
                assert!(
                    form_row <= header,
                    "edit-form footer is {form_row}px, wider than the day header {header}px that sizes the pane"
                );
            });
        });
        for h in [DEFAULT_WINDOW_HEIGHT, DEFAULT_WINDOW_HEIGHT - 1.0] {
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, h))),
                ..Default::default()
            };
            let _ = ctx.run(raw, |ctx| {
                let screen_h = h;
                let compact_hero = screen_h < 620.0;
                let ultra_short = screen_h < 540.0;
                let hero_pad_v: i8 = if ultra_short { 6 } else if compact_hero { 8 } else { 10 };
                let cal_pad_v: i8 = if ultra_short { 8 } else if compact_hero { 10 } else { 12 };
                egui::TopBottomPanel::top("mt").frame(toolbar_panel_frame(p)).show(ctx, |ui| app.toolbar(ui, p));
                egui::TopBottomPanel::bottom("mf").show(ctx, |ui| {
                    horizontal_divider(ui, p.border);
                    Frame::new().inner_margin(Margin::symmetric(SPACE_MD as i8, 5)).show(ui, |ui| {
                        ui.label(RichText::new("footer").size(11.0));
                    });
                });
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    Frame::new().inner_margin(Margin::symmetric(12, hero_pad_v)).show(ui, |ui| {
                        app.brand_hero(ui, p, compact_hero);
                    });
                    horizontal_divider(ui, p.border);
                    let split_h = ui.available_height();
                    let inner_h = split_h - (cal_pad_v as f32) * 2.0;
                    let scroll_h = inner_h - 70.0;
                    let cell = cal_day_cell_for_layout(420.0, 6, scroll_h);
                    let _ = (split_h, compact_hero);
                    if (h - DEFAULT_WINDOW_HEIGHT).abs() < 0.5 {
                        assert!(
                            cell.y + 0.05 >= crate::ui::chrome::CAL_DAY_CELL.y,
                            "launch height {h} lays out day rows at {:.2}px, below the preferred {}",
                            cell.y,
                            crate::ui::chrome::CAL_DAY_CELL.y
                        );
                    } else {
                        assert!(
                            cell.y < crate::ui::chrome::CAL_DAY_CELL.y,
                            "height {h} still has full {:.2}px rows, so the window is taller than necessary",
                            cell.y
                        );
                    }
                });
            });
        }
    }

    /// Exercises every major UI view (splash, toolbar, brand hero banner in both standard and
    /// compact viewport heights, calendar, empty & populated day detail, `+ Log Today` action,
    /// item form, export modal, settings tabs, and copy modal) across all six themes inside a
    /// headless `egui::Context` without touching disk storage.
    #[test]
    fn headless_views_hero_and_modals_render_cleanly_across_all_themes() {
        let ctx = egui::Context::default();
        icons::install_fonts(&ctx);
        let today = today_local();
        let today_key = format_date_key(today);

        let mut app = WeeklyTrackerApp {
            items: Vec::new(),
            month_offset: -2,
            pinned_date: None,
            hover_date: None,
            theme: ThemeId::Default,
            prefs: Prefs::default(),
            show_import: false,
            show_export: false,
            show_settings: false,
            settings_tab: SettingsTab::Storage,
            import_mode: ImportMode::MergeSkipDuplicates,
            import_file_name: None,
            import_parsed: None,
            import_banner: None,
            form_mode: None,
            form_item_id: None,
            form_date: None,
            form_title: String::new(),
            form_description: String::new(),
            form_category: "development".into(),
            form_achievement: false,
            export_start: format_date_key(start_of_month(today)),
            export_end: format_date_key(end_of_month(today)),
            copy_source_id: None,
            copy_target_date: format_date_key(today + Duration::days(1)),
            splash_active: true,
            splash_started: Instant::now(),
            update_rx: None,
            update_poll_kind: None,
            update_busy: false,
            update_banner: Some(UpdateBanner {
                title: "v0.2.0".into(),
                url: APP_RELEASES_URL.into(),
            }),
            settings_update_notice: None,
        };

        let raw_input_sized = |w: f32, h: f32| egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(w, h))),
            ..Default::default()
        };

        // 1. Splash overlay frame + `+ Log Today` state transition check.
        apply_visuals(&ctx, app.theme);
        let _ = ctx.run(
            raw_input_sized(DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT),
            |ctx| {
                let p = app.palette();
                app.splash_overlay(ctx, p);
            },
        );
        app.dismiss_splash(&ctx);
        assert!(!app.splash_active);

        app.log_today();
        assert_eq!(app.month_offset, 0);
        assert_eq!(app.pinned_date, Some(today));
        assert!(matches!(app.form_mode, Some(FormMode::Add)));
        app.cancel_form();

        // 2. Empty & populated Brand Hero + toolbar + calendar + day detail across all 6 themes
        // at the launch size and at a 960×600 compact-height viewport.
        for theme in ThemeId::ALL {
            app.theme = theme;
            apply_visuals(&ctx, theme);
            let p = app.palette();

            for &(w, h, compact) in &[
                (DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT, false),
                (960.0, 600.0, true),
            ] {
                let ultra_short = h < 540.0;
                let hero_pad_v: i8 = if ultra_short {
                    6
                } else if compact {
                    8
                } else {
                    10
                };
                let cal_pad_v: i8 = if ultra_short {
                    8
                } else if compact {
                    10
                } else {
                    12
                };

                let _ = ctx.run(raw_input_sized(w, h), |ctx| {
                    egui::TopBottomPanel::top("t")
                        .frame(toolbar_panel_frame(p))
                        .show(ctx, |ui| app.toolbar(ui, p));
                    egui::TopBottomPanel::bottom("f")
                        .frame(Frame::new().fill(p.background).inner_margin(Margin::ZERO))
                        .show(ctx, |ui| {
                            horizontal_divider(ui, p.border);
                            Frame::new()
                                .inner_margin(Margin::symmetric(SPACE_MD as i8, 5))
                                .show(ui, |ui| {
                                    ui.label(RichText::new("footer").size(11.0));
                                });
                        });
                    egui::CentralPanel::default()
                        .frame(Frame::new().fill(p.background).inner_margin(Margin::ZERO))
                        .show(ctx, |ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            Frame::new()
                                .inner_margin(Margin::symmetric(12, hero_pad_v))
                                .show(ui, |ui| app.brand_hero(ui, p, compact));
                            horizontal_divider(ui, p.border);

                            let split_h = ui.available_height();
                            let inner_h = split_h - (cal_pad_v as f32) * 2.0;
                            let scroll_h = inner_h - 70.0;
                            let cell = cal_day_cell_for_layout(420.0, 6, scroll_h);
                            let grid_h = cal_month_grid_content_height(6, cell);
                            assert!(
                                grid_h <= scroll_h + 0.5,
                                "theme {theme:?} at {w}x{h}: 6-row calendar {grid_h} > scroll_h {scroll_h}"
                            );

                            app.month_calendar(ui, p, inner_h);
                            app.day_detail(ui, p);
                        });
                });
            }
        }

        // 3. Populated item card + Edit form + Export + Settings tabs + Copy modal.
        app.items.push(WorkItem {
            id: "item-1".into(),
            date: today_key,
            title: "Redesigned UI architecture".into(),
            description: "Layered neutrals, brand hero banner, and Phosphor iconography.".into(),
            category: "development".into(),
            is_achievement: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        });
        app.show_export = true;
        app.show_settings = true;
        app.copy_source_id = Some("item-1".into());

        let p = app.palette();
        for tab in [
            SettingsTab::Storage,
            SettingsTab::Updates,
            SettingsTab::About,
            SettingsTab::Contact,
        ] {
            app.settings_tab = tab;
            let _ = ctx.run(
                raw_input_sized(DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT),
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        app.brand_hero(ui, p, false);
                        app.day_detail(ui, p);
                        app.open_add_form(today);
                        app.item_form(ui, p);
                        app.cancel_form();
                    });
                    app.export_modal(ctx, p);
                    app.settings_modal(ctx, p);
                    app.copy_modal(ctx, p);
                },
            );
        }

        // 4. Import Markdown modal across empty, error, Merge preview, and Replace preview states.
        app.open_import_modal();
        for theme in [ThemeId::Default, ThemeId::Dark] {
            app.theme = theme;
            apply_visuals(&ctx, theme);
            let p = app.palette();

            app.import_file_name = None;
            app.import_parsed = None;
            let _ = ctx.run(
                raw_input_sized(DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT),
                |ctx| app.import_modal(ctx, p),
            );

            app.load_import_markdown_content("invalid.md".into(), Ok("# Not a report".into()));
            let _ = ctx.run(
                raw_input_sized(DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT),
                |ctx| app.import_modal(ctx, p),
            );

            let valid_md = generate_markdown_for_range(&app.items, today, today);
            app.load_import_markdown_content("report.md".into(), Ok(valid_md));
            for mode in [ImportMode::MergeSkipDuplicates, ImportMode::ReplaceDateRange] {
                app.import_mode = mode;
                let _ = ctx.run(
                    raw_input_sized(DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT),
                    |ctx| app.import_modal(ctx, p),
                );
            }
        }
    }

    /// Verifies that `MergeSkipDuplicates` skips entries matching `(date, case-insensitive trimmed
    /// title, category)` while appending genuinely new entries, and that `ReplaceDateRange`
    /// replaces entries strictly inside `start_date..=end_date` while leaving outside dates intact.
    #[test]
    fn markdown_import_merge_skips_duplicates_and_replace_scopes_to_range() {
        let now = Utc::now();
        let mk = |id: &str, date: &str, title: &str, cat: &str, ach: bool| WorkItem {
            id: id.into(),
            date: date.into(),
            title: title.into(),
            description: String::new(),
            category: cat.into(),
            is_achievement: ach,
            created_at: now,
            updated_at: now,
        };

        let mut existing = vec![
            mk("keep-oct", "2026-10-31", "October prep", "planning", false),
            mk("dup-nov", "2026-11-17", "Ship Feature", "development", true),
            mk("old-nov", "2026-11-18", "Temporary draft", "other", false),
        ];

        let md = "\
# Report: Nov 17 - Nov 19, 2026

## Work Items

### Tuesday, Nov 17

-   ship feature   *(Development)* ⭐
- Code review pass *(Code Review)*

### Wednesday, Nov 18

*(No items)*

### Thursday, Nov 19

- Release v0.2.0 - Tagged & published *(DevOps)* ⭐
";
        let report = parse_markdown_report(md).expect("report parses");
        let plan = compute_import_plan(&existing, &report);
        assert_eq!(plan.duplicate_count, 1);
        assert_eq!(plan.new_merge_count, 2);
        assert_eq!(plan.existing_in_replace_range, 2);

        // 1. Merge mode skips "ship feature" duplicate on Nov 17, keeps "Temporary draft" on Nov 18,
        // and imports the 2 new entries (latest date = 2026-11-19).
        let mut merged = existing.clone();
        let merge_outcome =
            apply_markdown_import(&mut merged, &report, ImportMode::MergeSkipDuplicates);
        assert_eq!(merge_outcome.imported_count, 2);
        assert_eq!(
            merge_outcome.latest_date,
            chrono::NaiveDate::from_ymd_opt(2026, 11, 19)
        );
        assert_eq!(merged.len(), 5);
        assert!(merged.iter().any(|i| i.id == "old-nov"));

        // 2. ReplaceDateRange removes "dup-nov" & "old-nov" inside Nov 17..=Nov 19, preserves
        // "keep-oct" (Oct 31) outside the range, and inserts all 3 report items.
        let replace_outcome =
            apply_markdown_import(&mut existing, &report, ImportMode::ReplaceDateRange);
        assert_eq!(replace_outcome.imported_count, 3);
        assert_eq!(
            replace_outcome.latest_date,
            chrono::NaiveDate::from_ymd_opt(2026, 11, 19)
        );
        assert_eq!(existing.len(), 4);
        assert!(existing.iter().any(|i| i.id == "keep-oct"));
        assert!(!existing.iter().any(|i| i.id == "old-nov"));
    }

    /// Locks the Linux file chooser to zenity. Opening the XDG portal from `rfd` segfaults in
    /// zbus (`PropertiesCache::update_cache`) before rfd can fall back, so a regression that
    /// puts `--file-selection` behind the portal would crash import again.
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_report_dialog_uses_zenity_not_the_xdg_portal() {
        let open = report_file_dialog_command(
            "Work Tracker — Import Markdown Report",
            "Markdown",
            &["md", "markdown"],
            None,
        );
        assert_eq!(open.get_program().to_string_lossy(), "zenity");
        let open_args: Vec<String> = open
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(open_args.iter().any(|a| a == "--file-selection"));
        assert!(open_args.iter().any(|a| a == "--no-markup"));
        assert!(open_args
            .iter()
            .any(|a| a == "Markdown | *.md *.markdown"));
        assert!(!open_args.iter().any(|a| a == "--save"));

        let save = report_file_dialog_command(
            "Work Tracker — Export Report",
            "Markdown",
            &["md"],
            Some("report-2026-11-01-to-2026-11-30.md"),
        );
        let save_args: Vec<String> = save
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(save_args
            .windows(2)
            .any(|w| w[0] == "--save" && w[1] == "--confirm-overwrite"));
        assert!(save_args
            .iter()
            .any(|a| a == "report-2026-11-01-to-2026-11-30.md"));
        assert!(save_args.iter().any(|a| a == "Markdown | *.md"));
    }
}