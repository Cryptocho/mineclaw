//! 文件系统工具
//!
//! 提供安全的文件操作工具，包括路径检查、大小限制等安全机制。

use super::{LocalTool, ToolContext};
use crate::config::FilesystemConfig;
use crate::error::{Error, Result};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, warn};

// ==================== Path Security ====================

/// 规范化并验证路径
fn normalize_and_validate_path(path: &str, allowed_directories: &[String]) -> Result<PathBuf> {
    let path = Path::new(path);

    if path.components().any(|c| c.as_os_str() == "..") {
        return Err(Error::PathTraversal(path.to_string_lossy().to_string()));
    }

    let (full_path, check_dir) = if path.exists() {
        let canonical = std::fs::canonicalize(path)?;
        let check = if canonical.is_dir() {
            canonical.clone()
        } else {
            canonical.parent().unwrap_or(&canonical).to_path_buf()
        };
        (canonical, check)
    } else {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let parent_canonical = if parent.as_os_str().is_empty() {
            std::env::current_dir()?
        } else {
            std::fs::canonicalize(parent)?
        };
        let full = parent_canonical.join(path.file_name().unwrap_or_default());
        (full, parent_canonical)
    };

    if !allowed_directories.is_empty() {
        let allowed = allowed_directories.iter().any(|allowed| {
            if let Ok(canonical_allowed) = std::fs::canonicalize(Path::new(allowed)) {
                check_dir.starts_with(canonical_allowed)
            } else {
                false
            }
        });

        if !allowed {
            return Err(Error::PathNotAllowed(
                full_path.to_string_lossy().to_string(),
            ));
        }
    }

    Ok(full_path)
}

/// 获取文件系统配置
fn get_filesystem_config(context: &ToolContext) -> FilesystemConfig {
    context.config.filesystem.clone()
}

// ==================== Checkpoint 辅助函数 ====================

/// 在文件操作前自动创建 checkpoint
async fn maybe_create_checkpoint(
    context: &ToolContext,
    affected_files: Vec<String>,
    description: Option<String>,
) -> Result<Option<String>> {
    // 检查是否有 checkpoint manager
    let Some(checkpoint_manager) = &context.checkpoint_manager else {
        debug!("Checkpoint manager not available, skipping checkpoint creation");
        return Ok(None);
    };

    // 检查 checkpoint 是否启用
    if !checkpoint_manager.config().enabled {
        debug!("Checkpoint is disabled in config");
        return Ok(None);
    }

    // 只有在有受影响文件时才创建 checkpoint
    if affected_files.is_empty() {
        debug!("No affected files, skipping checkpoint");
        return Ok(None);
    }

    debug!(
        "Creating automatic checkpoint for files: {:?}",
        affected_files
    );

    // 创建 checkpoint
    let checkpoint = checkpoint_manager
        .create_checkpoint(
            context.session.id,
            description,
            Some(affected_files),
            context.agent_id,
        )
        .await?;

    Ok(Some(checkpoint.id))
}

/// 在单个文件中搜索模式
fn search_in_file(
    file_path: &Path,
    display_path: &str,
    pattern: &regex::Regex,
) -> Option<FileSearchResult> {
    match std::fs::read_to_string(file_path) {
        Ok(content) => {
            let mut matches = Vec::new();
            for (line_number, line) in content.lines().enumerate() {
                if pattern.is_match(line) {
                    matches.push(SearchMatch {
                        file_path: display_path.to_string(),
                        line_number: line_number + 1,
                        content: line.to_string(),
                    });
                }
            }

            if !matches.is_empty() {
                Some(FileSearchResult {
                    file_path: display_path.to_string(),
                    matches,
                })
            } else {
                None
            }
        }
        Err(e) => {
            warn!("Failed to read file {}: {}", file_path.display(), e);
            None
        }
    }
}

// ==================== Tool Parameters and Results ====================

#[derive(Debug, Deserialize)]
pub struct ReadFileParams {
    pub path: String,
    #[serde(default)]
    pub start_line: Option<usize>,
    #[serde(default)]
    pub end_line: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct ReadFileResult {
    pub content: String,
    pub truncated: bool,
    pub total_bytes: usize,
    pub total_lines: usize,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct WriteFileParams {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct WriteFileResult {
    pub success: bool,
    pub bytes_written: usize,
}

#[derive(Debug, Deserialize)]
pub struct ListDirectoryParams {
    pub path: String,
    #[serde(default)]
    pub recursive: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

#[derive(Debug, Serialize)]
pub struct ListDirectoryResult {
    pub entries: Vec<DirectoryEntry>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteFileParams {
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct DeleteFileResult {
    pub success: bool,
}

#[derive(Debug, Deserialize)]
pub struct SearchFileParams {
    pub path: String,
    pub pattern: String,
    #[serde(default)]
    pub case_sensitive: Option<bool>,
    #[serde(default)]
    pub recursive: Option<bool>,
    #[serde(default)]
    pub file_pattern: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SearchMatch {
    pub file_path: String,
    pub line_number: usize,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct FileSearchResult {
    pub file_path: String,
    pub matches: Vec<SearchMatch>,
}

#[derive(Debug, Serialize)]
pub struct SearchFileResult {
    pub results: Vec<FileSearchResult>,
}

#[derive(Debug, Deserialize)]
pub struct MoveFileParams {
    pub source: String,
    pub destination: String,
}

#[derive(Debug, Serialize)]
pub struct MoveFileResult {
    pub success: bool,
}

#[derive(Debug, Deserialize)]
pub struct MoveDirectoryParams {
    pub source: String,
    pub destination: String,
}

#[derive(Debug, Serialize)]
pub struct MoveDirectoryResult {
    pub success: bool,
}

#[derive(Debug, Deserialize)]
pub struct DeleteDirectoryParams {
    pub path: String,
    #[serde(default)]
    pub recursive: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct DeleteDirectoryResult {
    pub success: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateDirectoryParams {
    pub path: String,
    #[serde(default)]
    pub parents: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct CreateDirectoryResult {
    pub success: bool,
}

#[derive(Debug, Deserialize)]
pub struct SearchAndReplaceParams {
    pub path: String,
    pub diff: String,
    #[serde(default)]
    pub global: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct SearchAndReplaceResult {
    pub success: bool,
    pub replacements: usize,
}

#[derive(Debug, Deserialize)]
pub struct ReplaceAllKeywordsParams {
    pub path: String,
    pub search: String,
    pub replace: String,
    #[serde(default)]
    pub case_sensitive: Option<bool>,
    #[serde(default)]
    pub use_regex: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ReplaceAllKeywordsResult {
    pub success: bool,
    pub replacements: usize,
}

// ==================== Individual Tools ====================

struct ReadFileTool;

#[async_trait]
impl LocalTool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the complete contents of a file"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read"
                },
                "start_line": {
                    "type": "integer",
                    "description": "Starting line number (1-based, inclusive). If not specified, reads from the beginning.",
                    "minimum": 1
                },
                "end_line": {
                    "type": "integer",
                    "description": "Ending line number (1-based, inclusive). If not specified, reads to the end.",
                    "minimum": 1
                }
            },
            "required": ["path"]
        })
    }

    async fn call(&self, arguments: Value, context: ToolContext) -> Result<Value> {
        let params: ReadFileParams = serde_json::from_value(arguments)?;
        let config = get_filesystem_config(&context);

        let path = normalize_and_validate_path(&params.path, &config.allowed_directories)?;

        let metadata = std::fs::metadata(&path)?;
        let total_bytes = metadata.len() as usize;

        // Read all content first
        let full_content = std::fs::read_to_string(&path)?;
        let all_lines: Vec<&str> = full_content.lines().collect();
        let total_lines = all_lines.len();

        // Determine line range
        let start_line = params.start_line.unwrap_or(1);
        let end_line = params.end_line.unwrap_or(total_lines);

        // Validate and adjust line numbers
        let start_idx = if start_line > total_lines {
            total_lines
        } else if start_line < 1 {
            1
        } else {
            start_line
        } - 1; // Convert to 0-based index

        let end_idx = if end_line > total_lines {
            total_lines
        } else if end_line < 1 {
            1
        } else {
            end_line
        }; // Exclusive in slice, so no -1 needed

        let selected_lines = &all_lines[start_idx..end_idx];
        let content = selected_lines.join("\n");

        // Check if the selected content exceeds max_read_bytes
        let truncated = content.len() > config.max_read_bytes;
        let final_content = if truncated {
            warn!(
                path = %path.display(),
                content_size = content.len(),
                max_size = config.max_read_bytes,
                "Selected content too large, truncating"
            );
            content.chars().take(config.max_read_bytes).collect()
        } else {
            content
        };

        let result = ReadFileResult {
            content: final_content,
            truncated,
            total_bytes,
            total_lines,
            start_line: params.start_line,
            end_line: params.end_line,
        };

        Ok(serde_json::to_value(result)?)
    }
}

struct WriteFileTool;

#[async_trait]
impl LocalTool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Create a new file or completely overwrite an existing file"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                }
            },
            "required": ["path", "content"]
        })
    }

    async fn call(&self, arguments: Value, context: ToolContext) -> Result<Value> {
        let params: WriteFileParams = serde_json::from_value(arguments)?;
        let config = get_filesystem_config(&context);

        let path = normalize_and_validate_path(&params.path, &config.allowed_directories)?;

        // 为该文件操作创建自动 checkpoint
        let _ = maybe_create_checkpoint(
            &context,
            vec![path.to_string_lossy().to_string()],
            Some(format!(
                "Auto checkpoint before writing to: {}",
                params.path
            )),
        )
        .await;

        if let Some(parent) = path.parent()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&path, &params.content)?;

        let result = WriteFileResult {
            success: true,
            bytes_written: params.content.len(),
        };

        Ok(serde_json::to_value(result)?)
    }
}

struct ListDirectoryTool;

#[async_trait]
impl LocalTool for ListDirectoryTool {
    fn name(&self) -> &str {
        "list_directory"
    }

    fn description(&self) -> &str {
        "List the contents of a directory"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the directory to list"
                },
                "recursive": {
                    "type": "boolean",
                    "description": "Whether to list recursively",
                    "default": false
                }
            },
            "required": ["path"]
        })
    }

    async fn call(&self, arguments: Value, context: ToolContext) -> Result<Value> {
        let params: ListDirectoryParams = serde_json::from_value(arguments)?;
        let config = get_filesystem_config(&context);

        let abs_dir_path = normalize_and_validate_path(&params.path, &config.allowed_directories)?;

        let mut entries = Vec::new();

        if params.recursive.unwrap_or(false) {
            for entry in walkdir::WalkDir::new(&abs_dir_path) {
                let entry = entry?;
                if entry.path() == abs_dir_path {
                    continue;
                }
                let file_type = entry.file_type();

                let abs_path = entry.path();
                // 计算相对于 abs_dir_path 的路径
                let rel_to_abs_dir = abs_path.strip_prefix(&abs_dir_path).unwrap_or(abs_path);
                // 拼接用户原始路径
                let final_path = Path::new(&params.path).join(rel_to_abs_dir);

                entries.push(DirectoryEntry {
                    name: entry.file_name().to_string_lossy().to_string(),
                    path: final_path.to_string_lossy().to_string(),
                    is_dir: file_type.is_dir(),
                });
            }
        } else {
            for entry in std::fs::read_dir(&abs_dir_path)? {
                let entry = entry?;
                let file_type = entry.file_type()?;
                let file_name = entry.file_name();

                // 拼接用户原始路径
                let final_path = Path::new(&params.path).join(&file_name);

                entries.push(DirectoryEntry {
                    name: file_name.to_string_lossy().to_string(),
                    path: final_path.to_string_lossy().to_string(),
                    is_dir: file_type.is_dir(),
                });
            }
        }

        let result = ListDirectoryResult { entries };

        Ok(serde_json::to_value(result)?)
    }
}

struct DeleteFileTool;

#[async_trait]
impl LocalTool for DeleteFileTool {
    fn name(&self) -> &str {
        "delete_file"
    }

    fn description(&self) -> &str {
        "Delete a file"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to delete"
                }
            },
            "required": ["path"]
        })
    }

    async fn call(&self, arguments: Value, context: ToolContext) -> Result<Value> {
        let params: DeleteFileParams = serde_json::from_value(arguments)?;
        let config = get_filesystem_config(&context);

        let path = normalize_and_validate_path(&params.path, &config.allowed_directories)?;

        // 在修改前创建 checkpoint
        let _ = maybe_create_checkpoint(
            &context,
            vec![params.path.clone()],
            Some(format!("Before deleting file: {}", params.path)),
        )
        .await;

        std::fs::remove_file(&path)?;

        let result = DeleteFileResult { success: true };

        Ok(serde_json::to_value(result)?)
    }
}

struct SearchFileTool;

#[async_trait]
impl LocalTool for SearchFileTool {
    fn name(&self) -> &str {
        "search_file"
    }

    fn description(&self) -> &str {
        "Search for a pattern in a file"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file or directory to search"
                },
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for"
                },
                "case_sensitive": {
                    "type": "boolean",
                    "description": "Whether the search is case-sensitive",
                    "default": true
                },
                "recursive": {
                    "type": "boolean",
                    "description": "Whether to search recursively in directories",
                    "default": false
                },
                "file_pattern": {
                    "type": "string",
                    "description": "Optional glob pattern to filter files (e.g., '*.rs', '*.txt')"
                }
            },
            "required": ["path", "pattern"]
        })
    }

    async fn call(&self, arguments: Value, context: ToolContext) -> Result<Value> {
        let params: SearchFileParams = serde_json::from_value(arguments)?;
        let config = get_filesystem_config(&context);

        let path = normalize_and_validate_path(&params.path, &config.allowed_directories)?;

        let pattern = if params.case_sensitive.unwrap_or(true) {
            regex::Regex::new(&params.pattern)?
        } else {
            regex::Regex::new(&format!("(?i){}", params.pattern))?
        };

        let mut file_results = Vec::new();

        // Check if path is a file or directory
        if path.is_file() {
            // Search single file
            if let Some(file_matches) = search_in_file(&path, &params.path, &pattern) {
                file_results.push(file_matches);
            }
        } else if path.is_dir() {
            // Search directory
            let recursive = params.recursive.unwrap_or(false);

            // Create file pattern matcher if provided
            let file_pattern_matcher = params
                .file_pattern
                .as_ref()
                .and_then(|p| glob::Pattern::new(p).ok());

            let walkdir_iter = if recursive {
                walkdir::WalkDir::new(&path)
            } else {
                walkdir::WalkDir::new(&path).max_depth(1)
            };

            for entry in walkdir_iter {
                let entry = entry?;
                let entry_path = entry.path();

                if entry_path.is_file() {
                    // Check file pattern if provided
                    if let Some(matcher) = &file_pattern_matcher
                        && let Some(file_name) = entry_path.file_name()
                        && !matcher.matches(&file_name.to_string_lossy())
                    {
                        continue;
                    }

                    // Calculate relative path for display
                    let rel_path = if entry_path == path {
                        params.path.clone()
                    } else {
                        let rel_to_base = entry_path.strip_prefix(&path).unwrap_or(entry_path);
                        Path::new(&params.path)
                            .join(rel_to_base)
                            .to_string_lossy()
                            .to_string()
                    };

                    if let Some(file_matches) = search_in_file(entry_path, &rel_path, &pattern) {
                        file_results.push(file_matches);
                    }
                }
            }
        }

        let result = SearchFileResult {
            results: file_results,
        };

        Ok(serde_json::to_value(result)?)
    }
}

struct MoveFileTool;

#[async_trait]
impl LocalTool for MoveFileTool {
    fn name(&self) -> &str {
        "move_file"
    }

    fn description(&self) -> &str {
        "Move or rename a file"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "source": {
                    "type": "string",
                    "description": "Source file path"
                },
                "destination": {
                    "type": "string",
                    "description": "Destination file path"
                }
            },
            "required": ["source", "destination"]
        })
    }

    async fn call(&self, arguments: Value, context: ToolContext) -> Result<Value> {
        let params: MoveFileParams = serde_json::from_value(arguments)?;
        let config = get_filesystem_config(&context);

        let source = normalize_and_validate_path(&params.source, &config.allowed_directories)?;
        let destination =
            normalize_and_validate_path(&params.destination, &config.allowed_directories)?;

        // 在修改前创建 checkpoint
        let _ = maybe_create_checkpoint(
            &context,
            vec![params.source.clone(), params.destination.clone()],
            Some(format!(
                "Before moving file: {} -> {}",
                params.source, params.destination
            )),
        )
        .await;

        if let Some(parent) = destination.parent()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::rename(&source, &destination)?;

        let result = MoveFileResult { success: true };

        Ok(serde_json::to_value(result)?)
    }
}

struct MoveDirectoryTool;

#[async_trait]
impl LocalTool for MoveDirectoryTool {
    fn name(&self) -> &str {
        "move_directory"
    }

    fn description(&self) -> &str {
        "Move or rename a directory"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "source": {
                    "type": "string",
                    "description": "Source directory path"
                },
                "destination": {
                    "type": "string",
                    "description": "Destination directory path"
                }
            },
            "required": ["source", "destination"]
        })
    }

    async fn call(&self, arguments: Value, context: ToolContext) -> Result<Value> {
        let params: MoveDirectoryParams = serde_json::from_value(arguments)?;
        let config = get_filesystem_config(&context);

        let source = normalize_and_validate_path(&params.source, &config.allowed_directories)?;
        let destination =
            normalize_and_validate_path(&params.destination, &config.allowed_directories)?;

        // 在修改前创建 checkpoint
        let _ = maybe_create_checkpoint(
            &context,
            vec![params.source.clone(), params.destination.clone()],
            Some(format!(
                "Before moving directory: {} -> {}",
                params.source, params.destination
            )),
        )
        .await;

        if let Some(parent) = destination.parent()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::rename(&source, &destination)?;

        let result = MoveDirectoryResult { success: true };

        Ok(serde_json::to_value(result)?)
    }
}

struct DeleteDirectoryTool;

#[async_trait]
impl LocalTool for DeleteDirectoryTool {
    fn name(&self) -> &str {
        "delete_directory"
    }

    fn description(&self) -> &str {
        "Delete a directory"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the directory to delete"
                },
                "recursive": {
                    "type": "boolean",
                    "description": "Whether to delete recursively",
                    "default": false
                }
            },
            "required": ["path"]
        })
    }

    async fn call(&self, arguments: Value, context: ToolContext) -> Result<Value> {
        let params: DeleteDirectoryParams = serde_json::from_value(arguments)?;
        let config = get_filesystem_config(&context);

        let path = normalize_and_validate_path(&params.path, &config.allowed_directories)?;

        // 在修改前创建 checkpoint
        let _ = maybe_create_checkpoint(
            &context,
            vec![params.path.clone()],
            Some(format!("Before deleting directory: {}", params.path)),
        )
        .await;

        if params.recursive.unwrap_or(false) {
            std::fs::remove_dir_all(&path)?;
        } else {
            std::fs::remove_dir(&path)?;
        }

        let result = DeleteDirectoryResult { success: true };

        Ok(serde_json::to_value(result)?)
    }
}

struct CreateDirectoryTool;

#[async_trait]
impl LocalTool for CreateDirectoryTool {
    fn name(&self) -> &str {
        "create_directory"
    }

    fn description(&self) -> &str {
        "Create a directory"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the directory to create"
                },
                "parents": {
                    "type": "boolean",
                    "description": "Whether to create parent directories",
                    "default": false
                }
            },
            "required": ["path"]
        })
    }

    async fn call(&self, arguments: Value, context: ToolContext) -> Result<Value> {
        let params: CreateDirectoryParams = serde_json::from_value(arguments)?;
        let config = get_filesystem_config(&context);

        let path = normalize_and_validate_path(&params.path, &config.allowed_directories)?;

        if params.parents.unwrap_or(false) {
            std::fs::create_dir_all(&path)?;
        } else {
            std::fs::create_dir(&path)?;
        }

        let result = CreateDirectoryResult { success: true };

        Ok(serde_json::to_value(result)?)
    }
}

struct SearchAndReplaceTool;

/// 从 diff 文本中提取 SEARCH/REPLACE 块
pub fn parse_search_replace_blocks_from_diff(diff: &str) -> Vec<(String, String)> {
    let mut blocks = Vec::new();
    let mut lines = diff.lines().peekable();

    while let Some(line) = lines.next() {
        if line.trim() == "------- SEARCH" {
            let mut search_content = Vec::new();
            let mut replace_content = Vec::new();

            // 读取 SEARCH 部分直到 =======
            for line in lines.by_ref() {
                if line.trim() == "=======" {
                    break;
                }
                search_content.push(line);
            }

            // 读取 REPLACE 部分直到 +++++++ REPLACE
            for line in lines.by_ref() {
                if line.trim() == "+++++++ REPLACE" {
                    break;
                }
                replace_content.push(line);
            }

            // 移除开头和末尾的空行
            while search_content.first().is_some_and(|l| l.trim().is_empty()) {
                search_content.remove(0);
            }
            while search_content.last().is_some_and(|l| l.trim().is_empty()) {
                search_content.pop();
            }
            while replace_content.first().is_some_and(|l| l.trim().is_empty()) {
                replace_content.remove(0);
            }
            while replace_content.last().is_some_and(|l| l.trim().is_empty()) {
                replace_content.pop();
            }

            let search_str = search_content.join("\n");
            let replace_str = replace_content.join("\n");

            if !search_str.is_empty() {
                blocks.push((search_str, replace_str));
            }
        }
    }

    blocks
}

#[async_trait]
impl LocalTool for SearchAndReplaceTool {
    fn name(&self) -> &str {
        "search_and_replace"
    }

    fn description(&self) -> &str {
        "Search and replace text in a file using SEARCH/REPLACE block format only."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file"
                },
                "diff": {
                    "type": "string",
                    "description": "SEARCH/REPLACE block(s) with format: ------- SEARCH\\nold content\\n=======\\nnew content\\n+++++++ REPLACE"
                },
                "global": {
                    "type": "boolean",
                    "description": "Whether to replace all occurrences within each block",
                    "default": true
                }
            },
            "required": ["path", "diff"]
        })
    }

    async fn call(&self, arguments: Value, context: ToolContext) -> Result<Value> {
        let params: SearchAndReplaceParams = serde_json::from_value(arguments)?;
        let config = get_filesystem_config(&context);

        let path = normalize_and_validate_path(&params.path, &config.allowed_directories)?;

        // 在修改前创建 checkpoint
        let _ = maybe_create_checkpoint(
            &context,
            vec![params.path.clone()],
            Some(format!(
                "Before search and replace in file: {}",
                params.path
            )),
        )
        .await;

        // 检查是否是有效的 SEARCH/REPLACE 块格式
        let search_has_blocks = params.diff.contains("------- SEARCH")
            && params.diff.contains("=======")
            && params.diff.contains("+++++++ REPLACE");

        if !search_has_blocks {
            return Err(Error::LocalToolExecution {
                tool: "search_and_replace".to_string(),
                message: "search_and_replace requires SEARCH/REPLACE block format. Use: ------- SEARCH\\nold\\n=======\\nnew\\n+++++++ REPLACE".to_string()
            });
        }

        let mut content = std::fs::read_to_string(&path)?;
        let mut total_replacements = 0;

        let blocks = parse_search_replace_blocks_from_diff(&params.diff);
        let global = params.global.unwrap_or(true);

        for (search_str, replace_str) in blocks {
            if search_str.is_empty() {
                continue;
            }

            if global {
                let count = content.matches(&search_str).count();
                content = content.replace(&search_str, &replace_str);
                total_replacements += count;
            } else if let Some(index) = content.find(&search_str) {
                content.replace_range(index..index + search_str.len(), &replace_str);
                total_replacements += 1;
            }
        }

        std::fs::write(&path, &content)?;

        let result = SearchAndReplaceResult {
            success: true,
            replacements: total_replacements,
        };

        Ok(serde_json::to_value(result)?)
    }
}

struct ReplaceAllKeywordsTool;

#[async_trait]
impl LocalTool for ReplaceAllKeywordsTool {
    fn name(&self) -> &str {
        "replace_all_keywords"
    }

    fn description(&self) -> &str {
        "Find and replace all occurrences of a keyword in a file"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file"
                },
                "search": {
                    "type": "string",
                    "description": "Keyword or regex pattern to search for"
                },
                "replace": {
                    "type": "string",
                    "description": "Replacement text"
                },
                "case_sensitive": {
                    "type": "boolean",
                    "description": "Whether the search is case-sensitive",
                    "default": true
                },
                "use_regex": {
                    "type": "boolean",
                    "description": "Whether to use regex for matching",
                    "default": false
                }
            },
            "required": ["path", "search", "replace"]
        })
    }

    async fn call(&self, arguments: Value, context: ToolContext) -> Result<Value> {
        let params: ReplaceAllKeywordsParams = serde_json::from_value(arguments)?;
        let config = get_filesystem_config(&context);

        let path = normalize_and_validate_path(&params.path, &config.allowed_directories)?;

        // 在修改前创建 checkpoint
        let _ = maybe_create_checkpoint(
            &context,
            vec![params.path.clone()],
            Some(format!(
                "Before replacing all keywords in file: {}",
                params.path
            )),
        )
        .await;
        let mut content = std::fs::read_to_string(&path)?;
        let case_sensitive = params.case_sensitive.unwrap_or(true);
        let use_regex = params.use_regex.unwrap_or(false);

        let replacements = if use_regex {
            let pattern = if case_sensitive {
                regex::Regex::new(&params.search)?
            } else {
                regex::Regex::new(&format!("(?i){}", params.search))?
            };
            let count = pattern.find_iter(&content).count();
            content = pattern.replace_all(&content, &params.replace).to_string();
            count
        } else if case_sensitive {
            let count = content.matches(&params.search).count();
            content = content.replace(&params.search, &params.replace);
            count
        } else {
            let mut result = String::new();
            let mut last_end = 0;
            let search_lower = params.search.to_lowercase();
            let content_lower = content.to_lowercase();
            let mut count = 0;

            while let Some(start) = content_lower[last_end..].find(&search_lower) {
                let real_start = last_end + start;
                let real_end = real_start + params.search.len();
                result.push_str(&content[last_end..real_start]);
                result.push_str(&params.replace);
                last_end = real_end;
                count += 1;
            }
            result.push_str(&content[last_end..]);
            content = result;
            count
        };

        std::fs::write(&path, &content)?;

        let result = ReplaceAllKeywordsResult {
            success: true,
            replacements,
        };

        Ok(serde_json::to_value(result)?)
    }
}

// ==================== FilesystemTool ====================

/// 文件系统工具集合
pub struct FilesystemTool;

impl FilesystemTool {
    /// 注册所有文件系统工具到注册表
    pub fn register_all(registry: &mut super::registry::LocalToolRegistry) {
        registry.register(Arc::new(ReadFileTool));
        registry.register(Arc::new(WriteFileTool));
        registry.register(Arc::new(ListDirectoryTool));
        registry.register(Arc::new(DeleteFileTool));
        registry.register(Arc::new(SearchFileTool));
        registry.register(Arc::new(MoveFileTool));
        registry.register(Arc::new(MoveDirectoryTool));
        registry.register(Arc::new(DeleteDirectoryTool));
        registry.register(Arc::new(CreateDirectoryTool));
        registry.register(Arc::new(SearchAndReplaceTool));
        registry.register(Arc::new(ReplaceAllKeywordsTool));
    }
}
