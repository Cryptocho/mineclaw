use axum::{
    Json,
    extract::{Path, Query, State},
    response::sse::{Event, KeepAlive, Sse},
};
use chrono::{DateTime, Utc};
use futures_util::stream::Stream;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use tracing::info;
use uuid::Uuid;

use crate::api::v1::types::{ApiResponse, ListParams, Pagination};
use crate::error::{Error, Result};
use crate::models::{Message, MessageRole, SessionInfo};
use crate::state::AppState;

/// Session DTO for API v1
/// 按照 MINECLAW_API_CONTRACT.md 1.1 章节定义
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionV1 {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status: String,
}

impl From<SessionInfo> for SessionV1 {
    fn from(info: SessionInfo) -> Self {
        Self {
            id: info.id,
            title: info.title.unwrap_or_else(|| "Untitled Session".to_string()),
            created_at: info.created_at,
            updated_at: info.updated_at,
            status: info.state.to_string().to_lowercase(),
        }
    }
}

/// Message DTO for API v1
/// 按照 MINECLAW_API_CONTRACT.md 1.4 章节定义
#[derive(Debug, Serialize, Deserialize)]
pub struct MessageV1 {
    pub id: Uuid,
    pub role: String, // user, assistant, system
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

impl From<Message> for MessageV1 {
    fn from(msg: Message) -> Self {
        let role = match msg.role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
            MessageRole::ToolCall => "tool_call",
            MessageRole::ToolResult => "tool_result",
        };

        Self {
            id: msg.id,
            role: role.to_string(),
            content: msg.content,
            timestamp: msg.timestamp,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageRequestV1 {
    pub content: String,
    #[serde(default)]
    pub use_orchestrator: bool, // Phase 4 核心：开启多 Agent 总控模式
}

#[derive(Debug, Serialize)]
pub struct SendMessageResponseV1 {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<Uuid>,
    pub session_id: Uuid,
}

/// 获取会话列表 (GET /api/v1/sessions)
pub async fn list_sessions(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<Json<ApiResponse<Pagination<SessionV1>>>> {
    info!("V1: List sessions request received");
    let all_sessions = state.session_repo.list().await;

    let page = params.page();
    let page_size = params.page_size();
    let total = all_sessions.len();

    let start = (page - 1) * page_size;
    let end = (start + page_size).min(total);

    let items: Vec<SessionV1> = if start < total {
        all_sessions[start..end]
            .iter()
            .cloned()
            .map(SessionV1::from)
            .collect()
    } else {
        vec![]
    };

    let pagination = Pagination {
        items,
        total,
        page,
        page_size,
        has_more: end < total,
    };

    Ok(Json(ApiResponse::success(pagination)))
}

/// 创建新会话 (POST /api/v1/sessions)
pub async fn create_session(
    State(state): State<AppState>,
    Json(payload): Json<CreateSessionRequest>,
) -> Result<Json<ApiResponse<SessionV1>>> {
    info!("V1: Create session request received: {:?}", payload);
    let mut session = state.session_repo.create().await;
    if let Some(title) = payload.title {
        session.set_title(title);
        state.session_repo.update(session.clone()).await?;
    }

    let info = SessionInfo::from(&session);
    Ok(Json(ApiResponse::success(SessionV1::from(info))))
}

/// 获取单个会话详情 (GET /api/v1/sessions/{id})
pub async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<SessionV1>>> {
    info!("V1: Get session request received: {}", id);
    let session = state
        .session_repo
        .get(&id)
        .await
        .ok_or_else(|| Error::SessionNotFound(id.to_string()))?;

    let info = SessionInfo::from(&session);
    Ok(Json(ApiResponse::success(SessionV1::from(info))))
}

/// 删除会话 (DELETE /api/v1/sessions/{id})
pub async fn delete_session(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>> {
    info!("V1: Delete session request received: {}", id);
    state.session_repo.delete(&id).await?;
    Ok(Json(ApiResponse::success(())))
}

/// 获取会话历史消息 (GET /api/v1/sessions/{id}/messages)
pub async fn list_messages(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Query(params): Query<ListParams>,
) -> Result<Json<ApiResponse<Pagination<MessageV1>>>> {
    info!(
        "V1: List messages request received for session: {}",
        session_id
    );
    let session = state
        .session_repo
        .get(&session_id)
        .await
        .ok_or_else(|| Error::SessionNotFound(session_id.to_string()))?;

    let messages = session.messages;
    let page = params.page();
    let page_size = params.page_size();
    let total = messages.len();

    let start = (page - 1) * page_size;
    let end = (start + page_size).min(total);

    let items: Vec<MessageV1> = if start < total {
        messages[start..end]
            .iter()
            .cloned()
            .map(MessageV1::from)
            .collect()
    } else {
        vec![]
    };

    let pagination = Pagination {
        items,
        total,
        page,
        page_size,
        has_more: end < total,
    };

    Ok(Json(ApiResponse::success(pagination)))
}

/// 发送消息/提交任务 (POST /api/v1/sessions/{id}/messages)
pub async fn send_message(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(payload): Json<SendMessageRequestV1>,
) -> Result<Json<ApiResponse<SendMessageResponseV1>>> {
    info!(
        "V1: Send message request received for session: {}",
        session_id
    );

    let session = state
        .session_repo
        .get(&session_id)
        .await
        .ok_or_else(|| Error::SessionNotFound(session_id.to_string()))?;

    if payload.use_orchestrator {
        // 多 Agent 总控模式 (Phase 4)
        info!("Using orchestrator mode for task");

        // TODO: 对接 orchestrator_executor.assign_task 实现真正的任务调度
        // 目前返回一个虚拟的 task_id 以供前端占位展示
        let task_id = format!("tsk_{}", &Uuid::new_v4().to_string()[..8]);

        Ok(Json(ApiResponse::success(SendMessageResponseV1 {
            task_id: Some(task_id),
            message_id: None,
            session_id,
        })))
    } else {
        // 标准单 Agent 模式 (Phase 3 兼容)
        let user_message = Message::new(session_id, MessageRole::User, payload.content.clone());
        let user_message_id = user_message.id;

        let mut session = session;
        let _ = session.add_message(user_message);

        // 运行 tool_coordinator
        let (_assistant_response, intermediate_messages) =
            state.tool_coordinator.run(session.clone()).await?;

        for msg in intermediate_messages {
            let _ = session.add_message(msg);
        }

        state.session_repo.update(session).await?;

        Ok(Json(ApiResponse::success(SendMessageResponseV1 {
            task_id: None,
            message_id: Some(user_message_id),
            session_id,
        })))
    }
}

/// 会话实时状态流 (GET /api/v1/sessions/{id}/stream)
pub async fn session_stream(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Sse<impl Stream<Item = std::result::Result<Event, Infallible>>> {
    info!(
        "V1: Session stream request received, session_id={}",
        session_id
    );

    // 复用已有的 handle_stream_request
    // 内部会处理 SSE 连接并根据 session_id 推送事件
    crate::api::sse::handle_stream_request(state, session_id, None)
        .await
        .keep_alive(KeepAlive::default())
}
