// SPDX-License-Identifier: GPL-3.0-only
use crate::{
    app::{Message, ProductLink},
    config::AppletConfig,
    models::state::{DaemonConnectionState, Page},
    ui::{
        components::page_slide::{Direction, PageSlide},
        sections::{
            app_info::create_app_info_section,
            settings::{colors_page::create_colors_page, section::create_applet_settings_section},
            status::create_status_section,
        },
    },
};
use cosmic::{
    Apply, Element,
    applet::{menu_control_padding, padded_control},
    iced::widget::column,
    theme,
    widget::{divider, segmented_button::SingleSelectModel},
};

/// Parameters for creating popup content to avoid too many function arguments
pub struct PopupContentParams<'a> {
    pub links: &'a [ProductLink],
    pub config: &'a AppletConfig,
    pub icon_alignment_model: &'a SingleSelectModel,
    /// The popup this content is drawn in, which the dropdowns open their
    /// menus over.
    pub popup: Option<cosmic::iced::window::Id>,
}

/// The popup: the page `shown`, or while `slide` says the popup is moving
/// to it, the page it leaves sliding out as `shown` slides in.
pub fn create_popup<'a>(
    params: &PopupContentParams<'a>,
    shown: Page,
    slide: Option<(Page, f32)>,
) -> Element<'a, Message> {
    let page = |page: Page| match page {
        Page::Main => create_popup_content(params),
        Page::Colors => create_colors_page(params.config.visualization.palette),
    };
    match slide {
        Some((from, progress)) => PageSlide::new(
            page(from),
            page(shown),
            progress,
            if shown > from {
                Direction::Forward
            } else {
                Direction::Back
            },
        )
        .into(),
        None => page(shown),
    }
}

/// A line between two of the popup's sections, with as much room above it
/// as below.
pub fn section_divider<'a>() -> Element<'a, Message> {
    let spacing = theme::active().cosmic().spacing;
    padded_control(divider::horizontal::default())
        .padding([spacing.space_xs, spacing.space_s])
        .apply(Element::from)
}

/// The main page: the applet, with the apps it opens, then each product's
/// status and the settings.
fn create_popup_content<'a>(params: &PopupContentParams<'a>) -> Element<'a, Message> {
    let divider = section_divider;
    let any_connected = params
        .links
        .iter()
        .any(|l| l.installed && l.daemon_state == DaemonConnectionState::Connected);

    let mut content = column![
        padded_control(create_app_info_section(params.links))
            .padding(menu_control_padding())
            .apply(Element::from),
    ];
    if let Some(status) = create_status_section(params.links) {
        content = content.push(divider()).push(
            padded_control(status)
                .padding(menu_control_padding())
                .apply(Element::from),
        );
    }
    // The settings appear once either daemon is connected.
    if any_connected {
        content = content
            .push(divider())
            .push(create_applet_settings_section(params));
    }
    content.padding([8, 0]).into()
}

#[cfg(test)]
mod popup_tests {
    //! The popup as a whole. Each section has its own tests; these pin what
    //! the sections add up to.
    use crate::app::Message;
    use crate::app::testing::TestApplet;
    use crate::features::Feature::{Stt, Tts};
    use crate::models::state::Page;
    use crate::ui::simulator::{Simulator, every_click, popup_texts};

    #[test]
    fn with_nothing_installed_it_is_the_header_and_a_note() {
        let applet = TestApplet::new(&[]);
        assert_eq!(
            popup_texts(&applet),
            [
                "Super Applet".to_string(),
                format!("v{}", crate::VERSION),
                "Super Voice is not installed.".to_string(),
                "Install it to use this applet.".to_string(),
            ]
        );
        assert_eq!(
            every_click(|| applet.popup()),
            ["OpenGitHub"],
            "nothing to open, set or retry"
        );
    }

    /// The apps to open are in the header, beside the applet's name, and
    /// the popup ends with the last setting: no row of their own below.
    #[test]
    fn it_opens_with_the_header_and_its_apps_and_ends_with_the_settings() {
        let mut applet = TestApplet::new(&[Stt, Tts]);
        applet.connect(Stt).connect(Tts);

        assert_eq!(popup_texts(&applet)[0], "Super Applet");
        let clicks = every_click(|| applet.popup());
        assert_eq!(clicks[..2], ["LaunchApp", "OpenGitHub"]);
        assert_eq!(clicks.last().map(String::as_str), Some("ShowPage(Colors)"));
    }

    /// As a page slides in, the page it replaces is still there, sliding
    /// out: the Colors page comes in from the right, and the main page,
    /// back, from the left.
    #[test]
    fn a_page_further_in_comes_from_the_right_and_back_from_the_left() {
        const WIDTH: f32 = 360.0;
        let mut applet = TestApplet::new(&[Stt, Tts]);
        applet.connect(Stt).connect(Tts);

        let x_of =
            |applet: &TestApplet, text: &str| Simulator::new(applet.popup()).bounds_of(text).x;

        applet.send(Message::ShowPage(Page::Colors));
        assert!(x_of(&applet, "Show Icon") < WIDTH / 2.0);
        assert!(x_of(&applet, "Follows your system color") >= WIDTH);

        applet
            .show_page(Page::Colors)
            .send(Message::ShowPage(Page::Main));
        assert!(x_of(&applet, "Show Icon") < 0.0);
        assert!(x_of(&applet, "Follows your system color") < WIDTH / 2.0);
    }
}
