use axum::Json;
use axum::extract::{Path, State};
use axum::response::sse::{Event, Sse};
use futures_util::stream::Stream;
use std::convert::Infallible;
use tracing::{info, warn};
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

// ==================== Config Handlers ====================

use crate::config::Config;

const MASKED_API_KEY: &str = "sk-****";

pub async fn get_config(State(state): State<AppState>) -> Json<Config> {
    info!("Get config request received");
    let mut config = state.config.read().await.clone();
    
    // 敏感信息脱敏
    if !config.llm.api_key.is_empty() {
        config.llm.api_key = MASKED_API_KEY.to_string();
    }
    
    Json(config)
}

pub async fn update_config(
    State(state): State<AppState>,
    Json(mut new_config): Json<Config>,
) -> Result<Json<Config>> {
    info!("Update config request received");
    
    // 1. 更新内存配置
    {
        let mut config_guard = state.config.write().await;
        
        // 处理敏感信息增量更新
        // 如果新配置中的 API Key 是掩码或为空（视具体需求），则保留旧值
        if new_config.llm.api_key == MASKED_API_KEY || new_config.llm.api_key.is_empty() {
            new_config.llm.api_key = config_guard.llm.api_key.clone();
            info!("Retaining existing API key");
        } else {
            info!("Updating API key");
        }

        *config_guard = new_config.clone();
        
        // 2. 保存到文件
        if let Err(e) = config_guard.save() {
            warn!("Failed to save config to file: {}", e);
            // 这里我们选择不返回错误，因为内存更新已成功，
            // 但记录日志以便排查
        }
    }
    
    // 3. 应用热更新
    // 更新 LLM Provider
    // 注意：这里必须使用包含真实 API Key 的 new_config
    let llm_provider = crate::create_provider(new_config.llm.clone());
    
    // 更新 AppState 中的 Provider
    {
        let mut provider_guard = state.llm_provider.write().await;
        *provider_guard = llm_provider.clone();
    }
    
    // 更新 ToolCoordinator 中的 Provider
    state.tool_coordinator.update_llm_provider(llm_provider).await;
    
    // 更新 MCP Servers
    // 注意：这里是一个简化的实现，直接重启所有服务器
    // 更优的实现应该是 diff 配置，只重启变更的服务器
    {
        let mut mcp_manager = state.mcp_server_manager.lock().await;
        
        // 停止所有现有服务器
        if let Err(e) = mcp_manager.stop_all().await {
            warn!("Failed to stop all MCP servers: {}", e);
        }
        
        // 启动新配置中的服务器
        if let Some(mcp_config) = &new_config.mcp {
            if mcp_config.enabled {
                info!("Restarting {} MCP servers", mcp_config.servers.len());
                for server_config in &mcp_config.servers {
                    if let Err(e) = mcp_manager.start_server(server_config.clone()).await {
                        warn!("Failed to start MCP server {}: {}", server_config.name, e);
                    }
                }
            }
        }
    }

    // 更新 ToolCoordinator 中的工具列表
    // ToolCoordinator 会在每次运行时动态获取工具列表，所以不需要手动更新
    // 只要 McpServerManager 中的状态更新了即可
    // state.tool_coordinator.update_tools(tools).await;
 
    info!("Config updated successfully");
    
    // 返回给客户端时，再次脱敏
    if !new_config.llm.api_key.is_empty() {
        new_config.llm.api_key = MASKED_API_KEY.to_string();
    }
    
    Ok(Json(new_config))
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
