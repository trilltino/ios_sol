use bip39::{Mnemonic, Language};
use crate::error::{WalletError, Result};

pub struct MnemonicHandler;

impl MnemonicHandler {
    /// Generates a new 12-word mnemonic phrase in English.
    pub fn generate() -> Result<String> {
        let mnemonic = Mnemonic::generate_in(Language::English, 12)
            .map_err(|e| WalletError::MnemonicGeneration(e.to_string()))?;
        Ok(mnemonic.to_string())
    }

    /// Validates and parses a mnemonic phrase.
    pub fn parse(phrase: &str) -> Result<Mnemonic> {
        Mnemonic::parse_in(Language::English, phrase)
            .map_err(|e| WalletError::InvalidMnemonic(e.to_string()))
    }

    /// Converts a mnemonic to a 64-byte seed.
    pub fn to_seed(phrase: &str) -> Result<[u8; 64]> {
        let mnemonic = Self::parse(phrase)?;
        Ok(mnemonic.to_seed(""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_mnemonic() {
        let phrase = MnemonicHandler::generate().unwrap();
        let words: Vec<&str> = phrase.split_whitespace().collect();
        assert_eq!(words.len(), 12);
    }

    #[test]
    fn test_parse_invalid_mnemonic() {
        let result = MnemonicHandler::parse("invalid phrase here");
        assert!(result.is_err());
    }

    #[test]
    fn test_to_seed() {
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let seed = MnemonicHandler::to_seed(phrase).unwrap();
        assert_eq!(seed.len(), 64);
    }
}
