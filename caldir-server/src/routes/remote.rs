//! Remote sync endpoints (pull, push, status)

use axum::{
    Router,
    extract::State,
    routing::{get, post},
    Json,
};
use serde::Serialize;

use caldir_lib::diff::{DiffKind, EventDiff};

use crate::routes::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/remote/pull", post(pull))
        .route("/remote/push", post(push))
        .route("/remote/status", get(status))
}

/// Result of a sync operation for one calendar
#[derive(Serialize)]
pub struct SyncResult {
    pub calendar: String,
    pub created: usize,
    pub updated: usize,
    pub deleted: usize,
    pub error: Option<String>,
}

/// A pending change for status endpoint
#[derive(Serialize)]
pub struct PendingChange {
    pub calendar: String,
    pub direction: String, // "push" or "pull"
    pub kind: String,      // "create", "update", "delete"
    pub event_summary: String,
    pub event_time: String,
}

fn count_changes(diffs: &[EventDiff]) -> (usize, usize, usize) {
    let mut created = 0;
    let mut updated = 0;
    let mut deleted = 0;

    for diff in diffs {
        match diff.kind {
            DiffKind::Create => created += 1,
            DiffKind::Update => updated += 1,
            DiffKind::Delete => deleted += 1,
        }
    }

    (created, updated, deleted)
}

/// POST /remote/pull - Pull changes from remote for each calendar
async fn pull(State(state): State<AppState>) -> Result<Json<Vec<SyncResult>>, AppError> {
    let caldir = state.caldir()?;
    let mut results = Vec::new();

    for calendar in caldir.calendars() {
        let result = match calendar.get_diff().await {
            Ok(diff) => {
                if let Err(e) = diff.apply_pull() {
                    SyncResult {
                        calendar: calendar.name.clone(),
                        created: 0,
                        updated: 0,
                        deleted: 0,
                        error: Some(e.to_string()),
                    }
                } else {
                    let (created, updated, deleted) = count_changes(&diff.to_pull);
                    SyncResult {
                        calendar: calendar.name.clone(),
                        created,
                        updated,
                        deleted,
                        error: None,
                    }
                }
            }
            Err(e) => SyncResult {
                calendar: calendar.name.clone(),
                created: 0,
                updated: 0,
                deleted: 0,
                error: Some(e.to_string()),
            },
        };
        results.push(result);
    }

    Ok(Json(results))
}

/// POST /remote/push - Push changes to remote for each calendar
async fn push(State(state): State<AppState>) -> Result<Json<Vec<SyncResult>>, AppError> {
    let caldir = state.caldir()?;
    let mut results = Vec::new();

    for calendar in caldir.calendars() {
        let result = match calendar.get_diff().await {
            Ok(diff) => {
                if let Err(e) = diff.apply_push().await {
                    SyncResult {
                        calendar: calendar.name.clone(),
                        created: 0,
                        updated: 0,
                        deleted: 0,
                        error: Some(e.to_string()),
                    }
                } else {
                    let (created, updated, deleted) = count_changes(&diff.to_push);
                    SyncResult {
                        calendar: calendar.name.clone(),
                        created,
                        updated,
                        deleted,
                        error: None,
                    }
                }
            }
            Err(e) => SyncResult {
                calendar: calendar.name.clone(),
                created: 0,
                updated: 0,
                deleted: 0,
                error: Some(e.to_string()),
            },
        };
        results.push(result);
    }

    Ok(Json(results))
}

/// GET /sync/status - Get pending changes for all calendars
async fn status(State(state): State<AppState>) -> Result<Json<Vec<PendingChange>>, AppError> {
    let caldir = state.caldir()?;
    let mut changes = Vec::new();

    for calendar in caldir.calendars() {
        match calendar.get_diff().await {
            Ok(diff) => {
                // Push changes (local -> remote)
                for event_diff in &diff.to_push {
                    changes.push(PendingChange {
                        calendar: calendar.name.clone(),
                        direction: "push".to_string(),
                        kind: kind_to_string(&event_diff.kind),
                        event_summary: event_diff.event().summary.clone(),
                        event_time: event_diff.event().start.to_string(),
                    });
                }

                // Pull changes (remote -> local)
                for event_diff in &diff.to_pull {
                    changes.push(PendingChange {
                        calendar: calendar.name.clone(),
                        direction: "pull".to_string(),
                        kind: kind_to_string(&event_diff.kind),
                        event_summary: event_diff.event().summary.clone(),
                        event_time: event_diff.event().start.to_string(),
                    });
                }
            }
            Err(_) => {
                // Skip calendars that fail to diff (e.g., no remote configured)
            }
        }
    }

    Ok(Json(changes))
}

fn kind_to_string(kind: &DiffKind) -> String {
    match kind {
        DiffKind::Create => "create".to_string(),
        DiffKind::Update => "update".to_string(),
        DiffKind::Delete => "delete".to_string(),
    }
}
