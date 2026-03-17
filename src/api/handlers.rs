use axum::Json;
use axum::extract::{Path, State};
use axum::response::sse::{Event, Sse};
use futures_util::stream::Stream;
use std::convert::Infallible;
use std::time::Instant;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::api::sse;
use crate::error::Result;
use crate::models::*;
use crate::state::AppState;

// ==================== 调试 API 相关类型 ====================

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct DebugInfoResponse {
    pub version: String,
    pub uptime_seconds: u64,
    pub timestamp: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct DebugConfigResponse {
    pub server_host: String,
    pub server_port: u16,
    pub llm_provider: String,
    pub llm_model: String,
    pub llm_base_url: String,
    pub terminal_enabled: bool,
    pub terminal_timeout_seconds: u64,
    pub terminal_max_output_bytes: usize,
    pub terminal_max_concurrent: usize,
    pub mcp_enabled: bool,
    pub checkpoint_enabled: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct DebugEchoRequest {
    pub message: String,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct DebugEchoResponse {
    pub original_message: String,
    pub original_data: Option<serde_json::Value>,
    pub timestamp: String,
    pub echoed: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct DebugSessionCountResponse {
    pub total_sessions: usize,
    pub timestamp: String,
}

// 终端测试相关类型
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct DebugTerminalRunRequest {
    pub command: String,
    pub timeout_seconds: Option<u64>,
    pub max_output_bytes: Option<usize>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct DebugTerminalRunResponse {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub stdout_len: usize,
    pub stderr_len: usize,
    pub truncated: bool,
    pub is_timeout: bool,
    pub execution_time_ms: u128,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct DebugTerminalTestOutputRequest {
    pub lines: Option<usize>,
    pub line_length: Option<usize>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct DebugTerminalTestTimeoutRequest {
    pub sleep_seconds: Option<u64>,
}

// 服务器启动时间
static START_TIME: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

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
    let _ = session.add_message(user_message);

    info!("Running tool coordinator");
    let (assistant_response, intermediate_messages) =
        state.tool_coordinator.run(session.clone()).await?;
    info!(
        "Tool coordinator finished, intermediate_messages={}",
        intermediate_messages.len()
    );

    // 添加中间消息（工具调用和结果，以及中间的 Assistant 消息）到会话
    for msg in intermediate_messages {
        let _ = session.add_message(msg);
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

// ==================== 调试 API Handlers ====================

pub async fn debug_info() -> Result<Json<DebugInfoResponse>> {
    info!("Debug info requested");

    let start_time = START_TIME.get_or_init(Instant::now);
    let uptime = start_time.elapsed().as_secs();

    Ok(Json(DebugInfoResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: uptime,
        timestamp: chrono::Utc::now().to_rfc3339(),
    }))
}

pub async fn debug_config(State(state): State<AppState>) -> Result<Json<DebugConfigResponse>> {
    info!("Debug config requested");

    let config = &state.config;

    Ok(Json(DebugConfigResponse {
        server_host: config.server.host.clone(),
        server_port: config.server.port,
        llm_provider: config.llm.provider.clone(),
        llm_model: config.llm.model.clone(),
        llm_base_url: config.llm.base_url.clone(),
        terminal_enabled: config.local_tools.terminal.enabled,
        terminal_timeout_seconds: config.local_tools.terminal.timeout_seconds,
        terminal_max_output_bytes: config.local_tools.terminal.max_output_bytes,
        terminal_max_concurrent: config.local_tools.terminal.max_concurrent_processes,
        mcp_enabled: config.mcp.as_ref().map(|m| m.enabled).unwrap_or(false),
        checkpoint_enabled: config.checkpoint.enabled,
    }))
}

pub async fn debug_echo(Json(request): Json<DebugEchoRequest>) -> Result<Json<DebugEchoResponse>> {
    info!("Debug echo requested: {}", request.message);

    Ok(Json(DebugEchoResponse {
        original_message: request.message.clone(),
        original_data: request.data,
        timestamp: chrono::Utc::now().to_rfc3339(),
        echoed: true,
    }))
}

pub async fn debug_session_count(State(state): State<AppState>) -> Result<Json<DebugSessionCountResponse>> {
    info!("Debug session count requested");

    let sessions = state.session_repo.list().await;

    Ok(Json(DebugSessionCountResponse {
        total_sessions: sessions.len(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    }))
}

// ==================== 终端测试 API Handlers ====================

pub async fn debug_terminal_run(
    State(state): State<AppState>,
    Json(request): Json<DebugTerminalRunRequest>,
) -> Result<Json<DebugTerminalRunResponse>> {
    info!("Debug terminal run requested: {}", request.command);

    let start_time = Instant::now();

    // 创建临时session用于工具调用
    let session = Session::new();
    let context = crate::tools::ToolContext::new(session, state.config.clone());

    let params = crate::tools::terminal::RunCommandParams {
        command: request.command.clone(),
        task_id: None,
        detach: false,
        cwd: None,
        stream_output: false,
    };

    let result_value = state
        .local_tool_registry
        .call_tool("run_command", serde_json::to_value(params)?, context)
        .await?;

    let result: crate::tools::terminal::RunCommandResult = serde_json::from_value(result_value)?;

    let execution_time = start_time.elapsed().as_millis();

    Ok(Json(DebugTerminalRunResponse {
        exit_code: result.exit_code,
        stdout: result.stdout.clone(),
        stderr: result.stderr.clone(),
        stdout_len: result.stdout.len(),
        stderr_len: result.stderr.len(),
        truncated: result.truncated,
        is_timeout: result.is_timeout,
        execution_time_ms: execution_time,
    }))
}

pub async fn debug_terminal_test_output(
    State(state): State<AppState>,
    Json(request): Json<DebugTerminalTestOutputRequest>,
) -> Result<Json<DebugTerminalRunResponse>> {
    info!("Debug terminal test output requested");

    let lines = request.lines.unwrap_or(1000);
    let line_length = request.line_length.unwrap_or(100);

    let start_time = Instant::now();

    // 生成大量输出的脚本
    let script = format!(
        "for i in $(seq 1 {}); do printf '%*s\\n' {} '' | tr ' ' 'x'; done",
        lines, line_length
    );

    let session = Session::new();
    let context = crate::tools::ToolContext::new(session, state.config.clone());

    let params = crate::tools::terminal::RunCommandParams {
        command: format!("sh -c \"{}\"", script),
        task_id: None,
        detach: false,
        cwd: None,
        stream_output: false,
    };

    let result_value = state
        .local_tool_registry
        .call_tool("run_command", serde_json::to_value(params)?, context)
        .await?;
    let result: crate::tools::terminal::RunCommandResult = serde_json::from_value(result_value)?;

    let execution_time = start_time.elapsed().as_millis();

    Ok(Json(DebugTerminalRunResponse {
        exit_code: result.exit_code,
        stdout: result.stdout.clone(),
        stderr: result.stderr.clone(),
        stdout_len: result.stdout.len(),
        stderr_len: result.stderr.len(),
        truncated: result.truncated,
        is_timeout: result.is_timeout,
        execution_time_ms: execution_time,
    }))
}

pub async fn debug_terminal_test_timeout(
    State(state): State<AppState>,
    Json(request): Json<DebugTerminalTestTimeoutRequest>,
) -> Result<Json<DebugTerminalRunResponse>> {
    info!("Debug terminal test timeout requested");

    let sleep_seconds = request.sleep_seconds.unwrap_or(10);

    let start_time = Instant::now();

    let session = Session::new();
    let context = crate::tools::ToolContext::new(session, state.config.clone());

    let params = crate::tools::terminal::RunCommandParams {
        command: format!("sleep {}", sleep_seconds),
        task_id: None,
        detach: false,
        cwd: None,
        stream_output: false,
    };

    let result_value = state
        .local_tool_registry
        .call_tool("run_command", serde_json::to_value(params)?, context)
        .await?;
    let result: crate::tools::terminal::RunCommandResult = serde_json::from_value(result_value)?;

    let execution_time = start_time.elapsed().as_millis();

    Ok(Json(DebugTerminalRunResponse {
        exit_code: result.exit_code,
        stdout: result.stdout.clone(),
        stderr: result.stderr.clone(),
        stdout_len: result.stdout.len(),
        stderr_len: result.stderr.len(),
        truncated: result.truncated,
        is_timeout: result.is_timeout,
        execution_time_ms: execution_time,
    }))
}

pub async fn debug_terminal_test_truncation(
    State(state): State<AppState>,
) -> Result<Json<DebugTerminalRunResponse>> {
    info!("Debug terminal test truncation requested");

    let start_time = Instant::now();

    // 生成超长输出（约2MB，超过默认1MB限制）
    let script = "head -c 2000000 /dev/zero | tr '\\0' 'x'";

    let session = Session::new();
    let context = crate::tools::ToolContext::new(session, state.config.clone());

    let params = crate::tools::terminal::RunCommandParams {
        command: format!("sh -c \"{}\"", script),
        task_id: None,
        detach: false,
        cwd: None,
        stream_output: false,
    };

    let result_value = state
        .local_tool_registry
        .call_tool("run_command", serde_json::to_value(params)?, context)
        .await?;
    let result: crate::tools::terminal::RunCommandResult = serde_json::from_value(result_value)?;

    let execution_time = start_time.elapsed().as_millis();

    Ok(Json(DebugTerminalRunResponse {
        exit_code: result.exit_code,
        stdout: result.stdout.clone(),
        stderr: result.stderr.clone(),
        stdout_len: result.stdout.len(),
        stderr_len: result.stderr.len(),
        truncated: result.truncated,
        is_timeout: result.is_timeout,
        execution_time_ms: execution_time,
    }))
}
