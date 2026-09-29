// SPDX-License-Identifier: GPL-3.0-only
use crate::app::{Message, ProductLink};
use cosmic::{
    Element,
    applet::menu_button,
    iced::{Length, widget::Column},
    widget::text,
};

/// A button that opens each installed product's settings app.
pub fn create_launch_section(links: &[ProductLink]) -> Element<'static, Message> {
    links
        .iter()
        .filter(|l| l.installed)
        .fold(Column::new().spacing(4), |col, link| {
            col.push(
                menu_button(text::body(format!("Open {}", link.product.display_name())))
                    .on_press(Message::LaunchApp(link.product))
                    .width(Length::Fill),
            )
        })
        .into()
}
