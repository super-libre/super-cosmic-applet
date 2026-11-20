// SPDX-License-Identifier: GPL-3.0-only
pub mod centered_bars;
pub mod equalizer;
pub mod pulse;
pub mod waveform;

pub use centered_bars::CenteredBarsVisualization;
pub use equalizer::EqualizerVisualization;
pub use pulse::PulseVisualization;
pub use waveform::WaveformVisualization;

use cosmic::{
    Renderer,
    iced::{
        Padding, Point, Size, border,
        core::Rectangle,
        widget::canvas::{Fill, Frame, path},
    },
};

use crate::config::FREQUENCY_NORMALIZATION_MAX;
use crate::models::palette::Gradient;
use crate::models::theme::VisualizationSide;
use crate::util::{f32_to_usize, usize_to_f32};
use super_engine_protocol::FrequencyData;

/// Maximum number of frequency bands any bar/waveform visualization renders.
/// Frequency data may carry more; the extras are ignored so bar sizing stays
/// stable across inputs. Previously a bare `32` copy-pasted into every renderer.
pub const MAX_VISUALIZATION_BANDS: usize = 32;

/// Vertical anchoring for a bar within the drawable bounds. The only thing that
/// distinguishes the bottom equalizer from the centered equalizer.
#[derive(Clone, Copy)]
pub enum BarAnchor {
    /// Bars grow upward from the bottom edge (bottom equalizer).
    Bottom,
    /// Bars are centered vertically (centered equalizer).
    Center,
}

/// Split the frequency spectrum for the applet's rendered `side`, returning
/// `(count, start_index)`: how many bands this instance draws and where its
/// slice begins. `Left`/`Right` each render half the (capped) spectrum; `Full`
/// renders all of it. Shared by every bar/waveform renderer so the
/// [`MAX_VISUALIZATION_BANDS`] cap and the half-split live in one place.
pub fn visible_band_range(side: &VisualizationSide, total_bands: usize) -> (usize, usize) {
    let total = total_bands.min(MAX_VISUALIZATION_BANDS);
    match side {
        VisualizationSide::Left => (total / 2, 0),
        VisualizationSide::Right => (total / 2, total / 2),
        VisualizationSide::Full => (total, 0),
    }
}

/// How far apart bars sit along the applet, in pixels: 48 bars across a
/// 300px applet, and fewer on a shorter one.
pub const BAR_PITCH: f32 = 300.0 / 48.0;

/// How many bars fit along `length` pixels at [`BAR_PITCH`]: at least one.
pub fn bar_count(length: f32) -> usize {
    f32_to_usize((length / BAR_PITCH).round()).max(1)
}

/// The level of bar `bar` of `bars` spread evenly over `bands`: the two bands
/// its middle falls between, blended by how close it sits to each. With more
/// bars than bands, neighbors step smoothly between band levels; with fewer,
/// each bar reads the spectrum where it stands.
pub fn bar_level(bands: &[f32], bar: usize, bars: usize) -> f32 {
    let Some(last) = bands.len().checked_sub(1) else {
        return 0.0;
    };
    let at = ((usize_to_f32(bar) + 0.5) * usize_to_f32(bands.len()) / usize_to_f32(bars) - 0.5)
        .clamp(0.0, usize_to_f32(last));
    let below = f32_to_usize(at);
    let above = (below + 1).min(last);
    let t = at - usize_to_f32(below);
    bands[below] * (1.0 - t) + bands[above] * t
}

/// Render the vertical-bar family of visualizations (bottom equalizer and
/// centered equalizer). The two differ only in their per-style
/// [`VisualizationConfig`] and vertical [`BarAnchor`]; all band selection, bar
/// sizing, spacing, centering, and drawing is identical and lives here.
pub fn render_bars(
    config: &VisualizationConfig,
    anchor: BarAnchor,
    frame: &mut Frame<Renderer>,
    ctx: &DrawContext,
) {
    let DrawContext {
        bounds,
        frequency_data,
        side,
        gradient,
    } = *ctx;
    let effective_bounds = config.effective_bounds(bounds);
    let (band_count, band_start) = visible_band_range(side, frequency_data.bands.len());

    // Nothing to draw (e.g. malformed/empty frequency data decoded to zero
    // bands). Bail before sampling an empty slice (Tier 1 #21, same class as
    // #268).
    if band_count == 0 {
        return;
    }
    let bands = &frequency_data.bands[band_start..band_start + band_count];

    // As many bars as the applet's length holds, whatever the band count, so
    // a longer applet draws more bars rather than wider ones.
    let bars_to_show = bar_count(effective_bounds.width);
    let bars_f32 = usize_to_f32(bars_to_show);
    let bar_width = effective_bounds.width / bars_f32 * 0.8;
    let spacing = effective_bounds.width / bars_f32 * 0.2;

    // Center the bars in the available width.
    let total_bars_width =
        (bar_width * bars_f32) + (spacing * usize_to_f32(bars_to_show.saturating_sub(1)));
    let start_x = effective_bounds.x + (effective_bounds.width - total_bars_width) / 2.0;

    let normalization_factor = 1.0 / FREQUENCY_NORMALIZATION_MAX;

    // Loop-invariant: neither the fill nor the height ceiling depends on the
    // per-bar amplitude, so resolve them once instead of per bar. One gradient
    // spans the whole strip the way the bars grow, deep where they start, so
    // a bar takes the colors of the heights it reaches.
    let fill = match anchor {
        BarAnchor::Bottom => gradient.bottom_to_top(bounds),
        BarAnchor::Center => gradient.center_out(bounds),
    };
    let max_height = config.max_element_height(effective_bounds.height);

    for display_bar in 0..bars_to_show {
        let x = start_x + (usize_to_f32(display_bar) * (bar_width + spacing));
        let average_amplitude = bar_level(bands, display_bar, bars_to_show);

        let height_factor = average_amplitude * normalization_factor;
        let capped_height_factor = height_factor.min(1.0);
        let bar_height = max_height * capped_height_factor;
        let clamped_height = config.clamped_element_height(bar_height, effective_bounds.height);

        let y = match anchor {
            BarAnchor::Bottom => effective_bounds.y + effective_bounds.height - clamped_height,
            BarAnchor::Center => {
                effective_bounds.y + (effective_bounds.height - clamped_height) / 2.0
            }
        };

        let mut path_builder = path::Builder::new();
        path_builder.rounded_rectangle(
            Point { x, y },
            Size::new(bar_width, clamped_height),
            config.corner_radius,
        );
        let path = path_builder.build();
        frame.fill(
            &path,
            Fill {
                style: fill,
                ..Default::default()
            },
        );
    }
}

/// Shared configuration for visualization rendering with proper margin and height management
#[derive(Debug, Clone)]
pub struct VisualizationConfig {
    /// Horizontal and vertical margins using Iced's Padding type
    pub margins: Padding,

    /// Corner radius for rounded rectangles
    pub corner_radius: border::Radius,

    /// Minimum height for bars/elements
    pub min_element_height: f32,

    /// Margin from canvas edges specifically for maximum height calculation
    pub height_safety_margin: f32,
}

impl VisualizationConfig {
    /// Calculate effective drawing bounds given canvas bounds
    pub fn effective_bounds(&self, canvas_bounds: Rectangle) -> Rectangle {
        Rectangle {
            x: canvas_bounds.x + self.margins.left,
            y: canvas_bounds.y + self.margins.top,
            width: canvas_bounds.width - (self.margins.left + self.margins.right),
            height: canvas_bounds.height - (self.margins.top + self.margins.bottom),
        }
    }

    /// Calculate maximum safe element height within canvas bounds
    pub fn max_element_height(&self, canvas_height: f32) -> f32 {
        let available_height = canvas_height - self.height_safety_margin;
        available_height.max(1.0) // Minimum 1px height
    }

    /// Calculate minimum element height based on canvas size
    pub fn min_element_height(&self, canvas_height: f32) -> f32 {
        let canvas_min = canvas_height / 4.0;
        canvas_min.min(self.min_element_height).max(1.0)
    }

    /// Get clamped element height between min and max bounds
    pub fn clamped_element_height(&self, desired_height: f32, canvas_height: f32) -> f32 {
        let min_height = self.min_element_height(canvas_height);
        let max_height = self.max_element_height(canvas_height);
        desired_height.max(min_height).min(max_height)
    }
}

/// Per-frame inputs shared by every [`VisualizationRenderer`]. Bundled
/// into one context so renderers take a single argument instead of a
/// long positional parameter list. All fields are `Copy`, so a renderer
/// can destructure `*ctx` directly.
#[derive(Clone, Copy)]
pub struct DrawContext<'a> {
    /// Canvas bounds to draw within.
    pub bounds: Rectangle,
    /// Frequency analysis data for the current frame.
    pub frequency_data: &'a FrequencyData,
    /// Which portion of the spectrum this applet instance renders.
    pub side: &'a VisualizationSide,
    /// The gradient to draw in, resolved for the theme being drawn on.
    pub gradient: &'a Gradient,
}

/// Common trait for all visualization renderers.
pub trait VisualizationRenderer {
    /// Draw the visualization using the per-frame [`DrawContext`].
    fn draw(&self, frame: &mut Frame<Renderer>, ctx: &DrawContext);
}

#[cfg(test)]
pub(crate) mod test_support {
    //! Drawing for real in a test. The applet renders with tiny-skia, which
    //! needs no window or GPU, so a test can rasterize what a visualization or
    //! working animation paints and assert on the pixels: where paint landed,
    //! how much of it, and in what color.
    //!
    //! A renderer draws into a strip the size of the applet's, inside a frame
    //! with a margin around it. The applet's own frame is the strip, and would
    //! clip anything drawn past it; the margin is there to show it.
    use cosmic::iced::{
        Color, Pixels as TextSize, Size,
        advanced::graphics::geometry::Renderer as _,
        core::{Rectangle, renderer::Headless as _},
        widget::canvas::{Frame, Geometry},
    };

    use super::{DrawContext, VisualizationRenderer};
    use crate::models::palette::{Gradient, ProductColors};
    use crate::models::theme::{VisualizationSide, WorkingAnimationTheme};
    use crate::ui::components::working_animations::{self, WorkingDrawContext};
    use crate::util::{u32_to_f32, usize_to_f32};
    use super_engine_protocol::FrequencyData;

    /// The strip a test draws into: wide and thin, like the applet's.
    pub(crate) const WIDTH: u32 = 160;
    pub(crate) const HEIGHT: u32 = 40;

    /// The margin around the strip, in the frame.
    pub(crate) const MARGIN: u32 = 8;

    /// Alpha below which a pixel counts as unpainted: antialiasing smears a
    /// shape's edge a pixel or so past it, faintly.
    const PAINTED: u8 = 24;

    /// A rasterized frame, RGBA, row by row. Coordinates are the strip's: the
    /// strip's top-left pixel is `(0, 0)`, and the margin lies at negative
    /// coordinates and past the strip's size.
    pub(crate) struct Pixels {
        width: u32,
        height: u32,
        margin: u32,
        rgba: Vec<u8>,
    }

    impl Pixels {
        fn index(&self, x: i32, y: i32) -> Option<usize> {
            let (fx, fy) = (x + self.margin as i32, y + self.margin as i32);
            let inside =
                (0..self.width as i32).contains(&fx) && (0..self.height as i32).contains(&fy);
            inside.then(|| ((fy as u32 * self.width + fx as u32) * 4) as usize)
        }

        /// Every pixel of the frame, margin included, in strip coordinates.
        fn all(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
            let m = self.margin as i32;
            let (w, h) = (self.width as i32, self.height as i32);
            (-m..h - m).flat_map(move |y| (-m..w - m).map(move |x| (x, y)))
        }

        pub(crate) fn alpha(&self, x: i32, y: i32) -> u8 {
            self.index(x, y).map_or(0, |i| self.rgba[i + 3])
        }

        pub(crate) fn is_painted(&self, x: i32, y: i32) -> bool {
            self.alpha(x, y) >= PAINTED
        }

        /// How many pixels are painted, in all.
        pub(crate) fn painted(&self) -> usize {
            self.all().filter(|&(x, y)| self.is_painted(x, y)).count()
        }

        /// How many pixels are painted outside the `width`×`height` strip, by
        /// more than `along` pixels past its ends or `across` pixels past its
        /// top and bottom.
        pub(crate) fn painted_outside(
            &self,
            width: u32,
            height: u32,
            along: i32,
            across: i32,
        ) -> usize {
            let (w, h) = (width as i32, height as i32);
            self.all()
                .filter(|&(x, y)| x < -along || x >= w + along || y < -across || y >= h + across)
                .filter(|&(x, y)| self.is_painted(x, y))
                .count()
        }

        /// The total alpha painted, so a fainter shape counts for less.
        pub(crate) fn ink(&self) -> u64 {
            self.all().map(|(x, y)| u64::from(self.alpha(x, y))).sum()
        }

        /// The total alpha painted within `columns` and `rows`.
        pub(crate) fn ink_in(
            &self,
            columns: std::ops::Range<i32>,
            rows: std::ops::Range<i32>,
        ) -> u64 {
            columns
                .flat_map(|x| rows.clone().map(move |y| (x, y)))
                .map(|(x, y)| u64::from(self.alpha(x, y)))
                .sum()
        }

        /// The column, among `columns`, with the most ink.
        pub(crate) fn heaviest_column(&self, columns: std::ops::Range<i32>) -> i32 {
            let m = self.margin as i32;
            let rows = -m..self.height as i32 - m;
            columns
                .max_by_key(|&x| self.ink_in(x..x + 1, rows.clone()))
                .expect("no columns")
        }

        /// The painted rows of column `x`, as `(top, bottom)`, inclusive.
        pub(crate) fn painted_rows(&self, x: i32) -> Option<(i32, i32)> {
            let m = self.margin as i32;
            let mut rows = (-m..self.height as i32 - m).filter(|&y| self.is_painted(x, y));
            let top = rows.next()?;
            Some((top, rows.last().unwrap_or(top)))
        }

        /// The smallest box holding every painted pixel, as
        /// `(left, top, right, bottom)`, inclusive. `None` for a blank frame.
        pub(crate) fn painted_box(&self) -> Option<(i32, i32, i32, i32)> {
            self.all()
                .filter(|&(x, y)| self.is_painted(x, y))
                .fold(None, |found, (x, y)| {
                    Some(match found {
                        None => (x, y, x, y),
                        Some((l, t, r, b)) => (l.min(x), t.min(y), r.max(x), b.max(y)),
                    })
                })
        }

        /// The average color of the painted pixels, as `[r, g, b]` in
        /// `0.0..=1.0`. The renderer stores premultiplied color, so dividing
        /// by the alpha recovers the paint's own, however faint.
        pub(crate) fn paint_color(&self) -> [f32; 3] {
            let (mut sum, mut weight) = ([0.0_f32; 3], 0.0_f32);
            for i in self.all().filter_map(|(x, y)| self.index(x, y)) {
                let pixel = &self.rgba[i..i + 4];
                if pixel[3] < PAINTED {
                    continue;
                }
                for (channel, value) in sum.iter_mut().zip(&pixel[..3]) {
                    *channel += f32::from(*value) / 255.0;
                }
                weight += f32::from(pixel[3]) / 255.0;
            }
            assert!(weight > 0.0, "nothing painted to take a color from");
            sum.map(|channel| channel / weight)
        }

        /// The color painted at `(x, y)`, as [`Self::paint_color`] gives it.
        pub(crate) fn color_at(&self, x: i32, y: i32) -> [f32; 3] {
            let i = self.index(x, y).expect("a pixel in the frame");
            let pixel = &self.rgba[i..i + 4];
            assert!(pixel[3] >= PAINTED, "nothing painted at ({x}, {y})");
            let alpha = f32::from(pixel[3]) / 255.0;
            [0, 1, 2].map(|c| f32::from(pixel[c]) / 255.0 / alpha)
        }

        /// The frame flipped left to right, to compare a side with its mirror.
        pub(crate) fn mirrored(&self) -> Self {
            let mut rgba = Vec::with_capacity(self.rgba.len());
            for row in self.rgba.chunks(self.width as usize * 4) {
                for pixel in row.chunks(4).rev() {
                    rgba.extend_from_slice(pixel);
                }
            }
            Self { rgba, ..*self }
        }

        /// How many pixels are painted in one frame and not the other.
        pub(crate) fn difference(&self, other: &Self) -> usize {
            assert_eq!(
                (self.width, self.height, self.margin),
                (other.width, other.height, other.margin)
            );
            self.all()
                .filter(|&(x, y)| self.is_painted(x, y) != other.is_painted(x, y))
                .count()
        }
    }

    fn renderer() -> cosmic::Renderer {
        cosmic::Renderer::new(cosmic::font::default(), TextSize(14.0))
    }

    fn screenshot(mut renderer: cosmic::Renderer, width: u32, height: u32, margin: u32) -> Pixels {
        let rgba = renderer.screenshot(Size::new(width, height), 1.0, Color::TRANSPARENT);
        Pixels {
            width,
            height,
            margin,
            rgba,
        }
    }

    /// Rasterize what `draw` paints into a `width`×`height` frame with no
    /// margin, over a transparent background.
    pub(crate) fn render(
        width: u32,
        height: u32,
        draw: impl FnOnce(&mut Frame<cosmic::Renderer>),
    ) -> Pixels {
        let mut renderer = renderer();
        let mut frame = Frame::new(&renderer, Size::new(u32_to_f32(width), u32_to_f32(height)));
        draw(&mut frame);
        renderer.draw_geometry(frame.into_geometry());
        screenshot(renderer, width, height, 0)
    }

    /// Rasterize the geometry a canvas program draws, given the renderer to
    /// draw it with, in a `width`×`height` frame with no margin.
    pub(crate) fn render_program(
        width: u32,
        height: u32,
        draw: impl FnOnce(&cosmic::Renderer) -> Vec<Geometry<cosmic::Renderer>>,
    ) -> Pixels {
        let mut renderer = renderer();
        for geometry in draw(&renderer) {
            renderer.draw_geometry(geometry);
        }
        screenshot(renderer, width, height, 0)
    }

    /// Rasterize what `draw` paints into the strip, given the strip's bounds
    /// in the frame, which has a [`MARGIN`] around it.
    pub(crate) fn render_strip(
        draw: impl FnOnce(&mut Frame<cosmic::Renderer>, Rectangle),
    ) -> Pixels {
        let (width, height) = (WIDTH + 2 * MARGIN, HEIGHT + 2 * MARGIN);
        let mut renderer = renderer();
        let mut frame = Frame::new(&renderer, Size::new(u32_to_f32(width), u32_to_f32(height)));
        let strip = Rectangle {
            x: u32_to_f32(MARGIN),
            y: u32_to_f32(MARGIN),
            width: u32_to_f32(WIDTH),
            height: u32_to_f32(HEIGHT),
        };
        draw(&mut frame, strip);
        renderer.draw_geometry(frame.into_geometry());
        screenshot(renderer, width, height, MARGIN)
    }

    pub(crate) fn theme(dark: bool) -> cosmic::Theme {
        if dark {
            cosmic::Theme::dark()
        } else {
            cosmic::Theme::light()
        }
    }

    /// Colors a test can check for: `color` all the way up the gradient, in
    /// either theme.
    pub(crate) fn colors(color: Color) -> ProductColors {
        ProductColors::Fixed(Gradient::solid(color))
    }

    /// Distinct colors for tests to tell apart.
    pub(crate) const RED: Color = Color::from_rgb(1.0, 0.3, 0.45);
    pub(crate) const GREEN: Color = Color::from_rgb(0.3, 0.8, 0.5);

    /// `[r, g, b]` of `color`, as [`Pixels::paint_color`] gives it.
    pub(crate) fn rgb(color: Color) -> [f32; 3] {
        [color.r, color.g, color.b]
    }

    /// Whether two colors are the same, to within what rasterizing loses.
    pub(crate) fn same_color(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(&b).all(|(a, b)| (a - b).abs() < 0.06)
    }

    /// `count` bands, each at `level` (`FREQUENCY_NORMALIZATION_MAX` fills a
    /// bar).
    pub(crate) fn flat_bands(count: usize, level: f32) -> FrequencyData {
        bands(vec![level; count])
    }

    pub(crate) fn bands(bands: Vec<f32>) -> FrequencyData {
        let total_energy = bands.iter().sum::<f32>() / usize_to_f32(bands.len().max(1));
        FrequencyData {
            bands,
            total_energy,
            ..FrequencyData::default()
        }
    }

    /// One visualization's frame for `data` on `side`, in the dark theme, a
    /// solid `color`.
    pub(crate) fn render_visualization(
        visualization: &dyn VisualizationRenderer,
        data: &FrequencyData,
        side: &VisualizationSide,
        color: Color,
    ) -> Pixels {
        render_gradient(visualization, data, side, &Gradient::solid(color))
    }

    /// One visualization's frame for `data` on `side`, in `gradient`.
    pub(crate) fn render_gradient(
        visualization: &dyn VisualizationRenderer,
        data: &FrequencyData,
        side: &VisualizationSide,
        gradient: &Gradient,
    ) -> Pixels {
        render_strip(|frame, strip| {
            visualization.draw(
                frame,
                &DrawContext {
                    bounds: strip,
                    frequency_data: data,
                    side,
                    gradient,
                },
            );
        })
    }

    /// One working animation's frame, `elapsed_ms` in, on `side`, in the
    /// dark theme, a solid `color`.
    pub(crate) fn render_working(
        animation: WorkingAnimationTheme,
        elapsed_ms: f32,
        side: &VisualizationSide,
        color: Color,
    ) -> Pixels {
        let gradient = Gradient::solid(color);
        render_strip(|frame, strip| {
            working_animations::draw(
                animation,
                frame,
                &WorkingDrawContext {
                    bounds: strip,
                    elapsed_ms,
                    gradient: &gradient,
                    side,
                },
            );
        })
    }

    #[test]
    fn a_frame_rasterizes_where_and_in_the_color_it_was_painted() {
        // The harness itself: a red square comes back red, where it was
        // painted, and nowhere else.
        let pixels = render(WIDTH, HEIGHT, |frame| {
            frame.fill_rectangle(
                cosmic::iced::Point::new(2.0, 3.0),
                Size::new(10.0, 5.0),
                Color::from_rgb(1.0, 0.0, 0.0),
            );
        });
        assert_eq!(pixels.painted_box(), Some((2, 3, 11, 7)));
        assert_eq!(pixels.painted(), 50);
        assert!(same_color(pixels.paint_color(), [1.0, 0.0, 0.0]));
    }

    #[test]
    fn the_strip_sits_inside_its_margin() {
        let pixels = render_strip(|frame, strip| {
            frame.fill_rectangle(strip.position(), strip.size(), Color::WHITE);
        });
        assert_eq!(
            pixels.painted_box(),
            Some((0, 0, WIDTH as i32 - 1, HEIGHT as i32 - 1))
        );
        assert_eq!(pixels.painted_outside(WIDTH, HEIGHT, 0, 0), 0);
    }
}

#[cfg(test)]
mod bar_tests {
    //! How many bars an applet draws, and what each reads from the bands.
    use super::{bar_count, bar_level};

    #[test]
    fn a_300px_applet_draws_48_bars_and_a_shorter_one_fewer() {
        assert_eq!(bar_count(300.0), 48);
        assert_eq!(bar_count(150.0), 24);
        assert_eq!(bar_count(120.0), 19);
        assert_eq!(bar_count(60.0), 10);
        assert_eq!(bar_count(0.0), 1, "always at least one");
    }

    #[test]
    fn as_many_bars_as_bands_read_each_band() {
        let bands = [0.0, 1.0, 2.0, 3.0];
        for (i, band) in bands.iter().enumerate() {
            assert!((bar_level(&bands, i, bands.len()) - band).abs() < 1e-6);
        }
    }

    #[test]
    fn more_bars_than_bands_step_smoothly_between_them() {
        let bands = [0.0, 2.0];
        let levels: Vec<f32> = (0..4).map(|i| bar_level(&bands, i, 4)).collect();
        assert_eq!(levels.first(), Some(&0.0));
        assert_eq!(levels.last(), Some(&2.0));
        assert!(levels.windows(2).all(|w| w[0] <= w[1]), "{levels:?}");
        assert!(levels[1] > 0.0 && levels[2] < 2.0, "{levels:?}");
    }

    #[test]
    fn no_bands_read_as_silence() {
        assert_eq!(bar_level(&[], 0, 10), 0.0);
    }
}

#[cfg(test)]
mod visible_band_range_tests {
    use super::{MAX_VISUALIZATION_BANDS, VisualizationSide, visible_band_range};

    #[test]
    fn full_takes_all_capped_bands_from_the_start() {
        assert_eq!(visible_band_range(&VisualizationSide::Full, 32), (32, 0));
        assert_eq!(visible_band_range(&VisualizationSide::Full, 8), (8, 0));
    }

    #[test]
    fn left_and_right_split_the_spectrum_in_half() {
        assert_eq!(visible_band_range(&VisualizationSide::Left, 32), (16, 0));
        assert_eq!(visible_band_range(&VisualizationSide::Right, 32), (16, 16));
    }

    #[test]
    fn caps_at_max_visualization_bands() {
        // Frequency data can carry more than we render (e.g. 64 bands); the
        // extras are ignored so bar sizing stays stable.
        assert_eq!(
            visible_band_range(&VisualizationSide::Full, 64),
            (MAX_VISUALIZATION_BANDS, 0)
        );
        assert_eq!(
            visible_band_range(&VisualizationSide::Right, 64),
            (MAX_VISUALIZATION_BANDS / 2, MAX_VISUALIZATION_BANDS / 2)
        );
    }

    #[test]
    fn zero_bands_yields_zero_count() {
        // Renderers rely on a zero count to bail before dividing by zero.
        assert_eq!(visible_band_range(&VisualizationSide::Full, 0), (0, 0));
        assert_eq!(visible_band_range(&VisualizationSide::Left, 0), (0, 0));
    }
}

#[cfg(test)]
mod visualization_config_tests {
    //! The height math every bar renderer shares. Its whole job is keeping an
    //! element inside the panel: the applet draws into a fixed-size strip of
    //! someone else's panel, so a bar that overshoots is drawn over its
    //! neighbours rather than clipped.
    use super::VisualizationConfig;
    use cosmic::iced::{Padding, border::Radius, core::Rectangle};

    fn config(min_element_height: f32, height_safety_margin: f32) -> VisualizationConfig {
        VisualizationConfig {
            margins: Padding {
                top: 2.0,
                right: 4.0,
                bottom: 6.0,
                left: 8.0,
            },
            corner_radius: Radius::new(2.0),
            min_element_height,
            height_safety_margin,
        }
    }

    #[test]
    fn effective_bounds_inset_every_margin() {
        let bounds = config(4.0, 0.0).effective_bounds(Rectangle {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 50.0,
        });

        assert_eq!((bounds.x, bounds.y), (18.0, 22.0));
        assert_eq!(bounds.width, 100.0 - (8.0 + 4.0));
        assert_eq!(bounds.height, 50.0 - (2.0 + 6.0));
    }

    #[test]
    fn heights_never_collapse_to_zero() {
        // A panel shorter than the safety margin still has to draw something:
        // a zero height is an invisible bar, not a small one.
        let config = config(4.0, 40.0);

        assert_eq!(config.max_element_height(10.0), 1.0);
        assert_eq!(config.min_element_height(0.0), 1.0);
    }

    #[test]
    fn the_minimum_element_height_is_capped_by_the_canvas() {
        // On a short panel a quarter of the canvas wins over the configured
        // minimum, so a quiet band can never fill the whole applet.
        let config = config(20.0, 0.0);

        assert_eq!(config.min_element_height(40.0), 10.0);
        assert_eq!(config.min_element_height(200.0), 20.0);
    }

    #[test]
    fn clamped_element_height_keeps_a_bar_between_its_floor_and_ceiling() {
        let config = config(4.0, 6.0);
        let canvas = 60.0;

        assert_eq!(
            config.clamped_element_height(1000.0, canvas),
            config.max_element_height(canvas),
        );
        assert_eq!(
            config.clamped_element_height(0.0, canvas),
            config.min_element_height(canvas),
        );
        assert_eq!(config.clamped_element_height(20.0, canvas), 20.0);
    }

    #[test]
    fn the_ceiling_wins_when_it_sits_below_the_floor() {
        // A safety margin that eats the whole canvas leaves the floor above
        // the ceiling. The ceiling has to win, or the bar is drawn outside
        // the panel.
        let config = config(20.0, 100.0);

        assert_eq!(config.clamped_element_height(50.0, 60.0), 1.0);
        assert_eq!(
            config.clamped_element_height(50.0, 60.0),
            config.max_element_height(60.0)
        );
    }
}

#[cfg(test)]
mod render_tests {
    //! What every visualization promises, drawn for real: it paints the color
    //! the user picked, and it stays inside the strip, however loud the audio,
    //! on whichever side.
    use super::test_support::{
        HEIGHT, WIDTH, bands, flat_bands, render_visualization, rgb, same_color,
    };
    use super::{
        CenteredBarsVisualization, EqualizerVisualization, PulseVisualization,
        VisualizationRenderer, WaveformVisualization,
    };
    use crate::models::palette::{Gradient, STOPS};
    use crate::models::theme::VisualizationSide;
    use crate::ui::components::visualizations::test_support::{GREEN, RED, render_gradient};
    use cosmic::iced::Color;

    fn all() -> [(&'static str, Box<dyn VisualizationRenderer>); 4] {
        [
            (
                "centered bars",
                Box::new(CenteredBarsVisualization::default()),
            ),
            ("equalizer", Box::new(EqualizerVisualization::default())),
            ("pulse", Box::new(PulseVisualization::default())),
            ("waveform", Box::new(WaveformVisualization::default())),
        ]
    }

    const SIDES: [VisualizationSide; 3] = [
        VisualizationSide::Full,
        VisualizationSide::Left,
        VisualizationSide::Right,
    ];

    #[test]
    fn every_visualization_paints_the_color_picked() {
        for color in [RED, GREEN] {
            for (name, visualization) in all() {
                let pixels = render_visualization(
                    visualization.as_ref(),
                    &flat_bands(32, 1.5),
                    &VisualizationSide::Full,
                    color,
                );
                let painted = pixels.paint_color();
                assert!(
                    same_color(painted, rgb(color)),
                    "{name} painted {painted:?} for {color:?}"
                );
            }
        }
    }

    /// The gradient follows how a style grows: what grows from the bottom is
    /// deep at the bottom and light at the top; what grows out of the middle
    /// is deep at the middle and light toward both edges.
    #[test]
    fn the_gradient_follows_how_each_style_grows() {
        let mut gradient = Gradient::solid(RED);
        for stop in &mut gradient.stops[STOPS / 2..] {
            *stop = GREEN;
        }
        for (name, visualization) in all().into_iter().take(3) {
            let pixels = render_gradient(
                visualization.as_ref(),
                &flat_bands(32, 3.0),
                &VisualizationSide::Full,
                &gradient,
            );
            let x = pixels.heaviest_column(0..WIDTH as i32);
            let (top, bottom) = pixels.painted_rows(x).expect("a full bar");
            let [low, middle, high] =
                [bottom - 1, (top + bottom) / 2, top + 1].map(|y| pixels.color_at(x, y));
            assert!(same_color(high, rgb(GREEN)), "{name} top {high:?}");
            if name == "equalizer" {
                assert!(same_color(low, rgb(RED)), "{name} bottom {low:?}");
            } else {
                assert!(same_color(middle, rgb(RED)), "{name} middle {middle:?}");
                assert!(same_color(low, rgb(GREEN)), "{name} bottom {low:?}");
            }
        }
    }

    /// The applet draws into a strip of someone else's panel: paint past it
    /// lands on the neighbours. A pixel of slack is for antialiasing, and the
    /// waveform's round line caps.
    #[test]
    fn nothing_is_painted_outside_the_strip_however_loud() {
        let spike = {
            let mut b = vec![0.0; 32];
            b[15] = 1000.0;
            b[16] = 1000.0;
            bands(b)
        };
        let cases = [
            flat_bands(32, 0.0),
            flat_bands(32, 1.5),
            flat_bands(32, 3.0),
            flat_bands(64, 1000.0),
            spike,
        ];
        for (name, visualization) in all() {
            for side in &SIDES {
                for data in &cases {
                    let pixels =
                        render_visualization(visualization.as_ref(), data, side, Color::WHITE);
                    assert_eq!(
                        pixels.painted_outside(WIDTH, HEIGHT, 1, 1),
                        0,
                        "{name} on {side:?} painted outside the strip for {:?}",
                        &data.bands[..4]
                    );
                }
            }
        }
    }

    /// Louder audio has to look louder, in every style.
    #[test]
    fn louder_audio_paints_more() {
        for (name, visualization) in all() {
            let ink = |level| {
                render_visualization(
                    visualization.as_ref(),
                    &flat_bands(32, level),
                    &VisualizationSide::Full,
                    Color::WHITE,
                )
                .ink()
            };
            let (quiet, loud, louder) = (ink(0.3), ink(1.5), ink(3.0));
            assert!(
                quiet < loud && loud < louder,
                "{name}: {quiet} / {loud} / {louder}"
            );
        }
    }
}
