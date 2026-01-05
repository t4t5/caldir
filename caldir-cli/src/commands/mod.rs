// pub mod auth;
// pub mod new;
// pub mod pull;
// pub mod push;
// pub mod status;
pub mod status2;

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use chrono::{Duration, Utc};

use crate::event::Event;
use crate::provider::{Provider, build_params};
use crate::{config, diff, ics, store, sync};

/// Number of days to sync in each direction (past and future)
pub const SYNC_DAYS: i64 = 365;
