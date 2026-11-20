// SPDX-License-Identifier: GPL-3.0-only
use crate::{
    app::Message,
    features::Feature,
    models::palette::ColorPalette,
    models::state::IsOpen,
    models::theme::{VisualizationTheme, WorkingAnimationTheme},
    ui::components::{
        common::{revealer, revealer_head},
        palette_swatch::{accent_choice, color_dot, palette_swatch},
    },
};
use cosmic::{
    Apply, Element,
    applet::padded_control,
    iced::{
        Alignment, Length,
        widget::{Row, column, row},
    },
    theme,
    widget::{Space, container, text},
};

/// How many palettes a row of swatches holds: the twelve with colors of their
/// own fill two rows, under the accent's.
const SWATCHES_PER_ROW: usize = 6;

pub fn create_visualization_theme_selector<'a>(
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
        |theme_str| Message::SetVisualizationTheme(theme_str.parse().unwrap_or_default()),
    )
    .apply(Element::from)
}

/// Dropdown selector for the working animation. Mirrors
/// [`create_visualization_theme_selector`]; the chosen style drives the
/// animation shown while a product transcribes or synthesizes.
pub fn create_working_animation_selector<'a>(
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
        |anim_str| Message::SetWorkingAnimation(anim_str.parse().unwrap_or_default()),
    )
    .apply(Element::from)
}

/// The palettes, as rows of swatches under a legend showing each product's
/// gradient in the picked palette.
pub fn create_color_palette_selector<'a>(
    selected: ColorPalette,
    is_open: &IsOpen,
) -> Element<'a, Message> {
    let open = *is_open == IsOpen::VisualizationColors;
    let head = revealer_head(
        open,
        "Colors".to_string(),
        selected.pretty_name().to_string(),
        Message::RevealerToggle(IsOpen::VisualizationColors),
    );
    if !open {
        return column![head].apply(Element::from);
    }

    let current = theme::active();
    let cosmic_theme = current.cosmic();
    let spacing = cosmic_theme.spacing;

    let legend_item = |product: Feature| {
        row![
            color_dot(selected, product, cosmic_theme),
            text::caption(product.display_name()),
        ]
        .spacing(spacing.space_xxs)
        .align_y(Alignment::Center)
    };
    let legend = row![
        legend_item(Feature::Stt),
        Space::new().width(Length::Fill),
        legend_item(Feature::Tts),
    ]
    .align_y(Alignment::Center);

    // The accent pair leads, on a row of its own: it takes the system's
    // color where the others have colors of their own.
    let [_accent, pairs @ ..] = ColorPalette::ALL;
    let accent = accent_choice(selected == ColorPalette::Accent, cosmic_theme);

    // Equal cells, so the columns line up and a short last row keeps them.
    let swatches = pairs.chunks(SWATCHES_PER_ROW).fold(
        column![].spacing(spacing.space_xxs),
        |rows, palettes| {
            let row =
                (0..SWATCHES_PER_ROW).fold(Row::new().align_y(Alignment::Center), |row, i| {
                    match palettes.get(i) {
                        Some(palette) => row.push(
                            container(palette_swatch(*palette, *palette == selected, cosmic_theme))
                                .width(Length::Fill)
                                .align_x(Alignment::Center),
                        ),
                        None => row.push(Space::new().width(Length::Fill)),
                    }
                });
            rows.push(row)
        },
    );

    column![
        head,
        padded_control(column![legend, accent, swatches].spacing(spacing.space_xs))
            .padding([8, 48]),
    ]
    .width(Length::Fill)
    .apply(Element::from)
}

#[cfg(test)]
mod selector_tests {
    //! The selectors, on a real applet: each opens its list, and what it
    //! picks is saved for both products.
    use crate::app::testing::TestApplet;
    use crate::features::Feature::{Stt, Tts};
    use crate::models::palette::ColorPalette;
    use crate::models::theme::{VisualizationTheme, WorkingAnimationTheme};
    use crate::ui::simulator::{Simulator, every_click, in_popup, popup_texts, where_to_send};

    /// Both products installed and connected.
    fn connected() -> TestApplet {
        let mut applet = TestApplet::new(&[Stt, Tts]);
        applet.connect(Stt).connect(Tts);
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

    #[test]
    fn the_style_list_sets_the_style() {
        let mut applet = connected();
        assert!(!shows(&applet, "Pulse"), "the list starts closed");

        open(&mut applet, "Visualization Style");
        for option in ["Pulse", "Equalizer", "Waveform"] {
            assert!(shows(&applet, option), "{option:?} missing");
        }
        let sent = in_popup(&mut applet, |ui| {
            ui.click("Waveform");
        });

        assert_eq!(sent, ["SetVisualizationTheme(Waveform)"]);
        assert_eq!(
            applet.config().visualization.theme,
            VisualizationTheme::Waveform
        );
        assert_eq!(
            applet.saved().expect("saved").visualization.theme,
            VisualizationTheme::Waveform
        );
        assert!(!shows(&applet, "Pulse"), "picking closes the list");
        assert!(shows(&applet, "Waveform"), "the head shows the pick");
    }

    #[test]
    fn the_loading_animation_list_sets_the_animation() {
        let mut applet = connected();

        open(&mut applet, "Loading Animation");
        let sent = in_popup(&mut applet, |ui| {
            ui.click("Comet");
        });

        assert_eq!(sent, ["SetWorkingAnimation(Comet)"]);
        assert_eq!(
            applet.config().visualization.working_animation,
            WorkingAnimationTheme::Comet
        );
    }

    #[test]
    fn a_list_closes_from_its_head_or_when_another_opens() {
        let mut applet = connected();
        open(&mut applet, "Visualization Style");
        open(&mut applet, "Visualization Style");
        assert!(!shows(&applet, "Pulse"), "its head closes it");

        open(&mut applet, "Visualization Style");
        open(&mut applet, "Loading Animation");
        assert!(shows(&applet, "Comet"));
        assert!(!shows(&applet, "Pulse"), "opening another closes it");
    }

    /// The head names the palette in use, and opening it shows which half
    /// of a swatch is which product's, and a swatch for every palette.
    #[test]
    fn the_colors_offer_every_palette_once() {
        let mut applet = connected();
        assert!(shows(&applet, "Sunset"), "the head names the default");
        // The status section names both products too; the legend adds one
        // more of each.
        let count = |applet: &TestApplet, text: &str| {
            popup_texts(applet).iter().filter(|t| *t == text).count()
        };
        let closed = [
            count(&applet, "Speech to text"),
            count(&applet, "Text to speech"),
        ];

        open(&mut applet, "Colors");
        assert!(
            shows(&applet, "Follows your system color"),
            "the accent row"
        );
        assert_eq!(count(&applet, "Speech to text"), closed[0] + 1);
        assert_eq!(count(&applet, "Text to speech"), closed[1] + 1);

        let palettes: Vec<String> = every_click(|| applet.popup())
            .into_iter()
            .filter(|sent| sent.starts_with("SetColorPalette("))
            .collect();
        let offered: Vec<String> = ColorPalette::ALL
            .iter()
            .map(|p| format!("SetColorPalette({p:?})"))
            .collect();
        assert_eq!(palettes, offered);
    }

    #[test]
    fn a_picked_palette_is_saved_and_drawn_picked() {
        let mut applet = connected();
        open(&mut applet, "Colors");
        let before = Simulator::new(applet.popup()).screenshot();

        let swatch =
            where_to_send(|| applet.popup(), "SetColorPalette(Rose)").expect("a Rosé swatch");
        in_popup(&mut applet, |ui| {
            ui.click_at(swatch);
        });

        assert_eq!(applet.config().visualization.palette, ColorPalette::Rose);
        assert_eq!(
            applet.saved().expect("saved").visualization.palette,
            ColorPalette::Rose
        );
        assert!(shows(&applet, "Rosé"), "the head names the pick");
        assert!(shows(&applet, "Speech to text"), "the swatches stay open");
        assert_ne!(
            Simulator::new(applet.popup()).screenshot(),
            before,
            "the picked swatch looks the same as before"
        );
    }
}
