# MineClaw 完整 API 契约文档 (V1.0.0)

本文件是 MineClaw 后端 (Rust/Axum) 与 前端控制台 (Flutter) 之间的权威通信协议。
本协议覆盖了 **Phase 4 多 Agent 协作**、**Phase 8 像素工作室视觉化**、**全景状态监控**以及**时空穿梭（Checkpoint）** 的所有底层接口。

---

## 0. 全局约定与数据模型 (Global Conventions & Data Models)

- **Base URL**: `http://localhost:18789` (默认开发端口)
- **Content-Type**: `application/json`
- **通信模式**: REST 负责指令与查询，SSE (Server-Sent Events) 负责实时状态下发。

### 0.1 统一响应封装 (Standard Response Wrapper)
所有非流式 (Non-SSE) 的 REST 接口都将使用如下包装：

```json
{
  "success": true,
  "data": { ... },       // 成功时携带的数据，根据各接口定义变化
  "error": {             // 仅当 success=false 时存在
    "code": "ERROR_CODE_ENUM",
    "message": "Human readable error message"
  }
}
```

### 0.2 分页对象 (Pagination Object)
如果接口支持分页，`data` 结构必定如下：
```json
{
  "items": [ ... ],
  "total": 42,
  "page": 1,
  "page_size": 20,
  "has_more": true
}
```

---

## 1. 会话与对话模块 (Session & Chat)

### 1.1 获取会话列表
- **GET /api/v1/sessions**
- **Query Params**: `page` (int), `page_size` (int)
- **Response `data`**:
  ```json
  {
    "items": [
      {
        "id": "ses_12345",
        "title": "Debug TCP Connection Issue",
        "created_at": "2024-03-20T10:00:00Z",
        "updated_at": "2024-03-20T10:15:00Z",
        "status": "active" // active, archived
      }
    ],
    "total": 1, "page": 1, "page_size": 20, "has_more": false
  }
  ```

### 1.2 创建新会话
- **POST /api/v1/sessions**
- **Request Body**:
  ```json
  {
    "title": "新建的任务会话" // (可选) 若不填则后台自动基于第一条消息生成
  }
  ```
- **Response `data`**: `{ "id": "ses_12345", "title": "新建的任务会话", ... }`

### 1.3 删除会话
- **DELETE /api/v1/sessions/{session_id}**
- **Response `data`**: `null` (Success true)

### 1.4 获取会话历史消息
- **GET /api/v1/sessions/{session_id}/messages**
- **Response `data`**:
  ```json
  {
    "items": [
      {
        "id": "msg_999",
        "role": "user",      // user, assistant, system
        "content": "帮我看看这里的代码",
        "timestamp": "2024-03-20T10:01:00Z"
      }
    ],
    ...分页数据
  }
  ```

### 1.5 提交任务/发送消息 (Trigger Orchestrator)
- **POST /api/v1/sessions/{session_id}/messages**
- **Request Body**:
  ```json
  {
    "content": "用 Rust 实现一个快排算法，然后进行测试",
    "use_orchestrator": true // Phase 4 核心：开启多 Agent 总控模式
  }
  ```
- **Response**: 只返回提交成功 `{"success": true, "data": {"task_id": "tsk_888"}}`。具体的文本生成和任务进度通过 **SSE 流** 推送。

---

## 2. 实时状态总线 (SSE Stream)

这是前端全景大屏的“心脏”。

- **GET /api/v1/sessions/{session_id}/stream**
- **协议**: `text/event-stream`

### 2.1 事件字典 (Event Dictionary)

| Event 名称 | 描述 | Payload (`data` JSON) 示例 |
| :--- | :--- | :--- |
| `assistant_message` | 智能体生成的完整回复消息 (消息级推送) | `{"type": "assistant_message", "agent_id": "A1", "content": "你好，我是老板..."}` |
| `agent_spawned` | 总控新创建了一个子 Agent | `{"type": "agent_spawned", "agent_id": "A2", "role": "Coder", "parent_id": "A1", "depth": 1}` |
| `agent_status` | Agent 内部状态变更 (控制像素动作) | `{"type": "agent_status", "agent_id": "A2", "status": "thinking"}` *(status: idle, thinking, typing, searching, panicked, celebrating)* |
| `tool_call` | Agent 发起工具调用请求 | `{"type": "tool_call", "agent_id": "A2", "tool": "run_terminal", "arguments": {"cmd": "cargo test"}}` |
| `tool_result` | 工具执行结果返回 | `{"type": "tool_result", "agent_id": "A2", "content": "test result: ok", "is_error": false}` |
| `work_order_update`| 工单在 Agent 间流转 | `{"type": "work_order_update", "order_id": "wo_11", "from": "A1", "to": "A2", "status": "assigned"}` |
| `cma_alert` | 上下文管理 Agent 发出告警 | `{"type": "cma_alert", "level": "warning", "message": "触发自动裁剪", "agent_id": "CMA_1"}` |

*(注意：在像素工作室界面中，`agent_spawned` 对应员工入职动画，`agent_status` 对应角色动作，`work_order_update` 对应抛物线信封动画。)*

---

## 3. 多 Agent 拓扑与配置 (Topology & Config)

### 3.1 获取当前拓扑树 (Topology)
用于前端渲染组织架构图。

- **GET /api/v1/sessions/{session_id}/topology**
- **Response `data`**:
  ```json
  {
    "nodes": [
      { "id": "A1", "role": "Master Orchestrator", "parent_id": null, "depth": 0, "status": "idle" },
      { "id": "A2", "role": "Rust Coder", "parent_id": "A1", "depth": 1, "status": "typing" },
      { "id": "CMA_1", "role": "Context Manager", "parent_id": "A1", "depth": 1, "status": "idle" }
    ],
    "edges": [
      { "from": "A1", "to": "A2", "type": "supervise" }
    ]
  }
  ```

### 3.2 查看 Agent 深度上下文 (Agent Inspector)
用户点击拓扑图中的某个节点时调用。

- **GET /api/v1/agents/{agent_id}/context**
- **Response `data`**:
  ```json
  {
    "system_prompt": "你是一个 Rust 专家，负责实现核心逻辑...",
    "active_memory_tokens": 4096,
    "current_task": "实现快速排序算法",
    "tool_mask": {
      "fs_access_level": "ReadWrite",
      "allowed_tools": ["edit_file", "read_file", "run_terminal"]
    }
  }
  ```

### 3.3 查看 Agent 思考链 (Thoughts/Logs)
- **GET /api/v1/agents/{agent_id}/logs**
- **Response `data`**:
  ```json
  {
    "logs": [
      "10:05:00 - 开始分析用户需求",
      "10:05:02 - 发现当前目录没有 src 文件夹",
      "10:05:05 - 调用 run_terminal 创建 crate"
    ]
  }
  ```

### 3.4 动态修改 Agent 配置
- **PATCH /api/v1/agents/{agent_id}/config**
- **Request Body**: (支持局部更新)
  ```json
  {
    "fs_access_level": "ReadOnly",  // 一键剥夺写权限
    "model": "gpt-4o"
  }
  ```
- **Response `data`**: 更新后的 Config 对象。

---

## 4. 全景监控 (Monitoring)

### 4.1 获取后端硬件与任务看板数据
- **GET /api/v1/monitor/stats**
- **Response `data`**:
  ```json
  {
    "cpu_usage_percent": 12.5,
    "memory_usage_mb": 256,
    "active_sessions": 2,
    "total_active_agents": 5
  }
  ```

### 4.2 取消/强杀 任务
- **DELETE /api/v1/tasks/{task_id}**
- **Response**: 成功停止返回 `true`。

---

## 5. Checkpoint 时空穿梭 (Time Travel)

### 5.1 获取会话快照列表
- **GET /api/v1/sessions/{session_id}/checkpoints**
- **Response `data`**:
  ```json
  {
    "items": [
      {
        "checkpoint_id": "ckpt_001",
        "description": "初始状态",
        "timestamp": "2024-03-20T10:00:00Z"
      },
      {
        "checkpoint_id": "ckpt_002",
        "description": "成功运行 cargo init 后",
        "timestamp": "2024-03-20T10:05:00Z"
      }
    ]
  }
  ```

### 5.2 手动创建快照
- **POST /api/v1/sessions/{session_id}/checkpoints**
- **Request Body**: `{ "description": "准备重构危险代码前" }`
- **Response `data`**: 创建好的 Checkpoint 对象。

### 5.3 回滚至指定快照 (Restore)
- **POST /api/v1/sessions/{session_id}/checkpoints/{checkpoint_id}/restore**
- **Response `data`**: `{"success": true}` (此操作会引发 Agent 树重置，文件系统覆盖，前端需要播放“时光倒流”特效)。

### 5.4 查看两个快照间的文件系统差异 (Diff)
- **GET /api/v1/sessions/{session_id}/checkpoints/diff?base={ckpt_1}&target={ckpt_2}**
- **Response `data`**:
  ```json
  {
    "files": [
      { "path": "src/main.rs", "status": "modified", "lines_added": 10, "lines_removed": 2 },
      { "path": "Cargo.toml", "status": "added", "lines_added": 15, "lines_removed": 0 }
    ]
  }
  ```

---

## 6. 像素工作室互动模块 (Pixel Studio System)

### 6.1 获取当前角色阵列 (Characters)
前端用来在物理房间中放置人物。基于拓扑图(Topology)的二次视觉映射。

- **GET /api/v1/studio/characters?session_id={session_id}**
- **Response `data`**:
  ```json
  {
    "characters": [
      {
        "agent_id": "A1",
        "role_class": "boss",           // 决定加载 boss_sprite.png
        "name": "总控老板",
        "current_action": "idle",       // 当前正在播放的动画
        "focus_target_id": null         // 正在与谁交谈 (如果有)
      },
      {
        "agent_id": "A2",
        "role_class": "coder",          // 决定加载 coder_sprite.png
        "name": "Rust打工仔",
        "current_action": "typing",
        "focus_target_id": "A1"
      }
    ]
  }
  ```

### 6.2 角色互动 (Poke / Interact)
当用户点击前端的小人时，触发此接口。

- **POST /api/v1/studio/interact**
- **Request Body**:
  ```json
  {
    "agent_id": "A2",
    "action": "poke" // 可选: poke(戳), feed(投喂), praise(表扬)
  }
  ```
- **Response `data`**:
  ```json
  {
    "bark": "别戳我！这泛型生命周期快把我逼疯了！", // 基于大模型当前状态生成的吐槽
    "emotion": "angry"                           // 决定角色头顶冒出的表情符号
  }
  ```

---

## 7. 全局系统与工具 (System & Tools)

### 7.1 获取后端本地可用工具池
- **GET /api/v1/tools**
- **Response `data`**:
  ```json
  {
    "local_tools": [
      { "name": "read_file", "description": "读取本地文件内容" },
      { "name": "grep", "description": "正则搜索" }
    ],
    "mcp_tools": [
      { "name": "git_commit", "description": "来自 Git MCP Server" }
    ]
  }
  ```

### 7.2 MCP 服务器管理
- **GET /api/v1/mcp/servers** (获取连接状态)
- **POST /api/v1/mcp/servers/{server_name}/restart** (重启特定 MCP 插件)

### 7.3 全局系统配置查询 (Debug/Config)
- **GET /api/v1/system/config**
- **Response `data`**:
  ```json
  {
    "llm_provider": "anthropic",
    "llm_model": "claude-3-5-sonnet",
    "terminal_timeout_seconds": 120,
    "checkpoint_enabled": true
  }
  ```

---
**维护者**: MineClaw Architecture Team
**版本约定**: 向下兼容的变更迭代小版本号，破坏性变更迭代大版本号。