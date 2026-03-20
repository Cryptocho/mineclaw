use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

use crate::agent::types::AgentId;

// ============================================================================
// CheckpointArchivingStrategy - Checkpoint 归档策略
// ============================================================================

/// Checkpoint 归档策略类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckpointArchivingStrategyType {
    /// Session 归档时归档
    OnSessionArchive,
    /// 手动触发
    Manual,
    /// 定期归档
    Periodic,
}

impl fmt::Display for CheckpointArchivingStrategyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OnSessionArchive => write!(f, "OnSessionArchive"),
            Self::Manual => write!(f, "Manual"),
            Self::Periodic => write!(f, "Periodic"),
        }
    }
}

/// Checkpoint 归档策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointArchivingStrategy {
    /// 策略类型
    pub strategy_type: CheckpointArchivingStrategyType,
    /// 保留的 Checkpoint 数量（可选）
    pub retain_count: Option<usize>,
    /// 定期归档的间隔（秒，仅 Periodic 策略需要）
    pub periodic_interval_seconds: Option<u64>,
}

impl CheckpointArchivingStrategy {
    /// 创建默认策略（Session 归档时归档，保留所有）
    pub fn new() -> Self {
        Self {
            strategy_type: CheckpointArchivingStrategyType::OnSessionArchive,
            retain_count: None,
            periodic_interval_seconds: None,
        }
    }

    /// 创建保留指定数量的策略
    pub fn with_retain_count(
        strategy_type: CheckpointArchivingStrategyType,
        retain_count: usize,
    ) -> Self {
        Self {
            strategy_type,
            retain_count: Some(retain_count),
            periodic_interval_seconds: None,
        }
    }

    /// 创建定期归档策略
    pub fn periodic(interval_seconds: u64, retain_count: Option<usize>) -> Self {
        Self {
            strategy_type: CheckpointArchivingStrategyType::Periodic,
            retain_count,
            periodic_interval_seconds: Some(interval_seconds),
        }
    }
}

impl Default for CheckpointArchivingStrategy {
    fn default() -> Self {
        Self::new()
    }
}

/// 文件信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    /// 文件路径
    pub path: String,
    /// 文件大小（字节）
    pub size: u64,
    /// 修改时间
    pub modified_at: DateTime<Utc>,
    /// 文件内容哈希（可选，用于检测变更）
    pub content_hash: Option<String>,
}

/// Checkpoint 元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Checkpoint ID
    pub id: String,
    /// 所属会话 ID
    pub session_id: Uuid,
    /// 创建该 Checkpoint 的 Agent ID
    pub agent_id: AgentId,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 描述
    pub description: Option<String>,
    /// 受影响的文件列表
    pub affected_files: Vec<FileInfo>,
    /// 元数据
    pub metadata: Option<serde_json::Value>,
    /// 是否已归档
    #[serde(default)]
    pub is_archived: bool,
    /// 归档时间（可选）
    pub archived_at: Option<DateTime<Utc>>,
}

pub struct CheckpointBuilder {
    session_id: Uuid,
    agent_id: AgentId,
    affected_files: Vec<FileInfo>,
    description: Option<String>,
    metadata: Option<serde_json::Value>,
}

impl Checkpoint {
    pub fn builder(session_id: Uuid, agent_id: AgentId, affected_files: Vec<FileInfo>) -> CheckpointBuilder {
        CheckpointBuilder {
            session_id,
            agent_id,
            affected_files,
            description: None,
            metadata: None,
        }
    }
}

impl CheckpointBuilder {
    pub fn description(mut self, desc: String) -> Self {
        self.description = Some(desc);
        self
    }

    pub fn metadata(mut self, meta: serde_json::Value) -> Self {
        self.metadata = Some(meta);
        self
    }

    pub fn build(self) -> Checkpoint {
        Checkpoint {
            id: Uuid::new_v4().to_string(),
            session_id: self.session_id,
            agent_id: self.agent_id,
            created_at: Utc::now(),
            description: self.description,
            affected_files: self.affected_files,
            metadata: self.metadata,
            is_archived: false,
            archived_at: None,
        }
    }
}

impl Checkpoint {
    pub fn archive(&mut self) {
        if !self.is_archived {
            self.is_archived = true;
            self.archived_at = Some(Utc::now());
        }
    }

    pub fn is_archived(&self) -> bool {
        self.is_archived
    }
}

/// Checkpoint 快照（用于恢复）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointSnapshot {
    /// Checkpoint 元数据
    pub checkpoint: Checkpoint,
    /// 会话快照
    pub session: Option<crate::models::Session>,
    /// 文件快照（路径 -> 内容）
    pub files: HashMap<String, Vec<u8>>,
}

/// Checkpoint 列表项（用于列表展示）
#[derive(Debug, Clone, Serialize)]
pub struct CheckpointListItem {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub description: Option<String>,
    pub file_count: usize,
    pub agent_id: AgentId,
}

impl From<&Checkpoint> for CheckpointListItem {
    fn from(checkpoint: &Checkpoint) -> Self {
        Self {
            id: checkpoint.id.clone(),
            created_at: checkpoint.created_at,
            description: checkpoint.description.clone(),
            file_count: checkpoint.affected_files.len(),
            agent_id: checkpoint.agent_id,
        }
    }
}

/// 列出 checkpoints 响应
#[derive(Debug, Serialize)]
pub struct ListCheckpointsResponse {
    pub checkpoints: Vec<CheckpointListItem>,
    pub total_count: usize,
}

/// 创建 checkpoint 请求
#[derive(Debug, Deserialize)]
pub struct CreateCheckpointRequest {
    pub session_id: Uuid,
    pub description: Option<String>,
    pub affected_files: Option<Vec<String>>,
}

/// 创建 checkpoint 响应
#[derive(Debug, Serialize)]
pub struct CreateCheckpointResponse {
    pub checkpoint_id: String,
    pub success: bool,
}

/// 恢复 checkpoint 请求
#[derive(Debug, Deserialize)]
pub struct RestoreCheckpointRequest {
    pub checkpoint_id: String,
    pub restore_files: Option<bool>,
    pub restore_session: Option<bool>,
}

/// 恢复 checkpoint 响应
#[derive(Debug, Serialize)]
pub struct RestoreCheckpointResponse {
    pub success: bool,
    pub restored_files: Vec<String>,
    pub message: String,
}

/// 删除 checkpoint 请求
#[derive(Debug, Deserialize)]
pub struct DeleteCheckpointRequest {
    pub checkpoint_id: String,
}

/// 删除 checkpoint 响应
#[derive(Debug, Serialize)]
pub struct DeleteCheckpointResponse {
    pub success: bool,
    pub message: String,
}
