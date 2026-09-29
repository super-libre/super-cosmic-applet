// SPDX-License-Identifier: GPL-3.0-only
use cosmic::{iced::window, widget::segmented_button::Entity};

use crate::models::{
    state::IsOpen,
    theme::{VisualizationColor, VisualizationTheme, WorkingAnimationTheme},
};
use crate::products::Product;

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    CloseRequested(window::Id),
    /// Something from, or about, one product's daemon.
    Daemon(Product, DaemonMessage),
    /// The periodic liveness ping of every installed product's daemon.
    PingTimeout,
    /// Both products are active: show the other one's visualization.
    AlternateFront,
    OpenGitHub,
    LaunchApp(Product),
    RevealerToggle(IsOpen),
    SetVisualizationTheme(VisualizationTheme),
    /// Pick the working animation style.
    SetWorkingAnimation(WorkingAnimationTheme),
    SetAppletWidth(u32),
    SetShowIcon(bool),
    SetIconAlignmentEntity(Entity),
    SetShowVisualizations(bool),
    SetVisualizationColor(VisualizationColor, bool), // Color and is_dark flag
    SetColorThemeEntity(Entity),                     // Theme selector for color configuration
    /// Animation frame tick while a product works (drives the working animation).
    WorkingAnimationTick,
}

/// A message about one product's daemon. [`Message::Daemon`] says which.
#[derive(Debug, Clone)]
pub enum DaemonMessage {
    Connected,
    Error(String),
    /// A periodic liveness ping succeeded while already connected.
    PingResponse,
    ScheduleRetry,
    RetryConnection,
    /// Triggered by the popup's Retry button. Clears the cached session token
    /// and restarts the `/events` subscription so the daemon shows a fresh
    /// consent dialog.
    RetryAuthorization,
    /// Super STT's `recording_state`.
    RecordingState(bool),
    /// Super STT's `transcribing_started`: decoding of the captured audio began.
    TranscribingStarted,
    /// Super STT's `transcribing_stopped`: decode and typing are over.
    TranscribingStopped,
    /// Super TTS's `speaking_state`.
    SpeakingState(bool),
    /// Super TTS's `speech_progress`.
    SpeechProgress {
        spoken_ms: u64,
    },
    /// `frequency_bands`: pre-computed visualization bands.
    FrequencyBands {
        bands: Vec<f32>,
        total_energy: f32,
    },
    /// `revoked`: the daemon dropped the session. Carries the event's
    /// `reason` (e.g. `"exe_changed"`).
    Revoked(String),
    /// The user denied the consent dialog. The subscription ended and won't
    /// retry by itself: the popup's "Retry authorization" button restarts it.
    Blocked(String),
    /// The subscription dropped, before or after its handshake. It
    /// reconnects by itself; this is for the log.
    SubscriptionError(String),
    /// Any other event (or a parse failure). Carries the raw event name for
    /// the log.
    OtherEvent(String),
}
