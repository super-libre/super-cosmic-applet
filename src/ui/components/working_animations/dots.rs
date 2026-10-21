// SPDX-License-Identifier: GPL-3.0-only
use cosmic::{
    Renderer,
    iced::{
        Color, Point,
        widget::canvas::{Frame, path},
    },
};

use super::{WorkingAnimationRenderer, WorkingDrawContext};

/// Three dots pulsing in sequence — a compact "working" indicator.
///
/// Unlike the wave animations, this renderer **ignores `side`**: it renders the
/// dots centred in full on every applet (exempt from the left/right split), so
/// the loader reads the same on a full applet or on either side applet.
pub struct DotsAnimation;

impl WorkingAnimationRenderer for DotsAnimation {
    #[allow(clippy::cast_precision_loss)] // `i` is a 0..3 dot index
    #[allow(clippy::many_single_char_names)] // math variables: w, h, r are standard notation
    fn draw(&self, frame: &mut Frame<Renderer>, ctx: &WorkingDrawContext) {
        // `side` is intentionally unused — this loader is exempt from the split.
        let WorkingDrawContext {
            bounds,
            elapsed_ms,
            color_config,
            is_dark,
            cosmic_theme,
            ..
        } = *ctx;
        let (w, h) = (bounds.width, bounds.height);
        if w <= 0.0 {
            return;
        }
        let base = color_config.get_color_with_theme(is_dark, cosmic_theme);
        let cy = bounds.y + h / 2.0;
        let gap = (w * 0.085).min(28.0);
        let r = (h * 0.18).min(7.0);
        // Centre the three-dot group on the applet centre (dots at -gap, 0, +gap).
        let cx = bounds.x + w / 2.0 - gap;

        for i in 0..3 {
            // Each dot pulses, offset in phase so they ripple left-to-right.
            let s = 0.45f32.mul_add((elapsed_ms * 0.004 - i as f32 * 0.6).sin(), 0.55);
            let alpha = 0.7f32.mul_add(s, 0.3);
            let radius = r * 0.5f32.mul_add(s, 0.7);
            let x = cx + i as f32 * gap;

            // Soft glow under-dot (iced has no shadow-blur).
            let glow = path::Path::circle(Point::new(x, cy), radius + 2.0);
            frame.fill(
                &glow,
                Color::from_rgba(base.r, base.g, base.b, base.a * alpha * 0.3),
            );
            let dot = path::Path::circle(Point::new(x, cy), radius);
            frame.fill(
                &dot,
                Color::from_rgba(base.r, base.g, base.b, base.a * alpha),
            );
        }
    }
}

#[cfg(test)]
mod dots_tests {
    //! The dots, drawn: three of them, a 13.6px gap apart at this width,
    //! centred on the strip, pulsing in turn from left to right.
    use std::f32::consts::FRAC_PI_2;

    use super::DotsAnimation;
    use crate::models::theme::{VisualizationColor, VisualizationSide, WorkingAnimationTheme};
    use crate::ui::components::visualizations::test_support::{
        HEIGHT, Pixels, WIDTH, colors, render, render_working, theme,
    };
    use crate::ui::components::working_animations::{WorkingAnimationRenderer, WorkingDrawContext};

    fn draw(elapsed_ms: f32, side: &VisualizationSide) -> Pixels {
        render_working(
            WorkingAnimationTheme::Dots,
            elapsed_ms,
            side,
            VisualizationColor::White,
        )
    }

    /// Unlike the wave animations, the dots don't split across the two side
    /// applets: each draws the whole loader.
    #[test]
    fn every_side_draws_the_same_dots() {
        let full = draw(500.0, &VisualizationSide::Full);
        for side in [VisualizationSide::Left, VisualizationSide::Right] {
            assert_eq!(full.difference(&draw(500.0, &side)), 0, "{side:?}");
        }
    }

    #[test]
    fn the_dots_are_centred_on_the_strip() {
        let (left, top, right, bottom) = draw(500.0, &VisualizationSide::Full)
            .painted_box()
            .expect("dots");
        assert!(
            (left + right - (WIDTH as i32 - 1)).abs() <= 2,
            "{left}..{right}"
        );
        assert!(
            (top + bottom - (HEIGHT as i32 - 1)).abs() <= 2,
            "{top}..{bottom}"
        );
    }

    /// Each dot peaks a little after the one to its left.
    #[test]
    fn the_dots_pulse_in_turn_from_left_to_right() {
        let centre = |pixels: &Pixels, dot: i32| pixels.alpha(80 + (dot - 1) * 14, 20);
        // The first dot's peak, then the last's.
        let first = draw(FRAC_PI_2 / 0.004, &VisualizationSide::Full);
        let last = draw((FRAC_PI_2 + 1.2) / 0.004, &VisualizationSide::Full);

        assert!(centre(&first, 0) > centre(&first, 1) && centre(&first, 1) > centre(&first, 2));
        assert!(centre(&last, 2) > centre(&last, 1) && centre(&last, 1) > centre(&last, 0));
    }

    #[test]
    fn a_strip_with_no_width_draws_nothing() {
        let theme = theme(true);
        let colors = colors(VisualizationColor::White);
        let pixels = render(WIDTH, HEIGHT, |frame| {
            DotsAnimation.draw(
                frame,
                &WorkingDrawContext {
                    bounds: cosmic::iced::core::Rectangle::new(
                        cosmic::iced::Point::ORIGIN,
                        cosmic::iced::Size::new(0.0, 40.0),
                    ),
                    elapsed_ms: 500.0,
                    color_config: &colors,
                    is_dark: true,
                    cosmic_theme: theme.cosmic(),
                    side: &VisualizationSide::Full,
                },
            );
        });
        assert_eq!(pixels.painted(), 0);
    }
}
