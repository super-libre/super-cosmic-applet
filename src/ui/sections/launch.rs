// SPDX-License-Identifier: GPL-3.0-only
use crate::app::{Message, ProductLink};
use cosmic::{
    Element,
    widget::{
        button,
        icon::{self, IconFallback},
        text, tooltip,
    },
};

/// The settings app's full-color icon, which opens it. Only when either
/// feature is installed: one daemon serves both, and one app shows both.
///
/// The icon comes from the icon theme, so it follows the app's own artwork
/// instead of a copy kept here. An app that isn't installed shows the generic
/// application icon: the default fallback would try `super-voice`, then
/// `super`, and could land on an unrelated icon.
pub fn create_launch_button(links: &[ProductLink]) -> Option<Element<'static, Message>> {
    if !links.iter().any(|l| l.installed) {
        return None;
    }
    let app = crate::features::SETTINGS_APP;
    let app_icon = icon::from_name(app.icon)
        .size(24)
        .fallback(Some(IconFallback::Names(vec![
            "application-x-executable".into(),
        ])))
        .handle();
    // Sized and padded as the GitHub button beside it in the header.
    Some(
        tooltip(
            button::icon(app_icon)
                .on_press(Message::LaunchApp)
                .padding(4),
            text(format!("Open {}", app.name)),
            tooltip::Position::Bottom,
        )
        .into(),
    )
}

#[cfg(test)]
mod launch_tests {
    use super::create_launch_button;
    use crate::app::ProductLink;
    use crate::features::Feature::{Stt, Tts};
    use crate::ui::simulator::{Simulator, every_click};

    #[test]
    fn one_icon_opens_the_app_for_both_features() {
        let links = [ProductLink::new(Stt, true), ProductLink::new(Tts, true)];
        assert_eq!(
            every_click(|| create_launch_button(&links).expect("a button")),
            ["LaunchApp"]
        );
    }

    #[test]
    fn nothing_installed_gets_no_icon() {
        let links = [ProductLink::new(Stt, false), ProductLink::new(Tts, false)];
        assert!(create_launch_button(&links).is_none());
    }

    /// The name is in the tooltip, which only shows on hover.
    #[test]
    fn the_button_is_an_icon_without_text() {
        let links = [ProductLink::new(Stt, true), ProductLink::new(Tts, true)];
        assert_eq!(
            Simulator::new(create_launch_button(&links).expect("a button")).texts(),
            Vec::<String>::new()
        );
    }
}
