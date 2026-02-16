//! Positronic Bridge — GPU frontend shell.
//!
//! Binary entry point. Application logic lives in the library crate.

use positronic_bridge::shell;
use positronic_bridge::util;

fn main() {
    util::init_tracing();
    util::install_panic_hook();

    tracing::info!("=== Positronic v0.3.0 Starting ===");

    if let Err(e) = shell::run() {
        tracing::error!("Fatal: {:#}", e);
        std::process::exit(1);
    }
}
