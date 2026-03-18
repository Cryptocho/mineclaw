pub mod checkpoint;
pub mod message;
pub mod session;
pub mod sse;

pub use checkpoint::*;
pub use message::*;
pub use session::*;
pub use sse::*;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::error::Result;
use crate::checkpoint::CheckpointManager;

use agentfs::{AgentFS, KvStore};
use tracing::{debug, info, warn};

#[derive(Clone)]
pub struct SessionRepository {
    sessions: Arc<RwLock<HashMap<Uuid, Session>>>,
    agent_fs: Option<Arc<AgentFS>>,
    checkpoint_manager: Option<Arc<CheckpointManager>>,
}

impl std::fmt::Debug for SessionRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionRepository")
            .field("sessions", &self.sessions)
            .field("agent_fs", &"<AgentFS>")
            .finish()
    }
}

impl SessionRepository {
    /// 创建新的 SessionRepository（不带 agentfs，纯内存存储）
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            agent_fs: None,
            checkpoint_manager: None,
        }
    }

    /// 创建新的 SessionRepository（带 agentfs 持久化）
    pub async fn with_agent_fs(agent_fs: AgentFS) -> Result<Self> {
        let repo = Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            agent_fs: Some(Arc::new(agent_fs)),
            checkpoint_manager: None,
        };

        repo.load_sessions_from_agentfs().await?;

        Ok(repo)
    }

    /// 创建新的 SessionRepository（带 agentfs 和 checkpoint manager）
    pub async fn with_agent_fs_and_checkpoint(
        agent_fs: AgentFS,
        checkpoint_manager: CheckpointManager,
    ) -> Result<Self> {
        let repo = Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            agent_fs: Some(Arc::new(agent_fs)),
            checkpoint_manager: Some(Arc::new(checkpoint_manager)),
        };

        repo.load_sessions_from_agentfs().await?;

        Ok(repo)
    }

    /// 设置 CheckpointManager
    pub fn with_checkpoint_manager(mut self, checkpoint_manager: CheckpointManager) -> Self {
        self.checkpoint_manager = Some(Arc::new(checkpoint_manager));
        self
    }

    /// 从 agentfs 加载所有会话到内存
    async fn load_sessions_from_agentfs(&self) -> Result<()> {
        let Some(agent_fs) = &self.agent_fs else {
            return Ok(());
        };

        let session_keys = match agent_fs.kv.scan("sessions/").await {
            Ok(keys) => keys,
            Err(e) => {
                warn!(error = %e, "Failed to scan sessions from agentfs");
                return Ok(());
            }
        };

        let mut sessions = self.sessions.write().await;
        let mut loaded_count = 0;

        for key in session_keys {
            let session_data = match agent_fs.kv.get(&key).await {
                Ok(Some(data)) => data,
                Ok(None) => continue,
                Err(e) => {
                    warn!(key = %key, error = %e, "Failed to read session from agentfs");
                    continue;
                }
            };

            let session: Session = match serde_json::from_slice(&session_data) {
                Ok(s) => s,
                Err(e) => {
                    warn!(key = %key, error = %e, "Failed to deserialize session");
                    continue;
                }
            };

            sessions.insert(session.id, session);
            loaded_count += 1;
        }

        debug!(count = %loaded_count, "Loaded sessions from agentfs");
        Ok(())
    }

    /// 保存单个会话到 agentfs
    async fn save_session_to_agentfs(&self, session: &Session) {
        let Some(agent_fs) = &self.agent_fs else {
            return;
        };

        let key = format!("sessions/{}.json", session.id);
        let session_json = match serde_json::to_vec(session) {
            Ok(json) => json,
            Err(e) => {
                warn!(session_id = %session.id, error = %e, "Failed to serialize session");
                return;
            }
        };

        if let Err(e) = agent_fs.kv.set(&key, &session_json).await {
            warn!(session_id = %session.id, key = %key, error = %e, "Failed to save session to agentfs");
        }
    }

    /// 从 agentfs 删除会话
    async fn delete_session_from_agentfs(&self, session_id: &Uuid) {
        let Some(agent_fs) = &self.agent_fs else {
            return;
        };

        let key = format!("sessions/{}.json", session_id);

        if let Err(e) = agent_fs.kv.delete(&key).await {
            warn!(session_id = %session_id, key = %key, error = %e, "Failed to delete session from agentfs");
        }
    }

    pub async fn create(&self) -> Session {
        let mut session = Session::new();
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.id, session.clone());
        drop(sessions);

        // 如果有 CheckpointManager，创建初始 Checkpoint
        if let Some(checkpoint_manager) = &self.checkpoint_manager {
            if let Ok(checkpoint) = checkpoint_manager
                .create_checkpoint(
                    session.id,
                    Some("Initial checkpoint".to_string()),
                    None,
                    None,
                )
                .await
            {
                session.current_checkpoint_id = Some(checkpoint.id.clone());
                // 更新 Session
                let mut sessions = self.sessions.write().await;
                sessions.insert(session.id, session.clone());
                drop(sessions);
                self.save_session_to_agentfs(&session).await;
            }
        } else {
            self.save_session_to_agentfs(&session).await;
        }

        session
    }

    pub async fn get(&self, id: &Uuid) -> Option<Session> {
        let sessions = self.sessions.read().await;
        sessions.get(id).cloned()
    }

    pub async fn list(&self) -> Vec<SessionInfo> {
        let sessions = self.sessions.read().await;
        sessions.values().map(SessionInfo::from).collect()
    }

    /// 按状态过滤列出 Session
    pub async fn list_by_state(
        &self,
        state: crate::models::session::SessionState,
    ) -> Vec<SessionInfo> {
        let sessions = self.sessions.read().await;
        sessions
            .values()
            .filter(|s| s.state == state)
            .map(SessionInfo::from)
            .collect()
    }

    /// 按总控 ID 过滤列出 Session
    pub async fn list_by_orchestrator(
        &self,
        orchestrator_id: crate::orchestrator::OrchestratorId,
    ) -> Vec<SessionInfo> {
        let sessions = self.sessions.read().await;
        sessions
            .values()
            .filter(|s| s.orchestrator_id == Some(orchestrator_id))
            .map(SessionInfo::from)
            .collect()
    }

    /// 按时间范围过滤列出 Session
    pub async fn list_by_time_range(
        &self,
        created_after: Option<chrono::DateTime<chrono::Utc>>,
        created_before: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Vec<SessionInfo> {
        let sessions = self.sessions.read().await;
        sessions
            .values()
            .filter(|s| {
                let after_ok = created_after.is_none_or(|after| s.created_at >= after);
                let before_ok = created_before.is_none_or(|before| s.created_at <= before);
                after_ok && before_ok
            })
            .map(SessionInfo::from)
            .collect()
    }

    pub async fn update(&self, session: Session) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        if let std::collections::hash_map::Entry::Occupied(mut e) = sessions.entry(session.id) {
            e.insert(session.clone());
            drop(sessions);

            self.save_session_to_agentfs(&session).await;

            Ok(())
        } else {
            Err(crate::error::Error::SessionNotFound(session.id.to_string()))
        }
    }

    pub async fn delete(&self, id: &Uuid) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        if sessions.remove(id).is_some() {
            drop(sessions);

            // 清理相关的 checkpoints
            if let Some(checkpoint_manager) = &self.checkpoint_manager {
                match checkpoint_manager
                    .delete_all_checkpoints_for_session(id)
                    .await
                {
                    Ok(count) => {
                        info!(
                            session_id = %id,
                            checkpoint_count = %count,
                            "Cleaned up checkpoints for deleted session"
                        );
                    }
                    Err(e) => {
                        warn!(
                            session_id = %id,
                            error = %e,
                            "Failed to clean up checkpoints for deleted session"
                        );
                    }
                }
            }

            self.delete_session_from_agentfs(id).await;

            Ok(())
        } else {
            Err(crate::error::Error::SessionNotFound(id.to_string()))
        }
    }
}

impl Default for SessionRepository {
    fn default() -> Self {
        Self::new()
    }
}
