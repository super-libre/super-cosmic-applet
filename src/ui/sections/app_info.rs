// SPDX-License-Identifier: GPL-3.0-only
use crate::app::{Message, ProductLink};
use crate::ui::sections::launch::create_launch_button;
use cosmic::{
    Element,
    iced::{Alignment, Length, widget::row},
    theme,
    widget::{Space, button, icon, text},
};

// Cache GitHub icon bytes to avoid allocation on every render
static GITHUB_ICON_DARK: &[u8] =
    include_bytes!("../../../resources/assets/github-mark/github-mark-white.svg");
static GITHUB_ICON_LIGHT: &[u8] =
    include_bytes!("../../../resources/assets/github-mark/github-mark.svg");

/// The popup's header: the applet's name and version, then the buttons that
/// open Super Voice, when it's installed, and the applet's repository.
pub fn create_app_info_section(links: &[ProductLink]) -> Element<'static, Message> {
    let current_theme = theme::active();
    let spacing = current_theme.cosmic().spacing;

    // Check if dark theme is active
    let is_dark = current_theme.cosmic().is_dark;

    // Choose appropriate GitHub icon based on theme
    let github_icon = if is_dark {
        GITHUB_ICON_DARK
    } else {
        GITHUB_ICON_LIGHT
    };

    row![
        // Left side: the applet's name with its version
        row![
            text("Super Applet").size(16),
            text::caption(format!("v{}", crate::VERSION)),
        ]
        .spacing(spacing.space_xs)
        .align_y(Alignment::Center),
        // Spacer to push the buttons to the right
        Space::new().width(Length::Fill),
    ]
    .push_maybe(create_launch_button(links))
    .push(cosmic::widget::tooltip(
        button::icon(icon::from_svg_bytes(github_icon))
            .on_press(Message::OpenGitHub)
            .padding(4),
        "View on GitHub",
        cosmic::widget::tooltip::Position::Bottom,
    ))
    .spacing(spacing.space_xxs)
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}

#[cfg(test)]
mod app_info_tests {
    use super::create_app_info_section;
    use crate::app::ProductLink;
    use crate::features::Feature::{Stt, Tts};
    use crate::ui::simulator::{Simulator, every_click};

    fn installed(installed: bool) -> [ProductLink; 2] {
        [
            ProductLink::new(Stt, installed),
            ProductLink::new(Tts, installed),
        ]
    }

    #[test]
    fn the_header_names_the_applet_and_its_version() {
        let mut ui = Simulator::new(create_app_info_section(&installed(true)));
        assert_eq!(
            ui.texts(),
            ["Super Applet".to_string(), format!("v{}", crate::VERSION)]
        );
    }

    /// Super Voice's button sits beside the repository's, at the header's
    /// far end, rather than on a row of its own.
    #[test]
    fn its_buttons_open_super_voice_and_the_repository() {
        let links = installed(true);
        assert_eq!(
            every_click(|| create_app_info_section(&links)),
            ["LaunchApp", "OpenGitHub"]
        );
    }

    #[test]
    fn without_super_voice_only_the_repository_is_left() {
        let links = installed(false);
        assert_eq!(
            every_click(|| create_app_info_section(&links)),
            ["OpenGitHub"]
        );
    }
}
