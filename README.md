# XFHotWallet - Solana iOS Wallet

An open-source, non-custodial Solana hot wallet built specifically for iOS utilizing the Tauri framework. This repository serves as a secure reference implementation for mobile-first Solana interactions, prioritizing robust key management, sleek UI, and seamless dApp bridging.

## Architecture

The project is structured as a monorepo containing two primary layers:

### 1. `wallet-core` (Rust)
A dedicated, platform-agnostic Rust library isolating all blockchain and cryptographic operations.
- **BIP39 Mnemonic Management & Key Derivation:** Secure extraction of Solana ed25519 Keypairs and Addresses.
- **Hardware-Grade Encryption:** EncryptedVault securing data at rest using AES-256-GCM and Argon2id.
- **NFT & cNFT Loading:** Integrated Metaplex PDA decoding and Helius DAS endpoints to correctly parse and fetch standard and compressed NFTs.
- **Web3 Networking:** Broadcasts transactions natively using the Solana Devnet via RPC.

### 2. `ios-app` (React/TypeScript + Tauri)
A highly responsive mobile-focused frontend built over Tauri, wrapped in a premium Glassmorphism aesthetic.
- **Stateful UI Flow:** Biometric unlocking, MWA popups, NFT galleries, and seamless transaction flows.
- **Mobile Wallet Adapter (MWA) Server:** Uses a robust localhost WebSocket (`ws://localhost:44444`) to provide native ECDH P-256 handshakes and session-encrypted tunnels via `ring`, enabling direct bridging for mobile dApps.
- **Biometric Integration:** Employs `tauri-plugin-biometric` to enforce FaceID/TouchID checks before accessing sensitive wallet capabilities.

## Core Features
*   **Premium Glassmorphism UX:** A modern, slick interface with soft transitions and blurred UI cards.
*   **Create or Import Wallets:** Manage non-custodial wallets effortlessly via 12-word mnemonics.
*   **Helius DAS NFTs:** Full visualization support for both Standard NFTs (Token Metadata) and Compressed NFTs (cNFTs).
*   **MWA dApp Connectivity:** Connect, Authorize, and Sign transactions gracefully with any Mobile Web3 app across the Solana ecosystem using standard ECDH handshakes.
*   **Biometric Vaults:** Vault encryption with PIN overlay and biometric gating before transactions.

## Development and Building

Ensure that you have the standard Rust toolchain (`stable`) and Node.js installed.

To spin up the local development interface:
1. Navigate to the `crates/ios-app` directory.
2. Install core Node dependencies via `npm install`.
3. Run the development environment with `npm run tauri dev`.

### Testing dApp Interoperability (MWA)
1. Keep the Tauri application running.
2. Navigate to the root `test-dapp/` directory.
3. Run `node simulate_dapp.mjs`.
4. Approve the prompt in the Wallet UI and watch the encrypted payloads pass seamlessly between the script and the wallet server!
