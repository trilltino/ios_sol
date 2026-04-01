//! MWA (Mobile Wallet Adapter) Protocol — session types and AES-GCM encryption.
//!
//! This module holds:
//!   - Typed request/response enums (MwaRequest, MwaResponse)
//!   - The MwaSession struct that encrypts/decrypts frames once the ECDH
//!     handshake has already been performed in the ios-app layer
//!   - Helper builders for common response payloads
//!
//! The ECDH P-256 handshake (HELLO_REQ / HELLO_RSP) lives in mwa_server.rs
//! in the ios-app crate because it needs p256/hkdf which conflict with some
//! wallet-core dependencies at the resolver level.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};

// ─── MWA Request types ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppIdentity {
    pub uri: Option<String>,
    pub icon: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizeParams {
    pub cluster: Option<String>,
    pub identity: Option<AppIdentity>,
    pub sign_in_payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReauthorizeParams {
    pub auth_token: String,
    pub identity: Option<AppIdentity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeauthorizeParams {
    pub auth_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignTransactionsParams {
    pub payloads: Vec<String>, // base64-encoded transactions
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignAndSendTransactionsParams {
    pub payloads: Vec<String>,
    pub options: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignMessagesParams {
    pub addresses: Vec<String>,
    pub payloads: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum MwaRequest {
    #[serde(rename = "authorize")]
    Authorize(AuthorizeParams),
    #[serde(rename = "reauthorize")]
    Reauthorize(ReauthorizeParams),
    #[serde(rename = "deauthorize")]
    Deauthorize(DeauthorizeParams),
    #[serde(rename = "sign_transactions")]
    SignTransactions(SignTransactionsParams),
    #[serde(rename = "sign_and_send_transactions")]
    SignAndSendTransactions(SignAndSendTransactionsParams),
    #[serde(rename = "sign_messages")]
    SignMessages(SignMessagesParams),
}

/// A pending MWA request with its JSON-RPC ID (needed to send the response).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MwaPendingRequest {
    pub id: u64,
    pub request_id: String, // UUID for frontend tracking
    pub request: MwaRequest,
}

// ─── JSON-RPC frame types ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    pub id: u64,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    pub id: u64,
    pub result: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    pub jsonrpc: &'static str,
    pub id: u64,
    pub error: JsonRpcErrorObj,
}

#[derive(Debug, Serialize)]
struct JsonRpcErrorObj {
    pub code: i32,
    pub message: String,
}

// ─── Constants ────────────────────────────────────────────────────────────────

const SEQUENCE_NUMBER_BYTES: usize = 4;
const AES_GCM_NONCE_LEN: usize = 12;

// ─── Session (AES-GCM encrypt/decrypt once key is established) ────────────────

/// AES-256-GCM encrypted MWA session.
/// The 32-byte `aes_key` is derived externally via ECDH+HKDF (in mwa_server.rs).
pub struct MwaSession {
    cipher: Aes256Gcm,
    inbound_seq: AtomicU32,
    outbound_seq: AtomicU32,
}

impl MwaSession {
    /// Create a session from a pre-derived 32-byte AES key.
    pub fn from_key(aes_key: &[u8; 32]) -> Result<Self> {
        let key = Key::<Aes256Gcm>::from_slice(aes_key);
        let cipher = Aes256Gcm::new(key);
        Ok(Self {
            cipher,
            inbound_seq: AtomicU32::new(0),
            outbound_seq: AtomicU32::new(0),
        })
    }

    /// Decrypt an incoming encrypted JSON-RPC frame.
    /// Frame format: [4-byte big-endian seq# | 12-byte nonce | ciphertext+tag]
    pub fn decrypt_request(&self, frame: &[u8]) -> Result<MwaPendingRequest> {
        if frame.len() < SEQUENCE_NUMBER_BYTES + AES_GCM_NONCE_LEN {
            return Err(anyhow!("Frame too short: {} bytes", frame.len()));
        }

        let seq_bytes = &frame[..SEQUENCE_NUMBER_BYTES];
        let seq = u32::from_be_bytes([seq_bytes[0], seq_bytes[1], seq_bytes[2], seq_bytes[3]]);

        let expected = self.inbound_seq.load(Ordering::SeqCst) + 1;
        if seq != expected {
            return Err(anyhow!("Sequence mismatch: got {}, expected {}", seq, expected));
        }
        self.inbound_seq.store(seq, Ordering::SeqCst);

        let nonce_bytes = &frame[SEQUENCE_NUMBER_BYTES..SEQUENCE_NUMBER_BYTES + AES_GCM_NONCE_LEN];
        let ciphertext = &frame[SEQUENCE_NUMBER_BYTES + AES_GCM_NONCE_LEN..];

        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| anyhow!("AES-GCM decryption failed"))?;

        let rpc_req: JsonRpcRequest = serde_json::from_slice(&plaintext)
            .map_err(|e| anyhow!("JSON-RPC parse failed: {}", e))?;

        let mwa_request = Self::parse_rpc_request(&rpc_req)?;
        let request_id = format!("{}-{}", rpc_req.id, seq);

        Ok(MwaPendingRequest {
            id: rpc_req.id,
            request_id,
            request: mwa_request,
        })
    }

    /// Encrypt a JSON-RPC success response.
    pub fn encrypt_response(&self, rpc_id: u64, result: serde_json::Value) -> Result<Vec<u8>> {
        let response = JsonRpcResponse {
            jsonrpc: "2.0",
            id: rpc_id,
            result,
        };
        let plaintext = serde_json::to_vec(&response)
            .map_err(|e| anyhow!("Serialization failed: {}", e))?;
        self.encrypt_frame(&plaintext)
    }

    /// Encrypt a JSON-RPC error response.
    pub fn encrypt_error(&self, rpc_id: u64, code: i32, message: &str) -> Result<Vec<u8>> {
        let error = JsonRpcError {
            jsonrpc: "2.0",
            id: rpc_id,
            error: JsonRpcErrorObj {
                code,
                message: message.to_string(),
            },
        };
        let plaintext = serde_json::to_vec(&error)
            .map_err(|e| anyhow!("Serialization failed: {}", e))?;
        self.encrypt_frame(&plaintext)
    }

    fn encrypt_frame(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let seq = self.outbound_seq.fetch_add(1, Ordering::SeqCst) + 1;
        let mut nonce_bytes = [0u8; AES_GCM_NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| anyhow!("AES-GCM encryption failed"))?;

        let mut frame =
            Vec::with_capacity(SEQUENCE_NUMBER_BYTES + AES_GCM_NONCE_LEN + ciphertext.len());
        frame.extend_from_slice(&seq.to_be_bytes());
        frame.extend_from_slice(&nonce_bytes);
        frame.extend_from_slice(&ciphertext);
        Ok(frame)
    }

    fn parse_rpc_request(rpc: &JsonRpcRequest) -> Result<MwaRequest> {
        let params = rpc.params.clone().unwrap_or(serde_json::Value::Null);
        match rpc.method.as_str() {
            "authorize" => Ok(MwaRequest::Authorize(
                serde_json::from_value(params).map_err(|e| anyhow!("authorize params: {}", e))?,
            )),
            "reauthorize" => Ok(MwaRequest::Reauthorize(
                serde_json::from_value(params)
                    .map_err(|e| anyhow!("reauthorize params: {}", e))?,
            )),
            "deauthorize" => Ok(MwaRequest::Deauthorize(
                serde_json::from_value(params)
                    .map_err(|e| anyhow!("deauthorize params: {}", e))?,
            )),
            "sign_transactions" => Ok(MwaRequest::SignTransactions(
                serde_json::from_value(params)
                    .map_err(|e| anyhow!("sign_transactions params: {}", e))?,
            )),
            "sign_and_send_transactions" => Ok(MwaRequest::SignAndSendTransactions(
                serde_json::from_value(params)
                    .map_err(|e| anyhow!("sign_and_send_transactions params: {}", e))?,
            )),
            "sign_messages" => Ok(MwaRequest::SignMessages(
                serde_json::from_value(params)
                    .map_err(|e| anyhow!("sign_messages params: {}", e))?,
            )),
            other => Err(anyhow!("Unknown MWA method: {}", other)),
        }
    }
}

// ─── Response builders ─────────────────────────────────────────────────────────

pub fn build_authorize_result(auth_token: &str, public_key_b58: &str, label: &str) -> serde_json::Value {
    serde_json::json!({
        "auth_token": auth_token,
        "accounts": [{
            "address": public_key_b58,
            "display_address": public_key_b58,
            "label": label,
            "chains": ["solana:mainnet"],
            "features": [
                "solana:signTransaction",
                "solana:signAndSendTransaction",
                "solana:signMessage"
            ]
        }],
        "wallet_uri_base": null
    })
}

pub fn build_sign_result(signed_payloads: Vec<Vec<u8>>) -> serde_json::Value {
    let encoded: Vec<String> = signed_payloads
        .into_iter()
        .map(|tx| BASE64.encode(tx))
        .collect();
    serde_json::json!({ "signed_payloads": encoded })
}

pub fn build_sign_and_send_result(signatures: Vec<String>) -> serde_json::Value {
    serde_json::json!({ "signatures": signatures })
}

pub fn user_declined_error() -> (i32, &'static str) {
    (-32601, "User declined the request")
}
