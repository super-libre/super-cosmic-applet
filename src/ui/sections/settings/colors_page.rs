// SPDX-License-Identifier: GPL-3.0-only
//! The Colors page: the way back to the main page, then every palette.
use cosmic::{
    Element,
    applet::menu_button,
    iced::{
        Alignment,
        widget::{column, row},
    },
    theme,
    widget::{icon, text},
};

use crate::{
    app::Message,
    models::{palette::ColorPalette, state::Page},
    ui::{
        sections::settings::components::visualization_theme::create_palette_picker,
        views::section_divider,
    },
};

pub fn create_colors_page<'a>(selected: ColorPalette) -> Element<'a, Message> {
    let spacing = theme::active().cosmic().spacing;
    // The page's name, as large as the applet's on the main page, behind an
    // arrow back to it.
    let back = menu_button(
        row![
            icon::from_name("go-previous-symbolic").size(16).icon(),
            text("Colors").size(16),
        ]
        .spacing(spacing.space_xs)
        .align_y(Alignment::Center),
    )
    .on_press(Message::ShowPage(Page::Main));

    column![back, section_divider(), create_palette_picker(selected)]
        .padding([8, 0])
        .into()
}

#[cfg(test)]
mod colors_page_tests {
    //! The Colors page, on a real applet.
    use crate::app::testing::TestApplet;
    use crate::features::Feature::{Stt, Tts};
    use crate::models::state::Page;
    use crate::ui::simulator::{in_popup, popup_texts};

    fn on_the_colors_page() -> TestApplet {
        let mut applet = TestApplet::new(&[Stt, Tts]);
        applet.connect(Stt).connect(Tts);
        applet.show_page(Page::Colors);
        applet
    }

    /// The page's name, then the palettes, and nothing of the main page.
    #[test]
    fn it_shows_only_the_colors() {
        let texts = popup_texts(&on_the_colors_page());
        assert_eq!(texts.first().map(String::as_str), Some("Colors"));
        assert!(texts.iter().any(|t| t == "Follows your system color"));
        for elsewhere in ["Super Applet", "Show Icon", "Visualization Style"] {
            assert!(!texts.iter().any(|t| t == elsewhere), "{elsewhere:?}");
        }
    }

    #[test]
    fn its_head_leads_back_to_the_main_page() {
        let mut applet = on_the_colors_page();
        let sent = in_popup(&mut applet, |ui| {
            ui.click("Colors");
        });
        assert_eq!(sent, ["ShowPage(Main)"]);
    }
}
