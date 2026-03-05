use std::sync::Arc;
use tokio::sync::Mutex;

use crate::llm::LlmProvider;
use crate::mcp::{McpServerManager, ToolExecutor};
use crate::models::SessionRepository;
use crate::tool_coordinator::ToolCoordinator;

#[derive(Clone)]
pub struct AppState {
    pub session_repo: SessionRepository,
    pub llm_provider: Arc<dyn LlmProvider>,
    pub mcp_server_manager: Arc<Mutex<McpServerManager>>,
    pub tool_executor: ToolExecutor,
    pub tool_coordinator: Arc<ToolCoordinator>,
}

impl AppState {
    pub fn new(
        session_repo: SessionRepository,
        llm_provider: Arc<dyn LlmProvider>,
        mcp_server_manager: Arc<Mutex<McpServerManager>>,
        tool_executor: ToolExecutor,
        tool_coordinator: ToolCoordinator,
    ) -> Self {
        Self {
            session_repo,
            llm_provider,
            mcp_server_manager,
            tool_executor,
            tool_coordinator: Arc::new(tool_coordinator),
        }
    }
}
