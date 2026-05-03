use chrono::{DateTime, TimeZone, Utc};

use super::Calendar;
use crate::event::{Event, Recurrence};
use crate::event_time::EventTime;

pub fn t(year: i32, month: u32, day: u32, hour: u32, min: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, min, 0)
        .unwrap()
}

/// Build a recurring master event with a fixed UID so tests can find it
/// without depending on the random uuid generator.
pub fn make_master(uid: &str, start_utc: DateTime<Utc>, rrule: &str) -> Event {
    let start = EventTime::DateTimeUtc(start_utc);
    let end = EventTime::DateTimeUtc(start_utc + chrono::Duration::hours(1));
    let mut event = Event::new(
        "Daily standup".into(),
        start,
        end,
        Some("Notes".into()),
        Some("Office".into()),
        Some(Recurrence {
            rrule: rrule.into(),
            exdates: vec![],
        }),
        vec![],
    );
    event.uid = uid.into();
    event
}

/// Build an instance override sharing `master_uid` at `rid_utc`.
pub fn make_override(master_uid: &str, rid_utc: DateTime<Utc>, summary: &str) -> Event {
    let start = EventTime::DateTimeUtc(rid_utc);
    let end = EventTime::DateTimeUtc(rid_utc + chrono::Duration::hours(1));
    let mut event = Event::new(summary.into(), start, end, None, None, None, vec![]);
    event.uid = master_uid.into();
    event.recurrence_id = Some(EventTime::DateTimeUtc(rid_utc));
    event
}

/// Make a Calendar pointing at a fresh tempdir. The TempDir is returned
/// alongside so it stays alive for the test's lifetime.
pub fn make_calendar() -> (tempfile::TempDir, Calendar) {
    let tmp = tempfile::tempdir().unwrap();
    let cal = Calendar::load_in("test", tmp.path()).unwrap();
    std::fs::create_dir_all(cal.data_path()).unwrap();
    (tmp, cal)
}

/// Find the master (recurring) event for `uid` in the calendar's on-disk events.
pub fn loaded_master(cal: &Calendar, uid: &str) -> Event {
    cal.events()
        .unwrap()
        .into_iter()
        .map(|ce| ce.event)
        .find(|e| e.uid == uid && e.recurrence.is_some())
        .expect("master not found on disk")
}

pub fn loaded_overrides(cal: &Calendar, uid: &str) -> Vec<Event> {
    cal.events()
        .unwrap()
        .into_iter()
        .map(|ce| ce.event)
        .filter(|e| e.uid == uid && e.recurrence_id.is_some())
        .collect()
}
