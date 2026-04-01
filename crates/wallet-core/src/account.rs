use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::signer::SeedDerivable;
use solana_sdk::pubkey::Pubkey;
use crate::error::{WalletError, Result};
use crate::mnemonic::MnemonicHandler;
use std::convert::TryInto;
use std::str::FromStr;

pub struct AccountHandler;

impl AccountHandler {
    /// Derives a Solana Keypair from a mnemonic phrase.
    pub fn derive_keypair(phrase: &str) -> Result<Keypair> {
        let seed_bytes = MnemonicHandler::to_seed(phrase)?;
        let seed_slice = &seed_bytes[0..32];
        let seed_array: [u8; 32] = seed_slice.try_into()
            .map_err(|_| WalletError::KeyDerivation("Failed to convert seed to 32 bytes".to_string()))?;
        
        Keypair::from_seed(&seed_array)
            .map_err(|e| WalletError::KeyDerivation(e.to_string()))
    }

    /// Derives a Solana Public Key from a mnemonic phrase.
    pub fn derive_pubkey(phrase: &str) -> Result<Pubkey> {
        let keypair = Self::derive_keypair(phrase)?;
        Ok(keypair.pubkey())
    }

    /// Validates and parses a Public Key string.
    pub fn parse_pubkey(pubkey_str: &str) -> Result<Pubkey> {
        Pubkey::from_str(pubkey_str)
            .map_err(|e| WalletError::InvalidPubkey(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_keypair() {
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let keypair = AccountHandler::derive_keypair(phrase).unwrap();
        assert_eq!(keypair.pubkey().to_string(), "9R8VbRuiZ3Kzh4zH8Gv5b8r8z7f7p5z7z7z7p5z7z7z7");
    }

    #[test]
    fn test_derive_pubkey() {
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let pubkey = AccountHandler::derive_pubkey(phrase).unwrap();
        assert_eq!(pubkey.to_string(), "9R8VbRuiZ3Kzh4zH8Gv5b8r8z7f7p5z7z7z7p5z7z7z7");
    }

    #[test]
    fn test_parse_invalid_pubkey() {
        let result = AccountHandler::parse_pubkey("invalid_pubkey_here");
        assert!(result.is_err());
    }
}
