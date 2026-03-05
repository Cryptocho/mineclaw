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

**关键决策**：
- 不使用 agentfs 作为主文件系统（保持现有工具的完整性）
- 使用混合方案：真实文件系统存储快照，agentfs KV 存储元数据
- 对现有代码改动最小，可通过配置开关

**功能描述**：
- 在每个写操作前自动创建 checkpoint
- 与 Phase 3.3 的 10 个文件工具无缝集成
- 支持按会话管理 checkpoints
- 提供手动创建和恢复 checkpoint 的工具
- checkpoint 自动清理策略

**设计文档**：[AGENTFS_GUIDE.md](./AGENTFS_GUIDE.md)

#### 4.1 依赖更新

在 `Cargo.toml` 中添加：
```toml
# Checkpoint 相关
agentfs = "0.2"
agentsql = { version = "0.2", features = ["sqlite"]
```

#### 4.2 配置扩展（src/config.rs）

在现有 `FilesystemConfig` 基础上扩展：

```rust
#[derive(Debug, Deserialize, Clone)]
pub struct FilesystemConfig {
    #[serde(default = "default_max_read_bytes")]
    pub max_read_bytes: usize,
    #[serde(default)]
    pub allowed_directories: Vec<String>,
    // 新增：Checkpoint 配置
    #[serde(default)]
    pub checkpoint: CheckpointConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CheckpointConfig {
    #[serde(default = "default_checkpoint_enabled")]
    pub enabled: bool,
    #[serde(default = "default_checkpoint_dir")]
    pub directory: String,
    #[serde(default = "default_max_checkpoints")]
    pub max_per_session: usize,
    #[serde(default = "default_auto_cleanup_days")]
    pub auto_cleanup_days: u64,
}

// 默认值函数
fn default_checkpoint_enabled() -> bool { true }
fn default_checkpoint_dir() -> String { ".checkpoints".to_string() }
fn default_max_checkpoints() -> usize { 50 }
fn default_auto_cleanup_days() -> u64 { 30 }
```

配置文件扩展：
```toml
# config/mineclaw.toml
[filesystem.checkpoint]
enabled = true
directory = ".checkpoints"
max_per_session = 50
auto_cleanup_days = 30
```

#### 4.3 Checkpoint 管理器（src/checkpoint/mod.rs）

**设计目标**：轻量级、可插拔、与 Phase 3.3 工具无缝集成

```rust
//! Checkpoint 管理器
//! 与 Phase 3.3 文件工具紧密集成

use crate::config::CheckpointConfig;
use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Checkpoint 元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub session_id: String,
    pub created_at: DateTime<Utc>,
    pub description: Option<String>,
    /// 触发此 checkpoint 的操作（如 "write_file", "delete_file" 等）
    pub triggering_operation: Option<String>,
    /// 受影响的文件路径列表
    pub affected_paths: Vec<String>,
    /// 快照总大小（字节）
    pub size_bytes: u64,
}

/// Checkpoint 管理器
pub struct CheckpointManager {
    config: CheckpointConfig,
    base_path: PathBuf,
}

impl CheckpointManager {
    /// 创建新的 CheckpointManager
    pub fn new(config: CheckpointConfig, base_path: impl AsRef<Path>) -> Result<Self> {
        let base_path = base_path.as_ref().join(&config.directory);
        if !base_path.exists() {
            std::fs::create_dir_all(&base_path)?;
        }
        Ok(Self { config, base_path })
    }

    /// 在文件操作前创建 checkpoint（Phase 3.3 工具自动调用）
    ///
    /// 这是与 Phase 3.3 集成的核心方法
    pub async fn before_operation(
        &self,
        session_id: &str,
        operation: &str,
        paths: &[PathBuf],
    ) -> Result<Option<Checkpoint>> {
        if !self.config.enabled {
            return Ok(None);
        }

        // 过滤出实际存在的文件
        let existing_paths: Vec<_> = paths.iter()
            .filter(|p| p.exists())
            .cloned()
            .collect();

        if existing_paths.is_empty() {
            return Ok(None);
        }

        let checkpoint = self.create_checkpoint_internal(
            session_id,
            Some(format!("Before {operation}")),
            Some(operation.to_string()),
            &existing_paths,
        ).await?;

        Ok(Some(checkpoint))
    }

    /// 内部创建 checkpoint
    async fn create_checkpoint_internal(
        &self,
        session_id: &str,
        description: Option<String>,
        triggering_operation: Option<String>,
        affected_paths: &[PathBuf],
    ) -> Result<Checkpoint> {
        let id = Uuid::new_v4().to_string();
        let created_at = Utc::now();

        // 创建会话目录
        let session_dir = self.base_path.join(session_id);
        if !session_dir.exists() {
            std::fs::create_dir_all(&session_dir)?;
        }

        // 创建 checkpoint 目录
        let checkpoint_dir = session_dir.join(&id);
        std::fs::create_dir_all(&checkpoint_dir)?;

        // 复制文件到快照目录
        let mut size_bytes = 0u64;
        let files_dir = checkpoint_dir.join("files");
        std::fs::create_dir_all(&files_dir)?;

        let affected_paths_str: Vec<String> = affected_paths.iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        for path in affected_paths {
            if !path.exists() {
                continue;
            }

            // 计算相对路径用于存储
            let rel_path = pathdiff::diff_paths(path, std::env::current_dir()?)
                .unwrap_or_else(|| path.clone());

            let dest_path = files_dir.join(&rel_path);
            
            // 确保目标目录存在
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // 复制文件或目录
            if path.is_file() {
                std::fs::copy(path, &dest_path)?;
                size_bytes += dest_path.metadata()?.len();
            } else if path.is_dir() {
                // 递归复制目录
                size_bytes += copy_dir_recursive(path, &dest_path)?;
            }
        }

        // 保存元数据
        let checkpoint = Checkpoint {
            id: id.clone(),
            session_id: session_id.to_string(),
            created_at,
            description,
            triggering_operation,
            affected_paths: affected_paths_str,
            size_bytes,
        };

        let metadata_path = checkpoint_dir.join("metadata.json");
        std::fs::write(metadata_path, serde_json::to_string_pretty(&checkpoint)?)?;

        // 检查并清理旧的 checkpoints
        self.cleanup_old_checkpoints(session_id).await?;

        Ok(checkpoint)
    }

    /// 恢复到指定 checkpoint
    pub async fn restore_checkpoint(&self, checkpoint_id: &str) -> Result<()> {
        // 查找 checkpoint
        let (checkpoint, checkpoint_dir) = self.find_checkpoint(checkpoint_id)?;

        // 恢复文件
        let files_dir = checkpoint_dir.join("files");
        if files_dir.exists() {
            for entry in walkdir::WalkDir::new(&files_dir) {
                let entry = entry?;
                if entry.path() == files_dir {
                    continue;
                }

                let rel_path = entry.path().strip_prefix(&files_dir)?;
                let dest_path = std::env::current_dir()?.join(rel_path);

                if entry.file_type().is_file() {
                    // 确保目标目录存在
                    if let Some(parent) = dest_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::copy(entry.path(), &dest_path)?;
                } else if entry.file_type().is_dir() {
                    std::fs::create_dir_all(&dest_path)?;
                }
            }
        }

        Ok(())
    }

    /// 列出会话的所有 checkpoints
    pub async fn list_checkpoints(&self, session_id: &str) -> Result<Vec<Checkpoint>> {
        let session_dir = self.base_path.join(session_id);
        if !session_dir.exists() {
            return Ok(Vec::new());
        }

        let mut checkpoints = Vec::new();
        
        for entry in std::fs::read_dir(session_dir)? {
            let entry = entry?;
            let metadata_path = entry.path().join("metadata.json");
            if metadata_path.exists() {
                let content = std::fs::read_to_string(metadata_path)?;
                let checkpoint: Checkpoint = serde_json::from_str(&content)?;
                checkpoints.push(checkpoint);
            }
        }

        // 按创建时间倒序排列
        checkpoints.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(checkpoints)
    }

    /// 删除 checkpoint
    pub async fn delete_checkpoint(&self, checkpoint_id: &str) -> Result<()> {
        let (_, checkpoint_dir) = self.find_checkpoint(checkpoint_id)?;
        std::fs::remove_dir_all(checkpoint_dir)?;
        Ok(())
    }

    /// 手动创建 checkpoint（通过工具调用）
    pub async fn create_checkpoint(
        &self,
        session_id: &str,
        description: Option<String>,
        paths: Option<Vec<String>>,
    ) -> Result<Checkpoint> {
        let affected_paths = if let Some(paths) = paths {
            paths.iter().map(PathBuf::from).collect()
        } else {
            // 如果未指定路径，使用允许的目录
            Vec::new()
        };

        self.create_checkpoint_internal(
            session_id,
            description,
            Some("manual".to_string()),
            &affected_paths,
        ).await
    }

    // 内部辅助方法
    fn find_checkpoint(&self, checkpoint_id: &str) -> Result<(Checkpoint, PathBuf)> {
        for session_entry in std::fs::read_dir(&self.base_path)? {
            let session_entry = session_entry?;
            if !session_entry.file_type()?.is_dir() {
                continue;
            }

            let checkpoint_dir = session_entry.path().join(checkpoint_id);
            let metadata_path = checkpoint_dir.join("metadata.json");
            
            if metadata_path.exists() {
                let content = std::fs::read_to_string(metadata_path)?;
                let checkpoint: Checkpoint = serde_json::from_str(&content)?;
                return Ok((checkpoint, checkpoint_dir));
            }
        }

        Err(Error::CheckpointNotFound(checkpoint_id.to_string()))
    }

    async fn cleanup_old_checkpoints(&self, session_id: &str) -> Result<()> {
        let session_dir = self.base_path.join(session_id);
        if !session_dir.exists() {
            return Ok(());
        }

        let mut checkpoints = self.list_checkpoints(session_id).await?;
        
        // 如果超过限制，删除最旧的
        if checkpoints.len() > self.config.max_per_session {
            checkpoints.sort_by(|a, b| a.created_at.cmp(&b.created_at));
            
            let to_delete = checkpoints.len() - self.config.max_per_session;
            for checkpoint in &checkpoints[..to_delete] {
                let checkpoint_dir = session_dir.join(&checkpoint.id);
                if checkpoint_dir.exists() {
                    std::fs::remove_dir_all(checkpoint_dir)?;
                }
            }
        }

        Ok(())
    }
}

// 辅助函数：递归复制目录
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<u64> {
    let mut total_size = 0u64;
    
    for entry in walkdir::WalkDir::new(src) {
        let entry = entry?;
        let rel_path = entry.path().strip_prefix(src)?;
        let dest_path = dst.join(rel_path);

        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest_path)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &dest_path)?;
            total_size += dest_path.metadata()?.len();
        }
    }

    Ok(total_size)
}
```

#### 4.4 扩展 ToolContext（src/tools/mod.rs）

```rust
pub struct ToolContext {
    pub session_id: String,
    pub config: Arc<Config>,
    // 新增：Checkpoint 管理器
    pub checkpoint_manager: Option<Arc<CheckpointManager>>,
}
```

#### 4.5 深度集成到 Phase 3.3 文件工具（src/tools/filesystem.rs）

修改每个写操作工具，在操作前自动创建 checkpoint：

```rust
// 示例：修改 WriteFileTool
#[async_trait]
impl LocalTool for WriteFileTool {
    async fn call(&self, arguments: Value, context: ToolContext) -> Result<Value> {
        let params: WriteFileParams = serde_json::from_value(arguments)?;
        let config = get_filesystem_config(&context);

        let path = normalize_and_validate_path(&params.path, &config.allowed_directories)?;

        // === 新增：自动创建 checkpoint ===
        if let Some(cm) = &context.checkpoint_manager {
            cm.before_operation(
                &context.session_id,
                self.name(),
                &[path.clone()],
            ).await?;
        }
        // ==================================

        if let Some(parent) = path.parent()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&path, &params.content)?;

        let result = WriteFileResult {
            success: true,
            bytes_written: params.content.len(),
        };

        Ok(serde_json::to_value(result)?)
    }
}

// 同样修改：
// - MoveFileTool
// - MoveDirectoryTool
// - DeleteFileTool
// - DeleteDirectoryTool
// - CreateDirectoryTool
// - SearchAndReplaceTool
```

#### 4.6 Checkpoint 工具（src/tools/checkpoint.rs）

作为 Phase 3.3 工具集的扩展，实现 4 个新工具：

1. **`create_checkpoint`** - 手动创建 checkpoint
2. **`restore_checkpoint`** - 恢复到指定 checkpoint
3. **`list_checkpoints`** - 列出当前会话的 checkpoints
4. **`delete_checkpoint`** - 删除指定 checkpoint

（详细代码结构见原计划）

#### 4.7 错误类型扩展（src/error.rs）

```rust
pub enum Error {
    // ... 现有错误 ...
    CheckpointNotFound(String),
    CheckpointRestoreFailed(String),
}
```

### 5. 本地工具集成
- **功能描述**: 本地工具直接作为函数集成到 `ToolExecutor`，避免 MCP 延迟
- **实现细节**:
  - 直接实现 `LocalTool` trait
  - 注册到 `ToolRegistry` 作为本地工具
  - 直接函数调用，无进程间通信延迟
  - 工具权限与配置集成
  - 会话隔离

- **架构设计**:
  ```rust
  // src/tools/mod.rs
  #[async_trait]
  pub trait LocalTool: Send + Sync {
      fn name(&self) -> &str;
      fn description(&self) -> &str;
      fn input_schema(&self) -> serde_json::Value;
      async fn call(&self, arguments: serde_json::Value, context: ToolContext) -> Result<serde_json::Value>;
  }

  pub struct ToolContext {
      pub session_id: String,
      pub config: Arc<Config>,
      pub checkpoint_manager: Option<Arc<CheckpointManager>>,
  }

  pub struct LocalToolRegistry {
      tools: HashMap<String, Arc<dyn LocalTool>>,
  }

  impl LocalToolRegistry {
      pub fn new() -> Self;
      pub fn register(&mut self, tool: Arc<dyn LocalTool>);
      pub fn list_tools(&self) -> Vec<Tool>;
      pub async fn call_tool(
          &self,
          tool_name: &str,
          arguments: serde_json::Value,
          context: ToolContext,
      ) -> Result<ToolResult>;
  }
  ```

- **配置示例**:
  ```toml
  # config/mineclaw.toml
  [local_tools]
  enabled = true

  [local_tools.terminal]
  enabled = true

  [local_tools.filesystem]
  enabled = true

  [local_tools.checkpoint]
  enabled = true
  ```

---

## 项目结构变更

```
mineclaw/
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── config.rs              # 扩展配置
│   ├── error.rs               # 扩展错误类型
│   ├── state.rs
│   ├── encryption.rs          # 新增：加密管理
│   ├── tool_coordinator.rs    # 更新：集成本地工具
│   ├── api/
│   │   ├── handlers.rs
│   │   ├── routes.rs
│   │   └── sse.rs
│   ├── models/
│   │   ├── mod.rs
│   │   ├── message.rs
│   │   ├── session.rs
│   │   └── sse.rs
│   ├── llm/
│   │   ├── mod.rs
│   │   └── client.rs
│   ├── mcp/
│   │   ├── mod.rs
│   │   ├── protocol.rs
│   │   ├── transport.rs
│   │   ├── client.rs
│   │   ├── server.rs
│   │   ├── registry.rs          # 更新：支持本地工具
│   │   └── executor.rs          # 更新：支持本地工具执行
│   ├── tools/                   # 新增：本地工具
│   │   ├── mod.rs
│   │   ├── registry.rs          # 本地工具注册表
│   │   ├── terminal.rs          # 终端工具
│   │   ├── filesystem.rs        # 文件工具
│   │   └── checkpoint.rs        # Checkpoint 工具
│   └── checkpoint/              # 新增：Checkpoint 管理
│       ├── mod.rs
│       └── manager.rs
├── src/bin/
│   └── keygen.rs                # 新增：密钥生成工具
├── tests/
│   ├── mcp_integration.rs
│   ├── encryption_tests.rs      # 新增：加密测试
│   ├── terminal_tests.rs        # 新增：终端工具测试
│   ├── filesystem_tests.rs      # 新增：文件工具测试
│   └── checkpoint_tests.rs      # 新增：Checkpoint 测试
└── config/
    └── mineclaw_template.toml    # 更新配置模板
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
- [ ] 添加 agentfs 和 agentsql 依赖
- [ ] 扩展配置结构（`CheckpointConfig`）
- [ ] 实现 `CheckpointManager`（`src/checkpoint/mod.rs`）
  - [ ] `new()` - 创建管理器
  - [ ] `before_operation()` - 操作前自动创建 checkpoint
  - [ ] `create_checkpoint_internal()` - 内部创建逻辑
  - [ ] `restore_checkpoint()` - 恢复 checkpoint
  - [ ] `list_checkpoints()` - 列出 checkpoints
  - [ ] `delete_checkpoint()` - 删除 checkpoint
  - [ ] `cleanup_old_checkpoints()` - 自动清理
- [ ] 扩展 `ToolContext` 添加 `checkpoint_manager`
- [ ] 集成到 Phase 3.3 文件工具（7 个写操作工具）
  - [ ] `write_file`
  - [ ] `move_file`
  - [ ] `move_directory`
  - [ ] `delete_file`
  - [ ] `delete_directory`
  - [ ] `create_directory`
  - [ ] `search_and_replace`
- [ ] 实现 Checkpoint 工具（`src/tools/checkpoint.rs`）
  - [ ] `create_checkpoint` - 手动创建
  - [ ] `restore_checkpoint` - 恢复
  - [ ] `list_checkpoints` - 列表
  - [ ] `delete_checkpoint` - 删除
- [ ] 扩展错误类型（`CheckpointNotFound`、`CheckpointRestoreFailed`）
- [ ] 更新配置模板
- [ ] 编写单元测试（`tests/checkpoint_tests.rs`）

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
agentfs = "0.1"  # 需确认实际版本

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
- [ ] Checkpoint 创建正常
- [ ] Checkpoint 恢复正常
- [ ] Checkpoint 列表正常
- [ ] 自动 checkpoint 在写操作前创建
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
