//! Core types for the caldir ecosystem.
//!
//! This crate provides shared types used by both caldir-cli and calendar providers.
//! The main abstraction is the `Event` struct, which represents calendar events
//! in a provider-agnostic way.

pub mod event;

// Re-export all event types at crate root for convenience
pub use event::*;
