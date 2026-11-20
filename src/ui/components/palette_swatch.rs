// SPDX-License-Identifier: GPL-3.0-only
//! A color palette as the popup offers it: a circle blending through both
//! products' gradients, from the top left to the bottom right, in the shades
//! the bars draw in on the theme the system is on.
use cosmic::{
    Element,
    iced::{
        Alignment, Background, Border, Color, Gradient as Fill, Length, Radians,
        gradient::Linear,
        mouse::Interaction,
        widget::{column, row},
    },
    widget::{Space, container, mouse_area, text, tooltip},
};

use crate::{
    app::Message,
    features::Feature,
    models::palette::{ColorPalette, Gradient, STOPS, SWATCH_ANGLE, accent_of},
    util::f64_to_f32,
};

/// The circle's diameter.
const SIZE: f32 = 28.0;
/// The room between the circle and the ring marking the picked palette.
const RING_GAP: f32 = 4.0;

/// The swatch that picks `palette`, ringed when it is the one `picked`.
pub fn palette_swatch(
    palette: ColorPalette,
    picked: bool,
    cosmic_theme: &cosmic::cosmic_theme::Theme,
) -> Element<'static, Message> {
    // Every swatch keeps the ring's room, drawn or not, so picking one
    // doesn't shift the others.
    let ring = container(circle(palette, cosmic_theme))
        .padding(RING_GAP)
        .style(move |theme| container::Style {
            border: Border {
                color: if picked {
                    theme.cosmic().accent_color().into()
                } else {
                    Color::TRANSPARENT
                },
                width: 2.0,
                radius: (SIZE / 2.0 + RING_GAP).into(),
            },
            ..Default::default()
        });

    tooltip(
        mouse_area(ring)
            .on_press(Message::SetColorPalette(palette))
            .interaction(Interaction::Pointer),
        text(palette.pretty_name()),
        tooltip::Position::Bottom,
    )
    .into()
}

/// The accent pair, set apart from the pairs with colors of their own: a
/// full-width row of its circle and what it is, the whole row picking it,
/// outlined in the accent when it's the one `picked`.
pub fn accent_choice(
    picked: bool,
    cosmic_theme: &cosmic::cosmic_theme::Theme,
) -> Element<'static, Message> {
    let spacing = cosmic_theme.spacing;
    let content = row![
        circle(ColorPalette::Accent, cosmic_theme),
        column![
            text::body(ColorPalette::Accent.pretty_name()),
            text::caption(ACCENT_CAPTION),
        ],
        Space::new().width(Length::Fill),
    ]
    .spacing(spacing.space_s)
    .align_y(Alignment::Center);

    mouse_area(
        container(content)
            .width(Length::Fill)
            .padding([RING_GAP, RING_GAP * 2.0])
            .style(move |theme| {
                let cosmic = theme.cosmic();
                let accent: Color = cosmic.accent_color().into();
                let on_bg: Color = cosmic.on_bg_color().into();
                container::Style {
                    background: Some(Background::Color(if picked {
                        Color { a: 0.12, ..accent }
                    } else {
                        Color { a: 0.04, ..on_bg }
                    })),
                    border: Border {
                        color: if picked {
                            accent
                        } else {
                            Color { a: 0.12, ..on_bg }
                        },
                        width: if picked { 2.0 } else { 1.0 },
                        radius: cosmic.corner_radii.radius_m.into(),
                    },
                    ..Default::default()
                }
            }),
    )
    .on_press(Message::SetColorPalette(ColorPalette::Accent))
    .interaction(Interaction::Pointer)
    .into()
}

/// What the accent pair is, under its name.
pub const ACCENT_CAPTION: &str = "Follows your system color";

/// `palette`'s circle: both products' gradients blended from the top left
/// to the bottom right.
fn circle(
    palette: ColorPalette,
    cosmic_theme: &cosmic::cosmic_theme::Theme,
) -> Element<'static, Message> {
    let [stt, tts] = [Feature::Stt, Feature::Tts].map(|p| gradient_of(palette, p, cosmic_theme));
    // Speech to text's gradient from its light end to its deep one, then text
    // to speech's from its deep end to its light one: every other stop of
    // each, eight in all, the most iced draws. The more stops, the gentler the
    // seam where the two gradients meet: iced doesn't dither a gradient, so a
    // steep one shows as steps.
    let blend = [
        stt.stops[STOPS - 1],
        stt.stops[4],
        stt.stops[2],
        stt.stops[0],
        tts.stops[0],
        tts.stops[2],
        tts.stops[4],
        tts.stops[STOPS - 1],
    ];
    disc(SIZE, background(&blend))
}

/// A small dot of `product`'s gradient in `palette`, for the legend that
/// says whose colors are whose.
pub fn color_dot(
    palette: ColorPalette,
    product: Feature,
    cosmic_theme: &cosmic::cosmic_theme::Theme,
) -> Element<'static, Message> {
    disc(
        10.0,
        background(&gradient_of(palette, product, cosmic_theme).stops),
    )
}

/// `product`'s gradient in `palette`, in the shades of the theme the system
/// is on, as its bars draw it.
fn gradient_of(
    palette: ColorPalette,
    product: Feature,
    cosmic_theme: &cosmic::cosmic_theme::Theme,
) -> Gradient {
    palette.gradient(product, cosmic_theme.is_dark, accent_of(cosmic_theme))
}

/// `colors`, evenly spread from the top left to the bottom right.
fn background(colors: &[Color]) -> Background {
    let last = colors.len().saturating_sub(1).max(1);
    let linear = colors.iter().enumerate().fold(
        Linear::new(Radians(f64_to_f32(SWATCH_ANGLE))),
        |linear, (i, color)| {
            let at = crate::util::usize_to_f32(i) / crate::util::usize_to_f32(last);
            linear.add_stop(at, *color)
        },
    );
    Background::Gradient(Fill::Linear(linear))
}

/// A `size`-wide circle filled with `background`.
fn disc(size: f32, background: Background) -> Element<'static, Message> {
    container(Space::new())
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(move |_| container::Style {
            background: Some(background),
            border: Border {
                radius: (size / 2.0).into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::{ACCENT_CAPTION, accent_choice, palette_swatch};
    use crate::app::Message;
    use crate::models::palette::ColorPalette;
    use crate::ui::simulator::Simulator;

    fn swatch(palette: ColorPalette, picked: bool) -> Simulator<'static> {
        Simulator::new(palette_swatch(
            palette,
            picked,
            cosmic::theme::active().cosmic(),
        ))
    }

    #[test]
    fn a_swatch_picks_its_palette() {
        for palette in ColorPalette::ALL {
            let mut ui = swatch(palette, false);
            let center = ui.containers()[0].center();
            ui.click_at(center);
            match ui.into_messages().as_slice() {
                [Message::SetColorPalette(picked)] => assert_eq!(*picked, palette),
                other => panic!("{palette:?}'s swatch sent {other:?}"),
            }
        }
    }

    #[test]
    fn the_accent_row_picks_the_accent_and_says_what_it_is() {
        let theme = cosmic::theme::active();
        let mut ui = Simulator::new(accent_choice(false, theme.cosmic()));
        assert_eq!(ui.texts(), ["Accent", ACCENT_CAPTION]);
        let bounds = ui.bounds_of(ACCENT_CAPTION);
        ui.click_at(bounds.center());
        match ui.into_messages().as_slice() {
            [Message::SetColorPalette(ColorPalette::Accent)] => {}
            other => panic!("the accent row sent {other:?}"),
        }
        assert_ne!(
            Simulator::new(accent_choice(true, theme.cosmic())).screenshot(),
            Simulator::new(accent_choice(false, theme.cosmic())).screenshot(),
            "picked looks the same as not"
        );
    }

    /// A swatch shows what the bars will draw: the dark panel's shades on a
    /// dark theme, the light panel's on a light one.
    #[test]
    fn a_swatch_takes_the_themes_shades() {
        use cosmic::cosmic_theme::Theme;
        let shot = |theme: &Theme| {
            Simulator::new(palette_swatch(ColorPalette::Sunset, false, theme)).screenshot()
        };
        assert_ne!(shot(&Theme::dark_default()), shot(&Theme::light_default()));
    }

    #[test]
    fn the_picked_swatch_is_ringed() {
        assert_ne!(
            swatch(ColorPalette::Sunset, true).screenshot(),
            swatch(ColorPalette::Sunset, false).screenshot()
        );
    }

    #[test]
    fn each_palette_draws_its_own_swatch() {
        let shots: Vec<Vec<u8>> = ColorPalette::ALL
            .iter()
            .map(|p| swatch(*p, false).screenshot())
            .collect();
        for (i, shot) in shots.iter().enumerate() {
            assert!(
                !shots[..i].contains(shot),
                "{:?} looks like another palette",
                ColorPalette::ALL[i]
            );
        }
    }
}
