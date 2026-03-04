//! Serde types for Microsoft Graph API responses.

use serde::{Deserialize, Serialize};

/// Paginated response wrapper from Graph API.
#[derive(Debug, Deserialize)]
pub struct GraphResponse<T> {
    pub value: Vec<T>,
    #[serde(rename = "@odata.nextLink")]
    pub next_link: Option<String>,
}

/// Graph API calendar resource.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphCalendar {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub can_edit: bool,
}

/// Graph API event resource.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEvent {
    pub id: String,
    #[serde(default, rename = "iCalUId")]
    pub i_cal_uid: String,
    #[serde(default)]
    pub subject: String,
    pub body: Option<GraphBody>,
    pub start: Option<DateTimeTimeZone>,
    pub end: Option<DateTimeTimeZone>,
    pub location: Option<GraphLocation>,
    #[serde(default)]
    pub is_all_day: bool,
    #[serde(default)]
    pub is_cancelled: bool,
    pub recurrence: Option<PatternedRecurrence>,
    #[serde(default)]
    pub attendees: Vec<GraphAttendee>,
    pub organizer: Option<GraphRecipient>,
    #[serde(default)]
    pub reminder_minutes_before_start: i64,
    #[serde(default)]
    pub show_as: String,
    pub last_modified_date_time: Option<String>,
    pub online_meeting: Option<OnlineMeeting>,
    pub original_start: Option<DateTimeTimeZone>,
}

/// Event body (content + type).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphBody {
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub content_type: String,
}

/// Graph datetime with timezone.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DateTimeTimeZone {
    pub date_time: String,
    pub time_zone: String,
}

/// Graph location.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphLocation {
    #[serde(default)]
    pub display_name: String,
}

/// Graph attendee.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphAttendee {
    pub email_address: EmailAddress,
    pub status: Option<ResponseStatus>,
}

/// Email address (name + address).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailAddress {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub address: String,
}

/// Attendee response status.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseStatus {
    #[serde(default)]
    pub response: String,
}

/// Recipient (organizer).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphRecipient {
    pub email_address: EmailAddress,
}

/// Online meeting info.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineMeeting {
    pub join_url: Option<String>,
}

/// Patterned recurrence (pattern + range).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternedRecurrence {
    pub pattern: RecurrencePattern,
    pub range: RecurrenceRange,
}

/// Recurrence pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurrencePattern {
    /// "daily", "weekly", "absoluteMonthly", "relativeMonthly", "absoluteYearly", "relativeYearly"
    #[serde(rename = "type")]
    pub pattern_type: String,
    #[serde(default = "default_interval")]
    pub interval: i32,
    #[serde(default)]
    pub days_of_week: Vec<String>,
    #[serde(default)]
    pub day_of_month: i32,
    #[serde(default)]
    pub month: i32,
    /// "first", "second", "third", "fourth", "last"
    #[serde(default)]
    pub index: String,
    #[serde(default)]
    pub first_day_of_week: String,
}

fn default_interval() -> i32 {
    1
}

/// Recurrence range.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurrenceRange {
    /// "endDate", "noEnd", "numbered"
    #[serde(rename = "type")]
    pub range_type: String,
    #[serde(default)]
    pub start_date: String,
    #[serde(default)]
    pub end_date: String,
    #[serde(default)]
    pub number_of_occurrences: i32,
    #[serde(default)]
    pub recurrence_time_zone: String,
}

/// User profile (from GET /me).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphUser {
    #[serde(default)]
    pub mail: Option<String>,
    #[serde(default)]
    pub user_principal_name: String,
}

impl GraphUser {
    /// Returns the best email to use as account identifier.
    pub fn email(&self) -> &str {
        self.mail
            .as_deref()
            .filter(|m| !m.is_empty())
            .unwrap_or(&self.user_principal_name)
    }
}
