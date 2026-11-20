// SPDX-License-Identifier: GPL-3.0-only
use crate::{
    app::Message,
    features::Feature,
    models::palette::ColorPalette,
    models::state::Page,
    models::theme::{VisualizationTheme, WorkingAnimationTheme},
    ui::components::palette_swatch::{accent_choice, color_dot, palette_circle, palette_swatch},
};
use cosmic::{
    Apply, Element,
    applet::{menu_button, padded_control},
    iced::{
        Alignment, Length,
        widget::{Row, column, row},
        window,
    },
    theme,
    widget::{Space, container, dropdown::popup_dropdown, icon, text},
};

/// How many palettes a row of swatches holds: the twelve with colors of their
/// own fill two rows, under the accent's.
const SWATCHES_PER_ROW: usize = 6;

/// The styles, in the order the dropdown lists them.
const STYLES: [VisualizationTheme; 4] = [
    VisualizationTheme::Pulse,
    VisualizationTheme::BottomEqualizer,
    VisualizationTheme::CenteredEqualizer,
    VisualizationTheme::Waveform,
];

/// The loading animations, in the order the dropdown lists them.
const ANIMATIONS: [WorkingAnimationTheme; 3] = [
    WorkingAnimationTheme::Droplet,
    WorkingAnimationTheme::Comet,
    WorkingAnimationTheme::Dots,
];

/// The visualization's style, as a dropdown beside its name.
pub fn create_visualization_theme_selector<'a>(
    selected: &VisualizationTheme,
    popup: Option<window::Id>,
) -> Element<'a, Message> {
    let names: Vec<String> = STYLES.iter().map(VisualizationTheme::pretty_name).collect();
    setting_row(
        "Visualization Style",
        popup_dropdown(
            names,
            STYLES.iter().position(|style| style == selected),
            |i| Message::SetVisualizationTheme(STYLES[i].clone()),
            popup.unwrap_or(window::Id::NONE),
            Message::Surface,
            |message| message,
        ),
    )
}

/// The animation shown while a product transcribes or synthesizes, as a
/// dropdown beside its name.
pub fn create_working_animation_selector<'a>(
    selected: WorkingAnimationTheme,
    popup: Option<window::Id>,
) -> Element<'a, Message> {
    let names: Vec<String> = ANIMATIONS.iter().map(|a| a.pretty_name()).collect();
    setting_row(
        "Loading Animation",
        popup_dropdown(
            names,
            ANIMATIONS
                .iter()
                .position(|animation| *animation == selected),
            |i| Message::SetWorkingAnimation(ANIMATIONS[i]),
            popup.unwrap_or(window::Id::NONE),
            Message::Surface,
            |message| message,
        ),
    )
}

/// How much of its padding a dropdown's row gives up, top and bottom. A
/// dropdown pads its text by 8px all round, which leaves it 9px taller than
/// a toggle; taking 4px off each side brings its row to a toggle row's
/// height, so the settings keep an even pitch.
const DROPDOWN_ROW_INSET: u16 = 4;

/// A setting's name, with its control at the far end of the row, as the
/// toggles above are laid out. A dropdown opens its menu as a popup of its
/// own, over the applet's, so the list is never cut off by it.
fn setting_row<'a>(
    label: &'static str,
    control: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let spacing = theme::active().cosmic().spacing;
    padded_control(
        row![
            text::body(label),
            Space::new().width(Length::Fill),
            control.into(),
        ]
        .spacing(spacing.space_xs)
        .align_y(Alignment::Center),
    )
    .padding([
        spacing.space_xxs.saturating_sub(DROPDOWN_ROW_INSET),
        spacing.space_m,
    ])
    .apply(Element::from)
}

/// How wide the picked palette's circle is in the Colors row: as tall as a
/// toggle, so the row is as tall as the ones above it.
const ROW_CIRCLE: f32 = 24.0;

/// The Colors setting, on the main page: the picked palette's circle, and an
/// arrow saying the palettes are a page of their own.
pub fn create_colors_row<'a>(selected: ColorPalette) -> Element<'a, Message> {
    let current = theme::active();
    let cosmic_theme = current.cosmic();
    menu_button(
        row![
            text::body("Colors"),
            Space::new().width(Length::Fill),
            palette_circle(selected, ROW_CIRCLE, cosmic_theme),
            icon::from_name("go-next-symbolic").size(16).icon(),
        ]
        .spacing(cosmic_theme.spacing.space_xs)
        .align_y(Alignment::Center),
    )
    .on_press(Message::ShowPage(Page::Colors))
    .into()
}

/// The palettes, for the Colors page: a legend showing each product's
/// gradient in the picked palette, the accent's row, then a swatch for each
/// of the others.
pub fn create_palette_picker<'a>(selected: ColorPalette) -> Element<'a, Message> {
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

    padded_control(column![legend, accent, swatches].spacing(spacing.space_xs)).apply(Element::from)
}

#[cfg(test)]
mod selector_tests {
    //! The selectors, on a real applet: each dropdown opens its menu, the
    //! Colors row opens the Colors page, and what they pick is saved for
    //! both products.
    use crate::app::Message;
    use crate::app::testing::TestApplet;
    use crate::features::Feature::{Stt, Tts};
    use crate::models::palette::ColorPalette;
    use crate::models::state::Page;
    use crate::models::theme::{VisualizationTheme, WorkingAnimationTheme};
    use crate::ui::simulator::{Simulator, every_click, in_popup, popup_texts, where_to_send};
    use cosmic::iced::Point;

    /// Both products installed and connected.
    fn connected() -> TestApplet {
        let mut applet = TestApplet::new(&[Stt, Tts]);
        applet.connect(Stt).connect(Tts);
        applet
    }

    fn shows(applet: &TestApplet, text: &str) -> bool {
        popup_texts(applet).iter().any(|t| t == text)
    }

    /// Where to click the dropdown beside `label`: the row's far end, where
    /// the dropdown sits.
    fn dropdown_beside(applet: &TestApplet, label: &str) -> Point {
        let mut ui = Simulator::new(applet.popup());
        let name = ui.bounds_of(label);
        Point::new(300.0, name.center_y())
    }

    /// Each dropdown sits at the far end of its row, and opens its menu as
    /// a popup of its own, over the applet's: a surface for libcosmic to
    /// open, not a list inside the popup.
    #[test]
    fn each_dropdown_opens_its_menu_over_the_popup() {
        for label in ["Visualization Style", "Loading Animation"] {
            let mut applet = connected();
            let at = dropdown_beside(&applet, label);
            let sent = in_popup(&mut applet, |ui| {
                ui.click_at(at);
            });
            assert!(
                matches!(sent.as_slice(), [only] if only.starts_with("Surface(")),
                "{label}: sent {sent:?}"
            );
        }
    }

    /// A dropdown shows what's picked: a new style or animation redraws it.
    #[test]
    fn each_dropdown_shows_the_pick() {
        for pick in [
            Message::SetVisualizationTheme(VisualizationTheme::Waveform),
            Message::SetWorkingAnimation(WorkingAnimationTheme::Comet),
        ] {
            let mut applet = connected();
            let before = Simulator::new(applet.popup()).screenshot();
            applet.send(pick.clone());
            assert_ne!(
                Simulator::new(applet.popup()).screenshot(),
                before,
                "{pick:?} left the dropdowns as they were"
            );
        }
    }

    /// The dropdowns offer every style and every animation, each once.
    #[test]
    fn the_dropdowns_offer_every_choice_once() {
        for style in &super::STYLES {
            // A match without a wildcard: a new style fails to compile here
            // until it is placed in the list too.
            match style {
                VisualizationTheme::Pulse
                | VisualizationTheme::BottomEqualizer
                | VisualizationTheme::CenteredEqualizer
                | VisualizationTheme::Waveform => {}
            }
        }
        let names: std::collections::BTreeSet<String> = super::STYLES
            .iter()
            .map(VisualizationTheme::pretty_name)
            .collect();
        assert_eq!(names.len(), super::STYLES.len());
        for animation in super::ANIMATIONS {
            match animation {
                WorkingAnimationTheme::Droplet
                | WorkingAnimationTheme::Comet
                | WorkingAnimationTheme::Dots => {}
            }
        }
        let names: std::collections::BTreeSet<String> =
            super::ANIMATIONS.iter().map(|a| a.pretty_name()).collect();
        assert_eq!(names.len(), super::ANIMATIONS.len());
    }

    /// The Colors row opens the palettes' page, and shows the palette
    /// picked as a circle rather than by name.
    #[test]
    fn the_colors_row_opens_the_colors_page() {
        let mut applet = connected();
        assert!(!shows(&applet, "Sunset"), "the row shows no name");

        let sent = in_popup(&mut applet, |ui| {
            ui.click("Colors");
        });

        assert_eq!(sent, ["ShowPage(Colors)"]);
    }

    #[test]
    fn the_colors_row_shows_the_palette_picked() {
        let mut applet = connected();
        let before = Simulator::new(applet.popup()).screenshot();
        applet.send(Message::SetColorPalette(ColorPalette::Rose));
        assert_ne!(Simulator::new(applet.popup()).screenshot(), before);
    }

    /// The Colors page shows which half of a swatch is which product's, and
    /// a swatch for every palette.
    #[test]
    fn the_colors_page_offers_every_palette_once() {
        let mut applet = connected();
        applet.show_page(Page::Colors);
        assert!(
            shows(&applet, "Follows your system color"),
            "the accent row"
        );
        assert!(shows(&applet, "Speech to text") && shows(&applet, "Text to speech"));

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
        applet.show_page(Page::Colors);
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
        assert!(
            shows(&applet, "Follows your system color"),
            "still on the page"
        );
        assert_ne!(
            Simulator::new(applet.popup()).screenshot(),
            before,
            "the picked swatch looks the same as before"
        );
    }
}
