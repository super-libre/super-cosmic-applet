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

#[cfg(test)]
mod settings_section_tests {
    //! The settings part of the popup, on a real applet: what shows when, and
    //! that each control's click reaches the saved settings.
    use crate::app::testing::TestApplet;
    use crate::models::theme::IconAlignment;
    use crate::products::Product::{Stt, Tts};
    use crate::ui::simulator::{in_popup, popup_texts};
    use cosmic::iced::Point;

    /// An applet with both products installed and connected.
    fn connected() -> TestApplet {
        let mut applet = TestApplet::new(&[Stt, Tts]);
        applet.connect(Stt).connect(Tts);
        applet
    }

    fn shows(applet: &TestApplet, text: &str) -> bool {
        popup_texts(applet).iter().any(|t| t == text)
    }

    #[test]
    fn the_settings_wait_for_a_connected_daemon() {
        let mut applet = TestApplet::new(&[Stt, Tts]);
        assert!(!shows(&applet, "Show Icon"));

        applet.connect(Tts);

        assert!(shows(&applet, "Show Icon"));
    }

    #[test]
    fn the_product_tabs_show_only_with_both_installed() {
        let mut one = TestApplet::new(&[Stt]);
        one.connect(Stt);
        let texts = popup_texts(&one);
        assert!(!texts.iter().any(|t| t == "Super STT"), "{texts:?}");
        assert!(
            texts.iter().any(|t| t == "Visualization Style"),
            "{texts:?}"
        );

        let texts = popup_texts(&connected());
        assert!(texts.iter().any(|t| t == "Super STT"), "{texts:?}");
        assert!(texts.iter().any(|t| t == "Super TTS"), "{texts:?}");
    }

    #[test]
    fn the_shared_settings_come_before_the_tabs() {
        let texts = popup_texts(&connected());
        let at = |label: &str| texts.iter().position(|t| t == label).unwrap();
        assert!(at("Show Icon") < at("Icon Position"));
        assert!(at("Icon Position") < at("Show Visualization"));
        assert!(at("Visualization Size") < at("Super STT"));
        assert!(at("Super TTS") < at("Visualization Style"));
    }

    #[test]
    fn turning_the_icon_off_saves_it_and_hides_its_position() {
        let mut applet = connected();

        let sent = in_popup(&mut applet, |ui| {
            ui.click_toggler_of("Show Icon");
        });

        assert_eq!(sent, ["SetShowIcon(false)"]);
        assert!(!applet.config().ui.show_icon);
        assert!(!applet.saved().expect("saved").ui.show_icon);
        assert!(!shows(&applet, "Icon Position"));
    }

    #[test]
    fn turning_the_visualization_off_hides_its_settings_and_the_tabs() {
        let mut applet = connected();

        let sent = in_popup(&mut applet, |ui| {
            ui.click_toggler_of("Show Visualization");
        });

        assert_eq!(sent, ["SetShowVisualizations(false)"]);
        let texts = popup_texts(&applet);
        for hidden in ["Visualization Size", "Super STT", "Visualization Style"] {
            assert!(
                !texts.iter().any(|t| t == hidden),
                "{hidden:?} in {texts:?}"
            );
        }
        assert!(texts.iter().any(|t| t == "Show Icon"));
    }

    #[test]
    fn the_icon_position_control_sets_where_the_icon_sits() {
        let mut applet = connected();
        for (fraction, alignment) in [
            (1.0 / 6.0, IconAlignment::Start),
            (0.5, IconAlignment::Center),
            (5.0 / 6.0, IconAlignment::End),
        ] {
            in_popup(&mut applet, |ui| {
                let control = ui.control_below("Icon Position");
                ui.click_at(Point::new(
                    control.x + control.width * fraction,
                    control.center_y(),
                ));
            });
            assert_eq!(applet.config().ui.icon_alignment, alignment);
            assert_eq!(applet.saved().expect("saved").ui.icon_alignment, alignment);
        }
    }

    #[test]
    fn the_size_slider_sets_the_visualization_size() {
        let mut applet = connected();

        in_popup(&mut applet, |ui| {
            let slider = ui.control_below("Visualization Size");
            ui.click_at(Point::new(slider.x + slider.width - 1.0, slider.center_y()));
        });

        let width = applet.config().ui.applet_width;
        assert!(width > 250, "the far end of the slider set {width}px");
        assert_eq!(applet.saved().expect("saved").ui.applet_width, width);
        assert!(shows(&applet, &format!("{width}px")));
    }
}
