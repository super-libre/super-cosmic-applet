// SPDX-License-Identifier: GPL-3.0-only
//! Daemon-facing operations for the applet.
//!
//! Liveness probes (`/ping`) use the same session token as the product's
//! `/events` subscription, which is [`super::events::subscribe`]. Both get it
//! from [`session`], which shares it with the applet's other processes.

use super_engine_client::http_client;

use super::session;
use crate::features::Feature;

/// Ping `product`'s daemon to check it's reachable. Returns the daemon's
/// `message` field (typically `"pong"`).
///
/// On `invalid_session` the token is reported refused and the ping retried
/// once, with the token another applet process got or a fresh one.
///
/// # Errors
///
/// Returns an error string if the daemon's HTTP listener is unreachable, the
/// consent flow fails, or the token is no longer valid after one retry.
pub async fn ping_daemon(product: Feature) -> Result<String, String> {
    let socket = product.socket_path();
    session::with_token(product, move |token| {
        let socket = socket.clone();
        async move { http_client::ping(socket, &token).await }
    })
    .await
    .map_err(String::from)
}
