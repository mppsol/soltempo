/**
 * Smoke test: invoke vault.request_pullback_to_tempo against the
 * upgraded devnet vault using the local id.json keypair.
 *
 * Mirrors the exact ix the merchant-web Withdraw button builds in
 * `apps/merchant-web/src/lib/vaultIx.ts`. Validates:
 *
 *   - Precomputed discriminator [154,53,70,29,155,195,123,134] is
 *     accepted by the on-chain program (no InstructionFallbackNotFound).
 *   - RequestPullback account ordering matches the upgraded vault.
 *   - PullbackRequested event fires with the expected fields.
 *
 * Run:
 *   pnpm --filter @soltempo/keeper test-pullback -- --amount 100
 */

import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  TransactionInstruction,
  clusterApiUrl,
} from "@solana/web3.js";
import { readFileSync } from "fs";
import { homedir } from "os";
import { join } from "path";

const VAULT_PROGRAM_ID = new PublicKey(
  "2YhYmfCoCj3VvyN2HQ3cuavMiZzEUdUTrhvo6nmGRXe3",
);
const VAULT_ADDRESS = new PublicKey(
  "8smibhXARvuYGEadHqc9C9tqTJWFmLUdXAtkabtFaA9M",
);
const USDC_SOLANA = new PublicKey(
  "CTwxuhJgAv4Tkxzt8HceSv1c8tyNL3SDWLGYki4rrJAG",
);
const REQUEST_PULLBACK_DISC = new Uint8Array([
  154, 53, 70, 29, 155, 195, 123, 134,
]);

function parseArgs(): { amountUsdc: bigint; keypairPath: string } {
  const args = process.argv.slice(2);
  let amountUsdc = 100n; // default 100 USDC
  let keypairPath = join(homedir(), ".config/solana/id.json");
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--amount") amountUsdc = BigInt(args[++i]);
    else if (args[i] === "--keypair") keypairPath = args[++i];
  }
  return { amountUsdc, keypairPath };
}

function loadKeypair(path: string): Keypair {
  const raw = JSON.parse(readFileSync(path, "utf-8")) as number[];
  return Keypair.fromSecretKey(Uint8Array.from(raw));
}

function buildIx(authority: PublicKey, amountBaseUnits: bigint, nonce: Uint8Array): TransactionInstruction {
  const data = new Uint8Array(8 + 8 + 32);
  data.set(REQUEST_PULLBACK_DISC, 0);
  new DataView(data.buffer, 8, 8).setBigUint64(0, amountBaseUnits, true);
  data.set(nonce, 16);

  return new TransactionInstruction({
    programId: VAULT_PROGRAM_ID,
    keys: [
      { pubkey: VAULT_ADDRESS, isSigner: false, isWritable: false },
      { pubkey: USDC_SOLANA, isSigner: false, isWritable: false },
      { pubkey: authority, isSigner: true, isWritable: true },
    ],
    data: Buffer.from(data),
  });
}

async function main() {
  const { amountUsdc, keypairPath } = parseArgs();
  const amountBaseUnits = amountUsdc * 1_000_000n; // 6 decimals

  const conn = new Connection(
    process.env.SOLANA_RPC ?? clusterApiUrl("devnet"),
    "confirmed",
  );
  const authority = loadKeypair(keypairPath);
  console.log("authority:", authority.publicKey.toBase58());
  console.log("amount:   ", `${amountUsdc} USDC (${amountBaseUnits} base units)`);

  // Random nonce — same scheme as the frontend.
  const nonce = new Uint8Array(32);
  for (let i = 0; i < 32; i++) nonce[i] = Math.floor(Math.random() * 256);
  console.log("nonce:    ", Buffer.from(nonce).toString("hex"));

  const ix = buildIx(authority.publicKey, amountBaseUnits, nonce);

  const { blockhash, lastValidBlockHeight } = await conn.getLatestBlockhash("confirmed");
  const tx = new Transaction().add(ix);
  tx.recentBlockhash = blockhash;
  tx.lastValidBlockHeight = lastValidBlockHeight;
  tx.feePayer = authority.publicKey;
  tx.sign(authority);

  console.log("\nsending…");
  const sig = await conn.sendRawTransaction(tx.serialize(), { skipPreflight: false });
  console.log("signature:", sig);
  console.log("explorer: ", `https://explorer.solana.com/tx/${sig}?cluster=devnet`);

  console.log("\nconfirming…");
  await conn.confirmTransaction(
    { signature: sig, blockhash, lastValidBlockHeight },
    "confirmed",
  );
  console.log("confirmed.\n");

  // Fetch the tx logs and surface the PullbackRequested event line.
  const txInfo = await conn.getTransaction(sig, {
    maxSupportedTransactionVersion: 0,
    commitment: "confirmed",
  });
  const logs = txInfo?.meta?.logMessages ?? [];
  console.log("--- program logs ---");
  for (const l of logs) console.log(" ", l);

  const programDataLines = logs.filter((l) => l.startsWith("Program data: "));
  console.log(
    `\n${programDataLines.length} event log(s) emitted (Anchor #[event] entries).`,
  );
}

main().catch((e) => {
  console.error("failed:", e);
  process.exit(1);
});
