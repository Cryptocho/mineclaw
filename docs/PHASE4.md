# MineClaw Phase 4: 多 Agent 基础架构 详细设计


## 概述

### 目标
建立多 Agent 运行的基础框架，支持层级化总控架构、可嵌套子总控、上下文管理 Agent 的两种触发机制。

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

### 成功标准（Definition of Done）
- 主总控可以创建和管理子总控（知道嵌套深度）
- 子总控可以创建普通 Agent、更小的子总控或集群
- Agent 可以主动向 CMA 发送求助工单
- CMA 只在两种情况下触发：Agent 求助、上下文满载
- 上下文管理 Agent 可以裁剪上下文、判断回退和转交
- Session 与 Checkpoint 完整集成
- 所有单元测试通过
- 完整的文档和验收清单

### 与前后阶段的依赖关系
- **前置依赖**：Phase 3 完成（本地工具与 Checkpoint 集成）
- **后续阶段**：
  - Phase 5（任务编排与路由系统）
  - Phase 6（高级协作与质量保障）
  - Phase 7（核心 API 层）
  - Phase 8（Flutter 前端）
  - Phase 9（经验学习与持续进化）- 最低优先级

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
- [x] Agent 可以调用 LLM
- [x] Agent 可以使用工具 (基础框架实现，完整工具循环待后续增强)
- [x] Agent 可以返回响应
- [x] Agent 状态正确更新
- [x] Agent 可以发送工单给 CMA（求助）
- [x] Agent 可以发送工单给总控（任务完成）
- [x] 错误处理正常工作
- [x] 所有单元测试通过 (119 tests passed)
- [x] 集成测试通过

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
- [x] 实现任务分配功能（并行）
- [x] 实现结果回收功能
- [x] 实现工单生成和处理（统一工单结构）
- [x] 实现 Session 与 Agent 的关联
- [x] 实现 CMA 通知处理（回退和转交）
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

**TaskManager 取消任务（基础设施预留）**
- 输入：&amp;mut TaskManager, TaskId
- 输出：Result&lt;(), Error&gt;
- abort JoinHandle
- 更新任务状态为 Cancelled
- 说明：此功能为 Phase 7-8 前端 API 预留，Agent 不直接调用取消任务

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
- [x] 总控可以并行分配任务
- [x] 总控可以查询任务状态
- [x] 总控可以等待任务完成
- [x] TaskManager 支持取消任务（基础设施预留，供 Phase 7-8 API 使用）
- [x] 总控可以生成工单
- [x] 总控可以处理 CMA 通知
- [x] 多个并行任务正常工作
- [x] 所有单元测试通过
- [x] 集成测试通过
- [x] 并发测试通过

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
- [x] **按 Agent 分组的 Checkpoint 列表**
- [x] 定义 Checkpoint 归档策略
- [x] 实现 Checkpoint 跟随 Session 生命周期
- [x] 实现 Checkpoint 清理策略
- [x] 优化 AgentFS 集成
- [x] 定义 SessionRepository
- [x] 编写单元测试
- [x] 编写集成测试
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
- **agent_checkpoints: HashMap<AgentId, Vec<CheckpointId>>（新增，按 Agent 分组的 Checkpoint 列表）**

**SessionLifecycleEvent**
- 枚举类型，定义 Session 生命周期事件
- 值：Created, Activated, Paused, Resumed, Archived, Deleted
- 包含事件时间戳
- 包含触发者信息

**Checkpoint**
- session_id: SessionId（确保存在）
- agent_id: Option&lt;AgentId&gt;（创建该 Checkpoint 的 Agent）
- is_archived: bool
- archived_at: Option&lt;DateTime&lt;Utc&gt;&gt;

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
- **集成测试：按 Agent 分组的 Checkpoint 管理**
- **集成测试：Agent 起始 Checkpoint 记录与回退**
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
- [x] **可以按 Agent 查询其创建的 Checkpoint 列表**
- [x] **可以回退到 Agent 接手时的状态**
- [x] 可以清理过期的 Checkpoint
- [x] 软删除的 Session 不再出现在正常列表中
- [x] 永久删除的 Session 及其 Checkpoint 被完全清理
- [x] 所有单元测试通过
- [x] 集成测试通过
- [x] 回归测试通过

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
- [x] 集成测试通过

---

### Phase 4.5: 上下文管理 Agent（基础版，两种触发机制） ✅

#### 任务清单
- [x] 定义 ContextId 类型
- [x] 定义 ContextChunk 结构
- [x] 定义 ContextMetadata 结构
- [x] 定义 ContextStore 结构
- [x] 实现 Agent 求助接收机制（触发点 1）
- [x] 实现上下文长度监控（触发点 2，唯一自动触发点）
- [x] 实现上下文清理（裁剪）
- [x] 实现持续犯错判断
- [x] 实现回退到指定 Checkpoint (由 CMA 建议并触发通知)
- [x] 实现触发总控转交 (通过 CmaNotification)
- [x] 定义上下文裁剪策略模板
- [x] 实现 ContextManagerAgent（作为服务型 Agent）
- [x] 编写单元测试
- [x] 验证验收清单

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
- 单元测试：工单接收和处理（来自 Agent 的求助）
- 单元测试：工单生成
- 集成测试：ContextManagerAgent 端到端流程（两种触发机制）

#### 验收标准
- [x] 可以存储上下文块
- [x] 可以获取完整上下文
- [x] 可以正确计算 token 数 (简易估算实现)
- [x] 可以检查是否需要裁剪（唯一自动触发点）
- [x] 可以接收 Agent 的求助工单（recipient = ContextManager）
- [x] FIFO 裁剪策略正常工作
- [x] 基于重要性的裁剪策略正常工作
- [x] 混合裁剪策略正常工作
- [x] 重要块被优先保留
- [x] 可以标记块为重要
- [x] 持续犯错检测正常工作
- [x] 可以决定回退到哪个 Checkpoint (通过工单建议)
- [x] 收到求助工单后可以通知总控进行转交
- [x] 可以从消息/工单创建上下文块
- [x] 策略模板可以保存和读取 (基础实现已完成，完整模板化待后续增强)
- [x] 可以设置默认模板/策略
- [x] ContextManagerAgent 只在两种情况下触发
- [x] ContextManagerAgent 可以监控上下文
- [x] ContextManagerAgent 可以自动触发裁剪
- [x] 所有单元测试通过
- [x] 集成测试通过 (CMA 逻辑验证)



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
