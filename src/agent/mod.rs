//! Agent 模块 - 多 Agent 系统的核心实现
//!
//! 提供 Agent 的定义、创建、执行任务和发送工单等功能。

// 占位模块声明，后续会实现
pub mod builder;
pub mod context;
pub mod context_manager;
pub mod types;
pub mod work_order;

// 占位 use 声明，后续会实现
pub use builder::*;
pub use context::*;
pub use context_manager::*;
pub use types::*;
pub use work_order::*;

use crate::error::{Error, Result};
use tracing::{debug, info};

/// Agent 执行器
///
/// 负责创建 Agent、执行任务和发送工单
pub struct AgentExecutor;

impl AgentExecutor {
    /// 创建一个新的 Agent
    ///
    /// # 参数
    /// * `config` - Agent 配置
    ///
    /// # 返回
    /// 返回创建的 Agent 或错误
    pub fn create_agent(config: AgentConfig) -> Result<Agent> {
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
    /// * `agent` - 要执行任务的 Agent
    /// * `task` - 任务信息
    ///
    /// # 返回
    /// 返回任务执行结果或错误
    pub async fn execute_task(agent: &mut Agent, task: AgentTask) -> Result<AgentTaskResult> {
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

        // 这里是实际执行任务的逻辑
        // 目前是占位实现，后续会集成 LLM 和工具调用
        let start_time = std::time::Instant::now();

        let result = AgentTaskResult {
            success: true,
            agent_id: agent.id,
            session_id: task.session_id,
            response: format!(
                "Task executed successfully (placeholder). Message: {}",
                task.user_message
            ),
            tool_calls: Vec::new(),
            error: None,
            execution_time_ms: start_time.elapsed().as_millis() as u64,
            new_checkpoint_id: None,
        };

        // 更新 Agent 状态回 Idle
        agent.set_state(AgentState::Idle);

        info!(
            agent_id = %agent.id,
            success = %result.success,
            execution_time_ms = %result.execution_time_ms,
            "Task execution completed"
        );

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_executor_creation() {
        let _executor = AgentExecutor;
        // 简单测试，确保可以创建
    }

    #[test]
    fn test_create_agent_success() {
        let config = AgentConfig::new(
            "Test Agent".to_string(),
            AgentRole::Worker,
            LlmConfig::new("gpt-4".to_string()),
            "You are a helpful assistant.".to_string(),
        );

        let agent = AgentExecutor::create_agent(config).unwrap();
        assert_eq!(agent.name, "Test Agent");
        assert_eq!(agent.role, AgentRole::Worker);
        assert_eq!(agent.state, AgentState::Idle);
    }

    #[test]
    fn test_create_agent_invalid_config() {
        let config = AgentConfig::new(
            "".to_string(), // 空名称，应该失败
            AgentRole::Worker,
            LlmConfig::new("gpt-4".to_string()),
            "You are a helpful assistant.".to_string(),
        );

        let result = AgentExecutor::create_agent(config);
        assert!(result.is_err());
    }
}
