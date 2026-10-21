// SPDX-License-Identifier: GPL-3.0-only
use crate::ui::components::visualizations::{
    BarAnchor, DrawContext, VisualizationConfig, VisualizationRenderer, render_bars,
};
use cosmic::iced::{Padding, Radius, widget::canvas::Frame};

/// Bars that are centered vertically within the drawable bounds.
pub struct CenteredBarsVisualization {
    config: VisualizationConfig,
}

impl Default for CenteredBarsVisualization {
    fn default() -> Self {
        Self {
            config: VisualizationConfig {
                margins: Padding {
                    top: 1.0,
                    right: 0.0,
                    bottom: 1.0,
                    left: 0.0,
                },
                corner_radius: Radius::new(24.0),
                min_element_height: 4.0,
                height_safety_margin: 0.0,
            },
        }
    }
}

impl VisualizationRenderer for CenteredBarsVisualization {
    fn draw(&self, frame: &mut Frame<cosmic::Renderer>, ctx: &DrawContext) {
        render_bars(&self.config, BarAnchor::Center, frame, ctx);
    }
}

#[cfg(test)]
mod centered_bars_tests {
    //! The centered bars, drawn. The strip is 160×40 and the bars keep a 1px
    //! margin top and bottom, so they have rows 1 to 38: 32 bars, each 4px
    //! wide with 1px between, starting half a pixel in.
    use super::CenteredBarsVisualization;
    use crate::models::theme::{VisualizationColor, VisualizationSide};
    use crate::ui::components::visualizations::test_support::{
        HEIGHT, Pixels, bands, flat_bands, render_visualization,
    };

    fn draw(data: &super_engine_protocol::FrequencyData) -> Pixels {
        render_visualization(
            &CenteredBarsVisualization::default(),
            data,
            &VisualizationSide::Full,
            VisualizationColor::White,
        )
    }

    /// Silence still shows the bars, at their minimum: a row of 4px dots
    /// across the middle, so the applet visibly listens.
    #[test]
    fn silence_leaves_a_row_of_minimum_bars_across_the_middle() {
        let (left, top, right, bottom) = draw(&flat_bands(32, 0.0)).painted_box().expect("bars");

        assert!(left <= 1 && right >= 158, "bars span {left}..{right}");
        assert!(
            (17..=19).contains(&top) && (20..=22).contains(&bottom),
            "rows {top}..{bottom}"
        );
    }

    #[test]
    fn a_full_bar_fills_the_strip_inside_its_margin() {
        let (_, top, _, bottom) = draw(&flat_bands(32, 3.0)).painted_box().expect("bars");
        assert_eq!((top, bottom), (1, HEIGHT as i32 - 2));
    }

    #[test]
    fn bars_grow_from_the_middle_both_ways() {
        for level in [0.5, 1.0, 2.0] {
            let pixels = draw(&flat_bands(32, level));
            // The middle of bar 10, at x = 50.5..54.5.
            let (top, bottom) = pixels.painted_rows(52).expect("a bar");
            assert!(
                (top + bottom - (HEIGHT as i32 - 1)).abs() <= 1,
                "level {level}: rows {top}..{bottom} aren't centred"
            );
        }
    }

    /// A band lights its own bar and no other.
    #[test]
    fn each_band_drives_its_own_bar() {
        let mut levels = vec![0.0; 32];
        levels[10] = 3.0;
        let pixels = draw(&bands(levels));

        // Above the silent bars' dots, only bar 10 is painted.
        let tall: Vec<i32> = (0..160)
            .filter(|&x| (1..15).any(|y| pixels.is_painted(x, y)))
            .collect();
        assert_eq!(tall.first(), Some(&50), "tall columns {tall:?}");
        assert_eq!(tall.last(), Some(&54), "tall columns {tall:?}");
    }

    /// Malformed frequency data decodes to no bands: nothing to draw, and no
    /// division by zero.
    #[test]
    fn no_bands_draw_no_bars() {
        assert_eq!(draw(&bands(Vec::new())).painted(), 0);
    }
}
