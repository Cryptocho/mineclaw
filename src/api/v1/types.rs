use serde::{Deserialize, Serialize};

/// 统一响应封装 (Standard Response Wrapper)
/// 按照 MINECLAW_API_CONTRACT.md 0.1 章节定义
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiErrorDetails>,
}

/// 错误详情 (Error Details)
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiErrorDetails {
    pub code: String,
    pub message: String,
}

/// 分页对象 (Pagination Object)
/// 按照 MINECLAW_API_CONTRACT.md 0.2 章节定义
#[derive(Debug, Serialize, Deserialize)]
pub struct Pagination<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    pub has_more: bool,
}

impl<T> ApiResponse<T> {
    /// 创建成功的响应
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    /// 创建失败的响应
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(ApiErrorDetails {
                code: code.into(),
                message: message.into(),
            }),
        }
    }
}

/// 通用列表请求参数
#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

impl ListParams {
    pub fn page(&self) -> usize {
        self.page.unwrap_or(1)
    }

    pub fn page_size(&self) -> usize {
        self.page_size.unwrap_or(20)
    }
}
