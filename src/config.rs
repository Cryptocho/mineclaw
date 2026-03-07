use crate::encryption::EncryptionManager;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::info;

/// Checkpoint 配置
#[derive(Debug, Deserialize, Clone)]
pub struct CheckpointConfig {
    /// 是否启用 checkpoint
    #[serde(default = "default_checkpoint_enabled")]
    pub enabled: bool,
    /// Checkpoint 存储目录（agentfs 路径）
    #[serde(default = "default_checkpoint_directory")]
    pub checkpoint_directory: String,
}

fn default_checkpoint_enabled() -> bool {
    true
}

fn default_checkpoint_directory() -> String {
    ".checkpoints".to_string()
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            enabled: default_checkpoint_enabled(),
            checkpoint_directory: default_checkpoint_directory(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub llm: LlmConfig,
    #[serde(default)]
    pub mcp: Option<McpConfig>,
    #[serde(default)]
    pub filesystem: FilesystemConfig,
    #[serde(default)]
    pub checkpoint: CheckpointConfig,
    #[serde(default = "default_agentfs_db_path")]
    pub agentfs_db_path: String,
    pub encryption: Option<EncryptionConfig>,
}

fn default_agentfs_db_path() -> String {
    "data/mineclaw.db".to_string()
}

#[derive(Debug, Deserialize, Clone)]
pub struct EncryptionConfig {
    // 加密密钥通过环境变量 MINECLAW_ENCRYPTION_KEY 提供，不需要在文件中配置
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LlmConfig {
    pub provider: String,
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct McpConfig {
    pub enabled: bool,
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FilesystemConfig {
    #[serde(default = "default_max_read_bytes")]
    pub max_read_bytes: usize,
    #[serde(default)]
    pub allowed_directories: Vec<String>,
}

fn default_max_read_bytes() -> usize {
    16384
}

impl Default for FilesystemConfig {
    fn default() -> Self {
        Self {
            max_read_bytes: default_max_read_bytes(),
            allowed_directories: Vec::new(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 18789,
            },
            llm: LlmConfig {
                provider: "openai".to_string(),
                api_key: "".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                model: "gpt-4o".to_string(),
                max_tokens: 2048,
                temperature: 0.7,
            },
            mcp: None,
            filesystem: FilesystemConfig::default(),
            checkpoint: CheckpointConfig::default(),
            agentfs_db_path: default_agentfs_db_path(),
            encryption: None,
        }
    }
}

impl Config {
    pub fn load() -> crate::error::Result<Self> {
        let config_path = Self::get_config_path()?;

        let mut settings = config::Config::builder();

        let default_config = Config::default();
        settings = settings
            .set_default("server.host", default_config.server.host)
            .map_err(crate::error::Error::Config)?
            .set_default("server.port", default_config.server.port)
            .map_err(crate::error::Error::Config)?
            .set_default("llm.provider", default_config.llm.provider)
            .map_err(crate::error::Error::Config)?
            .set_default("llm.api_key", default_config.llm.api_key)
            .map_err(crate::error::Error::Config)?
            .set_default("llm.base_url", default_config.llm.base_url)
            .map_err(crate::error::Error::Config)?
            .set_default("llm.model", default_config.llm.model)
            .map_err(crate::error::Error::Config)?
            .set_default("llm.max_tokens", default_config.llm.max_tokens)
            .map_err(crate::error::Error::Config)?
            .set_default("llm.temperature", default_config.llm.temperature)
            .map_err(crate::error::Error::Config)?
            .set_default("agentfs_db_path", default_config.agentfs_db_path)
            .map_err(crate::error::Error::Config)?;

        if config_path.exists() {
            settings = settings.add_source(config::File::from(config_path));
        }

        let settings = settings
            .add_source(config::Environment::with_prefix("MINECLAW").separator("__"))
            .build()?;

        let mut config = settings.try_deserialize::<Config>()?;

        // 尝试自动解密敏感字段
        if config.llm.api_key.starts_with("encrypted:") {
            let key = std::env::var("MINECLAW_ENCRYPTION_KEY").map_err(|_| {
                crate::error::Error::Config(config::ConfigError::Message(
                    "Encrypted API Key detected but MINECLAW_ENCRYPTION_KEY is missing".to_string(),
                ))
            })?;

            let manager = EncryptionManager::new(&key).map_err(|e| {
                crate::error::Error::Config(config::ConfigError::Message(format!(
                    "Invalid encryption key: {}",
                    e
                )))
            })?;

            let cipher_text = config.llm.api_key.trim_start_matches("encrypted:");
            let plain_text = manager.decrypt(cipher_text).map_err(|e| {
                crate::error::Error::Config(config::ConfigError::Message(format!(
                    "Failed to decrypt LLM API Key: {}",
                    e
                )))
            })?;

            info!("Successfully decrypted LLM API Key");
            config.llm.api_key = plain_text;
        }

        Ok(config)
    }

    fn get_config_path() -> crate::error::Result<PathBuf> {
        Ok(PathBuf::from("config/mineclaw.toml"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();

        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 18789);
        assert_eq!(config.llm.provider, "openai");
        assert!(config.mcp.is_none());
    }

    #[test]
    fn test_mcp_config_deserialization() {
        let toml_content = r#"
[server]
host = "127.0.0.1"
port = 18789

[llm]
provider = "openai"
api_key = "test-key"
base_url = "https://api.openai.com/v1"
model = "gpt-4o"
max_tokens = 2048
temperature = 0.7

[mcp]
enabled = true

[[mcp.servers]]
name = "filesystem"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/test"]
env = { "TEST_ENV" = "value" }
"#;

        let config: Config = toml::from_str(toml_content).unwrap();

        assert!(config.mcp.is_some());
        let mcp = config.mcp.unwrap();
        assert!(mcp.enabled);
        assert_eq!(mcp.servers.len(), 1);

        let server = &mcp.servers[0];
        assert_eq!(server.name, "filesystem");
        assert_eq!(server.command, "npx");
        assert_eq!(
            server.args,
            vec!["-y", "@modelcontextprotocol/server-filesystem", "/test"]
        );
        assert_eq!(server.env.get("TEST_ENV"), Some(&"value".to_string()));
    }

    #[test]
    fn test_mcp_config_without_servers() {
        let toml_content = r#"
[server]
host = "127.0.0.1"
port = 18789

[llm]
provider = "openai"
api_key = "test-key"
base_url = "https://api.openai.com/v1"
model = "gpt-4o"
max_tokens = 2048
temperature = 0.7

[mcp]
enabled = false
"#;

        let config: Config = toml::from_str(toml_content).unwrap();

        assert!(config.mcp.is_some());
        let mcp = config.mcp.unwrap();
        assert!(!mcp.enabled);
        assert!(mcp.servers.is_empty());
    }

    #[test]
    fn test_mcp_config_without_env_and_args() {
        let toml_content = r#"
[server]
host = "127.0.0.1"
port = 18789

[llm]
provider = "openai"
api_key = "test-key"
base_url = "https://api.openai.com/v1"
model = "gpt-4o"
max_tokens = 2048
temperature = 0.7

[mcp]
enabled = true

[[mcp.servers]]
name = "simple"
command = "echo"
"#;

        let config: Config = toml::from_str(toml_content).unwrap();

        let mcp = config.mcp.unwrap();
        let server = &mcp.servers[0];
        assert!(server.args.is_empty());
        assert!(server.env.is_empty());
    }
}
