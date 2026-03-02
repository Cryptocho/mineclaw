mod api;
mod config;
mod error;
mod llm;
mod mcp;
mod models;
mod state;

use std::net::SocketAddr;

use tokio::net::TcpListener;
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::EnvFilter;

use crate::api::create_router;
use crate::config::Config;
use crate::llm::create_provider;
use crate::models::SessionRepository;
use crate::state::AppState;

#[tokio::main]
async fn main() -> error::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let config = Config::load()?;
    info!("Configuration loaded successfully");

    let session_repo = SessionRepository::new();
    let llm_provider = create_provider(config.llm.clone());

    let app_state = AppState::new(session_repo, llm_provider);
    let app = create_router(app_state);

    let addr = SocketAddr::new(config.server.host.parse()?, config.server.port);
    let listener = TcpListener::bind(addr).await?;

    info!("MineClaw server listening on {}", addr);
    info!("Health check: http://{}/health", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
