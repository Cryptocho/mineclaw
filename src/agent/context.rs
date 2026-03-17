//! 上下文核心类型与存储
//!
//! 提供上下文块的定义、元数据管理和内存存储功能。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::agent::work_order::WorkOrder;
use crate::models::{Message, MessageRole};

// ============================================================================
// ContextId - 上下文块唯一标识
// ============================================================================

/// 上下文块唯一标识
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContextId(Uuid);

impl ContextId {
    /// 创建一个新的随机 ContextId
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// 从 Uuid 创建 ContextId
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// 获取底层的 Uuid
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    /// 从字符串解析 ContextId
    pub fn parse_str(s: &str) -> Result<Self, String> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|e| format!("Invalid ContextId: {}", e))
    }
}

impl Default for ContextId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ContextId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ============================================================================
// ContextChunkType - 上下文块类型
// ============================================================================

/// 上下文块类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextChunkType {
    /// 用户消息
    UserMessage,
    /// 助手消息
    AssistantMessage,
    /// 工具调用
    ToolCall,
    /// 工具结果
    ToolResult,
    /// 系统提示词
    SystemPrompt,
    /// 系统通知
    SystemNotification,
    /// 工单
    WorkOrder,
    /// 求助请求
    HelpRequest,
}

impl fmt::Display for ContextChunkType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UserMessage => write!(f, "UserMessage"),
            Self::AssistantMessage => write!(f, "AssistantMessage"),
            Self::ToolCall => write!(f, "ToolCall"),
            Self::ToolResult => write!(f, "ToolResult"),
            Self::SystemPrompt => write!(f, "SystemPrompt"),
            Self::SystemNotification => write!(f, "SystemNotification"),
            Self::WorkOrder => write!(f, "WorkOrder"),
            Self::HelpRequest => write!(f, "HelpRequest"),
        }
    }
}

// ============================================================================
// ContextChunk - 上下文块
// ============================================================================

/// 上下文块
///
/// 代表上下文中的一个原子单元。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextChunk {
    /// 唯一标识
    pub id: ContextId,
    /// 会话 ID
    pub session_id: Uuid,
    /// 内容
    pub content: String,
    /// 类型
    pub chunk_type: ContextChunkType,
    /// Token 计数
    pub token_count: usize,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 元数据
    pub metadata: HashMap<String, String>,
    /// 是否重要（裁剪时优先保留）
    pub is_important: bool,
    /// 保留优先级（0-10，越高越优先保留）
    pub retention_priority: u8,
}

impl ContextChunk {
    /// 创建新的上下文块
    pub fn new(
        session_id: Uuid,
        content: String,
        chunk_type: ContextChunkType,
        token_count: usize,
    ) -> Self {
        Self {
            id: ContextId::new(),
            session_id,
            content,
            chunk_type,
            token_count,
            created_at: Utc::now(),
            metadata: HashMap::new(),
            is_important: false,
            retention_priority: 0,
        }
    }

    /// 从 Message 转换为 ContextChunk
    pub fn from_message(message: &Message) -> Self {
        let chunk_type = match message.role {
            MessageRole::User => ContextChunkType::UserMessage,
            MessageRole::Assistant => ContextChunkType::AssistantMessage,
            MessageRole::System => ContextChunkType::SystemPrompt,
            MessageRole::ToolCall => ContextChunkType::ToolCall,
            MessageRole::ToolResult => ContextChunkType::ToolResult,
        };

        let token_count = estimate_tokens(&message.content);

        let mut chunk = Self::new(
            message.session_id,
            message.content.clone(),
            chunk_type,
            token_count,
        );

        if let Some(checkpoint_id) = &message.checkpoint_id {
            chunk
                .metadata
                .insert("checkpoint_id".to_string(), checkpoint_id.clone());
        }

        chunk
    }

    /// 从 WorkOrder 转换为 ContextChunk
    pub fn from_work_order(work_order: &WorkOrder) -> Self {
        let chunk_type = if work_order.is_help_request() {
            ContextChunkType::HelpRequest
        } else {
            ContextChunkType::WorkOrder
        };

        let token_count = estimate_tokens(&work_order.content);

        let mut chunk = Self::new(
            work_order.session_id,
            work_order.content.clone(),
            chunk_type,
            token_count,
        );

        chunk
            .metadata
            .insert("work_order_id".to_string(), work_order.id().to_string());
        chunk.metadata.insert(
            "work_order_type".to_string(),
            work_order.work_order_type.to_string(),
        );

        if let Some(agent_id) = work_order.created_by {
            chunk
                .metadata
                .insert("created_by".to_string(), agent_id.to_string());
        }

        chunk
    }

    /// 设置是否重要
    pub fn with_importance(mut self, is_important: bool) -> Self {
        self.is_important = is_important;
        self
    }

    /// 设置保留优先级
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.retention_priority = priority;
        self
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// 估算 Token 数量（简易实现，后续可替换为 tiktoken）
pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }

    // 粗略估算：英文平均 4 个字符一个 token，中文平均 1 个字符 1.5-2 个 token
    // 这里使用一个综合估算：(字符数 * 0.3) + (非 ASCII 字符数 * 1.2)
    let char_count = text.chars().count();
    let non_ascii_count = text.chars().filter(|c| !c.is_ascii()).count();

    let estimated = (char_count as f64 * 0.3) + (non_ascii_count as f64 * 1.2);

    // 至少为 1，除非为空
    estimated.max(1.0).ceil() as usize
}

// ============================================================================
// ContextMetadata - 上下文元数据
// ============================================================================

/// 会话上下文元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMetadata {
    /// 会话 ID
    pub session_id: Uuid,
    /// 总 Token 计数
    pub total_token_count: usize,
    /// 块数量
    pub chunk_count: usize,
    /// 第一条消息时间
    pub first_message_at: DateTime<Utc>,
    /// 最后一条消息时间
    pub last_message_at: DateTime<Utc>,
    /// 预估成本
    pub estimated_cost: Option<f64>,
}

// ============================================================================
// ContextStore - 上下文存储
// ============================================================================

/// 上下文存储
///
/// 负责管理所有会话的上下文块。
#[derive(Debug, Clone, Default)]
pub struct ContextStore {
    /// 按会话 ID 存储的上下文块列表
    chunks: Arc<RwLock<HashMap<Uuid, Vec<ContextChunk>>>>,
}

impl ContextStore {
    /// 创建一个新的上下文存储
    pub fn new() -> Self {
        Self {
            chunks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 向会话添加一个上下文块
    pub async fn add_chunk(&self, chunk: ContextChunk) {
        let mut chunks_map = self.chunks.write().await;
        chunks_map.entry(chunk.session_id).or_default().push(chunk);
    }

    /// 批量向会话添加上下文块
    pub async fn add_chunks(&self, session_id: Uuid, mut new_chunks: Vec<ContextChunk>) {
        let mut chunks_map = self.chunks.write().await;
        let entry = chunks_map.entry(session_id).or_default();
        entry.append(&mut new_chunks);
    }

    /// 获取会话的所有上下文块
    pub async fn get_chunks(&self, session_id: &Uuid) -> Vec<ContextChunk> {
        let chunks_map = self.chunks.read().await;
        chunks_map.get(session_id).cloned().unwrap_or_default()
    }

    /// 获取会话的元数据
    pub async fn get_metadata(&self, session_id: &Uuid) -> Option<ContextMetadata> {
        let chunks_map = self.chunks.read().await;
        let session_chunks = chunks_map.get(session_id)?;

        if session_chunks.is_empty() {
            return None;
        }

        let total_tokens = session_chunks.iter().map(|c| c.token_count).sum();
        let first_msg = session_chunks.first()?.created_at;
        let last_msg = session_chunks.last()?.created_at;

        Some(ContextMetadata {
            session_id: *session_id,
            total_token_count: total_tokens,
            chunk_count: session_chunks.len(),
            first_message_at: first_msg,
            last_message_at: last_msg,
            estimated_cost: None,
        })
    }

    /// 清空会话的上下文
    pub async fn clear_session(&self, session_id: &Uuid) {
        let mut chunks_map = self.chunks.write().await;
        chunks_map.remove(session_id);
    }

    /// 覆盖会话的上下文（用于裁剪后更新）
    pub async fn update_session_chunks(&self, session_id: Uuid, new_chunks: Vec<ContextChunk>) {
        let mut chunks_map = self.chunks.write().await;
        chunks_map.insert(session_id, new_chunks);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_id_new() {
        let id1 = ContextId::new();
        let id2 = ContextId::new();
        assert_ne!(id1, id2);
    }

    #[tokio::test]
    async fn test_context_store_basic() {
        let store = ContextStore::new();
        let session_id = Uuid::new_v4();

        let chunk = ContextChunk::new(
            session_id,
            "Hello".to_string(),
            ContextChunkType::UserMessage,
            10,
        );

        store.add_chunk(chunk).await;

        let chunks = store.get_chunks(&session_id).await;
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, "Hello");

        let metadata = store.get_metadata(&session_id).await.unwrap();
        assert_eq!(metadata.total_token_count, 10);
        assert_eq!(metadata.chunk_count, 1);
    }

    #[tokio::test]
    async fn test_context_store_clear() {
        let store = ContextStore::new();
        let session_id = Uuid::new_v4();

        store
            .add_chunk(ContextChunk::new(
                session_id,
                "Test".to_string(),
                ContextChunkType::SystemPrompt,
                5,
            ))
            .await;

        store.clear_session(&session_id).await;
        let chunks = store.get_chunks(&session_id).await;
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("hello"), 2); // 5 * 0.3 = 1.5 -> 2
        assert_eq!(estimate_tokens("你好"), 3); // 2 * 0.3 + 2 * 1.2 = 0.6 + 2.4 = 3
    }
}
