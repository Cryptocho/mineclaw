//! MCP 服务器管理器
//!
//! 管理多个 MCP 服务器的生命周期和状态。

use crate::config::McpServerConfig;
use crate::error::{Error, Result};
use crate::mcp::client::McpClient;
use crate::mcp::protocol::{CallToolResponse, ProtocolTool};
use crate::mcp::registry::ToolRegistry;
use crate::mcp::transport::StdioTransport;
use crate::models::Tool;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

// ==================== ServerStatus ====================

/// MCP 服务器状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "message")]
pub enum ServerStatus {
    /// 正在连接
    Connecting,
    /// 已连接
    Connected,
    /// 已断开连接
    Disconnected,
    /// 发生错误
    Error(String),
}

// ==================== McpServerHandle ====================

/// MCP 服务器句柄
pub struct McpServerHandle {
    /// 服务器名称
    pub name: String,
    /// MCP 客户端
    pub client: Option<McpClient>,
    /// 服务器提供的工具
    pub tools: Vec<Tool>,
    /// 服务器状态
    pub status: ServerStatus,
    /// 服务器配置（用于重启）
    pub config: Option<McpServerConfig>,
    /// 启动时间
    pub started_at: Option<DateTime<Utc>>,
    /// 最后健康检查时间
    pub last_health_check: Option<DateTime<Utc>>,
}

impl McpServerHandle {
    /// 创建一个新的服务器句柄
    fn new(name: String) -> Self {
        Self {
            name,
            client: None,
            tools: Vec::new(),
            status: ServerStatus::Connecting,
            config: None,
            started_at: None,
            last_health_check: None,
        }
    }

    /// 将 ProtocolTool 转换为 models::Tool
    fn protocol_tool_to_model(tool: &ProtocolTool) -> Tool {
        Tool {
            name: tool.name.clone(),
            description: tool.description.clone(),
            input_schema: tool.input_schema.clone(),
        }
    }

    /// 获取运行时间（秒）
    pub fn uptime_seconds(&self) -> Option<u64> {
        self.started_at
            .map(|started| (Utc::now() - started).num_seconds().max(0) as u64)
    }
}

// ==================== McpServerManager ====================

/// MCP 服务器管理器
pub struct McpServerManager {
    servers: HashMap<String, McpServerHandle>,
    /// 工具注册表
    tool_registry: ToolRegistry,
}

impl McpServerManager {
    /// 创建一个新的服务器管理器
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
            tool_registry: ToolRegistry::new(),
        }
    }

    /// 获取工具注册表的引用
    pub fn tool_registry(&self) -> &ToolRegistry {
        &self.tool_registry
    }

    /// 启动一个 MCP 服务器
    pub async fn start_server(&mut self, config: &McpServerConfig) -> Result<()> {
        info!(server_name = %config.name, "Starting MCP server");

        if self.servers.contains_key(&config.name) {
            warn!(server_name = %config.name, "Server already exists, stopping first");
            self.stop_server(&config.name).await?;
        }

        let mut handle = McpServerHandle::new(config.name.clone());
        handle.config = Some(config.clone());
        handle.started_at = Some(Utc::now());
        self.servers.insert(config.name.clone(), handle);

        // 启动传输层
        let transport =
            match StdioTransport::spawn(&config.command, &config.args, &config.env).await {
                Ok(t) => t,
                Err(e) => {
                    error!(server_name = %config.name, error = %e, "Failed to spawn server");
                    if let Some(handle) = self.servers.get_mut(&config.name) {
                        handle.status = ServerStatus::Error(e.to_string());
                    }
                    return Err(e);
                }
            };

        // 创建客户端
        let mut client = McpClient::new(Box::new(transport));

        // 初始化
        match client.initialize().await {
            Ok(_) => {
                debug!(server_name = %config.name, "Initialized successfully");
            }
            Err(e) => {
                error!(server_name = %config.name, error = %e, "Failed to initialize server");
                if let Some(handle) = self.servers.get_mut(&config.name) {
                    handle.status = ServerStatus::Error(e.to_string());
                }
                let _ = client.close().await;
                return Err(e);
            }
        }

        // 获取工具列表
        let tools_response = match client.list_tools().await {
            Ok(r) => r,
            Err(e) => {
                error!(server_name = %config.name, error = %e, "Failed to list tools");
                if let Some(handle) = self.servers.get_mut(&config.name) {
                    handle.status = ServerStatus::Error(e.to_string());
                }
                let _ = client.close().await;
                return Err(e);
            }
        };

        let tools: Vec<Tool> = tools_response
            .tools
            .iter()
            .map(McpServerHandle::protocol_tool_to_model)
            .collect();

        info!(
            server_name = %config.name,
            tool_count = tools.len(),
            "MCP server started successfully"
        );

        // 更新句柄
        if let Some(handle) = self.servers.get_mut(&config.name) {
            handle.client = Some(client);
            handle.tools = tools.clone();
            handle.status = ServerStatus::Connected;
            handle.last_health_check = Some(Utc::now());
        }

        // 注册到工具注册表
        self.tool_registry
            .register_server(config.name.clone(), tools);

        Ok(())
    }

    /// 重启一个 MCP 服务器
    pub async fn restart_server(&mut self, name: &str) -> Result<()> {
        info!(server_name = name, "Restarting MCP server");

        let config = self
            .get_server(name)
            .and_then(|h| h.config.clone())
            .ok_or_else(|| Error::McpServer {
                server: name.to_string(),
                message: "Server configuration not found".to_string(),
            })?;

        self.stop_server(name).await?;
        self.start_server(&config).await?;

        info!(server_name = name, "MCP server restarted successfully");
        Ok(())
    }

    /// 健康检查一个 MCP 服务器
    pub async fn health_check(&mut self, name: &str) -> Result<bool> {
        debug!(server_name = name, "Performing health check");

        let handle = match self.servers.get_mut(name) {
            Some(h) => h,
            None => {
                return Ok(false);
            }
        };

        handle.last_health_check = Some(Utc::now());

        if handle.status != ServerStatus::Connected {
            return Ok(false);
        }

        let client = match handle.client.as_mut() {
            Some(c) => c,
            None => {
                handle.status = ServerStatus::Disconnected;
                return Ok(false);
            }
        };

        // 通过调用 list_tools 来测试连接
        match client.list_tools().await {
            Ok(_) => {
                debug!(server_name = name, "Health check passed");
                Ok(true)
            }
            Err(e) => {
                error!(server_name = name, error = %e, "Health check failed");
                handle.status = ServerStatus::Error(e.to_string());
                Ok(false)
            }
        }
    }

    /// 停止一个 MCP 服务器
    pub async fn stop_server(&mut self, name: &str) -> Result<()> {
        info!(server_name = name, "Stopping MCP server");

        if let Some(mut handle) = self.servers.remove(name) {
            if let Some(mut client) = handle.client.take()
                && let Err(e) = client.close().await
            {
                warn!(server_name = name, error = %e, "Error while closing client");
            }
            handle.status = ServerStatus::Error("Stopped".to_string());
        }

        // 从工具注册表中注销
        self.tool_registry.unregister_server(name);

        Ok(())
    }

    /// 获取一个服务器的句柄
    pub fn get_server(&self, name: &str) -> Option<&McpServerHandle> {
        self.servers.get(name)
    }

    /// 获取一个服务器的可变句柄
    pub fn get_server_mut(&mut self, name: &str) -> Option<&mut McpServerHandle> {
        self.servers.get_mut(name)
    }

    /// 列出所有服务器
    pub fn list_servers(&self) -> Vec<&McpServerHandle> {
        self.servers.values().collect()
    }

    /// 获取所有服务器提供的所有工具
    pub fn all_tools(&self) -> Vec<(String, Tool)> {
        let mut result = Vec::new();
        for (server_name, handle) in &self.servers {
            if handle.status == ServerStatus::Connected {
                for tool in &handle.tools {
                    result.push((server_name.clone(), tool.clone()));
                }
            }
        }
        result
    }

    /// 停止所有服务器
    pub async fn stop_all(&mut self) -> Result<()> {
        info!("Stopping all MCP servers");

        let server_names: Vec<String> = self.servers.keys().cloned().collect();
        for name in server_names {
            self.stop_server(&name).await?;
        }

        Ok(())
    }

    /// 查找工具所在的服务器
    pub fn find_tool_server(&self, tool_name: &str) -> Option<&str> {
        self.tool_registry.find_server(tool_name)
    }

    /// 调用工具
    pub async fn call_tool(
        &mut self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<CallToolResponse> {
        debug!(
            server_name = %server_name,
            tool_name = %tool_name,
            "Calling tool"
        );

        let handle = self
            .servers
            .get_mut(server_name)
            .ok_or_else(|| Error::McpServer {
                server: server_name.to_string(),
                message: "Server not found".to_string(),
            })?;

        if handle.status != ServerStatus::Connected {
            return Err(Error::McpServer {
                server: server_name.to_string(),
                message: "Server not connected".to_string(),
            });
        }

        let client = handle.client.as_mut().ok_or_else(|| Error::McpServer {
            server: server_name.to_string(),
            message: "Client not initialized".to_string(),
        })?;

        let response = client.call_tool(tool_name.to_string(), arguments).await?;

        Ok(response)
    }
}

impl Default for McpServerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for McpServerManager {
    fn drop(&mut self) {
        // 因为 drop 不是 async，我们无法优雅地关闭服务器
        // 只能记录警告
        if !self.servers.is_empty() {
            warn!(
                server_count = self.servers.len(),
                "McpServerManager dropped with active servers, they may not be cleanly shut down"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::protocol::ProtocolTool;
    use serde_json::json;

    #[test]
    fn test_server_manager_new() {
        let manager = McpServerManager::new();
        assert!(manager.servers.is_empty());
    }

    #[test]
    fn test_server_handle_new() {
        let handle = McpServerHandle::new("test-server".to_string());
        assert_eq!(handle.name, "test-server");
        assert!(handle.client.is_none());
        assert!(handle.tools.is_empty());
        assert_eq!(handle.status, ServerStatus::Connecting);
    }

    #[test]
    fn test_protocol_tool_to_model() {
        let protocol_tool = ProtocolTool {
            name: "test-tool".to_string(),
            description: "A test tool".to_string(),
            input_schema: json!({"type": "object"}),
        };

        let tool = McpServerHandle::protocol_tool_to_model(&protocol_tool);
        assert_eq!(tool.name, "test-tool");
        assert_eq!(tool.description, "A test tool");
        assert_eq!(tool.input_schema, json!({"type": "object"}));
    }

    #[test]
    fn test_manager_list_servers_empty() {
        let manager = McpServerManager::new();
        let servers = manager.list_servers();
        assert!(servers.is_empty());
    }

    #[test]
    fn test_manager_all_tools_empty() {
        let manager = McpServerManager::new();
        let tools = manager.all_tools();
        assert!(tools.is_empty());
    }

    #[test]
    fn test_server_handle_tool_conversion() {
        let protocol_tool = ProtocolTool {
            name: "echo".to_string(),
            description: "Echo tool".to_string(),
            input_schema: json!({"type": "object"}),
        };

        let tool = McpServerHandle::protocol_tool_to_model(&protocol_tool);
        assert_eq!(tool.name, "echo");
        assert_eq!(tool.description, "Echo tool");
    }
}
