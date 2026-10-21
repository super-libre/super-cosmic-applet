// SPDX-License-Identifier: GPL-3.0-only
//! Reading a setting so that a value the applet doesn't know takes its
//! default, rather than failing, and resetting, the whole settings file.
use log::warn;
use serde::de::{self, DeserializeOwned, Deserializer, IntoDeserializer, Visitor};
use std::fmt;

/// An enum setting, as its variant's name, or the default for a name it
/// doesn't have.
///
/// super-engine's `deserialize_or_default` does this by buffering the value
/// as JSON, which a RON variant doesn't survive: RON writes one as a bare
/// identifier, `Waveform`, and its self-describing reader takes that for a
/// unit struct and drops the name. This asks for an identifier instead. RON
/// only ever gives it one: anything else where a name goes doesn't parse,
/// and fails the file before it gets here.
pub fn variant_or_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned + Default,
{
    let name = deserializer.deserialize_identifier(VariantName)?;
    let value: Result<T, de::value::Error> = T::deserialize(name.as_str().into_deserializer());
    Ok(value.unwrap_or_else(|e| {
        warn!("Unknown setting {name:?} ({e}); using its default");
        T::default()
    }))
}

/// A variant's name.
struct VariantName;

impl Visitor<'_> for VariantName {
    type Value = String;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the name of a choice")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        Ok(v.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use crate::models::theme::{IconAlignment, VisualizationTheme};
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct Settings {
        #[serde(deserialize_with = "super::variant_or_default")]
        theme: VisualizationTheme,
        #[serde(deserialize_with = "super::variant_or_default")]
        icon_alignment: IconAlignment,
        width: u32,
    }

    #[test]
    fn ron_reads_a_variant_by_its_name() {
        let s: Settings =
            ron::from_str("(theme: Waveform, icon_alignment: center, width: 3)").unwrap();
        assert_eq!(s.theme, VisualizationTheme::Waveform);
        assert_eq!(s.icon_alignment, IconAlignment::Center);
        assert_eq!(s.width, 3);
    }

    #[test]
    fn an_unknown_name_takes_the_default_and_keeps_the_rest() {
        let s: Settings =
            ron::from_str("(theme: Sparkles, icon_alignment: sideways, width: 3)").unwrap();
        assert_eq!(s.theme, VisualizationTheme::default());
        assert_eq!(s.icon_alignment, IconAlignment::default());
        assert_eq!(s.width, 3);
    }

    /// Only a name gets that far: RON refuses anything else where one goes,
    /// and the error says what was wanted.
    #[test]
    fn a_value_that_isnt_a_name_says_what_it_wanted() {
        use serde::de::value::{BytesDeserializer, Error};

        let bytes: Result<VisualizationTheme, Error> =
            super::variant_or_default(BytesDeserializer::new(b"Pulse"));

        let error = bytes.expect_err("bytes are not a name");
        assert!(
            error.to_string().contains("the name of a choice"),
            "{error}"
        );
    }
}
