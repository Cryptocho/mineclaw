//! MineClaw 服务器入口点

use std::net::SocketAddr;

use axum::serve;
use mineclaw::mcp::{McpServerManager, ToolExecutor};
use mineclaw::{
    AppState, Config, SessionRepository, ToolCoordinator, create_provider, create_router,
};
use tokio::net::TcpListener;
use tracing::{info, level_filters::LevelFilter, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> mineclaw::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let config = Config::load()?;
    info!("Configuration loaded successfully");

    let session_repo = SessionRepository::new();
    let llm_provider = create_provider(config.llm.clone());

    // 初始化 MCP 服务器管理器
    let mut mcp_server_manager = McpServerManager::new();

    // 启动配置的 MCP 服务器
    if let Some(mcp_config) = &config.mcp {
        if mcp_config.enabled {
            info!(
                "MCP is enabled, starting {} servers",
                mcp_config.servers.len()
            );
            for server_config in &mcp_config.servers {
                match mcp_server_manager.start_server(server_config.clone()).await {
                    Ok(_) => {
                        info!("Successfully started MCP server: {}", server_config.name);
                    }
                    Err(e) => {
                        warn!("Failed to start MCP server {}: {}", server_config.name, e);
                    }
                }
            }
        } else {
            info!("MCP is disabled in config");
        }
    }

    let tool_executor = ToolExecutor::new();

    // 创建 Arc<Mutex<McpServerManager>> 用于共享
    let mcp_server_manager_arc = std::sync::Arc::new(tokio::sync::Mutex::new(mcp_server_manager));

    // 创建 ToolCoordinator
    let tool_coordinator = ToolCoordinator::new(
        llm_provider.clone(),
        mcp_server_manager_arc.clone(),
        tool_executor.clone(),
    );

    let app_state = AppState::new(
        config.clone(),
        session_repo,
        llm_provider,
        mcp_server_manager_arc,
        tool_executor,
        tool_coordinator,
    );
    let app = create_router(app_state);

    let addr = SocketAddr::new(config.server.host.parse()?, config.server.port);
    let listener = TcpListener::bind(addr).await?;

    info!("MineClaw server listening on {}", addr);
    info!("Health check: http://{}/health", addr);

    serve(listener, app).await?;

    Ok(())
}
