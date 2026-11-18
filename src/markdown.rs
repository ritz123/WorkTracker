//! Markdown report generator and parser for portable `.md` work-item backups and reports.
//!
//! Format contract (shared between the Rust desktop app and the upstream Electron app):
//! ```markdown
//! # Report: Nov 1 - Nov 30, 2026
//!
//! ## Achievements
//!
//! - **Ship feature** - Details *(Development)*
//!
//! ## Work Items
//!
//! ### Sunday, Nov 1
//!
//! *(No items)*
//!
//! ### Tuesday, Nov 17
//!
//! - Ship feature - Details *(Development)* ⭐
//! ```
//!
//! Architectural & domain invariants:
//! 1. **Single source of truth on import**: [`generate_markdown_for_range`] emits starred wins in
//!    *both* `## Achievements` and `## Work Items` (with a trailing `⭐`). [`parse_markdown_report`]
//!    ignores bullets inside `## Achievements` and reconstructs items strictly from `### <Day>`
//!    subsections so achievements are never double-imported.
//! 2. **Cross-year & multi-format date fidelity**: Resolves `# Report:` headers across year
//!    boundaries (e.g. `# Report: Dec 28 - Jan 3, 2027` maps December dates to `2026` and January
//!    dates to `2027`), explicit per-side years, en/em dashes, and ISO `YYYY-MM-DD` ranges, while
//!    supporting `### Weekday, Mon D`, `### Weekday, Mon D, YYYY`, and `### YYYY-MM-DD` day headers.
//! 3. **Electron & Rust category normalization**: Trailing `*(<Category>)*` tags match [`CATEGORIES`]
//!    case-insensitively against both canonical IDs (`"code-review"`) and display labels
//!    (`"Code Review"`), falling back to `"other"` for unrecognized or omitted tags.

use std::collections::BTreeSet;

use chrono::{Datelike, NaiveDate, Utc};
use uuid::Uuid;

use crate::categories::{category_label, CATEGORIES};
use crate::dates::{format_date_key, format_day_full, parse_date_key, today_local};
use crate::models::WorkItem;

/// Structured result of parsing an exported Work Tracker Markdown (`.md`) report.
#[derive(Debug, Clone)]
pub struct ParsedMarkdownReport {
    /// Inclusive start date of the report (`# Report:` start or earliest `### <Day>` heading).
    pub start_date: Option<NaiveDate>,
    /// Inclusive end date of the report (`# Report:` end or latest `### <Day>` heading).
    pub end_date: Option<NaiveDate>,
    /// Reconstructed work items with fresh UUIDv4 identifiers and UTC timestamps.
    pub items: Vec<WorkItem>,
    /// Count of distinct calendar dates in [`Self::items`] that have at least one work item.
    pub active_days: usize,
}

fn each_day_inclusive(start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
    let mut days = Vec::new();
    let mut cursor = start;
    while cursor <= end {
        days.push(cursor);
        cursor += chrono::Duration::days(1);
    }
    days
}

/// Serializes `items` falling within `start..=end` into the canonical Work Tracker Markdown report.
///
/// Emits a `# Report:` range title, an optional `## Achievements` highlight list when at least one
/// starred item falls inside the range, and a `## Work Items` section with a `### Weekday, Mon D`
/// heading for every calendar day in `start..=end` (`*(No items)*` on empty days).
pub fn generate_markdown_for_range(items: &[WorkItem], start: NaiveDate, end: NaiveDate) -> String {
    let days = each_day_inclusive(start, end);
    let range_label = format!(
        "{} - {}",
        start.format("%b %-d"),
        end.format("%b %-d, %Y")
    );
    let start_key = format_date_key(start);
    let end_key = format_date_key(end);

    let mut lines = Vec::new();
    lines.push(format!("# Report: {range_label}"));
    lines.push(String::new());

    let achievements: Vec<_> = items
        .iter()
        .filter(|item| item.is_achievement && item.date >= start_key && item.date <= end_key)
        .collect();

    if !achievements.is_empty() {
        lines.push("## Achievements".into());
        lines.push(String::new());
        for a in achievements {
            let desc = if a.description.is_empty() {
                String::new()
            } else {
                format!(" - {}", a.description)
            };
            lines.push(format!(
                "- **{}**{} *({})*",
                a.title,
                desc,
                category_label(&a.category)
            ));
        }
        lines.push(String::new());
    }

    lines.push("## Work Items".into());
    lines.push(String::new());

    for day in days {
        let day_key = format_date_key(day);
        let day_items: Vec<_> = items.iter().filter(|i| i.date == day_key).collect();
        lines.push(format!("### {}", format_day_full(day)));
        lines.push(String::new());
        if day_items.is_empty() {
            lines.push("*(No items)*".into());
        } else {
            for item in day_items {
                let desc = if item.description.is_empty() {
                    String::new()
                } else {
                    format!(" - {}", item.description)
                };
                let star = if item.is_achievement { " ⭐" } else { "" };
                lines.push(format!(
                    "- {}{}{} *({})*{}",
                    item.title,
                    desc,
                    "",
                    category_label(&item.category),
                    star
                ));
            }
        }
        lines.push(String::new());
    }

    lines.join("\n")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReportSection {
    Preamble,
    Achievements,
    WorkItems,
    Other,
}

/// Parses an exported Work Tracker Markdown (`.md`) report back into [`WorkItem`] entries.
///
/// Responsibility & flow:
/// - Validates that `md` is non-empty and contains at least one recognizable `### <Day>` section;
///   reports where every day contains `*(No items)*` succeed with `items: vec![]` and
///   `active_days: 0` so the import modal can distinguish an empty valid report from a non-report
///   file.
/// - Parses `# Report: <start> - <end>` when present to anchor year inference (including Dec -> Jan
///   cross-year ranges), falling back to `today_local().year()` if `# Report:` was omitted.
/// - Skips `## Achievements` summary bullets to prevent duplicating starred entries that also
///   appear under their respective `### <Day>` heading in `## Work Items`.
/// - Supports multi-line and multi-paragraph item descriptions by accumulating continuation lines
///   until the next bullet, heading, or `*(No items)*` sentinel.
pub fn parse_markdown_report(md: &str) -> Result<ParsedMarkdownReport, String> {
    parse_markdown_report_with_fallback_year(md, today_local().year())
}

fn parse_markdown_report_with_fallback_year(
    md: &str,
    fallback_year: i32,
) -> Result<ParsedMarkdownReport, String> {
    let md = md.strip_prefix('\u{FEFF}').unwrap_or(md);
    if md.trim().is_empty() {
        return Err(
            "The selected Markdown file is empty. Choose an exported Work Tracker .md report."
                .into(),
        );
    }

    let mut header_range: Option<(NaiveDate, NaiveDate)> = None;
    let mut section = ReportSection::Preamble;
    let mut current_day: Option<NaiveDate> = None;
    let mut min_day_seen: Option<NaiveDate> = None;
    let mut max_day_seen: Option<NaiveDate> = None;
    let mut recognized_day_sections: usize = 0;

    let mut items: Vec<WorkItem> = Vec::new();
    let mut pending_bullet: Option<(NaiveDate, String)> = None;
    let mut pending_blank_lines: usize = 0;
    let now = Utc::now();

    let flush_pending = |pending: &mut Option<(NaiveDate, String)>,
                         pending_blanks: &mut usize,
                         out: &mut Vec<WorkItem>| {
        *pending_blanks = 0;
        if let Some((day, raw_body)) = pending.take() {
            if let Some(item) = build_work_item_from_bullet(day, &raw_body, now) {
                out.push(item);
            }
        }
    };

    for line in md.lines() {
        let trimmed = line.trim();

        // Top-level `# ...` heading (e.g. `# Report: Nov 1 - Nov 30, 2026`).
        if let Some(h1) = trimmed.strip_prefix("# ") {
            flush_pending(&mut pending_bullet, &mut pending_blank_lines, &mut items);
            current_day = None;
            let h1_trim = h1.trim();
            if h1_trim.len() >= 7 && h1_trim[..7].eq_ignore_ascii_case("report:") {
                if let Some(range) = parse_report_header_range(h1_trim[7..].trim()) {
                    header_range = Some(range);
                }
            }
            continue;
        }

        // Second-level `## ...` section heading (`## Achievements`, `## Work Items`, etc.).
        if let Some(h2) = trimmed.strip_prefix("## ") {
            flush_pending(&mut pending_bullet, &mut pending_blank_lines, &mut items);
            current_day = None;
            let h2_trim = h2.trim();
            section = if h2_trim.eq_ignore_ascii_case("achievements") {
                ReportSection::Achievements
            } else if h2_trim.eq_ignore_ascii_case("work items") {
                ReportSection::WorkItems
            } else {
                ReportSection::Other
            };
            continue;
        }

        // Third-level `### <Day>` heading (`### Tuesday, Nov 17`, `### 2026-11-17`, etc.).
        if let Some(h3) = trimmed.strip_prefix("### ") {
            flush_pending(&mut pending_bullet, &mut pending_blank_lines, &mut items);
            if section == ReportSection::Achievements {
                current_day = None;
                continue;
            }
            if let Some(day) = resolve_day_heading(h3.trim(), header_range, fallback_year) {
                current_day = Some(day);
                recognized_day_sections += 1;
                min_day_seen = Some(min_day_seen.map_or(day, |prev| prev.min(day)));
                max_day_seen = Some(max_day_seen.map_or(day, |prev| prev.max(day)));
            } else {
                current_day = None;
            }
            continue;
        }

        // Ignore any other ATX headings (`#### ...`).
        if trimmed.starts_with('#') {
            flush_pending(&mut pending_bullet, &mut pending_blank_lines, &mut items);
            continue;
        }

        if section == ReportSection::Achievements {
            continue;
        }

        let Some(day) = current_day else {
            continue;
        };

        if trimmed.is_empty() {
            if let Some((_, ref buf)) = pending_bullet {
                if bullet_has_closing_metadata(buf) {
                    flush_pending(&mut pending_bullet, &mut pending_blank_lines, &mut items);
                } else {
                    pending_blank_lines += 1;
                }
            }
            continue;
        }

        if trimmed.eq_ignore_ascii_case("*(no items)*")
            || trimmed.eq_ignore_ascii_case("(no items)")
            || trimmed.eq_ignore_ascii_case("_no items_")
        {
            flush_pending(&mut pending_bullet, &mut pending_blank_lines, &mut items);
            continue;
        }

        // New work-item bullet (`- ...` or `* ...` with mandatory space so `*(Category)*` on a
        // continuation line is never misread as a new list bullet).
        let trimmed_start = line.trim_start();
        if let Some(bullet_rest) = trimmed_start
            .strip_prefix("- ")
            .or_else(|| trimmed_start.strip_prefix("* "))
        {
            flush_pending(&mut pending_bullet, &mut pending_blank_lines, &mut items);
            pending_bullet = Some((day, bullet_rest.to_string()));
            continue;
        }

        // Continuation line belonging to an in-progress multi-line work-item description.
        if let Some((_, ref mut buf)) = pending_bullet {
            for _ in 0..=pending_blank_lines {
                buf.push('\n');
            }
            pending_blank_lines = 0;
            buf.push_str(line);
        }
    }

    flush_pending(&mut pending_bullet, &mut pending_blank_lines, &mut items);

    if recognized_day_sections == 0 {
        return Err(
            "No recognizable day sections (such as '### Tuesday, Nov 17' or '### 2026-11-17') were found in this file."
                .into(),
        );
    }

    let (start_date, end_date) = match (header_range, min_day_seen, max_day_seen) {
        (Some((hs, he)), Some(min_d), Some(max_d)) => (Some(hs.min(min_d)), Some(he.max(max_d))),
        (Some((hs, he)), _, _) => (Some(hs), Some(he)),
        (None, min_d, max_d) => (min_d, max_d),
    };

    let active_days = items
        .iter()
        .map(|i| i.date.as_str())
        .collect::<BTreeSet<_>>()
        .len();

    Ok(ParsedMarkdownReport {
        start_date,
        end_date,
        items,
        active_days,
    })
}

/// Checks whether a buffered bullet already ends with `*(<Category>)*` or an achievement star,
/// indicating that a subsequent blank line marks the end of the item rather than a paragraph break
/// inside an unclosed multi-paragraph description.
fn bullet_has_closing_metadata(raw: &str) -> bool {
    let trimmed = strip_trailing_stars(raw.trim()).0;
    if let Some(without_close) = trimmed.strip_suffix(")*") {
        if let Some(open_idx) = without_close.rfind("*(") {
            let cat = &without_close[open_idx + 2..];
            return !cat.is_empty() && !cat.contains('\n');
        }
    }
    false
}

/// Strips trailing achievement star glyphs (`⭐` `\u{2B50}`, optional `\u{FE0F}` variation
/// selector, or `★` `\u{2605}`) and surrounding horizontal/trailing whitespace.
fn strip_trailing_stars(mut s: &str) -> (&str, bool) {
    let mut starred = false;
    loop {
        let no_vs = s.trim_end().trim_end_matches('\u{FE0F}');
        if let Some(rest) = no_vs
            .strip_suffix('\u{2B50}')
            .or_else(|| no_vs.strip_suffix('\u{2605}'))
        {
            starred = true;
            s = rest.trim_end();
        } else {
            break;
        }
    }
    (s, starred)
}

/// Parses a single accumulated bullet body (`"<Title> [- <Description>] [*(<Category>)*] [⭐]"`)
/// into a [`WorkItem`] on `day`.
///
/// Why first-line `" - "` splitting is used:
/// - `WorkItem.title` is always single-line, whereas `description` may span multiple lines and
///   itself contain `" - "` substrings on later lines. Splitting `" - "` only on the first line
///   guarantees `title` never absorbs embedded newlines when a hand-edited multi-line bullet omits
///   an inline `" - "` on line 1.
fn build_work_item_from_bullet(
    day: NaiveDate,
    raw_body: &str,
    timestamp: chrono::DateTime<Utc>,
) -> Option<WorkItem> {
    let (mut rest, is_achievement) = strip_trailing_stars(raw_body.trim());

    let mut category = "other".to_string();
    if let Some(without_close) = rest.strip_suffix(")*") {
        if let Some(open_idx) = without_close.rfind("*(") {
            let candidate_cat = without_close[open_idx + 2..].trim();
            if !candidate_cat.is_empty() && !candidate_cat.contains('\n') {
                category = normalize_category(candidate_cat);
                rest = without_close[..open_idx].trim_end();
            }
        }
    }

    let (first_line, tail_lines) = match rest.split_once('\n') {
        Some((first, tail)) => (first, Some(tail)),
        None => (rest, None),
    };

    let (title, description) = if let Some((t, d_first)) = first_line.split_once(" - ") {
        let desc = match tail_lines {
            Some(tail) => format!("{d_first}\n{tail}").trim().to_string(),
            None => d_first.trim().to_string(),
        };
        (t.trim().to_string(), desc)
    } else {
        let desc = tail_lines
            .map(|tail| tail.trim().to_string())
            .unwrap_or_default();
        (first_line.trim().to_string(), desc)
    };

    if title.is_empty() {
        return None;
    }

    Some(WorkItem {
        id: Uuid::new_v4().to_string(),
        date: format_date_key(day),
        title,
        description,
        category,
        is_achievement,
        created_at: timestamp,
        updated_at: timestamp,
    })
}

/// Maps a category string from `*(<Category>)*` case-insensitively against both [`CATEGORIES`]
/// `id` (Electron exports, e.g. `"code-review"`) and `label` (Rust exports, e.g. `"Code Review"`),
/// falling back to `"other"` for custom or unknown names.
pub fn normalize_category(raw: &str) -> String {
    let trimmed = raw.trim();
    for cat in CATEGORIES {
        if cat.id.eq_ignore_ascii_case(trimmed) || cat.label.eq_ignore_ascii_case(trimmed) {
            return cat.id.to_string();
        }
    }
    "other".to_string()
}

/// Parses `# Report: <left> - <right>` range headers into `(start_date, end_date)`.
///
/// Handles:
/// - Same-year ranges (`"Nov 1 - Nov 30, 2026"`)
/// - Cross-year ranges (`"Dec 28 - Jan 3, 2027"` -> `2026-12-28..=2027-01-03`)
/// - Explicit start & end years (`"Dec 28, 2026 - Jan 3, 2027"`)
/// - En-dash (`–`), em-dash (`—`), `" to "`, and ISO `YYYY-MM-DD` date endpoints.
fn parse_report_header_range(s: &str) -> Option<(NaiveDate, NaiveDate)> {
    let s = s.trim();
    if let Some(single) = parse_date_key(s) {
        return Some((single, single));
    }

    let split = s
        .split_once(" - ")
        .or_else(|| s.split_once(" \u{2013} "))
        .or_else(|| s.split_once(" \u{2014} "))
        .or_else(|| s.split_once(" to "))
        .or_else(|| s.split_once('\u{2013}'))
        .or_else(|| s.split_once('\u{2014}'))
        .or_else(|| s.split_once('-'));

    let (left, right) = split?;
    let (end_m, end_d, end_y_opt) = parse_month_day_opt_year(right.trim())?;
    let end_y = end_y_opt?;
    let end_date = NaiveDate::from_ymd_opt(end_y, end_m, end_d)?;

    let (start_m, start_d, start_y_opt) = parse_month_day_opt_year(left.trim())?;
    let start_y = start_y_opt.unwrap_or_else(|| {
        if (start_m, start_d) > (end_m, end_d) {
            end_y - 1
        } else {
            end_y
        }
    });
    let start_date = NaiveDate::from_ymd_opt(start_y, start_m, start_d)?;

    (start_date <= end_date).then_some((start_date, end_date))
}

/// Resolves a `### <Day Heading>` string (`"Tuesday, Nov 17"`, `"Tuesday, Nov 17, 2026"`, or
/// `"2026-11-17"`) to a [`NaiveDate`] using `header_range` when year is omitted.
fn resolve_day_heading(
    heading: &str,
    header_range: Option<(NaiveDate, NaiveDate)>,
    fallback_year: i32,
) -> Option<NaiveDate> {
    let (month, day, explicit_year, weekday_hint) = parse_day_heading_parts(heading)?;
    if let Some(y) = explicit_year {
        return NaiveDate::from_ymd_opt(y, month, day);
    }

    if let Some((range_start, range_end)) = header_range {
        let mut in_range_matches = Vec::new();
        for y in range_start.year()..=range_end.year() {
            if let Some(d) = NaiveDate::from_ymd_opt(y, month, day) {
                if d >= range_start && d <= range_end {
                    in_range_matches.push(d);
                }
            }
        }
        if let Some(wd) = weekday_hint {
            if let Some(&exact) = in_range_matches.iter().find(|d| d.weekday() == wd) {
                return Some(exact);
            }
        }
        if let Some(&first) = in_range_matches.first() {
            return Some(first);
        }

        let inferred_year = if (month, day) > (range_end.month(), range_end.day())
            && range_start.year() < range_end.year()
        {
            range_start.year()
        } else {
            range_end.year()
        };
        return NaiveDate::from_ymd_opt(inferred_year, month, day);
    }

    NaiveDate::from_ymd_opt(fallback_year, month, day)
}

fn parse_month_day_opt_year(s: &str) -> Option<(u32, u32, Option<i32>)> {
    let (m, d, y, _) = parse_day_heading_parts(s)?;
    Some((m, d, y))
}

fn parse_day_heading_parts(
    s: &str,
) -> Option<(u32, u32, Option<i32>, Option<chrono::Weekday>)> {
    let mut s = s.trim();
    if let Some(iso) = parse_date_key(s) {
        return Some((iso.month(), iso.day(), Some(iso.year()), Some(iso.weekday())));
    }

    let mut weekday_hint = None;
    if let Some((maybe_wd, after_comma)) = s.split_once(',') {
        if let Some(wd) = parse_weekday_name(maybe_wd.trim()) {
            weekday_hint = Some(wd);
            s = after_comma.trim();
            if let Some(iso) = parse_date_key(s) {
                return Some((iso.month(), iso.day(), Some(iso.year()), Some(iso.weekday())));
            }
        }
    }

    let cleaned = s.replace(',', " ");
    let mut tokens = cleaned.split_whitespace();
    let first = tokens.next()?;
    let (month_str, day_str) = if let Some(wd) = parse_weekday_name(first) {
        weekday_hint = Some(wd);
        (tokens.next()?, tokens.next()?)
    } else {
        (first, tokens.next()?)
    };

    let month = parse_month_name(month_str)?;
    let day = strip_ordinal_suffix(day_str).parse::<u32>().ok()?;
    let year = match tokens.next() {
        Some(y_tok) => Some(y_tok.parse::<i32>().ok()?),
        None => None,
    };
    if tokens.next().is_some() {
        return None;
    }

    Some((month, day, year, weekday_hint))
}

fn strip_ordinal_suffix(s: &str) -> &str {
    let lower = s.to_ascii_lowercase();
    for suffix in ["st", "nd", "rd", "th"] {
        if lower.ends_with(suffix) && s.len() > suffix.len() {
            let prefix = &s[..s.len() - suffix.len()];
            if prefix.chars().all(|c| c.is_ascii_digit()) {
                return prefix;
            }
        }
    }
    s
}

fn parse_weekday_name(s: &str) -> Option<chrono::Weekday> {
    use chrono::Weekday::*;
    match s.to_ascii_lowercase().as_str() {
        "mon" | "monday" => Some(Mon),
        "tue" | "tues" | "tuesday" => Some(Tue),
        "wed" | "wednesday" => Some(Wed),
        "thu" | "thur" | "thurs" | "thursday" => Some(Thu),
        "fri" | "friday" => Some(Fri),
        "sat" | "saturday" => Some(Sat),
        "sun" | "sunday" => Some(Sun),
        _ => None,
    }
}

fn parse_month_name(s: &str) -> Option<u32> {
    match s.trim_end_matches('.').to_ascii_lowercase().as_str() {
        "jan" | "january" => Some(1),
        "feb" | "february" => Some(2),
        "mar" | "march" => Some(3),
        "apr" | "april" => Some(4),
        "may" => Some(5),
        "jun" | "june" => Some(6),
        "jul" | "july" => Some(7),
        "aug" | "august" => Some(8),
        "sep" | "sept" | "september" => Some(9),
        "oct" | "october" => Some(10),
        "nov" | "november" => Some(11),
        "dec" | "december" => Some(12),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_includes_achievements_section() {
        let day = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let item = WorkItem {
            id: "1".into(),
            date: format_date_key(day),
            title: "Ship feature".into(),
            description: String::new(),
            category: "development".into(),
            is_achievement: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let md = generate_markdown_for_range(&[item], day, day);
        assert!(md.contains("## Achievements"));
        assert!(md.contains("Ship feature"));
    }

    /// Verifies lossless round-trip export -> import across multiple days, preserving `date`,
    /// `title`, `description` (including `" - "` inside descriptions and multi-line notes),
    /// `category`, and `is_achievement`, while ignoring empty `*(No items)*` days and never
    /// double-importing bullets from `## Achievements`.
    #[test]
    fn round_trip_export_and_parse_preserves_all_fields_without_achievement_duplication() {
        let start = NaiveDate::from_ymd_opt(2026, 11, 15).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 11, 18).unwrap();
        let now = Utc::now();

        let mk = |date: &str, title: &str, desc: &str, cat: &str, ach: bool| WorkItem {
            id: Uuid::new_v4().to_string(),
            date: date.into(),
            title: title.into(),
            description: desc.into(),
            category: cat.into(),
            is_achievement: ach,
            created_at: now,
            updated_at: now,
        };

        let original = vec![
            mk(
                "2026-11-15",
                "Ship vector PDF exporter",
                "Zero-dependency A4 layout - active days only",
                "development",
                true,
            ),
            mk(
                "2026-11-17",
                "Review storage PR",
                "",
                "code-review",
                false,
            ),
            mk(
                "2026-11-17",
                "Architecture sync",
                "Agreed on schema\nAction items assigned",
                "meetings",
                false,
            ),
            mk(
                "2026-11-18",
                "Automate release build",
                "Cut CI build time by 42%",
                "devops",
                true,
            ),
        ];

        let md = generate_markdown_for_range(&original, start, end);
        assert!(md.contains("## Achievements"));
        assert!(md.contains("*(No items)*")); // Nov 16 has no items

        let parsed = parse_markdown_report(&md).expect("exported markdown must parse");
        assert_eq!(parsed.start_date, Some(start));
        assert_eq!(parsed.end_date, Some(end));
        assert_eq!(parsed.active_days, 3);
        assert_eq!(parsed.items.len(), original.len());

        for (got, want) in parsed.items.iter().zip(original.iter()) {
            assert_ne!(got.id, want.id, "imported items must receive fresh UUIDs");
            assert_eq!(got.date, want.date);
            assert_eq!(got.title, want.title);
            assert_eq!(got.description, want.description);
            assert_eq!(got.category, want.category);
            assert_eq!(got.is_achievement, want.is_achievement);
        }
    }

    /// Verifies compatibility with upstream Electron `.md` exports (lowercase category IDs like
    /// `*(code-review)*`), cross-year `# Report: Dec 30 - Jan 2, 2027` headers, `★` stars, and
    /// unknown category fallback to `"other"`.
    #[test]
    fn parses_electron_category_ids_and_cross_year_date_ranges() {
        let electron_md = "\
# Report: Dec 30 - Jan 2, 2027

## Achievements

- **Year-end rollout** - Zero downtime *(devops)*

## Work Items

### Wednesday, Dec 30

- Year-end rollout - Zero downtime *(devops)* ⭐
- PR triage *(code-review)*

### Thursday, Dec 31

*(No items)*

### Friday, Jan 1

- On-call handoff *(Custom Category)* ★

### Saturday, Jan 2

*(No items)*
";
        let parsed = parse_markdown_report(electron_md).expect("electron cross-year report parses");
        assert_eq!(
            parsed.start_date,
            NaiveDate::from_ymd_opt(2026, 12, 30)
        );
        assert_eq!(
            parsed.end_date,
            NaiveDate::from_ymd_opt(2027, 1, 2)
        );
        assert_eq!(parsed.active_days, 2);
        assert_eq!(parsed.items.len(), 3);

        assert_eq!(parsed.items[0].date, "2026-12-30");
        assert_eq!(parsed.items[0].title, "Year-end rollout");
        assert_eq!(parsed.items[0].description, "Zero downtime");
        assert_eq!(parsed.items[0].category, "devops");
        assert!(parsed.items[0].is_achievement);

        assert_eq!(parsed.items[1].date, "2026-12-30");
        assert_eq!(parsed.items[1].title, "PR triage");
        assert_eq!(parsed.items[1].description, "");
        assert_eq!(parsed.items[1].category, "code-review");
        assert!(!parsed.items[1].is_achievement);

        assert_eq!(parsed.items[2].date, "2027-01-01");
        assert_eq!(parsed.items[2].title, "On-call handoff");
        assert_eq!(parsed.items[2].category, "other");
        assert!(parsed.items[2].is_achievement);
    }

    #[test]
    fn validates_empty_and_unrecognized_files_and_handles_zero_item_reports() {
        assert!(parse_markdown_report("   \n  ").is_err());
        assert!(parse_markdown_report("# Random Notes\n\nJust prose without day headings.").is_err());

        let empty_range_md = "\
# Report: Nov 1 – Nov 2, 2026

## Work Items

### Sunday, Nov 1

*(No items)*

### Monday, Nov 2

*(No items)*
";
        let parsed = parse_markdown_report(empty_range_md).expect("valid 0-item report succeeds");
        assert_eq!(parsed.items.len(), 0);
        assert_eq!(parsed.active_days, 0);
        assert_eq!(parsed.start_date, NaiveDate::from_ymd_opt(2026, 11, 1));
        assert_eq!(parsed.end_date, NaiveDate::from_ymd_opt(2026, 11, 2));
    }

    #[test]
    fn supports_iso_and_explicit_year_day_headings_without_report_header() {
        let md = "\
### 2026-07-04
- Ship CLI parser *(Documentation)*

### Sunday, Jul 5, 2026
- Write release notes - Ready for review
";
        let parsed =
            parse_markdown_report_with_fallback_year(md, 2025).expect("explicit day years parse");
        assert_eq!(parsed.items.len(), 2);
        assert_eq!(parsed.items[0].date, "2026-07-04");
        assert_eq!(parsed.items[0].category, "documentation");
        assert_eq!(parsed.items[1].date, "2026-07-05");
        assert_eq!(parsed.items[1].category, "other");
        assert_eq!(parsed.items[1].description, "Ready for review");
    }
}
