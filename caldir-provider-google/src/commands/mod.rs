pub mod authenticate;
pub mod create_event;
pub mod delete_event;
pub mod list_calendars;
pub mod list_events;
pub mod update_event;

pub use authenticate::handle_authenticate;
pub use create_event::handle_create_event;
pub use delete_event::handle_delete_event;
pub use list_calendars::handle_list_calendars;
pub use list_events::handle_list_events;
pub use update_event::handle_update_event;

use anyhow::Result;
use google_calendar::Client;

use crate::config::GoogleAppConfig;
use crate::google_auth::{redirect_uri, refresh_token, tokens_need_refresh};

pub async fn authed_client(account_email: &str) -> Result<Client> {
    let app = GoogleAppConfig::load()?;
    let account_config = app.account(account_email);
    let creds = &account_config.creds;
    let mut tokens = account_config.load_tokens()?;

    if tokens_need_refresh(&tokens) {
        tokens = refresh_token(&creds, &tokens).await?;
        account_config.save_tokens(&tokens)?;
    }

    Ok(Client::new(
        creds.client_id.clone(),
        creds.client_secret.clone(),
        redirect_uri(),
        tokens.access_token,
        tokens.refresh_token,
    ))
}
