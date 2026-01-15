//! Core library for the caldir ecosystem.
//!
//! This crate provides:
//! - Provider-neutral event types (`Event`, `EventTime`, `Attendee`, etc.)
//! - Calendar management (`Caldir`, `Calendar`)
//! - Provider communication protocol
//! - Bidirectional sync logic
//! - ICS file generation and parsing

// Types (shared with providers)
pub mod calendar_config;
pub mod event;
pub mod protocol;

// Business logic
pub mod caldir;
pub mod calendar;
pub mod config;
pub mod constants;
pub mod diff;
pub mod ics;
pub mod local;
pub mod provider;
pub mod remote;

// Re-export commonly used types at crate root
pub use caldir::Caldir;
pub use calendar::Calendar;
pub use event::*;
