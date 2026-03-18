//! Orchestrator 模块 - 多 Agent 系统的总控机制
//!
//! 提供总控的定义、创建、Agent 管理、任务分配和工单处理等功能。

pub mod executor;
pub mod prompt_template;
pub mod task_manager;
pub mod types;

pub use executor::*;
pub use prompt_template::*;
pub use types::*;

// 从 task_manager 导出
pub use task_manager::{SharedTaskManager, TaskInfo, TaskManager};
