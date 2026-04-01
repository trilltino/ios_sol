import { invoke } from "@tauri-apps/api/core";
import { PinPad } from "./PinPad";

interface AppIdentity {
  name?: string;
  uri?: string;
  icon?: string;
}

interface AuthorizeParams {
  cluster?: string;
  identity?: AppIdentity;
}

interface SignTransactionsParams {
  payloads: string[];
}

type MwaRequestType =
  | { method: "authorize"; params: AuthorizeParams; id: number; request_id: string }
  | { method: "sign_transactions"; params: SignTransactionsParams; id: number; request_id: string }
  | { method: "sign_and_send_transactions"; params: SignTransactionsParams; id: number; request_id: string }
  | { method: "sign_messages"; params: { payloads: string[]; addresses: string[] }; id: number; request_id: string }
  | { method: "deauthorize"; params: { auth_token: string }; id: number; request_id: string };

interface Props {
  request: MwaRequestType;
  walletPubkey: string;
  onDone: () => void;
}

export function MwaRequestSheet({ request, walletPubkey, onDone }: Props) {
  const [step, setStep] = useState<"review" | "pin">("review");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function handleApprove() {
    if (
      request.method === "sign_transactions" ||
      request.method === "sign_and_send_transactions"
    ) {
      setStep("pin");
    } else if (request.method === "authorize") {
      await doAuthorize();
    } else if (request.method === "deauthorize") {
      await doReject();
    }
  }

  async function doAuthorize() {
    setBusy(true);
    try {
      await invoke("mwa_approve_authorization", {
        requestId: request.request_id,
        pubkeyB58: walletPubkey,
      });
      onDone();
    } catch (e) {
      setError(e as string);
    } finally {
      setBusy(false);
    }
  }

  async function doReject() {
    setBusy(true);
    try {
      await invoke("mwa_reject_request", { requestId: request.request_id });
    } catch (_) {}
    setBusy(false);
    onDone();
  }

  async function onPinConfirm(pin: string) {
    setBusy(true);
    setError(null);
    try {
      await invoke("mwa_approve_sign_transactions", {
        requestId: request.request_id,
        pin,
        txPayloadsB64: (request.params as SignTransactionsParams).payloads,
        sendToNetwork: request.method === "sign_and_send_transactions",
      });
      onDone();
    } catch (e) {
      setError(e as string);
      setStep("review");
    } finally {
      setBusy(false);
    }
  }

  const identity = (request.params as AuthorizeParams).identity;
  const dappName = identity?.name ?? "Unknown dApp";
  const dappUri = identity?.uri ?? "";
  const txCount =
    request.method === "sign_transactions" || request.method === "sign_and_send_transactions"
      ? (request.params as SignTransactionsParams).payloads.length
      : undefined;

  if (step === "pin") {
    return (
      <div className="mwa-overlay">
        <div className="mwa-sheet">
          <PinPad
            onComplete={onPinConfirm}
            error={error}
            title={`Sign ${txCount} Transaction${txCount !== 1 ? "s" : ""}`}
          />
          <button className="mwa-cancel-btn" onClick={() => setStep("review")}>
            Cancel
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="mwa-overlay">
      <div className="mwa-sheet">
        {/* Header */}
        <div className="mwa-header">
          <div className="mwa-icon">
            {identity?.icon ? (
              <img src={`${dappUri}${identity.icon}`} alt={dappName} className="mwa-dapp-icon" onError={(e) => { (e.target as HTMLImageElement).style.display = "none"; }} />
            ) : (
              <span>🔗</span>
            )}
          </div>
          <h3 className="mwa-title">
            {request.method === "authorize" && "Connect Wallet"}
            {request.method === "sign_transactions" && "Sign Transaction"}
            {request.method === "sign_and_send_transactions" && "Sign & Send"}
            {request.method === "sign_messages" && "Sign Message"}
            {request.method === "deauthorize" && "Disconnect"}
          </h3>
          <button className="mwa-x-btn" onClick={doReject} disabled={busy}>✕</button>
        </div>

        {/* dApp info */}
        <div className="mwa-dapp-info">
          <div className="mwa-dapp-name">{dappName}</div>
          {dappUri && <div className="mwa-dapp-uri">{dappUri}</div>}
        </div>

        {/* Request-specific content */}
        <div className="mwa-content">
          {request.method === "authorize" && (
            <div className="mwa-auth-body">
              <div className="mwa-permission-row">
                <span className="mwa-perm-icon">👀</span>
                <span>View your wallet address</span>
              </div>
              <div className="mwa-permission-row">
                <span className="mwa-perm-icon">📝</span>
                <span>Request transaction signatures</span>
              </div>
              <div className="mwa-account-pill">
                {walletPubkey.slice(0, 8)}…{walletPubkey.slice(-6)}
              </div>
            </div>
          )}

          {(request.method === "sign_transactions" || request.method === "sign_and_send_transactions") && (
            <div className="mwa-sign-body">
              <div className="mwa-tx-count">
                {txCount} transaction{txCount !== 1 ? "s" : ""} to{" "}
                {request.method === "sign_and_send_transactions" ? "sign & send" : "sign"}
              </div>
              <div className="mwa-warning">
                ⚠ Always verify you trust <strong>{dappName}</strong> before signing.
              </div>
            </div>
          )}

          {request.method === "sign_messages" && (
            <div className="mwa-sign-body">
              <div className="mwa-tx-count">
                {(request.params as { payloads: string[] }).payloads.length} message(s) to sign
              </div>
            </div>
          )}
        </div>

        {error && <p className="mwa-error">{error}</p>}

        {/* Actions */}
        <div className="mwa-actions">
          <button className="mwa-reject-btn" onClick={doReject} disabled={busy}>
            Decline
          </button>
          <button className="mwa-approve-btn" onClick={handleApprove} disabled={busy}>
            {busy ? "…" : request.method === "authorize" ? "Connect" : "Approve"}
          </button>
        </div>
      </div>
    </div>
  );
}

// Need to import useState at top — fix:
import { useState } from "react";
