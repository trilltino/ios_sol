pub mod error;
pub mod mnemonic;
pub mod account;
pub mod crypto;
pub mod network;
pub mod transaction;
pub mod nft;
pub mod mwa;

pub use error::{WalletError, Result};
pub use mnemonic::MnemonicHandler;
pub use account::AccountHandler;
pub use crypto::EncryptedVault;
pub use network::NetworkHandler;
pub use transaction::TransactionHandler;
pub use nft::{NftHandler, NftMetadata, NftAttribute};
pub use mwa::{MwaSession, MwaPendingRequest, MwaRequest};

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WalletInfo {
    pub pubkey: String,
    pub mnemonic: String,
}

pub fn generate_new_wallet() -> Result<WalletInfo> {
    let mnemonic = MnemonicHandler::generate()?;
    let pubkey = AccountHandler::derive_pubkey(&mnemonic)?;
    
    Ok(WalletInfo {
        pubkey: pubkey.to_string(),
        mnemonic,
    })
}

pub fn import_wallet_from_mnemonic(phrase: &str) -> Result<WalletInfo> {
    let pubkey = AccountHandler::derive_pubkey(phrase)?;
    
    Ok(WalletInfo {
        pubkey: pubkey.to_string(),
        mnemonic: phrase.to_string(),
    })
}

pub fn get_balance(pubkey_str: &str) -> Result<f64> {
    let pubkey = AccountHandler::parse_pubkey(pubkey_str)?;
    NetworkHandler::get_balance(&pubkey)
}

pub fn transfer_sol(mnemonic_phrase: &str, recipient_str: &str, lamports: u64) -> Result<String> {
    let sender = AccountHandler::derive_keypair(mnemonic_phrase)?;
    let recipient = AccountHandler::parse_pubkey(recipient_str)?;
    TransactionHandler::transfer_sol(&sender, &recipient, lamports)
}

pub fn fetch_nfts(pubkey_str: &str) -> anyhow::Result<Vec<NftMetadata>> {
    NftHandler::fetch_nfts(pubkey_str)
}

pub fn fetch_compressed_nfts(pubkey_str: &str) -> anyhow::Result<Vec<NftMetadata>> {
    NftHandler::fetch_compressed_nfts(pubkey_str)
}

pub fn fetch_all_nfts(pubkey_str: &str) -> anyhow::Result<Vec<NftMetadata>> {
    NftHandler::fetch_all_nfts(pubkey_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_new_wallet() {
        let wallet = generate_new_wallet().unwrap();
        assert_eq!(wallet.mnemonic.split_whitespace().count(), 12);
        assert!(wallet.pubkey.len() >= 43 && wallet.pubkey.len() <= 44);
    }
}
