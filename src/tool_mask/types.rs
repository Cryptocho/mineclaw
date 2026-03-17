//! 工具掩码类型定义

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

use crate::agent::AgentId;

// ============================================================================
// McpToolPermission - MCP 工具权限
// ============================================================================

/// MCP 工具权限
///
/// MCP 工具只有可用/不可用两种状态，权限由 MCP 服务器端控制。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpToolPermission {
    /// 工具可用
    Available,
    /// 工具不可用
    NotAvailable,
}

impl fmt::Display for McpToolPermission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Available => write!(f, "Available"),
            Self::NotAvailable => write!(f, "NotAvailable"),
        }
    }
}

// ============================================================================
// LocalToolPermission - 本地工具权限
// ============================================================================

/// 本地工具权限
///
/// 本地工具支持只读/读写两种权限级别。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalToolPermission {
    /// 只读权限
    ReadOnly,
    /// 读写权限
    ReadWrite,
}

impl fmt::Display for LocalToolPermission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadOnly => write!(f, "ReadOnly"),
            Self::ReadWrite => write!(f, "ReadWrite"),
        }
    }
}

// ============================================================================
// LocalToolMode - 本地工具全局模式
// ============================================================================

/// 本地工具全局模式
///
/// 一键切换所有本地工具的权限模式，优先于单个工具配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalToolMode {
    /// 所有本地工具只读
    ReadOnly,
    /// 所有本地工具读写
    ReadWrite,
}

impl fmt::Display for LocalToolMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadOnly => write!(f, "ReadOnly"),
            Self::ReadWrite => write!(f, "ReadWrite"),
        }
    }
}

// ============================================================================
// ToolMask - 工具掩码配置
// ============================================================================

/// 工具掩码配置
///
/// 为 Agent 配置的工具权限集合。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMask {
    /// 目标 Agent ID
    pub agent_id: AgentId,
    /// MCP 工具权限：{server_name: {tool_name: permission}}
    pub mcp_permissions: HashMap<String, HashMap<String, McpToolPermission>>,
    /// 本地工具权限：{tool_name: permission}
    pub local_permissions: HashMap<String, LocalToolPermission>,
    /// 本地工具全局模式（优先于单个工具配置）
    pub local_tool_mode: Option<LocalToolMode>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

impl ToolMask {
    /// 创建新的工具掩码配置
    pub fn new(agent_id: AgentId) -> Self {
        Self {
            agent_id,
            mcp_permissions: HashMap::new(),
            local_permissions: HashMap::new(),
            local_tool_mode: None,
            updated_at: Utc::now(),
        }
    }

    /// 设置 MCP 工具权限
    pub fn set_mcp_permission(
        &mut self,
        server_name: String,
        tool_name: String,
        permission: McpToolPermission,
    ) {
        self.mcp_permissions
            .entry(server_name)
            .or_insert_with(HashMap::new)
            .insert(tool_name, permission);
        self.updated_at = Utc::now();
    }

    /// 获取 MCP 工具权限
    pub fn get_mcp_permission(
        &self,
        server_name: &str,
        tool_name: &str,
    ) -> Option<&McpToolPermission> {
        self.mcp_permissions
            .get(server_name)
            .and_then(|tools| tools.get(tool_name))
    }

    /// 设置本地工具权限
    pub fn set_local_permission(&mut self, tool_name: String, permission: LocalToolPermission) {
        self.local_permissions.insert(tool_name, permission);
        self.updated_at = Utc::now();
    }

    /// 获取本地工具权限（考虑全局模式）
    pub fn get_local_permission(&self, tool_name: &str) -> LocalToolPermission {
        // 全局模式优先
        if let Some(mode) = &self.local_tool_mode {
            return match mode {
                LocalToolMode::ReadOnly => LocalToolPermission::ReadOnly,
                LocalToolMode::ReadWrite => LocalToolPermission::ReadWrite,
            };
        }

        // 否则使用单个工具配置，默认只读
        self.local_permissions
            .get(tool_name)
            .cloned()
            .unwrap_or(LocalToolPermission::ReadOnly)
    }

    /// 设置本地工具全局模式
    pub fn set_local_tool_mode(&mut self, mode: Option<LocalToolMode>) {
        self.local_tool_mode = mode;
        self.updated_at = Utc::now();
    }

    /// 检查 MCP 工具是否可用
    pub fn is_mcp_tool_available(&self, server_name: &str, tool_name: &str) -> bool {
        // 默认 MCP 工具不可用
        self.get_mcp_permission(server_name, tool_name)
            .map(|p| *p == McpToolPermission::Available)
            .unwrap_or(false)
    }

    /// 检查本地工具是否可用（总是可用，只是权限级别不同）
    pub fn is_local_tool_available(&self, _tool_name: &str) -> bool {
        // 本地工具总是可用
        true
    }

    /// 过滤 MCP 工具，只返回可用的工具
    ///
    /// # 参数
    /// - `server_name`: MCP 服务器名称
    /// - `tools`: 该服务器的所有工具列表
    ///
    /// # 返回
    /// 过滤后的可用工具列表
    pub fn filter_mcp_tools(
        &self,
        server_name: &str,
        tools: Vec<(String, crate::models::Tool)>,
    ) -> Vec<(String, crate::models::Tool)> {
        tools
            .into_iter()
            .filter(|(tool_name, _)| self.is_mcp_tool_available(server_name, tool_name))
            .collect()
    }

    /// 过滤本地工具（本地工具总是可见，只是权限不同）
    ///
    /// # 参数
    /// - `tools`: 所有本地工具列表
    ///
    /// # 返回
    /// 所有本地工具（本地工具总是可见）
    pub fn filter_local_tools(
        &self,
        tools: Vec<(String, crate::models::Tool)>,
    ) -> Vec<(String, crate::models::Tool)> {
        // 本地工具总是可见
        tools
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_mask_new() {
        let agent_id = AgentId::new();
        let mask = ToolMask::new(agent_id);
        assert_eq!(mask.agent_id, agent_id);
        assert!(mask.mcp_permissions.is_empty());
        assert!(mask.local_permissions.is_empty());
        assert!(mask.local_tool_mode.is_none());
    }

    #[test]
    fn test_mcp_tool_permission() {
        let agent_id = AgentId::new();
        let mut mask = ToolMask::new(agent_id);

        mask.set_mcp_permission(
            "server1".to_string(),
            "tool1".to_string(),
            McpToolPermission::Available,
        );

        assert_eq!(
            mask.get_mcp_permission("server1", "tool1"),
            Some(&McpToolPermission::Available)
        );
        assert!(mask.is_mcp_tool_available("server1", "tool1"));
        assert!(!mask.is_mcp_tool_available("server1", "tool2"));
    }

    #[test]
    fn test_local_tool_permission() {
        let agent_id = AgentId::new();
        let mut mask = ToolMask::new(agent_id);

        mask.set_local_permission("tool1".to_string(), LocalToolPermission::ReadWrite);

        assert_eq!(
            mask.get_local_permission("tool1"),
            LocalToolPermission::ReadWrite
        );
        // 默认只读
        assert_eq!(
            mask.get_local_permission("tool2"),
            LocalToolPermission::ReadOnly
        );
    }

    #[test]
    fn test_local_tool_mode_override() {
        let agent_id = AgentId::new();
        let mut mask = ToolMask::new(agent_id);

        mask.set_local_permission("tool1".to_string(), LocalToolPermission::ReadWrite);
        mask.set_local_tool_mode(Some(LocalToolMode::ReadOnly));

        // 全局模式优先，即使单个工具配置为读写
        assert_eq!(
            mask.get_local_permission("tool1"),
            LocalToolPermission::ReadOnly
        );

        // 取消全局模式，使用单个工具配置
        mask.set_local_tool_mode(None);
        assert_eq!(
            mask.get_local_permission("tool1"),
            LocalToolPermission::ReadWrite
        );
    }
}
