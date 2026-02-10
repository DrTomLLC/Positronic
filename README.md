# Positronic

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.93%2B-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-windows-blue.svg)](https://microsoft.com)
[![Architecture](https://img.shields.io/badge/arch-amd64-red.svg)]()

**Positronic** is a high-performance, block-based terminal environment engineered for privacy, speed, and local intelligence.

Unlike commercial alternatives that offload processing to the cloud or enforce mandatory logins, Positronic is architected to run entirely on your metal. It leverages a strictly modular Rust workspace—**Core**, **Bridge**, and **Neural**—to deliver a modern terminal experience that utilizes local NPU hardware for AI assistance without telemetry.

---

## 🏗 Architecture

Positronic ignores the monolithic design of traditional emulators in favor of a strictly decoupled workspace architecture.

| Module | Crate Name | Function |
| :--- | :--- | :--- |
| **The Engine** | `positronic-core` | The internal PTY state machine, VTE parsing logic, and OSC 133 block segmentation. Handles the raw "physics" of the terminal. |
| **The Interface** | `positronic-bridge` | The GPU-accelerated frontend. Handles user input, rendering, and visual feedback using `iced`/`gpui`. |
| **The Intelligence** | `positronic-neural` | The local AI adapter. Interacts with local inference servers (Lemonade) via AMD NPU to provide command correction and context. |

## 🚀 Features

-   **Block-Based Output:** Commands and outputs are segmented into discrete, interactive blocks rather than a continuous stream of text.
-   **Local NPU Acceleration:** Direct integration with **Lemonade** to run LLMs on AMD Ryzen AI hardware. Your prompt history never leaves `localhost`.
-   **Rust-Native Scripting:** (Experimental) First-class support for executing `.rs` files as shell scripts via embedded `cargo-script` caching.
-   **Zero Telemetry:** No analytics. No "cloud sync." No login required.

## 🛠 Prerequisites

To build Positronic from source, you need a Windows 11 environment set up for systems programming.

1.  **Rust Toolchain (1.93+)**
    ```powershell
    rustup update
    rustup default stable-x86_64-pc-windows-msvc
    ```
2.  **C++ Build Tools**
    -   Visual Studio 2022 Build Tools (Required for PTY backend compilation).
3.  **Lemonade Server (Optional for AI features)**
    -   Must be running on `localhost:8000` to enable `positronic-neural` capabilities.

## ⚡ Quick Start

```powershell
# Clone the repository
git clone https://github.com/yourusername/positronic.git
cd positronic

# Build the bridge (Release mode recommended for GPU performance)
cargo build --release --bin positronic-bridge

# Run
./target/release/positronic-bridge.exe
