use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP request error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("JSON serialization error: {0}")]
    SerdeJson(#[from] serde_json::Error),

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

    #[error("Internal server error")]
    Internal,
}

impl axum::response::IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let status = match &self {
            Error::Config(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Error::ConfigError(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Error::Io(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Error::Reqwest(_) => axum::http::StatusCode::BAD_GATEWAY,
            Error::SerdeJson(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Error::AddrParse(_) => axum::http::StatusCode::BAD_REQUEST,
            Error::Llm(_) => axum::http::StatusCode::BAD_GATEWAY,
            Error::SessionNotFound(_) => axum::http::StatusCode::NOT_FOUND,
            Error::MessageNotFound(_) => axum::http::StatusCode::NOT_FOUND,
            Error::InvalidInput(_) => axum::http::StatusCode::BAD_REQUEST,
            Error::Mcp(_) => axum::http::StatusCode::BAD_GATEWAY,
            Error::McpServer { .. } => axum::http::StatusCode::BAD_GATEWAY,
            Error::McpToolNotFound(_) => axum::http::StatusCode::NOT_FOUND,
            Error::McpToolExecution { .. } => axum::http::StatusCode::BAD_GATEWAY,
            Error::Internal => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        };

        let message = self.to_string();

        let body = axum::Json(serde_json::json!({
            "error": message
        }));

        (status, body).into_response()
    }
}
