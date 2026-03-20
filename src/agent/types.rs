//! Agent 基础类型定义
//!
//! 包含 AgentId、AgentRole、AgentCapability、LlmConfig、AgentState 等基础类型。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

use crate::config::ResolvedModelProfile;
use crate::error::{Error, Result};

// ============================================================================
// AgentId - Agent 唯一标识
// ============================================================================

/// Agent 唯一标识
///
/// 使用 Uuid v4 作为底层实现，提供类型安全的包装。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(Uuid);

impl AgentId {
    /// 创建一个新的随机 AgentId
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// 创建一个系统级 AgentId（用于非特定 Agent 的操作如 Session 状态变更）
    pub fn system() -> Self {
        Self(Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap())
    }

    /// 从 Uuid 创建 AgentId
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// 获取底层的 Uuid
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    /// 从字符串解析 AgentId
    pub fn parse_str(s: &str) -> Result<Self> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|e| Error::AgentInvalidConfig(format!("Invalid AgentId: {}", e)))
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ============================================================================
// AgentRole - Agent 角色
// ============================================================================

/// Agent 角色定义
///
/// 定义 Agent 在系统中的角色和职责。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentRole {
    /// 主总控 - 负责整个系统的协调和管理
    MasterOrchestrator,
    /// 子总控 - 负责特定任务域的协调
    SubOrchestrator,
    /// 工作 Agent - 实际执行任务的 Agent
    Worker,
    /// 上下文管理 Agent - 负责上下文管理和优化
    ContextManager,
}

impl std::str::FromStr for AgentRole {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.to_lowercase().replace("_", "").replace("-", "");
        match normalized.as_str() {
            "masterorchestrator" | "master" => Ok(Self::MasterOrchestrator),
            "suborchestrator" | "sub" => Ok(Self::SubOrchestrator),
            "worker" => Ok(Self::Worker),
            "contextmanager" | "cma" => Ok(Self::ContextManager),
            _ => Err(crate::error::Error::InvalidInput(format!(
                "Invalid agent role: {}",
                s
            ))),
        }
    }
}

impl fmt::Display for AgentRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MasterOrchestrator => write!(f, "MasterOrchestrator"),
            Self::SubOrchestrator => write!(f, "SubOrchestrator"),
            Self::Worker => write!(f, "Worker"),
            Self::ContextManager => write!(f, "ContextManager"),
        }
    }
}

// ============================================================================
// AgentCapability - Agent 能力标签
// ============================================================================

/// Agent 能力标签
///
/// 用于描述 Agent 的能力和专长。
pub type AgentCapability = String;

// ============================================================================
// LlmConfig - LLM 配置
// ============================================================================

/// LLM 配置
///
/// 配置 Agent 使用的 LLM 参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// 模型档案名称（如 "default", "cheap"）
    /// 这是 Phase 4 引入的核心字段，用于从 Registry 获取提供者
    #[serde(default = "default_model_profile")]
    pub model_profile: String,
    /// 模型显示名称（如 "gpt-4o"）
    pub model_name: String,
    /// 温度参数（可选，默认 0.7）
    pub temperature: Option<f32>,
    /// top_p 参数（可选）
    pub top_p: Option<f32>,
    /// 最大 token 数（可选）
    pub max_tokens: Option<u32>,
    /// 其他 LLM 特定参数（JSON 格式）
    pub extra_params: Option<serde_json::Value>,
}

fn default_model_profile() -> String {
    "default".to_string()
}

impl LlmConfig {
    /// 创建新的 LLM 配置
    pub fn new(profile_or_model: String) -> Self {
        Self {
            model_profile: profile_or_model.clone(),
            model_name: profile_or_model,
            temperature: Some(0.7),
            top_p: None,
            max_tokens: None,
            extra_params: None,
        }
    }

    /// 从已解析的模型档案创建配置
    pub fn from_profile(profile: &ResolvedModelProfile) -> Self {
        Self {
            model_profile: profile.model.clone(),
            model_name: profile.model.clone(),
            temperature: Some(profile.temperature as f32),
            top_p: None,
            max_tokens: Some(profile.max_tokens),
            extra_params: None,
        }
    }

    /// 设置模型名称（显示名称）
    pub fn with_model_name(mut self, model_name: String) -> Self {
        self.model_name = model_name;
        self
    }

    /// 设置温度参数
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// 设置 top_p 参数
    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    /// 设置最大 token 数
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// 设置额外参数
    pub fn with_extra_params(mut self, params: serde_json::Value) -> Self {
        self.extra_params = Some(params);
        self
    }

    /// 验证配置
    pub fn validate(&self) -> Result<()> {
        if self.model_profile.is_empty() {
            return Err(Error::AgentInvalidConfig(
                "Model profile name cannot be empty".to_string(),
            ));
        }

        if self.model_name.is_empty() {
            return Err(Error::AgentInvalidConfig(
                "Model name cannot be empty".to_string(),
            ));
        }

        if let Some(temp) = self.temperature
            && !(0.0..=2.0).contains(&temp)
        {
            return Err(Error::AgentInvalidConfig(format!(
                "Temperature must be between 0.0 and 2.0, got {}",
                temp
            )));
        }

        if let Some(top_p) = self.top_p
            && !(0.0..=1.0).contains(&top_p)
        {
            return Err(Error::AgentInvalidConfig(format!(
                "Top_p must be between 0.0 and 1.0, got {}",
                top_p
            )));
        }

        Ok(())
    }
}

// ============================================================================
// AgentState - Agent 状态
// ============================================================================

/// Agent 状态
///
/// 描述 Agent 当前的状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentState {
    /// 空闲，可接受任务
    Idle,
    /// 忙碌，正在执行任务
    Busy,
    /// 已完成，提交结果/求助等待审查/响应
    WaitingForReview,
}

impl fmt::Display for AgentState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Busy => write!(f, "Busy"),
            Self::WaitingForReview => write!(f, "WaitingForReview"),
        }
    }
}

// ============================================================================
// Agent - 核心 Agent 结构
// ============================================================================

/// Agent 核心数据结构
///
/// 代表一个完整的 Agent 实例。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    /// Agent 唯一标识
    pub id: AgentId,
    /// 人类可读名称
    pub name: String,
    /// Agent 角色
    pub role: AgentRole,
    /// Agent 能力标签
    pub capabilities: Vec<AgentCapability>,
    /// LLM 配置
    pub llm_config: LlmConfig,
    /// 当前状态
    pub state: AgentState,
    /// 系统提示词
    pub system_prompt: String,
    /// 嵌套深度（仅 SubOrchestrator 有）
    pub nested_depth: Option<u8>,
    /// 父总控 ID（仅 SubOrchestrator 有）
    pub parent_orchestrator_id: Option<AgentId>,
    /// 工具掩码
    pub tool_mask: Option<crate::tool_mask::types::ToolMask>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后更新时间
    pub updated_at: DateTime<Utc>,
}

impl Agent {
    /// 创建新的 Agent（内部使用，建议通过 AgentConfig 或 AgentBuilder）
    pub(crate) fn new(config: AgentConfig) -> Self {
        let now = Utc::now();
        Self {
            id: AgentId::new(),
            name: config.name,
            role: config.role,
            capabilities: config.capabilities,
            llm_config: config.llm_config,
            state: AgentState::Idle,
            system_prompt: config.system_prompt,
            nested_depth: config.nested_depth,
            parent_orchestrator_id: config.parent_orchestrator_id,
            tool_mask: config.tool_mask,
            created_at: now,
            updated_at: now,
        }
    }

    /// 设置 Agent 状态
    pub fn set_state(&mut self, state: AgentState) {
        self.state = state;
        self.updated_at = Utc::now();
    }

    /// 检查 Agent 是否是总控类型
    pub fn is_orchestrator(&self) -> bool {
        matches!(
            self.role,
            AgentRole::MasterOrchestrator | AgentRole::SubOrchestrator
        )
    }

    /// 检查 Agent 是否可以接受任务
    pub fn can_accept_task(&self) -> bool {
        matches!(self.state, AgentState::Idle)
    }

    /// 检查 Agent 是否在等待审查
    pub fn is_waiting_for_review(&self) -> bool {
        matches!(self.state, AgentState::WaitingForReview)
    }
}

// ============================================================================
// AgentConfig - Agent 配置
// ============================================================================

/// Agent 配置
///
/// 用于创建新的 Agent。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Agent 名称
    pub name: String,
    /// Agent 角色
    pub role: AgentRole,
    /// Agent 能力标签
    pub capabilities: Vec<AgentCapability>,
    /// LLM 配置
    pub llm_config: LlmConfig,
    /// 系统提示词
    pub system_prompt: String,
    /// 嵌套深度（仅 SubOrchestrator 需要）
    pub nested_depth: Option<u8>,
    /// 父总控 ID（仅 SubOrchestrator 需要）
    pub parent_orchestrator_id: Option<AgentId>,
    /// 工具掩码
    pub tool_mask: Option<crate::tool_mask::types::ToolMask>,
}

impl AgentConfig {
    /// 创建新的 Agent 配置
    pub fn new(
        name: String,
        role: AgentRole,
        llm_config: LlmConfig,
        system_prompt: String,
    ) -> Self {
        Self {
            name,
            role,
            capabilities: Vec::new(),
            llm_config,
            system_prompt,
            nested_depth: None,
            parent_orchestrator_id: None,
            tool_mask: None,
        }
    }

    /// 添加能力标签
    pub fn with_capability(mut self, capability: AgentCapability) -> Self {
        self.capabilities.push(capability);
        self
    }

    /// 设置多个能力标签
    pub fn with_capabilities(mut self, capabilities: Vec<AgentCapability>) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// 设置嵌套深度（用于 SubOrchestrator）
    pub fn with_nested_depth(mut self, depth: u8) -> Self {
        self.nested_depth = Some(depth);
        self
    }

    /// 设置父总控 ID（用于 SubOrchestrator）
    pub fn with_parent_orchestrator(mut self, parent_id: AgentId) -> Self {
        self.parent_orchestrator_id = Some(parent_id);
        self
    }

    /// 设置工具掩码
    pub fn with_tool_mask(mut self, tool_mask: crate::tool_mask::types::ToolMask) -> Self {
        self.tool_mask = Some(tool_mask);
        self
    }

    /// 验证配置
    pub fn validate(&self) -> Result<()> {
        // 验证名称
        if self.name.is_empty() {
            return Err(Error::AgentInvalidConfig(
                "Agent name cannot be empty".to_string(),
            ));
        }

        // 验证 LLM 配置
        self.llm_config.validate()?;

        // 验证系统提示词
        if self.system_prompt.is_empty() {
            return Err(Error::AgentInvalidConfig(
                "System prompt cannot be empty".to_string(),
            ));
        }

        // 验证 SubOrchestrator 的嵌套配置
        if self.role == AgentRole::SubOrchestrator {
            if self.nested_depth.is_none() {
                return Err(Error::AgentInvalidConfig(
                    "SubOrchestrator must have nested_depth set".to_string(),
                ));
            }
            if self.parent_orchestrator_id.is_none() {
                return Err(Error::AgentInvalidConfig(
                    "SubOrchestrator must have parent_orchestrator_id set".to_string(),
                ));
            }
        } else {
            // 非 SubOrchestrator 不应该有嵌套配置
            if self.nested_depth.is_some() {
                return Err(Error::AgentInvalidConfig(format!(
                    "Agent with role {:?} cannot have nested_depth",
                    self.role
                )));
            }
            if self.parent_orchestrator_id.is_some() {
                return Err(Error::AgentInvalidConfig(format!(
                    "Agent with role {:?} cannot have parent_orchestrator_id",
                    self.role
                )));
            }
        }

        Ok(())
    }
}

// ============================================================================
// AgentTask - Agent 任务
// ============================================================================

/// Agent 任务
///
/// 代表分配给 Agent 执行的任务。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    /// 目标 Agent ID
    pub agent_id: AgentId,
    /// 会话 ID
    pub session_id: Uuid,
    /// 用户消息（包含工单，如果是转交）
    pub user_message: String,
    /// 可用工具列表（可选）
    pub tools: Option<Vec<String>>,
    /// Checkpoint ID（可选）
    pub checkpoint_id: Option<String>,
}

// ============================================================================
// AgentTaskResult - Agent 任务结果
// ============================================================================

/// 工具调用记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// 工具名称
    pub tool_name: String,
    /// 工具参数
    pub arguments: serde_json::Value,
    /// 执行结果
    pub result: Option<serde_json::Value>,
    /// 是否成功
    pub success: bool,
    /// 错误信息（如果失败）
    pub error: Option<String>,
    /// 执行时间（毫秒）
    pub execution_time_ms: u64,
}

/// Agent 任务结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskResult {
    /// 是否成功
    pub success: bool,
    /// 执行任务的 Agent ID
    pub agent_id: AgentId,
    /// 会话 ID
    pub session_id: Uuid,
    /// Agent 响应
    pub response: String,
    /// 工具调用记录
    pub tool_calls: Vec<ToolCallRecord>,
    /// 错误信息（如果失败）
    pub error: Option<String>,
    /// 执行时间（毫秒）
    pub execution_time_ms: u64,
    /// 新的 Checkpoint ID（如果创建了）
    pub new_checkpoint_id: Option<String>,
}
