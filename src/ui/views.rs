// SPDX-License-Identifier: GPL-3.0-only
use crate::{
    app::{Message, ProductLink},
    config::AppletConfig,
    models::state::{DaemonConnectionState, IsOpen},
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
        divider(),
        padded_control(create_status_section(params.links))
            .padding(menu_control_padding())
            .apply(Element::from),
    ];
    // The visualization settings apply to whichever product is active, so
    // they appear once either daemon is connected.
    if any_connected {
        content = content.push(divider()).push(create_applet_settings_section(
            params.config,
            params.is_open,
            params.icon_alignment_model,
            params.theme_selector_model,
            params.selected_theme_for_config,
        ));
    }
    content
        .push(divider())
        .push(create_launch_section(params.links))
        .padding([8, 0])
        .into()
}
