use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use crate::config::Config;
use crate::llm::LlmProvider;
use crate::mcp::{McpServerManager, ToolExecutor};
use crate::models::SessionRepository;
use crate::tool_coordinator::ToolCoordinator;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<RwLock<Config>>,
    pub session_repo: SessionRepository,
    pub llm_provider: Arc<RwLock<Arc<dyn LlmProvider>>>,
    pub mcp_server_manager: Arc<Mutex<McpServerManager>>,
    pub tool_executor: ToolExecutor,
    pub tool_coordinator: Arc<ToolCoordinator>,
}

impl AppState {
    pub fn new(
        config: Config,
        session_repo: SessionRepository,
        llm_provider: Arc<dyn LlmProvider>,
        mcp_server_manager: Arc<Mutex<McpServerManager>>,
        tool_executor: ToolExecutor,
        tool_coordinator: ToolCoordinator,
    ) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            session_repo,
            llm_provider: Arc::new(RwLock::new(llm_provider)),
            mcp_server_manager,
            tool_executor,
            tool_coordinator: Arc::new(tool_coordinator),
        }
    }
}
