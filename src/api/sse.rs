//! SSE 响应处理
//!
//! 实现 Server-Sent Events 流式响应，用于实时推送工具协调器的事件。

use crate::models::{SendMessageRequest, SseEvent};
use crate::state::AppState;
use crate::tool_coordinator::ToolCoordinatorCallback;
use async_trait::async_trait;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::Stream;
use serde_json::Value;
use std::convert::Infallible;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use uuid::Uuid;

// ==================== SseChannel ====================

/// SSE 事件通道
///
/// 用于将工具协调器的事件转发到 SSE 流。
pub struct SseChannel {
    sender: mpsc::UnboundedSender<SseEvent>,
}

impl SseChannel {
    /// 创建一个新的 SSE 通道
    pub fn new() -> (Self, mpsc::UnboundedReceiver<SseEvent>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (Self { sender }, receiver)
    }

    /// 发送事件
    pub fn send(&self, event: SseEvent) {
        let _ = self.sender.send(event);
    }
}

#[async_trait]
impl ToolCoordinatorCallback for SseChannel {
    async fn on_assistant_message(&self, content: &str) {
        self.send(SseEvent::assistant_message(content));
    }

    async fn on_tool_call(&self, tool: &str, arguments: &Value) {
        self.send(SseEvent::tool_call(tool, arguments.clone()));
    }

    async fn on_tool_result(&self, content: &str, is_error: bool) {
        self.send(SseEvent::tool_result(content, is_error));
    }

    async fn on_completed(&self, _final_response: &str) {
        self.send(SseEvent::completed());
    }

    async fn on_error(&self, message: &str) {
        self.send(SseEvent::error(message));
    }
}

// ==================== SSE Handler Helpers ====================

/// 创建 SSE 流
///
/// 将接收器转换为 SSE 流。
fn create_sse_stream(
    mut receiver: mpsc::UnboundedReceiver<SseEvent>,
) -> impl Stream<Item = std::result::Result<Event, Infallible>> {
    async_stream::stream! {
        while let Some(event) = receiver.recv().await {
            match event.to_json() {
                Ok(json) => {
                    yield Ok(Event::default().data(json));
                }
                Err(e) => {
                    warn!("Failed to serialize SSE event: {}", e);
                    let error_event = SseEvent::error(format!("Serialization error: {}", e));
                    if let Ok(error_json) = error_event.to_json() {
                        yield Ok(Event::default().data(error_json));
                    }
                }
            }
        }
    }
}

/// 处理消息流式请求
///
/// 这是内部辅助函数，被 `send_message_stream` 和 `session_stream` 调用。
pub async fn handle_stream_request(
    state: AppState,
    session_id: Uuid,
    content: Option<String>,
) -> Sse<impl Stream<Item = std::result::Result<Event, Infallible>>> {
    info!("Handling stream request, session_id={}", session_id);

    // 创建 SSE 通道
    let (sse_channel, receiver) = SseChannel::new();

    // 立即发送会话开始事件
    sse_channel.send(SseEvent::session_started(session_id.to_string()));

    // 获取会话
    let session = if let Some(session) = state.session_repo.get(&session_id).await {
        session
    } else {
        warn!("Session not found, creating new one");
        state.session_repo.create().await
    };

    // 如果有新内容，添加用户消息并运行工具协调器
    if let Some(content) = content {
        let mut session = session;
        let user_message =
            crate::models::Message::new(session_id, crate::models::MessageRole::User, content);
        let _ = session.add_message(user_message);

        // 保存会话
        let _ = state.session_repo.update(session.clone()).await;

        // 在后台运行工具协调器
        let state_clone = state.clone();
        tokio::spawn(async move {
            let result = state_clone
                .tool_coordinator
                .run_with_callback(session.clone(), sse_channel)
                .await;

            match result {
                Ok((_final_response, intermediate_messages)) => {
                    debug!("Tool coordinator completed successfully");
                    // 更新会话
                    if let Some(mut session) = state_clone.session_repo.get(&session_id).await {
                        for msg in intermediate_messages {
                            let _ = session.add_message(msg);
                        }
                        let _ = state_clone.session_repo.update(session).await;
                    }
                }
                Err(e) => {
                    warn!("Tool coordinator error: {}", e);
                }
            }
        });
    } else {
        // 没有新内容，只发送一个 completed 事件说明会话已存在
        info!("No new content, session exists");
        sse_channel.send(SseEvent::completed());
        // 这里不运行工具协调器，因为没有新的用户消息
        // 如果想要重放历史事件，需要额外的逻辑
    }

    // 创建 SSE 响应
    Sse::new(create_sse_stream(receiver)).keep_alive(KeepAlive::default())
}

/// 发送消息并建立 SSE 流式响应（新建会话）
pub async fn send_message_stream(
    state: AppState,
    request: SendMessageRequest,
) -> Sse<impl Stream<Item = std::result::Result<Event, Infallible>>> {
    info!("Send message stream request received");

    // 创建新会话或使用现有会话
    let session_id = if let Some(session_id) = request.session_id {
        session_id
    } else {
        let session = state.session_repo.create().await;
        session.id
    };

    handle_stream_request(state, session_id, Some(request.content)).await
}

/// 连接现有会话的 SSE 流式响应
pub async fn session_stream(
    state: AppState,
    session_id: Uuid,
) -> Sse<impl Stream<Item = std::result::Result<Event, Infallible>>> {
    info!("Session stream request received, session_id={}", session_id);

    // 对于现有会话，不添加新内容
    handle_stream_request(state, session_id, None).await
}
