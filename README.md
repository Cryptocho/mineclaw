<div align="center">

# 🦞 MineClaw

**多 Agent 并行 + 自动化编排协作的新一代 AI 智能体平台**

[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](docs/PLAN.md)

</div>

## ✨ 项目特性

### 🚀 革命性的多 Agent 架构

- **混合协作模式**：集群并行 + 接力转交，灵活应对各种场景
- **层级化总控**：主总控 + 分支总控，专业分工，高效协作
- **智能路由**：自动判断任务复杂度，选择最优协作策略

### 🧠 上下文管理

- **轻量但强大**：专门的上下文管理 Agent，负责裁剪、总结、转交判断
- **智能裁剪**：像编辑文件一样优化上下文，保持专注
- **质量评估**：裁剪效果可评分，为持续优化积累数据

### 🛠️ 丰富的工具生态

- **MCP 集成**：完整的 Model Context Protocol 支持
- **本地工具**：文件操作、终端执行、Checkpoint 管理
- **精细掩码**：按工具粒度控制权限，终端工具全开放

### 🔒 安全与可靠

- **API Key 加密**：安全存储敏感信息
- **Checkpoint 机制**：随时可以回滚，由 AgentFS 提供强力支持
- **单向流水线**：简化协作逻辑，完整的消息追踪

## 🎯 为什么选择 MineClaw？

| 特性 | 传统 Claw | MineClaw |
|------|-----------|----------|
| 执行模式 | 单线程顺序 | 多 Agent 并行 |
| 上下文管理 | 人工切换 | 智能裁剪优化 |
| 任务编排 | 人工引导 | 自动化协作 |
| 能力范围 | 单一模型 | 专业分工组合 |
| 工作效率 | 串行处理 | 并行加速 |

## 📖 文档

- [项目计划 (PLAN.md)](docs/PLAN.md) - 详细的开发路线图
- [Claw 定义 (CLAW_DEFINITION.md)](docs/CLAW_DEFINITION.md) - 了解什么是 Claw
- [Phase 1 文档](docs/PHASE1.md) - 基础消息流转设计
- [Phase 2 文档](docs/PHASE2.md) - MCP 集成设计
- [Phase 3 文档](docs/PHASE3.md) - 本地工具与 Checkpoint 集成
- [Phase 4 文档 (进行中)](docs/PHASE4_inprogress.md) - 多 Agent 基础架构

## 🚀 快速开始

### 前置要求

- Rust 1.75+
- Tokio 异步运行时
- 一个 LLM API Key（OpenAI、Ollama 等）

### 安装

```bash
# 克隆仓库
git clone https://github.com/Cryptocho/mineclaw.git
cd mineclaw

# 构建项目
cargo build --release

# 运行服务
cargo run
```

### 配置

复制配置模板并编辑：

```bash
# 配置文件位于 config/ 目录
# 根据你的需求调整设置
```

## 🏗️ 架构设计

### 整体协作模式

```
用户请求
    │
    ▼
┌─────────────────────────────────────┐
│     路由模型 (Router Model)         │
│  任务开始 & 上下文裁剪后判断        │
└─────────────────────────────────────┘
    │
    ├─ 简单任务 → 单总控 Agent
    │
    └─ 复杂任务
        │
        ├─ 集群模式：创意脑洞 / API查询 / 大项目分工 / 多进程测试
        │   └─ 分支总控 → 多个 Agent 并行工作
        │
        └─ 接力模式：持续犯错 / 复杂项目分阶段
            └─ Agent A → [JSON 工单] → Agent B → [JSON 工单] → Agent C ...
```

### 核心组件

- **Agent Pool**：管理 Agent 生命周期
- **消息总线**：Agent 间通信基础设施
- **上下文管理 Agent**：裁剪、总结、转交判断
- **任务编排器**：分解、分配、调度任务
- **工单系统**：接力转交时的信息传递
- **Checkpoint 管理器**：状态管理与回滚

## 🗺️ 路线图

### ✅ 已完成

- **Phase 1**：基础消息流转 ✅
- **Phase 2**：MCP 集成 ✅
- **Phase 3**：本地工具与 Checkpoint 集成 ✅

### 📋 未来计划

- **Phase 4**：多 Agent 基础架构 🔄（进行中）
- **Phase 5**：任务编排与路由系统
- **Phase 6**：高级协作与质量保障
- **Phase 7**：经验学习与持续进化
- **Phase 8**：Flutter 前端

详细信息请查看 [PLAN.md](docs/PLAN.md)。

## 💡 核心设计理念

### Baby Steps™ 方法论

1. **最小有意义的改变**：每次只做一件事
2. **过程就是产品**：学习和执行的过程最重要
3. **一次一个实质性成果**：完成一个再开始下一个
4. **完整完成每一步**：实现、验证、文档缺一不可
5. **增量验证**：每一步都要验证
6. **专注记录每一步**：文档和代码同样重要

### 关键设计决策

1. **混合协作模式**：灵活组合集群和接力
2. **层级化总控**：避免单点故障，职责清晰
3. **上下文管理是核心**：轻量但关键的角色
4. **单向流水线**：简化协作，完整追踪
5. **为未来预留接口**：从 Phase 6 就开始为经验学习做准备

### 核心设计共识（重要！）

在 2026 年的时间节点上，我们的设计基于以下明确共识：

| # | 设计澄清 | 说明 |
|---|---------|------|
| 1 | **CMA 的定位是注意力管理，不是容量管理** | 即使有 200k+ 上下文窗口，保持高信噪比依然重要。CMA 裁剪无关上下文是为了提升模型输出质量，而不是防止超过窗口限制。 |
| 2 | **API 便宜使能多 Agent 设计** | 低成本 token 让我们可以负担多个 Agent 并行探索，这不是浪费，而是利用价格优势获得更好的结果。 |
| 3 | **模型强才敢给总控这么大自由** | 正是因为当前模型足够强大，我们才信任总控去创建 Agent、编写提示词、规划工作流。这是利用模型能力，不是弥补模型弱点。 |
| 4 | **CMA 只有两个触发点** | Agent 主动求助、上下文满载。没有"持续监控"或"过早介入"。 |
| 5 | **线性设计避免同文件冲突** | 单向流水线 + 接口约定，根本不会出现同一个文件被多个 Agent 编辑的情况。 |
| 6 | **转交通过总控进行** | Agent A → 总控 → 总控 spawn Agent B，不是 A 直接给 B。 |
| 7 | **EvoMap 是已有项目** | evomap.ai 是真实存在的基础设施，Phase 9 是集成，不是探索。 |

### 常见问题解答

| 问题 | 回答 |
|------|------|
| **编排延迟叠加会不会影响体验？** | MineClaw 主要针对后台/自动模式设计，而非实时交互。对于需要长时间运行的任务，编排延迟不是问题。 |
| **工单转交会不会导致信息损失？** | OpenViking 作为上下文数据库会完整保留信息。工单是简洁的摘要，而不是唯一的信息载体。 |
| **纯后端集成测试会不会很困难？** | 我们计划在 Phase 4-6 完成后优先开发 Flutter 前端，通过可视化来辅助测试和调试。 |
| **模型会不会过度自信从不求助？** | 总控会有轻量级的进度监控，发现 Agent 原地打转时会**建议**（不强制）Agent 考虑求助。 |
| **Checkpoint 粒度会不会有问题？** | 每个 Agent 会维护自己的 Checkpoint 列表，并记录 `agent_start_checkpoint_id`，支持灵活的回退策略。 |

### 讨论中确定的改进点

以下改进点来自深入的设计讨论，将在相关阶段中实施：

| # | 改进点 | 关联 Phase | 简要说明 |
|---|--------|-----------|---------|
| 1 | **Per-agent Checkpoint 列表 + `agent_start_checkpoint_id`** | 4.3 | Session 中按 Agent 分组 Checkpoint，记录每个 Agent 的起始 Checkpoint |
| 2 | **CMA 上下文阈值自适应** | 5.1 + 6.3 | 成功处理后放宽阈值，频繁失败则收紧，形成反馈闭环 |
| 3 | **语义复述确认（简化版）** | 6.1 | 总控 spawn 新 Agent 后，让新 Agent 复述任务理解，总控确认 |
| 4 | **实时成本预估 + 阈值告警** | 7.6 | 从"事后统计"升级到"实时预估"，超阈值触发用户确认 |

## 🛠️ 技术栈

### 后端

- **Rust** - 主要开发语言
- **Axum** - Web 框架
- **Tokio** - 异步运行时
- **AgentFS** - Checkpoint 管理
- **AgentSQL** - 数据持久化

### AI/ML

- OpenAI API 兼容接口
- Ollama 本地模型支持
- OpenViking - 上下文数据库
- EvoMap (evomap.ai) - AI 自我进化基础设施（已有项目）

## 🤝 贡献

欢迎贡献！请查看：

- 提交 Issue 报告 bug 或提出新功能
- 提交 PR 改进代码或文档
- 参与讨论，分享想法

## 📄 许可证

MIT License - 详见 [LICENSE](LICENSE) 文件

## 🙏 致谢

- **OpenClaw** - Claw 概念的先驱
- **AgentFS** - 优秀的 Checkpoint 管理库
- **所有贡献者** - 感谢参与这个项目的每一个人

---

<div align="center">

**Made with ❤️ and 🦞 by the MineClaw team**

[⭐ Star us on GitHub](https://github.com/Cryptocho/mineclaw)

</div>
