// SPDX-License-Identifier: GPL-3.0-only
mod init;
mod layout;
pub mod messages;
mod subscription;
#[cfg(test)]
pub(crate) mod testing;
mod update;
mod view;

use std::path::PathBuf;
use std::time::Duration;

use cosmic::{
    Element, app as cosmic_app,
    iced::{Subscription, window},
    widget::segmented_button::{Entity, SingleSelectModel},
};

pub use messages::*;

use crate::config::AppletConfig;
use crate::daemon::RetryStrategy;
use crate::models::state::{DaemonConnectionState, IsOpen, Phase};
use crate::models::theme::VisualizationSide;
use crate::products::Product;
use crate::ui::components::sound_visualization::VisualizationComponent;
use crate::ui::components::working_animation_component::WorkingAnimationComponent;
use subscription::{EventsSubscriptionId, PING_INTERVAL_SECS, events_subscription, watch_settings};

/// How long the panel shows one product's visualization before the other's,
/// when both are active at once.
const ALTERNATE_SECS: u64 = 3;

/// The applet's link to one product's daemon.
pub struct ProductLink {
    pub product: Product,
    /// Whether the product is installed. A product that isn't gets no
    /// subscription, no pings and no status line. Checked again on every
    /// ping, so a product installed later shows up without a restart.
    pub installed: bool,
    pub daemon_state: DaemonConnectionState,
    pub retry_strategy: RetryStrategy,
    /// Keys the product's `/events` subscription, so a forced re-auth
    /// (`DaemonMessage::RetryAuthorization`) tears down the old stream and
    /// starts a fresh one.
    pub restart_counter: u64,
    /// What the product is doing, and the clock behind its working animation.
    pub phase: Phase,
}

impl ProductLink {
    pub(crate) fn new(product: Product, installed: bool) -> Self {
        Self {
            product,
            installed,
            daemon_state: DaemonConnectionState::Connecting,
            retry_strategy: RetryStrategy::for_initial_connection(),
            restart_counter: 0,
            phase: match product {
                Product::Stt => Phase::Recording(crate::models::state::RecordingPhase::default()),
                Product::Tts => Phase::Speech(crate::models::state::SpeechPhase::default()),
            },
        }
    }

    /// Active, as the panel counts it: connected, and recording, transcribing,
    /// synthesizing or speaking.
    fn is_active(&self) -> bool {
        self.installed
            && self.daemon_state == DaemonConnectionState::Connected
            && self.phase.is_active()
    }
}

/// The product the panel shows: the active one, or when both are, whichever
/// `alternate` points at.
fn pick_front(active: &[Product], alternate: bool) -> Option<Product> {
    match active {
        [] => None,
        [only] => Some(*only),
        [first, second, ..] => Some(if alternate { *second } else { *first }),
    }
}

/// The product whose tab the popup's settings show: the one last picked while
/// it is installed, else the first installed one.
fn pick_settings_tab(installed: &[Product], picked: Product) -> Option<Product> {
    if installed.contains(&picked) {
        Some(picked)
    } else {
        installed.first().copied()
    }
}

pub struct SuperApplet {
    core: cosmic::app::Core,
    /// One link per product, in `Product::ALL` order.
    links: [ProductLink; 2],
    popup: Option<window::Id>,
    audio_level: f32,
    is_speech_detected: bool,
    is_open: IsOpen,
    /// Which product the panel shows while both are active. Flipped every
    /// `ALTERNATE_SECS`.
    alternate: bool,
    /// The product the visualization was last fed, so switching products
    /// clears the other one's bars.
    shown: Option<Product>,
    visualization: VisualizationComponent,
    working_animation: WorkingAnimationComponent,
    config: AppletConfig,
    icon_alignment_model: SingleSelectModel,
    icon_alignment_start: Entity,
    icon_alignment_center: Entity,
    icon_alignment_end: Entity,
    theme_selector_model: SingleSelectModel,
    theme_selector_light: Entity,
    theme_selector_dark: Entity,
    selected_theme_for_config: bool, // false = light, true = dark
    /// The settings tab last picked. The popup falls back to another
    /// installed product's while this one isn't installed.
    picked_settings_tab: Product,
    /// Where the settings are saved, and watched for the other applets'
    /// changes.
    settings_dir: PathBuf,
    /// Whether a product is installed: [`Product::is_installed`], but for a
    /// test.
    is_installed: fn(Product) -> bool,
}

impl SuperApplet {
    fn link(&self, product: Product) -> &ProductLink {
        &self.links[product as usize]
    }

    fn link_mut(&mut self, product: Product) -> &mut ProductLink {
        &mut self.links[product as usize]
    }

    fn active_products(&self) -> Vec<Product> {
        self.links
            .iter()
            .filter(|l| l.is_active())
            .map(|l| l.product)
            .collect()
    }

    /// The product whose activity the panel shows, if any is active.
    fn front(&self) -> Option<Product> {
        pick_front(&self.active_products(), self.alternate)
    }

    fn installed_products(&self) -> Vec<Product> {
        self.links
            .iter()
            .filter(|l| l.installed)
            .map(|l| l.product)
            .collect()
    }

    /// The product whose settings the popup shows, if any is installed.
    fn settings_tab(&self) -> Option<Product> {
        pick_settings_tab(&self.installed_products(), self.picked_settings_tab)
    }
}

impl cosmic::Application for SuperApplet {
    type Message = Message;
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = VisualizationSide;
    const APP_ID: &'static str = "ai.menjivar.super-cosmic-applet";

    fn init(
        core: cosmic::app::Core,
        visualization_side: Self::Flags,
    ) -> (Self, cosmic_app::Task<Self::Message>) {
        Self::new(core, visualization_side)
    }

    fn core(&self) -> &cosmic::app::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::app::Core {
        &mut self.core
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subs: Vec<Subscription<Message>> = self
            .links
            .iter()
            .filter(|l| l.installed)
            .map(|l| {
                Subscription::run_with(
                    EventsSubscriptionId {
                        product: l.product,
                        restart_counter: l.restart_counter,
                    },
                    events_subscription,
                )
            })
            .collect();
        subs.push(
            cosmic::iced::time::every(Duration::from_secs(PING_INTERVAL_SECS))
                .map(|_| Message::PingTimeout),
        );
        subs.push(Subscription::run_with(
            self.settings_dir.clone(),
            watch_settings,
        ));
        if self.active_products().len() > 1 {
            subs.push(
                cosmic::iced::time::every(Duration::from_secs(ALTERNATE_SECS))
                    .map(|_| Message::AlternateFront),
            );
        }
        if self
            .front()
            .is_some_and(|p| self.link(p).phase.is_working())
        {
            subs.push(
                cosmic::iced::time::every(Duration::from_millis(33))
                    .map(|_| Message::WorkingAnimationTick),
            );
        }
        Subscription::batch(subs)
    }

    fn update(&mut self, message: Self::Message) -> cosmic_app::Task<Self::Message> {
        self.handle_message(message)
    }

    fn view(&self) -> Element<'_, Message> {
        self.view_applet()
    }

    fn view_window(&self, _id: window::Id) -> Element<'_, Message> {
        self.view_popup()
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Message> {
        Some(Message::CloseRequested(id))
    }
}

#[cfg(test)]
mod tests {
    use super::{pick_front, pick_settings_tab};
    use crate::products::Product::{Stt, Tts};

    #[test]
    fn a_test_applet_builds_and_draws_headless() {
        use super::testing::TestApplet;
        let applet = TestApplet::new(&[Stt, Tts]);
        let _ = applet.popup();
        let _ = applet.panel();
        assert!(applet.link(Stt).installed && applet.link(Tts).installed);
        assert_eq!(applet.saved(), None, "nothing changed, so nothing saved");
    }

    #[test]
    fn the_picked_settings_tab_stays_while_its_product_is_installed() {
        assert_eq!(pick_settings_tab(&[Stt, Tts], Tts), Some(Tts));
        assert_eq!(pick_settings_tab(&[Tts], Tts), Some(Tts));
    }

    #[test]
    fn a_settings_tab_for_a_removed_product_falls_back_to_an_installed_one() {
        assert_eq!(pick_settings_tab(&[Stt], Tts), Some(Stt));
        assert_eq!(pick_settings_tab(&[Tts], Stt), Some(Tts));
    }

    #[test]
    fn with_nothing_installed_there_is_no_settings_tab() {
        assert_eq!(pick_settings_tab(&[], Stt), None);
    }

    #[test]
    fn nothing_active_shows_the_icon() {
        assert_eq!(pick_front(&[], false), None);
        assert_eq!(pick_front(&[], true), None);
    }

    #[test]
    fn one_active_product_is_shown_whatever_the_alternation() {
        for alternate in [false, true] {
            assert_eq!(pick_front(&[Stt], alternate), Some(Stt));
            assert_eq!(pick_front(&[Tts], alternate), Some(Tts));
        }
    }

    #[test]
    fn two_active_products_take_turns() {
        assert_eq!(pick_front(&[Stt, Tts], false), Some(Stt));
        assert_eq!(pick_front(&[Stt, Tts], true), Some(Tts));
    }
}
