// SPDX-License-Identifier: GPL-3.0-only
use crate::config::FREQUENCY_NORMALIZATION_MAX;
use crate::ui::components::visualizations::{
    DrawContext, VisualizationConfig, VisualizationRenderer,
};
use crate::util::usize_to_f32;
use cosmic::iced::{
    Padding, Point, Radius,
    widget::canvas::{Fill, Frame, path, stroke},
};

/// A horizontal line pulse that grows in height/thickness with audio intensity
pub struct PulseVisualization {
    config: VisualizationConfig,
}

impl Default for PulseVisualization {
    fn default() -> Self {
        Self {
            config: VisualizationConfig {
                margins: Padding {
                    top: 2.0,
                    right: 2.0,
                    bottom: 2.0,
                    left: 2.0,
                },
                corner_radius: Radius::new(0.0),
                min_element_height: 4.0,
                height_safety_margin: 1.0,
            },
        }
    }
}

impl VisualizationRenderer for PulseVisualization {
    fn draw(&self, frame: &mut Frame<cosmic::Renderer>, ctx: &DrawContext) {
        let DrawContext {
            bounds,
            frequency_data,
            color_config,
            is_dark,
            cosmic_theme,
            ..
        } = *ctx;
        let effective_bounds = self.config.effective_bounds(bounds);

        let normalization_factor = 1.0 / FREQUENCY_NORMALIZATION_MAX;

        // Focus on vocal frequencies (bands 8-24) for pulse intensity.
        let vocal_start = 8;
        let vocal_end = 24;
        let mut vocal_energy = 0.0;
        let mut vocal_bands = 0;

        for i in vocal_start..vocal_end.min(frequency_data.bands.len()) {
            vocal_energy += frequency_data.bands[i];
            vocal_bands += 1;
        }

        let average_vocal_energy = if vocal_bands > 0 {
            vocal_energy / usize_to_f32(vocal_bands)
        } else {
            frequency_data.total_energy
        };

        // Normalize and create pulse intensity
        let pulse_intensity = (average_vocal_energy * normalization_factor).min(1.0);

        // The line always spans the full width of the effective bounds
        let line_width = effective_bounds.width;

        // Calculate line height/thickness based on audio intensity
        let min_height = 2.0; // Minimum visible height when silent
        let max_height = effective_bounds.height * 0.8; // Use up to 80% of available height

        // Create pulsing height based on audio intensity
        let line_height = min_height + (max_height - min_height) * pulse_intensity;

        // Position: always start at left edge (x), centered vertically
        let x = effective_bounds.x;
        let y = effective_bounds.y + (effective_bounds.height - line_height) / 2.0; // Center vertically

        // Draw the full-width horizontal line with rounded ends
        let border_radius = line_height / 2.0; // Make the ends fully rounded (pill shape)
        let mut path_builder = path::Builder::new();
        path_builder.rounded_rectangle(
            Point { x, y },
            cosmic::iced::Size::new(line_width, line_height),
            border_radius.into(),
        );

        let base = color_config.get_color_with_theme(is_dark, cosmic_theme);

        let path = path_builder.build();
        frame.fill(
            &path,
            Fill {
                style: stroke::Style::Solid(base),
                ..Default::default()
            },
        );
    }
}

#[cfg(test)]
mod pulse_tests {
    //! The pulse, drawn: one pill across the strip, 2px in from every edge,
    //! thickening with the energy in the vocal bands.
    use super::PulseVisualization;
    use crate::models::theme::{VisualizationColor, VisualizationSide};
    use crate::ui::components::visualizations::test_support::{
        HEIGHT, Pixels, bands, flat_bands, render_visualization,
    };
    use super_engine_protocol::FrequencyData;

    fn draw(data: &FrequencyData) -> Pixels {
        render_visualization(
            &PulseVisualization::default(),
            data,
            &VisualizationSide::Full,
            VisualizationColor::White,
        )
    }

    fn thickness(pixels: &Pixels) -> i32 {
        let (_, top, _, bottom) = pixels.painted_box().expect("a line");
        bottom - top + 1
    }

    #[test]
    fn silence_draws_a_thin_line_across_the_strip() {
        let pixels = draw(&flat_bands(32, 0.0));
        let (left, top, right, bottom) = pixels.painted_box().expect("a line");
        assert!(left <= 3 && right >= 156, "line spans {left}..{right}");
        assert!(
            bottom - top <= 2,
            "a silent line {} rows thick",
            bottom - top + 1
        );
        assert!(
            (top + bottom - (HEIGHT as i32 - 1)).abs() <= 1,
            "off centre"
        );
    }

    #[test]
    fn the_line_thickens_with_vocal_energy_and_stays_centred() {
        let mut previous = 0;
        for level in [0.0, 1.0, 2.0, 3.0] {
            let pixels = draw(&flat_bands(32, level));
            let thick = thickness(&pixels);
            assert!(thick > previous, "level {level}: {thick} rows");
            let (_, top, _, bottom) = pixels.painted_box().unwrap();
            assert!(
                (top + bottom - (HEIGHT as i32 - 1)).abs() <= 1,
                "level {level} off centre"
            );
            previous = thick;
        }
        // At its fullest the line takes 80% of the 36 rows inside the margin.
        assert!((28..=31).contains(&previous), "{previous} rows at full");
    }

    /// Only bands 8 to 23, the voice's, move the pulse: a hum below them or
    /// hiss above leaves it resting.
    #[test]
    fn only_the_vocal_bands_count() {
        let outside: Vec<f32> = (0..32)
            .map(|i| if (8..24).contains(&i) { 0.0 } else { 3.0 })
            .collect();
        let inside: Vec<f32> = (0..32)
            .map(|i| if (8..24).contains(&i) { 3.0 } else { 0.0 })
            .collect();

        assert_eq!(
            thickness(&draw(&bands(outside))),
            thickness(&draw(&flat_bands(32, 0.0)))
        );
        assert!(thickness(&draw(&bands(inside))) >= 28);
    }

    /// Without bands to read, the pulse follows the total energy instead.
    #[test]
    fn without_bands_it_follows_the_total_energy() {
        let loud = FrequencyData {
            bands: Vec::new(),
            total_energy: 3.0,
            ..FrequencyData::default()
        };
        assert!(thickness(&draw(&loud)) >= 28);
    }
}
