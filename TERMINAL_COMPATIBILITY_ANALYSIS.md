# 终端兼容性与跨平台 Shell 支持分析报告

## 1. 现状分析
当前 MineClaw 使用 Node.js 的 `child_process.exec` 执行命令。
- **Windows**: 默认使用 `cmd.exe` (`%ComSpec%`)。
- **macOS/Linux**: 默认使用 `/bin/sh`。

这导致了以下核心问题：
1.  **语法不兼容**: AI 经常默认生成 Bash 脚本（如 `export`, `&&`, `ls -la`），这些在 `cmd.exe` 或老版本 PowerShell 中无法直接运行。
2.  **环境差异**:
    - **路径分隔符**: Windows `\` vs Unix `/`。
    - **变量设置**: Windows `set` / `$env:` vs Unix `export`。
    - **管道与重定向**: PowerShell 处理对象流，Bash 处理文本流，且 PS 的编码处理（UTF-16LE BOM）有时会破坏工具链。

## 2. Windows 终端生态的复杂性
在 Windows 上，开发者可能使用三种完全不同的 Shell 环境，语法各不相同：

| 特性 | CMD (Legacy) | PowerShell (Modern) | Git Bash (MinGW/MSYS2) |
| :--- | :--- | :--- | :--- |
| **定位** | 遗留系统兼容 | Windows 原生，功能强大 | 模拟 Linux 环境 |
| **变量** | `%VAR%` | `$env:VAR`, `$VAR` | `$VAR` |
| **设置变量** | `set VAR=val` | `$env:VAR = 'val'` | `export VAR=val` |
| **连接符** | `&&` | `;` (PS 7+ 支持 `&&`) | `&&` |
| **常用命令** | `dir`, `type` | `ls` (alias), `cat` (alias) | `ls`, `cat` |
| **AI 友好度** | 低 (语法晦涩) | 中 (语法冗长但逻辑强) | 高 (通用 Bash 语法) |

**Git Bash 的特殊挑战**:
- 它不是原生 Windows 控制台程序，通常通过 `bash.exe` 运行。
- **路径转换**: Windows 路径 `C:\Windows` 在 Git Bash 中是 `/c/Windows`，虽然它通常能兼容 Windows 路径，但在某些脚本中会出问题。
- **交互性**: `mintty` 终端模拟器有时不接受标准输入注入。

## 3. 解决方案：显式 Shell 环境感知

为了让 AI 生成正确的命令，我们必须**显式地**告诉它当前运行在什么环境，并允许用户配置首选 Shell。

### 3.1 方案 A: 自动探测 + System Prompt 注入 (推荐)
在 Session 初始化时，探测宿主机的 OS 和 Shell，并将这些信息注入到 System Prompt 中。

**Prompt 示例**:
> You are running on **Windows 11**.
> The current shell is **PowerShell**.
> When generating commands, please use **PowerShell 5.1** syntax.
> Do NOT use Bash syntax like `export` or `source`.

### 3.2 方案 B: MCP Server 配置化 Shell
修改 `terminal-mcp-server.js`，允许通过参数指定使用的 Shell executable。

**配置示例 (`mineclaw.toml`)**:
```toml
[[mcp.servers]]
name = "terminal-server"
command = "node"
args = [
    "terminal-mcp-server.js",
    "--shell", "C:\\Program Files\\Git\\bin\\bash.exe"  # 强制使用 Git Bash
]
```

### 3.3 方案 C: 智能 Shell 转换 (不推荐)
尝试编写中间层，将 Bash 命令自动转换为 PowerShell。
- **风险**: 极其复杂，容易出错（如 `awk`, `sed` 等复杂管道难以完美转换）。
- **结论**: 让 AI 自己生成正确的语法比转换更可靠。

## 4. 实施路线图

1.  **验证当前环境**: 写一个测试工具，打印 `echo $0` 或 `$PSVersionTable` 确认当前 MCP 使用的 Shell。
2.  **增强 MCP Server**: 修改 JS 代码，支持接受 `--shell` 参数，并将其传递给 `child_process.exec` 的 `shell` 选项。
3.  **注入上下文**: 修改 MineClaw 的 `Session` 逻辑，在构建 System Prompt 时加入 `OS` 和 `Shell` 信息。
4.  **Git Bash 适配**: 测试指定 `bash.exe` 作为 Shell 的可行性。

## 5. 总结
解决跨平台问题的核心不在于“抹平差异”，而在于**“如实告知”**。AI 模型（特别是 GPT-4o/Claude 3.5）非常擅长写 PowerShell 或 CMD 脚本，前提是它知道自己**正在** Windows 上运行。
