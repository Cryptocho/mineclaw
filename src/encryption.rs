use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose, Engine as _};
use anyhow::{anyhow, Result, Context};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// 加密管理器，负责处理数据的加密和解密
/// 
/// # Security
/// - Key is zeroized on drop
/// - Uses AES-256-GCM
/// - Generates random 96-bit nonce for each encryption
/// - Enforces authentication tag verification
pub struct EncryptionManager {
    key: ZeroizingKey,
}

#[derive(Zeroize, ZeroizeOnDrop)]
struct ZeroizingKey(Key<Aes256Gcm>);

impl EncryptionManager {
    /// 创建一个新的 EncryptionManager，使用提供的 Base64 编码的密钥
    pub fn new(key_base64: &str) -> Result<Self> {
        let mut key_bytes = general_purpose::STANDARD
            .decode(key_base64)
            .map_err(|e| anyhow!("Invalid base64 key: {}", e))?;
        
        if key_bytes.len() != 32 {
            key_bytes.zeroize();
            return Err(anyhow!("Invalid key length: expected 32 bytes, got {}", key_bytes.len()));
        }

        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        let manager = Ok(Self { key: ZeroizingKey(*key) });
        
        // Zeroize intermediate buffer
        key_bytes.zeroize();
        
        manager
    }

    /// 生成一个新的随机密钥（Base64 编码）
    pub fn generate_key() -> String {
        let key = Aes256Gcm::generate_key(OsRng);
        general_purpose::STANDARD.encode(key)
    }

    /// 加密明文数据
    /// 返回格式: nonce(12 bytes) + ciphertext + tag(16 bytes) 的 Base64 编码
    /// 注意：aes-gcm crate 的 encrypt 方法返回的 ciphertext 已经包含了 tag
    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        let cipher = Aes256Gcm::new(&self.key.0);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 96-bits; unique per message
        
        let ciphertext_with_tag = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| anyhow!("Encryption failed: {}", e))?;
            
        // 组合 nonce 和密文(含tag)
        let mut combined = nonce.to_vec();
        combined.extend_from_slice(&ciphertext_with_tag);
        
        Ok(general_purpose::STANDARD.encode(combined))
    }

    /// 解密密文数据（Base64 编码的 nonce + ciphertext + tag）
    /// 
    /// # Security
    /// - 验证 GCM Tag（由 aes-gcm crate 内部处理）
    /// - 检查 Nonce 长度
    pub fn decrypt(&self, ciphertext_base64: &str) -> Result<String> {
        let combined = general_purpose::STANDARD
            .decode(ciphertext_base64)
            .context("Invalid base64 ciphertext")?;
            
        // 12 bytes nonce + min 16 bytes tag = 28 bytes
        if combined.len() < 28 {
            return Err(anyhow!("Ciphertext too short (min 28 bytes required for Nonce+Tag)"));
        }
        
        let (nonce_bytes, ciphertext_with_tag) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let cipher = Aes256Gcm::new(&self.key.0);
        
        let mut plaintext_bytes = cipher
            .decrypt(nonce, ciphertext_with_tag)
            .map_err(|_| anyhow!("Decryption failed: Tag verification failed or ciphertext corrupted"))?;
            
        let plaintext = String::from_utf8(plaintext_bytes.clone())
            .context("Invalid UTF-8 plaintext")?;
            
        // Zeroize intermediate plaintext buffer
        plaintext_bytes.zeroize();
        
        Ok(plaintext)
    }
}
