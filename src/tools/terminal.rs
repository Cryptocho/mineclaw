use crate::error::{Error, Result};
use crate::tools::ToolContext;
use crate::tools::shell_detection::{self, ShellType};
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::Duration;

// ==================== 终端工具参数和结果类型 ====================

/// 运行命令参数
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RunCommandParams {
    /// 命令
    pub command: String,
    /// 任务 ID (用于续航 Phase EX1)
    pub task_id: Option<String>,
    /// 是否在后台运行 (Phase EX3)
    #[serde(default)]
    pub detach: bool,
    /// 工作目录（可选）
    pub cwd: Option<String>,
}

/// 运行命令结果
#[derive(Debug, Serialize, Deserialize)]
pub struct RunCommandResult {
    /// 退出码
    pub exit_code: i32,
    /// 标准输出
    pub stdout: String,
    /// 标准错误输出
    pub stderr: String,
    /// 是否被截断
    pub truncated: bool,
    /// 是否超时 (Phase EX1: 实时快照)
    pub is_timeout: bool,
    /// 任务唯一标识符 (Phase EX1: 长时任务管理)
    pub task_id: String,
}

/// 活跃进程信息 (Phase EX1: 长时任务管理)
pub struct ActiveProcess {
    pub child: tokio::process::Child,
    pub stdout_handle: Option<tokio::process::ChildStdout>,
    pub stderr_handle: Option<tokio::process::ChildStderr>,
    pub stdout_buf: Arc<Mutex<Vec<u8>>>,
    pub stderr_buf: Arc<Mutex<Vec<u8>>>,
    pub params: RunCommandParams,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

struct ProcessGuard<'a>(&'a AtomicUsize);
impl<'a> Drop for ProcessGuard<'a> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

// ==================== 终端工具实现 ====================

struct RunCommandTool {
    /// 当前正在运行的进程数 (Phase 3: 并发控制)
    running_processes: std::sync::Arc<AtomicUsize>,
    /// 活跃进程注册表 (Phase EX1: 长时任务管理)
    processes: std::sync::Arc<Mutex<HashMap<String, ActiveProcess>>>,
}

impl RunCommandTool {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            running_processes: std::sync::Arc::new(AtomicUsize::new(0)),
            processes: std::sync::Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_shared_state(
        running_processes: std::sync::Arc<AtomicUsize>,
        processes: std::sync::Arc<Mutex<HashMap<String, ActiveProcess>>>,
    ) -> Self {
        Self {
            running_processes,
            processes,
        }
    }

    /// 检查命令是否在黑名单中，并返回拦截原因
    fn is_command_blacklisted(&self, command: &str, context: &ToolContext) -> Option<String> {
        // 1. 硬编码的黑名单 (基础风险词汇)
        let hardcoded_blacklist = [
            ("rm -rf /", "Destructive command: rm -rf / is forbidden."),
            ("mkfs", "Destructive command: mkfs is forbidden."),
            ("dd if=", "Dangerous command: dd can overwrite disks."),
            (
                ":(){ :|:& };:",
                "Security Policy: Fork bomb pattern detected.",
            ),
            (
                "fdisk",
                "Security Policy: Disk partitioning tools are forbidden.",
            ),
            (
                "parted",
                "Security Policy: Disk partitioning tools are forbidden.",
            ),
            (
                "shutdown",
                "Security Policy: System power commands are forbidden.",
            ),
            (
                "reboot",
                "Security Policy: System power commands are forbidden.",
            ),
            (
                "halt",
                "Security Policy: System power commands are forbidden.",
            ),
            (
                "poweroff",
                "Security Policy: System power commands are forbidden.",
            ),
            (
                "crontab -r",
                "Security Policy: Deleting crontab is forbidden.",
            ),
        ];

        // 检查硬编码黑名单 (简单字符串匹配)
        for (pattern, reason) in hardcoded_blacklist {
            if command.contains(pattern) {
                return Some(reason.to_string());
            }
        }

        let parts: Vec<&str> = command.split_whitespace().collect();

        // Phase EX2: 拦截交互式分页器和编辑器 (支持管道流识别)
        let interactive_pagers = ["less", "more", "man"];
        let interactive_editors = ["vi", "vim", "nano"];

        for part in &parts {
            let cmd_name = part.to_lowercase();
            if interactive_pagers.contains(&cmd_name.as_str()) {
                if command.contains('|') {
                    return Some(format!(
                        "MineClaw Hint: Pipeline contains interactive pager '{}'. In this environment, you should avoid piping to pagers. Just run the preceding command directly to see all output, or pipe to 'head', 'tail', or 'grep' for filtering.",
                        cmd_name
                    ));
                }
                return Some(format!(
                    "MineClaw Hint: '{}' is an interactive pager and is not supported in this environment. Please use non-interactive alternatives like 'cat', 'head -n 20', 'tail -n 20', or 'grep'.",
                    cmd_name
                ));
            }
            if interactive_editors.contains(&cmd_name.as_str()) {
                return Some(format!(
                    "MineClaw Hint: '{}' is an interactive text editor and is not supported. Please use the 'write_file' or 'search_and_replace' tools provided by MineClaw instead.",
                    cmd_name
                ));
            }
        }

        // 针对 rm / -rf 等变体的增强型硬编码处理 (Zed 风格)
        if command.contains("rm") {
            let has_rm = parts.contains(&"rm");
            let dangerous_targets = ["/", ".", "..", "~"];
            let has_dangerous_target = parts.iter().any(|&p| dangerous_targets.contains(&p));
            let has_force = parts
                .iter()
                .any(|&p| (p.starts_with('-') && p.contains('f')) || p == "--force");
            let has_recursive = parts
                .iter()
                .any(|&p| (p.starts_with('-') && p.contains('r')) || p == "--recursive");

            if has_rm && has_dangerous_target && (has_force || has_recursive) {
                return Some("Security Policy: Destructive rm command targetting sensitive paths is forbidden.".to_string());
            }
        }

        // 2. 检查配置中的普通黑名单 (简单字符串匹配)
        if context
            .config
            .local_tools
            .terminal
            .command_blacklist
            .iter()
            .any(|pattern| command.contains(pattern))
        {
            return Some(
                "Security Policy: Command contains forbidden pattern from configuration."
                    .to_string(),
            );
        }

        // 3. 正则表达式黑名单 (由配置定义)
        for regex in &context.config.local_tools.terminal.compiled_blacklist {
            if regex.is_match(command) {
                return Some(format!(
                    "Security Policy: Command matches forbidden pattern '{}'.",
                    regex
                ));
            }
        }

        None
    }

    fn is_command_always_allowed(&self, command: &str, context: &ToolContext) -> bool {
        for re in &context.config.local_tools.terminal.compiled_always_allow {
            if re.is_match(command) {
                return true;
            }
        }
        false
    }

    fn tokenize_commands(&self, full_command: &str) -> Vec<String> {
        let mut commands = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;
        let mut quote_char = ' ';

        let chars: Vec<char> = full_command.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if (c == '"' || c == '\'') && (i == 0 || chars[i - 1] != '\\') {
                if in_quotes {
                    if c == quote_char {
                        in_quotes = false;
                    }
                } else {
                    in_quotes = true;
                    quote_char = c;
                }
                current.push(c);
            } else if !in_quotes {
                let mut found_separator = false;
                let separators = [";", "&&", "||", "|"];
                for sep in separators {
                    if full_command[i..].starts_with(sep) {
                        if !current.trim().is_empty() {
                            commands.push(current.trim().to_string());
                        }
                        current = String::new();
                        i += sep.len();
                        found_separator = true;
                        break;
                    }
                }
                if found_separator {
                    continue;
                }
                current.push(c);
            } else {
                current.push(c);
            }
            i += 1;
        }
        if !current.trim().is_empty() {
            commands.push(current.trim().to_string());
        }
        commands
    }

    fn normalize_path_safe(&self, path: &Path) -> Option<std::path::PathBuf> {
        use std::path::Component;
        let mut components = Vec::new();
        for component in path.components() {
            match component {
                Component::Prefix(p) => {
                    components.clear();
                    components.push(Component::Prefix(p));
                }
                Component::RootDir => {
                    if !components.iter().any(|c| matches!(c, Component::Prefix(_))) {
                        components.clear();
                    }
                    components.push(Component::RootDir);
                }
                Component::CurDir => {}
                Component::ParentDir => {
                    if components.is_empty() {
                        return None;
                    }
                    match components.last() {
                        Some(Component::RootDir) | Some(Component::Prefix(_)) => return None,
                        _ => {
                            components.pop();
                        }
                    }
                }
                Component::Normal(c) => components.push(Component::Normal(c)),
            }
        }
        if components.is_empty() {
            return Some(std::path::PathBuf::from("."));
        }
        let mut result = std::path::PathBuf::new();
        for c in components {
            result.push(c);
        }
        Some(result)
    }

    fn is_working_dir_allowed(&self, cwd: &str, context: &ToolContext) -> bool {
        let path = Path::new(cwd);
        if self.normalize_path_safe(path).is_none() {
            return false;
        }
        if context
            .config
            .local_tools
            .terminal
            .allowed_workspaces
            .is_empty()
        {
            return true;
        }
        let absolute_cwd = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => return false,
        };
        let normalize_unc = |p: std::path::PathBuf| -> String {
            let s = p.to_string_lossy().to_string();
            if let Some(stripped) = s.strip_prefix(r"\\?\") {
                stripped.to_string()
            } else {
                s
            }
        };
        let clean_cwd = normalize_unc(absolute_cwd);
        context
            .config
            .local_tools
            .terminal
            .allowed_workspaces
            .iter()
            .any(|allowed_dir| {
                let clean_allowed = if let Ok(allowed_path) = Path::new(allowed_dir).canonicalize()
                {
                    normalize_unc(allowed_path)
                } else {
                    allowed_dir.to_string()
                };
                // Windows 下路径不区分大小写，Unix 下区分
                if shell_detection::system_shell() == ShellType::Windows {
                    clean_cwd
                        .to_lowercase()
                        .starts_with(&clean_allowed.to_lowercase())
                } else {
                    clean_cwd.starts_with(&clean_allowed)
                }
            })
    }

    fn apply_output_filters(&self, command: &str, output: &str, context: &ToolContext) -> String {
        if let Some(filters) = context.config.local_tools.terminal.filters.get(command) {
            let lines: Vec<&str> = output.lines().collect();
            let filtered_lines: Vec<&str> = lines
                .into_iter()
                .filter(|line| filters.iter().any(|filter| line.contains(filter)))
                .collect();
            return filtered_lines.join("\n");
        }
        output.to_string()
    }

    fn truncate_output(&self, output: &str, max_bytes: usize, from_tail: bool) -> (String, bool) {
        if output.len() <= max_bytes {
            return (output.to_string(), false);
        }

        if from_tail {
            let start_index = output.len() - max_bytes;
            let mut start = start_index;
            while start < output.len() && !output.is_char_boundary(start) {
                start += 1;
            }
            let truncated_part = &output[start..];
            if let Some(first_newline) = truncated_part.find('\n')
                && first_newline < (max_bytes / 2)
            {
                return (truncated_part[first_newline + 1..].to_string(), true);
            }
            (truncated_part.to_string(), true)
        } else {
            let mut end = max_bytes;
            while end > 0 && !output.is_char_boundary(end) {
                end -= 1;
            }
            let truncated_part = &output[..end];
            if let Some(last_newline) = truncated_part.rfind('\n')
                && last_newline > (max_bytes * 7 / 10)
            {
                let mut result_len = last_newline;
                if result_len > 0 && truncated_part.as_bytes()[result_len - 1] == b'\r' {
                    result_len -= 1;
                }
                return (truncated_part[..result_len].to_string(), true);
            }
            (truncated_part.to_string(), true)
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_and_handle_output(
        &self,
        task_id: String,
        mut child: tokio::process::Child,
        mut stdout_handle: tokio::process::ChildStdout,
        mut stderr_handle: tokio::process::ChildStderr,
        stdout_buf_shared: Arc<Mutex<Vec<u8>>>,
        stderr_buf_shared: Arc<Mutex<Vec<u8>>>,
        params: RunCommandParams,
        start_time: chrono::DateTime<chrono::Utc>,
        context: &ToolContext,
        _guard: ProcessGuard<'_>,
    ) -> Result<Value> {
        let timeout_duration =
            Duration::from_secs(context.config.local_tools.terminal.timeout_seconds);
        let timeout_sleep = tokio::time::sleep(timeout_duration);
        tokio::pin!(timeout_sleep);

        let mut stdout_done = false;
        let mut stderr_done = false;
        let mut is_timeout = false;

        let exit_status = loop {
            let mut out_chunk = [0u8; 4096];
            let mut err_chunk = [0u8; 4096];

            tokio::select! {
                status = child.wait() => {
                    break Some(status.map_err(|e| crate::error::Error::LocalToolExecution {
                        tool: "run_command".to_string(),
                        message: e.to_string(),
                    })?);
                }
                res = stdout_handle.read(&mut out_chunk), if !stdout_done => {
                    match res {
                        Ok(0) | Err(_) => stdout_done = true,
                        Ok(n) => {
                            let mut buf = stdout_buf_shared.lock().unwrap();
                            buf.extend_from_slice(&out_chunk[..n]);
                        }
                    }
                }
                res = stderr_handle.read(&mut err_chunk), if !stderr_done => {
                    match res {
                        Ok(0) | Err(_) => stderr_done = true,
                        Ok(n) => {
                            let mut buf = stderr_buf_shared.lock().unwrap();
                            buf.extend_from_slice(&err_chunk[..n]);
                        }
                    }
                }
                _ = &mut timeout_sleep => {
                    is_timeout = true;
                    break None;
                }
            }
        };

        // 读取剩余输出 (如果未超时)
        if !is_timeout {
            let mut stdout_buf = Vec::new();
            let mut stderr_buf = Vec::new();
            let _ = stdout_handle.read_to_end(&mut stdout_buf).await;
            let _ = stderr_handle.read_to_end(&mut stderr_buf).await;

            stdout_buf_shared
                .lock()
                .unwrap()
                .extend_from_slice(&stdout_buf);
            stderr_buf_shared
                .lock()
                .unwrap()
                .extend_from_slice(&stderr_buf);
        }

        let stdout = String::from_utf8_lossy(&stdout_buf_shared.lock().unwrap()).to_string();
        let stderr = String::from_utf8_lossy(&stderr_buf_shared.lock().unwrap()).to_string();

        let exit_code = if let Some(status) = exit_status {
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                status
                    .code()
                    .or_else(|| status.signal().map(|s| -s))
                    .unwrap_or(-1)
            }
            #[cfg(not(unix))]
            {
                status.code().unwrap_or(-1)
            }
        } else {
            -1
        };

        let max_output_bytes = context.config.local_tools.terminal.max_output_bytes;
        let from_tail = exit_code != 0;

        let (processed_stdout, stdout_truncated) =
            self.truncate_output(&stdout, max_output_bytes, from_tail);
        let (processed_stderr, stderr_truncated) =
            self.truncate_output(&stderr, max_output_bytes, from_tail);
        let truncated = stdout_truncated || stderr_truncated;

        // 如果超时，则将进程存入注册表以便续航
        if is_timeout {
            let mut procs = self
                .processes
                .lock()
                .expect("Failed to lock processes registry");
            procs.insert(
                task_id.clone(),
                ActiveProcess {
                    child,
                    stdout_handle: Some(stdout_handle),
                    stderr_handle: Some(stderr_handle),
                    stdout_buf: stdout_buf_shared,
                    stderr_buf: stderr_buf_shared,
                    params: params.clone(),
                    start_time,
                    last_activity: Utc::now(),
                },
            );
        }

        let filtered_stdout =
            self.apply_output_filters(&params.command, &processed_stdout, context);

        let res = RunCommandResult {
            exit_code,
            stdout: filtered_stdout,
            stderr: processed_stderr,
            truncated,
            is_timeout,
            task_id,
        };

        Ok(serde_json::to_value(res)?)
    }
}

struct ListBackgroundTasksTool {
    processes: std::sync::Arc<Mutex<HashMap<String, ActiveProcess>>>,
}

impl ListBackgroundTasksTool {
    pub fn new(processes: std::sync::Arc<Mutex<HashMap<String, ActiveProcess>>>) -> Self {
        Self { processes }
    }
}

#[async_trait]
impl crate::tools::LocalTool for ListBackgroundTasksTool {
    fn name(&self) -> &str {
        "list_background_tasks"
    }

    fn description(&self) -> &str {
        "List all background terminal tasks, including their PID, start time, and output summary."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn call(&self, _arguments: Value, _context: ToolContext) -> Result<Value> {
        let procs = self
            .processes
            .lock()
            .expect("Failed to lock processes registry");
        let mut tasks = Vec::new();

        for (task_id, ap) in procs.iter() {
            let pid = ap.child.id();
            let duration = Utc::now().signed_duration_since(ap.start_time);

            let stdout_buf = ap.stdout_buf.lock().unwrap();
            let stderr_buf = ap.stderr_buf.lock().unwrap();
            let stdout_summary = String::from_utf8_lossy(&stdout_buf);
            let stderr_summary = String::from_utf8_lossy(&stderr_buf);

            let stdout_tail = if stdout_summary.len() > 200 {
                format!("...{}", &stdout_summary[stdout_summary.len() - 200..])
            } else {
                stdout_summary.to_string()
            };

            let stderr_tail = if stderr_summary.len() > 200 {
                format!("...{}", &stderr_summary[stderr_summary.len() - 200..])
            } else {
                stderr_summary.to_string()
            };

            tasks.push(json!({
                "task_id": task_id,
                "pid": pid,
                "command": ap.params.command,
                "start_time": ap.start_time.to_rfc3339(),
                "uptime_seconds": duration.num_seconds(),
                "stdout_summary": stdout_tail,
                "stderr_summary": stderr_tail,
            }));
        }

        Ok(json!({ "background_tasks": tasks }))
    }
}

struct GetTaskResultTool {
    processes: std::sync::Arc<Mutex<HashMap<String, ActiveProcess>>>,
}

impl GetTaskResultTool {
    pub fn new(processes: std::sync::Arc<Mutex<HashMap<String, ActiveProcess>>>) -> Self {
        Self { processes }
    }
}

#[async_trait]
impl crate::tools::LocalTool for GetTaskResultTool {
    fn name(&self) -> &str {
        "get_task_result"
    }

    fn description(&self) -> &str {
        "Retrieve the current output of a background task or terminate it. If the task is completed, it will be removed from the background registry."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The unique task ID of the background process."
                },
                "kill": {
                    "type": "boolean",
                    "description": "Whether to forcefully terminate the task and remove it from the registry."
                }
            },
            "required": ["task_id"]
        })
    }

    async fn call(&self, arguments: Value, _context: ToolContext) -> Result<Value> {
        let task_id = arguments["task_id"]
            .as_str()
            .ok_or_else(|| Error::LocalToolExecution {
                tool: "get_task_result".to_string(),
                message: "Missing task_id".to_string(),
            })?;

        let kill = arguments["kill"].as_bool().unwrap_or(false);

        let maybe_ap = {
            let mut procs = self
                .processes
                .lock()
                .expect("Failed to lock processes registry");
            procs.remove(task_id)
        };

        if let Some(mut ap) = maybe_ap {
            if kill {
                let _ = ap.child.kill().await;
                return Ok(json!({
                    "task_id": task_id,
                    "status": "terminated",
                    "stdout": String::from_utf8_lossy(&ap.stdout_buf.lock().unwrap()),
                    "stderr": String::from_utf8_lossy(&ap.stderr_buf.lock().unwrap()),
                }));
            }

            // Check if it's already finished
            match ap.child.try_wait() {
                Ok(Some(status)) => {
                    // Finished, return full output and don't put it back
                    Ok(json!({
                        "task_id": task_id,
                        "status": "completed",
                        "exit_code": status.code().unwrap_or(-1),
                        "stdout": String::from_utf8_lossy(&ap.stdout_buf.lock().unwrap()),
                        "stderr": String::from_utf8_lossy(&ap.stderr_buf.lock().unwrap()),
                    }))
                }
                _ => {
                    // Still running, return snapshot and put it back
                    ap.last_activity = Utc::now();
                    let res = json!({
                        "task_id": task_id,
                        "status": "running",
                        "stdout": String::from_utf8_lossy(&ap.stdout_buf.lock().unwrap()),
                        "stderr": String::from_utf8_lossy(&ap.stderr_buf.lock().unwrap()),
                    });

                    let mut procs = self
                        .processes
                        .lock()
                        .expect("Failed to lock processes registry");
                    procs.insert(task_id.to_string(), ap);
                    Ok(res)
                }
            }
        } else {
            Err(Error::LocalToolExecution {
                tool: "get_task_result".to_string(),
                message: format!("Task ID {} not found.", task_id),
            })
        }
    }
}

#[async_trait]
impl crate::tools::LocalTool for RunCommandTool {
    fn name(&self) -> &str {
        "run_command"
    }

    fn description(&self) -> &str {
        "Execute a terminal command and return the output. Restricted to safe commands and specific workspaces."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command to run (e.g., 'ls', 'git status')."
                },
                "cwd": {
                    "type": "string",
                    "description": "The working directory to run the command in."
                },

                "task_id": {
                    "type": "string",
                    "description": "The unique task ID for continuing long-running tasks."
                },
                "detach": {
                    "type": "boolean",
                    "description": "Whether to run the command in the background and return immediately."
                }
            },
            "required": ["command"]
        })
    }

    async fn call(&self, arguments: Value, context: ToolContext) -> Result<Value> {
        let params: RunCommandParams = serde_json::from_value(arguments)?;

        // 验证基本参数 (Phase EX2: 鲁棒性增强)
        // 如果没有提供 task_id，则必须提供非空的 command
        if params.task_id.is_none() && params.command.trim().is_empty() {
            return Err(crate::error::Error::LocalToolExecution {
                tool: "run_command".to_string(),
                message: "Command cannot be empty.".to_string(),
            });
        }

        // 尝试找回现有进程 (Phase EX1: 续航逻辑)
        if let Some(tid) = &params.task_id {
            if tid.trim().is_empty() {
                return Err(crate::error::Error::LocalToolExecution {
                    tool: "run_command".to_string(),
                    message: "Task ID cannot be empty.".to_string(),
                });
            }
            // 首先尝试获取并发许可
            let max_concurrent = context.config.local_tools.terminal.max_concurrent_processes;
            loop {
                let current = self.running_processes.load(Ordering::SeqCst);
                if current >= max_concurrent {
                    return Err(crate::error::Error::LocalToolExecution {
                        tool: "run_command".to_string(),
                        message: format!(
                            "Security Policy: Concurrency limit reached ({} running). Limit is {}.",
                            current, max_concurrent
                        ),
                    });
                }
                if self
                    .running_processes
                    .compare_exchange(current, current + 1, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    break;
                }
                std::hint::spin_loop();
            }
            let _guard = ProcessGuard(self.running_processes.as_ref());

            let maybe_ap = {
                let mut procs = self
                    .processes
                    .lock()
                    .expect("Failed to lock processes registry");
                procs.remove(tid)
            };

            if let Some(mut ap) = maybe_ap {
                let stdout_handle = ap.stdout_handle.take().ok_or_else(|| Error::LocalToolExecution {
                    tool: "run_command".to_string(),
                    message: "Cannot continue a detached task that is already being read by a background worker.".to_string(),
                })?;
                let stderr_handle = ap.stderr_handle.take().ok_or_else(|| Error::LocalToolExecution {
                    tool: "run_command".to_string(),
                    message: "Cannot continue a detached task that is already being read by a background worker.".to_string(),
                })?;

                return self
                    .run_and_handle_output(
                        tid.clone(),
                        ap.child,
                        stdout_handle,
                        stderr_handle,
                        ap.stdout_buf,
                        ap.stderr_buf,
                        ap.params,
                        ap.start_time,
                        &context,
                        _guard,
                    )
                    .await;
            } else {
                return Err(crate::error::Error::LocalToolExecution {
                    tool: "run_command".to_string(),
                    message: format!("Task ID {} not found or already completed.", tid),
                });
            }
        }

        let task_id = uuid::Uuid::new_v4().to_string();
        let full_command = params.command.clone();

        if let Some(reason) = self.is_command_blacklisted(&full_command, &context) {
            return Err(crate::error::Error::LocalToolExecution {
                tool: "run_command".to_string(),
                message: reason,
            });
        }

        let sub_commands = self.tokenize_commands(&full_command);
        for cmd in &sub_commands {
            if self.is_command_always_allowed(cmd, &context) {
                continue;
            }
            if let Some(reason) = self.is_command_blacklisted(cmd, &context) {
                return Err(crate::error::Error::LocalToolExecution {
                    tool: "run_command".to_string(),
                    message: reason,
                });
            }
        }

        let max_concurrent = context.config.local_tools.terminal.max_concurrent_processes;
        loop {
            let current = self.running_processes.load(Ordering::SeqCst);
            if current >= max_concurrent {
                return Err(crate::error::Error::LocalToolExecution {
                    tool: "run_command".to_string(),
                    message: format!(
                        "Security Policy: Concurrency limit reached ({} running). Limit is {}.",
                        current, max_concurrent
                    ),
                });
            }
            if self
                .running_processes
                .compare_exchange(current, current + 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                break;
            }
            std::hint::spin_loop();
        }

        let _guard = ProcessGuard(self.running_processes.as_ref());
        let start_time = Utc::now();

        if let Some(cwd) = &params.cwd
            && !self.is_working_dir_allowed(cwd, &context)
        {
            return Err(crate::error::Error::LocalToolExecution {
                tool: "run_command".to_string(),
                message: format!("Working directory not allowed: {}", cwd),
            });
        }

        let shell_type = shell_detection::system_shell();
        let full_command = params.command.clone();

        let mut command = match shell_type {
            ShellType::Windows => {
                let mut cmd = Command::new("powershell");
                cmd.arg("-NonInteractive");
                cmd.arg("-NoProfile");
                cmd.arg("-Command");

                // 在 PowerShell 中执行完整命令字符串
                cmd.arg(full_command);
                cmd
            }
            ShellType::Unix => {
                // 优先使用 SHELL 环境变量指定的 shell，回退到 sh
                let shell = std::env::var("SHELL")
                    .ok()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "sh".to_string());

                let mut cmd = Command::new(&shell);
                cmd.arg("-c");
                cmd.arg(full_command);
                cmd
            }
        };

        if let Some(cwd) = &params.cwd {
            command.current_dir(cwd);
        }

        // Phase EX2: 环境抑制注入 - 强制非交互模式
        command.env("PAGER", "cat");
        command.env("MANPAGER", "cat");
        command.env("GIT_PAGER", "cat");
        command.env("TERM", "dumb"); // 告知工具这是一个非交互式终端

        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .map_err(|e| crate::error::Error::LocalToolExecution {
                tool: "run_command".to_string(),
                message: format!("Failed to spawn process: {}", e),
            })?;

        let stdout_handle = child.stdout.take().unwrap();
        let stderr_handle = child.stderr.take().unwrap();

        // Phase EX3: 任务分离 (Detach) 机制
        let stdout_buf_shared = Arc::new(Mutex::new(Vec::new()));
        let stderr_buf_shared = Arc::new(Mutex::new(Vec::new()));

        if params.detach {
            // 启动后台读取任务
            let mut stdout_bg = stdout_handle;
            let mut stderr_bg = stderr_handle;
            let stdout_buf_bg = stdout_buf_shared.clone();
            let stderr_buf_bg = stderr_buf_shared.clone();

            tokio::spawn(async move {
                let mut out_buf = [0u8; 4096];
                let mut err_buf = [0u8; 4096];
                loop {
                    tokio::select! {
                        res = stdout_bg.read(&mut out_buf) => {
                            match res {
                                Ok(0) | Err(_) => break,
                                Ok(n) => stdout_buf_bg.lock().unwrap().extend_from_slice(&out_buf[..n]),
                            }
                        }
                        res = stderr_bg.read(&mut err_buf) => {
                            match res {
                                Ok(0) | Err(_) => break,
                                Ok(n) => stderr_buf_bg.lock().unwrap().extend_from_slice(&err_buf[..n]),
                            }
                        }
                    }
                }
            });

            let mut procs = self
                .processes
                .lock()
                .expect("Failed to lock processes registry");
            procs.insert(
                task_id.clone(),
                ActiveProcess {
                    child,
                    stdout_handle: None,
                    stderr_handle: None,
                    stdout_buf: stdout_buf_shared,
                    stderr_buf: stderr_buf_shared,
                    params: params.clone(),
                    start_time,
                    last_activity: start_time,
                },
            );

            let res = RunCommandResult {
                exit_code: -1, // Still running
                stdout: String::new(),
                stderr: String::new(),
                truncated: false,
                is_timeout: false,
                task_id,
            };
            return Ok(serde_json::to_value(res)?);
        }

        self.run_and_handle_output(
            task_id,
            child,
            stdout_handle,
            stderr_handle,
            stdout_buf_shared,
            stderr_buf_shared,
            params,
            start_time,
            &context,
            _guard,
        )
        .await
    }
}

pub struct TerminalTool;

impl Default for TerminalTool {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalTool {
    pub fn new() -> Self {
        Self
    }

    pub fn register_all(registry: &mut crate::tools::registry::LocalToolRegistry) {
        let running_processes = Arc::new(AtomicUsize::new(0));
        let processes = Arc::new(Mutex::new(HashMap::<String, ActiveProcess>::new()));

        // Phase EX3: 启动后台 GC 工作协程 (定期清理不活跃的后台任务)
        let processes_gc = processes.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 每 5 分钟检查一次
            loop {
                interval.tick().await;
                let now = Utc::now();
                let mut to_remove = Vec::new();

                {
                    let procs = processes_gc
                        .lock()
                        .expect("Failed to lock processes for GC");
                    for (id, ap) in procs.iter() {
                        let age = now.signed_duration_since(ap.last_activity);
                        // 默认 TTL 为 30 分钟。对于长时间无交互的任务进行自动回收。
                        if age.num_minutes() >= 30 {
                            to_remove.push(id.clone());
                        }
                    }
                }

                for id in to_remove {
                    let maybe_ap = {
                        let mut procs = processes_gc
                            .lock()
                            .expect("Failed to lock processes for GC removal");
                        procs.remove(&id)
                    };

                    if let Some(mut ap) = maybe_ap {
                        let _ = ap.child.kill().await;
                        tracing::info!(task_id = %id, "GC: Terminated inactive background task after TTL expiration.");
                    }
                }
            }
        });

        registry.register(Arc::new(RunCommandTool::with_shared_state(
            running_processes,
            processes.clone(),
        )));
        registry.register(Arc::new(ListBackgroundTasksTool::new(processes.clone())));
        registry.register(Arc::new(GetTaskResultTool::new(processes)));
    }
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::models::Session;
    use crate::tools::LocalTool;
    use std::sync::Arc;

    // 辅助函数：创建测试上下文
    fn create_test_context() -> (Arc<Config>, Session, ToolContext) {
        let config = Arc::new(Config::default());
        let session = Session::new();
        let context = ToolContext::new(session.clone(), config.clone());
        (config, session, context)
    }

    // 辅助函数：判断是否是 Windows
    fn is_windows() -> bool {
        shell_detection::system_shell() == ShellType::Windows
    }

    // 辅助函数：创建简单的 echo 命令参数
    fn create_echo_params(message: &str) -> RunCommandParams {
        RunCommandParams {
            command: format!("echo {}", message),
            task_id: None,
            detach: false,
            cwd: None,
        }
    }

    // 辅助函数：创建 detach 的 sleep 命令参数
    fn create_detach_sleep_params(seconds: u32) -> RunCommandParams {
        RunCommandParams {
            command: format!("sleep {}", seconds),
            task_id: None,
            detach: true,
            cwd: None,
        }
    }

    #[test]
    fn test_truncate_output() {
        let tool = RunCommandTool::new();
        let input = "Hello World";
        let (output, truncated) = tool.truncate_output(input, 20, false);
        assert_eq!(output, input);
        assert!(!truncated);

        let input = "Line 1\nLine 2\nLine 3";
        let (output, truncated) = tool.truncate_output(input, 15, false);
        assert_eq!(output, "Line 1\nLine 2");
        assert!(truncated);

        let input = "Line 1\nLine 2\nLine 3";
        let (output, truncated) = tool.truncate_output(input, 12, true);
        assert_eq!(output, "Line 3");
        assert!(truncated);

        let input = "🦀🦀🦀🦀";
        let (output, truncated) = tool.truncate_output(input, 6, false);
        assert_eq!(output, "🦀");
        assert!(truncated);

        let input = "Line 1\r\nLine 2\r\nLine 3";
        let (output, truncated) = tool.truncate_output(input, 17, false);
        assert_eq!(output, "Line 1\r\nLine 2");
        assert!(truncated);
    }

    #[tokio::test]
    async fn test_run_command_failure() {
        let (_, _, context) = create_test_context();
        let tool = RunCommandTool::new();

        let params = RunCommandParams {
            command: "exit 1".to_string(),
            task_id: None,
            detach: false,
            cwd: None,
        };

        let result_value = tool
            .call(serde_json::to_value(params).unwrap(), context)
            .await
            .unwrap();
        let result: RunCommandResult = serde_json::from_value(result_value).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(!result.truncated);
    }

    #[tokio::test]
    async fn test_run_command_concurrency_limit() {
        let mut config = Config::default();
        config.local_tools.terminal.max_concurrent_processes = 1;
        let config = Arc::new(config);
        let tool = RunCommandTool::new();
        tool.running_processes.store(1, Ordering::SeqCst);
        let session = Session::new();
        let context = ToolContext::new(session, config);
        let params = RunCommandParams {
            command: "sleep 5".to_string(),
            task_id: None,
            detach: false,
            cwd: None,
        };
        let result = tool
            .call(serde_json::to_value(params).unwrap(), context)
            .await;
        assert!(result.is_err());
        tool.running_processes.store(0, Ordering::SeqCst);
    }

    #[tokio::test]
    async fn test_run_command_timeout_snapshot() {
        let mut config = Config::default();
        // 设置极短超时以触发快照
        config.local_tools.terminal.timeout_seconds = 1;
        let config = Arc::new(config);
        let session = Session::new();
        let context = ToolContext::new(session, config);
        let tool = RunCommandTool::new();

        let params = RunCommandParams {
            command: "echo start; sleep 5; echo end".to_string(),
            task_id: None,
            detach: false,
            cwd: None,
        };

        let result_value = tool
            .call(serde_json::to_value(&params).unwrap(), context)
            .await
            .unwrap();
        let result: RunCommandResult = serde_json::from_value(result_value).unwrap();

        assert!(result.is_timeout);
        assert!(result.stdout.contains("start"));
        assert!(!result.stdout.contains("end"));
    }

    #[tokio::test]
    async fn test_run_command_continue() {
        let mut config = Config::default();
        config.local_tools.terminal.timeout_seconds = 2;
        let config = Arc::new(config);
        let session = Session::new();
        let context = ToolContext::new(session, config.clone());
        let tool = RunCommandTool::new();

        let params = if is_windows() {
            RunCommandParams {
                command: "echo step1 & ping -n 4 127.0.0.1 > nul & echo step2".to_string(),
                task_id: None,
                detach: false,
                cwd: None,
            }
        } else {
            RunCommandParams {
                command: "echo step1; sleep 3; echo step2".to_string(),
                task_id: None,
                detach: false,
                cwd: None,
            }
        };

        // 第一次执行：触发超时并挂起
        let result_value = tool
            .call(serde_json::to_value(&params).unwrap(), context.clone())
            .await
            .unwrap();
        let result1: RunCommandResult = serde_json::from_value(result_value).unwrap();
        assert!(
            result1.is_timeout,
            "Expected timeout, but got exit_code: {}",
            result1.exit_code
        );
        assert!(
            result1.stdout.contains("step1"),
            "stdout doesn't contain 'step1': {:?}",
            result1.stdout
        );
        assert!(!result1.stdout.contains("step2"));
        let task_id = result1.task_id;

        // 第二次执行：使用 task_id 续命
        // 增加超时时间确保任务能跑完
        let mut longer_config = (*config).clone();
        longer_config.local_tools.terminal.timeout_seconds = 5;
        let longer_context = ToolContext::new(Session::new(), Arc::new(longer_config));

        let continue_params = RunCommandParams {
            command: String::new(),
            task_id: Some(task_id),
            detach: false,
            cwd: None,
        };

        let result_value2 = tool
            .call(
                serde_json::to_value(&continue_params).unwrap(),
                longer_context,
            )
            .await
            .unwrap();
        let result2: RunCommandResult = serde_json::from_value(result_value2).unwrap();

        assert!(!result2.is_timeout);
        assert_eq!(result2.exit_code, 0);
        // 续命后的输出应该包含之前没拿到的内容
        assert!(result2.stdout.contains("step2"));
    }

    #[tokio::test]
    async fn test_run_command_pager_protection() {
        let (_, _, context) = create_test_context();
        let tool = RunCommandTool::new();

        let params = RunCommandParams {
            command: "echo $PAGER".to_string(),
            task_id: None,
            detach: false,
            cwd: None,
        };

        let result_value = tool
            .call(serde_json::to_value(&params).unwrap(), context)
            .await
            .unwrap();
        let result: RunCommandResult = serde_json::from_value(result_value).unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("cat"));
    }

    #[tokio::test]
    async fn test_run_command_interactive_blocked() {
        let config = Arc::new(Config::default());
        let session = Session::new();
        let context = ToolContext::new(session, config);
        let tool = RunCommandTool::new();

        let params = RunCommandParams {
            command: "less test.txt".to_string(),
            task_id: None,
            detach: false,
            cwd: None,
        };

        let result = tool
            .call(serde_json::to_value(&params).unwrap(), context)
            .await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("MineClaw Hint")
                || err_msg.contains("forbidden")
                || err_msg.contains("blocked")
        );
    }

    #[tokio::test]
    async fn test_run_command_pipeline_pager_blocked() {
        let config = Arc::new(Config::default());
        let session = Session::new();
        let context = ToolContext::new(session, config);
        let tool = RunCommandTool::new();

        let params = RunCommandParams {
            command: "cat test.txt | less".to_string(),
            task_id: None,
            detach: false,
            cwd: None,
        };

        let result = tool
            .call(serde_json::to_value(&params).unwrap(), context)
            .await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Pipeline contains interactive pager"));
        assert!(err_msg.contains("less"));
    }

    #[tokio::test]
    async fn test_run_command_detach() {
        let (_, _, context) = create_test_context();
        let tool = RunCommandTool::new();
        let params = create_detach_sleep_params(5);

        let result_value = tool
            .call(serde_json::to_value(&params).unwrap(), context)
            .await
            .unwrap();
        let result: RunCommandResult = serde_json::from_value(result_value).unwrap();

        // 验证任务已立即返回 ID 且 exit_code 为 -1
        assert_eq!(result.exit_code, -1);
        assert!(!result.task_id.is_empty());

        // 验证任务已在注册表中
        {
            let procs = tool.processes.lock().unwrap();
            assert!(procs.contains_key(&result.task_id));
        }
    }

    #[tokio::test]
    async fn test_list_background_tasks() {
        let (_config, _session, context) = create_test_context();

        let running_processes = std::sync::Arc::new(AtomicUsize::new(0));
        let processes = std::sync::Arc::new(Mutex::new(HashMap::new()));

        let run_tool = RunCommandTool::with_shared_state(running_processes, processes.clone());
        let list_tool = ListBackgroundTasksTool::new(processes);

        let params = create_detach_sleep_params(5);

        // 启动后台任务
        let result_value = run_tool
            .call(serde_json::to_value(&params).unwrap(), context.clone())
            .await
            .unwrap();
        let result: RunCommandResult = serde_json::from_value(result_value).unwrap();
        let task_id = result.task_id;

        // 列出任务
        let list_result_value = list_tool
            .call(serde_json::to_value(json!({})).unwrap(), context)
            .await
            .unwrap();

        let tasks = list_result_value
            .get("background_tasks")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(
            tasks
                .iter()
                .any(|t| t.get("task_id").unwrap().as_str() == Some(&task_id))
        );
    }

    #[tokio::test]
    async fn test_get_task_result() {
        let (_, _, context) = create_test_context();

        let running_processes = std::sync::Arc::new(AtomicUsize::new(0));
        let processes = std::sync::Arc::new(Mutex::new(HashMap::new()));

        let run_tool = RunCommandTool::with_shared_state(running_processes, processes.clone());
        let get_tool = GetTaskResultTool::new(processes);

        let params = RunCommandParams {
            command: "echo hello_background".to_string(),
            task_id: None,
            detach: true,
            cwd: None,
        };

        // 启动后台任务
        let result_value = run_tool
            .call(serde_json::to_value(&params).unwrap(), context.clone())
            .await
            .unwrap();
        let result: RunCommandResult = serde_json::from_value(result_value).unwrap();
        let task_id = result.task_id;

        // 等待一小会儿确保进程至少运行并可能结束
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // 获取结果
        let get_result_value = get_tool
            .call(json!({ "task_id": task_id }), context.clone())
            .await
            .unwrap();

        assert!(
            get_result_value
                .get("stdout")
                .unwrap()
                .as_str()
                .unwrap()
                .contains("hello_background")
        );

        // 验证 kill 功能
        let params_sleep = create_detach_sleep_params(10);

        let sleep_result_value = run_tool
            .call(
                serde_json::to_value(&params_sleep).unwrap(),
                context.clone(),
            )
            .await
            .unwrap();
        let sleep_task_id = sleep_result_value.get("task_id").unwrap().as_str().unwrap();

        let kill_result = get_tool
            .call(json!({ "task_id": sleep_task_id, "kill": true }), context)
            .await
            .unwrap();

        assert_eq!(
            kill_result.get("status").unwrap().as_str().unwrap(),
            "terminated"
        );
    }

    #[tokio::test]
    async fn test_run_command() {
        let (_, _, context) = create_test_context();
        let tool = RunCommandTool::new();
        let params = create_echo_params("hello");

        let result_value = tool
            .call(serde_json::to_value(&params).unwrap(), context)
            .await
            .unwrap();
        let result: RunCommandResult = serde_json::from_value(result_value).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("hello"));
    }
}
