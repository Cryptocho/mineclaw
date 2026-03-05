use axum::Json;
use axum::extract::{Path, State};
use axum::response::sse::{Event, Sse};
use futures_util::stream::Stream;
use std::convert::Infallible;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::api::sse;
use crate::error::Result;
use crate::models::*;
use crate::state::AppState;

pub async fn health() -> &'static str {
    info!("Health check requested");
    "OK"
}

pub async fn send_message(
    State(state): State<AppState>,
    Json(request): Json<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>> {
    info!("Send message request received");

    let session = if let Some(session_id) = request.session_id {
        info!("Using existing session: {}", session_id);
        state
            .session_repo
            .get(&session_id)
            .await
            .ok_or_else(|| crate::error::Error::SessionNotFound(session_id.to_string()))?
    } else {
        info!("Creating new session");
        state.session_repo.create().await
    };

    let user_message = Message::new(session.id, MessageRole::User, request.content.clone());
    let user_message_id = user_message.id;

    let mut session = session;
    session.add_message(user_message);

    info!("Running tool coordinator");
    let (assistant_response, intermediate_messages) =
        state.tool_coordinator.run(session.messages.clone()).await?;
    info!(
        "Tool coordinator finished, intermediate_messages={}",
        intermediate_messages.len()
    );

    // 添加中间消息（工具调用和结果，以及中间的 Assistant 消息）到会话
    for msg in intermediate_messages {
        session.add_message(msg);
    }

    // 注意：不再额外添加最终的助手回复
    // 因为 ToolCoordinator 已经在 intermediate_messages 中包含了最终回复
    // （当 LLM 只返回文本时，ToolCoordinator 会保存为 Assistant 消息）

    state.session_repo.update(session.clone()).await?;

    info!("Send message response sent, session_id: {}", session.id);

    Ok(Json(SendMessageResponse {
        message_id: user_message_id,
        session_id: session.id,
        assistant_response,
    }))
}

pub async fn get_session(
    State(state): State<AppState>,
    Path(session_id_str): Path<String>,
) -> Result<Json<Session>> {
    info!("Get session request received: {}", session_id_str);

    let session_id = Uuid::parse_str(&session_id_str).map_err(|_| {
        crate::error::Error::InvalidInput(format!("Invalid UUID: {}", session_id_str))
    })?;

    let session = state
        .session_repo
        .get(&session_id)
        .await
        .ok_or_else(|| crate::error::Error::SessionNotFound(session_id.to_string()))?;

    info!("Get session response sent");

    Ok(Json(session))
}

pub async fn list_sessions(State(state): State<AppState>) -> Result<Json<ListSessionsResponse>> {
    info!("List sessions request received");

    let sessions = state.session_repo.list().await;

    info!("List sessions response sent, count: {}", sessions.len());

    Ok(Json(ListSessionsResponse { sessions }))
}

pub async fn list_messages(
    State(state): State<AppState>,
    Path(session_id_str): Path<String>,
) -> Result<Json<ListMessagesResponse>> {
    info!("List messages request received: {}", session_id_str);

    let session_id = Uuid::parse_str(&session_id_str).map_err(|_| {
        crate::error::Error::InvalidInput(format!("Invalid UUID: {}", session_id_str))
    })?;

    let session = state
        .session_repo
        .get(&session_id)
        .await
        .ok_or_else(|| crate::error::Error::SessionNotFound(session_id.to_string()))?;

    info!(
        "List messages response sent, count: {}",
        session.messages.len()
    );

    Ok(Json(ListMessagesResponse {
        messages: session.messages,
    }))
}

pub async fn delete_session(
    State(state): State<AppState>,
    Path(session_id_str): Path<String>,
) -> Result<Json<serde_json::Value>> {
    info!("Delete session request received: {}", session_id_str);

    let session_id = Uuid::parse_str(&session_id_str).map_err(|_| {
        crate::error::Error::InvalidInput(format!("Invalid UUID: {}", session_id_str))
    })?;

    state.session_repo.delete(&session_id).await?;

    info!("Delete session response sent");

    Ok(Json(serde_json::json!({ "success": true })))
}

// ==================== SSE Handlers ====================

pub async fn send_message_stream(
    State(state): State<AppState>,
    Json(request): Json<SendMessageRequest>,
) -> Sse<impl Stream<Item = std::result::Result<Event, Infallible>>> {
    sse::send_message_stream(state, request).await
}

pub async fn session_stream(
    State(state): State<AppState>,
    Path(session_id_str): Path<String>,
) -> Sse<impl Stream<Item = std::result::Result<Event, Infallible>>> {
    let session_id = Uuid::parse_str(&session_id_str).unwrap_or_else(|_| {
        warn!("Invalid UUID in session stream request: {}", session_id_str);
        Uuid::new_v4()
    });
    sse::session_stream(state, session_id).await
}

// ==================== 管理 API Handlers ====================

pub async fn list_tools(
    State(state): State<AppState>,
) -> Result<Json<crate::models::ListToolsResponse>> {
    info!("List tools request received");

    let manager = state.mcp_server_manager.lock().await;
    let all_tools = manager.all_tools();

    let tools: Vec<crate::models::ToolInfo> = all_tools
        .into_iter()
        .map(|(server_name, tool)| crate::models::ToolInfo {
            name: tool.name,
            description: tool.description,
            server_name,
            input_schema: tool.input_schema,
        })
        .collect();

    info!("List tools response sent, count: {}", tools.len());

    Ok(Json(crate::models::ListToolsResponse { tools }))
}

pub async fn list_mcp_servers(
    State(state): State<AppState>,
) -> Result<Json<crate::models::ListMcpServersResponse>> {
    info!("List MCP servers request received");

    let manager = state.mcp_server_manager.lock().await;
    let servers = manager.list_servers();

    let servers_info: Vec<crate::models::McpServerInfo> = servers
        .into_iter()
        .map(|handle| crate::models::McpServerInfo {
            name: handle.name.clone(),
            status: handle.status.clone(),
            tool_count: handle.tools.len(),
            uptime_seconds: handle.uptime_seconds(),
            last_health_check: handle.last_health_check,
        })
        .collect();

    info!(
        "List MCP servers response sent, count: {}",
        servers_info.len()
    );

    Ok(Json(crate::models::ListMcpServersResponse {
        servers: servers_info,
    }))
}

pub async fn restart_mcp_server(
    State(state): State<AppState>,
    Path(server_name): Path<String>,
) -> Result<Json<crate::models::RestartMcpServerResponse>> {
    info!("Restart MCP server request received: {}", server_name);

    let mut manager = state.mcp_server_manager.lock().await;

    match manager.restart_server(&server_name).await {
        Ok(_) => {
            info!("MCP server '{}' restarted successfully", server_name);
            Ok(Json(crate::models::RestartMcpServerResponse {
                success: true,
                message: format!("Server '{}' restarted successfully", server_name),
            }))
        }
        Err(e) => {
            error!("Failed to restart MCP server '{}': {}", server_name, e);
            Ok(Json(crate::models::RestartMcpServerResponse {
                success: false,
                message: format!("Failed to restart server: {}", e),
            }))
        }
    }
}
