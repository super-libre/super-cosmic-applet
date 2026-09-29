// SPDX-License-Identifier: GPL-3.0-only
use crate::{app::Message, app::ProductLink, models::state::DaemonConnectionState};
use cosmic::{
    Element,
    iced::widget::{Column, column},
    widget::{button, text},
};

/// One status line per installed product, or a note that neither is.
pub fn create_status_section(links: &[ProductLink]) -> Element<'static, Message> {
    let installed: Vec<&ProductLink> = links.iter().filter(|l| l.installed).collect();
    if installed.is_empty() {
        return column![
            text("Neither Super STT nor Super TTS is installed.").size(12),
            text("Install either one to use this applet.").size(10),
        ]
        .spacing(4)
        .into();
    }
    installed
        .into_iter()
        .fold(Column::new().spacing(10), |col, link| {
            col.push(product_status(link))
        })
        .into()
}

fn product_status(link: &ProductLink) -> Element<'static, Message> {
    let product = link.product;
    let name = product.display_name();
    match &link.daemon_state {
        DaemonConnectionState::Connected => text(format!("{name}: connected")).size(12).into(),
        DaemonConnectionState::Connecting => column![
            text(format!("{name}: connecting")).size(12),
            text("The daemon may still be starting.").size(10),
        ]
        .spacing(4)
        .into(),
        DaemonConnectionState::Error(e) => column![
            text(format!("{name}: {e}")).size(12),
            text("The daemon may still be starting.").size(10),
        ]
        .spacing(4)
        .into(),
        DaemonConnectionState::Blocked(reason) => column![
            text(format!("{name}: authorization denied")).size(12),
            text(format!("Reason: {reason}")).size(10),
            text("Restart the daemon to clear the denial:").size(10),
            text(format!(
                "  systemctl --user restart {}",
                product.spec().slug
            ))
            .size(10),
            text("Then ask for authorization again.").size(10),
            button::standard("Retry authorization").on_press(Message::Daemon(
                product,
                crate::app::DaemonMessage::RetryAuthorization
            )),
        ]
        .spacing(6)
        .into(),
    }
}
