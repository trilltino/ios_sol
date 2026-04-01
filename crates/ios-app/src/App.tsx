import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { PinPad } from "./PinPad";
import { NFTGallery } from "./NFTGallery";
import { MwaRequestSheet } from "./MwaRequestSheet";
import { useMwaListener } from "./hooks/useMwaListener";
import "./App.css";

interface WalletInfo {
  pubkey: string;
  mnemonic: string;
}

interface BiometricStatus {
  is_available: boolean;
  biometry_type: string;
  error?: string;
}

type AppState =
  | "LOADING"
  | "LOCKED"
  | "BIOMETRIC_PROMPT"
  | "UNINITIALIZED"
  | "SETTING_PIN"
  | "MAIN_WALLET";

type WalletTab = "assets" | "nfts" | "settings";

function App() {
  const [appState, setAppState] = useState<AppState>("LOADING");
  const [wallet, setWallet] = useState<WalletInfo | null>(null);
  const [balance, setBalance] = useState<number | null>(null);
  const [importPhrase, setImportPhrase] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [theme, setTheme] = useState<"light" | "dark">("dark");
  const [pendingMnemonic, setPendingMnemonic] = useState<string | null>(null);
  const [biometricStatus, setBiometricStatus] = useState<BiometricStatus | null>(null);
  const [activeTab, setActiveTab] = useState<WalletTab>("assets");

  // MWA request listener
  const { currentRequest, clearRequest } = useMwaListener();

  // Send SOL states
  const [isSendModalOpen, setIsSendModalOpen] = useState(false);
  const [sendRecipient, setSendRecipient] = useState("");
  const [sendAmount, setSendAmount] = useState("");
  const [txSignature, setTxSignature] = useState<string | null>(null);
  const [isSending, setIsSending] = useState(false);
  const [requiresPinForSend, setRequiresPinForSend] = useState(false);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
  }, [theme]);

  useEffect(() => {
    checkInitialState();
  }, []);

  async function checkInitialState() {
    try {
      // Check biometric availability first
      const bio = await invoke<BiometricStatus>("biometric_status");
      setBiometricStatus(bio);

      const exists = await invoke<boolean>("vault_exists");
      if (!exists) {
        setAppState("UNINITIALIZED");
      } else if (bio.is_available) {
        // Vault exists + biometric available → show biometric prompt
        setAppState("BIOMETRIC_PROMPT");
        // Auto-trigger biometric immediately
        const passed = await invoke<boolean>("biometric_authenticate");
        if (passed) {
          // Biometric passed: now show PIN to complete decryption
          setAppState("LOCKED");
        }
        // If not passed (user cancelled / not enrolled) → fall to PIN
      } else {
        setAppState("LOCKED");
      }
    } catch (e) {
      console.error(e);
      setAppState("UNINITIALIZED");
    }
  }

  useEffect(() => {
    if (wallet) fetchBalance(wallet.pubkey);
  }, [wallet]);

  async function fetchBalance(pubkey: string) {
    try {
      const res = await invoke<number>("get_sol_balance", { pubkey });
      setBalance(res);
    } catch (e) {
      console.error("Failed to fetch balance:", e);
    }
  }

  async function handleCreateWallet() {
    setError(null);
    try {
      const res = await invoke<WalletInfo>("generate_wallet");
      setPendingMnemonic(res.mnemonic);
      setAppState("SETTING_PIN");
    } catch (e) {
      setError(e as string);
    }
  }

  async function handleImportWallet() {
    if (!importPhrase) return;
    setError(null);
    try {
      const res = await invoke<WalletInfo>("import_wallet", { phrase: importPhrase });
      setPendingMnemonic(res.mnemonic);
      setAppState("SETTING_PIN");
      setImportPhrase("");
    } catch (e) {
      setError(e as string);
    }
  }

  async function onPinSet(pin: string) {
    if (!pendingMnemonic) return;
    try {
      await invoke("save_to_vault", { pin, mnemonic: pendingMnemonic });
      const walletInfo = await invoke<WalletInfo>("unlock_from_vault", { pin });
      setWallet(walletInfo);
      setAppState("MAIN_WALLET");
      setPendingMnemonic(null);
    } catch (e) {
      setError(e as string);
    }
  }

  async function onPinUnlock(pin: string) {
    setError(null);
    try {
      const biometricPassed = biometricStatus?.is_available ?? false;
      const walletInfo = await invoke<WalletInfo>("biometric_unlock", {
        pin,
        biometricPassed,
      });
      setWallet(walletInfo);
      setAppState("MAIN_WALLET");
    } catch (e) {
      setError("Incorrect PIN");
    }
  }

  async function handleSendTransaction(pin: string) {
    setIsSending(true);
    setError(null);
    try {
      const signature = await invoke<string>("send_sol", {
        pin,
        recipient: sendRecipient,
        amount: parseFloat(sendAmount),
      });
      setTxSignature(signature);
      setRequiresPinForSend(false);
      if (wallet) fetchBalance(wallet.pubkey);
    } catch (e) {
      setError(e as string);
      setRequiresPinForSend(false);
    } finally {
      setIsSending(false);
    }
  }

  async function handleReset() {
    if (
      confirm(
        "This will permanently delete your local vault. Ensure you have your 12-word seed phrase saved!"
      )
    ) {
      await invoke("reset_vault");
      setWallet(null);
      setBalance(null);
      setAppState("UNINITIALIZED");
    }
  }

  const toggleTheme = (e: React.ChangeEvent<HTMLInputElement>) => {
    setTheme(e.target.checked ? "dark" : "light");
  };

  const closeApp = () => getCurrentWindow().close();

  if (appState === "LOADING") return <div className="container" />;

  return (
    <div className="container">
      {/* Custom Title Bar */}
      <header className="title-bar">
        <div className="drag-region" data-tauri-drag-region />
        <h2>XFHotWallet</h2>
        <div className="controls">
          <div className="theme-switch-wrapper">
            <label className="theme-switch" htmlFor="checkbox">
              <input
                type="checkbox"
                id="checkbox"
                checked={theme === "dark"}
                onChange={toggleTheme}
              />
              <div className="slider" />
            </label>
          </div>
          <button className="close-btn" onClick={closeApp}>✕</button>
        </div>
      </header>

      <main
        className="content"
        style={{ padding: appState === "MAIN_WALLET" ? "1.5rem" : "0" }}
      >
        {/* ── Biometric Prompt ── */}
        {appState === "BIOMETRIC_PROMPT" && (
          <div className="biometric-prompt">
            <div className="bio-icon">
              {biometricStatus?.biometry_type === "FaceID" ? "👤" : "👆"}
            </div>
            <h2>{biometricStatus?.biometry_type ?? "Biometric"} Required</h2>
            <p>Use {biometricStatus?.biometry_type} to unlock your wallet</p>
            <button
              className="main-btn"
              onClick={async () => {
                const passed = await invoke<boolean>("biometric_authenticate");
                if (passed) setAppState("LOCKED");
              }}
            >
              Try Again
            </button>
            <button
              className="main-btn action-btn"
              style={{ marginTop: "0.5rem" }}
              onClick={() => setAppState("LOCKED")}
            >
              Use PIN Instead
            </button>
          </div>
        )}

        {/* ── PIN Unlock ── */}
        {appState === "LOCKED" && (
          <PinPad onComplete={onPinUnlock} error={error} title="Unlock Wallet" onReset={handleReset} />
        )}

        {/* ── Set PIN ── */}
        {appState === "SETTING_PIN" && (
          <PinPad onComplete={onPinSet} error={error} title="Set New Security PIN" />
        )}

        {/* ── Onboarding ── */}
        {appState === "UNINITIALIZED" && (
          <div className="setup-view" style={{ padding: "2rem" }}>
            <div className="wallet-card" style={{ textAlign: "center" }}>
              <h1 style={{ fontSize: "1.8rem", marginBottom: "2rem", fontWeight: 800 }}>
                Welcome
              </h1>
              <button className="main-btn" onClick={handleCreateWallet}>
                Create New Wallet
              </button>
              <div className="divider">OR</div>
              <input
                placeholder="Enter 12-word mnemonic..."
                value={importPhrase}
                onChange={(e) => setImportPhrase(e.target.value)}
              />
              <button className="main-btn action-btn" onClick={handleImportWallet}>
                Import Existing
              </button>
            </div>
          </div>
        )}

        {/* ── Main Wallet ── */}
        {appState === "MAIN_WALLET" && wallet && (
          <div className="wallet-view">
            {/* Tab bar */}
            <div className="tab-bar">
              <button
                className={`tab-btn ${activeTab === "assets" ? "tab-active" : ""}`}
                onClick={() => setActiveTab("assets")}
              >
                Assets
              </button>
              <button
                className={`tab-btn ${activeTab === "nfts" ? "tab-active" : ""}`}
                onClick={() => setActiveTab("nfts")}
              >
                NFTs
              </button>
              <button
                className={`tab-btn ${activeTab === "settings" ? "tab-active" : ""}`}
                onClick={() => setActiveTab("settings")}
              >
                Settings
              </button>
            </div>

            {/* ── Assets Tab ── */}
            {activeTab === "assets" && (
              <>
                <div className="balance-card">
                  <div className="label">Total Balance</div>
                  <div className="balance-amount">
                    {balance !== null
                      ? balance.toLocaleString(undefined, {
                          minimumFractionDigits: 2,
                          maximumFractionDigits: 4,
                        })
                      : "0.00"}
                    <span className="balance-symbol">SOL</span>
                  </div>
                  <button
                    className="main-btn"
                    style={{ marginTop: "1.5rem" }}
                    onClick={() => setIsSendModalOpen(true)}
                  >
                    Send SOL
                  </button>
                </div>

                <div className="wallet-card" style={{ marginTop: "1rem" }}>
                  <div className="info-section">
                    <div className="label">Public Address</div>
                    <div className="value-box">{wallet.pubkey}</div>
                  </div>
                  <div className="info-section">
                    <div className="label">MWA Status</div>
                    <div className="value-box" style={{ fontSize: "0.8rem", color: "var(--accent)" }}>
                      🔗 Listening on ws://localhost:44444
                    </div>
                  </div>
                </div>

                <div className="actions">
                  <button className="main-btn" onClick={() => fetchBalance(wallet.pubkey)}>
                    Refresh
                  </button>
                </div>
              </>
            )}

            {/* ── NFTs Tab ── */}
            {activeTab === "nfts" && (
              <NFTGallery pubkey={wallet.pubkey} />
            )}

            {/* ── Settings Tab ── */}
            {activeTab === "settings" && (
              <div className="wallet-card">
                <div className="info-section" style={{ marginBottom: 0 }}>
                  <div className="label">Seed Phrase</div>
                  <div className="value-box" style={{ fontSize: "0.75rem", opacity: 0.8 }}>
                    {wallet.mnemonic}
                  </div>
                </div>
                <button
                  className="main-btn action-btn"
                  style={{ marginTop: "1.5rem" }}
                  onClick={handleReset}
                >
                  Sign Out / Reset Vault
                </button>
              </div>
            )}
          </div>
        )}

        {/* ── Send Modal ── */}
        {isSendModalOpen && (
          <div className="send-modal">
            {!txSignature ? (
              requiresPinForSend ? (
                <PinPad
                  onComplete={handleSendTransaction}
                  error={error}
                  title="Confirm Transaction"
                />
              ) : (
                <>
                  <div className="modal-header">
                    <h3>Send SOL</h3>
                    <button
                      className="close-btn"
                      onClick={() => setIsSendModalOpen(false)}
                    >
                      ✕
                    </button>
                  </div>
                  <div className="send-input-group">
                    <label>Recipient Address</label>
                    <input
                      autoFocus
                      placeholder="Solana Address..."
                      value={sendRecipient}
                      onChange={(e) => setSendRecipient(e.target.value)}
                    />
                  </div>
                  <div className="send-input-group">
                    <label>Amount (SOL)</label>
                    <input
                      type="number"
                      placeholder="0.00"
                      value={sendAmount}
                      onChange={(e) => setSendAmount(e.target.value)}
                    />
                  </div>
                  <button
                    className="main-btn"
                    disabled={isSending || !sendRecipient || !sendAmount}
                    onClick={() => setRequiresPinForSend(true)}
                  >
                    {isSending ? "Processing..." : "Review & Send"}
                  </button>
                  {error && (
                    <p style={{ color: "#ff4b4b", textAlign: "center", marginTop: "1rem" }}>
                      {error}
                    </p>
                  )}
                </>
              )
            ) : (
              <div className="success-screen">
                <div className="success-icon">✓</div>
                <h2 style={{ fontWeight: 800 }}>Transfer Sent!</h2>
                <div className="tx-id-box">{txSignature}</div>
                <button
                  className="main-btn back-btn"
                  onClick={() => {
                    setIsSendModalOpen(false);
                    setTxSignature(null);
                    setSendRecipient("");
                    setSendAmount("");
                  }}
                >
                  Close
                </button>
              </div>
            )}
          </div>
        )}
      </main>

      {/* ── MWA Request Sheet (floats over everything) ── */}
      {currentRequest && wallet && (
        <MwaRequestSheet
          request={currentRequest as any}
          walletPubkey={wallet.pubkey}
          onDone={clearRequest}
        />
      )}
    </div>
  );
}

export default App;
