//! 工具调用协调器
//!
//! 负责协调 LLM 和 MCP 工具之间的交互，实现工具调用循环。

use crate::agent::AgentId;
use crate::checkpoint::CheckpointManager;
use crate::error::{Error, Result};
use crate::llm::{ChatMessage, ChatTool, LlmProvider};
use crate::mcp::{ExecutionResult, McpServerManager, ToolExecutor};
use crate::models::{Message, MessageRole, Session, Tool, ToolCall, ToolResult};
use crate::tool_mask::{ToolMask, ToolMaskRepository};
use crate::tools::{LocalToolRegistry, ToolContext};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

// ==================== ToolCoordinatorCallback ====================

/// 工具协调器回调 trait
///
/// 用于在工具调用循环的各个阶段接收事件通知。
#[async_trait]
pub trait ToolCoordinatorCallback: Send + Sync {
    /// 当助手发送消息时触发
    async fn on_assistant_message(&self, content: &str);

    /// 当工具调用时触发
    async fn on_tool_call(&self, tool: &str, arguments: &Value);

    /// 当工具返回结果时触发
    async fn on_tool_result(&self, content: &str, is_error: bool);

    /// 当整个流程完成时触发
    async fn on_completed(&self, final_response: &str);

    /// 当发生错误时触发
    async fn on_error(&self, message: &str);
}

/// 空回调实现（默认行为）
pub struct NoopCallback;

#[async_trait]
impl ToolCoordinatorCallback for NoopCallback {
    async fn on_assistant_message(&self, _content: &str) {}
    async fn on_tool_call(&self, _tool: &str, _arguments: &Value) {}
    async fn on_tool_result(&self, _content: &str, _is_error: bool) {}
    async fn on_completed(&self, _final_response: &str) {}
    async fn on_error(&self, _message: &str) {}
}

// ==================== ToolCoordinator ====================

/// 工具调用协调器
pub struct ToolCoordinator {
    llm_provider: Arc<dyn LlmProvider>,
    mcp_server_manager: Arc<Mutex<McpServerManager>>,
    tool_executor: ToolExecutor,
    local_tool_registry: Arc<LocalToolRegistry>,
    config: Arc<crate::config::Config>,
    checkpoint_manager: Option<Arc<CheckpointManager>>,
    tool_mask_repository: Option<Arc<dyn ToolMaskRepository>>,
    /// 最大工具调用轮数
    max_iterations: usize,
}

impl ToolCoordinator {
    /// 创建一个新的工具调用协调器
    pub fn new(
        llm_provider: Arc<dyn LlmProvider>,
        mcp_server_manager: Arc<Mutex<McpServerManager>>,
        tool_executor: ToolExecutor,
        local_tool_registry: Arc<LocalToolRegistry>,
        config: Arc<crate::config::Config>,
    ) -> Self {
        Self {
            llm_provider,
            mcp_server_manager,
            tool_executor,
            local_tool_registry,
            config,
            checkpoint_manager: None,
            tool_mask_repository: None,
            max_iterations: 10,
        }
    }

    /// 设置 CheckpointManager
    pub fn with_checkpoint_manager(mut self, checkpoint_manager: Arc<CheckpointManager>) -> Self {
        self.checkpoint_manager = Some(checkpoint_manager);
        self
    }

    /// 设置 ToolMaskRepository
    pub fn with_tool_mask_repository(
        mut self,
        tool_mask_repository: Arc<dyn ToolMaskRepository>,
    ) -> Self {
        self.tool_mask_repository = Some(tool_mask_repository);
        self
    }

    /// 设置最大工具调用轮数
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// 运行工具调用协调循环（无回调版本）
    ///
    /// # 参数
    /// - `session`: 会话对象
    ///
    /// # 返回
    /// - 最终文本响应
    /// - 所有中间消息（包括工具调用和结果）
    pub async fn run(&self, session: Session) -> Result<(String, Vec<Message>)> {
        self.run_with_callback(session, NoopCallback).await
    }

    /// 运行工具调用协调循环（带回调版本）
    ///
    /// # 参数
    /// - `session`: 会话对象
    /// - `callback`: 回调实现，用于接收事件通知
    ///
    /// # 返回
    /// - 最终文本响应
    /// - 所有中间消息（包括工具调用和结果）
    pub async fn run_with_callback<C>(
        &self,
        session: Session,
        callback: C,
    ) -> Result<(String, Vec<Message>)>
    where
        C: ToolCoordinatorCallback,
    {
        info!(
            "Starting tool coordinator, message_count={}",
            session.messages.len()
        );

        let mut intermediate_messages = Vec::new();
        let mut iteration = 0;

        while iteration < self.max_iterations {
            iteration += 1;
            debug!("Tool coordinator iteration {}", iteration);

            // 1. 获取可用工具
            let tools = self.get_available_tools().await;
            debug!("Available tools: {}", tools.len());

            // 2. 转换消息为 LLM 格式
            let chat_messages: Vec<ChatMessage> = session
                .messages
                .iter()
                .chain(intermediate_messages.iter())
                .map(ChatMessage::from_message)
                .collect();

            // 3. 转换工具为 LLM 格式
            let chat_tools: Vec<ChatTool> = tools
                .iter()
                .map(|(_, tool)| ChatMessage::tool_to_chat_tool(tool))
                .collect();

            // 4. 调用 LLM
            let llm_response = self
                .llm_provider
                .chat_with_tools(chat_messages, chat_tools)
                .await?;

            // 5. 处理响应
            // 如果有工具调用
            if !llm_response.tool_calls.is_empty() {
                info!("LLM returned {} tool calls", llm_response.tool_calls.len());

                // 方案：只创建 ToolCall 消息，文本放在 ToolCall 消息的 content 中
                // 这样避免了消息重复，也保持了数据完整性
                let mut tool_call_message =
                    self.create_tool_call_message(&session.messages, &llm_response.tool_calls);

                // 如果有文本，添加到 ToolCall 消息中并触发回调
                if let Some(text) = &llm_response.text
                    && !text.is_empty()
                {
                    tool_call_message.content = text.clone();
                    callback.on_assistant_message(text).await;
                }

                intermediate_messages.push(tool_call_message);

                // 执行工具调用
                for tool_call in llm_response.tool_calls {
                    // 触发工具调用回调
                    callback
                        .on_tool_call(&tool_call.name, &tool_call.arguments)
                        .await;

                    let result = self.execute_tool(tool_call.clone(), &session).await?;

                    // 触发工具结果回调
                    callback
                        .on_tool_result(&result.text_content, result.is_error)
                        .await;

                    // 创建工具结果消息
                    let tool_result_message =
                        self.create_tool_result_message(&session.messages, &tool_call, &result);
                    intermediate_messages.push(tool_result_message);
                }
            } else {
                // 没有工具调用，结束循环
                info!(
                    "LLM returned only text response, ending after {} iterations",
                    iteration
                );
                let final_text = llm_response
                    .text
                    .ok_or_else(|| Error::Llm("LLM returned empty response".into()))?;

                // 添加最终的文本消息
                let assistant_message =
                    self.create_assistant_message(&session.messages, final_text.clone());
                intermediate_messages.push(assistant_message);

                // 触发助手消息回调和完成回调
                callback.on_assistant_message(&final_text).await;
                callback.on_completed("").await;

                return Ok((final_text, intermediate_messages));
            }
        }

        warn!("Max iterations ({}) reached", self.max_iterations);
        let error_msg = format!("Max tool call iterations ({}) reached", self.max_iterations);
        callback.on_error(&error_msg).await;
        Err(Error::Mcp(error_msg))
    }

    /// 获取所有可用工具列表（不过滤）
    async fn get_all_available_tools(&self) -> Vec<(String, Tool)> {
        let mut tools = Vec::new();

        // 添加 MCP 工具
        let manager = self.mcp_server_manager.lock().await;
        tools.extend(manager.all_tools());

        // 添加本地工具
        let local_tools = self.local_tool_registry.list_tools();
        for tool in local_tools {
            tools.push((tool.name.clone(), tool));
        }

        tools
    }

    /// 获取 Agent 可见的工具列表（过滤后的）
    ///
    /// 如果没有配置 tool_mask_repository，则返回所有工具。
    /// 如果 Agent 没有 ToolMask 配置，则：
    /// - MCP 工具默认不可用
    /// - 本地工具默认可见（只读）
    pub async fn get_available_tools_for_agent(
        &self,
        agent_id: AgentId,
    ) -> Result<Vec<(String, Tool)>> {
        // 如果没有配置 tool_mask_repository，返回所有工具
        let Some(repo) = &self.tool_mask_repository else {
            return Ok(self.get_all_available_tools().await);
        };

        // 获取 Agent 的 ToolMask，如果不存在则创建默认的
        let mask = if repo.exists(agent_id).await? {
            repo.get(agent_id).await?
        } else {
            ToolMask::new(agent_id)
        };

        // 获取所有工具，然后按服务器分组
        let manager = self.mcp_server_manager.lock().await;
        let all_mcp_tools = manager.all_tools();

        // 按服务器名称分组 MCP 工具
        let mut mcp_tools_by_server: std::collections::HashMap<String, Vec<(String, Tool)>> =
            std::collections::HashMap::new();
        for (server_name, tool) in all_mcp_tools {
            mcp_tools_by_server
                .entry(server_name.clone())
                .or_insert_with(Vec::new)
                .push((server_name, tool));
        }
        drop(manager);

        let mut tools = Vec::new();

        // 过滤 MCP 工具
        for (server_name, server_tools) in mcp_tools_by_server {
            let filtered = mask.filter_mcp_tools(&server_name, server_tools);
            tools.extend(filtered);
        }

        // 添加本地工具（本地工具总是可见）
        let local_tools = self.local_tool_registry.list_tools();
        let local_tools_with_names: Vec<_> = local_tools
            .into_iter()
            .map(|t| (t.name.clone(), t))
            .collect();
        let filtered_local = mask.filter_local_tools(local_tools_with_names);
        tools.extend(filtered_local);

        Ok(tools)
    }

    /// 获取可用工具列表（不过滤，保持向后兼容）
    async fn get_available_tools(&self) -> Vec<(String, Tool)> {
        self.get_all_available_tools().await
    }

    /// 执行单个工具调用
    async fn execute_tool(
        &self,
        tool_call: ToolCall,
        session: &Session,
    ) -> Result<ExecutionResult> {
        info!(tool_name = %tool_call.name, "Executing tool");

        // 首先检查是否是本地工具
        if self.local_tool_registry.has_tool(&tool_call.name) {
            debug!(tool_name = %tool_call.name, "Executing as local tool");

            // 创建工具上下文
            let mut context = ToolContext::new(session.clone(), Arc::clone(&self.config));

            // 添加 CheckpointManager 如果可用
            if let Some(cm) = &self.checkpoint_manager {
                context = context.with_checkpoint_manager(cm.clone());
            }

            // 执行本地工具
            let result = self
                .local_tool_registry
                .call_tool(&tool_call.name, tool_call.arguments.clone(), context)
                .await;

            // 转换为 ExecutionResult
            match result {
                Ok(value) => {
                    let text_content = serde_json::to_string(&value).unwrap_or_default();
                    Ok(ExecutionResult {
                        tool_name: tool_call.name,
                        is_error: false,
                        text_content,
                        raw_content: vec![],
                    })
                }
                Err(e) => Ok(ExecutionResult {
                    tool_name: tool_call.name,
                    is_error: true,
                    text_content: e.to_string(),
                    raw_content: vec![],
                }),
            }
        } else {
            // 否则作为 MCP 工具执行
            debug!(tool_name = %tool_call.name, "Executing as MCP tool");

            let mut manager = self.mcp_server_manager.lock().await;
            self.tool_executor
                .execute(&mut manager, &tool_call.name, tool_call.arguments.clone())
                .await
        }
    }

    /// 创建助手文本消息
    fn create_assistant_message(&self, original_messages: &[Message], content: String) -> Message {
        let session_id = original_messages
            .first()
            .map(|m| m.session_id)
            .unwrap_or_else(uuid::Uuid::new_v4);

        Message::new(session_id, MessageRole::Assistant, content)
    }

    /// 创建工具调用消息
    fn create_tool_call_message(
        &self,
        original_messages: &[Message],
        tool_calls: &[ToolCall],
    ) -> Message {
        let session_id = original_messages
            .first()
            .map(|m| m.session_id)
            .unwrap_or_else(uuid::Uuid::new_v4);

        // ToolCall 消息的 content 可以为空，
        // 因为主要信息在 tool_calls 字段中
        Message::new(session_id, MessageRole::ToolCall, "".to_string())
            .with_tool_calls(tool_calls.to_vec())
    }

    /// 创建工具结果消息
    fn create_tool_result_message(
        &self,
        original_messages: &[Message],
        tool_call: &ToolCall,
        result: &ExecutionResult,
    ) -> Message {
        let session_id = original_messages
            .first()
            .map(|m| m.session_id)
            .unwrap_or_else(uuid::Uuid::new_v4);

        let tool_result = ToolResult {
            tool_call_id: tool_call.id.clone(),
            content: result.text_content.clone(),
            is_error: result.is_error,
        };

        // 注意：ToolResult 消息的 content 字段需要设置为工具结果内容
        Message::new(
            session_id,
            MessageRole::ToolResult,
            result.text_content.clone(),
        )
        .with_tool_result(tool_result)
    }
}
