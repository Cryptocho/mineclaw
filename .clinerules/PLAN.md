# MineClaw MVP 计划

## 项目概述

MineClaw 是一个用 Rust 实现的轻量化 Claw 工具，参考 OpenClaw 的设计理念，提供：
- Web API 接口，支持从其他设备访问（如 Telegram Bot）
- 与 AI 模型交互的能力
- 通过 MCP (Model Context Protocol) 扩展编码能力
- 终端工具调用（lint、格式化等）

## 核心设计理念

### 轻量化架构
- 单一二进制文件，易于部署
- 模块化设计，功能可插拔
- 配置文件驱动，无需重新编译

### MCP 优先
- 大部分功能通过 MCP 工具实现
- Rust 核心只负责消息路由和状态管理
- 易于扩展新的 MCP 服务器

### 多渠道支持
- Web API 作为核心接口
- 可适配 Telegram、Discord 等平台
- 会话隔离，支持多用户

---

## MVP 功能范围

### Phase 1: 基础消息流转 (当前目标)
- [x] Web API 服务启动
- [x] 接收消息 API
- [x] 发送消息 API
- [x] 会话管理（内存存储）
- [x] 配置文件加载
- [x] 与 LLM 集成（OpenAI 兼容接口）
- [x] 简单的对话流程

### Phase 2: MCP 集成
- [ ] MCP 客户端实现
- [ ] 工具调用支持
- [ ] MCP 服务器配置
- [ ] 工具执行结果反馈

### Phase 3: 终端工具集成
- [ ] 命令执行沙箱
- [ ] Lint 工具调用
- [ ] 文件读写操作
- [ ] 安全权限控制

### Phase 4: 多渠道适配器
- [ ] Telegram Bot 适配器
- [ ] Webhook 支持
- [ ] 消息队列

---

## 技术架构

```
mineclaw/
├── src/
│   ├── main.rs                 # 入口点
│   ├── config.rs               # 配置管理
│   ├── error.rs                # 错误类型定义
│   ├── api/                    # Web API 层
│   │   ├── mod.rs
│   │   ├── routes.rs           # 路由定义
│   │   └── handlers.rs         # 请求处理器
│   ├── models/                 # 数据模型
│   │   ├── mod.rs
│   │   ├── message.rs          # 消息模型
│   │   └── session.rs          # 会话模型
│   ├── llm/                    # LLM 集成
│   │   ├── mod.rs
│   │   ├── client.rs           # LLM 客户端
│   │   └── provider.rs         # 提供商接口
│   ├── mcp/                    # MCP 集成 (Phase 2)
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   └── tools.rs
│   ├── terminal/               # 终端工具 (Phase 3)
│   │   ├── mod.rs
│   │   └── executor.rs
│   └── state.rs                # 应用状态
├── config/
│   └── mineclaw.toml           # 默认配置
└── Cargo.toml
```

---

## 数据模型设计

### Message
```rust
pub struct Message {
    pub id: Uuid,
    pub session_id: Uuid,
    pub role: MessageRole,  // User, Assistant, System, Tool
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
}
```

### Session
```rust
pub struct Session {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub messages: Vec<Message>,
    pub metadata: serde_json::Map<String, serde_json::Value>,
}
```

### API 请求/响应
```rust
// POST /api/messages
pub struct SendMessageRequest {
    pub session_id: Option<Uuid>,  // None = 新建会话
    pub content: String,
    pub metadata: Option<serde_json::Value>,
}

pub struct SendMessageResponse {
    pub message_id: Uuid,
    pub session_id: Uuid,
    pub assistant_response: String,
}

// GET /api/sessions/:id/messages
pub struct ListMessagesResponse {
    pub messages: Vec<Message>,
}
```

---

## Phase 1 详细实现计划

### 步骤 1: 项目基础架构
1. 完善 `Cargo.toml` 依赖
2. 创建模块结构
3. 配置日志系统 (tracing)
4. 错误类型定义 (thiserror)

### 步骤 2: 配置管理
1. 配置文件结构 (TOML)
2. 配置加载与验证
3. 环境变量覆盖
4. 配置示例:
```toml
[server]
host = "127.0.0.1"
port = 18789

[llm]
provider = "openai"  # openai, anthropic, custom
api_key = "${OPENAI_API_KEY}"
base_url = "https://api.openai.com/v1"
model = "gpt-4o"
max_tokens = 2048
temperature = 0.7

[security]
api_key = "optional-secret-key"  # 用于 API 认证
```

### 步骤 3: 核心数据模型
1. Message 模型
2. Session 模型
3. 序列化/反序列化
4. 内存存储实现

### 步骤 4: LLM 客户端
1. LLM Provider trait
2. OpenAI 兼容实现
3. 请求/响应处理
4. 重试与错误处理

### 步骤 5: Web API
1. Axum 路由设置
2. 消息发送端点
3. 会话查询端点
4. 健康检查端点
5. CORS 配置

### 步骤 6: 状态管理
1. AppState 定义
2. 会话仓库 (SessionRepository)
3. 线程安全的状态共享

### 步骤 7: 集成测试
1. API 端点测试
2. 端到端对话测试
3. 配置加载测试

---

## API 端点设计 (Phase 1)

| 方法 | 路径 | 描述 |
|------|------|------|
| POST | `/api/messages` | 发送消息并获取回复 |
| GET | `/api/sessions/:id` | 获取会话信息 |
| GET | `/api/sessions/:id/messages` | 获取会话消息列表 |
| DELETE | `/api/sessions/:id` | 删除会话 |
| GET | `/api/sessions` | 列出所有会话 |
| GET | `/health` | 健康检查 |

---

## 依赖选型

### 已选择
- `axum` - Web 框架
- `tokio` - 异步运行时
- `serde` - 序列化
- `config` - 配置管理
- `tracing` - 日志
- `thiserror` - 错误处理
- `uuid` - ID 生成
- `dirs` - 目录管理
- `reqwest` - HTTP 客户端 (LLM 调用)
- `chrono` - 日期时间
- `anyhow` - 便捷错误处理
- `validator` - 输入验证