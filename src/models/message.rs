use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::AgentId;

/// 工具定义
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// 工具调用
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// 工具结果
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub content: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    ToolCall,
    ToolResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub session_id: Uuid,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
    /// 工具调用列表（仅当 role == ToolCall 时有值）
    pub tool_calls: Option<Vec<ToolCall>>,
    /// 工具结果（仅当 role == ToolResult 时有值）
    pub tool_result: Option<ToolResult>,
    /// 关联的 checkpoint ID（可选）
    pub checkpoint_id: Option<String>,
    /// 发送该消息的 Agent ID（可选，None 表示用户/系统消息）
    pub agent_id: Option<AgentId>,
}

impl Message {
    pub fn new(session_id: Uuid, role: MessageRole, content: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            session_id,
            role,
            content,
            timestamp: Utc::now(),
            metadata: None,
            tool_calls: None,
            tool_result: None,
            checkpoint_id: None,
            agent_id: None,
        }
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn with_tool_calls(mut self, tool_calls: Vec<ToolCall>) -> Self {
        self.tool_calls = Some(tool_calls);
        self
    }

    pub fn with_tool_result(mut self, tool_result: ToolResult) -> Self {
        self.tool_result = Some(tool_result);
        self
    }

    pub fn with_checkpoint_id(mut self, checkpoint_id: String) -> Self {
        self.checkpoint_id = Some(checkpoint_id);
        self
    }

    pub fn with_agent_id(mut self, agent_id: AgentId) -> Self {
        self.agent_id = Some(agent_id);
        self
    }
}

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub session_id: Option<Uuid>,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct SendMessageResponse {
    pub message_id: Uuid,
    pub session_id: Uuid,
    pub assistant_response: String,
}

#[derive(Debug, Serialize)]
pub struct ListMessagesResponse {
    pub messages: Vec<Message>,
}

// ==================== 管理 API 数据结构 ====================

/// 工具信息（用于列表 API）
#[derive(Debug, Clone, Serialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub server_name: String,
    pub input_schema: serde_json::Value,
}

/// 工具列表响应
#[derive(Debug, Serialize)]
pub struct ListToolsResponse {
    pub tools: Vec<ToolInfo>,
}

/// MCP 服务器信息
#[derive(Debug, Clone, Serialize)]
pub struct McpServerInfo {
    pub name: String,
    pub status: crate::mcp::ServerStatus,
    pub tool_count: usize,
    pub uptime_seconds: Option<u64>,
    pub last_health_check: Option<DateTime<Utc>>,
}

/// MCP 服务器列表响应
#[derive(Debug, Serialize)]
pub struct ListMcpServersResponse {
    pub servers: Vec<McpServerInfo>,
}

/// 重启 MCP 服务器响应
#[derive(Debug, Serialize)]
pub struct RestartMcpServerResponse {
    pub success: bool,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_message_new_defaults() {
        let session_id = Uuid::new_v4();
        let msg = Message::new(session_id, MessageRole::User, "hello".to_string());

        assert_eq!(msg.session_id, session_id);
        assert_eq!(msg.role, MessageRole::User);
        assert_eq!(msg.content, "hello");
        assert!(msg.metadata.is_none());
        assert!(msg.tool_calls.is_none());
        assert!(msg.tool_result.is_none());
        assert!(msg.checkpoint_id.is_none());
        assert!(msg.agent_id.is_none());
    }

    #[test]
    fn test_with_checkpoint_id() {
        let session_id = Uuid::new_v4();
        let checkpoint_id = "checkpoint_123".to_string();

        let msg = Message::new(session_id, MessageRole::User, "hello".to_string())
            .with_checkpoint_id(checkpoint_id.clone());

        assert_eq!(msg.checkpoint_id, Some(checkpoint_id));
    }

    #[test]
    fn test_message_with_checkpoint_id_serialization() {
        let session_id = Uuid::new_v4();
        let checkpoint_id = "checkpoint_456".to_string();

        let msg = Message::new(session_id, MessageRole::User, "hello".to_string())
            .with_checkpoint_id(checkpoint_id.clone());

        let serialized = serde_json::to_string(&msg).unwrap();
        let deserialized: Message = serde_json::from_str(&serialized).unwrap();

        assert_eq!(msg.id, deserialized.id);
        assert_eq!(msg.role, deserialized.role);
        assert_eq!(deserialized.checkpoint_id, Some(checkpoint_id));
    }

    #[test]
    fn test_with_tool_calls() {
        let session_id = Uuid::new_v4();
        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "test_tool".to_string(),
            arguments: json!({"param": "value"}),
        };

        let msg = Message::new(session_id, MessageRole::ToolCall, "".to_string())
            .with_tool_calls(vec![tool_call.clone()]);

        assert!(msg.tool_calls.is_some());
        let calls = msg.tool_calls.unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_123");
        assert_eq!(calls[0].name, "test_tool");
    }

    #[test]
    fn test_with_tool_result() {
        let session_id = Uuid::new_v4();
        let tool_result = ToolResult {
            tool_call_id: "call_123".to_string(),
            content: "result content".to_string(),
            is_error: false,
        };

        let msg = Message::new(session_id, MessageRole::ToolResult, "".to_string())
            .with_tool_result(tool_result.clone());

        assert!(msg.tool_result.is_some());
        let result = msg.tool_result.unwrap();
        assert_eq!(result.tool_call_id, "call_123");
        assert_eq!(result.content, "result content");
        assert!(!result.is_error);
    }

    #[test]
    fn test_message_role_serialization() {
        let roles = vec![
            MessageRole::User,
            MessageRole::Assistant,
            MessageRole::System,
            MessageRole::ToolCall,
            MessageRole::ToolResult,
        ];

        for role in roles {
            let serialized = serde_json::to_string(&role).unwrap();
            let deserialized: MessageRole = serde_json::from_str(&serialized).unwrap();
            assert_eq!(role, deserialized);
        }
    }

    #[test]
    fn test_tool_serialization() {
        let tool = Tool {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            input_schema: json!({"type": "object"}),
        };

        let serialized = serde_json::to_string(&tool).unwrap();
        let deserialized: Tool = serde_json::from_str(&serialized).unwrap();

        assert_eq!(tool.name, deserialized.name);
        assert_eq!(tool.description, deserialized.description);
        assert_eq!(tool.input_schema, deserialized.input_schema);
    }

    #[test]
    fn test_tool_call_serialization() {
        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "test_tool".to_string(),
            arguments: json!({"param": "value"}),
        };

        let serialized = serde_json::to_string(&tool_call).unwrap();
        let deserialized: ToolCall = serde_json::from_str(&serialized).unwrap();

        assert_eq!(tool_call.id, deserialized.id);
        assert_eq!(tool_call.name, deserialized.name);
        assert_eq!(tool_call.arguments, deserialized.arguments);
    }

    #[test]
    fn test_tool_result_serialization() {
        let tool_result = ToolResult {
            tool_call_id: "call_123".to_string(),
            content: "result".to_string(),
            is_error: true,
        };

        let serialized = serde_json::to_string(&tool_result).unwrap();
        let deserialized: ToolResult = serde_json::from_str(&serialized).unwrap();

        assert_eq!(tool_result.tool_call_id, deserialized.tool_call_id);
        assert_eq!(tool_result.content, deserialized.content);
        assert_eq!(tool_result.is_error, deserialized.is_error);
    }

    #[test]
    fn test_message_with_tool_calls_serialization() {
        let session_id = Uuid::new_v4();
        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "test_tool".to_string(),
            arguments: json!({"param": "value"}),
        };

        let msg = Message::new(session_id, MessageRole::ToolCall, "".to_string())
            .with_tool_calls(vec![tool_call]);

        let serialized = serde_json::to_string(&msg).unwrap();
        let deserialized: Message = serde_json::from_str(&serialized).unwrap();

        assert_eq!(msg.id, deserialized.id);
        assert_eq!(msg.role, deserialized.role);
        assert!(deserialized.tool_calls.is_some());
    }

    #[test]
    fn test_with_agent_id() {
        let session_id = Uuid::new_v4();
        let agent_id = AgentId::new();

        let msg = Message::new(session_id, MessageRole::Assistant, "hello".to_string())
            .with_agent_id(agent_id);

        assert_eq!(msg.agent_id, Some(agent_id));
    }

    #[test]
    fn test_message_with_agent_id_serialization() {
        let session_id = Uuid::new_v4();
        let agent_id = AgentId::new();

        let msg = Message::new(session_id, MessageRole::Assistant, "hello".to_string())
            .with_agent_id(agent_id);

        let serialized = serde_json::to_string(&msg).unwrap();
        let deserialized: Message = serde_json::from_str(&serialized).unwrap();

        assert_eq!(msg.id, deserialized.id);
        assert_eq!(msg.role, deserialized.role);
        assert_eq!(deserialized.agent_id, Some(agent_id));
    }

    #[test]
    fn test_chained_builders() {
        let session_id = Uuid::new_v4();
        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "test_tool".to_string(),
            arguments: json!({"param": "value"}),
        };
        let metadata = json!({"key": "value"});
        let checkpoint_id = "checkpoint_789".to_string();
        let agent_id = AgentId::new();

        let msg = Message::new(session_id, MessageRole::ToolCall, "".to_string())
            .with_metadata(metadata.clone())
            .with_tool_calls(vec![tool_call])
            .with_checkpoint_id(checkpoint_id.clone())
            .with_agent_id(agent_id);

        assert_eq!(msg.metadata, Some(metadata));
        assert!(msg.tool_calls.is_some());
        assert_eq!(msg.checkpoint_id, Some(checkpoint_id));
        assert_eq!(msg.agent_id, Some(agent_id));
    }
}
