// SPDX-License-Identifier: GPL-3.0-only
use cosmic::iced::futures::Stream;
use futures_util::{SinkExt, StreamExt};
use log::{info, warn};
use std::pin::Pin;
use super_engine_client::http_client::WidgetEvent;
use super_engine_client::widget_subscription::{
    WidgetSubscriptionConfig, WidgetSubscriptionUpdate, run_widget_subscription,
};

use crate::app::{DaemonMessage, Message};
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
