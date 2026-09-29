// SPDX-License-Identifier: GPL-3.0-only
use cosmic::{
    app as cosmic_app,
    widget::segmented_button::{Entity, SingleSelectModel},
};

use super::{ProductLink, SuperApplet};
use crate::app::{DaemonMessage, Message};
use crate::config::AppletConfig;
use crate::daemon::ping_daemon;
use crate::models::state::IsOpen;
use crate::models::theme::{IconAlignment, VisualizationSide};
use crate::products::Product;
use crate::ui::components::sound_visualization::VisualizationComponent;
use crate::ui::components::working_animation_component::WorkingAnimationComponent;

/// Build the icon-alignment selector and activate the entry for the stored
/// [`IconAlignment`].
fn build_icon_alignment_model(
    active: IconAlignment,
) -> (SingleSelectModel, Entity, Entity, Entity) {
    let mut model = SingleSelectModel::default();
    let start = model.insert().text(IconAlignment::Start.pretty_name()).id();
    let center = model
        .insert()
        .text(IconAlignment::Center.pretty_name())
        .id();
    let end = model.insert().text(IconAlignment::End.pretty_name()).id();
    model.activate(match active {
        IconAlignment::Start => start,
        IconAlignment::Center => center,
        IconAlignment::End => end,
    });
    (model, start, center, end)
}

/// Build the color-config theme selector, activating the dark or light
/// entry to match the current system theme.
fn build_theme_selector_model(is_dark: bool) -> (SingleSelectModel, Entity, Entity) {
    let mut model = SingleSelectModel::default();
    let light = model.insert().text("Light Theme").id();
    let dark = model.insert().text("Dark Theme").id();
    if is_dark {
        model.activate(dark);
    } else {
        model.activate(light);
    }
    (model, light, dark)
}

/// Ping `product`'s daemon, reporting a success as connected and a failure
/// as a retry to schedule.
pub(super) fn ping_or_retry(product: Product) -> cosmic_app::Task<Message> {
    cosmic_app::Task::perform(ping_daemon(product), move |result| {
        cosmic::Action::App(Message::Daemon(
            product,
            match result {
                Ok(_) => DaemonMessage::Connected,
                Err(e) => {
                    log::debug!("{} is not answering yet: {e}", product.display_name());
                    DaemonMessage::ScheduleRetry
                }
            },
        ))
    })
}

impl SuperApplet {
    pub(super) fn new(
        core: cosmic::app::Core,
        visualization_side: VisualizationSide,
    ) -> (Self, cosmic_app::Task<Message>) {
        let variant_name = AppletConfig::get_variant_name(&visualization_side);
        let config = AppletConfig::load(variant_name, visualization_side.clone());

        let visualization = VisualizationComponent::new(
            0.0,
            false,
            config.visualization.theme.clone(),
            visualization_side.clone(),
            config.visualization.colors.clone(),
        );

        let working_animation = WorkingAnimationComponent::new(
            config.visualization.working_animation,
            visualization_side,
            config.visualization.colors.clone(),
        );

        let (icon_alignment_model, icon_alignment_start, icon_alignment_center, icon_alignment_end) =
            build_icon_alignment_model(config.ui.icon_alignment);

        let is_dark = cosmic::theme::active().cosmic().is_dark;
        let (theme_selector_model, theme_selector_light, theme_selector_dark) =
            build_theme_selector_model(is_dark);

        let applet = Self {
            core,
            links: Product::ALL.map(ProductLink::new),
            popup: None,
            audio_level: 0.0,
            is_speech_detected: false,
            is_open: IsOpen::None,
            alternate: false,
            shown: None,
            visualization,
            working_animation,
            config,
            icon_alignment_model,
            icon_alignment_start,
            icon_alignment_center,
            icon_alignment_end,
            theme_selector_model,
            theme_selector_light,
            theme_selector_dark,
            selected_theme_for_config: is_dark,
        };

        for link in &applet.links {
            log::info!(
                "{} is {}",
                link.product.display_name(),
                if link.installed {
                    "installed"
                } else {
                    "not installed"
                }
            );
        }

        // Ping each installed product's daemon on startup. A failure drops
        // into that product's retry loop rather than surfacing an error.
        let pings = applet
            .links
            .iter()
            .filter(|l| l.installed)
            .map(|l| ping_or_retry(l.product))
            .collect::<Vec<_>>();

        (applet, cosmic_app::Task::batch(pings))
    }
}
