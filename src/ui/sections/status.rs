// SPDX-License-Identifier: GPL-3.0-only
use crate::{app::Message, app::ProductLink, models::state::DaemonConnectionState};
use cosmic::{
    Element,
    iced::widget::{Column, column},
    widget::{button, text},
};

/// A status line for each installed product that isn't connected, or a note
/// that neither is installed. A connected product needs no line: its settings
/// tab and its launch button already show it is there. `None` once every
/// installed product is connected.
pub fn create_status_section(links: &[ProductLink]) -> Option<Element<'static, Message>> {
    if !links.iter().any(|l| l.installed) {
        return Some(
            column![
                text("Neither Super STT nor Super TTS is installed.").size(12),
                text("Install either one to use this applet.").size(10),
            ]
            .spacing(4)
            .into(),
        );
    }
    let statuses: Vec<Element<'static, Message>> = links
        .iter()
        .filter(|l| l.installed)
        .filter_map(product_status)
        .collect();
    (!statuses.is_empty()).then(|| Column::with_children(statuses).spacing(10).into())
}

fn product_status(link: &ProductLink) -> Option<Element<'static, Message>> {
    let product = link.product;
    let name = product.display_name();
    Some(match &link.daemon_state {
        DaemonConnectionState::Connected => return None,
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
    })
}
