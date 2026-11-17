pub struct Category {
    pub id: &'static str,
    pub label: &'static str,
}

pub const CATEGORIES: &[Category] = &[
    Category {
        id: "development",
        label: "Development",
    },
    Category {
        id: "code-review",
        label: "Code Review",
    },
    Category {
        id: "meetings",
        label: "Meetings",
    },
    Category {
        id: "devops",
        label: "DevOps",
    },
    Category {
        id: "documentation",
        label: "Documentation",
    },
    Category { id: "other", label: "Other" },
];

pub fn category_label(id: &str) -> &str {
    CATEGORIES
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.label)
        .unwrap_or(id)
}