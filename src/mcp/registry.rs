//! MCP 工具注册表
//!
//! 管理多个 MCP 服务器的工具，提供工具查找和路由功能。

use crate::models::Tool;
use std::collections::HashMap;

// ==================== ToolRegistry ====================

/// 工具注册表
pub struct ToolRegistry {
    /// 工具名称到服务器名称的映射
    /// 注意：如果有同名工具，只会保留最后注册的那个
    tool_to_server: HashMap<String, String>,
    /// 服务器名称到工具列表的映射
    server_tools: HashMap<String, Vec<Tool>>,
}

impl ToolRegistry {
    /// 创建一个新的工具注册表
    pub fn new() -> Self {
        Self {
            tool_to_server: HashMap::new(),
            server_tools: HashMap::new(),
        }
    }

    /// 注册服务器工具
    pub fn register_server(&mut self, server_name: String, tools: Vec<Tool>) {
        // 先移除旧的映射
        self.unregister_server(&server_name);

        // 添加新的映射
        for tool in &tools {
            self.tool_to_server
                .insert(tool.name.clone(), server_name.clone());
        }

        self.server_tools.insert(server_name, tools);
    }

    /// 注销服务器工具
    pub fn unregister_server(&mut self, server_name: &str) {
        if let Some(tools) = self.server_tools.remove(server_name) {
            for tool in tools {
                self.tool_to_server.remove(&tool.name);
            }
        }
    }

    /// 查找工具所在的服务器
    pub fn find_server(&self, tool_name: &str) -> Option<&str> {
        self.tool_to_server.get(tool_name).map(|s| s.as_str())
    }

    /// 获取所有工具列表
    pub fn all_tools(&self) -> Vec<(String, Tool)> {
        let mut result = Vec::new();
        for (server_name, tools) in &self.server_tools {
            for tool in tools {
                result.push((server_name.clone(), tool.clone()));
            }
        }
        result
    }

    /// 获取指定服务器的工具列表
    pub fn server_tools(&self, server_name: &str) -> Option<&[Tool]> {
        self.server_tools.get(server_name).map(|v| v.as_slice())
    }

    /// 检查工具是否存在
    pub fn has_tool(&self, tool_name: &str) -> bool {
        self.tool_to_server.contains_key(tool_name)
    }

    /// 获取工具定义
    pub fn get_tool(&self, tool_name: &str) -> Option<&Tool> {
        let server_name = self.tool_to_server.get(tool_name)?;
        let tools = self.server_tools.get(server_name)?;
        tools.iter().find(|t| t.name == tool_name)
    }

    /// 清空注册表
    pub fn clear(&mut self) {
        self.tool_to_server.clear();
        self.server_tools.clear();
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_registry_new() {
        let registry = ToolRegistry::new();
        assert!(registry.tool_to_server.is_empty());
        assert!(registry.server_tools.is_empty());
    }

    #[test]
    fn test_register_server() {
        let mut registry = ToolRegistry::new();

        let tools = vec![Tool {
            name: "echo".to_string(),
            description: "Echo tool".to_string(),
            input_schema: json!({"type": "object"}),
        }];

        registry.register_server("server1".to_string(), tools);

        assert!(registry.has_tool("echo"));
        assert_eq!(registry.find_server("echo"), Some("server1"));
    }

    #[test]
    fn test_unregister_server() {
        let mut registry = ToolRegistry::new();

        let tools = vec![Tool {
            name: "echo".to_string(),
            description: "Echo tool".to_string(),
            input_schema: json!({"type": "object"}),
        }];

        registry.register_server("server1".to_string(), tools);
        assert!(registry.has_tool("echo"));

        registry.unregister_server("server1");
        assert!(!registry.has_tool("echo"));
    }

    #[test]
    fn test_tool_name_conflict() {
        let mut registry = ToolRegistry::new();

        let tools1 = vec![Tool {
            name: "echo".to_string(),
            description: "Echo from server1".to_string(),
            input_schema: json!({"type": "object"}),
        }];

        let tools2 = vec![Tool {
            name: "echo".to_string(),
            description: "Echo from server2".to_string(),
            input_schema: json!({"type": "object"}),
        }];

        registry.register_server("server1".to_string(), tools1);
        registry.register_server("server2".to_string(), tools2);

        // 后注册的应该覆盖先注册的
        assert_eq!(registry.find_server("echo"), Some("server2"));
    }

    #[test]
    fn test_all_tools() {
        let mut registry = ToolRegistry::new();

        let tools1 = vec![Tool {
            name: "echo".to_string(),
            description: "Echo tool".to_string(),
            input_schema: json!({"type": "object"}),
        }];

        let tools2 = vec![Tool {
            name: "add".to_string(),
            description: "Add tool".to_string(),
            input_schema: json!({"type": "object"}),
        }];

        registry.register_server("server1".to_string(), tools1);
        registry.register_server("server2".to_string(), tools2);

        let all_tools = registry.all_tools();
        assert_eq!(all_tools.len(), 2);
    }

    #[test]
    fn test_get_tool() {
        let mut registry = ToolRegistry::new();

        let tool = Tool {
            name: "echo".to_string(),
            description: "Echo tool".to_string(),
            input_schema: json!({"type": "object"}),
        };

        registry.register_server("server1".to_string(), vec![tool.clone()]);

        let found = registry.get_tool("echo");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "echo");
    }

    #[test]
    fn test_server_tools() {
        let mut registry = ToolRegistry::new();

        let tools = vec![
            Tool {
                name: "echo".to_string(),
                description: "Echo tool".to_string(),
                input_schema: json!({"type": "object"}),
            },
            Tool {
                name: "add".to_string(),
                description: "Add tool".to_string(),
                input_schema: json!({"type": "object"}),
            },
        ];

        registry.register_server("server1".to_string(), tools.clone());

        let server_tools = registry.server_tools("server1");
        assert!(server_tools.is_some());
        assert_eq!(server_tools.unwrap().len(), 2);
    }

    #[test]
    fn test_clear() {
        let mut registry = ToolRegistry::new();

        let tools = vec![Tool {
            name: "echo".to_string(),
            description: "Echo tool".to_string(),
            input_schema: json!({"type": "object"}),
        }];

        registry.register_server("server1".to_string(), tools);
        assert!(!registry.all_tools().is_empty());

        registry.clear();
        assert!(registry.all_tools().is_empty());
    }
}
