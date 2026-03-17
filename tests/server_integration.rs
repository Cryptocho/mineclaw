//! MineClaw 服务器集成测试
//!
//! 测试整个服务器的功能，特别是终端工具的超时和输出读取

use mineclaw::SessionRepository;
use mineclaw::config::Config;
use mineclaw::models::*;
use mineclaw::tools::{
    checkpoint::CheckpointTools, filesystem::FilesystemTool, registry::LocalToolRegistry,
    terminal::TerminalTool,
};
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn test_server_health_check() {
    // 创建测试配置
    let mut config = Config::default();
    config.local_tools.terminal.timeout_seconds = 2;
    config.local_tools.terminal.max_output_bytes = 65536;

    // 初始化测试状态（不启动真实服务器，只测试核心组件）
    let _session_repo = SessionRepository::new();

    // 初始化本地工具注册表
    let mut local_tool_registry = LocalToolRegistry::new();
    FilesystemTool::register_all(&mut local_tool_registry);
    CheckpointTools::register_all(&mut local_tool_registry);
    TerminalTool::register_all(&mut local_tool_registry);

    let local_tool_registry_arc = Arc::new(local_tool_registry);

    // 验证本地工具已注册
    let tools = local_tool_registry_arc.list_tools();

    // 验证终端工具存在
    let has_run_command = tools.iter().any(|t| t.name == "run_command");
    let has_list_background = tools.iter().any(|t| t.name == "list_background_tasks");
    let has_get_task = tools.iter().any(|t| t.name == "get_task_result");

    assert!(has_run_command, "run_command 工具应该已注册");
    assert!(has_list_background, "list_background_tasks 工具应该已注册");
    assert!(has_get_task, "get_task_result 工具应该已注册");
}

#[tokio::test]
async fn test_terminal_tool_basic_execution() {
    // 创建测试配置
    let mut config = Config::default();
    config.local_tools.terminal.timeout_seconds = 10;
    config.local_tools.terminal.max_output_bytes = 65536;

    let config_arc = Arc::new(config);

    // 初始化本地工具注册表
    let mut local_tool_registry = LocalToolRegistry::new();
    TerminalTool::register_all(&mut local_tool_registry);

    // 创建测试上下文
    let session = Session::new();
    let context = mineclaw::tools::ToolContext::new(session, config_arc.clone());

    // 测试简单命令
    let is_windows = cfg!(windows);
    let params = if is_windows {
        json!({
            "command": "echo hello from integration test"
        })
    } else {
        json!({
            "command": "echo hello from integration test"
        })
    };

    let result = local_tool_registry
        .call_tool("run_command", params, context)
        .await;

    let result_value = result.unwrap();

    let exit_code = result_value["exit_code"].as_i64().unwrap();
    let stdout = result_value["stdout"].as_str().unwrap();

    assert_eq!(exit_code, 0, "退出码应该是 0");
    assert!(stdout.contains("hello"), "输出应该包含 'hello'");
}

#[tokio::test]
async fn test_terminal_tool_timeout() {
    // 创建测试配置 - 设置很短的超时时间
    let mut config = Config::default();
    config.local_tools.terminal.timeout_seconds = 1;
    config.local_tools.terminal.max_output_bytes = 65536;

    let config_arc = Arc::new(config);

    // 初始化本地工具注册表
    let mut local_tool_registry = LocalToolRegistry::new();
    TerminalTool::register_all(&mut local_tool_registry);

    // 创建测试上下文
    let session = Session::new();
    let context = mineclaw::tools::ToolContext::new(session, config_arc.clone());

    // 测试超时
    let is_windows = cfg!(windows);
    let params = if is_windows {
        json!({
            "command": "echo start & ping -n 5 127.0.0.1 > nul & echo end"
        })
    } else {
        json!({
            "command": "echo start; sleep 5; echo end"
        })
    };

    let result = local_tool_registry
        .call_tool("run_command", params, context)
        .await;
    assert!(result.is_ok(), "超时命令应该返回结果（不是错误）");

    let result_value = result.unwrap();

    let is_timeout = result_value["is_timeout"].as_bool().unwrap();
    let stdout = result_value["stdout"].as_str().unwrap();

    assert!(is_timeout, "应该标记为超时");
    assert!(stdout.contains("start"), "应该包含 'start'");
    assert!(!stdout.contains("end"), "不应该包含 'end'");
}

#[tokio::test]
async fn test_terminal_tool_output_truncation() {
    // 创建测试配置 - 设置很小的输出限制
    let mut config = Config::default();
    config.local_tools.terminal.timeout_seconds = 10;
    config.local_tools.terminal.max_output_bytes = 50; // 很小的限制

    let config_arc = Arc::new(config);

    // 初始化本地工具注册表
    let mut local_tool_registry = LocalToolRegistry::new();
    TerminalTool::register_all(&mut local_tool_registry);

    // 创建测试上下文
    let session = Session::new();
    let context = mineclaw::tools::ToolContext::new(session, config_arc.clone());

    // 测试长输出
    let is_windows = cfg!(windows);
    let long_text = "a".repeat(200);
    let params = if is_windows {
        json!({
            "command": format!("echo {}", long_text)
        })
    } else {
        json!({
            "command": format!("echo {}", long_text)
        })
    };

    let result = local_tool_registry
        .call_tool("run_command", params, context)
        .await;
    assert!(result.is_ok(), "命令应该执行成功");

    let result_value = result.unwrap();

    let truncated = result_value["truncated"].as_bool().unwrap();
    let stdout = result_value["stdout"].as_str().unwrap();

    assert!(truncated, "输出应该被截断");
    assert!(stdout.len() <= 50, "截断后的输出应该不超过限制");
}

#[tokio::test]
async fn test_terminal_tool_detach_and_list() {
    // 创建测试配置
    let mut config = Config::default();
    config.local_tools.terminal.timeout_seconds = 10;
    config.local_tools.terminal.max_output_bytes = 65536;

    let config_arc = Arc::new(config);

    // 初始化本地工具注册表
    let mut local_tool_registry = LocalToolRegistry::new();
    TerminalTool::register_all(&mut local_tool_registry);

    // 创建测试上下文
    let session = Session::new();
    let context = mineclaw::tools::ToolContext::new(session.clone(), config_arc.clone());

    // 测试 detach 模式
    let is_windows = cfg!(windows);
    let params = if is_windows {
        json!({
            "command": "timeout /t 3",
            "detach": true
        })
    } else {
        json!({
            "command": "sleep 3",
            "detach": true
        })
    };

    let result = local_tool_registry
        .call_tool("run_command", params, context.clone())
        .await;
    assert!(result.is_ok(), "detach 命令应该执行成功");

    let result_value = result.unwrap();

    let task_id = result_value["task_id"].as_str().unwrap();
    let exit_code = result_value["exit_code"].as_i64().unwrap();

    assert!(!task_id.is_empty(), "应该返回 task_id");
    assert_eq!(exit_code, -1, "detach 模式应该返回 exit_code = -1");

    // 测试 list_background_tasks
    let list_result = local_tool_registry
        .call_tool("list_background_tasks", json!({}), context.clone())
        .await;
    assert!(list_result.is_ok(), "list_background_tasks 应该执行成功");

    let list_value = list_result.unwrap();

    let tasks = list_value["background_tasks"].as_array().unwrap();
    assert!(!tasks.is_empty(), "应该至少有一个后台任务");

    let found = tasks.iter().any(|t| t["task_id"].as_str() == Some(task_id));
    assert!(found, "应该能找到刚才创建的任务");

    // 测试 get_task_result - 先等一会儿
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let get_result = local_tool_registry
        .call_tool(
            "get_task_result",
            json!({ "task_id": task_id, "kill": true }),
            context,
        )
        .await;
    assert!(get_result.is_ok(), "get_task_result 应该执行成功");

    let get_value = get_result.unwrap();

    let status = get_value["status"].as_str().unwrap();
    assert_eq!(status, "terminated", "任务应该被终止");
}

#[tokio::test]
async fn test_complete_integration_suite() {
    // 这个测试验证所有组件能够协同工作
    let _config = Config::default();

    // 初始化 SessionRepository
    let session_repo = SessionRepository::new();

    // 创建 session
    let session = session_repo.create().await;

    // 验证 session 状态
    assert_eq!(session.state, SessionState::Draft);
    assert!(session.messages.is_empty());

    // 验证可以查询 session
    let retrieved = session_repo.get(&session.id).await;
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, session.id);

    // 验证可以列出 sessions
    let sessions = session_repo.list().await;
    assert_eq!(sessions.len(), 1);

    // 删除 session
    session_repo.delete(&session.id).await.unwrap();

    // 验证 session 已删除
    let deleted = session_repo.get(&session.id).await;
    assert!(deleted.is_none());
}
