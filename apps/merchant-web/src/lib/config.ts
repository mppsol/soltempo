/**
 * Frontend config — pulled from NEXT_PUBLIC_* env vars at build time.
 *
 * IMPORTANT: every env access below uses a *literal* key on the
 * `process.env.NEXT_PUBLIC_X` form. Next.js performs static analysis
 * to replace these references with their literal values in the client
 * bundle. Dynamic forms like `process.env[someVar]` are NOT replaced
 * — the client receives `undefined` and silently falls through to
 * defaults. Earlier revs of this file used a dynamic helper and broke
 * client-side hydration; don't reintroduce one.
 *
 * Defaults match the verified reference deploy in scripts/keeper.sh so
 * the dashboard works out of the box against testnet without any local
 * env file. Override via .env.local to point at a different deploy.
 */

import { Address } from "viem";
import { PublicKey } from "@solana/web3.js";

export const TEMPO_RPC =
  process.env.NEXT_PUBLIC_TEMPO_RPC ?? "https://rpc.moderato.tempo.xyz";
export const TEMPO_CHAIN_ID = Number(
  process.env.NEXT_PUBLIC_TEMPO_CHAIN_ID ?? "42069",
);
export const BUFFER_ADDRESS = (process.env.NEXT_PUBLIC_BUFFER_ADDRESS ??
  "0xe8c675523AFd81587c35Da2BeF6ECc268654D0BE") as Address;
export const USDC_TEMPO = (process.env.NEXT_PUBLIC_USDC_TEMPO ??
  "0x20c0000000000000000000000000000000000000") as Address;

export const SOLANA_RPC =
  process.env.NEXT_PUBLIC_SOLANA_RPC ?? "https://api.devnet.solana.com";
export const VAULT_ADDRESS = new PublicKey(
  process.env.NEXT_PUBLIC_VAULT_ADDRESS ??
    "8smibhXARvuYGEadHqc9C9tqTJWFmLUdXAtkabtFaA9M",
);
export const VAULT_PROGRAM_ID = new PublicKey(
  process.env.NEXT_PUBLIC_VAULT_PROGRAM_ID ??
    "2YhYmfCoCj3VvyN2HQ3cuavMiZzEUdUTrhvo6nmGRXe3",
);
export const USDC_SOLANA = new PublicKey(
  process.env.NEXT_PUBLIC_USDC_SOLANA ??
    "CTwxuhJgAv4Tkxzt8HceSv1c8tyNL3SDWLGYki4rrJAG",
);

/**
 * Demo merchant key — provided via env. If unset/zero we render the
 * dashboard in read-only mode (no deposit button). The hot-wallet
 * pattern is acceptable for a hackathon demo against testnet but
 * obviously not for prod; the UI banner says so.
 */
export const MERCHANT_PRIVATE_KEY = (process.env
  .NEXT_PUBLIC_MERCHANT_PRIVATE_KEY ??
  "0x0000000000000000000000000000000000000000000000000000000000000000") as `0x${string}`;

export const HAS_DEMO_KEY =
  MERCHANT_PRIVATE_KEY !==
  "0x0000000000000000000000000000000000000000000000000000000000000000";

/**
 * Solana vault authority keypair — signs `request_pullback_to_tempo`.
 * Format: JSON array of 64 numbers (the `solana-keygen` / id.json form).
 * Empty array means the Withdraw button stays disabled.
 */
export const SOLANA_AUTHORITY_KEYPAIR_RAW =
  process.env.NEXT_PUBLIC_SOLANA_AUTHORITY_KEYPAIR ?? "[]";

export const HAS_SOLANA_AUTHORITY_KEY = (() => {
  try {
    const arr = JSON.parse(SOLANA_AUTHORITY_KEYPAIR_RAW);
    return Array.isArray(arr) && arr.length === 64;
  } catch {
    return false;
  }
})();
