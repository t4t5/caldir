//! Core types and functionality for the caldir ecosystem.
//!
//! This crate provides shared types used by both caldir-cli and calendar providers:
//! - `event` - Event types (Event, Attendee, EventTime, etc.)
//! - `protocol` - CLI-provider communication protocol
//! - `error` - Error types (CalDirError, CalDirResult)
//! - `config` - Configuration types (GlobalConfig, LocalConfig, RemoteConfig)
//! - `local` - Local state (LocalEvent, LocalState)
//! - `ics` - ICS file generation and parsing
//! - `provider` - Provider subprocess protocol
//! - `remote` - Remote calendar operations
//! - `sync` - Sync/diff types
//! - `constants` - Shared constants

pub mod caldir;
pub mod calendar;
pub mod calendar_config;
pub mod config;
pub mod constants;
pub mod error;
pub mod event;
pub mod ics;
pub mod local;
pub mod protocol;
pub mod provider;
pub mod remote;
pub mod sync;
