use wallet_core::{self, WalletInfo, EncryptedVault, MnemonicHandler, AccountHandler};

#[test]
fn test_wallet_full_flow() {
    // 1. Generate Wallet
    let wallet = wallet_core::generate_new_wallet().expect("Failed to generate wallet");
    assert_eq!(wallet.mnemonic.split_whitespace().count(), 12);

    // 2. Derive Pubkey manually
    let derived_pubkey = AccountHandler::derive_pubkey(&wallet.mnemonic).expect("Failed to derive pubkey");
    assert_eq!(wallet.pubkey, derived_pubkey.to_string());

    // 3. Encrypt with PIN
    let pin = "123456";
    let encrypted = EncryptedVault::encrypt(pin, &wallet.mnemonic).expect("Failed to encrypt");
    
    // 4. Decrypt with PIN
    let decrypted_mnemonic = EncryptedVault::decrypt(pin, &encrypted).expect("Failed to decrypt");
    assert_eq!(wallet.mnemonic, decrypted_mnemonic);

    // 5. Derive Keypair from decrypted mnemonic
    let keypair = AccountHandler::derive_keypair(&decrypted_mnemonic).expect("Failed to derive keypair");
    assert_eq!(wallet.pubkey, keypair.pubkey().to_string());
}

#[test]
fn test_invalid_pin_failure() {
    let wallet = wallet_core::generate_new_wallet().unwrap();
    let pin = "111111";
    let encrypted = EncryptedVault::encrypt(pin, &wallet.mnemonic).unwrap();
    
    // Try wrong PIN
    let result = EncryptedVault::decrypt("wrong_pin", &encrypted);
    assert!(result.is_err());
}

#[test]
fn test_import_validation() {
    let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let result = wallet_core::import_wallet_from_mnemonic(phrase);
    assert!(result.is_ok());
    
    let invalid_phrase = "invalid phrase here";
    let result = wallet_core::import_wallet_from_mnemonic(invalid_phrase);
    assert!(result.is_err());
}
