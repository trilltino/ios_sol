//! Deep link handler for Solana MWA on iOS.
//!
//! On iOS, dApps open our wallet via a custom URL scheme:
//!   solana-wallet://v1/associate?association=<base64_key>&crypto=<algo>
//!
//! On desktop (Windows/macOS), deep links arrive as CLI arguments and are
//! also handled by tauri-plugin-deep-link (for testing with `solana-wallet://`
//! URLs launched from the browser or terminal).
//!
//! This module registers the event listener on startup and connects outbound
//! to the dApp's reflector/WebSocket server when a link arrives.

use tauri::{AppHandle, Listener, Manager};
use url::Url;

/// Initialize the deep link event listener.
/// Call this once from `run()` after the app is set up.
pub fn init_deep_link_handler(app: &AppHandle) {
    let app_clone = app.clone();
    app.listen("deep-link://new-url", move |event| {
        // Parse the URL list from the event payload
        let urls: Vec<String> = match serde_json::from_str(event.payload()) {
            Ok(v) => v,
            Err(e) => {
                log::error!("Failed to parse deep link payload: {}", e);
                return;
            }
        };

        for url_str in urls {
            if let Ok(url) = url_str.parse::<Url>() {
                if url.scheme() == "solana-wallet" {
                    log::info!("MWA deep link received: {}", url);
                    let app_inner = app_clone.clone();
                    let url_clone = url.clone();
                    tauri::async_runtime::spawn(async move {
                        handle_mwa_deep_link(app_inner, url_clone).await;
                    });
                }
            }
        }
    });
}

/// Parse an MWA association URL and emit an event so the frontend can
/// show the "dApp wants to connect" confirmation screen.
///
/// For the remote scenario (iOS → dApp on web), we would normally connect
/// outbound to the dApp's reflector. For the local scenario (desktop testing),
/// the dApp connects inbound to our localhost:44444 server instead, so this
/// handler is mainly for iOS production use.
async fn handle_mwa_deep_link(app: AppHandle, url: Url) {
    // Extract association parameters
    let query_params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();

    let _association_key = query_params.get("association").cloned();
    let _crypto = query_params.get("crypto").cloned();
    let dapp_uri = query_params
        .get("uri")
        .and_then(|u| percent_encoding::percent_decode_str(u).decode_utf8().ok().map(|s| s.to_string()))
        .unwrap_or_default();

    log::info!("MWA deep link — dApp URI: {}", dapp_uri);

    // Notify the frontend about the incoming connection
    use tauri::Emitter;
    let _ = app.emit(
        "mwa://deep-link-received",
        serde_json::json!({
            "dapp_uri": dapp_uri,
            "url": url.to_string()
        }),
    );
}
