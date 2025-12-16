use anyhow::{Context, Result};
use google_calendar::types::{MinAccessRole, OrderBy};
use google_calendar::Client;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

use crate::config::{AccountTokens, GcalConfig};
use crate::event::{Attendee, Event, EventStatus, EventTime, Reminder, Transparency};

const REDIRECT_PORT: u16 = 8085;
const REDIRECT_URI: &str = "http://localhost:8085/callback";

const SCOPES: &[&str] = &["https://www.googleapis.com/auth/calendar.readonly"];

/// Create a Google Calendar client from stored tokens
pub fn create_client(config: &GcalConfig, tokens: &AccountTokens) -> Client {
    Client::new(
        config.client_id.clone(),
        config.client_secret.clone(),
        REDIRECT_URI.to_string(),
        tokens.access_token.clone(),
        tokens.refresh_token.clone(),
    )
}

/// Create a new client for initial authentication (no tokens yet)
fn create_auth_client(config: &GcalConfig) -> Client {
    Client::new(
        config.client_id.clone(),
        config.client_secret.clone(),
        REDIRECT_URI.to_string(),
        String::new(),
        String::new(),
    )
}

/// Start a local HTTP server to receive the OAuth callback
/// Returns (code, state)
fn wait_for_callback() -> Result<(String, String)> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", REDIRECT_PORT))
        .with_context(|| format!("Failed to bind to port {}", REDIRECT_PORT))?;

    println!("Waiting for OAuth callback on port {}...", REDIRECT_PORT);

    let (mut stream, _) = listener.accept().context("Failed to accept connection")?;

    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    // Parse the request to get the code and state
    // Request line looks like: GET /callback?code=xxx&state=yyy HTTP/1.1
    let url_part = request_line
        .split_whitespace()
        .nth(1)
        .context("Invalid request")?;

    let url = url::Url::parse(&format!("http://localhost{}", url_part))?;

    let code = url
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.to_string())
        .context("No code in callback")?;

    let state = url
        .query_pairs()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.to_string())
        .context("No state in callback")?;

    // Send a response to the browser
    let response = "HTTP/1.1 200 OK\r\n\
        Content-Type: text/html\r\n\
        Connection: close\r\n\
        \r\n\
        <html><body>\
        <h1>Authentication successful!</h1>\
        <p>You can close this window and return to the terminal.</p>\
        </body></html>";

    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    Ok((code, state))
}

/// Run the full OAuth authentication flow
pub async fn authenticate(config: &GcalConfig) -> Result<AccountTokens> {
    let mut client = create_auth_client(config);

    // Get the authorization URL
    let scopes: Vec<String> = SCOPES.iter().map(|s| s.to_string()).collect();
    let auth_url = client.user_consent_url(&scopes);

    println!("\nOpen this URL in your browser to authenticate:\n");
    println!("{}\n", auth_url);

    // Try to open the browser automatically
    if open::that(&auth_url).is_err() {
        println!("(Could not open browser automatically, please copy the URL above)");
    }

    // Wait for the callback
    let (code, state) = wait_for_callback()?;

    println!("\nReceived authorization code, exchanging for tokens...");

    // Exchange code for tokens
    let access_token = client
        .get_access_token(&code, &state)
        .await
        .context("Failed to exchange code for tokens")?;

    println!("Authentication successful!");

    // Calculate expires_at from expires_in
    let expires_at = if access_token.expires_in > 0 {
        Some(chrono::Utc::now() + chrono::Duration::seconds(access_token.expires_in))
    } else {
        None
    };

    Ok(AccountTokens {
        access_token: access_token.access_token,
        refresh_token: access_token.refresh_token,
        expires_at,
    })
}

/// Refresh an expired access token
pub async fn refresh_token(config: &GcalConfig, tokens: &AccountTokens) -> Result<AccountTokens> {
    let client = create_client(config, tokens);

    let access_token = client
        .refresh_access_token()
        .await
        .context("Failed to refresh token")?;

    // Calculate expires_at from expires_in
    let expires_at = if access_token.expires_in > 0 {
        Some(chrono::Utc::now() + chrono::Duration::seconds(access_token.expires_in))
    } else {
        None
    };

    Ok(AccountTokens {
        access_token: access_token.access_token,
        refresh_token: access_token.refresh_token,
        expires_at,
    })
}

/// Fetch the user's email to verify authentication
pub async fn fetch_user_email(config: &GcalConfig, tokens: &AccountTokens) -> Result<String> {
    let client = create_client(config, tokens);

    // Get calendar list and find primary calendar (its ID is typically the user's email)
    let response = client
        .calendar_list()
        .list_all(MinAccessRole::default(), false, false)
        .await?;

    // Find the primary calendar which typically has the user's email as ID
    for cal in response.body {
        if cal.primary && !cal.id.is_empty() {
            return Ok(cal.id);
        }
    }

    Ok("(unknown email)".to_string())
}

/// A calendar from the user's calendar list
#[derive(Debug)]
pub struct Calendar {
    pub id: String,
    pub name: String,
    pub primary: bool,
    /// API URL for this calendar (for ICS SOURCE property)
    pub source_url: String,
}

/// Fetch the list of calendars for the authenticated user
pub async fn fetch_calendars(config: &GcalConfig, tokens: &AccountTokens) -> Result<Vec<Calendar>> {
    let client = create_client(config, tokens);

    let response = client
        .calendar_list()
        .list_all(MinAccessRole::default(), false, false)
        .await
        .context("Failed to fetch calendars")?;

    Ok(response
        .body
        .into_iter()
        .filter(|c| !c.id.is_empty())
        .map(|c| {
            let source_url = format!(
                "https://www.googleapis.com/calendar/v3/calendars/{}",
                urlencoding::encode(&c.id)
            );
            Calendar {
                id: c.id,
                name: if c.summary.is_empty() {
                    "(unnamed)".to_string()
                } else {
                    c.summary
                },
                primary: c.primary,
                source_url,
            }
        })
        .collect())
}

/// Fetch events from a specific calendar
pub async fn fetch_events(
    config: &GcalConfig,
    tokens: &AccountTokens,
    calendar_id: &str,
) -> Result<Vec<Event>> {
    let client = create_client(config, tokens);

    // Fetch events from 1 year ago to 1 year ahead
    let now = chrono::Utc::now();
    let time_min = (now - chrono::Duration::days(365)).to_rfc3339();
    let time_max = (now + chrono::Duration::days(365)).to_rfc3339();

    let response = client
        .events()
        .list_all(
            calendar_id,
            "",                     // i_cal_uid
            0,                      // max_attendees
            OrderBy::default(),     // order_by
            &[],                    // private_extended_property
            "",                     // q (search query)
            &[],                    // shared_extended_property
            false,                  // show_deleted
            false,                  // show_hidden_invitations
            false,                  // single_events: false to get master events with RRULE
            &time_max,              // time_max
            &time_min,              // time_min
            "",                     // time_zone
            "",                     // updated_min
        )
        .await
        .context("Failed to fetch events")?;

    let mut result = Vec::new();

    for event in response.body {
        // Skip cancelled events or events without an ID
        if event.status == "cancelled" || event.id.is_empty() {
            continue;
        }

        // Parse start time
        let start = if let Some(ref start) = event.start {
            if let Some(dt) = start.date_time {
                EventTime::DateTime(dt)
            } else if let Some(d) = start.date {
                EventTime::Date(d)
            } else {
                continue;
            }
        } else {
            continue;
        };

        // Parse end time
        let end = if let Some(ref end) = event.end {
            if let Some(dt) = end.date_time {
                EventTime::DateTime(dt)
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

        // Extract recurrence fields
        let recurrence = if event.recurrence.is_empty() {
            None
        } else {
            Some(event.recurrence)
        };

        // Parse original start time (for recurring event instances)
        let original_start = if let Some(ref orig) = event.original_start_time {
            if let Some(dt) = orig.date_time {
                Some(EventTime::DateTime(dt))
            } else if let Some(d) = orig.date {
                Some(EventTime::Date(d))
            } else {
                None
            }
        } else {
            None
        };

        // Extract reminders
        let reminders = if let Some(ref rem) = event.reminders {
            rem.overrides
                .iter()
                .map(|r| Reminder {
                    minutes: r.minutes,
                })
                .collect()
        } else {
            Vec::new()
        };

        // Extract transparency (busy/free status)
        let transparency = if event.transparency == "transparent" {
            Transparency::Transparent
        } else {
            Transparency::Opaque // Default
        };

        // Extract organizer
        let organizer = event.organizer.as_ref().map(|o| Attendee {
            name: if o.display_name.is_empty() {
                None
            } else {
                Some(o.display_name.clone())
            },
            email: o.email.clone(),
            response_status: None, // Organizer doesn't have response status
        });

        // Extract attendees
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
                response_status: if a.response_status.is_empty() {
                    None
                } else {
                    Some(a.response_status.clone())
                },
            })
            .collect();

        // Extract conference URL (video call link)
        let conference_url = event.conference_data.as_ref().and_then(|cd| {
            // Find the first video entry point
            cd.entry_points
                .iter()
                .find(|ep| ep.entry_point_type == "video")
                .map(|ep| ep.uri.clone())
        });

        // Build custom properties for Google-specific fields
        let mut custom_properties = Vec::new();
        if let Some(ref url) = conference_url {
            custom_properties.push(("X-GOOGLE-CONFERENCE".to_string(), url.clone()));
        }

        // Extract sync infrastructure fields
        let updated = event.updated;
        let sequence = if event.sequence > 0 {
            Some(event.sequence)
        } else {
            None
        };

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
            updated,
            sequence,
            custom_properties,
        });
    }

    Ok(result)
}
