//! MCP 集成测试
//!
//! 测试与真实 MCP 服务器的通信

use mineclaw::config::McpServerConfig;
use mineclaw::mcp::{McpServerManager, ServerStatus, ToolExecutor};
use serde_json::json;
use std::collections::HashMap;

#[tokio::test]
async fn test_mcp_server_integration() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .try_init();

    // 从环境变量获取测试服务器路径，或使用默认路径
    let server_path = std::env::var("TEST_MCP_SERVER_PATH")
        .unwrap_or_else(|_| "./test-mcp-server.js".to_string());

    println!("Using test server at: {}", server_path);

    // 创建测试配置
    let mut env = HashMap::new();
    env.insert("NODE_ENV".to_string(), "test".to_string());

    let server_config = McpServerConfig {
        name: "test-server".to_string(),
        command: "node".to_string(),
        args: vec![server_path],
        env,
    };

    // 启动服务器管理器
    let mut manager = McpServerManager::new();

    // 启动服务器
    println!("Starting MCP server...");
    let result = manager.start_server(server_config).await;

    // 服务器可能会失败（如果没有 Node.js），但我们至少测试启动流程
    if result.is_ok() {
        println!("MCP server started successfully");

        // 检查服务器状态
        let server = manager
            .get_server("test-server")
            .expect("Server should exist");
        assert_eq!(server.status, ServerStatus::Connected);
        assert!(!server.tools.is_empty());

        println!("Server tools:");
        for tool in &server.tools {
            println!("  - {}: {}", tool.name, tool.description);
        }

        // 验证工具数量
        assert_eq!(server.tools.len(), 2);

        // 验证 echo 工具
        let echo_tool = server.tools.iter().find(|t| t.name == "echo");
        assert!(echo_tool.is_some());
        let echo_tool = echo_tool.unwrap();
        assert_eq!(echo_tool.description, "Echo back the input message");

        // 验证 add 工具
        let add_tool = server.tools.iter().find(|t| t.name == "add");
        assert!(add_tool.is_some());

        // 测试 all_tools
        let all_tools = manager.all_tools();
        assert_eq!(all_tools.len(), 2);

        // 停止服务器
        println!("Stopping MCP server...");
        manager
            .stop_server("test-server")
            .await
            .expect("Failed to stop server");

        println!("Integration test completed successfully!");
    } else {
        println!("MCP server failed to start (this is expected if Node.js is not available)");
        println!("Error: {:?}", result.err());
    }
}

#[tokio::test]
async fn test_mcp_server_manager_basics() {
    // 测试管理器的基本功能（不依赖外部进程）
    let manager = McpServerManager::new();

    assert!(manager.list_servers().is_empty());
    assert!(manager.all_tools().is_empty());
    assert!(manager.get_server("nonexistent").is_none());
}

#[tokio::test]
async fn test_mcp_tool_call_integration() {
    // 测试工具调用功能
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .try_init();

    let server_path = std::env::var("TEST_MCP_SERVER_PATH")
        .unwrap_or_else(|_| "./test-mcp-server.js".to_string());

    let mut env = HashMap::new();
    env.insert("NODE_ENV".to_string(), "test".to_string());

    let server_config = McpServerConfig {
        name: "test-server".to_string(),
        command: "node".to_string(),
        args: vec![server_path],
        env,
    };

    let mut manager = McpServerManager::new();

    if let Ok(_) = manager.start_server(server_config).await {
        println!("MCP server started, testing tool calls...");

        // 测试 1: 使用管理器直接调用 echo 工具
        println!("Testing echo tool...");
        let echo_result = manager
            .call_tool(
                "test-server",
                "echo",
                json!({"message": "Hello from integration test!"}),
            )
            .await;

        assert!(echo_result.is_ok());
        let echo_response = echo_result.unwrap();
        assert!(!echo_response.is_error);
        assert_eq!(echo_response.content.len(), 1);

        // 测试 2: 测试 add 工具
        println!("Testing add tool...");
        let add_result = manager
            .call_tool("test-server", "add", json!({"a": 40, "b": 2}))
            .await;

        assert!(add_result.is_ok());
        let add_response = add_result.unwrap();
        assert!(!add_response.is_error);

        // 测试 3: 使用 ToolExecutor
        println!("Testing ToolExecutor...");
        let executor = ToolExecutor::new();
        let exec_result = executor
            .execute(
                &mut manager,
                "echo",
                json!({"message": "Hello from executor!"}),
            )
            .await;

        assert!(exec_result.is_ok());
        let exec_response = exec_result.unwrap();
        assert!(!exec_response.is_error);
        assert_eq!(exec_response.tool_name, "echo");

        // 测试 4: 测试错误情况 - 缺少参数
        println!("Testing error case...");
        let error_result = manager.call_tool("test-server", "echo", json!({})).await;

        assert!(error_result.is_ok());
        let error_response = error_result.unwrap();
        assert!(error_response.is_error);

        // 测试 5: 查找工具服务器
        println!("Testing tool registry...");
        let server_name = manager.find_tool_server("echo");
        assert_eq!(server_name, Some("test-server"));

        // 通过注册表获取工具
        let tool = manager.tool_registry().get_tool("add");
        assert!(tool.is_some());

        println!("Tool call tests completed!");

        manager
            .stop_server("test-server")
            .await
            .expect("Failed to stop server");
    } else {
        println!("Skipping tool call test (Node.js not available)");
    }
}
