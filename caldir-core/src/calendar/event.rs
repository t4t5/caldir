mod error;

use crate::{Calendar, Event, EventTime, ParticipationStatus};
use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
pub use error::CalendarEventError;

#[derive(Debug)]
pub struct CalendarEvent {
    event: Event,
    path: PathBuf,
}

impl CalendarEvent {
    pub fn create(calendar: &Calendar, event: Event) -> Result<Self, CalendarEventError> {
        let base_slug = event.base_slug();
        let contents = event.to_ics_string();

        let path = write_best_event_file(calendar.path(), &base_slug, None, contents.as_bytes())?;
        sync_file_mtime(&path, event.last_modified)?;

        Ok(CalendarEvent { event, path })
    }

    pub fn load(path: impl Into<PathBuf>) -> Result<Self, CalendarEventError> {
        let path = path.into();

        if !path.is_file() {
            return Err(CalendarEventError::NotFound(path));
        }

        let contents = std::fs::read_to_string(&path)?;

        let events = Event::from_ics_str(&contents)
            .map_err(|err| CalendarEventError::InvalidEvent(path.clone(), err))?;

        let event = match <[Result<Event, _>; 1]>::try_from(events) {
            Ok([result]) => {
                result.map_err(|err| CalendarEventError::InvalidEvent(path.clone(), err))?
            }
            Err(events) => {
                return Err(CalendarEventError::ExpectedSingleEvent {
                    path,
                    found: events.len(),
                });
            }
        };

        Ok(CalendarEvent { event, path })
    }

    pub fn update(&mut self, event: Event) -> Result<(), CalendarEventError> {
        let base_slug = event.base_slug();
        let contents = event.to_ics_string();
        let dir = self.path.parent().unwrap_or_else(|| Path::new("."));

        let new_path =
            write_best_event_file(dir, &base_slug, Some(&self.path), contents.as_bytes())?;
        sync_file_mtime(&new_path, event.last_modified)?;

        if new_path == self.path {
            self.event = event;
            return Ok(());
        }

        if let Err(err) = std::fs::remove_file(&self.path) {
            let _ = std::fs::remove_file(&new_path);
            return Err(err.into());
        }

        self.event = event;
        self.path = new_path;

        Ok(())
    }

    pub fn delete(self) -> Result<(), CalendarEventError> {
        std::fs::remove_file(self.path).map_err(Into::into)
    }

    pub fn event(&self) -> &Event {
        &self.event
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn filename(&self) -> Option<&str> {
        self.path.file_name().and_then(|name| name.to_str())
    }

    // File mtime
    pub fn modified_at(&self) -> Option<DateTime<Utc>> {
        std::fs::metadata(self.path())
            .ok()
            .and_then(|m| m.modified().ok())
            .map(DateTime::<Utc>::from)
    }

    pub fn update_attendee_status(
        &mut self,
        email: &str,
        status: ParticipationStatus,
    ) -> Result<(), CalendarEventError> {
        let mut event = self.event.clone();

        event.set_attendee_status(email, status)?;

        event.sequence += 1;
        event.last_modified = Some(Utc::now());

        // Save file
        self.update(event)
    }

    /// Add an EXDATE to a recurring master
    pub(crate) fn add_exdate(&mut self, exdate: EventTime) -> Result<(), CalendarEventError> {
        let mut event = self.event.clone();

        let Some(recurrence) = event.recurrence.as_mut() else {
            return Err(CalendarEventError::NotRecurring(
                self.event.uid.as_str().to_string(),
            ));
        };

        let exdate_utc = exdate.to_utc();

        if recurrence
            .exdates
            .iter()
            .any(|ex| ex.to_utc() == exdate_utc)
        {
            return Ok(());
        }

        recurrence.exdates.push(exdate);

        event.last_modified = Some(Utc::now());
        event.sequence += 1;

        self.update(event)
    }
}

// Pin the file mtime to the event's LAST-MODIFIED so direction detection
// reflects when the event was changed, not when bytes hit disk. Without this,
// every pull leaves the file appearing newer than its remote counterpart —
// any later code change that adds a parsed property would surface as an
// outgoing push.
fn sync_file_mtime(
    path: &Path,
    last_modified: Option<DateTime<Utc>>,
) -> Result<(), CalendarEventError> {
    let Some(ts) = last_modified else {
        return Ok(());
    };
    let ft = filetime::FileTime::from_unix_time(ts.timestamp(), ts.timestamp_subsec_nanos());
    filetime::set_file_mtime(path, ft)?;
    Ok(())
}

fn write_best_event_file(
    calendar_dir: &Path,
    base_slug: &str,
    current_path: Option<&Path>,
    contents: &[u8],
) -> Result<PathBuf, CalendarEventError> {
    let mut suffix = 1;

    loop {
        let filename = if suffix == 1 {
            format!("{base_slug}.ics")
        } else {
            format!("{base_slug}-{suffix}.ics")
        };
        let path = calendar_dir.join(filename);

        if current_path == Some(path.as_path()) {
            std::fs::write(&path, contents)?;
            return Ok(path);
        }

        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(err) = file.write_all(contents) {
                    let _ = std::fs::remove_file(&path);
                    return Err(err.into());
                }

                return Ok(path);
            }
            Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                suffix += 1;
            }
            Err(err) => return Err(err.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Attendee;
    use crate::event::EventError;
    use crate::test_utils::test_calendar;
    use crate::test_utils::test_calendar_event;
    use crate::test_utils::test_event;
    use chrono::TimeZone;
    use std::fs;

    fn cal_event_with_attendees(attendees: Vec<Attendee>) -> (tempfile::TempDir, CalendarEvent) {
        let (tmp, calendar) = test_calendar();
        let mut event = test_event();
        event.attendees = attendees;
        let cal_event = CalendarEvent::create(&calendar, event).unwrap();
        (tmp, cal_event)
    }

    #[test]
    fn create_saves_event_to_file() {
        let (_tmp, calendar) = test_calendar();
        let cal_event = CalendarEvent::create(&calendar, test_event()).unwrap();

        assert!(cal_event.path().is_file());
        assert_eq!(
            cal_event.filename(),
            Some("2026-01-01T1200__test-event.ics")
        );
    }

    #[test]
    fn create_generates_unique_filenames_within_calendar() {
        let (_tmp, calendar) = test_calendar();

        let cal_event_1 = CalendarEvent::create(&calendar, test_event()).unwrap();

        assert_eq!(
            cal_event_1.filename(),
            Some("2026-01-01T1200__test-event.ics")
        );

        let cal_event_2 = CalendarEvent::create(&calendar, test_event()).unwrap();

        assert_eq!(
            cal_event_2.filename(),
            Some("2026-01-01T1200__test-event-2.ics")
        );

        let cal_event_3 = CalendarEvent::create(&calendar, test_event()).unwrap();

        assert_eq!(
            cal_event_3.filename(),
            Some("2026-01-01T1200__test-event-3.ics")
        );
    }

    #[test]
    fn create_keeps_base_filenames_in_different_calendars() {
        let (_tmp, calendar_1) = test_calendar();
        let cal_event_1 = CalendarEvent::create(&calendar_1, test_event()).unwrap();

        let (_tmp, calendar_2) = test_calendar();
        let cal_event_2 = CalendarEvent::create(&calendar_2, test_event()).unwrap();

        assert_eq!(
            cal_event_1.filename().unwrap(),
            cal_event_2.filename().unwrap()
        );
    }

    #[test]
    fn load_errors_on_missing_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("missing.ics");

        let err = CalendarEvent::load(path).unwrap_err();

        assert!(matches!(err, CalendarEventError::NotFound(p) if p.ends_with("missing.ics")));
    }

    #[test]
    fn load_errors_on_invalid_ics() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.ics");
        fs::write(&path, "BEGIN:VCALENDAR").unwrap(); // Missing END

        let err = CalendarEvent::load(path).unwrap_err();

        assert!(matches!(err, CalendarEventError::InvalidEvent(p, _) if p.ends_with("test.ics")));
    }

    #[test]
    fn load_parses_valid_ics() {
        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nUID:test-uid@caldir\nDTSTART:20240101T120000Z\nSUMMARY:Test Event\nEND:VEVENT\nEND:VCALENDAR";
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.ics");
        fs::write(&path, ics).unwrap();

        assert!(CalendarEvent::load(path).is_ok());
    }

    #[test]
    fn update_renames_file_when_summary_changes() {
        let (_tmp, mut cal_event) = test_calendar_event();

        assert_eq!(
            cal_event.filename(),
            Some("2026-01-01T1200__test-event.ics")
        );

        let mut event = cal_event.event().clone();
        event.summary = Some("Planning Session".to_string());
        cal_event.update(event).unwrap();

        assert_eq!(
            cal_event.filename(),
            Some("2026-01-01T1200__planning-session.ics")
        );
    }

    #[test]
    fn update_keeps_filename_when_other_properties_change() {
        let (_tmp, mut cal_event) = test_calendar_event();

        assert_eq!(
            cal_event.filename(),
            Some("2026-01-01T1200__test-event.ics")
        );

        let mut event = cal_event.event().clone();
        event.location = Some("Conference Room".to_string());
        cal_event.update(event).unwrap();

        assert_eq!(
            cal_event.filename(),
            Some("2026-01-01T1200__test-event.ics")
        );

        let contents = fs::read_to_string(cal_event.path()).unwrap();
        assert!(contents.contains("LOCATION:Conference Room"));
    }

    #[test]
    fn create_pins_file_mtime_to_event_last_modified() {
        let (_tmp, calendar) = test_calendar();
        let mut event = test_event();
        let when = Utc.with_ymd_and_hms(2025, 6, 15, 12, 0, 0).unwrap();
        event.last_modified = Some(when);

        let cal_event = CalendarEvent::create(&calendar, event).unwrap();

        assert_eq!(cal_event.modified_at(), Some(when));
    }

    #[test]
    fn create_leaves_file_mtime_alone_when_last_modified_is_none() {
        // Fresh local events have no remote LAST-MODIFIED yet — fall back to
        // the OS's wall-clock write time so the file's mtime still tracks
        // local edits.
        let (_tmp, calendar) = test_calendar();
        let event = test_event();
        assert!(event.last_modified.is_none());

        let before = Utc::now();
        let cal_event = CalendarEvent::create(&calendar, event).unwrap();
        let after = Utc::now();

        let mtime = cal_event.modified_at().expect("file should have mtime");
        assert!(mtime >= before - chrono::Duration::seconds(1));
        assert!(mtime <= after + chrono::Duration::seconds(1));
    }

    #[test]
    fn update_pins_file_mtime_to_new_last_modified() {
        let (_tmp, mut cal_event) = test_calendar_event();
        let mut event = cal_event.event().clone();
        let when = Utc.with_ymd_and_hms(2025, 6, 15, 12, 0, 0).unwrap();
        event.last_modified = Some(when);

        cal_event.update(event).unwrap();

        assert_eq!(cal_event.modified_at(), Some(when));
    }

    #[test]
    fn update_attendee_status_sets_partstat_and_persists() {
        let (_tmp, mut cal_event) =
            cal_event_with_attendees(vec![Attendee::new("bob@example.com")]);

        cal_event
            .update_attendee_status("bob@example.com", ParticipationStatus::Accepted)
            .unwrap();

        assert_eq!(
            cal_event.event().attendees[0].status,
            Some(ParticipationStatus::Accepted)
        );

        let contents = fs::read_to_string(cal_event.path()).unwrap();
        assert!(contents.contains("PARTSTAT=ACCEPTED"));
    }

    #[test]
    fn update_attendee_status_bumps_sequence_and_sets_last_modified() {
        let (_tmp, mut cal_event) =
            cal_event_with_attendees(vec![Attendee::new("bob@example.com")]);
        let sequence_before = cal_event.event().sequence;

        let before = Utc::now();
        cal_event
            .update_attendee_status("bob@example.com", ParticipationStatus::Declined)
            .unwrap();
        let after = Utc::now();

        assert_eq!(cal_event.event().sequence, sequence_before + 1);
        let last_modified = cal_event
            .event()
            .last_modified
            .expect("last_modified should be set");
        assert!(last_modified >= before);
        assert!(last_modified <= after);
    }

    #[test]
    fn update_attendee_status_matches_email_case_insensitively() {
        let (_tmp, mut cal_event) =
            cal_event_with_attendees(vec![Attendee::new("Bob@Example.com")]);

        cal_event
            .update_attendee_status("BOB@example.com", ParticipationStatus::Tentative)
            .unwrap();

        assert_eq!(
            cal_event.event().attendees[0].status,
            Some(ParticipationStatus::Tentative)
        );
    }

    #[test]
    fn update_attendee_status_errors_on_unknown_email() {
        let (_tmp, mut cal_event) =
            cal_event_with_attendees(vec![Attendee::new("bob@example.com")]);

        let err = cal_event
            .update_attendee_status("carol@example.com", ParticipationStatus::Accepted)
            .unwrap_err();

        assert!(matches!(
            err,
            CalendarEventError::Event(EventError::AttendeeNotFound { email })
                if email == "carol@example.com"
        ));
    }

    #[test]
    fn update_updates_filename_to_base_when_base_is_available() {
        let (_tmp, calendar) = test_calendar();

        let cal_event_1 = CalendarEvent::create(&calendar, test_event()).unwrap();
        let mut cal_event_2 = CalendarEvent::create(&calendar, test_event()).unwrap();

        assert_eq!(
            cal_event_2.filename(),
            Some("2026-01-01T1200__test-event-2.ics")
        );

        // Delete original event that had "test-event" slug
        cal_event_1
            .delete()
            .expect("Failed to delete calendar event");

        // "test-event" slug is now available for cal_event_2 to use:
        cal_event_2.update(cal_event_2.event().clone()).unwrap();

        assert_eq!(
            cal_event_2.filename(),
            Some("2026-01-01T1200__test-event.ics")
        );
    }

    fn recurring_cal_event() -> (tempfile::TempDir, CalendarEvent) {
        let (tmp, calendar) = test_calendar();
        let mut event = test_event();
        event.recurrence = Some(crate::Recurrence::new("FREQ=DAILY"));
        let cal_event = CalendarEvent::create(&calendar, event).unwrap();
        (tmp, cal_event)
    }

    #[test]
    fn add_exdate_records_exclusion_and_bumps_sequence() {
        let (_tmp, mut cal_event) = recurring_cal_event();
        let exdate = EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2026, 1, 3, 12, 0, 0).unwrap());

        cal_event.add_exdate(exdate.clone()).unwrap();

        assert_eq!(
            cal_event.event().recurrence.as_ref().unwrap().exdates,
            vec![exdate]
        );
        assert_eq!(cal_event.event().sequence, 1);
    }

    #[test]
    fn add_exdate_is_idempotent() {
        let (_tmp, mut cal_event) = recurring_cal_event();
        let exdate = EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2026, 1, 3, 12, 0, 0).unwrap());

        cal_event.add_exdate(exdate.clone()).unwrap();
        cal_event.add_exdate(exdate.clone()).unwrap();

        // No duplicate EXDATE, and the second call didn't bump SEQUENCE again.
        assert_eq!(
            cal_event.event().recurrence.as_ref().unwrap().exdates,
            vec![exdate]
        );
        assert_eq!(cal_event.event().sequence, 1);
    }

    #[test]
    fn add_exdate_errors_when_event_is_not_recurring() {
        let (_tmp, mut cal_event) = test_calendar_event();
        assert!(cal_event.event().recurrence.is_none());

        let err = cal_event
            .add_exdate(EventTime::DateTimeUtc(
                Utc.with_ymd_and_hms(2026, 1, 3, 12, 0, 0).unwrap(),
            ))
            .unwrap_err();

        assert!(matches!(err, CalendarEventError::NotRecurring(_)));
    }
}
