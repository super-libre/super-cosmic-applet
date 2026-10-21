// SPDX-License-Identifier: GPL-3.0-only
use std::f32::consts::PI;

use cosmic::{
    Renderer,
    iced::{
        Color, Point,
        widget::canvas::{Frame, path, stroke},
    },
};

use super::{WorkingAnimationRenderer, WorkingDrawContext, logical_span};

pub struct CometAnimation;

impl WorkingAnimationRenderer for CometAnimation {
    #[allow(clippy::cast_precision_loss)] // trail index is a tiny loop counter
    #[allow(clippy::many_single_char_names)] // math variables: w, h, k, a are standard notation
    fn draw(&self, frame: &mut Frame<Renderer>, ctx: &WorkingDrawContext) {
        let WorkingDrawContext {
            bounds,
            elapsed_ms,
            color_config,
            is_dark,
            cosmic_theme,
            side,
        } = *ctx;
        let (w, h) = (bounds.width, bounds.height);
        if w <= 0.0 {
            return;
        }
        let mid_y = bounds.y + h / 2.0;
        let amp = h * 0.30;
        let base = color_config.get_color_with_theme(is_dark, cosmic_theme);

        // Split by side: the comet path/head live in a logical full-width space;
        // this applet renders the `[x_lo, x_lo + w)` slice of it, so the head
        // travels continuously from the Left applet into the Right one.
        let (wlog, x_lo) = logical_span(side, w);
        let k = 2.0 * PI * 1.8 / wlog;
        // `lx` is a logical x-coordinate; `lx - x_lo` maps it to this canvas.
        let path_y = |lx: f32| mid_y + amp * (lx * k).sin();

        // Faint guide line over this applet's slice.
        let mut gb = path::Builder::new();
        let mut local = 0.0;
        let mut first = true;
        while local <= w {
            let p = Point::new(bounds.x + local, path_y(x_lo + local));
            if first {
                gb.move_to(p);
                first = false;
            } else {
                gb.line_to(p);
            }
            local += 3.0;
        }
        frame.stroke(
            &gb.build(),
            stroke::Stroke {
                style: stroke::Style::Solid(Color::from_rgba(
                    base.r,
                    base.g,
                    base.b,
                    base.a * 0.18,
                )),
                width: 1.4,
                ..Default::default()
            },
        );

        // Comet head + fading trail, positioned in logical space and clipped to
        // this applet's slice.
        let trail: i32 = 44;
        let head = (elapsed_ms * 0.11) % (wlog + 40.0) - 20.0;
        for i in (0..=trail).rev() {
            let lx = head - (i as f32) * 3.0;
            if lx < x_lo || lx > x_lo + w {
                continue;
            }
            let sx = bounds.x + (lx - x_lo);
            let yy = path_y(lx);
            let a = 1.0 - (i as f32) / (trail as f32);
            if i == 0 {
                let glow = path::Path::circle(Point::new(sx, yy), 7.0);
                frame.fill(
                    &glow,
                    Color::from_rgba(base.r, base.g, base.b, base.a * 0.3),
                );
            }
            let r = if i == 0 { 4.0 } else { 2.0 * a + 0.6 };
            let dot = path::Path::circle(Point::new(sx, yy), r);
            frame.fill(
                &dot,
                Color::from_rgba(base.r, base.g, base.b, base.a * a * a * 0.9),
            );
        }
    }
}

#[cfg(test)]
mod comet_tests {
    //! The comet, drawn: a bright head with a fading trail, travelling along
    //! a faint sine guide at 0.11 px/ms. The head enters 20px before the
    //! animation's start and leaves 20px past its end.
    use super::CometAnimation;
    use crate::models::theme::{VisualizationColor, VisualizationSide, WorkingAnimationTheme};
    use crate::ui::components::visualizations::test_support::{
        HEIGHT, Pixels, WIDTH, colors, render, render_working, theme,
    };
    use crate::ui::components::working_animations::{WorkingAnimationRenderer, WorkingDrawContext};

    fn draw(elapsed_ms: f32, side: &VisualizationSide) -> Pixels {
        render_working(
            WorkingAnimationTheme::Comet,
            elapsed_ms,
            side,
            VisualizationColor::White,
        )
    }

    /// The elapsed time that puts the head at logical x `head`, on a side
    /// whose animation is `logical_width` wide.
    fn when_head_at(head: f32, logical_width: f32) -> f32 {
        ((head + 20.0) % (logical_width + 40.0)) / 0.11
    }

    #[test]
    fn the_head_travels_left_to_right() {
        for head in [35.0, 90.0, 140.0] {
            let pixels = draw(when_head_at(head, 160.0), &VisualizationSide::Full);
            let found = pixels.heaviest_column(0..WIDTH as i32);
            assert!(
                (found - head as i32).abs() <= 4,
                "the head should be at {head}, the most ink is at {found}"
            );
        }
    }

    /// The side applets share one animation twice their width: the head
    /// crosses the left applet, then carries on into the right one.
    #[test]
    fn the_head_crosses_from_the_left_applet_into_the_right() {
        let in_left = when_head_at(120.0, 320.0);
        let in_right = when_head_at(220.0, 320.0);

        let left = draw(in_left, &VisualizationSide::Left);
        let right = draw(in_left, &VisualizationSide::Right);
        assert!((left.heaviest_column(0..160) - 120).abs() <= 4);
        let guide_only = right.ink();

        let right = draw(in_right, &VisualizationSide::Right);
        assert!((right.heaviest_column(0..160) - 60).abs() <= 4);
        assert!(
            right.ink() > guide_only * 2,
            "no head reached the right applet"
        );
        // Its trail still runs back across the left applet.
        let left = draw(in_right, &VisualizationSide::Left);
        let rows = 0..HEIGHT as i32;
        assert!(left.ink_in(100..160, rows.clone()) > left.ink_in(0..40, rows));
    }

    /// Ahead of the head there is only the guide: faint, but there.
    #[test]
    fn ahead_of_the_head_only_the_faint_guide_shows() {
        let pixels = draw(when_head_at(35.0, 160.0), &VisualizationSide::Full);
        let ahead = (100..160).flat_map(|x| (0..HEIGHT as i32).map(move |y| (x, y)));
        let brightest = ahead.map(|(x, y)| pixels.alpha(x, y)).max().unwrap_or(0);
        assert!(
            (24..=70).contains(&brightest),
            "the guide's alpha is {brightest}"
        );
    }

    #[test]
    fn a_strip_with_no_width_draws_nothing() {
        let theme = theme(true);
        let colors = colors(VisualizationColor::White);
        let pixels = render(WIDTH, HEIGHT, |frame| {
            CometAnimation.draw(
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
