// SPDX-License-Identifier: GPL-3.0-only
use cosmic::{
    Apply, Element,
    applet::padded_control,
    iced::{
        Alignment, Length,
        widget::{column, row, slider},
    },
    theme,
    widget::{Space, divider, segmented_control, text, toggler},
};

use crate::{
    app::Message,
    products::Product,
    ui::{
        components::tabs::product_tabs,
        sections::settings::components::visualization_theme::{
            create_visualization_color_selector, create_visualization_theme_selector,
            create_working_animation_selector,
        },
        views::PopupContentParams,
    },
};

/// The applet's settings. The panel's own (the icon, the visualization's
/// size) come first and are shared; how each product's activity is drawn sits
/// below them, under a tab per installed product, showing `tab`'s.
pub fn create_applet_settings_section<'a>(
    params: &PopupContentParams<'a>,
    tab: Product,
) -> Element<'a, Message> {
    let PopupContentParams {
        config,
        icon_alignment_model,
        ..
    } = *params;
    let spacing = theme::active().cosmic().spacing;

    let mut settings_column = column![
        // Show icon toggle
        padded_control(
            row![
                text::body("Show Icon"),
                Space::new().width(Length::Fill),
                toggler(config.ui.show_icon).on_toggle(Message::SetShowIcon)
            ]
            .spacing(spacing.space_xs)
            .align_y(Alignment::Center),
        ),
    ]
    .spacing(spacing.space_xs)
    .width(Length::Fill);

    // Icon alignment selector (only show if icon is enabled)
    if config.ui.show_icon {
        settings_column = settings_column.push(
            padded_control(
                column![
                    text::body("Icon Position"),
                    segmented_control::horizontal(icon_alignment_model)
                        .on_activate(Message::SetIconAlignmentEntity)
                ]
                .spacing(spacing.space_xxs)
                .apply(Element::from),
            )
            .apply(Element::from),
        );
    }

    settings_column = settings_column.push(
        padded_control(divider::horizontal::default())
            .padding([0, spacing.space_s])
            .apply(Element::from),
    );

    // Last, so the toggle sits right above the tabs it hides.
    settings_column = settings_column.push(
        // Show visualizations toggle
        padded_control(
            row![
                text::body("Show Visualization"),
                Space::new().width(Length::Fill),
                toggler(config.ui.show_visualization).on_toggle(Message::SetShowVisualizations)
            ]
            .spacing(spacing.space_xs)
            .align_y(Alignment::Center),
        ),
    );

    // Visualization size slide (only show if the visualization is enabled)
    if config.ui.show_visualization {
        // Width slider
        settings_column = settings_column.push(
            column![
                padded_control(
                    column![
                        text::body("Visualization Size"),
                        row![
                            text::caption(format!("{}px", config.ui.applet_width)),
                            slider(60..=300, config.ui.applet_width, Message::SetAppletWidth)
                                .width(Length::Fill)
                        ]
                        .spacing(spacing.space_xs)
                        .align_y(Alignment::Center),
                    ]
                    .spacing(spacing.space_xxs)
                    .apply(Element::from)
                ),
                create_product_settings(params, tab),
            ]
            .spacing(spacing.space_xxs)
            .apply(Element::from),
        );
    }

    settings_column.apply(Element::from)
}

/// How `tab`'s activity is drawn, under a tab bar of the installed products.
/// With one product installed there is nothing to switch between, so it gets
/// no tab bar.
fn create_product_settings<'a>(
    params: &PopupContentParams<'a>,
    tab: Product,
) -> Element<'a, Message> {
    let spacing = theme::active().cosmic().spacing;
    let look = params.config.visualization.product(tab);
    let installed: Vec<Product> = params
        .links
        .iter()
        .filter(|l| l.installed)
        .map(|l| l.product)
        .collect();

    let mut settings = column![].spacing(spacing.space_xxs).width(Length::Fill);
    if installed.len() > 1 {
        settings = settings.push(padded_control(product_tabs(&installed, tab)));
    }
    settings
        .push(create_visualization_theme_selector(
            tab,
            &look.theme,
            params.is_open,
        ))
        .push(create_working_animation_selector(
            tab,
            look.working_animation,
            params.is_open,
        ))
        .push(create_visualization_color_selector(
            tab,
            &look.colors,
            params.is_open,
            params.theme_selector_model,
            params.selected_theme_for_config,
        ))
        .apply(Element::from)
}
