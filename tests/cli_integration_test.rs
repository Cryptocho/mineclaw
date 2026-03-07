use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_keygen_lifecycle() {
    // 1. 生成密钥 (Generate)
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_keygen"));
    let assert = cmd.arg("generate").assert();

    let assert = assert
        .success()
        .stderr(predicate::str::contains("Generating new encryption key..."));

    let output = assert.get_output();
    let key = String::from_utf8_lossy(&output.stdout).trim().to_string();
    
    // 验证 Key 格式 (Base64, 长度合理)
    assert!(key.len() > 30, "Key length should be reasonable");
    assert!(!key.contains('\n'), "Key should be single line");

    // 2. 加密 (Encrypt)
    let plaintext = "sk-test-secret-12345";
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_keygen"));
    let assert = cmd
        .arg("encrypt")
        .arg(&key)
        .write_stdin(plaintext)
        .assert();

    let assert = assert.success();
    let output = assert.get_output();
    let ciphertext = String::from_utf8_lossy(&output.stdout).trim().to_string();
    
    assert!(ciphertext.starts_with("encrypted:"), "Ciphertext must have prefix");
    assert!(ciphertext.len() > plaintext.len(), "Ciphertext should be longer than plaintext");

    // 3. 解密 (Decrypt)
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_keygen"));
    let assert = cmd
        .arg("decrypt")
        .arg(&key)
        .write_stdin(ciphertext.as_bytes())
        .assert();

    assert
        .success()
        .stdout(predicate::str::diff(plaintext)); // diff check for exact match
}

#[test]
fn test_encrypt_empty_input() {
    // Generate Key
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_keygen"));
    let key = String::from_utf8(cmd.arg("generate").output().unwrap().stdout).unwrap();
    let key = key.trim();

    // Encrypt Empty String
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_keygen"));
    let assert = cmd
        .arg("encrypt")
        .arg(key)
        .write_stdin("")
        .assert();

    let assert = assert
        .success()
        .stderr(predicate::str::contains("Warning: Encrypting empty string"));
        
    let ciphertext = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    
    // Decrypt Empty String
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_keygen"));
    cmd.arg("decrypt")
        .arg(key)
        .write_stdin(ciphertext.as_bytes())
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

#[test]
fn test_invalid_key() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_keygen"));
    cmd.arg("encrypt")
        .arg("invalid-key")
        .write_stdin("data")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid key"));
}

#[test]
fn test_decrypt_invalid_ciphertext() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_keygen"));
    let key = String::from_utf8(cmd.arg("generate").output().unwrap().stdout).unwrap();
    let key = key.trim();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_keygen"));
    cmd.arg("decrypt")
        .arg(key)
        .write_stdin("encrypted:invalid-base64!")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Decryption failed"));
}
