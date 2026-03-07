#[cfg(test)]
mod tests {
    use mineclaw::encryption::EncryptionManager;
    use base64::Engine;

    #[test]
    fn test_encryption_manager_lifecycle() {
        // 1. 生成密钥
        let key = EncryptionManager::generate_key();
        assert_eq!(key.len(), 44); // Base64 encoded 32 bytes

        // 2. 创建管理器
        let manager = EncryptionManager::new(&key).expect("Failed to create manager");

        // 3. 原始数据
        let plaintext = "sk-test-1234567890abcdef";

        // 4. 加密
        let encrypted = manager.encrypt(plaintext).expect("Encryption failed");
        assert_ne!(plaintext, encrypted);

        // 5. 解密
        let decrypted = manager.decrypt(&encrypted).expect("Decryption failed");
        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_invalid_key() {
        // 无效的 Base64
        assert!(EncryptionManager::new("invalid-base64!").is_err());
        
        // 长度不对
        let short_key = base64::engine::general_purpose::STANDARD.encode(vec![0u8; 16]);
        assert!(EncryptionManager::new(&short_key).is_err());
    }

    #[test]
    fn test_invalid_ciphertext() {
        let key = EncryptionManager::generate_key();
        let manager = EncryptionManager::new(&key).unwrap();

        // 随机字符串无法解密
        assert!(manager.decrypt("invalid-ciphertext").is_err());
    }

    #[test]
    fn test_tampered_ciphertext_gcm_tag() {
        let key = EncryptionManager::generate_key();
        let manager = EncryptionManager::new(&key).unwrap();
        let plaintext = "secret-data";
        
        let encrypted_base64 = manager.encrypt(plaintext).unwrap();
        let mut encrypted_bytes = base64::engine::general_purpose::STANDARD.decode(&encrypted_base64).unwrap();
        
        // GCM 密文结构: Nonce (12) + Ciphertext + Tag (16)
        // 修改最后一个字节 (Tag 的一部分)
        let last_index = encrypted_bytes.len() - 1;
        encrypted_bytes[last_index] ^= 0xFF; // Flip bits
        
        let tampered_base64 = base64::engine::general_purpose::STANDARD.encode(&encrypted_bytes);
        
        // 解密应该失败 (Tag mismatch)
        let result = manager.decrypt(&tampered_base64);
        assert!(result.is_err(), "Decryption should fail when Tag is tampered");
    }

    #[test]
    fn test_nonce_uniqueness() {
        let key = EncryptionManager::generate_key();
        let manager = EncryptionManager::new(&key).unwrap();
        let plaintext = "same-data";
        
        let enc1 = manager.encrypt(plaintext).unwrap();
        let enc2 = manager.encrypt(plaintext).unwrap();
        
        // 即使明文相同，密文也必须不同 (因为 Nonce 不同)
        assert_ne!(enc1, enc2, "Ciphertext must be different for same plaintext (Nonce reuse?)");
        
        // 解密都应该成功
        assert_eq!(manager.decrypt(&enc1).unwrap(), plaintext);
        assert_eq!(manager.decrypt(&enc2).unwrap(), plaintext);
    }
}
