// SPDX-License-Identifier: GPL-3.0-only
mod app;
mod config;
mod daemon;
mod models;
mod products;
mod ui;
mod util;

// Types needed by the binary entry point (`main.rs`).
pub use app::SuperApplet;
pub use models::theme::VisualizationSide;

// Crate version/repository sourced from Cargo.toml for UI display and
// CLI metadata.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
