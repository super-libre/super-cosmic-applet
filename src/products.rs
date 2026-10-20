// SPDX-License-Identifier: GPL-3.0-only
//! The two products the applet serves, and what it asks of each daemon.
//!
//! The applet holds one session with each daemon, under the same client name,
//! so each product's keyring keeps the applet's token beside its own clients'.
//! Both the `/ping` liveness client and the `/events` subscription of a product
//! present the same [`AppId`] and scopes, so they share that one token.

use std::path::{Path, PathBuf};

use super_engine_client::session::AppId;
use super_engine_protocol::ProductSpec;

/// The applet's name as each daemon's client list knows it: its binary, and
/// the keyring user its session tokens are stored under. super-engine names
/// it `SHARED_APPLET` from 6a9408d on; this is the same string until the
/// engine pin reaches that.
pub const CLIENT_NAME: &str = "super-cosmic-applet";

/// The applet's name in each daemon's consent dialog.
pub const APP_NAME: &str = "Super Applet";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Product {
    Stt,
    Tts,
}

impl Product {
    pub const ALL: [Self; 2] = [Self::Stt, Self::Tts];

    pub fn spec(self) -> &'static ProductSpec {
        match self {
            Self::Stt => &super_stt_registry_types::product::SUPER_STT,
            Self::Tts => &super_tts_registry_types::product::SUPER_TTS,
        }
    }

    pub fn app_id(self) -> AppId {
        AppId {
            product: self.spec(),
            name: CLIENT_NAME,
        }
    }

    /// `audio_visualization` for the frequency bands, and the scope that
    /// carries the product's activity: recording for Super STT, playback for
    /// Super TTS.
    pub fn scopes(self) -> &'static [&'static str] {
        match self {
            Self::Stt => &["recording_events", "audio_visualization"],
            Self::Tts => &["playback_events", "audio_visualization"],
        }
    }

    /// The `/events` topics the applet subscribes to.
    ///
    /// Super TTS's `speech_progress` is there for one transition: an utterance
    /// is accepted before any audio exists for it, and with a remote backend
    /// that gap is long enough to look like a hang. `spoken_ms > 0` is the
    /// daemon saying the first audio is playing.
    pub fn topics(self) -> &'static [&'static str] {
        match self {
            Self::Stt => &[
                "recording_state",
                "frequency_bands",
                "transcribing_started",
                "transcribing_stopped",
            ],
            Self::Tts => &["speaking_state", "speech_progress", "frequency_bands"],
        }
    }

    pub fn display_name(self) -> &'static str {
        self.spec().display_name
    }

    /// The settings app the popup's launch button opens.
    pub fn app_binary(self) -> String {
        format!("{}-app", self.spec().slug)
    }

    /// The settings app's icon in the icon theme: the name its desktop entry
    /// gives, which is the app's binary.
    pub fn app_icon(self) -> String {
        self.app_binary()
    }

    /// Whether the product is installed on this machine: its daemon binary
    /// sits beside the applet, as both installers put them, or on `PATH`.
    ///
    /// A product that is not installed gets no subscription and no status
    /// line, rather than a "Connecting..." that never ends.
    pub fn is_installed(self) -> bool {
        let beside_us = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf));
        let path_dirs = std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).collect::<Vec<PathBuf>>())
            .unwrap_or_default();
        self.is_installed_in(beside_us.into_iter().chain(path_dirs))
    }

    /// Whether the product's daemon binary is a file in one of `dirs`.
    fn is_installed_in(self, dirs: impl IntoIterator<Item = PathBuf>) -> bool {
        let daemon = self.spec().daemon_binary();
        dirs.into_iter().any(|dir| dir.join(&daemon).is_file())
    }

    /// The daemon's HTTP socket.
    pub fn socket_path(self) -> PathBuf {
        super_engine_protocol::runtime::get_http_socket_path(self.spec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each daemon refuses a subscription whose topics its scopes don't grant,
    /// with a 403 for the whole stream.
    #[test]
    fn every_topic_is_granted_by_the_products_scopes() {
        for product in Product::ALL {
            assert_eq!(
                super_engine_protocol::scopes::uncovered_topic(
                    product.spec(),
                    product.scopes(),
                    product.topics()
                ),
                None,
                "{} would refuse the subscription",
                product.display_name()
            );
        }
    }

    /// The two products must never reach the same daemon or keyring entry.
    #[test]
    fn the_two_products_do_not_collide() {
        let (stt, tts) = (Product::Stt.spec(), Product::Tts.spec());
        assert_ne!(stt.slug, tts.slug);
        assert_ne!(stt.short_name, tts.short_name);
        assert_ne!(stt.env_prefix, tts.env_prefix);
        assert_ne!(stt.tcp_port, tts.tcp_port);
        assert_ne!(Product::Stt.socket_path(), Product::Tts.socket_path());
        assert_ne!(stt.session_keyring_service(), tts.session_keyring_service());
    }
}

#[cfg(test)]
mod product_tests {
    use super::{CLIENT_NAME, Product};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn each_products_app_is_named_after_its_slug() {
        assert_eq!(Product::Stt.app_binary(), "super-stt-app");
        assert_eq!(Product::Tts.app_binary(), "super-tts-app");
        // The launch row looks the icon up under the name the desktop entry
        // gives it, which is the binary's.
        for product in Product::ALL {
            assert_eq!(product.app_icon(), product.app_binary());
        }
    }

    #[test]
    fn the_products_are_shown_by_their_own_names() {
        assert_eq!(Product::Stt.display_name(), "Super STT");
        assert_eq!(Product::Tts.display_name(), "Super TTS");
    }

    /// Both daemons keep the applet's token under the same client name, in
    /// their own keyring service.
    #[test]
    fn the_applet_presents_one_name_to_each_products_daemon() {
        for product in Product::ALL {
            let app_id = product.app_id();
            assert_eq!(app_id.name, CLIENT_NAME);
            assert_eq!(app_id.product.slug, product.spec().slug);
        }
    }

    #[test]
    fn each_product_subscribes_to_its_own_activity() {
        assert!(Product::Stt.scopes().contains(&"recording_events"));
        assert!(Product::Tts.scopes().contains(&"playback_events"));
        for product in Product::ALL {
            assert!(product.scopes().contains(&"audio_visualization"));
            assert!(product.topics().contains(&"frequency_bands"));
        }
    }

    /// A directory for one test, removed when it ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "super-cosmic-applet-products-{name}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("create scratch dir");
            Self(dir)
        }

        fn dir(&self, name: &str) -> PathBuf {
            let dir = self.0.join(name);
            fs::create_dir_all(&dir).expect("create dir");
            dir
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_product_is_installed_when_its_daemon_is_in_any_searched_dir() {
        let scratch = Scratch::new("installed");
        let (empty, bin) = (scratch.dir("empty"), scratch.dir("bin"));
        fs::write(bin.join(Product::Tts.spec().daemon_binary()), "").expect("write daemon");

        assert!(Product::Tts.is_installed_in([empty.clone(), bin.clone()]));
        // The other product's daemon isn't there.
        assert!(!Product::Stt.is_installed_in([empty, bin]));
    }

    #[test]
    fn a_product_with_no_daemon_in_reach_is_not_installed() {
        let scratch = Scratch::new("missing");
        assert!(!Product::Stt.is_installed_in([scratch.dir("a"), scratch.dir("b")]));
        assert!(!Product::Stt.is_installed_in(Vec::new()));
    }

    /// A directory with the daemon's name is not the daemon.
    #[test]
    fn only_a_file_counts_as_the_daemon() {
        let scratch = Scratch::new("dir-named-daemon");
        let bin = scratch.dir("bin");
        fs::create_dir_all(bin.join(Product::Stt.spec().daemon_binary())).expect("mkdir");

        assert!(!Product::Stt.is_installed_in([bin]));
    }
}
