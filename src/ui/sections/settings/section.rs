// SPDX-License-Identifier: GPL-3.0-only
use cosmic::{
    Apply, Element,
    applet::padded_control,
    iced::{
        Alignment, Length,
        widget::{column, row, slider},
    },
    theme,
    widget::{Space, segmented_control, text, toggler},
};

use crate::{
    app::Message,
    ui::{
        sections::settings::components::visualization_theme::{
            create_colors_row, create_visualization_theme_selector,
            create_working_animation_selector,
        },
        views::{PopupContentParams, section_divider},
    },
};

/// The applet's settings: the icon, then the visualization. Both products
/// draw in the same style and animation; the palette gives each its color.
pub fn create_applet_settings_section<'a>(params: &PopupContentParams<'a>) -> Element<'a, Message> {
    let PopupContentParams {
        config,
        icon_alignment_model,
        ..
    } = *params;
    let spacing = theme::active().cosmic().spacing;

    // The settings stack with no gap of their own: each row's padding is
    // the space between two of them, as in COSMIC's own applets.
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

    settings_column = settings_column.push(section_divider());

    // Last, so the toggle sits right above the settings it hides.
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
        settings_column = settings_column
            .push(padded_control(
                column![
                    text::body("Visualization Size"),
                    row![
                        text::caption(format!("{}px", config.ui.applet_width)),
                        slider(60..=300, config.ui.applet_width, Message::SetAppletWidth)
                            .on_release(Message::AppletWidthSet)
                            .width(Length::Fill)
                    ]
                    .spacing(spacing.space_xs)
                    .align_y(Alignment::Center),
                ]
                .spacing(spacing.space_xxs),
            ))
            .push(create_visualization_theme_selector(
                &config.visualization.theme,
                params.popup,
            ))
            .push(create_working_animation_selector(
                config.visualization.working_animation,
                params.popup,
            ))
            .push(create_colors_row(config.visualization.palette));
    }

    settings_column.apply(Element::from)
}

#[cfg(test)]
mod settings_section_tests {
    //! The settings part of the popup, on a real applet: what shows when, and
    //! that each control's click reaches the saved settings.
    use crate::app::testing::TestApplet;
    use crate::features::Feature::{Stt, Tts};
    use crate::models::theme::IconAlignment;
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

    /// One set of visualization settings, whatever is installed: the
    /// palette is what sets the products apart.
    #[test]
    fn there_are_no_product_tabs() {
        for installed in [&[Stt][..], &[Tts], &[Stt, Tts]] {
            let mut applet = TestApplet::new(installed);
            applet.connect(installed[0]);
            let texts = popup_texts(&applet);
            for label in ["Visualization Style", "Loading Animation", "Colors"] {
                assert_eq!(
                    texts.iter().filter(|t| *t == label).count(),
                    1,
                    "{label:?} with {installed:?}: {texts:?}"
                );
            }
        }
    }

    #[test]
    fn the_settings_run_from_the_icon_to_the_colors() {
        let texts = popup_texts(&connected());
        let at = |label: &str| texts.iter().position(|t| t == label).unwrap();
        assert!(at("Show Icon") < at("Icon Position"));
        assert!(at("Icon Position") < at("Show Visualization"));
        assert!(at("Visualization Size") < at("Visualization Style"));
        assert!(at("Visualization Style") < at("Loading Animation"));
        assert!(at("Loading Animation") < at("Colors"));
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
    fn turning_the_visualization_off_hides_its_settings() {
        let mut applet = connected();

        let sent = in_popup(&mut applet, |ui| {
            ui.click_toggler_of("Show Visualization");
        });

        assert_eq!(sent, ["SetShowVisualizations(false)"]);
        let texts = popup_texts(&applet);
        for hidden in [
            "Visualization Size",
            "Visualization Style",
            "Loading Animation",
            "Colors",
        ] {
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
