use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{Argon2, Params};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::{rngs::OsRng, RngCore};
use zeroize::Zeroize;
use anyhow::{anyhow, Result};

pub struct EncryptedVault;

impl EncryptedVault {
    /// Encrypts the mnemonic using a key derived from the PIN.
    /// Returns a base64 encoded string containing: [salt(16) + nonce(12) + ciphertext]
    pub fn encrypt(pin: &str, mnemonic: &str) -> Result<String> {
        let mut salt = [0u8; 16];
        OsRng.fill_bytes(&mut salt);

        let key = Self::derive_key(pin, &salt)?;
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|_| anyhow!("Failed to initialize cipher"))?;
        
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, mnemonic.as_bytes())
            .map_err(|e| anyhow!("Encryption failed: {}", e))?;

        let mut combined = Vec::with_capacity(salt.len() + nonce_bytes.len() + ciphertext.len());
        combined.extend_from_slice(&salt);
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);

        Ok(BASE64.encode(combined))
    }

    /// Decrypts the base64 encoded vault using the PIN.
    pub fn decrypt(pin: &str, b64_data: &str) -> Result<String> {
        let combined = BASE64.decode(b64_data)
            .map_err(|e| anyhow!("Failed to decode base64: {}", e))?;

        if combined.len() < 28 {
            return Err(anyhow!("Invalid vault data size"));
        }

        let salt = &combined[0..16];
        let nonce_bytes = &combined[16..28];
        let ciphertext = &combined[28..];

        let key = Self::derive_key(pin, salt)?;
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|_| anyhow!("Failed to initialize cipher"))?;
        
        let nonce = Nonce::from_slice(nonce_bytes);
        let decrypted_bytes = cipher.decrypt(nonce, ciphertext)
            .map_err(|_| anyhow!("Incorrect PIN or corrupted data"))?;

        String::from_utf8(decrypted_bytes)
            .map_err(|_| anyhow!("Decrypted data is not valid UTF-8"))
    }

    /// Derives a 32-byte key from the PIN and salt using Argon2id.
    fn derive_key(pin: &str, salt: &[u8]) -> Result<[u8; 32]> {
        // High-security parameters for PIN derivation
        // m_cost: 16MB, t_cost: 3, p_cost: 4
        let params = Params::new(16384, 3, 4, Some(32))
            .map_err(|e| anyhow!("Invalid argon2 params: {}", e))?;
        
        let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
        
        let mut key = [0u8; 32];
        let mut pin_bytes = pin.as_bytes().to_vec();
        
        argon2.hash_password_into(&pin_bytes, salt, &mut key)
            .map_err(|e| anyhow!("Key derivation failed: {}", e))?;
            
        pin_bytes.zeroize(); // Scrub PIN from memory
        
        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let pin = "123456";
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        
        let encrypted = EncryptedVault::encrypt(pin, mnemonic).unwrap();
        let decrypted = EncryptedVault::decrypt(pin, &encrypted).unwrap();
        
        assert_eq!(mnemonic, decrypted);
    }

    #[test]
    fn test_decrypt_wrong_pin() {
        let pin = "123456";
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let encrypted = EncryptedVault::encrypt(pin, mnemonic).unwrap();
        let wrong_pin = "654321";
        let result = EncryptedVault::decrypt(wrong_pin, &encrypted);
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_invalid_data() {
        let pin = "123456";
        let result = EncryptedVault::decrypt(pin, "invalid_base64_data!");
        assert!(result.is_err());
    }
}
