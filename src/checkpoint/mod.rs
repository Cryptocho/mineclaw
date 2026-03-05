//! Checkpoint 管理器
//! 与 Phase 3.3 文件工具紧密集成

use crate::config::CheckpointConfig;
use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Checkpoint 元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub session_id: String,
    pub created_at: DateTime<Utc>,
    pub description: Option<String>,
    /// 触发此 checkpoint 的操作（如 "write_file", "delete_file" 等）
    pub triggering_operation: Option<String>,
    /// 受影响的文件路径列表
    pub affected_paths: Vec<String>,
    /// 快照总大小（字节）
    pub size_bytes: u64,
}

/// Checkpoint 管理器
pub struct CheckpointManager {
    config: CheckpointConfig,
    base_path: PathBuf,
}

impl CheckpointManager {
    /// 创建新的 CheckpointManager
    pub fn new(config: CheckpointConfig, base_path: impl AsRef<Path>) -> Result<Self> {
        let base_path = base_path.as_ref().join(&config.directory);
        if !base_path.exists() {
            std::fs::create_dir_all(&base_path)?;
        }
        Ok(Self { config, base_path })
    }

    /// 在文件操作前创建 checkpoint（Phase 3.3 工具自动调用）
    ///
    /// 这是与 Phase 3.3 集成的核心方法
    pub async fn before_operation(
        &self,
        session_id: &str,
        operation: &str,
        paths: &[PathBuf],
    ) -> Result<Option<Checkpoint>> {
        if !self.config.enabled {
            return Ok(None);
        }

        // 过滤出实际存在的文件
        let existing_paths: Vec<_> = paths
            .iter()
            .filter(|p| p.exists())
            .cloned()
            .collect();

        if existing_paths.is_empty() {
            return Ok(None);
        }

        let checkpoint = self
            .create_checkpoint_internal(
                session_id,
                Some(format!("Before {operation}")),
                Some(operation.to_string()),
                &existing_paths,
            )
            .await?;

        Ok(Some(checkpoint))
    }

    /// 内部创建 checkpoint
    async fn create_checkpoint_internal(
        &self,
        session_id: &str,
        description: Option<String>,
        triggering_operation: Option<String>,
        affected_paths: &[PathBuf],
    ) -> Result<Checkpoint> {
        let id = Uuid::new_v4().to_string();
        let created_at = Utc::now();

        // 创建会话目录
        let session_dir = self.base_path.join(session_id);
        if !session_dir.exists() {
            std::fs::create_dir_all(&session_dir)?;
        }

        // 创建 checkpoint 目录
        let checkpoint_dir = session_dir.join(&id);
        std::fs::create_dir_all(&checkpoint_dir)?;

        // 复制文件到快照目录
        let mut size_bytes = 0u64;
        let files_dir = checkpoint_dir.join("files");
        std::fs::create_dir_all(&files_dir)?;

        let affected_paths_str: Vec<String> = affected_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        for path in affected_paths {
            if !path.exists() {
                continue;
            }

            // 计算相对路径用于存储
            let rel_path = pathdiff::diff_paths(path, std::env::current_dir()?)
                .unwrap_or_else(|| path.clone());

            let dest_path = files_dir.join(&rel_path);

            // 确保目标目录存在
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // 复制文件或目录
            if path.is_file() {
                std::fs::copy(path, &dest_path)?;
                size_bytes += dest_path.metadata()?.len();
            } else if path.is_dir() {
                // 递归复制目录
                size_bytes += copy_dir_recursive(path, &dest_path)?;
            }
        }

        // 保存元数据
        let checkpoint = Checkpoint {
            id: id.clone(),
            session_id: session_id.to_string(),
            created_at,
            description,
            triggering_operation,
            affected_paths: affected_paths_str,
            size_bytes,
        };

        let metadata_path = checkpoint_dir.join("metadata.json");
        std::fs::write(metadata_path, serde_json::to_string_pretty(&checkpoint)?)?;

        // 检查并清理旧的 checkpoints
        self.cleanup_old_checkpoints(session_id).await?;

        Ok(checkpoint)
    }

    /// 恢复到指定 checkpoint
    pub async fn restore_checkpoint(&self, checkpoint_id: &str) -> Result<()> {
        // 查找 checkpoint
        let (_checkpoint, checkpoint_dir) = self.find_checkpoint(checkpoint_id)?;

        // 恢复文件
        let files_dir = checkpoint_dir.join("files");
        if files_dir.exists() {
            for entry in walkdir::WalkDir::new(&files_dir) {
                let entry = entry?;
                if entry.path() == files_dir {
                    continue;
                }

                let rel_path = entry.path().strip_prefix(&files_dir)?;
                let dest_path = std::env::current_dir()?.join(rel_path);

                if entry.file_type().is_file() {
                    // 确保目标目录存在
                    if let Some(parent) = dest_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::copy(entry.path(), &dest_path)?;
                } else if entry.file_type().is_dir() {
                    std::fs::create_dir_all(&dest_path)?;
                }
            }
        }

        Ok(())
    }

    /// 列出会话的所有 checkpoints
    pub async fn list_checkpoints(&self, session_id: &str) -> Result<Vec<Checkpoint>> {
        let session_dir = self.base_path.join(session_id);
        if !session_dir.exists() {
            return Ok(Vec::new());
        }

        let mut checkpoints = Vec::new();

        for entry in std::fs::read_dir(session_dir)? {
            let entry = entry?;
            let metadata_path = entry.path().join("metadata.json");
            if metadata_path.exists() {
                let content = std::fs::read_to_string(metadata_path)?;
                let checkpoint: Checkpoint = serde_json::from_str(&content)?;
                checkpoints.push(checkpoint);
            }
        }

        // 按创建时间倒序排列
        checkpoints.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(checkpoints)
    }

    /// 删除 checkpoint
    pub async fn delete_checkpoint(&self, checkpoint_id: &str) -> Result<()> {
        let (_, checkpoint_dir) = self.find_checkpoint(checkpoint_id)?;
        std::fs::remove_dir_all(checkpoint_dir)?;
        Ok(())
    }

    /// 手动创建 checkpoint（通过工具调用）
    pub async fn create_checkpoint(
        &self,
        session_id: &str,
        description: Option<String>,
        paths: Option<Vec<String>>,
    ) -> Result<Checkpoint> {
        let affected_paths = if let Some(paths) = paths {
            paths.iter().map(PathBuf::from).collect()
        } else {
            // 如果未指定路径，使用允许的目录
            Vec::new()
        };

        self.create_checkpoint_internal(
            session_id,
            description,
            Some("manual".to_string()),
            &affected_paths,
        )
        .await
    }

    // 内部辅助方法
    fn find_checkpoint(&self, checkpoint_id: &str) -> Result<(Checkpoint, PathBuf)> {
        for session_entry in std::fs::read_dir(&self.base_path)? {
            let session_entry = session_entry?;
            if !session_entry.file_type()?.is_dir() {
                continue;
            }

            let checkpoint_dir = session_entry.path().join(checkpoint_id);
            let metadata_path = checkpoint_dir.join("metadata.json");

            if metadata_path.exists() {
                let content = std::fs::read_to_string(metadata_path)?;
                let checkpoint: Checkpoint = serde_json::from_str(&content)?;
                return Ok((checkpoint, checkpoint_dir));
            }
        }

        Err(Error::CheckpointNotFound(checkpoint_id.to_string()))
    }

    async fn cleanup_old_checkpoints(&self, session_id: &str) -> Result<()> {
        let session_dir = self.base_path.join(session_id);
        if !session_dir.exists() {
            return Ok(());
        }

        let mut checkpoints = self.list_checkpoints(session_id).await?;

        // 如果超过限制，删除最旧的
        if checkpoints.len() > self.config.max_per_session {
            checkpoints.sort_by(|a, b| a.created_at.cmp(&b.created_at));

            let to_delete = checkpoints.len() - self.config.max_per_session;
            for checkpoint in &checkpoints[..to_delete] {
                let checkpoint_dir = session_dir.join(&checkpoint.id);
                if checkpoint_dir.exists() {
                    std::fs::remove_dir_all(checkpoint_dir)?;
                }
            }
        }

        Ok(())
    }
}

// 辅助函数：递归复制目录
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<u64> {
    let mut total_size = 0u64;

    for entry in walkdir::WalkDir::new(src) {
        let entry = entry?;
        let rel_path = entry.path().strip_prefix(src)?;
        let dest_path = dst.join(rel_path);

        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest_path)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &dest_path)?;
            total_size += dest_path.metadata()?.len();
        }
    }

    Ok(total_size)
}