// SPDX-License-Identifier: GPL-3.0-only
//! Reading a setting so that a value the applet doesn't know takes its
//! default, rather than failing, and resetting, the whole settings file.
use log::warn;
use serde::de::{
    self, DeserializeOwned, Deserializer, IgnoredAny, IntoDeserializer, MapAccess, SeqAccess,
    Visitor,
};
use std::fmt;

/// An enum setting, as its variant's name, or the default for a name it
/// doesn't have.
///
/// super-engine's `deserialize_or_default` does this by buffering the value
/// as JSON, which a RON variant doesn't survive: RON writes one as a bare
/// identifier, `Waveform`, and its self-describing reader takes that for a
/// unit struct and drops the name. This asks for an identifier instead, which
/// RON reads as the name and TOML as its string.
pub fn variant_or_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned + Default,
{
    let Some(name) = deserializer.deserialize_identifier(VariantName)? else {
        warn!("A setting holds a value that can't be a choice; using its default");
        return Ok(T::default());
    };
    let value: Result<T, de::value::Error> = T::deserialize(name.as_str().into_deserializer());
    Ok(value.unwrap_or_else(|e| {
        warn!("Unknown setting {name:?} ({e}); using its default");
        T::default()
    }))
}

/// A variant's name, or `None` for a value that can't be one: a number, a
/// table. Those are read through, so the rest of the file still parses.
struct VariantName;

impl<'de> Visitor<'de> for VariantName {
    type Value = Option<String>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the name of a choice")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        Ok(Some(v.to_owned()))
    }

    fn visit_bool<E: de::Error>(self, _: bool) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_i64<E: de::Error>(self, _: i64) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_u64<E: de::Error>(self, _: u64) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_f64<E: de::Error>(self, _: f64) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        while seq.next_element::<IgnoredAny>()?.is_some() {}
        Ok(None)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(None)
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
    fn toml_reads_a_variant_from_its_string() {
        let s: Settings =
            toml::from_str("theme = \"Waveform\"\nicon_alignment = \"center\"\nwidth = 3").unwrap();
        assert_eq!(s.theme, VisualizationTheme::Waveform);
        assert_eq!(s.icon_alignment, IconAlignment::Center);
    }

    #[test]
    fn an_unknown_name_takes_the_default_and_keeps_the_rest() {
        let s: Settings =
            ron::from_str("(theme: Sparkles, icon_alignment: sideways, width: 3)").unwrap();
        assert_eq!(s.theme, VisualizationTheme::default());
        assert_eq!(s.icon_alignment, IconAlignment::default());
        assert_eq!(s.width, 3);
    }

    #[test]
    fn a_value_that_cant_be_a_name_takes_the_default_and_keeps_the_rest() {
        let s: Settings =
            toml::from_str("width = 3\nicon_alignment = 7\n[theme]\nnested = true").unwrap();
        assert_eq!(s.theme, VisualizationTheme::default());
        assert_eq!(s.icon_alignment, IconAlignment::default());
        assert_eq!(s.width, 3);
    }

    /// Whatever a hand edit puts where a name goes is read through, so the
    /// settings after it still load.
    #[test]
    fn every_kind_of_wrong_value_takes_the_default() {
        for wrong in ["true", "1.5", "[1, \"Pulse\"]"] {
            let s: Settings = toml::from_str(&format!(
                "theme = {wrong}\nicon_alignment = \"start\"\nwidth = 3"
            ))
            .unwrap_or_else(|e| panic!("{wrong} failed the whole file: {e}"));
            assert_eq!(s.theme, VisualizationTheme::default(), "for {wrong}");
            assert_eq!(s.icon_alignment, IconAlignment::Start, "after {wrong}");
        }
    }

    /// Unsigned numbers and units come from formats other than RON and TOML,
    /// through any self-describing reader.
    #[test]
    fn unsigned_numbers_and_units_take_the_default() {
        use serde::de::{IntoDeserializer, value::Error};

        let number: Result<VisualizationTheme, Error> =
            super::variant_or_default(IntoDeserializer::<Error>::into_deserializer(5_u64));
        let unit: Result<VisualizationTheme, Error> =
            super::variant_or_default(IntoDeserializer::<Error>::into_deserializer(()));

        assert_eq!(number, Ok(VisualizationTheme::default()));
        assert_eq!(unit, Ok(VisualizationTheme::default()));
    }

    /// Bytes are the one value it can't read through; the error the reset
    /// logs says what was wanted.
    #[test]
    fn a_value_it_cant_read_through_says_what_it_wanted() {
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
