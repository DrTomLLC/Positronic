use std::path::Path;

// ============================================================================
// execute_script Tests
// ============================================================================

#[tokio::test]
async fn test_execute_script_missing_file() {
    let path = Path::new("/tmp/nonexistent_positronic_test_script.rs");
    let result = positronic_script::execute_script(path, &[]).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("not found"));
}

#[tokio::test]
async fn test_execute_script_empty_path() {
    let path = Path::new("");
    let result = positronic_script::execute_script(path, &[]).await;
    assert!(result.is_err());
}

// ============================================================================
// WasmHost Tests
// ============================================================================

#[test]
fn test_wasm_host_creation() {
    let host = positronic_script::wasm_host::WasmHost::new();
    assert!(host.is_ok());
}

#[test]
fn test_wasm_host_debug() {
    let host = positronic_script::wasm_host::WasmHost::new().unwrap();
    let debug = format!("{:?}", host);
    assert!(debug.contains("WasmHost"));
}

#[test]
fn test_wasm_host_invalid_bytes() {
    let host = positronic_script::wasm_host::WasmHost::new().unwrap();
    // Invalid WASM bytes should fail
    let result = host.run_plugin(b"not valid wasm");
    assert!(result.is_err());
}

#[test]
fn test_wasm_host_empty_bytes() {
    let host = positronic_script::wasm_host::WasmHost::new().unwrap();
    let result = host.run_plugin(b"");
    assert!(result.is_err());
}

#[test]
fn test_wasm_host_valid_minimal_module() {
    let host = positronic_script::wasm_host::WasmHost::new().unwrap();
    // Minimal valid WASM module (magic + version + empty)
    let minimal_wasm = [
        0x00, 0x61, 0x73, 0x6D, // magic: \0asm
        0x01, 0x00, 0x00, 0x00, // version: 1
    ];
    // This should parse as valid WASM but fail to find _start
    let result = host.run_plugin(&minimal_wasm);
    assert!(result.is_err());
}
