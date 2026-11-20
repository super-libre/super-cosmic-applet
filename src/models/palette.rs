// SPDX-License-Identifier: GPL-3.0-only
//! The color pairs the panel draws the two products in.
//!
//! Each product gets a [`Gradient`], and the two gradients of a
//! [`ColorPalette`] are designed as a family:
//!
//! - their hues are related, as similar hues make the most liked and most
//!   harmonious pairs (Schloss & Palmer 2011), while each product keeps a hue
//!   family of its own;
//! - each runs from a deeper, cooler end to a lighter, warmer one (hue
//!   shifting), over a moderate lightness range (Ou & Luo 2006);
//! - no browns, olives or ochres, the least liked colors (Palmer & Schloss
//!   2010): on a light panel a yellow turns toward amber and orange instead
//!   of darkening into olive;
//! - every color clears WCAG's 3:1 non-text contrast against its panel.
//!
//! The pairs are designed in OKLCH for COSMIC's dark panel, in
//! [`ColorPalette::design`], the one place to tailor them. The light panel's
//! shades are derived from that design, and each gradient is spread over
//! [`STOPS`] colors in OKLCH, so its middle stays as vivid as its ends.
use std::f64::consts::FRAC_PI_4;
use std::sync::LazyLock;

use cosmic::iced::{
    Color, Point, Rectangle,
    widget::canvas::{gradient::Linear, stroke},
};
use serde::{Deserialize, Serialize};

use crate::features::Feature;
use crate::util::f64_to_f32;

/// How many colors a gradient holds. iced draws up to 8 stops; between
/// neighbors it blends in sRGB, so the stops sit close enough together that
/// the blend can't gray out.
pub const STOPS: usize = 7;

/// One product's gradient: [`STOPS`] colors from its deep, cool end to its
/// light, warm one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gradient {
    pub stops: [Color; STOPS],
}

impl Gradient {
    /// `color` all the way along.
    pub fn solid(color: Color) -> Self {
        Self {
            stops: [color; STOPS],
        }
    }

    /// The same gradient, `alpha` times as opaque.
    #[must_use]
    pub fn with_alpha(mut self, alpha: f32) -> Self {
        for stop in &mut self.stops {
            stop.a *= alpha;
        }
        self
    }

    /// A canvas fill for what grows out of the middle of `bounds`: the deep
    /// end along the middle, the light end at the top and bottom edges, so
    /// whatever reaches further from the middle reaches lighter colors.
    ///
    /// Mirrored, the gradient needs twice the stops iced draws, so each half
    /// takes every other one: seven in all, as evenly spaced as before. They
    /// go in from the top down, as iced takes them: a stop added before one
    /// above it overwrites a stop instead.
    pub fn center_out(&self, bounds: Rectangle) -> stroke::Style {
        let start = Point::new(bounds.x, bounds.y);
        let end = Point::new(bounds.x, bounds.y + bounds.height);
        let every_other: Vec<usize> = (0..STOPS).step_by(2).collect();
        let top_half = every_other
            .iter()
            .rev()
            .map(|&i| (0.5 - offset(i) / 2.0, i));
        let bottom_half = every_other
            .iter()
            .skip(1)
            .map(|&i| (0.5 + offset(i) / 2.0, i));
        let linear = top_half
            .chain(bottom_half)
            .fold(Linear::new(start, end), |linear, (at, i)| {
                linear.add_stop(at, self.stops[i])
            });
        stroke::Style::Gradient(linear.into())
    }

    /// A canvas fill running up `bounds`, for what grows from the bottom: the
    /// deep end along the bottom edge, the light end along the top. Whatever
    /// is drawn takes the color of the height it reaches, so taller shapes
    /// reach the light end.
    pub fn bottom_to_top(&self, bounds: Rectangle) -> stroke::Style {
        let start = Point::new(bounds.x, bounds.y + bounds.height);
        let end = Point::new(bounds.x, bounds.y);
        let linear = self
            .stops
            .iter()
            .enumerate()
            .fold(Linear::new(start, end), |linear, (i, color)| {
                linear.add_stop(offset(i), *color)
            });
        stroke::Style::Gradient(linear.into())
    }
}

/// Where stop `i` of [`STOPS`] sits along a gradient, from 0 to 1.
pub fn offset(i: usize) -> f32 {
    crate::util::usize_to_f32(i) / crate::util::usize_to_f32(STOPS - 1)
}

/// A pair of gradients, one for each product, the user picks from the popup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ColorPalette {
    /// Built around the system's accent color, so it follows the theme. The
    /// popup offers it first, apart from the pairs with colors of their own.
    Accent,
    #[default]
    Sunset,
    Synthwave,
    Nebula,
    Aurora,
    Candy,
    Glacier,
    Rose,
    EmberAndAsh,
    CitrusAndSea,
    Harbor,
    FernAndPlum,
    Graphite,
}

impl ColorPalette {
    /// Every palette, in the order the popup offers them, which is their
    /// declaration order.
    pub const ALL: [ColorPalette; 13] = [
        ColorPalette::Accent,
        ColorPalette::Sunset,
        ColorPalette::Synthwave,
        ColorPalette::Nebula,
        ColorPalette::Aurora,
        ColorPalette::Candy,
        ColorPalette::Glacier,
        ColorPalette::Rose,
        ColorPalette::EmberAndAsh,
        ColorPalette::CitrusAndSea,
        ColorPalette::Harbor,
        ColorPalette::FernAndPlum,
        ColorPalette::Graphite,
    ];

    pub fn pretty_name(self) -> &'static str {
        match self {
            ColorPalette::Sunset => "Sunset",
            ColorPalette::Synthwave => "Synthwave",
            ColorPalette::Nebula => "Nebula",
            ColorPalette::Aurora => "Aurora",
            ColorPalette::Accent => "Accent",
            ColorPalette::Candy => "Candy",
            ColorPalette::Glacier => "Glacier",
            ColorPalette::Rose => "Rosé",
            ColorPalette::EmberAndAsh => "Ember & Ash",
            ColorPalette::CitrusAndSea => "Citrus & Sea",
            ColorPalette::Harbor => "Harbor",
            ColorPalette::FernAndPlum => "Fern & Plum",
            ColorPalette::Graphite => "Graphite",
        }
    }

    /// `product`'s gradient, in the shades for a dark or a light panel: what
    /// the bars draw in, and the popup's circles show.
    ///
    /// [`Self::Accent`] is built on `accent`, the theme's own accent color,
    /// and keeps it exactly as it is on either panel: the theme picked it for
    /// its own colors, so it isn't deepened or lightened to fit the panel.
    /// The other palettes ignore `accent`.
    pub fn gradient(self, product: Feature, is_dark: bool, accent: Color) -> Gradient {
        if self.design().is_some() {
            let panel = Panel::of(is_dark);
            return DESIGNED[self as usize][product as usize][panel as usize];
        }
        let stops =
            spread(&accent_ends(accent, product)).map(|point| linear_to_color(in_gamut(point)));
        Gradient { stops }
    }

    /// The pairs themselves, the one place to tailor them: speech to text's
    /// gradient, then text to speech's, as OKLCH (lightness, chroma, hue)
    /// ends on COSMIC's dark panel, deep and cool end first. The light
    /// panel's shades are derived from these, unless a design gives its own.
    /// `None` for [`Self::Accent`], which is built from the accent.
    fn design(self) -> Option<Design> {
        let pair = |stt, tts| {
            Some(Design {
                stt,
                tts,
                light: [None, None],
            })
        };
        match self {
            // Coral into amber, against violet into pink.
            ColorPalette::Sunset => pair(
                [lch(0.70, 0.19, 28.0), lch(0.86, 0.15, 72.0)],
                [lch(0.60, 0.21, 303.0), lch(0.74, 0.19, 352.0)],
            ),
            // Hot pink into orange, against purple into cyan.
            ColorPalette::Synthwave => pair(
                [lch(0.66, 0.24, 355.0), lch(0.82, 0.16, 58.0)],
                [lch(0.56, 0.23, 296.0), lch(0.84, 0.13, 205.0)],
            ),
            // Magenta into rose, against indigo into cyan.
            ColorPalette::Nebula => pair(
                [lch(0.62, 0.25, 330.0), lch(0.82, 0.12, 12.0)],
                [lch(0.52, 0.21, 280.0), lch(0.82, 0.12, 212.0)],
            ),
            // Teal into mint, against violet into sky.
            ColorPalette::Aurora => pair(
                [lch(0.74, 0.14, 192.0), lch(0.89, 0.16, 152.0)],
                [lch(0.60, 0.19, 292.0), lch(0.78, 0.13, 236.0)],
            ),
            ColorPalette::Accent => None,
            // Bubblegum into peach, against lilac into baby blue.
            ColorPalette::Candy => pair(
                [lch(0.80, 0.13, 358.0), lch(0.90, 0.08, 60.0)],
                [lch(0.75, 0.12, 296.0), lch(0.86, 0.08, 232.0)],
            ),
            // Cyan into ice, against indigo into periwinkle.
            ColorPalette::Glacier => pair(
                [lch(0.80, 0.12, 214.0), lch(0.95, 0.045, 198.0)],
                [lch(0.56, 0.18, 280.0), lch(0.80, 0.09, 266.0)],
            ),
            // Rose into champagne, against mauve into silver lilac.
            ColorPalette::Rose => pair(
                [lch(0.74, 0.12, 14.0), lch(0.90, 0.06, 72.0)],
                [lch(0.62, 0.10, 322.0), lch(0.84, 0.045, 300.0)],
            ),
            // Glowing red into gold, against slate into ash.
            ColorPalette::EmberAndAsh => pair(
                [lch(0.64, 0.21, 30.0), lch(0.88, 0.16, 82.0)],
                [lch(0.58, 0.05, 258.0), lch(0.80, 0.025, 240.0)],
            ),
            // Tangerine into lemon, against deep teal into aqua.
            ColorPalette::CitrusAndSea => pair(
                [lch(0.78, 0.17, 58.0), lch(0.93, 0.16, 104.0)],
                [lch(0.60, 0.11, 205.0), lch(0.82, 0.13, 178.0)],
            ),
            // Orange into amber, against blue into sky: the clearest pair for
            // color-blind viewers.
            ColorPalette::Harbor => pair(
                [lch(0.76, 0.17, 52.0), lch(0.88, 0.15, 80.0)],
                [lch(0.58, 0.17, 262.0), lch(0.77, 0.13, 238.0)],
            ),
            // Silver into white, against graphite into steel; on a light
            // panel, black into charcoal against slate.
            ColorPalette::Graphite => Some(Design {
                stt: [lch(0.86, 0.0, 0.0), lch(0.99, 0.0, 0.0)],
                tts: [lch(0.56, 0.02, 262.0), lch(0.72, 0.015, 255.0)],
                light: [
                    Some([lch(0.20, 0.0, 0.0), lch(0.34, 0.0, 0.0)]),
                    Some([lch(0.46, 0.025, 262.0), lch(0.56, 0.02, 255.0)]),
                ],
            }),
            // Emerald into lime, against plum into orchid: the green the
            // others lack. On a light panel the lime would darken into olive,
            // so its speech to text runs deep emerald into a clean green
            // there.
            ColorPalette::FernAndPlum => Some(Design {
                stt: [lch(0.68, 0.17, 150.0), lch(0.90, 0.17, 132.0)],
                tts: [lch(0.52, 0.18, 315.0), lch(0.74, 0.15, 330.0)],
                light: [Some([lch(0.40, 0.13, 158.0), lch(0.56, 0.16, 146.0)]), None],
            }),
        }
    }
}

/// The least chroma the accent pair's other colors have, however pale the
/// accent: COSMIC's own accents have about 0.10. The accent itself is never
/// changed.
const ACCENT_CHROMA: f64 = 0.10;

/// Below this chroma an accent is a gray, with no hue to build on.
const GRAY_CHROMA: f64 = 0.02;

/// How far round the hue circle text to speech's color sits from the accent,
/// at its light end and its deep one.
const COMPANION_TURN: (f64, f64) = (60.0, 95.0);

/// Hues where a companion would land in yellow-green and olive, the least
/// liked colors: a turn that lands here goes the other way instead.
const MURKY_HUES: std::ops::Range<f64> = 95.0..150.0;

/// How far round the hue circle text to speech's light and deep ends sit
/// from an accent of hue `h`: [`COMPANION_TURN`] one way, or the other way
/// where that would land in [`MURKY_HUES`].
fn companion_turns(h: f64) -> (f64, f64) {
    let (near, far) = COMPANION_TURN;
    if MURKY_HUES.contains(&(h + near).rem_euclid(360.0)) {
        (-near, -far)
    } else {
        (near, far)
    }
}

/// The color the system's accent is, in `cosmic_theme`.
pub fn accent_of(cosmic_theme: &cosmic::cosmic_theme::Theme) -> Color {
    let accent = cosmic_theme.accent.base.color;
    Color::from_rgb(accent.red, accent.green, accent.blue)
}

/// The colors one product draws in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProductColors {
    /// `product`'s gradient in a palette, in the shades of the theme it's
    /// drawn on.
    Palette(ColorPalette, Feature),
    /// One gradient whatever the theme: colors a test knows.
    #[cfg(test)]
    Fixed(Gradient),
}

impl Default for ProductColors {
    fn default() -> Self {
        Self::Palette(ColorPalette::default(), Feature::Stt)
    }
}

impl ProductColors {
    /// The gradient to draw with on `cosmic_theme`.
    pub fn gradient(self, cosmic_theme: &cosmic::cosmic_theme::Theme) -> Gradient {
        match self {
            Self::Palette(palette, product) => {
                palette.gradient(product, cosmic_theme.is_dark, accent_of(cosmic_theme))
            }
            #[cfg(test)]
            Self::Fixed(gradient) => gradient,
        }
    }
}

/// The two panels a gradient is fitted to: COSMIC's default dark and light
/// ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Panel {
    Dark,
    Light,
}

impl Panel {
    fn of(is_dark: bool) -> Self {
        if is_dark { Self::Dark } else { Self::Light }
    }

    /// The panel's background, in linear RGB.
    fn background(self) -> [f64; 3] {
        let level = srgb_to_linear(match self {
            Self::Dark => f64::from(0x1b_u8) / 255.0,
            Self::Light => f64::from(0xd7_u8) / 255.0,
        });
        [level; 3]
    }
}

/// The contrast every color keeps with its panel: WCAG's for meaningful
/// graphics.
pub const MIN_CONTRAST: f64 = 3.0;

/// A color as OKLCH: lightness from 0 to 1, chroma, and hue in degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Lch {
    l: f64,
    c: f64,
    h: f64,
}

const fn lch(l: f64, c: f64, h: f64) -> Lch {
    Lch { l, c, h }
}

/// A pair's two gradients, each as its two ends.
struct Design {
    stt: [Lch; 2],
    tts: [Lch; 2],
    /// A product's own shades for a light panel, speech to text's then text
    /// to speech's, instead of the derived ones.
    light: [Option<[Lch; 2]>; 2],
}

/// Every designed palette's gradients, by palette, product and panel,
/// worked out once.
static DESIGNED: LazyLock<Vec<[[Gradient; 2]; 2]>> = LazyLock::new(|| {
    ColorPalette::ALL
        .iter()
        .map(|palette| {
            let Some(design) = palette.design() else {
                // Built from the accent when drawn; never read from here.
                return [[Gradient::solid(Color::BLACK); 2]; 2];
            };
            let light = [
                design.light[0].unwrap_or_else(|| for_light_panel(&design.stt)),
                design.light[1].unwrap_or_else(|| for_light_panel(&design.tts)),
            ];
            [
                [
                    build(&design.stt, Panel::Dark),
                    build(&light[0], Panel::Light),
                ],
                [
                    build(&design.tts, Panel::Dark),
                    build(&light[1], Panel::Light),
                ],
            ]
        })
        .collect()
});

/// The accent palette's two gradients: speech to text runs from a slightly
/// deeper shade of the accent up to the accent itself, unchanged, and text
/// to speech is a neighboring hue at the accent's lightness, deepening a
/// little. The pair reads as the accent and its companion, not as colors
/// drifted away from it. On a light panel, [`fit`] deepens what the panel
/// needs deeper, as for every pair.
fn accent_design(accent: Color) -> [[Lch; 2]; 2] {
    let Lch { l, c, h } = linear_to_lch(color_to_linear(accent));
    // The other colors keep some chroma however pale the accent, unless it's
    // a gray, which has no hue to give them.
    let other = if c < GRAY_CHROMA {
        c
    } else {
        c.max(ACCENT_CHROMA)
    };
    let (near, far) = companion_turns(h);
    [
        [lch(l - 0.10, other * 1.1, h - 15.0), lch(l, c, h)],
        [
            lch(l - 0.12, other * 1.3, h + far),
            lch(l, other * 1.05, h + near),
        ],
    ]
}

/// A dark-panel design, deepened for a light panel.
///
/// A light panel leaves lightness from about 0.25 to 0.58 for 3:1, so the
/// design's lightness maps into that band, keeping its spread. Yellows and
/// ambers compress toward orange rather than darkening into olive, keeping
/// their order, so a gradient through them still moves.
fn for_light_panel(ends: &[Lch; 2]) -> [Lch; 2] {
    let top = ends[0].l.max(ends[1].l);
    ends.map(|Lch { l, c, h }| {
        let h = if (40.0..125.0).contains(&h) && c > 0.03 {
            35.0 + (h - 40.0) * (35.0 / 85.0)
        } else {
            h
        };
        lch(0.56 - (top - l) * 0.8, c * 1.05, h)
    })
}

/// `product`'s ends in the accent pair built on `accent`.
fn accent_ends(accent: Color, product: Feature) -> [Lch; 2] {
    let [stt, tts] = accent_design(accent);
    match product {
        Feature::Stt => stt,
        Feature::Tts => tts,
    }
}

/// [`STOPS`] colors from one end to the other, evenly in OKLCH, the shorter
/// way round the hue circle.
fn spread(ends: &[Lch; 2]) -> [Lch; STOPS] {
    let [from, to] = *ends;
    let turn = (to.h - from.h + 180.0).rem_euclid(360.0) - 180.0;
    std::array::from_fn(|i| {
        let t = f64::from(offset(i));
        lch(
            from.l + (to.l - from.l) * t,
            from.c + (to.c - from.c) * t,
            from.h + turn * t,
        )
    })
}

/// The gradient between `ends`, spread over [`STOPS`], each stop fitted
/// into sRGB and to `panel`.
fn build(ends: &[Lch; 2], panel: Panel) -> Gradient {
    Gradient {
        stops: spread(ends).map(|point| linear_to_color(fit(point, panel))),
    }
}

/// `color` in linear RGB, darkened on a light panel or lightened on a dark
/// one until it clears [`MIN_CONTRAST`].
fn fit(color: Lch, panel: Panel) -> [f64; 3] {
    let background = panel.background();
    let step = match panel {
        Panel::Dark => 0.005,
        Panel::Light => -0.005,
    };
    let mut l = color.l;
    let mut rgb = in_gamut(lch(l, color.c, color.h));
    while contrast(rgb, background) < MIN_CONTRAST && l > 0.05 && l < 0.98 {
        l += step;
        rgb = in_gamut(lch(l, color.c, color.h));
    }
    rgb
}

/// `color` in linear RGB, its chroma lowered until it fits in sRGB.
fn in_gamut(color: Lch) -> [f64; 3] {
    let at = |c: f64| {
        let (sin, cos) = color.h.to_radians().sin_cos();
        oklab_to_linear([color.l, c * cos, c * sin])
    };
    let fits = |rgb: &[f64; 3]| rgb.iter().all(|v| (-1e-6..=1.0 + 1e-6).contains(v));
    let (mut low, mut high) = (0.0, color.c);
    let mut rgb = at(0.0);
    for _ in 0..24 {
        let mid = f64::midpoint(low, high);
        let candidate = at(mid);
        if fits(&candidate) {
            low = mid;
            rgb = candidate;
        } else {
            high = mid;
        }
    }
    if fits(&at(color.c)) {
        rgb = at(color.c);
    }
    rgb.map(|v| v.clamp(0.0, 1.0))
}

/// WCAG's contrast ratio between two linear RGB colors.
pub(crate) fn contrast(a: [f64; 3], b: [f64; 3]) -> f64 {
    let luminance = |c: [f64; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

fn srgb_to_linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(v: f64) -> f64 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.003_130_8 {
        12.92 * v
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

pub(crate) fn color_to_linear(color: Color) -> [f64; 3] {
    [color.r, color.g, color.b].map(|v| srgb_to_linear(f64::from(v)))
}

fn linear_to_color(rgb: [f64; 3]) -> Color {
    let [r, g, b] = rgb.map(|v| f64_to_f32(linear_to_srgb(v)));
    Color::from_rgb(r, g, b)
}

/// Linear sRGB to Oklab, with Ottosson's published matrices.
#[allow(clippy::unreadable_literal)] // published constants, kept as written
#[allow(clippy::many_single_char_names)] // the color channels' standard names
pub(crate) fn linear_to_oklab([r, g, b]: [f64; 3]) -> [f64; 3] {
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    [
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    ]
}

/// Oklab to linear sRGB, with Ottosson's published matrices.
#[allow(clippy::unreadable_literal)] // published constants, kept as written
#[allow(clippy::many_single_char_names)] // the color channels' standard names
fn oklab_to_linear([l, a, b]: [f64; 3]) -> [f64; 3] {
    let l_ = (l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m_ = (l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s_ = (l - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    [
        4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_,
        -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_,
        -0.0041960863 * l_ - 0.7034186147 * m_ + 1.7076147010 * s_,
    ]
}

fn linear_to_lch(rgb: [f64; 3]) -> Lch {
    let [l, a, b] = linear_to_oklab(rgb);
    lch(l, a.hypot(b), b.atan2(a).to_degrees().rem_euclid(360.0))
}

/// The angle the popup's swatches blend at: from top left to bottom right.
pub const SWATCH_ANGLE: f64 = 3.0 * FRAC_PI_4;

#[cfg(test)]
mod tests {
    use super::{
        ColorPalette, Gradient, MIN_CONTRAST, Panel, ProductColors, STOPS, color_to_linear,
        contrast, linear_to_oklab,
    };
    use crate::features::Feature::{self, Stt, Tts};
    use cosmic::cosmic_theme::Theme;
    use cosmic::iced::Color;

    /// COSMIC's default accent on each theme.
    fn accent(is_dark: bool) -> Color {
        let theme = if is_dark {
            Theme::dark_default()
        } else {
            Theme::light_default()
        };
        super::accent_of(&theme)
    }

    fn every_gradient() -> Vec<(ColorPalette, Feature, bool, Gradient)> {
        let mut all = Vec::new();
        for palette in ColorPalette::ALL {
            for product in [Stt, Tts] {
                for is_dark in [true, false] {
                    all.push((
                        palette,
                        product,
                        is_dark,
                        palette.gradient(product, is_dark, accent(is_dark)),
                    ));
                }
            }
        }
        all
    }

    /// The distance between two colors in Oklab: about 0.02 is the least
    /// anyone notices.
    fn distance(a: Color, b: Color) -> f64 {
        let (a, b) = (
            linear_to_oklab(color_to_linear(a)),
            linear_to_oklab(color_to_linear(b)),
        );
        a.iter()
            .zip(b)
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    /// A gradient's average color, in Oklab.
    fn mean(gradient: &Gradient) -> [f64; 3] {
        let mut sum = [0.0; 3];
        for stop in gradient.stops {
            let lab = linear_to_oklab(color_to_linear(stop));
            for (s, v) in sum.iter_mut().zip(lab) {
                *s += v;
            }
        }
        sum.map(|s| s / f64::from(u8::try_from(STOPS).unwrap()))
    }

    #[test]
    fn the_popup_offers_every_palette_in_declaration_order() {
        assert_eq!(
            ColorPalette::ALL[0],
            ColorPalette::Accent,
            "the accent leads"
        );
        for (i, palette) in ColorPalette::ALL.iter().enumerate() {
            assert_eq!(*palette as usize, i, "{palette:?}");
        }
        let names: std::collections::BTreeSet<&str> =
            ColorPalette::ALL.iter().map(|p| p.pretty_name()).collect();
        assert_eq!(names.len(), ColorPalette::ALL.len());
    }

    #[test]
    fn every_color_clears_three_to_one_against_its_panel() {
        // The accent pair keeps the theme's accent as it is.
        let fitted = every_gradient()
            .into_iter()
            .filter(|(palette, ..)| *palette != ColorPalette::Accent);
        for (palette, product, is_dark, gradient) in fitted {
            let panel = Panel::of(is_dark).background();
            for stop in gradient.stops {
                let ratio = contrast(color_to_linear(stop), panel);
                assert!(
                    ratio >= MIN_CONTRAST - 0.01,
                    "{palette:?} {product:?} on a {} panel: {stop:?} at {ratio:.2}:1",
                    if is_dark { "dark" } else { "light" }
                );
            }
        }
    }

    /// The point of a gradient: it moves, and it doesn't jump.
    #[test]
    fn every_gradient_moves_smoothly() {
        for (palette, product, is_dark, gradient) in every_gradient() {
            let ends = distance(gradient.stops[0], gradient.stops[STOPS - 1]);
            assert!(
                ends > 0.04,
                "{palette:?} {product:?} {is_dark}: flat ({ends:.3})"
            );
            for pair in gradient.stops.windows(2) {
                let step = distance(pair[0], pair[1]);
                assert!(
                    step < 0.12,
                    "{palette:?} {product:?} {is_dark}: jumps {step:.3}"
                );
            }
        }
    }

    /// No color sinks into olive or a dark yellow-green, the least liked
    /// colors: a deep shade of a yellow-green hue is what makes one. (The
    /// accent's companion keeps a wider berth, [`super::MURKY_HUES`], to land
    /// clearly away from them; a deep emerald past it is a clean green.)
    #[test]
    fn no_color_turns_olive() {
        const OLIVE: std::ops::Range<f64> = 95.0..135.0;
        for (palette, product, is_dark, gradient) in every_gradient() {
            for stop in gradient.stops {
                let [l, a, b] = linear_to_oklab(color_to_linear(stop));
                let (chroma, hue) = (a.hypot(b), b.atan2(a).to_degrees().rem_euclid(360.0));
                assert!(
                    !(l < 0.7 && chroma > 0.05 && OLIVE.contains(&hue)),
                    "{palette:?} {product:?} {is_dark}: {stop:?} at {hue:.0}°, lightness {l:.2}"
                );
            }
        }
    }

    /// The products stay apart on either panel, enough to tell which is in
    /// front from its color alone.
    #[test]
    fn the_two_products_are_told_apart() {
        for palette in ColorPalette::ALL {
            for is_dark in [true, false] {
                let [stt, tts] =
                    [Stt, Tts].map(|p| mean(&palette.gradient(p, is_dark, accent(is_dark))));
                let apart = stt
                    .iter()
                    .zip(tts)
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f64>()
                    .sqrt();
                assert!(
                    apart > 0.10,
                    "{palette:?} on a {} panel: {apart:.3}",
                    if is_dark { "dark" } else { "light" }
                );
            }
        }
    }

    /// Each gradient runs from its deep end to its light one, which the bars
    /// draw from the bottom up.
    #[test]
    fn every_gradient_starts_deep() {
        for (palette, product, is_dark, gradient) in every_gradient() {
            let [first, last] = [gradient.stops[0], gradient.stops[STOPS - 1]]
                .map(|c| linear_to_oklab(color_to_linear(c))[0]);
            let ok = if is_dark {
                first < last
            } else {
                first <= last + 0.02
            };
            assert!(
                ok,
                "{palette:?} {product:?} {is_dark}: {first:.2} to {last:.2}"
            );
        }
    }

    #[test]
    fn the_accent_palette_follows_the_accent() {
        for accent in [
            Color::from_rgb(0.9, 0.2, 0.2),
            Color::from_rgb(0.2, 0.4, 0.9),
            accent(true),
        ] {
            // Speech to text's bars reach exactly the accent at their tips.
            let gradient = ColorPalette::Accent.gradient(Stt, true, accent);
            let tips = gradient.stops[STOPS - 1];
            assert!(distance(tips, accent) < 0.005, "{accent:?}: {tips:?}");
        }
        // Every other palette ignores it.
        assert_eq!(
            ColorPalette::Sunset.gradient(Stt, true, Color::WHITE),
            ColorPalette::Sunset.gradient(Stt, true, Color::BLACK)
        );
    }

    #[test]
    fn a_product_draws_its_half_of_the_palette_in_the_themes_shades() {
        let colors = ProductColors::Palette(ColorPalette::Aurora, Tts);
        assert_eq!(
            colors.gradient(&Theme::dark_default()),
            ColorPalette::Aurora.gradient(Tts, true, accent(true))
        );
        assert_eq!(
            colors.gradient(&Theme::light_default()),
            ColorPalette::Aurora.gradient(Tts, false, accent(false))
        );
        assert_ne!(
            colors.gradient(&Theme::dark_default()),
            colors.gradient(&Theme::light_default())
        );
        let fixed = Gradient::solid(Color::WHITE);
        assert_eq!(
            ProductColors::Fixed(fixed).gradient(&Theme::light_default()),
            fixed
        );
    }

    /// The popup shows a pair in its own colors on either theme: the dark
    /// panel's, which the light panel's only deepen.
    #[test]
    fn the_designed_pairs_have_their_own_light_panel_shades() {
        for palette in ColorPalette::ALL {
            if palette == ColorPalette::Accent {
                continue;
            }
            for product in [Stt, Tts] {
                assert_ne!(
                    palette.gradient(product, true, accent(true)),
                    palette.gradient(product, false, accent(false)),
                    "{palette:?} {product:?} looks the same on either panel"
                );
            }
        }
    }

    /// Whatever the accent, pale like a sand, deep like a light theme's, or
    /// one of COSMIC's own, the pair shows it exactly as the system has it on
    /// either panel, and its companion has a color of its own, never a murky
    /// yellow-green.
    #[test]
    fn the_accent_pair_shows_the_accent_as_it_is() {
        let accents = [
            ("sand", Color::from_rgb8(0xee, 0xd4, 0x9f)),
            ("a light theme's teal", accent(false)),
            ("COSMIC's cyan", accent(true)),
            ("orange", Color::from_rgb8(0xe0, 0x8a, 0x20)),
            ("purple", Color::from_rgb8(0xa2, 0x8a, 0xff)),
        ];
        for ((name, accent), is_dark) in accents.into_iter().flat_map(|a| [(a, true), (a, false)]) {
            let stt = ColorPalette::Accent.gradient(Stt, is_dark, accent);
            let shown = stt.stops[STOPS - 1];
            assert!(distance(shown, accent) < 0.005, "{name}: shows {shown:?}");

            let tts = ColorPalette::Accent.gradient(Tts, is_dark, accent);
            for stop in tts.stops {
                let [_, a, b] = linear_to_oklab(color_to_linear(stop));
                let (chroma, hue) = (a.hypot(b), b.atan2(a).to_degrees().rem_euclid(360.0));
                assert!(chroma > 0.05, "{name}: a gray companion, {chroma:.3}");
                assert!(
                    !super::MURKY_HUES.contains(&hue),
                    "{name}: a murky companion at {hue:.0}°"
                );
            }
        }
    }

    /// The centered fill keeps all seven of its stops, mirrored about the
    /// middle: none overwritten by another added out of order.
    #[test]
    fn the_centered_fill_mirrors_the_gradient_about_the_middle() {
        use cosmic::iced::widget::canvas::{Gradient as Canvas, stroke::Style};
        let gradient = ColorPalette::Sunset.gradient(Stt, true, Color::WHITE);
        let bounds = cosmic::iced::Rectangle::new(
            cosmic::iced::Point::ORIGIN,
            cosmic::iced::Size::new(10.0, 40.0),
        );
        let Style::Gradient(Canvas::Linear(linear)) = gradient.center_out(bounds) else {
            panic!("not a gradient");
        };
        let stops: Vec<_> = linear.stops.iter().flatten().collect();
        assert_eq!(stops.len(), 7);
        let expected = [6, 4, 2, 0, 2, 4, 6];
        for (stop, (i, at)) in stops.iter().zip(expected.iter().zip([
            0.0,
            1.0 / 6.0,
            1.0 / 3.0,
            0.5,
            2.0 / 3.0,
            5.0 / 6.0,
            1.0,
        ])) {
            assert!((stop.offset - at).abs() < 1e-6, "{stops:?}");
            assert_eq!(stop.color, gradient.stops[*i], "{stops:?}");
        }
    }

    #[test]
    fn transparency_applies_along_the_whole_gradient() {
        let faded = ColorPalette::Nebula
            .gradient(Tts, true, Color::WHITE)
            .with_alpha(0.25);
        assert!(faded.stops.iter().all(|c| (c.a - 0.25).abs() < 1e-6));
    }

    #[test]
    fn settings_files_name_a_palette_by_its_variant() {
        assert_eq!(
            serde_json::to_string(&ColorPalette::EmberAndAsh).unwrap(),
            "\"EmberAndAsh\""
        );
        assert_eq!(
            serde_json::from_str::<ColorPalette>("\"Rose\"").unwrap(),
            ColorPalette::Rose
        );
        assert_eq!(ColorPalette::default(), ColorPalette::Sunset);
    }
}
