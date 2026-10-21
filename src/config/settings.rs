// SPDX-License-Identifier: GPL-3.0-only
//! The applet's settings: one file, `settings.ron`, for every side of every
//! panel the applet runs on. Each side is its own process, and each watches
//! the file (`app::subscription::watch_settings`), so a change made in one
//! popup shows in all of them.
use super::lenient::variant_or_default;
use crate::models::theme::{
    IconAlignment, VisualizationColorConfig, VisualizationTheme, WorkingAnimationTheme,
};
use crate::products::{CLIENT_NAME, Product};
use log::{debug, error, warn};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

/// The settings file's name, in [`AppletConfig::config_dir`].
pub const SETTINGS_FILE: &str = "settings.ron";

/// What this process last saved, by file: there is one, but a test saves its
/// own. Held while it saves, and while its watch reads the file, so the watch
/// can tell this applet's own saves from the others' (see
/// [`AppletConfig::read_new`]).
static OWN_SAVES: Mutex<BTreeMap<PathBuf, String>> = Mutex::new(BTreeMap::new());

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppletConfig {
    pub visualization: VisualizationConfig,
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VisualizationConfig {
    /// How the panel draws Super STT's recording and transcribing.
    pub stt: ProductVisualization,
    /// How the panel draws Super TTS's speech and synthesizing.
    pub tts: ProductVisualization,
}

impl VisualizationConfig {
    pub fn product(&self, product: Product) -> &ProductVisualization {
        match product {
            Product::Stt => &self.stt,
            Product::Tts => &self.tts,
        }
    }

    pub fn product_mut(&mut self, product: Product) -> &mut ProductVisualization {
        match product {
            Product::Stt => &mut self.stt,
            Product::Tts => &mut self.tts,
        }
    }
}

/// How the panel draws one product's activity, so the two can be told apart
/// when both are active and the panel switches between them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProductVisualization {
    #[serde(default, deserialize_with = "variant_or_default")]
    pub theme: VisualizationTheme,
    /// Animation shown while the product works: transcribing for Super STT,
    /// synthesizing the first audio for Super TTS.
    #[serde(default, deserialize_with = "variant_or_default")]
    pub working_animation: WorkingAnimationTheme,
    #[serde(default)]
    pub colors: VisualizationColorConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub show_icon: bool,
    #[serde(deserialize_with = "variant_or_default")]
    pub icon_alignment: IconAlignment,
    /// How far the visualization runs along the panel, in pixels:
    /// its width on a top or bottom panel, its height on a side one.
    pub applet_width: u32,
    pub show_visualization: bool, // Whether to show visualizations when recording
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_icon: true,
            icon_alignment: IconAlignment::End,
            applet_width: 120,        // Default width in pixels
            show_visualization: true, // Default to showing visualizations when recording
        }
    }
}

impl AppletConfig {
    /// The applet's own config directory, `$XDG_CONFIG_HOME/super-cosmic-applet`,
    /// beside the products' own.
    pub fn config_dir() -> PathBuf {
        super_engine_protocol::paths::config_dir(Product::Stt.spec()).with_file_name(CLIENT_NAME)
    }

    /// Parse `settings.ron`, falling back to defaults on a parse error. Pure
    /// (no I/O) so the load/reset decision is unit-testable. Returns the
    /// config and whether a reset occurred.
    fn parse_or_reset(content: &str) -> (Self, bool) {
        match ron::from_str::<AppletConfig>(content) {
            Ok(config) => (config, false),
            Err(e) => {
                warn!("Failed to parse the applet's settings: {e}. Using defaults.");
                (Self::default(), true)
            }
        }
    }

    /// Load the settings: the defaults until an applet has saved some.
    pub fn load() -> Self {
        Self::load_in(&Self::config_dir())
    }

    /// [`Self::load`], from the settings file in `dir`.
    fn load_in(dir: &Path) -> Self {
        let path = dir.join(SETTINGS_FILE);
        match fs::read_to_string(&path) {
            Ok(content) => Self::parse_or_reset(&content).0,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(e) => {
                warn!("Failed to read {}: {e}. Using defaults.", path.display());
                Self::default()
            }
        }
    }

    /// The settings another applet, or a hand edit, just saved to `path`.
    ///
    /// `None` for what this process saved itself. Its own saves reach its
    /// watch like the others', and during a run of them, a slider drag,
    /// applying an earlier one would set the slider back. Taking the lock
    /// [`Self::save_in`] holds makes the comparison exact: the file is read
    /// either wholly before a save or wholly after it.
    ///
    /// `None` too while the file is missing or doesn't parse, say halfway
    /// through a hand edit, so the running applet keeps what it has instead
    /// of resetting.
    pub fn read_new(path: &Path) -> Option<Self> {
        let own = OWN_SAVES.lock().unwrap_or_else(PoisonError::into_inner);
        let content = fs::read_to_string(path).ok()?;
        if own.get(path) == Some(&content) {
            return None;
        }
        drop(own);
        ron::from_str(&content)
            .inspect_err(|e| debug!("Ignoring the settings file for now: {e}"))
            .ok()
    }

    /// Save the settings to `dir`, by writing them beside the file and
    /// renaming them over it, so an applet watching the file never reads half
    /// of it.
    pub fn save_in(&self, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all(dir)?;
        let content = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())?;
        // One temporary file per process, so two applets saving at once each
        // rename a whole file into place.
        let temporary = dir.join(format!(".{SETTINGS_FILE}.{}", std::process::id()));
        let path = dir.join(SETTINGS_FILE);
        let mut own = OWN_SAVES.lock().unwrap_or_else(PoisonError::into_inner);
        fs::write(&temporary, &content)?;
        fs::rename(&temporary, &path)?;
        own.insert(path.clone(), content);
        debug!("Saved the applet's settings to {}", path.display());
        Ok(())
    }

    /// Apply an in-place mutation and save it to `dir`. Replaces the fan-out
    /// of `update_*` setters (Applet Tier 3 #21): `config.update_in(dir, |c|
    /// c.ui.show_icon = v)` mutates then saves, logging (not propagating) a
    /// write failure.
    pub fn update_in(&mut self, dir: &Path, mutate: impl FnOnce(&mut Self)) {
        mutate(self);
        if let Err(e) = self.save_in(dir) {
            error!("Failed to save config: {e}");
        }
    }
}

#[cfg(test)]
mod working_animation_config_tests {
    use super::AppletConfig;
    use crate::models::theme::WorkingAnimationTheme;

    #[test]
    fn default_working_animation_is_droplet() {
        let cfg = AppletConfig::default();
        for look in [&cfg.visualization.stt, &cfg.visualization.tts] {
            assert_eq!(look.working_animation, WorkingAnimationTheme::Droplet);
        }
    }

    #[test]
    fn old_settings_without_the_field_still_load() {
        // A settings file from before the field existed must still parse
        // (serde default) and yield Droplet, not reset everything else.
        let full = ron::ser::to_string_pretty(&AppletConfig::default(), Default::default())
            .expect("serialize");
        let without: String = full
            .lines()
            .filter(|l| !l.trim_start().starts_with("working_animation"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !without.contains("working_animation"),
            "test setup: line not removed"
        );
        let (cfg, was_reset) = AppletConfig::parse_or_reset(&without);
        assert!(!was_reset, "old settings must still parse:\n{without}");
        for look in [&cfg.visualization.stt, &cfg.visualization.tts] {
            assert_eq!(look.working_animation, WorkingAnimationTheme::Droplet);
        }
    }
}

#[cfg(test)]
mod settings_file_tests {
    //! `settings.ron`, as the applet saves it and as a hand edit may leave it.
    use super::{AppletConfig, ProductVisualization};
    use crate::models::theme::{
        IconAlignment, VisualizationColor, VisualizationTheme, WorkingAnimationTheme,
    };

    #[test]
    fn every_setting_survives_a_save() {
        let mut cfg = AppletConfig::default();
        cfg.visualization.stt = ProductVisualization {
            theme: VisualizationTheme::Waveform,
            working_animation: WorkingAnimationTheme::Comet,
            ..ProductVisualization::default()
        };
        cfg.visualization.stt.colors.dark_colors = VisualizationColor::Blue;
        cfg.visualization.tts = ProductVisualization {
            theme: VisualizationTheme::Pulse,
            working_animation: WorkingAnimationTheme::Dots,
            ..ProductVisualization::default()
        };
        cfg.visualization.tts.colors.light_colors = VisualizationColor::DarkOrange;
        cfg.ui.show_icon = false;
        cfg.ui.icon_alignment = IconAlignment::Center;
        cfg.ui.applet_width = 210;
        cfg.ui.show_visualization = false;

        let saved = ron::ser::to_string_pretty(&cfg, Default::default()).expect("serialize");
        let (back, was_reset) = AppletConfig::parse_or_reset(&saved);

        assert!(!was_reset, "saved settings must load:\n{saved}");
        assert_eq!(back, cfg);
    }

    #[test]
    fn a_missing_setting_takes_its_default() {
        let (cfg, was_reset) = AppletConfig::parse_or_reset("(ui: (applet_width: 200))");

        assert!(!was_reset);
        assert_eq!(cfg.ui.applet_width, 200);
        let defaults = AppletConfig::default();
        assert_eq!(cfg.ui.show_icon, defaults.ui.show_icon);
        assert_eq!(cfg.ui.icon_alignment, defaults.ui.icon_alignment);
        assert_eq!(cfg.visualization, defaults.visualization);
    }

    #[test]
    fn an_unknown_value_falls_back_without_a_reset() {
        let (cfg, was_reset) = AppletConfig::parse_or_reset(
            "(
                visualization: (
                    stt: (theme: Nonexistent, colors: (light_colors: Blue)),
                ),
                ui: (icon_alignment: sideways, applet_width: 150),
            )",
        );

        assert!(!was_reset, "one bad value must not cost the whole file");
        assert_eq!(cfg.visualization.stt.theme, VisualizationTheme::default());
        assert_eq!(
            cfg.visualization.stt.colors.light_colors,
            VisualizationColor::Blue
        );
        assert_eq!(cfg.ui.icon_alignment, IconAlignment::default());
        assert_eq!(cfg.ui.applet_width, 150);
    }

    #[test]
    fn a_corrupt_file_resets_to_default() {
        let (cfg, was_reset) = AppletConfig::parse_or_reset("not ::: valid ron (");
        assert!(was_reset, "garbage input must trigger a reset");
        assert_eq!(cfg, AppletConfig::default());
    }
}

#[cfg(test)]
mod load_and_save_tests {
    //! The settings on disk, in a scratch directory standing in for
    //! `~/.config/super-cosmic-applet`.
    use super::{AppletConfig, SETTINGS_FILE};
    use crate::models::theme::VisualizationTheme;
    use crate::products::Product;
    use std::fs;
    use std::path::PathBuf;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "super-cosmic-applet-load-{name}-{}",
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

    fn with_width(width: u32) -> AppletConfig {
        let mut config = AppletConfig::default();
        config.ui.applet_width = width;
        config
    }

    #[test]
    fn saved_settings_load_back() {
        let scratch = Scratch::new("round-trip");
        let dir = scratch.dir("applet");
        with_width(240).save_in(&dir).expect("save");

        assert_eq!(AppletConfig::load_in(&dir), with_width(240));
        // The temporary file the save renamed into place is gone.
        let left: Vec<_> = fs::read_dir(&dir)
            .expect("read dir")
            .map(|e| e.expect("entry").file_name())
            .collect();
        assert_eq!(left, [SETTINGS_FILE]);
    }

    #[test]
    fn a_corrupt_settings_file_loads_the_defaults() {
        let scratch = Scratch::new("corrupt-ron");
        let dir = scratch.dir("applet");
        fs::write(dir.join(SETTINGS_FILE), "not ::: ron (").expect("write");

        assert_eq!(AppletConfig::load_in(&dir), AppletConfig::default());
    }

    #[test]
    fn with_nothing_on_disk_the_defaults_load() {
        let scratch = Scratch::new("empty");
        assert_eq!(
            AppletConfig::load_in(&scratch.dir("applet")),
            AppletConfig::default()
        );
    }

    #[test]
    fn an_unreadable_settings_file_loads_the_defaults() {
        let scratch = Scratch::new("unreadable");
        let dir = scratch.dir("applet");
        fs::create_dir_all(dir.join(SETTINGS_FILE)).expect("a directory where the file goes");

        assert_eq!(AppletConfig::load_in(&dir), AppletConfig::default());
    }

    #[test]
    fn saving_where_no_directory_can_be_made_fails() {
        let scratch = Scratch::new("save-fails");
        let file = scratch.0.join("a-file");
        fs::write(&file, "").expect("write");

        assert!(with_width(200).save_in(&file.join("applet")).is_err());
    }

    #[test]
    fn a_change_is_saved() {
        let scratch = Scratch::new("update");
        let dir = scratch.dir("applet");
        let mut config = AppletConfig::default();

        config.update_in(&dir, |c| c.ui.show_icon = false);

        assert!(!config.ui.show_icon);
        assert_eq!(AppletConfig::load_in(&dir), config);
    }

    /// The applet keeps running on what the user chose even when the disk
    /// won't take it.
    #[test]
    fn a_change_that_cant_be_saved_still_applies() {
        let scratch = Scratch::new("update-fails");
        let file = scratch.0.join("a-file");
        fs::write(&file, "").expect("write");
        let mut config = AppletConfig::default();

        config.update_in(&file.join("applet"), |c| c.ui.applet_width = 77);

        assert_eq!(config.ui.applet_width, 77);
    }

    #[test]
    fn another_applets_save_is_new_and_this_ones_is_not() {
        let scratch = Scratch::new("read-new");
        let dir = scratch.dir("applet");
        let path = dir.join(SETTINGS_FILE);

        assert_eq!(AppletConfig::read_new(&path), None, "no file yet");

        with_width(150).save_in(&dir).expect("save");
        assert_eq!(
            AppletConfig::read_new(&path),
            None,
            "this applet's own save"
        );

        let theirs = ron::ser::to_string_pretty(&with_width(260), Default::default()).unwrap();
        fs::write(&path, theirs).expect("write");
        assert_eq!(AppletConfig::read_new(&path), Some(with_width(260)));

        fs::write(&path, "(ui: (applet_width: ").expect("write");
        assert_eq!(AppletConfig::read_new(&path), None, "half a hand edit");
    }

    #[test]
    fn each_product_reads_and_writes_its_own_settings() {
        let mut config = AppletConfig::default();
        config.visualization.product_mut(Product::Tts).theme = VisualizationTheme::Pulse;

        assert_eq!(
            config.visualization.product(Product::Tts).theme,
            VisualizationTheme::Pulse
        );
        assert_eq!(config.visualization.tts.theme, VisualizationTheme::Pulse);
        assert_eq!(
            config.visualization.stt,
            AppletConfig::default().visualization.stt
        );
        config.visualization.product_mut(Product::Stt).theme = VisualizationTheme::Waveform;
        assert_eq!(config.visualization.stt.theme, VisualizationTheme::Waveform);
    }

    /// Beside the products' own directories, under the applet's name.
    #[test]
    fn the_settings_live_in_the_applets_own_directory() {
        let dir = AppletConfig::config_dir();
        assert_eq!(
            dir.file_name().and_then(|n| n.to_str()),
            Some("super-cosmic-applet")
        );
        assert_eq!(
            dir.parent(),
            super_engine_protocol::paths::config_dir(Product::Stt.spec()).parent()
        );
    }
}
