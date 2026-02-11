# Positronic Architecture: The System Design

**Overview:**
Positronic is a modular, high-performance terminal emulator designed as a distributed system of Rust crates. It adheres to strict separation of concerns to ensure crash safety, extensibility, and hardware-level performance. The system is designed to be the "Ultimate Terminal" for advanced developers, incorporating AI, Hardware Interfacing, and P2P Collaboration.

---

## 1. The Crate Topology

### `positronic-core` (The Iron Core)

* **Role:** The absolute source of truth. Manages PTYs, State, and Logic.
* **Dependencies:** `portable-pty`, `alacritty_terminal`, `sqlite`, `rusqlite`, `notify` (File Watcher).
* **Sub-Modules:**
  * `state`: The grid of characters (`Arc<RwLock<Grid>>`). Manages cell content, attributes, and Sixel placeholders.
  * `runtime`: The P-Shell parser and Legacy Bridge. Handles the "Dual-Mode" execution logic.
  * `vault`: SQLite persistence layer. Stores History, Logs, and Session State (rehydration).
  * `airlock`: Management of ephemeral containers (Docker/Firecracker) for sandboxed execution of dangerous commands.
  * `watcher`: Filesystem event listener for instant Git status updates (bypassing shell prompts).

### `positronic-bridge` (The Photon Bridge)

* **Role:** The Presentation Layer. Pure UI. No business logic.
* **Dependencies:** `iced`, `wgpu`, `image`, `plotters`, `tree-sitter`, `crossterm` (for key codes).
* **Sub-Modules:**
  * `holodeck`: Manages Rich Media. Renders Images, Sixels, and Interactive Dataframes (Tables/JSON).
  * `biolink`: Voice (Whisper) and Accessibility interfaces (Screen Reader hooks).
  * `vis`: Real-time plotting surfaces for hardware data (Oscilloscope rendering).
  * `input`: The "Intelli-Input" editor state. Handles Vim mode, Autosuggest rendering, and Syntax Highlighting.

### `positronic-neural` (The Brain)

* **Role:** Intelligence provider.
* **Dependencies:** `async-openai`, `ort` (ONNX Runtime).
* **Sub-Modules:**
  * `reflex`: The embedded ONNX runtime for offline inference. Runs the SLM (Small Language Model).
  * `cortex`: The Client for Lemonade/OpenAI compatible servers. Handles complex reasoning.
  * `privacy`: PII sanitization regex engine. Scrubs secrets before inference.
  * `training`: (Optional) Logic for fine-tuning local models on user history.

### `positronic-hive` (The Network)

* **Role:** Peer-to-Peer Collaboration and Sync.
* **Dependencies:** `iroh` (or `libp2p`), `webrtc-rs`.
* **Sub-Modules:**
  * `mesh`: Discovery of local peers (mDNS/Bluetooth) for zero-conf LAN collaboration.
  * `sync`: CRDT-based merging of Command History (Team Vault).
  * `stream`: Encrypted WebRTC channels for "Live Session" screen sharing.

### `positronic-io` (The Hardware Bridge)

* **Role:** Direct Hardware Communication (bypassing PTY).
* **Dependencies:** `serialport`, `usb-enumeration`.
* **Sub-Modules:**
  * `serial`: Async serial port readers with auto-baud detection.
  * `protocol`: Parsers for common embedded protocols (UART, I2C debug output).
  * `buffer`: RingBuffers for high-frequency sensor data (zero-allocation).

### `positronic-script` (The Executor)

* **Role:** Standalone binary/crate for running user scripts.
* **Dependencies:** `wasmtime` (or similar WASM runtime).
* **Sub-Modules:**
  * `rust_runner`: Native implementation of `positronic-script` execution.
  * `wasm_host`: Host for loading community runners (Python/JS) safely.

---

## 2. Data Flow Architecture

### The Input Loop (Intelli-Input)

1. **User Types:** `bridge` captures keystrokes in a local buffer (Text Editor). Do NOT send to PTY.
2. **Syntax Check:** `tree-sitter` highlights the buffer in real-time.
3. **Reflex Check:** `neural/reflex` runs a prediction on the buffer (Autosuggest).
4. **Submit:** User hits Enter -> Buffer sent to `core` via `mpsc`.

### The Execution Loop (Dual-Mode)

1. **Core Receives:** `core/runtime` parses the command.
2. **Security Check:** `neural/privacy` scans for secrets. `core/airlock` checks if "Disposable Mode" is active.
3. **Mode Decision:**
    * **Native:** Execute internal logic (P-Shell). Return `Object`.
    * **Legacy:** Spawn `portable-pty` process (Bash/Zsh). Return `Stream`.
    * **Sandboxed:** Spawn Micro-VM -> Stream output.
4. **Output Stream:**
    * **Legacy:** PTY stream -> `alacritty_terminal` -> Grid.
    * **Native:** Structured Object -> `holodeck` (Bridge).

### The Sensor Loop (Hardware Oscilloscope)

* *Critical Difference:* Does **not** go through the PTY (too slow).

1. **Source:** `positronic-io` reads Serial byte stream (e.g., 115200 baud).
2. **Pipe:** Data pushed to a `RingBuffer<f32>` in shared memory (Lock-Free).
3. **Render:** `bridge/vis` reads RingBuffer @ 60Hz and draws lines using WGPU directly.

### The Rendering Loop (Zero-Copy)

1. **State Snapshot:** `bridge` requests read-lock on `core/state`.
2. **Dirty-Rect:** Compare with previous frame. Identify changed cells.
3. **GPU Upload:** Upload only changed cells + new Textures + Sensor Vertices to WGPU buffers.
4. **Draw:** Composite text, images, and sensor graphs in a single render pass.

---

## 3. Safety & Concurrency Model

* **No Panics:** All `unwrap()` calls are forbidden in `core`. `Result<T, E>` is mandatory.
* **Async/Await:** All I/O (PTY, Network, Disk) is `async`.
* **Actors:** `bridge`, `core`, `hive`, and `io` are separate Actors. They communicate via `mpsc` channels.
* **State Sharing:** `bridge` reads `core` state via `Arc<RwLock<State>>` (Reader Priority).
* **Isolation:** `positronic-io` runs in a separate thread to ensure high-frequency sensor data never blocks the UI or Shell.

---

## 4. The Plugin System (WASM)

* **Sandboxing:** All community plugins (Runners, Themes) run in a WASM sandbox.
* **Capabilities:** Plugins must request permissions (Network, FS) explicitly.
* **Zero-Crash:** A bad plugin panics inside WASM, leaving the Terminal running perfectly.

---
*“I am superior, sir, in many ways, but I would give it all up to be human.”*
