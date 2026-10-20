// SPDX-License-Identifier: GPL-3.0-only
use crate::VisualizationSide;
use crate::models::theme::{
    IconAlignment, VisualizationColorConfig, VisualizationTheme, WorkingAnimationTheme,
};
use crate::products::{CLIENT_NAME, Product};
use log::{debug, error, warn};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppletConfig {
    pub visualization: VisualizationConfig,
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "VisualizationConfigOnDisk")]
pub struct VisualizationConfig {
    // This will be fixed per binary but stored for completeness.
    pub side: VisualizationSide,
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
    #[serde(
        default,
        deserialize_with = "super_engine_protocol::serde_helpers::deserialize_or_default"
    )]
    pub theme: VisualizationTheme,
    /// Animation shown while the product works: transcribing for Super STT,
    /// synthesizing the first audio for Super TTS.
    #[serde(
        default,
        deserialize_with = "super_engine_protocol::serde_helpers::deserialize_or_default"
    )]
    pub working_animation: WorkingAnimationTheme,
    #[serde(default)]
    pub colors: VisualizationColorConfig,
}

/// [`VisualizationConfig`] as a file holds it. A config from before each
/// product had its own table keeps one set of settings at the top of
/// `[visualization]`; that set seeds every product without a table, so an
/// upgrade draws both products the way the user had set the panel up.
#[derive(Deserialize)]
struct VisualizationConfigOnDisk {
    #[serde(
        default,
        deserialize_with = "super_engine_protocol::serde_helpers::deserialize_or_default"
    )]
    side: VisualizationSide,
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

impl From<VisualizationConfigOnDisk> for VisualizationConfig {
    fn from(disk: VisualizationConfigOnDisk) -> Self {
        Self {
            side: disk.side,
            stt: disk.stt.unwrap_or_else(|| disk.shared.clone()),
            tts: disk.tts.unwrap_or(disk.shared),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub show_icon: bool,
    #[serde(
        default,
        deserialize_with = "super_engine_protocol::serde_helpers::deserialize_or_default"
    )]
    pub icon_alignment: IconAlignment,
    /// How far the visualization runs along the panel, in pixels:
    /// its width on a top or bottom panel, its height on a side one.
    pub applet_width: u32,
    pub show_visualization: bool, // Whether to show visualizations when recording
}

impl Default for AppletConfig {
    fn default() -> Self {
        Self {
            visualization: VisualizationConfig {
                side: VisualizationSide::Full,
                stt: ProductVisualization::default(),
                tts: ProductVisualization::default(),
            },
            ui: UiConfig {
                show_icon: true,
                icon_alignment: IconAlignment::End,
                applet_width: 120,        // Default width in pixels
                show_visualization: true, // Default to showing visualizations when recording
            },
        }
    }
}

impl AppletConfig {
    /// The applet's own config directory, `$XDG_CONFIG_HOME/super-cosmic-applet`,
    /// beside the products' own.
    fn config_dir() -> PathBuf {
        super_engine_protocol::paths::config_dir(Product::Stt.spec()).with_file_name(CLIENT_NAME)
    }

    /// Get the config file path for a specific applet variant
    fn get_config_path(variant: &str) -> PathBuf {
        Self::config_dir().join(format!("applet-{variant}.toml"))
    }

    /// Where the product applets this one replaces kept the same variant's
    /// config, in the order a first run imports them from.
    fn legacy_config_paths(variant: &str) -> Vec<PathBuf> {
        Product::ALL
            .iter()
            .map(|p| {
                super_engine_protocol::paths::config_dir(p.spec())
                    .join(format!("applet-{variant}.toml"))
            })
            .collect()
    }

    /// Parse applet config `content`, falling back to defaults on a parse
    /// error. Pure (no I/O) so the load/reset decision is unit-testable.
    /// Returns the config and whether a reset occurred.
    fn parse_or_reset(content: &str) -> (Self, bool) {
        match toml::from_str::<AppletConfig>(content) {
            Ok(config) => (config, false),
            Err(e) => {
                warn!("Failed to parse applet config: {e}. Using defaults.");
                (Self::default(), true)
            }
        }
    }

    /// Load configuration from disk for a specific variant
    ///
    /// Until the applet has saved a config of its own, it reads the one the
    /// Super STT applet, or else the Super TTS applet, left for the same side,
    /// so an upgrade keeps the user's visualization settings. Those files are
    /// only read, never removed.
    pub fn load(variant: &str, vis_side: VisualizationSide) -> Self {
        let own = Self::get_config_path(variant);
        let found = std::iter::once(own)
            .chain(Self::legacy_config_paths(variant))
            .find_map(|path| {
                fs::read_to_string(&path)
                    .ok()
                    .map(|content| (path, content))
            });

        let mut config = match found {
            Some((path, content)) => {
                debug!("Loaded applet config from {}", path.display());
                Self::parse_or_reset(&content).0
            }
            None => Self::default(),
        };
        // Always override the vis_side with the binary-specific value.
        config.visualization.side = vis_side;
        config
    }

    /// Save configuration to disk. The variant (which file to write) is derived
    /// from `visualization.side`, which `load` fixes to the binary-specific
    /// value — so callers no longer thread a `variant` string around.
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let variant = Self::get_variant_name(&self.visualization.side);
        let config_path = Self::get_config_path(variant);

        // Create config directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let toml_content = toml::to_string_pretty(self)?;
        fs::write(&config_path, toml_content)?;

        debug!("Saved config for {variant} to {}", config_path.display());
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

    /// Get the variant name based on `VisualizationSide`
    pub fn get_variant_name(vis_side: &VisualizationSide) -> &'static str {
        match vis_side {
            VisualizationSide::Full => "full",
            VisualizationSide::Left => "left",
            VisualizationSide::Right => "right",
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
    fn round_trips_through_toml() {
        let mut cfg = AppletConfig::default();
        cfg.visualization.stt.working_animation = WorkingAnimationTheme::Comet;
        let s = toml::to_string_pretty(&cfg).expect("serialize");
        let back: AppletConfig = toml::from_str(&s).expect("deserialize");
        assert_eq!(
            back.visualization.stt.working_animation,
            WorkingAnimationTheme::Comet
        );
    }

    #[test]
    fn old_config_without_field_still_loads() {
        // Build a TOML string from a default config but with the
        // working_animation line removed, simulating a pre-existing config
        // file from before the field existed. It must still parse (serde
        // default) and yield Droplet — not error out and lose settings.
        let full = toml::to_string_pretty(&AppletConfig::default()).expect("serialize");
        let without: String = full
            .lines()
            .filter(|l| !l.trim_start().starts_with("working_animation"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !without.contains("working_animation"),
            "test setup: line not removed"
        );
        let cfg: AppletConfig = toml::from_str(&without).expect("old config must still parse");
        for look in [&cfg.visualization.stt, &cfg.visualization.tts] {
            assert_eq!(look.working_animation, WorkingAnimationTheme::Droplet);
        }
    }
}

#[cfg(test)]
mod per_product_tests {
    //! Each product has its own `[visualization.stt]` or `[visualization.tts]`
    //! table. Every config written before that keeps one set of settings at
    //! the top of `[visualization]`, which has to reach both products.
    use super::{AppletConfig, ProductVisualization};
    use crate::models::theme::{VisualizationColor, VisualizationTheme, WorkingAnimationTheme};
    use crate::products::Product;

    #[test]
    fn each_product_keeps_its_own_settings_through_a_save() {
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
        cfg.visualization.tts.colors.dark_colors = VisualizationColor::Orange;

        let saved = toml::to_string_pretty(&cfg).expect("serialize");
        let (back, was_reset) = AppletConfig::parse_or_reset(&saved);

        assert!(!was_reset, "a saved config must load:\n{saved}");
        assert_eq!(back.visualization.stt, cfg.visualization.stt);
        assert_eq!(back.visualization.tts, cfg.visualization.tts);
    }

    #[test]
    fn a_product_without_a_table_takes_the_shared_settings() {
        let cfg: AppletConfig = toml::from_str(
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
    fn a_malformed_product_table_falls_back_without_a_reset() {
        let (cfg, was_reset) = AppletConfig::parse_or_reset(
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
        );

        assert!(!was_reset, "one bad table must not cost the whole config");
        assert_eq!(cfg.visualization.stt.theme, VisualizationTheme::Pulse);
        assert_eq!(cfg.ui.applet_width, 200);
    }
}

#[cfg(test)]
mod upgrade_compat_tests {
    use super::AppletConfig;
    use crate::models::theme::{
        IconAlignment, VisualizationColor, VisualizationTheme, WorkingAnimationTheme,
    };

    /// The committed v0.1.3 `applet-full.toml` fixture. The canonical copy lives
    /// in the on-disk corpus so the release gate and these detailed assertions
    /// test the same bytes.
    fn v0_1_3_applet_fixture() -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/configs/v0.1.3/applet-full.toml");
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
    }

    #[test]
    fn v0_1_3_full_applet_config_loads() {
        let (cfg, was_reset) = AppletConfig::parse_or_reset(&v0_1_3_applet_fixture());
        assert!(
            !was_reset,
            "a valid v0.1.3 applet config must load, not reset"
        );
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
    fn all_published_applet_configs_load_cleanly() {
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
                let (_, was_reset) = AppletConfig::parse_or_reset(&content);
                assert!(
                    !was_reset,
                    "applet fixture {} must load cleanly (no reset)",
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
    fn corrupt_applet_config_resets_to_default() {
        let (cfg, was_reset) = AppletConfig::parse_or_reset("not ::: valid toml [");
        assert!(was_reset, "garbage input must trigger a reset");
        assert_eq!(cfg.visualization.stt.theme, VisualizationTheme::default());
        assert_eq!(cfg.visualization.tts.theme, VisualizationTheme::default());
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
        let cfg: AppletConfig = toml::from_str(toml_str).expect("must parse, not error");
        for look in [&cfg.visualization.stt, &cfg.visualization.tts] {
            assert_eq!(look.theme, VisualizationTheme::default()); // reset
            assert_eq!(look.colors.light_colors, VisualizationColor::Blue); // preserved
        }
        assert_eq!(cfg.ui.icon_alignment, IconAlignment::default()); // bad value → default
        assert_eq!(cfg.ui.applet_width, 150); // preserved
        assert!(cfg.ui.show_icon);
    }
}
