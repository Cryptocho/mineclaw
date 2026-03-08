//! MineClaw 服务器入口点

use std::net::SocketAddr;

use agentfs::AgentFS;
use agentsql::SqlBackend;
use axum::serve;
use mineclaw::checkpoint::CheckpointManager;
use mineclaw::mcp::{McpServerManager, ToolExecutor};
use mineclaw::tools::{LocalToolRegistry, checkpoint::CheckpointTools, filesystem::FilesystemTool};
use mineclaw::{
    AppState, Config, SessionRepository, ToolCoordinator, create_provider, create_router,
};
use std::sync::Arc;
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

    // 初始化 AgentFS
    info!("Initializing AgentFS...");
    // 确保数据库文件的父目录存在
    if let Some(parent_dir) = std::path::Path::new(&config.agentfs_db_path).parent()
        && !parent_dir.exists() {
            std::fs::create_dir_all(parent_dir)?;
            info!("Created database directory: {:?}", parent_dir);
        }
    let backend = SqlBackend::sqlite(&config.agentfs_db_path).await?;
    let agent_fs = Arc::new(AgentFS::new(Box::new(backend), "mineclaw", "/mineclaw").await?);
    info!("AgentFS initialized successfully");

    // 初始化 CheckpointManager
    info!("Initializing CheckpointManager...");
    let checkpoint_manager = Arc::new(CheckpointManager::new(
        agent_fs.clone(),
        config.checkpoint.clone(),
    ));
    let checkpoint_manager_arc = checkpoint_manager.clone();
    info!("CheckpointManager initialized successfully");

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
                match mcp_server_manager.start_server(server_config).await {
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

    // 先保存服务器地址信息
    let server_host = config.server.host.clone();
    let server_port = config.server.port;

    // 初始化本地工具注册表
    let mut local_tool_registry = LocalToolRegistry::new();
    FilesystemTool::register_all(&mut local_tool_registry);
    CheckpointTools::register_all(&mut local_tool_registry);
    info!(
        "Local tools registered: {:?}",
        local_tool_registry
            .list_tools()
            .iter()
            .map(|t| t.name.clone())
            .collect::<Vec<_>>()
    );

    // 包装为 Arc
    let local_tool_registry_arc = std::sync::Arc::new(local_tool_registry);
    let config_arc = std::sync::Arc::new(config);

    // 创建 Arc<Mutex<McpServerManager>> 用于共享
    let mcp_server_manager_arc = std::sync::Arc::new(tokio::sync::Mutex::new(mcp_server_manager));

    // 创建 ToolCoordinator
    let tool_coordinator = ToolCoordinator::new(
        llm_provider.clone(),
        mcp_server_manager_arc.clone(),
        tool_executor.clone(),
        local_tool_registry_arc.clone(),
        config_arc.clone(),
    )
    .with_checkpoint_manager(checkpoint_manager_arc.clone());

    let app_state = AppState::new(
        session_repo,
        llm_provider,
        mcp_server_manager_arc,
        tool_executor,
        tool_coordinator,
        local_tool_registry_arc,
        config_arc,
        agent_fs,
        checkpoint_manager,
    );
    let app = create_router(app_state);

    let addr = SocketAddr::new(server_host.parse()?, server_port);
    let listener = TcpListener::bind(addr).await?;

    info!("MineClaw server listening on {}", addr);
    info!("Health check: http://{}/health", addr);

    serve(listener, app).await?;

    Ok(())
}
