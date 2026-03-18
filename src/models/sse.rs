//! SSE 事件定义
//!
//! 用于 Server-Sent Events 流式响应的事件类型。

use serde::Serialize;
use serde_json::Value;

/// SSE 事件类型
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SseEvent {
    /// 会话开始
    SessionStarted { session_id: String },
    /// 助手消息
    AssistantMessage {
        agent_id: Option<String>,
        content: String,
    },
    /// Agent 被创建 (agent_spawned)
    AgentSpawned {
        agent_id: String,
        role: String,
        parent_id: Option<String>,
        depth: usize,
    },
    /// Agent 状态变更 (agent_status)
    AgentStatus {
        agent_id: String,
        status: String, // idle, thinking, typing, searching, panicked, celebrating
    },
    /// 工具调用
    ToolCall {
        agent_id: Option<String>,
        tool: String,
        arguments: Value,
    },
    /// 工具结果
    ToolResult {
        agent_id: Option<String>,
        content: String,
        is_error: bool,
    },
    /// 工单更新 (work_order_update)
    WorkOrderUpdate {
        order_id: String,
        from: String,
        to: String,
        status: String, // assigned, completed, failed
    },
    /// 上下通管理告警 (cma_alert)
    CmaAlert {
        level: String, // info, warning, error
        message: String,
        agent_id: Option<String>,
    },
    /// 完成
    Completed,
    /// 错误
    Error { message: String },
}

impl SseEvent {
    /// 创建会话开始事件
    pub fn session_started(session_id: impl Into<String>) -> Self {
        Self::SessionStarted {
            session_id: session_id.into(),
        }
    }

    /// 创建助手消息事件
    pub fn assistant_message(agent_id: Option<String>, content: impl Into<String>) -> Self {
        Self::AssistantMessage {
            agent_id,
            content: content.into(),
        }
    }

    /// 创建 Agent 被创建事件
    pub fn agent_spawned(
        agent_id: impl Into<String>,
        role: impl Into<String>,
        parent_id: Option<String>,
        depth: usize,
    ) -> Self {
        Self::AgentSpawned {
            agent_id: agent_id.into(),
            role: role.into(),
            parent_id,
            depth,
        }
    }

    /// 创建 Agent 状态变更事件
    pub fn agent_status(agent_id: impl Into<String>, status: impl Into<String>) -> Self {
        Self::AgentStatus {
            agent_id: agent_id.into(),
            status: status.into(),
        }
    }

    /// 创建工具调用事件
    pub fn tool_call(agent_id: Option<String>, tool: impl Into<String>, arguments: Value) -> Self {
        Self::ToolCall {
            agent_id,
            tool: tool.into(),
            arguments,
        }
    }

    /// 创建工具结果事件
    pub fn tool_result(
        agent_id: Option<String>,
        content: impl Into<String>,
        is_error: bool,
    ) -> Self {
        Self::ToolResult {
            agent_id,
            content: content.into(),
            is_error,
        }
    }

    /// 创建工单更新事件
    pub fn work_order_update(
        order_id: impl Into<String>,
        from: impl Into<String>,
        to: impl Into<String>,
        status: impl Into<String>,
    ) -> Self {
        Self::WorkOrderUpdate {
            order_id: order_id.into(),
            from: from.into(),
            to: to.into(),
            status: status.into(),
        }
    }

    /// 创建 CMA 告警事件
    pub fn cma_alert(
        level: impl Into<String>,
        message: impl Into<String>,
        agent_id: Option<String>,
    ) -> Self {
        Self::CmaAlert {
            level: level.into(),
            message: message.into(),
            agent_id,
        }
    }

    /// 创建完成事件
    pub fn completed() -> Self {
        Self::Completed
    }

    /// 创建错误事件
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
        }
    }

    /// 序列化为 JSON 字符串
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_session_started_serialization() {
        let event = SseEvent::session_started("test-session-id");
        let json = event.to_json().unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["type"], "session_started");
        assert_eq!(parsed["session_id"], "test-session-id");
    }

    #[test]
    fn test_assistant_message_serialization() {
        let event = SseEvent::assistant_message(Some("A1".to_string()), "Hello, world!");
        let json = event.to_json().unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["type"], "assistant_message");
        assert_eq!(parsed["agent_id"], "A1");
        assert_eq!(parsed["content"], "Hello, world!");
    }

    #[test]
    fn test_agent_spawned_serialization() {
        let event = SseEvent::agent_spawned("A2", "Coder", Some("A1".to_string()), 1);
        let json = event.to_json().unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["type"], "agent_spawned");
        assert_eq!(parsed["agent_id"], "A2");
        assert_eq!(parsed["role"], "Coder");
        assert_eq!(parsed["parent_id"], "A1");
        assert_eq!(parsed["depth"], 1);
    }

    #[test]
    fn test_tool_call_serialization() {
        let args = json!({ "a": 1, "b": 2 });
        let event = SseEvent::tool_call(Some("A2".to_string()), "add", args.clone());
        let json = event.to_json().unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["type"], "tool_call");
        assert_eq!(parsed["agent_id"], "A2");
        assert_eq!(parsed["tool"], "add");
        assert_eq!(parsed["arguments"], args);
    }

    #[test]
    fn test_tool_result_serialization() {
        let event = SseEvent::tool_result(Some("A2".to_string()), "3", false);
        let json = event.to_json().unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["type"], "tool_result");
        assert_eq!(parsed["agent_id"], "A2");
        assert_eq!(parsed["content"], "3");
        assert_eq!(parsed["is_error"], false);
    }

    #[test]
    fn test_completed_serialization() {
        let event = SseEvent::completed();
        let json = event.to_json().unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["type"], "completed");
    }

    #[test]
    fn test_error_serialization() {
        let event = SseEvent::error("Something went wrong");
        let json = event.to_json().unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["type"], "error");
        assert_eq!(parsed["message"], "Something went wrong");
    }
}
