//! Agent 模块 - 多 Agent 系统的核心实现
//!
//! 提供 Agent 的定义、创建、执行任务和发送工单等功能。

pub mod builder;
pub mod context;
pub mod context_manager;
pub mod types;
pub mod work_order;

pub use builder::*;
pub use context::*;
pub use context_manager::*;
pub use types::*;
pub use work_order::*;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::llm::{ChatMessage, ChatTool, LlmProviderRegistry};
use crate::models::{SessionRepository, ToolCall};
use crate::tool_mask::types::ToolMask;
use crate::tools::orchestration::OrchestrationInterface;
use crate::tools::ToolContext;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{debug, error, info};
use crate::mcp::ExecutionResult;

/// Agent 执行器
///
/// 负责创建 Agent、执行任务和发送工单
#[derive(Clone)]
pub struct AgentExecutor {
    /// LLM 提供者注册表
    pub provider_registry: Arc<LlmProviderRegistry>,
    /// MCP 服务器管理器
    pub mcp_server_manager: Arc<tokio::sync::Mutex<crate::mcp::McpServerManager>>,
    /// 工具执行器
    pub tool_executor: crate::mcp::ToolExecutor,
    /// 本地工具注册表
    pub local_tool_registry: Arc<crate::tools::LocalToolRegistry>,
    /// 应用配置
    pub config: Arc<Config>,
    /// Session 仓库（用于工具执行时获取真实 session）
    pub session_repo: Option<Arc<SessionRepository>>,
}

impl AgentExecutor {
    /// 创建新的 AgentExecutor
    pub fn new(
        provider_registry: Arc<LlmProviderRegistry>,
        mcp_server_manager: Arc<tokio::sync::Mutex<crate::mcp::McpServerManager>>,
        tool_executor: crate::mcp::ToolExecutor,
        local_tool_registry: Arc<crate::tools::LocalToolRegistry>,
        config: Arc<Config>,
    ) -> Self {
        Self {
            provider_registry,
            mcp_server_manager,
            tool_executor,
            local_tool_registry,
            config,
            session_repo: None,
        }
    }

    /// 使用 SessionRepository 创建 AgentExecutor
    pub fn with_session_repo(
        provider_registry: Arc<LlmProviderRegistry>,
        mcp_server_manager: Arc<tokio::sync::Mutex<crate::mcp::McpServerManager>>,
        tool_executor: crate::mcp::ToolExecutor,
        local_tool_registry: Arc<crate::tools::LocalToolRegistry>,
        config: Arc<Config>,
        session_repo: Arc<SessionRepository>,
    ) -> Self {
        Self {
            provider_registry,
            mcp_server_manager,
            tool_executor,
            local_tool_registry,
            config,
            session_repo: Some(session_repo),
        }
    }

    /// 获取 Agent 可用的工具列表
    ///
    /// 根据 Agent 嵌入的 ToolMask 进行过滤。
    pub async fn get_available_tools_for_agent(
        &self,
        agent: &Agent,
    ) -> Result<Vec<(String, crate::models::Tool)>> {
        let mask = agent.tool_mask.clone().unwrap_or_else(ToolMask::readonly);

        let manager = self.mcp_server_manager.lock().await;
        let all_mcp_tools = manager.all_tools();

        let mut mcp_tools_by_server: std::collections::HashMap<
            String,
            Vec<(String, crate::models::Tool)>,
        > = std::collections::HashMap::new();
        for (server_name, tool) in all_mcp_tools {
            mcp_tools_by_server
                .entry(server_name.clone())
                .or_default()
                .push((tool.name.clone(), tool));
        }
        drop(manager);

        let mut tools = Vec::new();

        for (server_name, server_tools) in mcp_tools_by_server {
            let filtered = mask.filter_tools(Some(&server_name), server_tools);
            tools.extend(filtered);
        }

        let local_tools = self.local_tool_registry.list_tools();
        let local_tools_with_names: Vec<_> = local_tools
            .into_iter()
            .map(|t| (t.name.clone(), t))
            .collect();

        tools.extend(mask.filter_tools(None, local_tools_with_names));

        Ok(tools)
    }

    /// 创建一个新的 Agent
    ///
    /// # 参数
    /// * `config` - Agent 配置
    ///
    /// # 返回
    /// 返回创建的 Agent 或错误
    pub fn create_agent(&self, config: AgentConfig) -> Result<Agent> {
        debug!(name = %config.name, role = ?config.role, "Creating new agent");

        // 验证配置
        config.validate()?;

        let agent = Agent::new(config);

        info!(agent_id = %agent.id, name = %agent.name, "Agent created successfully");

        Ok(agent)
    }

    /// 执行任务
    ///
    /// # 参数
    /// * `agent` - 要执行任务 of the Agent
    /// * `task` - 任务信息
    /// * `orchestrator` - 总控接口（可选，用于协作工具）
    ///
    /// # 返回
    /// 返回任务执行结果或错误
    pub async fn execute_task(
        &self,
        agent: &mut Agent,
        task: AgentTask,
        _orchestrator: Option<Arc<dyn OrchestrationInterface>>,
    ) -> Result<AgentTaskResult> {
        debug!(agent_id = %agent.id, session_id = %task.session_id, "Executing task");

        if task.agent_id != agent.id {
            return Err(Error::AgentInvalidConfig(format!(
                "Task is for agent {}, but current agent is {}",
                task.agent_id, agent.id
            )));
        }

        if !agent.can_accept_task() {
            return Err(Error::AgentExecution(format!(
                "Agent {} is not available (state: {})",
                agent.id, agent.state
            )));
        }

        agent.set_state(AgentState::Busy);

        let start_time = std::time::Instant::now();
        let mut result = AgentTaskResult {
            success: false,
            agent_id: agent.id,
            session_id: task.session_id,
            response: String::new(),
            tool_calls: Vec::new(),
            error: None,
            execution_time_ms: 0,
            new_checkpoint_id: None,
        };

        let execution_result: Result<()> = async {
            let model_profile = &agent.llm_config.model_profile;
            let provider = self.provider_registry.get_provider(model_profile)?;

            let available_tools = self.get_available_tools_for_agent(agent).await?;
            let chat_tools: Vec<ChatTool> = available_tools
                .iter()
                .map(|(_, tool)| ChatMessage::tool_to_chat_tool(tool))
                .collect();

            let mut messages: Vec<ChatMessage> = vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: Some(agent.system_prompt.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: Some(task.user_message.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                },
            ];

            let max_iterations = self.config.context_manager.max_tool_iterations;
            let mut iteration = 0;

            while iteration < max_iterations {
                iteration += 1;
                debug!(
                    agent_id = %agent.id,
                    iteration = %iteration,
                    message_count = %messages.len(),
                    "Calling LLM"
                );

                let llm_response = provider.chat_with_tools(messages.clone(), chat_tools.clone()).await?;

                if llm_response.tool_calls.is_empty() {
                    result.response = llm_response.text.unwrap_or_default();
                    result.success = true;
                    return Ok(());
                }

                info!(
                    agent_id = %agent.id,
                    tool_call_count = %llm_response.tool_calls.len(),
                    "LLM returned tool calls"
                );

                let current_text = llm_response.text.clone();
                let tool_calls = llm_response.tool_calls;
                let this = self.clone();

                let mut join_set = JoinSet::new();

                for tool_call in tool_calls.clone() {
                    let this = this.clone();
                    join_set.spawn(async move {
                        let exec_result = this.execute_tool(&tool_call, task.session_id).await;
                        (tool_call, exec_result)
                    });
                }

                let mut tool_results = Vec::new();
                while let Some(res) = join_set.join_next().await {
                    if let Ok((tool_call, result)) = res {
                        tool_results.push((tool_call, result));
                    }
                }

                tool_results.sort_by_key(|(tc, _)| tc.id.clone());

                for (tool_call, exec_result) in tool_results {
                    let (tool_result_content, is_error) = match &exec_result {
                        Ok(r) => (r.text_content.clone(), r.is_error),
                        Err(e) => (e.to_string(), true),
                    };

                    let tool_result = ChatMessage {
                        role: "tool".to_string(),
                        content: Some(tool_result_content.clone()),
                        tool_calls: None,
                        tool_call_id: Some(tool_call.id.clone()),
                    };
                    messages.push(tool_result);

                    let record = crate::agent::types::ToolCallRecord {
                        tool_name: tool_call.name.clone(),
                        arguments: tool_call.arguments.clone(),
                        result: Some(serde_json::json!({ "content": tool_result_content })),
                        success: !is_error,
                        error: if is_error { exec_result.err().map(|e| e.to_string()) } else { None },
                        execution_time_ms: 0,
                    };
                    result.tool_calls.push(record);
                }

                if let Some(text) = current_text
                    && !text.is_empty() {
                        let assistant_msg = ChatMessage {
                            role: "assistant".to_string(),
                            content: Some(text),
                            tool_calls: Some(tool_calls.iter().map(|tc| {
                                crate::llm::ChatToolCall {
                                    id: tc.id.clone(),
                                    r#type: "function".to_string(),
                                    function: crate::llm::ChatToolCallFunction {
                                        name: tc.name.clone(),
                                        arguments: tc.arguments.to_string(),
                                    },
                                }
                            }).collect()),
                            tool_call_id: None,
                        };
                        messages.push(assistant_msg);
                    }
            }

            Err(Error::MaxToolIterations {
                message: format!(
                    "Max tool iterations ({}) reached after {} tool calls. Context may need trimming.",
                    max_iterations,
                    result.tool_calls.len()
                ),
                tool_call_count: result.tool_calls.len(),
            })
        }
        .await;

        match execution_result {
            Ok(_) => {
                info!(
                    agent_id = %agent.id,
                    success = %result.success,
                    "Task execution completed"
                );
            }
            Err(e) => {
                error!(
                    agent_id = %agent.id,
                    error = %e,
                    "Task execution failed"
                );
                result.success = false;
                result.error = Some(e.to_string());
            }
        }

        result.execution_time_ms = start_time.elapsed().as_millis() as u64;

        agent.set_state(AgentState::Idle);

        Ok(result)
    }

    /// 执行单个工具调用
    async fn execute_tool(&self, tool_call: &ToolCall, session_id: uuid::Uuid) -> Result<ExecutionResult> {
        if self.local_tool_registry.has_tool(&tool_call.name) {
            debug!(tool_name = %tool_call.name, "Executing as local tool");

            let session = if let Some(ref repo) = self.session_repo {
                repo.get(&session_id).await.unwrap_or_else(|| {
                    crate::models::Session {
                        id: session_id,
                        title: None,
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                        state: crate::models::SessionState::Active,
                        orchestrator_id: None,
                        current_checkpoint_id: None,
                        archived_at: None,
                        messages: Vec::new(),
                        metadata: std::collections::HashMap::new(),
                        lifecycle_events: Vec::new(),
                        agent_checkpoints: std::collections::HashMap::new(),
                    }
                })
            } else {
                crate::models::Session {
                    id: session_id,
                    title: None,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                    state: crate::models::SessionState::Active,
                    orchestrator_id: None,
                    current_checkpoint_id: None,
                    archived_at: None,
                    messages: Vec::new(),
                    metadata: std::collections::HashMap::new(),
                    lifecycle_events: Vec::new(),
                    agent_checkpoints: std::collections::HashMap::new(),
                }
            };

            let context = ToolContext::new(session, Arc::clone(&self.config));

            let result = self
                .local_tool_registry
                .call_tool(&tool_call.name, tool_call.arguments.clone(), context)
                .await;

            match result {
                Ok(value) => {
                    let text_content = serde_json::to_string(&value).unwrap_or_default();
                    Ok(ExecutionResult {
                        tool_name: tool_call.name.clone(),
                        is_error: false,
                        text_content,
                        raw_content: vec![],
                    })
                }
                Err(e) => Ok(ExecutionResult {
                    tool_name: tool_call.name.clone(),
                    is_error: true,
                    text_content: e.to_string(),
                    raw_content: vec![],
                }),
            }
        } else {
            debug!(tool_name = %tool_call.name, "Executing as MCP tool");

            let mut manager = self.mcp_server_manager.lock().await;
            self.tool_executor
                .execute(&mut manager, &tool_call.name, tool_call.arguments.clone())
                .await
        }
    }

    /// 发送工单
    ///
    /// # 参数
    /// * `agent` - 发送工单的 Agent
    /// * `work_order` - 工单信息
    ///
    /// # 返回
    /// 成功返回 Ok(()), 失败返回错误
    pub fn send_work_order(agent: &mut Agent, work_order: WorkOrder) -> Result<()> {
        debug!(
            agent_id = %agent.id,
            work_order_id = %work_order.id(),
            work_order_type = ?work_order.work_order_type,
            recipient = ?work_order.recipient,
            "Sending work order"
        );

        // 这里是实际发送工单的逻辑
        // 目前是占位实现，后续会集成工单路由机制
        info!(
            agent_id = %agent.id,
            work_order_id = %work_order.id(),
            work_order_type = %work_order.work_order_type,
            recipient = %work_order.recipient,
            "Work order sent successfully"
        );

        // 发送工单后，Agent 进入等待审查状态
        agent.set_state(AgentState::WaitingForReview);

        Ok(())
    }
}
