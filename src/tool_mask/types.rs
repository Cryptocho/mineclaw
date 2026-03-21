//! 工具掩码类型定义
//!
//! 提供工具权限管理和过滤功能，支持 MCP 工具精细授权和本地文件工具一键模式。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

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
// FsPermission - 文件系统工具权限
// ============================================================================

/// 本地文件系统工具权限
///
/// 支持只读/读写两种权限级别。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FsPermission {
    /// 只读权限
    ReadOnly,
    /// 读写权限
    ReadWrite,
}

impl fmt::Display for FsPermission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadOnly => write!(f, "ReadOnly"),
            Self::ReadWrite => write!(f, "ReadWrite"),
        }
    }
}

// ============================================================================
// FsAccessLevel - 文件系统访问级别（一键模式）
// ============================================================================

/// 本地文件系统访问级别
///
/// 一键切换所有本地文件工具的权限模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FsAccessLevel {
    /// 所有文件工具只读
    ReadOnly,
    /// 所有文件工具读写
    ReadWrite,
}

impl fmt::Display for FsAccessLevel {
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

/// 写入型本地工具列表（方案 A：硬编码筛选）
const WRITE_TOOLS: &[&str] = &[
    "write_file",
    "edit_file",
    "delete_path",
    "move_path",
    "copy_path",
    "create_directory",
    "create_checkpoint",
    "restore_checkpoint",
];

/// 工具掩码配置
///
/// 决定 Agent 可见和可用的工具子集。直接嵌入在 Agent 结构中。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMask {
    /// MCP 工具权限：{server_name: {tool_name: permission}}
    pub mcp_permissions: HashMap<String, HashMap<String, McpToolPermission>>,
    /// 本地工具精细权限配置：{tool_name: permission}
    pub local_permissions: HashMap<String, FsPermission>,
    /// 本地文件系统访问级别（一键模式，优先级最高）
    pub fs_access_level: Option<FsAccessLevel>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

impl ToolMask {
    /// 创建新的工具掩码配置
    pub fn new() -> Self {
        Self {
            mcp_permissions: HashMap::new(),
            local_permissions: HashMap::new(),
            fs_access_level: None,
            updated_at: Utc::now(),
        }
    }

    /// 创建一个默认只读的掩码
    pub fn readonly() -> Self {
        let mut mask = Self::new();
        mask.fs_access_level = Some(FsAccessLevel::ReadOnly);
        mask
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
            .or_default()
            .insert(tool_name, permission);
        self.updated_at = Utc::now();
    }

    /// 设置本地工具权限
    pub fn set_local_permission(
        &mut self,
        tool_name: String,
        permission: FsPermission,
    ) {
        self.local_permissions.insert(tool_name, permission);
        self.updated_at = Utc::now();
    }

    /// 检查 MCP 工具是否可见/可用
    pub fn is_mcp_tool_available(&self, server_name: &str, tool_name: &str) -> bool {
        // 默认 MCP 工具不可见
        self.mcp_permissions
            .get(server_name)
            .and_then(|tools| tools.get(tool_name))
            .map(|p| *p == McpToolPermission::Available)
            .unwrap_or(false)
    }

    /// 检查本地工具是否可见/可用
    pub fn is_local_tool_available(&self, tool_name: &str) -> bool {
        // 终端工具（Terminal）总是全开放，豁免掩码检查
        if tool_name.starts_with("terminal_") || tool_name == "execute_command" {
            return true;
        }

        // 检查是否是写入型工具
        let is_write = WRITE_TOOLS.contains(&tool_name);

        // 1. 全局 FsAccessLevel 优先
        if let Some(level) = self.fs_access_level {
            return match level {
                FsAccessLevel::ReadOnly => !is_write, // 只读模式下禁用写入工具
                FsAccessLevel::ReadWrite => true,     // 读写模式下全部开启
            };
        }

        // 2. 检查单个工具配置
        if let Some(perm) = self.local_permissions.get(tool_name) {
            return match perm {
                FsPermission::ReadOnly => !is_write,
                FsPermission::ReadWrite => true,
            };
        }

        // 3. 默认策略：如果是写入工具，默认禁用；如果是读取工具，默认允许
        !is_write
    }

    /// 过滤工具列表
    pub fn filter_tools(
        &self,
        server_name: Option<&str>,
        tools: Vec<(String, crate::models::Tool)>,
    ) -> Vec<(String, crate::models::Tool)> {
        tools
            .into_iter()
            .filter(|(name, _)| {
                if let Some(sn) = server_name {
                    self.is_mcp_tool_available(sn, name)
                } else {
                    self.is_local_tool_available(name)
                }
            })
            .collect()
    }
}

impl Default for ToolMask {
    fn default() -> Self {
        Self::new()
    }
}
