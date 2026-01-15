//! Remote sync endpoints (pull, push, status)

use axum::{
    Router,
    extract::State,
    routing::{get, post},
    Json,
};
use serde::Serialize;

use caldir_lib::diff::EventDiff;

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
    pub events: Vec<EventDiff>,
    pub error: Option<String>,
}

/// Status result for one calendar
#[derive(Serialize)]
pub struct StatusResult {
    pub calendar: String,
    pub to_push: Vec<EventDiff>,
    pub to_pull: Vec<EventDiff>,
    pub error: Option<String>,
}

/// POST /remote/pull - Pull changes from remote for each calendar
async fn pull(State(state): State<AppState>) -> Result<Json<Vec<SyncResult>>, AppError> {
    let caldir = state.caldir()?;
    let mut results = Vec::new();

    for calendar in caldir.calendars() {
        let result = match calendar.get_diff().await {
            Ok(diff) => {
                let events = diff.to_pull.clone();

                if let Err(e) = diff.apply_pull() {
                    SyncResult {
                        calendar: calendar.name.clone(),
                        events: Vec::new(),
                        error: Some(e.to_string()),
                    }
                } else {
                    SyncResult {
                        calendar: calendar.name.clone(),
                        events,
                        error: None,
                    }
                }
            }
            Err(e) => SyncResult {
                calendar: calendar.name.clone(),
                events: Vec::new(),
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
                let events = diff.to_push.clone();

                if let Err(e) = diff.apply_push().await {
                    SyncResult {
                        calendar: calendar.name.clone(),
                        events: Vec::new(),
                        error: Some(e.to_string()),
                    }
                } else {
                    SyncResult {
                        calendar: calendar.name.clone(),
                        events,
                        error: None,
                    }
                }
            }
            Err(e) => SyncResult {
                calendar: calendar.name.clone(),
                events: Vec::new(),
                error: Some(e.to_string()),
            },
        };
        results.push(result);
    }

    Ok(Json(results))
}

/// GET /remote/status - Get pending changes for each calendar
async fn status(State(state): State<AppState>) -> Result<Json<Vec<StatusResult>>, AppError> {
    let caldir = state.caldir()?;
    let mut results = Vec::new();

    for calendar in caldir.calendars() {
        let result = match calendar.get_diff().await {
            Ok(diff) => StatusResult {
                calendar: calendar.name.clone(),
                to_push: diff.to_push,
                to_pull: diff.to_pull,
                error: None,
            },
            Err(e) => StatusResult {
                calendar: calendar.name.clone(),
                to_push: Vec::new(),
                to_pull: Vec::new(),
                error: Some(e.to_string()),
            },
        };
        results.push(result);
    }

    Ok(Json(results))
}
