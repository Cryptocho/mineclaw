//! CMA 上下文管理工具
//!
//! 提供 CMA Agent 使用的工具：读取和裁剪对话上下文。
//!
//! CMA 通过这些工具编辑 Session 的 messages，实现上下文管理。

use std::sync::Arc;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::error::Result;
use crate::models::message::{Message, MessageRole};
use crate::tools::{LocalTool, ToolContext};

pub struct ReadMessagesTool;

impl ReadMessagesTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReadMessagesTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LocalTool for ReadMessagesTool {
    fn name(&self) -> &str {
        "read_messages"
    }

    fn description(&self) -> &str {
        "读取当前会话的所有消息。返回消息列表，包含角色、内容、时间戳和工具调用信息。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn call(&self, _arguments: Value, context: ToolContext) -> Result<Value> {
        let messages = &context.session.messages;
        let total_chars: usize = messages.iter().map(|m| m.content.len()).sum();

        let messages_info: Vec<Value> = messages.iter().enumerate().map(|(i, msg)| {
            json!({
                "index": i,
                "role": format!("{:?}", msg.role),
                "content": msg.content.chars().take(300).collect::<String>(),
                "content_length": msg.content.len(),
                "timestamp": msg.timestamp.to_rfc3339(),
                "agent_id": msg.agent_id.map(|a| a.to_string()),
                "tool_calls_count": msg.tool_calls.as_ref().map(|tc| tc.len()).unwrap_or(0),
            })
        }).collect();

        Ok(json!({
            "total_messages": messages.len(),
            "total_characters": total_chars,
            "messages": messages_info,
        }))
    }
}

pub struct TrimMessagesTool;

impl TrimMessagesTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TrimMessagesTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LocalTool for TrimMessagesTool {
    fn name(&self) -> &str {
        "trim_messages"
    }

    fn description(&self) -> &str {
        "裁剪会话消息。指定要保留的消息索引（0-based），或提供目标消息数让系统自动裁剪（保留重要的消息）。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "keep_indices": {
                    "type": "array",
                    "items": { "type": "integer" },
                    "description": "要保留的消息索引列表（0-based），其他消息会被删除"
                },
                "target_count": {
                    "type": "integer",
                    "description": "目标消息数，系统会自动保留最近的重要消息"
                },
                "add_notice": {
                    "type": "string",
                    "description": "可选，在裁剪后添加的通知消息"
                }
            },
            "required": []
        })
    }

    async fn call(&self, arguments: Value, context: ToolContext) -> Result<Value> {
        let session_id = context.session.id;
        let mut messages = context.session.messages.clone();

        let removed_count;
        let removed_chars;

        if let Some(indices) = arguments["keep_indices"].as_array() {
            let indices: Vec<usize> = indices.iter()
                .filter_map(|v| v.as_u64().map(|n| n as usize))
                .collect();

            let original_count = messages.len();
            let original_chars: usize = messages.iter().map(|m| m.content.len()).sum();

            let mut new_messages = Vec::new();
            for (i, msg) in messages.into_iter().enumerate() {
                if indices.contains(&i) {
                    new_messages.push(msg);
                }
            }
            messages = new_messages;

            removed_count = original_count - messages.len();
            removed_chars = original_chars - messages.iter().map(|m| m.content.len()).sum::<usize>();
        } else if let Some(target) = arguments["target_count"].as_u64() {
            let target = target as usize;

            if messages.len() > target {
                removed_count = messages.len() - target;
                removed_chars = messages[..removed_count].iter().map(|m| m.content.len()).sum::<usize>();
                messages = messages[removed_count..].to_vec();
            } else {
                removed_count = 0;
                removed_chars = 0;
            }
        } else {
            return Err(crate::error::Error::InvalidInput(
                "必须提供 keep_indices 或 target_count".to_string()
            ));
        }

        let mut result = json!({
            "success": true,
            "remaining_messages": messages.len(),
            "removed_count": removed_count,
            "removed_characters": removed_chars,
            "modified_messages": messages.iter().map(|m| json!({
                "index": m.id,
                "role": format!("{:?}", m.role),
                "content_preview": m.content.chars().take(100).collect::<String>(),
            })).collect::<Vec<_>>(),
        });

        if let Some(notice) = arguments["add_notice"].as_str() {
            let notice_msg = Message::new(session_id, MessageRole::System, notice.to_string());
            messages.insert(0, notice_msg);
            result["notice_added"] = json!(notice);
        }

        result["session_id"] = json!(session_id.to_string());
        result["new_messages"] = json!(messages.iter().map(|m| json!({
            "id": m.id.to_string(),
            "role": format!("{:?}", m.role),
            "content": &m.content,
            "timestamp": m.timestamp.to_rfc3339(),
        })).collect::<Vec<_>>());

        Ok(result)
    }
}

pub struct ContextTools;

impl ContextTools {
    pub fn register_all(registry: &mut crate::tools::LocalToolRegistry) {
        registry.register(Arc::new(ReadMessagesTool::new()));
        registry.register(Arc::new(TrimMessagesTool::new()));
    }
}
