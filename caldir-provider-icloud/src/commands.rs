pub mod connect;
pub mod create_event;
pub mod delete_event;
pub mod list_calendars;
pub mod list_events;
pub mod update_event;

fn normalize_event(mut event: caldir_core::Event) -> caldir_core::Event {
    // iCloud adds this query-window-dependent marker; RECURRENCE-ID carries identity.
    event
        .x_properties
        .retain(|p| !p.name.eq_ignore_ascii_case("X-RECURRENCE-EXCEPTION"));
    event
}
