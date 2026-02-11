use positronic_core::runtime::parser::{CommandParser, CommandType, HiveCommand, IOCommand};
use positronic_core::{PtyCommand, PtyEvent, TerminalBlock};

// ============================================================================
// TerminalBlock Tests
// ============================================================================

#[test]
fn test_terminal_block_creation() {
    let block = TerminalBlock {
        id: 1,
        command: "ls -la".to_string(),
        output: "file1.txt\nfile2.txt".to_string(),
        exit_code: Some(0),
        timestamp: 1700000000,
    };
    assert_eq!(block.id, 1);
    assert_eq!(block.command, "ls -la");
    assert_eq!(block.exit_code, Some(0));
}

#[test]
fn test_terminal_block_clone() {
    let block = TerminalBlock {
        id: 42,
        command: "cargo build".to_string(),
        output: "Compiling...".to_string(),
        exit_code: Some(0),
        timestamp: 1700000000,
    };
    let cloned = block.clone();
    assert_eq!(block.id, cloned.id);
    assert_eq!(block.command, cloned.command);
}

#[test]
fn test_terminal_block_no_exit_code() {
    let block = TerminalBlock {
        id: 0,
        command: "sleep 60".to_string(),
        output: String::new(),
        exit_code: None,
        timestamp: 1700000000,
    };
    assert!(block.exit_code.is_none());
}

#[test]
fn test_terminal_block_serialization() {
    let block = TerminalBlock {
        id: 1,
        command: "echo hello".to_string(),
        output: "hello".to_string(),
        exit_code: Some(0),
        timestamp: 1700000000,
    };
    let json = serde_json::to_string(&block).unwrap();
    let deserialized: TerminalBlock = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.command, "echo hello");
    assert_eq!(deserialized.output, "hello");
}

// ============================================================================
// PtyEvent Tests
// ============================================================================

#[test]
fn test_pty_event_output() {
    let event = PtyEvent::Output(vec![72, 101, 108, 108, 111]);
    match event {
        PtyEvent::Output(bytes) => assert_eq!(bytes, b"Hello"),
        _ => panic!("Expected Output variant"),
    }
}

#[test]
fn test_pty_event_bell() {
    let event = PtyEvent::Bell;
    assert!(matches!(event, PtyEvent::Bell));
}

#[test]
fn test_pty_event_block_finished() {
    let block = TerminalBlock {
        id: 1,
        command: "test".to_string(),
        output: "ok".to_string(),
        exit_code: Some(0),
        timestamp: 0,
    };
    let event = PtyEvent::BlockFinished(block);
    match event {
        PtyEvent::BlockFinished(b) => assert_eq!(b.command, "test"),
        _ => panic!("Expected BlockFinished"),
    }
}

// ============================================================================
// PtyCommand Tests
// ============================================================================

#[test]
fn test_pty_command_input() {
    let cmd = PtyCommand::Input("hello".to_string());
    match cmd {
        PtyCommand::Input(s) => assert_eq!(s, "hello"),
        _ => panic!("Expected Input variant"),
    }
}

#[test]
fn test_pty_command_resize() {
    let cmd = PtyCommand::Resize(120, 40);
    match cmd {
        PtyCommand::Resize(cols, rows) => {
            assert_eq!(cols, 120);
            assert_eq!(rows, 40);
        }
        _ => panic!("Expected Resize variant"),
    }
}

#[test]
fn test_pty_command_execute() {
    let cmd = PtyCommand::Execute("ls".to_string());
    match cmd {
        PtyCommand::Execute(s) => assert_eq!(s, "ls"),
        _ => panic!("Expected Execute variant"),
    }
}

// ============================================================================
// CommandParser Tests
// ============================================================================

#[test]
fn test_parse_legacy_command() {
    let result = CommandParser::parse("ls -la");
    assert_eq!(result, CommandType::Legacy("ls -la".to_string()));
}

#[test]
fn test_parse_legacy_empty_passthrough() {
    let result = CommandParser::parse("");
    assert_eq!(result, CommandType::Legacy("".to_string()));
}

#[test]
fn test_parse_native_ver() {
    let result = CommandParser::parse("!ver");
    assert_eq!(
        result,
        CommandType::Native("ver".to_string(), vec![])
    );
}

#[test]
fn test_parse_native_history() {
    let result = CommandParser::parse("!history git");
    assert_eq!(
        result,
        CommandType::Native("history".to_string(), vec!["git".to_string()])
    );
}

#[test]
fn test_parse_neural_ai() {
    let result = CommandParser::parse("!ai explain git rebase");
    assert_eq!(
        result,
        CommandType::Neural("explain git rebase".to_string())
    );
}

#[test]
fn test_parse_neural_ask() {
    let result = CommandParser::parse("!ask what is rust");
    assert_eq!(
        result,
        CommandType::Neural("what is rust".to_string())
    );
}

#[test]
fn test_parse_script_run() {
    let result = CommandParser::parse("!run ./test.rs");
    assert_eq!(
        result,
        CommandType::Script("run".to_string(), "./test.rs".to_string())
    );
}

#[test]
fn test_parse_script_wasm() {
    let result = CommandParser::parse("!wasm ./plugin.wasm");
    assert_eq!(
        result,
        CommandType::Script("wasm".to_string(), "./plugin.wasm".to_string())
    );
}

#[test]
fn test_parse_sandboxed() {
    let result = CommandParser::parse("sandbox curl example.com");
    assert_eq!(
        result,
        CommandType::Sandboxed("curl example.com".to_string())
    );
}

#[test]
fn test_parse_hive_scan() {
    let result = CommandParser::parse("!hive scan");
    assert_eq!(result, CommandType::Hive(HiveCommand::Scan));
}

#[test]
fn test_parse_hive_status() {
    let result = CommandParser::parse("!hive status");
    assert_eq!(result, CommandType::Hive(HiveCommand::Status));
}

#[test]
fn test_parse_chat() {
    let result = CommandParser::parse("!chat hello world");
    assert_eq!(
        result,
        CommandType::Hive(HiveCommand::Chat("hello world".to_string()))
    );
}

#[test]
fn test_parse_io_scan() {
    let result = CommandParser::parse("!io scan");
    assert_eq!(result, CommandType::IO(IOCommand::Scan));
}

#[test]
fn test_parse_io_list() {
    let result = CommandParser::parse("!io list");
    assert_eq!(result, CommandType::IO(IOCommand::Scan));
}

#[test]
fn test_parse_io_connect() {
    let result = CommandParser::parse("!io connect COM3 115200");
    assert_eq!(
        result,
        CommandType::IO(IOCommand::Connect("COM3".to_string(), 115200))
    );
}

#[test]
fn test_parse_io_connect_invalid_baud() {
    let result = CommandParser::parse("!io connect COM3 notanumber");
    assert_eq!(
        result,
        CommandType::Native(
            "io".to_string(),
            vec![
                "connect".to_string(),
                "COM3".to_string(),
                "notanumber".to_string(),
            ]
        )
    );
}

#[test]
fn test_parse_whitespace_handling() {
    let result = CommandParser::parse("  !ver  ");
    assert_eq!(
        result,
        CommandType::Native("ver".to_string(), vec![])
    );
}

#[test]
fn test_parse_native_unknown_bang_command() {
    let result = CommandParser::parse("!custom arg1 arg2");
    assert_eq!(
        result,
        CommandType::Native(
            "custom".to_string(),
            vec!["arg1".to_string(), "arg2".to_string()]
        )
    );
}

// ============================================================================
// Airlock Tests
// ============================================================================

#[test]
fn test_airlock_creation() {
    let airlock = positronic_core::airlock::Airlock::new();
    assert!(airlock.enabled);
}

#[test]
fn test_airlock_disabled() {
    let airlock = positronic_core::airlock::Airlock { enabled: false };
    assert!(!airlock.enabled);
}

#[tokio::test]
async fn test_airlock_disabled_returns_error() {
    let airlock = positronic_core::airlock::Airlock { enabled: false };
    let result = airlock.run_sandboxed("echo hello").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("disabled"));
}

#[tokio::test]
async fn test_airlock_empty_command_returns_error() {
    let airlock = positronic_core::airlock::Airlock::new();
    let result = airlock.run_sandboxed("").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_airlock_echo_command() {
    let airlock = positronic_core::airlock::Airlock::new();
    let result = airlock.run_sandboxed("echo airlock_test").await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("AIRLOCK SECURE EXECUTION"));
    assert!(output.contains("airlock_test"));
}

// ============================================================================
// Vault Tests
// ============================================================================

#[test]
fn test_vault_open_in_memory() {
    let vault = positronic_core::vault::Vault::open(":memory:");
    assert!(vault.is_ok());
}

#[test]
fn test_vault_log_command() {
    let vault = positronic_core::vault::Vault::open(":memory:").unwrap();
    let result = vault.log_command("ls -la", Some("output"), Some(0), "/home", Some(100));
    assert!(result.is_ok());
}

#[test]
fn test_vault_log_command_null_fields() {
    let vault = positronic_core::vault::Vault::open(":memory:").unwrap();
    let result = vault.log_command("test", None, None, "/tmp", None);
    assert!(result.is_ok());
}

#[test]
fn test_vault_search_history() {
    let vault = positronic_core::vault::Vault::open(":memory:").unwrap();
    vault
        .log_command("git status", None, Some(0), "/repo", None)
        .unwrap();
    vault
        .log_command("git push", None, Some(0), "/repo", None)
        .unwrap();
    vault
        .log_command("cargo build", None, Some(0), "/repo", None)
        .unwrap();

    let results = vault.search_history("git").unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn test_vault_search_history_empty() {
    let vault = positronic_core::vault::Vault::open(":memory:").unwrap();
    let results = vault.search_history("nonexistent").unwrap();
    assert!(results.is_empty());
}

#[test]
fn test_vault_search_returns_most_recent_first() {
    let vault = positronic_core::vault::Vault::open(":memory:").unwrap();
    vault
        .log_command("git first", None, Some(0), "/repo", None)
        .unwrap();
    vault
        .log_command("git second", None, Some(0), "/repo", None)
        .unwrap();

    let results = vault.search_history("git").unwrap();
    assert_eq!(results.len(), 2);
    assert!(results[0].command.contains("second"));
}

#[test]
fn test_vault_command_record_fields() {
    let vault = positronic_core::vault::Vault::open(":memory:").unwrap();
    vault
        .log_command("echo hi", Some("hi"), Some(0), "/tmp", Some(50))
        .unwrap();

    let results = vault.search_history("echo").unwrap();
    assert_eq!(results.len(), 1);
    let record = &results[0];
    assert_eq!(record.command, "echo hi");
    assert_eq!(record.output, Some("hi".to_string()));
    assert_eq!(record.exit_code, Some(0));
    assert_eq!(record.directory, "/tmp");
    assert_eq!(record.duration_ms, Some(50));
}

// ============================================================================
// Vault Schema Tests
// ============================================================================

#[test]
fn test_migration_init_contains_tables() {
    let schema = positronic_core::vault::schema::MIGRATION_INIT;
    assert!(schema.contains("CREATE TABLE IF NOT EXISTS session"));
    assert!(schema.contains("CREATE TABLE IF NOT EXISTS history"));
    assert!(schema.contains("CREATE INDEX IF NOT EXISTS"));
}

// ============================================================================
// StateMachine Tests
// ============================================================================

#[test]
fn test_state_machine_creation() {
    let sm = positronic_core::state_machine::StateMachine::new(80, 24);
    let snapshot = sm.snapshot();
    assert_eq!(snapshot.len(), 24);
    assert_eq!(snapshot[0].len(), 80);
}

#[test]
fn test_state_machine_resize() {
    let sm = positronic_core::state_machine::StateMachine::new(80, 24);
    sm.resize(120, 40);
    let snapshot = sm.snapshot();
    assert_eq!(snapshot.len(), 40);
    assert_eq!(snapshot[0].len(), 120);
}

#[test]
fn test_state_machine_process_ascii() {
    let sm = positronic_core::state_machine::StateMachine::new(80, 24);
    sm.process_bytes(b"Hello");
    let snapshot = sm.snapshot();
    assert_eq!(snapshot[0][0].0, 'H');
    assert_eq!(snapshot[0][1].0, 'e');
    assert_eq!(snapshot[0][2].0, 'l');
    assert_eq!(snapshot[0][3].0, 'l');
    assert_eq!(snapshot[0][4].0, 'o');
}

#[test]
fn test_state_machine_empty_initial_state() {
    let sm = positronic_core::state_machine::StateMachine::new(10, 5);
    let snapshot = sm.snapshot();
    for row in &snapshot {
        for (ch, _) in row {
            assert_eq!(*ch, ' ');
        }
    }
}

// ============================================================================
// MyColor Tests
// ============================================================================

#[test]
fn test_mycolor_default() {
    let color = positronic_core::MyColor::Default;
    assert_eq!(color, positronic_core::MyColor::Default);
}

#[test]
fn test_mycolor_rgb() {
    let color = positronic_core::MyColor::Rgb(255, 128, 0);
    match color {
        positronic_core::MyColor::Rgb(r, g, b) => {
            assert_eq!(r, 255);
            assert_eq!(g, 128);
            assert_eq!(b, 0);
        }
        _ => panic!("Expected Rgb variant"),
    }
}

#[test]
fn test_mycolor_indexed() {
    let color = positronic_core::MyColor::Indexed(42);
    assert_eq!(color, positronic_core::MyColor::Indexed(42));
}

#[test]
fn test_mycolor_named_colors() {
    let colors = vec![
        positronic_core::MyColor::Black,
        positronic_core::MyColor::Red,
        positronic_core::MyColor::Green,
        positronic_core::MyColor::Yellow,
        positronic_core::MyColor::Blue,
        positronic_core::MyColor::Magenta,
        positronic_core::MyColor::Cyan,
        positronic_core::MyColor::White,
        positronic_core::MyColor::BrightBlack,
        positronic_core::MyColor::BrightRed,
        positronic_core::MyColor::BrightGreen,
        positronic_core::MyColor::BrightYellow,
        positronic_core::MyColor::BrightBlue,
        positronic_core::MyColor::BrightMagenta,
        positronic_core::MyColor::BrightCyan,
        positronic_core::MyColor::BrightWhite,
    ];
    assert_eq!(colors.len(), 16);
    for (i, c1) in colors.iter().enumerate() {
        for (j, c2) in colors.iter().enumerate() {
            if i != j {
                assert_ne!(c1, c2);
            }
        }
    }
}

#[test]
fn test_mycolor_clone_eq() {
    let a = positronic_core::MyColor::Rgb(10, 20, 30);
    let b = a.clone();
    assert_eq!(a, b);
}
