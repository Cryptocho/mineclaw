//! 工具调用协调器
//!
//! 负责协调 LLM 和 MCP 工具之间的交互，实现工具调用循环。

use crate::error::{Error, Result};
use crate::llm::{ChatMessage, ChatTool, LlmProvider};
use crate::mcp::{ExecutionResult, McpServerManager, ToolExecutor};
use crate::models::{Message, MessageRole, Tool, ToolCall, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
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
    llm_provider: Arc<RwLock<Arc<dyn LlmProvider>>>,
    mcp_server_manager: Arc<Mutex<McpServerManager>>,
    tool_executor: ToolExecutor,
    /// 最大工具调用轮数
    max_iterations: usize,
}

impl ToolCoordinator {
    /// 创建一个新的工具调用协调器
    pub fn new(
        llm_provider: Arc<dyn LlmProvider>,
        mcp_server_manager: Arc<Mutex<McpServerManager>>,
        tool_executor: ToolExecutor,
    ) -> Self {
        Self {
            llm_provider: Arc::new(RwLock::new(llm_provider)),
            mcp_server_manager,
            tool_executor,
            max_iterations: 10,
        }
    }

    /// 设置最大工具调用轮数
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// 运行工具调用协调循环（无回调版本）
    ///
    /// # 参数
    /// - `messages`: 初始消息列表
    ///
    /// # 返回
    /// - 最终文本响应
    /// - 所有中间消息（包括工具调用和结果）
    pub async fn run(&self, messages: Vec<Message>) -> Result<(String, Vec<Message>)> {
        self.run_with_callback(messages, NoopCallback).await
    }

    /// 运行工具调用协调循环（带回调版本）
    ///
    /// # 参数
    /// - `messages`: 初始消息列表
    /// - `callback`: 回调实现，用于接收事件通知
    ///
    /// # 返回
    /// - 最终文本响应
    /// - 所有中间消息（包括工具调用和结果）
    pub async fn run_with_callback<C>(
        &self,
        messages: Vec<Message>,
        callback: C,
    ) -> Result<(String, Vec<Message>)>
    where
        C: ToolCoordinatorCallback,
    {
        info!(
            "Starting tool coordinator, message_count={}",
            messages.len()
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
            let mut chat_messages: Vec<ChatMessage> = Vec::new();

            // 注入 System Prompt
            // 检测 OS 和 Shell 环境
            #[cfg(target_os = "windows")]
            let os_info = "Windows";
            #[cfg(not(target_os = "windows"))]
            let os_info = "Linux/macOS";
            
            // 获取 Terminal Server 的 Shell 类型
            let mut shell_type_string = String::new();
            let mut shell_info = "Unknown Shell";

            {
                let manager = self.mcp_server_manager.lock().await;
                if let Some(server) = manager.get_server("terminal-server") {
                    if let Some(st) = server.metadata.get("shell_type") {
                        shell_type_string = st.clone();
                    }
                }
            }

            match shell_type_string.as_str() {
                "powershell" => {
                    shell_info = "PowerShell (use `$env:VAR = 'val'`, `;` or `&&` for chaining)";
                }
                "cmd" => {
                    shell_info = "CMD (use `set VAR=val`, `&&` for chaining)";
                }
                "bash" | "zsh" | "sh" => {
                    shell_info = "Bash/Zsh (use `export VAR=val`, `&&` for chaining)";
                }
                _ => {
                    // Fallback detection
                    if cfg!(target_os = "windows") {
                        shell_info = "PowerShell (Default)";
                    } else {
                        shell_info = "Bash/Zsh (Default)";
                    }
                }
            }

            let system_prompt = format!(
                "You are MineClaw, an advanced AI coding assistant.\n\
                 Current Environment:\n\
                 - OS: {}\n\
                 - Shell: {}\n\
                 \n\
                 When executing terminal commands, ensure you use the correct syntax for the current shell.\n\
                 For PowerShell, use `$env:VAR = 'val'` to set variables, and `;` or `&&` (if PS 7+) to chain commands.\n\
                 For file paths, prefer forward slashes `/` as they work in both, or use `join-path` in PS.",
                os_info, shell_info
            );

            chat_messages.push(ChatMessage {
                role: "system".to_string(),
                content: Some(system_prompt),
                tool_calls: None,
                tool_call_id: None,
            });

            // 添加历史消息
            chat_messages.extend(
                messages
                    .iter()
                    .chain(intermediate_messages.iter())
                    .map(ChatMessage::from_message)
            );

            // 3. 转换工具为 LLM 格式
            let chat_tools: Vec<ChatTool> = tools
                .iter()
                .map(|(_, tool)| ChatMessage::tool_to_chat_tool(tool))
                .collect();

            // 4. 调用 LLM
            let llm_provider = self.llm_provider.read().await;
            let llm_response = llm_provider
                .chat_with_tools(chat_messages, chat_tools)
                .await?;

            // 5. 处理响应
            // 如果有工具调用
            if !llm_response.tool_calls.is_empty() {
                info!("LLM returned {} tool calls", llm_response.tool_calls.len());

                // 方案：只创建 ToolCall 消息，文本放在 ToolCall 消息的 content 中
                // 这样避免了消息重复，也保持了数据完整性
                let mut tool_call_message =
                    self.create_tool_call_message(&messages, &llm_response.tool_calls);

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

                    let result = self.execute_tool(tool_call.clone()).await?;

                    // 触发工具结果回调
                    callback
                        .on_tool_result(&result.text_content, result.is_error)
                        .await;

                    // 创建工具结果消息
                    let tool_result_message =
                        self.create_tool_result_message(&messages, &tool_call, &result);
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
                    self.create_assistant_message(&messages, final_text.clone());
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

    /// 获取可用工具列表
    async fn get_available_tools(&self) -> Vec<(String, Tool)> {
        let manager = self.mcp_server_manager.lock().await;
        manager.all_tools()
    }

    /// 执行单个工具调用
    async fn execute_tool(&self, tool_call: ToolCall) -> Result<ExecutionResult> {
        info!(tool_name = %tool_call.name, "Executing tool");

        let mut manager = self.mcp_server_manager.lock().await;
        self.tool_executor
            .execute(&mut manager, &tool_call.name, tool_call.arguments.clone())
            .await
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

    /// 更新 LLM Provider
    pub async fn update_llm_provider(&self, provider: Arc<dyn LlmProvider>) {
        let mut llm_provider = self.llm_provider.write().await;
        *llm_provider = provider;
    }
}
