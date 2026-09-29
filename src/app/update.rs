// SPDX-License-Identifier: GPL-3.0-only
use cosmic::{
    app as cosmic_app,
    iced::window,
    surface::{action as surface_action, surface_task},
    widget::segmented_button::Entity,
};
use log::{debug, info, warn};
use super_engine_client::session;

use super::SuperApplet;
use super::init::ping_or_retry;
use crate::app::{DaemonMessage, Message};
use crate::daemon::{RetryStrategy, ping_daemon};
use crate::models::state::{
    CycleEvent, DaemonConnectionState, IsOpen, Phase, PhaseChange, SpeechEvent,
};
use crate::models::theme::{
    IconAlignment, VisualizationColor, VisualizationTheme, WorkingAnimationTheme,
};
use crate::products::Product;

impl SuperApplet {
    pub(super) fn handle_message(&mut self, message: Message) -> cosmic_app::Task<Message> {
        match message {
            Message::TogglePopup => self.toggle_popup(),
            Message::CloseRequested(id) => self.close_popup(id),
            Message::Daemon(product, msg) => self.handle_daemon_message(product, msg),
            Message::PingTimeout => self.ping_timeout(),
            Message::AlternateFront => {
                self.alternate = !self.alternate;
                self.sync_shown();
                cosmic_app::Task::none()
            }
            Message::RevealerToggle(src) => self.revealer_toggle(src),
            Message::SetVisualizationTheme(theme) => self.set_visualization_theme(theme),
            Message::SetWorkingAnimation(theme) => self.set_working_animation(theme),
            Message::OpenGitHub => Self::open_github(),
            Message::LaunchApp(product) => Self::launch_app(product),
            Message::SetAppletWidth(width) => self.set_applet_width(width),
            Message::SetShowIcon(show_icon) => self.set_show_icon(show_icon),
            Message::SetIconAlignmentEntity(entity) => self.set_icon_alignment(entity),
            Message::SetShowVisualizations(show) => self.set_show_visualizations(show),
            Message::SetVisualizationColor(color, is_dark) => {
                self.set_visualization_color(color, is_dark)
            }
            Message::SetColorThemeEntity(entity) => self.set_color_theme(entity),
            Message::WorkingAnimationTick => {
                if let Some(elapsed_ms) = self
                    .front()
                    .and_then(|p| self.link(p).phase.animation_elapsed_ms())
                {
                    self.working_animation.set_elapsed(elapsed_ms);
                }
                cosmic_app::Task::none()
            }
        }
    }

    fn handle_daemon_message(
        &mut self,
        product: Product,
        message: DaemonMessage,
    ) -> cosmic_app::Task<Message> {
        match message {
            DaemonMessage::Connected => self.daemon_connected(product),
            DaemonMessage::PingResponse => self.ping_response(product),
            DaemonMessage::Error(err) => self.daemon_error(product, &err),
            DaemonMessage::ScheduleRetry => self.schedule_retry(product),
            DaemonMessage::RetryConnection => self.retry_connection(product),
            DaemonMessage::RetryAuthorization => self.retry_authorization(product),
            DaemonMessage::RecordingState(is_recording) => {
                self.apply_cycle_event(product, CycleEvent::RecordingFlag(is_recording))
            }
            DaemonMessage::TranscribingStarted => {
                self.apply_cycle_event(product, CycleEvent::TranscribingStarted)
            }
            DaemonMessage::TranscribingStopped => {
                self.apply_cycle_event(product, CycleEvent::TranscribingStopped)
            }
            DaemonMessage::SpeakingState(is_speaking) => {
                self.apply_speech_event(product, SpeechEvent::SpeakingFlag(is_speaking))
            }
            DaemonMessage::SpeechProgress { spoken_ms } => {
                self.apply_speech_event(product, SpeechEvent::Progress { spoken_ms })
            }
            DaemonMessage::FrequencyBands {
                bands,
                total_energy,
            } => self.frequency_bands(product, &bands, total_energy),
            DaemonMessage::Revoked(reason) => self.revoked(product, &reason),
            DaemonMessage::Blocked(reason) => self.blocked(product, reason),
            DaemonMessage::OtherEvent(_) | DaemonMessage::SubscriptionError(_) => {
                // Informational only; the subscription task logs its errors.
                cosmic_app::Task::none()
            }
        }
    }

    /// Open or close the panel popup.
    ///
    /// Routed through `cosmic::surface` rather than the raw `get_popup` /
    /// `destroy_popup` wayland commands so libcosmic tracks the surface: it
    /// owns the popup's corner radii and its frosted-glass blur, and re-applies
    /// both when the theme changes. A popup spawned directly is invisible to
    /// that bookkeeping, which leaves it translucent with nothing blurred
    /// behind it whenever the theme has frosted applets on.
    fn toggle_popup(&mut self) -> cosmic_app::Task<Message> {
        if let Some(p) = self.popup.take() {
            return surface_task(surface_action::destroy_popup(p));
        }
        let Some(main_window_id) = self.core.main_window_id() else {
            warn!("Cannot toggle popup: main window ID not available");
            return cosmic_app::Task::none();
        };
        surface_task(surface_action::app_popup::<Self>(
            // Defaults: inherit the blur and corner radii libcosmic derives
            // from the theme for an applet popup.
            |_| surface_action::LiveSettings::default(),
            move |app: &mut Self| {
                let new_id = window::Id::unique();
                app.popup.replace(new_id);
                app.core
                    .applet
                    .get_popup_settings(main_window_id, new_id, None, None, None)
            },
            // No dedicated view: libcosmic falls back to `view_window`.
            None,
        ))
    }

    fn close_popup(&mut self, id: window::Id) -> cosmic_app::Task<Message> {
        if Some(id) == self.popup {
            self.popup = None;
        }
        cosmic_app::Task::none()
    }

    fn daemon_connected(&mut self, product: Product) -> cosmic_app::Task<Message> {
        let link = self.link_mut(product);
        // Log only the transition: this fires from both a successful
        // (re)connect ping and the subscription's `Connected` update, which
        // would otherwise log twice at startup.
        if link.daemon_state != DaemonConnectionState::Connected {
            info!("Connected to {}", product.display_name());
        }
        link.daemon_state = DaemonConnectionState::Connected;
        link.retry_strategy.reset();
        // The /events subscription reconnects by itself, so this deliberately
        // does NOT bump `restart_counter`. Restarting it on every successful
        // ping would cancel the engine's task mid-flight and send every ping
        // cycle back through `session::obtain`: another keyring touch.
        cosmic_app::Task::none()
    }

    fn ping_response(&mut self, product: Product) -> cosmic_app::Task<Message> {
        // Fires on every liveness ping (~5 s) while connected: steady state,
        // so log at debug.
        debug!("{} ping OK", product.display_name());
        let link = self.link_mut(product);
        link.daemon_state = DaemonConnectionState::Connected;
        link.retry_strategy.reset();
        cosmic_app::Task::none()
    }

    fn daemon_error(&mut self, product: Product, err: &str) -> cosmic_app::Task<Message> {
        warn!("{} error: {err}", product.display_name());
        let link = self.link_mut(product);
        // Reset backoff when an established connection drops, so reconnecting
        // starts from the initial-connection strategy.
        if matches!(link.daemon_state, DaemonConnectionState::Connected) {
            link.retry_strategy = RetryStrategy::for_initial_connection();
            info!(
                "Lost the connection to {}, reconnecting",
                product.display_name()
            );
        }
        // Retry forever: schedule the next attempt.
        cosmic_app::Task::perform(async {}, move |()| {
            cosmic::Action::App(Message::Daemon(product, DaemonMessage::ScheduleRetry))
        })
    }

    fn schedule_retry(&mut self, product: Product) -> cosmic_app::Task<Message> {
        let link = self.link_mut(product);
        link.retry_strategy.should_retry(); // Always true; increments the attempt counter.
        let delay = link.retry_strategy.next_delay();
        debug!(
            "Retrying {} in {delay:?} (attempt {})",
            product.display_name(),
            link.retry_strategy.attempt
        );
        link.daemon_state = DaemonConnectionState::Connecting;
        cosmic_app::Task::perform(
            async move {
                tokio::time::sleep(delay).await;
            },
            move |()| cosmic::Action::App(Message::Daemon(product, DaemonMessage::RetryConnection)),
        )
    }

    fn retry_connection(&mut self, product: Product) -> cosmic_app::Task<Message> {
        let link = self.link_mut(product);
        if !link.installed {
            // Uninstalled since the retry was scheduled: stop retrying.
            return cosmic_app::Task::none();
        }
        if matches!(link.daemon_state, DaemonConnectionState::Error(_)) {
            // Manual retry from the error state: reset backoff.
            link.retry_strategy = RetryStrategy::for_initial_connection();
            link.daemon_state = DaemonConnectionState::Connecting;
        }
        ping_or_retry(product)
    }

    fn retry_authorization(&mut self, product: Product) -> cosmic_app::Task<Message> {
        info!(
            "Asking {} for authorization again after a denial",
            product.display_name()
        );
        // Drop any cached token (in memory and in the keyring) so the next
        // subscription cycle reaches the daemon's /auth/request and a fresh
        // consent dialog.
        if let Err(e) = session::forget(product.app_id()) {
            warn!("Failed to forget the session before retrying: {e}");
        }
        let link = self.link_mut(product);
        // Restart the subscription so the ended engine task starts afresh.
        link.restart_counter = link.restart_counter.wrapping_add(1);
        link.daemon_state = DaemonConnectionState::Connecting;
        link.retry_strategy = RetryStrategy::for_initial_connection();
        cosmic_app::Task::none()
    }

    /// Ping every installed product's daemon, and notice a product installed
    /// or removed since the last ping.
    fn ping_timeout(&mut self) -> cosmic_app::Task<Message> {
        let mut tasks = Vec::new();
        for link in &mut self.links {
            let product = link.product;
            let installed = product.is_installed();
            if installed != link.installed {
                info!(
                    "{} was {}",
                    product.display_name(),
                    if installed { "installed" } else { "removed" }
                );
                link.installed = installed;
                link.daemon_state = DaemonConnectionState::Connecting;
                link.retry_strategy = RetryStrategy::for_initial_connection();
                if installed {
                    tasks.push(ping_or_retry(product));
                }
                continue;
            }
            if link.installed && link.daemon_state == DaemonConnectionState::Connected {
                tasks.push(cosmic_app::Task::perform(
                    ping_daemon(product),
                    move |result| {
                        cosmic::Action::App(Message::Daemon(
                            product,
                            match result {
                                Ok(_) => DaemonMessage::PingResponse,
                                Err(e) => DaemonMessage::Error(format!("Connection lost: {e}")),
                            },
                        ))
                    },
                ));
            }
        }
        self.sync_shown();
        cosmic_app::Task::batch(tasks)
    }

    /// Fold one of Super STT's recording-cycle events into its phase.
    fn apply_cycle_event(
        &mut self,
        product: Product,
        event: CycleEvent,
    ) -> cosmic_app::Task<Message> {
        let change = match &mut self.link_mut(product).phase {
            Phase::Recording(phase) => phase.apply(event),
            Phase::Speech(_) => {
                warn!("{} sent a recording event", product.display_name());
                PhaseChange::default()
            }
        };
        self.after_phase_change(product, change)
    }

    /// Fold one of Super TTS's utterance events into its phase.
    fn apply_speech_event(
        &mut self,
        product: Product,
        event: SpeechEvent,
    ) -> cosmic_app::Task<Message> {
        let change = match &mut self.link_mut(product).phase {
            Phase::Speech(phase) => phase.apply(event),
            Phase::Recording(_) => {
                warn!("{} sent a speech event", product.display_name());
                PhaseChange::default()
            }
        };
        self.after_phase_change(product, change)
    }

    /// Carry out what a phase change leaves for the drawing components, which
    /// only ever draw the product in front.
    fn after_phase_change(
        &mut self,
        product: Product,
        change: PhaseChange,
    ) -> cosmic_app::Task<Message> {
        let in_front = self.front() == Some(product);
        if change.animation_restarted && in_front {
            self.working_animation.reset();
        }
        if change.visualization_stale && self.shown == Some(product) {
            self.visualization.clear();
        }
        self.sync_shown();
        cosmic_app::Task::none()
    }

    /// Keep `shown` on the product in front, clearing the bars and restarting
    /// the animation when that changes, so the panel never draws one
    /// product's audio as the other's.
    fn sync_shown(&mut self) {
        let front = self.front();
        if front != self.shown {
            self.visualization.clear();
            self.working_animation.reset();
            self.audio_level = 0.0;
            self.is_speech_detected = false;
            self.shown = front;
        }
    }

    fn frequency_bands(
        &mut self,
        product: Product,
        bands: &[f32],
        total_energy: f32,
    ) -> cosmic_app::Task<Message> {
        // Only the product in front draws. The other's bands are dropped
        // rather than mixed into the same bars.
        if self.front() == Some(product) {
            self.visualization
                .update_frequency_bands(bands, total_energy);
            self.audio_level = total_energy;
            self.is_speech_detected = total_energy > 0.02;
        }
        cosmic_app::Task::none()
    }

    fn revoked(&mut self, product: Product, reason: &str) -> cosmic_app::Task<Message> {
        warn!(
            "{} revoked the session (reason={reason}); the next retry asks for consent again",
            product.display_name()
        );
        // Treat like a dropped connection, so the reconnect path does a fresh
        // /auth/request, consent dialog and subscription.
        self.link_mut(product).daemon_state =
            DaemonConnectionState::Error(format!("revoked: {reason}"));
        self.sync_shown();
        cosmic_app::Task::none()
    }

    fn blocked(&mut self, product: Product, reason: String) -> cosmic_app::Task<Message> {
        warn!(
            "{} denied the subscription ({reason}); waiting for an explicit retry",
            product.display_name()
        );
        self.link_mut(product).daemon_state = DaemonConnectionState::Blocked(reason);
        self.sync_shown();
        cosmic_app::Task::none()
    }

    fn revealer_toggle(&mut self, is_open_src: IsOpen) -> cosmic_app::Task<Message> {
        self.is_open = if self.is_open == is_open_src {
            IsOpen::None
        } else {
            is_open_src
        };
        cosmic_app::Task::none()
    }

    fn set_visualization_theme(&mut self, theme: VisualizationTheme) -> cosmic_app::Task<Message> {
        self.config
            .update(|c| c.visualization.theme = theme.clone());
        self.visualization.update_theme(theme);
        self.visualization
            .update_audio_level(self.audio_level, self.is_speech_detected);
        self.is_open = IsOpen::None;
        cosmic_app::Task::none()
    }

    fn set_working_animation(&mut self, theme: WorkingAnimationTheme) -> cosmic_app::Task<Message> {
        self.config
            .update(|c| c.visualization.working_animation = theme);
        self.working_animation.update_theme(theme);
        self.is_open = IsOpen::None;
        cosmic_app::Task::none()
    }

    /// Spawn a fire-and-forget child and reap it in a detached thread, so a
    /// launched helper never lingers as a zombie in the session-long applet.
    /// The exit status doesn't matter; the `wait` only prevents the zombie.
    fn spawn_detached(cmd: &mut std::process::Command) -> std::io::Result<()> {
        let mut child = cmd.spawn()?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }

    fn open_github() -> cosmic_app::Task<Message> {
        if let Err(e) =
            Self::spawn_detached(std::process::Command::new("xdg-open").arg(crate::REPOSITORY))
        {
            warn!("Failed to open GitHub URL: {e}");
        }
        cosmic_app::Task::none()
    }

    fn launch_app(product: Product) -> cosmic_app::Task<Message> {
        // `Command::new` already searches `PATH`; then the standard install
        // prefixes.
        let app = product.app_binary();
        let launch_attempts = [
            app.clone(),
            format!("/usr/local/bin/{app}"),
            format!("/usr/bin/{app}"),
        ];
        for command in &launch_attempts {
            if Self::spawn_detached(&mut std::process::Command::new(command)).is_ok() {
                info!("Launched {} with {command}", product.display_name());
                return cosmic_app::Task::none();
            }
        }
        warn!(
            "Failed to launch the {} app: tried {launch_attempts:?}",
            product.display_name()
        );
        cosmic_app::Task::none()
    }

    fn set_applet_width(&mut self, width: u32) -> cosmic_app::Task<Message> {
        self.config.update(|c| c.ui.applet_width = width);
        // Refresh the visualization so it adapts to the new size.
        self.visualization.clear();
        self.visualization
            .update_audio_level(self.audio_level, self.is_speech_detected);
        cosmic_app::Task::none()
    }

    fn set_show_icon(&mut self, show_icon: bool) -> cosmic_app::Task<Message> {
        self.config.update(|c| c.ui.show_icon = show_icon);
        cosmic_app::Task::none()
    }

    fn set_icon_alignment(&mut self, entity: Entity) -> cosmic_app::Task<Message> {
        self.icon_alignment_model.activate(entity);
        let alignment = if entity == self.icon_alignment_start {
            IconAlignment::Start
        } else if entity == self.icon_alignment_center {
            IconAlignment::Center
        } else if entity == self.icon_alignment_end {
            IconAlignment::End
        } else {
            IconAlignment::Start
        };
        self.config.update(|c| c.ui.icon_alignment = alignment);
        cosmic_app::Task::none()
    }

    fn set_show_visualizations(&mut self, show: bool) -> cosmic_app::Task<Message> {
        self.config.update(|c| c.ui.show_visualization = show);
        cosmic_app::Task::none()
    }

    fn set_visualization_color(
        &mut self,
        color: VisualizationColor,
        is_dark: bool,
    ) -> cosmic_app::Task<Message> {
        let mut updated_colors = self.config.visualization.colors.clone();
        updated_colors.set_color(color, is_dark);
        self.config
            .update(|c| c.visualization.colors = updated_colors.clone());
        self.working_animation.update_colors(updated_colors.clone());
        self.visualization.update_colors(updated_colors);
        cosmic_app::Task::none()
    }

    fn set_color_theme(&mut self, entity: Entity) -> cosmic_app::Task<Message> {
        self.theme_selector_model.activate(entity);
        if entity == self.theme_selector_light {
            self.selected_theme_for_config = false;
        } else if entity == self.theme_selector_dark {
            self.selected_theme_for_config = true;
        }
        cosmic_app::Task::none()
    }
}
