//! Convert Microsoft Graph event types to caldir Event.

use anyhow::Result;
use caldir_core::event::{
    Attendee, CustomProperty, Event, EventStatus, EventTime, ParticipationStatus, Recurrence,
    Reminder, Reminders, Transparency,
};
use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;

use crate::constants::{HTML_DESC_PROPERTY, PROVIDER_EVENT_ID_PROPERTY};
use crate::graph_api::types::{
    GraphEvent, PatternedRecurrence, RecurrencePattern, RecurrenceRange,
};

pub fn from_outlook(event: GraphEvent, account_email: &str) -> Result<Event> {
    let start = parse_event_time(
        event.start.as_ref(),
        event.original_start_time_zone.as_deref(),
        event.is_all_day,
        "start",
    )?;
    let end = parse_event_time(
        event.end.as_ref(),
        event.original_end_time_zone.as_deref(),
        event.is_all_day,
        "end",
    )?;

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

    // Outlook bodies arrive as HTML by default. We keep the original markup
    // in an X-ALT-DESC custom property so it round-trips back to Outlook,
    // and store a normalized plaintext version in DESCRIPTION for `ls` /
    // grep / LLM-friendliness. `to_outlook` only re-sends the HTML if the
    // plaintext still matches — so a local edit to DESCRIPTION wins.
    let (description, html_body) = match event.body.as_ref() {
        Some(b) if !b.content.is_empty() => {
            if b.content_type == "html" {
                let text = html_to_plaintext(&b.content);
                if text.is_empty() {
                    (None, None)
                } else {
                    (Some(text), Some(b.content.clone()))
                }
            } else {
                (Some(b.content.clone()), None)
            }
        }
        _ => (None, None),
    };

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

    let mut custom_properties = vec![CustomProperty::new(PROVIDER_EVENT_ID_PROPERTY, event.id)];
    if let Some(html) = html_body {
        // FMTTYPE=text/html identifies the alternate description as HTML per
        // the Outlook/IBM convention. VALUE=TEXT routes the value through
        // RFC 5545 text escaping when written to .ics, so embedded `\r\n`
        // and `;` survive line folding without breaking the file.
        custom_properties.push(
            CustomProperty::new(HTML_DESC_PROPERTY, html)
                .with_param("FMTTYPE", "text/html")
                .with_param("VALUE", "TEXT"),
        );
    }

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
    dtz: Option<&crate::graph_api::types::DateTimeTimeZone>,
    original_timezone: Option<&str>,
    is_all_day: bool,
    field: &str,
) -> Result<EventTime> {
    let dtz = dtz.ok_or_else(|| anyhow::anyhow!("Event has no {field} time"))?;
    parse_datetime_timezone(
        &dtz.date_time,
        &dtz.time_zone,
        original_timezone,
        is_all_day,
    )
}

fn parse_datetime_timezone(
    datetime_str: &str,
    timezone: &str,
    original_timezone: Option<&str>,
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
        // Graph defaults `start.timeZone` to UTC on read, but `originalStartTimeZone`
        // preserves what the event was created in. Recover that zone so a
        // round-trip (push 12:00 Europe/London → fetch → status) doesn't show
        // a phantom "11:00 → 12:00" change. Fall back to UTC if the original
        // is itself UTC, missing, or a Windows name we can't map to IANA.
        if let Some(original) = original_timezone
            && !original.is_empty()
            && original != "UTC"
            && original != "tzone://Microsoft/Utc"
        {
            let tzid = normalize_timezone(original);
            if let Ok(tz) = tzid.parse::<Tz>() {
                let local = Utc.from_utc_datetime(&dt).with_timezone(&tz).naive_local();
                return Ok(EventTime::DateTimeZoned {
                    datetime: local,
                    tzid,
                });
            }
        }
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
fn recurrence_from_outlook(rec: &PatternedRecurrence) -> Result<Recurrence> {
    let mut parts = Vec::new();

    let freq = match &rec.pattern {
        RecurrencePattern::Daily { .. } => "DAILY",
        RecurrencePattern::Weekly { .. } => "WEEKLY",
        RecurrencePattern::AbsoluteMonthly { .. } | RecurrencePattern::RelativeMonthly { .. } => {
            "MONTHLY"
        }
        RecurrencePattern::AbsoluteYearly { .. } | RecurrencePattern::RelativeYearly { .. } => {
            "YEARLY"
        }
    };
    parts.push(format!("FREQ={freq}"));

    let interval = match &rec.pattern {
        RecurrencePattern::Daily { interval }
        | RecurrencePattern::Weekly { interval, .. }
        | RecurrencePattern::AbsoluteMonthly { interval, .. }
        | RecurrencePattern::RelativeMonthly { interval, .. }
        | RecurrencePattern::AbsoluteYearly { interval, .. }
        | RecurrencePattern::RelativeYearly { interval, .. } => *interval,
    };
    if interval > 1 {
        parts.push(format!("INTERVAL={interval}"));
    }

    match &rec.pattern {
        RecurrencePattern::Weekly { days_of_week, .. } => {
            let days: Vec<&str> = days_of_week
                .iter()
                .filter_map(|d| outlook_day_to_rrule(d))
                .collect();
            if !days.is_empty() {
                parts.push(format!("BYDAY={}", days.join(",")));
            }
        }
        RecurrencePattern::RelativeMonthly {
            days_of_week,
            index,
            ..
        }
        | RecurrencePattern::RelativeYearly {
            days_of_week,
            index,
            ..
        } => {
            let days: Vec<&str> = days_of_week
                .iter()
                .filter_map(|d| outlook_day_to_rrule(d))
                .collect();
            if !days.is_empty() {
                let index_num = outlook_index_to_number(index);
                let prefixed: Vec<String> =
                    days.iter().map(|d| format!("{index_num}{d}")).collect();
                parts.push(format!("BYDAY={}", prefixed.join(",")));
            }
        }
        _ => {}
    }

    if let RecurrencePattern::AbsoluteMonthly { day_of_month, .. }
    | RecurrencePattern::AbsoluteYearly { day_of_month, .. } = &rec.pattern
        && *day_of_month > 0
    {
        parts.push(format!("BYMONTHDAY={day_of_month}"));
    }

    if let RecurrencePattern::AbsoluteYearly { month, .. }
    | RecurrencePattern::RelativeYearly { month, .. } = &rec.pattern
        && *month > 0
    {
        parts.push(format!("BYMONTH={month}"));
    }

    match &rec.range {
        RecurrenceRange::EndDate { end_date, .. } if !end_date.is_empty() => {
            // Convert "2025-12-31" to "20251231"
            let until = end_date.replace('-', "");
            parts.push(format!("UNTIL={until}"));
        }
        RecurrenceRange::Numbered {
            number_of_occurrences,
            ..
        } if *number_of_occurrences > 0 => {
            parts.push(format!("COUNT={number_of_occurrences}"));
        }
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

/// Render an Outlook HTML body as the plaintext we store in DESCRIPTION.
///
/// We use html2text's `TrivialDecorator` to get clean plaintext (no
/// markdown markup like `**bold**` or `[link][1]` references), then trim —
/// trailing newlines and the non-breaking spaces Outlook auto-inserts for
/// empty bodies are both `is_whitespace`, so trim collapses them away.
/// Empty HTML maps to "" so the caller can treat that as "no description".
///
/// The result is also used by `to_outlook` to detect a local edit to
/// DESCRIPTION (compare stripped HTML with the current description); both
/// sides go through this same function so they stay byte-equal.
pub(super) fn html_to_plaintext(html: &str) -> String {
    let cfg = html2text::config::with_decorator(html2text::render::TrivialDecorator::new());
    // Outlook freely sprinkles `&nbsp;` between words (Word-ism). Those
    // arrive as U+00A0 from html2text and look identical to spaces but
    // break grep and our edit-detection comparison — collapse to ASCII.
    cfg.string_from_read(html.as_bytes(), HTML_RENDER_WIDTH)
        .unwrap_or_default()
        .replace('\u{a0}', " ")
        .trim()
        .to_string()
}

/// Effectively-no-wrap width for html2text. Calendar bodies are short and
/// we don't want spurious line breaks in DESCRIPTION.
const HTML_RENDER_WIDTH: usize = 10_000;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_api::types::*;

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
            original_start_time_zone: None,
            original_end_time_zone: None,
            location: None,
            is_all_day: false,
            is_cancelled: false,
            recurrence: None,
            attendees: vec![],
            organizer: None,
            reminder_minutes_before_start: 0,
            is_reminder_on: false,
            show_as: "busy".to_string(),
            last_modified_date_time: None,
            online_meeting: None,
            original_start: None,
            response_status: None,
            event_type: String::new(),
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

    /// What `/me/calendars/{id}/events` returns for a daily recurring event:
    /// a single `seriesMaster` carrying its full recurrence pattern. Compare
    /// to `/calendarView`, which would return ~92 expanded `occurrence`s,
    /// each with its own synthetic `iCalUId`.
    const SERIES_MASTER_JSON: &str = r#"{
        "id": "AAMkAD-master-id",
        "iCalUId": "040000008200E00074C5B7101A82E008000000002F2E84387DD9DC0100000000000000001000000020abc",
        "subject": "Foo",
        "type": "seriesMaster",
        "start": {"dateTime": "2026-05-01T16:00:00.0000000", "timeZone": "UTC"},
        "end":   {"dateTime": "2026-05-01T16:30:00.0000000", "timeZone": "UTC"},
        "isAllDay": false,
        "isCancelled": false,
        "showAs": "busy",
        "originalStart": null,
        "recurrence": {
            "pattern": {
                "type": "daily",
                "interval": 1,
                "month": 0,
                "dayOfMonth": 0,
                "firstDayOfWeek": "sunday",
                "index": "first"
            },
            "range": {
                "type": "endDate",
                "startDate": "2026-05-01",
                "endDate": "2026-08-01",
                "recurrenceTimeZone": "GMT Standard Time",
                "numberOfOccurrences": 0
            }
        }
    }"#;

    #[test]
    fn series_master_becomes_one_event_with_rrule() {
        // Regression: when the provider used `/calendarView` instead of
        // `/events`, this same recurring meeting came back as ~92 expanded
        // occurrences with synthetic iCalUIds — one file per day.  The fix
        // (switch to `/events`) means a series master arrives as a single
        // event carrying its RRULE; caldir stores it as one .ics with a
        // RECURRENCE-RULE rather than 92 dated files.
        let parsed: GraphEvent = serde_json::from_str(SERIES_MASTER_JSON).unwrap();
        let event = from_outlook(parsed, "me@example.com").unwrap();

        assert!(
            event.recurrence_id.is_none(),
            "master must not have a recurrence_id"
        );
        let rec = event
            .recurrence
            .expect("series master must carry recurrence");
        assert_eq!(rec.rrule, "FREQ=DAILY;UNTIL=20260801");
    }

    #[test]
    fn html_body_keeps_original_in_x_alt_desc_and_normalizes_description() {
        // Outlook bodies are HTML by default and arrive full of `\r\n`
        // between tags. We want a clean plaintext DESCRIPTION for ls/grep,
        // and the original markup preserved in X-ALT-DESC so a round-trip
        // back to Outlook keeps the formatting (bold, color, images).
        let mut event = minimal_graph_event();
        let html = "<html>\r\n<body>\r\n<div>Here's a <b>fun</b>&nbsp;little tricky thing to <span style=\"color:red\">decode</span>!</div>\r\n</body>\r\n</html>";
        event.body = Some(GraphBody {
            content: html.to_string(),
            content_type: "html".to_string(),
        });

        let result = from_outlook(event, "me@example.com").unwrap();

        assert_eq!(
            result.description.as_deref(),
            Some("Here's a fun little tricky thing to decode!"),
            "DESCRIPTION should be normalized plaintext"
        );
        let alt = result
            .custom_properties
            .iter()
            .find(|p| p.name == "X-ALT-DESC")
            .expect("X-ALT-DESC must be set when body is HTML");
        assert_eq!(alt.value, html, "X-ALT-DESC must hold the unmodified HTML");
        assert!(
            alt.params
                .iter()
                .any(|(k, v)| k == "FMTTYPE" && v == "text/html"),
            "X-ALT-DESC must carry FMTTYPE=text/html"
        );
        assert!(
            alt.params.iter().any(|(k, v)| k == "VALUE" && v == "TEXT"),
            "X-ALT-DESC must carry VALUE=TEXT for proper escape on .ics write"
        );
    }

    #[test]
    fn html_body_with_only_whitespace_is_dropped() {
        // Outlook auto-generates a near-empty HTML body (just &nbsp; and
        // tags) for events created without a description. That should
        // collapse to no description at all rather than leaking a wall of
        // empty markup into X-ALT-DESC.
        let mut event = minimal_graph_event();
        event.body = Some(GraphBody {
            content: "<html><body><div>&nbsp;</div></body></html>".to_string(),
            content_type: "html".to_string(),
        });

        let result = from_outlook(event, "me@example.com").unwrap();
        assert!(result.description.is_none());
        assert!(
            result
                .custom_properties
                .iter()
                .all(|p| p.name != "X-ALT-DESC"),
            "no X-ALT-DESC should be set when the HTML body is empty"
        );
    }

    #[test]
    fn plain_text_body_does_not_set_x_alt_desc() {
        // If Outlook ever returns a `text` body, there's no HTML to preserve.
        let mut event = minimal_graph_event();
        event.body = Some(GraphBody {
            content: "just plain text".to_string(),
            content_type: "text".to_string(),
        });

        let result = from_outlook(event, "me@example.com").unwrap();
        assert_eq!(result.description.as_deref(), Some("just plain text"));
        assert!(
            result
                .custom_properties
                .iter()
                .all(|p| p.name != "X-ALT-DESC")
        );
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
                attendee_type: String::new(),
            },
            GraphAttendee {
                email_address: EmailAddress {
                    name: "Me".to_string(),
                    address: "me@example.com".to_string(),
                },
                status: Some(ResponseStatus {
                    response: "none".to_string(),
                }),
                attendee_type: String::new(),
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

    #[test]
    fn utc_response_with_original_timezone_recovers_zoned_time() {
        // Regression: Graph defaults `start.timeZone` to UTC on read, but the
        // POST response echoes back the timezone we sent. Without using
        // `originalStartTimeZone` on read, an event created as
        // "12:00 Europe/London" lands locally as DateTimeZoned, then the next
        // status fetch sees DateTimeUtc(11:00Z) and the diff engine reports
        // a phantom "11:00 → 12:00" change.
        let mut event = minimal_graph_event();
        event.start = Some(DateTimeTimeZone {
            date_time: "2026-05-05T11:00:00.0000000".to_string(),
            time_zone: "UTC".to_string(),
        });
        event.end = Some(DateTimeTimeZone {
            date_time: "2026-05-05T12:00:00.0000000".to_string(),
            time_zone: "UTC".to_string(),
        });
        event.original_start_time_zone = Some("GMT Standard Time".to_string());
        event.original_end_time_zone = Some("GMT Standard Time".to_string());

        let result = from_outlook(event, "me@example.com").unwrap();
        match result.start {
            EventTime::DateTimeZoned { datetime, tzid } => {
                assert_eq!(tzid, "Europe/London");
                assert_eq!(
                    datetime.format("%Y-%m-%dT%H:%M:%S").to_string(),
                    "2026-05-05T12:00:00"
                );
            }
            other => panic!("expected DateTimeZoned, got {other:?}"),
        }
        match result.end {
            EventTime::DateTimeZoned { datetime, tzid } => {
                assert_eq!(tzid, "Europe/London");
                assert_eq!(
                    datetime.format("%Y-%m-%dT%H:%M:%S").to_string(),
                    "2026-05-05T13:00:00"
                );
            }
            other => panic!("expected DateTimeZoned, got {other:?}"),
        }
    }

    #[test]
    fn utc_response_with_utc_original_stays_utc() {
        // If the event was actually created in UTC, keep it as DateTimeUtc.
        let mut event = minimal_graph_event();
        event.original_start_time_zone = Some("UTC".to_string());
        event.original_end_time_zone = Some("UTC".to_string());

        let result = from_outlook(event, "me@example.com").unwrap();
        assert!(matches!(result.start, EventTime::DateTimeUtc(_)));
    }

    #[test]
    fn utc_response_with_unknown_original_falls_back_to_utc() {
        // An unmappable Windows zone name (or a tzone://Microsoft/Custom
        // legacy value) shouldn't crash — fall back to UTC rather than
        // producing a DateTimeZoned with an unparseable tzid.
        let mut event = minimal_graph_event();
        event.original_start_time_zone = Some("tzone://Microsoft/Custom".to_string());
        event.original_end_time_zone = Some("tzone://Microsoft/Custom".to_string());

        let result = from_outlook(event, "me@example.com").unwrap();
        assert!(matches!(result.start, EventTime::DateTimeUtc(_)));
    }
}
