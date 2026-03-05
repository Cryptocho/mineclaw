use thiserror::Error;
use std::path::StripPrefixError;

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

    #[error("Message not found: {0}")]
    MessageNotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

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

    #[error("Checkpoint not found: {0}")]
    CheckpointNotFound(String),

    #[error("Checkpoint restore failed: {0}")]
    CheckpointRestoreFailed(String),

    #[error("Path prefix error: {0}")]
    PathPrefix(#[from] StripPrefixError),

    #[error("Internal server error")]
    Internal,
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
            Error::MessageNotFound(_) => axum::http::StatusCode::NOT_FOUND,
            Error::InvalidInput(_) => axum::http::StatusCode::BAD_REQUEST,
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
            Error::CheckpointNotFound(_) => axum::http::StatusCode::NOT_FOUND,
            Error::CheckpointRestoreFailed(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Error::PathPrefix(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Error::Internal => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        };

        let message = self.to_string();

        let body = axum::Json(serde_json::json!({
            "error": message
        }));

        (status, body).into_response()
    }
}
