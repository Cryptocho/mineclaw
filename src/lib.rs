//! MineClaw - 轻量化 AI 编码助手
//!
//! 核心库模块

pub mod api;
pub mod checkpoint;
pub mod config;
pub mod encryption;
pub mod error;
pub mod llm;
pub mod mcp;
pub mod models;
pub mod state;
pub mod tool_coordinator;
pub mod tools;

pub use api::create_router;
pub use config::Config;
pub use error::{Error, Result};
pub use llm::create_provider;
pub use models::SessionRepository;
pub use state::AppState;
pub use tool_coordinator::ToolCoordinator;

// 方便的重导出
pub mod prelude {
    pub use crate::config::Config;
    pub use crate::error::{Error, Result};
    pub use crate::models::{Session, SessionRepository};
    pub use crate::state::AppState;
}
