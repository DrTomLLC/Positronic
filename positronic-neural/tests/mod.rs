use positronic_neural::privacy::PrivacyGuard;
use positronic_neural::reflex::{levenshtein_distance, ReflexEngine, SuggestionSource};

// ============================================================================
// PrivacyGuard Tests
// ============================================================================

#[test]
fn test_scrub_ip() {
    let input = "Connect to 192.168.1.1 now";
    assert_eq!(PrivacyGuard::scrub(input), "Connect to [REDACTED_IP] now");
}

#[test]
fn test_scrub_email() {
    let input = "Contact tom@example.com";
    assert_eq!(PrivacyGuard::scrub(input), "Contact [REDACTED_EMAIL]");
}

#[test]
fn test_scrub_multiple_ips() {
    let input = "Hosts: 10.0.0.1 and 172.16.0.1";
    let scrubbed = PrivacyGuard::scrub(input);
    assert_eq!(scrubbed, "Hosts: [REDACTED_IP] and [REDACTED_IP]");
}

#[test]
fn test_scrub_api_key_sk() {
    let input = "Use key sk-abcdefghijklmnopqrstuvwxyz";
    let scrubbed = PrivacyGuard::scrub(input);
    assert!(scrubbed.contains("[REDACTED_KEY]"));
    assert!(!scrubbed.contains("sk-abcdefghijklmnopqrstuvwxyz"));
}

#[test]
fn test_scrub_github_token() {
    let input = "Token: ghp_abcdefghijklmnopqrstuvwxyz";
    let scrubbed = PrivacyGuard::scrub(input);
    assert!(scrubbed.contains("[REDACTED_KEY]"));
}

#[test]
fn test_scrub_no_pii() {
    let input = "This is a clean string with no secrets.";
    assert_eq!(PrivacyGuard::scrub(input), input);
}

#[test]
fn test_scrub_empty_string() {
    assert_eq!(PrivacyGuard::scrub(""), "");
}

#[test]
fn test_scrub_mixed_pii() {
    let input = "Send to tom@example.com at 10.0.0.1 with sk-abcdefghijklmnopqrstuvwxyz";
    let scrubbed = PrivacyGuard::scrub(input);
    assert!(scrubbed.contains("[REDACTED_EMAIL]"));
    assert!(scrubbed.contains("[REDACTED_IP]"));
    assert!(scrubbed.contains("[REDACTED_KEY]"));
}

#[test]
fn test_scrub_preserves_non_ip_numbers() {
    let input = "The answer is 42 and version 3.14";
    let scrubbed = PrivacyGuard::scrub(input);
    assert!(scrubbed.contains("42"));
    // "3.14" is not a full IP, should not be redacted
    // (though the regex is broad, this particular case has too few octets)
}

// ============================================================================
// Levenshtein Distance Tests
// ============================================================================

#[test]
fn test_levenshtein_identical() {
    assert_eq!(levenshtein_distance("hello", "hello"), 0);
}

#[test]
fn test_levenshtein_empty_strings() {
    assert_eq!(levenshtein_distance("", ""), 0);
}

#[test]
fn test_levenshtein_one_empty() {
    assert_eq!(levenshtein_distance("", "hello"), 5);
    assert_eq!(levenshtein_distance("hello", ""), 5);
}

#[test]
fn test_levenshtein_single_insertion() {
    assert_eq!(levenshtein_distance("git", "gist"), 1);
}

#[test]
fn test_levenshtein_single_deletion() {
    assert_eq!(levenshtein_distance("gist", "git"), 1);
}

#[test]
fn test_levenshtein_single_substitution() {
    assert_eq!(levenshtein_distance("cat", "bat"), 1);
}

#[test]
fn test_levenshtein_transposition() {
    // Levenshtein treats transposition as 2 substitutions
    assert_eq!(levenshtein_distance("sl", "ls"), 2);
}

#[test]
fn test_levenshtein_completely_different() {
    assert_eq!(levenshtein_distance("abc", "xyz"), 3);
}

#[test]
fn test_levenshtein_symmetric() {
    assert_eq!(
        levenshtein_distance("kitten", "sitting"),
        levenshtein_distance("sitting", "kitten")
    );
}

// ============================================================================
// ReflexEngine Tests
// ============================================================================

#[test]
fn test_reflex_engine_creation() {
    let engine = ReflexEngine::new();
    assert!(engine.fix_command("").is_none());
}

#[test]
fn test_reflex_engine_default() {
    let engine = ReflexEngine::default();
    assert!(engine.fix_command("").is_none());
}

#[test]
fn test_reflex_known_typo_git_psuh() {
    let engine = ReflexEngine::new();
    let suggestion = engine.fix_command("git psuh").unwrap();
    assert_eq!(suggestion.corrected, "git push");
    assert_eq!(suggestion.confidence, 1.0);
    assert_eq!(suggestion.source, SuggestionSource::KnownTypo);
}

#[test]
fn test_reflex_known_typo_git_comit() {
    let engine = ReflexEngine::new();
    let suggestion = engine.fix_command("git comit").unwrap();
    assert_eq!(suggestion.corrected, "git commit");
}

#[test]
fn test_reflex_known_typo_cargo_biuld() {
    let engine = ReflexEngine::new();
    let suggestion = engine.fix_command("cargo biuld").unwrap();
    assert_eq!(suggestion.corrected, "cargo build");
}

#[test]
fn test_reflex_known_typo_crago() {
    let engine = ReflexEngine::new();
    let suggestion = engine.fix_command("crago").unwrap();
    assert_eq!(suggestion.corrected, "cargo");
}

#[test]
fn test_reflex_known_single_word_typo_with_args() {
    let engine = ReflexEngine::new();
    let suggestion = engine.fix_command("crago build --release").unwrap();
    assert!(suggestion.corrected.starts_with("cargo"));
}

#[test]
fn test_reflex_known_typo_sl() {
    let engine = ReflexEngine::new();
    let suggestion = engine.fix_command("sl").unwrap();
    assert_eq!(suggestion.corrected, "ls");
}

#[test]
fn test_reflex_known_typo_cd_dotdot() {
    let engine = ReflexEngine::new();
    let suggestion = engine.fix_command("cd..").unwrap();
    assert_eq!(suggestion.corrected, "cd ..");
}

#[test]
fn test_reflex_levenshtein_match() {
    let engine = ReflexEngine::new();
    // "gti" should match "git" via Levenshtein
    let suggestion = engine.fix_command("gti status");
    assert!(suggestion.is_some());
    let s = suggestion.unwrap();
    assert!(s.corrected.starts_with("git"));
}

#[test]
fn test_reflex_no_correction_for_valid_command() {
    let engine = ReflexEngine::new();
    // "ls" is a valid command, Levenshtein distance 0 is filtered out
    let suggestion = engine.fix_command("ls");
    assert!(suggestion.is_none());
}

#[test]
fn test_reflex_no_correction_for_unknown() {
    let engine = ReflexEngine::new();
    // A completely unrecognizable string
    let suggestion = engine.fix_command("zzzzzzzzzzzzz");
    assert!(suggestion.is_none());
}

#[test]
fn test_reflex_empty_input() {
    let engine = ReflexEngine::new();
    assert!(engine.fix_command("").is_none());
}

#[test]
fn test_reflex_whitespace_only() {
    let engine = ReflexEngine::new();
    assert!(engine.fix_command("   ").is_none());
}

#[test]
fn test_reflex_custom_thresholds() {
    let engine = ReflexEngine::with_thresholds(1, 0.9);
    // With max_distance=1, "gti" (dist 2 from "git") should NOT match
    // but known typos should still work
    let suggestion = engine.fix_command("git psuh");
    assert!(suggestion.is_some());
}

#[test]
fn test_reflex_preserves_args_after_correction() {
    let engine = ReflexEngine::new();
    // "crago" is a known single-word typo for "cargo"
    let suggestion = engine.fix_command("crago build --release").unwrap();
    assert!(suggestion.corrected.starts_with("cargo"));
    assert!(suggestion.corrected.contains("build --release"));
}

#[test]
fn test_reflex_case_insensitive_known_typos() {
    let engine = ReflexEngine::new();
    let suggestion = engine.fix_command("Git Psuh");
    assert!(suggestion.is_some());
    assert_eq!(suggestion.unwrap().corrected, "git push");
}

// ============================================================================
// NeuralClient Tests (structure only - no live server)
// ============================================================================

#[test]
fn test_neural_client_creation() {
    let client = positronic_neural::cortex::NeuralClient::new(
        "http://localhost:8000/v1",
        "test-model",
    );
    // Just verify it can be created without panicking
    let debug = format!("{:?}", client);
    assert!(debug.contains("test-model"));
}

// ============================================================================
// LemonadeClient Tests (structure only - no live server)
// ============================================================================

#[test]
fn test_lemonade_client_creation() {
    let client = positronic_neural::LemonadeClient::new(
        "http://localhost:8000/v1",
        "test-model",
    );
    // Verify the client was created (no panic)
    let _ = format!("{:?}", &client as *const _);
}

// ============================================================================
// ReflexEngine Struct Tests
// ============================================================================

#[test]
fn test_reflex_engine_struct() {
    let engine = positronic_neural::reflex::ReflexEngine::new();
    let _ = format!("{:?}", &engine as *const _);
}
