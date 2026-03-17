use agentfs::AgentFsError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP request error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("JSON serialization error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("Walkdir error: {0}")]
    Walkdir(#[from] walkdir::Error),

    #[error("Address parse error: {0}")]
    AddrParse(#[from] std::net::AddrParseError),

    #[error("LLM error: {0}")]
    Llm(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Invalid session state: {0}")]
    SessionInvalidState(String),

    #[error("Message not found: {0}")]
    MessageNotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("MCP error: {0}")]
    Mcp(String),

    #[error("MCP server error: {server}: {message}")]
    McpServer { server: String, message: String },

    #[error("MCP tool not found: {0}")]
    McpToolNotFound(String),

    #[error("MCP tool execution error: {tool}: {message}")]
    McpToolExecution { tool: String, message: String },

    #[error("Filesystem error: {0}")]
    Filesystem(String),

    #[error("Path not allowed: {0}")]
    PathNotAllowed(String),

    #[error("Path traversal detected: {0}")]
    PathTraversal(String),

    #[error("File too large: {0} bytes (max: {1} bytes)")]
    FileTooLarge(usize, usize),

    #[error("Local tool not found: {0}")]
    LocalToolNotFound(String),

    #[error("Local tool execution error: {tool}: {message}")]
    LocalToolExecution { tool: String, message: String },

    #[error("Confirmation required for tool {tool}: {message}")]
    ConfirmationRequired { tool: String, message: String },

    #[error("Checkpoint error: {0}")]
    Checkpoint(String),

    #[error("Checkpoint not found: {0}")]
    CheckpointNotFound(String),

    #[error("Checkpoint already exists: {0}")]
    CheckpointAlreadyExists(String),

    #[error("Checkpoint limit reached for session: {0} (max: {1})")]
    CheckpointLimitReached(String, usize),

    #[error("AgentFS error: {0}")]
    AgentFS(String),

    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    #[error("Invalid agent configuration: {0}")]
    AgentInvalidConfig(String),

    #[error("Agent execution error: {0}")]
    AgentExecution(String),

    #[error("Work order error: {0}")]
    WorkOrder(String),

    #[error("Tool mask not found: {0}")]
    ToolMaskNotFound(String),

    #[error("Internal server error")]
    Internal,
}

// From trait implementations for CheckpointError
impl From<crate::checkpoint::CheckpointError> for Error {
    fn from(err: crate::checkpoint::CheckpointError) -> Self {
        match err {
            crate::checkpoint::CheckpointError::NotFound(id) => Error::CheckpointNotFound(id),
            crate::checkpoint::CheckpointError::SessionNotFound(id) => Error::SessionNotFound(id),
            crate::checkpoint::CheckpointError::LimitReached(msg) => Error::Checkpoint(msg),
            crate::checkpoint::CheckpointError::AgentFS(msg) => Error::AgentFS(msg),
            crate::checkpoint::CheckpointError::Serialization(e) => Error::SerdeJson(e),
            crate::checkpoint::CheckpointError::Io(e) => Error::Io(e),
            crate::checkpoint::CheckpointError::InvalidData(msg) => Error::Checkpoint(msg),
        }
    }
}

// From trait implementation for AgentFsError
impl From<AgentFsError> for Error {
    fn from(err: AgentFsError) -> Self {
        Error::AgentFS(err.to_string())
    }
}

// From trait implementation for SqlError
impl From<agentsql::SqlError> for Error {
    fn from(err: agentsql::SqlError) -> Self {
        Error::AgentFS(err.to_string())
    }
}

impl axum::response::IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let status = match &self {
            Error::Config(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Error::Io(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Error::Reqwest(_) => axum::http::StatusCode::BAD_GATEWAY,
            Error::SerdeJson(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Error::Regex(_) => axum::http::StatusCode::BAD_REQUEST,
            Error::Walkdir(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Error::AddrParse(_) => axum::http::StatusCode::BAD_REQUEST,
            Error::Llm(_) => axum::http::StatusCode::BAD_GATEWAY,
            Error::SessionNotFound(_) => axum::http::StatusCode::NOT_FOUND,
            Error::SessionInvalidState(_) => axum::http::StatusCode::BAD_REQUEST,
            Error::MessageNotFound(_) => axum::http::StatusCode::NOT_FOUND,
            Error::InvalidInput(_) => axum::http::StatusCode::BAD_REQUEST,
            Error::InvalidConfig(_) => axum::http::StatusCode::BAD_REQUEST,
            Error::Mcp(_) => axum::http::StatusCode::BAD_GATEWAY,
            Error::McpServer { .. } => axum::http::StatusCode::BAD_GATEWAY,
            Error::McpToolNotFound(_) => axum::http::StatusCode::NOT_FOUND,
            Error::McpToolExecution { .. } => axum::http::StatusCode::BAD_GATEWAY,
            Error::Filesystem(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Error::PathNotAllowed(_) => axum::http::StatusCode::FORBIDDEN,
            Error::PathTraversal(_) => axum::http::StatusCode::FORBIDDEN,
            Error::FileTooLarge(_, _) => axum::http::StatusCode::PAYLOAD_TOO_LARGE,
            Error::LocalToolNotFound(_) => axum::http::StatusCode::NOT_FOUND,
            Error::LocalToolExecution { .. } => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Error::ConfirmationRequired { .. } => axum::http::StatusCode::BAD_REQUEST,
            Error::Checkpoint(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Error::CheckpointNotFound(_) => axum::http::StatusCode::NOT_FOUND,
            Error::CheckpointAlreadyExists(_) => axum::http::StatusCode::CONFLICT,
            Error::CheckpointLimitReached(_, _) => axum::http::StatusCode::BAD_REQUEST,
            Error::AgentFS(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Error::AgentNotFound(_) => axum::http::StatusCode::NOT_FOUND,
            Error::AgentInvalidConfig(_) => axum::http::StatusCode::BAD_REQUEST,
            Error::AgentExecution(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Error::WorkOrder(_) => axum::http::StatusCode::BAD_REQUEST,
            Error::ToolMaskNotFound(_) => axum::http::StatusCode::NOT_FOUND,
            Error::Internal => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        };

        let message = self.to_string();

        let body = axum::Json(serde_json::json!({
            "error": message
        }));

        (status, body).into_response()
    }
}
