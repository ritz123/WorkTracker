use chrono::{Datelike, Duration, Local, Months, NaiveDate, Timelike, Utc};

pub fn format_date_key(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

pub fn parse_date_key(key: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(key, "%Y-%m-%d").ok()
}

pub fn format_day_full(date: NaiveDate) -> String {
    date.format("%A, %b %-d").to_string()
}

pub fn format_month_year(date: NaiveDate) -> String {
    date.format("%B %Y").to_string()
}

pub fn today_local() -> NaiveDate {
    Local::now().date_naive()
}

pub fn is_today(date: NaiveDate) -> bool {
    date == today_local()
}

pub fn start_of_week(date: NaiveDate) -> NaiveDate {
    let weekday = date.weekday().num_days_from_monday();
    date - Duration::days(weekday as i64)
}

pub fn end_of_week(date: NaiveDate) -> NaiveDate {
    start_of_week(date) + Duration::days(6)
}

pub fn start_of_month(date: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(date.year(), date.month(), 1).unwrap()
}

pub fn end_of_month(date: NaiveDate) -> NaiveDate {
    let next = start_of_month(date) + Months::new(1);
    next - Duration::days(1)
}

pub fn month_offset_for_date(date: NaiveDate) -> i32 {
    let today = today_local();
    let t0 = start_of_month(today);
    let t1 = start_of_month(date);
    ((t1.year() - t0.year()) * 12 + (t1.month() as i32 - t0.month() as i32)) as i32
}

pub fn is_in_current_week(date: NaiveDate) -> bool {
    let now = today_local();
    date >= start_of_week(now) && date <= end_of_week(now)
}

pub struct MonthWeek {
    pub week_number: u32,
    pub days: Vec<NaiveDate>,
}

pub struct MonthGrid {
    pub weeks: Vec<MonthWeek>,
    pub month: NaiveDate,
}

pub fn get_month_grid(month_offset: i32) -> MonthGrid {
    let base = if month_offset >= 0 {
        today_local() + Months::new(month_offset as u32)
    } else {
        today_local() - Months::new(month_offset.unsigned_abs())
    };
    let month_start = start_of_month(base);
    let month_end = end_of_month(base);
    let cal_start = start_of_week(month_start);
    let cal_end = end_of_week(month_end);

    let mut days = Vec::new();
    let mut cursor = cal_start;
    while cursor <= cal_end {
        days.push(cursor);
        cursor += Duration::days(1);
    }

    let mut weeks = Vec::new();
    for chunk in days.chunks(7) {
        let week_number = chunk[0].iso_week().week();
        weeks.push(MonthWeek {
            week_number,
            days: chunk.to_vec(),
        });
    }

    MonthGrid { weeks, month: base }
}

pub fn is_same_month(date: NaiveDate, reference: NaiveDate) -> bool {
    date.year() == reference.year() && date.month() == reference.month()
}

pub fn items_for_day<'a>(items: &'a [crate::models::WorkItem], date: NaiveDate) -> Vec<&'a crate::models::WorkItem> {
    let key = format_date_key(date);
    items.iter().filter(|i| i.date == key).collect()
}

pub fn iso_weekday_labels() -> [&'static str; 7] {
    ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"]
}

/// Local wall-clock time for display on item cards (matches original `h:mm a`).
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_grid_covers_month_in_full_weeks() {
        for offset in -3..=3 {
            let grid = get_month_grid(offset);
            assert!(
                (4..=6).contains(&grid.weeks.len()),
                "offset {offset}: expected 4–6 weeks, got {}",
                grid.weeks.len()
            );
            for (i, week) in grid.weeks.iter().enumerate() {
                assert_eq!(week.days.len(), 7, "offset {offset} week {i}");
            }
            let month_start = start_of_month(grid.month);
            let month_end = end_of_month(grid.month);
            let all_days: Vec<NaiveDate> = grid.weeks.iter().flat_map(|w| w.days.clone()).collect();
            assert!(all_days.contains(&month_start));
            assert!(all_days.contains(&month_end));
            assert_eq!(all_days.first().copied(), Some(start_of_week(month_start)));
            assert_eq!(all_days.last().copied(), Some(end_of_week(month_end)));
        }
    }
}

pub fn format_local_time(iso: chrono::DateTime<Utc>) -> String {
    let local = iso.with_timezone(&Local);
    let hour = local.hour();
    let minute = local.minute();
    let (h12, ampm) = if hour == 0 {
        (12, "AM")
    } else if hour < 12 {
        (hour, "AM")
    } else if hour == 12 {
        (12, "PM")
    } else {
        (hour - 12, "PM")
    };
    format!("{h12}:{minute:02} {ampm}")
}
