//! Lightweight Microsoft Graph API client wrapping reqwest.

use anyhow::{Context, Result};
use reqwest::Client;

const GRAPH_BASE_URL: &str = "https://graph.microsoft.com/v1.0";

pub struct GraphClient {
    client: Client,
    access_token: String,
}

impl GraphClient {
    pub fn new(access_token: &str) -> Self {
        GraphClient {
            client: Client::new(),
            access_token: access_token.to_string(),
        }
    }

    pub async fn get(&self, path: &str) -> Result<reqwest::Response> {
        let url = format!("{}{}", GRAPH_BASE_URL, path);
        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.access_token)
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;
        check_status(response).await
    }

    pub async fn get_url(&self, url: &str) -> Result<reqwest::Response> {
        let response = self
            .client
            .get(url)
            .bearer_auth(&self.access_token)
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;
        check_status(response).await
    }

    pub async fn post(&self, path: &str, body: &serde_json::Value) -> Result<reqwest::Response> {
        let url = format!("{}{}", GRAPH_BASE_URL, path);
        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.access_token)
            .json(body)
            .send()
            .await
            .with_context(|| format!("POST {url}"))?;
        check_status(response).await
    }

    pub async fn patch(&self, path: &str, body: &serde_json::Value) -> Result<reqwest::Response> {
        let url = format!("{}{}", GRAPH_BASE_URL, path);
        let response = self
            .client
            .patch(&url)
            .bearer_auth(&self.access_token)
            .json(body)
            .send()
            .await
            .with_context(|| format!("PATCH {url}"))?;
        check_status(response).await
    }

    pub async fn delete(&self, path: &str) -> Result<reqwest::Response> {
        let url = format!("{}{}", GRAPH_BASE_URL, path);
        let response = self
            .client
            .delete(&url)
            .bearer_auth(&self.access_token)
            .send()
            .await
            .with_context(|| format!("DELETE {url}"))?;
        check_status(response).await
    }
}

async fn check_status(response: reqwest::Response) -> Result<reqwest::Response> {
    if response.status().is_success() {
        Ok(response)
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Graph API error ({}): {}", status, body)
    }
}
