//! CalDAV client + pure operations.

pub mod client;
pub mod ops;

pub use client::{
    CalDavClient_, absolute_url, create_caldav_client, event_url, format_caldav_datetime,
    url_to_href,
};
