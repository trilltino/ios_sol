# XFHotWallet - Solana iOS Wallet


<img width="490" height="974" alt="Screenshot 2026-04-02 004329" src="https://github.com/user-attachments/assets/b37a9cef-5f9d-4342-89d5-06052c9a6138" />

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

## How It Works on iOS & Security Profile
This wallet is purpose-built to package directly to iOS runtimes utilizing the Tauri `mobile` targets. 

### iOS Security Model:
1. **Biometric Enclaves:** Leverages `tauri-plugin-biometric` hooked tightly into iOS FaceID/TouchID native prompts to pause all transaction signing until the physical owner is verified.
2. **Encrypted Vaults:** Utilizes AES-256-GCM encryption with Argon2id Key Derivation. Currently backed by the local file system; architecturally prepared to push bytes directly into the **Apple Secure Enclave** via Keychain APIs for ultimate hardware-level immutability.
3. **App Sandboxing:** Native WKWebView execution isolates the JavaScript bundle from the Rust layer. The mnemonic seed is held exclusively in Rust-managed memory and never passed exposed to the DOM overlay.

### Deploying to an iOS Device
Tauri v2 brings native iOS generation natively out-of-the-box:
1. Initialize the Xcode project: `npm run tauri ios init`
2. Connect your iPhone or start a Simulator.
3. Boot the environment natively: `npm run tauri ios dev`

## Development and Building (PC / Desktop)

You can run and test this entire wallet seamlessly on Windows, macOS, or Linux! Ensure you have the standard Rust toolchain (`stable`) and Node.js installed.

1. Navigate to the `crates/ios-app` directory.
2. Install core Node dependencies via `npm install`.
3. Run the development environment with `npm run tauri dev`.
*(A native desktop window will appear simulating the mobile viewport).*

### Docker Clean-Room Builds
For developers who want to compile the environment in an entirely clean setup without installing local dependencies, a `Dockerfile` has been included.
1. Build the image: `docker build -t ios-sol-builder .`
2. Run the build container: `docker run -v $(pwd)/target:/app/target ios-sol-builder`

### Testing dApp Interoperability (MWA/Desktop Simulation)
1. Keep the Tauri application running.
2. Navigate to the root `test-dapp/` directory.
3. Run `node simulate_dapp.mjs`.
4. Approve the prompt in the Wallet UI and watch the encrypted payloads pass seamlessly between the script and the wallet server!
