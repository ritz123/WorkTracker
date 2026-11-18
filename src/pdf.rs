//! Self-contained, zero-dependency PDF 1.4 vector report generator (`A4` portrait).
//!
//! Architectural & domain invariants:
//! 1. **Active-days-only invariant**: [`generate_pdf_for_range`] filters [`WorkItem`] records to
//!    `start..=end` and emits day sections **exclusively** for dates that contain at least one
//!    saved work item, sorted chronologically (`start -> end`). Days with zero items never appear
//!    as headings or `"(No items)"` placeholders. When the entire range contains zero items, the
//!    generator emits a valid 1-page PDF containing the report header and a calm empty-range notice
//!    with zero day headings.
//! 2. **Orphan-free pagination & coordinate safety**: Every section heading, day header band,
//!    achievement card, and work-item block is pre-measured before painting. Section headings and
//!    day header bands reserve enough vertical budget to keep at least their first child block on
//!    the same page, and ultra-long multi-paragraph descriptions paginate line-by-line so vertical
//!    coordinates never cross `CONTENT_BOTTOM_Y` into the page footer.
//! 3. **Single-byte `WinAnsiEncoding` fidelity**: Built-in Type 1 fonts (`Helvetica`,
//!    `Helvetica-Bold`, `Helvetica-Oblique`) declare `/Encoding /WinAnsiEncoding`. UTF-8 inputs are
//!    transcoded to `Windows-1252` byte sequences (preserving smart quotes, en/em dashes, bullets,
//!    ellipses, `·`, `©`, and Western accented letters, with clean ASCII fallbacks for unsupported
//!    symbols) and serialized using escaped ASCII plus 3-digit octal escapes (`\ddd`) so content
//!    streams remain 7-bit clean and immune to octal digit-continuation ambiguity.

use std::collections::BTreeMap;
use std::f32::consts::{FRAC_PI_2, PI};
use std::fmt::Write as _;

use chrono::{Datelike, NaiveDate};

use crate::categories::category_label;
use crate::dates::{format_day_full, parse_date_key};
use crate::models::WorkItem;
use crate::theme::category_style;

/// A4 portrait page width in PostScript points (`210 mm`).
const PAGE_W: f32 = 595.28;
/// A4 portrait page height in PostScript points (`297 mm`).
const PAGE_H: f32 = 841.89;
/// Symmetric left/right page margin (`48 pt` / `~16.9 mm`).
const MARGIN_X: f32 = 48.0;
/// Top page margin (`48 pt`).
const MARGIN_TOP: f32 = 48.0;
/// Lowest `y` coordinate permitted for body content before breaking to a new page, reserving a
/// dedicated quiet zone above the footer rule at `y = 40 pt`.
const CONTENT_BOTTOM_Y: f32 = 54.0;
/// Usable horizontal content measure (`499.28 pt`).
const CONTENT_W: f32 = PAGE_W - 2.0 * MARGIN_X;

/// Cubic Bézier control-point ratio (`4 * (sqrt(2) - 1) / 3`) approximating a quarter-circle arc.
const KAPPA: f32 = 0.552_284_8;

#[derive(Clone, Copy, Debug)]
struct PdfRgb(f32, f32, f32);

impl PdfRgb {
    const fn from_hex(rgb: u32) -> Self {
        Self(
            ((rgb >> 16) & 0xFF) as f32 / 255.0,
            ((rgb >> 8) & 0xFF) as f32 / 255.0,
            (rgb & 0xFF) as f32 / 255.0,
        )
    }

    fn from_color32(c: egui::Color32) -> Self {
        let [r, g, b, _] = c.to_srgba_unmultiplied();
        Self(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
    }
}

// Editorial light-mode report palette aligned with `ThemeId::Default` ("Studio Slate").
const INK_TEXT: PdfRgb = PdfRgb::from_hex(0x0F172A);
const INK_BODY: PdfRgb = PdfRgb::from_hex(0x334155);
const INK_MUTED: PdfRgb = PdfRgb::from_hex(0x64748B);
const INK_FAINT: PdfRgb = PdfRgb::from_hex(0x94A3B8);
const ACCENT_PRIMARY: PdfRgb = PdfRgb::from_hex(0x1D4ED8);
const ACCENT_DOT: PdfRgb = PdfRgb::from_hex(0x2563EB);
const SURFACE_CARD: PdfRgb = PdfRgb::from_hex(0xF8FAFC);
const SURFACE_BAND: PdfRgb = PdfRgb::from_hex(0xF1F4F8);
const BORDER_RULE: PdfRgb = PdfRgb::from_hex(0xE2E8F0);
const DIVIDER_RULE: PdfRgb = PdfRgb::from_hex(0xEDF2F7);

const ACH_BG: PdfRgb = PdfRgb::from_hex(0xFEF3C7);
const ACH_CARD_BG: PdfRgb = PdfRgb::from_hex(0xFFFBEB);
const ACH_BORDER: PdfRgb = PdfRgb::from_hex(0xFDE68A);
const ACH_FG: PdfRgb = PdfRgb::from_hex(0xB45309);
const ACH_ACCENT: PdfRgb = PdfRgb::from_hex(0xD97706);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PdfFont {
    Regular,
    Bold,
    Oblique,
}

impl PdfFont {
    const fn resource_name(self) -> &'static str {
        match self {
            PdfFont::Regular => "/F1",
            PdfFont::Bold => "/F2",
            PdfFont::Oblique => "/F3",
        }
    }
}

/// Summary counts for a `[start, end]` report range, shared between the Export modal UI summary
/// and [`generate_pdf_for_range`] so preview telemetry and generated PDF metrics never diverge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RangeReportStats {
    pub total_items: usize,
    pub active_days: usize,
    pub achievements: usize,
}

/// Groups saved [`WorkItem`] entries whose `YYYY-MM-DD` date parses cleanly and falls within
/// `start..=end` into a chronologically ordered map of **active days only**.
///
/// Responsibility & invariants:
/// - Dates within `start..=end` that have zero items are never inserted into the returned
///   [`BTreeMap`], enforcing the active-days-only domain rule at the data-model boundary before
///   any layout or rendering code runs.
/// - Preserves the saved slice order of items within each active day.
fn collect_active_days(
    items: &[WorkItem],
    start: NaiveDate,
    end: NaiveDate,
) -> BTreeMap<NaiveDate, Vec<&WorkItem>> {
    let mut by_day: BTreeMap<NaiveDate, Vec<&WorkItem>> = BTreeMap::new();
    if start > end {
        return by_day;
    }
    for item in items {
        if let Some(day) = parse_date_key(&item.date) {
            if day >= start && day <= end {
                by_day.entry(day).or_default().push(item);
            }
        }
    }
    by_day
}

/// Computes item, active-day, and achievement counts for `start..=end` using the exact active-day
/// grouping consumed by [`generate_pdf_for_range`].
pub fn range_report_stats(
    items: &[WorkItem],
    start: NaiveDate,
    end: NaiveDate,
) -> RangeReportStats {
    let active_map = collect_active_days(items, start, end);
    let mut total_items = 0usize;
    let mut achievements = 0usize;
    for day_items in active_map.values() {
        total_items += day_items.len();
        achievements += day_items.iter().filter(|i| i.is_achievement).count();
    }
    RangeReportStats {
        total_items,
        active_days: active_map.len(),
        achievements,
    }
}

/// Transcodes a UTF-8 string slice into single-byte `WinAnsiEncoding` (`Windows-1252`) bytes.
///
/// Why this transcoding step is required:
/// - Standard PDF Type 1 fonts (`/Helvetica`, `/Helvetica-Bold`, `/Helvetica-Oblique`) are
///   single-byte fonts. Writing raw multi-byte UTF-8 sequences into `(...) Tj` causes PDF viewers
///   to render mojibake (`â€”` instead of `—`).
/// - Mapping Unicode scalars to exact `Windows-1252` code points (`0x80..=0x9F` typographic
///   punctuation plus `0xA0..=0xFF` Latin-1) preserves smart quotes, en/em dashes, bullets,
///   ellipses, middle dots, copyright symbols, and accented European characters, while degrading
///   non-WinAnsi symbols (stars, arrows, checkmarks, emoji) to clean ASCII equivalents.
fn to_winansi_bytes(input: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\t' => out.extend_from_slice(b"    "),
            '\r' | '\n' => out.push(b' '),
            '\u{00A0}' => out.push(b' '),
            ' '..='~' => out.push(ch as u8),
            '\u{00A1}'..='\u{00FF}' => out.push(ch as u8),
            // Windows-1252 0x80..=0x9F extensions supported by PDF `/WinAnsiEncoding`.
            '\u{20AC}' => out.push(0x80), // €
            '\u{201A}' => out.push(0x82), // ‚
            '\u{0192}' => out.push(0x83), // ƒ
            '\u{201E}' => out.push(0x84), // „
            '\u{2026}' => out.push(0x85), // …
            '\u{2020}' => out.push(0x86), // †
            '\u{2021}' => out.push(0x87), // ‡
            '\u{02C6}' => out.push(0x88), // ˆ
            '\u{2030}' => out.push(0x89), // ‰
            '\u{0160}' => out.push(0x8A), // Š
            '\u{2039}' => out.push(0x8B), // ‹
            '\u{0152}' => out.push(0x8C), // Œ
            '\u{017D}' => out.push(0x8E), // Ž
            '\u{2018}' => out.push(0x91), // ‘
            '\u{2019}' => out.push(0x92), // ’
            '\u{201C}' => out.push(0x93), // “
            '\u{201D}' => out.push(0x94), // ”
            '\u{2022}' => out.push(0x95), // •
            '\u{2013}' => out.push(0x96), // –
            '\u{2014}' => out.push(0x97), // —
            '\u{02DC}' => out.push(0x98), // ˜
            '\u{2122}' => out.push(0x99), // ™
            '\u{0161}' => out.push(0x9A), // š
            '\u{203A}' => out.push(0x9B), // ›
            '\u{0153}' => out.push(0x9C), // œ
            '\u{017E}' => out.push(0x9E), // ž
            '\u{0178}' => out.push(0x9F), // Ÿ
            // Clean ASCII fallbacks for common symbols outside WinAnsiEncoding.
            '\u{2212}' | '\u{2010}' | '\u{2011}' => out.push(b'-'),
            '\u{2605}' | '\u{2606}' | '\u{2B50}' => out.push(b'*'),
            '\u{2192}' => out.extend_from_slice(b"->"),
            '\u{2190}' => out.extend_from_slice(b"<-"),
            '\u{2194}' => out.extend_from_slice(b"<->"),
            '\u{21D2}' => out.extend_from_slice(b"=>"),
            '\u{2713}' | '\u{2714}' => out.extend_from_slice(b"[x]"),
            '\u{2717}' | '\u{2718}' => out.extend_from_slice(b"[ ]"),
            c if c.is_control() => {}
            _ => out.push(b'?'),
        }
    }
    out
}

/// Serializes `WinAnsiEncoding` bytes into a PDF literal string `(...)` with proper escaping of
/// `\`, `(`, `)` and zero-padded 3-digit octal escapes (`\ddd`) for high bytes (`0x80..=0xFF`).
///
/// Always emitting 3 octal digits prevents an immediately following ASCII digit `'0'..'7'` from
/// being misparsed as part of the octal escape per PDF 1.4 Section 7.3.4.2.
fn encode_pdf_literal_bytes(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() + 8);
    s.push('(');
    for &b in bytes {
        match b {
            b'\\' => s.push_str("\\\\"),
            b'(' => s.push_str("\\("),
            b')' => s.push_str("\\)"),
            0x20..=0x7E => s.push(b as char),
            _ => {
                let _ = write!(s, "\\{b:03o}");
            }
        }
    }
    s.push(')');
    s
}

/// Adobe AFM glyph advance width in thousandths of `1 em` for `Helvetica` / `Helvetica-Bold`.
fn glyph_width_units(b: u8, bold: bool) -> u16 {
    if bold {
        match b {
            b' ' => 278,
            b'!' => 333,
            b'"' => 474,
            b'#' | b'$' | b'0'..=b'9' | b'_' => 556,
            b'%' => 889,
            b'&' | b'A'..=b'D' | b'H' | b'K' | b'N' | b'R' | b'U' => 722,
            b'\'' => 238,
            b'(' | b')' | b'-' | b':' | b';' | b'[' | b']' | b'`' | b'f' | b't' => 333,
            b'*' | b'r' | b'{' | b'}' => 389,
            b'+' | b'<' | b'=' | b'>' | b'^' | b'~' => 584,
            b',' | b'.' | b'/' | b'I' | b'\\' | b'i' | b'j' | b'l' | 0x91 | 0x92 | 0xB7 => 278,
            b'?' | b'F' | b'L' | b'T' | b'Z' | b'b' | b'd' | b'g' | b'h' | b'n'..=b'q' | b'u' => {
                611
            }
            b'@' => 975,
            b'E' | b'P' | b'S' | b'V' | b'X' | b'Y' => 667,
            b'G' | b'O' | b'Q' | b'w' => 778,
            b'J' | b'a' | b'c' | b'e' | b'k' | b's' | b'v' | b'x' | b'y' | 0x96 => 556,
            b'M' => 833,
            b'W' => 944,
            b'm' => 889,
            b'z' | 0x93 | 0x94 => 500,
            b'|' => 280,
            0x85 | 0x97 => 1000,
            0x95 => 350,
            0xA9 => 737,
            _ => 611,
        }
    } else {
        match b {
            b' ' | b'!' | b',' | b'.' | b'/' | b':' | b';' | b'I' | b'[' | b'\\' | b']'
            | b'f' | b't' | 0xB7 => 278,
            b'"' => 355,
            b'#' | b'$' | b'0'..=b'9' | b'?' | b'L' | b'_' | b'a' | b'b' | b'd' | b'e' | b'g'
            | b'h' | b'n'..=b'q' | b'u' | 0x96 => 556,
            b'%' => 889,
            b'&' | b'A' | b'B' | b'E' | b'K' | b'P' | b'S' | b'V' | b'X' | b'Y' => 667,
            b'\'' => 191,
            b'(' | b')' | b'-' | b'`' | b'r' | 0x93 | 0x94 => 333,
            b'*' => 389,
            b'+' | b'<' | b'=' | b'>' | b'~' => 584,
            b'@' => 1015,
            b'C' | b'D' | b'H' | b'N' | b'R' | b'U' | b'w' => 722,
            b'F' | b'T' | b'Z' => 611,
            b'G' | b'O' | b'Q' => 778,
            b'J' | b'c' | b'k' | b's' | b'v' | b'x'..=b'z' => 500,
            b'M' | b'm' => 833,
            b'W' => 944,
            b'^' => 469,
            b'i' | b'j' | b'l' | 0x91 | 0x92 => 222,
            b'{' | b'}' => 334,
            b'|' => 260,
            0x85 | 0x97 => 1000,
            0x95 => 350,
            0xA9 => 737,
            _ => 556,
        }
    }
}

fn measure_winansi_bytes(bytes: &[u8], font: PdfFont, size: f32) -> f32 {
    let bold = matches!(font, PdfFont::Bold);
    let units: u32 = bytes.iter().map(|&b| glyph_width_units(b, bold) as u32).sum();
    (units as f32) * size / 1000.0
}

#[derive(Clone, Debug)]
enum WrappedLine {
    Text(Vec<u8>),
    ParagraphGap,
}

/// Word-wraps UTF-8 `text` to `max_width` points in `font` at `size` while preserving explicit
/// `\n` paragraph breaks and splitting overlong unbroken tokens (such as URLs) so lines never
/// overflow the right page margin.
fn wrap_paragraphs(text: &str, font: PdfFont, size: f32, max_width: f32) -> Vec<WrappedLine> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    // Slight 2% safety cushion so kerning/viewer metric differences never clip against right edge.
    let safe_max = (max_width * 0.98).max(24.0);
    let space_w = measure_winansi_bytes(b" ", font, size);
    let mut result = Vec::new();

    for raw_para in trimmed.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
        let para = raw_para.trim();
        if para.is_empty() {
            if !matches!(result.last(), None | Some(WrappedLine::ParagraphGap)) {
                result.push(WrappedLine::ParagraphGap);
            }
            continue;
        }

        let mut current: Vec<u8> = Vec::new();
        let mut current_w = 0.0f32;

        for word in para.split_whitespace() {
            let word_bytes = to_winansi_bytes(word);
            if word_bytes.is_empty() {
                continue;
            }
            let word_w = measure_winansi_bytes(&word_bytes, font, size);

            if current.is_empty() && word_w <= safe_max {
                current = word_bytes;
                current_w = word_w;
            } else if !current.is_empty() && current_w + space_w + word_w <= safe_max {
                current.push(b' ');
                current.extend_from_slice(&word_bytes);
                current_w += space_w + word_w;
            } else if word_w <= safe_max {
                result.push(WrappedLine::Text(std::mem::take(&mut current)));
                current = word_bytes;
                current_w = word_w;
            } else {
                // Hard-break an unbroken token (e.g. long URL or commit hash) within `safe_max`.
                if !current.is_empty() {
                    result.push(WrappedLine::Text(std::mem::take(&mut current)));
                    current_w = 0.0;
                }
                for &b in &word_bytes {
                    let bw = measure_winansi_bytes(&[b], font, size);
                    if !current.is_empty() && current_w + bw > safe_max {
                        result.push(WrappedLine::Text(std::mem::take(&mut current)));
                        current_w = 0.0;
                    }
                    current.push(b);
                    current_w += bw;
                }
            }
        }

        if !current.is_empty() {
            result.push(WrappedLine::Text(current));
        }
    }

    if matches!(result.last(), Some(WrappedLine::ParagraphGap)) {
        result.pop();
    }
    result
}

/// Wraps a single-paragraph string (such as an item title) into one or more non-empty WinAnsi lines.
fn wrap_single_line_or_more(text: &str, font: PdfFont, size: f32, max_width: f32) -> Vec<Vec<u8>> {
    let flat = text.replace(['\r', '\n'], " ");
    let wrapped = wrap_paragraphs(&flat, font, size, max_width);
    let lines: Vec<Vec<u8>> = wrapped
        .into_iter()
        .filter_map(|l| match l {
            WrappedLine::Text(b) => Some(b),
            WrappedLine::ParagraphGap => None,
        })
        .collect();
    if lines.is_empty() {
        vec![to_winansi_bytes("(Untitled)")]
    } else {
        lines
    }
}

/// Accumulates PDF content-stream drawing operators across one or more A4 pages.
struct PdfCanvas {
    pages: Vec<String>,
    cursor_y: f32,
}

impl PdfCanvas {
    fn new() -> Self {
        Self {
            pages: vec![String::with_capacity(4096)],
            cursor_y: PAGE_H - MARGIN_TOP,
        }
    }

    fn buf(&mut self) -> &mut String {
        self.pages.last_mut().expect("at least one page exists")
    }

    fn new_page(&mut self) {
        self.pages.push(String::with_capacity(4096));
        self.cursor_y = PAGE_H - MARGIN_TOP;
    }

    /// Starts a new page if `needed_height` does not fit above [`CONTENT_BOTTOM_Y`] and the
    /// current page already has content drawn below the top margin.
    fn ensure_space(&mut self, needed_height: f32) {
        let top_y = PAGE_H - MARGIN_TOP;
        if self.cursor_y - needed_height < CONTENT_BOTTOM_Y && (top_y - self.cursor_y) > 1.0 {
            self.new_page();
        }
    }

    fn set_fill(&mut self, c: PdfRgb) {
        let _ = writeln!(self.buf(), "{:.4} {:.4} {:.4} rg", c.0, c.1, c.2);
    }

    fn set_stroke(&mut self, c: PdfRgb, width: f32) {
        let _ = writeln!(
            self.buf(),
            "{:.4} {:.4} {:.4} RG {:.2} w",
            c.0, c.1, c.2, width
        );
    }

    fn hline(&mut self, x1: f32, x2: f32, y: f32, width: f32, color: PdfRgb) {
        self.set_stroke(color, width);
        let _ = writeln!(self.buf(), "{x1:.2} {y:.2} m {x2:.2} {y:.2} l S");
    }

    fn append_rounded_rect_path(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32) {
        let r = radius.clamp(0.0, 0.5 * w.min(h));
        if r <= 0.05 {
            let _ = writeln!(self.buf(), "{x:.2} {y:.2} {w:.2} {h:.2} re");
            return;
        }
        let k = KAPPA * r;
        let x0 = x;
        let y0 = y;
        let x1 = x + w;
        let y1 = y + h;
        let b = self.buf();
        let _ = writeln!(b, "{:.2} {:.2} m", x0 + r, y0);
        let _ = writeln!(b, "{:.2} {:.2} l", x1 - r, y0);
        let _ = writeln!(
            b,
            "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c",
            x1 - r + k,
            y0,
            x1,
            y0 + r - k,
            x1,
            y0 + r
        );
        let _ = writeln!(b, "{:.2} {:.2} l", x1, y1 - r);
        let _ = writeln!(
            b,
            "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c",
            x1,
            y1 - r + k,
            x1 - r + k,
            y1,
            x1 - r,
            y1
        );
        let _ = writeln!(b, "{:.2} {:.2} l", x0 + r, y1);
        let _ = writeln!(
            b,
            "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c",
            x0 + r - k,
            y1,
            x0,
            y1 - r + k,
            x0,
            y1 - r
        );
        let _ = writeln!(b, "{:.2} {:.2} l", x0, y0 + r);
        let _ = writeln!(
            b,
            "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c h",
            x0,
            y0 + r - k,
            x0 + r - k,
            y0,
            x0 + r,
            y0
        );
    }

    fn rounded_rect_filled(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, fill: PdfRgb) {
        self.set_fill(fill);
        self.append_rounded_rect_path(x, y, w, h, radius);
        let _ = writeln!(self.buf(), "f");
    }

    fn rounded_rect_filled_stroked(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        fill: PdfRgb,
        stroke: PdfRgb,
        stroke_w: f32,
    ) {
        self.set_fill(fill);
        self.set_stroke(stroke, stroke_w);
        self.append_rounded_rect_path(x, y, w, h, radius);
        let _ = writeln!(self.buf(), "B");
    }

    fn circle_filled(&mut self, cx: f32, cy: f32, r: f32, fill: PdfRgb) {
        self.rounded_rect_filled(cx - r, cy - r, 2.0 * r, 2.0 * r, r, fill);
    }

    /// Paints an upright 5-point vector star centered at `(cx, cy)` in PDF `+y`-up coordinates.
    fn star_filled(&mut self, cx: f32, cy: f32, outer_r: f32, inner_r: f32, fill: PdfRgb) {
        self.set_fill(fill);
        let b = self.buf();
        for i in 0..10 {
            let angle = FRAC_PI_2 + (i as f32) * (PI / 5.0);
            let r = if i % 2 == 0 { outer_r } else { inner_r };
            let px = cx + r * angle.cos();
            let py = cy + r * angle.sin();
            if i == 0 {
                let _ = writeln!(b, "{px:.2} {py:.2} m");
            } else {
                let _ = writeln!(b, "{px:.2} {py:.2} l");
            }
        }
        let _ = writeln!(b, "h f");
    }

    fn text_bytes(&mut self, x: f32, baseline_y: f32, font: PdfFont, size: f32, color: PdfRgb, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let lit = encode_pdf_literal_bytes(bytes);
        let fname = font.resource_name();
        let _ = writeln!(
            self.buf(),
            "BT {fname} {size:.2} Tf {:.4} {:.4} {:.4} rg 1 0 0 1 {x:.2} {baseline_y:.2} Tm {lit} Tj ET",
            color.0, color.1, color.2
        );
    }

    fn text_str(&mut self, x: f32, baseline_y: f32, font: PdfFont, size: f32, color: PdfRgb, text: &str) {
        let bytes = to_winansi_bytes(text);
        self.text_bytes(x, baseline_y, font, size, color, &bytes);
    }

    fn text_right(&mut self, right_x: f32, baseline_y: f32, font: PdfFont, size: f32, color: PdfRgb, text: &str) {
        let bytes = to_winansi_bytes(text);
        let w = measure_winansi_bytes(&bytes, font, size);
        self.text_bytes(right_x - w, baseline_y, font, size, color, &bytes);
    }

    /// Paints a semantic tinted category pill matching [`crate::theme::category_style`] in light
    /// mode (`bg` fill, `border` hairline, `dot` circle, and bold `fg` label) and returns its
    /// rendered horizontal advance width in points.
    fn draw_category_pill(&mut self, x: f32, bottom_y: f32, category_id: &str) -> f32 {
        let label_bytes = to_winansi_bytes(category_label(category_id));
        let style = category_style(category_id, false);
        let font_sz = 8.0;
        let pill_h = 14.5;
        let pad_x = 6.5;
        let dot_slot = 8.5;
        let text_w = measure_winansi_bytes(&label_bytes, PdfFont::Bold, font_sz);
        let pill_w = pad_x + dot_slot + text_w + pad_x;

        self.rounded_rect_filled_stroked(
            x,
            bottom_y,
            pill_w,
            pill_h,
            4.2,
            PdfRgb::from_color32(style.bg),
            PdfRgb::from_color32(style.border),
            0.65,
        );
        self.circle_filled(
            x + pad_x + 2.4,
            bottom_y + pill_h * 0.5,
            2.2,
            PdfRgb::from_color32(style.dot),
        );
        let baseline_y = bottom_y + pill_h * 0.5 - 0.35 * font_sz;
        self.text_bytes(
            x + pad_x + dot_slot,
            baseline_y,
            PdfFont::Bold,
            font_sz,
            PdfRgb::from_color32(style.fg),
            &label_bytes,
        );
        pill_w
    }

    fn measure_category_pill(category_id: &str) -> f32 {
        let label_bytes = to_winansi_bytes(category_label(category_id));
        6.5 + 8.5 + measure_winansi_bytes(&label_bytes, PdfFont::Bold, 8.0) + 6.5
    }

    /// Paints a warm gold `Achievement` pill with an upright 5-point vector star.
    fn draw_achievement_pill(&mut self, x: f32, bottom_y: f32) -> f32 {
        let label_bytes = b"Achievement";
        let font_sz = 8.0;
        let pill_h = 14.5;
        let pad_x = 6.5;
        let star_slot = 9.5;
        let text_w = measure_winansi_bytes(label_bytes, PdfFont::Bold, font_sz);
        let pill_w = pad_x + star_slot + text_w + pad_x;

        self.rounded_rect_filled_stroked(
            x,
            bottom_y,
            pill_w,
            pill_h,
            4.2,
            ACH_BG,
            ACH_BORDER,
            0.65,
        );
        self.star_filled(
            x + pad_x + 3.2,
            bottom_y + pill_h * 0.5 - 0.2,
            3.3,
            1.45,
            ACH_ACCENT,
        );
        let baseline_y = bottom_y + pill_h * 0.5 - 0.35 * font_sz;
        self.text_bytes(
            x + pad_x + star_slot,
            baseline_y,
            PdfFont::Bold,
            font_sz,
            ACH_FG,
            label_bytes,
        );
        pill_w
    }

    fn measure_achievement_pill() -> f32 {
        6.5 + 9.5 + measure_winansi_bytes(b"Achievement", PdfFont::Bold, 8.0) + 6.5
    }
}

fn format_range_heading(start: NaiveDate, end: NaiveDate) -> String {
    if start == end {
        format!("Report: {}", end.format("%b %-d, %Y"))
    } else if start.year() == end.year() {
        format!(
            "Report: {} \u{2013} {}",
            start.format("%b %-d"),
            end.format("%b %-d, %Y")
        )
    } else {
        format!(
            "Report: {} \u{2013} {}",
            start.format("%b %-d, %Y"),
            end.format("%b %-d, %Y")
        )
    }
}

fn plural<'a>(n: usize, one: &'a str, many: &'a str) -> &'a str {
    if n == 1 {
        one
    } else {
        many
    }
}

struct MeasuredItem {
    title_lines: Vec<Vec<u8>>,
    inline_pills: bool,
    desc_lines: Vec<WrappedLine>,
    total_height: f32,
}

const TITLE_FONT_SZ: f32 = 10.5;
const TITLE_LINE_H: f32 = 14.0;
const PILL_ROW_H: f32 = 16.5;
const DESC_FONT_SZ: f32 = 9.5;
const DESC_LINE_H: f32 = 13.2;
const DESC_PARA_GAP: f32 = 5.0;
const ITEM_INDENT: f32 = 14.0;
const ITEM_BOTTOM_GAP: f32 = 9.0;

fn measure_work_item(item: &WorkItem, available_w: f32, extra_right_meta_w: f32) -> MeasuredItem {
    let text_w = (available_w - ITEM_INDENT).max(80.0);
    let mut pills_w = PdfCanvas::measure_category_pill(&item.category) + extra_right_meta_w;
    if item.is_achievement && extra_right_meta_w == 0.0 {
        pills_w += 5.0 + PdfCanvas::measure_achievement_pill();
    }

    let title_lines = wrap_single_line_or_more(&item.title, PdfFont::Bold, TITLE_FONT_SZ, text_w);
    let first_line_w = measure_winansi_bytes(&title_lines[0], PdfFont::Bold, TITLE_FONT_SZ);
    let inline_pills = title_lines.len() == 1 && (first_line_w + 10.0 + pills_w <= text_w);

    let desc_lines = wrap_paragraphs(&item.description, PdfFont::Regular, DESC_FONT_SZ, text_w);

    let mut h = (title_lines.len() as f32) * TITLE_LINE_H;
    if !inline_pills {
        h += 2.0 + PILL_ROW_H;
    } else {
        h = h.max(PILL_ROW_H);
    }
    if !desc_lines.is_empty() {
        h += 3.0;
        for dl in &desc_lines {
            h += match dl {
                WrappedLine::Text(_) => DESC_LINE_H,
                WrappedLine::ParagraphGap => DESC_PARA_GAP,
            };
        }
    }
    MeasuredItem {
        title_lines,
        inline_pills,
        desc_lines,
        total_height: h,
    }
}

fn draw_section_heading(canvas: &mut PdfCanvas, title: &str, accent: PdfRgb, min_keep_with_next: f32) {
    let heading_h = 22.0;
    canvas.ensure_space(heading_h + min_keep_with_next);
    let top_y = canvas.cursor_y;
    canvas.rounded_rect_filled(MARGIN_X, top_y - 14.5, 3.2, 14.0, 1.6, accent);
    canvas.text_str(
        MARGIN_X + 10.0,
        top_y - 12.0,
        PdfFont::Bold,
        13.0,
        INK_TEXT,
        title,
    );
    canvas.cursor_y -= heading_h;
}

fn draw_report_header(canvas: &mut PdfCanvas, start: NaiveDate, end: NaiveDate, stats: RangeReportStats) {
    let top_y = canvas.cursor_y;

    // Brand eyebrow with cobalt accent pill dot.
    canvas.circle_filled(MARGIN_X + 3.0, top_y - 5.5, 2.6, ACCENT_PRIMARY);
    canvas.text_str(
        MARGIN_X + 10.0,
        top_y - 8.5,
        PdfFont::Bold,
        8.5,
        ACCENT_PRIMARY,
        "WORK TRACKER  \u{00B7}  WORK REPORT",
    );

    // Bold report range title.
    let heading = format_range_heading(start, end);
    canvas.text_str(
        MARGIN_X,
        top_y - 31.0,
        PdfFont::Bold,
        20.0,
        INK_TEXT,
        &heading,
    );

    // Subtle hairline rule below title.
    canvas.hline(MARGIN_X, MARGIN_X + CONTENT_W, top_y - 42.0, 0.75, BORDER_RULE);

    // Rounded summary metric strip.
    let strip_h = 26.0;
    let strip_bottom = top_y - 50.0 - strip_h;
    canvas.rounded_rect_filled_stroked(
        MARGIN_X,
        strip_bottom,
        CONTENT_W,
        strip_h,
        6.0,
        SURFACE_CARD,
        BORDER_RULE,
        0.75,
    );

    let summary = format!(
        "{} {}  \u{00B7}  {} active {}  \u{00B7}  {} {}",
        stats.total_items,
        plural(stats.total_items, "item", "items"),
        stats.active_days,
        plural(stats.active_days, "day", "days"),
        stats.achievements,
        plural(stats.achievements, "achievement", "achievements"),
    );
    let summary_baseline = strip_bottom + strip_h * 0.5 - 0.35 * 9.5;
    canvas.text_str(
        MARGIN_X + 12.0,
        summary_baseline,
        PdfFont::Bold,
        9.5,
        INK_BODY,
        &summary,
    );
    canvas.text_right(
        MARGIN_X + CONTENT_W - 12.0,
        summary_baseline,
        PdfFont::Regular,
        8.5,
        INK_MUTED,
        "Active days only",
    );

    canvas.cursor_y = strip_bottom - 20.0;
}

fn draw_achievement_row(canvas: &mut PdfCanvas, item: &WorkItem, day: NaiveDate) {
    let inner_x = MARGIN_X + 12.0;
    let inner_w = CONTENT_W - 24.0;
    let date_tag = day.format("%b %-d").to_string();
    let date_bytes = to_winansi_bytes(&date_tag);
    let date_meta_w = 8.0 + measure_winansi_bytes(&date_bytes, PdfFont::Regular, 8.5);

    let measured = measure_work_item(item, inner_w, date_meta_w);
    let card_pad_v = 8.5;
    let card_h = measured.total_height + 2.0 * card_pad_v;

    // Fit intact card on current page when it fits inside a single page; ultra-long cards break cleanly.
    if card_h <= (PAGE_H - MARGIN_TOP - CONTENT_BOTTOM_Y) {
        canvas.ensure_space(card_h + 6.0);
    } else {
        canvas.ensure_space(TITLE_LINE_H * 2.0 + 24.0);
    }

    let card_top = canvas.cursor_y;
    let visible_card_h = card_h.min((card_top - CONTENT_BOTTOM_Y).max(28.0));
    let card_bottom = card_top - visible_card_h;

    canvas.rounded_rect_filled_stroked(
        MARGIN_X,
        card_bottom,
        CONTENT_W,
        visible_card_h,
        6.0,
        ACH_CARD_BG,
        ACH_BORDER,
        0.75,
    );
    canvas.rounded_rect_filled(
        MARGIN_X + 3.5,
        card_bottom + 4.5,
        3.0,
        (visible_card_h - 9.0).max(6.0),
        1.5,
        ACH_ACCENT,
    );

    canvas.cursor_y = card_top - card_pad_v;
    draw_measured_item_contents(canvas, item, &measured, inner_x, inner_w, Some(&date_bytes), true);
    canvas.cursor_y -= card_pad_v + 7.0;
}

fn draw_day_header_band(canvas: &mut PdfCanvas, day: NaiveDate, count: usize, first_item_h: f32) {
    let band_h = 24.0;
    // Prevent orphaned day header: require room for the band + gap + first item (capped to half page).
    let keep_h = band_h + 8.0 + first_item_h.min(180.0);
    canvas.ensure_space(keep_h);

    let top_y = canvas.cursor_y;
    let bottom_y = top_y - band_h;
    canvas.rounded_rect_filled_stroked(
        MARGIN_X,
        bottom_y,
        CONTENT_W,
        band_h,
        5.5,
        SURFACE_BAND,
        BORDER_RULE,
        0.7,
    );

    let day_title = format!("{}, {}", format_day_full(day), day.year());
    let baseline = bottom_y + band_h * 0.5 - 0.35 * 10.5;
    canvas.text_str(
        MARGIN_X + 10.0,
        baseline,
        PdfFont::Bold,
        10.5,
        INK_TEXT,
        &day_title,
    );

    let count_str = format!("{count} {}", plural(count, "item", "items"));
    canvas.text_right(
        MARGIN_X + CONTENT_W - 10.0,
        baseline,
        PdfFont::Regular,
        9.0,
        INK_MUTED,
        &count_str,
    );

    canvas.cursor_y = bottom_y - 9.0;
}

fn draw_measured_item_contents(
    canvas: &mut PdfCanvas,
    item: &WorkItem,
    measured: &MeasuredItem,
    x: f32,
    w: f32,
    achievement_date_bytes: Option<&[u8]>,
    in_achievement_section: bool,
) {
    let text_x = x + ITEM_INDENT;
    let first_baseline = canvas.cursor_y - 0.78 * TITLE_FONT_SZ;

    // Left item marker: upright gold star for achievements, crisp cobalt dot for regular items.
    if item.is_achievement || in_achievement_section {
        canvas.star_filled(x + 4.8, first_baseline + 3.4, 4.0, 1.75, ACH_ACCENT);
    } else {
        canvas.circle_filled(x + 4.8, first_baseline + 3.3, 2.4, ACCENT_DOT);
    }

    for (idx, line_bytes) in measured.title_lines.iter().enumerate() {
        canvas.ensure_space(TITLE_LINE_H);
        let line_top = canvas.cursor_y;
        let baseline = line_top - 0.78 * TITLE_FONT_SZ;
        canvas.text_bytes(text_x, baseline, PdfFont::Bold, TITLE_FONT_SZ, INK_TEXT, line_bytes);

        if idx == 0 && measured.inline_pills {
            let title_w = measure_winansi_bytes(line_bytes, PdfFont::Bold, TITLE_FONT_SZ);
            let pill_bottom = baseline - 3.2;
            let mut px = text_x + title_w + 8.0;
            px += canvas.draw_category_pill(px, pill_bottom, &item.category);
            if item.is_achievement && !in_achievement_section {
                let _ = canvas.draw_achievement_pill(px + 5.0, pill_bottom);
            }
            if let Some(dbytes) = achievement_date_bytes {
                let dw = measure_winansi_bytes(dbytes, PdfFont::Regular, 8.5);
                canvas.text_bytes(
                    x + w - dw,
                    baseline,
                    PdfFont::Regular,
                    8.5,
                    INK_MUTED,
                    dbytes,
                );
            }
        }

        canvas.cursor_y -= if idx == 0 && measured.inline_pills {
            PILL_ROW_H.max(TITLE_LINE_H)
        } else {
            TITLE_LINE_H
        };
    }

    if !measured.inline_pills {
        canvas.ensure_space(PILL_ROW_H);
        let pill_bottom = canvas.cursor_y - 14.8;
        let mut px = text_x;
        px += canvas.draw_category_pill(px, pill_bottom, &item.category);
        if item.is_achievement && !in_achievement_section {
            let _ = canvas.draw_achievement_pill(px + 5.0, pill_bottom);
        }
        if let Some(dbytes) = achievement_date_bytes {
            let baseline = pill_bottom + 14.5 * 0.5 - 0.35 * 8.5;
            let dw = measure_winansi_bytes(dbytes, PdfFont::Regular, 8.5);
            canvas.text_bytes(
                x + w - dw,
                baseline,
                PdfFont::Regular,
                8.5,
                INK_MUTED,
                dbytes,
            );
        }
        canvas.cursor_y -= PILL_ROW_H;
    }

    if !measured.desc_lines.is_empty() {
        canvas.cursor_y -= 2.5;
        for dl in &measured.desc_lines {
            match dl {
                WrappedLine::ParagraphGap => {
                    canvas.cursor_y -= DESC_PARA_GAP;
                }
                WrappedLine::Text(bytes) => {
                    canvas.ensure_space(DESC_LINE_H);
                    let baseline = canvas.cursor_y - 0.78 * DESC_FONT_SZ;
                    canvas.text_bytes(
                        text_x,
                        baseline,
                        PdfFont::Regular,
                        DESC_FONT_SZ,
                        INK_BODY,
                        bytes,
                    );
                    canvas.cursor_y -= DESC_LINE_H;
                }
            }
        }
    }
}

fn draw_footers(canvas: &mut PdfCanvas) {
    let total = canvas.pages.len();
    for idx in 0..total {
        let page_label = format!("Page {} of {total}", idx + 1);
        let right_bytes = to_winansi_bytes(&page_label);
        let right_w = measure_winansi_bytes(&right_bytes, PdfFont::Regular, 8.5);
        let right_lit = encode_pdf_literal_bytes(&right_bytes);
        let page = &mut canvas.pages[idx];
        let _ = writeln!(
            page,
            "{:.4} {:.4} {:.4} RG 0.55 w {MARGIN_X:.2} 40.00 m {:.2} 40.00 l S",
            BORDER_RULE.0,
            BORDER_RULE.1,
            BORDER_RULE.2,
            PAGE_W - MARGIN_X
        );
        let _ = writeln!(
            page,
            "BT /F1 8.50 Tf {:.4} {:.4} {:.4} rg 1 0 0 1 {MARGIN_X:.2} 26.50 Tm (Work Tracker) Tj ET",
            INK_FAINT.0, INK_FAINT.1, INK_FAINT.2
        );
        let _ = writeln!(
            page,
            "BT /F1 8.50 Tf {:.4} {:.4} {:.4} rg 1 0 0 1 {:.2} 26.50 Tm {right_lit} Tj ET",
            INK_FAINT.0,
            INK_FAINT.1,
            INK_FAINT.2,
            PAGE_W - MARGIN_X - right_w
        );
    }
}

/// Serializes completed page content streams into a standards-compliant PDF 1.4 byte buffer with
/// exact byte offsets in `xref` and `startxref`.
fn serialize_pdf_document(pages: &[String]) -> Vec<u8> {
    let page_count = pages.len().max(1);
    // Object IDs:
    // 1: Catalog, 2: Pages, 3: Helvetica, 4: Helvetica-Bold, 5: Helvetica-Oblique,
    // 6 + 2*i: Page i, 7 + 2*i: Contents i, last: Info.
    let info_id = 6 + 2 * page_count;
    let total_objects = info_id;

    let mut out: Vec<u8> = Vec::with_capacity(4096 * page_count);
    out.extend_from_slice(b"%PDF-1.4\n");

    let mut offsets = vec![0usize; total_objects + 1];
    let write_obj = |out: &mut Vec<u8>, offsets: &mut [usize], id: usize, body: &str| {
        offsets[id] = out.len();
        let _ = write!(ByteWriter(out), "{id} 0 obj\n{body}\nendobj\n");
    };

    write_obj(&mut out, &mut offsets, 1, "<< /Type /Catalog /Pages 2 0 R >>");

    let mut kids = String::new();
    for i in 0..page_count {
        if i > 0 {
            kids.push(' ');
        }
        let _ = write!(kids, "{} 0 R", 6 + 2 * i);
    }
    write_obj(
        &mut out,
        &mut offsets,
        2,
        &format!("<< /Type /Pages /Kids [{kids}] /Count {page_count} >>"),
    );

    write_obj(
        &mut out,
        &mut offsets,
        3,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
    );
    write_obj(
        &mut out,
        &mut offsets,
        4,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>",
    );
    write_obj(
        &mut out,
        &mut offsets,
        5,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Oblique /Encoding /WinAnsiEncoding >>",
    );

    for (i, stream) in pages.iter().enumerate() {
        let page_id = 6 + 2 * i;
        let content_id = 7 + 2 * i;
        let page_dict = format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_W:.2} {PAGE_H:.2}] \
             /Resources << /Font << /F1 3 0 R /F2 4 0 R /F3 5 0 R >> >> \
             /Contents {content_id} 0 R >>"
        );
        write_obj(&mut out, &mut offsets, page_id, &page_dict);

        let stream_bytes = stream.as_bytes();
        offsets[content_id] = out.len();
        let _ = write!(
            ByteWriter(&mut out),
            "{content_id} 0 obj\n<< /Length {} >>\nstream\n",
            stream_bytes.len()
        );
        out.extend_from_slice(stream_bytes);
        out.extend_from_slice(b"\nendstream\nendobj\n");
    }

    write_obj(
        &mut out,
        &mut offsets,
        info_id,
        "<< /Title (Work Tracker Work Report) /Producer (Work Tracker) >>",
    );

    let xref_offset = out.len();
    let _ = write!(ByteWriter(&mut out), "xref\n0 {}\n", total_objects + 1);
    // PDF 1.4 Section 7.5.4: every cross-reference entry must be strictly 20 bytes long.
    out.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets.iter().skip(1) {
        let _ = write!(ByteWriter(&mut out), "{offset:010} 00000 n \n");
    }

    let _ = write!(
        ByteWriter(&mut out),
        "trailer\n<< /Size {} /Root 1 0 R /Info {info_id} 0 R >>\nstartxref\n{xref_offset}\n%%EOF",
        total_objects + 1
    );

    out
}

struct ByteWriter<'a>(&'a mut Vec<u8>);

impl std::fmt::Write for ByteWriter<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.extend_from_slice(s.as_bytes());
        Ok(())
    }
}

/// Generates an A4 vector PDF 1.4 work report for `start..=end` containing **only active days**
/// (dates with at least one saved [`WorkItem`]).
///
/// Responsibility & behavior by case:
/// - **Populated range (`>= 1` items)**: Renders the brand report header + metric strip, an
///   optional warm-amber `Achievements` highlight section when `>= 1` items have `is_achievement`,
///   and a chronological `Work Items` section emitting day header bands and tinted category pills
///   exclusively for days that have saved entries.
/// - **Empty range (`0` items)**: Emits a valid 1-page PDF with the report header and a calm
///   single-line notice (`"No work items recorded in this date range."`), with zero day headings
///   or `"(No items)"` placeholders.
/// - Returns complete PDF 1.4 bytes ready to write directly to disk.
pub fn generate_pdf_for_range(items: &[WorkItem], start: NaiveDate, end: NaiveDate) -> Vec<u8> {
    let active_days = collect_active_days(items, start, end);
    let stats = range_report_stats(items, start, end);

    let mut canvas = PdfCanvas::new();
    draw_report_header(&mut canvas, start, end, stats);

    if active_days.is_empty() {
        let box_h = 38.0;
        let bottom_y = canvas.cursor_y - box_h;
        canvas.rounded_rect_filled_stroked(
            MARGIN_X,
            bottom_y,
            CONTENT_W,
            box_h,
            6.0,
            SURFACE_CARD,
            BORDER_RULE,
            0.75,
        );
        canvas.text_str(
            MARGIN_X + 14.0,
            bottom_y + box_h * 0.5 - 0.35 * 10.0,
            PdfFont::Oblique,
            10.0,
            INK_MUTED,
            "No work items recorded in this date range.",
        );
        canvas.cursor_y = bottom_y - 16.0;
    } else {
        let achievements: Vec<(NaiveDate, &WorkItem)> = active_days
            .iter()
            .flat_map(|(&day, day_items)| {
                day_items
                    .iter()
                    .copied()
                    .filter(|i| i.is_achievement)
                    .map(move |i| (day, i))
            })
            .collect();

        if !achievements.is_empty() {
            draw_section_heading(&mut canvas, "Achievements", ACH_ACCENT, 52.0);
            for (day, item) in achievements {
                draw_achievement_row(&mut canvas, item, day);
            }
            canvas.cursor_y -= 8.0;
        }

        draw_section_heading(&mut canvas, "Work Items", ACCENT_PRIMARY, 64.0);

        for (day, day_items) in &active_days {
            let measured_items: Vec<MeasuredItem> = day_items
                .iter()
                .map(|item| measure_work_item(item, CONTENT_W - 6.0, 0.0))
                .collect();

            let first_h = measured_items.first().map_or(24.0, |m| m.total_height);
            draw_day_header_band(&mut canvas, *day, day_items.len(), first_h);

            for (idx, (item, measured)) in day_items.iter().zip(measured_items.iter()).enumerate() {
                // Keep a normal-sized item intact on a single page; paginate line-by-line only if
                // a single item's description is taller than a full A4 content area.
                if measured.total_height <= (PAGE_H - MARGIN_TOP - CONTENT_BOTTOM_Y) {
                    canvas.ensure_space(measured.total_height);
                } else {
                    canvas.ensure_space(TITLE_LINE_H * 2.0);
                }

                draw_measured_item_contents(
                    &mut canvas,
                    item,
                    measured,
                    MARGIN_X + 2.0,
                    CONTENT_W - 4.0,
                    None,
                    false,
                );

                if idx + 1 < day_items.len() {
                    canvas.cursor_y -= 5.0;
                    canvas.hline(
                        MARGIN_X + ITEM_INDENT + 2.0,
                        MARGIN_X + CONTENT_W,
                        canvas.cursor_y,
                        0.45,
                        DIVIDER_RULE,
                    );
                    canvas.cursor_y -= ITEM_BOTTOM_GAP;
                } else {
                    canvas.cursor_y -= 13.0;
                }
            }
        }
    }

    draw_footers(&mut canvas);
    serialize_pdf_document(&canvas.pages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::format_date_key;
    use chrono::{Duration, Utc};

    fn make_item(
        id: &str,
        date: NaiveDate,
        title: &str,
        description: &str,
        category: &str,
        is_achievement: bool,
    ) -> WorkItem {
        WorkItem {
            id: id.into(),
            date: format_date_key(date),
            title: title.into(),
            description: description.into(),
            category: category.into(),
            is_achievement,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Verifies the core active-days-only invariant: across a 7-day range where only 2 days have
    /// saved items, the generated PDF includes both active days and their items while completely
    /// omitting all 5 empty days and never emitting `"No items"`.
    #[test]
    fn pdf_emits_only_active_days_and_omits_empty_days() {
        let start = NaiveDate::from_ymd_opt(2026, 11, 16).unwrap(); // Monday
        let end = start + Duration::days(6); // Sunday Nov 22
        let active_tue = start + Duration::days(1); // Tuesday, Nov 17
        let active_fri = start + Duration::days(4); // Friday, Nov 20

        let items = vec![
            make_item(
                "1",
                active_tue,
                "Ship vector PDF engine",
                "Zero-dependency A4 layout",
                "development",
                true,
            ),
            make_item(
                "2",
                active_fri,
                "Review storage migrations",
                "Verified schema roundtrip",
                "code-review",
                false,
            ),
            // Out-of-range item must also be excluded.
            make_item(
                "out",
                start - Duration::days(3),
                "Out of range item",
                "",
                "other",
                false,
            ),
        ];

        let pdf = generate_pdf_for_range(&items, start, end);
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF"));

        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains(&format_day_full(active_tue)));
        assert!(text.contains(&format_day_full(active_fri)));
        assert!(text.contains("Ship vector PDF engine"));
        assert!(text.contains("Review storage migrations"));
        assert!(text.contains("2 items"));
        assert!(text.contains("2 active days"));
        assert!(text.contains("1 achievement"));
        assert!(!text.contains("Out of range item"));

        for offset in [0i64, 2, 3, 5, 6] {
            let empty_day = start + Duration::days(offset);
            let label = format_day_full(empty_day);
            assert!(
                !text.contains(&label),
                "empty day '{label}' must never appear in active-days-only PDF"
            );
        }
        assert!(!text.contains("No items"));
        assert!(!text.contains("(No items)"));
    }

    #[test]
    fn empty_date_range_produces_valid_single_page_pdf_without_day_headings() {
        let start = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 11, 7).unwrap();

        let pdf = generate_pdf_for_range(&[], start, end);
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF"));

        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("No work items recorded in this date range."));
        assert!(text.contains("/Count 1"));
        assert!(text.contains("Page 1 of 1"));
        assert!(!text.contains("(Work Items)"));
        for offset in 0..=6 {
            let day = start + Duration::days(offset);
            assert!(!text.contains(&format_day_full(day)));
        }
    }

    /// Verifies multi-page pagination, word wrapping, explicit `\n` paragraph breaks, coordinate
    /// bounds (`y >= 20.0` across all text matrices), and byte-exact `xref` offsets across 65 items.
    #[test]
    fn pdf_paginates_sixty_five_detailed_items_with_valid_coordinates_and_xref() {
        let start = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 11, 20).unwrap();

        let mut items = Vec::new();
        let categories = [
            "development",
            "code-review",
            "meetings",
            "devops",
            "documentation",
            "other",
        ];
        for i in 0..65usize {
            let day = start + Duration::days(((i * 2) % 15) as i64);
            items.push(make_item(
                &format!("item-{i}"),
                day,
                &format!("Engineering deliverable #{i}: latency & throughput optimization pass"),
                "Implemented deterministic batch layout with explicit paragraph breaks.\n\n\
                 Second paragraph verifying multi-line word wrapping across the full A4 measure \
                 including https://github.com/ritz123/TimeTracker/pull/1234567890abcdef1234567890.",
                categories[i % categories.len()],
                i % 9 == 0,
            ));
        }

        let pdf = generate_pdf_for_range(&items, start, end);
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF"));

        let text = String::from_utf8(pdf.clone()).expect("PDF output must remain clean 7-bit ASCII");
        let page_objs = text.matches("/Type /Page ").count();
        assert!(
            page_objs >= 4,
            "65 detailed items must paginate across multiple pages, got {page_objs}"
        );
        assert!(text.contains(&format!("/Count {page_objs}")));
        assert!(text.contains(&format!("Page {page_objs} of {page_objs}")));

        // Verify no `Tm` text baseline underflows into negative or sub-footer coordinates.
        for line in text.lines() {
            if let Some(tm_idx) = line.find(" Tm ") {
                let prefix = &line[..tm_idx];
                let nums: Vec<&str> = prefix.split_whitespace().collect();
                if nums.len() >= 2 {
                    let y: f32 = nums[nums.len() - 1]
                        .parse()
                        .expect("valid float y before Tm");
                    assert!(
                        (20.0..=PAGE_H).contains(&y),
                        "text matrix y={y} out of A4 bounds in line: {line}"
                    );
                }
            }
        }

        // Verify startxref points directly to `xref\n` and every `n` entry points to `<id> 0 obj`.
        let startxref_pos = text.rfind("startxref\n").expect("startxref present");
        let after = &text[startxref_pos + "startxref\n".len()..];
        let xref_offset: usize = after
            .lines()
            .next()
            .unwrap()
            .parse()
            .expect("numeric startxref");
        assert!(
            pdf[xref_offset..].starts_with(b"xref\n"),
            "startxref {xref_offset} did not land on xref table"
        );

        let xref_section = &text[xref_offset..startxref_pos];
        let mut raw_lines = xref_section.split_inclusive('\n');
        assert_eq!(raw_lines.next(), Some("xref\n"));
        assert!(raw_lines.next().unwrap().starts_with("0 "));
        assert_eq!(
            raw_lines.next(),
            Some("0000000000 65535 f \n"),
            "object 0 free entry must be exact 20-byte PDF 1.4 record"
        );
        for (idx, entry) in raw_lines.enumerate() {
            if entry.starts_with("trailer") {
                break;
            }
            let obj_id = idx + 1;
            assert_eq!(entry.len(), 20, "PDF 1.4 xref entry must be 20 bytes: {entry:?}");
            let off: usize = entry[..10].parse().expect("10-digit xref offset");
            let expected_header = format!("{obj_id} 0 obj\n");
            assert!(
                pdf[off..].starts_with(expected_header.as_bytes()),
                "xref object {obj_id} offset {off} did not match {expected_header:?}",
            );
        }
    }

    /// Verifies PDF string escaping (`\(`, `\)`, `\\`), `WinAnsiEncoding` octal escapes (`—` ->
    /// `\227`, `–` -> `\226`, `•` -> `\225`, `©` -> `\251`), and achievement serialization.
    #[test]
    fn pdf_escapes_parentheses_backslashes_smart_punctuation_and_achievements() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 18).unwrap();
        let items = vec![make_item(
            "esc-1",
            day,
            r"Fix parser (foo) & path C:\work\a\b — v1.0",
            "Smart quotes \u{201C}hello\u{201D} \u{2022} ellipsis\u{2026} \u{00A9} café ★",
            "devops",
            true,
        )];

        let pdf = generate_pdf_for_range(&items, day, day);
        let text = String::from_utf8(pdf).expect("7-bit clean PDF stream");

        assert!(text.contains("/Encoding /WinAnsiEncoding"));
        assert!(text.contains(r"\(foo\)"));
        assert!(text.contains(r"C:\\work\\a\\b"));
        // Em dash U+2014 -> WinAnsi 0x97 -> octal \227.
        assert!(text.contains(r"\227"));
        // Bullet U+2022 -> WinAnsi 0x95 -> octal \225; © U+00A9 -> octal \251; é U+00E9 -> \351.
        assert!(text.contains(r"\225"));
        assert!(text.contains(r"\251"));
        assert!(text.contains(r"caf\351"));
        assert!(text.contains("(Achievements)"));
        assert!(text.contains("(DevOps)"));
    }
}
