//! Biometric vault module.
//!
//! Security model:
//!   - iOS: FaceID/TouchID authentication FIRST (via tauri-plugin-biometric).
//!     On success, the mnemonic is retrieved from a Stronghold vault (encrypted
//!     at rest with Argon2id(PIN)).
//!   - Desktop/Fallback: PIN-only via the existing EncryptedVault (AES-256-GCM).
//!
//! The user experience is:
//!   1. App opens → check biometric availability
//!   2a. Biometric available → show FaceID/TouchID button → on success, prompt
//!       for PIN to decrypt Stronghold (or skip PIN if store uses device cred)
//!   2b. No biometric → show PIN pad directly
//!
//! For the initial implementation, we use a simplified secure storage:
//!   - Stronghold is initialized with Argon2(PIN) on all platforms
//!   - On iOS the biometric gate must pass before the PIN prompt appears
//!   - The vault stores the mnemonic as a secret in the Stronghold client

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiometricStatus {
    pub is_available: bool,
    pub biometry_type: String, // "FaceID", "TouchID", "None"
    pub error: Option<String>,
}

/// Check whether biometrics are available on this device.
/// On desktop this always returns unavailable (biometric plugin is mobile-only).
#[cfg(mobile)]
pub async fn check_biometric_status<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> BiometricStatus {
    use tauri_plugin_biometric::BiometricExt;

    match app.biometric().status() {
        Ok(status) => {
            let biometry_type = match status.biometry_type {
                tauri_plugin_biometric::BiometryType::FaceID => "FaceID".to_string(),
                tauri_plugin_biometric::BiometryType::TouchID => "TouchID".to_string(),
                tauri_plugin_biometric::BiometryType::None => "None".to_string(),
            };
            BiometricStatus {
                is_available: status.is_available,
                biometry_type,
                error: status.error,
            }
        }
        Err(e) => BiometricStatus {
            is_available: false,
            biometry_type: "None".to_string(),
            error: Some(e.to_string()),
        },
    }
}

#[cfg(not(mobile))]
pub async fn check_biometric_status<R: tauri::Runtime>(
    _app: &tauri::AppHandle<R>,
) -> BiometricStatus {
    // Desktop: biometrics not available — user will always use PIN
    BiometricStatus {
        is_available: false,
        biometry_type: "None".to_string(),
        error: None,
    }
}

/// Attempt biometric authentication.
/// Returns `Ok(true)` if authentication succeeded, `Ok(false)` if the user
/// cancelled, `Err` on system error.
#[cfg(mobile)]
pub async fn authenticate_biometric<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    reason: &str,
) -> Result<bool, String> {
    use tauri_plugin_biometric::{AuthOptions, BiometricExt};

    let options = AuthOptions {
        allow_device_credential: true, // fallback to device passcode if biometric fails
        cancel_title: Some("Use PIN instead".to_string()),
        fallback_title: Some("Enter PIN".to_string()),
        ..Default::default()
    };

    match app.biometric().authenticate(reason.to_string(), options) {
        Ok(()) => Ok(true),
        Err(e) => {
            let msg = e.to_string();
            // User cancelled / chose fallback — not a hard error
            if msg.contains("cancel") || msg.contains("fallback") || msg.contains("passcode") {
                Ok(false)
            } else {
                Err(msg)
            }
        }
    }
}

#[cfg(not(mobile))]
pub async fn authenticate_biometric<R: tauri::Runtime>(
    _app: &tauri::AppHandle<R>,
    _reason: &str,
) -> Result<bool, String> {
    // Desktop: always report biometric as unavailable — caller should use PIN
    Ok(false)
}
