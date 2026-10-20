// SPDX-License-Identifier: GPL-3.0-only
//! A real applet for tests: built from the default settings, saving them to a
//! scratch directory of its own, with the products a test says are installed.
//!
//! It never touches the user's settings and never reaches a daemon: the task
//! an update returns is dropped, so a test plays the daemons' side by sending
//! their messages.
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use cosmic::Element;

use super::{DaemonMessage, Message, ProductLink, SuperApplet};
use crate::config::{AppletConfig, SETTINGS_FILE};
use crate::models::theme::VisualizationSide;
use crate::products::Product;

pub(crate) struct TestApplet {
    pub(crate) applet: SuperApplet,
    dir: PathBuf,
}

impl TestApplet {
    /// An applet on the full side, with `installed` installed and none of
    /// their daemons connected yet.
    pub(crate) fn new(installed: &[Product]) -> Self {
        Self::with_side(installed, VisualizationSide::Full)
    }

    pub(crate) fn with_side(installed: &[Product], side: VisualizationSide) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "super-cosmic-applet-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let (applet, _) = SuperApplet::with_settings(
            cosmic::app::Core::default(),
            side,
            AppletConfig::default(),
            dir.clone(),
            installed_check(installed),
        );
        Self { applet, dir }
    }

    /// Send `message`, as iced would.
    pub(crate) fn send(&mut self, message: Message) -> &mut Self {
        let _ = self.applet.handle_message(message);
        self
    }

    /// Send `message` from `product`'s daemon.
    pub(crate) fn daemon(&mut self, product: Product, message: DaemonMessage) -> &mut Self {
        self.send(Message::Daemon(product, message))
    }

    pub(crate) fn connect(&mut self, product: Product) -> &mut Self {
        self.daemon(product, DaemonMessage::Connected)
    }

    pub(crate) fn link(&self, product: Product) -> &ProductLink {
        self.applet.link(product)
    }

    pub(crate) fn config(&self) -> &AppletConfig {
        &self.applet.config
    }

    /// The settings file as this applet last saved it, if it has.
    pub(crate) fn saved(&self) -> Option<AppletConfig> {
        let content = std::fs::read_to_string(self.dir.join(SETTINGS_FILE)).ok()?;
        Some(ron::from_str(&content).expect("the applet saved settings it can't read"))
    }

    pub(crate) fn popup(&self) -> Element<'_, Message> {
        self.applet.view_popup()
    }

    pub(crate) fn panel(&self) -> Element<'_, Message> {
        self.applet.view_applet()
    }
}

impl Drop for TestApplet {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// [`Product::is_installed`], answering for the products in `installed`. A
/// plain function, as the applet keeps one, so one per case.
fn installed_check(installed: &[Product]) -> fn(Product) -> bool {
    match (
        installed.contains(&Product::Stt),
        installed.contains(&Product::Tts),
    ) {
        (true, true) => |_| true,
        (true, false) => |p| p == Product::Stt,
        (false, true) => |p| p == Product::Tts,
        (false, false) => |_| false,
    }
}
