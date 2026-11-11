// SPDX-License-Identifier: GPL-3.0-only
//! The applet's logger: super-engine's `logging::init`, with libcosmic's
//! per-redraw height line left out.
//!
//! libcosmic's `Context::suggested_window_size` logs the height it returns at
//! `info`, under `cosmic::applet`, and the applet asks for that size each time
//! it lays itself out, which is every redraw. cosmic-panel forwards an
//! applet's stderr, and the journal files it at warning priority, so each
//! redraw was a warning: ~285 a minute idle and ~6,500 while a visualization
//! drew, most of what `journalctl -p warning` showed.
//!
//! zbus is held to warnings too, as super-engine's `logging::init` holds it.
//! It logs each step of connecting to the session bus at `info`, and the
//! spans it opens arrive through `tracing`'s bridge as `tracing::span`: a
//! dozen lines per instance on every start, each filed as a warning.

use env_logger::Builder;
use log::LevelFilter;

/// Install the logger. `RUST_LOG` always wins, as with super-engine's
/// initializer; otherwise the level is `info`, without libcosmic's applet
/// chatter (see [`defaults`]). Call it once, first thing in `main`.
pub fn init() {
    let mut builder = Builder::from_default_env();
    if std::env::var_os("RUST_LOG").is_none() {
        defaults(&mut builder);
    }
    builder.init();
}

/// `info`, but only warnings and errors from `cosmic::applet` and from zbus.
/// `cosmic::applet`'s `token` module keeps `info`: that is where libcosmic
/// says the applet is exiting because the panel did.
fn defaults(builder: &mut Builder) -> &mut Builder {
    builder
        .filter_level(LevelFilter::Info)
        .filter_module("cosmic::applet", LevelFilter::Warn)
        .filter_module("cosmic::applet::token", LevelFilter::Info)
        .filter_module("zbus", LevelFilter::Warn)
        .filter_module("tracing::span", LevelFilter::Warn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::{Level, Record};

    fn logs(target: &str, level: Level) -> bool {
        let logger = defaults(&mut Builder::new()).build();
        logger.matches(&Record::builder().target(target).level(level).build())
    }

    #[test]
    fn drops_libcosmics_height_line() {
        assert!(!logs("cosmic::applet", Level::Info));
    }

    #[test]
    fn keeps_libcosmics_applet_warnings() {
        assert!(logs("cosmic::applet", Level::Warn));
        assert!(logs("cosmic::applet", Level::Error));
    }

    #[test]
    fn keeps_the_panel_exit_notice() {
        assert!(logs("cosmic::applet::token::wayland_handler", Level::Info));
    }

    #[test]
    fn drops_zbus_connection_steps() {
        assert!(!logs("zbus::connection::handshake::common", Level::Info));
        assert!(!logs("tracing::span", Level::Info));
    }

    #[test]
    fn keeps_zbus_warnings() {
        assert!(logs("zbus::connection", Level::Warn));
    }

    #[test]
    fn keeps_info_from_everything_else() {
        assert!(logs("super_cosmic_applet", Level::Info));
        assert!(logs("cosmic::app", Level::Info));
        assert!(!logs("super_cosmic_applet", Level::Debug));
    }
}
