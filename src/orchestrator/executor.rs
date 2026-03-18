//! Orchestrator 执行器
//!
//! 提供总控的创建、Agent 管理、任务分配和工单处理等核心功能。

use chrono::Utc;
use tracing::{debug, info};
use uuid::Uuid;

use crate::agent::work_order::{WorkOrder, WorkOrderRecipient, WorkOrderType};
use crate::agent::{
    Agent, AgentConfig, AgentExecutor, AgentId, AgentRole, AgentState, AgentTask, AgentTaskResult,
};
use crate::error::{Error, Result};
use crate::mcp::{McpServerManager, ToolExecutor};
use crate::tools::LocalToolRegistry;

use super::task_manager::SharedTaskManager;
use super::types::{
    CmaNotification, CmaNotificationType, Orchestrator, OrchestratorConfig, ParallelTasks, TaskId,
    TaskStatus,
};

use super::prompt_template::PromptAssembler;
use crate::config::Config;
use crate::llm::LlmProviderRegistry;
use crate::tools::orchestration::OrchestrationInterface;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::sync::Arc;
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
        );

        // 创建 AgentExecutor 实例
        let agent_executor = AgentExecutor::new(
            self.provider_registry.clone(),
            self.mcp_server_manager.clone(),
            self.tool_executor.clone(),
            self.local_tool_registry.clone(),
            self.config.clone(),
        );

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

        // 如果创建的是子总控，自动设置 nested_depth
        if matches!(
            agent_config.role,
            AgentRole::MasterOrchestrator | AgentRole::SubOrchestrator
        ) {
            agent_config = agent_config.with_nested_depth(orchestrator.nested_depth + 1);
            agent_config = agent_config
                .with_parent_orchestrator(AgentId::from_uuid(*orchestrator.id.as_uuid()));
        }

        // 创建 AgentExecutor 实例
        let agent_executor = AgentExecutor::new(
            self.provider_registry.clone(),
            self.mcp_server_manager.clone(),
            self.tool_executor.clone(),
            self.local_tool_registry.clone(),
            self.config.clone(),
        );

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
        let agent_executor = AgentExecutor::new(
            self.provider_registry.clone(),
            self.mcp_server_manager.clone(),
            self.tool_executor.clone(),
            self.local_tool_registry.clone(),
            self.config.clone(),
        );

        // 执行任务
        let result = agent_executor.execute_task(agent, task, provider).await?;

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

    /// 处理 CMA 通知
    ///
    /// # 参数
    /// * `orchestrator` - 总控实例
    /// * `notification` - CMA 通知
    /// * `task_manager` - 任务管理器（可选，用于取消相关任务）
    ///
    /// # 返回
    /// 返回更新后的总控或错误
    pub fn handle_cma_notification(
        mut orchestrator: Orchestrator,
        notification: CmaNotification,
        task_manager: Option<&SharedTaskManager>,
    ) -> Result<Orchestrator> {
        debug!(
            orchestrator_id = %orchestrator.id,
            notification_type = ?notification.notification_type,
            session_id = %notification.session_id,
            "Handling CMA notification"
        );

        match notification.notification_type {
            CmaNotificationType::RollbackAndHandover => {
                info!(
                    orchestrator_id = %orchestrator.id,
                    checkpoint_id = ?notification.checkpoint_id,
                    reason = %notification.reason,
                    "Processing RollbackAndHandover notification"
                );

                // 如果有 TaskManager，取消该 Session 相关的所有任务
                if let Some(_tm) = task_manager {
                    // 注意：这里需要根据 session_id 找到相关任务，
                    // 目前 TaskManager 没有按 session_id 索引，
                    // 将来可以扩展 TaskManager 来支持这个功能
                    info!(
                        orchestrator_id = %orchestrator.id,
                        "TaskManager available, but session-based task cancellation not implemented yet"
                    );
                }

                // TODO: 完整实现需要：
                // 1. 回退到指定的 Checkpoint
                // 2. 恢复 Session 状态
                // 3. 创建新的 Agent 进行转交
                // 4. 传递必要的上下文给新 Agent
                info!(
                    orchestrator_id = %orchestrator.id,
                    "RollbackAndHandover placeholder - full implementation pending"
                );
            }
            CmaNotificationType::ContextTrimmed => {
                info!(
                    orchestrator_id = %orchestrator.id,
                    reason = %notification.reason,
                    "Processing ContextTrimmed notification"
                );

                // TODO: 完整实现需要：
                // 1. 记录上下文已裁剪
                // 2. 可能需要更新 Session 元数据
                // 3. 考虑是否需要重新评估路由策略
                info!(
                    orchestrator_id = %orchestrator.id,
                    "ContextTrimmed placeholder - full implementation pending"
                );
            }
        }

        orchestrator.updated_at = Utc::now();

        info!(
            orchestrator_id = %orchestrator.id,
            "CMA notification handled successfully"
        );

        Ok(orchestrator)
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
}

// ==================== OrchestrationProvider ====================

/// 总控接口实现
///
/// 为本地工具提供访问总控能力的能力，同时保持模块解耦。
#[derive(Clone)]
pub struct OrchestratorProvider {
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
}

impl std::fmt::Debug for OrchestratorProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OrchestratorProvider")
            .field("orchestrator", &"Arc<RwLock<Orchestrator>>")
            .field("task_manager", &self.task_manager)
            .field("provider_registry", &"Arc<LlmProviderRegistry>")
            .field("mcp_server_manager", &"Arc<Mutex<McpServerManager>>")
            .field("tool_executor", &"ToolExecutor")
            .field("local_tool_registry", &"Arc<LocalToolRegistry>")
            .field("config", &self.config)
            .finish()
    }
}

impl OrchestratorProvider {
    /// 创建新的总控提供者
    pub fn new(
        orchestrator: Arc<RwLock<Orchestrator>>,
        task_manager: Option<SharedTaskManager>,
        provider_registry: Arc<LlmProviderRegistry>,
        mcp_server_manager: Arc<Mutex<McpServerManager>>,
        tool_executor: ToolExecutor,
        local_tool_registry: Arc<LocalToolRegistry>,
        config: Arc<Config>,
    ) -> Self {
        Self {
            orchestrator,
            task_manager,
            provider_registry,
            mcp_server_manager,
            tool_executor,
            local_tool_registry,
            config,
        }
    }
}

#[async_trait]
impl OrchestrationInterface for OrchestratorProvider {
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
        let agent_config = AgentConfig::new(
            name.to_string(),
            agent_role,
            llm_config,
            format!("You are a specialized agent. Capability: {}", capability),
        )
        .with_capability(capability.to_string());

        // 执行创建逻辑
        // 因为 OrchestratorExecutor::create_agent 消费 Orchestrator，
        // 我们利用 Orchestrator 是 Clone 的特性进行原地更新。
        // 创建 AgentExecutor 实例
        let agent_executor = AgentExecutor::new(
            self.provider_registry.clone(),
            self.mcp_server_manager.clone(),
            self.tool_executor.clone(),
            self.local_tool_registry.clone(),
            self.config.clone(),
        );

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
            let orch_executor = OrchestratorExecutor::new(
                self.provider_registry.clone(),
                self.mcp_server_manager.clone(),
                self.tool_executor.clone(),
                self.local_tool_registry.clone(),
                self.config.clone(),
            );

            let result = orch_executor
                .assign_task_serial(&mut orch, &agent_id, task, Some(Arc::new(self.clone())))
                .await?;
            Ok(serde_json::to_value(result)?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::work_order::WorkOrderRecipient;
    use crate::agent::{AgentConfig, AgentRole, LlmConfig};
    use crate::config::Config;
    use crate::llm::LlmProviderRegistry;
    use std::sync::Arc;

    use crate::orchestrator::OrchestratorId;

    // 辅助函数：创建测试用的 OrchestratorExecutor
    fn create_test_executor() -> OrchestratorExecutor {
        let mut config = Config::default();
        config.llm.model = "gpt-4-test".to_string();
        config.llm.api_key = "test-key".to_string();
        config.llm.provider = "openai".to_string();

        let registry = LlmProviderRegistry::from_config(&config).unwrap();
        let mcp_server_manager =
            Arc::new(tokio::sync::Mutex::new(crate::mcp::McpServerManager::new()));
        let tool_executor = crate::mcp::ToolExecutor::new();
        let local_tool_registry = Arc::new(crate::tools::LocalToolRegistry::new());

        OrchestratorExecutor::new(
            Arc::new(registry),
            mcp_server_manager,
            tool_executor,
            local_tool_registry,
            Arc::new(config),
        )
    }

    #[test]
    fn test_create_orchestrator_master() {
        let executor = create_test_executor();
        let agent_config = AgentConfig::new(
            "Master Agent".to_string(),
            AgentRole::MasterOrchestrator,
            LlmConfig::new("gpt-4".to_string()),
            "You are a master orchestrator.".to_string(),
        );

        let config = OrchestratorConfig::new_master("Test Master".to_string(), agent_config);
        let orchestrator = executor.create_orchestrator(config).unwrap();

        assert_eq!(orchestrator.name, "Test Master");
        assert!(orchestrator.is_master());
        assert_eq!(orchestrator.nested_depth, 0);
        assert!(orchestrator.parent_orchestrator_id.is_none());
        assert!(orchestrator.managed_agents.is_empty());

        // 验证系统提示词被增强了
        assert!(orchestrator.agent.system_prompt.contains("可用模型信息"));
    }

    #[test]
    fn test_create_orchestrator_sub() {
        let executor = create_test_executor();
        let parent_id = OrchestratorId::new();
        let agent_config = AgentConfig::new(
            "Sub Agent".to_string(),
            AgentRole::SubOrchestrator,
            LlmConfig::new("gpt-4".to_string()),
            "You are a sub orchestrator.".to_string(),
        )
        .with_nested_depth(1)
        .with_parent_orchestrator(AgentId::from_uuid(*parent_id.as_uuid()));

        let config =
            OrchestratorConfig::new_sub("Test Sub".to_string(), agent_config, 1, parent_id);
        let orchestrator = executor.create_orchestrator(config).unwrap();

        assert_eq!(orchestrator.name, "Test Sub");
        assert!(orchestrator.is_sub());
        assert_eq!(orchestrator.nested_depth, 1);
        assert_eq!(orchestrator.parent_orchestrator_id, Some(parent_id));
    }

    #[test]
    fn test_create_orchestrator_invalid_config() {
        let executor = create_test_executor();
        let agent_config = AgentConfig::new(
            "Master Agent".to_string(),
            AgentRole::MasterOrchestrator,
            LlmConfig::new("gpt-4".to_string()),
            "You are a master orchestrator.".to_string(),
        );

        let mut config = OrchestratorConfig::new_master("Test".to_string(), agent_config);
        config.name = "".to_string(); // 空名称，应该失败

        let result = executor.create_orchestrator(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_agent_via_orchestrator() {
        let executor = create_test_executor();
        let agent_config = AgentConfig::new(
            "Master Agent".to_string(),
            AgentRole::MasterOrchestrator,
            LlmConfig::new("gpt-4".to_string()),
            "You are a master orchestrator.".to_string(),
        );

        let config = OrchestratorConfig::new_master("Test Master".to_string(), agent_config);
        let orchestrator = executor.create_orchestrator(config).unwrap();

        let worker_config = AgentConfig::new(
            "Worker Agent".to_string(),
            AgentRole::Worker,
            LlmConfig::new("gpt-4".to_string()),
            "You are a helpful worker.".to_string(),
        );

        let (orchestrator, agent) = executor.create_agent(orchestrator, worker_config).unwrap();

        assert_eq!(agent.name, "Worker Agent");
        assert_eq!(agent.role, AgentRole::Worker);
        assert_eq!(orchestrator.managed_agents.len(), 1);
        assert!(orchestrator.get_agent(&agent.id).is_some());
    }

    #[test]
    fn test_create_sub_orchestrator_via_orchestrator() {
        let executor = create_test_executor();
        let agent_config = AgentConfig::new(
            "Master Agent".to_string(),
            AgentRole::MasterOrchestrator,
            LlmConfig::new("gpt-4".to_string()),
            "You are a master orchestrator.".to_string(),
        );

        let config = OrchestratorConfig::new_master("Test Master".to_string(), agent_config);
        let orchestrator = executor.create_orchestrator(config).unwrap();

        let sub_config = AgentConfig::new(
            "Sub Agent".to_string(),
            AgentRole::SubOrchestrator,
            LlmConfig::new("gpt-4".to_string()),
            "You are a sub orchestrator.".to_string(),
        );

        let (orchestrator, agent) = executor.create_agent(orchestrator, sub_config).unwrap();

        assert_eq!(agent.name, "Sub Agent");
        assert_eq!(agent.role, AgentRole::SubOrchestrator);
        assert_eq!(agent.nested_depth, Some(1)); // 应该自动设置为父深度 + 1
        assert_eq!(orchestrator.managed_agents.len(), 1);
    }

    #[test]
    fn test_list_agents() {
        let executor = create_test_executor();
        let agent_config = AgentConfig::new(
            "Master Agent".to_string(),
            AgentRole::MasterOrchestrator,
            LlmConfig::new("gpt-4".to_string()),
            "You are a master orchestrator.".to_string(),
        );

        let config = OrchestratorConfig::new_master("Test Master".to_string(), agent_config);
        let mut orchestrator = executor.create_orchestrator(config).unwrap();

        // 创建两个 Agent
        let worker1_config = AgentConfig::new(
            "Worker 1".to_string(),
            AgentRole::Worker,
            LlmConfig::new("gpt-4".to_string()),
            "You are worker 1.".to_string(),
        );
        let (o1, _) = executor.create_agent(orchestrator, worker1_config).unwrap();
        orchestrator = o1;

        let worker2_config = AgentConfig::new(
            "Worker 2".to_string(),
            AgentRole::Worker,
            LlmConfig::new("gpt-4".to_string()),
            "You are worker 2.".to_string(),
        );
        let (o2, _) = executor.create_agent(orchestrator, worker2_config).unwrap();
        orchestrator = o2;

        let agents = OrchestratorExecutor::list_agents(&orchestrator);
        assert_eq!(agents.len(), 2);
    }

    #[test]
    fn test_get_agent() {
        let executor = create_test_executor();
        let agent_config = AgentConfig::new(
            "Master Agent".to_string(),
            AgentRole::MasterOrchestrator,
            LlmConfig::new("gpt-4".to_string()),
            "You are a master orchestrator.".to_string(),
        );

        let config = OrchestratorConfig::new_master("Test Master".to_string(), agent_config);
        let orchestrator = executor.create_orchestrator(config).unwrap();

        let worker_config = AgentConfig::new(
            "Worker Agent".to_string(),
            AgentRole::Worker,
            LlmConfig::new("gpt-4".to_string()),
            "You are a helpful worker.".to_string(),
        );

        let (orchestrator, agent) = executor.create_agent(orchestrator, worker_config).unwrap();

        let found = OrchestratorExecutor::get_agent(&orchestrator, &agent.id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, agent.id);

        let non_existent = AgentId::new();
        let not_found = OrchestratorExecutor::get_agent(&orchestrator, &non_existent);
        assert!(not_found.is_none());
    }

    #[test]
    fn test_remove_agent() {
        let executor = create_test_executor();
        let agent_config = AgentConfig::new(
            "Master Agent".to_string(),
            AgentRole::MasterOrchestrator,
            LlmConfig::new("gpt-4".to_string()),
            "You are a master orchestrator.".to_string(),
        );

        let config = OrchestratorConfig::new_master("Test Master".to_string(), agent_config);
        let orchestrator = executor.create_orchestrator(config).unwrap();

        let worker_config = AgentConfig::new(
            "Worker Agent".to_string(),
            AgentRole::Worker,
            LlmConfig::new("gpt-4".to_string()),
            "You are a helpful worker.".to_string(),
        );

        let (orchestrator, agent) = executor.create_agent(orchestrator, worker_config).unwrap();

        assert_eq!(orchestrator.managed_agents.len(), 1);

        let orchestrator = OrchestratorExecutor::remove_agent(orchestrator, &agent.id).unwrap();
        assert_eq!(orchestrator.managed_agents.len(), 0);
    }

    #[test]
    fn test_remove_nonexistent_agent() {
        let executor = create_test_executor();
        let agent_config = AgentConfig::new(
            "Master Agent".to_string(),
            AgentRole::MasterOrchestrator,
            LlmConfig::new("gpt-4".to_string()),
            "You are a master orchestrator.".to_string(),
        );

        let config = OrchestratorConfig::new_master("Test Master".to_string(), agent_config);
        let orchestrator = executor.create_orchestrator(config).unwrap();

        let non_existent = AgentId::new();
        let result = OrchestratorExecutor::remove_agent(orchestrator, &non_existent);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_work_order() {
        let executor = create_test_executor();
        let agent_config = AgentConfig::new(
            "Master Agent".to_string(),
            AgentRole::MasterOrchestrator,
            LlmConfig::new("gpt-4".to_string()),
            "You are a master orchestrator.".to_string(),
        );

        let config = OrchestratorConfig::new_master("Test Master".to_string(), agent_config);
        let orchestrator = executor.create_orchestrator(config).unwrap();

        let work_order = OrchestratorExecutor::generate_work_order(
            &orchestrator,
            WorkOrderType::HelpRequest,
            WorkOrderRecipient::ContextManager,
            "Test Title".to_string(),
            "Test Content".to_string(),
        )
        .unwrap();

        assert_eq!(work_order.title, "Test Title");
        assert_eq!(work_order.content, "Test Content");
        assert_eq!(work_order.created_by, Some(orchestrator.agent.id));
    }

    #[test]
    fn test_associate_session() {
        let executor = create_test_executor();
        let agent_config = AgentConfig::new(
            "Master Agent".to_string(),
            AgentRole::MasterOrchestrator,
            LlmConfig::new("gpt-4".to_string()),
            "You are a master orchestrator.".to_string(),
        );

        let config = OrchestratorConfig::new_master("Test Master".to_string(), agent_config);
        let orchestrator = executor.create_orchestrator(config).unwrap();

        assert!(orchestrator.session_id.is_none());

        let session_id = Uuid::new_v4();
        let orchestrator = OrchestratorExecutor::associate_session(orchestrator, session_id);

        assert_eq!(orchestrator.session_id, Some(session_id));
    }

    #[test]
    fn test_handle_cma_notification() {
        let executor = create_test_executor();
        let agent_config = AgentConfig::new(
            "Master Agent".to_string(),
            AgentRole::MasterOrchestrator,
            LlmConfig::new("gpt-4".to_string()),
            "You are a master orchestrator.".to_string(),
        );

        let config = OrchestratorConfig::new_master("Test Master".to_string(), agent_config);
        let orchestrator = executor.create_orchestrator(config).unwrap();

        let session_id = Uuid::new_v4();
        let notification = CmaNotification::new(
            CmaNotificationType::RollbackAndHandover,
            session_id,
            orchestrator.id,
            "Test reason".to_string(),
        );

        let result =
            OrchestratorExecutor::handle_cma_notification(orchestrator, notification, None);
        assert!(result.is_ok());
    }
}
