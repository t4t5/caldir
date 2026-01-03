//! Google Calendar API implementation.

use crate::config;
use crate::types::{
    AccountTokens, Attendee, Calendar, Event, EventStatus, EventTime, GoogleCredentials,
    ParticipationStatus, Reminder, Transparency,
};
use anyhow::{Context, Result};
use google_calendar::types::{
    EventAttendee, EventDateTime, EventReminder, MinAccessRole, OrderBy, Reminders, SendUpdates,
};
use google_calendar::Client;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

const REDIRECT_PORT: u16 = 8085;
const REDIRECT_URI: &str = "http://localhost:8085/callback";
const SCOPES: &[&str] = &["https://www.googleapis.com/auth/calendar"];

/// Convert Google's response status to ParticipationStatus
fn google_to_participation_status(google_status: &str) -> Option<ParticipationStatus> {
    match google_status {
        "accepted" => Some(ParticipationStatus::Accepted),
        "declined" => Some(ParticipationStatus::Declined),
        "tentative" => Some(ParticipationStatus::Tentative),
        "needsAction" => Some(ParticipationStatus::NeedsAction),
        _ => None,
    }
}

/// Convert ParticipationStatus to Google's response status format
fn participation_status_to_google(status: ParticipationStatus) -> &'static str {
    match status {
        ParticipationStatus::Accepted => "accepted",
        ParticipationStatus::Declined => "declined",
        ParticipationStatus::Tentative => "tentative",
        ParticipationStatus::NeedsAction => "needsAction",
    }
}

/// Create a Google Calendar client from stored tokens
fn create_client(creds: &GoogleCredentials, tokens: &AccountTokens) -> Client {
    Client::new(
        creds.client_id.clone(),
        creds.client_secret.clone(),
        REDIRECT_URI.to_string(),
        tokens.access_token.clone(),
        tokens.refresh_token.clone(),
    )
}

/// Create a new client for initial authentication (no tokens yet)
fn create_auth_client(creds: &GoogleCredentials) -> Client {
    Client::new(
        creds.client_id.clone(),
        creds.client_secret.clone(),
        REDIRECT_URI.to_string(),
        String::new(),
        String::new(),
    )
}

/// Start a local HTTP server to receive the OAuth callback
fn wait_for_callback() -> Result<(String, String)> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", REDIRECT_PORT))
        .with_context(|| format!("Failed to bind to port {}", REDIRECT_PORT))?;

    eprintln!("Waiting for OAuth callback on port {}...", REDIRECT_PORT);

    let (mut stream, _) = listener.accept().context("Failed to accept connection")?;

    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    // Parse the request to get the code and state
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

/// Run the full OAuth authentication flow.
/// Returns the account email/identifier.
pub async fn authenticate() -> Result<String> {
    let creds = config::load_credentials()?;
    let mut client = create_auth_client(&creds);

    let scopes: Vec<String> = SCOPES.iter().map(|s| s.to_string()).collect();
    let auth_url = client.user_consent_url(&scopes);

    eprintln!("\nOpen this URL in your browser to authenticate:\n");
    eprintln!("{}\n", auth_url);

    // Try to open the browser automatically
    if open::that(&auth_url).is_err() {
        eprintln!("(Could not open browser automatically, please copy the URL above)");
    }

    let (code, state) = wait_for_callback()?;

    eprintln!("\nReceived authorization code, exchanging for tokens...");

    let access_token = client
        .get_access_token(&code, &state)
        .await
        .context("Failed to exchange code for tokens")?;

    let expires_at = if access_token.expires_in > 0 {
        Some(chrono::Utc::now() + chrono::Duration::seconds(access_token.expires_in))
    } else {
        None
    };

    let tokens = AccountTokens {
        access_token: access_token.access_token,
        refresh_token: access_token.refresh_token,
        expires_at,
    };

    // Discover the user's email
    let client = create_client(&creds, &tokens);
    let response = client
        .calendar_list()
        .list_all(MinAccessRole::default(), false, false)
        .await?;

    let email = response
        .body
        .iter()
        .find(|cal| cal.primary)
        .map(|cal| cal.id.clone())
        .unwrap_or_else(|| "(unknown)".to_string());

    // Save tokens for this account
    config::save_tokens(&email, &tokens)?;

    eprintln!("Authentication successful!");

    Ok(email)
}

/// Get tokens for an account, refreshing if needed
async fn get_valid_tokens(account: &str) -> Result<AccountTokens> {
    let creds = config::load_credentials()?;
    let mut tokens = config::load_tokens(account)?;

    if config::tokens_need_refresh(&tokens) {
        eprintln!("Access token expired, refreshing...");
        tokens = refresh_token_internal(&creds, &tokens).await?;
        config::save_tokens(account, &tokens)?;
    }

    Ok(tokens)
}

/// Internal token refresh
async fn refresh_token_internal(creds: &GoogleCredentials, tokens: &AccountTokens) -> Result<AccountTokens> {
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
    let default_time_min = (now - chrono::Duration::days(365)).to_rfc3339();
    let default_time_max = (now + chrono::Duration::days(365)).to_rfc3339();

    let time_min = time_min.unwrap_or(&default_time_min);
    let time_max = time_max.unwrap_or(&default_time_max);

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
            false,                  // single_events
            time_max,
            time_min,
            "",                     // time_zone
            "",                     // updated_min
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

/// Convert EventTime to Google's EventDateTime
fn event_time_to_google(time: &EventTime) -> EventDateTime {
    match time {
        EventTime::Date(d) => EventDateTime {
            date: Some(*d),
            date_time: None,
            time_zone: String::new(),
        },
        EventTime::DateTimeUtc(dt) => EventDateTime {
            date: None,
            date_time: Some(*dt),
            time_zone: String::new(),
        },
        EventTime::DateTimeFloating(dt) => EventDateTime {
            date: None,
            date_time: Some(dt.and_utc()),
            time_zone: String::new(),
        },
        EventTime::DateTimeZoned { datetime, tzid } => EventDateTime {
            date: None,
            date_time: Some(datetime.and_utc()),
            time_zone: tzid.clone(),
        },
    }
}

/// Convert our Event to a Google Calendar API Event
fn to_google_event(event: &Event) -> google_calendar::types::Event {
    let start = event_time_to_google(&event.start);
    let end = event_time_to_google(&event.end);

    let status = match event.status {
        EventStatus::Confirmed => "confirmed".to_string(),
        EventStatus::Tentative => "tentative".to_string(),
        EventStatus::Cancelled => "cancelled".to_string(),
    };

    let transparency = match event.transparency {
        Transparency::Opaque => "opaque".to_string(),
        Transparency::Transparent => "transparent".to_string(),
    };

    let reminders = if event.reminders.is_empty() {
        None
    } else {
        Some(Reminders {
            overrides: event
                .reminders
                .iter()
                .map(|r| EventReminder {
                    method: "popup".to_string(),
                    minutes: r.minutes,
                })
                .collect(),
            use_default: false,
        })
    };

    let attendees: Vec<EventAttendee> = event
        .attendees
        .iter()
        .map(|a| EventAttendee {
            email: a.email.clone(),
            display_name: a.name.clone().unwrap_or_default(),
            response_status: a
                .response_status
                .map(participation_status_to_google)
                .unwrap_or("needsAction")
                .to_string(),
            additional_guests: 0,
            comment: String::new(),
            id: String::new(),
            optional: false,
            organizer: false,
            resource: false,
            self_: false,
        })
        .collect();

    let recurrence = event.recurrence.clone().unwrap_or_default();

    let original_start_time = event.original_start.as_ref().map(event_time_to_google);

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
        ..Default::default()
    }
}

/// Create a new event on Google Calendar
pub async fn create_event(
    account: &str,
    calendar_id: &str,
    event: &Event,
) -> Result<Event> {
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
pub async fn update_event(
    account: &str,
    calendar_id: &str,
    event: &Event,
) -> Result<()> {
    let creds = config::load_credentials()?;
    let tokens = get_valid_tokens(account).await?;
    let client = create_client(&creds, &tokens);

    let google_event = to_google_event(event);

    client
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

    Ok(())
}

/// Delete an event
pub async fn delete_event(
    account: &str,
    calendar_id: &str,
    event_id: &str,
) -> Result<()> {
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

/// Convert a Google Calendar API Event to our Event
fn from_google_event(event: google_calendar::types::Event) -> Result<Event> {
    let start = if let Some(ref start) = event.start {
        if let Some(dt) = start.date_time {
            EventTime::DateTimeUtc(dt)
        } else if let Some(d) = start.date {
            EventTime::Date(d)
        } else {
            anyhow::bail!("Event has no start time");
        }
    } else {
        anyhow::bail!("Event has no start time");
    };

    let end = if let Some(ref end) = event.end {
        if let Some(dt) = end.date_time {
            EventTime::DateTimeUtc(dt)
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
