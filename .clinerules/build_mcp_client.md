# 构建 MCP 客户端完整指南

> 基于 https://modelcontextprotocol.io/docs/develop/build-client 文档整理

## 目录

- [概述](#概述)
- [前置准备](#前置准备)
- [系统要求](#系统要求)
- [环境设置](#环境设置)
- [API Key 配置](#api-key-配置)
- [创建客户端](#创建客户端)
- [关键组件详解](#关键组件详解)
- [运行客户端](#运行客户端)
- [工作原理](#工作原理)
- [最佳实践](#最佳实践)
- [常见问题排查](#常见问题排查)
- [后续步骤](#后续步骤)

---

## 概述

在本教程中，你将学习如何构建一个基于 LLM 的聊天机器人客户端，该客户端可以连接到 MCP 服务器。

在开始之前，建议先阅读 [构建 MCP 服务器](https://modelcontextprotocol.io/docs/develop/build-server) 教程，以便理解客户端和服务器之间的通信方式。

本教程提供以下语言版本：
- Python
- TypeScript
- Java
- Kotlin
- C#

本文档以 Python 为例。

---

## 前置准备

你可以在 [这里](https://github.com/modelcontextprotocol/quickstart-resources/tree/main/mcp-client-python) 找到本教程的完整代码。

---

## 系统要求

在开始之前，请确保你的系统满足以下要求：

- Mac 或 Windows 电脑
- 已安装最新版本的 Python
- 已安装最新版本的 `uv`

---

## 环境设置

首先，使用 `uv` 创建一个新的 Python 项目：

### macOS/Linux

```bash
# 创建项目目录
uv init mcp-client
cd mcp-client

# 创建虚拟环境
uv venv

# 激活虚拟环境
source .venv/bin/activate

# 安装所需的包
uv add mcp anthropic python-dotenv

# 移除样板文件
rm main.py

# 创建我们的主文件
touch client.py
```

### Windows

```powershell
# 创建项目目录
uv init mcp-client
cd mcp-client

# 创建虚拟环境
uv venv

# 激活虚拟环境
.venv\Scripts\activate

# 安装所需的包
uv add mcp anthropic python-dotenv

# 移除样板文件
del main.py

# 创建我们的主文件
echo. > client.py
```

---

## API Key 配置

你需要从 [Anthropic 控制台](https://console.anthropic.com/settings/keys) 获取一个 Anthropic API key。

创建一个 `.env` 文件来存储它：

```bash
echo "ANTHROPIC_API_KEY=your-api-key-goes-here" > .env
```

将 `.env` 添加到你的 `.gitignore`：

```bash
echo ".env" >> .gitignore
```

> **警告：** 请确保妥善保管你的 `ANTHROPIC_API_KEY`！

---

## 创建客户端

### 基本客户端结构

首先，让我们设置导入并创建基本的客户端类：

```python
import asyncio
from typing import Optional
from contextlib import AsyncExitStack
from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client
from anthropic import Anthropic
from dotenv import load_dotenv

load_dotenv()  # 从 .env 加载环境变量


class MCPClient:
    def __init__(self):
        # 初始化会话和客户端对象
        self.session: Optional[ClientSession] = None
        self.exit_stack = AsyncExitStack()
        self.anthropic = Anthropic()
        # 方法将在这里添加
```

### 服务器连接管理

接下来，我们将实现连接到 MCP 服务器的方法：

```python
    async def connect_to_server(self, server_script_path: str):
        """连接到 MCP 服务器

        Args:
            server_script_path: 服务器脚本路径（.py 或 .js）
        """
        is_python = server_script_path.endswith('.py')
        is_js = server_script_path.endswith('.js')
        if not (is_python or is_js):
            raise ValueError("服务器脚本必须是 .py 或 .js 文件")

        command = "python" if is_python else "node"
        server_params = StdioServerParameters(
            command=command,
            args=[server_script_path],
            env=None
        )

        stdio_transport = await self.exit_stack.enter_async_context(stdio_client(server_params))
        self.stdio, self.write = stdio_transport

        self.session = await self.exit_stack.enter_async_context(ClientSession(self.stdio, self.write))

        await self.session.initialize()

        # 列出可用的工具
        response = await self.session.list_tools()
        tools = response.tools
        print("\n已连接到服务器，可用工具：", [tool.name for tool in tools])
```

### 查询处理逻辑

现在让我们添加处理查询和处理工具调用的核心功能：

```python
    async def process_query(self, query: str) -> str:
        """使用 Claude 和可用工具处理查询"""
        messages = [
            {
                "role": "user",
                "content": query
            }
        ]

        response = await self.session.list_tools()
        available_tools = [{
            "name": tool.name,
            "description": tool.description,
            "input_schema": tool.inputSchema
        } for tool in response.tools]

        # 初始 Claude API 调用
        response = self.anthropic.messages.create(
            model="claude-sonnet-4-20250514",
            max_tokens=1000,
            messages=messages,
            tools=available_tools
        )

        # 处理响应并处理工具调用
        final_text = []
        assistant_message_content = []

        for content in response.content:
            if content.type == 'text':
                final_text.append(content.text)
                assistant_message_content.append(content)
            elif content.type == 'tool_use':
                tool_name = content.name
                tool_args = content.input

                # 执行工具调用
                result = await self.session.call_tool(tool_name, tool_args)
                final_text.append(f"[调用工具 {tool_name}，参数 {tool_args}]")
                assistant_message_content.append(content)

                messages.append({
                    "role": "assistant",
                    "content": assistant_message_content
                })
                messages.append({
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": content.id,
                            "content": result.content
                        }
                    ]
                })

                # 从 Claude 获取下一个响应
                response = self.anthropic.messages.create(
                    model="claude-sonnet-4-20250514",
                    max_tokens=1000,
                    messages=messages,
                    tools=available_tools
                )
                final_text.append(response.content[0].text)

        return "\n".join(final_text)
```

### 交互式聊天界面

现在我们将添加聊天循环和清理功能：

```python
    async def chat_loop(self):
        """运行交互式聊天循环"""
        print("\nMCP 客户端已启动！")
        print("输入你的查询，或输入 'quit' 退出。")

        while True:
            try:
                query = input("\n查询：").strip()
                if query.lower() == 'quit':
                    break
                response = await self.process_query(query)
                print("\n" + response)
            except Exception as e:
                print(f"\n错误：{str(e)}")

    async def cleanup(self):
        """清理资源"""
        await self.exit_stack.aclose()
```

### 主入口点

最后，我们将添加主要的执行逻辑：

```python
async def main():
    if len(sys.argv) < 2:
        print("用法：python client.py <服务器脚本路径>")
        sys.exit(1)

    client = MCPClient()
    try:
        await client.connect_to_server(sys.argv[1])
        await client.chat_loop()
    finally:
        await client.cleanup()


if __name__ == "__main__":
    import sys
    asyncio.run(main())
```

你可以在 [这里](https://github.com/modelcontextprotocol/quickstart-resources/blob/main/mcp-client-python/client.py) 找到完整的 `client.py` 文件。

---

## 关键组件详解

### 1. 客户端初始化

- `MCPClient` 类初始化时设置会话管理和 API 客户端
- 使用 `AsyncExitStack` 进行适当的资源管理
- 配置用于 Claude 交互的 Anthropic 客户端

### 2. 服务器连接

- 支持 Python 和 Node.js 服务器
- 验证服务器脚本类型
- 设置适当的通信通道
- 初始化会话并列出可用工具

### 3. 查询处理

- 维护对话上下文
- 处理 Claude 的响应和工具调用
- 管理 Claude 和工具之间的消息流
- 将结果组合成连贯的响应

### 4. 交互界面

- 提供简单的命令行界面
- 处理用户输入并显示响应
- 包括基本错误处理
- 允许优雅退出

### 5. 资源管理

- 适当的资源清理
- 连接问题的错误处理
- 优雅的关闭程序

---

## 常见自定义点

### 工具处理

- 修改 `process_query()` 以处理特定的工具类型
- 为工具调用添加自定义错误处理
- 实现工具特定的响应格式

### 响应处理

- 自定义工具结果的格式化方式
- 添加响应过滤或转换
- 实现自定义日志记录

### 用户界面

- 添加 GUI 或 Web 界面
- 实现丰富的控制台输出
- 添加命令历史或自动补全

---

## 运行客户端

要使用任何 MCP 服务器运行你的客户端：

```bash
uv run client.py path/to/server.py  # python 服务器
uv run client.py path/to/build/index.js  # node 服务器
```

> **注意：** 如果你继续 [服务器快速入门中的天气教程](https://github.com/modelcontextprotocol/quickstart-resources/tree/main/weather-server-python)，你的命令可能如下所示：
>
> ```bash
> python client.py .../quickstart-resources/weather-server-python/weather.py
> ```

客户端将：
1. 连接到指定的服务器
2. 列出可用工具
3. 启动一个交互式聊天会话，你可以：
   - 输入查询
   - 查看工具执行
   - 获取 Claude 的响应

---

## 工作原理

当你提交查询时：

1. 客户端从服务器获取可用工具列表
2. 你的查询与工具描述一起发送给 Claude
3. Claude 决定使用哪些工具（如果有）
4. 客户端通过服务器执行任何请求的工具调用
5. 结果发送回 Claude
6. Claude 提供自然语言响应
7. 响应显示给你

---

## 最佳实践

### 错误处理

- 始终将工具调用包装在 try-catch 块中
- 提供有意义的错误消息
- 优雅地处理连接问题

### 资源管理

- 使用 `AsyncExitStack` 进行适当的清理
- 完成后关闭连接
- 处理服务器断开连接

### 安全

- 将 API 密钥安全地存储在 `.env` 中
- 验证服务器响应
- 谨慎使用工具权限

### 工具名称

- 工具名称可以根据 [此处](https://modelcontextprotocol.io/specification/draft/server/tools#tool-names) 指定的格式进行验证
- 如果工具名称符合指定的格式，它应该不会被 MCP 客户端验证失败

---

## 常见问题排查

### 服务器路径问题

- 再次检查服务器脚本的路径是否正确
- 如果相对路径不起作用，请使用绝对路径
- 对于 Windows 用户，请确保在路径中使用正斜杠 (/) 或转义的反斜杠 (\\)
- 验证服务器文件具有正确的扩展名（.py 用于 Python，.js 用于 Node.js）

正确路径用法示例：

```bash
# 相对路径
uv run client.py ./server/weather.py

# 绝对路径
uv run client.py /Users/username/projects/mcp-server/weather.py

# Windows 路径（两种格式都可以）
uv run client.py C:/projects/mcp-server/weather.py
uv run client.py C:\\projects\\mcp-server\\weather.py
```

### 响应时间

- 第一个响应可能需要长达 30 秒才能返回
- 这是正常的，发生在：
  - 服务器初始化时
  - Claude 处理查询时
  - 工具正在执行时
- 后续响应通常更快
- 在此初始等待期间不要中断该过程

### 常见错误消息

如果你看到：

- `FileNotFoundError`：检查你的服务器路径
- `Connection refused`：确保服务器正在运行且路径正确
- `Tool execution failed`：验证工具所需的环境变量已设置
- `Timeout error`：考虑在你的客户端配置中增加超时时间

---

## 后续步骤

- [示例服务器](https://modelcontextprotocol.io/examples) - 查看我们的官方 MCP 服务器和实现库
- [示例客户端](https://modelcontextprotocol.io/clients) - 查看支持 MCP 集成的客户端列表

---

**版权所有 © Model Context Protocol a Series of LF Projects, LLC.**