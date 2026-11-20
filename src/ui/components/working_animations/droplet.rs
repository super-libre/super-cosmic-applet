// SPDX-License-Identifier: GPL-3.0-only
use std::f32::consts::PI;

use cosmic::{
    Renderer,
    iced::{
        Point,
        widget::canvas::{Fill, Frame, path},
    },
};

use super::{WorkingAnimationRenderer, WorkingDrawContext, logical_span, smoothstep};

const PERIOD_MS: f32 = 1900.0;
const RAMP_MS: f32 = 260.0;

/// Vertical offset (px, upward positive) of the rippling line at distance `d`
/// (px) from the ripple origin. `half_width` is the distance from the origin
/// to the far edge of the *logical full* animation, so the logical full width
/// is `2 * half_width` (for a `Full` applet that's the applet width; for a side
/// applet it's twice the applet width, with the origin at the inner edge).
/// Pure + deterministic so it can be unit-tested without a renderer.
#[allow(clippy::many_single_char_names)] // math variables: d, h, c, k are standard notation
pub(crate) fn droplet_offset(d: f32, half_width: f32, elapsed_ms: f32, h: f32) -> f32 {
    if half_width <= 0.0 {
        return 0.0;
    }
    let wfull = 2.0 * half_width;
    let c = wfull / 1700.0;
    let k = 3.0 * 4.0 * PI / wfull;
    let omega = c * k;
    let a0 = h * 0.38;
    let spatial = 2.2 / wfull;
    let tdecay = 0.0026;
    let drop = |age: f32| -> f32 {
        let la = age - d / c;
        if la <= 0.0 {
            return 0.0;
        }
        a0 * (-d * spatial).exp()
            * (-la * tdecay).exp()
            * smoothstep(0.0, RAMP_MS, la)
            * (omega * la).sin()
    };
    let age_now = elapsed_ms % PERIOD_MS;
    drop(age_now) + drop(age_now + PERIOD_MS)
}

/// The line's thickness where the drop lands, and at its far ends.
const LINE_WIDTH: (f32, f32) = (2.6, 0.7);

/// The glow's thickness where the drop lands, and at its far ends.
const GLOW_WIDTH: (f32, f32) = (5.0, 1.2);

/// How thick a band of `widths` (at the origin, at the ends) is `d` from the
/// origin, `half_width` being as far as the line runs: full near the origin,
/// thinning mostly toward the ends.
fn taper(widths: (f32, f32), d: f32, half_width: f32) -> f32 {
    let t = (d / half_width).clamp(0.0, 1.0);
    let (full, thin) = widths;
    thin + (full - thin) * (1.0 - t * t)
}

/// The band along `samples`, each a point and its distance from the
/// origin, as thick as [`taper`] says there. A stroke has one width, so the
/// band is a filled outline: one side out along each point's normal, the
/// other side back.
fn ribbon(samples: &[(Point, f32)], widths: (f32, f32), half_width: f32) -> path::Path {
    let last = samples.len().saturating_sub(1);
    let edge = |i: usize, side: f32| {
        let (point, d) = samples[i];
        // The normal to the line here, from its neighbours on either side.
        let (before, after) = (samples[i.saturating_sub(1)].0, samples[(i + 1).min(last)].0);
        let (tx, ty) = (after.x - before.x, after.y - before.y);
        let length = tx.hypot(ty).max(f32::EPSILON);
        let reach = side * taper(widths, d, half_width) / 2.0;
        Point::new(point.x - ty / length * reach, point.y + tx / length * reach)
    };
    path::Path::new(|builder| {
        if samples.is_empty() {
            return;
        }
        builder.move_to(edge(0, 1.0));
        for i in 1..=last {
            builder.line_to(edge(i, 1.0));
        }
        for i in (0..=last).rev() {
            builder.line_to(edge(i, -1.0));
        }
        builder.close();
    })
}

pub struct DropletAnimation;

impl WorkingAnimationRenderer for DropletAnimation {
    #[allow(clippy::many_single_char_names)] // math variables: x, y, w, h are standard notation
    fn draw(&self, frame: &mut Frame<Renderer>, ctx: &WorkingDrawContext) {
        let WorkingDrawContext {
            bounds,
            elapsed_ms,
            gradient,
            side,
        } = *ctx;
        let (w, h) = (bounds.width, bounds.height);
        let mid_y = bounds.y + h / 2.0;

        // Split the ripple by side: the origin sits at the logical centre
        // (`half_width` from each edge of the logical full animation). For a
        // side applet that centre lands on its inner edge, so Left + Right form
        // one ripple seamed at the panel middle.
        let (wlog, x_lo) = logical_span(side, w);
        let half_width = wlog / 2.0;

        // Sample the line every 2px, and at the far edge too, each point with
        // how far it lies from the ripple's origin.
        let mut samples = Vec::new();
        let mut lx = 0.0;
        loop {
            let d = (x_lo + lx - half_width).abs();
            let y = droplet_offset(d, half_width, elapsed_ms, h);
            samples.push((Point::new(bounds.x + lx, mid_y - y), d));
            if lx >= w {
                break;
            }
            lx = (lx + 2.0).min(w);
        }

        // Glow approximation: a wide, faint band beneath the line. Both thin
        // toward the ends, away from the origin; on a side applet only its
        // outer end, as the inner one meets the other half at the origin.
        frame.fill(
            &ribbon(&samples, GLOW_WIDTH, half_width),
            Fill {
                style: gradient.with_alpha(0.25).center_out(bounds),
                ..Default::default()
            },
        );
        frame.fill(
            &ribbon(&samples, LINE_WIDTH, half_width),
            Fill {
                style: gradient.center_out(bounds),
                ..Default::default()
            },
        );

        // Impact flash at the ripple origin (the logical centre) for the first
        // ~180ms of each droplet. On a side applet that origin maps to the inner
        // edge, so the flash appears at the seam.
        let impact = (1.0 - (elapsed_ms % PERIOD_MS) / 180.0).max(0.0);
        if impact > 0.0 {
            let origin_x = bounds.x + (half_width - x_lo);
            let dot = path::Path::circle(Point::new(origin_x, mid_y), 2.4 + 3.0 * impact);
            frame.fill(
                &dot,
                Fill {
                    style: gradient.with_alpha(0.8 * impact).center_out(bounds),
                    ..Default::default()
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(
        clippy::many_single_char_names,
        reason = "geometry: width/height/half-width/distance/offset"
    )]
    fn near_rest_at_start() {
        // Full applet: half_width = w/2, origin (d=0) at the centre.
        let (w, h) = (240.0_f32, 60.0_f32);
        let hw = w / 2.0;
        let mut x = 0.0;
        while x <= w {
            let d = (x - hw).abs();
            let y = droplet_offset(d, hw, 0.0, h);
            assert!(y.abs() <= 0.02 * h, "offset {y} at x={x} exceeds 2% of H");
            x += 2.0;
        }
    }

    #[test]
    #[allow(clippy::cast_precision_loss)] // loop counter
    fn moves_after_a_drop() {
        let (w, h) = (240.0_f32, 60.0_f32);
        let hw = w / 2.0;
        let mut max = 0.0_f32;
        for i in 0..=120 {
            let d = (i as f32 * 2.0 - hw).abs();
            max = max.max(droplet_offset(d, hw, 400.0, h).abs());
        }
        assert!(
            max > 0.05 * h,
            "expected visible motion mid-cycle, got {max}"
        );
    }
}

#[cfg(test)]
mod droplet_drawing_tests {
    //! The droplet, drawn: a line across the middle of the strip that a drop
    //! ripples every 1.9 s, with a flash where it lands. On a side applet
    //! the drop lands on the inner edge, so the two halves ripple as one.
    use crate::models::theme::{VisualizationSide, WorkingAnimationTheme};
    use crate::ui::components::visualizations::test_support::{HEIGHT, Pixels, render_working};
    use cosmic::iced::Color;

    fn draw(elapsed_ms: f32, side: &VisualizationSide) -> Pixels {
        render_working(
            WorkingAnimationTheme::Droplet,
            elapsed_ms,
            side,
            Color::WHITE,
        )
    }

    fn height(pixels: &Pixels) -> i32 {
        let (_, top, _, bottom) = pixels.painted_box().expect("a line");
        bottom - top + 1
    }

    #[test]
    fn between_drops_the_line_lies_flat_across_the_middle() {
        let pixels = draw(1850.0, &VisualizationSide::Full);
        let (left, top, right, bottom) = pixels.painted_box().expect("a line");
        assert!(left <= 2 && right >= 157, "the line spans {left}..{right}");
        assert!(top >= 15 && bottom <= 24, "rows {top}..{bottom}");
        assert!((top + bottom - (HEIGHT as i32 - 1)).abs() <= 1);
    }

    #[test]
    fn a_drop_ripples_the_line() {
        let rest = height(&draw(1850.0, &VisualizationSide::Full));
        let ripple = height(&draw(400.0, &VisualizationSide::Full));
        assert!(ripple >= rest + 6, "{ripple} rows rippling, {rest} at rest");
    }

    /// How far the line has moved off the strip's middle in `columns`, at
    /// most.
    fn displacement(pixels: &Pixels, columns: std::ops::Range<i32>) -> i32 {
        columns
            .filter_map(|x| pixels.painted_rows(x))
            .map(|(top, bottom)| (top + bottom - (HEIGHT as i32 - 1)).abs() / 2)
            .max()
            .unwrap_or(0)
    }

    /// A quarter second in, the ripple has only reached the line near where
    /// the drop landed: the strip's centre on a full applet, the seam on a
    /// side one.
    #[test]
    fn the_ripple_spreads_out_from_where_the_drop_lands() {
        for (side, near, far) in [
            (VisualizationSide::Full, 70..91, 0..30),
            (VisualizationSide::Left, 135..160, 0..60),
            (VisualizationSide::Right, 0..25, 100..160),
        ] {
            let pixels = draw(250.0, &side);
            let (near, far) = (displacement(&pixels, near), displacement(&pixels, far));
            assert!(
                near >= 3 && far <= 1,
                "{side:?}: moved {near}px near the landing, {far}px far from it"
            );
        }
    }

    /// For its first 180ms a drop flashes where it lands.
    #[test]
    fn a_drop_flashes_where_it_lands() {
        let landing = (75..86, 10..30);
        let flash =
            draw(20.0, &VisualizationSide::Full).ink_in(landing.0.clone(), landing.1.clone());
        let none = draw(1850.0, &VisualizationSide::Full).ink_in(landing.0, landing.1);
        assert!(
            flash > none * 3 / 2,
            "{flash} with the flash, {none} without"
        );
    }

    /// The line is full where the drop lands and thins toward its outer
    /// ends: both ends on a full applet, only the outer end on a side one,
    /// whose inner end meets the other half at full thickness.
    #[test]
    fn the_line_thins_toward_its_outer_ends() {
        let strip = 0..HEIGHT as i32;
        for (side, thick, thin) in [
            (VisualizationSide::Full, 76..84, 0..8),
            (VisualizationSide::Full, 76..84, 152..160),
            (VisualizationSide::Left, 152..160, 0..8),
            (VisualizationSide::Right, 0..8, 152..160),
        ] {
            let pixels = draw(1850.0, &side);
            let (thick, thin) = (
                pixels.ink_in(thick, strip.clone()),
                pixels.ink_in(thin, strip.clone()),
            );
            assert!(
                thick > thin * 2,
                "{side:?}: {thick} ink where it lands, {thin} at the outer end"
            );
        }
    }

    #[test]
    fn a_full_applets_ripple_is_symmetric() {
        for elapsed_ms in [20.0, 400.0, 1100.0] {
            let full = draw(elapsed_ms, &VisualizationSide::Full);
            let off = full.difference(&full.mirrored());
            assert!(
                off <= full.painted() / 50,
                "{off} of {} pixels at {elapsed_ms}ms",
                full.painted()
            );
        }
    }

    /// Side by side, inner edge to inner edge, the right applet is the left
    /// one mirrored: one ripple, centred on the seam.
    #[test]
    fn the_side_applets_mirror_each_other_across_the_seam() {
        for elapsed_ms in [20.0, 400.0, 1100.0] {
            let left = draw(elapsed_ms, &VisualizationSide::Left);
            let right = draw(elapsed_ms, &VisualizationSide::Right);
            let off = left.mirrored().difference(&right);
            assert!(
                off <= left.painted() / 50,
                "{off} of {} pixels at {elapsed_ms}ms",
                left.painted()
            );
        }
        // The flash is at the seam: the left applet's right edge.
        let left = draw(20.0, &VisualizationSide::Left);
        let rows = 0..HEIGHT as i32;
        assert!(left.ink_in(150..160, rows.clone()) > left.ink_in(0..10, rows));
    }
}
