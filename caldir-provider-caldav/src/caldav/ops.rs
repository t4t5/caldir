//! Pure CalDAV operations taking credentials and URLs as parameters.
//!
//! These functions are provider-agnostic and can be used by any CalDAV-based
//! provider (iCloud, generic CalDAV, etc.).

mod resource;

pub mod create_event;
pub mod delete_event;
pub mod discover;
pub mod list_calendars;
pub mod list_events;
pub mod update_event;

pub use create_event::create_event;
pub use delete_event::delete_event;
pub use discover::{DiscoveredEndpoints, discover_endpoints};
pub use list_calendars::{RawCalendar, list_calendars_raw};
pub use list_events::fetch_events;
pub use update_event::update_event;
