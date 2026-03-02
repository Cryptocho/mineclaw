pub mod message;
pub mod session;

pub use message::*;
pub use session::*;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::error::Result;

#[derive(Debug, Clone)]
pub struct SessionRepository {
    sessions: Arc<RwLock<HashMap<Uuid, Session>>>,
}

impl SessionRepository {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create(&self) -> Session {
        let session = Session::new();
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.id, session.clone());
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

    pub async fn update(&self, session: Session) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        if let std::collections::hash_map::Entry::Occupied(mut e) = sessions.entry(session.id) {
            e.insert(session);
            Ok(())
        } else {
            Err(crate::error::Error::SessionNotFound(session.id.to_string()))
        }
    }

    pub async fn delete(&self, id: &Uuid) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        if sessions.remove(id).is_some() {
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
