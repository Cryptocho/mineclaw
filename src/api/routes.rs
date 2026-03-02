use axum::Router;
use axum::routing::{delete, get, post};

use crate::api::handlers;
use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/api/messages", post(handlers::send_message))
        .route("/api/sessions", get(handlers::list_sessions))
        .route("/api/sessions/{id}", get(handlers::get_session))
        .route("/api/sessions/{id}", delete(handlers::delete_session))
        .route("/api/sessions/{id}/messages", get(handlers::list_messages))
        .with_state(state)
}
