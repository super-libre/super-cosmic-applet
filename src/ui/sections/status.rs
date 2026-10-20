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

#[cfg(test)]
mod status_tests {
    use super::create_status_section;
    use crate::app::{DaemonMessage, Message, ProductLink};
    use crate::models::state::DaemonConnectionState::{
        self, Blocked, Connected, Connecting, Error,
    };
    use crate::products::Product::{self, Stt, Tts};
    use crate::ui::simulator::Simulator;

    fn link(product: Product, installed: bool, state: DaemonConnectionState) -> ProductLink {
        let mut link = ProductLink::new(product, installed);
        link.daemon_state = state;
        link
    }

    /// What the section shows for `links`, or `None` without a section.
    fn shown(links: &[ProductLink]) -> Option<Vec<String>> {
        create_status_section(links).map(|section| Simulator::new(section).texts())
    }

    #[test]
    fn with_every_installed_product_connected_there_is_no_section() {
        assert!(shown(&[link(Stt, true, Connected), link(Tts, true, Connected)]).is_none());
        assert!(shown(&[link(Stt, true, Connected), link(Tts, false, Connecting)]).is_none());
    }

    #[test]
    fn only_a_product_that_isnt_connected_gets_a_line() {
        let texts = shown(&[link(Stt, true, Connected), link(Tts, true, Connecting)])
            .expect("Super TTS isn't connected");
        assert_eq!(
            texts,
            ["Super TTS: connecting", "The daemon may still be starting."]
        );
    }

    #[test]
    fn an_error_says_what_went_wrong() {
        let texts = shown(&[link(Stt, true, Error("revoked: exe_changed".into()))])
            .expect("Super STT failed");
        assert_eq!(texts[0], "Super STT: revoked: exe_changed");
    }

    #[test]
    fn a_product_that_isnt_installed_gets_no_line() {
        let texts = shown(&[link(Stt, true, Connecting), link(Tts, false, Connecting)])
            .expect("Super STT is connecting");
        assert!(texts.iter().all(|t| !t.contains("Super TTS")), "{texts:?}");
    }

    #[test]
    fn with_nothing_installed_it_says_so() {
        let texts = shown(&[link(Stt, false, Connecting), link(Tts, false, Connecting)])
            .expect("a note that nothing is installed");
        assert_eq!(
            texts,
            [
                "Neither Super STT nor Super TTS is installed.",
                "Install either one to use this applet."
            ]
        );
    }

    /// A denial only clears once the daemon restarts, so the line says how,
    /// and its button asks again for the product that was denied.
    #[test]
    fn a_denial_says_how_to_clear_it_and_retries_that_product() {
        let links = [
            link(Stt, true, Connected),
            link(Tts, true, Blocked("user denied".into())),
        ];
        let mut ui = Simulator::new(create_status_section(&links).expect("Super TTS is denied"));
        for line in [
            "Super TTS: authorization denied",
            "Reason: user denied",
            "Restart the daemon to clear the denial:",
            "  systemctl --user restart super-tts",
        ] {
            assert!(ui.shows(line), "{line:?} missing from {:?}", ui.texts());
        }

        ui.click("Retry authorization");

        assert!(matches!(
            ui.into_messages().as_slice(),
            [Message::Daemon(Tts, DaemonMessage::RetryAuthorization)]
        ));
    }
}
