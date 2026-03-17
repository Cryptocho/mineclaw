//! Shell 检测模块
//! 提供检测当前 Shell 类型的功能

/// Shell 类型枚举
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellType {
    /// Unix Shell (bash, zsh, sh, git bash 等)
    Unix,
    /// Windows PowerShell 或 Cmd
    Windows,
}

/// 检测当前系统 Shell 类型
pub fn system_shell() -> ShellType {
    if std::env::var("SHELL").is_ok_and(|s| !s.is_empty()) {
        return ShellType::Unix;
    }

    #[cfg(windows)]
    {
        ShellType::Windows
    }

    #[cfg(not(windows))]
    {
        ShellType::Unix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_shell() {
        let shell = system_shell();
        assert!(matches!(shell, ShellType::Unix | ShellType::Windows));
    }
}
