use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use caldir_core::{
    Caldir, Calendar, DateBounds, Event, EventInstanceId, ParticipationStatus, expand_in_range,
};
use chrono::{DateTime, Duration, Utc};

use crate::output::agenda::AgendaEntry;
use crate::output::event::is_visible;
use crate::output::invites::{InviteEntry, InvitesView};
use crate::utils::{require_calendars, resolve_calendars};

pub fn run(caldir: &Caldir, calendar: Option<String>, all: bool) -> Result<InvitesView> {
    require_calendars(caldir)?;
    let calendars = resolve_calendars(caldir, calendar.as_deref())?;

    let tz: chrono_tz::Tz = iana_time_zone::get_timezone()?.parse()?;
    let today = Utc::now().with_timezone(&tz).date_naive();

    let from = today
        .start_of_date()
        .and_local_timezone(tz)
        .earliest()
        .unwrap()
        .with_timezone(&Utc);

    let to = (today + Duration::days(30))
        .end_of_date()
        .and_local_timezone(tz)
        .latest()
        .unwrap()
        .with_timezone(&Utc);

    let mut entries = Vec::new();
    for cal in &calendars {
        entries.extend(collect_invites(cal, from, to, all)?);
    }
    entries.sort_by_key(|invite| invite.entry.event.start.to_utc());

    Ok(InvitesView {
        entries,
        time_format: caldir.config().time_format(),
    })
}

/// Invites in `[from, to]` for the calendar's account, each paired with its source file.
fn collect_invites(
    cal: &Calendar,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    all: bool,
) -> Result<Vec<InviteEntry>> {
    let Some(email) = cal.remote_email() else {
        return Ok(Vec::new());
    };
    let cal_slug = cal.slug().map(str::to_owned);

    let files = cal.events()?;
    let paths: HashMap<EventInstanceId, PathBuf> = files
        .iter()
        .map(|ce| (ce.event().event_instance_id(), ce.path().to_path_buf()))
        .collect();
    let events = expand_in_range(files.iter().map(|ce| ce.event().clone()), from, to);

    let mut invites = Vec::new();
    for event in events {
        if !is_visible(&event) || !event.is_invite_for(email) {
            continue;
        }
        let rsvp = event.attendee_status(email);
        if !all && rsvp != Some(ParticipationStatus::NeedsAction) {
            continue;
        }
        let Some(path) = source_path(&paths, &event) else {
            continue;
        };
        invites.push(InviteEntry {
            path,
            entry: AgendaEntry {
                event,
                calendar: cal_slug.clone(),
                rsvp,
            },
        });
    }

    Ok(invites)
}

/// The file an expanded occurrence came from: its own override, else the series master.
fn source_path(paths: &HashMap<EventInstanceId, PathBuf>, event: &Event) -> Option<PathBuf> {
    paths
        .get(&event.event_instance_id())
        .or_else(|| paths.get(&EventInstanceId::new(event.uid.clone(), None)))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use caldir_core::{CalendarConfig, ProviderSlug, RemoteConfig, RemoteConfigParams};
    use chrono::TimeZone;
    use pretty_assertions::assert_eq;
    use std::path::Path;

    fn calendar(dir: &Path) -> Calendar {
        let mut params = RemoteConfigParams::new();
        params.insert(
            "google_account".to_string(),
            toml::Value::String("me@example.com".to_string()),
        );
        let remote = RemoteConfig::new(ProviderSlug::from("google"), params);
        let config = CalendarConfig::new(None, None, None, Some(remote));
        Calendar::create(&dir.join("work"), Some(config)).unwrap()
    }

    fn write_ics(cal: &Calendar, name: &str, body: &str) -> PathBuf {
        let path = cal.path().join(name);
        let ics = format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\n{}END:VEVENT\r\nEND:VCALENDAR\r\n",
            body.lines()
                .map(|line| format!("{line}\r\n"))
                .collect::<String>()
        );
        std::fs::write(&path, ics).unwrap();
        path
    }

    fn range() -> (DateTime<Utc>, DateTime<Utc>) {
        (
            Utc.with_ymd_and_hms(2026, 9, 8, 0, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2026, 9, 20, 23, 59, 59).unwrap(),
        )
    }

    const SINGLE: &str = "UID:single@caldir
DTSTAMP:20260901T120000Z
DTSTART:20260910T140000Z
DTEND:20260910T150000Z
SUMMARY:Planning
ORGANIZER:mailto:host@example.com
ATTENDEE;PARTSTAT=NEEDS-ACTION:mailto:me@example.com";

    const WEEKLY: &str = "UID:weekly@caldir
DTSTAMP:20260901T120000Z
DTSTART:20260907T090000Z
DTEND:20260907T093000Z
RRULE:FREQ=WEEKLY;COUNT=10
SUMMARY:Standup
ORGANIZER:mailto:host@example.com
ATTENDEE;PARTSTAT=NEEDS-ACTION:mailto:me@example.com";

    const WEEKLY_OVERRIDE: &str = "UID:weekly@caldir
RECURRENCE-ID:20260914T090000Z
DTSTAMP:20260901T120000Z
DTSTART:20260914T100000Z
DTEND:20260914T103000Z
SUMMARY:Standup (moved)
ORGANIZER:mailto:host@example.com
ATTENDEE;PARTSTAT=ACCEPTED:mailto:me@example.com";

    #[test]
    fn pending_invites_point_at_their_source_files() {
        let tmp = tempfile::tempdir().unwrap();
        let cal = calendar(tmp.path());
        let single = write_ics(&cal, "2026-09-10T1400__planning.ics", SINGLE);
        let weekly = write_ics(&cal, "2026-09-07T0900__standup.ics", WEEKLY);
        let (from, to) = range();

        let mut invites = collect_invites(&cal, from, to, false).unwrap();
        invites.sort_by_key(|invite| invite.entry.event.start.to_utc());

        let found: Vec<(&Path, Option<ParticipationStatus>)> = invites
            .iter()
            .map(|invite| (invite.path.as_path(), invite.entry.rsvp))
            .collect();
        assert_eq!(
            found,
            vec![
                (single.as_path(), Some(ParticipationStatus::NeedsAction)),
                (weekly.as_path(), Some(ParticipationStatus::NeedsAction)),
            ]
        );
        assert_eq!(invites[1].entry.event.summary.as_deref(), Some("Standup"));
        assert!(invites[1].entry.event.recurrence_id.is_some());
    }

    #[test]
    fn overridden_occurrence_points_at_override_file() {
        let tmp = tempfile::tempdir().unwrap();
        let cal = calendar(tmp.path());
        write_ics(&cal, "2026-09-07T0900__standup.ics", WEEKLY);
        let moved = write_ics(&cal, "2026-09-14T1000__standup-moved.ics", WEEKLY_OVERRIDE);
        let (from, to) = range();

        assert!(collect_invites(&cal, from, to, false).unwrap().is_empty());

        let invites = collect_invites(&cal, from, to, true).unwrap();
        assert_eq!(invites.len(), 1);
        assert_eq!(invites[0].path, moved);
        assert_eq!(invites[0].entry.rsvp, Some(ParticipationStatus::Accepted));
    }

    #[test]
    fn calendars_without_an_account_have_no_invites() {
        let tmp = tempfile::tempdir().unwrap();
        let cal = Calendar::create(&tmp.path().join("local"), None).unwrap();
        write_ics(&cal, "2026-09-10T1400__planning.ics", SINGLE);
        let (from, to) = range();

        assert!(collect_invites(&cal, from, to, true).unwrap().is_empty());
    }
}
