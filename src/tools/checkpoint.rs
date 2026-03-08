//! Checkpoint 本地工具
//!
//! 提供给 LLM 使用的 checkpoint 管理工具。

use crate::error::{Error, Result};
use crate::tools::{LocalTool, ToolContext};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, info};

// ==================== 工具参数和结果结构 ====================

/// 列出 checkpoints 参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCheckpointsToolParams {
    /// 最大返回数量（可选，默认 50）
    pub limit: Option<usize>,
}

/// 列出 checkpoints 结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCheckpointsToolResult {
    pub success: bool,
    pub checkpoints: Vec<CheckpointInfo>,
    pub total_count: usize,
}

/// Checkpoint 信息（简化版，用于工具返回）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointInfo {
    pub id: String,
    pub created_at: String,
    pub description: Option<String>,
    pub checkpoint_type: String,
    pub file_count: usize,
}

/// 恢复 checkpoint 参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreCheckpointToolParams {
    /// Checkpoint ID
    pub checkpoint_id: String,
    /// 是否恢复文件（默认 true）
    pub restore_files: Option<bool>,
    /// 是否恢复会话历史（默认 true）
    pub restore_session: Option<bool>,
}

/// 恢复 checkpoint 结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreCheckpointToolResult {
    pub success: bool,
    pub message: String,
    pub restored_files: Vec<String>,
}

/// 删除 checkpoint 参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteCheckpointToolParams {
    /// Checkpoint ID
    pub checkpoint_id: String,
}

/// 删除 checkpoint 结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteCheckpointToolResult {
    pub success: bool,
    pub message: String,
}

// ==================== 工具实现 ====================

/// 列出 checkpoints 工具
pub struct ListCheckpointsTool;

#[async_trait]
impl LocalTool for ListCheckpointsTool {
    fn name(&self) -> &str {
        "list_checkpoints"
    }

    fn description(&self) -> &str {
        "列出当前会话的所有 checkpoints"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "最大返回数量（默认 50）",
                    "minimum": 1,
                    "maximum": 100
                }
            },
            "required": []
        })
    }

    async fn call(&self, arguments: Value, context: ToolContext) -> Result<Value> {
        debug!("ListCheckpointsTool called");

        let params: ListCheckpointsToolParams = serde_json::from_value(arguments)
            .map_err(|e| Error::InvalidInput(format!("Invalid arguments: {}", e)))?;

        let Some(checkpoint_manager) = &context.checkpoint_manager else {
            return Err(Error::Checkpoint("Checkpoint manager not available".into()));
        };

        let response = checkpoint_manager
            .list_checkpoints(&context.session.id)
            .await?;

        let limit = params.limit.unwrap_or(50);
        let mut checkpoints: Vec<CheckpointInfo> = response
            .checkpoints
            .into_iter()
            .take(limit)
            .map(|item| CheckpointInfo {
                id: item.id,
                created_at: item.created_at.to_rfc3339(),
                description: item.description,
                checkpoint_type: format!("{:?}", item.checkpoint_type),
                file_count: item.file_count,
            })
            .collect();

        // 按时间倒序排列
        checkpoints.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        let result = ListCheckpointsToolResult {
            success: true,
            checkpoints,
            total_count: response.total_count,
        };

        info!(
            session_id = %context.session.id,
            count = %result.total_count,
            "Checkpoints listed via tool"
        );

        serde_json::to_value(result).map_err(Error::SerdeJson)
    }
}

/// 恢复 checkpoint 工具
pub struct RestoreCheckpointTool;

#[async_trait]
impl LocalTool for RestoreCheckpointTool {
    fn name(&self) -> &str {
        "restore_checkpoint"
    }

    fn description(&self) -> &str {
        "恢复到指定的 checkpoint，包括文件状态和/或会话历史"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "checkpoint_id": {
                    "type": "string",
                    "description": "要恢复的 checkpoint ID"
                },
                "restore_files": {
                    "type": "boolean",
                    "description": "是否恢复文件（默认 true）"
                },
                "restore_session": {
                    "type": "boolean",
                    "description": "是否恢复会话历史（默认 true）"
                }
            },
            "required": ["checkpoint_id"]
        })
    }

    async fn call(&self, arguments: Value, context: ToolContext) -> Result<Value> {
        debug!("RestoreCheckpointTool called");

        let params: RestoreCheckpointToolParams = serde_json::from_value(arguments)
            .map_err(|e| Error::InvalidInput(format!("Invalid arguments: {}", e)))?;

        let Some(checkpoint_manager) = &context.checkpoint_manager else {
            return Err(Error::Checkpoint("Checkpoint manager not available".into()));
        };

        let restore_files = params.restore_files.unwrap_or(true);
        let restore_session = params.restore_session.unwrap_or(true);

        let snapshot = checkpoint_manager
            .restore_checkpoint(&params.checkpoint_id, restore_files, restore_session)
            .await?;

        let restored_files: Vec<String> = snapshot.files.keys().cloned().collect();

        let result = RestoreCheckpointToolResult {
            success: true,
            message: format!(
                "Checkpoint restored successfully. Restored {} files.",
                restored_files.len()
            ),
            restored_files,
        };

        info!(
            checkpoint_id = %params.checkpoint_id,
            "Checkpoint restored via tool"
        );

        serde_json::to_value(result).map_err(Error::SerdeJson)
    }
}

/// 删除 checkpoint 工具
pub struct DeleteCheckpointTool;

#[async_trait]
impl LocalTool for DeleteCheckpointTool {
    fn name(&self) -> &str {
        "delete_checkpoint"
    }

    fn description(&self) -> &str {
        "删除指定的 checkpoint"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "checkpoint_id": {
                    "type": "string",
                    "description": "要删除的 checkpoint ID"
                }
            },
            "required": ["checkpoint_id"]
        })
    }

    async fn call(&self, arguments: Value, context: ToolContext) -> Result<Value> {
        debug!("DeleteCheckpointTool called");

        let params: DeleteCheckpointToolParams = serde_json::from_value(arguments)
            .map_err(|e| Error::InvalidInput(format!("Invalid arguments: {}", e)))?;

        let Some(checkpoint_manager) = &context.checkpoint_manager else {
            return Err(Error::Checkpoint("Checkpoint manager not available".into()));
        };

        checkpoint_manager
            .delete_checkpoint(&params.checkpoint_id)
            .await?;

        let result = DeleteCheckpointToolResult {
            success: true,
            message: format!("Checkpoint {} deleted successfully", params.checkpoint_id),
        };

        info!(
            checkpoint_id = %params.checkpoint_id,
            "Checkpoint deleted via tool"
        );

        serde_json::to_value(result).map_err(Error::SerdeJson)
    }
}

// ==================== Checkpoint 工具注册 ====================

/// Checkpoint 工具集合
pub struct CheckpointTools;

impl CheckpointTools {
    /// 注册所有 checkpoint 工具到注册表
    pub fn register_all(registry: &mut crate::tools::LocalToolRegistry) {
        registry.register(Arc::new(ListCheckpointsTool));
        registry.register(Arc::new(RestoreCheckpointTool));
        registry.register(Arc::new(DeleteCheckpointTool));
    }
}
