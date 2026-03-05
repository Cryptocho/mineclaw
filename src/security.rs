use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose, Engine as _};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use crate::error::{Error, Result};

const MASTER_KEY_FILENAME: &str = "master.key";
const ENC_PREFIX: &str = "ENC:";

/// 安全管理器，负责处理加密和解密
pub struct SecurityManager {
    config_dir: PathBuf,
}

impl SecurityManager {
    pub fn new<P: AsRef<Path>>(config_dir: P) -> Self {
        Self {
            config_dir: config_dir.as_ref().to_path_buf(),
        }
    }

    /// 获取或创建 Master Key
    /// 如果不存在则创建新的随机密钥
    fn get_or_create_master_key(&self) -> Result<Key<Aes256Gcm>> {
        let key_path = self.config_dir.join(MASTER_KEY_FILENAME);

        if key_path.exists() {
            let key_bytes = fs::read(&key_path).map_err(|e| {
                Error::ConfigError(format!("Failed to read master key: {}", e))
            })?;

            if key_bytes.len() != 32 {
                return Err(Error::ConfigError(
                    "Invalid master key length (must be 32 bytes)".to_string(),
                ));
            }

            Ok(*Key::<Aes256Gcm>::from_slice(&key_bytes))
        } else {
            info!("Master key not found, creating new one at {:?}", key_path);
            let key = Aes256Gcm::generate_key(OsRng);
            fs::write(&key_path, key.as_slice()).map_err(|e| {
                Error::ConfigError(format!("Failed to write master key: {}", e))
            })?;
            Ok(key)
        }
    }

    /// 加密字符串
    /// 返回格式: ENC:<base64(nonce + ciphertext)>
    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        if plaintext.starts_with(ENC_PREFIX) {
            // 已经是加密格式，直接返回
            return Ok(plaintext.to_string());
        }

        let key = self.get_or_create_master_key()?;
        let cipher = Aes256Gcm::new(&key);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 96-bits; unique per message

        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| Error::ConfigError(format!("Encryption failed: {}", e)))?;

        // 拼接 nonce 和密文
        let mut combined = nonce.to_vec();
        combined.extend_from_slice(&ciphertext);

        let encoded = general_purpose::STANDARD.encode(&combined);
        Ok(format!("{}{}", ENC_PREFIX, encoded))
    }

    /// 解密字符串
    /// 如果输入不是 ENC: 开头，则原样返回（假设是明文）
    pub fn decrypt(&self, input: &str) -> Result<String> {
        if !input.starts_with(ENC_PREFIX) {
            return Ok(input.to_string());
        }

        let encoded = &input[ENC_PREFIX.len()..];
        let combined = general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| Error::ConfigError(format!("Base64 decode failed: {}", e)))?;

        if combined.len() < 12 {
            return Err(Error::ConfigError("Invalid encrypted data length".to_string()));
        }

        let key = self.get_or_create_master_key()?;
        let cipher = Aes256Gcm::new(&key);

        let nonce = Nonce::from_slice(&combined[..12]);
        let ciphertext = &combined[12..];

        let plaintext_bytes = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| Error::ConfigError(format!("Decryption failed: {}", e)))?;

        String::from_utf8(plaintext_bytes)
            .map_err(|e| Error::ConfigError(format!("Invalid UTF-8 in decrypted data: {}", e)))
    }
}
