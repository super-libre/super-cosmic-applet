// SPDX-License-Identifier: GPL-3.0-only
use crate::{
    app::Message,
    models::state::IsOpen,
    models::theme::{
        VisualizationColor, VisualizationColorConfig, VisualizationTheme, WorkingAnimationTheme,
    },
    products::Product,
    ui::components::{
        color_buttons::{create_color_button, create_system_accent_button},
        common::{revealer, revealer_head},
    },
};
use cosmic::{
    Apply, Element, Theme,
    applet::padded_control,
    iced::{
        Length,
        widget::{Row, column, row},
    },
    theme,
    widget::{Space, segmented_button::SingleSelectModel, segmented_control, text},
};

pub fn create_visualization_theme_selector<'a>(
    product: Product,
    selected_theme: &VisualizationTheme,
    is_open: &IsOpen,
) -> Element<'a, Message> {
    let vis_themes = vec![
        VisualizationTheme::Pulse,
        VisualizationTheme::BottomEqualizer,
        VisualizationTheme::CenteredEqualizer,
        VisualizationTheme::Waveform,
    ];

    let options: Vec<(String, String)> = vis_themes
        .into_iter()
        .map(|theme| (theme.to_string(), theme.pretty_name()))
        .collect();

    revealer(
        *is_open == IsOpen::VisualizationTheme,
        "Visualization Style".to_string(),
        selected_theme.pretty_name(),
        &options,
        Message::RevealerToggle(IsOpen::VisualizationTheme),
        move |theme_str| {
            Message::SetVisualizationTheme(product, theme_str.parse().unwrap_or_default())
        },
    )
    .apply(Element::from)
}

/// Dropdown selector for the working animation. Mirrors
/// [`create_visualization_theme_selector`]; the chosen style drives the
/// animation shown while `product` transcribes or synthesizes.
pub fn create_working_animation_selector<'a>(
    product: Product,
    selected: WorkingAnimationTheme,
    is_open: &IsOpen,
) -> Element<'a, Message> {
    let anims = vec![
        WorkingAnimationTheme::Droplet,
        WorkingAnimationTheme::Comet,
        WorkingAnimationTheme::Dots,
    ];

    let options: Vec<(String, String)> = anims
        .into_iter()
        .map(|anim| (anim.to_string(), anim.pretty_name()))
        .collect();

    revealer(
        *is_open == IsOpen::WorkingAnimation,
        "Loading Animation".to_string(),
        selected.pretty_name(),
        &options,
        Message::RevealerToggle(IsOpen::WorkingAnimation),
        move |anim_str| Message::SetWorkingAnimation(product, anim_str.parse().unwrap_or_default()),
    )
    .apply(Element::from)
}

pub fn create_visualization_color_selector<'a>(
    product: Product,
    color_config: &VisualizationColorConfig,
    is_open: &IsOpen,
    theme_selector_model: &'a SingleSelectModel,
    selected_theme_for_config: bool,
) -> Element<'a, Message> {
    let spacing = theme::active().cosmic().spacing;

    if *is_open == IsOpen::VisualizationColors {
        column![
            revealer_head(
                true,
                "Visualization Color".to_string(),
                "Theme-Aware Color".to_string(),
                Message::RevealerToggle(IsOpen::VisualizationColors)
            ),
            // Theme selector
            padded_control(
                column![
                    text::caption("Configure Colors For:"),
                    segmented_control::horizontal(theme_selector_model)
                        .on_activate(Message::SetColorThemeEntity)
                ]
                .spacing(spacing.space_xxs)
                .apply(Element::from)
            )
            .padding([8, 48]),
            // Theme-specific color controls
            create_theme_color_section(
                product,
                selected_theme_for_config,
                &color_config.get_color(selected_theme_for_config)
            ),
        ]
        .width(Length::Fill)
        .apply(Element::from)
    } else {
        column![revealer_head(
            false,
            "Visualization Color".to_string(),
            "Theme-Aware Color".to_string(),
            Message::RevealerToggle(IsOpen::VisualizationColors)
        )]
        .apply(Element::from)
    }
}

fn create_colors_row<'a>(
    product: Product,
    colors: &[VisualizationColor],
    selected_theme_is_dark: bool,
    current_selected_color: &VisualizationColor,
) -> Row<'a, Message, Theme> {
    let mut light_colors_row = row![];
    for (index, color) in colors.iter().enumerate() {
        light_colors_row = light_colors_row.push(create_color_button(
            color.to_color(),
            color,
            Message::SetVisualizationColor(product, color.clone(), selected_theme_is_dark),
            20.0,
            current_selected_color == color,
        ));
        if index != colors.len() - 1 {
            light_colors_row = light_colors_row.push(Space::new().width(Length::Fill));
        }
    }
    light_colors_row
}

fn create_theme_color_section<'a>(
    product: Product,
    selected_theme_is_dark: bool,
    current_selected_color: &VisualizationColor,
) -> Element<'a, Message> {
    let current_theme = theme::active();
    let spacing = current_theme.cosmic().spacing;
    let current_cosmic_theme = current_theme.cosmic();

    // Light colors that work well in light themes
    let light_colors = vec![
        VisualizationColor::White,
        VisualizationColor::Gray,
        VisualizationColor::Blue,
        VisualizationColor::Green,
        VisualizationColor::Orange,
        VisualizationColor::Purple,
        VisualizationColor::Violet,
        VisualizationColor::Red,
        VisualizationColor::Cyan,
        VisualizationColor::Pink,
    ];

    // Darker/deeper colors that work well in dark themes
    let dark_colors = vec![
        VisualizationColor::Black,
        VisualizationColor::DarkGray,
        VisualizationColor::DarkBlue,
        VisualizationColor::DarkGreen,
        VisualizationColor::DarkOrange,
        VisualizationColor::DarkPurple,
        VisualizationColor::DarkViolet,
        VisualizationColor::DarkRed,
        VisualizationColor::DarkCyan,
        VisualizationColor::DarkPink,
    ];

    let pastel_colors = vec![
        VisualizationColor::PastelBlue,
        VisualizationColor::PastelGreen,
        VisualizationColor::PastelOrange,
        VisualizationColor::PastelPurple,
        VisualizationColor::PastelRed,
        VisualizationColor::PastelCyan,
        VisualizationColor::PastelPink,
        VisualizationColor::PastelYellow,
        VisualizationColor::PastelMagenta,
        VisualizationColor::PastelLavender,
    ];

    padded_control(
        column![
            // Color grid section label
            text::caption("Default Colors").width(Length::Fill),
            // Dedicated System Accent row - full width button
            create_system_accent_button(
                product,
                selected_theme_is_dark,
                current_cosmic_theme,
                current_selected_color
            ),
            // Color grid section label
            text::caption("Custom Colors").width(Length::Fill),
            // Top row of colors
            create_colors_row(
                product,
                &light_colors,
                selected_theme_is_dark,
                current_selected_color
            )
            .spacing(0u16),
            // Row of dark colors
            create_colors_row(
                product,
                &dark_colors,
                selected_theme_is_dark,
                current_selected_color
            ),
            // Row of pastel colors
            create_colors_row(
                product,
                &pastel_colors,
                selected_theme_is_dark,
                current_selected_color
            )
            .spacing(0u16)
        ]
        .spacing(spacing.space_xxs),
    )
    .padding([8, 48])
    .apply(Element::from)
}

#[cfg(test)]
mod selector_tests {
    //! The per-product selectors, on a real applet: each opens its list, and
    //! sets the product whose tab is showing, never the other one.
    use crate::app::{Message, testing::TestApplet};
    use crate::models::theme::{VisualizationColor, VisualizationTheme, WorkingAnimationTheme};
    use crate::products::Product::{self, Stt, Tts};
    use crate::ui::simulator::{Simulator, every_click, in_popup, popup_texts, where_to_send};
    use cosmic::iced::Point;

    /// Both products installed and connected, with `tab`'s settings showing.
    fn on_tab(tab: Product) -> TestApplet {
        let mut applet = TestApplet::new(&[Stt, Tts]);
        applet.connect(Stt).connect(Tts);
        applet.send(Message::SelectSettingsTab(tab));
        applet
    }

    fn shows(applet: &TestApplet, text: &str) -> bool {
        popup_texts(applet).iter().any(|t| t == text)
    }

    fn open(applet: &mut TestApplet, list: &str) {
        in_popup(applet, |ui| {
            ui.click(list);
        });
    }

    /// Pick the light (`dark` false) or dark half of "Configure Colors For:".
    fn configure_for(applet: &mut TestApplet, dark: bool) {
        in_popup(applet, |ui| {
            let control = ui.control_below("Configure Colors For:");
            let fraction = if dark { 0.75 } else { 0.25 };
            ui.click_at(Point::new(
                control.x + control.width * fraction,
                control.center_y(),
            ));
        });
    }

    #[test]
    fn the_style_list_sets_the_tabs_product_only() {
        let mut applet = on_tab(Tts);
        assert!(!shows(&applet, "Pulse"), "the list starts closed");

        open(&mut applet, "Visualization Style");
        for option in ["Pulse", "Equalizer", "Waveform"] {
            assert!(shows(&applet, option), "{option:?} missing");
        }
        let sent = in_popup(&mut applet, |ui| {
            ui.click("Waveform");
        });

        assert_eq!(sent, ["SetVisualizationTheme(Tts, Waveform)"]);
        let visualization = &applet.config().visualization;
        assert_eq!(visualization.tts.theme, VisualizationTheme::Waveform);
        assert_eq!(visualization.stt.theme, VisualizationTheme::default());
        assert_eq!(
            applet.saved().expect("saved").visualization.tts.theme,
            VisualizationTheme::Waveform
        );
        assert!(!shows(&applet, "Pulse"), "picking closes the list");
        assert!(shows(&applet, "Waveform"), "the head shows the pick");
    }

    #[test]
    fn the_loading_animation_list_sets_the_tabs_product_only() {
        let mut applet = on_tab(Stt);

        open(&mut applet, "Loading Animation");
        let sent = in_popup(&mut applet, |ui| {
            ui.click("Comet");
        });

        assert_eq!(sent, ["SetWorkingAnimation(Stt, Comet)"]);
        let visualization = &applet.config().visualization;
        assert_eq!(
            visualization.stt.working_animation,
            WorkingAnimationTheme::Comet
        );
        assert_eq!(
            visualization.tts.working_animation,
            WorkingAnimationTheme::default()
        );
    }

    #[test]
    fn a_list_closes_from_its_head_or_when_another_opens() {
        let mut applet = on_tab(Stt);
        open(&mut applet, "Visualization Style");
        open(&mut applet, "Visualization Style");
        assert!(!shows(&applet, "Pulse"), "its head closes it");

        open(&mut applet, "Visualization Style");
        open(&mut applet, "Loading Animation");
        assert!(shows(&applet, "Comet"));
        assert!(!shows(&applet, "Pulse"), "opening another closes it");
    }

    /// The swatches and the accent button all set the product on show, for
    /// whichever theme is being configured.
    #[test]
    fn every_color_sets_the_tabs_product() {
        let mut applet = on_tab(Tts);
        open(&mut applet, "Visualization Color");
        for text in [
            "Configure Colors For:",
            "Default Colors",
            "System accent color",
            "Custom Colors",
        ] {
            assert!(shows(&applet, text), "{text:?} missing");
        }

        let colors: Vec<String> = every_click(|| applet.popup())
            .into_iter()
            .filter(|sent| sent.starts_with("SetVisualizationColor("))
            .collect();

        assert_eq!(colors.len(), 31, "the accent and 30 swatches: {colors:?}");
        assert!(
            colors
                .iter()
                .all(|sent| sent.starts_with("SetVisualizationColor(Tts, ")),
            "{colors:?}"
        );
    }

    #[test]
    fn the_light_or_dark_choice_picks_the_theme_a_color_sets() {
        let mut applet = on_tab(Stt);
        open(&mut applet, "Visualization Color");
        for dark in [false, true] {
            configure_for(&mut applet, dark);
            let sent = in_popup(&mut applet, |ui| {
                ui.click("System accent color");
            });
            assert_eq!(
                sent,
                [format!("SetVisualizationColor(Stt, SystemAccent, {dark})")]
            );
        }
    }

    #[test]
    fn a_picked_swatch_is_saved_for_that_theme_and_drawn_picked() {
        let mut applet = on_tab(Stt);
        open(&mut applet, "Visualization Color");
        configure_for(&mut applet, true);
        let before = Simulator::new(applet.popup()).screenshot();

        let swatch = where_to_send(|| applet.popup(), "SetVisualizationColor(Stt, Blue, true)")
            .expect("a Blue swatch");
        in_popup(&mut applet, |ui| {
            ui.click_at(swatch);
        });

        let colors = &applet.config().visualization.stt.colors;
        assert_eq!(colors.dark_colors, VisualizationColor::Blue);
        assert_eq!(colors.light_colors, VisualizationColor::SystemAccent);
        assert_eq!(
            applet.config().visualization.tts.colors.dark_colors,
            VisualizationColor::SystemAccent
        );
        assert_eq!(
            applet
                .saved()
                .expect("saved")
                .visualization
                .stt
                .colors
                .dark_colors,
            VisualizationColor::Blue
        );
        assert_ne!(
            Simulator::new(applet.popup()).screenshot(),
            before,
            "the picked swatch looks the same as before"
        );
    }
}
