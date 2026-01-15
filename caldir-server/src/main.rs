mod routes;
mod singleton;
mod state;

use anyhow::Result;
use axum::Router;
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};

use crate::state::AppState;

const DEFAULT_PORT: u16 = 4096;

#[tokio::main]
async fn main() -> Result<()> {
    // Ensure only one instance is running
    let _lock = singleton::acquire_lock()?;

    let state = AppState::new()?;

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .merge(routes::calendars::router())
        .merge(routes::remote::router())
        .merge(routes::auth::router())
        .with_state(state)
        .layer(cors);

    let addr = SocketAddr::from(([127, 0, 0, 1], DEFAULT_PORT));
    println!("caldir-server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
