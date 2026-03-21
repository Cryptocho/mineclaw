//! 上下文管理器
//!
//! 提供 CMA 上下文管理功能。
//!
//! CMA 直接执行回退，不通过通知机制：
//!
//! ## 两种触发路径
//!
//! **路径 1：Agent 主动求助（不经过 CMA）**
//! - Agent 意识到自己无法解决的问题 → 直接发求助工单给 Master
//! - Master 分析后决定如何处理
//!
//! **路径 2：上下文满载时 CMA 处理**
//! - 上下文超过阈值 → CMA 自动处理
//! - CMA 像编辑 JSON 一样编辑上下文：读取 → 分析 → 编辑
//!   - 如果分析时发现权限外的问题，通过 OrchestrationInterface 发工单给 Master
//!   - 完成后插入 trim_hint 通知 Agent

use tracing::info;

use crate::agent::context::{ContextChunk, ContextChunkType, ContextStore};
use crate::error::Result;
use crate::models::Session;

// ============================================================================
// CmaResult - CMA 操作结果
// ============================================================================

/// CMA 操作结果
///
/// 指示 CMA 处理后的下一步行动。
#[derive(Debug, Clone)]
pub enum CmaResult {
    /// CMA 已处理完成（裁剪）
    Handled,
    /// 上下文已被裁剪
    ContextTrimmed {
        removed_token_count: usize,
    },
}

// ============================================================================
// ContextManagerAgent - 上下文管理 Agent
// ============================================================================

/// 上下文管理 Agent
///
/// 负责监控和维护所有会话的上下文，处理裁剪。
///
/// 注意：CMA 分析上下文时如果发现 Agent 权限外的问题，
/// 应该通过 OrchestrationInterface::submit_help_work_order 发工单给 Master。
pub struct ContextManagerAgent {
    /// 上下文存储
    pub store: ContextStore,
    /// 裁剪触发阈值（Token 数）
    pub global_max_tokens: usize,
    /// 裁剪后注入的提示词
    pub trim_hint: String,
    /// 裁剪阈值（默认 0.6，即 60% 时触发裁剪）
    pub threshold: f64,
}

impl ContextManagerAgent {
    /// 创建新的 ContextManagerAgent
    pub fn new(store: ContextStore, max_tokens: usize) -> Self {
        Self {
            store,
            global_max_tokens: max_tokens,
            trim_hint: "注意：之前的对话上下文已被 CMA 裁剪以保持注意力专注".to_string(),
            threshold: 0.6,
        }
    }

    /// 创建新的 ContextManagerAgent（完整配置）
    pub fn with_config(
        store: ContextStore,
        max_tokens: usize,
        trim_hint: String,
        threshold: f64,
    ) -> Self {
        Self {
            store,
            global_max_tokens: max_tokens,
            trim_hint,
            threshold,
        }
    }

    /// 分析内容复杂度并返回调整后的阈值
    ///
    /// 当检测到任务复杂度高时，降低阈值以保留更多上下文
    #[allow(dead_code)]
    pub fn analyze_and_adjust_threshold(&self, chunks: &[ContextChunk]) -> f64 {
        if chunks.is_empty() {
            return self.threshold;
        }

        let mut complexity_score = 0.0;
        let mut help_request_count = 0;
        let mut tool_call_count = 0;

        for chunk in chunks {
            match chunk.chunk_type {
                ContextChunkType::HelpRequest => help_request_count += 1,
                ContextChunkType::ToolCall => tool_call_count += 1,
                _ => {}
            }

            if chunk.is_important {
                complexity_score += 1.0;
            }

            complexity_score += chunk.retention_priority as f64 / 10.0;
        }

        let chunk_count = chunks.len() as f64;
        complexity_score /= chunk_count.max(1.0);

        let mut adjusted_threshold = self.threshold;

        if help_request_count > 0 {
            adjusted_threshold = (adjusted_threshold + 0.15).min(0.9);
        }

        if tool_call_count > 3 {
            adjusted_threshold = (adjusted_threshold + 0.10).min(0.85);
        }

        if complexity_score > 0.7 {
            adjusted_threshold = (adjusted_threshold + 0.15).min(0.9);
        }

        adjusted_threshold
    }

    /// 向会话添加上下文并监控限制
    ///
    /// CMA 的主要入口：
    /// - 当上下文接近限制时，触发裁剪
    /// - CMA 像编辑 JSON 一样：读取 → 分析 → 编辑
    /// - 如果分析时发现权限外的问题，通过 OrchestrationInterface 发工单给 Master
    pub async fn add_chunk_and_monitor(
        &self,
        chunk: ContextChunk,
        _session: &Session,
    ) -> Result<CmaResult> {
        let session_id = chunk.session_id;
        self.store.add_chunk(chunk).await;

        let chunks = self.store.get_chunks(&session_id).await;
        let current_token_count = chunks.iter().map(|c| c.token_count).sum::<usize>();

        let trigger_threshold =
            (self.global_max_tokens as f64 * self.analyze_and_adjust_threshold(&chunks)) as usize;

        if current_token_count > trigger_threshold {
            info!(
                session_id = %session_id,
                current_tokens = %current_token_count,
                trigger_threshold = %trigger_threshold,
                "Context approaching limit, CMA trimming"
            );

            let target_tokens = (self.global_max_tokens as f64 * 0.8) as usize;
            let removed_count = current_token_count - target_tokens;

            let mut remaining = chunks;
            while remaining.iter().map(|c| c.token_count).sum::<usize>() > target_tokens
                && remaining.len() > 1
            {
                if remaining[0].is_important {
                    if let Some(pos) = remaining.iter().position(|c| !c.is_important) {
                        remaining.remove(pos);
                    } else {
                        break;
                    }
                } else {
                    remaining.remove(0);
                }
            }

            self.store
                .update_session_chunks(session_id, remaining.clone())
                .await;

            let trim_hint_chunk = ContextChunk::new(
                session_id,
                self.trim_hint.clone(),
                ContextChunkType::SystemNotification,
                self.trim_hint.len() / 4,
            );
            self.store.add_chunk(trim_hint_chunk).await;

            info!(
                session_id = %session_id,
                removed_tokens = %removed_count,
                "Context trimmed successfully"
            );

            return Ok(CmaResult::ContextTrimmed {
                removed_token_count: removed_count,
            });
        }

        Ok(CmaResult::Handled)
    }
}