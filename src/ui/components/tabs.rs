// SPDX-License-Identifier: GPL-3.0-only
use cosmic::{
    Element,
    iced::{
        Background, Border, Color, Length, Padding,
        widget::{Row, column},
    },
    theme,
    widget::{button, container, divider, text},
};

use crate::{app::Message, products::Product};

/// Tab bar over the per-product settings, one tab per product in `products`:
/// flat text tabs with an accent rim marking the active one, over a
/// full-width hairline divider. The same tabs as Super STT's Library page.
pub fn product_tabs(products: &[Product], active: Product) -> Element<'static, Message> {
    let spacing = theme::active().cosmic().spacing;

    let tabs = products
        .iter()
        .fold(Row::new().spacing(spacing.space_xxs), |row, &product| {
            let is_active = product == active;
            let label = text::body(product.display_name());
            let label = if is_active {
                label.class(theme::Text::Accent)
            } else {
                label
            };

            let inner = button::custom(label)
                .padding([spacing.space_xxs, spacing.space_s])
                .on_press(Message::SelectSettingsTab(product))
                .class(tab_inner_class(is_active));

            // Accent border on the top and sides only (open at the bottom): an
            // accent-filled layer revealed as a 1px rim by per-side padding (0
            // at the bottom), with the inner button masking the centre. iced
            // borders are uniform, so this fill and mask is the only way to a
            // three-sided, top-rounded tab outline.
            row.push(
                container(inner)
                    .padding(Padding {
                        top: 1.0,
                        right: 1.0,
                        bottom: 0.0,
                        left: 1.0,
                    })
                    .class(theme::Container::custom(move |theme| {
                        let cosmic = theme.cosmic();
                        let r = cosmic.corner_radii.radius_s;
                        let bg = if is_active {
                            cosmic.accent.base.into()
                        } else {
                            Color::TRANSPARENT
                        };
                        container::Style {
                            background: Some(Background::Color(bg)),
                            border: Border {
                                radius: [r[0], r[1], 0.0, 0.0].into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    })),
            )
        });

    column![tabs, divider::horizontal::default()]
        .width(Length::Fill)
        .into()
}

/// The inner surface of a tab. The accent outline lives on the wrapping
/// container; this layer masks it down to its 1px rim. The active tab fills
/// with the popup's own background, frosted or not, so only the rim shows;
/// the others stay transparent. Hover and press take the standard button
/// fills. Top corners rounded, bottom square, to match the open-bottomed
/// outline.
fn tab_inner_class(active: bool) -> theme::Button {
    fn shape(theme: &cosmic::Theme, fill: Color) -> button::Style {
        let r = theme.cosmic().corner_radii.radius_s;
        button::Style {
            background: Some(Background::Color(fill)),
            border_radius: [r[0], r[1], 0.0, 0.0].into(),
            text_color: Some(theme.current_container().component.on.into()),
            ..Default::default()
        }
    }
    let idle_fill = move |theme: &cosmic::Theme| {
        if active {
            theme.current_container().base.into()
        } else {
            Color::TRANSPARENT
        }
    };
    theme::Button::Custom {
        active: Box::new(move |_focused, theme| shape(theme, idle_fill(theme))),
        disabled: Box::new(move |theme| shape(theme, idle_fill(theme))),
        hovered: Box::new(|_focused, theme| shape(theme, theme.cosmic().button.hover.into())),
        pressed: Box::new(|_focused, theme| shape(theme, theme.cosmic().button.pressed.into())),
    }
}
