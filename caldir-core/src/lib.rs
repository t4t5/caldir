//! Core types for the caldir ecosystem.
//!
//! This crate provides shared types used by both caldir-cli and calendar providers:
//! - `Event` and related types for calendar events
//! - `protocol` module for the CLI-provider communication protocol

pub mod event;
pub mod protocol;

// Re-export all event types at crate root for convenience
pub use event::*;
