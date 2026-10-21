// SPDX-License-Identifier: GPL-3.0-only
use crate::{
    app::{Message, ProductLink},
    config::AppletConfig,
    models::state::{DaemonConnectionState, IsOpen},
    products::Product,
    ui::sections::{
        app_info::create_app_info_section, launch::create_launch_section,
        settings::section::create_applet_settings_section, status::create_status_section,
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
    pub is_open: &'a IsOpen,
    pub config: &'a AppletConfig,
    pub icon_alignment_model: &'a SingleSelectModel,
    pub theme_selector_model: &'a SingleSelectModel,
    pub selected_theme_for_config: bool,
    /// The product whose settings the tabs show, if any is installed.
    pub settings_tab: Option<Product>,
}

pub fn create_popup_content<'a>(params: &PopupContentParams<'a>) -> Element<'a, Message> {
    let spacing = theme::active().cosmic().spacing;
    let divider = || {
        padded_control(divider::horizontal::default())
            .padding([spacing.space_xs, spacing.space_s])
            .apply(Element::from)
    };
    let any_connected = params
        .links
        .iter()
        .any(|l| l.installed && l.daemon_state == DaemonConnectionState::Connected);

    let mut content = column![
        padded_control(create_app_info_section())
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
    // The settings appear once either daemon is connected, with a tab for
    // each installed product's own.
    if let Some(tab) = params.settings_tab.filter(|_| any_connected) {
        content = content
            .push(divider())
            .push(create_applet_settings_section(params, tab));
    }
    content
        .push(divider())
        .push(
            padded_control(create_launch_section(params.links))
                .padding(menu_control_padding())
                .apply(Element::from),
        )
        .padding([8, 0])
        .into()
}

#[cfg(test)]
mod popup_tests {
    //! The popup as a whole. Each section has its own tests; these pin what
    //! the sections add up to.
    use crate::app::testing::TestApplet;
    use crate::products::Product::{Stt, Tts};
    use crate::ui::simulator::{every_click, popup_texts};

    #[test]
    fn with_nothing_installed_it_is_the_header_and_a_note() {
        let applet = TestApplet::new(&[]);
        assert_eq!(
            popup_texts(&applet),
            [
                "Super Applet".to_string(),
                format!("v{}", crate::VERSION),
                "Neither Super STT nor Super TTS is installed.".to_string(),
                "Install either one to use this applet.".to_string(),
            ]
        );
        assert_eq!(
            every_click(|| applet.popup()),
            ["OpenGitHub"],
            "nothing to open, set or retry"
        );
    }

    #[test]
    fn it_opens_with_the_header_and_ends_with_the_apps() {
        let mut applet = TestApplet::new(&[Stt, Tts]);
        applet.connect(Stt).connect(Tts);

        assert_eq!(popup_texts(&applet)[0], "Super Applet");
        let clicks = every_click(|| applet.popup());
        assert_eq!(clicks.first().map(String::as_str), Some("OpenGitHub"));
        assert_eq!(
            clicks[clicks.len() - 2..],
            ["LaunchApp(Stt)", "LaunchApp(Tts)"]
        );
    }
}
