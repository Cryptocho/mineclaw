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
use crate::llm::{ChatMessage, LlmProviderRegistry};
use crate::tool_mask::types::ToolMask;
use crate::tools::orchestration::OrchestrationInterface;
use std::sync::Arc;
use tracing::{debug, error, info};

/// Agent 执行器
///
/// 负责创建 Agent、执行任务和发送工单
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

        let mut mcp_tools_by_server: std::collections::HashMap<String, Vec<(String, crate::models::Tool)>> =
            std::collections::HashMap::new();
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

        // 验证任务是否分配给了正确的 Agent
        if task.agent_id != agent.id {
            return Err(Error::AgentInvalidConfig(format!(
                "Task is for agent {}, but current agent is {}",
                task.agent_id, agent.id
            )));
        }

        // 检查 Agent 是否可以接受任务
        if !agent.can_accept_task() {
            return Err(Error::AgentExecution(format!(
                "Agent {} is not available (state: {})",
                agent.id, agent.state
            )));
        }

        // 更新 Agent 状态为 Busy
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

        // 使用 finally 模式确保状态回退
        let execution_result: Result<()> = async {
            // 获取 LLM 提供者
            let model_profile = &agent.llm_config.model_profile;
            let provider = self.provider_registry.get_provider(model_profile)?;

            // 获取可用工具（根据 Agent 的 ToolMask 过滤）
            let available_tools = self.get_available_tools_for_agent(agent).await?;
            let chat_tools: Vec<crate::llm::ChatTool> = available_tools
                .iter()
                .map(|(_, tool)| ChatMessage::tool_to_chat_tool(tool))
                .collect();

            // 构建消息列表
            let messages = vec![
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

            // 调用 LLM
            info!(
                agent_id = %agent.id,
                model_profile = %model_profile,
                message_count = %messages.len(),
                "Calling LLM"
            );

            let llm_response = provider.chat_with_tools(messages, chat_tools).await?;

            // 处理响应
            result.success = true;
            result.response = llm_response.text.unwrap_or_default();

            // 注意：工具调用功能暂未完全实现，需要更复杂的消息历史管理
            // 目前 tool_calls 保持为空

            Ok(())
        }
        .await;

        // 处理执行结果
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

        // 更新执行时间
        result.execution_time_ms = start_time.elapsed().as_millis() as u64;

        // 更新 Agent 状态回 Idle
        agent.set_state(AgentState::Idle);

        Ok(result)
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
