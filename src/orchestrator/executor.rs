//! Orchestrator 执行器
//!
//! 提供总控的创建、Agent 管理、任务分配和工单处理等核心功能。

use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::agent::work_order::{WorkOrder, WorkOrderRecipient, WorkOrderType};
use crate::agent::{
    Agent, AgentConfig, AgentExecutor, AgentId, AgentRole, AgentState, AgentTask, AgentTaskResult,
};
use crate::error::{Error, Result};
use crate::mcp::{McpServerManager, ToolExecutor};
use crate::models::SessionRepository;
use crate::tools::LocalToolRegistry;

use super::task_manager::SharedTaskManager;
use super::types::{
    Orchestrator, OrchestratorConfig, ParallelTasks, TaskId,
    TaskStatus,
};

use super::prompt_template::PromptAssembler;
use crate::config::Config;
use crate::llm::LlmProviderRegistry;
use crate::tools::orchestration::OrchestrationInterface;
use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::sync::{Mutex, RwLock};

/// 总控执行器
///
/// 负责创建总控、管理 Agent、分配任务和处理工单等功能。
pub struct OrchestratorExecutor {
    /// LLM 提供者注册表
    pub provider_registry: Arc<LlmProviderRegistry>,
    /// MCP 服务器管理器
    pub mcp_server_manager: Arc<Mutex<McpServerManager>>,
    /// 工具执行器
    pub tool_executor: ToolExecutor,
    /// 本地工具注册表
    pub local_tool_registry: Arc<LocalToolRegistry>,
    /// 应用配置
    pub config: Arc<Config>,
    /// Session 仓库（用于工具执行时获取真实 session）
    pub session_repo: Option<Arc<SessionRepository>>,
}

impl OrchestratorExecutor {
    /// 创建新的 OrchestratorExecutor
    pub fn new(
        provider_registry: Arc<LlmProviderRegistry>,
        mcp_server_manager: Arc<Mutex<McpServerManager>>,
        tool_executor: ToolExecutor,
        local_tool_registry: Arc<LocalToolRegistry>,
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

    /// 使用 SessionRepository 创建 OrchestratorExecutor
    pub fn with_session_repo(
        provider_registry: Arc<LlmProviderRegistry>,
        mcp_server_manager: Arc<Mutex<McpServerManager>>,
        tool_executor: ToolExecutor,
        local_tool_registry: Arc<LocalToolRegistry>,
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
}

impl OrchestratorExecutor {
    /// 创建新的总控
    ///
    /// # 参数
    /// * `config` - 总控配置
    ///
    /// # 返回
    /// 返回创建的总控或错误
    pub fn create_orchestrator(&self, mut config: OrchestratorConfig) -> Result<Orchestrator> {
        debug!(
            name = %config.name,
            role = ?config.role,
            nested_depth = %config.nested_depth,
            "Creating new orchestrator"
        );

        // 验证配置
        config.validate()?;

        // 使用 PromptAssembler 增强系统提示词，注入可用的模型信息
        config.agent_config.system_prompt = PromptAssembler::build_orchestrator_prompt(
            &config.agent_config.system_prompt,
            &self.provider_registry,
            config.nested_depth,
            self.config.orchestrator.max_nested_depth,
        );

        // 创建 AgentExecutor 实例
        let agent_executor = if let Some(ref repo) = self.session_repo {
            AgentExecutor::with_session_repo(
                self.provider_registry.clone(),
                self.mcp_server_manager.clone(),
                self.tool_executor.clone(),
                self.local_tool_registry.clone(),
                self.config.clone(),
                repo.clone(),
            )
        } else {
            AgentExecutor::new(
                self.provider_registry.clone(),
                self.mcp_server_manager.clone(),
                self.tool_executor.clone(),
                self.local_tool_registry.clone(),
                self.config.clone(),
            )
        };

        // 创建总控自身的 Agent
        let agent = agent_executor.create_agent(config.agent_config.clone())?;

        // 创建总控
        let orchestrator = Orchestrator::new(config, agent);

        info!(
            orchestrator_id = %orchestrator.id,
            name = %orchestrator.name,
            role = ?orchestrator.role,
            "Orchestrator created successfully"
        );

        Ok(orchestrator)
    }

    /// 总控创建 Agent
    ///
    /// # 参数
    /// * `orchestrator` - 总控实例
    /// * `agent_config` - Agent 配置
    ///
    /// # 返回
    /// 返回更新后的总控和新创建的 Agent，或错误
    pub fn create_agent(
        &self,
        mut orchestrator: Orchestrator,
        mut agent_config: AgentConfig,
    ) -> Result<(Orchestrator, Agent)> {
        debug!(
            orchestrator_id = %orchestrator.id,
            agent_name = %agent_config.name,
            agent_role = ?agent_config.role,
            "Creating new agent via orchestrator"
        );

        // 如果创建的是子总控，自动设置 nested_depth 并检查限制
        if matches!(
            agent_config.role,
            AgentRole::MasterOrchestrator | AgentRole::SubOrchestrator
        ) {
            let new_depth = orchestrator.nested_depth + 1;
            if new_depth > self.config.orchestrator.max_nested_depth {
                return Err(Error::InvalidConfig(format!(
                    "Cannot create nested orchestrator: depth {} exceeds max_nested_depth {}",
                    new_depth, self.config.orchestrator.max_nested_depth
                )));
            }
            agent_config = agent_config.with_nested_depth(new_depth);
            agent_config = agent_config
                .with_parent_orchestrator(AgentId::from_uuid(*orchestrator.id.as_uuid()));
        }

        // 创建 AgentExecutor 实例
        let agent_executor = if let Some(ref repo) = self.session_repo {
            AgentExecutor::with_session_repo(
                self.provider_registry.clone(),
                self.mcp_server_manager.clone(),
                self.tool_executor.clone(),
                self.local_tool_registry.clone(),
                self.config.clone(),
                repo.clone(),
            )
        } else {
            AgentExecutor::new(
                self.provider_registry.clone(),
                self.mcp_server_manager.clone(),
                self.tool_executor.clone(),
                self.local_tool_registry.clone(),
                self.config.clone(),
            )
        };

        // 创建 Agent
        let agent = agent_executor.create_agent(agent_config)?;

        // 添加到总控的管理列表
        orchestrator.add_agent(agent.clone());

        info!(
            orchestrator_id = %orchestrator.id,
            agent_id = %agent.id,
            agent_name = %agent.name,
            "Agent created and added to orchestrator"
        );

        Ok((orchestrator, agent))
    }

    /// 总控获取 Agent
    ///
    /// # 参数
    /// * `orchestrator` - 总控实例
    /// * `agent_id` - Agent ID
    ///
    /// # 返回
    /// 返回 Agent 引用或 None
    pub fn get_agent<'a>(orchestrator: &'a Orchestrator, agent_id: &AgentId) -> Option<&'a Agent> {
        orchestrator.get_agent(agent_id)
    }

    /// 总控列出所有 Agent
    ///
    /// # 参数
    /// * `orchestrator` - 总控实例
    ///
    /// # 返回
    /// 返回所有管理的 Agent 引用列表
    pub fn list_agents(orchestrator: &Orchestrator) -> Vec<&Agent> {
        orchestrator.list_agents()
    }

    /// 总控移除 Agent
    ///
    /// # 参数
    /// * `orchestrator` - 总控实例
    /// * `agent_id` - 要移除的 Agent ID
    ///
    /// # 返回
    /// 返回更新后的总控或错误
    pub fn remove_agent(
        mut orchestrator: Orchestrator,
        agent_id: &AgentId,
    ) -> Result<Orchestrator> {
        debug!(
            orchestrator_id = %orchestrator.id,
            agent_id = %agent_id,
            "Removing agent from orchestrator"
        );

        // 检查 Agent 是否存在并且不在 Busy 状态
        if let Some(agent) = orchestrator.get_agent(agent_id) {
            if agent.state == AgentState::Busy {
                return Err(Error::AgentExecution(format!(
                    "Cannot remove busy agent {}",
                    agent_id
                )));
            }
        } else {
            return Err(Error::AgentNotFound(agent_id.to_string()));
        }

        // 移除 Agent
        orchestrator.remove_agent(agent_id);

        info!(
            orchestrator_id = %orchestrator.id,
            agent_id = %agent_id,
            "Agent removed from orchestrator"
        );

        Ok(orchestrator)
    }

    /// 串行分配任务
    ///
    /// # 参数
    /// * `orchestrator` - 总控实例（可变引用）
    /// * `agent_id` - 目标 Agent ID
    /// * `task` - 任务内容
    ///
    /// # 返回
    /// 返回任务执行结果或错误
    pub async fn assign_task_serial(
        &self,
        orchestrator: &mut Orchestrator,
        agent_id: &AgentId,
        task: AgentTask,
        provider: Option<Arc<dyn OrchestrationInterface>>,
    ) -> Result<AgentTaskResult> {
        debug!(
            orchestrator_id = %orchestrator.id,
            agent_id = %agent_id,
            "Assigning task serially"
        );

        // 获取 Agent 的可变引用
        let agent = orchestrator
            .get_agent_mut(agent_id)
            .ok_or_else(|| Error::AgentNotFound(agent_id.to_string()))?;

        // 创建 AgentExecutor 实例
        let agent_executor = if let Some(ref repo) = self.session_repo {
            AgentExecutor::with_session_repo(
                self.provider_registry.clone(),
                self.mcp_server_manager.clone(),
                self.tool_executor.clone(),
                self.local_tool_registry.clone(),
                self.config.clone(),
                repo.clone(),
            )
        } else {
            AgentExecutor::new(
                self.provider_registry.clone(),
                self.mcp_server_manager.clone(),
                self.tool_executor.clone(),
                self.local_tool_registry.clone(),
                self.config.clone(),
            )
        };

        // 执行任务
        let result = agent_executor.execute_task(agent, task, provider).await;

        if let Err(Error::MaxToolIterations { message: _, tool_call_count }) = &result {
            warn!(
                orchestrator_id = %orchestrator.id,
                agent_id = %agent_id,
                tool_call_count = %tool_call_count,
                max_iterations = %self.config.context_manager.max_tool_iterations,
                "Max tool iterations reached - Agent may need self-correction or human intervention"
            );
        }

        let result = result.map_err(|e| {
            if matches!(e, Error::MaxToolIterations { .. }) {
                error!(
                    orchestrator_id = %orchestrator.id,
                    agent_id = %agent_id,
                    "Max tool iterations error propagated to caller for handling"
                );
            }
            e
        })?;

        info!(
            orchestrator_id = %orchestrator.id,
            agent_id = %agent_id,
            success = %result.success,
            "Serial task execution completed"
        );

        Ok(result)
    }

    /// 并行分配任务
    ///
    /// # 参数
    /// * `orchestrator` - 总控实例
    /// * `parallel_tasks` - 并行任务配置
    /// * `task_manager` - 任务管理器（可选）
    ///
    /// # 返回
    /// 返回任务 ID
    pub async fn assign_task_parallel(
        orchestrator: &Orchestrator,
        parallel_tasks: ParallelTasks,
        task_manager: Option<&SharedTaskManager>,
        _provider: Option<Arc<dyn OrchestrationInterface>>,
    ) -> Result<TaskId> {
        debug!(
            orchestrator_id = %orchestrator.id,
            task_id = %parallel_tasks.task_id,
            assignment_count = %parallel_tasks.assignments.len(),
            "Assigning tasks in parallel"
        );

        let main_task_id = parallel_tasks.task_id;

        // 如果有 TaskManager，为每个子任务注册
        if let Some(tm) = task_manager {
            let mut tm_guard = tm.lock().await;

            for assignment in &parallel_tasks.assignments {
                tm_guard.register_task(assignment.task_id, assignment.agent_id)?;
                tm_guard.update_task_status(&assignment.task_id, TaskStatus::Running)?;
            }
        }

        info!(
            orchestrator_id = %orchestrator.id,
            task_id = %main_task_id,
            "Parallel tasks assigned"
        );

        Ok(main_task_id)
    }

    /// 查询任务状态
    ///
    /// # 参数
    /// * `orchestrator` - 总控实例
    /// * `task_id` - 任务 ID
    /// * `task_manager` - 任务管理器（可选）
    ///
    /// # 返回
    /// 返回任务状态或 None
    pub async fn get_task_status(
        _orchestrator: &Orchestrator,
        task_id: &TaskId,
        task_manager: Option<&SharedTaskManager>,
    ) -> Option<TaskStatus> {
        if let Some(tm) = task_manager {
            let tm_guard = tm.lock().await;
            tm_guard.get_task_status(task_id)
        } else {
            // 占位实现
            Some(TaskStatus::Completed)
        }
    }

    /// 等待任务完成
    ///
    /// # 参数
    /// * `task_id` - 任务 ID
    /// * `task_manager` - 任务管理器
    ///
    /// # 返回
    /// 返回任务结果或错误
    pub async fn wait_for_task(
        task_id: &TaskId,
        task_manager: &SharedTaskManager,
    ) -> Result<AgentTaskResult> {
        let mut tm_guard = task_manager.lock().await;
        tm_guard.wait_for_task(task_id).await
    }

    /// 等待所有任务完成
    ///
    /// # 参数
    /// * `task_manager` - 任务管理器
    ///
    /// # 返回
    /// 返回所有任务的结果
    pub async fn wait_for_all_tasks(
        task_manager: &SharedTaskManager,
    ) -> Vec<(TaskId, Result<AgentTaskResult>)> {
        let mut tm_guard = task_manager.lock().await;
        tm_guard.wait_for_all_tasks().await
    }

    /// 生成工单
    ///
    /// # 参数
    /// * `orchestrator` - 总控实例
    /// * `work_order_type` - 工单类型
    /// * `recipient` - 接收者
    /// * `title` - 工单标题
    /// * `content` - 工单内容
    ///
    /// # 返回
    /// 返回生成的工单或错误
    pub fn generate_work_order(
        orchestrator: &Orchestrator,
        work_order_type: WorkOrderType,
        recipient: WorkOrderRecipient,
        title: String,
        content: String,
    ) -> Result<WorkOrder> {
        debug!(
            orchestrator_id = %orchestrator.id,
            work_order_type = ?work_order_type,
            recipient = ?recipient,
            title = %title,
            "Generating work order"
        );

        // 使用临时的 session_id，实际使用时应该传入真正的 session_id
        let session_id = Uuid::new_v4();
        let work_order = WorkOrder::new(work_order_type, recipient, session_id, title, content)
            .with_created_by(orchestrator.agent.id);

        info!(
            orchestrator_id = %orchestrator.id,
            work_order_id = %work_order.id(),
            "Work order generated successfully"
        );

        Ok(work_order)
    }

    /// 关联会话
    ///
    /// # 参数
    /// * `orchestrator` - 总控实例
    /// * `session_id` - 会话 ID
    ///
    /// # 返回
    /// 返回更新后的总控
    pub fn associate_session(orchestrator: Orchestrator, session_id: Uuid) -> Orchestrator {
        orchestrator.with_session_id(session_id)
    }

    /// 触发 CMA（上下文管理 Agent）处理上下文满载
    ///
    /// 当 Agent 达到最大工具调用次数时调用此方法。
    ///
    /// # 参数
    /// * `session_id` - 要处理的会话 ID
    /// * `problem_description` - 问题描述（可选）
    ///
    /// # 返回
    /// 返回 CMA 处理结果
    pub async fn spawn_cma(
        &self,
        session_id: Uuid,
        problem_description: Option<String>,
    ) -> Result<CmaResult> {
        info!(
            session_id = %session_id,
            "Spawning CMA to handle context overflow"
        );

        let session_repo = self.session_repo.as_ref().ok_or_else(|| Error::Internal)?;

        let session: crate::models::Session = session_repo.get(&session_id).await.ok_or_else(|| {
            Error::SessionNotFound(session_id.to_string())
        })?;

        let cma_system_prompt = format!(
            "You are the Context Management Agent (CMA). Your role is to analyze and optimize the conversation context.

Current task: {}
Instructions:
1. Use read_messages to examine the current conversation
2. Use trim_messages to remove less important messages while keeping context intact
3. If the context is severely overloaded, you may need to:
   - Identify the most important recent messages to keep
   - Add a system notice summarizing what was trimmed
   - Request human intervention if the context cannot be reasonably managed

Remember: Your goal is to make the conversation manageable while preserving critical information.",
            problem_description.unwrap_or_else(|| "Handle context overflow".to_string())
        );

        let mut tool_mask = crate::tool_mask::types::ToolMask::new();
        tool_mask.set_local_permission(
            "read_messages".to_string(),
            crate::tool_mask::types::FsPermission::ReadOnly,
        );
        tool_mask.set_local_permission(
            "trim_messages".to_string(),
            crate::tool_mask::types::FsPermission::ReadWrite,
        );
        tool_mask.set_local_permission(
            "request_help".to_string(),
            crate::tool_mask::types::FsPermission::ReadOnly,
        );

        let llm_config = crate::agent::LlmConfig::new("default".to_string());

        let cma_config = AgentConfig::new(
            "CMA".to_string(),
            AgentRole::ContextManager,
            llm_config,
            cma_system_prompt,
        )
        .with_tool_mask(tool_mask);

        let agent_executor = AgentExecutor::with_session_repo(
            self.provider_registry.clone(),
            self.mcp_server_manager.clone(),
            self.tool_executor.clone(),
            self.local_tool_registry.clone(),
            self.config.clone(),
            session_repo.clone(),
        );

        let mut cma_agent = agent_executor.create_agent(cma_config)?;

        let cma_task = AgentTask {
            agent_id: cma_agent.id,
            session_id,
            user_message: "Please analyze and optimize the conversation context.".to_string(),
            tools: None,
            checkpoint_id: None,
        };

        let result = agent_executor
            .execute_task(&mut cma_agent, cma_task, None)
            .await;

        match result {
            Ok(task_result) => {
                if task_result.success {
                    session_repo.update(session).await?;

                    Ok(CmaResult {
                        success: true,
                        trimmed_count: 0,
                        message: "CMA handled context successfully".to_string(),
                    })
                } else {
                    Ok(CmaResult {
                        success: false,
                        trimmed_count: 0,
                        message: task_result.error.unwrap_or_else(|| "CMA task failed".to_string()),
                    })
                }
            }
            Err(e) => {
                Ok(CmaResult {
                    success: false,
                    trimmed_count: 0,
                    message: format!("CMA execution error: {}", e),
                })
            }
        }
    }
}

// ==================== CmaResult ====================

/// CMA 处理结果
#[derive(Debug, Clone)]
pub struct CmaResult {
    /// 是否成功
    pub success: bool,
    /// 裁剪的消息数量
    pub trimmed_count: usize,
    /// 结果消息
    pub message: String,
}

// ==================== OrchestrationProvider ====================

/// 总控接口实现
///
/// 为本地工具提供访问总控能力的能力，同时保持模块解耦。
#[derive(Clone)]
pub struct OrchestrationProvider {
    /// 共享的总控状态
    pub orchestrator: Arc<RwLock<Orchestrator>>,
    /// 共享的任务管理器
    pub task_manager: Option<SharedTaskManager>,
    /// LLM 提供者注册表
    pub provider_registry: Arc<LlmProviderRegistry>,
    /// MCP 服务器管理器
    pub mcp_server_manager: Arc<Mutex<McpServerManager>>,
    /// 工具执行器
    pub tool_executor: ToolExecutor,
    /// 本地工具注册表
    pub local_tool_registry: Arc<LocalToolRegistry>,
    /// 应用配置
    pub config: Arc<Config>,
    /// Session 仓库
    pub session_repo: Option<Arc<SessionRepository>>,
}

impl std::fmt::Debug for OrchestrationProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OrchestrationProvider")
            .field("orchestrator", &"Arc<RwLock<Orchestrator>>")
            .field("task_manager", &self.task_manager)
            .field("provider_registry", &"Arc<LlmProviderRegistry>")
            .field("mcp_server_manager", &"Arc<Mutex<McpServerManager>>")
            .field("tool_executor", &"ToolExecutor")
            .field("local_tool_registry", &"Arc<LocalToolRegistry>")
            .field("config", &self.config)
            .field("session_repo", &"Option<Arc<SessionRepository>>")
            .finish()
    }
}

impl OrchestrationProvider {
    /// 创建新的总控提供者
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        orchestrator: Arc<RwLock<Orchestrator>>,
        task_manager: Option<SharedTaskManager>,
        provider_registry: Arc<LlmProviderRegistry>,
        mcp_server_manager: Arc<Mutex<McpServerManager>>,
        tool_executor: ToolExecutor,
        local_tool_registry: Arc<LocalToolRegistry>,
        config: Arc<Config>,
        session_repo: Option<Arc<SessionRepository>>,
    ) -> Self {
        Self {
            orchestrator,
            task_manager,
            provider_registry,
            mcp_server_manager,
            tool_executor,
            local_tool_registry,
            config,
            session_repo,
        }
    }
}

#[async_trait]
impl OrchestrationInterface for OrchestrationProvider {
    async fn submit_report_work_order(
        &self,
        completed_details: &str,
        related_files: &[String],
        next_stage_plan: &str,
    ) -> Result<Value> {
        let orch = self.orchestrator.read().await;

        // 构造 PLAN.md 规范要求的工单内容
        let work_order_content = json!({
            "completed_details": completed_details,
            "related_files": related_files,
            "next_stage_plan": next_stage_plan,
        });

        info!(
            orchestrator_id = %orch.id,
            "Work order (Report) submitted via tool"
        );

        // 在 Phase 5 中，这里会触发真正的工单发送逻辑。
        // 目前返回格式化的 JSON 以供 Agent 确认。
        Ok(json!({
            "status": "submitted",
            "type": "report",
            "work_order": work_order_content
        }))
    }

    async fn submit_help_work_order(
        &self,
        problem_description: &str,
        current_status: &str,
    ) -> Result<Value> {
        let orch = self.orchestrator.read().await;

        info!(
            orchestrator_id = %orch.id,
            "Work order (Help) submitted via tool"
        );

        Ok(json!({
            "status": "submitted",
            "type": "help",
            "problem": problem_description,
            "current_status": current_status
        }))
    }

    async fn spawn_sub_agent(
        &self,
        name: &str,
        role: &str,
        capability: &str,
        model_profile: Option<&str>,
    ) -> Result<String> {
        let mut orch = self.orchestrator.write().await;

        // 解析角色
        use std::str::FromStr;
        let agent_role = crate::agent::AgentRole::from_str(role)?;

        // 确定使用的 LLM 配置
        let llm_config = if let Some(model_name) = model_profile {
            // 尝试从注册表解析指定的模型档案
            if let Some(profile) = self.provider_registry.get_model_profile(model_name) {
                crate::agent::LlmConfig::from_profile(profile)
            } else {
                // 如果模型档案不存在，返回错误
                return Err(crate::error::Error::ModelProfileNotFound(
                    model_name.to_string(),
                ));
            }
        } else {
            // 如果没有指定，继承父节点的配置
            orch.agent.llm_config.clone()
        };

        // 构造配置
        let mut agent_config = AgentConfig::new(
            name.to_string(),
            agent_role,
            llm_config,
            format!("You are a specialized agent. Capability: {}", capability),
        )
        .with_capability(capability.to_string());

        // 如果创建的是子总控，检查嵌套深度限制
        if matches!(
            agent_config.role,
            AgentRole::MasterOrchestrator | AgentRole::SubOrchestrator
        ) {
            let new_depth = orch.nested_depth + 1;
            if new_depth > self.config.orchestrator.max_nested_depth {
                return Err(Error::InvalidConfig(format!(
                    "Cannot create nested orchestrator: depth {} exceeds max_nested_depth {}",
                    new_depth, self.config.orchestrator.max_nested_depth
                )));
            }
            agent_config = agent_config.with_nested_depth(new_depth);
            agent_config =
                agent_config.with_parent_orchestrator(AgentId::from_uuid(*orch.id.as_uuid()));
        }

        // 执行创建逻辑
        // 因为 OrchestratorExecutor::create_agent 消费 Orchestrator，
        // 我们利用 Orchestrator 是 Clone 的特性进行原地更新。
        // 创建 AgentExecutor 实例
        let agent_executor = if let Some(ref repo) = self.session_repo {
            AgentExecutor::with_session_repo(
                self.provider_registry.clone(),
                self.mcp_server_manager.clone(),
                self.tool_executor.clone(),
                self.local_tool_registry.clone(),
                self.config.clone(),
                repo.clone(),
            )
        } else {
            AgentExecutor::new(
                self.provider_registry.clone(),
                self.mcp_server_manager.clone(),
                self.tool_executor.clone(),
                self.local_tool_registry.clone(),
                self.config.clone(),
            )
        };

        // 创建 Agent
        let agent = agent_executor.create_agent(agent_config)?;

        // 添加到总控的管理列表
        orch.add_agent(agent.clone());

        // 因为我们直接修改了 orch，不需要 new_orch
        let new_orch = orch.clone();
        *orch = new_orch;

        Ok(agent.id.to_string())
    }

    async fn assign_task(
        &self,
        target_agent_id: &str,
        instruction: &str,
        is_parallel: bool,
    ) -> Result<Value> {
        let mut orch = self.orchestrator.write().await;
        let agent_id = AgentId::parse_str(target_agent_id)?;

        let session_id = orch.session_id.ok_or_else(|| {
            Error::InvalidConfig("Orchestrator has no associated session".to_string())
        })?;

        let task = AgentTask {
            agent_id,
            session_id,
            user_message: instruction.to_string(),
            tools: None,
            checkpoint_id: None,
        };

        if is_parallel {
            let main_task_id = TaskId::new();
            let sub_task_id = TaskId::new();
            let mut parallel_tasks = ParallelTasks::new(main_task_id, true);
            parallel_tasks.add_assignment(crate::orchestrator::types::TaskAssignment::new(
                sub_task_id,
                agent_id,
                task,
            ));

            let task_id = OrchestratorExecutor::assign_task_parallel(
                &orch,
                parallel_tasks,
                self.task_manager.as_ref(),
                Some(Arc::new(self.clone())),
            )
            .await?;

            Ok(json!({
                "task_id": task_id.to_string(),
                "status": "Running"
            }))
        } else {
            // 创建 OrchestratorExecutor 实例
            let orch_executor = if let Some(ref repo) = self.session_repo {
                OrchestratorExecutor::with_session_repo(
                    self.provider_registry.clone(),
                    self.mcp_server_manager.clone(),
                    self.tool_executor.clone(),
                    self.local_tool_registry.clone(),
                    self.config.clone(),
                    repo.clone(),
                )
            } else {
                OrchestratorExecutor::new(
                    self.provider_registry.clone(),
                    self.mcp_server_manager.clone(),
                    self.tool_executor.clone(),
                    self.local_tool_registry.clone(),
                    self.config.clone(),
                )
            };

            let result = orch_executor
                .assign_task_serial(&mut orch, &agent_id, task, Some(Arc::new(self.clone())))
                .await?;
            Ok(serde_json::to_value(result)?)
        }
    }
}