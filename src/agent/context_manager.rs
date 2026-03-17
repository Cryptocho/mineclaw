//! 上下文管理器与裁剪策略
//!
//! 提供上下文裁剪、策略配置以及持续犯错检测功能。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

use crate::agent::context::{ContextChunk, ContextStore};
use crate::agent::work_order::WorkOrder;
use crate::error::{Error, Result};
use crate::models::Session;
use crate::orchestrator::types::{CmaNotification, CmaNotificationType};

// ============================================================================
// TrimmingTrigger - 裁剪触发条件
// ============================================================================

/// 裁剪触发条件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrimmingTrigger {
    /// 最大 Token 数（默认触发点，如 90% 的上下文长度）
    pub max_token_count: usize,
}

impl TrimmingTrigger {
    /// 创建新的裁剪触发条件
    pub fn new(max_token_count: usize) -> Self {
        Self { max_token_count }
    }
}

// ============================================================================
// TrimmingStrategy - 裁剪策略
// ============================================================================

/// 裁剪策略类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrimmingStrategyType {
    /// 先进先出，删除最早的
    Fifo,
    /// 基于重要性，删除最不重要的
    ImportanceBased,
    /// 混合策略，结合时间和重要性
    Hybrid,
}

/// 裁剪策略配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrimmingStrategy {
    /// 策略类型
    pub strategy_type: TrimmingStrategyType,
    /// 保留最近 N 个块（可选）
    pub keep_recent_n_chunks: Option<usize>,
    /// 是否保留重要块
    pub keep_important_chunks: bool,
    /// 最少保留块数
    pub min_chunks_to_keep: usize,
}

impl TrimmingStrategy {
    /// 创建默认裁剪策略
    pub fn new(strategy_type: TrimmingStrategyType) -> Self {
        Self {
            strategy_type,
            keep_recent_n_chunks: Some(10),
            keep_important_chunks: true,
            min_chunks_to_keep: 5,
        }
    }

    /// 执行裁剪逻辑
    ///
    /// # 参数
    /// * `chunks` - 原始上下文块列表
    /// * `target_token_count` - 裁剪后的目标 Token 总数
    ///
    /// # 返回
    /// 返回裁剪后的块列表和被删除的块列表
    pub fn trim(
        &self,
        chunks: Vec<ContextChunk>,
        target_token_count: usize,
    ) -> (Vec<ContextChunk>, Vec<ContextChunk>) {
        if chunks.is_empty()
            || chunks.iter().map(|c| c.token_count).sum::<usize>() <= target_token_count
        {
            return (chunks, Vec::new());
        }

        match self.strategy_type {
            TrimmingStrategyType::Fifo => self.trim_fifo(chunks, target_token_count),
            TrimmingStrategyType::ImportanceBased => {
                self.trim_importance(chunks, target_token_count)
            }
            TrimmingStrategyType::Hybrid => self.trim_hybrid(chunks, target_token_count),
        }
    }

    fn trim_fifo(
        &self,
        mut chunks: Vec<ContextChunk>,
        target_token_count: usize,
    ) -> (Vec<ContextChunk>, Vec<ContextChunk>) {
        let mut removed = Vec::new();

        // 按照时间顺序（索引 0 是最早的）尝试删除
        while chunks.len() > self.min_chunks_to_keep
            && chunks.iter().map(|c| c.token_count).sum::<usize>() > target_token_count
        {
            // 检查是否受 keep_important_chunks 保护
            if self.keep_important_chunks && chunks[0].is_important {
                // 如果是重要的，寻找第一个不重要的进行删除
                if let Some(index) = chunks.iter().position(|c| !c.is_important) {
                    removed.push(chunks.remove(index));
                } else {
                    // 全部都是重要的，打破循环避免由于无法删除而死循环
                    break;
                }
            } else {
                removed.push(chunks.remove(0));
            }
        }

        (chunks, removed)
    }

    fn trim_importance(
        &self,
        mut chunks: Vec<ContextChunk>,
        target_token_count: usize,
    ) -> (Vec<ContextChunk>, Vec<ContextChunk>) {
        let mut removed = Vec::new();

        // 按照优先级排序，删除优先级最低的（retention_priority 最小）
        while chunks.len() > self.min_chunks_to_keep
            && chunks.iter().map(|c| c.token_count).sum::<usize>() > target_token_count
        {
            // 找到优先级最低且不被 keep_important_chunks 保护的索引
            let mut best_index = None;
            let mut min_priority = u8::MAX;

            for (i, chunk) in chunks.iter().enumerate() {
                if self.keep_important_chunks && chunk.is_important {
                    continue;
                }
                if chunk.retention_priority < min_priority {
                    min_priority = chunk.retention_priority;
                    best_index = Some(i);
                }
            }

            if let Some(index) = best_index {
                removed.push(chunks.remove(index));
            } else {
                break;
            }
        }

        (chunks, removed)
    }

    fn trim_hybrid(
        &self,
        chunks: Vec<ContextChunk>,
        target_token_count: usize,
    ) -> (Vec<ContextChunk>, Vec<ContextChunk>) {
        // 混合策略：此处简化为 FIFO 逻辑，后续可扩展
        self.trim_fifo(chunks, target_token_count)
    }
}

// ============================================================================
// ContinuousMistakeDetector - 持续犯错检测器
// ============================================================================

/// 持续犯错检测器
#[derive(Debug, Clone)]
pub struct ContinuousMistakeDetector {
    /// 会话 ID
    pub session_id: Uuid,
    /// 时间窗口（秒）
    pub mistake_window_seconds: u64,
    /// 错误次数阈值
    pub mistake_threshold: u32,
    /// 最近的错误记录时间点
    pub recent_mistakes: Vec<DateTime<Utc>>,
}

impl ContinuousMistakeDetector {
    /// 创建新的持续犯错检测器
    pub fn new(session_id: Uuid, window_secs: u64, threshold: u32) -> Self {
        Self {
            session_id,
            mistake_window_seconds: window_secs,
            mistake_threshold: threshold,
            recent_mistakes: Vec::new(),
        }
    }

    /// 记录一次错误
    ///
    /// # 返回
    /// 返回是否达到持续犯错阈值
    pub fn record_mistake(&mut self) -> bool {
        let now = Utc::now();
        self.recent_mistakes.push(now);
        self.is_continuous_mistake()
    }

    /// 检查是否在持续犯错
    pub fn is_continuous_mistake(&mut self) -> bool {
        let now = Utc::now();
        let window = chrono::Duration::seconds(self.mistake_window_seconds as i64);

        // 清理窗口外的错误
        self.recent_mistakes
            .retain(|t| now.signed_duration_since(*t) <= window);

        self.recent_mistakes.len() >= self.mistake_threshold as usize
    }

    /// 重置记录
    pub fn reset(&mut self) {
        self.recent_mistakes.clear();
    }
}

// ============================================================================
// ContextManagerAgent - 上下文管理 Agent
// ============================================================================

/// 上下文管理 Agent
///
/// 负责监控和维护所有会话的上下文，处理裁剪和求助请求。
pub struct ContextManagerAgent {
    /// 上下文存储
    pub store: ContextStore,
    /// 默认裁剪策略
    pub default_strategy: TrimmingStrategy,
    /// 裁剪触发阈值（Token 数）
    pub global_max_tokens: usize,
}

impl ContextManagerAgent {
    /// 创建新的 ContextManagerAgent
    pub fn new(store: ContextStore, max_tokens: usize) -> Self {
        Self {
            store,
            default_strategy: TrimmingStrategy::new(TrimmingStrategyType::Fifo),
            global_max_tokens: max_tokens,
        }
    }

    /// 向会话添加上下文并监控限制
    pub async fn add_chunk_and_monitor(
        &self,
        chunk: ContextChunk,
        session: &Session,
    ) -> Result<Option<CmaNotification>> {
        let session_id = chunk.session_id;
        self.store.add_chunk(chunk).await;

        // 获取当前元数据以检查 Token 数
        if let Some(metadata) = self.store.get_metadata(&session_id).await {
            if metadata.total_token_count > self.global_max_tokens {
                info!(
                    session_id = %session_id,
                    current_tokens = %metadata.total_token_count,
                    limit = %self.global_max_tokens,
                    "Context limit exceeded, triggering auto-trim"
                );

                // 执行裁剪
                let chunks = self.store.get_chunks(&session_id).await;
                let target_tokens = (self.global_max_tokens as f64 * 0.8) as usize;
                let (remaining, _removed) = self.default_strategy.trim(chunks, target_tokens);

                let removed_count = metadata.chunk_count - remaining.len();
                self.store
                    .update_session_chunks(session_id, remaining)
                    .await;

                info!(
                    session_id = %session_id,
                    removed_chunks = %removed_count,
                    "Context trimmed successfully"
                );

                // 通知总控上下文已裁剪
                if let Some(orchestrator_id) = session.orchestrator_id {
                    return Ok(Some(CmaNotification::new(
                        CmaNotificationType::ContextTrimmed,
                        session_id,
                        orchestrator_id,
                        format!(
                            "Automatically trimmed {} chunks due to token limit",
                            removed_count
                        ),
                    )));
                }
            }
        }

        Ok(None)
    }

    /// 处理求助工单
    pub async fn handle_help_request(
        &self,
        work_order: WorkOrder,
        session: &Session,
    ) -> Result<CmaNotification> {
        let session_id = work_order.session_id;
        let orchestrator_id = session.orchestrator_id.ok_or_else(|| Error::Internal)?;

        info!(
            session_id = %session_id,
            work_order_id = %work_order.id(),
            "Processing help request in CMA"
        );

        // 将求助工单存入上下文
        self.store
            .add_chunk(ContextChunk::from_work_order(&work_order))
            .await;

        // 决策：对于求助，目前默认建议回退并转交
        // 未来可以根据内容、错误历史等进行更复杂的决策
        let mut notification = CmaNotification::new(
            CmaNotificationType::RollbackAndHandover,
            session_id,
            orchestrator_id,
            format!("Agent requested help: {}", work_order.title),
        );

        if let Some(checkpoint_id) = work_order.suggested_checkpoint_id {
            notification = notification.with_checkpoint_id(checkpoint_id);
        }

        Ok(notification)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::context::ContextChunkType;

    #[test]
    fn test_fifo_trimming() {
        let session_id = Uuid::new_v4();
        let chunks = vec![
            ContextChunk::new(session_id, "c1".into(), ContextChunkType::UserMessage, 10),
            ContextChunk::new(
                session_id,
                "c2".into(),
                ContextChunkType::AssistantMessage,
                10,
            ),
            ContextChunk::new(session_id, "c3".into(), ContextChunkType::UserMessage, 10),
        ];

        let strategy = TrimmingStrategy {
            strategy_type: TrimmingStrategyType::Fifo,
            keep_recent_n_chunks: None,
            keep_important_chunks: false,
            min_chunks_to_keep: 1,
        };

        let (trimmed, removed) = strategy.trim(chunks, 15);
        assert_eq!(trimmed.len(), 1);
        assert_eq!(removed.len(), 2);
        assert_eq!(trimmed[0].content, "c3");
    }

    #[test]
    fn test_importance_trimming() {
        let session_id = Uuid::new_v4();
        let chunks = vec![
            ContextChunk::new(session_id, "low".into(), ContextChunkType::UserMessage, 10)
                .with_priority(1),
            ContextChunk::new(session_id, "high".into(), ContextChunkType::UserMessage, 10)
                .with_priority(10),
            ContextChunk::new(session_id, "mid".into(), ContextChunkType::UserMessage, 10)
                .with_priority(5),
        ];

        let strategy = TrimmingStrategy {
            strategy_type: TrimmingStrategyType::ImportanceBased,
            keep_recent_n_chunks: None,
            keep_important_chunks: true,
            min_chunks_to_keep: 1,
        };

        let (trimmed, _removed) = strategy.trim(chunks, 15);
        assert_eq!(trimmed.len(), 1);
        assert_eq!(trimmed[0].content, "high");
    }

    #[test]
    fn test_mistake_detector() {
        let mut detector = ContinuousMistakeDetector::new(Uuid::new_v4(), 60, 3);

        assert!(!detector.record_mistake());
        assert!(!detector.record_mistake());
        assert!(detector.record_mistake());
    }
}
