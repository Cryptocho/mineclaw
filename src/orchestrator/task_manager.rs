//! Task Manager - 任务生命周期管理
//!
//! 负责管理任务的注册、状态跟踪、结果收集和清理。

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::agent::{AgentId, AgentTaskResult};
use crate::error::{Error, Result};

use super::types::{TaskId, TaskStatus};

// ==================== 任务状态辅助方法 ====================

/// 检查任务是否处于终态
fn is_terminal_status(status: &TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
    )
}

// ==================== 任务信息 ====================

/// 任务信息 - 包含任务的完整状态和结果
#[derive(Debug, Clone)]
pub struct TaskInfo {
    /// 任务 ID
    pub task_id: TaskId,
    /// 关联的 Agent ID
    pub agent_id: AgentId,
    /// 当前状态
    pub status: TaskStatus,
    /// 任务结果（仅在 Completed/Failed 时存在）
    pub result: Option<AgentTaskResult>,
    /// 错误信息（仅在 Failed 时存在）
    pub error: Option<String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 开始时间（如果已开始）
    pub started_at: Option<DateTime<Utc>>,
    /// 完成时间（如果已完成）
    pub completed_at: Option<DateTime<Utc>>,
}

impl TaskInfo {
    /// 创建新的待处理任务
    pub fn new(task_id: TaskId, agent_id: AgentId) -> Self {
        Self {
            task_id,
            agent_id,
            status: TaskStatus::Pending,
            result: None,
            error: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
        }
    }

    /// 标记任务为运行中
    pub fn mark_running(&mut self) {
        if !is_terminal_status(&self.status) {
            self.status = TaskStatus::Running;
            self.started_at = Some(Utc::now());
        }
    }

    /// 标记任务为已完成
    pub fn mark_completed(&mut self, result: AgentTaskResult) {
        if !is_terminal_status(&self.status) {
            self.status = TaskStatus::Completed;
            self.result = Some(result);
            self.completed_at = Some(Utc::now());
        }
    }

    /// 标记任务为失败
    pub fn mark_failed(&mut self, error: String) {
        if !is_terminal_status(&self.status) {
            self.status = TaskStatus::Failed;
            self.error = Some(error);
            self.completed_at = Some(Utc::now());
        }
    }

    /// 标记任务为已取消
    pub fn mark_cancelled(&mut self) {
        if !is_terminal_status(&self.status) {
            self.status = TaskStatus::Cancelled;
            self.completed_at = Some(Utc::now());
        }
    }
}

// ==================== Task Manager ====================

/// Task Manager - 任务生命周期管理器
///
/// 负责管理任务的注册、状态跟踪、结果收集和清理。
#[derive(Debug)]
pub struct TaskManager {
    /// 任务信息映射
    tasks: HashMap<TaskId, TaskInfo>,
    /// 活跃任务的 JoinHandle
    join_handles: HashMap<TaskId, JoinHandle<Result<AgentTaskResult>>>,
    /// 按 Agent ID 索引的任务列表
    tasks_by_agent: HashMap<AgentId, Vec<TaskId>>,
}

impl TaskManager {
    /// 创建新的 TaskManager
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            join_handles: HashMap::new(),
            tasks_by_agent: HashMap::new(),
        }
    }

    // ==================== 基本查询方法 ====================

    /// 获取任务信息
    pub fn get_task(&self, task_id: &TaskId) -> Option<&TaskInfo> {
        self.tasks.get(task_id)
    }

    /// 获取任务信息（可变引用）
    pub fn get_task_mut(&mut self, task_id: &TaskId) -> Option<&mut TaskInfo> {
        self.tasks.get_mut(task_id)
    }

    /// 获取任务状态
    pub fn get_task_status(&self, task_id: &TaskId) -> Option<TaskStatus> {
        self.tasks.get(task_id).map(|t| t.status.clone())
    }

    /// 检查任务是否存在
    pub fn contains_task(&self, task_id: &TaskId) -> bool {
        self.tasks.contains_key(task_id)
    }

    /// 获取指定 Agent 的所有任务
    pub fn get_tasks_for_agent(&self, agent_id: &AgentId) -> Vec<&TaskInfo> {
        self.tasks_by_agent
            .get(agent_id)
            .map(|task_ids| {
                task_ids
                    .iter()
                    .filter_map(|id| self.tasks.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 列出所有任务
    pub fn list_tasks(&self) -> Vec<&TaskInfo> {
        self.tasks.values().collect()
    }

    /// 获取任务总数
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    // ==================== 任务注册方法 ====================

    /// 注册新任务
    ///
    /// 创建新的任务信息并添加到管理器中。
    pub fn register_task(&mut self, task_id: TaskId, agent_id: AgentId) -> Result<&TaskInfo> {
        if self.tasks.contains_key(&task_id) {
            return Err(Error::InvalidConfig(format!(
                "Task {} already exists",
                task_id
            )));
        }

        let task_info = TaskInfo::new(task_id, agent_id);
        self.tasks.insert(task_id, task_info);

        // 更新按 Agent 索引的任务列表
        self.tasks_by_agent
            .entry(agent_id)
            .or_default()
            .push(task_id);

        Ok(self.tasks.get(&task_id).unwrap())
    }

    // ==================== 状态更新方法 ====================

    /// 更新任务状态
    pub fn update_task_status(&mut self, task_id: &TaskId, status: TaskStatus) -> Result<()> {
        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| Error::InvalidConfig(format!("Task {} not found", task_id)))?;

        if is_terminal_status(&task.status) {
            return Err(Error::InvalidConfig(format!(
                "Cannot update status of terminal task {}",
                task_id
            )));
        }

        match status {
            TaskStatus::Running => task.mark_running(),
            TaskStatus::Completed => {
                return Err(Error::InvalidConfig(
                    "Use store_task_result() to mark task as completed".to_string(),
                ));
            }
            TaskStatus::Failed => {
                return Err(Error::InvalidConfig(
                    "Use store_task_result() to mark task as failed".to_string(),
                ));
            }
            TaskStatus::Cancelled => task.mark_cancelled(),
            TaskStatus::Pending => {}
        }

        Ok(())
    }

    /// 存储任务结果
    pub fn store_task_result(
        &mut self,
        task_id: &TaskId,
        result: Result<AgentTaskResult>,
    ) -> Result<()> {
        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| Error::InvalidConfig(format!("Task {} not found", task_id)))?;

        if is_terminal_status(&task.status) {
            return Err(Error::InvalidConfig(format!(
                "Cannot store result for terminal task {}",
                task_id
            )));
        }

        match result {
            Ok(result) => task.mark_completed(result),
            Err(e) => task.mark_failed(e.to_string()),
        }

        Ok(())
    }

    // ==================== JoinHandle 管理方法 ====================

    /// 注册任务的 JoinHandle
    pub fn register_join_handle(
        &mut self,
        task_id: TaskId,
        handle: JoinHandle<Result<AgentTaskResult>>,
    ) -> Result<()> {
        if !self.tasks.contains_key(&task_id) {
            return Err(Error::InvalidConfig(format!(
                "Task {} not found, cannot register join handle",
                task_id
            )));
        }

        self.join_handles.insert(task_id, handle);
        Ok(())
    }

    /// 获取任务的 JoinHandle（如果存在且活跃）
    pub fn get_join_handle(
        &mut self,
        task_id: &TaskId,
    ) -> Option<&mut JoinHandle<Result<AgentTaskResult>>> {
        self.join_handles.get_mut(task_id)
    }

    /// 检查任务是否有活跃的 JoinHandle
    pub fn has_active_join_handle(&self, task_id: &TaskId) -> bool {
        self.join_handles.contains_key(task_id)
    }

    // ==================== 任务等待方法 ====================

    /// 等待单个任务完成
    ///
    /// 如果任务已经完成，直接返回结果。
    /// 如果任务还在运行，等待它完成。
    pub async fn wait_for_task(&mut self, task_id: &TaskId) -> Result<AgentTaskResult> {
        // 首先检查任务是否已经完成
        if let Some(task) = self.tasks.get(task_id) {
            match &task.status {
                TaskStatus::Completed => {
                    if let Some(result) = task.result.clone() {
                        return Ok(result);
                    }
                }
                TaskStatus::Failed => {
                    if let Some(error) = &task.error {
                        return Err(Error::AgentExecution(error.clone()));
                    }
                }
                TaskStatus::Cancelled => {
                    return Err(Error::AgentExecution(format!(
                        "Task {} was cancelled",
                        task_id
                    )));
                }
                _ => {}
            }
        }

        // 任务还在运行，等待 JoinHandle
        let handle = self.join_handles.remove(task_id).ok_or_else(|| {
            Error::InvalidConfig(format!("Task {} not found or not running", task_id))
        })?;

        // handle.await 返回 Result<Result<AgentTaskResult, Error>, JoinError>
        let join_result = handle.await.map_err(|e| {
            Error::AgentExecution(format!("Task {} failed to join: {}", task_id, e))
        })?;

        // 现在我们需要存储结果并返回
        match join_result {
            Ok(task_result) => {
                // 成功的情况：clone 结果，存储，然后返回
                let result_clone = task_result.clone();
                self.store_task_result(task_id, Ok(task_result))?;
                Ok(result_clone)
            }
            Err(e) => {
                // 失败的情况：把错误转换成字符串，存储，然后返回新的错误
                let error_str = e.to_string();
                self.store_task_result(task_id, Err(e))?;
                Err(Error::AgentExecution(error_str))
            }
        }
    }

    /// 等待所有任务完成
    ///
    /// 返回所有任务的结果。
    pub async fn wait_for_all_tasks(&mut self) -> Vec<(TaskId, Result<AgentTaskResult>)> {
        let task_ids: Vec<TaskId> = self.join_handles.keys().cloned().collect();
        let mut results = Vec::with_capacity(task_ids.len());

        for task_id in task_ids {
            let result = self.wait_for_task(&task_id).await;
            results.push((task_id, result));
        }

        results
    }

    // ==================== 任务管理方法 ====================

    /// 取消任务
    pub fn cancel_task(&mut self, task_id: &TaskId) -> Result<()> {
        // 首先尝试 abort JoinHandle（如果存在）
        if let Some(handle) = self.join_handles.remove(task_id) {
            handle.abort();
        }

        // 更新任务状态
        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| Error::InvalidConfig(format!("Task {} not found", task_id)))?;

        task.mark_cancelled();

        Ok(())
    }

    /// 清理已完成的任务
    ///
    /// 从管理器中移除已处于终态的任务。
    /// 返回被清理的任务数量。
    pub fn cleanup_completed_tasks(&mut self) -> usize {
        let tasks_to_remove: Vec<TaskId> = self
            .tasks
            .iter()
            .filter(|(_, task)| is_terminal_status(&task.status))
            .map(|(id, _)| *id)
            .collect();

        let count = tasks_to_remove.len();

        for task_id in tasks_to_remove {
            self.remove_task(&task_id);
        }

        count
    }

    /// 移除任务（内部方法）
    fn remove_task(&mut self, task_id: &TaskId) {
        // 从 tasks 中移除
        if let Some(task) = self.tasks.remove(task_id) {
            // 从 join_handles 中移除
            self.join_handles.remove(task_id);

            // 从 tasks_by_agent 中移除
            if let Some(task_ids) = self.tasks_by_agent.get_mut(&task.agent_id) {
                task_ids.retain(|id| id != task_id);
                if task_ids.is_empty() {
                    self.tasks_by_agent.remove(&task.agent_id);
                }
            }
        }
    }
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 线程安全的 TaskManager 包装器
pub type SharedTaskManager = Arc<Mutex<TaskManager>>;
