// SPDX-License-Identifier: GPL-3.0-only
//! The self-healing `/events` subscription.
//!
//! This is super-engine's `widget_subscription::run_widget_subscription` with
//! one change: the token comes from [`session`], which the applet's
//! processes share. The engine's loop keeps its token per process, and when
//! the daemon refuses it the loop deletes the token file every process reads.
//! Then every process asks for a token of its own, and each ask may raise a
//! consent popup. Here a refused token is reported to [`session`], and the
//! next one comes from whichever process asks first.
//!
//! Everything else is the engine's: the updates it yields, its backoff, the
//! keepalive deadline, and which failures end the stream.

use futures_util::{Stream, StreamExt};
use super_engine_client::http_client::{self, HttpError};
use super_engine_client::retry::RetryStrategy;
use super_engine_client::widget_subscription::{
    DEFAULT_IDLE_TIMEOUT, DEFAULT_INITIAL_BACKOFF, DEFAULT_MAX_BACKOFF, WidgetSubscriptionUpdate,
    is_user_denied,
};

use super::session;
use crate::features::Feature;

/// `product`'s events, for as long as the stream is polled. It reconnects
/// with backoff, gets a new token when the daemon refuses the one it has, and
/// ends only when the user denies the applet a token.
pub fn subscribe(
    product: Feature,
) -> impl Stream<Item = WidgetSubscriptionUpdate> + Send + 'static {
    let idle_timeout = DEFAULT_IDLE_TIMEOUT;
    let socket = product.socket_path();
    async_stream::stream! {
        let mut retry = RetryStrategy {
            attempt: 0,
            initial_delay: DEFAULT_INITIAL_BACKOFF,
            max_delay: DEFAULT_MAX_BACKOFF,
            use_exponential_backoff: true,
        };
        loop {
            let token = match session::token(product).await {
                Ok(token) => token,
                // Asking again would only meet the daemon's cached denial.
                Err(e) if is_user_denied(&e) => {
                    yield WidgetSubscriptionUpdate::Blocked { reason: e.to_string() };
                    return;
                }
                Err(e @ (HttpError::AuthDenied { .. }
                | HttpError::InvalidSession { .. }
                | HttpError::ScopeDenied)) => {
                    yield WidgetSubscriptionUpdate::NeedsReauth { reason: e.to_string() };
                    tokio::time::sleep(retry.next_delay()).await;
                    retry.should_retry();
                    continue;
                }
                Err(e @ HttpError::Other(_)) => {
                    yield WidgetSubscriptionUpdate::Disconnected { reason: e.to_string() };
                    tokio::time::sleep(retry.next_delay()).await;
                    retry.should_retry();
                    continue;
                }
            };

            let mut stream = match http_client::events_stream(socket.clone(), &token, product.topics()).await {
                Ok(stream) => Box::pin(stream),
                // `scope_denied` too: the daemon refuses the whole stream when
                // the token lacks a topic's scope, as a token from before the
                // applet subscribed to that topic does.
                Err(e) if e.is_invalid_session() || e.is_scope_denied() => {
                    session::refused(&token);
                    yield WidgetSubscriptionUpdate::NeedsReauth { reason: e.to_string() };
                    continue;
                }
                Err(e) => {
                    yield WidgetSubscriptionUpdate::Disconnected { reason: e.to_string() };
                    tokio::time::sleep(retry.next_delay()).await;
                    retry.should_retry();
                    continue;
                }
            };
            yield WidgetSubscriptionUpdate::Connected;
            retry.reset();

            // Read with an idle deadline: the daemon sends a keepalive every
            // 30 s, so silence past it means a wedged connection. End of
            // stream, the deadline and a revocation all reconnect.
            let mut revoked = None;
            loop {
                match tokio::time::timeout(idle_timeout, stream.next()).await {
                    Ok(Some(event)) => {
                        if event.name == "revoked" {
                            let reason = event.payload.get("reason")
                                .and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                            revoked = Some(format!("revoked: {reason}"));
                            break;
                        }
                        if matches!(event.name.as_str(), "subscribed" | "keepalive") {
                            continue;
                        }
                        yield WidgetSubscriptionUpdate::Event(event);
                    }
                    Ok(None) => {
                        yield WidgetSubscriptionUpdate::Disconnected { reason: "stream ended".to_string() };
                        break;
                    }
                    Err(_) => {
                        yield WidgetSubscriptionUpdate::Disconnected {
                            reason: format!("idle_timeout ({}s)", idle_timeout.as_secs()),
                        };
                        break;
                    }
                }
            }

            if let Some(reason) = revoked {
                session::refused(&token);
                yield WidgetSubscriptionUpdate::NeedsReauth { reason };
                continue;
            }
            tokio::time::sleep(retry.next_delay()).await;
            retry.should_retry();
        }
    }
}
