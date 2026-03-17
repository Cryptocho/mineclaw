//! 工具掩码仓库

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::agent::AgentId;
use crate::error::{Error, Result};

use super::types::{LocalToolMode, LocalToolPermission, McpToolPermission, ToolMask};

// ============================================================================
// ToolMaskRepository - 工具掩码仓库 trait
// ============================================================================

/// 工具掩码仓库 trait
///
/// 定义工具掩码配置的存储和查询接口。
#[async_trait::async_trait]
pub trait ToolMaskRepository: Send + Sync {
    /// 获取 Agent 的工具掩码配置
    async fn get(&self, agent_id: AgentId) -> Result<ToolMask>;

    /// 保存 Agent 的工具掩码配置
    async fn save(&self, mask: ToolMask) -> Result<()>;

    /// 删除 Agent 的工具掩码配置
    async fn delete(&self, agent_id: AgentId) -> Result<()>;

    /// 检查 Agent 是否有工具掩码配置
    async fn exists(&self, agent_id: AgentId) -> Result<bool>;

    /// 设置 MCP 工具权限
    async fn set_mcp_permission(
        &self,
        agent_id: AgentId,
        server_name: String,
        tool_name: String,
        permission: McpToolPermission,
    ) -> Result<()>;

    /// 设置本地工具权限
    async fn set_local_permission(
        &self,
        agent_id: AgentId,
        tool_name: String,
        permission: LocalToolPermission,
    ) -> Result<()>;

    /// 设置本地工具全局模式
    async fn set_local_tool_mode(
        &self,
        agent_id: AgentId,
        mode: Option<LocalToolMode>,
    ) -> Result<()>;

    /// 复制 Agent 的权限配置
    async fn copy_permissions(&self, source_agent_id: AgentId, target_agent_id: AgentId) -> Result<()>;
}

// ============================================================================
// InMemoryToolMaskRepository - 内存实现
// ============================================================================

/// 内存实现的工具掩码仓库
pub struct InMemoryToolMaskRepository {
    masks: RwLock<HashMap<AgentId, ToolMask>>,
}

impl InMemoryToolMaskRepository {
    /// 创建新的内存仓库
    pub fn new() -> Self {
        Self {
            masks: RwLock::new(HashMap::new()),
        }
    }

    /// 创建 Arc 包装的内存仓库
    pub fn new_arc() -> Arc<Self> {
        Arc::new(Self::new())
    }
}

impl Default for InMemoryToolMaskRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ToolMaskRepository for InMemoryToolMaskRepository {
    async fn get(&self, agent_id: AgentId) -> Result<ToolMask> {
        let masks = self.masks.read().await;
        masks
            .get(&agent_id)
            .cloned()
            .ok_or_else(|| Error::ToolMaskNotFound(agent_id.to_string()))
    }

    async fn save(&self, mask: ToolMask) -> Result<()> {
        let mut masks = self.masks.write().await;
        masks.insert(mask.agent_id, mask);
        Ok(())
    }

    async fn delete(&self, agent_id: AgentId) -> Result<()> {
        let mut masks = self.masks.write().await;
        masks.remove(&agent_id);
        Ok(())
    }

    async fn exists(&self, agent_id: AgentId) -> Result<bool> {
        let masks = self.masks.read().await;
        Ok(masks.contains_key(&agent_id))
    }

    async fn set_mcp_permission(
        &self,
        agent_id: AgentId,
        server_name: String,
        tool_name: String,
        permission: McpToolPermission,
    ) -> Result<()> {
        let mut masks = self.masks.write().await;
        let mask = masks.entry(agent_id).or_insert_with(|| ToolMask::new(agent_id));
        mask.set_mcp_permission(server_name, tool_name, permission);
        Ok(())
    }

    async fn set_local_permission(
        &self,
        agent_id: AgentId,
        tool_name: String,
        permission: LocalToolPermission,
    ) -> Result<()> {
        let mut masks = self.masks.write().await;
        let mask = masks.entry(agent_id).or_insert_with(|| ToolMask::new(agent_id));
        mask.set_local_permission(tool_name, permission);
        Ok(())
    }

    async fn set_local_tool_mode(
        &self,
        agent_id: AgentId,
        mode: Option<LocalToolMode>,
    ) -> Result<()> {
        let mut masks = self.masks.write().await;
        let mask = masks.entry(agent_id).or_insert_with(|| ToolMask::new(agent_id));
        mask.set_local_tool_mode(mode);
        Ok(())
    }

    async fn copy_permissions(&self, source_agent_id: AgentId, target_agent_id: AgentId) -> Result<()> {
        let masks = self.masks.read().await;
        let source_mask = masks
            .get(&source_agent_id)
            .ok_or_else(|| Error::ToolMaskNotFound(source_agent_id.to_string()))?;

        let mut target_mask = ToolMask::new(target_agent_id);
        target_mask.mcp_permissions = source_mask.mcp_permissions.clone();
        target_mask.local_permissions = source_mask.local_permissions.clone();
        target_mask.local_tool_mode = source_mask.local_tool_mode.clone();

        drop(masks);

        let mut masks = self.masks.write().await;
        masks.insert(target_agent_id, target_mask);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_repository_save_and_get() {
        let repo = InMemoryToolMaskRepository::new();
        let agent_id = AgentId::new();
        let mask = ToolMask::new(agent_id);

        repo.save(mask.clone()).await.unwrap();

        let retrieved = repo.get(agent_id).await.unwrap();
        assert_eq!(retrieved.agent_id, agent_id);
    }

    #[tokio::test]
    async fn test_repository_exists() {
        let repo = InMemoryToolMaskRepository::new();
        let agent_id = AgentId::new();

        assert!(!repo.exists(agent_id).await.unwrap());

        repo.save(ToolMask::new(agent_id)).await.unwrap();

        assert!(repo.exists(agent_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_repository_delete() {
        let repo = InMemoryToolMaskRepository::new();
        let agent_id = AgentId::new();

        repo.save(ToolMask::new(agent_id)).await.unwrap();
        assert!(repo.exists(agent_id).await.unwrap());

        repo.delete(agent_id).await.unwrap();
        assert!(!repo.exists(agent_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_set_mcp_permission() {
        let repo = InMemoryToolMaskRepository::new();
        let agent_id = AgentId::new();

        repo.set_mcp_permission(
            agent_id,
            "server1".to_string(),
            "tool1".to_string(),
            McpToolPermission::Available,
        )
        .await
        .unwrap();

        let mask = repo.get(agent_id).await.unwrap();
        assert!(mask.is_mcp_tool_available("server1", "tool1"));
    }

    #[tokio::test]
    async fn test_set_local_permission() {
        let repo = InMemoryToolMaskRepository::new();
        let agent_id = AgentId::new();

        repo.set_local_permission(
            agent_id,
            "tool1".to_string(),
            LocalToolPermission::ReadWrite,
        )
        .await
        .unwrap();

        let mask = repo.get(agent_id).await.unwrap();
        assert_eq!(
            mask.get_local_permission("tool1"),
            LocalToolPermission::ReadWrite
        );
    }

    #[tokio::test]
    async fn test_set_local_tool_mode() {
        let repo = InMemoryToolMaskRepository::new();
        let agent_id = AgentId::new();

        repo.set_local_tool_mode(agent_id, Some(LocalToolMode::ReadOnly))
            .await
            .unwrap();

        let mask = repo.get(agent_id).await.unwrap();
        assert_eq!(mask.local_tool_mode, Some(LocalToolMode::ReadOnly));
    }

    #[tokio::test]
    async fn test_copy_permissions() {
        let repo = InMemoryToolMaskRepository::new();
        let source_id = AgentId::new();
        let target_id = AgentId::new();

        repo.set_mcp_permission(
            source_id,
            "server1".to_string(),
            "tool1".to_string(),
            McpToolPermission::Available,
        )
        .await
        .unwrap();

        repo.set_local_permission(
            source_id,
            "tool2".to_string(),
            LocalToolPermission::ReadWrite,
        )
        .await
        .unwrap();

        repo.copy_permissions(source_id, target_id).await.unwrap();

        let target_mask = repo.get(target_id).await.unwrap();
        assert!(target_mask.is_mcp_tool_available("server1", "tool1"));
        assert_eq!(
            target_mask.get_local_permission("tool2"),
            LocalToolPermission::ReadWrite
        );
    }
}
