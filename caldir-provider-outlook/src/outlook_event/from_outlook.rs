//! Convert Microsoft Graph event types to caldir Event.

use anyhow::{Result, bail};
use caldir_core::event::{
    Attendee, Event, EventStatus, EventTime, ParticipationStatus, Recurrence, Reminder, Reminders,
    Transparency,
};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};

use crate::constants::PROVIDER_EVENT_ID_PROPERTY;
use crate::graph_types::GraphEvent;

pub fn from_outlook(event: GraphEvent, account_email: &str) -> Result<Event> {
    let start = parse_event_time(event.start.as_ref(), event.is_all_day, "start")?;
    let end = parse_event_time(event.end.as_ref(), event.is_all_day, "end")?;

    let status = if event.is_cancelled {
        EventStatus::Cancelled
    } else {
        EventStatus::Confirmed
    };

    let transparency = match event.show_as.as_str() {
        "free" => Transparency::Transparent,
        _ => Transparency::Opaque,
    };

    let recurrence = event
        .recurrence
        .as_ref()
        .map(recurrence_from_outlook)
        .transpose()?;

    // For exception instances, original_start serves as recurrence_id.
    // Microsoft Graph returns this as a UTC RFC3339 timestamp.
    let recurrence_id = event
        .original_start
        .as_deref()
        .map(|s| parse_original_start(s, event.is_all_day))
        .transpose()?;

    let reminders = if event.reminder_minutes_before_start > 0 {
        Reminders(vec![Reminder {
            minutes: event.reminder_minutes_before_start,
        }])
    } else {
        Reminders(vec![])
    };

    let organizer = event.organizer.as_ref().map(|o| Attendee {
        name: if o.email_address.name.is_empty() {
            None
        } else {
            Some(o.email_address.name.clone())
        },
        email: o.email_address.address.clone(),
        response_status: None,
    });

    // The top-level responseStatus reflects the calendar owner's actual response,
    // which is more reliable than the per-attendee status in the attendees array
    // (the Graph API often returns "none" for all attendees there).
    let owner_status = event
        .response_status
        .as_ref()
        .and_then(|s| outlook_to_participation_status(&s.response));

    let attendees: Vec<Attendee> = event
        .attendees
        .iter()
        .map(|a| {
            let is_owner = a.email_address.address.eq_ignore_ascii_case(account_email);
            Attendee {
                name: if a.email_address.name.is_empty() {
                    None
                } else {
                    Some(a.email_address.name.clone())
                },
                email: a.email_address.address.clone(),
                response_status: if is_owner {
                    owner_status
                } else {
                    a.status
                        .as_ref()
                        .and_then(|s| outlook_to_participation_status(&s.response))
                },
            }
        })
        .collect();

    let conference_url = event
        .online_meeting
        .as_ref()
        .and_then(|m| m.join_url.clone());

    let description = event.body.as_ref().and_then(|b| {
        if b.content.is_empty() {
            None
        } else {
            // Strip HTML tags for plain text if contentType is "html"
            if b.content_type == "html" {
                let text = strip_html_tags(&b.content);
                if text.trim().is_empty() {
                    None
                } else {
                    Some(text)
                }
            } else {
                Some(b.content.clone())
            }
        }
    });

    let location = event.location.as_ref().and_then(|l| {
        if l.display_name.is_empty() {
            None
        } else {
            Some(l.display_name.clone())
        }
    });

    let updated = event
        .last_modified_date_time
        .as_ref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let custom_properties = vec![(PROVIDER_EVENT_ID_PROPERTY.to_string(), event.id)];

    Ok(Event {
        uid: event.i_cal_uid,
        summary: event.subject,
        description,
        location,
        start,
        end,
        status,
        recurrence,
        recurrence_id,
        reminders,
        transparency,
        organizer,
        attendees,
        conference_url,
        updated,
        sequence: None,
        custom_properties,
    })
}

fn parse_original_start(s: &str, is_all_day: bool) -> Result<EventTime> {
    if is_all_day {
        let date = NaiveDate::parse_from_str(&s[..s.len().min(10)], "%Y-%m-%d")
            .map_err(|e| anyhow::anyhow!("Failed to parse all-day originalStart '{}': {}", s, e))?;
        return Ok(EventTime::Date(date));
    }
    let dt = DateTime::parse_from_rfc3339(s)
        .map_err(|e| anyhow::anyhow!("Failed to parse originalStart '{}': {}", s, e))?
        .with_timezone(&Utc);
    Ok(EventTime::DateTimeUtc(dt))
}

fn parse_event_time(
    dtz: Option<&crate::graph_types::DateTimeTimeZone>,
    is_all_day: bool,
    field: &str,
) -> Result<EventTime> {
    let dtz = dtz.ok_or_else(|| anyhow::anyhow!("Event has no {field} time"))?;
    parse_datetime_timezone(&dtz.date_time, &dtz.time_zone, is_all_day)
}

fn parse_datetime_timezone(
    datetime_str: &str,
    timezone: &str,
    is_all_day: bool,
) -> Result<EventTime> {
    if is_all_day {
        // All-day: parse just the date portion
        let date_str = &datetime_str[..10]; // "2025-03-20"
        let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .or_else(|_| NaiveDate::parse_from_str(datetime_str, "%Y-%m-%dT%H:%M:%S%.f"))
            .map_err(|e| {
                anyhow::anyhow!("Failed to parse all-day date '{}': {}", datetime_str, e)
            })?;
        return Ok(EventTime::Date(date));
    }

    // Graph sends datetime strings like "2025-03-20T15:00:00.0000000"
    let dt = parse_graph_datetime(datetime_str)?;

    if timezone == "UTC" || timezone == "tzone://Microsoft/Utc" {
        Ok(EventTime::DateTimeUtc(dt.and_utc()))
    } else {
        Ok(EventTime::DateTimeZoned {
            datetime: dt,
            tzid: normalize_timezone(timezone),
        })
    }
}

/// Parse Graph datetime strings like "2025-03-20T15:00:00.0000000"
fn parse_graph_datetime(s: &str) -> Result<NaiveDateTime> {
    // Try various formats Graph API might return
    NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S"))
        .map_err(|e| anyhow::anyhow!("Failed to parse datetime '{}': {}", s, e))
}

/// Normalize Microsoft timezone names to IANA when possible.
fn normalize_timezone(tz: &str) -> String {
    // Graph API uses Windows timezone names; map common ones to IANA
    match tz {
        "Eastern Standard Time" => "America/New_York",
        "Central Standard Time" => "America/Chicago",
        "Mountain Standard Time" => "America/Denver",
        "Pacific Standard Time" => "America/Los_Angeles",
        "UTC" | "tzone://Microsoft/Utc" => "UTC",
        "GMT Standard Time" => "Europe/London",
        "Romance Standard Time" => "Europe/Paris",
        "W. Europe Standard Time" => "Europe/Berlin",
        "Central European Standard Time" => "Europe/Warsaw",
        "E. Europe Standard Time" => "Europe/Bucharest",
        "FLE Standard Time" => "Europe/Helsinki",
        "GTB Standard Time" => "Europe/Athens",
        "Russian Standard Time" => "Europe/Moscow",
        "Israel Standard Time" => "Asia/Jerusalem",
        "Arabian Standard Time" => "Asia/Dubai",
        "India Standard Time" => "Asia/Kolkata",
        "China Standard Time" => "Asia/Shanghai",
        "Tokyo Standard Time" => "Asia/Tokyo",
        "Korea Standard Time" => "Asia/Seoul",
        "AUS Eastern Standard Time" => "Australia/Sydney",
        "New Zealand Standard Time" => "Pacific/Auckland",
        "Hawaiian Standard Time" => "Pacific/Honolulu",
        "Alaskan Standard Time" => "America/Anchorage",
        "Atlantic Standard Time" => "America/Halifax",
        "SA Pacific Standard Time" => "America/Bogota",
        "SA Eastern Standard Time" => "America/Cayenne",
        "E. South America Standard Time" => "America/Sao_Paulo",
        "Argentina Standard Time" => "America/Buenos_Aires",
        "SE Asia Standard Time" => "Asia/Bangkok",
        "Singapore Standard Time" => "Asia/Singapore",
        "Taipei Standard Time" => "Asia/Taipei",
        "West Pacific Standard Time" => "Pacific/Port_Moresby",
        "South Africa Standard Time" => "Africa/Johannesburg",
        "Egypt Standard Time" => "Africa/Cairo",
        _ => tz, // Pass through if already IANA or unknown
    }
    .to_string()
}

fn outlook_to_participation_status(status: &str) -> Option<ParticipationStatus> {
    match status {
        "accepted" => Some(ParticipationStatus::Accepted),
        "declined" => Some(ParticipationStatus::Declined),
        "tentativelyAccepted" => Some(ParticipationStatus::Tentative),
        "none" | "notResponded" => Some(ParticipationStatus::NeedsAction),
        _ => None,
    }
}

/// Convert Graph PatternedRecurrence to an RRULE string + exdates.
fn recurrence_from_outlook(rec: &crate::graph_types::PatternedRecurrence) -> Result<Recurrence> {
    let pattern = &rec.pattern;
    let range = &rec.range;

    let mut parts = Vec::new();

    // FREQ
    let freq = match pattern.pattern_type.as_str() {
        "daily" => "DAILY",
        "weekly" => "WEEKLY",
        "absoluteMonthly" | "relativeMonthly" => "MONTHLY",
        "absoluteYearly" | "relativeYearly" => "YEARLY",
        other => bail!("Unsupported recurrence pattern type: {other}"),
    };
    parts.push(format!("FREQ={freq}"));

    // INTERVAL
    if pattern.interval > 1 {
        parts.push(format!("INTERVAL={}", pattern.interval));
    }

    // BYDAY
    if !pattern.days_of_week.is_empty() {
        let days: Vec<&str> = pattern
            .days_of_week
            .iter()
            .filter_map(|d| outlook_day_to_rrule(d))
            .collect();
        if !days.is_empty() {
            match pattern.pattern_type.as_str() {
                "relativeMonthly" | "relativeYearly" => {
                    // Prefix with index (e.g., "2MO" for second Monday)
                    let index_num = outlook_index_to_number(&pattern.index);
                    let prefixed: Vec<String> =
                        days.iter().map(|d| format!("{index_num}{d}")).collect();
                    parts.push(format!("BYDAY={}", prefixed.join(",")));
                }
                _ => {
                    parts.push(format!("BYDAY={}", days.join(",")));
                }
            }
        }
    }

    // BYMONTHDAY
    if pattern.day_of_month > 0
        && matches!(
            pattern.pattern_type.as_str(),
            "absoluteMonthly" | "absoluteYearly"
        )
    {
        parts.push(format!("BYMONTHDAY={}", pattern.day_of_month));
    }

    // BYMONTH
    if pattern.month > 0
        && matches!(
            pattern.pattern_type.as_str(),
            "absoluteYearly" | "relativeYearly"
        )
    {
        parts.push(format!("BYMONTH={}", pattern.month));
    }

    // Range
    match range.range_type.as_str() {
        "endDate" => {
            if !range.end_date.is_empty() {
                // Convert "2025-12-31" to "20251231"
                let until = range.end_date.replace('-', "");
                parts.push(format!("UNTIL={until}"));
            }
        }
        "numbered" => {
            if range.number_of_occurrences > 0 {
                parts.push(format!("COUNT={}", range.number_of_occurrences));
            }
        }
        "noEnd" => {} // No UNTIL or COUNT
        _ => {}
    }

    let rrule = parts.join(";");
    Ok(Recurrence {
        rrule,
        exdates: vec![],
    })
}

fn outlook_day_to_rrule(day: &str) -> Option<&'static str> {
    match day {
        "sunday" => Some("SU"),
        "monday" => Some("MO"),
        "tuesday" => Some("TU"),
        "wednesday" => Some("WE"),
        "thursday" => Some("TH"),
        "friday" => Some("FR"),
        "saturday" => Some("SA"),
        _ => None,
    }
}

fn outlook_index_to_number(index: &str) -> &'static str {
    match index {
        "first" => "1",
        "second" => "2",
        "third" => "3",
        "fourth" => "4",
        "last" => "-1",
        _ => "1",
    }
}

/// Simple HTML tag stripper for event body content.
fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_types::*;

    fn minimal_graph_event() -> GraphEvent {
        GraphEvent {
            id: "test-id".to_string(),
            i_cal_uid: "test-uid".to_string(),
            subject: "Test Event".to_string(),
            body: None,
            start: Some(DateTimeTimeZone {
                date_time: "2025-03-20T15:00:00.0000000".to_string(),
                time_zone: "UTC".to_string(),
            }),
            end: Some(DateTimeTimeZone {
                date_time: "2025-03-20T16:00:00.0000000".to_string(),
                time_zone: "UTC".to_string(),
            }),
            location: None,
            is_all_day: false,
            is_cancelled: false,
            recurrence: None,
            attendees: vec![],
            organizer: None,
            reminder_minutes_before_start: 0,
            show_as: "busy".to_string(),
            last_modified_date_time: None,
            online_meeting: None,
            original_start: None,
            response_status: None,
        }
    }

    /// Representative JSON of a recurring event instance returned by
    /// `calendarView`. `originalStart` is a UTC ISO-8601 string
    /// (Edm.DateTimeOffset), not a `dateTimeTimeZone` object — getting this
    /// wrong used to fail the entire pull with:
    ///   "invalid type: string \"...\", expected struct DateTimeTimeZone"
    const RECURRING_INSTANCE_JSON: &str = r#"{
        "id": "AAMkAD-instance-id",
        "iCalUId": "040000008200E00074C5B7101A82E00800000000abc@outlook.com",
        "subject": "Daily standup",
        "start": {"dateTime": "2026-05-01T16:00:00.0000000", "timeZone": "UTC"},
        "end":   {"dateTime": "2026-05-01T16:30:00.0000000", "timeZone": "UTC"},
        "isAllDay": false,
        "isCancelled": false,
        "showAs": "busy",
        "originalStart": "2026-05-01T16:00:00Z"
    }"#;

    #[test]
    fn graph_event_deserializes_with_original_start_as_string() {
        // Regression: Graph's `originalStart` is Edm.DateTimeOffset, a string —
        // not a `dateTimeTimeZone` struct. Any expanded recurring instance from
        // calendarView includes it, so a wrong type here breaks every pull on a
        // calendar that has a recurring event.
        let parsed: GraphEvent = serde_json::from_str(RECURRING_INSTANCE_JSON)
            .expect("recurring instance with string originalStart must parse");
        assert_eq!(
            parsed.original_start.as_deref(),
            Some("2026-05-01T16:00:00Z")
        );
    }

    #[test]
    fn from_outlook_uses_original_start_as_recurrence_id() {
        let parsed: GraphEvent = serde_json::from_str(RECURRING_INSTANCE_JSON).unwrap();
        let event = from_outlook(parsed, "me@example.com").unwrap();
        match event.recurrence_id {
            Some(EventTime::DateTimeUtc(dt)) => {
                assert_eq!(dt.to_rfc3339(), "2026-05-01T16:00:00+00:00");
            }
            other => panic!("expected DateTimeUtc recurrence_id, got {other:?}"),
        }
    }

    #[test]
    fn owner_status_from_response_status_overrides_attendee_none() {
        // Graph API returns "none" for all attendees in the attendees array,
        // but the top-level responseStatus correctly reflects the owner's response.
        let mut event = minimal_graph_event();
        event.organizer = Some(GraphRecipient {
            email_address: EmailAddress {
                name: "Organizer".to_string(),
                address: "organizer@example.com".to_string(),
            },
        });
        event.attendees = vec![
            GraphAttendee {
                email_address: EmailAddress {
                    name: "Organizer".to_string(),
                    address: "organizer@example.com".to_string(),
                },
                status: Some(ResponseStatus {
                    response: "none".to_string(),
                }),
            },
            GraphAttendee {
                email_address: EmailAddress {
                    name: "Me".to_string(),
                    address: "me@example.com".to_string(),
                },
                status: Some(ResponseStatus {
                    response: "none".to_string(),
                }),
            },
        ];
        event.response_status = Some(ResponseStatus {
            response: "accepted".to_string(),
        });

        let result = from_outlook(event, "me@example.com").unwrap();

        // Owner's status should come from top-level responseStatus, not the attendee array
        let me = result
            .attendees
            .iter()
            .find(|a| a.email == "me@example.com")
            .unwrap();
        assert_eq!(me.response_status, Some(ParticipationStatus::Accepted));

        // Other attendees should still use their per-attendee status
        let organizer_attendee = result
            .attendees
            .iter()
            .find(|a| a.email == "organizer@example.com")
            .unwrap();
        assert_eq!(
            organizer_attendee.response_status,
            Some(ParticipationStatus::NeedsAction)
        );
    }
}
