//! Credential storage for generic CalDAV authentication.
//!
//! All filesystem IO lives on [`SessionStore`].

mod store;
mod types;

pub use store::SessionStore;
pub use types::Session;
