use mineclaw::config::Config;
use mineclaw::encryption::EncryptionManager;
use temp_env::with_vars;

#[test]
fn test_config_decryption() {
    // 1. 生成密钥
    let key = EncryptionManager::generate_key();
    let manager = EncryptionManager::new(&key).unwrap();

    // 2. 准备加密后的 API Key
    let original_api_key = "sk-test-secret-key";
    let encrypted_api_key = manager.encrypt(original_api_key).unwrap();
    let config_api_key_value = format!("encrypted:{}", encrypted_api_key);

    // 3. 临时设置环境变量
    with_vars(
        vec![
            ("MINECLAW_ENCRYPTION_KEY", Some(&key)),
            ("MINECLAW__LLM__API_KEY", Some(&config_api_key_value)),
        ],
        || {
            // 4. 加载配置 (应该自动解密)
            let config = Config::load().expect("Failed to load config");
            
            // 5. 验证
            assert_eq!(config.llm.api_key, original_api_key);
        }
    );
}

#[test]
fn test_config_decryption_missing_key() {
    // 1. 生成密钥
    let key = EncryptionManager::generate_key();
    let manager = EncryptionManager::new(&key).unwrap();

    // 2. 准备加密后的 API Key
    let original_api_key = "sk-test-secret-key";
    let encrypted_api_key = manager.encrypt(original_api_key).unwrap();
    let config_api_key_value = format!("encrypted:{}", encrypted_api_key);

    // 3. 临时设置环境变量 (只设置配置，不设置解密 Key)
    with_vars(
        vec![
            ("MINECLAW_ENCRYPTION_KEY", None), // Ensure key is missing
            ("MINECLAW__LLM__API_KEY", Some(&config_api_key_value)),
        ],
        || {
            // 4. 加载配置 (应该失败)
            let result = Config::load();
            
            // 5. 验证错误
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(err.to_string().contains("MINECLAW_ENCRYPTION_KEY is missing"));
        }
    );
}

#[test]
fn test_config_decryption_invalid_key() {
    with_vars(
        vec![
            ("MINECLAW_ENCRYPTION_KEY", Some("invalid-key!")),
            ("MINECLAW__LLM__API_KEY", Some("encrypted:some-data")),
        ],
        || {
            // 2. 加载配置 (应该失败)
            let result = Config::load();
            
            // 3. 验证错误
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(err.to_string().contains("Invalid encryption key"));
        }
    );
}

#[test]
fn test_config_plaintext_fallback() {
    let plain_key = "sk-plain-text-key";
    let key = EncryptionManager::generate_key();
    
    with_vars(
        vec![
            ("MINECLAW_ENCRYPTION_KEY", Some(key.as_str())),
            ("MINECLAW__LLM__API_KEY", Some(plain_key)),
        ],
        || {
            // 2. 加载配置
            let config = Config::load().expect("Should load plaintext config successfully");
            
            // 3. 验证 (原样读取)
            assert_eq!(config.llm.api_key, plain_key);
        }
    );
}
