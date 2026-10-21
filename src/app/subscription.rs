// SPDX-License-Identifier: GPL-3.0-only
use cosmic::iced::futures::{Stream, channel::mpsc};
use futures_util::{SinkExt, StreamExt};
use log::{info, warn};
use notify::{
    Event, EventKind, RecursiveMode, Watcher,
    event::{AccessKind, AccessMode},
};
use std::ffi::OsStr;
use std::path::PathBuf;
use std::pin::Pin;
use super_engine_client::http_client::WidgetEvent;
use super_engine_client::widget_subscription::{
    WidgetSubscriptionConfig, WidgetSubscriptionUpdate, run_widget_subscription,
};

use crate::app::{DaemonMessage, Message};
use crate::config::{AppletConfig, SETTINGS_FILE};
use crate::products::{APP_NAME, Product};
use crate::util::f64_to_f32;

/// Interval at which the applet pings each daemon to check its health.
pub(super) const PING_INTERVAL_SECS: u64 = 5;

/// Keys `Subscription::run_with`, so a product's subscription restarts when
/// its counter changes and the two products' never collide.
#[derive(Hash)]
pub(super) struct EventsSubscriptionId {
    pub(super) product: Product,
    pub(super) restart_counter: u64,
}

/// Subscribes to one product's `GET /events` SSE stream and forwards each
/// event as a typed [`Message`]. The subscription is self-healing: if the
/// stream drops, the daemon revokes the session, or the connection wedges
/// past the keepalive deadline, [`run_widget_subscription`] reconnects (with
/// backoff) and re-auths by itself. The iced subscription wrapping it only
/// ends when the applet shuts down.
pub(super) fn events_subscription(
    id: &EventsSubscriptionId,
) -> Pin<Box<dyn Stream<Item = Message> + Send>> {
    let product = id.product;
    Box::pin(cosmic::iced::stream::channel(
        100,
        async move |mut channel| {
            let config = WidgetSubscriptionConfig::new(
                product.app_id(),
                APP_NAME,
                product.scopes(),
                product.topics(),
            );
            let mut updates = Box::pin(run_widget_subscription(product.socket_path(), config));
            info!("{} subscription starting", product.display_name());
            while let Some(update) = updates.next().await {
                let msg = Message::Daemon(product, subscription_update_to_message(update));
                if channel.send(msg).await.is_err() {
                    break; // applet shutting down
                }
            }
            info!("{} subscription ended", product.display_name());
        },
    ))
}

/// Watches the settings file in `dir` and reports what another applet, or a hand
/// edit, saved to it. Each side of each panel is its own process, so this is
/// how a change made in one popup reaches the others.
///
/// It watches the directory rather than the file: a save renames a new file
/// over the old one, which would end a watch on the file itself. Without a
/// watch the applet still works, only unsynced, so a failure to set one up
/// is logged and the subscription ends.
// `Subscription::run_with` hands the directory over as the `&PathBuf` it holds.
#[allow(clippy::ptr_arg)]
pub(super) fn watch_settings(dir: &PathBuf) -> Pin<Box<dyn Stream<Item = Message> + Send>> {
    let dir = dir.clone();
    Box::pin(cosmic::iced::stream::channel(
        8,
        async move |mut channel| {
            let path = dir.join(SETTINGS_FILE);
            let (events, mut received) = mpsc::unbounded();
            let watcher = std::fs::create_dir_all(&dir)
                .map_err(notify::Error::io)
                .and_then(|()| {
                    notify::recommended_watcher(move |event| {
                        let _ = events.unbounded_send(event);
                    })
                })
                .and_then(|mut watcher| {
                    watcher.watch(&dir, RecursiveMode::NonRecursive)?;
                    Ok(watcher)
                });
            // Held for as long as the loop runs: dropping it ends the watch.
            let _watcher = match watcher {
                Ok(watcher) => watcher,
                Err(e) => {
                    warn!("Can't watch {} for settings changes: {e}", dir.display());
                    return;
                }
            };
            while let Some(event) = received.next().await {
                match event {
                    Ok(event) if saves_settings(&event) => {
                        if let Some(config) = AppletConfig::read_new(&path)
                            && channel
                                .send(Message::SettingsChanged(config))
                                .await
                                .is_err()
                        {
                            break; // applet shutting down
                        }
                    }
                    Ok(_) => {}
                    Err(e) => warn!("Settings watch: {e}"),
                }
            }
        },
    ))
}

/// Whether `event` may leave new settings in the file. Of the accesses, only
/// the end of a write counts: inotify reports opens too, and the applet opens
/// the file to read it, so counting those would answer every read with
/// another.
fn saves_settings(event: &Event) -> bool {
    let counts = match event.kind {
        EventKind::Access(access) => access == AccessKind::Close(AccessMode::Write),
        _ => true,
    };
    counts
        && event
            .paths
            .iter()
            .any(|path| path.file_name() == Some(OsStr::new(SETTINGS_FILE)))
}

/// Project a [`WidgetSubscriptionUpdate`] into the applet's [`DaemonMessage`].
fn subscription_update_to_message(update: WidgetSubscriptionUpdate) -> DaemonMessage {
    match update {
        // A (re)connect goes through the same handler as a successful ping,
        // so it clears any earlier `Error("revoked: …")` state and resets the
        // retry clock. Without this, a denied → daemon restart → reconnect
        // cycle would leave a stale "revoked" error up although the stream is
        // live again.
        WidgetSubscriptionUpdate::Connected => DaemonMessage::Connected,
        WidgetSubscriptionUpdate::Event(evt) => widget_event_to_message(evt),
        WidgetSubscriptionUpdate::Disconnected { reason } => {
            warn!("/events disconnected ({reason}); reconnecting");
            DaemonMessage::SubscriptionError(reason)
        }
        WidgetSubscriptionUpdate::NeedsReauth { reason } => {
            warn!("Session needs re-auth ({reason}); will ask again on the next attempt");
            DaemonMessage::Revoked(reason)
        }
        WidgetSubscriptionUpdate::Blocked { reason } => {
            warn!("Subscription blocked ({reason}); stream ended");
            DaemonMessage::Blocked(reason)
        }
    }
}

/// Decode a base64-encoded little-endian `f32` buffer carried by an SSE
/// event payload. Returns an empty vector on a missing or malformed value.
fn b64_to_f32_vec(s: Option<&str>) -> Vec<f32> {
    let Some(s) = s else { return Vec::new() };
    let Ok(bytes) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, s) else {
        return Vec::new();
    };
    let (samples, _trailing) = bytes.as_chunks::<4>();
    samples.iter().copied().map(f32::from_le_bytes).collect()
}

/// Translate one [`WidgetEvent`] into a [`DaemonMessage`]. The two products'
/// topics have distinct names, so one mapping serves both; an unknown name
/// becomes `OtherEvent(name)` for the log.
fn widget_event_to_message(evt: WidgetEvent) -> DaemonMessage {
    use serde_json::Value;

    let p: &Value = &evt.payload;
    let flag = |key: &str| p.get(key).and_then(Value::as_bool).unwrap_or(false);
    match evt.name.as_str() {
        "recording_state" => DaemonMessage::RecordingState(flag("is_recording")),
        "transcribing_started" => DaemonMessage::TranscribingStarted,
        "transcribing_stopped" => DaemonMessage::TranscribingStopped,
        "speaking_state" => DaemonMessage::SpeakingState(flag("is_speaking")),
        "speech_progress" => DaemonMessage::SpeechProgress {
            spoken_ms: p.get("spoken_ms").and_then(Value::as_u64).unwrap_or(0),
        },
        "frequency_bands" => DaemonMessage::FrequencyBands {
            bands: b64_to_f32_vec(p.get("bands_b64").and_then(Value::as_str)),
            total_energy: f64_to_f32(p.get("total_energy").and_then(Value::as_f64).unwrap_or(0.0)),
        },
        "revoked" => DaemonMessage::Revoked(
            p.get("reason")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string(),
        ),
        _ => DaemonMessage::OtherEvent(evt.name),
    }
}

#[cfg(test)]
mod widget_subscription_mapping_tests {
    //! These tests pin the mapping between the engine's
    //! `WidgetSubscriptionUpdate` variants and the applet's `DaemonMessage`.
    //! Each variant has a load-bearing contract:
    //!
    //! - `Connected` → `Connected`, so the handler clears any earlier
    //!   `Error("revoked: …")` state after an automatic recovery. If this
    //!   mapping silently changes, the applet shows a stale revoked banner
    //!   forever.
    //! - `Blocked` → `Blocked`, so the popup flips to the sticky
    //!   "Authorization denied" view with a Retry button.
    //! - `NeedsReauth` → `Revoked`, a transient revoked banner while the
    //!   engine does the `session::forget` → fresh-consent cycle.
    //! - `Disconnected` → `SubscriptionError`, so the UI doesn't change state
    //!   during the engine's own backoff and reconnect.
    use super::*;

    #[test]
    fn blocked_maps_to_blocked_with_reason() {
        let update = WidgetSubscriptionUpdate::Blocked {
            reason: "auth_denied (user_denied_cached)".to_string(),
        };
        match subscription_update_to_message(update) {
            DaemonMessage::Blocked(reason) => {
                assert_eq!(reason, "auth_denied (user_denied_cached)");
            }
            other => panic!("Blocked must map to DaemonMessage::Blocked, got {other:?}"),
        }
    }

    #[test]
    fn needs_reauth_maps_to_revoked_with_reason() {
        let update = WidgetSubscriptionUpdate::NeedsReauth {
            reason: "invalid_session (expired)".to_string(),
        };
        match subscription_update_to_message(update) {
            DaemonMessage::Revoked(reason) => {
                assert_eq!(reason, "invalid_session (expired)");
            }
            other => panic!("NeedsReauth must map to DaemonMessage::Revoked, got {other:?}"),
        }
    }

    #[test]
    fn connected_maps_to_connected_for_state_clear() {
        // Critical regression guard: an earlier bug had this mapping to a
        // no-op, which left a stale `Error("revoked: …")` banner up after
        // automatic recovery.
        assert!(matches!(
            subscription_update_to_message(WidgetSubscriptionUpdate::Connected),
            DaemonMessage::Connected
        ));
    }

    #[test]
    fn disconnected_maps_to_subscription_error() {
        let update = WidgetSubscriptionUpdate::Disconnected {
            reason: "stream ended".to_string(),
        };
        match subscription_update_to_message(update) {
            DaemonMessage::SubscriptionError(reason) => assert_eq!(reason, "stream ended"),
            other => panic!("Disconnected must map to SubscriptionError, got {other:?}"),
        }
    }

    fn event(name: &str, payload: serde_json::Value) -> DaemonMessage {
        widget_event_to_message(WidgetEvent {
            name: name.to_string(),
            payload,
        })
    }

    #[test]
    fn revoked_event_maps_to_revoked() {
        match event("revoked", serde_json::json!({ "reason": "exe_changed" })) {
            DaemonMessage::Revoked(reason) => assert_eq!(reason, "exe_changed"),
            other => panic!("revoked event must map to Revoked, got {other:?}"),
        }
    }

    #[test]
    fn each_products_activity_events_map_to_their_messages() {
        assert!(matches!(
            event(
                "recording_state",
                serde_json::json!({ "is_recording": true })
            ),
            DaemonMessage::RecordingState(true)
        ));
        assert!(matches!(
            event("transcribing_started", serde_json::json!({})),
            DaemonMessage::TranscribingStarted
        ));
        assert!(matches!(
            event("transcribing_stopped", serde_json::json!({})),
            DaemonMessage::TranscribingStopped
        ));
        assert!(matches!(
            event("speaking_state", serde_json::json!({ "is_speaking": true })),
            DaemonMessage::SpeakingState(true)
        ));
        assert!(matches!(
            event("speech_progress", serde_json::json!({ "spoken_ms": 120 })),
            DaemonMessage::SpeechProgress { spoken_ms: 120 }
        ));
    }

    #[test]
    fn frequency_bands_decode_from_base64() {
        let bands = [0.25_f32, 0.5, 1.0];
        let bytes: Vec<u8> = bands.iter().flat_map(|b| b.to_le_bytes()).collect();
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes);
        match event(
            "frequency_bands",
            serde_json::json!({ "bands_b64": b64, "total_energy": 0.5 }),
        ) {
            DaemonMessage::FrequencyBands {
                bands: decoded,
                total_energy,
            } => {
                assert_eq!(decoded, bands);
                assert!((total_energy - 0.5).abs() < f32::EPSILON);
            }
            other => panic!("frequency_bands must map to FrequencyBands, got {other:?}"),
        }
    }

    #[test]
    fn an_unknown_event_is_logged_not_acted_on() {
        assert!(matches!(
            event("subscribed", serde_json::json!({})),
            DaemonMessage::OtherEvent(name) if name == "subscribed"
        ));
    }
}

#[cfg(test)]
mod settings_watch_tests {
    use super::saves_settings;
    use crate::config::SETTINGS_FILE;
    use notify::{
        Event, EventKind,
        event::{AccessKind, AccessMode, CreateKind, ModifyKind, RenameMode},
    };
    use std::path::PathBuf;

    fn event(kind: EventKind, name: &str) -> Event {
        Event::new(kind).add_path(PathBuf::from("/config/super-cosmic-applet").join(name))
    }

    #[test]
    fn a_save_renaming_the_settings_into_place_counts() {
        let rename = EventKind::Modify(ModifyKind::Name(RenameMode::To));
        assert!(saves_settings(&event(rename, SETTINGS_FILE)));
    }

    #[test]
    fn a_write_in_place_counts_once_it_is_done() {
        let closed = EventKind::Access(AccessKind::Close(AccessMode::Write));
        assert!(saves_settings(&event(closed, SETTINGS_FILE)));
        let created = EventKind::Create(CreateKind::File);
        assert!(saves_settings(&event(created, SETTINGS_FILE)));
    }

    /// Reading the file opens it, and inotify reports that: counting it
    /// would answer each read with another read.
    #[test]
    fn reading_the_settings_does_not_count() {
        for access in [
            AccessKind::Open(AccessMode::Any),
            AccessKind::Close(AccessMode::Read),
            AccessKind::Read,
        ] {
            assert!(!saves_settings(&event(
                EventKind::Access(access),
                SETTINGS_FILE
            )));
        }
    }

    /// A save writes a temporary file beside the settings first.
    #[test]
    fn other_files_in_the_directory_do_not_count() {
        let created = EventKind::Create(CreateKind::File);
        assert!(!saves_settings(&event(
            created,
            &format!(".{SETTINGS_FILE}.4242")
        )));
        assert!(!saves_settings(&event(created, "notes.txt")));
    }

    /// The watch end to end, on a real directory: what another applet saves
    /// reaches it, while reading the file, and this applet's own saves, don't.
    mod live {
        use super::super::watch_settings;
        use crate::app::Message;
        use crate::config::{AppletConfig, SETTINGS_FILE};
        use futures_util::StreamExt;
        use std::path::Path;
        use std::time::Duration;

        /// The next settings the watch reports, if one comes within `wait`.
        async fn next_settings(
            watch: &mut (impl futures_util::Stream<Item = Message> + Unpin),
            wait: Duration,
        ) -> Option<AppletConfig> {
            match tokio::time::timeout(wait, watch.next()).await {
                Ok(Some(Message::SettingsChanged(config))) => Some(config),
                _ => None,
            }
        }

        /// Save `config` as another applet's process would: the same rename
        /// into place, without this process's record of its own save.
        fn save_as_another_applet(config: &AppletConfig, dir: &Path) {
            std::fs::create_dir_all(dir).expect("create dir");
            let content = ron::ser::to_string_pretty(config, Default::default()).expect("ron");
            let temporary = dir.join(format!(".{SETTINGS_FILE}.other"));
            std::fs::write(&temporary, content).expect("write");
            std::fs::rename(&temporary, dir.join(SETTINGS_FILE)).expect("rename");
        }

        #[tokio::test]
        async fn another_applets_save_reaches_the_watch_and_nothing_else_does() {
            let dir = std::env::temp_dir()
                .join(format!("super-cosmic-applet-watch-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            let mut watch = watch_settings(&dir);
            let mut theirs = AppletConfig::default();
            theirs.ui.applet_width = 222;

            // The watch starts with the stream, so save until it reports one.
            let mut seen = None;
            for _ in 0..50 {
                save_as_another_applet(&theirs, &dir);
                seen = next_settings(&mut watch, Duration::from_millis(100)).await;
                if seen.is_some() {
                    break;
                }
            }
            assert_eq!(
                seen.as_ref(),
                Some(&theirs),
                "another applet's save never reached the watch"
            );
            // Drain whatever else that save produced.
            while next_settings(&mut watch, Duration::from_millis(200))
                .await
                .is_some()
            {}

            for _ in 0..3 {
                std::fs::read_to_string(dir.join(SETTINGS_FILE)).expect("read");
            }
            assert_eq!(
                next_settings(&mut watch, Duration::from_millis(500)).await,
                None,
                "reading the settings fed back into the watch"
            );

            let mut ours = theirs.clone();
            ours.ui.applet_width = 180;
            ours.save_in(&dir).expect("save");
            assert_eq!(
                next_settings(&mut watch, Duration::from_millis(500)).await,
                None,
                "this applet's own save came back to it"
            );

            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}
