# AgentFS 库使用指南

## 目录
- [什么是 AgentFS](#什么是-agentfs)
- [核心功能](#核心功能)
- [快速开始](#快速开始)
- [API 参考](#api-参考)
- [使用示例](#使用示例)

---

## 什么是 AgentFS

**AgentFS** 是一个为 AI 代理设计的 Rust 库，提供了完整的文件系统抽象和持久化解决方案。它构建在 `agentdb` 抽象层和 `agentsql` SQL 后端之上，提供零厂商锁定的存储方案。

### 主要特点

- ✅ **POSIX 风格文件系统** - 类 Unix 的文件操作
- ✅ **多后端支持** - SQLite（本地）、PostgreSQL（生产）、MySQL（云部署）
- ✅ **内置工具调用审计** - 完整的操作追踪
- ✅ **零厂商锁定** - 完全开源（MIT 许可证）
- ✅ **自托管** - 不依赖云服务
- ✅ **多代理原生支持** - 安全的多代理数据共享

---

## 核心功能

### 1. 文件系统操作

| 方法 | 描述 |
|------|------|
| `write_file(path, data)` | 创建或覆盖文件，自动创建父目录 |
| `read_file(path)` | 读取完整文件内容到内存 |
| `mkdir(path)` | 递归创建目录 |
| `readdir(path)` | 列出目录内容及元数据 |
| `remove(path)` | 递归删除文件和目录 |
| `exists(path)` | 检查路径是否存在 |
| `stat(path)` | 获取文件元数据（大小、时间戳、权限、类型） |
| `lstat(path)` | 获取文件元数据（不跟随符号链接） |
| `symlink(target, linkpath)` | 创建符号链接 |
| `readlink(path)` | 读取符号链接目标 |

### 2. 键值存储

| 方法 | 描述 |
|------|------|
| `set(key, value)` | 存储任意键值对 |
| `get(key)` | 通过键检索值 |
| `delete(key)` | 永久删除键 |
| `scan(prefix)` | 查找匹配前缀的所有键 |
| `exists(key)` | 检查键是否存在 |

### 3. 工具调用审计

| 方法 | 描述 |
|------|------|
| `start(name, params)` | 开始跟踪工具调用及参数 |
| `success(id, result)` | 标记工具调用成功，可选结果 |
| `error(id, error)` | 标记工具调用失败，错误信息 |
| `record(...)` | 单次操作记录已完成的工具调用 |
| `statistics(name)` | 获取工具统计信息（总调用数、成功率、平均时长） |
| `recent(n)` | 列出最近的工具调用 |

---

## 快速开始

### 安装要求

- Rust 1.70 或更高版本
- 数据库后端（SQLite、PostgreSQL 或 MySQL）

### 添加依赖

在 `Cargo.toml` 中添加：

```toml
[dependencies]
agentfs = "0.2"
agentsql = { version = "0.2", features = ["sqlite"] }

# 对于 PostgreSQL:
# agentsql = { version = "0.2", features = ["postgres"] }

# 对于 MySQL:
# agentsql = { version = "0.2", features = ["mysql"] }
```

或者使用 cargo 命令：

```bash
cargo add agentfs
cargo add agentsql --features sqlite
```

---

## API 参考

### AgentFS 结构体

主结构体，提供文件系统、KV 存储和工具记录功能。

```rust
pub struct AgentFS {
    pub fs: DbFileSystem,      // 文件系统
    pub kv: DbKvStore,          // 键值存储
    pub tools: DbToolRecorder,  // 工具记录器
}
```

### 创建实例

```rust
AgentFS::new(backend: Box<dyn AgentDB>, agent_name: &str, mount_point: &str) -> Result<Self>
```

---

## 使用示例

### 示例 1: SQLite 基本操作

```rust
use agentfs::{AgentFS, FileSystem, KvStore, ToolRecorder};
use agentsql::SqlBackend;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建 SQLite 后端的 AgentFS
    let backend = SqlBackend::sqlite("agent.db").await?;
    let agent_fs = AgentFS::new(Box::new(backend), "my-agent", "/agent").await?;

    // 文件系统操作
    agent_fs.fs.mkdir("/output").await?;
    agent_fs.fs.write_file("/output/report.txt", b"Hello, World!").await?;
    
    let content = agent_fs.fs.read_file("/output/report.txt").await?.unwrap();
    println!("文件内容: {}", String::from_utf8_lossy(&content));

    // 列出目录
    let entries = agent_fs.fs.readdir("/output").await?;
    for entry in entries {
        println!("{}: {} 字节", entry.name, entry.size);
    }

    // 键值存储
    agent_fs.kv.set("config:theme", b"dark").await?;
    let theme = agent_fs.kv.get("config:theme").await?.unwrap();
    println!("主题: {}", String::from_utf8_lossy(&theme));

    // 工具调用审计
    let id = agent_fs.tools.start(
        "web_search",
        Some(serde_json::json!({ "query": "Rust async programming" }))
    ).await?;
    
    // 模拟搜索...
    agent_fs.tools.success(
        id,
        Some(serde_json::json!({ "results": 10, "duration_ms": 123 }))
    ).await?;

    // 获取统计信息
    let stats = agent_fs.tools.statistics("web_search").await?;
    println!("成功率: {:.1}%", stats.success_rate * 100.0);

    Ok(())
}
```

### 示例 2: PostgreSQL 生产环境

```rust
use agentfs::AgentFS;
use agentsql::SqlBackend;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 连接到 PostgreSQL
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://user:pass@localhost/agentfs".to_string());
    
    let backend = SqlBackend::postgres(database_url).await?;
    let agent_fs = AgentFS::new(Box::new(backend), "prod-agent", "/agent").await?;

    // 与 SQLite 相同的 API！
    agent_fs.fs.write_file("/logs/app.log", b"System started").await?;

    // 扫描 KV 存储
    agent_fs.kv.set("session:user123", b"active").await?;
    let sessions = agent_fs.kv.scan("session:").await?;
    println!("找到 {} 个活动会话", sessions.len());

    Ok(())
}
```

### 示例 3: MySQL 云部署

```rust
use agentfs::AgentFS;
use agentsql::SqlBackend;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 连接到 MySQL（如 AWS Aurora）
    let backend = SqlBackend::mysql(
        "mysql://user:pass@aurora-cluster.region.rds.amazonaws.com/agentfs"
    ).await?;
    
    let agent_fs = AgentFS::new(Box::new(backend), "cloud-agent", "/agent").await?;

    // 多代理协作
    agent_fs.fs.mkdir("/shared").await?;
    agent_fs.fs.write_file("/shared/status.json", b"{\"status\":\"ready\"}").await?;

    // 列出所有代理的最近工具调用
    let recent = agent_fs.tools.recent(10).await?;
    for call in recent {
        println!("{}: {} ({})", call.name, call.status, call.duration_ms);
    }

    Ok(())
}
```

---

## 架构概览

```
┌─────────────────────────────────────────────────────────┐
│                  Application Layer                       │
│  • Agent frameworks (Rig, LangChain, custom)           │
│  • Multi-agent systems                                    │
│  • CLI tools and services                                 │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│                    AgentFS APIs                          │
│  ┌──────────────┬─────────────┬──────────────────┐    │
│  │  FileSystem  │   KvStore   │   ToolRecorder   │    │
│  │              │             │                  │    │
│  │ • mkdir      │ • set       │ • start          │    │
│  │ • write_file │ • get       │ • success        │    │
│  │ • read_file  │ • delete    │ • error          │    │
│  │ • readdir    │ • scan      │ • record         │    │
│  │ • remove     │ • exists    │ • statistics     │    │
│  │ • stat       │             │ • recent         │    │
│  └──────────────┴─────────────┴──────────────────┘    │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│              AgentDB Trait Interface                     │
│  • Database-agnostic operations                          │
│  • put, get, delete, scan, query                        │
│  • Transaction support                                    │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│                AgentSQL (SQLx)                           │
│  • Connection pooling                                    │
│  • Migration system                                      │
│  • Type-safe SQL                                         │
│  • Multi-backend support                                 │
└───┬────────────────┬────────────────┬───────────────┘
    │                │                │
┌───▼──────┐ ┌──────▼──────┐ ┌─────▼──────┐
│  SQLite  │ │ PostgreSQL  │ │   MySQL    │
│  Local   │ │ Production  │ │   Cloud    │
└──────────┘ └─────────────┘ └────────────┘
```

---

## 应用场景

1. **代理工作区** - 为代理提供隔离的文件系统工作区来存储输出
2. **多代理系统** - 通过通用文件系统在代理间共享数据
3. **工具调用审计** - 使用内置审计日志跟踪所有代理操作
4. **状态管理** - 在 KV 存储中存储代理配置和会话状态
5. **输出存储** - 持久化代理生成的文档、报告和工件
6. **云部署** - 部署在托管数据库上（AWS RDS、Google Cloud SQL、Azure）

---

## 与其他方案的对比

| 特性 | AgentFS | 其他方案 |
|------|---------|----------|
| 后端选择 | SQLite、PostgreSQL、MySQL | 厂商特定 |
| 开源 | ✅ MIT 许可证 | ⚠️ 各不相同 |
| 自托管 | ✅ 是 | ❌ 仅云 |
| 工具审计 | ✅ 内置 | ❌ 不包含 |
| 零成本 | ✅ 是 | ❌ 按使用量定价 |
| 本地开发 | ✅ SQLite | ⚠️ 需要云账户 |
| POSIX 风格 API | ✅ 是 | ⚠️ 有限 |
| 多代理 | ✅ 原生支持 | ⚠️ 需要变通方案 |

---

## 相关链接

- **crates.io**: https://crates.io/crates/agentfs
- **文档**: https://docs.rs/agentfs
- **GitHub**: https://github.com/cryptopatrick/agentfs
- **作者**: CryptoPatrick

---

## 许可证

MIT 许可证