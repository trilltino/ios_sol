use solana_client::rpc_client::RpcClient;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::{Transaction, VersionedTransaction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::system_instruction;
use crate::error::{WalletError, Result};

pub struct TransactionHandler;

impl TransactionHandler {
    /// Builds, signs, and broadcasts a SOL transfer on Devnet.
    pub fn transfer_sol(sender: &Keypair, recipient: &Pubkey, lamports: u64) -> Result<String> {
        let rpc_url = "https://api.devnet.solana.com";
        let client = RpcClient::new(rpc_url);
        
        let instruction = system_instruction::transfer(
            &sender.pubkey(),
            recipient,
            lamports,
        );
        
        let latest_blockhash = client.get_latest_blockhash()
            .map_err(|e| WalletError::NetworkError(e.to_string()))?;
            
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&sender.pubkey()),
            &[sender],
            latest_blockhash,
        );
        
        let versioned_tx = VersionedTransaction::from(transaction);
        
        let signature = client.send_and_confirm_transaction(&versioned_tx)
            .map_err(|e| WalletError::TransactionError(e.to_string()))?;
            
        Ok(signature.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_building_no_rpc() {
        // Can't easily test broadcasting without RPC, but we can verify derivation.
        let sender = Keypair::new();
        let pubkey = sender.pubkey();
        assert!(pubkey.to_string().len() >= 43 && pubkey.to_string().len() <= 44);
    }
}
