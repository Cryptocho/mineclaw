# Debug 模式手动测试流程

本流程用于开启详细日志（Debug Level）以排查问题，确保配置 API 和热更新逻辑正确执行。

## 1. 启动 Debug 模式服务器

在 **终端 1** 中执行以下 PowerShell 命令（注意：这将设置环境变量并启动服务器）：

```powershell
$env:RUST_LOG="debug,hyper=info,reqwest=info"
cargo run
```
*说明：*
- `debug`: 开启 `mineclaw` 及其他 crate 的 debug 日志。
- `hyper=info,reqwest=info`: 降低底层网络库的日志级别，避免刷屏。

**预期输出**：
你将看到大量 `DEBUG` 开头的日志，例如：
- `DEBUG mineclaw::mcp::server: Initialized successfully`
- `DEBUG mineclaw::api::handlers: Get config request received`

## 2. 执行 Curl 测试命令

保持 **终端 1** 运行，打开 **终端 2** 执行以下命令：

### A. 获取当前配置
```powershell
curl -v http://127.0.0.1:18789/api/config
```

### B. 修改配置（触发热更新）
```powershell
$body = '{"server":{"host":"127.0.0.1","port":18789},"llm":{"provider":"openai","api_key":"sk-test-debug","base_url":"https://api.openai.com/v1","model":"gpt-4o","max_tokens":2048,"temperature":0.9},"mcp":{"enabled":false}}'
curl -v -Method PUT -Uri "http://127.0.0.1:18789/api/config" -ContentType "application/json" -Body $body
```

### C. 观察服务器日志（关键步骤）
在 **终端 1** 中，寻找类似以下的日志：
- `INFO mineclaw::api::handlers: Update config request received`
- `DEBUG mineclaw::config: Config saved to file` (如果实现了 debug 日志)
- `INFO mineclaw::api::handlers: Restarting X MCP servers` (如果有 MCP 启用)
- `INFO mineclaw::api::handlers: Config updated successfully`

## 3. 验证持久化
```powershell
curl -v http://127.0.0.1:18789/api/config
```
**预期**: 返回的 JSON 中 `temperature` 应为 `0.9`。

## 4. 还原配置（清理环境）
```powershell
$body = '{"server":{"host":"127.0.0.1","port":18789},"llm":{"provider":"openai","api_key":"sk-test","base_url":"https://api.openai.com/v1","model":"gpt-4o","max_tokens":2048,"temperature":0.7},"mcp":{"enabled":false}}'
curl -v -Method PUT -Uri "http://127.0.0.1:18789/api/config" -ContentType "application/json" -Body $body
```
