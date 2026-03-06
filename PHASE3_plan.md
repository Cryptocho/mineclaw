# MineClaw Phase 3 实现计划

## 概述

Phase 3: 本地工具集与安全增强。在这一阶段，我们将实现核心的本地工具集，包括终端工具、文件工具，并集成 agentfs 实现 checkpoint 功能，同时增强 API Key 的安全性。

---

## 目标

- ✅ API Key 加密存储
- ✅ 终端工具（带输出限制和过滤）
- ✅ 文件读写工具（仅指定的 9 个工具）
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

- **数据结构**:
  ```rust
  // src/encryption.rs
  pub struct EncryptionManager {
      key: [u8; 32],
  }

  impl EncryptionManager {
      pub fn new(key: &str) -> Result<Self>;
      pub fn encrypt(&self, plaintext: &str) -> Result<String>;
      pub fn decrypt(&self, ciphertext: &str) -> Result<String>;
      pub fn generate_key() -> String;
  }
  ```

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

- **工具定义**:
  ```rust
  // src/tools/terminal.rs
  pub struct TerminalTool {
      config: TerminalConfig,
  }

  #[derive(Debug, Deserialize, Clone)]
  pub struct TerminalConfig {
      pub max_output_bytes: usize,
      pub timeout_seconds: u64,
      pub allowed_workspaces: Vec<String>,
      pub command_blacklist: Vec<String>,
      pub filters: HashMap<String, Vec<String>>,
  }

  pub struct RunCommandParams {
      pub command: String,
      pub args: Vec<String>,
      pub cwd: Option<String>,
      pub stream_output: Option<bool>,
  }

  pub struct RunCommandResult {
      pub exit_code: i32,
      pub stdout: String,
      pub stderr: String,
      pub truncated: bool,
  }
  ```

### 3. 文件工具集
- **功能描述**: 提供 9 个指定的文件操作工具
- **实现细节**:
  - 所有读取操作有最大文本限制（默认 16KB）
  - 工作目录限制（基于配置）
  - 路径遍历防护（`..` 检查）
  - 基于 agentfs 的 checkpoint 集成

- **工具列表**:

  1. **read_file** - 读取完整文件内容
     ```rust
     pub struct ReadFileParams {
         pub path: String,
     }
     pub struct ReadFileResult {
         pub content: String,
         pub truncated: bool,
         pub total_bytes: usize,
     }
     ```

  2. **write_file** - 创建新文件或完全覆盖现有文件
     ```rust
     pub struct WriteFileParams {
         pub path: String,
         pub content: String,
     }
     pub struct WriteFileResult {
         pub success: bool,
         pub bytes_written: usize,
     }
     ```

  3. **list_directory** - 列出目录内容
     ```rust
     pub struct ListDirectoryParams {
         pub path: String,
         pub recursive: Option<bool>,
     }
     pub struct DirectoryEntry {
         pub name: String,
         pub path: String,
         pub is_dir: bool,
         pub size: Option<u64>,
         pub modified: Option<DateTime<Utc>>,
     }
     pub struct ListDirectoryResult {
         pub entries: Vec<DirectoryEntry>,
     }
     ```

  4. **search_file** - 在文件中搜索特定文本模式
     ```rust
     pub struct SearchFileParams {
         pub path: String,
         pub pattern: String,
         pub case_sensitive: Option<bool>,
     }
     pub struct SearchMatch {
         pub line_number: usize,
         pub line_content: String,
         pub start_column: usize,
         pub end_column: usize,
     }
     pub struct SearchFileResult {
         pub matches: Vec<SearchMatch>,
         pub total_matches: usize,
     }
     ```

  5. **move_file** - 移动或重命名文件
     ```rust
     pub struct MoveFileParams {
         pub source: String,
         pub destination: String,
         pub overwrite: Option<bool>,
     }
     pub struct MoveFileResult {
         pub success: bool,
     }
     ```

  6. **move_directory** - 移动或重命名目录
     ```rust
     pub struct MoveDirectoryParams {
         pub source: String,
         pub destination: String,
         pub overwrite: Option<bool>,
     }
     pub struct MoveDirectoryResult {
         pub success: bool,
     }
     ```

  7. **delete_file** - 删除文件
     ```rust
     pub struct DeleteFileParams {
         pub path: String,
     }
     pub struct DeleteFileResult {
         pub success: bool,
     }
     ```

  8. **delete_directory** - 删除目录
     ```rust
     pub struct DeleteDirectoryParams {
         pub path: String,
         pub recursive: Option<bool>,
     }
     pub struct DeleteDirectoryResult {
         pub success: bool,
     }
     ```

  9. **create_directory** - 创建新目录
     ```rust
     pub struct CreateDirectoryParams {
         pub path: String,
         pub parents: Option<bool>,
     }
     pub struct CreateDirectoryResult {
         pub success: bool,
     }
     ```

  10. **search_and_replace** - 精准替换文件中的特定内容
      ```rust
      pub struct SearchAndReplaceParams {
          pub path: String,
          pub search: String,
          pub replace: String,
          pub case_sensitive: Option<bool>,
      }
      pub struct SearchAndReplaceResult {
          pub success: bool,
          pub replacements_made: usize,
      }
      ```

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

#### Message 结构更新

```rust
// src/models/message.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub session_id: Uuid,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_result: Option<ToolResult>,
    pub checkpoint_id: Option<String>,  // 新增：关联的 checkpoint ID
}
```

#### 数据结构设计

**Checkpoint 管理器** (`src/tools/checkpoint.rs`):
```rust
pub struct CheckpointManager {
    agent_fs: AgentFS,
    config: CheckpointConfig,
}

pub struct CheckpointConfig {
    pub enabled: bool,
    pub checkpoint_directory: String,
    pub max_checkpoints_per_session: usize,
    pub auto_cleanup_days: u32,
}

pub struct Checkpoint {
    pub id: String,
    pub session_id: String,
    pub timestamp: DateTime<Utc>,
    pub description: Option<String>,
    pub checkpoint_type: CheckpointType,
    pub affected_files: Vec<String>,
}

pub enum CheckpointType {
    Auto,    // 自动创建（文件操作前）
    Manual,  // 手动创建
}

pub struct CheckpointSnapshot {
    pub session: Session,
    pub files: HashMap<String, Vec<u8>>,
}
```

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
- [ ] 设计加密模块架构
- [ ] 实现 `EncryptionManager`
- [ ] 集成到配置加载流程
- [ ] 创建密钥生成工具 (`src/bin/keygen.rs`)
- [ ] 编写单元测试
- [ ] 更新配置模板

### Phase 3.2: 终端工具
- [ ] 设计终端工具配置结构
- [ ] 实现命令黑名单检查
- [ ] 实现输出过滤系统
- [ ] 实现 `TerminalTool`
- [ ] 集成 SSE 流式输出
- [ ] 编写单元测试

### Phase 3.3: 文件工具集
- [x] 设计文件工具配置结构
- [x] 实现路径安全检查
- [x] 实现 10 个文件工具
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
- [x] 集成读取大小限制
- [x] 编写单元测试

**Phase 3.3 已完成** ✅
- 实现了全部 10 个文件工具
- 路径安全检查（路径遍历防护、目录白名单）
- 完整的单元测试（9 个测试）
- 所有 62+9+3 = 74 个测试通过
- `search_and_replace` 使用 SEARCH/REPLACE 块格式（单个 `diff` 参数）
- `search_file` 支持正则表达式

详细内容请参考 [PHASE3_3.md](./PHASE3_3.md)

### Phase 3.4: Checkpoint 集成（与 Phase 3.3 紧密结合）

#### Phase 3.4.1: 依赖与基础结构
- [ ] 添加 `agentfs` 和 `agentsql` 依赖到 `Cargo.toml`
- [ ] 创建 `src/tools/checkpoint.rs`
- [ ] 实现 `CheckpointConfig` 配置结构
- [ ] 实现 `Checkpoint`、`CheckpointType`、`CheckpointSnapshot` 数据结构
- [ ] 实现 `CheckpointManager` 基础结构

#### Phase 3.4.2: Message 结构更新
- [ ] 修改 `Message` 添加 `checkpoint_id: Option<String>`
- [ ] 更新 `Message::new()` 设置 `checkpoint_id = None`
- [ ] 添加 `with_checkpoint_id()` builder 方法
- [ ] 更新消息序列化/反序列化测试

#### Phase 3.4.3: SessionRepository 持久化
- [ ] 修改 `SessionRepository` 集成 agentfs
- [ ] 实现会话保存到 agentfs（JSON 格式）
- [ ] 实现启动时从 agentfs 加载会话
- [ ] 实现会话自动保存（add_message/update 时）
- [ ] 测试会话持久化

#### Phase 3.4.4: Checkpoint 核心功能
- [ ] 实现 `CheckpointManager::new()` - 初始化 agentfs
- [ ] 实现 `create_checkpoint()` - 创建 checkpoint（会话+文件），返回 checkpoint_id
- [ ] 实现 `restore_chat()` - 仅恢复会话到指定 checkpoint
- [ ] 实现 `restore_all()` - 恢复会话+文件到指定 checkpoint
- [ ] 实现 `list_checkpoints()` - 列出 checkpoints
- [ ] 实现 `delete_checkpoint()` - 删除 checkpoint
- [ ] 实现自动清理旧 checkpoints

#### Phase 3.4.5: 集成到文件工具
- [ ] 修改 `ToolContext` 添加 `checkpoint_manager`
- [ ] 修改文件工具返回 `Result<(Value, Option<String>)>`（结果 + checkpoint_id）
- [ ] 在 `write_file` 前自动创建 checkpoint，返回 checkpoint_id
- [ ] 在 `move_file` 前自动创建 checkpoint，返回 checkpoint_id
- [ ] 在 `move_directory` 前自动创建 checkpoint，返回 checkpoint_id
- [ ] 在 `delete_file` 前自动创建 checkpoint，返回 checkpoint_id
- [ ] 在 `delete_directory` 前自动创建 checkpoint，返回 checkpoint_id
- [ ] 在 `create_directory` 前自动创建 checkpoint，返回 checkpoint_id
- [ ] 在 `search_and_replace` 前自动创建 checkpoint，返回 checkpoint_id

#### Phase 3.4.6: 集成到消息流
- [ ] 修改工具调用流程，将 checkpoint_id 设置到 ToolCall/ToolResult 消息中
- [ ] 更新 `ToolExecutor` 处理 checkpoint_id

#### Phase 3.4.7: Checkpoint 工具暴露
- [ ] 实现 `CreateCheckpointTool`
- [ ] 实现 `RestoreChatTool`
- [ ] 实现 `RestoreAllTool`
- [ ] 实现 `ListCheckpointsTool`
- [ ] 实现 `DeleteCheckpointTool`
- [ ] 注册到 `LocalToolRegistry`

#### Phase 3.4.8: 集成到 AppState
- [ ] 修改 `AppState` 添加 `CheckpointManager`
- [ ] 更新 `main.rs` 初始化 `CheckpointManager`
- [ ] 更新 `main.rs` 初始化持久化 `SessionRepository`

#### Phase 3.4.9: 测试
- [ ] Message `checkpoint_id` 测试
- [ ] 会话持久化测试
- [ ] Checkpoint 创建测试
- [ ] `restore_chat` 测试
- [ ] `restore_all` 测试
- [ ] Checkpoint 列表测试
- [ ] 自动 checkpoint 测试（验证消息中的 checkpoint_id）
- [ ] 自动清理测试
- [ ] 会话隔离测试

### Phase 3.5: 本地工具集成
- [ ] 实现 `LocalTool` trait
- [ ] 实现 `LocalToolRegistry`
- [ ] 更新 `ToolRegistry` 支持本地工具
- [ ] 更新 `ToolExecutor` 支持本地工具执行
- [ ] 更新 `ToolCoordinator` 集成本地工具
- [ ] 更新 `AppState` 添加本地工具注册表
- [ ] 编写集成测试

### Phase 3.6: 集成与测试
- [ ] 更新配置加载流程
- [ ] 更新 `AppState` 初始化
- [ ] 完整端到端测试
- [ ] 更新文档
- [ ] 安全审计

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
- [ ] 密钥生成正常
- [ ] 加密/解密功能正常
- [ ] 配置文件加载时自动解密
- [ ] 错误处理正常（密钥错误等）

### Phase 3.2: 终端工具
- [ ] 命令执行正常
- [ ] 输出限制生效
- [ ] 黑名单命令被阻止
- [ ] 过滤规则生效
- [ ] 工作目录限制生效
- [ ] 超时控制生效
- [ ] SSE 流式输出正常

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
- [x] 路径遍历防护生效
- [x] 目录限制生效

### Phase 3.4: Checkpoint 集成
- [ ] Message `checkpoint_id` 测试
- [ ] 会话持久化测试
- [ ] Checkpoint 创建正常
- [ ] `restore_chat` 测试（仅恢复聊天）
- [ ] `restore_all` 测试（恢复聊天+文件）
- [ ] Checkpoint 列表正常
- [ ] 自动 checkpoint 在写操作前创建
- [ ] 自动 checkpoint 关联到消息（checkpoint_id 在 Message 中）
- [ ] 自动清理正常
- [ ] 会话隔离正常

### Phase 3.5: 本地工具集成
- [ ] 本地工具注册正常
- [ ] 工具列表查询正常（包含 MCP 和本地工具）
- [ ] 本地工具调用执行正常
- [ ] 本地工具与 MCP 工具协同工作正常
- [ ] 会话隔离正常

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
- 内存使用监控（可选）

---

## 后续扩展方向

Phase 3 完成后，可以考虑：

1. **更多工具**: Git 工具、Docker 工具、数据库工具等
2. **Web UI**: 提供图形化界面管理会话和工具
3. **多用户**: 支持多用户隔离和权限管理
4. **插件系统**: 支持自定义工具插件
5. **审计日志**: 详细的操作审计和回放
