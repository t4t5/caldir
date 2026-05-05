use chrono::Utc;

use super::Calendar;
use crate::error::{CalDirError, CalDirResult};
use crate::event::{Event, Recurrence};
use crate::event_time::EventTime;
use crate::recurrence::truncate_recurrence_before;

impl Calendar {
    /// Split a recurring series at `split_start`.
    ///
    /// The original master's RRULE is truncated to end strictly before
    /// `split_start`, any EXDATEs at or after `split_start` are dropped, and
    /// any override files at or after `split_start` are deleted (they're
    /// either being replaced by the new series or are now orphaned).
    ///
    /// A new master is created starting at `split_start` (with `split_end`
    /// and `new_recurrence`), inheriting all other metadata (summary,
    /// description, location, reminders, attendees, etc.) from the original
    /// master. The new master gets a fresh UID and a reset SEQUENCE.
    ///
    /// Returns the new master event. Errors if no master with `master_uid`
    /// exists or if the master is not recurring.
    pub fn split_recurring_series_at(
        &self,
        master_uid: &str,
        split_start: EventTime,
        split_end: EventTime,
        new_recurrence: Option<Recurrence>,
    ) -> CalDirResult<Event> {
        let all_events = self.events()?;

        // 1. Find the master.
        let master = all_events
            .iter()
            .find(|ce| ce.event.uid == master_uid && ce.event.recurrence_id.is_none())
            .map(|ce| ce.event.clone())
            .ok_or_else(|| {
                CalDirError::Config(format!("Master event not found: {}", master_uid))
            })?;
        let master_recurrence = master
            .recurrence
            .as_ref()
            .ok_or_else(|| CalDirError::Config(format!("Event {} is not recurring", master_uid)))?;

        // 2. Truncate the master's recurrence and write it back.
        let truncated_recurrence =
            truncate_recurrence_before(master_recurrence, &master.start, &split_start);
        let truncated_master = Event {
            recurrence: Some(truncated_recurrence),
            updated: Some(Utc::now()),
            sequence: master.sequence.map(|s| s + 1).or(Some(1)),
            ..master.clone()
        };
        self.update_event(
            &master.uid,
            master.recurrence_id.as_ref(),
            &truncated_master,
        )?;

        // 3. Create the new master, inheriting all metadata from the original.
        let new_master = Event {
            start: split_start.clone(),
            end: split_end,
            recurrence: new_recurrence,
            recurrence_id: None,
            updated: Some(Utc::now()),
            sequence: None,
            ..master.with_new_uid()
        };
        self.create_event(&new_master)?;

        // 4. Delete overrides at or after split_start. Includes the override
        //    at split_start itself (the new master replaces it) and orphaned
        //    overrides at later dates that no longer match an occurrence of
        //    the truncated master.
        let split_start_utc = split_start.to_utc();
        for ce in &all_events {
            if ce.event.uid != master_uid {
                continue;
            }
            let Some(rid) = &ce.event.recurrence_id else {
                continue;
            };
            if let (Some(rid_utc), Some(start_utc)) = (rid.to_utc(), split_start_utc)
                && rid_utc >= start_utc
            {
                self.delete_event(&ce.event.uid, Some(rid))?;
            }
        }

        Ok(new_master)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::test_support::{
        loaded_master, loaded_overrides, make_calendar, make_master, make_override, t,
    };

    #[test]
    fn split_truncates_master_rrule_before_split() {
        let (_tmp, cal) = make_calendar();
        let uid = "master@test";
        let master_start = t(2026, 4, 1, 10, 0);
        cal.create_event(&make_master(uid, master_start, "FREQ=DAILY"))
            .unwrap();

        let split_start = EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0));
        let split_end = EventTime::DateTimeUtc(t(2026, 4, 5, 11, 0));
        cal.split_recurring_series_at(uid, split_start, split_end, None)
            .unwrap();

        let master = loaded_master(&cal, uid);
        let rrule = &master.recurrence.as_ref().unwrap().rrule;
        // UNTIL is one second before split_start, in UTC form.
        assert_eq!(rrule, "FREQ=DAILY;UNTIL=20260405T095959Z");
    }

    #[test]
    fn split_creates_new_master_with_fresh_uid_and_inherited_metadata() {
        let (_tmp, cal) = make_calendar();
        let uid = "master@test";
        cal.create_event(&make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY"))
            .unwrap();

        let new_recurrence = Some(Recurrence {
            rrule: "FREQ=WEEKLY".into(),
            exdates: vec![],
        });
        let new_master = cal
            .split_recurring_series_at(
                uid,
                EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0)),
                EventTime::DateTimeUtc(t(2026, 4, 5, 11, 30)),
                new_recurrence,
            )
            .unwrap();

        // Fresh UID, not the master's.
        assert_ne!(new_master.uid, uid);
        // Inherits metadata.
        assert_eq!(new_master.summary, "Daily standup");
        assert_eq!(new_master.description.as_deref(), Some("Notes"));
        assert_eq!(new_master.location.as_deref(), Some("Office"));
        // Uses the new start/end and recurrence.
        assert_eq!(
            new_master.start,
            EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0))
        );
        assert_eq!(
            new_master.end,
            EventTime::DateTimeUtc(t(2026, 4, 5, 11, 30))
        );
        assert_eq!(new_master.recurrence.as_ref().unwrap().rrule, "FREQ=WEEKLY");
        assert!(new_master.recurrence_id.is_none());

        // And it landed on disk.
        let on_disk = loaded_master(&cal, &new_master.uid);
        assert_eq!(on_disk.uid, new_master.uid);
    }

    #[test]
    fn split_bumps_master_sequence() {
        let (_tmp, cal) = make_calendar();
        let uid = "master@test";
        let mut master = make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY");
        master.sequence = Some(3);
        cal.create_event(&master).unwrap();

        cal.split_recurring_series_at(
            uid,
            EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0)),
            EventTime::DateTimeUtc(t(2026, 4, 5, 11, 0)),
            None,
        )
        .unwrap();

        assert_eq!(loaded_master(&cal, uid).sequence, Some(4));
    }

    #[test]
    fn split_drops_overrides_at_or_after_split_keeps_earlier_ones() {
        let (_tmp, cal) = make_calendar();
        let uid = "master@test";
        cal.create_event(&make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY"))
            .unwrap();

        // Three overrides: before, exactly at, and after the split.
        cal.create_event(&make_override(uid, t(2026, 4, 3, 10, 0), "before"))
            .unwrap();
        cal.create_event(&make_override(uid, t(2026, 4, 5, 10, 0), "at-split"))
            .unwrap();
        cal.create_event(&make_override(uid, t(2026, 4, 7, 10, 0), "after"))
            .unwrap();

        cal.split_recurring_series_at(
            uid,
            EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0)),
            EventTime::DateTimeUtc(t(2026, 4, 5, 11, 0)),
            None,
        )
        .unwrap();

        let overrides = loaded_overrides(&cal, uid);
        assert_eq!(
            overrides.len(),
            1,
            "only the pre-split override should remain"
        );
        assert_eq!(overrides[0].summary, "before");
    }

    #[test]
    fn split_drops_exdates_at_or_after_split() {
        let (_tmp, cal) = make_calendar();
        let uid = "master@test";
        let mut master = make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY");
        let kept = EventTime::DateTimeUtc(t(2026, 4, 3, 10, 0));
        let dropped_at = EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0));
        let dropped_after = EventTime::DateTimeUtc(t(2026, 4, 6, 10, 0));
        master.recurrence = Some(Recurrence {
            rrule: "FREQ=DAILY".into(),
            exdates: vec![kept.clone(), dropped_at, dropped_after],
        });
        cal.create_event(&master).unwrap();

        cal.split_recurring_series_at(
            uid,
            EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0)),
            EventTime::DateTimeUtc(t(2026, 4, 5, 11, 0)),
            None,
        )
        .unwrap();

        assert_eq!(
            loaded_master(&cal, uid).recurrence.unwrap().exdates,
            vec![kept]
        );
    }

    #[test]
    fn split_with_no_new_recurrence_creates_single_event() {
        let (_tmp, cal) = make_calendar();
        let uid = "master@test";
        cal.create_event(&make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY"))
            .unwrap();

        let new_master = cal
            .split_recurring_series_at(
                uid,
                EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0)),
                EventTime::DateTimeUtc(t(2026, 4, 5, 11, 0)),
                None,
            )
            .unwrap();

        assert!(new_master.recurrence.is_none());
    }

    #[test]
    fn split_errors_when_master_not_found() {
        let (_tmp, cal) = make_calendar();
        let err = cal
            .split_recurring_series_at(
                "nonexistent",
                EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0)),
                EventTime::DateTimeUtc(t(2026, 4, 5, 11, 0)),
                None,
            )
            .unwrap_err();
        assert!(format!("{err}").contains("not found"));
    }

    #[test]
    fn split_errors_when_master_is_not_recurring() {
        let (_tmp, cal) = make_calendar();
        let uid = "single@test";
        let mut single = Event::new(
            "Single".into(),
            EventTime::DateTimeUtc(t(2026, 4, 1, 10, 0)),
            EventTime::DateTimeUtc(t(2026, 4, 1, 11, 0)),
            None,
            None,
            None,
            vec![],
        );
        single.uid = uid.into();
        cal.create_event(&single).unwrap();

        let err = cal
            .split_recurring_series_at(
                uid,
                EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0)),
                EventTime::DateTimeUtc(t(2026, 4, 5, 11, 0)),
                None,
            )
            .unwrap_err();
        assert!(format!("{err}").contains("not recurring"));
    }
}
