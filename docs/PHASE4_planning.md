# MineClaw Phase 4 详细规划

## 概述

### 阶段目标
建立多 Agent 运行的基础框架，让单个 Agent 能够正常工作并支持基本管理。

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
- 单个 Agent 可以完整执行任务（接收消息、调用 LLM、使用工具、返回响应）
- Agent 状态机完整实现并验证
- 消息总线可以在 Agent 间传递消息
- Session 与 Checkpoint 完整集成
- 基础 API 可以管理 Agent 和 Session
- 所有单元测试通过
- 完整的文档和验收清单

### 与前后阶段的依赖关系
- **前置依赖**：Phase 3 完成（本地工具与 Checkpoint 集成）
- **后续阶段**：Phase 5（任务编排与路由系统）

---

## 关键设计决策

### 1. Agent 模型：进程内多 Agent
**决策**：采用进程内多 Agent 架构

**原因**：
- 简化调试和开发
- 减少通信开销
- 便于状态共享和管理
- 可以在后续阶段轻松扩展为进程间架构

**设计**：
- 每个 Agent 运行在独立的 tokio task 中
- 通过消息总线进行通信
- 共享的应用状态通过 Arc<Mutex> 或 Arc<RwLock> 保护

### 2. 消息总线：tokio::sync::mpsc + broadcast
**决策**：使用 tokio 标准库的 mpsc 和 broadcast channel

**原因**：
- 轻量级、高性能
- 与 tokio 运行时完美集成
- 足够满足 Phase 4 的需求
- 可以在需要时轻松替换为更复杂的方案

**设计**：
- 点对点通信：使用 mpsc::channel
- 广播通信：使用 broadcast::channel（可选，Phase 4 可能不需要）
- 消息确认：通过单独的响应 channel 实现

### 3. Agent 状态机设计

**状态定义**：
```
Idle（空闲）
  ↓ [任务分配]
Busy（忙碌）
  ↓ [任务完成/出错]
Idle / Error
  ↓ [错误恢复]
Idle

Paused（暂停）- 可从任何状态进入
  ↓ [恢复]
原状态
```

**状态转换规则**：
- Idle → Busy：分配新任务
- Busy → Idle：任务成功完成
- Busy → Error：任务执行出错
- Error → Idle：错误恢复成功
- 任何状态 → Paused：暂停指令
- Paused → 原状态：恢复指令

### 4. 与现有代码的集成策略

**复用现有组件**：
- LLM 客户端（src/llm/）- 完全复用
- MCP 工具集成（src/mcp/）- 完全复用
- 本地工具（src/tools/）- 完全复用
- Checkpoint 系统（src/checkpoint/）- 增强集成
- Session 模型（src/models/session.rs）- 增强

**新增组件**：
- Agent 定义与管理（src/agent/）
- 消息总线（src/message_bus/）
- 工具掩码（src/tool_mask/）
- 上下文管理（src/context_manager/）

---

## 详细实施计划

---

## 🎯 第一优先级：核心基础设施

### Phase 4.1: Agent 基础定义与生命周期管理

#### 任务清单
- [ ] 定义 AgentId 类型（新type 包装 Uuid）
- [ ] 定义 AgentRole 枚举（通用 Agent、上下文管理 Agent、总控 Agent 等）
- [ ] 定义 AgentCapability 标签系统
- [ ] 定义 LLM 配置结构
- [ ] 定义 AgentState 枚举（Idle, Busy, Error, Paused）
- [ ] 定义 Agent 核心数据结构
- [ ] 定义 AgentConfig 配置结构
- [ ] 实现 Agent 仓库（存储和管理所有 Agent 实例）
- [ ] 实现 Agent 创建流程
- [ ] 实现 Agent 初始化流程
- [ ] 实现状态转换验证逻辑
- [ ] 实现健康检查机制
- [ ] 实现错误恢复机制
- [ ] 实现优雅关闭流程
- [ ] 编写单元测试
- [ ] 验证验收清单

#### 数据结构设计

**AgentId**
- 唯一标识 Agent
- 使用 Uuid v4
- 支持序列化和反序列化
- 实现 Display、Debug、Clone、Copy、PartialEq、Eq、Hash

**AgentRole**
- 枚举类型，定义 Agent 的角色
- 初始值：GeneralPurpose, ContextManager, Orchestrator
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
- Error: 错误状态，包含错误信息
- Paused: 暂停状态，记录暂停前的状态

**Agent**
- id: AgentId
- name: String（人类可读名称）
- role: AgentRole
- capabilities: Vec<AgentCapability>
- llm_config: LlmConfig
- state: AgentState
- system_prompt: Option<String>
- created_at: DateTime<Utc>
- updated_at: DateTime<Utc>

**AgentConfig**
- name: String
- role: AgentRole
- capabilities: Vec<String>
- llm_config: LlmConfig
- system_prompt: Option<String>

**AgentRepository**
- 存储所有 Agent 实例
- 支持 CRUD 操作
- 线程安全
- 支持按 ID、角色、能力查询

#### API 设计

**Agent 创建**
- 输入：AgentConfig
- 输出：Result<Agent, Error>
- 验证配置有效性
- 生成唯一 ID
- 初始化状态为 Idle
- 保存到仓库

**Agent 状态查询**
- 输入：AgentId
- 输出：Result<AgentState, Error>

**Agent 完整信息查询**
- 输入：AgentId
- 输出：Result<Agent, Error>

**Agent 列表查询**
- 输入：可选过滤条件（角色、能力、状态）
- 输出：Result<Vec<Agent>, Error>

**Agent 状态更新**
- 输入：AgentId, 新状态
- 输出：Result<(), Error>
- 验证状态转换的合法性

**Agent 删除**
- 输入：AgentId
- 输出：Result<(), Error>
- 确保 Agent 不在 Busy 状态
- 清理相关资源

**健康检查**
- 输入：AgentId
- 输出：Result<HealthStatus, Error>
- HealthStatus: Healthy, Unhealthy, Unknown

#### 测试策略
- 单元测试：所有数据结构和基础操作
- 状态机测试：验证所有合法和非法的状态转换
- 并发测试：验证多线程环境下的仓库安全性
- 错误处理测试：验证各种错误场景的处理

#### 验收标准
- [ ] 可以创建 Agent 并分配唯一 ID
- [ ] Agent 状态机所有合法转换都能正常工作
- [ ] 非法状态转换被正确拒绝
- [ ] 可以查询 Agent 列表和详情
- [ ] 可以按角色、能力、状态过滤 Agent
- [ ] 可以更新 Agent 状态
- [ ] 可以删除空闲状态的 Agent
- [ ] 不能删除忙碌状态的 Agent
- [ ] 健康检查机制正常工作
- [ ] 所有单元测试通过
- [ ] 并发测试通过

---

### Phase 4.2: 消息总线基础

#### 任务清单
- [ ] 定义 MessageId 类型
- [ ] 定义 MessageType 枚举
- [ ] 定义 Message 核心结构
- [ ] 定义 MessagePayload trait
- [ ] 实现消息序列化和反序列化
- [ ] 实现消息 ID 生成器
- [ ] 定义点对点消息通道
- [ ] 实现消息发送机制
- [ ] 实现消息接收机制
- [ ] 实现消息确认机制
- [ ] 实现超时处理
- [ ] 实现基础的错误重试
- [ ] 定义 MessageBus 中央协调器
- [ ] 编写单元测试
- [ ] 验证验收清单

#### 数据结构设计

**MessageId**
- 唯一标识消息
- 使用 Uuid v4
- 支持序列化和反序列化
- 实现 Display、Debug、Clone、Copy、PartialEq、Eq、Hash

**MessageType**
- 枚举类型，定义消息类型
- 初始值：TaskRequest, TaskResponse, Heartbeat, SystemNotification, Error
- 可扩展

**MessagePriority**
- 枚举类型，定义消息优先级
- 值：Low, Normal, High, Critical
- 默认：Normal

**Message**
- id: MessageId
- message_type: MessageType
- priority: MessagePriority
- from: Option<AgentId>（None 表示系统）
- to: Option<AgentId>（None 表示广播）
- payload: Vec<u8>（序列化的 payload）
- created_at: DateTime<Utc>
- expires_at: Option<DateTime<Utc>>
- correlation_id: Option<MessageId>（用于关联请求-响应）

**TaskRequestPayload**
- task_description: String
- context: Option<Vec<u8>>
- tools: Option<Vec<String>>
- deadline: Option<DateTime<Utc>>

**TaskResponsePayload**
- success: bool
- result: Option<Vec<u8>>
- error: Option<String>
- execution_time_ms: u64

**HeartbeatPayload**
- status: AgentState
- timestamp: DateTime<Utc>

**MessageBus**
- 中央消息协调器
- 维护所有 Agent 的消息通道
- 路由消息到目标 Agent
- 管理消息超时和重试

#### API 设计

**消息创建**
- 输入：MessageType, 优先级, 发送者, 接收者, payload
- 输出：Message
- 自动生成 MessageId 和时间戳

**消息发送**
- 输入：Message
- 输出：Result<MessageId, Error>
- 验证消息有效性
- 路由到目标接收者
- 返回消息 ID

**消息接收**
- 输入：AgentId, 可选超时时间
- 输出：Result<Option<Message>, Error>
- 非阻塞，如果没有消息返回 None
- 支持超时等待

**消息确认**
- 输入：MessageId
- 输出：Result<(), Error>
- 标记消息为已处理

**消息订阅（点对点）**
- 输入：AgentId
- 输出：Result<Receiver<Message>, Error>
- 为 Agent 创建消息接收通道

**消息取消订阅**
- 输入：AgentId
- 输出：Result<(), Error>
- 清理 Agent 的消息通道

**消息查询（已发送但未确认）**
- 输入：AgentId
- 输出：Result<Vec<Message>, Error>

**发送请求并等待响应**
- 输入：请求消息, 超时时间
- 输出：Result<Message, Error>
- 封装请求-响应模式
- 自动处理 correlation_id
- 超时后返回错误

#### 测试策略
- 单元测试：消息创建、序列化、反序列化
- 集成测试：两个 Agent 之间的通信
- 超时测试：验证消息超时机制
- 重试测试：验证错误重试逻辑
- 并发测试：多 Agent 同时发送消息
- 压力测试：大量消息的处理能力

#### 验收标准
- [ ] 可以创建消息并自动生成唯一 ID
- [ ] 消息可以正确序列化和反序列化
- [ ] Agent 可以订阅消息通道
- [ ] 点对点消息可以正确传递
- [ ] 消息确认机制正常工作
- [ ] 超时机制正常工作
- [ ] 请求-响应模式正常工作
- [ ] 消息优先级被正确处理
- [ ] 过期消息被正确处理
- [ ] 可以查询未确认的消息
- [ ] Agent 可以取消订阅
- [ ] 并发消息传递正常工作
- [ ] 所有单元测试通过
- [ ] 集成测试通过

---

### Phase 4.3: Checkpoint 与会话增强

#### 任务清单
- [ ] 回顾现有的 Checkpoint 实现
- [ ] 回顾现有的 Session 模型
- [ ] 定义 SessionState 枚举
- [ ] 增强 Session 结构（关联 Agent）
- [ ] 定义 Session 生命周期事件
- [ ] 实现 Session 创建流程
- [ ] 实现 Session 激活流程
- [ ] 实现 Session 归档流程
- [ ] 实现 Session 删除流程
- [ ] 实现 Session 与 Agent 的关联
- [ ] 实现 Session 与 Checkpoint 的强关联
- [ ] 定义 Checkpoint 归档策略
- [ ] 实现 Checkpoint 跟随 Session 生命周期
- [ ] 实现 Checkpoint 清理策略
- [ ] 优化 AgentFS 集成
- [ ] 定义 SessionRepository
- [ ] 编写单元测试
- [ ] 编写集成测试
- [ ] 验证验收清单

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
- agent_id: Option<AgentId>（新增，关联的 Agent）
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
- 支持按 Agent 查询
- 线程安全

**CheckpointArchivingStrategy**
- 配置何时归档 Checkpoint
- 选项：Session 归档时、手动触发、定期
- 配置保留策略（保留多少个 Checkpoint）

#### API 设计

**Session 创建**
- 输入：可选标题、可选 AgentId
- 输出：Result<Session, Error>
- 初始状态：Draft
- 创建初始 Checkpoint

**Session 激活**
- 输入：SessionId
- 输出：Result<(), Error>
- 状态转换：Draft/Paused → Active
- 如果有关联 Agent，通知 Agent

**Session 暂停**
- 输入：SessionId
- 输出：Result<(), Error>
- 状态转换：Active → Paused
- 创建 Checkpoint
- 如果有关联 Agent，通知 Agent

**Session 恢复**
- 输入：SessionId
- 输出：Result<(), Error>
- 状态转换：Paused → Active
- 恢复到最新的 Checkpoint
- 如果有关联 Agent，通知 Agent

**Session 归档**
- 输入：SessionId
- 输出：Result<(), Error>
- 状态转换：任何状态 → Archived
- 创建最终 Checkpoint
- 归档所有相关 Checkpoint
- 如果有关联 Agent，通知 Agent 释放资源

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

**Session 关联 Agent**
- 输入：SessionId, AgentId
- 输出：Result<(), Error>
- Session 必须是 Draft 或 Active 状态

**Session 解绑 Agent**
- 输入：SessionId
- 输出：Result<(), Error>
- 创建 Checkpoint
- 通知 Agent

**Session 查询**
- 输入：SessionId
- 输出：Result<Session, Error>

**Session 列表查询**
- 输入：可选过滤条件（状态、AgentId、创建时间范围）
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
- 集成测试：Session 与 Agent 的协作
- 集成测试：Session 与 Checkpoint 的集成
- 回归测试：确保现有功能不受影响

#### 验收标准
- [ ] 可以创建 Session，初始状态为 Draft
- [ ] Session 状态机所有合法转换都能正常工作
- [ ] 非法状态转换被正确拒绝
- [ ] 可以将 Session 关联到 Agent
- [ ] 可以解绑 Agent
- [ ] Session 创建时自动创建初始 Checkpoint
- [ ] Session 状态变化时自动创建 Checkpoint
- [ ] Session 归档时归档所有 Checkpoint
- [ ] 可以查询 Session 列表
- [ ] 可以按状态、Agent 过滤 Session
- [ ] 可以查询 Session 的生命周期历史
- [ ] 可以恢复到指定的 Checkpoint
- [ ] 可以清理过期的 Checkpoint
- [ ] 软删除的 Session 不再出现在正常列表中
- [ ] 永久删除的 Session 及其 Checkpoint 被完全清理
- [ ] 所有单元测试通过
- [ ] 集成测试通过
- [ ] 回归测试通过

---

## 🏗️ 第二优先级：功能模块

### Phase 4.4: 工具掩码基础机制

#### 任务清单
- [ ] 定义 ToolId 类型
- [ ] 定义 ToolCategory 枚举
- [ ] 定义 ToolPermission 枚举
- [ ] 定义 ToolDescriptor 结构
- [ ] 定义 ToolMask 结构
- [ ] 实现工具分类（MCP 工具、本地工具、终端工具）
- [ ] 实现工具注册表
- [ ] 实现工具掩码配置
- [ ] 实现 Agent 工具集分配
- [ ] 实现工具调用权限检查
- [ ] 实现终端工具特殊处理（全开放）
- [ ] 定义 ToolMaskRepository
- [ ] 与现有 ToolCoordinator 集成
- [ ] 编写单元测试
- [ ] 验证验收清单

#### 数据结构设计

**ToolId**
- 唯一标识工具
- 格式：{category}:{name}
- 例如："mcp:filesystem_read", "local:git_status", "terminal:bash"
- 实现 Display、Debug、Clone、PartialEq、Eq、Hash

**ToolCategory**
- 枚举类型，定义工具分类
- 值：Mcp, Local, Terminal
- 实现 Display、Debug、Clone、PartialEq、Eq

**ToolPermission**
- 枚举类型，定义工具权限级别
- 值：Denied, ReadOnly, ReadWrite, Full
- 实现 Display、Debug、Clone、PartialEq、Eq, PartialOrd, Ord

**ToolDescriptor**
- id: ToolId
- name: String
- description: String
- category: ToolCategory
- default_permission: ToolPermission
- input_schema: Option<serde_json::Value>（工具输入参数定义）
- output_schema: Option<serde_json::Value>（工具输出定义）
- is_dangerous: bool（是否是危险操作）
- requires_approval: bool（是否需要审批）

**ToolMask**
- agent_id: AgentId
- tool_id: ToolId
- permission: ToolPermission
- granted_at: DateTime<Utc>
- granted_by: Option<String>（谁授权的）
- expires_at: Option<DateTime<Utc>>（授权过期时间）
- notes: Option<String>（备注）

**ToolRegistry**
- 存储所有可用工具的描述符
- 支持按分类查询
- 支持按名称搜索
- 线程安全

**AgentToolSet**
- agent_id: AgentId
- allowed_tools: HashMap<ToolId, ToolMask>
- default_permission: ToolPermission（未明确配置的工具的默认权限）
- updated_at: DateTime<Utc>

**ToolMaskRepository**
- 存储所有 Agent 的工具掩码配置
- 支持按 Agent 查询
- 支持按工具查询
- 线程安全

#### API 设计

**工具注册**
- 输入：ToolDescriptor
- 输出：Result<(), Error>
- 验证工具描述符有效性
- 添加到注册表

**工具注销**
- 输入：ToolId
- 输出：Result<(), Error>

**查询所有可用工具**
- 输出：Result<Vec<ToolDescriptor>, Error>

**按分类查询工具**
- 输入：ToolCategory
- 输出：Result<Vec<ToolDescriptor>, Error>

**搜索工具**
- 输入：关键词
- 输出：Result<Vec<ToolDescriptor>, Error>

**为 Agent 配置工具权限**
- 输入：AgentId, ToolId, ToolPermission, 可选过期时间, 可选备注
- 输出：Result<ToolMask, Error>
- 验证工具存在
- 验证权限级别不超过工具的最大允许权限

**批量配置 Agent 工具权限**
- 输入：AgentId, Vec<(ToolId, ToolPermission)>
- 输出：Result<Vec<ToolMask>, Error>

**移除 Agent 工具权限**
- 输入：AgentId, ToolId
- 输出：Result<(), Error>

**查询 Agent 的工具集**
- 输入：AgentId
- 输出：Result<AgentToolSet, Error>

**查询 Agent 对某个工具的权限**
- 输入：AgentId, ToolId
- 输出：Result<ToolPermission, Error>
- 如果未配置，返回默认权限

**检查工具调用权限**
- 输入：AgentId, ToolId, 请求的权限级别
- 输出：Result<bool, Error>
- 返回 true 表示有权限，false 表示无权限
- 终端工具总是返回 true（全开放）

**设置 Agent 的默认工具权限**
- 输入：AgentId, ToolPermission
- 输出：Result<(), Error>

**从现有 Agent 复制工具权限配置**
- 输入：源 AgentId, 目标 AgentId
- 输出：Result<(), Error>

**清理过期的工具权限**
- 输出：Result<usize, Error>（清理的数量）

#### 与现有 ToolCoordinator 集成
- 在调用工具前进行权限检查
- 对于被拒绝的工具，返回权限错误
- 对于只读工具，限制为只读操作（如果工具支持）
- 终端工具绕过权限检查

#### 测试策略
- 单元测试：工具注册表、权限检查
- 单元测试：终端工具特殊处理
- 集成测试：与 ToolCoordinator 的集成
- 安全测试：尝试越权调用工具

#### 验收标准
- [ ] 可以注册工具到注册表
- [ ] 可以查询所有可用工具
- [ ] 可以按分类和关键词搜索工具
- [ ] 可以为 Agent 配置工具权限
- [ ] 可以批量配置工具权限
- [ ] 可以移除工具权限
- [ ] 可以查询 Agent 的工具集
- [ ] 权限检查正常工作
- [ ] 只读权限不允许写操作
- [ ] 终端工具总是允许调用
- [ ] 过期的权限自动失效
- [ ] 可以复制 Agent 的权限配置
- [ ] 与 ToolCoordinator 集成正常
- [ ] 越权调用被正确拒绝
- [ ] 所有单元测试通过
- [ ] 集成测试通过
- [ ] 安全测试通过

---

### Phase 4.5: 上下文管理 Agent（基础版）

#### 任务清单
- [ ] 定义 ContextId 类型
- [ ] 定义 ContextChunk 结构
- [ ] 定义 ContextMetadata 结构
- [ ] 定义 ContextStore 结构
- [ ] 定义裁剪触发条件
- [ ] 实现上下文接收与存储
- [ ] 实现上下文长度监控
- [ ] 实现基于规则的裁剪触发判断
- [ ] 定义转交判断规则
- [ ] 实现转交时机识别
- [ ] 定义上下文裁剪策略模板
- [ ] 定义转交判断策略模板
- [ ] 实现模板版本管理
- [ ] 实现 ContextManagerAgent（作为特殊 Agent）
- [ ] 编写单元测试
- [ ] 验证验收清单

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
- 值：UserMessage, AssistantMessage, ToolCall, ToolResult, SystemPrompt, SystemNotification

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
- max_token_count: usize（最大 token 数）
- max_chunk_count: usize（最大块数）
- max_age_seconds: Option<u64>（最大存活时间）

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

**HandoverRule**
- 转交规则
- rule_type: HandoverRuleType
- condition: String（规则条件描述）
- target_agent_role: Option<AgentRole>（目标 Agent 角色）
- target_agent_id: Option<AgentId>（目标 Agent ID）
- priority: u8（规则优先级）

**HandoverRuleType**
- TokenLimitReached: token 达到限制
- TaskTypeMismatch: 任务类型不匹配
- ComplexityExceeded: 复杂度超出
- ExplicitRequest: 明确请求转交
- ErrorRecovery: 错误恢复

**StrategyTemplate**
- 模板 ID
- 模板名称
- 模板类型（Trimming, Handover）
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

**检查是否需要裁剪**
- 输入：SessionId, TrimmingTrigger
- 输出：Result<bool, Error>
- 返回 true 表示需要裁剪

**执行裁剪**
- 输入：SessionId, TrimmingStrategy
- 输出：Result<usize, Error>（裁剪的块数）
- 从存储中移除被裁剪的块（或标记为已裁剪）

**标记上下文块为重要**
- 输入：ContextId, is_important: bool, retention_priority: Option<u8>
- 输出：Result<(), Error>

**查询上下文元数据**
- 输入：SessionId
- 输出：Result<ContextMetadata, Error>

**添加转交规则**
- 输入：HandoverRule
- 输出：Result<(), Error>

**移除转交规则**
- 输入：规则 ID
- 输出：Result<(), Error>

**查询所有转交规则**
- 输出：Result<Vec<HandoverRule>, Error>

**检查是否需要转交**
- 输入：SessionId, 当前 AgentId
- 输出：Result<Option<HandoverRecommendation>, Error>
- HandoverRecommendation 包含目标 Agent 和原因

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
- 监控 Session 上下文大小
- 自动触发裁剪
- 评估转交条件
- 生成转交建议
- 执行上下文裁剪（根据配置）

#### 测试策略
- 单元测试：上下文存储和检索
- 单元测试：裁剪策略
- 单元测试：转交规则
- 集成测试：ContextManagerAgent 端到端流程

#### 验收标准
- [ ] 可以存储上下文块
- [ ] 可以获取完整上下文
- [ ] 可以正确计算 token 数
- [ ] 可以检查是否需要裁剪
- [ ] FIFO 裁剪策略正常工作
- [ ] 基于重要性的裁剪策略正常工作
- [ ] 混合裁剪策略正常工作
- [ ] 重要块被优先保留
- [ ] 可以标记块为重要
- [ ] 转交规则可以正确评估
- [ ] 可以检查是否需要转交
- [ ] 策略模板可以保存和读取
- [ ] 可以设置默认模板
- [ ] ContextManagerAgent 可以监控上下文
- [ ] ContextManagerAgent 可以自动触发裁剪
- [ ] 所有单元测试通过
- [ ] 集成测试通过

---

### Phase 4.6: 基础 API 扩展

#### 任务清单
- [ ] 设计 Agent 管理 REST API
- [ ] 实现 Agent 创建 API
- [ ] 实现 Agent 查询 API（列表和详情）
- [ ] 实现 Agent 更新 API
- [ ] 实现 Agent 删除 API
- [ ] 实现 Agent 状态查询 API
- [ ] 设计 Session 管理 REST API
- [ ] 实现 Session 创建 API
- [ ] 实现 Session 激活 API
- [ ] 实现 Session 暂停 API
- [ ] 实现 Session 归档 API
- [ ] 实现 Session 删除 API
- [ ] 实现 Session 查询 API（列表和详情）
- [ ] 实现 Session 历史查询 API
- [ ] 设计单 Agent 任务执行 API
- [ ] 实现任务提交 API
- [ ] 实现任务状态查询 API
- [ ] 实现任务取消 API
- [ ] 添加 API 请求/响应日志
- [ ] 添加 API 文档注释（OpenAPI/Swagger）
- [ ] 编写 API 集成测试
- [ ] 验证验收清单

#### API 设计

**Agent 管理 API**

`POST /api/agents`
- 创建新 Agent
- 请求体：AgentConfig
- 响应：Agent（包含生成的 ID）
- 状态码：201 Created

`GET /api/agents`
- 列出所有 Agent
- 查询参数：role, capability, state, page, page_size
- 响应：{ agents: Vec<Agent>, total: usize, page: usize, page_size: usize }
- 状态码：200 OK

`GET /api/agents/:id`
- 获取 Agent 详情
- 响应：Agent
- 状态码：200 OK, 404 Not Found

`PUT /api/agents/:id`
- 更新 Agent 配置
- 请求体：部分 AgentConfig 字段
- 响应：Agent
- 状态码：200 OK, 404 Not Found

`DELETE /api/agents/:id`
- 删除 Agent
- 响应：204 No Content, 404 Not Found, 409 Conflict（如果 Agent 忙碌）

`GET /api/agents/:id/state`
- 获取 Agent 状态
- 响应：{ state: AgentState, updated_at: DateTime }
- 状态码：200 OK, 404 Not Found

`GET /api/agents/:id/health`
- 健康检查
- 响应：{ status: HealthStatus, details: Option<String> }
- 状态码：200 OK, 404 Not Found

**Session 管理 API**

`POST /api/sessions`
- 创建新 Session
- 请求体：{ title?: string, agent_id?: string }
- 响应：Session
- 状态码：201 Created

`GET /api/sessions`
- 列出所有 Session
- 查询参数：state, agent_id, created_before, created_after, page, page_size
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

`POST /api/sessions/:id/assign-agent`
- 为 Session 分配 Agent
- 请求体：{ agent_id: string }
- 响应：Session
- 状态码：200 OK, 404 Not Found, 409 Conflict

`POST /api/sessions/:id/unassign-agent`
- 解绑 Agent
- 响应：Session
- 状态码：200 OK, 404 Not Found

**单 Agent 任务执行 API**

`POST /api/tasks`
- 提交任务
- 请求体：{ session_id?: string, agent_id: string, task_description: string, context?: any }
- 响应：{ task_id: string, status: TaskStatus }
- 状态码：202 Accepted

`GET /api/tasks/:id`
- 获取任务状态
- 响应：{ task_id: string, status: TaskStatus, result?: any, error?: string, created_at: DateTime, updated_at: DateTime }
- 状态码：200 OK, 404 Not Found

`DELETE /api/tasks/:id`
- 取消任务
- 响应：204 No Content, 404 Not Found, 409 Conflict（如果任务已完成）

`GET /api/tasks`
- 列出任务
- 查询参数：agent_id, session_id, status, page, page_size
- 响应：{ tasks: Vec<TaskInfo>, total: usize, page: usize, page_size: usize }
- 状态码：200 OK

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
- [ ] 可以通过 API 创建 Agent
- [ ] 可以通过 API 查询 Agent 列表和详情
- [ ] 可以通过 API 更新 Agent
- [ ] 可以通过 API 删除空闲 Agent
- [ ] 不能通过 API 删除忙碌 Agent
- [ ] 可以查询 Agent 状态和健康
- [ ] 可以通过 API 创建 Session
- [ ] 可以通过 API 查询 Session 列表和详情
- [ ] 可以通过 API 激活、暂停、归档 Session
- [ ] 可以通过 API 删除 Session
- [ ] 可以查询 Session 历史和 Checkpoint
- [ ] 可以恢复到指定 Checkpoint
- [ ] 可以为 Session 分配和解绑 Agent
- [ ] 可以通过 API 提交任务
- [ ] 可以查询任务状态
- [ ] 可以取消待执行的任务
- [ ] 单 Agent 任务执行流程完整工作
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
- 共享应用状态（AgentRepository, SessionRepository 等）
- 使用 ACP 协议与客户端通信

**ACP Agent 实现**
- 包装 MineClaw 的核心逻辑
- 将 ACP 请求转换为内部调用
- 将内部响应转换为 ACP 格式

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
- agent_id: Option<AgentId>（关联的 Agent）
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
3. 如果需要，分配 Agent
4. 将用户输入转换为内部消息
5. 调用 Agent 处理
6. 将 Agent 响应转换为 ACP 格式
7. 返回响应

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

**风险 1：Agent 并发控制复杂**
- 概率：中等
- 影响：高
- 缓解措施：
  - 从简单的进程内模型开始
  - 使用成熟的 tokio 同步原语
  - 充分的并发测试
  - 详细的日志记录

**风险 2：消息总线性能瓶颈**
- 概率：低（Phase 4 规模小）
- 影响：中等
- 缓解措施：
  - 使用高性能的 mpsc channel
  - 设计时预留扩展空间
  - 性能监控和基准测试

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
- 相关学术论文
- 开源项目参考

---

## 附录

### 术语表
- **Agent**：独立的 AI 实体，可以执行任务
- **Session**：用户与系统的一次交互会话
- **Checkpoint**：Session 状态的快照
- **Message Bus**：Agent 间通信的基础设施
- **Tool Mask**：控制 Agent 工具使用权限的机制
- **Context Manager**：管理会话上下文的特殊 Agent
- **ACP**：Agent Client Protocol，用于与编辑器集成的协议

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

**文档版本**：1.0
**最后更新**：2024
**维护者**：MineClaw 团队