//! List events within a time range from a webcal subscription.

use anyhow::Result;
use caldir_core::Event;
use caldir_core::rpc::ListEvents;
use chrono::{DateTime, Utc};

use crate::http;
use crate::remote_config::WebcalRemoteConfig;

pub async fn handle(cmd: ListEvents) -> Result<Vec<Event>> {
    let config = WebcalRemoteConfig::try_from(&cmd.remote)?;

    let feed = http::fetch_feed(&config.webcal_url).await?;

    let all_events: Vec<Event> = Event::from_ics_str(&feed.body)
        .map_err(|e| anyhow::anyhow!("Failed to parse webcal feed: {e}"))?
        .into_iter()
        .filter_map(|result| match result {
            Ok(event) => Some(event),
            Err(err) => {
                eprintln!("caldir-provider-webcal: skipping malformed event: {err}");
                None
            }
        })
        .map(|mut event| {
            if event.last_modified.is_none() {
                event.last_modified = feed.last_modified;
            }
            event
        })
        .collect();

    let from_utc = DateTime::parse_from_rfc3339(&cmd.from).map(|dt| dt.with_timezone(&Utc))?;

    let to_utc = DateTime::parse_from_rfc3339(&cmd.to).map(|dt| dt.with_timezone(&Utc))?;

    let filtered = all_events
        .into_iter()
        .filter(|event| {
            // Master recurring events pass through; core's recurrence
            // expansion handles per-occurrence range selection later.
            event.recurrence.is_some() || event.occurs_in_range(from_utc, to_utc)
        })
        .collect();

    Ok(filtered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use caldir_core::RemoteConfigParams;

    fn ics_with(events: &str) -> String {
        format!("BEGIN:VCALENDAR\nVERSION:2.0\n{events}END:VCALENDAR\n").replace('\n', "\r\n")
    }

    fn empty_remote() -> RemoteConfigParams {
        // list_events doesn't use the remote at parse time; we only need
        // a value to satisfy the cmd struct shape.
        let mut params = RemoteConfigParams::new();
        params.insert(
            "webcal_url".to_string(),
            toml::Value::String("https://example.invalid/cal.ics".to_string()),
        );
        params
    }

    /// Apply the in-process filter logic without doing the HTTP fetch.
    fn filter_events(body: &str, from: &str, to: &str) -> Vec<Event> {
        filter_events_with_feed_last_modified(body, from, to, None)
    }

    fn filter_events_with_feed_last_modified(
        body: &str,
        from: &str,
        to: &str,
        feed_last_modified: Option<DateTime<Utc>>,
    ) -> Vec<Event> {
        let all: Vec<Event> = Event::from_ics_str(body)
            .unwrap()
            .into_iter()
            .map(Result::unwrap)
            .map(|mut event| {
                if event.last_modified.is_none() {
                    event.last_modified = feed_last_modified;
                }
                event
            })
            .collect();

        let from_utc = DateTime::parse_from_rfc3339(from)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap();

        let to_utc = DateTime::parse_from_rfc3339(to)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap();

        all.into_iter()
            .filter(|event| event.recurrence.is_some() || event.occurs_in_range(from_utc, to_utc))
            .collect()
    }

    #[test]
    fn includes_event_inside_range() {
        let body = ics_with(
            r"BEGIN:VEVENT
UID:in@caldir
DTSTART:20260615T100000Z
DTEND:20260615T110000Z
SUMMARY:Inside
END:VEVENT
",
        );

        let events = filter_events(
            &body,
            "2026-06-01T00:00:00+00:00",
            "2026-06-30T23:59:59+00:00",
        );

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].uid.as_str(), "in@caldir");
    }

    #[test]
    fn excludes_event_before_range() {
        let body = ics_with(
            r"BEGIN:VEVENT
UID:before@caldir
DTSTART:20260101T100000Z
DTEND:20260101T110000Z
SUMMARY:Past
END:VEVENT
",
        );

        let events = filter_events(
            &body,
            "2026-06-01T00:00:00+00:00",
            "2026-06-30T23:59:59+00:00",
        );

        assert!(events.is_empty());
    }

    #[test]
    fn excludes_event_after_range() {
        let body = ics_with(
            r"BEGIN:VEVENT
UID:after@caldir
DTSTART:20260801T100000Z
DTEND:20260801T110000Z
SUMMARY:Future
END:VEVENT
",
        );

        let events = filter_events(
            &body,
            "2026-06-01T00:00:00+00:00",
            "2026-06-30T23:59:59+00:00",
        );

        assert!(events.is_empty());
    }

    #[test]
    fn passes_through_recurring_master_even_when_dtstart_is_outside_range() {
        // Master DTSTART is well before the requested range, but the RRULE
        // means occurrences happen during it. We must return the master so
        // core's recurrence expansion can produce the in-range occurrences.
        let body = ics_with(
            r"BEGIN:VEVENT
UID:weekly@caldir
DTSTART:20240101T100000Z
DTEND:20240101T110000Z
RRULE:FREQ=WEEKLY
SUMMARY:Weekly retro
END:VEVENT
",
        );

        let events = filter_events(
            &body,
            "2026-06-01T00:00:00+00:00",
            "2026-06-30T23:59:59+00:00",
        );

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].uid.as_str(), "weekly@caldir");
        assert!(events[0].recurrence.is_some());
    }

    #[test]
    fn uses_feed_last_modified_when_event_has_none() {
        let body = ics_with(
            r"BEGIN:VEVENT
UID:missing@caldir
DTSTART:20260615T100000Z
DTEND:20260615T110000Z
SUMMARY:Missing
END:VEVENT
",
        );
        let feed_last_modified = DateTime::parse_from_rfc3339("2026-07-13T06:00:11Z")
            .unwrap()
            .with_timezone(&Utc);

        let events = filter_events_with_feed_last_modified(
            &body,
            "2026-06-01T00:00:00+00:00",
            "2026-06-30T23:59:59+00:00",
            Some(feed_last_modified),
        );

        assert_eq!(events[0].last_modified, Some(feed_last_modified));
    }

    #[test]
    fn keeps_event_last_modified_when_present() {
        let body = ics_with(
            r"BEGIN:VEVENT
UID:present@caldir
DTSTART:20260615T100000Z
DTEND:20260615T110000Z
LAST-MODIFIED:20260701T120000Z
SUMMARY:Present
END:VEVENT
",
        );
        let feed_last_modified = DateTime::parse_from_rfc3339("2026-07-13T06:00:11Z")
            .unwrap()
            .with_timezone(&Utc);
        let event_last_modified = DateTime::parse_from_rfc3339("2026-07-01T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let events = filter_events_with_feed_last_modified(
            &body,
            "2026-06-01T00:00:00+00:00",
            "2026-06-30T23:59:59+00:00",
            Some(feed_last_modified),
        );

        assert_eq!(events[0].last_modified, Some(event_last_modified));
    }

    #[test]
    fn try_from_extracts_webcal_url() {
        let params = empty_remote();
        let config = WebcalRemoteConfig::try_from(&params).unwrap();
        assert_eq!(config.webcal_url, "https://example.invalid/cal.ics");
    }
}
