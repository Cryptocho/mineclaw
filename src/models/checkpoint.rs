use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

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

/// Checkpoint 类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CheckpointType {
    /// 自动创建的 checkpoint
    Auto,
    /// 手动创建的 checkpoint
    Manual,
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
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 描述
    pub description: Option<String>,
    /// Checkpoint 类型
    pub checkpoint_type: CheckpointType,
    /// 受影响的文件列表
    pub affected_files: Vec<FileInfo>,
    /// 父 checkpoint ID（用于构建 checkpoint 树）
    pub parent_id: Option<String>,
    /// 元数据
    pub metadata: Option<serde_json::Value>,
    /// 是否已归档
    #[serde(default)]
    pub is_archived: bool,
    /// 归档时间（可选）
    pub archived_at: Option<DateTime<Utc>>,
}

impl Checkpoint {
    /// 创建新的 Checkpoint
    pub fn new(
        session_id: Uuid,
        checkpoint_type: CheckpointType,
        description: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            session_id,
            created_at: Utc::now(),
            description,
            checkpoint_type,
            affected_files: Vec::new(),
            parent_id: None,
            metadata: None,
            is_archived: false,
            archived_at: None,
        }
    }

    /// 归档 Checkpoint
    pub fn archive(&mut self) {
        if !self.is_archived {
            self.is_archived = true;
            self.archived_at = Some(Utc::now());
        }
    }

    /// 检查是否已归档
    pub fn is_archived(&self) -> bool {
        self.is_archived
    }

    /// 添加受影响的文件
    pub fn with_affected_files(mut self, files: Vec<FileInfo>) -> Self {
        self.affected_files = files;
        self
    }

    /// 设置父 checkpoint
    pub fn with_parent_id(mut self, parent_id: String) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    /// 设置元数据
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
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
    pub checkpoint_type: CheckpointType,
    pub file_count: usize,
    pub parent_id: Option<String>,
}

impl From<&Checkpoint> for CheckpointListItem {
    fn from(checkpoint: &Checkpoint) -> Self {
        Self {
            id: checkpoint.id.clone(),
            created_at: checkpoint.created_at,
            description: checkpoint.description.clone(),
            checkpoint_type: checkpoint.checkpoint_type.clone(),
            file_count: checkpoint.affected_files.len(),
            parent_id: checkpoint.parent_id.clone(),
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
    pub checkpoint_type: Option<CheckpointType>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn test_checkpoint_new() {
        let session_id = Uuid::new_v4();
        let checkpoint = Checkpoint::new(
            session_id,
            CheckpointType::Manual,
            Some("Test checkpoint".to_string()),
        );

        assert_eq!(checkpoint.session_id, session_id);
        assert_eq!(checkpoint.checkpoint_type, CheckpointType::Manual);
        assert_eq!(checkpoint.description, Some("Test checkpoint".to_string()));
        assert!(checkpoint.affected_files.is_empty());
        assert!(checkpoint.parent_id.is_none());
        assert!(checkpoint.metadata.is_none());
    }

    #[test]
    fn test_checkpoint_with_affected_files() {
        let session_id = Uuid::new_v4();
        let file_info = FileInfo {
            path: "test.txt".to_string(),
            size: 1024,
            modified_at: Utc::now(),
            content_hash: None,
        };

        let checkpoint = Checkpoint::new(session_id, CheckpointType::Auto, None)
            .with_affected_files(vec![file_info.clone()]);

        assert_eq!(checkpoint.affected_files.len(), 1);
        assert_eq!(checkpoint.affected_files[0].path, "test.txt");
        assert_eq!(checkpoint.affected_files[0].size, 1024);
    }

    #[test]
    fn test_checkpoint_with_parent_id() {
        let session_id = Uuid::new_v4();
        let parent_id = "parent_123".to_string();

        let checkpoint = Checkpoint::new(session_id, CheckpointType::Manual, None)
            .with_parent_id(parent_id.clone());

        assert_eq!(checkpoint.parent_id, Some(parent_id));
    }

    #[test]
    fn test_checkpoint_with_metadata() {
        let session_id = Uuid::new_v4();
        let metadata = json!({"key": "value"});

        let checkpoint = Checkpoint::new(session_id, CheckpointType::Manual, None)
            .with_metadata(metadata.clone());

        assert_eq!(checkpoint.metadata, Some(metadata));
    }

    #[test]
    fn test_checkpoint_type_serialization() {
        let types = vec![CheckpointType::Auto, CheckpointType::Manual];

        for checkpoint_type in types {
            let serialized = serde_json::to_string(&checkpoint_type).unwrap();
            let deserialized: CheckpointType = serde_json::from_str(&serialized).unwrap();
            assert_eq!(checkpoint_type, deserialized);
        }
    }

    #[test]
    fn test_checkpoint_serialization() {
        let session_id = Uuid::new_v4();
        let checkpoint = Checkpoint::new(
            session_id,
            CheckpointType::Manual,
            Some("Serialized checkpoint".to_string()),
        );

        let serialized = serde_json::to_string(&checkpoint).unwrap();
        let deserialized: Checkpoint = serde_json::from_str(&serialized).unwrap();

        assert_eq!(checkpoint.id, deserialized.id);
        assert_eq!(checkpoint.session_id, deserialized.session_id);
        assert_eq!(checkpoint.checkpoint_type, deserialized.checkpoint_type);
        assert_eq!(checkpoint.description, deserialized.description);
    }

    #[test]
    fn test_checkpoint_list_item_from_checkpoint() {
        let session_id = Uuid::new_v4();
        let file_info = FileInfo {
            path: "test.txt".to_string(),
            size: 1024,
            modified_at: Utc::now(),
            content_hash: None,
        };

        let checkpoint =
            Checkpoint::new(session_id, CheckpointType::Manual, Some("Test".to_string()))
                .with_affected_files(vec![file_info])
                .with_parent_id("parent_456".to_string());

        let list_item = CheckpointListItem::from(&checkpoint);

        assert_eq!(list_item.id, checkpoint.id);
        assert_eq!(list_item.created_at, checkpoint.created_at);
        assert_eq!(list_item.description, checkpoint.description);
        assert_eq!(list_item.checkpoint_type, checkpoint.checkpoint_type);
        assert_eq!(list_item.file_count, 1);
        assert_eq!(list_item.parent_id, checkpoint.parent_id);
    }

    #[test]
    fn test_create_checkpoint_request() {
        let session_id = Uuid::new_v4();
        let request = CreateCheckpointRequest {
            session_id,
            description: Some("Test request".to_string()),
            checkpoint_type: Some(CheckpointType::Manual),
            affected_files: Some(vec!["file1.txt".to_string(), "file2.txt".to_string()]),
        };

        assert_eq!(request.session_id, session_id);
        assert_eq!(request.description, Some("Test request".to_string()));
        assert_eq!(request.checkpoint_type, Some(CheckpointType::Manual));
        assert_eq!(
            request.affected_files,
            Some(vec!["file1.txt".to_string(), "file2.txt".to_string()])
        );
    }

    #[test]
    fn test_restore_checkpoint_request() {
        let request = RestoreCheckpointRequest {
            checkpoint_id: "checkpoint_123".to_string(),
            restore_files: Some(true),
            restore_session: Some(false),
        };

        assert_eq!(request.checkpoint_id, "checkpoint_123");
        assert_eq!(request.restore_files, Some(true));
        assert_eq!(request.restore_session, Some(false));
    }

    #[test]
    fn test_file_info() {
        let file_info = FileInfo {
            path: "src/main.rs".to_string(),
            size: 2048,
            modified_at: Utc::now(),
            content_hash: Some("abc123".to_string()),
        };

        assert_eq!(file_info.path, "src/main.rs");
        assert_eq!(file_info.size, 2048);
        assert_eq!(file_info.content_hash, Some("abc123".to_string()));
    }

    #[test]
    fn test_chained_builders() {
        let session_id = Uuid::new_v4();
        let file_info = FileInfo {
            path: "test.txt".to_string(),
            size: 512,
            modified_at: Utc::now(),
            content_hash: None,
        };
        let parent_id = "parent_789".to_string();
        let metadata = json!({"user": "test", "version": 1});

        let checkpoint = Checkpoint::new(
            session_id,
            CheckpointType::Auto,
            Some("Chained".to_string()),
        )
        .with_affected_files(vec![file_info])
        .with_parent_id(parent_id.clone())
        .with_metadata(metadata.clone());

        assert_eq!(checkpoint.description, Some("Chained".to_string()));
        assert_eq!(checkpoint.affected_files.len(), 1);
        assert_eq!(checkpoint.parent_id, Some(parent_id));
        assert_eq!(checkpoint.metadata, Some(metadata));
    }
}
