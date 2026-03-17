use crate::checkpoint::error::{CheckpointError, Result};
use crate::config::CheckpointConfig;
use crate::models::Session;
use crate::models::checkpoint::*;
use agentfs::{AgentFS, KvStore};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

/// Checkpoint 管理器
#[derive(Clone)]
pub struct CheckpointManager {
    agent_fs: Arc<AgentFS>,
    config: CheckpointConfig,
}

impl std::fmt::Debug for CheckpointManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CheckpointManager")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl CheckpointManager {
    /// 创建新的 CheckpointManager
    pub fn new(agent_fs: Arc<AgentFS>, config: CheckpointConfig) -> Self {
        Self { agent_fs, config }
    }

    /// 获取配置引用
    pub fn config(&self) -> &CheckpointConfig {
        &self.config
    }

    // ==================== 核心功能 ====================

    /// 创建 checkpoint
    pub async fn create_checkpoint(
        &self,
        session: &Session,
        description: Option<String>,
        checkpoint_type: CheckpointType,
        affected_files: Option<Vec<String>>,
    ) -> Result<Checkpoint> {
        if !self.config.enabled {
            return Err(CheckpointError::InvalidData(
                "Checkpoint is disabled".to_string(),
            ));
        }

        // 获取父 checkpoint
        let parent_id = self.get_latest_checkpoint_id(&session.id).await?;

        // 创建 checkpoint 对象
        let mut checkpoint = Checkpoint::new(session.id, checkpoint_type, description);
        if let Some(parent_id) = parent_id {
            checkpoint = checkpoint.with_parent_id(parent_id);
        }

        // 收集文件信息
        let file_infos = if let Some(files) = affected_files {
            self.collect_file_infos(&files).await?
        } else {
            Vec::new()
        };
        checkpoint = checkpoint.with_affected_files(file_infos.clone());

        // 保存 checkpoint
        self.save_checkpoint(&checkpoint).await?;

        // 保存文件快照
        self.save_file_snapshots(&checkpoint.id, &session.id, &file_infos)
            .await?;

        // 保存会话快照
        self.save_session_snapshot(&checkpoint.id, session).await?;

        info!(
            checkpoint_id = %checkpoint.id,
            session_id = %session.id,
            "Checkpoint created successfully"
        );

        Ok(checkpoint)
    }

    /// 列出会话的所有 checkpoints
    pub async fn list_checkpoints(&self, session_id: &Uuid) -> Result<ListCheckpointsResponse> {
        let checkpoints = self.load_checkpoints_for_session(session_id).await?;

        let items: Vec<CheckpointListItem> =
            checkpoints.iter().map(CheckpointListItem::from).collect();

        Ok(ListCheckpointsResponse {
            checkpoints: items,
            total_count: checkpoints.len(),
        })
    }

    /// 获取单个 checkpoint
    pub async fn get_checkpoint(&self, checkpoint_id: &str) -> Result<Checkpoint> {
        self.load_checkpoint(checkpoint_id).await
    }

    /// 恢复 checkpoint
    pub async fn restore_checkpoint(
        &self,
        checkpoint_id: &str,
        restore_files: bool,
        restore_session: bool,
    ) -> Result<CheckpointSnapshot> {
        let checkpoint = self.load_checkpoint(checkpoint_id).await?;

        let mut snapshot = CheckpointSnapshot {
            checkpoint: checkpoint.clone(),
            session: None,
            files: HashMap::new(),
        };

        // 恢复会话
        if restore_session {
            snapshot.session = self.load_session_snapshot(checkpoint_id).await?;
        }

        // 恢复文件
        if restore_files {
            snapshot.files = self
                .load_file_snapshots(checkpoint_id, &checkpoint.session_id)
                .await?;

            // 实际写入文件
            self.restore_files(&snapshot.files).await?;
        }

        info!(
            checkpoint_id = %checkpoint_id,
            "Checkpoint restored successfully"
        );

        Ok(snapshot)
    }

    /// 删除 checkpoint
    pub async fn delete_checkpoint(&self, checkpoint_id: &str) -> Result<()> {
        let checkpoint = self.load_checkpoint(checkpoint_id).await?;

        // 删除文件快照
        self.delete_file_snapshots(checkpoint_id, &checkpoint.session_id)
            .await?;

        // 删除会话快照
        self.delete_session_snapshot(checkpoint_id).await?;

        // 删除 checkpoint 元数据
        self.delete_checkpoint_metadata(checkpoint_id, &checkpoint.session_id)
            .await?;

        info!(
            checkpoint_id = %checkpoint_id,
            "Checkpoint deleted successfully"
        );

        Ok(())
    }

    // ==================== 内部方法 ====================

    /// 获取最新的 checkpoint ID
    async fn get_latest_checkpoint_id(&self, session_id: &Uuid) -> Result<Option<String>> {
        let mut checkpoints = self.load_checkpoints_for_session(session_id).await?;
        checkpoints.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(checkpoints.first().map(|c| c.id.clone()))
    }

    /// 删除会话的所有 checkpoints
    pub async fn delete_all_checkpoints_for_session(&self, session_id: &Uuid) -> Result<usize> {
        let checkpoints = self.load_checkpoints_for_session(session_id).await?;
        let mut deleted_count = 0;

        for checkpoint in checkpoints {
            match self.delete_checkpoint(&checkpoint.id).await {
                Ok(_) => deleted_count += 1,
                Err(e) => {
                    warn!(
                        checkpoint_id = %checkpoint.id,
                        error = %e,
                        "Failed to delete checkpoint when cleaning up session"
                    );
                }
            }
        }

        // 删除会话的 checkpoint 列表文件
        let list_key = self.checkpoint_list_key(session_id);
        let _ = self.agent_fs.kv.delete(&list_key).await;

        info!(
            session_id = %session_id,
            count = %deleted_count,
            "Deleted all checkpoints for session"
        );

        Ok(deleted_count)
    }

    /// 按保留策略清理过期的 Checkpoint
    pub async fn cleanup_old_checkpoints(
        &self,
        session_id: &Uuid,
        retain_count: usize,
    ) -> Result<usize> {
        let mut checkpoints = self.load_checkpoints_for_session(session_id).await?;

        if checkpoints.len() <= retain_count {
            return Ok(0);
        }

        // 按创建时间排序，最新的在前
        checkpoints.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        // 保留最新的 retain_count 个，删除其余的
        let checkpoints_to_delete = &checkpoints[retain_count..];
        let mut deleted_count = 0;

        for checkpoint in checkpoints_to_delete {
            // 不删除已归档的 Checkpoint
            if checkpoint.is_archived() {
                continue;
            }

            match self.delete_checkpoint(&checkpoint.id).await {
                Ok(_) => deleted_count += 1,
                Err(e) => {
                    tracing::warn!(
                        checkpoint_id = %checkpoint.id,
                        error = %e,
                        "Failed to delete checkpoint during cleanup"
                    );
                }
            }
        }

        tracing::info!(
            session_id = %session_id,
            retain_count = %retain_count,
            deleted_count = %deleted_count,
            "Cleaned up old checkpoints"
        );

        Ok(deleted_count)
    }

    /// 按归档策略清理所有过期的 Checkpoint
    pub async fn cleanup_all_old_checkpoints(
        &self,
        strategy: &crate::models::checkpoint::CheckpointArchivingStrategy,
    ) -> Result<usize> {
        let retain_count = strategy.retain_count.unwrap_or(10);

        // 扫描所有会话的 Checkpoint
        let session_keys = self
            .agent_fs
            .kv
            .scan("checkpoints/")
            .await
            .map_err(|e| CheckpointError::AgentFS(e.to_string()))?;

        let mut session_ids = std::collections::HashSet::new();

        for session_key in session_keys {
            if session_key.ends_with("/list.json")
                && let Some(session_id_str) = session_key
                    .strip_prefix("checkpoints/")
                    .and_then(|s| s.strip_suffix("/list.json"))
                && let Ok(session_id) = Uuid::parse_str(session_id_str)
            {
                session_ids.insert(session_id);
            }
        }

        let mut total_deleted = 0;

        for session_id in session_ids {
            match self
                .cleanup_old_checkpoints(&session_id, retain_count)
                .await
            {
                Ok(count) => total_deleted += count,
                Err(e) => {
                    tracing::warn!(
                        session_id = %session_id,
                        error = %e,
                        "Failed to cleanup checkpoints for session"
                    );
                }
            }
        }

        Ok(total_deleted)
    }

    /// 收集文件信息
    async fn collect_file_infos(&self, paths: &[String]) -> Result<Vec<FileInfo>> {
        let mut infos = Vec::new();

        for path in paths {
            if let Ok(metadata) = tokio::fs::metadata(path).await
                && metadata.is_file()
            {
                let modified_at = metadata
                    .modified()
                    .map(|t| t.into())
                    .unwrap_or_else(|_| Utc::now());

                infos.push(FileInfo {
                    path: path.clone(),
                    size: metadata.len(),
                    modified_at,
                    content_hash: None, // 可以后续实现文件哈希计算
                });
            }
        }

        Ok(infos)
    }

    // ==================== 存储相关方法 ====================

    /// 更新 checkpoint 元数据
    pub async fn update_checkpoint(&self, checkpoint: &Checkpoint) -> Result<()> {
        let key = self.checkpoint_metadata_key(&checkpoint.session_id, &checkpoint.id);
        let data = serde_json::to_vec(checkpoint)?;
        self.agent_fs
            .kv
            .set(&key, &data)
            .await
            .map_err(|e| CheckpointError::AgentFS(e.to_string()))?;

        info!(
            checkpoint_id = %checkpoint.id,
            "Checkpoint updated successfully"
        );

        Ok(())
    }

    /// 保存 checkpoint 元数据
    async fn save_checkpoint(&self, checkpoint: &Checkpoint) -> Result<()> {
        let key = self.checkpoint_metadata_key(&checkpoint.session_id, &checkpoint.id);
        let data = serde_json::to_vec(checkpoint)?;
        self.agent_fs
            .kv
            .set(&key, &data)
            .await
            .map_err(|e| CheckpointError::AgentFS(e.to_string()))?;

        // 更新会话的 checkpoint 列表
        self.update_checkpoint_list(&checkpoint.session_id, checkpoint)
            .await?;

        Ok(())
    }

    /// 加载 checkpoint 元数据
    async fn load_checkpoint(&self, checkpoint_id: &str) -> Result<Checkpoint> {
        // 我们需要先找到这个 checkpoint 属于哪个会话
        // 这里简化处理，实际可能需要更好的索引
        let session_keys = self
            .agent_fs
            .kv
            .scan("checkpoints/")
            .await
            .map_err(|e| CheckpointError::AgentFS(e.to_string()))?;

        for session_key in session_keys {
            if session_key.ends_with("/list.json") {
                let session_id_str = session_key
                    .strip_prefix("checkpoints/")
                    .and_then(|s| s.strip_suffix("/list.json"));

                if let Some(session_id_str) = session_id_str {
                    let key = format!(
                        "checkpoints/{}/{}/metadata.json",
                        session_id_str, checkpoint_id
                    );
                    if let Ok(Some(data)) = self.agent_fs.kv.get(&key).await {
                        return Ok(serde_json::from_slice(&data)?);
                    }
                }
            }
        }

        Err(CheckpointError::NotFound(checkpoint_id.to_string()))
    }

    /// 加载会话的所有 checkpoints
    async fn load_checkpoints_for_session(&self, session_id: &Uuid) -> Result<Vec<Checkpoint>> {
        let list_key = self.checkpoint_list_key(session_id);

        let data_opt = self
            .agent_fs
            .kv
            .get(&list_key)
            .await
            .map_err(|e| CheckpointError::AgentFS(e.to_string()))?;

        let Some(data) = data_opt else {
            return Ok(Vec::new());
        };

        let ids: Vec<String> = serde_json::from_slice(&data)?;
        let mut checkpoints = Vec::new();

        for id in ids {
            let key = self.checkpoint_metadata_key(session_id, &id);
            if let Ok(Some(data)) = self.agent_fs.kv.get(&key).await
                && let Ok(checkpoint) = serde_json::from_slice(&data)
            {
                checkpoints.push(checkpoint);
            }
        }

        Ok(checkpoints)
    }

    /// 更新 checkpoint 列表
    async fn update_checkpoint_list(
        &self,
        session_id: &Uuid,
        checkpoint: &Checkpoint,
    ) -> Result<()> {
        let list_key = self.checkpoint_list_key(session_id);

        let data_opt = self
            .agent_fs
            .kv
            .get(&list_key)
            .await
            .map_err(|e| CheckpointError::AgentFS(e.to_string()))?;

        let mut ids = match data_opt {
            Some(data) => serde_json::from_slice::<Vec<String>>(&data).unwrap_or_default(),
            None => Vec::new(),
        };

        if !ids.contains(&checkpoint.id) {
            ids.push(checkpoint.id.clone());
        }

        let data = serde_json::to_vec(&ids)?;
        self.agent_fs
            .kv
            .set(&list_key, &data)
            .await
            .map_err(|e| CheckpointError::AgentFS(e.to_string()))?;

        Ok(())
    }

    /// 保存文件快照
    async fn save_file_snapshots(
        &self,
        checkpoint_id: &str,
        session_id: &Uuid,
        files: &[FileInfo],
    ) -> Result<()> {
        for file_info in files {
            if let Ok(content) = tokio::fs::read(&file_info.path).await {
                let key = self.file_snapshot_key(session_id, checkpoint_id, &file_info.path);
                self.agent_fs
                    .kv
                    .set(&key, &content)
                    .await
                    .map_err(|e| CheckpointError::AgentFS(e.to_string()))?;
            }
        }
        Ok(())
    }

    /// 加载文件快照
    async fn load_file_snapshots(
        &self,
        checkpoint_id: &str,
        session_id: &Uuid,
    ) -> Result<HashMap<String, Vec<u8>>> {
        let prefix = format!("checkpoints/{}/{}/files/", session_id, checkpoint_id);
        let keys = self
            .agent_fs
            .kv
            .scan(&prefix)
            .await
            .map_err(|e| CheckpointError::AgentFS(e.to_string()))?;

        let mut files = HashMap::new();

        for key in keys {
            if let Ok(Some(data)) = self.agent_fs.kv.get(&key).await {
                // 从 key 中提取文件路径
                if let Some(path) = key.strip_prefix(&prefix) {
                    files.insert(path.to_string(), data);
                }
            }
        }

        Ok(files)
    }

    /// 恢复文件到磁盘
    async fn restore_files(&self, files: &HashMap<String, Vec<u8>>) -> Result<()> {
        for (path, content) in files {
            // 从安全路径恢复原始路径
            let original_path = path.replace("__", "/");
            if let Some(parent) = std::path::Path::new(&original_path).parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&original_path, content).await?;
        }
        Ok(())
    }

    /// 删除文件快照
    async fn delete_file_snapshots(&self, checkpoint_id: &str, session_id: &Uuid) -> Result<()> {
        let prefix = format!("checkpoints/{}/{}/files/", session_id, checkpoint_id);
        let keys = self
            .agent_fs
            .kv
            .scan(&prefix)
            .await
            .map_err(|e| CheckpointError::AgentFS(e.to_string()))?;

        for key in keys {
            let _ = self.agent_fs.kv.delete(&key).await;
        }

        Ok(())
    }

    /// 保存会话快照
    async fn save_session_snapshot(&self, checkpoint_id: &str, session: &Session) -> Result<()> {
        let key = self.session_snapshot_key(checkpoint_id);
        let data = serde_json::to_vec(session)?;
        self.agent_fs
            .kv
            .set(&key, &data)
            .await
            .map_err(|e| CheckpointError::AgentFS(e.to_string()))?;
        Ok(())
    }

    /// 加载会话快照
    async fn load_session_snapshot(&self, checkpoint_id: &str) -> Result<Option<Session>> {
        let key = self.session_snapshot_key(checkpoint_id);
        match self.agent_fs.kv.get(&key).await {
            Ok(Some(data)) => Ok(Some(serde_json::from_slice(&data)?)),
            Ok(None) => Ok(None),
            Err(e) => Err(CheckpointError::AgentFS(e.to_string())),
        }
    }

    /// 删除会话快照
    async fn delete_session_snapshot(&self, checkpoint_id: &str) -> Result<()> {
        let key = self.session_snapshot_key(checkpoint_id);
        let _ = self.agent_fs.kv.delete(&key).await;
        Ok(())
    }

    /// 删除 checkpoint 元数据
    async fn delete_checkpoint_metadata(
        &self,
        checkpoint_id: &str,
        session_id: &Uuid,
    ) -> Result<()> {
        let key = self.checkpoint_metadata_key(session_id, checkpoint_id);
        self.agent_fs
            .kv
            .delete(&key)
            .await
            .map_err(|e| CheckpointError::AgentFS(e.to_string()))?;

        // 从列表中移除
        self.remove_checkpoint_from_list(session_id, checkpoint_id)
            .await?;

        Ok(())
    }

    /// 从列表中移除 checkpoint
    async fn remove_checkpoint_from_list(
        &self,
        session_id: &Uuid,
        checkpoint_id: &str,
    ) -> Result<()> {
        let list_key = self.checkpoint_list_key(session_id);

        let data_opt = self
            .agent_fs
            .kv
            .get(&list_key)
            .await
            .map_err(|e| CheckpointError::AgentFS(e.to_string()))?;

        if let Some(data) = data_opt {
            let mut ids = serde_json::from_slice::<Vec<String>>(&data).unwrap_or_default();
            ids.retain(|id| id != checkpoint_id);

            let data = serde_json::to_vec(&ids)?;
            self.agent_fs
                .kv
                .set(&list_key, &data)
                .await
                .map_err(|e| CheckpointError::AgentFS(e.to_string()))?;
        }

        Ok(())
    }

    // ==================== Key 生成帮助方法 ====================

    fn checkpoint_list_key(&self, session_id: &Uuid) -> String {
        format!("checkpoints/{}/list.json", session_id)
    }

    fn checkpoint_metadata_key(&self, session_id: &Uuid, checkpoint_id: &str) -> String {
        format!("checkpoints/{}/{}/metadata.json", session_id, checkpoint_id)
    }

    fn file_snapshot_key(&self, session_id: &Uuid, checkpoint_id: &str, file_path: &str) -> String {
        // 需要对文件路径进行安全处理
        let safe_path = file_path.replace(['/', '\\'], "__");
        format!(
            "checkpoints/{}/{}/files/{}",
            session_id, checkpoint_id, safe_path
        )
    }

    fn session_snapshot_key(&self, checkpoint_id: &str) -> String {
        format!("checkpoints/sessions/{}.json", checkpoint_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Session;
    use agentfs::AgentFS;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::fs;

    // ==================== 测试辅助函数 ====================

    async fn create_test_agent_fs() -> (Arc<AgentFS>, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("agentfs.db");
        let backend = agentsql::SqlBackend::sqlite(db_path.to_str().unwrap())
            .await
            .unwrap();
        let agent_fs = AgentFS::new(Box::new(backend), "test", "/test")
            .await
            .unwrap();
        (Arc::new(agent_fs), temp_dir)
    }

    fn create_test_session() -> Session {
        Session::new()
    }

    fn create_test_config(enabled: bool) -> CheckpointConfig {
        CheckpointConfig {
            enabled,
            checkpoint_directory: ".checkpoints".to_string(),
        }
    }

    async fn create_test_file(
        dir: &std::path::Path,
        name: &str,
        content: &str,
    ) -> std::path::PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).await.unwrap();
        path
    }

    // ==================== 基础功能测试 ====================

    #[tokio::test]
    async fn test_create_manager() {
        let (agent_fs, _temp_dir) = create_test_agent_fs().await;
        let config = create_test_config(true);
        let manager = CheckpointManager::new(agent_fs.clone(), config.clone());

        assert_eq!(manager.config().enabled, config.enabled);
    }

    #[tokio::test]
    async fn test_create_simple_checkpoint() {
        let (agent_fs, _temp_dir) = create_test_agent_fs().await;
        let config = create_test_config(true);
        let manager = CheckpointManager::new(agent_fs, config);
        let session = create_test_session();

        let checkpoint = manager
            .create_checkpoint(
                &session,
                Some("Test checkpoint".to_string()),
                CheckpointType::Manual,
                None,
            )
            .await
            .unwrap();

        assert_eq!(checkpoint.session_id, session.id);
        assert_eq!(checkpoint.checkpoint_type, CheckpointType::Manual);
        assert_eq!(checkpoint.description, Some("Test checkpoint".to_string()));
        assert!(checkpoint.affected_files.is_empty());
    }

    #[tokio::test]
    async fn test_create_checkpoint_with_files() {
        let temp_dir = tempdir().unwrap();
        let (agent_fs, _temp_dir) = create_test_agent_fs().await;
        let config = create_test_config(true);
        let manager = CheckpointManager::new(agent_fs, config);
        let session = create_test_session();

        let test_file = create_test_file(temp_dir.path(), "test.txt", "Hello, World!").await;

        let checkpoint = manager
            .create_checkpoint(
                &session,
                None,
                CheckpointType::Auto,
                Some(vec![test_file.to_string_lossy().to_string()]),
            )
            .await
            .unwrap();

        assert_eq!(checkpoint.affected_files.len(), 1);
        assert_eq!(
            checkpoint.affected_files[0].path,
            test_file.to_string_lossy().to_string()
        );
        assert_eq!(checkpoint.affected_files[0].size, 13);
    }

    #[tokio::test]
    async fn test_list_checkpoints() {
        let (agent_fs, _temp_dir) = create_test_agent_fs().await;
        let config = create_test_config(true);
        let manager = CheckpointManager::new(agent_fs, config);
        let session = create_test_session();

        manager
            .create_checkpoint(
                &session,
                Some("CP1".to_string()),
                CheckpointType::Manual,
                None,
            )
            .await
            .unwrap();
        manager
            .create_checkpoint(
                &session,
                Some("CP2".to_string()),
                CheckpointType::Manual,
                None,
            )
            .await
            .unwrap();

        let response = manager.list_checkpoints(&session.id).await.unwrap();
        assert_eq!(response.total_count, 2);
        assert_eq!(response.checkpoints.len(), 2);
    }

    #[tokio::test]
    async fn test_get_checkpoint() {
        let (agent_fs, _temp_dir) = create_test_agent_fs().await;
        let config = create_test_config(true);
        let manager = CheckpointManager::new(agent_fs, config);
        let session = create_test_session();

        let created = manager
            .create_checkpoint(
                &session,
                Some("Get test".to_string()),
                CheckpointType::Manual,
                None,
            )
            .await
            .unwrap();

        let retrieved = manager.get_checkpoint(&created.id).await.unwrap();
        assert_eq!(retrieved.id, created.id);
        assert_eq!(retrieved.description, Some("Get test".to_string()));
    }

    #[tokio::test]
    async fn test_delete_checkpoint() {
        let (agent_fs, _temp_dir) = create_test_agent_fs().await;
        let config = create_test_config(true);
        let manager = CheckpointManager::new(agent_fs, config);
        let session = create_test_session();

        let checkpoint = manager
            .create_checkpoint(&session, None, CheckpointType::Manual, None)
            .await
            .unwrap();

        manager.delete_checkpoint(&checkpoint.id).await.unwrap();

        let result = manager.get_checkpoint(&checkpoint.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_all_checkpoints_for_session() {
        let (agent_fs, _temp_dir) = create_test_agent_fs().await;
        let config = create_test_config(true);
        let manager = CheckpointManager::new(agent_fs, config);
        let session = create_test_session();

        manager
            .create_checkpoint(&session, None, CheckpointType::Manual, None)
            .await
            .unwrap();
        manager
            .create_checkpoint(&session, None, CheckpointType::Manual, None)
            .await
            .unwrap();

        let deleted_count = manager
            .delete_all_checkpoints_for_session(&session.id)
            .await
            .unwrap();
        assert_eq!(deleted_count, 2);

        let response = manager.list_checkpoints(&session.id).await.unwrap();
        assert_eq!(response.total_count, 0);
    }

    #[tokio::test]
    async fn test_checkpoint_parent_relationship() {
        let (agent_fs, _temp_dir) = create_test_agent_fs().await;
        let config = create_test_config(true);
        let manager = CheckpointManager::new(agent_fs, config);
        let session = create_test_session();

        let cp1 = manager
            .create_checkpoint(&session, None, CheckpointType::Manual, None)
            .await
            .unwrap();

        let cp2 = manager
            .create_checkpoint(&session, None, CheckpointType::Manual, None)
            .await
            .unwrap();

        assert!(cp1.parent_id.is_none());
        assert_eq!(cp2.parent_id, Some(cp1.id));
    }

    #[tokio::test]
    async fn test_checkpoint_disabled() {
        let (agent_fs, _temp_dir) = create_test_agent_fs().await;
        let config = create_test_config(false);
        let manager = CheckpointManager::new(agent_fs, config);
        let session = create_test_session();

        let result = manager
            .create_checkpoint(&session, None, CheckpointType::Manual, None)
            .await;

        assert!(result.is_err());
        match result.err().unwrap() {
            CheckpointError::InvalidData(msg) => {
                assert!(msg.contains("Checkpoint is disabled"));
            }
            _ => panic!("Expected InvalidData error"),
        }
    }

    #[tokio::test]
    async fn test_restore_checkpoint_files() {
        let temp_dir = tempdir().unwrap();
        let (agent_fs, _temp_dir) = create_test_agent_fs().await;
        let config = create_test_config(true);
        let manager = CheckpointManager::new(agent_fs, config);
        let session = create_test_session();

        let test_file =
            create_test_file(temp_dir.path(), "restore_test.txt", "Original content").await;
        let file_path_str = test_file.to_string_lossy().to_string();

        let checkpoint = manager
            .create_checkpoint(
                &session,
                None,
                CheckpointType::Manual,
                Some(vec![file_path_str.clone()]),
            )
            .await
            .unwrap();

        fs::write(&test_file, "Modified content").await.unwrap();
        let modified_content = fs::read_to_string(&test_file).await.unwrap();
        assert_eq!(modified_content, "Modified content");

        let snapshot = manager
            .restore_checkpoint(&checkpoint.id, true, false)
            .await
            .unwrap();

        let restored_content = fs::read_to_string(&test_file).await.unwrap();
        assert_eq!(restored_content, "Original content");
        assert_eq!(snapshot.checkpoint.id, checkpoint.id);
    }

    // ==================== 错误处理测试 ====================

    #[tokio::test]
    async fn test_restore_nonexistent_checkpoint() {
        let (agent_fs, _temp_dir) = create_test_agent_fs().await;
        let config = create_test_config(true);
        let manager = CheckpointManager::new(agent_fs, config);

        let result = manager
            .restore_checkpoint("nonexistent_id", true, true)
            .await;
        assert!(result.is_err());
        match result.err().unwrap() {
            CheckpointError::NotFound(_) => {}
            _ => panic!("Expected NotFound error"),
        }
    }

    #[tokio::test]
    async fn test_delete_nonexistent_checkpoint() {
        let (agent_fs, _temp_dir) = create_test_agent_fs().await;
        let config = create_test_config(true);
        let manager = CheckpointManager::new(agent_fs, config);

        let result = manager.delete_checkpoint("nonexistent_id").await;
        assert!(result.is_err());
        match result.err().unwrap() {
            CheckpointError::NotFound(_) => {}
            _ => panic!("Expected NotFound error"),
        }
    }

    #[tokio::test]
    async fn test_get_nonexistent_checkpoint() {
        let (agent_fs, _temp_dir) = create_test_agent_fs().await;
        let config = create_test_config(true);
        let manager = CheckpointManager::new(agent_fs, config);

        let result = manager.get_checkpoint("nonexistent_id").await;
        assert!(result.is_err());
        match result.err().unwrap() {
            CheckpointError::NotFound(_) => {}
            _ => panic!("Expected NotFound error"),
        }
    }

    #[tokio::test]
    async fn test_collect_nonexistent_files() {
        let (agent_fs, _temp_dir) = create_test_agent_fs().await;
        let config = create_test_config(true);
        let manager = CheckpointManager::new(agent_fs, config);
        let session = create_test_session();

        let checkpoint = manager
            .create_checkpoint(
                &session,
                None,
                CheckpointType::Manual,
                Some(vec!["/nonexistent/file.txt".to_string()]),
            )
            .await
            .unwrap();

        assert!(checkpoint.affected_files.is_empty());
    }
}
