# MineClaw Phase 2.7: API 扩展和管理功能 - 测试文档

## 概述

Phase 2.7 添加了管理 API 和监控功能，用于查看和管理 MCP 服务器状态。

## 功能列表

1. `GET /api/tools` - 列出所有可用工具
2. `GET /api/mcp/servers` - 列出 MCP 服务器状态
3. `POST /api/mcp/servers/:name/restart` - 重启 MCP 服务器

---

## 前置条件

1. 确保配置文件 `config/mineclaw.toml` 已正确配置 MCP 服务器
2. 确保测试服务器 `test-mcp-server.js` 存在
3. 确保 Node.js 已安装

---

## 测试步骤

### 1. 启动 MineClaw 服务

```bash
cargo run
```

服务应该在 `http://127.0.0.1:18789` 启动。

---

### 2. 测试 `GET /api/tools` - 列出所有工具

**请求：**
```bash
curl -X GET http://127.0.0.1:18789/api/tools
```

**预期响应：**
```json
{
  "tools": [
    {
      "name": "echo",
      "description": "Echo back the input message",
      "server_name": "test-server",
      "input_schema": {
        "type": "object",
        "properties": {
          "message": {
            "type": "string"
          }
        },
        "required": ["message"]
      }
    },
    {
      "name": "add",
      "description": "Add two numbers",
      "server_name": "test-server",
      "input_schema": {
        "type": "object",
        "properties": {
          "a": {
            "type": "number"
          },
          "b": {
            "type": "number"
          }
        },
        "required": ["a", "b"]
      }
    }
  ]
}
```

**验证点：**
- [ ] 响应状态码为 200
- [ ] 返回的工具列表包含 `echo` 和 `add`
- [ ] 每个工具都有正确的 `server_name` 字段
- [ ] `input_schema` 格式正确

---

### 3. 测试 `GET /api/mcp/servers` - 列出 MCP 服务器

**请求：**
```bash
curl -X GET http://127.0.0.1:18789/api/mcp/servers
```

**预期响应：**
```json
{
  "servers": [
    {
      "name": "test-server",
      "status": {
        "type": "Connected"
      },
      "tool_count": 2,
      "uptime_seconds": 120,
      "last_health_check": "2026-03-05T20:30:00Z"
    }
  ]
}
```

**验证点：**
- [ ] 响应状态码为 200
- [ ] 服务器列表包含 `test-server`
- [ ] 状态为 `Connected`
- [ ] `tool_count` 为 2
- [ ] `uptime_seconds` 有值（大于 0）
- [ ] `last_health_check` 有值

---

### 4. 测试 `POST /api/mcp/servers/{name}/restart` - 重启 MCP 服务器

**请求：**
```bash
curl -X POST http://127.0.0.1:18789/api/mcp/servers/test-server/restart
```

**预期响应：**
```json
{
  "success": true,
  "message": "Server 'test-server' restarted successfully"
}
```

**验证点：**
- [ ] 响应状态码为 200
- [ ] `success` 为 `true`
- [ ] 消息表明重启成功

**重启后验证：**
再次调用 `/api/mcp/servers`，确认服务器仍然处于 `Connected` 状态。

---

### 5. 测试错误场景：重启不存在的服务器

**请求：**
```bash
curl -X POST http://127.0.0.1:18789/api/mcp/servers/nonexistent-server/restart
```

**预期响应：**
```json
{
  "success": false,
  "message": "Failed to restart server: ..."
}
```

**验证点：**
- [ ] 响应状态码为 200
- [ ] `success` 为 `false`
- [ ] 消息表明服务器未找到

---

## API 完整参考

### GET /api/tools

列出所有 MCP 服务器提供的所有可用工具。

**响应字段：**
- `tools`: 工具信息数组
  - `name`: 工具名称
  - `description`: 工具描述
  - `server_name`: 提供此工具的服务器名称
  - `input_schema`: 工具输入参数 schema

---

### GET /api/mcp/servers

列出所有 MCP 服务器的状态信息。

**响应字段：**
- `servers`: 服务器信息数组
  - `name`: 服务器名称
  - `status`: 服务器状态
    - `Connecting`: 正在连接
    - `Connected`: 已连接
    - `Disconnected`: 已断开
    - `Error`: 发生错误（包含错误消息）
  - `tool_count`: 服务器提供的工具数量
  - `uptime_seconds`: 运行时间（秒）
  - `last_health_check`: 最后健康检查时间（ISO 8601）

---

### POST /api/mcp/servers/:name/restart

重启指定的 MCP 服务器。

**路径参数：**
- `name`: 服务器名称

**响应字段：**
- `success`: 是否成功
- `message`: 结果消息

---

## 数据结构说明

### ServerStatus

服务器状态枚举，使用 tag-content 格式序列化：

```json
// 正在连接
{ "type": "Connecting" }

// 已连接
{ "type": "Connected" }

// 已断开
{ "type": "Disconnected" }

// 发生错误
{ "type": "Error", "message": "错误描述" }
```

---

## 集成测试

Phase 2.7 不新增集成测试，现有集成测试已覆盖核心功能。

运行所有测试：
```bash
cargo test
```

---

## 注意事项

1. 服务器重启期间，该服务器提供的工具将暂时不可用
2. 重启后，服务器的 `uptime_seconds` 会重新开始计数
3. `last_health_check` 在服务器启动时和每次健康检查时更新
4. 如果服务器配置丢失，重启将失败

---

## 完成检查清单

- [ ] `GET /api/tools` 正常工作
- [ ] `GET /api/mcp/servers` 正常工作
- [ ] `POST /api/mcp/servers/:name/restart` 正常工作
- [ ] 错误场景处理正确
- [ ] 所有单元测试通过
- [ ] 所有集成测试通过