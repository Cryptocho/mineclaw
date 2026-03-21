use std::sync::Arc;
use tokio::sync::Mutex;

use crate::checkpoint::CheckpointManager;
use crate::config::Config;
use crate::llm::{LlmProvider, LlmProviderRegistry};
use crate::mcp::{McpServerManager, ToolExecutor};
use crate::models::SessionRepository;
use crate::tool_coordinator::ToolCoordinator;
use crate::tools::LocalToolRegistry;
use agentfs::AgentFS;

use crate::orchestrator::executor::OrchestratorExecutor;
use crate::orchestrator::task_manager::SharedTaskManager;
#[derive(Clone)]
pub struct AppState {
    pub session_repo: Arc<SessionRepository>,
    pub provider_registry: Arc<LlmProviderRegistry>,
    pub mcp_server_manager: Arc<Mutex<McpServerManager>>,
    pub tool_executor: ToolExecutor,
    pub tool_coordinator: Arc<ToolCoordinator>,
    pub local_tool_registry: Arc<LocalToolRegistry>,
    pub config: Arc<Config>,
    pub agent_fs: Arc<AgentFS>,
    pub checkpoint_manager: Arc<CheckpointManager>,

    // Phase 4 components
    pub orchestrator_executor: Arc<OrchestratorExecutor>,
    pub task_manager: SharedTaskManager,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_repo: Arc<SessionRepository>,
        provider_registry: Arc<LlmProviderRegistry>,
        mcp_server_manager: Arc<Mutex<McpServerManager>>,
        tool_executor: ToolExecutor,
        tool_coordinator: ToolCoordinator,
        local_tool_registry: Arc<LocalToolRegistry>,
        config: Arc<Config>,
        agent_fs: Arc<AgentFS>,
        checkpoint_manager: Arc<CheckpointManager>,
        orchestrator_executor: Arc<OrchestratorExecutor>,
        task_manager: SharedTaskManager,
    ) -> Self {
        Self {
            session_repo,
            provider_registry,
            mcp_server_manager,
            tool_executor,
            tool_coordinator: Arc::new(tool_coordinator),
            local_tool_registry,
            config,
            agent_fs,
            checkpoint_manager,
            orchestrator_executor,
            task_manager,
        }
    }

    /// 获取默认的 LLM 提供者（向后兼容）
    pub fn default_llm_provider(&self) -> Arc<dyn LlmProvider> {
        self.provider_registry.default_provider()
    }
}
