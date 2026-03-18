use axum::Router;
use axum::routing::{delete, get, post};

use crate::api::{handlers, v1};
use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    let v1_routes = Router::new()
        .route("/sessions", get(v1::sessions::list_sessions))
        .route("/sessions", post(v1::sessions::create_session))
        .route("/sessions/{id}", get(v1::sessions::get_session))
        .route("/sessions/{id}", delete(v1::sessions::delete_session))
        .route("/sessions/{id}/messages", get(v1::sessions::list_messages))
        .route("/sessions/{id}/messages", post(v1::sessions::send_message))
        .route("/sessions/{id}/stream", get(v1::sessions::session_stream))
        .with_state(state.clone());

    Router::new()
        .nest("/api/v1", v1_routes)
        .route("/health", get(handlers::health))
        .route("/api/messages", post(handlers::send_message))
        .route("/api/messages/stream", post(handlers::send_message_stream))
        .route("/api/sessions", get(handlers::list_sessions))
        .route("/api/sessions/{id}", get(handlers::get_session))
        .route("/api/sessions/{id}", delete(handlers::delete_session))
        .route("/api/sessions/{id}/messages", get(handlers::list_messages))
        .route("/api/sessions/{id}/stream", get(handlers::session_stream))
        // 管理 API
        .route("/api/tools", get(handlers::list_tools))
        .route("/api/mcp/servers", get(handlers::list_mcp_servers))
        .route(
            "/api/mcp/servers/{name}/restart",
            post(handlers::restart_mcp_server),
        )
        // 调试 API
        .route("/api/debug/info", get(handlers::debug_info))
        .route("/api/debug/config", get(handlers::debug_config))
        .route("/api/debug/echo", post(handlers::debug_echo))
        .route(
            "/api/debug/sessions/count",
            get(handlers::debug_session_count),
        )
        // 终端测试 API
        .route(
            "/api/debug/terminal/run",
            post(handlers::debug_terminal_run),
        )
        .route(
            "/api/debug/terminal/test-output",
            post(handlers::debug_terminal_test_output),
        )
        .route(
            "/api/debug/terminal/test-timeout",
            post(handlers::debug_terminal_test_timeout),
        )
        .route(
            "/api/debug/terminal/test-truncation",
            post(handlers::debug_terminal_test_truncation),
        )
        .with_state(state)
}
