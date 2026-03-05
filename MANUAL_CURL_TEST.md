# 手动验证配置 API (curl)

**有必要**。对于 API 开发，使用 `curl` 进行手动测试是“黄金标准”，因为它：
1.  **无黑盒**：你可以直接看到最原始的 HTTP 请求和响应，不经过任何 SDK 封装。
2.  **易复现**：同事可以直接复制命令在任何环境运行。
3.  **可调试**：通过 `-v` 参数可以查看完整的请求头、响应头，有助于排查细节问题（如 Content-Type、CORS 等）。

请按顺序执行以下命令进行验证：

## 1. 检查服务健康状态
```powershell
curl -v http://127.0.0.1:18789/health
```
**预期**: `HTTP/1.1 200 OK`，返回 `OK`。

## 2. 获取当前配置 (GET)
```powershell
curl -v http://127.0.0.1:18789/api/config
```
**预期**: 返回 JSON 配置，包含 `llm` 和 `mcp` 字段。

## 3. 修改配置 (PUT) - Windows PowerShell 版
*注意：PowerShell 中 JSON 的双引号需要转义，或者使用单引号包裹整个 Body。*

```powershell
# 修改 temperature 为 0.5
$body = '{"server":{"host":"127.0.0.1","port":18789},"llm":{"provider":"openai","api_key":"sk-test","base_url":"https://api.openai.com/v1","model":"gpt-4o","max_tokens":2048,"temperature":0.5},"mcp":{"enabled":false}}'
curl -v -Method PUT -Uri "http://127.0.0.1:18789/api/config" -ContentType "application/json" -Body $body
```
**预期**: 返回更新后的 JSON，`temperature` 字段应为 `0.5`。

## 4. 验证持久化 (再次 GET)
```powershell
curl -v http://127.0.0.1:18789/api/config
```
**预期**: 返回的 JSON 中 `temperature` 仍为 `0.5`，证明已保存。

## 5. 还原配置 (可选)
```powershell
# 还原 temperature 为 0.7
$body = '{"server":{"host":"127.0.0.1","port":18789},"llm":{"provider":"openai","api_key":"sk-test","base_url":"https://api.openai.com/v1","model":"gpt-4o","max_tokens":2048,"temperature":0.7},"mcp":{"enabled":false}}'
curl -v -Method PUT -Uri "http://127.0.0.1:18789/api/config" -ContentType "application/json" -Body $body
```

---
**提示**：
- 如果你使用的是 `cmd` 或 Git Bash，命令略有不同（主要在于引号转义）。
- 如果 `curl` 在 PowerShell 中是 `Invoke-WebRequest` 的别名，上面的 `-Method` 等参数适用；如果安装了原生 `curl.exe`，则使用 `-X PUT -d "..."` 语法。
