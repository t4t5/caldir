//! Serde types for Microsoft Graph API requests and responses.
//!
//! These types are used in both directions:
//! - Deserialized from Graph responses (`GET /events`, etc.)
//! - Serialized into POST/PATCH bodies by `outlook_event::to_outlook`
//!
//! Read-only and server-managed fields (`id`, `iCalUId`, `lastModifiedDateTime`,
//! `originalStart`, `responseStatus`, `organizer`, `onlineMeeting`, `type`) use
//! `skip_serializing_if` so they're never sent on outbound requests.

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
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEvent {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(default, rename = "iCalUId", skip_serializing_if = "String::is_empty")]
    pub i_cal_uid: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<GraphBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<DateTimeTimeZone>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<DateTimeTimeZone>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<GraphLocation>,
    #[serde(default)]
    pub is_all_day: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_cancelled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<PatternedRecurrence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attendees: Vec<GraphAttendee>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organizer: Option<GraphRecipient>,
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub reminder_minutes_before_start: i64,
    /// Outlook ignores `reminderMinutesBeforeStart` unless this is also set, so
    /// `to_outlook` always emits both. On inbound it's ignored.
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_reminder_on: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub show_as: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified_date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub online_meeting: Option<OnlineMeeting>,
    /// UTC ISO-8601 timestamp (Edm.DateTimeOffset). Present on every expanded
    /// occurrence returned by `calendarView`, identifying the scheduled start
    /// of the originating recurring instance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_start: Option<String>,
    /// The calendar owner's response to this event (more reliable than per-attendee status).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_status: Option<ResponseStatus>,
    /// Graph's event classification: `singleInstance`, `seriesMaster`,
    /// `occurrence`, or `exception`. Used by `list_events` to pick exceptions
    /// out of an `/instances` response (auto-expanded `occurrence` items get
    /// dropped).
    #[serde(default, rename = "type", skip_serializing_if = "String::is_empty")]
    pub event_type: String,
}

/// Event body (content + type).
#[derive(Debug, Serialize, Deserialize)]
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
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphLocation {
    #[serde(default)]
    pub display_name: String,
}

/// Graph attendee.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphAttendee {
    pub email_address: EmailAddress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ResponseStatus>,
    /// "required", "optional", "resource". Outbound-only field — `to_outlook`
    /// always sets it to "required". Empty string on inbound means we don't
    /// care about the value.
    #[serde(default, rename = "type", skip_serializing_if = "String::is_empty")]
    pub attendee_type: String,
}

/// Email address (name + address).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailAddress {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub address: String,
}

/// Attendee response status.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseStatus {
    #[serde(default)]
    pub response: String,
}

/// Recipient (organizer).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphRecipient {
    pub email_address: EmailAddress,
}

/// Online meeting info.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineMeeting {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub join_url: Option<String>,
}

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

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_zero_i64(n: &i64) -> bool {
    *n == 0
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
