use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

use super::message::Message;
use crate::agent::types::AgentId;
use crate::error::{Error, Result};
use crate::orchestrator::OrchestratorId;

// ============================================================================
// SessionState - Session 状态枚举
// ============================================================================

/// Session 状态
///
/// 定义 Session 在其生命周期中的各种状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    /// 草稿状态，刚创建
    Draft,
    /// 活跃状态，正在使用
    Active,
    /// 暂停状态
    Paused,
    /// 已归档，只读
    Archived,
    /// 已删除（软删除）
    Deleted,
}

impl fmt::Display for SessionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Draft => write!(f, "Draft"),
            Self::Active => write!(f, "Active"),
            Self::Paused => write!(f, "Paused"),
            Self::Archived => write!(f, "Archived"),
            Self::Deleted => write!(f, "Deleted"),
        }
    }
}

impl SessionState {
    /// 检查是否可以转换到目标状态
    pub fn can_transition_to(&self, target: &SessionState) -> bool {
        match (self, target) {
            // Draft 可以转换到 Active 或 Deleted
            (Self::Draft, Self::Active) => true,
            (Self::Draft, Self::Deleted) => true,
            // Active 可以转换到 Paused, Archived, Deleted
            (Self::Active, Self::Paused) => true,
            (Self::Active, Self::Archived) => true,
            (Self::Active, Self::Deleted) => true,
            // Paused 可以转换到 Active, Archived, Deleted
            (Self::Paused, Self::Active) => true,
            (Self::Paused, Self::Archived) => true,
            (Self::Paused, Self::Deleted) => true,
            // Archived 只能转换到 Deleted
            (Self::Archived, Self::Deleted) => true,
            // Deleted 是终态，不能转换
            (Self::Deleted, _) => false,
            // 其他转换都不允许
            _ => false,
        }
    }

    /// 检查是否是活跃状态（可以进行修改）
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Draft | Self::Active | Self::Paused)
    }

    /// 检查是否是只读状态
    pub fn is_readonly(&self) -> bool {
        matches!(self, Self::Archived | Self::Deleted)
    }
}

// ============================================================================
// SessionLifecycleEventType - Session 生命周期事件类型
// ============================================================================

/// Session 生命周期事件类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionLifecycleEventType {
    /// Session 已创建
    Created,
    /// Session 已激活
    Activated,
    /// Session 已暂停
    Paused,
    /// Session 已恢复
    Resumed,
    /// Session 已归档
    Archived,
    /// Session 已删除
    Deleted,
}

impl fmt::Display for SessionLifecycleEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Created => write!(f, "Created"),
            Self::Activated => write!(f, "Activated"),
            Self::Paused => write!(f, "Paused"),
            Self::Resumed => write!(f, "Resumed"),
            Self::Archived => write!(f, "Archived"),
            Self::Deleted => write!(f, "Deleted"),
        }
    }
}

// ============================================================================
// SessionLifecycleEvent - Session 生命周期事件
// ============================================================================

/// Session 生命周期事件
///
/// 记录 Session 在生命周期中的重要事件。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLifecycleEvent {
    /// 事件类型
    pub event_type: SessionLifecycleEventType,
    /// Session ID
    pub session_id: Uuid,
    /// 触发者（可选）
    pub triggered_by: Option<String>,
    /// 事件发生时间
    pub occurred_at: DateTime<Utc>,
    /// 元数据（可选）
    pub metadata: Option<serde_json::Value>,
}

impl SessionLifecycleEvent {
    /// 创建新的生命周期事件
    pub fn new(
        event_type: SessionLifecycleEventType,
        session_id: Uuid,
        triggered_by: Option<String>,
    ) -> Self {
        Self {
            event_type,
            session_id,
            triggered_by,
            occurred_at: Utc::now(),
            metadata: None,
        }
    }

    /// 设置元数据
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

// ============================================================================
// Session - 增强版 Session 结构
// ============================================================================

/// Session（增强版）
///
/// 代表用户与系统的一次交互会话。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Session ID
    pub id: Uuid,
    /// 标题（可选）
    pub title: Option<String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后更新时间
    pub updated_at: DateTime<Utc>,
    /// 当前状态
    pub state: SessionState,
    /// 关联的总控 ID（可选）
    pub orchestrator_id: Option<OrchestratorId>,
    /// 当前 Checkpoint ID（可选）
    pub current_checkpoint_id: Option<String>,
    /// 归档时间（可选）
    pub archived_at: Option<DateTime<Utc>>,
    /// 消息列表
    pub messages: Vec<Message>,
    /// 元数据
    pub metadata: HashMap<String, serde_json::Value>,
    /// 生命周期事件历史
    #[serde(default)]
    pub lifecycle_events: Vec<SessionLifecycleEvent>,
    /// 按 Agent 分组的 Checkpoint 列表（FIFO 存储，索引）
    #[serde(default)]
    pub agent_checkpoints: HashMap<AgentId, Vec<String>>,
}

impl Session {
    /// 创建新的 Session
    pub fn new() -> Self {
        let now = Utc::now();
        let id = Uuid::new_v4();

        let mut session = Self {
            id,
            title: None,
            created_at: now,
            updated_at: now,
            state: SessionState::Draft,
            orchestrator_id: None,
            current_checkpoint_id: None,
            archived_at: None,
            messages: Vec::new(),
            metadata: HashMap::new(),
            lifecycle_events: Vec::new(),
            agent_checkpoints: HashMap::new(),
        };

        // 添加 Created 事件
        session.add_lifecycle_event(SessionLifecycleEvent::new(
            SessionLifecycleEventType::Created,
            id,
            None,
        ));

        session
    }

    /// 使用标题创建新的 Session
    pub fn with_title(title: String) -> Self {
        let mut session = Self::new();
        session.title = Some(title);
        session
    }

    /// 设置标题
    pub fn set_title(&mut self, title: String) {
        self.title = Some(title);
        self.updated_at = Utc::now();
    }

    /// 添加消息
    pub fn add_message(&mut self, message: Message) -> Result<()> {
        if self.state.is_readonly() {
            return Err(Error::SessionInvalidState(format!(
                "Cannot add message to session in {} state",
                self.state
            )));
        }
        self.messages.push(message);
        self.updated_at = Utc::now();
        Ok(())
    }

    /// 插入元数据
    pub fn insert_metadata<K, V>(&mut self, key: K, value: V) -> Result<()>
    where
        K: Into<String>,
        V: Into<serde_json::Value>,
    {
        if self.state.is_readonly() {
            return Err(Error::SessionInvalidState(format!(
                "Cannot modify metadata in {} state",
                self.state
            )));
        }
        self.metadata.insert(key.into(), value.into());
        self.updated_at = Utc::now();
        Ok(())
    }

    /// 转换状态（不带自动 Checkpoint 创建）
    pub fn transition_to(&mut self, target_state: SessionState) -> Result<()> {
        if !self.state.can_transition_to(&target_state) {
            return Err(Error::SessionInvalidState(format!(
                "Cannot transition from {} to {}",
                self.state, target_state
            )));
        }

        // 记录状态转换事件
        let event_type = match (&self.state, &target_state) {
            (SessionState::Draft, SessionState::Active) => SessionLifecycleEventType::Activated,
            (SessionState::Active, SessionState::Paused) => SessionLifecycleEventType::Paused,
            (SessionState::Paused, SessionState::Active) => SessionLifecycleEventType::Resumed,
            (_, SessionState::Archived) => SessionLifecycleEventType::Archived,
            (_, SessionState::Deleted) => SessionLifecycleEventType::Deleted,
            _ => {
                // 不应该到达这里，因为上面已经检查了 can_transition_to
                return Err(Error::SessionInvalidState(format!(
                    "Unexpected state transition: {} -> {}",
                    self.state, target_state
                )));
            }
        };

        self.state = target_state;
        self.updated_at = Utc::now();

        // 如果是归档，记录归档时间
        if self.state == SessionState::Archived {
            self.archived_at = Some(Utc::now());
        }

        // 添加生命周期事件
        self.add_lifecycle_event(SessionLifecycleEvent::new(event_type, self.id, None));

        Ok(())
    }

    /// 转换状态并自动创建 Checkpoint（如果有 CheckpointManager）
    /// 注意：这是一个异步方法，需要 CheckpointManager 引用
    pub async fn transition_to_with_checkpoint(
        &mut self,
        target_state: SessionState,
        checkpoint_manager: Option<&crate::checkpoint::CheckpointManager>,
    ) -> Result<Option<crate::models::checkpoint::Checkpoint>> {
        // 在状态转换前，判断是否需要创建 Checkpoint
        let should_create_checkpoint = match (&self.state, &target_state) {
            (SessionState::Active, SessionState::Paused) => true,
            (SessionState::Paused, SessionState::Active) => true,
            (_, SessionState::Archived) => true,
            (_, SessionState::Deleted) => false, // 删除时不创建
            _ => false,
        };

        let mut checkpoint = None;

        // 如果需要，先创建 Checkpoint
        if should_create_checkpoint && let Some(cm) = checkpoint_manager {
            let description = match (&self.state, &target_state) {
                (SessionState::Active, SessionState::Paused) => Some("Session paused".to_string()),
                (SessionState::Paused, SessionState::Active) => Some("Session resumed".to_string()),
                (_, SessionState::Archived) => Some("Session archived".to_string()),
                _ => None,
            };

            checkpoint = Some(
                cm.create_checkpoint(self.id, description, None, AgentId::system())
                    .await?,
            );
        }

        // 执行状态转换
        self.transition_to(target_state)?;

        // 如果有新的 Checkpoint，更新 Session 的 current_checkpoint_id
        if let Some(cp) = &checkpoint {
            self.current_checkpoint_id = Some(cp.id.clone());
        }

        Ok(checkpoint)
    }

    /// 归档 Session 并归档所有相关的 Checkpoint
    pub async fn archive_with_checkpoints(
        &mut self,
        checkpoint_manager: Option<&crate::checkpoint::CheckpointManager>,
    ) -> Result<Vec<crate::models::checkpoint::Checkpoint>> {
        let mut archived_checkpoints = Vec::new();

        // 先归档所有 Checkpoint
        if let Some(cm) = checkpoint_manager {
            let response = cm.list_checkpoints(&self.id).await?;
            for checkpoint_item in response.checkpoints {
                let mut checkpoint = cm.get_checkpoint(&checkpoint_item.id).await?;
                if !checkpoint.is_archived() {
                    checkpoint.archive();
                    // 保存归档后的 Checkpoint
                    cm.update_checkpoint(&checkpoint).await?;
                    archived_checkpoints.push(checkpoint);
                }
            }
        }

        // 然后归档 Session（同时也会创建一个新的 Checkpoint）
        self.transition_to_with_checkpoint(SessionState::Archived, checkpoint_manager)
            .await?;

        Ok(archived_checkpoints)
    }

    /// 激活 Session
    pub fn activate(&mut self) -> Result<()> {
        self.transition_to(SessionState::Active)
    }

    /// 暂停 Session
    pub fn pause(&mut self) -> Result<()> {
        self.transition_to(SessionState::Paused)
    }

    /// 归档 Session
    pub fn archive(&mut self) -> Result<()> {
        self.transition_to(SessionState::Archived)
    }

    /// 软删除 Session
    pub fn soft_delete(&mut self) -> Result<()> {
        self.transition_to(SessionState::Deleted)
    }

    /// 关联总控
    pub fn assign_orchestrator(&mut self, orchestrator_id: OrchestratorId) -> Result<()> {
        if self.state.is_readonly() {
            return Err(Error::SessionInvalidState(format!(
                "Cannot assign orchestrator in {} state",
                self.state
            )));
        }
        self.orchestrator_id = Some(orchestrator_id);
        self.updated_at = Utc::now();
        Ok(())
    }

    /// 解绑总控
    pub fn unassign_orchestrator(&mut self) -> Result<Option<OrchestratorId>> {
        if self.state.is_readonly() {
            return Err(Error::SessionInvalidState(format!(
                "Cannot unassign orchestrator in {} state",
                self.state
            )));
        }
        let old_id = self.orchestrator_id.take();
        self.updated_at = Utc::now();
        Ok(old_id)
    }

    /// 设置当前 Checkpoint
    pub fn set_current_checkpoint(&mut self, checkpoint_id: String) -> Result<()> {
        if self.state.is_readonly() {
            return Err(Error::SessionInvalidState(format!(
                "Cannot set checkpoint in {} state",
                self.state
            )));
        }
        self.current_checkpoint_id = Some(checkpoint_id);
        self.updated_at = Utc::now();
        Ok(())
    }

    /// 添加生命周期事件（内部方法）
    fn add_lifecycle_event(&mut self, event: SessionLifecycleEvent) {
        self.lifecycle_events.push(event);
    }

    /// 获取生命周期历史
    pub fn lifecycle_history(&self) -> &[SessionLifecycleEvent] {
        &self.lifecycle_events
    }

    /// 检查是否可以修改
    pub fn can_modify(&self) -> bool {
        self.state.is_active()
    }

    /// 添加 Checkpoint 索引（仅更新内存中的索引）
    pub fn add_checkpoint_index(&mut self, agent_id: AgentId, checkpoint_id: String) {
        self.agent_checkpoints
            .entry(agent_id)
            .or_default()
            .push(checkpoint_id);
    }

    /// 获取 Agent 的 Checkpoint ID 列表（从索引读取）
    pub fn get_checkpoint_ids_for_agent(&self, agent_id: &AgentId) -> Vec<String> {
        self.agent_checkpoints
            .get(agent_id)
            .cloned()
            .unwrap_or_default()
    }

    /// 获取 Agent 的起始 Checkpoint ID（直接拿第一个）
    pub fn get_agent_start_checkpoint_id(&self, agent_id: &AgentId) -> Option<&String> {
        self.agent_checkpoints
            .get(agent_id)
            .and_then(|ids| ids.first())
    }

    /// 清空所有 Checkpoint 索引（Session 归档/删除时调用）
    pub fn clear_checkpoint_indexes(&mut self) {
        self.agent_checkpoints.clear();
    }

    /// 添加 Checkpoint 到 Agent 的列表（保留向后兼容）
    pub fn add_checkpoint_for_agent(&mut self, agent_id: AgentId, checkpoint_id: String) {
        self.add_checkpoint_index(agent_id, checkpoint_id);
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// SessionInfo - Session 列表项信息
// ============================================================================

/// Session 列表项信息
#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub id: Uuid,
    pub title: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub state: SessionState,
    pub message_count: usize,
    pub orchestrator_id: Option<String>,
}

impl From<&Session> for SessionInfo {
    fn from(session: &Session) -> Self {
        Self {
            id: session.id,
            title: session.title.clone(),
            created_at: session.created_at,
            updated_at: session.updated_at,
            state: session.state.clone(),
            message_count: session.messages.len(),
            orchestrator_id: session.orchestrator_id.map(|id| id.to_string()),
        }
    }
}

// ============================================================================
// ListSessionsResponse - 列出 Session 响应
// ============================================================================

#[derive(Debug, Serialize)]
pub struct ListSessionsResponse {
    pub sessions: Vec<SessionInfo>,
}
