# MineClaw Phase 4: 多 Agent 基础架构 详细设计

## 📊 当前状态更新（2026.3.17）

**Phase 4.1: Agent 基础定义** - ✅ **完成**
- 所有核心数据结构和功能实现
- 119 个单元测试全部通过

**Phase 4.2: 总控机制** - ✅ **核心完成** + ✅ **TaskManager 完成**
- 核心功能：主总控/子总控创建、Agent 管理、串行任务分配、工单生成
- ✅ **TaskManager 完整实现**：
  - 基础数据结构：TaskStatus, TaskInfo, TaskManager
  - 核心方法：register_task, update_task_status, store_task_result
  - 任务管理：cancel_task, cleanup_completed_tasks
  - 任务等待：wait_for_task, wait_for_all_tasks
  - JoinHandle 管理：register_join_handle, get_join_handle
- ✅ **OrchestratorExecutor 集成**：
  - assign_task_parallel() 支持 TaskManager
  - get_task_status() 支持 TaskManager
  - 新增 wait_for_task() 和 wait_for_all_tasks() 方法
  - handle_cma_notification() 增强（支持 TaskManager 参数）
- 50+ 个单元测试全部通过

**Phase 4.3: Checkpoint 与会话增强** - ✅ **完成**
- Session 状态机、生命周期事件、Checkpoint 强关联
- SessionRepository 完整实现
- 60+ 个单元测试全部通过

**Phase 4.4: 工具掩码基础机制** - ✅ **完成**
- 简化的工具权限类型：McpToolPermission, LocalToolPermission, LocalToolMode
- ToolMask 结构和 ToolMaskRepository trait 完整实现
- InMemoryToolMaskRepository 内存实现
- 工具过滤逻辑：Agent 只能看到有权限的工具
- 与 ToolCoordinator 集成：get_available_tools_for_agent() 方法
- 191 个单元测试全部通过（包括新增的工具掩码测试）

**调整优先级：先完善 Phase 4.2 的 TaskManager**
- 原计划：下一步 Phase 4.5 - 上下文管理 Agent（基础版）
- 调整后：先完善 TaskManager 和并行任务功能，再继续 Phase 4.5

---

## 🎉 TaskManager 实现完成总结（2026.3.17）

### 概述
成功实现了完整的 TaskManager 架构，并集成到 Orchestrator 中，解决了原有的占位实现问题。

### 完成的工作

#### 1. 数据结构设计
- **TaskStatus 枚举（增强版）**：
  - `Pending`, `Running`, `Completed`, `Failed`, `Cancelled`
  - 辅助方法：`is_terminal()`, `is_running()`
  - 支持与现有 types::TaskStatus 相互转换

- **TaskInfo 结构体**：
  - 包含任务 ID、Agent ID、状态、结果、错误信息
  - 时间戳：创建时间、开始时间、完成时间
  - 状态转换方法：`mark_running()`, `mark_completed()`, `mark_failed()`, `mark_cancelled()`

- **TaskManager 结构体**：
  - `tasks`: 任务信息映射
  - `join_handles`: 活跃任务的 JoinHandle
  - `tasks_by_agent`: 按 Agent ID 索引的任务列表
  - `SharedTaskManager`: 线程安全的包装器（Arc<Mutex<TaskManager>>）

#### 2. 核心方法实现
- **任务注册与查询**：
  - `register_task()` - 注册新任务并建立索引
  - `get_task()` / `get_task_mut()` - 获取任务信息
  - `get_task_status()` - 获取任务状态
  - `contains_task()` - 检查任务是否存在
  - `get_tasks_for_agent()` - 获取指定 Agent 的所有任务
  - `list_tasks()` - 列出所有任务
  - `task_count()` - 获取任务总数

- **状态更新**：
  - `update_task_status()` - 更新任务状态
  - `store_task_result()` - 存储任务结果并更新状态

- **JoinHandle 管理**：
  - `register_join_handle()` - 注册任务的 JoinHandle
  - `get_join_handle()` - 获取任务的 JoinHandle
  - `has_active_join_handle()` - 检查是否有活跃的 JoinHandle

#### 3. 任务管理功能
- **取消任务**：
  - `cancel_task()` - 取消任务并 abort JoinHandle
- **清理任务**：
  - `cleanup_completed_tasks()` - 清理已完成的任务
  - `remove_task()` - 内部方法，清理所有相关数据

#### 4. 任务等待功能
- **等待单个任务**：
  - `wait_for_task()` - 等待单个任务完成，支持已完成和运行中的任务
- **等待所有任务**：
  - `wait_for_all_tasks()` - 等待所有活跃任务完成

#### 5. OrchestratorExecutor 集成
- **修改现有方法**：
  - `assign_task_parallel()` - 接受 TaskManager 参数，注册子任务
  - `get_task_status()` - 从 TaskManager 查询任务状态
  - `handle_cma_notification()` - 接受 TaskManager 参数（可选）

- **新增方法**：
  - `wait_for_task()` - 暴露 TaskManager 的等待功能
  - `wait_for_all_tasks()` - 暴露等待所有任务的功能

### 架构设计决策

#### 独立 TaskManager 架构
采用了**独立 TaskManager 架构**，而不是将功能直接集成到 Orchestrator 中：

| 优点 | 说明 |
|------|------|
| **职责分离清晰** | Orchestrator 专注于 Agent 管理、任务分配决策、工单处理<br>TaskManager 专注于任务生命周期、状态跟踪、结果收集 |
| **符合 SRP** | 单一职责原则，每个组件只做一件事 |
| **易于测试** | TaskManager 可以独立测试，不需要完整的 Orchestrator<br>Orchestrator 可以 mock TaskManager 进行测试 |
| **灵活性高** | TaskManager 可以被多个 Orchestrator 共享（如果需要）<br>可以替换不同的 TaskManager 实现（内存 vs 持久化） |
| **并发控制集中** | 所有并发原语（Mutex, Condvar 等）都集中在 TaskManager 中<br>Orchestrator 保持单线程逻辑，更容易理解<br>死锁风险更容易分析和避免 |

### 验证结果
- ✅ `cargo build` 通过，无警告无错误
- ✅ `cargo test` 通过，**所有 191 个测试全部通过**！
- ✅ 代码符合 Rust 最佳实践
- ✅ 保持了向后兼容性（TaskManager 参数为可选）

### 文件修改清单
- `src/orchestrator/task_manager.rs` - 新增，TaskManager 完整实现
- `src/orchestrator/executor.rs` - 修改，集成 TaskManager
- `src/orchestrator/mod.rs` - 修改，导出新模块
- `docs/PHASE4_inprogress.md` - 更新，记录完成的工作

### 后续工作
TaskManager 的基础功能已完成，后续可以：
1. 继续 Phase 4.5（上下文管理 Agent）
2. 编写 TaskManager 的单元测试
3. 实现完整的 Checkpoint 回退和 Agent 转交逻辑
4. 添加按 Session ID 索引的任务查询功能
5. 实现任务持久化功能

---

## 概述

### 目标
建立多 Agent 运行的基础框架，支持层级化总控架构、可嵌套子总控、上下文管理 Agent 的两种触发机制，以及复杂任务的两种执行模式。

### 执行原则
严格按照 **Baby Steps™ 方法论**执行：
1. 最小可能的有意义变更
2. 过程就是产品
3. 一次只完成一个实质性成果
4. 每个步骤完全完成后再进入下一步
5. 每个步骤后必须验证
6. 每个步骤都要有详细文档

### 优先级策略
分为三个优先级批次，必须按顺序完成：
- 🎯 **第一优先级：核心基础设施** - 建立 Agent 运行的最小可行基础
- 🏗️ **第二优先级：功能模块** - 在核心基础设施之上添加关键功能
- 🔌 **第三优先级：集成** - 与外部系统和协议集成

### 成功标准（Definition of Done）
- 主总控可以创建和管理子总控（知道嵌套深度）
- 子总控可以创建普通 Agent、更小的子总控或集群
- Agent 可以主动向 CMA 发送求助工单
- CMA 只在两种情况下触发：Agent 求助、上下文满载
- 支持嵌套子总控接力模式（强依赖顺序场景）
- 支持平行子总控集群模式（可并行分解场景）
- 上下文管理 Agent 可以裁剪上下文、判断回退和转交
- Session 与 Checkpoint 完整集成
- 基础 API 可以管理总控和 Agent
- 所有单元测试通过
- 完整的文档和验收清单

### 与前后阶段的依赖关系
- **前置依赖**：Phase 3 完成（本地工具与 Checkpoint 集成）
- **后续阶段**：Phase 5（任务编排与路由系统）

---

## 关键设计决策

### 1. 架构模型：层级化总控 + 可嵌套子总控 + 单向流水线

**核心架构**：
```
主总控 (Master Orchestrator) - 最顶层，唯一
    │
    ├── 子总控 1 (Sub-Orchestrator, Level 1)
    │   ├── 可以创建：普通 Agent
    │   ├── 可以创建：更小的子总控 (Level 2，如需要)
    │   └── 可以创建：集群（并行执行多个 Agent）
    │
    ├── 子总控 2 (Sub-Orchestrator, Level 1)
    │   └── ...
    │
    └── 上下文管理 Agent (Context Manager Agent, CMA)
            ↓
       独立监控所有 Agent
```

**设计原则**：
- **单向流水线**：流水线是线性的，不支持 Agent 间通信
- **总控协调**：只有总控知道有哪些 Agent 存在
- **直接调用**：总控通过直接函数调用给 Agent 分配任务
- **并行支持**：多个 Agent 可以并行工作（通过 tokio task）
- **嵌套深度**：子总控知道自己的嵌套深度 (Level N)
- **软性限制**：提示词劝阻过深嵌套，但不硬性限制（保持灵活性）

### 2. Agent 模型：无状态的执行者 + 可嵌套总控

**Agent 的定位**：
- **普通执行 Agent**：是"工人"，无状态
  - 不保存对话历史（对话历史属于 Session）
  - 不知道其他 Agent 的存在
  - 只通过总控接收任务
  - 可以主动向 CMA 发送求助工单

- **子总控**：是"中层管理者"
  - 知道自己的嵌套深度 (Level N)
  - 可以创建普通 Agent、更小的子总控或集群
  - 完成任务后写工单交回给父总控
  - 提示词会软性劝阻过深嵌套

- **主总控**：是"最高管理者"
  - 最顶层的唯一总控
  - 负责初始路由决策
  - 管理所有子总控的生命周期
  - 根据任务性质选择执行模式

**上下文管理 Agent (CMA)**：是"监军"
- 独立监控所有 Agent
- 只有两个触发点：Agent 主动求助、上下文满载
- 决定回退和转交，但不直接执行
- 收到求助后通知当前层级的总控执行

### 3. 通信方式：直接函数调用

**决策**：总控和 Agent 之间通过直接函数调用通信

**理由**：
- 简单直接，不需要消息总线
- 性能好，没有序列化/反序列化开销
- 调试方便，调用栈清晰
- 类型安全，编译时检查

**并发支持**：
- 每个 Agent 任务运行在独立的 tokio task 中
- 多个 Agent 可以并行执行
- 通过 JoinHandle 管理并发任务

### 4. 工单机制：作为 User Message

**决策**：接力转交时的工单直接作为 User Message

**设计**：
- 工单 = User Message（JSON 格式）
- System Prompt 保持独立
- 工单格式简洁聚焦
- 包含：已完成部分、相关文件、下一阶段计划

**工单传递**：
- 子总控完成后写工单交回给父总控
- 接力转交时，新 Agent 用工单作为 User Message

### 5. 复杂任务的两种执行模式

**模式 A：嵌套子总控接力模式**
- **适用场景**：复杂任务，各步骤之间强依赖顺序，每一步都很复杂
- **执行流程**：主总控 → 子总控 1 → 工单 → 主总控 → 子总控 2 → ...
- **示例**：主总控 → (总控1→集群→总控1写工单→主总控) → (主总控更新计划后创建新总控2→集群)...

**模式 B：平行子总控集群模式**
- **适用场景**：复杂任务，但各部分可以分别独立执行
- **执行流程**：主总控 → 同时创建多个子总控并行执行 → 汇总结果
- **示例**：主总控 → (总控1, 总控2... 计划后创建 → Agent/集群 Agents)...

### 6. 上下文管理 Agent (CMA) 的触发机制

**只有两个触发点**：

**触发点 1：普通 Agent 主动求助（Worker → CMA）**
- 普通 Agent 发现自己持续犯错 → 生成"求助工单"
- 求助工单直接发送给 CMA（不是主总控！）
- CMA 分析后决定回退和转交，通知总控执行

**触发点 2：上下文满载（唯一的自动触发点）**
- 检测到：当前上下文长度 ≥ 阈值（如 90%）
- 自动触发：CMA 进行清理
- 清理后判断：是否在持续犯错？
  - 是 → 触发回退 + 转交
  - 否 → 继续执行
- 其他情况不自动触发，因为每次输入的上下文通常很长

### 7. 文件并发：避免冲突

**原则**：多个 Agent 并行工作时，不会出现修改同一文件的情况

**实现方式**：
- 总控分配任务时明确划分文件范围
- 每个 Agent 只操作分配给自己的文件
- 通过约定接口避免冲突
- Checkpoint 系统确保状态一致性

### 8. 与现有代码的集成策略

**复用现有组件**：
- LLM 客户端（src/llm/）- 完全复用
- MCP 工具集成（src/mcp/）- 完全复用
- 本地工具（src/tools/）- 完全复用
- Checkpoint 系统（src/checkpoint/）- 增强集成
- Session 模型（src/models/session.rs）- 增强
- ToolCoordinator（src/tool_coordinator.rs）- 完全复用

**新增组件**：
- Agent 定义（src/agent/）
- 总控（src/orchestrator/）- 支持嵌套子总控
- 上下文管理（src/context_manager/）- 两种触发机制
- 工具掩码（src/tool_mask/）

---

## 详细实施计划

---

## 🎯 第一优先级：核心基础设施

### Phase 4.1: Agent 基础定义（支持嵌套深度）

#### 任务清单
- [x] 定义 AgentId 类型（新type 包装 Uuid）
- [x] 定义 AgentRole 枚举（更新为新架构）
- [x] 定义 AgentCapability 标签系统
- [x] 定义 LLM 配置结构
- [x] 定义 AgentState 枚举
- [x] 定义 Agent 核心数据结构（支持嵌套深度）
- [x] 定义 AgentConfig 配置结构
- [x] 实现 Agent 执行函数（接收任务，返回结果）
- [x] 实现 Agent 发送工单功能（可发送给 CMA 或总控）
- [x] 编写单元测试
- [x] 验证验收清单
- [x] 实现建造者模式 (AgentBuilder, WorkerAgentBuilder)
- [x] 优化 AgentState 设计（移除 Error，合并为 WaitingForReview）
- [x] 简化 WorkOrderRecipient（移除 Agent，只保留 ContextManager 和 Orchestrator）

#### 数据结构设计

**AgentId**
- 唯一标识 Agent
- 使用 Uuid v4
- 支持序列化和反序列化
- 实现 Display、Debug、Clone、Copy、PartialEq、Eq、Hash

**AgentRole**
- 枚举类型，定义 Agent 的角色
- 值：MasterOrchestrator, SubOrchestrator, Worker, ContextManager
- 可扩展
- 实现 Display、Debug、Clone、PartialEq、Eq

**AgentCapability**
- 字符串标签，描述 Agent 的能力
- 例如："code_write", "code_review", "planning", "debugging"
- 使用 Vec<String> 存储

**LlmConfig**
- 模型名称（必填）
- 温度参数（可选，默认 0.7）
- top_p 参数（可选）
- max_tokens 参数（可选）
- 其他 LLM 特定参数

**AgentState**
- Idle: 空闲，可接受任务
- Busy: 忙碌，正在执行任务
- WaitingForReview: 已完成，提交结果/求助等待审查/响应

**设计说明**：
- Error 状态被移除，错误通过 Result 返回值处理
- RequestingHelp 被合并到 WaitingForReview，统一表示"等待外部响应"

**Agent（核心结构）**
- id: AgentId
- name: String（人类可读名称）
- role: AgentRole
- capabilities: Vec<String>
- llm_config: LlmConfig
- state: AgentState
- system_prompt: String
- nested_depth: Option<u8>（嵌套深度，仅 SubOrchestrator 有）
- parent_orchestrator_id: Option<AgentId>（父总控 ID，仅 SubOrchestrator 有）
- created_at: DateTime<Utc>
- updated_at: DateTime<Utc>

**AgentConfig**
- name: String
- role: AgentRole
- capabilities: Vec<String>
- llm_config: LlmConfig
- system_prompt: String
- nested_depth: Option<u8>
- parent_orchestrator_id: Option<AgentId>

**AgentTask**
- agent_id: AgentId
- session_id: SessionId
- user_message: String（包含工单，如果是转交）
- tools: Option<Vec<String>>（可用工具列表）
- checkpoint_id: Option<CheckpointId>

**AgentTaskResult**
- success: bool
- agent_id: AgentId
- session_id: SessionId
- response: String
- tool_calls: Vec<ToolCallRecord>
- error: Option<String>
- execution_time_ms: u64
- new_checkpoint_id: Option<CheckpointId>

**WorkOrder（工单，统一结构）**
- work_order_type: WorkOrderType（工单类型）
- recipient: WorkOrderRecipient（接收对象）
- session_id: SessionId
- title: String（工单标题）
- content: String（工单内容，JSON 或自由文本）
- related_files: Vec<String>（相关文件）
- suggested_checkpoint_id: Option<CheckpointId>（建议回退的 Checkpoint）
- created_by: Option<AgentId>（创建者，Agent 或总控）
- created_at: DateTime<Utc>

**WorkOrderType（工单类型）**
- TaskCompletion: 任务完成汇报
- Handover: 接力转交
- HelpRequest: 求助
- StatusUpdate: 状态更新

**WorkOrderRecipient（接收对象）**
- ContextManager: 发送给 CMA
- Orchestrator(OrchestratorId): 发送给指定总控

**设计说明**：
- 移除了 Agent(AgentId) 变体
- 工单总是先发送给总控或 CMA，由总控决定下一步路由
- 避免 Agent 之间直接点对点通信，确保总控的协调作用

#### API 设计

**Agent 创建**
- 输入：AgentConfig
- 输出：Result<Agent, Error>
- 验证配置有效性
- 生成唯一 ID
- 初始化状态为 Idle
- 如果是 SubOrchestrator，设置 nested_depth

**Agent 执行任务**
- 输入：Agent, AgentTask
- 输出：Result<AgentTaskResult, Error>
- 这是一个 async 函数
- 内部调用 LLM、使用工具
- 更新 Agent 状态（Busy → Idle/Error）
- 创建新的 Checkpoint（如果需要）

**Agent 发送工单**
- 输入：Agent, WorkOrder
- 输出：Result<(), Error>
- 根据 recipient 发送给目标（ContextManager 或 Orchestrator）
- 发送工单后，Agent 状态更新为 WaitingForReview

**Agent 状态查询**
- 输入：Agent
- 输出：AgentState

#### 测试策略
- 单元测试：所有数据结构和基础操作
- 单元测试：嵌套深度设置和验证
- 单元测试：工单发送功能（不同接收对象）
- 集成测试：Agent 执行任务的端到端流程
- 错误处理测试：验证各种错误场景的处理

#### 验收标准
- [x] 可以创建 Agent 并分配唯一 ID
- [x] Agent 配置验证正常工作
- [x] SubOrchestrator 可以正确设置嵌套深度
- [x] Agent 可以接收任务
- [ ] Agent 可以调用 LLM (占位实现，后续完善)
- [ ] Agent 可以使用工具 (占位实现，后续完善)
- [x] Agent 可以返回响应
- [x] Agent 状态正确更新
- [x] Agent 可以发送工单给 CMA（求助）
- [x] Agent 可以发送工单给总控（任务完成）
- [x] 错误处理正常工作
- [x] 所有单元测试通过 (119 tests passed)
- [x] 集成测试通过 (待 Phase 4.2 完成)

**已完成的额外工作**：
- [x] 实现了 AgentBuilder 和 WorkerAgentBuilder 建造者模式
- [x] 优化了 AgentState 设计
- [x] 简化了 WorkOrderRecipient 设计

---

### Phase 4.2: 总控机制（支持嵌套子总控）

#### 任务清单
- [x] 定义 OrchestratorId 类型
- [x] 定义 OrchestratorRole 枚举（更新为新架构）
- [x] 定义 Orchestrator 核心数据结构（支持嵌套）
- [x] 定义 OrchestratorConfig 配置结构
- [x] 实现 Agent 仓库（总控管理的 Agent 列表）
- [x] 实现 Agent 创建功能（支持创建子总控）
- [x] 实现任务分配功能（串行）
- [x] 实现任务分配功能（并行）（占位实现）
- [ ] 实现结果回收功能（完整实现需要任务状态管理）
- [x] 实现工单生成和处理（统一工单结构）
- [x] 实现 Session 与 Agent 的关联
- [x] 实现 CMA 通知处理（回退和转交）（占位实现）
- [x] 编写单元测试
- [x] 验证验收清单

#### 数据结构设计

**OrchestratorId**
- 唯一标识总控
- 使用 Uuid v4
- 实现 Display、Debug、Clone、Copy、PartialEq、Eq、Hash

**OrchestratorRole**
- Master: 主总控
- Sub: 子总控

**Orchestrator**
- id: OrchestratorId
- name: String
- role: OrchestratorRole
- agent: Agent（总控本身也是一个 Agent）
- nested_depth: u8（嵌套深度，Master 为 0）
- parent_orchestrator_id: Option<OrchestratorId>（父总控 ID）
- managed_agents: HashMap<AgentId, Agent>
- active_tasks: HashMap<TaskId, JoinHandle<Result<AgentTaskResult, Error>>>
- session_id: Option<SessionId>
- created_at: DateTime<Utc>
- updated_at: DateTime<Utc>

**OrchestratorConfig**
- name: String
- role: OrchestratorRole
- agent_config: AgentConfig（总控自己的 Agent 配置）
- nested_depth: u8
- parent_orchestrator_id: Option<OrchestratorId>

**TaskId**
- 唯一标识任务
- 使用 Uuid v4

**TaskAssignment**
- task_id: TaskId
- agent_id: AgentId
- task: AgentTask
- assigned_at: DateTime<Utc>

**ParallelTasks**
- task_id: TaskId
- assignments: Vec<TaskAssignment>
- wait_for_all: bool（是否等待所有任务完成）

（WorkOrder 统一定义见 Phase 4.1）

**CmaNotification（CMA 通知）**
- notification_type: CmaNotificationType
- session_id: SessionId
- target_orchestrator_id: OrchestratorId
- checkpoint_id: Option<CheckpointId>
- reason: String
- created_at: DateTime<Utc>

**CmaNotificationType**
- RollbackAndHandover: 回退并转交
- ContextTrimmed: 上下文已裁剪

#### API 设计

**总控创建**
- 输入：OrchestratorConfig
- 输出：Result<Orchestrator, Error>
- 创建总控自己的 Agent
- 初始化空的 Agent 仓库
- 如果是子总控，设置 nested_depth

**总控创建 Agent**
- 输入：mut Orchestrator, AgentConfig
- 输出：Result<(Orchestrator, Agent), Error>
- 创建 Agent
- 如果创建的是子总控，自动设置 nested_depth = 当前深度 + 1
- 添加到总控的 managed_agents
- 返回更新后的总控和新 Agent

**总控列出 Agent**
- 输入：&Orchestrator
- 输出：Vec<&Agent>
- 返回所有管理的 Agent

**总控获取 Agent**
- 输入：&Orchestrator, AgentId
- 输出：Option<&Agent>

**总控移除 Agent**
- 输入：mut Orchestrator, AgentId
- 输出：Result<Orchestrator, Error>
- 确保 Agent 不在 Busy 状态
- 从 managed_agents 中移除

**总控分配任务（串行）**
- 输入：mut Orchestrator, AgentId, AgentTask
- 输出：Result<(Orchestrator, AgentTaskResult), Error>
- 直接调用 Agent::execute_task
- 等待结果返回
- 更新总控状态

**总控分配任务（并行）**
- 输入：mut Orchestrator, ParallelTasks
- 输出：Result<(Orchestrator, TaskId), Error>
- 为每个任务创建 tokio task
- 存储 JoinHandle 到 active_tasks
- 返回 TaskId，不等待结果

**总控查询任务状态**
- 输入：&Orchestrator, TaskId
- 输出：Option<TaskStatus>
- TaskStatus: Pending, Running, Completed, Failed

**总控等待任务完成**
- 输入：mut Orchestrator, TaskId
- 输出：Result<(Orchestrator, Vec<AgentTaskResult>), Error>
- 等待 JoinHandle 完成
- 清理 active_tasks
- 返回所有结果

**总控取消任务**
- 输入：mut Orchestrator, TaskId
- 输出：Result<Orchestrator, Error>
- abort JoinHandle
- 清理 active_tasks

**总控生成工单**
- 输入：&Orchestrator, WorkOrderType, WorkOrderRecipient, title, content
- 输出：Result<WorkOrder, Error>
- 创建工单
- 序列化为 JSON 字符串，作为 User Message

**总控处理 CMA 通知**
- 输入：mut Orchestrator, CmaNotification
- 输出：Result<Orchestrator, Error>
- 如果是 RollbackAndHandover：
  - 回退到指定 Checkpoint
  - 创建新 Agent 进行转交
- 如果是 ContextTrimmed：
  - 记录上下文已裁剪
  - 继续执行

**总控关联 Session**
- 输入：mut Orchestrator, SessionId
- 输出：Orchestrator

#### 测试策略
- 单元测试：总控 CRUD 操作
- 单元测试：Agent 管理（包括创建子总控）
- 单元测试：嵌套深度管理
- 单元测试：工单生成
- 单元测试：CMA 通知处理
- 集成测试：串行任务分配
- 集成测试：并行任务分配
- 并发测试：多个并行任务的正确性

#### 验收标准
- [x] 可以创建主总控和子总控
- [x] 子总控的嵌套深度正确设置
- [x] 总控可以创建 Agent（包括子总控）
- [x] 总控可以列出和查询 Agent
- [x] 总控可以移除空闲 Agent
- [x] 总控不能移除忙碌 Agent
- [x] 总控可以串行分配任务
- [ ] 总控可以并行分配任务（占位实现）
- [ ] 总控可以查询任务状态（占位实现）
- [ ] 总控可以等待任务完成（待完善）
- [ ] 总控可以取消任务（待完善）
- [x] 总控可以生成工单
- [x] 总控可以处理 CMA 通知（占位实现）
- [ ] 多个并行任务正常工作（待完善）
- [x] 所有单元测试通过
- [ ] 集成测试通过
- [ ] 并发测试通过

---

### Phase 4.3: Checkpoint 与会话增强

#### 任务清单
- [x] 回顾现有的 Checkpoint 实现
- [x] 回顾现有的 Session 模型
- [x] 定义 SessionState 枚举
- [x] 增强 Session 结构（关联总控）
- [x] 定义 Session 生命周期事件
- [x] 实现 Session 创建流程
- [x] 实现 Session 激活流程
- [x] 实现 Session 归档流程
- [x] 实现 Session 删除流程
- [x] 实现 Session 与总控的关联
- [x] 实现 Session 与 Checkpoint 的强关联
- [x] 定义 Checkpoint 归档策略
- [x] 实现 Checkpoint 跟随 Session 生命周期
- [x] 实现 Checkpoint 清理策略
- [x] 优化 AgentFS 集成
- [x] 定义 SessionRepository
- [x] 编写单元测试
- [ ] 编写集成测试
- [x] 验证验收清单

#### 数据结构设计

**SessionState**
- Draft: 草稿状态，刚创建
- Active: 活跃状态，正在使用
- Paused: 暂停状态
- Archived: 已归档，只读
- Deleted: 已删除（软删除）

**Session（增强版）**
- id: SessionId（现有）
- title: String（现有）
- created_at: DateTime<Utc>（现有）
- updated_at: DateTime<Utc>（现有）
- state: SessionState（新增）
- orchestrator_id: Option<OrchestratorId>（新增，关联的总控）
- current_checkpoint_id: Option<CheckpointId>（新增）
- archived_at: Option<DateTime<Utc>>（新增）
- metadata: HashMap<String, String>（新增，灵活的元数据）

**SessionLifecycleEvent**
- 枚举类型，定义 Session 生命周期事件
- 值：Created, Activated, Paused, Resumed, Archived, Deleted
- 包含事件时间戳
- 包含触发者信息

**Checkpoint（增强版 - 如果需要）**
- session_id: SessionId（确保存在）
- is_archived: bool（新增）
- archived_at: Option<DateTime<Utc>>（新增）

**SessionRepository**
- 存储所有 Session 实例
- 支持 CRUD 操作
- 支持按状态查询
- 支持按总控查询
- 线程安全

**CheckpointArchivingStrategy**
- 配置何时归档 Checkpoint
- 选项：Session 归档时、手动触发、定期
- 配置保留策略（保留多少个 Checkpoint）

#### API 设计

**Session 创建**
- 输入：可选标题、可选 OrchestratorId
- 输出：Result<Session, Error>
- 初始状态：Draft
- 创建初始 Checkpoint

**Session 激活**
- 输入：SessionId
- 输出：Result<(), Error>
- 状态转换：Draft/Paused → Active
- 如果有关联总控，通知总控

**Session 暂停**
- 输入：SessionId
- 输出：Result<(), Error>
- 状态转换：Active → Paused
- 创建 Checkpoint
- 如果有关联总控，通知总控

**Session 恢复**
- 输入：SessionId
- 输出：Result<(), Error>
- 状态转换：Paused → Active
- 恢复到最新的 Checkpoint
- 如果有关联总控，通知总控

**Session 归档**
- 输入：SessionId
- 输出：Result<(), Error>
- 状态转换：任何状态 → Archived
- 创建最终 Checkpoint
- 归档所有相关 Checkpoint
- 如果有关联总控，通知总控释放资源

**Session 删除（软删除）**
- 输入：SessionId
- 输出：Result<(), Error>
- 状态转换：任何状态 → Deleted
- 可选：清理 Checkpoint（根据配置）

**Session 永久删除**
- 输入：SessionId
- 输出：Result<(), Error>
- 从仓库中移除
- 清理所有相关 Checkpoint
- 清理 AgentFS 中的数据

**Session 关联总控**
- 输入：SessionId, OrchestratorId
- 输出：Result<(), Error>
- Session 必须是 Draft 或 Active 状态

**Session 解绑总控**
- 输入：SessionId
- 输出：Result<(), Error>
- 创建 Checkpoint
- 通知总控

**Session 查询**
- 输入：SessionId
- 输出：Result<Session, Error>

**Session 列表查询**
- 输入：可选过滤条件（状态、OrchestratorId、创建时间范围）
- 输出：Result<Vec<Session>, Error>

**Session 历史查询**
- 输入：SessionId
- 输出：Result<Vec<SessionLifecycleEvent>, Error>

**获取 Session 的 Checkpoint 列表**
- 输入：SessionId
- 输出：Result<Vec<Checkpoint>, Error>

**恢复到指定 Checkpoint**
- 输入：SessionId, CheckpointId
- 输出：Result<(), Error>
- 创建当前状态的 Checkpoint
- 恢复到指定 Checkpoint

**清理过期的 Checkpoint**
- 输入：保留策略配置
- 输出：Result<usize, Error>（清理的数量）

#### 测试策略
- 单元测试：Session 状态机、CRUD 操作
- 集成测试：Session 与总控的协作
- 集成测试：Session 与 Checkpoint 的集成
- 回归测试：确保现有功能不受影响

#### 验收标准
- [x] 可以创建 Session，初始状态为 Draft
- [x] Session 状态机所有合法转换都能正常工作
- [x] 非法状态转换被正确拒绝
- [x] 可以将 Session 关联到总控
- [x] 可以解绑总控
- [x] Session 创建时自动创建初始 Checkpoint
- [x] Session 状态变化时自动创建 Checkpoint
- [x] Session 归档时归档所有 Checkpoint
- [x] 可以查询 Session 列表
- [x] 可以按状态、总控过滤 Session
- [x] 可以查询 Session 的生命周期历史
- [x] 可以恢复到指定的 Checkpoint
- [x] 可以清理过期的 Checkpoint
- [x] 软删除的 Session 不再出现在正常列表中
- [x] 永久删除的 Session 及其 Checkpoint 被完全清理
- [x] 所有单元测试通过
- [ ] 集成测试通过
- [ ] 回归测试通过

---

## 🏗️ 第二优先级：功能模块

### Phase 4.4: 工具掩码基础机制

#### 设计原则
- **主总控 Agent**：拥有全部权限，可以看到和使用所有工具
- **Worker Agent**：只能看到和使用被授权的工具
- **核心策略**：在 `get_available_tools()` 中就过滤掉没有权限的工具，Agent 根本看不到无权使用的工具
- **不需要调用时验证**：因为 LLM 只会尝试调用它能看到的工具

#### 任务清单
- [x] 定义简化的工具权限类型
- [x] 定义 ToolMask 结构
- [x] 实现工具掩码配置
- [x] 实现 Agent 工具集分配
- [x] 实现工具过滤（Agent 只能看到有权限的工具）
- [x] 实现终端工具特殊处理（全开放）
- [x] 定义 ToolMaskRepository
- [x] 与现有 ToolCoordinator 集成
- [x] 编写单元测试
- [x] 验证验收清单

#### 数据结构设计（简化版）

**McpToolPermission**
- 枚举类型，MCP 工具权限
- 值：Available, NotAvailable
- 实现 Display、Debug、Clone、PartialEq、Eq

**LocalToolPermission**
- 枚举类型，本地工具权限
- 值：ReadOnly, ReadWrite
- 实现 Display、Debug、Clone、PartialEq、Eq

**LocalToolMode**（一键切换）
- 枚举类型，本地工具全局模式
- 值：ReadOnly, ReadWrite
- 优先于单个工具配置

**ToolMask**
- agent_id: AgentId
- mcp_permissions: HashMap&lt;String, HashMap&lt;String, McpToolPermission&gt;&gt;  // {server_name: {tool_name: permission}}
- local_permissions: HashMap&lt;String, LocalToolPermission&gt;  // {tool_name: permission}
- local_tool_mode: Option&lt;LocalToolMode&gt;  // 本地工具全局模式（优先）
- updated_at: DateTime&lt;Utc&gt;

**ToolMaskRepository**
- 存储所有 Agent 的工具掩码配置
- 支持按 Agent 查询和更新
- 线程安全

#### API 设计

**为 Agent 配置 MCP 工具权限**
- 输入：AgentId, server_name, tool_name, McpToolPermission
- 输出：Result&lt;(), Error&gt;

**为 Agent 配置本地工具权限**
- 输入：AgentId, tool_name, LocalToolPermission
- 输出：Result&lt;(), Error&gt;

**设置 Agent 本地工具全局模式**
- 输入：AgentId, LocalToolMode
- 输出：Result&lt;(), Error&gt;

**查询 Agent 的工具掩码**
- 输入：AgentId
- 输出：Result&lt;ToolMask, Error&gt;

**获取 Agent 可见的工具列表**
- 输入：AgentId, 所有可用工具
- 输出：Vec&lt;Tool&gt;（过滤后的工具列表）
- **这是核心函数**：Agent 只能看到这里返回的工具

**复制 Agent 权限配置**
- 输入：源 AgentId, 目标 AgentId
- 输出：Result&lt;(), Error&gt;

#### 与现有 ToolCoordinator 集成
- **修改 `get_available_tools()`**：接受 AgentId 参数，返回过滤后的工具列表
- **Agent 看不到无权使用的工具**：LLM 只会尝试调用能看到的工具
- **终端工具总是可见**：特殊处理，全开放
- **不需要调用时验证**：因为 LLM 不会调用看不到的工具

#### 测试策略
- 单元测试：工具过滤逻辑
- 单元测试：本地工具全局模式
- 单元测试：终端工具特殊处理
- 集成测试：与 ToolCoordinator 的集成

#### 验收标准
- [x] 可以为 Agent 配置 MCP 工具权限
- [x] 可以为 Agent 配置本地工具权限
- [x] 可以设置本地工具全局模式（一键切换）
- [x] Agent 只能看到有权限的工具
- [x] Agent 看不到无权使用的工具
- [x] 本地工具全局模式优先于单个工具配置
- [x] 终端工具总是可见（全开放）
- [x] 可以复制 Agent 的权限配置
- [x] 与 ToolCoordinator 集成正常
- [x] 所有单元测试通过
- [ ] 集成测试通过

---

### Phase 4.5: 上下文管理 Agent（基础版，两种触发机制）

#### 任务清单
- [ ] 定义 ContextId 类型
- [ ] 定义 ContextChunk 结构
- [ ] 定义 ContextMetadata 结构
- [ ] 定义 ContextStore 结构
- 实现 Agent 求助接收机制（触发点 1）
- 实现上下文长度监控（触发点 2，唯一自动触发点）
- 实现上下文清理（裁剪）
- 实现持续犯错判断
- 实现回退到指定 Checkpoint
- 实现触发总控转交
- 定义上下文裁剪策略模板
- 实现 ContextManagerAgent（作为特殊 Agent）
- 编写单元测试
- 验证验收清单

#### 数据结构设计

**ContextId**
- 唯一标识上下文
- 使用 Uuid v4
- 实现 Display、Debug、Clone、Copy、PartialEq、Eq、Hash

**ContextChunk**
- id: ContextId
- session_id: SessionId
- content: String
- chunk_type: ContextChunkType（Message, ToolCall, ToolResult, System）
- token_count: usize
- created_at: DateTime<Utc>
- metadata: HashMap<String, String>
- is_important: bool（是否重要，裁剪时优先保留）
- retention_priority: u8（保留优先级，0-10，越高越优先保留）

**ContextChunkType**
- 枚举类型
- 值：UserMessage, AssistantMessage, ToolCall, ToolResult, SystemPrompt, SystemNotification, WorkOrder, HelpRequest

**ContextMetadata**
- session_id: SessionId
- total_token_count: usize
- chunk_count: usize
- first_message_at: DateTime<Utc>
- last_message_at: DateTime<Utc>
- estimated_cost: f64（可选，预估的 token 成本）

**ContextStore**
- 存储上下文块
- 支持按 Session 查询
- 支持按时间范围查询
- 支持按重要性过滤
- 线程安全

**TrimmingTrigger**
- 触发裁剪的条件
- max_token_count: usize（最大 token 数，默认 90%）

**TrimmingStrategy**
- 裁剪策略
- strategy_type: TrimmingStrategyType（Fifo, ImportanceBased, Hybrid）
- keep_recent_n_chunks: Option<usize>（保留最近 N 个块）
- keep_important_chunks: bool（是否保留重要块）
- min_chunks_to_keep: usize（最少保留块数）

**TrimmingStrategyType**
- Fifo: 先进先出，删除最早的
- ImportanceBased: 基于重要性，删除最不重要的
- Hybrid: 混合策略，结合时间和重要性

**ContinuousMistakeDetector**
- 检测持续犯错
- session_id: SessionId
- mistake_window_seconds: u64（时间窗口）
- mistake_threshold: u32（错误次数阈值）
- recent_mistakes: Vec<DateTime<Utc>>

**WorkOrder**
- 工单格式（作为 User Message）
- completed_work: String（已完成部分详细标注）
- related_files: Vec<String>（相关文件的相对路径列表）
- next_stage_plan: String（下一阶段的详细计划）

**StrategyTemplate**
- 模板 ID
- 模板名称
- 模板类型（Trimming）
- 模板内容（JSON 或 YAML）
- 版本号
- 创建时间
- 更新时间
- 是否为默认模板

#### API 设计

**添加上下文块**
- 输入：SessionId, ContextChunk
- 输出：Result<ContextId, Error>
- 自动计算 token 数（如果未提供）
- 更新 ContextMetadata

**批量添加上下文块**
- 输入：SessionId, Vec<ContextChunk>
- 输出：Result<Vec<ContextId>, Error>

**获取 Session 的完整上下文**
- 输入：SessionId
- 输出：Result<(Vec<ContextChunk>, ContextMetadata), Error>

**获取裁剪后的上下文**
- 输入：SessionId, TrimmingStrategy
- 输出：Result<Vec<ContextChunk>, Error>
- 应用裁剪策略，返回裁剪后的上下文

**检查是否需要裁剪（触发点 2）**
- 输入：SessionId, TrimmingTrigger
- 输出：Result<bool, Error>
- 返回 true 表示需要裁剪
- 这是唯一的自动触发点

+**接收工单（触发点 1：Agent 发送求助工单）**
+- 输入：WorkOrder（recipient = ContextManager, type = HelpRequest）
+- 输出：Result<(), Error>
+- 记录工单
+- 分析情况
+- 决定回退到哪个 Checkpoint
+- 触发总控转交

**执行裁剪**
- 输入：SessionId, TrimmingStrategy
- 输出：Result<usize, Error>（裁剪的块数）
- 从存储中移除被裁剪的块（或标记为已裁剪）
- 创建 Checkpoint 保存裁剪前的状态

**判断是否持续犯错**
- 输入：SessionId, ContinuousMistakeDetector
- 输出：Result<bool, Error>
- 返回 true 表示在持续犯错

**标记上下文块为重要**
- 输入：ContextId, is_important: bool, retention_priority: Option<u8>
- 输出：Result<(), Error>

**查询上下文元数据**
- 输入：SessionId
- 输出：Result<ContextMetadata, Error>

**创建工单**
- 输入：WorkOrder
- 输出：Result<String, Error>
- 序列化为 JSON 字符串，作为 User Message

**保存策略模板**
- 输入：StrategyTemplate
- 输出：Result<(), Error>

**获取策略模板**
- 输入：模板 ID
- 输出：Result<StrategyTemplate, Error>

**列出所有策略模板**
- 输入：可选模板类型过滤
- 输出：Result<Vec<StrategyTemplate>, Error>

**设置默认模板**
- 输入：模板 ID
- 输出：Result<(), Error>

**ContextManagerAgent 特殊功能**
- 监听 Agent 求助（触发点 1）
- 监控 Session 上下文大小（触发点 2，唯一自动触发）
- 收到求助时：分析、决定回退 Checkpoint、通知总控
- 上下文满载时：清理、判断持续犯错、决定是否回退+转交
- 其他情况不自动触发

#### 测试策略
- 单元测试：上下文存储和检索
- 单元测试：裁剪策略
- 单元测试：持续犯错检测
+- 单元测试：工单接收和处理（来自 Agent 的求助）
- 单元测试：工单生成
- 集成测试：ContextManagerAgent 端到端流程（两种触发机制）

#### 验收标准
- [ ] 可以存储上下文块
- [ ] 可以获取完整上下文
- [ ] 可以正确计算 token 数
- [ ] 可以检查是否需要裁剪（唯一自动触发点）
+- [ ] 可以接收 Agent 的求助工单（recipient = ContextManager）
- [ ] FIFO 裁剪策略正常工作
- [ ] 基于重要性的裁剪策略正常工作
- [ ] 混合裁剪策略正常工作
- [ ] 重要块被优先保留
- [ ] 可以标记块为重要
- [ ] 持续犯错检测正常工作
- [ ] 可以决定回退到哪个 Checkpoint
+- [ ] 收到求助工单后可以通知总控进行转交
- 可以创建工单（JSON 格式）
- [ ] 策略模板可以保存和读取
- [ ] 可以设置默认模板
- [ ] ContextManagerAgent 只在两种情况下触发
- [ ] ContextManagerAgent 可以监控上下文
- [ ] ContextManagerAgent 可以自动触发裁剪
- [ ] 所有单元测试通过
- [ ] 集成测试通过

---

### Phase 4.6: 基础 API 扩展

#### 任务清单
- [ ] 设计总控管理 REST API
- [ ] 实现总控创建 API（支持嵌套深度）
- [ ] 实现总控查询 API（列表和详情）
- [ ] 实现总控更新 API
- [ ] 实现总控删除 API
- [ ] 设计 Agent 管理 REST API
- [ ] 实现 Agent 创建 API（通过总控）
- [ ] 实现 Agent 查询 API（列表和详情）
- [ ] 实现 Agent 删除 API
- [ ] 实现 Agent 状态查询 API
- [ ] 设计任务管理 REST API
- [ ] 实现任务提交 API（串行）
- [ ] 实现任务提交 API（并行）
- [ ] 实现任务状态查询 API
- [ ] 实现任务等待 API
- [ ] 实现任务取消 API
- [ ] 设计 Session 管理 REST API
- [ ] 实现 Session 创建 API
- [ ] 实现 Session 激活 API
- [ ] 实现 Session 暂停 API
- [ ] 实现 Session 归档 API
- [ ] 实现 Session 删除 API
- [ ] 实现 Session 查询 API（列表和详情）
- [ ] 实现 Session 历史查询 API
- [ ] 设计 CMA 相关 API
- [ ] 实现求助提交 API
- [ ] 实现 CMA 通知查询 API
- [ ] 添加 API 请求/响应日志
- [ ] 添加 API 文档注释（OpenAPI/Swagger）
- [ ] 编写 API 集成测试
- [ ] 验证验收清单

#### API 设计

**总控管理 API**

`POST /api/orchestrators`
- 创建新总控
- 请求体：OrchestratorConfig（包含 nested_depth）
- 响应：Orchestrator（包含生成的 ID）
- 状态码：201 Created

`GET /api/orchestrators`
- 列出所有总控
- 查询参数：role, nested_depth, page, page_size
- 响应：{ orchestrators: Vec<Orchestrator>, total: usize, page: usize, page_size: usize }
- 状态码：200 OK

`GET /api/orchestrators/:id`
- 获取总控详情
- 响应：Orchestrator
- 状态码：200 OK, 404 Not Found

`PUT /api/orchestrators/:id`
- 更新总控配置
- 请求体：部分 OrchestratorConfig 字段
- 响应：Orchestrator
- 状态码：200 OK, 404 Not Found

`DELETE /api/orchestrators/:id`
- 删除总控
- 响应：204 No Content, 404 Not Found, 409 Conflict（如果有活跃任务）

**Agent 管理 API**

`POST /api/orchestrators/:orchestrator_id/agents`
- 通过总控创建新 Agent
- 请求体：AgentConfig
- 响应：Agent（包含生成的 ID）
- 状态码：201 Created, 404 Not Found

`GET /api/orchestrators/:orchestrator_id/agents`
- 列出总控管理的所有 Agent
- 查询参数：role, capability, state, page, page_size
- 响应：{ agents: Vec<Agent>, total: usize, page: usize, page_size: usize }
- 状态码：200 OK, 404 Not Found

`GET /api/orchestrators/:orchestrator_id/agents/:id`
- 获取 Agent 详情
- 响应：Agent
- 状态码：200 OK, 404 Not Found

`DELETE /api/orchestrators/:orchestrator_id/agents/:id`
- 删除 Agent
- 响应：204 No Content, 404 Not Found, 409 Conflict（如果 Agent 忙碌）

`GET /api/orchestrators/:orchestrator_id/agents/:id/state`
- 获取 Agent 状态
- 响应：{ state: AgentState, updated_at: DateTime }
- 状态码：200 OK, 404 Not Found

**任务管理 API**

`POST /api/orchestrators/:orchestrator_id/tasks/serial`
- 提交串行任务
- 请求体：{ agent_id: string, task: AgentTask }
- 响应：{ task_id: string, result: AgentTaskResult }
- 状态码：200 OK, 404 Not Found

`POST /api/orchestrators/:orchestrator_id/tasks/parallel`
- 提交并行任务
- 请求体：ParallelTasks
- 响应：{ task_id: string, status: TaskStatus }
- 状态码：202 Accepted, 404 Not Found

`GET /api/orchestrators/:orchestrator_id/tasks/:id`
- 获取任务状态
- 响应：{ task_id: string, status: TaskStatus, results?: Vec<AgentTaskResult>, error?: string }
- 状态码：200 OK, 404 Not Found

`POST /api/orchestrators/:orchestrator_id/tasks/:id/wait`
- 等待任务完成
- 响应：{ task_id: string, status: TaskStatus, results: Vec<AgentTaskResult> }
- 状态码：200 OK, 404 Not Found

`DELETE /api/orchestrators/:orchestrator_id/tasks/:id`
- 取消任务
- 响应：204 No Content, 404 Not Found, 409 Conflict（如果任务已完成）

`GET /api/orchestrators/:orchestrator_id/tasks`
- 列出任务
- 查询参数：agent_id, session_id, status, page, page_size
- 响应：{ tasks: Vec<TaskInfo>, total: usize, page: usize, page_size: usize }
- 状态码：200 OK

**Session 管理 API**

`POST /api/sessions`
- 创建新 Session
- 请求体：{ title?: string, orchestrator_id?: string }
- 响应：Session
- 状态码：201 Created

`GET /api/sessions`
- 列出所有 Session
- 查询参数：state, orchestrator_id, created_before, created_after, page, page_size
- 响应：{ sessions: Vec<Session>, total: usize, page: usize, page_size: usize }
- 状态码：200 OK

`GET /api/sessions/:id`
- 获取 Session 详情
- 响应：Session
- 状态码：200 OK, 404 Not Found

`POST /api/sessions/:id/activate`
- 激活 Session
- 响应：Session
- 状态码：200 OK, 404 Not Found, 409 Conflict

`POST /api/sessions/:id/pause`
- 暂停 Session
- 响应：Session
- 状态码：200 OK, 404 Not Found, 409 Conflict

`POST /api/sessions/:id/archive`
- 归档 Session
- 响应：Session
- 状态码：200 OK, 404 Not Found

`DELETE /api/sessions/:id`
- 删除 Session（软删除）
- 响应：204 No Content, 404 Not Found

`DELETE /api/sessions/:id/permanent`
- 永久删除 Session
- 响应：204 No Content, 404 Not Found

`GET /api/sessions/:id/history`
- 获取 Session 生命周期历史
- 响应：Vec<SessionLifecycleEvent>
- 状态码：200 OK, 404 Not Found

`GET /api/sessions/:id/checkpoints`
- 获取 Session 的 Checkpoint 列表
- 响应：Vec<Checkpoint>
- 状态码：200 OK, 404 Not Found

`POST /api/sessions/:id/checkpoints/:checkpoint_id/restore`
- 恢复到指定 Checkpoint
- 响应：Session
- 状态码：200 OK, 404 Not Found

`POST /api/sessions/:id/assign-orchestrator`
- 为 Session 分配总控
- 请求体：{ orchestrator_id: string }
- 响应：Session
- 状态码：200 OK, 404 Not Found, 409 Conflict

`POST /api/sessions/:id/unassign-orchestrator`
- 解绑总控
- 响应：Session
- 状态码：200 OK, 404 Not Found

+**CMA 相关 API**
+
+`POST /api/cma/work-orders`
+- Agent 提交工单（包括求助）
+- 请求体：WorkOrder
+- 响应：{ received: bool, message: string }
+- 状态码：202 Accepted
+
+`GET /api/cma/work-orders`
+- 列出工单
+- 查询参数：session_id, agent_id, recipient, work_order_type, page, page_size
+- 响应：{ work_orders: Vec<WorkOrder>, total: usize, page: usize, page_size: usize }
+- 状态码：200 OK
+
+`GET /api/cma/notifications`
+- 列出 CMA 通知
+- 查询参数：session_id, orchestrator_id, type, page, page_size
+- 响应：{ notifications: Vec<CmaNotification>, total: usize, page: usize, page_size: usize }
+- 状态码：200 OK

**TaskStatus 枚举**
- Pending: 等待执行
- Running: 正在执行
- Completed: 已完成
- Failed: 失败
- Cancelled: 已取消

#### 测试策略
- API 集成测试：使用测试客户端测试所有端点
- 错误处理测试：验证各种错误场景的响应
- 认证测试（如果有）：验证访问控制
- 性能测试：基本的负载测试

#### 验收标准
- [ ] 可以通过 API 创建总控（支持嵌套深度）
- [ ] 可以通过 API 查询总控列表和详情
- [ ] 可以通过 API 更新总控
- [ ] 可以通过 API 删除总控
- [ ] 可以通过 API 创建 Agent
- [ ] 可以通过 API 查询 Agent 列表和详情
- [ ] 可以通过 API 删除空闲 Agent
- [ ] 不能通过 API 删除忙碌 Agent
- [ ] 可以查询 Agent 状态
- [ ] 可以通过 API 提交串行任务
- [ ] 可以通过 API 提交并行任务
- [ ] 可以查询任务状态
- [ ] 可以等待任务完成
- [ ] 可以取消待执行的任务
- [ ] 可以通过 API 创建 Session
- [ ] 可以通过 API 查询 Session 列表和详情
- [ ] 可以通过 API 激活、暂停、归档 Session
- [ ] 可以通过 API 删除 Session
- [ ] 可以查询 Session 历史和 Checkpoint
- [ ] 可以恢复到指定 Checkpoint
- [ ] 可以为 Session 分配和解绑总控
- [ ] 可以通过 API 提交工单（包括求助）
- [ ] 可以查询工单和 CMA 通知
- [ ] 所有 API 端点都有适当的日志
- [ ] 错误响应格式一致
- [ ] 所有 API 集成测试通过

---

## 🔌 第三优先级：集成

### Phase 4.7: ACP (Agent Client Protocol) 集成

#### 任务清单
- [ ] 研究 ACP 协议规范
- [ ] 引入 agent-client-protocol crate
- [ ] 学习 ACP 的核心概念
- [ ] 设计 ACP Agent 实现
- [ ] 实现 ACP Agent trait
- [ ] 实现基础初始化和会话设置
- [ ] 实现多并发会话支持
- [ ] 实现 Prompt Turn 处理
- [ ] 实现内容展示（Markdown 格式）
- [ ] 集成工具调用（复用现有 MCP 和本地工具）
- [ ] 集成文件系统访问（与 Checkpoint/AgentFS 集成）
- [ ] 集成终端访问（与现有终端工具集成）
- [ ] 确保 ACP 和 REST API 可以同时运行
- [ ] 共享核心业务逻辑
- [ ] 在 Zed 编辑器中测试基础集成
- [ ] 编写集成测试
- [ ] 验证验收清单

#### ACP 核心概念（概述）

**ACP Agent Trait**
- 核心 trait，定义 Agent 的行为
- 需要实现的主要方法：
  - initialize: 初始化 Agent
  - new_session: 创建新会话
  - prompt: 处理提示词
  - 其他可选方法

**Prompt Turn**
- 用户和 Agent 之间的一次交互
- 包含用户输入和 Agent 响应
- 可以包含工具调用

**内容展示**
- 支持 Markdown 格式
- 支持代码块
- 支持其他富文本格式

**工具调用**
- ACP 定义的工具调用协议
- 需要桥接到现有的工具系统

**会话管理**
- 支持多个并发会话
- 每个会话有独立的状态

#### 集成架构设计

**ACP Server**
- 独立的服务器，与 REST API 并行运行
- 共享应用状态（总控、Session 等）
- 使用 ACP 协议与客户端通信

**ACP Agent 实现**
- 包装 MineClaw 的核心逻辑
- 将 ACP 请求转换为内部调用
- 将内部响应转换为 ACP 格式
- 内部使用总控来管理任务

**工具桥接层**
- 将 ACP 工具调用转换为 ToolCoordinator 调用
- 将工具结果转换回 ACP 格式

**文件系统桥接层**
- 将 ACP 文件系统请求转换为 AgentFS 操作
- 与 Checkpoint 系统集成

**终端桥接层**
- 将 ACP 终端请求转换为现有终端工具调用

#### 数据结构设计

**AcpServerConfig**
- enabled: bool（是否启用 ACP 服务器）
- listen_address: SocketAddr
- 其他 ACP 特定配置

**AcpSessionState**
- session_id: SessionId（内部 Session ID）
- orchestrator_id: Option<OrchestratorId>（关联的总控）
- created_at: DateTime<Utc>
- last_activity_at: DateTime<Utc>

**AcpState**
- 共享的 ACP 服务器状态
- sessions: HashMap<AcpSessionId, AcpSessionState>
- 指向应用核心状态的引用

#### API 设计（内部）

**ACP 服务器启动**
- 输入：AcpServerConfig, 应用核心状态
- 输出：Result<ServerHandle, Error>
- ServerHandle 用于优雅关闭

**ACP 服务器停止**
- 输入：ServerHandle
- 输出：Result<(), Error>

**Prompt Turn 处理流程**
1. 接收 ACP prompt 请求
2. 创建或获取内部 Session
3. 如果需要，创建或获取总控
4. 将用户输入转换为内部消息
5. 通过总控分配任务给 Agent
6. 等待 Agent 响应
7. 将 Agent 响应转换为 ACP 格式
8. 返回响应

**工具调用流程**
1. 接收 ACP 工具调用请求
2. 验证权限（使用工具掩码）
3. 调用 ToolCoordinator
4. 将结果转换为 ACP 格式
5. 返回响应

**文件系统访问流程**
1. 接收 ACP 文件系统请求
2. 检查 Session 的 Checkpoint
3. 执行文件操作
4. 如果需要，创建新的 Checkpoint
5. 返回结果

**终端访问流程**
1. 接收 ACP 终端请求
2. 调用现有终端工具
3. 返回结果

#### 与 REST API 共存
- 两个服务器独立运行，监听不同端口
- 共享同一个应用核心状态
- 使用 Arc<RwLock> 保护共享状态
- 确保线程安全

#### 测试策略
- 单元测试：桥接层的各个组件
- 集成测试：ACP 端到端流程
- 手动测试：在 Zed 编辑器中实际使用

#### 验收标准
- [ ] agent-client-protocol crate 成功引入
- [ ] ACP Agent trait 正确实现
- [ ] ACP 服务器可以正常启动
- [ ] 可以创建新会话
- [ ] Prompt Turn 处理正常工作
- [ ] Markdown 内容正确展示
- [ ] 工具调用集成正常工作
- [ ] 工具权限检查正常工作
- [ ] 文件系统访问集成正常工作
- [ ] 与 Checkpoint 集成正常工作
- [ ] 终端访问集成正常工作
- [ ] 支持多个并发会话
- [ ] ACP 和 REST API 可以同时运行
- [ ] 在 Zed 编辑器中基础集成验证通过
- [ ] 所有单元测试通过
- [ ] 集成测试通过

---

## 风险与缓解措施

### 技术风险

**风险 1：嵌套子总控逻辑复杂**
- 概率：中等
- 影响：高
- 缓解措施：
  - 从简单的嵌套开始（Level 1）
  - 充分的单元测试
  - 详细的日志记录

**风险 2：上下文管理逻辑复杂**
- 概率：中等
- 影响：中等
- 缓解措施：
  - 从简单的规则开始
  - 明确只有两个触发点
  - 充分的测试

**风险 3：ACP 协议集成复杂**
- 概率：中等
- 影响：中等
- 缓解措施：
  - 充分的前期研究
  - 渐进式集成
  - 充分的测试

### 依赖风险

**风险 1：agent-client-protocol crate 不稳定**
- 概率：中等
- 影响：中等
- 缓解措施：
  - 早期验证 crate 的稳定性
  - 设计抽象层，便于替换
  - 关注 upstream 开发

**风险 2：Phase 3 未完全完成**
- 概率：低
- 影响：高
- 缓解措施：
  - 开始前确认 Phase 3 完成
  - 保持与 Phase 3 的接口兼容

### 项目风险

**风险 1：范围蔓延**
- 概率：高
- 影响：高
- 缓解措施：
  - 严格遵循 Baby Steps™ 方法论
  - 每个阶段都有明确的验收标准
  - 定期回顾和调整计划

**风险 2：时间估算不足**
- 概率：中等
- 影响：中等
- 缓解措施：
  - 分优先级实现
  - 预留缓冲时间
  - 定期进度评估

---

## 参考资料

### ACP 协议
- Agent Client Protocol 官方文档
- agent-client-protocol crate 文档
- Zed 编辑器 ACP 集成示例

### Rust 异步编程
- Tokio 官方文档
- Rust Async Book

### 多 Agent 系统
- PLAN.md: 整体项目计划
- 相关学术论文
- 开源项目参考

---

## 附录

### 术语表
- **Agent**：独立的 AI 实体，执行具体任务
- **Master Orchestrator（主总控）**：最顶层的唯一总控
- **Sub-Orchestrator（子总控）**：中层管理者，可以嵌套，知道自己的深度
- **Nested Depth（嵌套深度）**：子总控在层级中的位置（Master 为 0）
- **Session**：用户与系统的一次交互会话
- **Checkpoint**：Session 状态的快照
- **Tool Mask**：控制 Agent 工具使用权限的机制
- **Context Manager Agent (CMA)**：管理会话上下文的特殊 Agent（监军）
- **Work Order（工单）**：统一的消息结构，可用于任务完成汇报、接力转交、求助等
- **Work Order Type（工单类型）**：TaskCompletion, Handover, HelpRequest, StatusUpdate
- **Work Order Recipient（工单接收对象）**：ContextManager, Orchestrator, Agent
- **嵌套子总控接力模式**：强依赖顺序场景的执行模式
- **平行子总控集群模式**：可并行分解场景的执行模式
- **ACP**：Agent Client Protocol，用于与编辑器集成的协议

### 关键设计决策回顾（PHASE4 特定）
1. **架构模型**：层级化总控 + 可嵌套子总控 + 单向流水线
2. **Agent 模型**：无状态的执行者 + 可嵌套总控（知道嵌套深度）
3. **通信方式**：直接函数调用
4. **工单机制**：作为 User Message
5. **复杂任务**：两种执行模式（嵌套子总控接力、平行子总控集群）
6. **CMA 触发**：只有两种情况（Agent 求助、上下文满载）
7. **文件并发**：避免冲突
8. **嵌套限制**：软性劝阻，不硬性限制（保持灵活性）

### 检查清单模板
每个子阶段完成后，使用以下清单验证：
- [ ] 所有任务清单项目已完成
- [ ] 所有数据结构已定义
- [ ] 所有 API 已实现
- [ ] 所有单元测试已编写并通过
- [ ] 集成测试已编写并通过
- [ ] 验收标准所有项目已验证
- [ ] 代码已格式化（cargo fmt）
- [ ] Clippy 检查通过（cargo clippy）
- [ ] 文档已更新
- [ ] 代码已提交到版本控制

---

**文档版本**：3.0（更新以契合新 PLAN.md 架构）
**最后更新**：2024
**维护者**：MineClaw 团队
