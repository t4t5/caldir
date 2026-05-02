use serde::{Deserialize, Serialize};

/// Patterned recurrence (pattern + range).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternedRecurrence {
    pub pattern: RecurrencePattern,
    pub range: RecurrenceRange,
}

/// Graph's six recurrence pattern types — modeled as a tagged enum because
/// each variant has a disjoint set of required fields. Sending fields that
/// don't apply (e.g. `index` on a daily pattern) gets rejected by Graph as
/// `"Cannot parse 'null' as a value of type 'microsoft.graph.weekIndex'"`.
/// Extra fields Graph includes on inbound (zero `dayOfMonth`, default
/// `firstDayOfWeek`) are silently ignored by serde during deserialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RecurrencePattern {
    Daily {
        #[serde(default = "default_interval")]
        interval: i32,
    },
    Weekly {
        #[serde(default = "default_interval")]
        interval: i32,
        #[serde(default)]
        days_of_week: Vec<String>,
        #[serde(default = "default_first_day_of_week")]
        first_day_of_week: String,
    },
    AbsoluteMonthly {
        #[serde(default = "default_interval")]
        interval: i32,
        #[serde(default)]
        day_of_month: i32,
    },
    RelativeMonthly {
        #[serde(default = "default_interval")]
        interval: i32,
        #[serde(default)]
        days_of_week: Vec<String>,
        #[serde(default)]
        index: String,
    },
    AbsoluteYearly {
        #[serde(default = "default_interval")]
        interval: i32,
        #[serde(default)]
        day_of_month: i32,
        #[serde(default)]
        month: i32,
    },
    RelativeYearly {
        #[serde(default = "default_interval")]
        interval: i32,
        #[serde(default)]
        days_of_week: Vec<String>,
        #[serde(default)]
        index: String,
        #[serde(default)]
        month: i32,
    },
}

fn default_interval() -> i32 {
    1
}

fn default_first_day_of_week() -> String {
    "sunday".to_string()
}

/// Recurrence range — three variants with different fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RecurrenceRange {
    EndDate {
        start_date: String,
        end_date: String,
    },
    NoEnd {
        start_date: String,
    },
    Numbered {
        start_date: String,
        number_of_occurrences: i32,
    },
}
