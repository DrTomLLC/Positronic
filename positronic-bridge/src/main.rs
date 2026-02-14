use iced::futures::SinkExt;
use iced::widget::{column, container, rich_text, row, scrollable, span, text, text_input};
use iced::{event, keyboard, Color, Element, Font, Length, Settings, Subscription, Task, Theme};

use positronic_core::runner::ExecuteResult;
use positronic_core::state_machine::Snapshot;
use positronic_core::PositronicEngine;

mod renderer;
mod completer;
use renderer::ThemeName;

use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

/// Max direct output buffer (~256KB). Oldest half trimmed when exceeded.
const MAX_DIRECT_BYTES: usize = 256 * 1024;

pub fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .init();

    eprintln!("=== Positronic Starting ===");

    iced::application(boot, update, view)
        .title(title)
        .theme(theme)
        .subscription(subscription)
        .settings(Settings {
            antialiasing: true,
            ..Settings::default()
        })
        .run()
}

// ====================================================================
// State
// ====================================================================

#[derive(Debug, Clone, PartialEq)]
enum AppState {
    Booting,
    Active,
    Error(String),
}

struct PositronicApp {
    engine: Option<Arc<PositronicEngine>>,
    redraw: Option<RedrawHandle>,

    /// Accumulated direct output (echoed commands, ! command output).
    direct_output: String,

    /// The latest PTY snapshot (with color data).
    last_snapshot: Option<Snapshot>,

    /// Command history and cursor for Up/Down navigation.
    cmd_history: Vec<String>,
    history_cursor: Option<usize>,

    /// Status bar data
    session_cmd_count: usize,
    boot_instant: std::time::Instant,

    /// Current working directory (best effort tracking)
    cwd: String,

    /// Active color theme
    theme_name: ThemeName,

    /// Tab completion state (active while cycling through completions)
    tab_state: Option<completer::CompletionState>,

    input: String,
    state: AppState,
    last_screen_hash: u64,
}

#[derive(Clone, Debug)]
struct RedrawHandle(Arc<Mutex<mpsc::Receiver<()>>>);

impl PartialEq for RedrawHandle {
    fn eq(&self, other: &Self) -> bool {
        Arc::as_ptr(&self.0) == Arc::as_ptr(&other.0)
    }
}
impl Eq for RedrawHandle {}
impl Hash for RedrawHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.0) as usize).hash(state);
    }
}

// ====================================================================
// Messages
// ====================================================================

#[derive(Clone, Debug)]
enum Message {
    EngineReady(Arc<PositronicEngine>, RedrawHandle),
    EngineFailed(String),
    Redraw,
    InputChanged(String),
    InputSent,
    CommandResult(ExecuteResult),
    CommandError(String),
    HistoryUp,
    HistoryDown,
    ClearScreen,
    CopyToClipboard,
    TabComplete,
    WindowResized(u32, u32),
    Tick,
    ThemeChanged(ThemeName),
}

// ====================================================================
// Output management
// ====================================================================

fn push_direct(app: &mut PositronicApp, new_text: &str) {
    app.direct_output.push_str(new_text);
    if !new_text.ends_with('\n') {
        app.direct_output.push('\n');
    }

    if app.direct_output.len() > MAX_DIRECT_BYTES {
        let mid = app.direct_output.len() / 2;
        if let Some(nl) = app.direct_output[mid..].find('\n') {
            let trim_at = mid + nl + 1;
            let kept = app.direct_output[trim_at..].to_string();
            app.direct_output = format!("··· (older output trimmed) ···\n{}", kept);
        }
    }
}

// ====================================================================
// Boot
// ====================================================================

fn boot() -> (PositronicApp, Task<Message>) {
    let app = PositronicApp {
        engine: None,
        redraw: None,
        direct_output: "⏳ Booting Positronic Engine...\n".to_string(),
        last_snapshot: None,
        cmd_history: Vec::new(),
        history_cursor: None,
        session_cmd_count: 0,
        boot_instant: std::time::Instant::now(),
        cwd: std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string()),
        theme_name: ThemeName::Default,
        tab_state: None,
        input: String::new(),
        state: AppState::Booting,
        last_screen_hash: 0,
    };

    let task = Task::perform(
        async {
            let (tx, rx) = mpsc::channel(100);
            match PositronicEngine::start(80, 24, tx).await {
                Ok(e) => Ok((Arc::new(e), RedrawHandle(Arc::new(Mutex::new(rx))))),
                Err(e) => Err(format!("{:#}", e)),
            }
        },
        |r| match r {
            Ok((e, h)) => Message::EngineReady(e, h),
            Err(s) => Message::EngineFailed(s),
        },
    );

    (app, task)
}

fn title(app: &PositronicApp) -> String {
    match &app.state {
        AppState::Booting => "Positronic /// Booting...".into(),
        AppState::Active => "Positronic /// Data Surface".into(),
        AppState::Error(e) => format!("Positronic /// ERROR: {}", &e[..e.len().min(60)]),
    }
}

fn theme(_: &PositronicApp) -> Theme {
    Theme::Dark
}

// ====================================================================
// Update
// ====================================================================

fn update(app: &mut PositronicApp, message: Message) -> Task<Message> {
    match message {
        Message::EngineReady(engine, redraw) => {
            eprintln!("[UI] Engine ready!");

            // Hydrate persistent history from the Vault
            match engine.runner.vault().recent_unique(100) {
                Ok(history) => {
                    app.cmd_history = history.into_iter().rev().collect();
                    eprintln!("[UI] Hydrated {} commands from Vault.", app.cmd_history.len());
                }
                Err(e) => eprintln!("[UI] Failed to load history: {}", e),
            }

            app.engine = Some(engine.clone());
            app.redraw = Some(redraw);
            app.state = AppState::Active;
            app.direct_output.clear();
            app.last_snapshot = None;

            // Load saved theme from vault config
            if let Ok(Some(saved_theme)) = engine.runner.vault().get_config("theme") {
                if let Some(t) = ThemeName::from_str(&saved_theme) {
                    app.theme_name = t;
                }
            }

            push_direct(app, "⚡ Positronic v0.2.0 Online.  Type !help for commands.");
            Task::none()
        }

        Message::EngineFailed(err) => {
            app.state = AppState::Error(err.clone());
            push_direct(app, &format!("❌ BOOT FAILED: {}", err));
            Task::none()
        }

        Message::Redraw => {
            if let Some(engine) = &app.engine {
                let snapshot = engine.state.snapshot();
                let new_hash = hash_snapshot(&snapshot);

                if new_hash != app.last_screen_hash {
                    app.last_screen_hash = new_hash;

                    // ── CWD tracking: parse the last non-empty line for prompt patterns ──
                    update_cwd_from_snapshot(&snapshot, &mut app.cwd);

                    app.last_snapshot = Some(snapshot);
                }
            }
            Task::none()
        }

        Message::InputChanged(val) => {
            app.input = val;
            app.history_cursor = None;
            app.tab_state = None; // Reset tab completion on any keystroke
            Task::none()
        }

        Message::InputSent => {
            let trimmed = app.input.trim().to_string();
            if trimmed.is_empty() {
                return Task::none();
            }

            app.cmd_history.push(trimmed.clone());
            app.history_cursor = None;
            app.tab_state = None;
            app.session_cmd_count += 1;

            push_direct(app, &format!("➜ {}", trimmed));
            app.input.clear();

            // ── CWD tracking: detect cd commands ──
            track_cd_command(&trimmed, &mut app.cwd);

            // ── Local theme handling (no runner round-trip needed) ──
            if trimmed.starts_with("!theme") {
                let args: Vec<&str> = trimmed.split_whitespace().collect();
                if args.len() < 2 {
                    let names: Vec<&str> = ThemeName::all().iter().map(|t| t.label()).collect();
                    push_direct(app, &format!("🎨 Current theme: {}", app.theme_name.label()));
                    push_direct(app, &format!("   Available: {}", names.join(", ")));
                } else if let Some(new_theme) = ThemeName::from_str(args[1]) {
                    app.theme_name = new_theme;
                    push_direct(app, &format!("🎨 Theme switched to: {}", new_theme.label()));
                    // Persist to vault config
                    if let Some(engine) = &app.engine {
                        let _ = engine.runner.vault().set_config("theme", new_theme.label());
                    }
                } else {
                    push_direct(app, &format!("❌ Unknown theme: {}", args[1]));
                    let names: Vec<&str> = ThemeName::all().iter().map(|t| t.label()).collect();
                    push_direct(app, &format!("   Available: {}", names.join(", ")));
                }
                return Task::none();
            }

            // ── Local !pwd handling ──
            if trimmed == "!pwd" {
                push_direct(app, &format!("📂 {}", app.cwd));
                return Task::none();
            }

            let Some(engine) = app.engine.clone() else {
                push_direct(app, "❌ Engine not ready.");
                return Task::none();
            };

            // Log to vault
            let _ = engine.runner.vault().log_command(&trimmed, None, None, &app.cwd, None);

            Task::perform(
                async move { engine.send_input(&format!("{}\n", trimmed)).await },
                move |r| match r {
                    Ok(result) => Message::CommandResult(result),
                    Err(e) => Message::CommandError(format!("{:#}", e)),
                },
            )
        }

        Message::CommandResult(result) => {
            match result {
                ExecuteResult::SentToPty => {}
                ExecuteResult::DirectOutput(lines) => {
                    push_direct(app, &lines.join("\n"));
                }
                ExecuteResult::ClearScreen => {
                    app.direct_output.clear();
                    app.last_snapshot = None;
                }
            }
            Task::none()
        }

        Message::CommandError(err) => {
            push_direct(app, &format!("❌ {}", err));
            Task::none()
        }

        Message::HistoryUp => {
            if app.cmd_history.is_empty() {
                return Task::none();
            }
            let idx = match app.history_cursor {
                Some(i) if i > 0 => i - 1,
                Some(i) => i,
                None => app.cmd_history.len() - 1,
            };
            app.history_cursor = Some(idx);
            app.input = app.cmd_history[idx].clone();
            app.tab_state = None;
            Task::none()
        }

        Message::HistoryDown => {
            let Some(cursor) = app.history_cursor else {
                return Task::none();
            };
            if cursor + 1 < app.cmd_history.len() {
                let idx = cursor + 1;
                app.history_cursor = Some(idx);
                app.input = app.cmd_history[idx].clone();
            } else {
                app.history_cursor = None;
                app.input.clear();
            }
            app.tab_state = None;
            Task::none()
        }

        Message::ClearScreen => {
            let Some(engine) = app.engine.clone() else {
                return Task::none();
            };
            app.direct_output.clear();
            app.last_snapshot = None;
            Task::perform(
                async move { engine.send_input("cls\n").await },
                |r| match r {
                    Ok(_) => Message::Redraw,
                    Err(e) => Message::CommandError(format!("{:#}", e)),
                },
            )
        }

        Message::CopyToClipboard => {
            // Build the full plain-text content for clipboard
            let mut clipboard_text = app.direct_output.clone();
            if let Some(ref snapshot) = app.last_snapshot {
                let pty_text = renderer::snapshot_to_plain(snapshot);
                if !pty_text.is_empty() {
                    if !clipboard_text.is_empty() && !clipboard_text.ends_with('\n') {
                        clipboard_text.push('\n');
                    }
                    clipboard_text.push_str(&pty_text);
                }
            }

            // Copy to system clipboard
            if let Ok(mut ctx) = copypasta::ClipboardContext::new() {
                use copypasta::ClipboardProvider;
                let _ = ctx.set_contents(clipboard_text);
                push_direct(app, "  📋 Copied to clipboard.");
            } else {
                push_direct(app, "  ⚠ Clipboard unavailable.");
            }
            Task::none()
        }

        // ── Tab Completion ──
        Message::TabComplete => {
            if app.input.trim().is_empty() {
                return Task::none();
            }

            if let Some(ref mut state) = app.tab_state {
                // Already cycling — advance to the next completion
                let next = state.next().to_string();
                app.input = next;
            } else {
                // First Tab press — generate completions
                let aliases = get_alias_names(app);
                if let Some(state) = completer::complete(&app.input, &aliases, &app.cwd) {
                    let first = state.current().to_string();
                    let count = state.len();
                    app.tab_state = Some(state);
                    app.input = first;

                    // Show hint if multiple completions
                    if count > 1 {
                        let all: Vec<String> = app.tab_state.as_ref().unwrap()
                            .completions.iter()
                            .map(|c| {
                                // Show just the last token for brevity
                                c.rsplit_once(' ').map(|(_, r)| r)
                                    .or_else(|| c.strip_prefix('!'))
                                    .unwrap_or(c)
                                    .to_string()
                            })
                            .collect();
                        push_direct(app, &format!("  💡 {} matches: {}", count, all.join("  ")));
                    }
                }
            }
            Task::none()
        }

        Message::WindowResized(width, _height) => {
            let cols = ((width as f32 - 20.0) / 8.0).max(40.0) as u16;
            let rows = 24u16;
            if let Some(engine) = app.engine.clone() {
                Task::perform(
                    async move { let _ = engine.resize(cols, rows).await; },
                    |_| Message::Redraw,
                )
            } else {
                Task::none()
            }
        }

        Message::Tick => Task::none(),

        Message::ThemeChanged(new_theme) => {
            app.theme_name = new_theme;
            Task::none()
        }
    }
}

/// Retrieve alias names from the vault (for tab completion).
fn get_alias_names(app: &PositronicApp) -> Vec<String> {
    let Some(engine) = &app.engine else {
        return vec![];
    };
    match engine.runner.vault().list_aliases() {
        Ok(aliases) => aliases.into_iter().map(|alias| alias.name).collect(),
        Err(_) => vec![],
    }
}

// ====================================================================
// CWD Tracking
// ====================================================================

/// Track `cd` / `pushd` / `Set-Location` commands to update CWD.
fn track_cd_command(cmd: &str, cwd: &mut String) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return;
    }

    let is_cd = matches!(
        parts[0].to_lowercase().as_str(),
        "cd" | "chdir" | "pushd" | "set-location" | "sl"
    );

    if !is_cd || parts.len() < 2 {
        return;
    }

    let target = parts[1..].join(" ");
    // Strip surrounding quotes
    let target = target.trim_matches('"').trim_matches('\'');

    if target == "-" || target == "~" {
        // Special cases — we can't resolve these without the shell,
        // so we'll let the PTY snapshot update pick it up.
        return;
    }

    let path = std::path::Path::new(target);
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::path::Path::new(cwd.as_str()).join(path)
    };

    // Only update if the directory actually exists
    if let Ok(canonical) = resolved.canonicalize() {
        *cwd = canonical.to_string_lossy().to_string();
    }
}

/// Try to extract CWD from the PTY snapshot by looking for common prompt patterns.
/// Looks for patterns like:
///   PS C:\Users\Doctor> _
///   user@host:~/project$ _
///   C:\Users\Doctor>
fn update_cwd_from_snapshot(snapshot: &Snapshot, cwd: &mut String) {
    let rows = snapshot.rows();
    if rows == 0 {
        return;
    }

    // Scan from the bottom for the first non-empty line (likely the prompt)
    for row_idx in (0..rows).rev() {
        let row = &snapshot[row_idx];
        let line: String = row.iter().map(|(c, _)| *c).collect();
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        // ── PowerShell: PS C:\path> or PS C:\path>  ──
        if let Some(rest) = trimmed.strip_prefix("PS ") {
            if let Some(path) = rest.strip_suffix('>').or_else(|| {
                // Could have trailing spaces before cursor
                rest.split('>').next()
            }) {
                let path = path.trim();
                if !path.is_empty() && (path.contains('\\') || path.contains('/') || path.starts_with('~')) {
                    let resolved = resolve_tilde(path);
                    if std::path::Path::new(&resolved).is_dir() || resolved.contains('\\') || resolved.contains('/') {
                        *cwd = resolved;
                    }
                }
            }
            return;
        }

        // ── Unix bash/zsh: user@host:~/path$ or ~/path$ ──
        // Look for `:` followed by path and `$` or `#`
        if let Some(colon_pos) = trimmed.find(':') {
            let after_colon = &trimmed[colon_pos + 1..];
            if let Some(prompt_end) = after_colon.rfind(|c: char| c == '$' || c == '#') {
                let path = after_colon[..prompt_end].trim();
                if !path.is_empty() {
                    let resolved = resolve_tilde(path);
                    *cwd = resolved;
                    return;
                }
            }
        }

        // ── cmd.exe: C:\path> ──
        if trimmed.ends_with('>') && trimmed.len() >= 3 {
            let path = &trimmed[..trimmed.len() - 1];
            // Check if it looks like a drive path (e.g., C:\...)
            if path.len() >= 2 && path.as_bytes()[1] == b':' {
                *cwd = path.to_string();
            }
        }

        // Only check the last non-empty line (the prompt)
        return;
    }
}

/// Replace leading ~ with the home directory.
fn resolve_tilde(path: &str) -> String {
    if path.starts_with('~') {
        if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
            return format!("{}{}", home, &path[1..]);
        }
    }
    path.to_string()
}

// ====================================================================
// View
// ====================================================================

fn view(app: &PositronicApp) -> Element<'_, Message> {
    let mut layout = column![]
        .spacing(0)
        .padding(0)
        .width(Length::Fill)
        .height(Length::Fill);

    // ── Boot / Error messages ──
    match &app.state {
        AppState::Booting => {
            layout = layout.push(
                container(
                    text("⏳ Booting engine...")
                        .font(Font::MONOSPACE)
                        .size(14)
                ).padding(10),
            );
        }
        AppState::Error(e) => {
            layout = layout.push(
                container(
                    text(format!("❌ {}", e))
                        .font(Font::MONOSPACE)
                        .size(14)
                        .color(Color::from_rgb(1.0, 0.4, 0.4))
                ).padding(10),
            );
        }
        AppState::Active => {}
    }

    // ── Colored output area ──
    // Build spans from direct output + PTY snapshot
    let mut all_spans: Vec<iced::widget::text::Span<'static>> = Vec::new();

    // Direct output (! command results, echoed commands)
    if !app.direct_output.is_empty() {
        all_spans.extend(renderer::direct_to_spans(&app.direct_output));
    }

    // PTY snapshot (colored terminal output from the shell)
    if let Some(ref snapshot) = app.last_snapshot {
        // Separator between direct output and PTY
        if !app.direct_output.is_empty() {
            all_spans.push(span("\n"));
        }
        all_spans.extend(renderer::snapshot_to_spans(snapshot, app.theme_name));
    }

    // If no spans at all, show a placeholder
    if all_spans.is_empty() {
        all_spans.push(
            span("").color(Color::from_rgb(0.5, 0.5, 0.5)),
        );
    }

    let terminal_display = rich_text(all_spans)
        .font(Font::MONOSPACE)
        .size(14);

    // FIX: Use .anchor_bottom() instead of snap_to/Id which don't exist in iced 0.14
    let scrollable_output = scrollable(
        container(terminal_display)
            .padding([5, 10])
            .width(Length::Fill),
    )
        .anchor_bottom()
        .height(Length::Fill)
        .width(Length::Fill);

    layout = layout.push(scrollable_output);

    // ── Status bar ──
    let uptime_secs = app.boot_instant.elapsed().as_secs() as i64;
    let uptime_str = format_duration_short(uptime_secs);
    let short_cwd = short_path(&app.cwd);

    // Show tab hint if cycling
    let tab_hint = if let Some(ref state) = app.tab_state {
        format!("  │  Tab {}/{}", state.completions.iter().position(|c| c == state.current()).map(|i| i + 1).unwrap_or(1), state.len())
    } else {
        String::new()
    };

    let status_text = format!(
        " ⚡ {} cmd  │  ⏱ {}  │  📂 {}  │  🎨 {}{}  │  Positronic v0.2.0",
        app.session_cmd_count,
        uptime_str,
        short_cwd,
        app.theme_name.label(),
        tab_hint,
    );

    let status_bar = container(
        text(status_text)
            .font(Font::MONOSPACE)
            .size(12)
            .color(Color::from_rgb(0.5, 0.55, 0.6))
    )
        .width(Length::Fill)
        .padding([3.0, 12.0])
        .style(status_bar_style);

    layout = layout.push(status_bar);

    // ── Input bar ──
    let input = text_input("Type a command… (!help for commands)", &app.input)
        .font(Font::MONOSPACE)
        .size(14)
        .padding(10)
        .on_input(Message::InputChanged)
        .on_submit(Message::InputSent);

    layout = layout.push(
        container(row![input].width(Length::Fill)).padding([10, 10])
    );

    layout.into()
}

/// Status bar styling — subtle dark background with top border.
fn status_bar_style(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(iced::Background::Color(Color::from_rgba(0.08, 0.09, 0.1, 1.0))),
        border: iced::Border {
            color: Color::from_rgb(0.2, 0.22, 0.25),
            width: 1.0,
            radius: 0.0.into(),
        },
        ..iced::widget::container::Style::default()
    }
}

// ====================================================================
// Subscriptions
// ====================================================================

fn redraw_worker(
    handle: &RedrawHandle,
) -> std::pin::Pin<Box<dyn iced::futures::Stream<Item = Message> + Send>> {
    use iced::futures::channel::mpsc;
    let handle = handle.clone();
    Box::pin(iced::stream::channel(
        16,
        async move |mut output: mpsc::Sender<Message>| {
            loop {
                let next = {
                    let mut guard = handle.0.lock().await;
                    guard.recv().await
                };
                match next {
                    Some(()) => { let _ = output.send(Message::Redraw).await; }
                    None => break,
                }
            }
        },
    ))
}

fn subscription(app: &PositronicApp) -> Subscription<Message> {
    let mut subs: Vec<Subscription<Message>> = Vec::new();

    // PTY redraw pump
    if let Some(handle) = &app.redraw {
        subs.push(Subscription::run_with(handle.clone(), redraw_worker));
    }

    // Keyboard + window events
    subs.push(event::listen_with(|evt, _status, _id| {
        match evt {
            iced::Event::Keyboard(keyboard::Event::KeyPressed {
                                      key,
                                      modifiers,
                                      ..
                                  }) => {
                if modifiers.control() {
                    match &key {
                        keyboard::Key::Character(c) if c.as_str() == "l" => {
                            return Some(Message::ClearScreen);
                        }
                        keyboard::Key::Character(c) if c.as_str() == "c" => {
                            return Some(Message::CopyToClipboard);
                        }
                        _ => return None,
                    }
                }
                match key {
                    keyboard::Key::Named(keyboard::key::Named::ArrowUp) => Some(Message::HistoryUp),
                    keyboard::Key::Named(keyboard::key::Named::ArrowDown) => Some(Message::HistoryDown),
                    keyboard::Key::Named(keyboard::key::Named::Tab) => Some(Message::TabComplete),
                    _ => None,
                }
            }
            iced::Event::Window(iced::window::Event::Resized(size)) => {
                Some(Message::WindowResized(size.width as u32, size.height as u32))
            }
            _ => None,
        }
    }));

    // Status bar tick — update uptime every 5 seconds
    subs.push(iced::time::every(std::time::Duration::from_secs(5)).map(|_| Message::Tick));

    Subscription::batch(subs)
}

// ====================================================================
// Snapshot hashing
// ====================================================================

fn hash_snapshot(snapshot: &Snapshot) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut h = DefaultHasher::new();
    snapshot.rows().hash(&mut h);
    snapshot.cols().hash(&mut h);
    for row in snapshot.into_iter() {
        for (c, _) in row {
            c.hash(&mut h);
        }
    }
    h.finish()
}

// ====================================================================
// Formatting
// ====================================================================

fn format_duration_short(secs: i64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        format!("{}h {}m", h, m)
    }
}

/// Shorten a path for status bar display.
/// "C:\Users\Doctor\Projects\positronic" → "~\Projects\positronic"
fn short_path(path: &str) -> String {
    // Replace home directory with ~
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        if let Some(rest) = path.strip_prefix(&home) {
            return format!("~{}", rest);
        }
    }
    // If still long, take last 35 chars
    if path.len() > 40 {
        return format!("…{}", &path[path.len() - 35..]);
    }
    path.to_string()
}