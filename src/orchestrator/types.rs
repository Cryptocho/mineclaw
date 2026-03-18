//! Orchestrator 数据类型定义
//!
//! 定义总控机制所需的所有数据结构。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

use crate::agent::{Agent, AgentConfig, AgentId, AgentTask};

use crate::error::{Error, Result};

// ==================== 基础 ID 类型 ====================

/// 总控唯一标识符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrchestratorId(Uuid);

impl OrchestratorId {
    /// 创建新的总控 ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// 从 Uuid 创建总控 ID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// 获取 Uuid 引用
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// 从字符串解析总控 ID
    pub fn parse_str(s: &str) -> Result<Self> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|e| Error::InvalidConfig(format!("Invalid OrchestratorId: {}", e)))
    }
}

impl Default for OrchestratorId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for OrchestratorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 总控角色
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrchestratorRole {
    /// 主总控
    Master,
    /// 子总控
    Sub,
}

impl fmt::Display for OrchestratorRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrchestratorRole::Master => write!(f, "Master"),
            OrchestratorRole::Sub => write!(f, "Sub"),
        }
    }
}

/// 任务唯一标识符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(Uuid);

impl TaskId {
    /// 创建新的任务 ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// 从 Uuid 创建任务 ID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// 获取 Uuid 引用
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// 从字符串解析任务 ID
    pub fn parse_str(s: &str) -> Result<Self> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|e| Error::InvalidConfig(format!("Invalid TaskId: {}", e)))
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 任务状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// 待处理
    Pending,
    /// 运行中
    Running,
    /// 已完成
    Completed,
    /// 失败
    Failed,
    /// 已取消
    Cancelled,
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "Pending"),
            TaskStatus::Running => write!(f, "Running"),
            TaskStatus::Completed => write!(f, "Completed"),
            TaskStatus::Failed => write!(f, "Failed"),
            TaskStatus::Cancelled => write!(f, "Cancelled"),
        }
    }
}

// ==================== 任务分配类型 ====================

/// 任务分配
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAssignment {
    /// 任务 ID
    pub task_id: TaskId,
    /// 代理 ID
    pub agent_id: AgentId,
    /// 任务内容
    pub task: AgentTask,
    /// 分配时间
    pub assigned_at: DateTime<Utc>,
}

impl TaskAssignment {
    /// 创建新的任务分配
    pub fn new(task_id: TaskId, agent_id: AgentId, task: AgentTask) -> Self {
        Self {
            task_id,
            agent_id,
            task,
            assigned_at: Utc::now(),
        }
    }
}

/// 并行任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelTasks {
    /// 主任务 ID
    pub task_id: TaskId,
    /// 任务分配列表
    pub assignments: Vec<TaskAssignment>,
    /// 是否等待所有任务完成
    pub wait_for_all: bool,
}

impl ParallelTasks {
    /// 创建新的并行任务
    pub fn new(task_id: TaskId, wait_for_all: bool) -> Self {
        Self {
            task_id,
            assignments: Vec::new(),
            wait_for_all,
        }
    }

    /// 添加任务分配
    pub fn add_assignment(&mut self, assignment: TaskAssignment) {
        self.assignments.push(assignment);
    }
}

// ==================== CMA 通知类型 ====================

/// CMA 通知类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CmaNotificationType {
    /// 回退并转交
    RollbackAndHandover,
    /// 上下文已裁剪
    ContextTrimmed,
}

impl fmt::Display for CmaNotificationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CmaNotificationType::RollbackAndHandover => write!(f, "RollbackAndHandover"),
            CmaNotificationType::ContextTrimmed => write!(f, "ContextTrimmed"),
        }
    }
}

/// CMA 通知
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CmaNotification {
    /// 通知类型
    pub notification_type: CmaNotificationType,
    /// 会话 ID
    pub session_id: Uuid,
    /// 目标总控 ID
    pub target_orchestrator_id: OrchestratorId,
    /// 检查点 ID（可选）
    pub checkpoint_id: Option<String>,
    /// 原因
    pub reason: String,
    /// 创建时间
    pub created_at: DateTime<Utc>,
}

impl CmaNotification {
    /// 创建新的 CMA 通知
    pub fn new(
        notification_type: CmaNotificationType,
        session_id: Uuid,
        target_orchestrator_id: OrchestratorId,
        reason: String,
    ) -> Self {
        Self {
            notification_type,
            session_id,
            target_orchestrator_id,
            checkpoint_id: None,
            reason,
            created_at: Utc::now(),
        }
    }

    /// 设置检查点 ID
    pub fn with_checkpoint_id(mut self, checkpoint_id: String) -> Self {
        self.checkpoint_id = Some(checkpoint_id);
        self
    }
}

// ==================== 总控配置 ====================

/// 总控配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    /// 总控名称
    pub name: String,
    /// 总控角色
    pub role: OrchestratorRole,
    /// 总控自身的 Agent 配置
    pub agent_config: AgentConfig,
    /// 嵌套深度（Master 为 0）
    pub nested_depth: u8,
    /// 父总控 ID（子总控需要）
    pub parent_orchestrator_id: Option<OrchestratorId>,
}

impl OrchestratorConfig {
    /// 创建新的主总控配置
    pub fn new_master(name: String, agent_config: AgentConfig) -> Self {
        Self {
            name,
            role: OrchestratorRole::Master,
            agent_config,
            nested_depth: 0,
            parent_orchestrator_id: None,
        }
    }

    /// 创建新的子总控配置
    pub fn new_sub(
        name: String,
        agent_config: AgentConfig,
        nested_depth: u8,
        parent_orchestrator_id: OrchestratorId,
    ) -> Self {
        Self {
            name,
            role: OrchestratorRole::Sub,
            agent_config,
            nested_depth,
            parent_orchestrator_id: Some(parent_orchestrator_id),
        }
    }

    /// 验证配置
    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(Error::InvalidConfig(
                "Orchestrator name cannot be empty".to_string(),
            ));
        }

        if self.role == OrchestratorRole::Master {
            if self.nested_depth != 0 {
                return Err(Error::InvalidConfig(
                    "Master orchestrator must have nested_depth 0".to_string(),
                ));
            }
            if self.parent_orchestrator_id.is_some() {
                return Err(Error::InvalidConfig(
                    "Master orchestrator cannot have a parent".to_string(),
                ));
            }
        } else {
            if self.nested_depth == 0 {
                return Err(Error::InvalidConfig(
                    "Sub orchestrator must have nested_depth > 0".to_string(),
                ));
            }
            if self.parent_orchestrator_id.is_none() {
                return Err(Error::InvalidConfig(
                    "Sub orchestrator must have a parent".to_string(),
                ));
            }
        }

        // 验证 Agent 配置
        self.agent_config.validate()?;

        Ok(())
    }
}

// ==================== 总控核心数据结构 ====================

/// 总控
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Orchestrator {
    /// 总控 ID
    pub id: OrchestratorId,
    /// 总控名称
    pub name: String,
    /// 总控角色
    pub role: OrchestratorRole,
    /// 总控自身的 Agent
    pub agent: Agent,
    /// 嵌套深度
    pub nested_depth: u8,
    /// 父总控 ID
    pub parent_orchestrator_id: Option<OrchestratorId>,
    /// 管理的 Agent 列表
    pub managed_agents: HashMap<AgentId, Agent>,
    /// 会话 ID（可选）
    pub session_id: Option<Uuid>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

#[allow(dead_code)]
impl Orchestrator {
    /// 创建新的总控（内部方法）
    pub(crate) fn new(config: OrchestratorConfig, agent: Agent) -> Self {
        let now = Utc::now();
        Self {
            id: OrchestratorId::new(),
            name: config.name,
            role: config.role,
            agent,
            nested_depth: config.nested_depth,
            parent_orchestrator_id: config.parent_orchestrator_id,
            managed_agents: HashMap::new(),
            session_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// 检查是否是主总控
    pub fn is_master(&self) -> bool {
        self.role == OrchestratorRole::Master
    }

    /// 检查是否是子总控
    pub fn is_sub(&self) -> bool {
        self.role == OrchestratorRole::Sub
    }

    /// 获取管理的 Agent 引用
    pub fn get_agent(&self, agent_id: &AgentId) -> Option<&Agent> {
        self.managed_agents.get(agent_id)
    }

    /// 获取管理的 Agent 可变引用
    pub fn get_agent_mut(&mut self, agent_id: &AgentId) -> Option<&mut Agent> {
        self.managed_agents.get_mut(agent_id)
    }

    /// 列出所有管理的 Agent
    pub fn list_agents(&self) -> Vec<&Agent> {
        self.managed_agents.values().collect()
    }

    /// 添加 Agent 到管理列表
    pub(crate) fn add_agent(&mut self, agent: Agent) {
        self.managed_agents.insert(agent.id, agent);
        self.updated_at = Utc::now();
    }

    /// 从管理列表移除 Agent
    pub(crate) fn remove_agent(&mut self, agent_id: &AgentId) -> Option<Agent> {
        let agent = self.managed_agents.remove(agent_id);
        if agent.is_some() {
            self.updated_at = Utc::now();
        }
        agent
    }

    /// 关联会话
    pub fn with_session_id(mut self, session_id: Uuid) -> Self {
        self.session_id = Some(session_id);
        self.updated_at = Utc::now();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{AgentConfig, AgentRole, LlmConfig};

    #[test]
    fn test_orchestrator_id_new() {
        let id1 = OrchestratorId::new();
        let id2 = OrchestratorId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_orchestrator_id_from_uuid() {
        let uuid = Uuid::new_v4();
        let id = OrchestratorId::from_uuid(uuid);
        assert_eq!(id.as_uuid(), &uuid);
    }

    #[test]
    fn test_orchestrator_id_parse_str() {
        let uuid = Uuid::new_v4();
        let id = OrchestratorId::from_uuid(uuid);
        let parsed = OrchestratorId::parse_str(&id.to_string()).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_orchestrator_role_display() {
        assert_eq!(OrchestratorRole::Master.to_string(), "Master");
        assert_eq!(OrchestratorRole::Sub.to_string(), "Sub");
    }

    #[test]
    fn test_task_id_new() {
        let id1 = TaskId::new();
        let id2 = TaskId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_task_status_display() {
        assert_eq!(TaskStatus::Pending.to_string(), "Pending");
        assert_eq!(TaskStatus::Running.to_string(), "Running");
        assert_eq!(TaskStatus::Completed.to_string(), "Completed");
        assert_eq!(TaskStatus::Failed.to_string(), "Failed");
        assert_eq!(TaskStatus::Cancelled.to_string(), "Cancelled");
    }

    #[test]
    fn test_cma_notification_type_display() {
        assert_eq!(
            CmaNotificationType::RollbackAndHandover.to_string(),
            "RollbackAndHandover"
        );
        assert_eq!(
            CmaNotificationType::ContextTrimmed.to_string(),
            "ContextTrimmed"
        );
    }

    #[test]
    fn test_orchestrator_config_new_master() {
        let agent_config = AgentConfig::new(
            "Test Agent".to_string(),
            AgentRole::MasterOrchestrator,
            LlmConfig::new("gpt-4".to_string()),
            "You are a master orchestrator.".to_string(),
        );

        let config = OrchestratorConfig::new_master("Test Orchestrator".to_string(), agent_config);

        assert_eq!(config.name, "Test Orchestrator");
        assert_eq!(config.role, OrchestratorRole::Master);
        assert_eq!(config.nested_depth, 0);
        assert!(config.parent_orchestrator_id.is_none());
    }

    #[test]
    fn test_orchestrator_config_new_sub() {
        let agent_config = AgentConfig::new(
            "Test Agent".to_string(),
            AgentRole::SubOrchestrator,
            LlmConfig::new("gpt-4".to_string()),
            "You are a sub orchestrator.".to_string(),
        );

        let parent_id = OrchestratorId::new();
        let config =
            OrchestratorConfig::new_sub("Test Sub".to_string(), agent_config, 1, parent_id);

        assert_eq!(config.name, "Test Sub");
        assert_eq!(config.role, OrchestratorRole::Sub);
        assert_eq!(config.nested_depth, 1);
        assert_eq!(config.parent_orchestrator_id, Some(parent_id));
    }

    #[test]
    fn test_orchestrator_config_validate_master() {
        let agent_config = AgentConfig::new(
            "Test Agent".to_string(),
            AgentRole::MasterOrchestrator,
            LlmConfig::new("gpt-4".to_string()),
            "You are a master orchestrator.".to_string(),
        );

        let config = OrchestratorConfig::new_master("Valid".to_string(), agent_config);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_orchestrator_config_validate_empty_name() {
        let agent_config = AgentConfig::new(
            "Test Agent".to_string(),
            AgentRole::MasterOrchestrator,
            LlmConfig::new("gpt-4".to_string()),
            "You are a master orchestrator.".to_string(),
        );

        let mut config = OrchestratorConfig::new_master("Valid".to_string(), agent_config);
        config.name = "".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_orchestrator_config_validate_master_with_parent() {
        let agent_config = AgentConfig::new(
            "Test Agent".to_string(),
            AgentRole::MasterOrchestrator,
            LlmConfig::new("gpt-4".to_string()),
            "You are a master orchestrator.".to_string(),
        );

        let mut config = OrchestratorConfig::new_master("Valid".to_string(), agent_config);
        config.parent_orchestrator_id = Some(OrchestratorId::new());
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_orchestrator_config_validate_sub_without_parent() {
        let agent_config = AgentConfig::new(
            "Test Agent".to_string(),
            AgentRole::SubOrchestrator,
            LlmConfig::new("gpt-4".to_string()),
            "You are a sub orchestrator.".to_string(),
        );

        let parent_id = OrchestratorId::new();
        let mut config =
            OrchestratorConfig::new_sub("Test Sub".to_string(), agent_config, 1, parent_id);
        config.parent_orchestrator_id = None;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_task_assignment_new() {
        let task_id = TaskId::new();
        let agent_id = AgentId::new();
        let task = AgentTask {
            agent_id,
            session_id: Uuid::new_v4(),
            user_message: "Test message".to_string(),
            tools: Some(Vec::new()),
            checkpoint_id: None,
        };

        let assignment = TaskAssignment::new(task_id, agent_id, task.clone());

        assert_eq!(assignment.task_id, task_id);
        assert_eq!(assignment.agent_id, agent_id);
        assert_eq!(assignment.task.user_message, task.user_message);
    }

    #[test]
    fn test_parallel_tasks_new() {
        let task_id = TaskId::new();
        let mut parallel_tasks = ParallelTasks::new(task_id, true);

        assert_eq!(parallel_tasks.task_id, task_id);
        assert!(parallel_tasks.wait_for_all);
        assert!(parallel_tasks.assignments.is_empty());

        let agent_id = AgentId::new();
        let task = AgentTask {
            agent_id,
            session_id: Uuid::new_v4(),
            user_message: "Test".to_string(),
            tools: Some(Vec::new()),
            checkpoint_id: None,
        };
        let assignment = TaskAssignment::new(TaskId::new(), agent_id, task);

        parallel_tasks.add_assignment(assignment);
        assert_eq!(parallel_tasks.assignments.len(), 1);
    }

    #[test]
    fn test_cma_notification_new() {
        let session_id = Uuid::new_v4();
        let target_id = OrchestratorId::new();

        let notification = CmaNotification::new(
            CmaNotificationType::RollbackAndHandover,
            session_id,
            target_id,
            "Test reason".to_string(),
        );

        assert_eq!(
            notification.notification_type,
            CmaNotificationType::RollbackAndHandover
        );
        assert_eq!(notification.session_id, session_id);
        assert_eq!(notification.target_orchestrator_id, target_id);
        assert_eq!(notification.reason, "Test reason");
        assert!(notification.checkpoint_id.is_none());
    }

    #[test]
    fn test_cma_notification_with_checkpoint_id() {
        let session_id = Uuid::new_v4();
        let target_id = OrchestratorId::new();
        let checkpoint_id = "checkpoint_123".to_string();

        let notification = CmaNotification::new(
            CmaNotificationType::RollbackAndHandover,
            session_id,
            target_id,
            "Test reason".to_string(),
        )
        .with_checkpoint_id(checkpoint_id.clone());

        assert_eq!(notification.checkpoint_id, Some(checkpoint_id));
    }
}
