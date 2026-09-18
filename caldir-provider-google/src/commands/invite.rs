use anyhow::{Context, Result, bail, ensure};
use caldir_core::Event;
use google_calendar::types::Reminders;
use serde_json::{Value, json};
use url::Url;

use crate::constants::GOOGLE_DEFAULT_REMINDERS_PROPERTY;
use crate::google_event::to_google::participation_status_to_google;
use crate::session::Session;

/// Update our response and personal reminders on an invitation.
pub(crate) async fn patch_invite_personal_fields(
    session: &Session,
    calendar_id: &str,
    event_id: &str,
    event: &Event,
    account_email: &str,
) -> Result<google_calendar::types::Event> {
    let url = event_url(calendar_id, event_id);
    patch_personal_fields_at_url(session.access_token(), url, event, account_email).await
}

fn event_url(calendar_id: &str, event_id: &str) -> Url {
    let mut url = Url::parse("https://www.googleapis.com/calendar/v3/calendars/").unwrap();
    url.path_segments_mut()
        .unwrap()
        .pop_if_empty()
        .extend([calendar_id, "events", event_id]);
    url
}

async fn patch_personal_fields_at_url(
    access_token: &str,
    url: Url,
    event: &Event,
    account_email: &str,
) -> Result<google_calendar::types::Event> {
    let client = reqwest::Client::new();
    // Read the exact instance being patched, preserving Google's reminder methods.
    let response = client
        .get(url.clone())
        .bearer_auth(access_token)
        .send()
        .await
        .context("failed to fetch invitation before updating personal fields")?;
    if !response.status().is_success() {
        bail!(
            "failed to fetch invitation before updating personal fields: {}",
            response.text().await.unwrap_or_default()
        );
    }
    let remote: google_calendar::types::Event = response
        .json()
        .await
        .context("failed to decode invitation before updating personal fields")?;
    let body = invite_patch_body(event, account_email, remote.reminders.as_ref())?;

    let response = client
        .patch(url)
        .bearer_auth(access_token)
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        bail!(
            "failed to update invitation personal fields: {}",
            error_text
        );
    }

    Ok(response.json().await?)
}

fn invite_patch_body(
    event: &Event,
    account_email: &str,
    remote_reminders: Option<&Reminders>,
) -> Result<Value> {
    let attendee = event
        .find_attendee(account_email)
        .ok_or_else(|| anyhow::anyhow!("No attendee matching {account_email} on event"))?;

    let response_status = attendee
        .status
        .map(participation_status_to_google)
        .unwrap_or("needsAction");
    let mut body = json!({
        "attendeesOmitted": true,
        "attendees": [{
            "email": attendee.email,
            "responseStatus": response_status,
        }],
    });
    if let Some(reminders) = changed_reminders(event, remote_reminders)? {
        body["reminders"] = reminders;
    }
    Ok(body)
}

fn changed_reminders(event: &Event, remote: Option<&Reminders>) -> Result<Option<Value>> {
    let mut offsets: Vec<_> = event
        .reminders
        .iter()
        .map(|reminder| reminder.minutes_before_start)
        .collect();
    ensure!(
        offsets.len() <= 5,
        "Google supports at most 5 reminder overrides; remove {} reminder(s)",
        offsets.len().saturating_sub(5)
    );
    for minutes in &offsets {
        ensure!(
            (0..=40320).contains(minutes),
            "Google reminder offset {minutes} is unsupported; use 0 to 40320 minutes before the event"
        );
    }
    offsets.sort_unstable();
    let use_default = offsets.is_empty()
        && event
            .x_property(GOOGLE_DEFAULT_REMINDERS_PROPERTY)
            .is_some_and(|value| value.eq_ignore_ascii_case("TRUE"));

    let mut remote_overrides: Vec<_> = remote
        .into_iter()
        .flat_map(|reminders| &reminders.overrides)
        .collect();
    remote_overrides.sort_by_key(|reminder| (reminder.minutes, &reminder.method));
    if use_default == remote.is_some_and(|reminders| reminders.use_default)
        && offsets
            == remote_overrides
                .iter()
                .map(|reminder| reminder.minutes)
                .collect::<Vec<_>>()
    {
        return Ok(None);
    }

    let overrides: Vec<_> = offsets
        .into_iter()
        .map(|minutes| {
            // Consume each match so duplicate offsets retain distinct methods.
            let method = remote_overrides
                .iter()
                .position(|reminder| reminder.minutes == minutes)
                .map(|index| remote_overrides.remove(index).method.as_str())
                .unwrap_or("popup");
            json!({"method": method, "minutes": minutes})
        })
        .collect();
    Ok(Some(
        json!({"useDefault": use_default, "overrides": overrides}),
    ))
}

#[cfg(test)]
mod tests;
