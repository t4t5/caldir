use chrono::{Duration, Utc};

use caldir_core::caldir::Caldir;
use caldir_core::event::Event;

use crate::notify::send_notification;

/// Scan all calendars for reminders due in the last 60 seconds and fire notifications.
pub fn check_and_notify() -> Result<(), Box<dyn std::error::Error>> {
    let caldir = Caldir::load()?;
    let now = Utc::now();
    let window_start = now - Duration::seconds(60);

    // Look at events starting within the next 24 hours (covers any reasonable reminder offset)
    let range_start = now - Duration::hours(1);
    let range_end = now + Duration::hours(24);

    for calendar in caldir.calendars() {
        let events = match calendar.events_in_range(range_start, range_end) {
            Ok(events) => events,
            Err(_) => continue,
        };

        for event in &events {
            for reminder in event.reminders.iter() {
                if let Some(trigger_time) = compute_trigger_time(event, reminder.minutes)
                    && trigger_time >= window_start
                    && trigger_time <= now
                {
                    send_notification(event, reminder.minutes)?;
                }
            }
        }
    }

    Ok(())
}

/// Compute when a reminder should fire: event start minus reminder minutes.
fn compute_trigger_time(
    event: &Event,
    minutes_before: i64,
) -> Option<chrono::DateTime<Utc>> {
    let start_utc = event.start.to_utc()?;
    Some(start_utc - Duration::minutes(minutes_before))
}
