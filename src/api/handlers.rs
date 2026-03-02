use axum::Json;
use axum::extract::{Path, State};
use tracing::info;
use uuid::Uuid;

use crate::error::Result;
use crate::llm::ChatMessage;
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

    let chat_messages: Vec<ChatMessage> = session
        .messages
        .iter()
        .map(|m| ChatMessage::from((m.role.clone(), m.content.clone())))
        .collect();

    info!("Calling LLM provider");
    let assistant_response = state.llm_provider.chat(chat_messages).await?;
    info!("LLM response received");

    let assistant_message = Message::new(
        session.id,
        MessageRole::Assistant,
        assistant_response.clone(),
    );
    session.add_message(assistant_message);

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
