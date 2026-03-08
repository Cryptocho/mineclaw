//! 文件系统工具测试

use mineclaw::config::{Config, FilesystemConfig};
use mineclaw::error::Error;
use mineclaw::models::Session;
use mineclaw::tools::filesystem::FilesystemTool;
use mineclaw::tools::{LocalToolRegistry, ToolContext};
use serde_json::json;
use std::sync::Arc;
use tempfile::tempdir;

/// 创建测试用的 ToolContext
fn create_test_context(config: Config) -> ToolContext {
    ToolContext::new(Session::new(), Arc::new(config))
}

#[tokio::test]
async fn test_read_write_file() {
    let temp_dir = tempdir().unwrap();
    let test_file = temp_dir.path().join("test.txt");

    let mut config = Config::default();
    config.filesystem = FilesystemConfig {
        max_read_bytes: 16384,
        allowed_directories: vec![temp_dir.path().to_string_lossy().to_string()],
    };

    let mut registry = LocalToolRegistry::new();
    FilesystemTool::register_all(&mut registry);

    let context = create_test_context(config);

    // Write file
    let write_result = registry
        .call_tool(
            "write_file",
            json!({
                "path": test_file.to_string_lossy().to_string(),
                "content": "Hello, World!"
            }),
            context.clone(),
        )
        .await
        .unwrap();

    assert!(write_result["success"].as_bool().unwrap());
    assert_eq!(write_result["bytes_written"].as_u64().unwrap(), 13);

    // Read file
    let read_result = registry
        .call_tool(
            "read_file",
            json!({
                "path": test_file.to_string_lossy().to_string()
            }),
            context,
        )
        .await
        .unwrap();

    assert_eq!(read_result["content"].as_str().unwrap(), "Hello, World!");
    assert!(!read_result["truncated"].as_bool().unwrap());
}

#[tokio::test]
async fn test_list_directory() {
    // Test with absolute path and relative path
    let temp_dir = tempdir().unwrap();
    let temp_dir_abs = temp_dir.path().canonicalize().unwrap();
    let temp_dir_str = temp_dir_abs.to_string_lossy().to_string();

    // Create test files
    std::fs::File::create(temp_dir_abs.join("file1.txt")).unwrap();
    std::fs::File::create(temp_dir_abs.join("file2.txt")).unwrap();
    std::fs::create_dir(temp_dir_abs.join("subdir")).unwrap();
    std::fs::File::create(temp_dir_abs.join("subdir/nested.txt")).unwrap();

    // Change to temp dir's parent to test relative paths
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir_abs.parent().unwrap()).unwrap();

    let mut config = Config::default();
    config.filesystem = FilesystemConfig {
        max_read_bytes: 16384,
        allowed_directories: vec![temp_dir_str.clone()],
    };

    let mut registry = LocalToolRegistry::new();
    FilesystemTool::register_all(&mut registry);

    let context = create_test_context(config);

    // Test 1: List with relative path
    let temp_dir_name = temp_dir_abs.file_name().unwrap().to_string_lossy();
    let result = registry
        .call_tool(
            "list_directory",
            json!({
                "path": temp_dir_name.to_string()
            }),
            context.clone(),
        )
        .await
        .unwrap();

    let entries = result["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 3);

    let names: Vec<_> = entries
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"file1.txt"));
    assert!(names.contains(&"file2.txt"));
    assert!(names.contains(&"subdir"));

    // Check that paths are relative to the caller's input
    for entry in entries {
        let path = entry["path"].as_str().unwrap();
        let name = entry["name"].as_str().unwrap();
        assert_eq!(path, format!("{}/{}", temp_dir_name, name));
    }

    // Test 2: List with absolute path - should return absolute paths
    let result = registry
        .call_tool(
            "list_directory",
            json!({
                "path": temp_dir_str
            }),
            context.clone(),
        )
        .await
        .unwrap();

    let entries = result["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 3);

    // Paths should include the full absolute path
    for entry in entries {
        let path = entry["path"].as_str().unwrap();
        let name = entry["name"].as_str().unwrap();
        assert_eq!(path, format!("{}/{}", temp_dir_str, name));
    }

    // Test 3: Recursive with relative path
    let result = registry
        .call_tool(
            "list_directory",
            json!({
                "path": temp_dir_name.to_string(),
                "recursive": true
            }),
            context,
        )
        .await
        .unwrap();

    let entries = result["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 4);

    // Find the nested file
    let nested_file = entries
        .iter()
        .find(|e| e["name"].as_str().unwrap() == "nested.txt")
        .unwrap();
    assert_eq!(
        nested_file["path"].as_str().unwrap(),
        format!("{}/subdir/nested.txt", temp_dir_name)
    );

    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();
}

#[tokio::test]
async fn test_delete_file() {
    let temp_dir = tempdir().unwrap();
    let test_file = temp_dir.path().join("test.txt");

    std::fs::write(&test_file, "Test content").unwrap();
    assert!(test_file.exists());

    let mut config = Config::default();
    config.filesystem = FilesystemConfig {
        max_read_bytes: 16384,
        allowed_directories: vec![temp_dir.path().to_string_lossy().to_string()],
    };

    let mut registry = LocalToolRegistry::new();
    FilesystemTool::register_all(&mut registry);

    let context = create_test_context(config);

    let result = registry
        .call_tool(
            "delete_file",
            json!({
                "path": test_file.to_string_lossy().to_string()
            }),
            context,
        )
        .await
        .unwrap();

    assert!(result["success"].as_bool().unwrap());
    assert!(!test_file.exists());
}

#[tokio::test]
async fn test_path_traversal_protection() {
    let temp_dir = tempdir().unwrap();

    let mut config = Config::default();
    config.filesystem = FilesystemConfig {
        max_read_bytes: 16384,
        allowed_directories: vec![temp_dir.path().to_string_lossy().to_string()],
    };

    let mut registry = LocalToolRegistry::new();
    FilesystemTool::register_all(&mut registry);

    let context = create_test_context(config);

    // Try to access outside allowed directory with ..
    let result = registry
        .call_tool(
            "read_file",
            json!({
                "path": "../etc/passwd"
            }),
            context,
        )
        .await;

    assert!(matches!(result, Err(Error::LocalToolExecution { .. })));
}

#[tokio::test]
async fn test_search_file() {
    let temp_dir = tempdir().unwrap();
    let test_file = temp_dir.path().join("search_test.txt");

    std::fs::write(
        &test_file,
        "Line 1: Hello World\nLine 2: hello again\nLine 3: Goodbye",
    )
    .unwrap();

    let mut config = Config::default();
    config.filesystem = FilesystemConfig {
        max_read_bytes: 16384,
        allowed_directories: vec![temp_dir.path().to_string_lossy().to_string()],
    };

    let mut registry = LocalToolRegistry::new();
    FilesystemTool::register_all(&mut registry);

    let context = create_test_context(config);

    // Case-sensitive search
    let result = registry
        .call_tool(
            "search_file",
            json!({
                "path": test_file.to_string_lossy().to_string(),
                "pattern": "Hello"
            }),
            context.clone(),
        )
        .await
        .unwrap();

    let results = result["results"].as_array().unwrap();
    assert_eq!(results.len(), 1);
    let file_result = &results[0];
    let matches = file_result["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0]["line_number"].as_u64().unwrap(), 1);

    // Case-insensitive search
    let result = registry
        .call_tool(
            "search_file",
            json!({
                "path": test_file.to_string_lossy().to_string(),
                "pattern": "hello",
                "case_sensitive": false
            }),
            context,
        )
        .await
        .unwrap();

    let results = result["results"].as_array().unwrap();
    assert_eq!(results.len(), 1);
    let file_result = &results[0];
    let matches = file_result["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 2);
}

#[tokio::test]
async fn test_move_file() {
    let temp_dir = tempdir().unwrap();
    let source = temp_dir.path().join("source.txt");
    let dest = temp_dir.path().join("dest.txt");

    std::fs::write(&source, "Test content").unwrap();
    assert!(source.exists());
    assert!(!dest.exists());

    let mut config = Config::default();
    config.filesystem = FilesystemConfig {
        max_read_bytes: 16384,
        allowed_directories: vec![temp_dir.path().to_string_lossy().to_string()],
    };

    let mut registry = LocalToolRegistry::new();
    FilesystemTool::register_all(&mut registry);

    let context = create_test_context(config);

    let result = registry
        .call_tool(
            "move_file",
            json!({
                "source": source.to_string_lossy().to_string(),
                "destination": dest.to_string_lossy().to_string()
            }),
            context,
        )
        .await
        .unwrap();

    assert!(result["success"].as_bool().unwrap());
    assert!(!source.exists());
    assert!(dest.exists());
}

#[tokio::test]
async fn test_create_and_delete_directory() {
    let temp_dir = tempdir().unwrap();
    let test_dir = temp_dir.path().join("test_dir");

    let mut config = Config::default();
    config.filesystem = FilesystemConfig {
        max_read_bytes: 16384,
        allowed_directories: vec![temp_dir.path().to_string_lossy().to_string()],
    };

    let mut registry = LocalToolRegistry::new();
    FilesystemTool::register_all(&mut registry);

    let context = create_test_context(config);

    // Create directory
    let result = registry
        .call_tool(
            "create_directory",
            json!({
                "path": test_dir.to_string_lossy().to_string()
            }),
            context.clone(),
        )
        .await
        .unwrap();

    assert!(result["success"].as_bool().unwrap());
    assert!(test_dir.exists());

    // Delete directory
    let result = registry
        .call_tool(
            "delete_directory",
            json!({
                "path": test_dir.to_string_lossy().to_string()
            }),
            context,
        )
        .await
        .unwrap();

    assert!(result["success"].as_bool().unwrap());
    assert!(!test_dir.exists());
}

#[tokio::test]
async fn test_move_directory() {
    let temp_dir = tempdir().unwrap();
    let source_dir = temp_dir.path().join("source_dir");
    let dest_dir = temp_dir.path().join("dest_dir");

    std::fs::create_dir(&source_dir).unwrap();
    assert!(source_dir.exists());
    assert!(!dest_dir.exists());

    let mut config = Config::default();
    config.filesystem = FilesystemConfig {
        max_read_bytes: 16384,
        allowed_directories: vec![temp_dir.path().to_string_lossy().to_string()],
    };

    let mut registry = LocalToolRegistry::new();
    FilesystemTool::register_all(&mut registry);

    let context = create_test_context(config);

    let result = registry
        .call_tool(
            "move_directory",
            json!({
                "source": source_dir.to_string_lossy().to_string(),
                "destination": dest_dir.to_string_lossy().to_string()
            }),
            context,
        )
        .await
        .unwrap();

    assert!(result["success"].as_bool().unwrap());
    assert!(!source_dir.exists());
    assert!(dest_dir.exists());
}

#[tokio::test]
async fn test_search_and_replace() {
    let temp_dir = tempdir().unwrap();
    let test_file = temp_dir.path().join("replace_test.txt");

    std::fs::write(&test_file, "Line A: foo\nLine B: bar\nLine C: foo").unwrap();

    let mut config = Config::default();
    config.filesystem = FilesystemConfig {
        max_read_bytes: 16384,
        allowed_directories: vec![temp_dir.path().to_string_lossy().to_string()],
    };

    let mut registry = LocalToolRegistry::new();
    FilesystemTool::register_all(&mut registry);

    let context = create_test_context(config);

    // Test 1: Simple string should fail (now only supports block format)
    let result = registry
        .call_tool(
            "search_and_replace",
            json!({
                "path": test_file.to_string_lossy().to_string(),
                "diff": "foo"
            }),
            context.clone(),
        )
        .await;

    assert!(result.is_err());

    // Test 2: SEARCH/REPLACE blocks format
    let diff_with_blocks = r#"
------- SEARCH
Line A: foo
=======
Line A: FOO
+++++++ REPLACE

------- SEARCH
Line C: foo
=======
Line C: FOO
+++++++ REPLACE
"#;

    let result = registry
        .call_tool(
            "search_and_replace",
            json!({
                "path": test_file.to_string_lossy().to_string(),
                "diff": diff_with_blocks
            }),
            context,
        )
        .await
        .unwrap();

    assert!(result["success"].as_bool().unwrap());
    assert_eq!(result["replacements"].as_u64().unwrap(), 2);

    let content = std::fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, "Line A: FOO\nLine B: bar\nLine C: FOO");
}

#[tokio::test]
async fn test_replace_all_keywords() {
    let temp_dir = tempdir().unwrap();
    let test_file = temp_dir.path().join("replace_all_test.txt");

    let mut config = Config::default();
    config.filesystem = FilesystemConfig {
        max_read_bytes: 16384,
        allowed_directories: vec![temp_dir.path().to_string_lossy().to_string()],
    };

    let mut registry = LocalToolRegistry::new();
    FilesystemTool::register_all(&mut registry);

    let context = create_test_context(config);

    // Test 1: Case-sensitive simple replace
    std::fs::write(&test_file, "apple apple APPLE").unwrap();
    let result = registry
        .call_tool(
            "replace_all_keywords",
            json!({
                "path": test_file.to_string_lossy().to_string(),
                "search": "apple",
                "replace": "banana"
            }),
            context.clone(),
        )
        .await
        .unwrap();

    assert!(result["success"].as_bool().unwrap());
    assert_eq!(result["replacements"].as_u64().unwrap(), 2);
    let content = std::fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, "banana banana APPLE");

    // Test 2: Case-insensitive simple replace
    std::fs::write(&test_file, "apple apple APPLE").unwrap();
    let result = registry
        .call_tool(
            "replace_all_keywords",
            json!({
                "path": test_file.to_string_lossy().to_string(),
                "search": "apple",
                "replace": "banana",
                "case_sensitive": false
            }),
            context.clone(),
        )
        .await
        .unwrap();

    assert!(result["success"].as_bool().unwrap());
    assert_eq!(result["replacements"].as_u64().unwrap(), 3);
    let content = std::fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, "banana banana banana");

    // Test 3: Regex replace
    std::fs::write(&test_file, "Test 123 Test 456").unwrap();
    let result = registry
        .call_tool(
            "replace_all_keywords",
            json!({
                "path": test_file.to_string_lossy().to_string(),
                "search": "\\d+",
                "replace": "NUM",
                "use_regex": true
            }),
            context.clone(),
        )
        .await
        .unwrap();

    assert!(result["success"].as_bool().unwrap());
    assert_eq!(result["replacements"].as_u64().unwrap(), 2);
    let content = std::fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, "Test NUM Test NUM");

    // Test 4: Case-insensitive regex
    std::fs::write(&test_file, "HELLO hello Hello").unwrap();
    let result = registry
        .call_tool(
            "replace_all_keywords",
            json!({
                "path": test_file.to_string_lossy().to_string(),
                "search": "hello",
                "replace": "Hi",
                "case_sensitive": false,
                "use_regex": true
            }),
            context,
        )
        .await
        .unwrap();

    assert!(result["success"].as_bool().unwrap());
    assert_eq!(result["replacements"].as_u64().unwrap(), 3);
    let content = std::fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, "Hi Hi Hi");
}

#[tokio::test]
async fn test_replace_no_chained_replacement() {
    let temp_dir = tempdir().unwrap();
    let test_file = temp_dir.path().join("no_chained_replace_test.txt");

    let mut config = Config::default();
    config.filesystem = FilesystemConfig {
        max_read_bytes: 16384,
        allowed_directories: vec![temp_dir.path().to_string_lossy().to_string()],
    };

    let mut registry = LocalToolRegistry::new();
    FilesystemTool::register_all(&mut registry);

    let context = create_test_context(config);

    // Test 1: Simple string replace - no chained replacement
    // "A" → "AB", should not replace the "A" in "AB"
    std::fs::write(&test_file, "A A A").unwrap();
    let result = registry
        .call_tool(
            "replace_all_keywords",
            json!({
                "path": test_file.to_string_lossy().to_string(),
                "search": "A",
                "replace": "AB"
            }),
            context.clone(),
        )
        .await
        .unwrap();

    assert!(result["success"].as_bool().unwrap());
    assert_eq!(result["replacements"].as_u64().unwrap(), 3);
    let content = std::fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, "AB AB AB");

    // Test 2: Regex replace - no chained replacement
    // "class " → "class_", should not create new matches
    std::fs::write(&test_file, "class A class B class C").unwrap();
    let result = registry
        .call_tool(
            "replace_all_keywords",
            json!({
                "path": test_file.to_string_lossy().to_string(),
                "search": "class ",
                "replace": "class_",
                "use_regex": true
            }),
            context.clone(),
        )
        .await
        .unwrap();

    assert!(result["success"].as_bool().unwrap());
    assert_eq!(result["replacements"].as_u64().unwrap(), 3);
    let content = std::fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, "class_A class_B class_C");

    // Test 3: Verify replacing with string containing search pattern
    // "foo" → "foobar", result should be "foobar foobar", not "foobarbaz..."
    std::fs::write(&test_file, "foo foo").unwrap();
    let result = registry
        .call_tool(
            "replace_all_keywords",
            json!({
                "path": test_file.to_string_lossy().to_string(),
                "search": "foo",
                "replace": "foobar"
            }),
            context,
        )
        .await
        .unwrap();

    assert!(result["success"].as_bool().unwrap());
    assert_eq!(result["replacements"].as_u64().unwrap(), 2);
    let content = std::fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, "foobar foobar");
}
