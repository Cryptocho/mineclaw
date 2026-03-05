//! MCP 客户端
//!
//! 实现 MCP 协议的客户端，负责与 MCP 服务器通信。

use crate::error::{Error, Result};
use crate::mcp::protocol::*;
use crate::mcp::transport::Transport;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use tokio::sync::oneshot;
use tokio::time::{timeout, Duration};
use tracing::{debug, error, info, warn};

// ==================== McpClient ====================

/// MCP 客户端，管理与 MCP 服务器的会话
pub struct McpClient {
    transport: Box<dyn Transport>,
    pending_requests: HashMap<RequestId, oneshot::Sender<JsonRpcResponse>>,
    next_id: u64,
}

impl McpClient {
    /// 创建一个新的 MCP 客户端
    pub fn new(transport: Box<dyn Transport>) -> Self {
        Self {
            transport,
            pending_requests: HashMap::new(),
            next_id: 1,
        }
    }

    /// 生成下一个请求 ID
    fn next_request_id(&mut self) -> RequestId {
        let id = self.next_id;
        self.next_id += 1;
        RequestId::Number(id)
    }

    /// 发送请求并等待响应（简化版，不使用 select!）
    async fn send_request<R: DeserializeOwned>(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<R> {
        let request_id = self.next_request_id();
        debug!(method, ?request_id, "Sending MCP request");

        let request = JsonRpcRequest::new(request_id.clone(), method.to_string(), params);
        let request_json = serde_json::to_string(&request).map_err(|e| {
            error!(error = %e, "Failed to serialize request");
            Error::Mcp(format!("Serialization failed: {}", e))
        })?;

        let (tx, rx) = oneshot::channel();
        self.pending_requests.insert(request_id.clone(), tx);

        self.transport.send(&request_json).await?;

        // 使用 tokio::time::timeout 包装接收循环
        let result = timeout(Duration::from_secs(30), async {
            loop {
                let message = self.transport.receive().await?;
                debug!(message, "Received message from server");

                // 尝试解析为响应
                if let Ok(response) = serde_json::from_str::<JsonRpcResponse>(&message) {
                    if response.id == request_id {
                        if let Some(sender) = self.pending_requests.remove(&response.id) {
                            debug!(?response.id, "Matched response to pending request");
                            let _ = sender.send(response);
                            break;
                        }
                    } else if let Some(sender) = self.pending_requests.remove(&response.id) {
                        warn!(?response.id, "Received response for different request ID");
                        let _ = sender.send(response);
                    } else {
                        warn!(?response.id, "Received response for unknown request ID");
                    }
                    continue;
                }

                // 尝试解析为通知
                if let Ok(notification) = serde_json::from_str::<JsonRpcNotification>(&message) {
                    self.handle_notification(notification).await;
                    continue;
                }

                warn!(message, "Received unrecognized message");
            }
            Ok::<(), Error>(())
        }).await;

        if let Err(_) = result {
             // 超时处理：清理 pending_requests
             self.pending_requests.remove(&request_id);
             return Err(Error::Mcp(format!("Request {} timed out", method)));
        }
        
        // 内部循环可能出错，也要处理
        if let Err(e) = result.unwrap() {
             return Err(e);
        }

        match rx.await {
            Ok(json_rpc_response) => {
                if let Some(error) = json_rpc_response.error {
                    error!(code = error.code, message = %error.message, "MCP request failed");
                    return Err(Error::Mcp(format!(
                        "MCP error: {} ({})",
                        error.message, error.code
                    )));
                }

                let result = json_rpc_response
                    .result
                    .ok_or_else(|| Error::Mcp("No result in response".to_string()))?;

                let deserialized = serde_json::from_value(result).map_err(|e| {
                    error!(error = %e, "Failed to deserialize response");
                    Error::Mcp(format!("Deserialization failed: {}", e))
                })?;

                Ok(deserialized)
            }
            Err(_) => {
                error!("Request channel closed");
                Err(Error::Mcp("Request cancelled".to_string()))
            }
        }
    }

    /// 处理服务器发送的通知
    async fn handle_notification(&mut self, notification: JsonRpcNotification) {
        debug!(method = %notification.method, "Handling notification");
        // 目前不处理任何通知，预留接口
    }

    /// 发送通知（不需要响应）
    async fn send_notification(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<()> {
        debug!(method, "Sending MCP notification");

        let notification = JsonRpcNotification::new(method.to_string(), params);
        let notification_json = serde_json::to_string(&notification).map_err(|e| {
            error!(error = %e, "Failed to serialize notification");
            Error::Mcp(format!("Serialization failed: {}", e))
        })?;

        self.transport.send(&notification_json).await
    }

    // ==================== 公共 MCP 方法 ====================

    /// 初始化 MCP 会话
    pub async fn initialize(&mut self) -> Result<InitializeResponse> {
        info!("Initializing MCP session");

        let request = InitializeRequest::new("mineclaw", "0.1.0");
        let params = serde_json::to_value(request)
            .map_err(|e| Error::Mcp(format!("Failed to serialize initialize request: {}", e)))?;

        let response: InitializeResponse = self.send_request("initialize", Some(params)).await?;

        info!(
            server_name = %response.server_info.name,
            server_version = %response.server_info.version,
            "MCP session initialized"
        );

        // 发送 initialized 通知
        self.send_notification("initialized", None).await?;

        Ok(response)
    }

    /// 获取工具列表
    pub async fn list_tools(&mut self) -> Result<ListToolsResponse> {
        debug!("Listing tools");

        // 注意：tools/list 请求可以不带参数，也可以带 { cursor: string }
        // 这里的 ListToolsRequest 可能会序列化出 { cursor: null }，导致某些服务器报错
        // 我们手动构建参数，如果 cursor 为 None 则不包含该字段
        let params = serde_json::json!({});
        // 或者如果 ListToolsRequest 实现了正确的 skip_serializing_if

        // 暂时简单处理：直接发送空对象作为参数，或者不传参数（视协议而定）
        // 根据 MCP 规范，params 应该是可选的。
        // 但如果之前的 ListToolsRequest::new() 生成了 { cursor: null }，且服务器校验严格（如 Node SDK），就会报错。
        
        let response: ListToolsResponse = self.send_request("tools/list", Some(params)).await?;

        debug!(tool_count = response.tools.len(), "Received tools list");

        Ok(response)
    }

    /// 调用工具
    pub async fn call_tool(
        &mut self,
        name: String,
        arguments: serde_json::Value,
    ) -> Result<CallToolResponse> {
        debug!(tool_name = %name, "Calling tool");

        let request = CallToolRequest { name, arguments };
        let params = serde_json::to_value(request)
            .map_err(|e| Error::Mcp(format!("Failed to serialize call_tool request: {}", e)))?;

        let response: CallToolResponse = self.send_request("tools/call", Some(params)).await?;

        debug!(is_error = response.is_error, "Tool call completed");

        Ok(response)
    }

    /// 关闭连接
    pub async fn close(&mut self) -> Result<()> {
        info!("Closing MCP client");
        self.transport.close().await
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        // 取消所有待处理的请求
        for (_, sender) in self.pending_requests.drain() {
            let _ = sender.send(JsonRpcResponse::error(
                RequestId::Number(0),
                -32603,
                "Client dropped".to_string(),
                None,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::transport::mock::MockTransport;
    use serde_json::json;

    #[tokio::test]
    async fn test_initialize() {
        let mut transport = MockTransport::new();

        // 设置初始化响应
        let initialize_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            }
        });
        transport
            .responses_to_receive
            .lock()
            .await
            .push_back(initialize_response.to_string());

        // initialized 通知会被发送，但不需要响应
        let mut client = McpClient::new(Box::new(transport));

        let result = client.initialize().await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.server_info.name, "test-server");
        assert_eq!(response.server_info.version, "1.0.0");
    }

    #[tokio::test]
    async fn test_list_tools() {
        let mut transport = MockTransport::new();

        // 设置初始化响应
        let initialize_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            }
        });
        transport
            .responses_to_receive
            .lock()
            .await
            .push_back(initialize_response.to_string());

        // 设置 list_tools 响应
        let list_tools_response = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": {
                "tools": [
                    {
                        "name": "echo",
                        "description": "Echo back the input",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "message": { "type": "string" }
                            },
                            "required": ["message"]
                        }
                    }
                ]
            }
        });
        transport
            .responses_to_receive
            .lock()
            .await
            .push_back(list_tools_response.to_string());

        let mut client = McpClient::new(Box::new(transport));

        // 先初始化
        client.initialize().await.unwrap();

        // 列出工具
        let result = client.list_tools().await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.tools.len(), 1);
        assert_eq!(response.tools[0].name, "echo");
    }

    #[tokio::test]
    async fn test_error_response() {
        let mut transport = MockTransport::new();

        // 设置错误响应
        let error_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32601,
                "message": "Method not found"
            }
        });
        transport
            .responses_to_receive
            .lock()
            .await
            .push_back(error_response.to_string());

        let mut client = McpClient::new(Box::new(transport));

        // 直接发送请求 - 因为我们修改了 send_request，需要用公共方法
        // 这里我们用 initialize 来测试错误路径
        let result = client.initialize().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Method not found"));
    }

    #[tokio::test]
    async fn test_call_tool() {
        let mut transport = MockTransport::new();

        // 设置初始化响应
        let initialize_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            }
        });
        transport
            .responses_to_receive
            .lock()
            .await
            .push_back(initialize_response.to_string());

        // 设置 call_tool 响应
        let call_tool_response = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": {
                "content": [
                    {
                        "type": "text",
                        "text": "Hello, test!"
                    }
                ],
                "isError": false
            }
        });
        transport
            .responses_to_receive
            .lock()
            .await
            .push_back(call_tool_response.to_string());

        let mut client = McpClient::new(Box::new(transport));

        // 先初始化
        client.initialize().await.unwrap();

        // 调用工具
        let result = client
            .call_tool("echo".to_string(), json!({"message": "test"}))
            .await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.content.len(), 1);
        assert!(!response.is_error);
    }

    #[tokio::test]
    async fn test_call_tool_error() {
        let mut transport = MockTransport::new();

        // 设置初始化响应
        let initialize_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            }
        });
        transport
            .responses_to_receive
            .lock()
            .await
            .push_back(initialize_response.to_string());

        // 设置错误响应
        let error_response = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "error": {
                "code": -32602,
                "message": "Invalid params"
            }
        });
        transport
            .responses_to_receive
            .lock()
            .await
            .push_back(error_response.to_string());

        let mut client = McpClient::new(Box::new(transport));

        // 先初始化
        client.initialize().await.unwrap();

        // 调用工具，应该返回错误
        let result = client.call_tool("echo".to_string(), json!({})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid params"));
    }
}
