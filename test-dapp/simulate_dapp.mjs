#!/usr/bin/env node
/**
 * XFHotWallet — MWA Desktop Test dApp
 *
 * This script simulates a dApp connecting to the wallet via the local MWA
 * WebSocket protocol (ws://localhost:44444/solana-wallet).
 *
 * Run the Tauri app first, then:
 *   node simulate_dapp.mjs              # full flow (authorize → sign)
 *   node simulate_dapp.mjs --auth-only  # only test authorization
 *   node simulate_dapp.mjs --sign       # authorize then sign a test transfer
 *
 * This uses raw WebSocket + the MWA protocol crypto directly (no React Native
 * wrappers) so it works on any Node.js >= 18 desktop environment.
 */

import { WebSocket } from 'ws';
import { webcrypto } from 'crypto';
import { PublicKey, SystemProgram, Transaction, Connection } from '@solana/web3.js';

// Polyfill Web Crypto for Node
globalThis.crypto = webcrypto;

const WALLET_WS = 'ws://localhost:44444/solana-wallet';
const MWA_PROTOCOL = 'com.solana.mobilewalletadapter.v1';

// ── Crypto helpers (mirrors the MWA protocol JS src) ──────────────────────────

async function generateECDHKeypair() {
  return await crypto.subtle.generateKey(
    { name: 'ECDH', namedCurve: 'P-256' },
    true,
    ['deriveKey', 'deriveBits']
  );
}

async function exportPublicKeyUncompressed(publicKey) {
  const raw = await crypto.subtle.exportKey('raw', publicKey);
  return new Uint8Array(raw); // 65 bytes uncompressed P-256
}

async function deriveSharedSecret(privateKey, peerPublicKeyBytes) {
  const peerKey = await crypto.subtle.importKey(
    'raw',
    peerPublicKeyBytes,
    { name: 'ECDH', namedCurve: 'P-256' },
    false,
    []
  );
  const sharedBits = await crypto.subtle.deriveBits(
    { name: 'ECDH', public: peerKey },
    privateKey,
    256
  );

  // HKDF-SHA256 → 32-byte AES key (same as Rust: expand label = "mobile_wallet_adapter_aes")
  const hkdfKey = await crypto.subtle.importKey('raw', sharedBits, 'HKDF', false, ['deriveKey']);
  return await crypto.subtle.deriveKey(
    {
      name: 'HKDF',
      hash: 'SHA-256',
      salt: new Uint8Array(0),
      info: new TextEncoder().encode('mobile_wallet_adapter_aes'),
    },
    hkdfKey,
    { name: 'AES-GCM', length: 256 },
    false,
    ['encrypt', 'decrypt']
  );
}

async function encryptFrame(aesKey, payload, seqNum) {
  const nonce = crypto.getRandomValues(new Uint8Array(12));
  const ciphertext = new Uint8Array(
    await crypto.subtle.encrypt({ name: 'AES-GCM', iv: nonce }, aesKey, payload)
  );
  const frame = new Uint8Array(4 + 12 + ciphertext.length);
  new DataView(frame.buffer).setUint32(0, seqNum, false); // big-endian
  frame.set(nonce, 4);
  frame.set(ciphertext, 16);
  return frame;
}

async function decryptFrame(aesKey, frameBytes) {
  const seq = new DataView(frameBytes.buffer, frameBytes.byteOffset, 4).getUint32(0, false);
  const nonce = frameBytes.slice(4, 16);
  const ciphertext = frameBytes.slice(16);
  const plaintext = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: nonce }, aesKey, ciphertext);
  return { seq, json: JSON.parse(new TextDecoder().decode(plaintext)) };
}

// ── Session class ──────────────────────────────────────────────────────────────

class MWASession {
  constructor(ws, aesKey) {
    this.ws = ws;
    this.aesKey = aesKey;
    this.outSeq = 0;
    this.nextId = 1;
    this._pending = {};

    ws.on('message', async (data) => {
      const frame = new Uint8Array(data.buffer || data);
      const { seq, json } = await decryptFrame(aesKey, frame);
      console.log(`  ← [seq=${seq}] ${JSON.stringify(json).slice(0, 120)}`);
      const handler = this._pending[json.id];
      if (handler) {
        delete this._pending[json.id];
        if (json.error) handler.reject(new Error(`${json.error.code}: ${json.error.message}`));
        else handler.resolve(json.result);
      }
    });
  }

  async send(method, params) {
    const id = this.nextId++;
    this.outSeq++;
    const payload = new TextEncoder().encode(
      JSON.stringify({ jsonrpc: '2.0', id, method, params: params ?? {} })
    );
    const frame = await encryptFrame(this.aesKey, payload, this.outSeq);
    this.ws.send(frame);
    console.log(`  → [seq=${this.outSeq}] ${method}`);
    return new Promise((resolve, reject) => {
      this._pending[id] = { resolve, reject };
    });
  }
}

// ── Connection establishment ───────────────────────────────────────────────────

async function connect() {
  console.log('\n🔌 Connecting to wallet at', WALLET_WS);
  const ws = await new Promise((resolve, reject) => {
    const socket = new WebSocket(WALLET_WS, [MWA_PROTOCOL]);
    socket.binaryType = 'arraybuffer';
    socket.on('open', () => resolve(socket));
    socket.on('error', reject);
    setTimeout(() => reject(new Error('Connection timeout — is the Tauri wallet running?')), 5000);
  });
  console.log('✓ WebSocket connected');

  // Generate dApp ECDH keypair
  const { publicKey: dappPub, privateKey: dappPriv } = await generateECDHKeypair();
  const dappPubBytes = await exportPublicKeyUncompressed(dappPub);

  // HELLO_REQ = 65 bytes dApp ECDH pubkey + 64 bytes zero signature (test only)
  const helloReq = new Uint8Array(65 + 64);
  helloReq.set(dappPubBytes, 0);
  // In production the 64 bytes would be an Ed25519 signature over [dappECDH || assocPubKey]
  // For local testing, the wallet accepts any signature

  console.log('  → HELLO_REQ (65-byte ECDH pubkey + 64-byte sig)');
  ws.send(helloReq);

  // Wait for HELLO_RSP (wallet's 65-byte ECDH pubkey)
  const helloRsp = await new Promise((resolve, reject) => {
    ws.once('message', (data) => resolve(new Uint8Array(data.buffer || data)));
    setTimeout(() => reject(new Error('Timeout waiting for HELLO_RSP')), 5000);
  });

  if (helloRsp.length < 65) throw new Error(`HELLO_RSP too short: ${helloRsp.length} bytes`);
  console.log('  ← HELLO_RSP received (wallet ECDH pubkey)');

  // Derive shared AES key
  const aesKey = await deriveSharedSecret(dappPriv, helloRsp.slice(0, 65));
  console.log('✓ ECDH handshake complete — session established\n');

  return new MWASession(ws, aesKey);
}

// ── Test flows ────────────────────────────────────────────────────────────────

async function testAuthorize(session) {
  console.log('── TEST: authorize ──────────────────────────────────────');
  const result = await session.send('authorize', {
    cluster: 'devnet',
    identity: {
      name: 'XFHotWallet Test dApp',
      uri: 'http://localhost:3000',
      icon: '/favicon.ico',
    },
  });
  console.log('✓ Authorized!');
  console.log('  auth_token:', result.auth_token);
  console.log('  accounts:', result.accounts?.map(a => a.address));
  return result;
}

async function testSignMessage(session, authResult) {
  console.log('\n── TEST: sign_messages ──────────────────────────────────');
  const msg = new TextEncoder().encode('Hello from XFHotWallet test dApp!');
  const msgB64 = Buffer.from(msg).toString('base64');
  const result = await session.send('sign_messages', {
    addresses: [authResult.accounts[0].address],
    payloads: [msgB64],
  });
  console.log('✓ Message signed!');
  console.log('  signature:', result.signed_payloads?.[0]?.slice(0, 20), '…');
  return result;
}

async function testSignTransaction(session, authResult) {
  console.log('\n── TEST: sign_transactions ──────────────────────────────');

  // Build a tiny devnet transfer tx (wallet → itself, 1 lamport)
  const connection = new Connection('https://api.devnet.solana.com', 'confirmed');
  const senderPubkey = new PublicKey(authResult.accounts[0].address);
  const { blockhash } = await connection.getLatestBlockhash();

  const tx = new Transaction({
    recentBlockhash: blockhash,
    feePayer: senderPubkey,
  }).add(
    SystemProgram.transfer({
      fromPubkey: senderPubkey,
      toPubkey: senderPubkey,
      lamports: 1,
    })
  );

  const txBytes = tx.serialize({ requireAllSignatures: false, verifySignatures: false });
  const txB64 = Buffer.from(txBytes).toString('base64');

  const result = await session.send('sign_transactions', {
    payloads: [txB64],
  });
  console.log('✓ Transaction signed!');
  console.log('  signed payload length:', result.signed_payloads?.[0]?.length, 'chars');
  return result;
}

// ── Main ──────────────────────────────────────────────────────────────────────

async function main() {
  const args = process.argv.slice(2);
  const authOnly = args.includes('--auth-only');
  const withSign = args.includes('--sign');

  let session;
  try {
    session = await connect();
    const authResult = await testAuthorize(session);

    if (!authOnly) {
      await testSignMessage(session, authResult);
    }

    if (withSign) {
      await testSignTransaction(session, authResult);
    }

    console.log('\n🎉 All tests passed!\n');
  } catch (e) {
    console.error('\n❌ Test failed:', e.message);
    process.exit(1);
  } finally {
    session?.ws?.close();
  }
}

main();
