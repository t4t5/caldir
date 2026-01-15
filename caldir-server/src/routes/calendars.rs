//! Calendar and event endpoints

use axum::{
    Router,
    extract::{Path, State},
    routing::{get, post},
    Json,
};
use serde::{Deserialize, Serialize};

use caldir_lib::{Event, EventTime};

use crate::routes::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/calendars", get(list_calendars))
        .route("/calendars/{id}/events", get(list_events))
        .route("/calendars/{id}/events", post(create_event))
}

/// Calendar info returned by API
#[derive(Serialize)]
pub struct CalendarInfo {
    pub name: String,
    pub path: String,
    pub has_remote: bool,
}

/// GET /calendars - List all calendars
async fn list_calendars(State(state): State<AppState>) -> Result<Json<Vec<CalendarInfo>>, AppError> {
    let caldir = state.caldir()?;

    let calendars: Vec<CalendarInfo> = caldir
        .calendars()
        .into_iter()
        .map(|cal| CalendarInfo {
            name: cal.name.clone(),
            path: cal.path.to_string_lossy().to_string(),
            has_remote: cal.remote().is_some(),
        })
        .collect();

    Ok(Json(calendars))
}

/// GET /calendars/:id/events - List events for a calendar
async fn list_events(
    State(state): State<AppState>,
    Path(calendar_id): Path<String>,
) -> Result<Json<Vec<Event>>, AppError> {
    let caldir = state.caldir()?;

    let calendar = caldir
        .calendars()
        .into_iter()
        .find(|c| c.name == calendar_id)
        .ok_or_else(|| anyhow::anyhow!("Calendar not found: {}", calendar_id))?;

    let events: Vec<Event> = calendar
        .events()?
        .into_iter()
        .map(|local| local.event)
        .collect();

    Ok(Json(events))
}

/// Request body for creating an event
#[derive(Deserialize)]
pub struct CreateEventRequest {
    pub summary: String,
    pub start: EventTime,
    pub end: EventTime,
    pub description: Option<String>,
    pub location: Option<String>,
}

/// POST /calendars/:id/events - Create a new event
async fn create_event(
    State(state): State<AppState>,
    Path(calendar_id): Path<String>,
    Json(req): Json<CreateEventRequest>,
) -> Result<Json<Event>, AppError> {
    let caldir = state.caldir()?;

    let calendar = caldir
        .calendars()
        .into_iter()
        .find(|c| c.name == calendar_id)
        .ok_or_else(|| anyhow::anyhow!("Calendar not found: {}", calendar_id))?;

    let mut event = Event::new(req.summary, req.start, req.end);
    event.description = req.description;
    event.location = req.location;

    calendar.create_event(&event)?;

    Ok(Json(event))
}
