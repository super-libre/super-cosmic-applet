// SPDX-License-Identifier: GPL-3.0-only
//! The applet's settings: one file, `settings.ron`, for every side of every
//! panel the applet runs on. Each side is its own process, and each watches
//! the file (`app::subscription::settings_watch`), so a change made in one
//! popup shows in all of them.
use super::lenient::variant_or_default;
use crate::models::theme::{
    IconAlignment, VisualizationColorConfig, VisualizationTheme, WorkingAnimationTheme,
};
use crate::products::{CLIENT_NAME, Product};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

/// The settings file's name, in [`AppletConfig::config_dir`].
pub const SETTINGS_FILE: &str = "settings.ron";

/// What this process last saved. Held while it saves, and while its watch
/// reads the file, so the watch can tell this applet's own saves from the
/// others' (see [`AppletConfig::read_new`]).
static OWN_SAVE: Mutex<Option<String>> = Mutex::new(None);

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

/// The settings as the applet kept them before `settings.ron`: a TOML file
/// per side, `applet-{full,left,right}.toml`, in the applet's own directory
/// or, before the applet was shared, in Super STT's or Super TTS's.
#[derive(Deserialize)]
struct TomlConfig {
    #[serde(default)]
    visualization: TomlVisualization,
    #[serde(default)]
    ui: UiConfig,
}

/// Every TOML config keeps one set of visualization settings at the top of
/// `[visualization]`, shared by both products. One written while each product
/// had its own settings has a `[visualization.stt]` or `[visualization.tts]`
/// table as well, which wins for that product.
#[derive(Default, Deserialize)]
struct TomlVisualization {
    #[serde(
        default,
        deserialize_with = "super_engine_protocol::serde_helpers::deserialize_or_default"
    )]
    stt: Option<ProductVisualization>,
    #[serde(
        default,
        deserialize_with = "super_engine_protocol::serde_helpers::deserialize_or_default"
    )]
    tts: Option<ProductVisualization>,
    #[serde(flatten)]
    shared: ProductVisualization,
}

impl From<TomlConfig> for AppletConfig {
    fn from(toml: TomlConfig) -> Self {
        let TomlVisualization { stt, tts, shared } = toml.visualization;
        Self {
            visualization: VisualizationConfig {
                stt: stt.unwrap_or_else(|| shared.clone()),
                tts: tts.unwrap_or(shared),
            },
            ui: toml.ui,
        }
    }
}

impl AppletConfig {
    /// The applet's own config directory, `$XDG_CONFIG_HOME/super-cosmic-applet`,
    /// beside the products' own.
    pub fn config_dir() -> PathBuf {
        super_engine_protocol::paths::config_dir(Product::Stt.spec()).with_file_name(CLIENT_NAME)
    }

    fn settings_path() -> PathBuf {
        Self::config_dir().join(SETTINGS_FILE)
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

    /// Parse one of the TOML configs from before `settings.ron`. `None` if it
    /// doesn't parse.
    fn from_toml(content: &str) -> Option<Self> {
        match toml::from_str::<TomlConfig>(content) {
            Ok(config) => Some(config.into()),
            Err(e) => {
                warn!("Failed to parse the old applet config: {e}");
                None
            }
        }
    }

    /// Load the settings.
    ///
    /// Until the applet has saved `settings.ron`, it imports the TOML config
    /// written last: one of the applet's own per-side files, or if it has
    /// none, one the Super STT or Super TTS applet left. Every side picks the
    /// same file, so they start out agreeing. The TOML files are only read,
    /// never removed.
    pub fn load() -> Self {
        let path = Self::settings_path();
        match fs::read_to_string(&path) {
            Ok(content) => Self::parse_or_reset(&content).0,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::import_toml(),
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
        let own = OWN_SAVE.lock().unwrap_or_else(PoisonError::into_inner);
        let content = fs::read_to_string(path).ok()?;
        if own.as_deref() == Some(content.as_str()) {
            return None;
        }
        drop(own);
        ron::from_str(&content)
            .inspect_err(|e| debug!("Ignoring the settings file for now: {e}"))
            .ok()
    }

    fn import_toml() -> Self {
        let products = Product::ALL
            .iter()
            .map(|p| super_engine_protocol::paths::config_dir(p.spec()))
            .collect();
        let Some(path) = newest_toml(&[vec![Self::config_dir()], products]) else {
            return Self::default();
        };
        info!("Importing the applet's settings from {}", path.display());
        fs::read_to_string(&path)
            .ok()
            .and_then(|content| Self::from_toml(&content))
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.save_in(&Self::config_dir())
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
        let mut own = OWN_SAVE.lock().unwrap_or_else(PoisonError::into_inner);
        fs::write(&temporary, &content)?;
        fs::rename(&temporary, &path)?;
        *own = Some(content);
        debug!("Saved the applet's settings to {}", path.display());
        Ok(())
    }

    /// Apply an in-place mutation and persist it. Replaces the fan-out of
    /// `update_*` setters (Applet Tier 3 #21): `config.update(|c| c.ui.show_icon
    /// = v)` mutates then saves, logging (not propagating) a write failure.
    pub fn update(&mut self, mutate: impl FnOnce(&mut Self)) {
        mutate(self);
        if let Err(e) = self.save() {
            error!("Failed to save config: {e}");
        }
    }
}

/// The TOML config written last, from the first of `tiers` with any. A tier
/// is a set of directories that may hold `applet-{full,left,right}.toml`.
fn newest_toml(tiers: &[Vec<PathBuf>]) -> Option<PathBuf> {
    tiers.iter().find_map(|dirs| {
        dirs.iter()
            .flat_map(|dir| {
                ["full", "left", "right"].map(|side| dir.join(format!("applet-{side}.toml")))
            })
            .filter_map(|path| {
                let modified = fs::metadata(&path).and_then(|m| m.modified()).ok()?;
                Some((modified, path))
            })
            .max_by_key(|(modified, _)| *modified)
            .map(|(_, path)| path)
    })
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
mod toml_import_tests {
    //! The TOML configs from before `settings.ron`, which a first run imports.
    //! Every one a release wrote has to load, migrate, or reset, never crash.
    use super::AppletConfig;
    use crate::models::theme::{
        IconAlignment, VisualizationColor, VisualizationTheme, WorkingAnimationTheme,
    };
    use crate::products::Product;

    /// The committed v0.1.3 `applet-full.toml` fixture. The canonical copy lives
    /// in the on-disk corpus so the release gate and these detailed assertions
    /// test the same bytes.
    fn v0_1_3_applet_fixture() -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/configs/v0.1.3/applet-full.toml");
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
    }

    #[test]
    fn v0_1_3_full_applet_config_imports() {
        let cfg = AppletConfig::from_toml(&v0_1_3_applet_fixture())
            .expect("a valid v0.1.3 applet config must import");
        // Its one set of settings reaches both products.
        for look in [&cfg.visualization.stt, &cfg.visualization.tts] {
            assert_eq!(look.theme, VisualizationTheme::Waveform);
            assert_eq!(look.colors.light_colors, VisualizationColor::Blue);
            assert_eq!(look.colors.dark_colors, VisualizationColor::PastelGreen);
            // New field materializes at its default.
            assert_eq!(look.working_animation, WorkingAnimationTheme::default());
        }
        assert_eq!(cfg.ui.applet_width, 150);
        assert_eq!(cfg.ui.icon_alignment, IconAlignment::End);
    }

    #[test]
    fn all_published_applet_configs_import() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/configs");
        let mut checked = 0;
        for entry in std::fs::read_dir(&dir).expect("fixtures/configs dir must exist") {
            let version_dir = entry.expect("readable dir entry").path();
            if !version_dir.is_dir() {
                continue; // skip README.md and any other non-version files
            }
            for file in std::fs::read_dir(&version_dir).expect("readable version dir") {
                let path = file.expect("readable file entry").path();
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let is_toml = path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("toml"));
                if !(name.starts_with("applet-") && is_toml) {
                    continue;
                }
                let content = std::fs::read_to_string(&path).expect("read applet fixture");
                assert!(
                    AppletConfig::from_toml(&content).is_some(),
                    "applet fixture {} must import",
                    path.display()
                );
                checked += 1;
            }
        }
        assert!(
            checked >= 4,
            "expected >= 4 applet fixtures (v0.1.0-v0.1.3), found {checked}"
        );
    }

    #[test]
    fn a_corrupt_config_is_not_imported() {
        assert!(AppletConfig::from_toml("not ::: valid toml [").is_none());
    }

    #[test]
    fn applet_bad_theme_falls_back_preserving_rest() {
        // `last_popup_state` is an old, since-removed field; serde must ignore
        // it (forward compat) rather than reject the whole config. `icon_alignment`
        // carries a bad value and must fall back to its default without erroring.
        let toml_str = r#"
[visualization]
theme = "Nonexistent"
side = "Full"

[visualization.colors]
light_colors = "Blue"
dark_colors = "PastelGreen"

[ui]
last_popup_state = "None"
show_icon = true
icon_alignment = "sideways"
applet_width = 150
show_visualization = true
"#;
        let cfg = AppletConfig::from_toml(toml_str).expect("must parse, not error");
        for look in [&cfg.visualization.stt, &cfg.visualization.tts] {
            assert_eq!(look.theme, VisualizationTheme::default()); // reset
            assert_eq!(look.colors.light_colors, VisualizationColor::Blue); // preserved
        }
        assert_eq!(cfg.ui.icon_alignment, IconAlignment::default()); // bad value → default
        assert_eq!(cfg.ui.applet_width, 150); // preserved
        assert!(cfg.ui.show_icon);
    }

    #[test]
    fn a_product_without_a_table_takes_the_shared_settings() {
        let cfg = AppletConfig::from_toml(
            r#"
[visualization]
theme = "Pulse"
side = "Full"

[visualization.colors]
light_colors = "Red"
dark_colors = "PastelRed"

[visualization.tts]
theme = "Waveform"

[ui]
show_icon = true
applet_width = 120
show_visualization = true
"#,
        )
        .expect("must parse");

        let stt = cfg.visualization.product(Product::Stt);
        assert_eq!(stt.theme, VisualizationTheme::Pulse);
        assert_eq!(stt.colors.dark_colors, VisualizationColor::PastelRed);
        // A table of its own wins outright: what it leaves out is the default,
        // not the shared value.
        let tts = cfg.visualization.product(Product::Tts);
        assert_eq!(tts.theme, VisualizationTheme::Waveform);
        assert_eq!(tts.colors.dark_colors, VisualizationColor::SystemAccent);
    }

    #[test]
    fn a_malformed_product_table_falls_back_to_the_shared_settings() {
        let cfg = AppletConfig::from_toml(
            r#"
[visualization]
theme = "Pulse"
side = "Full"
stt = "not a table"

[ui]
show_icon = true
applet_width = 200
show_visualization = true
"#,
        )
        .expect("one bad table must not cost the whole config");

        assert_eq!(cfg.visualization.stt.theme, VisualizationTheme::Pulse);
        assert_eq!(cfg.ui.applet_width, 200);
    }
}

#[cfg(test)]
mod newest_toml_tests {
    //! Which TOML config a first run imports, when each side left its own.
    use super::newest_toml;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};

    /// A fresh directory for one test, removed when it ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("super-cosmic-applet-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("create scratch dir");
            Self(dir)
        }

        /// `dir/applet-{side}.toml`, last modified `secs` after the epoch.
        fn config(&self, dir: &str, side: &str, secs: u64) -> PathBuf {
            let dir = self.0.join(dir);
            fs::create_dir_all(&dir).expect("create config dir");
            let path = dir.join(format!("applet-{side}.toml"));
            fs::write(&path, "").expect("write config");
            fs::File::options()
                .write(true)
                .open(&path)
                .and_then(|f| f.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(secs)))
                .expect("set mtime");
            path
        }

        fn dir(&self, dir: &str) -> PathBuf {
            self.0.join(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn tiers(scratch: &Scratch, tiers: &[&[&str]]) -> Vec<Vec<PathBuf>> {
        tiers
            .iter()
            .map(|dirs| dirs.iter().map(|d| scratch.dir(d)).collect())
            .collect()
    }

    #[test]
    fn the_side_changed_last_wins() {
        let scratch = Scratch::new("newest-side");
        scratch.config("applet", "full", 100);
        let right = scratch.config("applet", "right", 300);
        scratch.config("applet", "left", 200);

        assert_eq!(
            newest_toml(&tiers(&scratch, &[&["applet"]])).as_deref(),
            Some(right.as_path())
        );
    }

    #[test]
    fn the_applets_own_configs_win_over_newer_product_ones() {
        let scratch = Scratch::new("own-first");
        let own = scratch.config("applet", "left", 100);
        scratch.config("stt", "left", 500);

        let found = newest_toml(&tiers(&scratch, &[&["applet"], &["stt", "tts"]]));

        assert_eq!(found.as_deref(), Some(own.as_path()));
    }

    #[test]
    fn without_its_own_the_newest_product_config_is_imported() {
        let scratch = Scratch::new("products");
        scratch.config("stt", "full", 100);
        let tts = scratch.config("tts", "full", 200);

        let found = newest_toml(&tiers(&scratch, &[&["applet"], &["stt", "tts"]]));

        assert_eq!(found.as_deref(), Some(tts.as_path()));
    }

    #[test]
    fn nothing_to_import_is_none() {
        let scratch = Scratch::new("nothing");
        assert_eq!(
            newest_toml(&tiers(&scratch, &[&["applet"], &["stt"]])),
            None::<PathBuf>
        );
        // The scratch dir exists but holds no config.
        assert!(Path::new(&scratch.0).is_dir());
    }
}
