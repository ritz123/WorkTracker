use chrono::NaiveDate;

use crate::categories::category_label;
use crate::dates::{format_date_key, format_day_full};
use crate::models::WorkItem;

fn each_day_inclusive(start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
    let mut days = Vec::new();
    let mut cursor = start;
    while cursor <= end {
        days.push(cursor);
        cursor += chrono::Duration::days(1);
    }
    days
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

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
}