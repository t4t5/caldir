use crate::config;
use crate::google::actions::get_valid_tokens;
use crate::google::from_google::{from_google_event, google_to_participation_status};
use crate::google::to_google::to_google_event;
use crate::types::{AccountTokens, Calendar, GoogleCredentials};
use anyhow::{Context, Result};
use caldir_core::constants::DEFAULT_SYNC_DAYS;
use caldir_core::{Attendee, Event, EventStatus, EventTime, Reminder, Transparency};
use google_calendar::Client;

use google_calendar::types::{MinAccessRole, OrderBy, SendUpdates};

const REDIRECT_URI: &str = "http://localhost:8085/callback";

/// Create a Google Calendar client from stored tokens
pub fn create_client(creds: &GoogleCredentials, tokens: &AccountTokens) -> Client {
    Client::new(
        creds.client_id.clone(),
        creds.client_secret.clone(),
        REDIRECT_URI.to_string(),
        tokens.access_token.clone(),
        tokens.refresh_token.clone(),
    )
}

/// Create a new client for initial authentication (no tokens yet)
pub fn create_auth_client(creds: &GoogleCredentials) -> Client {
    Client::new(
        creds.client_id.clone(),
        creds.client_secret.clone(),
        REDIRECT_URI.to_string(),
        String::new(),
        String::new(),
    )
}

/// Create a new event on Google Calendar
pub async fn create_event(account: &str, calendar_id: &str, event: &Event) -> Result<Event> {
    let creds = config::load_credentials()?;
    let tokens = get_valid_tokens(account).await?;
    let client = create_client(&creds, &tokens);

    let mut google_event = to_google_event(event);
    google_event.id = String::new(); // Let Google assign the ID

    let response = client
        .events()
        .insert(
            calendar_id,
            0,
            0,
            false,
            SendUpdates::None,
            false,
            &google_event,
        )
        .await
        .with_context(|| format!("Failed to create event: {}", event.summary))?;

    from_google_event(response.body)
}

/// Update an existing event
pub async fn update_event(account: &str, calendar_id: &str, event: &Event) -> Result<Event> {
    let creds = config::load_credentials()?;
    let tokens = get_valid_tokens(account).await?;
    let client = create_client(&creds, &tokens);

    let google_event = to_google_event(event);

    let response = client
        .events()
        .update(
            calendar_id,
            &event.id,
            0,
            0,
            false,
            SendUpdates::None,
            false,
            &google_event,
        )
        .await
        .with_context(|| format!("Failed to update event: {}", event.summary))?;

    from_google_event(response.body)
}

/// Delete an event
pub async fn delete_event(account: &str, calendar_id: &str, event_id: &str) -> Result<()> {
    let creds = config::load_credentials()?;
    let tokens = get_valid_tokens(account).await?;
    let client = create_client(&creds, &tokens);

    let result = client
        .events()
        .delete(calendar_id, event_id, false, SendUpdates::None)
        .await;

    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            let error_str = e.to_string();
            if error_str.contains("410") || error_str.contains("Gone") {
                Ok(())
            } else {
                Err(e).with_context(|| format!("Failed to delete event: {}", event_id))
            }
        }
    }
}

/// Fetch the list of calendars
pub async fn fetch_calendars(account: &str) -> Result<Vec<Calendar>> {
    let creds = config::load_credentials()?;
    let tokens = get_valid_tokens(account).await?;
    let client = create_client(&creds, &tokens);

    let response = client
        .calendar_list()
        .list_all(MinAccessRole::default(), false, false)
        .await
        .context("Failed to fetch calendars")?;

    Ok(response
        .body
        .into_iter()
        .filter(|c| !c.id.is_empty())
        .map(|c| Calendar {
            id: c.id,
            name: if c.summary.is_empty() {
                "(unnamed)".to_string()
            } else {
                c.summary
            },
            primary: c.primary,
        })
        .collect())
}

/// Fetch events from a specific calendar
pub async fn fetch_events(
    account: &str,
    calendar_id: &str,
    time_min: Option<&str>,
    time_max: Option<&str>,
) -> Result<Vec<Event>> {
    let creds = config::load_credentials()?;
    let tokens = get_valid_tokens(account).await?;
    let client = create_client(&creds, &tokens);

    // Default to ±1 year if not specified
    let now = chrono::Utc::now();
    let default_time_min = (now - chrono::Duration::days(DEFAULT_SYNC_DAYS)).to_rfc3339();
    let default_time_max = (now + chrono::Duration::days(DEFAULT_SYNC_DAYS)).to_rfc3339();

    let time_min = time_min.unwrap_or(&default_time_min);
    let time_max = time_max.unwrap_or(&default_time_max);

    let response = client
        .events()
        .list_all(
            calendar_id,
            "",
            0,
            OrderBy::default(),
            &[],
            "", // search query
            &[],
            false,
            false,
            false,
            time_max,
            time_min,
            "",
            "",
        )
        .await
        .context("Failed to fetch events")?;

    let mut result = Vec::new();

    for event in response.body {
        if event.status == "cancelled" || event.id.is_empty() {
            continue;
        }

        let start = if let Some(ref start) = event.start {
            if let Some(dt) = start.date_time {
                EventTime::DateTimeUtc(dt)
            } else if let Some(d) = start.date {
                EventTime::Date(d)
            } else {
                continue;
            }
        } else {
            continue;
        };

        let end = if let Some(ref end) = event.end {
            if let Some(dt) = end.date_time {
                EventTime::DateTimeUtc(dt)
            } else if let Some(d) = end.date {
                EventTime::Date(d)
            } else {
                continue;
            }
        } else {
            continue;
        };

        let status = match event.status.as_str() {
            "tentative" => EventStatus::Tentative,
            "cancelled" => EventStatus::Cancelled,
            _ => EventStatus::Confirmed,
        };

        let recurrence = if event.recurrence.is_empty() {
            None
        } else {
            Some(event.recurrence)
        };

        let original_start = if let Some(ref orig) = event.original_start_time {
            if let Some(dt) = orig.date_time {
                Some(EventTime::DateTimeUtc(dt))
            } else {
                orig.date.map(EventTime::Date)
            }
        } else {
            None
        };

        let reminders = if let Some(ref rem) = event.reminders {
            rem.overrides
                .iter()
                .map(|r| Reminder { minutes: r.minutes })
                .collect()
        } else {
            Vec::new()
        };

        let transparency = if event.transparency == "transparent" {
            Transparency::Transparent
        } else {
            Transparency::Opaque
        };

        let organizer = event.organizer.as_ref().map(|o| Attendee {
            name: if o.display_name.is_empty() {
                None
            } else {
                Some(o.display_name.clone())
            },
            email: o.email.clone(),
            response_status: None,
        });

        let attendees: Vec<Attendee> = event
            .attendees
            .iter()
            .map(|a| Attendee {
                name: if a.display_name.is_empty() {
                    None
                } else {
                    Some(a.display_name.clone())
                },
                email: a.email.clone(),
                response_status: google_to_participation_status(&a.response_status),
            })
            .collect();

        let conference_url = event.conference_data.as_ref().and_then(|cd| {
            cd.entry_points
                .iter()
                .find(|ep| ep.entry_point_type == "video")
                .map(|ep| ep.uri.clone())
        });

        let mut custom_properties = Vec::new();
        if let Some(ref url) = conference_url {
            custom_properties.push(("X-GOOGLE-CONFERENCE".to_string(), url.clone()));
        }

        result.push(Event {
            id: event.id,
            summary: if event.summary.is_empty() {
                "(No title)".to_string()
            } else {
                event.summary
            },
            description: if event.description.is_empty() {
                None
            } else {
                Some(event.description)
            },
            location: if event.location.is_empty() {
                None
            } else {
                Some(event.location)
            },
            start,
            end,
            status,
            recurrence,
            original_start,
            reminders,
            transparency,
            organizer,
            attendees,
            conference_url,
            updated: event.updated,
            sequence: if event.sequence > 0 {
                Some(event.sequence)
            } else {
                None
            },
            custom_properties,
        });
    }

    Ok(result)
}

/// Internal token refresh
pub async fn refresh_token_internal(
    creds: &GoogleCredentials,
    tokens: &AccountTokens,
) -> Result<AccountTokens> {
    let client = create_client(creds, tokens);

    let access_token = client
        .refresh_access_token()
        .await
        .context("Failed to refresh token")?;

    let expires_at = if access_token.expires_in > 0 {
        Some(chrono::Utc::now() + chrono::Duration::seconds(access_token.expires_in))
    } else {
        None
    };

    // Google typically doesn't return a new refresh_token on refresh
    let refresh_token = if access_token.refresh_token.is_empty() {
        tokens.refresh_token.clone()
    } else {
        access_token.refresh_token
    };

    Ok(AccountTokens {
        access_token: access_token.access_token,
        refresh_token,
        expires_at,
    })
}
