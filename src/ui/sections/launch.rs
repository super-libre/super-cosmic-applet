// SPDX-License-Identifier: GPL-3.0-only
use crate::app::{Message, ProductLink};
use cosmic::{
    Element,
    iced::widget::Row,
    theme,
    widget::{
        button,
        icon::{self, IconFallback},
        text, tooltip,
    },
};

/// A row with each installed product's full-color app icon, which opens its
/// settings app.
///
/// The icon comes from the icon theme, so it follows the product's own
/// artwork instead of a copy kept here. One that isn't installed shows the
/// generic application icon: the default fallback would try `super-tts`, then
/// `super`, and could land on an unrelated icon.
pub fn create_launch_section(links: &[ProductLink]) -> Element<'static, Message> {
    let spacing = theme::active().cosmic().spacing;
    links
        .iter()
        .filter(|l| l.installed)
        .fold(Row::new().spacing(spacing.space_xs), |row, link| {
            let product = link.product;
            let app_icon = icon::from_name(product.app_icon())
                .size(32)
                .fallback(Some(IconFallback::Names(vec![
                    "application-x-executable".into(),
                ])))
                .handle();
            // Above the icon: the row is the popup's last, with no room below.
            row.push(tooltip(
                button::icon(app_icon)
                    .medium()
                    .on_press(Message::LaunchApp(product)),
                text(format!("Open {}", product.display_name())),
                tooltip::Position::Top,
            ))
        })
        .into()
}

#[cfg(test)]
mod launch_tests {
    use super::create_launch_section;
    use crate::app::ProductLink;
    use crate::products::Product::{Stt, Tts};
    use crate::ui::simulator::{Simulator, every_click};

    #[test]
    fn each_installed_product_gets_an_icon_that_opens_its_app() {
        let links = [ProductLink::new(Stt, true), ProductLink::new(Tts, true)];
        assert_eq!(
            every_click(|| create_launch_section(&links)),
            ["LaunchApp(Stt)", "LaunchApp(Tts)"]
        );
    }

    #[test]
    fn a_product_that_isnt_installed_gets_no_icon() {
        let links = [ProductLink::new(Stt, false), ProductLink::new(Tts, true)];
        assert_eq!(
            every_click(|| create_launch_section(&links)),
            ["LaunchApp(Tts)"]
        );
    }

    /// The names are in tooltips, which only show on hover.
    #[test]
    fn the_row_is_icons_without_text() {
        let links = [ProductLink::new(Stt, true), ProductLink::new(Tts, true)];
        assert!(
            Simulator::new(create_launch_section(&links))
                .texts()
                .is_empty()
        );
    }
}
