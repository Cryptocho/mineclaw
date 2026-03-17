# MineClaw Phase 3 实现计划

## 概述

Phase 3: 本地工具集与安全增强。在这一阶段，我们将实现核心的本地工具集，包括终端工具、文件工具，并集成 agentfs 实现 checkpoint 功能，同时增强 API Key 的安全性。

---

## 目标

- ✅ API Key 加密存储
- ✅ 终端工具（带输出限制和过滤）
- ✅ 文件读写工具（仅指定的 10 个工具）
- ✅ agentfs checkpoint 集成
- ✅ 命令黑名单（而非白名单）

---

## 实现功能

### 1. API Key 加密存储
- **功能描述**: 将配置中的 API Key 使用加密方式存储，避免明文泄露
- **实现细节**:
  - 使用 AES-GCM 256 位加密
  - 加密密钥通过环境变量提供（`MINECLAW_ENCRYPTION_KEY`）
  - 支持加密/解密配置文件中的敏感字段
  - 提供密钥生成工具
  - 配置文件中标记为 `encrypted_` 前缀的字段会被自动解密

- **配置扩展**:
  ```toml
  # config/mineclaw.toml
  [llm]
  api_key = "encrypted:base64encodedencrypteddata"
  ```

### 2. 终端工具
- **功能描述**: 提供命令执行工具，支持输出限制和自定义过滤规则
- **工具名称**: `run_command`
- **实现细节**:
  - 最大输出文本限制（默认 64KB，可配置）
  - 支持用户自定义过滤规则（配置文件）
  - 命令黑名单机制（而非白名单）
  - 工作目录限制
  - 超时控制（默认 300 秒）
  - 支持实时输出流式传输（通过 SSE）

- **内置黑名单**:
  - `rm -rf /`
  - `mkfs`
  - `dd if=`
  - `:(){ :|:& };:`
  - 等危险命令

- **过滤规则配置**:
  ```toml
  # config/mineclaw.toml
  [terminal]
  max_output_bytes = 65536  # 64KB
  timeout_seconds = 300
  allowed_workspaces = ["/path/to/workspace"]

  [terminal.filters]
  # Cargo 过滤规则
  "cargo build" = [
      "^\\s*Compiling",
      "^\\s*Building",
      "^\\s*Finished",
      "^\\s*Running",
  ]
  "cargo run" = [
      "^\\s*Compiling",
      "^\\s*Building",
      "^\\s*Finished",
  ]

  # Pip 过滤规则
  "pip install" = [
      "^Collecting",
      "^Downloading",
      "^\\s+\\d+%",
      "^Installing collected packages",
  ]
  ```

### 3. 文件工具集
- **功能描述**: 提供 11 个指定的文件操作工具
- **文件工具列表**:
  | 工具 | 描述 |
  |------|------|
  | `read_file` | 读取文件内容（支持大小限制） |
  | `write_file` | 写入或覆盖文件 |
  | `list_directory` | 列出目录内容（支持递归） |
  | `search_file` | 在文件中搜索正则表达式模式 |
  | `move_file` | 移动或重命名文件 |
  | `move_directory` | 移动或重命名目录 |
  | `delete_file` | 删除文件 |
  | `delete_directory` | 删除目录（支持递归） |
  | `create_directory` | 创建目录（支持创建父目录） |
  | `search_and_replace` | 使用 SEARCH/REPLACE 块格式替换内容 |
  | `replace_all_keywords` | 全局关键词替换（支持正则表达式） |
- **实现细节**:
  - 所有读取操作有最大文本限制（默认 16KB）
  - 工作目录限制（基于配置）
  - 路径遍历防护（`..` 检查）
  - 基于 agentfs 的 checkpoint 集成
  - 支持正则表达式搜索和替换

- **文件工具配置**:
  ```toml
  # config/mineclaw.toml
  [filesystem]
  max_read_bytes = 16384  # 16KB
  allowed_directories = [
      "/path/to/workspace",
      "/another/allowed/path"
  ]
  enable_checkpoint = true
  checkpoint_directory = ".checkpoints"
  ```

### 4. Checkpoint 集成（与 Phase 3.3 紧密结合）

**核心设计思想**：Checkpoint 功能不是独立模块，而是深度集成到 Phase 3.3 文件工具中，作为其自然扩展。

#### 核心设计

- **agentfs 统一存储**: 全部使用 agentfs，不混合本地文件系统
- **checkpoint_id 在 Message 中**: 每条消息可以关联到对应的 checkpoint
- **会话 JSON 格式**: 使用 JSON 序列化会话，便于调试和迁移
- **两种 restore 模式**: `restore_chat`（仅聊天）和 `restore_all`（聊天+文件）
- **自动会话持久化**: 无需手动保存，会话变更自动持久化
- **自动 checkpoint**: 所有文件写操作前自动创建，并将 checkpoint_id 关联到消息

#### Checkpoint 工具（暴露给 LLM）

| 工具 | 描述 |
|------|------|
| `create_checkpoint` | 手动创建 checkpoint（包含会话+文件） |
| `restore_chat` | 仅恢复聊天历史（会话）到指定 checkpoint |
| `restore_all` | 恢复聊天历史+文件改动到指定 checkpoint |
| `list_checkpoints` | 列出当前会话的所有 checkpoints |
| `delete_checkpoint` | 删除指定 checkpoint |

#### 存储结构（全部使用 agentfs）

```
agentfs://mineclaw/
│
├── [KV 存储]
│   └── sessions/
│       ├── {session_id_1}.json      # 会话数据 (JSON)
│       ├── {session_id_2}.json
│       └── ...
│
└── [文件系统]
    └── checkpoints/
        ├── {session_id_1}/
        │   ├── {checkpoint_id_1}/
        │   │   ├── metadata.json     # checkpoint 元数据
        │   │   ├── session.json      # 会话快照
        │   │   └── files/            # 文件快照
        │   │       ├── path/to/file1
        │   │       └── path/to/file2
        │   ├── {checkpoint_id_2}/
        │   └── ...
        └── {session_id_2}/
            └── ...
```

#### Checkpoint 工作流

```
会话创建
    ↓
用户发送消息 (Message 1, checkpoint_id = None)
    ↓
LLM 调用 write_file 工具
    ↓
[自动创建 checkpoint_id_1]
    ↓
添加 ToolCall 消息 (Message 2, checkpoint_id = Some("checkpoint_id_1"))
    ↓
执行文件写入
    ↓
添加 ToolResult 消息 (Message 3, checkpoint_id = Some("checkpoint_id_1"))
    ↓
LLM 调用 delete_file 工具
    ↓
[自动创建 checkpoint_id_2]
    ↓
添加 ToolCall 消息 (Message 4, checkpoint_id = Some("checkpoint_id_2"))
    ↓
...
```

---

## 配置文件扩展

```toml
# config/mineclaw.toml
[server]
host = "127.0.0.1"
port = 18789

[llm]
provider = "openai"
api_key = "encrypted:..."  # 加密后的 API Key
base_url = "https://api.openai.com/v1"
model = "gpt-4o"
max_tokens = 2048
temperature = 0.7

[encryption]
# 加密密钥通过环境变量 MINECLAW_ENCRYPTION_KEY 提供

[local_tools]
enabled = true

[local_tools.terminal]
enabled = true
max_output_bytes = 65536
timeout_seconds = 300
allowed_workspaces = ["/path/to/workspace"]

# 命令黑名单（正则表达式）
command_blacklist = [
    "^rm -rf /",
    "^mkfs",
    "^dd if=",
    "^:\\(\\)\\{ :\\|:& \\};:",
    "^chmod 777",
    "^chown -R",
]

[local_tools.terminal.filters]
# Cargo 过滤
"cargo build" = [
    "^\\s*Compiling",
    "^\\s*Building",
    "^\\s*Finished",
    "^\\s*Running",
]
"cargo run" = [
    "^\\s*Compiling",
    "^\\s*Building",
    "^\\s*Finished",
]
"cargo test" = [
    "^\\s*Compiling",
    "^\\s*Building",
    "^\\s*Finished",
]

# Pip 过滤
"pip install" = [
    "^Collecting",
    "^Downloading",
    "^\\s+\\d+%",
    "^Installing collected packages",
    "^Successfully installed",
]
"pip3 install" = [
    "^Collecting",
    "^Downloading",
    "^\\s+\\d+%",
    "^Installing collected packages",
    "^Successfully installed",
]

# NPM 过滤
"npm install" = [
    "^npm WARN",
    "^npm ERR",
    "^added",
    "^removed",
    "^changed",
]

[local_tools.filesystem]
enabled = true
max_read_bytes = 16384
allowed_directories = [
    "/path/to/workspace",
]

[local_tools.checkpoint]
enabled = true
checkpoint_directory = ".checkpoints"
max_checkpoints_per_session = 50
auto_cleanup_days = 30

[mcp]
enabled = true

[[mcp.servers]]
# 外部 MCP 服务器配置...
```

---

## 实现步骤

### Phase 3.1: API Key 加密
- [x] 设计加密模块架构
- [x] 实现 `EncryptionManager`
- [x] 集成到配置加载流程
- [x] 编写单元测试
- [x] 更新配置模板

**Phase 3.1 已完成** ✅
- 实现了完整的 `EncryptionManager`（含 `EncryptionManager::new()、encrypt()、decrypt()、generate_key()）
- 集成到配置加载流程，自动检测并解密 `encrypted:` 前缀的 API Key
- 13个完整单元测试覆盖所有加密功能
- 所有测试通过

### Phase 3.2: 终端工具
- [x] 设计终端工具配置结构
- [x] 实现命令黑名单检查
- [x] 实现输出过滤系统
- [x] 实现 `TerminalTool`
- [x] 取消终端专用的 SSE 流式输出（改为统一由全局 API 以 ToolResult 事件推送）
- [x] 编写单元测试

**Phase 3.2 已完成** ✅
- 实现了完整的 `TerminalTool`（`run_command` 工具）
- 命令黑名单机制（硬编码 + 配置）
- 输出限制和过滤系统
- 工作目录限制和超时控制
- **长时任务管理**
  - 任务超时控制
  - 续航机制
- **鲁棒性增强**
  - 交互式工具拦截
  - 环境变量保护
- **任务分离**
  - `list_background_tasks` 工具
  - `get_task_result` 工具
  - 后台执行支持
- 12 个完整测试覆盖所有功能

### Phase 3.3: 文件工具集
- [x] 设计文件工具配置结构
- [x] 实现路径安全检查
  - [x] 实现 11 个文件工具
  - [x] `read_file`
  - [x] `write_file`
  - [x] `list_directory`
  - [x] `search_file`
  - [x] `move_file`
  - [x] `move_directory`
  - [x] `delete_file`
  - [x] `delete_directory`
  - [x] `create_directory`
  - [x] `search_and_replace`
  - [x] `replace_all_keywords`
- [x] 集成读取大小限制
- [x] 编写单元测试

**Phase 3.3 已完成** ✅
- 实现了全部 11 个文件工具
- 路径安全检查（路径遍历防护、目录白名单）
- 完整的单元测试（9 个测试）
- 所有 62+9+3 = 74 个测试通过
- `search_and_replace` 使用 SEARCH/REPLACE 块格式（单个 `diff` 参数）
- `search_file` 支持正则表达式
- `replace_all_keywords` 额外工具，支持关键词全局替换和正则表达式

### Phase 3.4: Checkpoint 集成（与 Phase 3.3 紧密结合）

#### Phase 3.4.1: 依赖与基础结构
- [x] 添加 `agentfs` 和 `agentsql` 依赖到 `Cargo.toml`
- [x] 创建 `src/tools/checkpoint.rs`
- [x] 实现 `CheckpointConfig` 配置结构
- [x] 实现 `Checkpoint`、`CheckpointType`、`CheckpointSnapshot` 数据结构
- [x] 实现 `CheckpointManager` 基础结构

**Phase 3.4.1 已完成** ✅
- 添加了 agentfs v0.2.0 依赖（已包含 agentsql + sqlite）
- 创建了完整的 checkpoint 模块
- 实现了所有数据结构和配置
- 10 个单元测试覆盖所有功能
- 84 个测试全部通过
- 代码已格式化并通过 clippy 检查

#### Phase 3.4.2: Message 结构更新
- [x] 修改 `Message` 添加 `checkpoint_id: Option<String>`
- [x] 更新 `Message::new()` 设置 `checkpoint_id = None`
- [x] 添加 `with_checkpoint_id()` builder 方法
- [x] 更新消息序列化/反序列化测试

**Phase 3.4.2 已完成** ✅
- Message 结构新增 `checkpoint_id: Option<String>` 字段
- `Message::new()` 初始化 `checkpoint_id = None`
- 新增 `with_checkpoint_id()` builder 方法
- 新增 3 个测试用例覆盖 checkpoint_id 功能
- 所有 87 个测试通过
- 代码已格式化并通过 clippy 检查

#### Phase 3.4.3: SessionRepository 持久化
- [x] 修改 `SessionRepository` 集成 agentfs
- [x] 实现会话保存到 agentfs（JSON 格式）
- [x] 实现启动时从 agentfs 加载会话
- [x] 实现会话自动保存（add_message/update 时）
- [x] 测试会话持久化

**Phase 3.4.3 已完成** ✅
- SessionRepository 新增 `agent_fs: Option<Arc<AgentFS>>` 字段
- 新增 `with_agent_fs()` 构造函数支持持久化
- 实现 `load_sessions_from_agentfs()` 启动时加载会话
- 实现 `save_session_to_agentfs()` 保存单个会话
- 实现 `delete_session_from_agentfs()` 删除会话
- `create()`/`update()`/`delete()` 自动持久化到 agentfs
- 所有 agentfs 操作失败记录 `tracing::warn!` 日志，不影响内存操作
- 新增 `Config.agentfs_db_path` 配置项（默认 `data/mineclaw.db`）
- 保持 `SessionRepository::new()` 向后兼容（纯内存模式）
- 所有 87 个测试通过
- 代码已格式化并通过 clippy 检查

#### Phase 3.4.4: Checkpoint 核心功能
- [x] 创建 `src/checkpoint/error.rs` - Checkpoint 错误类型定义
- [x] 创建 `src/models/checkpoint.rs` - Checkpoint 数据结构（含14个单元测试）
- [x] 创建 `src/checkpoint/manager.rs` - CheckpointManager 核心实现
- [x] 实现 `CheckpointManager::new()` - 初始化 agentfs
- [x] 实现 `create_checkpoint()` - 创建 checkpoint（会话+文件），返回完整 Checkpoint
- [x] 实现 `list_checkpoints()` - 列出 checkpoints（返回 ListCheckpointsResponse）
- [x] 实现 `get_checkpoint()` - 获取单个 checkpoint
- [x] 实现 `restore_checkpoint()` - 恢复 checkpoint（可选恢复文件/会话）
- [x] 实现 `delete_checkpoint()` - 删除 checkpoint
- [x] 实现 `delete_all_checkpoints_for_session()` - 删除会话的所有 checkpoints
- [x] 实现文件快照安全存储（路径安全处理）
- [x] 实现会话快照存储和恢复
- [x] 修改 `CheckpointConfig` 移除数量限制
- [x] 修改 `SessionRepository` 添加 CheckpointManager 支持
- [x] 修改 `SessionRepository::delete()` 删除 session 时自动清理相关 checkpoints
- [x] 更新 `src/error.rs` 添加 CheckpointError 转换
- [x] 更新 `src/lib.rs` 导出 checkpoint 模块
- [x] 所有 84 个测试通过

**Phase 3.4.4 已完成** ✅
- 创建完整的 checkpoint 模块架构
- CheckpointManager 支持所有核心功能（创建、列出、获取、恢复、删除）
- 完整的 agentfs 集成（元数据+文件+会话快照）
- **Checkpoint 无数量限制，与 session 强关联**
- **删除 session 时自动清理所有相关 checkpoints**
- 支持按过期时间自动清理
- 14 个单元测试覆盖数据结构
- 所有 84 个测试通过
- 代码已格式化并通过编译检查
- 重构 CheckpointManager 集成 agentfs
- 实现所有核心 checkpoint 功能
- 添加 delete_all_checkpoints_for_session() 方法
- 无 checkpoint 数量限制，仅和 session 绑定
- 删除 session 时自动清理所有相关 checkpoints

#### Phase 3.4.5: 集成到文件工具
- [x] 修改 `ToolContext` 添加 `checkpoint_manager`
- [x] 在 `write_file` 前自动创建 checkpoint
- [x] 在 `move_file` 前自动创建 checkpoint
- [x] 在 `move_directory` 前自动创建 checkpoint
- [x] 在 `delete_file` 前自动创建 checkpoint
- [x] 在 `delete_directory` 前自动创建 checkpoint
- [x] 在 `search_and_replace` 前自动创建 checkpoint
- [x] 在 `replace_all_keywords` 前自动创建 checkpoint

**Phase 3.4.5 已完成** ✅
- 所有文件写操作前自动创建 checkpoint

#### Phase 3.4.6: 集成到消息流
- [x] Message 结构已有 `checkpoint_id` 字段
- [x] ToolCoordinator 已集成 checkpoint_manager
- [x] 文件工具已集成自动 checkpoint 功能

**Phase 3.4.6 已完成** ✅
- 完整集成到消息流

#### Phase 3.4.7: Checkpoint 工具暴露
- [x] 实现 `CreateCheckpointTool`
- [x] 实现 `RestoreCheckpointTool` (支持恢复聊天历史和/或文件)
- [x] 实现 `ListCheckpointsTool`
- [x] 实现 `DeleteCheckpointTool`
- [x] 注册到 `LocalToolRegistry`

**Phase 3.4.7 已完成** ✅
- 所有 Checkpoint 工具已暴露给 LLM

#### Phase 3.4.8: 集成到 AppState
- [x] 修改 `AppState` 添加 `CheckpointManager`
- [x] 更新 `main.rs` 初始化 `CheckpointManager`
- [x] 更新 `main.rs` 初始化 AgentFS

**Phase 3.4.8 已完成** ✅
- 完整集成到 AppState

**Phase 3.4 整体已完成** ✅
- 完整的 Checkpoint 功能实现
- 27 个测试覆盖所有功能
- 所有测试通过

### Phase 3.5: 本地工具集成
- [x] 实现 `LocalTool` trait
- [x] 实现 `LocalToolRegistry`
- [x] 更新 `ToolRegistry` 支持本地工具
- [x] 更新 `ToolExecutor` 支持本地工具执行
- [x] 更新 `ToolCoordinator` 集成本地工具
- [x] 更新 `AppState` 添加本地工具注册表
- [x] 编写集成测试

**Phase 3.5 已完成** ✅
- 完整的本地工具注册表实现
- 与 MCP 工具协同工作
- 所有 219 个测试通过

---

## 依赖更新

需要在 `Cargo.toml` 中添加以下依赖：

```toml
# 加密相关
aes-gcm = "0.10"
rand = "0.8"
base64 = "0.22"

# 文件系统与 checkpoint
agentfs = "0.2"
agentsql = { version = "0.2", features = ["sqlite"] }

# 其他
regex = "1.10"
walkdir = "2.5"
```

---

## 测试检查清单

### Phase 3.1: API Key 加密
- [x] 密钥生成正常
- [x] 加密/解密功能正常
- [x] 配置文件加载时自动解密
- [x] 错误处理正常（密钥错误等）

### Phase 3.2: 终端工具
- [x] 命令执行正常
- [x] 输出限制生效
- [x] 黑名单命令被阻止
- [x] 过滤规则生效
- [x] 工作目录限制生效
- [x] 超时控制生效
- [x] SSE 流式输出正常

### Phase 3.3: 文件工具集
- [x] `read_file` 正常（含截断）
- [x] `write_file` 正常
- [x] `list_directory` 正常
- [x] `search_file` 正常
- [x] `move_file` 正常
- [x] `move_directory` 正常
- [x] `delete_file` 正常
- [x] `delete_directory` 正常
- [x] `create_directory` 正常
- [x] `search_and_replace` 正常
- [x] `replace_all_keywords` 正常
- [x] 路径遍历防护生效
- [x] 目录限制生效

### Phase 3.4: Checkpoint 集成
- [x] Message `checkpoint_id` 测试（3个测试用例）
- [x] 会话持久化测试（SessionRepository 集成 agentfs）
- [x] Checkpoint 创建正常（create_checkpoint 实现完成）
- [x] `restore_checkpoint` 测试（可选恢复聊天/文件）
- [x] Checkpoint 列表正常（list_checkpoints 实现完成）
- [x] Checkpoint 删除正常（delete_checkpoint 实现完成）
- [x] **Checkpoint 无数量限制，与 session 强关联**
- [x] **删除 session 时自动清理所有相关 checkpoints**
- [x] 自动清理正常（cleanup_expired_checkpoints）
- [x] 会话隔离正常（checkpoint 按 session_id 存储）

### Phase 3.5: 本地工具集成
- [x] 本地工具注册正常
- [x] 工具列表查询正常（包含 MCP 和本地工具）
- [x] 本地工具调用执行正常
- [x] 本地工具与 MCP 工具协同工作正常
- [x] 会话隔离正常

---

## 安全考虑

### 1. 命令黑名单
- 使用正则表达式匹配
- 定期更新内置黑名单
- 支持用户自定义黑名单

### 2. 文件系统隔离
- 严格的路径规范化
- `..` 检测与阻止
- 白名单目录机制
- 符号链接处理

### 3. API Key 安全
- AES-GCM 256 位加密
- 密钥通过环境变量提供
- 不在日志中输出

### 4. 资源限制
- 输出大小限制
- 执行时间限制
- 内存使用监控
