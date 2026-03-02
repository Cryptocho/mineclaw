use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::message::Message;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub messages: Vec<Message>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Session {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        self.updated_at = Utc::now();
    }

    pub fn insert_metadata<K, V>(&mut self, key: K, value: V)
    where
        K: Into<String>,
        V: Into<serde_json::Value>,
    {
        self.metadata.insert(key.into(), value.into());
        self.updated_at = Utc::now();
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Serialize)]
pub struct SessionInfo {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: usize,
}

impl From<&Session> for SessionInfo {
    fn from(session: &Session) -> Self {
        Self {
            id: session.id,
            created_at: session.created_at,
            updated_at: session.updated_at,
            message_count: session.messages.len(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ListSessionsResponse {
    pub sessions: Vec<SessionInfo>,
}
