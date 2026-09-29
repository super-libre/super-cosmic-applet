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

    /// Whether the product is installed on this machine: its daemon binary
    /// sits beside the applet, as both installers put them, or on `PATH`.
    ///
    /// A product that is not installed gets no subscription and no status
    /// line, rather than a "Connecting..." that never ends.
    pub fn is_installed(self) -> bool {
        let daemon = self.spec().daemon_binary();
        let beside_us = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf));
        let path_dirs = std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).collect::<Vec<PathBuf>>())
            .unwrap_or_default();
        beside_us
            .into_iter()
            .chain(path_dirs)
            .any(|dir| dir.join(&daemon).is_file())
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
