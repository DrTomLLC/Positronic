use crate::airlock::Airlock;
use crate::pty_manager::PtyManager;
use crate::runtime::parser::{CommandParser, CommandType, HiveCommand, IOCommand};
use crate::vault::Vault;

use anyhow::Result;
use chrono::{TimeZone, Utc};
use positronic_hive::HiveNode;
use positronic_io::HardwareMonitor;
use positronic_neural::cortex::{NeuralClient, SystemContext, TaskType};
use positronic_neural::reflex::ReflexEngine;
use positronic_script::wasm_host::WasmHost;

use std::sync::Arc;
use tokio::sync::Mutex;

/// Result of executing a command — tells the UI what to display.
#[derive(Debug, Clone)]
pub enum ExecuteResult {
    /// Command was sent to the PTY shell. UI waits for snapshot redraws.
    SentToPty,
    /// Command produced direct output (bypass PTY). Display these lines.
    DirectOutput(Vec<String>),
    /// Clear the screen. PTY has already been sent cls/clear.
    ClearScreen,
}

const AUTO_CORRECT_THRESHOLD: f64 = 0.8;

/// Valid shell commands that must NEVER trigger Reflex suggestions.
/// Without this whitelist, short commands like `cd` and `ps` match `cp`/`ls`
/// via Levenshtein distance.
const COMMON_COMMANDS: &[&str] = &[
    "cd", "cp", "mv", "rm", "ls", "ps", "id", "df", "du", "dd",
    "ln", "wc", "nl", "od", "bc", "dc", "fc", "fg", "bg", "at",
    "cat", "pwd", "dir", "set", "env", "top", "man", "ssh", "scp",
    "tar", "zip", "apt", "git", "pip", "npm", "sed", "awk", "cut",
    "tee", "dig", "who", "yes",
    "echo", "find", "grep", "sort", "sudo", "curl", "wget", "make",
    "less", "more", "head", "tail", "kill", "ping", "ifconfig",
    "mkdir", "rmdir", "touch", "chmod", "chown", "mount", "which",
    "whoami", "uname", "rustc", "cargo", "python", "python3",
    "node", "code", "dotnet", "java", "ruby", "perl",
    // Windows
    "cls", "type", "copy", "move", "ren", "del", "attrib",
    "ipconfig", "netstat", "tasklist", "taskkill", "chkdsk",
];

#[derive(Debug)]
pub struct Runner {
    pty: Arc<Mutex<PtyManager>>,
    #[allow(dead_code)]
    airlock: Arc<Airlock>,
    neural: Arc<NeuralClient>,
    vault: Vault,
    #[allow(dead_code)]
    wasm_host: Arc<WasmHost>,
    #[allow(dead_code)]
    hive: Arc<HiveNode>,
    #[allow(dead_code)]
    io: Arc<HardwareMonitor>,
    reflex: ReflexEngine,
    /// Current working directory, updated by CWD tracker in main.
    cwd: Arc<Mutex<String>>,
}

impl Runner {
    pub fn new(
        pty: Arc<Mutex<PtyManager>>,
        airlock: Arc<Airlock>,
        neural: Arc<NeuralClient>,
        vault: Vault,
        wasm_host: Arc<WasmHost>,
        hive: Arc<HiveNode>,
        io: Arc<HardwareMonitor>,
    ) -> Self {
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string());
        Self {
            pty,
            airlock,
            neural,
            vault,
            wasm_host,
            hive,
            io,
            reflex: ReflexEngine::new(),
            cwd: Arc::new(Mutex::new(cwd)),
        }
    }

    /// Update the tracked CWD (called from main when CWD changes).
    pub async fn set_cwd(&self, path: &str) {
        let mut cwd = self.cwd.lock().await;
        *cwd = path.to_string();
    }

    /// Get a reference to the CWD lock for external reads.
    pub fn cwd_handle(&self) -> Arc<Mutex<String>> {
        self.cwd.clone()
    }

    /// Build system context for neural prompts.
    async fn build_context(&self) -> SystemContext {
        let cwd = self.cwd.lock().await.clone();
        let recent = self.vault.recent_unique(5).unwrap_or_default();
        SystemContext::gather(&cwd, recent)
    }

    /// Access the vault (for status bar queries from the UI).
    pub fn vault(&self) -> &Vault {
        &self.vault
    }

    pub async fn execute(&self, data: &str) -> Result<ExecuteResult> {
        let normalized = data
            .replace("\r\n", "\n")
            .trim_end_matches('\n')
            .to_string();

        if normalized.trim().is_empty() {
            let mut pty = self.pty.lock().await;
            let _ = pty.write_line("");
            return Ok(ExecuteResult::SentToPty);
        }

        let lower = normalized.trim().to_lowercase();
        if lower == "clear" || lower == "cls" || lower == "!clear" {
            let mut pty = self.pty.lock().await;
            if cfg!(windows) {
                pty.write_line("cls")?;
            } else {
                pty.write_line("clear")?;
            }
            return Ok(ExecuteResult::ClearScreen);
        }

        // ── Alias expansion ──
        // Check if the first word matches an alias before parsing.
        let effective = self.expand_alias(&normalized);

        let parsed = CommandParser::parse(&effective);

        match parsed {
            CommandType::Legacy(cmd) => self.execute_shell_command(&cmd).await,

            CommandType::Native(cmd, args) => {
                let lines = self.handle_native(&cmd, &args).await;
                Ok(ExecuteResult::DirectOutput(lines))
            }

            CommandType::Neural(prompt) => self.handle_neural(&prompt).await,

            CommandType::Sandboxed(_) => Ok(ExecuteResult::DirectOutput(vec![
                "🔒 Airlock sandboxing — not yet implemented.".to_string(),
            ])),

            CommandType::Script(kind, path) => Ok(ExecuteResult::DirectOutput(vec![
                format!("🚀 !{} {} — not yet implemented.", kind, path),
            ])),

            CommandType::Hive(hive_cmd) => {
                let msg = match hive_cmd {
                    HiveCommand::Scan => "📡 Hive peer discovery — not yet implemented.",
                    HiveCommand::Status => "📡 Hive is in loopback simulation mode.",
                    HiveCommand::Chat(_) => "💬 Hive mesh chat — not yet implemented.",
                };
                Ok(ExecuteResult::DirectOutput(vec![msg.to_string()]))
            }

            CommandType::IO(io_cmd) => {
                let msg = match io_cmd {
                    IOCommand::Scan | IOCommand::List => "🔌 Hardware IO — not yet implemented.",
                    IOCommand::Connect(_, _) => "🔌 Serial connection — not yet implemented.",
                };
                Ok(ExecuteResult::DirectOutput(vec![msg.to_string()]))
            }
        }
    }

    /// Expand the first word of input if it matches a stored alias.
    fn expand_alias(&self, input: &str) -> String {
        let trimmed = input.trim();
        if trimmed.starts_with('!') {
            // Don't expand aliases on native commands
            return input.to_string();
        }
        let first_word = trimmed.split_whitespace().next().unwrap_or("");
        match self.vault.get_alias(first_word) {
            Ok(Some(expansion)) => {
                let rest = trimmed.strip_prefix(first_word).unwrap_or("").trim_start();
                if rest.is_empty() {
                    expansion
                } else {
                    format!("{} {}", expansion, rest)
                }
            }
            _ => input.to_string(),
        }
    }

    async fn execute_shell_command(&self, cmd: &str) -> Result<ExecuteResult> {
        // Skip Reflex for known-valid commands — prevents false positives
        // like `cd` → "Did you mean: cp?" and `ps` → "Did you mean: ls?"
        let first_word = cmd.split_whitespace().next().unwrap_or("");
        let is_known = COMMON_COMMANDS.iter().any(|&c| c.eq_ignore_ascii_case(first_word));

        if !is_known {
            if let Some(suggestion) = self.reflex.fix_command(cmd) {
                if suggestion.confidence >= AUTO_CORRECT_THRESHOLD {
                    let lines = vec![format!(
                        "  💡 Auto-corrected → {} ({:.0}%, {:?})",
                        suggestion.corrected,
                        suggestion.confidence * 100.0,
                        suggestion.source
                    )];
                    let _ = self.vault.log_command(&suggestion.corrected, None, None, ".", None);
                    let mut pty = self.pty.lock().await;
                    pty.write_line(&suggestion.corrected)?;
                    return Ok(ExecuteResult::DirectOutput(lines));
                } else {
                    let hint = format!(
                        "  💡 Did you mean: {}? ({:.0}%)",
                        suggestion.corrected,
                        suggestion.confidence * 100.0
                    );
                    let _ = self.vault.log_command(cmd, None, None, ".", None);
                    let mut pty = self.pty.lock().await;
                    pty.write_line(cmd)?;
                    return Ok(ExecuteResult::DirectOutput(vec![hint]));
                }
            }
        }

        let _ = self.vault.log_command(cmd, None, None, ".", None);
        let mut pty = self.pty.lock().await;
        pty.write_line(cmd)?;
        Ok(ExecuteResult::SentToPty)
    }

    async fn handle_neural(&self, prompt: &str) -> Result<ExecuteResult> {
        if prompt.trim().is_empty() {
            return Ok(ExecuteResult::DirectOutput(vec![
                "Usage: !ai <your question>".to_string(),
                "  Example: !ai how do I list files recursively".to_string(),
            ]));
        }

        let task_type = TaskType::classify(prompt, None);
        let context = self.build_context().await;

        let model_hint = match self.neural.select_model(task_type).await {
            Ok(m) => format!(" [{}]", m.split('/').last().unwrap_or(&m)),
            Err(_) => String::new(),
        };

        let mut lines = vec![format!("🧠 Sending to Neural{}...", model_hint)];
        match self.neural.ask_smart(prompt, task_type, Some(&context)).await {
            Ok(response) => {
                for line in response.lines() {
                    lines.push(format!("  {}", line));
                }
            }
            Err(e) => {
                lines.push(format!("❌ Neural error: {}", e));
                lines.push("   Check Lemonade at http://localhost:8000".to_string());
            }
        }
        Ok(ExecuteResult::DirectOutput(lines))
    }

    async fn handle_native(&self, cmd: &str, args: &[String]) -> Vec<String> {
        match cmd {
            // ── Info ──
            "ver" | "version" => vec![
                "⚡ Positronic v0.2.0 — Local-First Terminal".to_string(),
                "  Neural:  http://localhost:8000/api/v1".to_string(),
                "  Reflex:  active (50+ known typos + Levenshtein)".to_string(),
                "  Vault:   SQLite + WAL (aliases, bookmarks, stats)".to_string(),
                "  Themes:  Default, Cyberpunk, Solarized, Monokai".to_string(),
            ],

            "help" => self.help_text(),

            // ── History ──
            "history" => self.cmd_history(args),

            "top" => self.cmd_top(args),

            // ── Aliases ──
            "alias" => self.cmd_alias(args),

            // ── Bookmarks ──
            "bookmark" | "bm" => self.cmd_bookmark(args),

            // ── Stats ──
            "stats" => self.cmd_stats(),

            // ── Export ──
            "export" => self.cmd_export(args),

            // ── Reflex ──
            "fix" => self.cmd_fix(args),

            // ── Config ──
            "set" => self.cmd_set(args),
            "get" => self.cmd_get(args),

            // ── Smart Neural ──
            "explain" => self.cmd_explain(args).await,
            "suggest" => self.cmd_suggest().await,
            "debug" => self.cmd_debug(args).await,

            // ── Hardware IO ──
            "io" => vec![
                "🔌 Hardware IO commands:".to_string(),
                "   !io scan      Discover serial/USB devices".to_string(),
                "   !io list      List known devices".to_string(),
                "   !io connect   Connect to a device".to_string(),
            ],

            other => vec![
                format!("❓ Unknown command: !{}", other),
                "   Type !help for available commands.".to_string(),
            ],
        }
    }

    // ════════════════════════════════════════════════════════════════
    // Command implementations
    // ════════════════════════════════════════════════════════════════

    fn help_text(&self) -> Vec<String> {
        vec![
            "⚡ Positronic Commands:".to_string(),
            String::new(),
            "  CORE:".to_string(),
            "  ─────────────────────────────────────────────────".to_string(),
            "  !ver                       Version info".to_string(),
            "  !help                      This help message".to_string(),
            "  !clear / clear / cls       Clear screen (Ctrl+L)".to_string(),
            "  !pwd                       Show current directory".to_string(),
            "  !theme [name]              Switch color theme".to_string(),
            String::new(),
            "  NEURAL:".to_string(),
            "  ─────────────────────────────────────────────────".to_string(),
            "  !ai <prompt>               Ask Neural (Lemonade)".to_string(),
            "  !ask <prompt>              Alias for !ai".to_string(),
            "  !explain <command>         AI explains a command".to_string(),
            "  !suggest                   AI suggests next command".to_string(),
            "  !debug <error text>        AI troubleshoots an error".to_string(),
            "  !fix <command>             Reflex typo correction".to_string(),
            String::new(),
            "  VAULT:".to_string(),
            "  ─────────────────────────────────────────────────".to_string(),
            "  !history [query]           Search command history".to_string(),
            "  !top [N]                   Most-used commands".to_string(),
            "  !stats                     Vault statistics".to_string(),
            "  !export [limit]            Export history to text".to_string(),
            String::new(),
            "  ALIASES:".to_string(),
            "  ─────────────────────────────────────────────────".to_string(),
            "  !alias                     List all aliases".to_string(),
            "  !alias set <n> <cmd>       Create/update alias".to_string(),
            "  !alias rm <n>              Remove alias".to_string(),
            String::new(),
            "  BOOKMARKS:".to_string(),
            "  ─────────────────────────────────────────────────".to_string(),
            "  !bookmark                  List bookmarks".to_string(),
            "  !bm add <cmd> [-- label]   Bookmark a command".to_string(),
            "  !bm rm <id>               Remove bookmark".to_string(),
            String::new(),
            "  CONFIG:".to_string(),
            "  ─────────────────────────────────────────────────".to_string(),
            "  !set <key> <value>         Set a config value".to_string(),
            "  !get <key>                 Get a config value".to_string(),
            String::new(),
            "  KEYBOARD:".to_string(),
            "  ─────────────────────────────────────────────────".to_string(),
            "  Up / Down                  Command history".to_string(),
            "  Ctrl+L                     Clear screen".to_string(),
            "  Ctrl+C                     Copy output to clipboard".to_string(),
            String::new(),
            "  IN PROGRESS:".to_string(),
            "  ─────────────────────────────────────────────────".to_string(),
            "  !hive / !chat              P2P mesh (loopback only)".to_string(),
            "  !io scan / connect         Hardware IO (stub)".to_string(),
            "  !run / !wasm               Script execution (stub)".to_string(),
            "  sandbox <cmd>              Airlock sandbox (stub)".to_string(),
            String::new(),
            "  Aliases expand automatically. Any other input".to_string(),
            "  goes to your system shell.".to_string(),
        ]
    }

    // ────────────────────────────────────────────────────────────────
    // Smart Neural commands
    // ────────────────────────────────────────────────────────────────

    /// !explain <command> — AI explains what a shell command does.
    async fn cmd_explain(&self, args: &[String]) -> Vec<String> {
        let command = args.join(" ");
        if command.is_empty() {
            return vec![
                "Usage: !explain <command>".to_string(),
                "  Example: !explain git rebase -i HEAD~3".to_string(),
                "  Example: !explain find . -name '*.rs' -exec grep -l 'todo' {} +".to_string(),
            ];
        }

        let mut lines = vec![format!("🧠 Explaining: {}", command)];

        let prompt = format!(
            "Explain this shell command in detail. Break it down part by part. \
             Be concise but thorough.\n\nCommand: {}",
            command
        );

        let context = self.build_context().await;
        match self.neural.ask_smart(&prompt, TaskType::Code, Some(&context)).await {
            Ok(response) => {
                lines.push(String::new());
                for line in response.lines() {
                    lines.push(format!("  {}", line));
                }
            }
            Err(e) => {
                lines.push(format!("❌ Neural error: {}", e));
                lines.push("   Check Lemonade at http://localhost:8000".to_string());
            }
        }
        lines
    }

    /// !suggest — AI suggests the next command based on recent history.
    async fn cmd_suggest(&self) -> Vec<String> {
        // Gather recent history for context
        let recent = match self.vault.recent_unique(15) {
            Ok(cmds) => cmds,
            Err(_) => vec![],
        };

        if recent.is_empty() {
            return vec![
                "🧠 No command history yet. Run some commands first!".to_string(),
                "   !suggest works best after you've been working for a while.".to_string(),
            ];
        }

        let history_context = recent
            .iter()
            .enumerate()
            .map(|(i, cmd)| format!("{}. {}", i + 1, cmd))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "Based on this recent command history from a developer's terminal session, \
             suggest 1-3 commands they might want to run next. For each suggestion, \
             give the command and a brief reason (one line each). \
             Only suggest practical, useful next steps.\n\n\
             Recent commands:\n{}",
            history_context
        );

        let mut lines = vec!["🧠 Analyzing your workflow...".to_string()];

        let context = self.build_context().await;
        match self.neural.ask_smart(&prompt, TaskType::General, Some(&context)).await {
            Ok(response) => {
                lines.push(String::new());
                for line in response.lines() {
                    lines.push(format!("  {}", line));
                }
            }
            Err(e) => {
                lines.push(format!("❌ Neural error: {}", e));
                lines.push("   Check Lemonade at http://localhost:8000".to_string());
            }
        }
        lines
    }

    /// !debug <error text> — AI troubleshoots an error message.
    async fn cmd_debug(&self, args: &[String]) -> Vec<String> {
        let error_text = args.join(" ");
        if error_text.is_empty() {
            return vec![
                "Usage: !debug <error message or text>".to_string(),
                "  Example: !debug EACCES permission denied".to_string(),
                "  Example: !debug cargo build failed with E0308".to_string(),
                String::new(),
                "  Paste any error message and Neural will help troubleshoot.".to_string(),
            ];
        }

        // Include recent commands for context
        let recent = self.vault.recent_unique(5).unwrap_or_default();
        let history_ctx = if recent.is_empty() {
            String::new()
        } else {
            format!(
                "\n\nRecent commands for context:\n{}",
                recent.iter().map(|c| format!("  $ {}", c)).collect::<Vec<_>>().join("\n")
            )
        };

        let prompt = format!(
            "A developer got this error in their terminal. \
             Diagnose the problem and suggest specific fixes. \
             Be concise and practical. Give the exact commands to run if applicable.\n\n\
             Error:\n{}{}",
            error_text,
            history_ctx
        );

        let mut lines = vec!["🧠 Diagnosing...".to_string()];

        let context = self.build_context().await;
        match self.neural.ask_smart(&prompt, TaskType::Debug, Some(&context)).await {
            Ok(response) => {
                lines.push(String::new());
                for line in response.lines() {
                    lines.push(format!("  {}", line));
                }
            }
            Err(e) => {
                lines.push(format!("❌ Neural error: {}", e));
                lines.push("   Check Lemonade at http://localhost:8000".to_string());
            }
        }
        lines
    }

    // ────────────────────────────────────────────────────────────────
    // Vault commands
    // ────────────────────────────────────────────────────────────────

    fn cmd_history(&self, args: &[String]) -> Vec<String> {
        let query = args.join(" ");
        let search = if query.is_empty() { "%" } else { &query };
        match self.vault.search_history(search) {
            Ok(records) => {
                if records.is_empty() {
                    vec!["📜 No history found.".to_string()]
                } else {
                    let mut lines = vec![format!("📜 {} result(s):", records.len())];
                    for r in records.iter().take(25) {
                        let code = r.exit_code
                            .map(|c| format!("{}", c))
                            .unwrap_or_else(|| "·".into());
                        let time = format_timestamp(r.timestamp);
                        let dir_display = if r.directory == "." {
                            String::new()
                        } else {
                            format!(" ({})", short_path(&r.directory))
                        };
                        lines.push(format!(
                            "  [{}] {} {}{}",
                            code, time, r.command, dir_display
                        ));
                    }
                    lines
                }
            }
            Err(e) => vec![format!("❌ History error: {}", e)],
        }
    }

    fn cmd_top(&self, args: &[String]) -> Vec<String> {
        let limit: usize = args.first()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);

        match self.vault.top_commands(limit) {
            Ok(top) => {
                if top.is_empty() {
                    return vec!["📊 No history yet.".to_string()];
                }
                let mut lines = vec![format!("📊 Top {} commands:", top.len())];
                let max_count = top.first().map(|t| t.count).unwrap_or(1);
                for t in &top {
                    let bar_len = ((t.count as f64 / max_count as f64) * 20.0) as usize;
                    let bar: String = "█".repeat(bar_len);
                    lines.push(format!(
                        "  {:>4}x  {}  {}",
                        t.count, bar, t.command
                    ));
                }
                lines
            }
            Err(e) => vec![format!("❌ Error: {}", e)],
        }
    }

    fn cmd_alias(&self, args: &[String]) -> Vec<String> {
        if args.is_empty() {
            match self.vault.list_aliases() {
                Ok(aliases) => {
                    if aliases.is_empty() {
                        return vec![
                            "🔗 No aliases defined.".to_string(),
                            "   Use: !alias set <n> <command>".to_string(),
                        ];
                    }
                    let mut lines = vec![format!("🔗 {} alias(es):", aliases.len())];
                    for a in &aliases {
                        lines.push(format!("  {} → {}", a.name, a.expansion));
                    }
                    lines
                }
                Err(e) => vec![format!("❌ Error: {}", e)],
            }
        } else {
            match args[0].as_str() {
                "set" => {
                    if args.len() < 3 {
                        return vec![
                            "Usage: !alias set <n> <command...>".to_string(),
                            "  Example: !alias set gs git status".to_string(),
                        ];
                    }
                    let name = &args[1];
                    let expansion = args[2..].join(" ");
                    match self.vault.set_alias(name, &expansion) {
                        Ok(()) => vec![format!("✅ Alias set: {} → {}", name, expansion)],
                        Err(e) => vec![format!("❌ Error: {}", e)],
                    }
                }
                "rm" | "remove" | "del" | "delete" => {
                    if args.len() < 2 {
                        return vec!["Usage: !alias rm <n>".to_string()];
                    }
                    match self.vault.remove_alias(&args[1]) {
                        Ok(true) => vec![format!("✅ Alias '{}' removed.", args[1])],
                        Ok(false) => vec![format!("❓ Alias '{}' not found.", args[1])],
                        Err(e) => vec![format!("❌ Error: {}", e)],
                    }
                }
                _ => vec![
                    "Usage: !alias [set <n> <cmd> | rm <n>]".to_string(),
                    "  !alias              List all aliases".to_string(),
                    "  !alias set gs git status".to_string(),
                    "  !alias rm gs".to_string(),
                ],
            }
        }
    }

    fn cmd_bookmark(&self, args: &[String]) -> Vec<String> {
        if args.is_empty() {
            match self.vault.list_bookmarks() {
                Ok(bookmarks) => {
                    if bookmarks.is_empty() {
                        return vec![
                            "🔖 No bookmarks.".to_string(),
                            "   Use: !bm add <command> [-- label]".to_string(),
                        ];
                    }
                    let mut lines = vec![format!("🔖 {} bookmark(s):", bookmarks.len())];
                    for b in &bookmarks {
                        let label = b.label.as_deref().unwrap_or("");
                        if label.is_empty() {
                            lines.push(format!("  [{}] {}", b.id, b.command));
                        } else {
                            lines.push(format!("  [{}] {} — {}", b.id, b.command, label));
                        }
                    }
                    lines
                }
                Err(e) => vec![format!("❌ Error: {}", e)],
            }
        } else {
            match args[0].as_str() {
                "add" => {
                    if args.len() < 2 {
                        return vec!["Usage: !bm add <command> [-- label]".to_string()];
                    }
                    let rest = args[1..].join(" ");
                    let (cmd, label) = if let Some(idx) = rest.find(" -- ") {
                        (&rest[..idx], Some(rest[idx + 4..].trim()))
                    } else {
                        (rest.as_str(), None)
                    };
                    match self.vault.add_bookmark(cmd, label) {
                        Ok(id) => vec![format!("✅ Bookmarked as #{}: {}", id, cmd)],
                        Err(e) => vec![format!("❌ Error: {}", e)],
                    }
                }
                "rm" | "remove" | "del" | "delete" => {
                    if args.len() < 2 {
                        return vec!["Usage: !bm rm <id>".to_string()];
                    }
                    match args[1].parse::<i64>() {
                        Ok(id) => match self.vault.remove_bookmark(id) {
                            Ok(true) => vec![format!("✅ Bookmark #{} removed.", id)],
                            Ok(false) => vec![format!("❓ Bookmark #{} not found.", id)],
                            Err(e) => vec![format!("❌ Error: {}", e)],
                        },
                        Err(_) => vec!["❌ ID must be a number.".to_string()],
                    }
                }
                _ => vec![
                    "Usage: !bm [add <cmd> [-- label] | rm <id>]".to_string(),
                    "  !bm / !bookmark       List bookmarks".to_string(),
                    "  !bm add git log --oneline -- Quick log".to_string(),
                    "  !bm rm 3".to_string(),
                ],
            }
        }
    }

    fn cmd_stats(&self) -> Vec<String> {
        match self.vault.stats() {
            Ok(s) => {
                let uptime = Utc::now().timestamp() - self.vault.start_time();
                let uptime_str = format_duration(uptime);

                let db_size = if s.db_size_bytes > 1_048_576 {
                    format!("{:.1} MB", s.db_size_bytes as f64 / 1_048_576.0)
                } else {
                    format!("{:.1} KB", s.db_size_bytes as f64 / 1024.0)
                };

                let history_span = s.earliest_timestamp
                    .map(|ts| {
                        let days = (Utc::now().timestamp() - ts) / 86400;
                        if days == 0 {
                            "today".to_string()
                        } else {
                            format!("{} day(s)", days)
                        }
                    })
                    .unwrap_or_else(|| "—".to_string());

                vec![
                    "📊 Vault Statistics".to_string(),
                    "  ─────────────────────────────────────────────".to_string(),
                    format!("  Session:        {} ({})", &self.vault.session_id()[..8], uptime_str),
                    format!("  Commands (now):  {}", s.session_commands),
                    format!("  Commands (all):  {} ({} unique)", s.total_commands, s.unique_commands),
                    format!("  Sessions:        {}", s.total_sessions),
                    format!("  Aliases:         {}", s.alias_count),
                    format!("  Bookmarks:       {}", s.bookmark_count),
                    format!("  History span:    {}", history_span),
                    format!("  Database:        {}", db_size),
                ]
            }
            Err(e) => vec![format!("❌ Stats error: {}", e)],
        }
    }

    fn cmd_export(&self, args: &[String]) -> Vec<String> {
        let limit: usize = args.first()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1000);

        match self.vault.export_history(limit) {
            Ok(lines) => {
                if lines.is_empty() {
                    return vec!["📤 No history to export.".to_string()];
                }
                let mut output = vec![format!("📤 Exported {} entries (shell history format):", lines.len())];
                output.push("─────────────────────────────────────────────".to_string());
                for line in &lines {
                    output.push(line.clone());
                }
                output.push("─────────────────────────────────────────────".to_string());
                output.push("  Tip: Copy and paste into ~/.bash_history or similar.".to_string());
                output
            }
            Err(e) => vec![format!("❌ Export error: {}", e)],
        }
    }

    fn cmd_fix(&self, args: &[String]) -> Vec<String> {
        let input = args.join(" ");
        if input.is_empty() {
            return vec![
                "Usage: !fix <command>".to_string(),
                "  Example: !fix gti status".to_string(),
            ];
        }
        match self.reflex.fix_command(&input) {
            Some(s) => vec![
                format!("💡 Suggestion: {}", s.corrected),
                format!("   Confidence: {:.0}%  Source: {:?}", s.confidence * 100.0, s.source),
            ],
            None => vec![format!("✅ No correction needed for: {}", input)],
        }
    }

    fn cmd_set(&self, args: &[String]) -> Vec<String> {
        if args.len() < 2 {
            return vec!["Usage: !set <key> <value>".to_string()];
        }
        let key = &args[0];
        let value = args[1..].join(" ");
        match self.vault.set_config(key, &value) {
            Ok(()) => vec![format!("✅ Set {} = {}", key, value)],
            Err(e) => vec![format!("❌ Error: {}", e)],
        }
    }

    fn cmd_get(&self, args: &[String]) -> Vec<String> {
        if args.is_empty() {
            return vec!["Usage: !get <key>".to_string()];
        }
        match self.vault.get_config(&args[0]) {
            Ok(Some(val)) => vec![format!("  {} = {}", args[0], val)],
            Ok(None) => vec![format!("  {} is not set.", args[0])],
            Err(e) => vec![format!("❌ Error: {}", e)],
        }
    }
}

// ════════════════════════════════════════════════════════════════════
// Formatting helpers
// ════════════════════════════════════════════════════════════════════

fn format_timestamp(ts: i64) -> String {
    Utc.timestamp_opt(ts, 0)
        .single()
        .map(|dt| dt.format("%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "??".into())
}

fn format_duration(secs: i64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

fn short_path(path: &str) -> String {
    if path.len() > 30 {
        format!("…{}", &path[path.len() - 28..])
    } else {
        path.to_string()
    }
}