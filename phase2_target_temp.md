# MineClaw Phase 2: MCP 集成 分阶段计划

## 概述

Phase 2 的目标是将 MCP (Model Context Protocol) 集成到 MineClaw 中，实现工具调用能力。这将使 AI 能够使用外部工具来完成更复杂的任务。

本计划将 Phase 2 拆分成多个小阶段，每个阶段都有明确的目标和可交付成果。

---

## Phase 2.1: 数据模型和配置扩展 ✅

**目标**: 为 MCP 集成做好准备工作，扩展数据模型和配置。

**状态**: 已完成（2026-03-03）

### 完成内容
- 扩展 `MessageRole` 枚举（Tool → ToolCall，新增 ToolResult）
- 定义 `Tool`/`ToolCall`/`ToolResult` 数据结构
- 扩展 `Message` 结构体，添加 `tool_calls`/`tool_result` 字段
- 添加配置结构 `McpConfig`/`McpServerConfig`
- 添加 MCP 相关错误类型
- 更新 `Cargo.toml`（添加 tokio-process、futures-util）
- 编写单元测试（13 个测试，全部通过）

### 任务清单
- [x] 扩展 `MessageRole` 枚举，添加 `ToolCall` 和 `ToolResult`
- [x] 扩展 `Message` 结构体，添加工具调用相关字段
- [x] 定义工具调用数据结构 (`ToolCall`, `ToolResult`, `Tool`)
- [x] 扩展配置文件结构，添加 `mcp` 配置段
- [x] 扩展错误类型，添加 MCP 相关错误
- [x] 更新 `Cargo.toml`，添加必要的依赖

### 交付物
- 扩展后的数据模型
- 支持 MCP 配置的配置系统
- 新的错误类型定义
- 单元测试（13 个，全部通过）

---

## Phase 2.2: MCP 协议和基础客户端

**目标**: 实现 MCP 协议的核心部分和基础客户端。

### 任务清单
- [ ] 定义 MCP JSON-RPC 2.0 协议消息类型
- [ ] 实现 stdio 传输层（进程启动、读写）
- [ ] 实现 MCP 客户端会话管理
- [ ] 实现初始化流程 (`initialize` → `initialized`)
- [ ] 实现工具列表查询 (`tools/list`)
- [ ] 实现 MCP 服务器管理器（单服务器）

### 交付物
- 可以连接到 MCP 服务器并获取工具列表的基础客户端
- 简单的 MCP 服务器管理

---

## Phase 2.3: 工具调用功能

**目标**: 实现工具调用和结果返回。

### 任务清单
- [ ] 实现工具调用 (`tools/call`)
- [ ] 实现工具注册表（聚合多个服务器的工具）
- [ ] 工具执行器（调用 MCP 工具）
- [ ] 工具调用超时控制
- [ ] 错误处理和日志记录

### 交付物
- 可以执行工具调用并获取结果的完整 MCP 客户端
- 工具注册表

---

## Phase 2.4: 扩展 LLM 支持工具调用

**目标**: 修改 LLM 客户端以支持工具调用。

### 任务清单
- [ ] 更新 `ChatCompletionRequest` 支持 `tools` 字段
- [ ] 更新 `ChatCompletionResponse` 解析 `tool_calls`
- [ ] 修改 `LlmProvider` trait，支持工具调用参数
- [ ] 实现消息转换（Message ↔ LLM 格式，包含工具）

### 交付物
- 支持工具调用的 LLM 客户端

---

## Phase 2.5: 集成工具调用循环

**目标**: 将所有组件集成，实现完整的工具调用流程。

### 任务清单
- [ ] 实现工具调用协调器（LLM → 工具 → LLM 循环）
- [ ] 修改 `send_message` handler 支持工具调用循环
- [ ] 保存工具调用和结果到会话历史
- [ ] 多轮工具调用支持

### 交付物
- 完整的工具调用流程集成

---

## Phase 2.6: API 扩展和管理功能

**目标**: 添加管理 API 和监控功能。

### 任务清单
- [ ] `GET /api/tools` - 列出所有可用工具
- [ ] `GET /api/mcp/servers` - 列出 MCP 服务器状态
- [ ] `POST /api/mcp/servers/:name/restart` - 重启 MCP 服务器
- [ ] MCP 服务器健康检查
- [ ] 自动重连机制
- [ ] 详细的 MCP 通信日志

### 交付物
- 完整的管理 API
- 健康检查和自动重连

---

## Phase 2.7: 测试和优化

**目标**: 全面测试和优化。

### 任务清单
- [ ] 端到端测试（用户消息 → 工具调用 → 最终回复）
- [ ] 错误场景测试（MCP 服务器崩溃、工具调用失败等）
- [ ] 性能优化
- [ ] 文档更新

### 交付物
- 完整的 Phase 2 功能，经过测试验证

---

## 总体时间线估算

| 阶段 | 工作量估算 | 依赖 |
|------|-----------|------|
| Phase 2.1 | 小 | 无 |
| Phase 2.2 | 中 | 2.1 |
| Phase 2.3 | 中 | 2.2 |
| Phase 2.4 | 小 | 2.1, 2.3 |
| Phase 2.5 | 中 | 2.4 |
| Phase 2.6 | 小 | 2.5 |
| Phase 2.7 | 中 | 2.6 |

---

## LLM 工具调用流程

```
1. 用户发送消息
   ↓
2. 构建消息历史（包含工具）
   ↓
3. 调用 LLM，传入可用工具列表
   ↓
4. LLM 返回响应
   ├─ 直接返回文本 → 结束
   └─ 返回工具调用 → 继续
       ↓
5. 执行工具调用
   ↓
6. 将工具结果添加到消息历史
   ↓
7. 回到步骤 3（循环直到 LLM 返回最终文本）
```

---

## 配置文件示例

```toml
[server]
host = "127.0.0.1"
port = 18789

[llm]
provider = "openai"
api_key = "${OPENAI_API_KEY}"
base_url = "https://api.openai.com/v1"
model = "gpt-4o"
max_tokens = 2048
temperature = 0.7

[mcp]
enabled = true

[[mcp.servers]]
name = "filesystem"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/workspace"]
env = {}

[[mcp.servers]]
name = "github"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
env = { "GITHUB_PERSONAL_ACCESS_TOKEN" = "${GITHUB_TOKEN}" }
```

---

## 后续规划 (Phase 3+)

- Phase 3: 终端工具集成（命令执行、lint、格式化等）
- Phase 4: 多渠道适配器（Telegram、Discord 等）
- Phase 5: 持久化存储（数据库替代内存存储）
- Phase 6: 权限和用户管理