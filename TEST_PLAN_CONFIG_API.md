# MineClaw 配置管理 API 与热更新测试方案

本方案用于人工验证 `GET /api/config` 和 `PUT /api/config` 接口的功能，以及配置修改后的热更新效果（包括文件持久化、LLM Provider 切换和 MCP Server 重启）。

## 1. 测试环境准备

1.  **启动 MineClaw 服务器**：
    在终端运行 `cargo run` 启动服务器。默认监听地址为 `127.0.0.1:18789`。

2.  **准备测试工具**：
    -   建议使用 `curl` 或 `Postman` 发送 HTTP 请求。
    -   也可以使用提供的 Python 测试脚本 `test_config_api.py`。

## 2. 测试用例

### 用例 1：获取当前配置 (GET)

**目标**：验证能否正确读取当前生效的配置。

**步骤**：
1.  发送 `GET http://127.0.0.1:18789/api/config` 请求。

**预期结果**：
-   HTTP 状态码 200 OK。
-   响应体为 JSON 格式，包含 `server`、`llm`、`mcp` 等字段，且值与 `config/mineclaw.toml` 或默认值一致。

### 用例 2：修改 LLM 配置并验证持久化 (PUT)

**目标**：验证修改配置能否生效并保存到文件。

**步骤**：
1.  构造一个新的配置 JSON，修改 `llm.temperature` 为 `0.5`（原默认为 `0.7`）。
2.  发送 `PUT http://127.0.0.1:18789/api/config` 请求，Body 为修改后的 JSON。
3.  查看 `config/mineclaw.toml` 文件内容。

**预期结果**：
-   HTTP 状态码 200 OK，返回更新后的配置 JSON。
-   `config/mineclaw.toml` 文件中的 `temperature` 字段变为 `0.5`。

### 用例 3：LLM Provider 热更新验证

**目标**：验证修改 LLM 配置后，系统是否立即使用新参数。

**步骤**：
1.  将 `llm.model` 修改为一个显眼的假模型名，例如 `gpt-4o-test-hot-reload`。
2.  发送 `PUT /api/config` 请求。
3.  发送一条聊天消息 `POST /api/messages`。
4.  观察服务器日志（stdout）。

**预期结果**：
-   服务器日志中 LLM 请求的 URL 或参数应包含新的模型名（需查看 debug 日志或抓包确认，或者如果 Provider 校验模型名则会报错，报错即证明新配置生效）。

### 用例 4：MCP Server 热更新验证

**目标**：验证修改 MCP 配置后，MCP 服务器是否重启。

**步骤**：
1.  构造配置，启用 MCP (`mcp.enabled = true`) 并添加一个简单的 `echo` 服务器（使用系统命令，如 `cmd /c echo` 或 `python` 脚本）。
    -   *注：Windows 下可以使用 `cmd` 作为 command，`["/c", "echo", "hello"]` 作为 args 测试启动，但它不会响应 MCP 协议，会立即退出。建议配置一个真实的 MCP Server 如 `filesystem` 或 `mock`。*
2.  发送 `PUT /api/config` 请求。
3.  观察服务器日志。

**预期结果**：
-   日志显示 `Stopping all MCP servers`。
-   日志显示 `Restarting X MCP servers`。
-   日志显示 `Starting MCP server: ...`。

### 用例 5：非法配置处理

**目标**：验证系统对非法配置的健壮性。

**步骤**：
1.  发送 `PUT /api/config`，Body 为非法 JSON 或缺少必填字段。

**预期结果**：
-   HTTP 状态码 400 Bad Request 或 422 Unprocessable Entity。
-   服务器未崩溃。

## 3. 自动化测试脚本

可以使用以下 Python 脚本快速执行上述部分验证：

```python
import requests
import json

BASE_URL = "http://127.0.0.1:18789"

def test_get_config():
    print("\n=== Testing GET /api/config ===")
    resp = requests.get(f"{BASE_URL}/api/config")
    print(f"Status: {resp.status_code}")
    print(f"Response: {json.dumps(resp.json(), indent=2)}")
    return resp.json()

def test_update_config(current_config):
    print("\n=== Testing PUT /api/config ===")
    
    # 修改 temperature
    new_config = current_config.copy()
    new_config['llm']['temperature'] = 0.99
    
    resp = requests.put(f"{BASE_URL}/api/config", json=new_config)
    print(f"Status: {resp.status_code}")
    
    if resp.status_code == 200:
        print("Update successful")
        updated_config = resp.json()
        print(f"New Temperature: {updated_config['llm']['temperature']}")
        assert updated_config['llm']['temperature'] == 0.99
    else:
        print(f"Update failed: {resp.text}")

if __name__ == "__main__":
    try:
        config = test_get_config()
        test_update_config(config)
    except Exception as e:
        print(f"Test failed: {e}")
```
