// SPDX-License-Identifier: GPL-3.0-only
use cosmic::{
    iced::{time::Instant, window},
    widget::segmented_button::Entity,
};

use crate::config::AppletConfig;
use crate::features::Feature;
use crate::models::{
    palette::ColorPalette,
    state::IsOpen,
    theme::{VisualizationTheme, WorkingAnimationTheme},
};

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    CloseRequested(window::Id),
    /// Something from, or about, one product's daemon.
    Daemon(Feature, DaemonMessage),
    /// The periodic liveness ping of every installed product's daemon.
    PingTimeout,
    /// Both products are active: show the other one's visualization.
    AlternateFront,
    OpenGitHub,
    /// Open the settings app.
    LaunchApp,
    RevealerToggle(IsOpen),
    SetVisualizationTheme(VisualizationTheme),
    /// Pick the working animation style.
    SetWorkingAnimation(WorkingAnimationTheme),
    /// Pick the pair of colors the products draw in.
    SetColorPalette(ColorPalette),
    SetAppletWidth(u32),
    /// The size slider was let go.
    AppletWidthSet,
    /// Time passed while the panel shows the applet's size: past the linger,
    /// it stops.
    SizePreviewTick(Instant),
    SetShowIcon(bool),
    SetIconAlignmentEntity(Entity),
    SetShowVisualizations(bool),
    /// The settings file changed: another applet, or a hand edit, saved it.
    SettingsChanged(AppletConfig),
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
    /// Triggered by the popup's Retry button. Restarts the `/events`
    /// subscription, so it asks the daemon again and the daemon shows a fresh
    /// consent dialog.
    RetryAuthorization,
    /// Speech to text's `recording_state`.
    RecordingState(bool),
    /// Speech to text's `transcribing_started`: decoding of the captured audio began.
    TranscribingStarted,
    /// Speech to text's `transcribing_stopped`: decode and typing are over.
    TranscribingStopped,
    /// Text to speech's `speaking_state`.
    SpeakingState(bool),
    /// Text to speech's `speech_progress`.
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
