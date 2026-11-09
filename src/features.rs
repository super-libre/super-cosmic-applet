// SPDX-License-Identifier: GPL-3.0-only
//! Super Voice's two features, and what the applet asks of the daemon for
//! each.
//!
//! One daemon serves both features, so the applet holds one session with it:
//! both features present the same [`AppId`] and scopes, and share the token.
//! Each feature has its own `/events` subscription, for its own topics, and
//! its own status line and settings app.

use std::path::{Path, PathBuf};

use super_engine_client::session::AppId;
use super_engine_protocol::ProductSpec;
use super_voice_registry_types::product::SUPER_VOICE;

/// The applet's name as the daemon's client list knows it: its binary, and
/// the name its session token is stored under.
pub const CLIENT_NAME: &str = "super-cosmic-applet";

/// The applet's name in the daemon's consent dialog.
pub const APP_NAME: &str = "Super Applet";

/// Every scope the applet needs, for both features. One token serves both,
/// so it asks for all of them at once: `audio_visualization` for the level
/// meters, `recording_events` for dictation, `playback_events` for speech.
const SCOPES: &[&str] = &["recording_events", "playback_events", "audio_visualization"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {
    Stt,
    Tts,
}

impl Feature {
    pub const ALL: [Self; 2] = [Self::Stt, Self::Tts];

    /// The daemon's names. One daemon, so the same for both features.
    #[allow(
        clippy::unused_self,
        reason = "asked of a feature like the rest, though both answer alike"
    )]
    pub fn spec(self) -> &'static ProductSpec {
        &SUPER_VOICE
    }

    pub fn app_id(self) -> AppId {
        AppId {
            product: self.spec(),
            name: CLIENT_NAME,
        }
    }

    /// The scopes the applet's token carries. The same for both features: see
    /// [`SCOPES`].
    #[allow(
        clippy::unused_self,
        reason = "asked of a feature like the rest, though both answer alike"
    )]
    pub fn scopes(self) -> &'static [&'static str] {
        SCOPES
    }

    /// The `/events` topics the applet subscribes to for this feature.
    ///
    /// The microphone's level is `frequency_bands`, the speakers'
    /// `playback_frequency_bands`. Text to speech's `speech_progress` is there
    /// for one transition: an utterance is accepted before any audio exists
    /// for it, and with a remote backend that gap is long enough to look like
    /// a hang. `spoken_ms > 0` is the daemon saying the first audio is playing.
    pub fn topics(self) -> &'static [&'static str] {
        match self {
            Self::Stt => &[
                "recording_state",
                "frequency_bands",
                "transcribing_started",
                "transcribing_stopped",
            ],
            Self::Tts => &[
                "speaking_state",
                "speech_progress",
                "playback_frequency_bands",
            ],
        }
    }

    /// The name the popup shows for this feature, the one Super Voice's app
    /// gives it.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Stt => "Speech to text",
            Self::Tts => "Text to speech",
        }
    }

    /// Whether the feature is installed on this machine: the daemon binary
    /// sits beside the applet, as the installer puts them, or on `PATH`. One
    /// daemon serves both features, so they are installed together.
    ///
    /// A feature that is not installed gets no subscription and no status
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

    /// Whether the daemon binary is a file in one of `dirs`.
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

    /// The daemon refuses a subscription whose topics its scopes don't grant,
    /// with a 403 for the whole stream.
    #[test]
    fn every_topic_is_granted_by_the_applets_scopes() {
        for product in Feature::ALL {
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

    /// Both features reach the one daemon with the one token: the same
    /// socket, the same client, the same scopes. A token minted for one
    /// feature's subscription must serve the other's too.
    #[test]
    fn both_features_share_the_daemon_and_the_token() {
        assert_eq!(Feature::Stt.socket_path(), Feature::Tts.socket_path());
        let (stt, tts) = (Feature::Stt.app_id(), Feature::Tts.app_id());
        assert_eq!(stt.name, tts.name);
        assert!(std::ptr::eq(stt.product, tts.product));
        assert_eq!(Feature::Stt.scopes(), Feature::Tts.scopes());
    }
}

/// Super Voice's settings app, which the popup's launch button opens. One app
/// shows both features.
pub struct SettingsApp {
    /// The executable, looked up on `PATH` and in the install prefixes.
    pub binary: &'static str,
    /// Its icon in the icon theme: the name its desktop entry gives.
    pub icon: &'static str,
    /// What the launch button's tooltip calls it.
    pub name: &'static str,
}

pub const SETTINGS_APP: SettingsApp = SettingsApp {
    binary: "super-voice-app",
    icon: "super-voice-app",
    name: "Super Voice",
};

#[cfg(test)]
mod feature_tests {
    use super::{CLIENT_NAME, Feature};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn both_features_open_super_voices_app() {
        assert_eq!(super::SETTINGS_APP.binary, "super-voice-app");
        // The launch row looks the icon up under the name the desktop entry
        // gives it, which is the binary's.
        assert_eq!(super::SETTINGS_APP.icon, super::SETTINGS_APP.binary);
        assert_eq!(super::SETTINGS_APP.name, "Super Voice");
    }

    #[test]
    fn the_features_are_shown_by_the_names_the_app_gives_them() {
        assert_eq!(Feature::Stt.display_name(), "Speech to text");
        assert_eq!(Feature::Tts.display_name(), "Text to speech");
    }

    /// The daemon keeps the applet's token under its client name.
    #[test]
    fn the_applet_presents_one_name_to_the_daemon() {
        for product in Feature::ALL {
            let app_id = product.app_id();
            assert_eq!(app_id.name, CLIENT_NAME);
            assert_eq!(app_id.product.slug, product.spec().slug);
        }
    }

    /// Each feature follows its own activity and its own sound: the
    /// microphone for dictation, the speakers for speech.
    #[test]
    fn each_feature_subscribes_to_its_own_activity() {
        assert!(Feature::Stt.topics().contains(&"recording_state"));
        assert!(Feature::Stt.topics().contains(&"frequency_bands"));
        assert!(Feature::Tts.topics().contains(&"speaking_state"));
        assert!(Feature::Tts.topics().contains(&"playback_frequency_bands"));
        assert!(!Feature::Tts.topics().contains(&"frequency_bands"));
    }

    /// A directory for one test, removed when it ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "super-cosmic-applet-features-{name}-{}",
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
    fn a_feature_is_installed_when_the_daemon_is_in_any_searched_dir() {
        let scratch = Scratch::new("installed");
        let (empty, bin) = (scratch.dir("empty"), scratch.dir("bin"));
        fs::write(bin.join(Feature::Tts.spec().daemon_binary()), "").expect("write daemon");

        // One daemon, so both features are installed with it.
        for feature in Feature::ALL {
            assert!(feature.is_installed_in([empty.clone(), bin.clone()]));
        }
    }

    #[test]
    fn a_feature_with_no_daemon_in_reach_is_not_installed() {
        let scratch = Scratch::new("missing");
        assert!(!Feature::Stt.is_installed_in([scratch.dir("a"), scratch.dir("b")]));
        assert!(!Feature::Stt.is_installed_in(Vec::new()));
    }

    /// A directory with the daemon's name is not the daemon.
    #[test]
    fn only_a_file_counts_as_the_daemon() {
        let scratch = Scratch::new("dir-named-daemon");
        let bin = scratch.dir("bin");
        fs::create_dir_all(bin.join(Feature::Stt.spec().daemon_binary())).expect("mkdir");

        assert!(!Feature::Stt.is_installed_in([bin]));
    }
}
