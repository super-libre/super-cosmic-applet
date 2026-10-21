// SPDX-License-Identifier: GPL-3.0-only
use crate::ui::components::visualizations::{
    BarAnchor, DrawContext, VisualizationConfig, VisualizationRenderer, render_bars,
};
use cosmic::iced::{Padding, Radius, widget::canvas::Frame};

/// Bars that are aligned to the bottom of the screen.
pub struct EqualizerVisualization {
    config: VisualizationConfig,
}

impl Default for EqualizerVisualization {
    fn default() -> Self {
        Self {
            config: VisualizationConfig {
                margins: Padding {
                    top: 1.0,
                    right: 2.0,
                    bottom: 0.0,
                    left: 2.0,
                },
                corner_radius: Radius::new(1.0),
                min_element_height: 4.0,
                height_safety_margin: 0.0,
            },
        }
    }
}

impl VisualizationRenderer for EqualizerVisualization {
    fn draw(&self, frame: &mut Frame<cosmic::Renderer>, ctx: &DrawContext) {
        render_bars(&self.config, BarAnchor::Bottom, frame, ctx);
    }
}

#[cfg(test)]
mod equalizer_tests {
    //! The bottom equalizer, drawn: bars standing on the strip's bottom edge,
    //! with a 2px margin either side and 1px above.
    use super::EqualizerVisualization;
    use crate::models::theme::{VisualizationColor, VisualizationSide};
    use crate::ui::components::visualizations::test_support::{
        HEIGHT, Pixels, bands, flat_bands, render_visualization,
    };
    use crate::util::usize_to_f32;

    fn draw(data: &super_engine_protocol::FrequencyData, side: &VisualizationSide) -> Pixels {
        render_visualization(
            &EqualizerVisualization::default(),
            data,
            side,
            VisualizationColor::White,
        )
    }

    #[test]
    fn bars_stand_on_the_bottom_edge_at_any_level() {
        for level in [0.0, 0.7, 1.5, 3.0] {
            let (left, top, right, bottom) = draw(&flat_bands(32, level), &VisualizationSide::Full)
                .painted_box()
                .expect("bars");
            assert_eq!(bottom, HEIGHT as i32 - 1, "level {level}");
            assert!(left >= 2 && right <= 157, "level {level}: {left}..{right}");
            assert!(top >= 1, "level {level}: top {top}");
        }
    }

    #[test]
    fn silence_leaves_minimum_bars_on_the_bottom() {
        let (_, top, _, _) = draw(&flat_bands(32, 0.0), &VisualizationSide::Full)
            .painted_box()
            .expect("bars");
        assert!(
            top >= HEIGHT as i32 - 5,
            "silent bars reach up to row {top}"
        );
    }

    /// The halves split the spectrum: with energy rising with frequency, the
    /// right applet's bars, the high half, stand taller than the left's, and
    /// each half rises left to right.
    #[test]
    fn left_and_right_each_draw_their_half_of_the_spectrum() {
        let rising = bands((0..32).map(|i| usize_to_f32(i) / 10.0).collect());
        let left = draw(&rising, &VisualizationSide::Left);
        let right = draw(&rising, &VisualizationSide::Right);

        assert!(
            right.ink() > left.ink() * 2,
            "{} vs {}",
            right.ink(),
            left.ink()
        );
        for half in [&left, &right] {
            let rows = 0..HEIGHT as i32;
            assert!(half.ink_in(80..160, rows.clone()) > half.ink_in(0..80, rows));
        }
    }
}
