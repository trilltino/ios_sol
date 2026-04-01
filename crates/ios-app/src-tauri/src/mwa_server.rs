//! MWA WebSocket server — runs on localhost:44444 on all platforms.
//!
//! Protocol: Solana Mobile Wallet Adapter (local session)
//! Port: 44444 (fixed, whitelisted in Tauri CSP)
//!
//! Flow:
//!   1. Listen on ws://localhost:44444/solana-wallet
//!   2. Accept HELLO_REQ (65-byte dApp P-256 ECDH pubkey + 64-byte sig)
//!   3. ECDH handshake → derive AES-256-GCM key via HKDF-SHA256
//!   4. Send HELLO_RSP (wallet's 65-byte P-256 ECDH pubkey)
//!   5. Decrypt/process/encrypt JSON-RPC messages in a loop
//!   6. Emit Tauri events for frontend approval, await oneshot response

use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use hkdf::Hkdf;
use ring::{
    agreement::{self, EphemeralPrivateKey, UnparsedPublicKey, ECDH_P256},
    rand::SystemRandom,
};
use sha2::Sha256;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::oneshot,
};
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::{
        handshake::server::{Request, Response},
        Message,
    },
};
use wallet_core::mwa::{
    build_authorize_result, build_sign_and_send_result, build_sign_result, user_declined_error,
    MwaRequest, MwaSession,
};

/// Length of an uncompressed P-256 public key: 0x04 || x(32) || y(32)
const UNCOMPRESSED_P256_KEY_LEN: usize = 65;

pub const MWA_PORT: u16 = 44444;
pub const MWA_WS_PATH: &str = "/solana-wallet";

// ─── Pending response map ─────────────────────────────────────────────────────

pub type PendingResponseSender = oneshot::Sender<MwaResponse>;
pub type PendingResponseMap = Arc<Mutex<HashMap<String, PendingResponseSender>>>;

#[derive(Debug)]
pub enum MwaResponse {
    AuthorizeApproved {
        auth_token: String,
        pubkey_b58: String,
    },
    SignApproved {
        signed_payloads: Vec<Vec<u8>>,
    },
    SignAndSendApproved {
        signatures: Vec<String>,
    },
    Rejected,
}

// ─── ECDH handshake (ring-based, no zeroize conflict) ────────────────────────

/// Parse HELLO_REQ, derive shared AES key, return (aes_key, hello_rsp).
///
/// Uses `ring` for ECDH P-256. `ring` bundles its own crypto without
/// exposing `zeroize` in its public API, avoiding the version conflict.
fn establish_session(hello_req: &[u8]) -> Result<([u8; 32], Vec<u8>)> {
    if hello_req.len() < UNCOMPRESSED_P256_KEY_LEN {
        return Err(anyhow!("HELLO_REQ too short: {} bytes", hello_req.len()));
    }

    let rng = SystemRandom::new();

    // Generate wallet's ephemeral P-256 ECDH keypair
    let wallet_private_key = EphemeralPrivateKey::generate(&ECDH_P256, &rng)
        .map_err(|e| anyhow!("Failed to generate ECDH key: {:?}", e))?;

    // Get the wallet's public key bytes (uncompressed, 65 bytes)
    let wallet_public_key_bytes = wallet_private_key
        .compute_public_key()
        .map_err(|e| anyhow!("Failed to compute public key: {:?}", e))?;
    let hello_rsp = wallet_public_key_bytes.as_ref().to_vec(); // 65 bytes

    // Parse dApp's P-256 public key
    let dapp_public_key = UnparsedPublicKey::new(
        &ECDH_P256,
        &hello_req[..UNCOMPRESSED_P256_KEY_LEN],
    );

    // ECDH: compute shared secret
    let mut aes_key = [0u8; 32];
    agreement::agree_ephemeral(
        wallet_private_key,
        &dapp_public_key,
        |shared_secret| {
            // HKDF-SHA256 to derive 32-byte AES-256-GCM key
            // Label matches the JS side: "mobile_wallet_adapter_aes"
            let hk = Hkdf::<Sha256>::new(None, shared_secret);
            hk.expand(b"mobile_wallet_adapter_aes", &mut aes_key)
                .map_err(|_| anyhow!("HKDF expand failed"))
        },
    )
    .map_err(|e| anyhow!("ECDH agreement failed: {:?}", e))??;

    Ok((aes_key, hello_rsp))
}

// ─── Server ───────────────────────────────────────────────────────────────────

pub async fn start_mwa_server(app: tauri::AppHandle, pending: PendingResponseMap) -> Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], MWA_PORT));
    let listener = TcpListener::bind(addr).await?;
    log::info!(
        "MWA WebSocket server listening on ws://localhost:{}{}",
        MWA_PORT,
        MWA_WS_PATH
    );

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                log::info!("MWA connection from {}", peer_addr);
                let app_clone = app.clone();
                let pending_clone = pending.clone();
                tokio::spawn(async move {
                    if let Err(e) =
                        handle_mwa_connection(stream, app_clone, pending_clone).await
                    {
                        log::error!("MWA connection error: {}", e);
                    }
                });
            }
            Err(e) => {
                log::error!("MWA accept error: {}", e);
            }
        }
    }
}

// ─── Per-connection ───────────────────────────────────────────────────────────

async fn handle_mwa_connection(
    stream: TcpStream,
    app: tauri::AppHandle,
    pending: PendingResponseMap,
) -> Result<()> {
    // WebSocket upgrade — must be on /solana-wallet path
    let ws_stream = accept_hdr_async(stream, |req: &Request, response: Response| {
        if req.uri().path() != MWA_WS_PATH {
            let mut reject = tokio_tungstenite::tungstenite::http::Response::new(Some("Wrong path".to_string()));
            *reject.status_mut() =
                tokio_tungstenite::tungstenite::http::StatusCode::NOT_FOUND;
            return Err(reject);
        }
        Ok(response)
    })
    .await?;

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // ── Phase 1: Receive HELLO_REQ ────────────────────────────────────────────
    let hello_req_bytes = loop {
        match ws_receiver.next().await {
            Some(Ok(Message::Binary(b))) => {
                if b.is_empty() {
                    ws_sender.send(Message::Binary(vec![])).await?;
                    continue; // pong empty APP_PING
                }
                break b;
            }
            Some(Ok(Message::Close(_))) | None => return Ok(()),
            Some(Ok(_)) | Some(Err(_)) => continue,
        }
    };

    // ── Phase 2: ECDH handshake ───────────────────────────────────────────────
    let (aes_key, hello_rsp) = establish_session(&hello_req_bytes)?;
    let session = MwaSession::from_key(&aes_key)?;
    ws_sender.send(Message::Binary(hello_rsp)).await?;
    log::info!("MWA session established");

    // ── Phase 3: Process encrypted JSON-RPC messages ──────────────────────────
    while let Some(msg_result) = ws_receiver.next().await {
        let frame = match msg_result {
            Ok(Message::Binary(b)) => b,
            Ok(Message::Close(_)) | Err(_) => break,
            Ok(_) => continue,
        };

        let pending_req = match session.decrypt_request(&frame) {
            Ok(r) => r,
            Err(e) => {
                log::error!("MWA decrypt error: {}", e);
                break;
            }
        };

        let rpc_id = pending_req.id;
        let request_id = pending_req.request_id.clone();

        // Emit to frontend
        let (tx, rx) = oneshot::channel::<MwaResponse>();
        {
            pending.lock().unwrap().insert(request_id.clone(), tx);
        }
        use tauri::Emitter;
        if let Err(e) = app.emit("mwa://request", &pending_req) {
            log::error!("Failed to emit mwa://request: {}", e);
        }

        // Wait for user response (120s timeout)
        let user_response = match tokio::time::timeout(
            std::time::Duration::from_secs(120),
            rx,
        )
        .await
        {
            Ok(Ok(r)) => r,
            _ => {
                log::warn!("MWA request {} timed out or cancelled", request_id);
                MwaResponse::Rejected
            }
        };

        // Build encrypted response
        let encrypted_frame = match user_response {
            MwaResponse::AuthorizeApproved { auth_token, pubkey_b58 } => {
                let result = build_authorize_result(&auth_token, &pubkey_b58, "XFHotWallet");
                session.encrypt_response(rpc_id, result)?
            }
            MwaResponse::SignApproved { signed_payloads } => {
                let result = build_sign_result(signed_payloads);
                session.encrypt_response(rpc_id, result)?
            }
            MwaResponse::SignAndSendApproved { signatures } => {
                let result = build_sign_and_send_result(signatures);
                session.encrypt_response(rpc_id, result)?
            }
            MwaResponse::Rejected => {
                let (code, msg) = user_declined_error();
                session.encrypt_error(rpc_id, code, msg)?
            }
        };

        if let Err(e) = ws_sender.send(Message::Binary(encrypted_frame)).await {
            log::error!("MWA send error: {}", e);
            break;
        }
    }

    log::info!("MWA session closed");
    Ok(())
}
