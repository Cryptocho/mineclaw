//! 本地工具模块
//!
//! 提供本地工具的 trait 定义和基础结构。

use crate::agent::types::AgentId;
use crate::checkpoint::CheckpointManager;
use crate::config::Config;
use crate::error::Result;
use crate::models::Session;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

pub mod checkpoint;
pub mod filesystem;
pub mod orchestration;
pub mod registry;
pub mod shell_detection;
pub mod terminal;

pub use self::checkpoint::{CheckpointTools, ListCheckpointsTool, RestoreCheckpointTool};
pub use self::orchestration::OrchestrationInterface;
pub use registry::LocalToolRegistry;

// ==================== ToolContext ====================

/// 工具执行上下文
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// 会话
    pub session: Session,
    /// 配置
    pub config: Arc<Config>,
    /// Checkpoint 管理器
    pub checkpoint_manager: Option<Arc<CheckpointManager>>,
    /// 总控接口（用于协作工具）
    pub orchestrator: Option<Arc<dyn OrchestrationInterface>>,
    /// 当前执行工具的 Agent ID
    pub agent_id: Option<AgentId>,
}

impl ToolContext {
    /// 创建新的工具上下文
    pub fn new(session: Session, config: Arc<Config>) -> Self {
        Self {
            session,
            config,
            checkpoint_manager: None,
            orchestrator: None,
            agent_id: None,
        }
    }

    /// 设置 Checkpoint 管理器
    pub fn with_checkpoint_manager(mut self, checkpoint_manager: Arc<CheckpointManager>) -> Self {
        self.checkpoint_manager = Some(checkpoint_manager);
        self
    }

    /// 设置总控接口
    pub fn with_orchestrator(mut self, orchestrator: Arc<dyn OrchestrationInterface>) -> Self {
        self.orchestrator = Some(orchestrator);
        self
    }

    /// 设置 Agent ID
    pub fn with_agent_id(mut self, agent_id: AgentId) -> Self {
        self.agent_id = Some(agent_id);
        self
    }
}

// ==================== LocalTool ====================

/// 本地工具 trait
#[async_trait]
pub trait LocalTool: Send + Sync {
    /// 获取工具名称
    fn name(&self) -> &str;

    /// 获取工具描述
    fn description(&self) -> &str;

    /// 获取工具输入 schema
    fn input_schema(&self) -> Value;

    /// 调用工具
    async fn call(&self, arguments: Value, context: ToolContext) -> Result<Value>;
}
