// SPDX-License-Identifier: GPL-3.0-only
//! Daemon-facing operations for the applet.
//!
//! Liveness probes (`/ping`) use the same cached session token as the
//! product's `/events` subscription (shared by `AppId`), so both request the
//! same scopes. The subscription itself is
//! `super_engine_client::widget_subscription::run_widget_subscription`.

use super_engine_client::http_client;
use super_engine_client::session;

use crate::products::{APP_NAME, Product};

/// Ping `product`'s daemon to check it's reachable. Returns the daemon's
/// `message` field (typically `"pong"`).
///
/// On `invalid_session` the cached token is dropped and the ping retried once
/// with a fresh consent flow.
///
/// # Errors
///
/// Returns an error string if the daemon's HTTP listener is unreachable, the
/// consent flow fails, or the token is no longer valid after one retry.
pub async fn ping_daemon(product: Product) -> Result<String, String> {
    let socket = product.socket_path();
    let socket_for_op = socket.clone();
    session::with_token(
        socket,
        product.app_id(),
        APP_NAME,
        product.scopes(),
        move |token| {
            let socket = socket_for_op.clone();
            async move { http_client::ping(socket, &token).await }
        },
    )
    .await
    .map_err(String::from)
}
