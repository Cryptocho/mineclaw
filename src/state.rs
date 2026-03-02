use std::sync::Arc;

use crate::llm::LlmProvider;
use crate::models::SessionRepository;

#[derive(Clone)]
pub struct AppState {
    pub session_repo: SessionRepository,
    pub llm_provider: Arc<dyn LlmProvider>,
}

impl AppState {
    pub fn new(session_repo: SessionRepository, llm_provider: Arc<dyn LlmProvider>) -> Self {
        Self {
            session_repo,
            llm_provider,
        }
    }
}
