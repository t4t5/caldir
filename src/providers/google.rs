use anyhow::{Context, Result};
use google_calendar::types::{
    EventAttendee, EventDateTime, EventReminder, MinAccessRole, OrderBy, Reminders, SendUpdates,
};
use google_calendar::Client;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

use crate::config::{AccountTokens, GoogleConfig};
use crate::event::{Attendee, Event, EventStatus, EventTime, Reminder, Transparency};

const REDIRECT_PORT: u16 = 8085;
const REDIRECT_URI: &str = "http://localhost:8085/callback";

const SCOPES: &[&str] = &["https://www.googleapis.com/auth/calendar"];

/// Create a Google Calendar client from stored tokens
pub fn create_client(config: &GoogleConfig, tokens: &AccountTokens) -> Client {
    Client::new(
        config.client_id.clone(),
        config.client_secret.clone(),
        REDIRECT_URI.to_string(),
        tokens.access_token.clone(),
        tokens.refresh_token.clone(),
    )
}

/// Create a new client for initial authentication (no tokens yet)
fn create_auth_client(config: &GoogleConfig) -> Client {
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
pub async fn authenticate(config: &GoogleConfig) -> Result<AccountTokens> {
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
pub async fn refresh_token(config: &GoogleConfig, tokens: &AccountTokens) -> Result<AccountTokens> {
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

    // Google typically doesn't return a new refresh_token on refresh responses,
    // so preserve the original one if the response is empty
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

/// Fetch the user's email to verify authentication
pub async fn fetch_user_email(config: &GoogleConfig, tokens: &AccountTokens) -> Result<String> {
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
}

/// Fetch the list of calendars for the authenticated user
pub async fn fetch_calendars(config: &GoogleConfig, tokens: &AccountTokens) -> Result<Vec<Calendar>> {
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
    config: &GoogleConfig,
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
            } else {
                orig.date.map(EventTime::Date)
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

/// Convert our provider-neutral Event to a Google Calendar API Event
fn to_google_event(event: &Event) -> google_calendar::types::Event {
    // Convert start time
    let start = match &event.start {
        EventTime::DateTime(dt) => EventDateTime {
            date: None,
            date_time: Some(*dt),
            time_zone: String::new(),
        },
        EventTime::Date(d) => EventDateTime {
            date: Some(*d),
            date_time: None,
            time_zone: String::new(),
        },
    };

    // Convert end time
    let end = match &event.end {
        EventTime::DateTime(dt) => EventDateTime {
            date: None,
            date_time: Some(*dt),
            time_zone: String::new(),
        },
        EventTime::Date(d) => EventDateTime {
            date: Some(*d),
            date_time: None,
            time_zone: String::new(),
        },
    };

    // Convert status
    let status = match event.status {
        EventStatus::Confirmed => "confirmed".to_string(),
        EventStatus::Tentative => "tentative".to_string(),
        EventStatus::Cancelled => "cancelled".to_string(),
    };

    // Convert transparency
    let transparency = match event.transparency {
        Transparency::Opaque => "opaque".to_string(),
        Transparency::Transparent => "transparent".to_string(),
    };

    // Convert reminders
    let reminders = if event.reminders.is_empty() {
        None
    } else {
        Some(Reminders {
            overrides: event
                .reminders
                .iter()
                .map(|r| EventReminder {
                    method: "popup".to_string(), // Default to popup notifications
                    minutes: r.minutes,
                })
                .collect(),
            use_default: false,
        })
    };

    // Convert attendees (skip organizer since it's read-only)
    let attendees: Vec<EventAttendee> = event
        .attendees
        .iter()
        .map(|a| EventAttendee {
            email: a.email.clone(),
            display_name: a.name.clone().unwrap_or_default(),
            response_status: a.response_status.clone().unwrap_or_default(),
            // Fill required fields with defaults
            additional_guests: 0,
            comment: String::new(),
            id: String::new(),
            optional: false,
            organizer: false,
            resource: false,
            self_: false,
        })
        .collect();

    // Convert recurrence rules
    let recurrence = event.recurrence.clone().unwrap_or_default();

    // Convert original start time for recurring event instances
    let original_start_time = event.original_start.as_ref().map(|os| match os {
        EventTime::DateTime(dt) => EventDateTime {
            date: None,
            date_time: Some(*dt),
            time_zone: String::new(),
        },
        EventTime::Date(d) => EventDateTime {
            date: Some(*d),
            date_time: None,
            time_zone: String::new(),
        },
    });

    google_calendar::types::Event {
        id: event.id.clone(),
        summary: event.summary.clone(),
        description: event.description.clone().unwrap_or_default(),
        location: event.location.clone().unwrap_or_default(),
        start: Some(start),
        end: Some(end),
        status,
        transparency,
        reminders,
        attendees,
        recurrence,
        original_start_time,
        sequence: event.sequence.unwrap_or(0),
        // Leave read-only fields at defaults
        ..Default::default()
    }
}

/// Update an existing event on Google Calendar
pub async fn update_event(
    config: &GoogleConfig,
    tokens: &AccountTokens,
    calendar_id: &str,
    event: &Event,
) -> Result<()> {
    let client = create_client(config, tokens);

    let google_event = to_google_event(event);

    client
        .events()
        .update(
            calendar_id,
            &event.id,
            0,                        // conference_data_version
            0,                        // max_attendees
            false,                    // send_notifications (deprecated)
            SendUpdates::None,        // send_updates
            false,                    // supports_attachments
            &google_event,
        )
        .await
        .with_context(|| format!("Failed to update event: {}", event.summary))?;

    Ok(())
}

/// Create a new event on Google Calendar
/// Returns the created Event with Google-assigned ID and all Google-added fields
pub async fn create_event(
    config: &GoogleConfig,
    tokens: &AccountTokens,
    calendar_id: &str,
    event: &Event,
) -> Result<Event> {
    let client = create_client(config, tokens);

    // Create a google event without the local ID (Google will assign one)
    let mut google_event = to_google_event(event);
    google_event.id = String::new(); // Let Google assign the ID

    let response = client
        .events()
        .insert(
            calendar_id,
            0,                        // conference_data_version
            0,                        // max_attendees
            false,                    // send_notifications (deprecated)
            SendUpdates::None,        // send_updates
            false,                    // supports_attachments
            &google_event,
        )
        .await
        .with_context(|| format!("Failed to create event: {}", event.summary))?;

    // Convert the response back to our Event type to capture Google-added fields
    let created = response.body;
    from_google_event(created)
}

/// Convert a Google Calendar API Event to our provider-neutral Event
fn from_google_event(event: google_calendar::types::Event) -> Result<Event> {
    // Parse start time
    let start = if let Some(ref start) = event.start {
        if let Some(dt) = start.date_time {
            EventTime::DateTime(dt)
        } else if let Some(d) = start.date {
            EventTime::Date(d)
        } else {
            anyhow::bail!("Event has no start time");
        }
    } else {
        anyhow::bail!("Event has no start time");
    };

    // Parse end time
    let end = if let Some(ref end) = event.end {
        if let Some(dt) = end.date_time {
            EventTime::DateTime(dt)
        } else if let Some(d) = end.date {
            EventTime::Date(d)
        } else {
            anyhow::bail!("Event has no end time");
        }
    } else {
        anyhow::bail!("Event has no end time");
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
        } else {
            orig.date.map(EventTime::Date)
        }
    } else {
        None
    };

    // Extract reminders
    let reminders = if let Some(ref rem) = event.reminders {
        rem.overrides
            .iter()
            .map(|r| Reminder { minutes: r.minutes })
            .collect()
    } else {
        Vec::new()
    };

    // Extract transparency (busy/free status)
    let transparency = if event.transparency == "transparent" {
        Transparency::Transparent
    } else {
        Transparency::Opaque
    };

    // Extract organizer
    let organizer = event.organizer.as_ref().map(|o| Attendee {
        name: if o.display_name.is_empty() {
            None
        } else {
            Some(o.display_name.clone())
        },
        email: o.email.clone(),
        response_status: None,
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

    // Extract conference URL
    let conference_url = event.conference_data.as_ref().and_then(|cd| {
        cd.entry_points
            .iter()
            .find(|ep| ep.entry_point_type == "video")
            .map(|ep| ep.uri.clone())
    });

    // Build custom properties
    let mut custom_properties = Vec::new();
    if let Some(ref url) = conference_url {
        custom_properties.push(("X-GOOGLE-CONFERENCE".to_string(), url.clone()));
    }

    Ok(Event {
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
    })
}
