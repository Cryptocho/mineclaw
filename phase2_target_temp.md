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

## Phase 2.2: MCP 协议和基础客户端 ✅

**目标**: 实现 MCP 协议的核心部分和基础客户端。

**状态**: 已完成（2026-03-03）

### 完成内容
- 定义 MCP JSON-RPC 2.0 协议消息类型 (`src/mcp/protocol.rs`)
- 实现 stdio 传输层（进程启动、异步读写）(`src/mcp/transport.rs`)
- 实现 MCP 客户端会话管理 (`src/mcp/client.rs`)
- 实现初始化流程 (`initialize` → `initialized`)
- 实现工具列表查询 (`tools/list`)
- 实现 MCP 服务器管理器（单服务器）(`src/mcp/server.rs`)
- 项目结构重构（`src/lib.rs` + `src/main.rs` 分离）
- 集成测试 (`tests/mcp_integration.rs`)
- 完整的测试文档 (`TEST.md`)
- 编写单元测试（31 个测试，全部通过）
- 集成测试验证通过

### 任务清单
- [x] 定义 MCP JSON-RPC 2.0 协议消息类型
- [x] 实现 stdio 传输层（进程启动、读写）
- [x] 实现 MCP 客户端会话管理
- [x] 实现初始化流程 (`initialize` → `initialized`)
- [x] 实现工具列表查询 (`tools/list`)
- [x] 实现 MCP 服务器管理器（单服务器）
- [x] 项目结构重构（lib/bin 分离）
- [x] 创建集成测试文件
- [x] 所有单元测试通过
- [x] 集成测试通过
- [x] TEST.md 文档完整
- [x] 测试用 MCP 服务器已创建

### 交付物
- 可以连接到 MCP 服务器并获取工具列表的基础客户端
- 简单的 MCP 服务器管理
- 完整的单元测试套件（31 个测试）
- 集成测试文件
- 测试指南文档（TEST.md）
- 重构后的项目结构
- 测试用 MCP 服务器（test-mcp-server.js）

---

## Phase 2.3: 工具调用功能 ✅

**目标**: 实现工具调用和结果返回。

**状态**: 已完成（2026-03-03）

### 完成内容
- 扩展协议定义，添加 `CallToolRequest`/`CallToolResponse`/`ToolResultContent`
- 扩展 MCP 客户端，添加 `call_tool()` 方法
- 创建工具注册表 (`ToolRegistry`) - 管理多服务器工具
- 创建工具执行器 (`ToolExecutor`) - 支持超时控制
- 扩展服务器管理器，集成工具注册表和工具调用
- 更新测试服务器，添加 `echo` 和 `add` 工具的 `tools/call` 支持
- 更新集成测试，添加完整的工具调用测试
- 编写单元测试（新增 20 个测试，总计 51 个，全部通过）
- 集成测试验证通过（3 个测试，全部通过）

### 任务清单
- [x] 实现工具调用 (`tools/call`)
- [x] 实现工具注册表（聚合多个服务器的工具）
- [x] 工具执行器（调用 MCP 工具）
- [x] 工具调用超时控制
- [x] 错误处理和日志记录

### 交付物
- 可以执行工具调用并获取结果的完整 MCP 客户端
- 工具注册表
- 工具执行器
- 完整的单元测试套件（51 个测试）
- 集成测试（3 个测试）
- 所有测试通过

---

## Phase 2.4: 扩展 LLM 支持工具调用 ✅

**目标**: 修改 LLM 客户端以支持工具调用。

**状态**: 已完成（2026-03-03）

### 完成内容
- 扩展 `ChatMessage` 添加 `tool_calls`/`tool_call_id` 字段
- 扩展 `ChatCompletionRequest` 支持 `tools` 字段
- 扩展 `ChatCompletionResponse` 解析 `tool_calls`
- 添加 `LlmResponse` 枚举（Text/ToolCalls）
- 添加 OpenAI 格式工具类型（`ChatTool`/`ChatToolCall` 等）
- 修改 `LlmProvider` trait，添加 `chat_with_tools()` 方法
- 实现消息转换（`from_message`/`tool_to_chat_tool`/`chat_tool_call_to_tool_call`）
- 扩展 `AppState` 添加 `mcp_server_manager`/`tool_executor`
- 创建 `ToolCoordinator` 工具调用协调器
- 更新 `main.rs` 初始化 MCP 服务器管理器
- 创建测试文档 `TEST_PHASE2_4.md`
- 编写单元测试（总计 51 个测试，全部通过）
- 集成测试验证通过（3 个测试，全部通过）

### 任务清单
- [x] 更新 `ChatCompletionRequest` 支持 `tools` 字段
- [x] 更新 `ChatCompletionResponse` 解析 `tool_calls`
- [x] 修改 `LlmProvider` trait，支持工具调用参数
- [x] 实现消息转换（Message ↔ LLM 格式，包含工具）
- [x] 创建 ToolCoordinator 工具调用协调器
- [x] 扩展 AppState 集成 MCP 组件
- [x] 更新 main.rs 初始化 MCP 服务器

### 交付物
- 支持工具调用的 LLM 客户端
- ToolCoordinator 工具调用协调器
- 完整的单元测试套件（51 个测试）
- 集成测试（3 个测试）

---

## Phase 2.5: 集成工具调用循环 ✅

**目标**: 将所有组件集成，实现完整的工具调用流程。

**状态**: 已完成（2026-03-03）

### 完成内容
- `ToolCoordinator` 已完整实现（LLM → 工具 → LLM 循环）
- 扩展 `AppState` 添加 `tool_coordinator: Arc<ToolCoordinator>` 字段
- 修改 `AppState::new()` 接受 `Arc<Mutex<McpServerManager>>`
- 修改 `main.rs` 初始化 `ToolCoordinator`
- 修改 `send_message` handler 使用 `ToolCoordinator::run()`
- 保存工具调用和结果到会话历史
- 支持多轮工具调用（默认最大 10 轮）
- 所有 51 个单元测试通过
- 所有 3 个集成测试通过

### 任务清单
- [x] 实现工具调用协调器（LLM → 工具 → LLM 循环）
- [x] 修改 `send_message` handler 支持工具调用循环
- [x] 保存工具调用和结果到会话历史
- [x] 多轮工具调用支持

### 交付物
- 完整的工具调用流程集成
- 完整的单元测试套件（51 个测试，全部通过）
- 完整的集成测试套件（3 个测试，全部通过）

---

## Phase 2.6: SSE 流式模式（按轮推送）✅

**目标**: 提供 SSE 流式 API，实时推送每一轮交互（不是逐个字符，而是完整的消息轮）。

**状态**: 已完成（2026-03-04）

### 设计架构

```
用户 ←(SSE 连接)→ MineClaw ←(普通 HTTP)→ LLM API
```

**核心特点：**
- **MineClaw ↔ LLM**: 普通 HTTP 请求（非流式），简单可靠
- **用户 ↔ MineClaw**: SSE 连接，实时推送每一轮完整消息
- **推送粒度**: 每轮推送一条完整消息，不是逐个字符

### 完成内容
- 定义 `SseEvent` 枚举（5种事件类型）
- 实现 `ToolCoordinatorCallback` trait（5个回调方法）
- 实现 `SseChannel` 事件通道（`tokio::sync::mpsc` 实现）
- 实现 `POST /api/messages/stream` - 新建会话并建立 SSE
- 实现 `GET /api/sessions/:id/stream` - 连接现有会话的 SSE
- 使用 `axum::response::sse` 实现 SSE 响应
- 创建测试文档 `TEST_PHASE2_6.md`
- 运行 `cargo clippy` 和 `cargo fmt` 优化代码
- 编写单元测试（新增 6 个测试，总计 56 个，全部通过）
- 集成测试验证通过（3 个测试，全部通过）

### SSE 推送格式

**消息类型：**
```
data: {"type": "assistant_message", "content": "我现在开始计算第一个和：1 + 2。"}

data: {"type": "tool_call", "tool": "add", "arguments": {"a": 1, "b": 2}}

data: {"type": "tool_result", "content": "3", "is_error": false}

data: {"type": "assistant_message", "content": "第一个计算结果是3。接下来计算第二个和：2 + 3。"}

data: {"type": "tool_call", "tool": "add", "arguments": {"a": 2, "b": 3}}

data: {"type": "tool_result", "content": "5", "is_error": false}

data: {"type": "assistant_message", "content": "第二个计算结果是5。接下来计算第三个和：3 + 4。"}

data: {"type": "tool_call", "tool": "add", "arguments": {"a": 3, "b": 4}}

data: {"type": "tool_result", "content": "7", "is_error": false}

data: {"type": "assistant_message", "content": "所有计算完成！1+2=3, 2+3=5, 3+4=7"}

data: {"type": "completed"}
```

### curl 使用示例

**请求：**
```bash
curl -N -X POST http://127.0.0.1:18789/api/messages/stream \
  -H "Content-Type: application/json" \
  -d '{
    "content": "Please calculate these sums one by one..."
  }'
```

**或使用会话 ID：**
```bash
curl -N -X POST http://127.0.0.1:18789/api/messages/stream \
  -H "Content-Type: application/json" \
  -d '{
    "session_id": "{session-id}",
    "content": "Tell me more"
  }'
```

### 数据结构设计

```rust
/// SSE 事件类型
pub enum SseEvent {
    /// 助手消息
    AssistantMessage { content: String },
    /// 工具调用
    ToolCall { tool: String, arguments: serde_json::Value },
    /// 工具结果
    ToolResult { content: String, is_error: bool },
    /// 完成
    Completed,
    /// 错误
    Error { message: String },
}
```

### 任务清单
- [x] 设计 SSE 事件数据结构
- [x] 实现 ToolCoordinator 回调机制
- [x] 修复 axum 依赖配置
- [x] 实现 SSE 事件通道和响应
- [x] 添加路由
- [x] 添加 SSE API Handlers
- [x] 测试验证

### 优点
- ✅ LLM API 简单（不需要处理流式）
- ✅ 用户体验好（实时看到每一步进度）
- ✅ 实现清晰（每轮推送完整消息）
- ✅ 易于调试（每条推送都是完整 JSON）

### 交付物
- SSE 流式 API
- 实时推送每轮交互
- 完整的 curl 使用示例
- TEST_PHASE2_6.md 测试文档

---

## Phase 2.7: API 扩展和管理功能

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

## Phase 2.8: 测试和优化

**目标**: 全面测试和优化。

### 任务清单
- [ ] 端到端测试（用户消息 → 工具调用 → 最终回复）
- [ ] 错误场景测试（MCP 服务器崩溃、工具调用失败等）
- [ ] 流式模式测试（长时间运行任务）
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
| Phase 2.6 | 中 | 2.5 |
| Phase 2.7 | 小 | 2.6 |
| Phase 2.8 | 中 | 2.7 |

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