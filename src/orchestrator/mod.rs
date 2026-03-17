//! Orchestrator 模块 - 多 Agent 系统的总控机制
//!
//! 提供总控的定义、创建、Agent 管理、任务分配和工单处理等功能。

pub mod executor;
pub mod task_manager;
pub mod types;

pub use executor::*;
pub use types::*;

// 从 task_manager 显式导出，避免与 types::TaskStatus 冲突
pub use task_manager::{SharedTaskManager, TaskInfo, TaskManager, TaskStatus as TaskManagerTaskStatus};
