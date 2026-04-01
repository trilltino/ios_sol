use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use crate::error::{WalletError, Result};

pub struct NetworkHandler;

impl NetworkHandler {
    /// Fetches the SOL balance for a given public key on Devnet.
    pub fn get_balance(pubkey: &Pubkey) -> Result<f64> {
        let rpc_url = "https://api.devnet.solana.com";
        let client = RpcClient::new(rpc_url);
        
        let lamports = client.get_balance(pubkey)
            .map_err(|e| WalletError::NetworkError(e.to_string()))?;
            
        Ok(lamports as f64 / 1_000_000_000.0)
    }

    /// Fetches the current lamports for a given public key on Devnet.
    pub fn get_lamports(pubkey: &Pubkey) -> Result<u64> {
        let rpc_url = "https://api.devnet.solana.com";
        let client = RpcClient::new(rpc_url);
        
        client.get_balance(pubkey)
            .map_err(|e| WalletError::NetworkError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_get_balance_invalid_pubkey() {
        let pubkey = Pubkey::from_str("11111111111111111111111111111111").unwrap();
        let result = NetworkHandler::get_balance(&pubkey);
        assert!(result.is_ok()); // Balance 0 is OK
    }
}
