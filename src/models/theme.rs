// SPDX-License-Identifier: GPL-3.0-only
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum VisualizationTheme {
    Pulse,
    BottomEqualizer,
    #[default]
    CenteredEqualizer,
    Waveform,
}

impl std::fmt::Display for VisualizationTheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VisualizationTheme::Pulse => write!(f, "pulse"),
            VisualizationTheme::BottomEqualizer => write!(f, "b_equalizer"),
            VisualizationTheme::CenteredEqualizer => write!(f, "c_equalizer"),
            VisualizationTheme::Waveform => write!(f, "waveform"),
        }
    }
}

impl std::str::FromStr for VisualizationTheme {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pulse" => Ok(VisualizationTheme::Pulse),
            "b_equalizer" => Ok(VisualizationTheme::BottomEqualizer),
            "c_equalizer" => Ok(VisualizationTheme::CenteredEqualizer),
            "waveform" => Ok(VisualizationTheme::Waveform),
            _ => Err(()),
        }
    }
}

impl VisualizationTheme {
    pub fn pretty_name(&self) -> String {
        match self {
            VisualizationTheme::Pulse => "Pulse".to_string(),
            VisualizationTheme::BottomEqualizer => "Equalizer".to_string(),
            VisualizationTheme::CenteredEqualizer => "Centered Bars".to_string(),
            VisualizationTheme::Waveform => "Waveform".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WorkingAnimationTheme {
    /// Damped water-droplet ripple radiating from the center.
    #[default]
    Droplet,
    /// Glowing dot tracing a sine path with a fading trail.
    Comet,
    /// Three dots pulsing in sequence — a compact loading indicator. Exempt
    /// from the side split: renders centred in full on every applet.
    Dots,
}

impl std::fmt::Display for WorkingAnimationTheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkingAnimationTheme::Droplet => write!(f, "droplet"),
            WorkingAnimationTheme::Comet => write!(f, "comet"),
            WorkingAnimationTheme::Dots => write!(f, "dots"),
        }
    }
}

impl std::str::FromStr for WorkingAnimationTheme {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "droplet" => Ok(WorkingAnimationTheme::Droplet),
            "comet" => Ok(WorkingAnimationTheme::Comet),
            "dots" => Ok(WorkingAnimationTheme::Dots),
            _ => Err(()),
        }
    }
}

impl WorkingAnimationTheme {
    pub fn pretty_name(self) -> String {
        match self {
            WorkingAnimationTheme::Droplet => "Droplet".to_string(),
            WorkingAnimationTheme::Comet => "Comet".to_string(),
            WorkingAnimationTheme::Dots => "Dots".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum VisualizationSide {
    #[default]
    Full,
    Left,
    Right,
}

impl std::fmt::Display for VisualizationSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VisualizationSide::Full => write!(f, "full"),
            VisualizationSide::Left => write!(f, "left"),
            VisualizationSide::Right => write!(f, "right"),
        }
    }
}

impl std::str::FromStr for VisualizationSide {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "full" => Ok(VisualizationSide::Full),
            "left" => Ok(VisualizationSide::Left),
            "right" => Ok(VisualizationSide::Right),
            _ => Err(()),
        }
    }
}

impl VisualizationSide {
    #[must_use]
    pub fn pretty_name(&self) -> String {
        match self {
            VisualizationSide::Full => "Full Wave".to_string(),
            VisualizationSide::Left => "Left Side".to_string(),
            VisualizationSide::Right => "Right Side".to_string(),
        }
    }
}

/// Where the panel icon sits when visualizations are hidden. Persisted in the
/// applet config as its snake-case wire id (`start`/`center`/`end`) and mapped
/// to an iced [`Alignment`] at render time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum IconAlignment {
    Start,
    Center,
    #[default]
    End,
}

impl IconAlignment {
    /// Human-facing label for the segmented selector.
    pub fn pretty_name(self) -> &'static str {
        match self {
            IconAlignment::Start => "Start",
            IconAlignment::Center => "Center",
            IconAlignment::End => "End",
        }
    }

    /// Map to the iced cross-axis alignment used when placing the panel icon.
    pub fn to_alignment(self) -> cosmic::iced::Alignment {
        match self {
            IconAlignment::Start => cosmic::iced::Alignment::Start,
            IconAlignment::Center => cosmic::iced::Alignment::Center,
            IconAlignment::End => cosmic::iced::Alignment::End,
        }
    }
}

impl std::fmt::Display for IconAlignment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IconAlignment::Start => write!(f, "start"),
            IconAlignment::Center => write!(f, "center"),
            IconAlignment::End => write!(f, "end"),
        }
    }
}

impl std::str::FromStr for IconAlignment {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "start" => Ok(IconAlignment::Start),
            "center" => Ok(IconAlignment::Center),
            "end" => Ok(IconAlignment::End),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod working_animation_theme_tests {
    use super::WorkingAnimationTheme;

    #[test]
    fn display_from_str_round_trip() {
        for t in [
            WorkingAnimationTheme::Droplet,
            WorkingAnimationTheme::Comet,
            WorkingAnimationTheme::Dots,
        ] {
            assert_eq!(t.to_string().parse::<WorkingAnimationTheme>(), Ok(t));
        }
    }

    #[test]
    fn unknown_is_rejected() {
        assert_eq!("nope".parse::<WorkingAnimationTheme>(), Err(()));
        assert_eq!(
            WorkingAnimationTheme::default(),
            WorkingAnimationTheme::Droplet
        );
    }
}

#[cfg(test)]
mod icon_alignment_tests {
    use super::IconAlignment;

    #[test]
    fn display_from_str_round_trip() {
        for a in [
            IconAlignment::Start,
            IconAlignment::Center,
            IconAlignment::End,
        ] {
            assert_eq!(a.to_string().parse::<IconAlignment>(), Ok(a));
        }
    }

    #[test]
    fn wire_ids_are_snake_case_lowercase() {
        assert_eq!(IconAlignment::Start.to_string(), "start");
        assert_eq!(IconAlignment::Center.to_string(), "center");
        assert_eq!(IconAlignment::End.to_string(), "end");
    }

    #[test]
    fn unknown_is_rejected_and_default_is_end() {
        assert_eq!("middle".parse::<IconAlignment>(), Err(()));
        assert_eq!(IconAlignment::default(), IconAlignment::End);
    }

    #[test]
    fn serde_uses_the_wire_id() {
        // The config persists the lowercase wire id, not the PascalCase variant.
        let json = serde_json::to_string(&IconAlignment::Center).expect("serialize");
        assert_eq!(json, "\"center\"");
        let back: IconAlignment = serde_json::from_str("\"end\"").expect("deserialize");
        assert_eq!(back, IconAlignment::End);
    }

    #[test]
    fn each_alignment_places_the_icon_where_it_says() {
        use cosmic::iced::Alignment;
        assert_eq!(IconAlignment::Start.to_alignment(), Alignment::Start);
        assert_eq!(IconAlignment::Center.to_alignment(), Alignment::Center);
        assert_eq!(IconAlignment::End.to_alignment(), Alignment::End);
    }

    #[test]
    fn the_selector_labels_each_alignment() {
        assert_eq!(IconAlignment::Start.pretty_name(), "Start");
        assert_eq!(IconAlignment::Center.pretty_name(), "Center");
        assert_eq!(IconAlignment::End.pretty_name(), "End");
    }
}

#[cfg(test)]
mod visualization_theme_tests {
    use super::VisualizationTheme;

    const ALL: [VisualizationTheme; 4] = [
        VisualizationTheme::Pulse,
        VisualizationTheme::BottomEqualizer,
        VisualizationTheme::CenteredEqualizer,
        VisualizationTheme::Waveform,
    ];

    /// The selector hands a theme back as its `Display` id, so every theme
    /// must parse back from it.
    #[test]
    fn display_from_str_round_trip() {
        for theme in ALL {
            assert_eq!(theme.to_string().parse::<VisualizationTheme>(), Ok(theme));
        }
    }

    #[test]
    fn unknown_is_rejected_and_default_is_centered_bars() {
        assert_eq!("sparkles".parse::<VisualizationTheme>(), Err(()));
        assert_eq!(
            VisualizationTheme::default(),
            VisualizationTheme::CenteredEqualizer
        );
    }

    /// The configs every release wrote name a theme by its variant, so the
    /// variant names are a wire format.
    #[test]
    fn serde_uses_the_variant_names_old_configs_hold() {
        for (theme, name) in
            ALL.into_iter()
                .zip(["Pulse", "BottomEqualizer", "CenteredEqualizer", "Waveform"])
        {
            assert_eq!(
                serde_json::to_string(&theme).expect("serialize"),
                format!("\"{name}\"")
            );
        }
    }

    #[test]
    fn every_theme_has_its_own_label() {
        let labels: std::collections::BTreeSet<String> =
            ALL.iter().map(VisualizationTheme::pretty_name).collect();
        assert_eq!(labels.len(), ALL.len());
        assert!(labels.iter().all(|label| !label.is_empty()));
    }
}

#[cfg(test)]
mod working_animation_label_tests {
    use super::WorkingAnimationTheme;

    #[test]
    fn every_animation_has_its_own_label() {
        assert_eq!(WorkingAnimationTheme::Droplet.pretty_name(), "Droplet");
        assert_eq!(WorkingAnimationTheme::Comet.pretty_name(), "Comet");
        assert_eq!(WorkingAnimationTheme::Dots.pretty_name(), "Dots");
    }
}

#[cfg(test)]
mod visualization_side_tests {
    use super::VisualizationSide;

    const ALL: [VisualizationSide; 3] = [
        VisualizationSide::Full,
        VisualizationSide::Left,
        VisualizationSide::Right,
    ];

    /// The launch flag names a side the way `Display` does.
    #[test]
    fn display_from_str_round_trip() {
        for side in ALL {
            assert_eq!(side.to_string().parse::<VisualizationSide>(), Ok(side));
        }
        assert_eq!(VisualizationSide::Left.to_string(), "left");
    }

    #[test]
    fn unknown_is_rejected_and_default_is_full() {
        assert_eq!("middle".parse::<VisualizationSide>(), Err(()));
        assert_eq!(VisualizationSide::default(), VisualizationSide::Full);
    }

    #[test]
    fn every_side_has_its_own_label() {
        let labels: std::collections::BTreeSet<String> =
            ALL.iter().map(VisualizationSide::pretty_name).collect();
        assert_eq!(labels.len(), ALL.len());
    }
}
