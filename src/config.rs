use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::security::SecurityManager;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub llm: LlmConfig,
    #[serde(default)]
    pub mcp: Option<McpConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LlmConfig {
    pub provider: String,
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct McpConfig {
    pub enabled: bool,
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
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
        }
    }
}

impl Config {
    pub fn load() -> crate::error::Result<Self> {
        let config_path = Self::get_config_path()?;
        let config_dir = config_path.parent().unwrap_or_else(|| std::path::Path::new("."));

        let mut settings = config::Config::builder();

        // 加载默认配置...
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
            settings = settings.add_source(config::File::from(config_path.clone()));
        }

        let settings = settings
            .add_source(config::Environment::with_prefix("MINECLAW").separator("__"))
            .build()?;

        let mut config = settings.try_deserialize::<Config>()?;

        // 尝试解密 API Key
        if !config.llm.api_key.is_empty() {
            let security = SecurityManager::new(config_dir);
            config.llm.api_key = security.decrypt(&config.llm.api_key)?;
        }

        Ok(config)
    }

    fn get_config_path() -> crate::error::Result<PathBuf> {
        Ok(PathBuf::from("config/mineclaw.toml"))
    }

    pub fn save(&self) -> crate::error::Result<()> {
        let config_path = Self::get_config_path()?;
        let config_dir = config_path.parent().unwrap_or_else(|| std::path::Path::new("."));

        // 克隆一份用于保存，以便修改 API Key
        let mut config_to_save = self.clone();

        // 加密 API Key
        if !config_to_save.llm.api_key.is_empty() {
            let security = SecurityManager::new(config_dir);
            config_to_save.llm.api_key = security.encrypt(&config_to_save.llm.api_key)?;
        }

        let toml_string = toml::to_string(&config_to_save)
            .map_err(|e| crate::error::Error::Config(config::ConfigError::Foreign(Box::new(e))))?;
        std::fs::write(config_path, toml_string)?;
        Ok(())
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
