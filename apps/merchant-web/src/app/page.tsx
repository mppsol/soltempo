"use client";

import { useCallback, useEffect, useState } from "react";
import {
  BUFFER_ADDRESS,
  HAS_DEMO_KEY,
  HAS_SOLANA_AUTHORITY_KEY,
  USDC_TEMPO,
  VAULT_ADDRESS,
  VAULT_PROGRAM_ID,
} from "@/lib/config";
import {
  depositAndBridge,
  fmtUsdc,
  getMerchantAddress,
  parseUsdc,
  readBufferTarget,
  readBufferUsdcBalance,
  readMerchantUsdcBalance,
} from "@/lib/tempo";
import {
  fmtSolanaUsdc,
  readVaultState,
  readVaultUsdcBalance,
  VAULT_USDC_ATA,
  type VaultState,
} from "@/lib/solana";
import { getAuthorityPubkey, sendRequestPullback } from "@/lib/vaultIx";

type FlowStep = "idle" | "approve" | "deposit" | "intent" | "bridging" | "done" | "error";
type WithdrawStep = "idle" | "signing" | "confirming" | "done" | "error";

interface Snapshot {
  merchantUsdc: bigint;
  bufferUsdc: bigint;
  bufferTarget: bigint;
  vaultUsdc: bigint;
  vault: VaultState | null;
  loadedAt: number;
}

interface ActivityEvent {
  ts: number;
  side: "tempo" | "solana";
  label: string;
  meta?: string;
}

const POLL_MS = 4000;

function shortAddr(addr: string): string {
  if (addr.length < 12) return addr;
  return `${addr.slice(0, 6)}…${addr.slice(-4)}`;
}

function ago(ms: number): string {
  const s = Math.floor((Date.now() - ms) / 1000);
  if (s < 5) return "just now";
  if (s < 60) return `${s}s ago`;
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  return `${Math.floor(s / 3600)}h ago`;
}

export default function Page() {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [step, setStep] = useState<FlowStep>("idle");
  const [amount, setAmount] = useState("1000");
  const [activity, setActivity] = useState<ActivityEvent[]>([]);
  const [txHashes, setTxHashes] = useState<{ approve?: string; deposit?: string; intent?: string }>({});

  const [withdrawAmount, setWithdrawAmount] = useState("100");
  const [withdrawStep, setWithdrawStep] = useState<WithdrawStep>("idle");
  const [withdrawSig, setWithdrawSig] = useState<string | null>(null);

  const pushActivity = useCallback((ev: ActivityEvent) => {
    setActivity((prev) => [ev, ...prev].slice(0, 8));
  }, []);

  const refresh = useCallback(async () => {
    try {
      const [merchantUsdc, bufferUsdc, bufferTarget, vault, vaultUsdc] =
        await Promise.all([
          readMerchantUsdcBalance(),
          readBufferUsdcBalance(),
          readBufferTarget(),
          readVaultState(),
          readVaultUsdcBalance(),
        ]);
      setSnap((prev) => {
        // Detect cross-VM bridge completion: vault total_deposits ticked up
        if (
          prev?.vault &&
          vault &&
          vault.totalDeposits > prev.vault.totalDeposits
        ) {
          const delta = vault.totalDeposits - prev.vault.totalDeposits;
          pushActivity({
            ts: Date.now(),
            side: "solana",
            label: "Vault total_deposits incremented",
            meta: `+${fmtSolanaUsdc(delta)} USDC · vault.total_deposits`,
          });
        }
        if (prev && bufferUsdc > prev.bufferUsdc) {
          pushActivity({
            ts: Date.now(),
            side: "tempo",
            label: "Buffer USDC balance increased",
            meta: `+${fmtUsdc(bufferUsdc - prev.bufferUsdc)} pathUSD`,
          });
        }
        return {
          merchantUsdc,
          bufferUsdc,
          bufferTarget,
          vaultUsdc,
          vault,
          loadedAt: Date.now(),
        };
      });
      setError(null);
    } catch (e) {
      setError((e as Error).message);
    }
  }, [pushActivity]);

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, POLL_MS);
    return () => clearInterval(id);
  }, [refresh]);

  const onDeposit = useCallback(async () => {
    setError(null);
    setTxHashes({});
    try {
      const amt = parseUsdc(amount);
      setStep("approve");
      pushActivity({ ts: Date.now(), side: "tempo", label: `Approving ${amount} pathUSD…` });

      // depositAndBridge handles approve + deposit + intent serially.
      // We can't sub-progress without breaking it apart, so stage-by-stage
      // visualization is approximated: approve completes fast, then we
      // jump to deposit, then intent, then bridging.
      setStep("deposit");
      const result = await depositAndBridge(amt);
      setTxHashes({
        approve: result.approveHash ?? undefined,
        deposit: result.depositHash,
        intent: result.intentHash,
      });
      pushActivity({
        ts: Date.now(),
        side: "tempo",
        label: `Deposit confirmed`,
        meta: shortAddr(result.depositHash),
      });
      pushActivity({
        ts: Date.now(),
        side: "tempo",
        label: `Cross-VM intent fired`,
        meta: shortAddr(result.intentHash),
      });
      setStep("bridging");
      // Keeper picks up event and relays — the polling loop will
      // detect the vault delta and add an event.
      setTimeout(() => setStep("done"), 25_000);
    } catch (e) {
      setError((e as Error).message);
      setStep("error");
    }
  }, [amount, pushActivity]);

  const onWithdraw = useCallback(async () => {
    setError(null);
    setWithdrawSig(null);
    try {
      const amt = parseUsdc(withdrawAmount); // shares 6-decimal precision
      setWithdrawStep("signing");
      pushActivity({
        ts: Date.now(),
        side: "solana",
        label: `Requesting pull-back of ${withdrawAmount} USDC…`,
      });
      setWithdrawStep("confirming");
      const { signature } = await sendRequestPullback(amt);
      setWithdrawSig(signature);
      pushActivity({
        ts: Date.now(),
        side: "solana",
        label: "Pull-back requested on vault",
        meta: `${shortAddr(signature)} · keeper consumes off-chain`,
      });
      setWithdrawStep("done");
    } catch (e) {
      setError((e as Error).message);
      setWithdrawStep("error");
    }
  }, [withdrawAmount, pushActivity]);

  const merchantAddr = HAS_DEMO_KEY ? getMerchantAddress() : null;
  const authorityAddr = HAS_SOLANA_AUTHORITY_KEY ? getAuthorityPubkey() : null;

  return (
    <main className="container">
      <div className="header">
        <div>
          <div className="brand">soltempo · merchant dashboard</div>
          <div className="title">
            Stripe-grade payments<br />
            <span className="accent">earning Solana DeFi yield.</span>
          </div>
          <div className="subtitle">
            Live state across Tempo Moderato + Solana devnet · refreshes every {POLL_MS / 1000}s
          </div>
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          <span className="network-pill">Tempo Moderato · {shortAddr(BUFFER_ADDRESS)}</span>
          <span className="network-pill">Solana devnet · {shortAddr(VAULT_ADDRESS.toBase58())}</span>
        </div>
      </div>

      {!HAS_DEMO_KEY && (
        <div className="banner">
          Read-only mode. Set <code>NEXT_PUBLIC_MERCHANT_PRIVATE_KEY</code> in
          <code> .env.local</code> to enable the deposit button. (Hot-wallet pattern is
          for testnet demos only — never use a real key in the browser.)
        </div>
      )}
      {HAS_DEMO_KEY && (
        <div className="banner">
          Demo mode — merchant key is loaded from env and signs in the browser. Testnet only.
        </div>
      )}
      {error && <div className="banner error">{error}</div>}

      <div className="grid">
        <div className="card">
          <div className="card-eyebrow">Merchant · Tempo USDC</div>
          <div className="card-value">
            {snap ? fmtUsdc(snap.merchantUsdc) : <span className="spinner" />}
            <span className="unit">pathUSD</span>
          </div>
          <div className="card-foot">
            {merchantAddr ? shortAddr(merchantAddr) : "—"}
          </div>
        </div>

        <div className="card">
          <div className="card-eyebrow">Buffer · Tempo USDC</div>
          <div className="card-value">
            {snap ? fmtUsdc(snap.bufferUsdc) : <span className="spinner" />}
            <span className="unit">pathUSD</span>
          </div>
          <div className="card-foot">
            target {snap ? fmtUsdc(snap.bufferTarget) : "—"} · {shortAddr(BUFFER_ADDRESS)}
          </div>
        </div>

        <div className="card">
          <div className="card-eyebrow">Vault · Solana USDC</div>
          <div className="card-value">
            {snap ? fmtSolanaUsdc(snap.vaultUsdc) : <span className="spinner" />}
            <span className="unit">USDC</span>
          </div>
          <div className="card-foot">ATA {shortAddr(VAULT_USDC_ATA.toBase58())}</div>
        </div>

        <div className="card">
          <div className="card-eyebrow">vault.total_deposits</div>
          <div className="card-value accent">
            {snap?.vault ? fmtSolanaUsdc(snap.vault.totalDeposits) : "—"}
            <span className="unit">USDC</span>
          </div>
          <div className="card-foot">on-chain proof · offset 0x88</div>
        </div>
      </div>

      <h3 className="section-title">Deposit + bridge</h3>
      <div className="deposit-card">
        <div className="deposit-row">
          <input
            type="number"
            min="1"
            step="1"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            disabled={!HAS_DEMO_KEY || step !== "idle" && step !== "done" && step !== "error"}
            placeholder="Amount in USDC"
          />
          <button
            onClick={onDeposit}
            disabled={
              !HAS_DEMO_KEY ||
              (step !== "idle" && step !== "done" && step !== "error") ||
              !amount ||
              Number(amount) <= 0
            }
          >
            {step === "idle" || step === "done" || step === "error"
              ? "Deposit + bridge"
              : "In flight…"}
          </button>
        </div>
        {step !== "idle" && (
          <div className="tx-progress">
            <div className={`step ${stepClass(step, "approve")}`}>
              {stepIcon(step, "approve")} approve(buffer, amount)
              {txHashes.approve && (
                <span className="copy-tag">{shortAddr(txHashes.approve)}</span>
              )}
            </div>
            <div className={`step ${stepClass(step, "deposit")}`}>
              {stepIcon(step, "deposit")} buffer.deposit(amount)
              {txHashes.deposit && (
                <span className="copy-tag">{shortAddr(txHashes.deposit)}</span>
              )}
            </div>
            <div className={`step ${stepClass(step, "intent")}`}>
              {stepIcon(step, "intent")} buffer.sendIntentToSolana()
              {txHashes.intent && (
                <span className="copy-tag">{shortAddr(txHashes.intent)}</span>
              )}
            </div>
            <div className={`step ${stepClass(step, "bridging")}`}>
              {stepIcon(step, "bridging")} keeper relay → vault.trustedKeeperReceive
            </div>
            <div className={`step ${stepClass(step, "done")}`}>
              {stepIcon(step, "done")} vault.total_deposits incremented
            </div>
          </div>
        )}
      </div>

      <h3 className="section-title">Pull-back to Tempo</h3>
      <div className="deposit-card">
        <div style={{ fontSize: "0.85rem", color: "var(--fg-dim)", lineHeight: 1.5 }}>
          Calls <code>vault.request_pullback_to_tempo</code> on Solana — emits a
          <code> PullbackRequested</code> event the keeper consumes to settle on Tempo.
          Mirrors the inbound trusted-keeper path; vault authority signs.
          {authorityAddr && (
            <> Authority: <span className="copy-tag">{shortAddr(authorityAddr.toBase58())}</span></>
          )}
        </div>
        <div className="deposit-row">
          <input
            type="number"
            min="1"
            step="1"
            value={withdrawAmount}
            onChange={(e) => setWithdrawAmount(e.target.value)}
            disabled={!HAS_SOLANA_AUTHORITY_KEY || withdrawStep === "signing" || withdrawStep === "confirming"}
            placeholder="Amount in USDC"
          />
          <button
            onClick={onWithdraw}
            disabled={
              !HAS_SOLANA_AUTHORITY_KEY ||
              withdrawStep === "signing" ||
              withdrawStep === "confirming" ||
              !withdrawAmount ||
              Number(withdrawAmount) <= 0
            }
          >
            {withdrawStep === "signing" || withdrawStep === "confirming"
              ? "Signing…"
              : "Request pull-back"}
          </button>
        </div>
        {!HAS_SOLANA_AUTHORITY_KEY && (
          <div style={{ fontSize: "0.78rem", color: "var(--fg-faint)" }}>
            Set <code>NEXT_PUBLIC_SOLANA_AUTHORITY_KEYPAIR</code> in <code>.env.local</code>
            {" "}(JSON byte array, like <code>~/.config/solana/id.json</code>) to enable.
          </div>
        )}
        {withdrawStep !== "idle" && (
          <div className="tx-progress">
            <div className={`step ${withdrawStepClass(withdrawStep, "signing")}`}>
              {withdrawStepIcon(withdrawStep, "signing")} authority signs request_pullback_to_tempo
            </div>
            <div className={`step ${withdrawStepClass(withdrawStep, "confirming")}`}>
              {withdrawStepIcon(withdrawStep, "confirming")} sendRawTransaction → confirmTransaction
              {withdrawSig && <span className="copy-tag">{shortAddr(withdrawSig)}</span>}
            </div>
            <div className={`step ${withdrawStepClass(withdrawStep, "done")}`}>
              {withdrawStepIcon(withdrawStep, "done")} PullbackRequested event emitted · keeper consumes
            </div>
          </div>
        )}
      </div>

      <h3 className="section-title">Cross-VM activity</h3>
      <div className="activity">
        {activity.length === 0 && (
          <div style={{ color: "var(--fg-faint)", fontSize: "0.85rem" }}>
            No events yet. Deposit to trigger a cross-VM cycle, or wait for a
            keeper-bridged settlement to land.
          </div>
        )}
        {activity.map((ev, i) => (
          <div key={i} className="event-row">
            <span className={`dot ${ev.side}`} />
            <div>
              <div className="event-label">{ev.label}</div>
              {ev.meta && <div className="event-meta">{ev.meta}</div>}
            </div>
            <span className="event-meta">{ago(ev.ts)}</span>
          </div>
        ))}
      </div>

      <div className="foot">
        Buffer {shortAddr(BUFFER_ADDRESS)} · USDC {shortAddr(USDC_TEMPO)} · Vault {shortAddr(VAULT_ADDRESS.toBase58())} ·
        Program {shortAddr(VAULT_PROGRAM_ID.toBase58())}
        {snap && <> · refreshed {ago(snap.loadedAt)}</>}
      </div>
    </main>
  );
}

function stepOrder(s: FlowStep): number {
  return ["idle", "approve", "deposit", "intent", "bridging", "done"].indexOf(s);
}
function stepClass(current: FlowStep, target: FlowStep): string {
  if (current === "error") return "pending";
  const c = stepOrder(current);
  const t = stepOrder(target);
  if (c > t) return "done";
  if (c === t) return "active";
  return "pending";
}
function stepIcon(current: FlowStep, target: FlowStep): string {
  const cls = stepClass(current, target);
  if (cls === "done") return "✓";
  if (cls === "active") return "▸";
  return "·";
}

function withdrawStepOrder(s: WithdrawStep): number {
  return ["idle", "signing", "confirming", "done"].indexOf(s);
}
function withdrawStepClass(current: WithdrawStep, target: WithdrawStep): string {
  if (current === "error") return "pending";
  const c = withdrawStepOrder(current);
  const t = withdrawStepOrder(target);
  if (c > t) return "done";
  if (c === t) return "active";
  return "pending";
}
function withdrawStepIcon(current: WithdrawStep, target: WithdrawStep): string {
  const cls = withdrawStepClass(current, target);
  if (cls === "done") return "✓";
  if (cls === "active") return "▸";
  return "·";
}
