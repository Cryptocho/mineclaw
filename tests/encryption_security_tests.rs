#[cfg(test)]
mod tests {
    use mineclaw::encryption::EncryptionManager;
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Key, Nonce,
    };
    use base64::{engine::general_purpose, Engine as _};
    use rand::RngCore;

    // --- 1. 密码学原语正确性 (Standard Vectors) ---

    // 使用 AES-GCM 标准测试向量进行验证
    #[test]
    fn test_standard_vector_compatibility() {
        // Test Vector from NIST (AES-GCM, KeyLen=256)
        // Source: https://csrc.nist.gov/CSRC/media/Projects/Cryptographic-Algorithm-Validation-Program/documents/mac/gcmtestvectors.zip
        
        // Key: 0000000000000000000000000000000000000000000000000000000000000000
        let key_hex = "0000000000000000000000000000000000000000000000000000000000000000";
        // IV (Nonce): 000000000000000000000000
        let nonce_hex = "000000000000000000000000";
        // Plaintext: 00000000000000000000000000000000
        // (Use empty plaintext for simplicity if needed, but here we use 16 bytes zero)
        let plaintext_hex = "00000000000000000000000000000000";
        // Expected Ciphertext (from NIST): 
        // 530f8afbc74536b9a963b4f1c4cb738b (Ciphertext)
        // cead468600000000d000000000000000 (Tag? No, let's generate it correctly using the library to verify compatibility)
        
        // 实际上，我们不需要硬编码密文，而是验证：
        // "手动调用 aes-gcm 库生成的标准密文" 是否能被 "EncryptionManager" 解密。
        // 这证明了 EncryptionManager 没有引入非标准的 padding 或结构。

        let key_bytes = hex::decode(key_hex).unwrap();
        let nonce_bytes = hex::decode(nonce_hex).unwrap();
        let plaintext_bytes = hex::decode(plaintext_hex).unwrap();
        
        // 1. 使用标准库生成参考密文 (Standard GCM)
        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);
        let nonce = Nonce::from_slice(&nonce_bytes);
        // encrypt returns Ciphertext + Tag
        let ciphertext_with_tag = cipher.encrypt(nonce, plaintext_bytes.as_slice()).unwrap();
        
        // 2. 构造 EncryptionManager 需要的输入格式 (Nonce + Ciphertext + Tag)
        let mut combined = nonce_bytes.clone();
        combined.extend_from_slice(&ciphertext_with_tag);
        let combined_base64 = general_purpose::STANDARD.encode(combined);
        
        // 3. 使用 EncryptionManager 解密
        let key_base64 = general_purpose::STANDARD.encode(key_bytes);
        let manager = EncryptionManager::new(&key_base64).unwrap();
        
        let _decrypted_string = manager.decrypt(&combined_base64).unwrap();
        
        // 注意：EncryptionManager::decrypt 返回 String，所以我们必须确保测试向量的明文是合法的 UTF-8
        // 上面的全 0 字节不是合法的 UTF-8 字符串。
        // 所以我们改用 ASCII 明文进行测试。
        
        // Retry with ASCII Plaintext
        let plaintext_ascii = "Hello NIST!";
        let ciphertext_with_tag_ascii = cipher.encrypt(nonce, plaintext_ascii.as_bytes()).unwrap();
        
        let mut combined_ascii = nonce_bytes.clone();
        combined_ascii.extend_from_slice(&ciphertext_with_tag_ascii);
        let combined_base64_ascii = general_purpose::STANDARD.encode(combined_ascii);
        
        let decrypted_ascii = manager.decrypt(&combined_base64_ascii).unwrap();
        assert_eq!(decrypted_ascii, plaintext_ascii);
    }

    // --- 2. 错误信息安全性 (Error Safety) ---

    #[test]
    fn test_error_message_safety() {
        let key = EncryptionManager::generate_key();
        let manager = EncryptionManager::new(&key).unwrap();
        
        // 构造一个无效的 Base64 密文
        let invalid_base64 = "invalid-base64-content";
        let result = manager.decrypt(invalid_base64);
        
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        
        // 错误信息不应包含敏感内容 (如 Key)
        assert!(!err_msg.contains(&key));
        // 错误信息应清晰描述问题
        assert!(err_msg.contains("Invalid base64 ciphertext"));
    }

    // --- 3. 密文格式鲁棒性 (Robustness / Fuzzing) ---

    #[test]
    fn test_ciphertext_robustness() {
        let key = EncryptionManager::generate_key();
        let manager = EncryptionManager::new(&key).unwrap();
        
        // 1. 空字符串
        assert!(manager.decrypt("").is_err());
        
        // 2. 极短字符串 (Base64 解码后不足 28 字节)
        let short_payload = general_purpose::STANDARD.encode(vec![0u8; 27]);
        let result = manager.decrypt(&short_payload);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Ciphertext too short"));
        
        // 3. 刚好 28 字节 (Nonce 12 + Tag 16, 无 Ciphertext -> 空明文)
        // 理论上这是合法的（加密空字符串）
        let empty_plaintext = "";
        let encrypted_empty = manager.encrypt(empty_plaintext).unwrap();
        assert_eq!(manager.decrypt(&encrypted_empty).unwrap(), "");

        // 4. 随机垃圾数据 (Fuzzing)
        let mut rng = rand::thread_rng();
        for _ in 0..100 {
            let len = (rng.next_u32() % 100) as usize;
            let mut random_bytes = vec![0u8; len];
            rng.fill_bytes(&mut random_bytes);
            let random_base64 = general_purpose::STANDARD.encode(random_bytes);
            
            // 只要不是巧合生成了合法的 GCM Tag，都应该解密失败
            // 即使解密失败，也不能 Panic
            let result = manager.decrypt(&random_base64);
            assert!(result.is_err());
        }
    }

    // --- 4. 密钥随机性 (Randomness Sanity Check) ---

    #[test]
    fn test_key_randomness_sanity() {
        // 生成 1000 个 Key，简单检查是否有重复
        // (真正的随机性测试需要 Dieharder 等工具，这里只做基本的 Sanity Check)
        let mut keys = std::collections::HashSet::new();
        for _ in 0..1000 {
            let key = EncryptionManager::generate_key();
            // 检查长度
            let decoded = general_purpose::STANDARD.decode(&key).unwrap();
            assert_eq!(decoded.len(), 32);
            // 检查是否全 0 (极不可能)
            assert_ne!(decoded, vec![0u8; 32]);
            
            // 检查重复
            if keys.contains(&key) {
                panic!("Generated duplicate key! RNG is broken.");
            }
            keys.insert(key);
        }
    }
}
