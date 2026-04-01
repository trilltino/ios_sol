use wallet_core::{self, WalletInfo, EncryptedVault, NftMetadata};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use tauri::Manager;

mod biometric_vault;
mod mwa_server;
mod mwa_deep_link;

use mwa_server::{PendingResponseMap, MwaResponse};

// ─── Global state for MWA pending request responses ──────────────────────────

struct MwaState {
    pending: PendingResponseMap,
}

// ─── Vault helpers ────────────────────────────────────────────────────────────

fn get_vault_path(app_handle: tauri::AppHandle) -> PathBuf {
    let app_dir = app_handle.path().app_data_dir().expect("Failed to get app data dir");
    if !app_dir.exists() {
        fs::create_dir_all(&app_dir).expect("Failed to create app data dir");
    }
    app_dir.join("vault.enc")
}

// ─── Wallet Commands ──────────────────────────────────────────────────────────

#[tauri::command]
async fn generate_wallet() -> Result<WalletInfo, String> {
    wallet_core::generate_new_wallet().map_err(|e| e.to_string())
}

#[tauri::command]
async fn import_wallet(phrase: String) -> Result<WalletInfo, String> {
    wallet_core::import_wallet_from_mnemonic(&phrase).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_sol_balance(pubkey: String) -> Result<f64, String> {
    wallet_core::get_balance(&pubkey).map_err(|e| e.to_string())
}

// ─── Vault Commands ───────────────────────────────────────────────────────────

#[tauri::command]
async fn vault_exists(app_handle: tauri::AppHandle) -> bool {
    get_vault_path(app_handle).exists()
}

#[tauri::command]
async fn save_to_vault(app_handle: tauri::AppHandle, pin: String, mnemonic: String) -> Result<(), String> {
    let encrypted = EncryptedVault::encrypt(&pin, &mnemonic).map_err(|e| e.to_string())?;
    let path = get_vault_path(app_handle);
    fs::write(path, encrypted).map_err(|e| format!("Failed to write vault: {}", e))
}

#[tauri::command]
async fn unlock_from_vault(app_handle: tauri::AppHandle, pin: String) -> Result<WalletInfo, String> {
    let path = get_vault_path(app_handle);
    if !path.exists() {
        return Err("No vault found".to_string());
    }
    
    let encrypted_b64 = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mnemonic = EncryptedVault::decrypt(&pin, &encrypted_b64).map_err(|e| e.to_string())?;
    wallet_core::import_wallet_from_mnemonic(&mnemonic).map_err(|e| e.to_string())
}

#[tauri::command]
async fn reset_vault(app_handle: tauri::AppHandle) -> Result<(), String> {
    let path = get_vault_path(app_handle);
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn send_sol(app_handle: tauri::AppHandle, pin: String, recipient: String, amount: f64) -> Result<String, String> {
    let path = get_vault_path(app_handle);
    if !path.exists() {
        return Err("No vault found".to_string());
    }
    
    let encrypted_b64 = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mnemonic = EncryptedVault::decrypt(&pin, &encrypted_b64).map_err(|e| e.to_string())?;
    let lamports = (amount * 1_000_000_000.0) as u64;
    wallet_core::transfer_sol(&mnemonic, &recipient, lamports).map_err(|e| e.to_string())
}

// ─── Biometric Commands ───────────────────────────────────────────────────────

#[tauri::command]
async fn biometric_status(app_handle: tauri::AppHandle) -> biometric_vault::BiometricStatus {
    biometric_vault::check_biometric_status(&app_handle).await
}

/// Attempt FaceID/TouchID, returns true if passed (user should then enter PIN).
/// On desktop always returns false (falls through to PIN).
#[tauri::command]
async fn biometric_authenticate(app_handle: tauri::AppHandle) -> Result<bool, String> {
    biometric_vault::authenticate_biometric(&app_handle, "Unlock XFHotWallet").await
}

/// Biometric-gated vault unlock: biometric first, then PIN to decrypt.
/// On desktop this is identical to unlock_from_vault (PIN only).
#[tauri::command]
async fn biometric_unlock(
    app_handle: tauri::AppHandle,
    pin: String,
    biometric_passed: bool,
) -> Result<WalletInfo, String> {
    // On iOS, biometric_passed must be true (set by the frontend after
    // calling biometric_authenticate). On desktop it's always false and
    // we skip the check.
    #[cfg(mobile)]
    if !biometric_passed {
        return Err("Biometric authentication required".to_string());
    }

    // PIN decrypts the vault (same as regular unlock)
    unlock_from_vault(app_handle, pin).await
}

// ─── NFT Commands ─────────────────────────────────────────────────────────────

#[tauri::command]
async fn get_nfts(pubkey: String) -> Result<Vec<NftMetadata>, String> {
    tokio::task::spawn_blocking(move || {
        wallet_core::fetch_nfts(&pubkey)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_compressed_nfts(pubkey: String) -> Result<Vec<NftMetadata>, String> {
    tokio::task::spawn_blocking(move || {
        wallet_core::fetch_compressed_nfts(&pubkey)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_all_nfts(pubkey: String) -> Result<Vec<NftMetadata>, String> {
    tokio::task::spawn_blocking(move || {
        wallet_core::fetch_all_nfts(&pubkey)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ─── MWA Commands ─────────────────────────────────────────────────────────────

/// Approve an `authorize` request from a dApp.
/// `request_id` is the unique ID emitted with the `mwa://request` event.
#[tauri::command]
async fn mwa_approve_authorization(
    app_handle: tauri::AppHandle,
    request_id: String,
    pubkey_b58: String,
) -> Result<(), String> {
    let state = app_handle.state::<MwaState>();
    let mut map = state.pending.lock().unwrap();
    if let Some(tx) = map.remove(&request_id) {
        // Generate a simple auth token (in production this would be a signed JWT)
        let auth_token = format!("xfw-auth-{}", &pubkey_b58[..8]);
        let _ = tx.send(MwaResponse::AuthorizeApproved {
            auth_token,
            pubkey_b58,
        });
        Ok(())
    } else {
        Err(format!("No pending request with id: {}", request_id))
    }
}

/// Reject any pending MWA request (user declined).
#[tauri::command]
async fn mwa_reject_request(
    app_handle: tauri::AppHandle,
    request_id: String,
) -> Result<(), String> {
    let state = app_handle.state::<MwaState>();
    let mut map = state.pending.lock().unwrap();
    if let Some(tx) = map.remove(&request_id) {
        let _ = tx.send(MwaResponse::Rejected);
        Ok(())
    } else {
        Err(format!("No pending request with id: {}", request_id))
    }
}

/// Approve a `sign_transactions` or `sign_and_send_transactions` request.
/// The PIN is used to decrypt the vault and retrieve the signing keypair.
#[tauri::command]
async fn mwa_approve_sign_transactions(
    app_handle: tauri::AppHandle,
    request_id: String,
    pin: String,
    tx_payloads_b64: Vec<String>,          // base64-encoded tx bytes from dApp
    send_to_network: bool,
) -> Result<(), String> {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

    // Decrypt vault to get mnemonic
    let path = get_vault_path(app_handle.clone());
    if !path.exists() {
        return Err("No vault found".to_string());
    }
    let encrypted_b64 = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mnemonic = EncryptedVault::decrypt(&pin, &encrypted_b64).map_err(|e| e.to_string())?;

    let state = app_handle.state::<MwaState>();
    
    if send_to_network {
        // sign_and_send_transactions: sign each tx and broadcast
        let mut signatures = Vec::new();
        for payload_b64 in &tx_payloads_b64 {
            let tx_bytes = BASE64.decode(payload_b64).map_err(|e| e.to_string())?;
            // Deserialize as VersionedTransaction and submit
            let sig = tokio::task::spawn_blocking({
                let mnemonic = mnemonic.clone();
                let tx_bytes = tx_bytes.clone();
                move || sign_and_send_transaction(&mnemonic, &tx_bytes)
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
            signatures.push(sig);
        }

        let mut map = state.pending.lock().unwrap();
        if let Some(tx) = map.remove(&request_id) {
            let _ = tx.send(MwaResponse::SignAndSendApproved { signatures });
        }
    } else {
        // sign_transactions: sign each tx and return signed bytes
        let mut signed_payloads = Vec::new();
        for payload_b64 in &tx_payloads_b64 {
            let tx_bytes = BASE64.decode(payload_b64).map_err(|e| e.to_string())?;
            let signed = tokio::task::spawn_blocking({
                let mnemonic = mnemonic.clone();
                let tx_bytes = tx_bytes.clone();
                move || sign_transaction(&mnemonic, &tx_bytes)
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
            signed_payloads.push(signed);
        }

        let mut map = state.pending.lock().unwrap();
        if let Some(tx) = map.remove(&request_id) {
            let _ = tx.send(MwaResponse::SignApproved { signed_payloads });
        }
    }

    Ok(())
}

// ─── Transaction signing helpers ──────────────────────────────────────────────

fn sign_transaction(mnemonic: &str, tx_bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    use solana_sdk::{
        signature::Signer,
        transaction::VersionedTransaction,
    };
    use wallet_core::AccountHandler;

    let keypair = AccountHandler::derive_keypair(mnemonic)?;
    let mut tx: VersionedTransaction = bincode::deserialize(tx_bytes)
        .map_err(|e| anyhow::anyhow!("Failed to deserialize tx: {}", e))?;

    // Sign the transaction
    tx.signatures[0] = keypair.sign_message(&tx.message.serialize());
    let signed_bytes = bincode::serialize(&tx)
        .map_err(|e| anyhow::anyhow!("Failed to serialize signed tx: {}", e))?;
    Ok(signed_bytes)
}

fn sign_and_send_transaction(mnemonic: &str, tx_bytes: &[u8]) -> anyhow::Result<String> {
    use solana_client::rpc_client::RpcClient;
    use solana_sdk::{
        commitment_config::CommitmentConfig,
        signature::Signer,
        transaction::VersionedTransaction,
    };
    use wallet_core::AccountHandler;

    let keypair = AccountHandler::derive_keypair(mnemonic)?;
    let mut tx: VersionedTransaction = bincode::deserialize(tx_bytes)
        .map_err(|e| anyhow::anyhow!("Failed to deserialize tx: {}", e))?;

    tx.signatures[0] = keypair.sign_message(&tx.message.serialize());

    let rpc = RpcClient::new_with_commitment(
        format!(
            "https://mainnet.helius-rpc.com/?api-key={}",
            "5bb5fed2-8d33-458b-b7d2-3d18fdbb3da5"
        ),
        CommitmentConfig::confirmed(),
    );

    let sig = rpc
        .send_and_confirm_transaction(&tx)
        .map_err(|e| anyhow::anyhow!("Send tx failed: {}", e))?;

    Ok(sig.to_string())
}

// ─── App entry point ──────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // MWA pending response map — shared between server and command handlers
    let pending: PendingResponseMap = Arc::new(Mutex::new(HashMap::new()));

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_deep_link::init())
        .manage(MwaState {
            pending: pending.clone(),
        });

    #[cfg(mobile)]
    {
        builder = builder.plugin(tauri_plugin_biometric::init());
    }

    builder
        .setup(move |app| {
            let app_handle = app.handle().clone();

            // Start MWA WebSocket server
            let pending_clone = pending.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = mwa_server::start_mwa_server(app_handle.clone(), pending_clone).await {
                    log::error!("MWA server error: {}", e);
                }
            });

            // Register deep link handler (iOS MWA entry point)
            mwa_deep_link::init_deep_link_handler(app.handle());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Wallet
            generate_wallet,
            import_wallet,
            get_sol_balance,
            // Vault
            vault_exists,
            save_to_vault,
            unlock_from_vault,
            reset_vault,
            send_sol,
            // Biometrics
            biometric_status,
            biometric_authenticate,
            biometric_unlock,
            // NFTs
            get_nfts,
            get_compressed_nfts,
            get_all_nfts,
            // MWA
            mwa_approve_authorization,
            mwa_reject_request,
            mwa_approve_sign_transactions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
