use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub llm: LlmConfig,
    #[serde(default)]
    pub mcp: Option<McpConfig>,
    #[serde(default)]
    pub filesystem: FilesystemConfig,
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
    #[serde(default)]
    pub checkpoint: CheckpointConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CheckpointConfig {
    #[serde(default = "default_checkpoint_enabled")]
    pub enabled: bool,
    #[serde(default = "default_checkpoint_dir")]
    pub directory: String,
    #[serde(default = "default_max_checkpoints")]
    pub max_per_session: usize,
    #[serde(default = "default_auto_cleanup_days")]
    pub auto_cleanup_days: u64,
}

fn default_max_read_bytes() -> usize {
    16384
}

fn default_checkpoint_enabled() -> bool {
    true
}

fn default_checkpoint_dir() -> String {
    ".checkpoints".to_string()
}

fn default_max_checkpoints() -> usize {
    50
}

fn default_auto_cleanup_days() -> u64 {
    30
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            enabled: default_checkpoint_enabled(),
            directory: default_checkpoint_dir(),
            max_per_session: default_max_checkpoints(),
            auto_cleanup_days: default_auto_cleanup_days(),
        }
    }
}

impl Default for FilesystemConfig {
    fn default() -> Self {
        Self {
            max_read_bytes: default_max_read_bytes(),
            allowed_directories: Vec::new(),
            checkpoint: CheckpointConfig::default(),
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
            .map_err(crate::error::Error::Config)?;

        if config_path.exists() {
            settings = settings.add_source(config::File::from(config_path));
        }

        let settings = settings
            .add_source(config::Environment::with_prefix("MINECLAW").separator("__"))
            .build()?;

        let config = settings.try_deserialize::<Config>()?;

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
