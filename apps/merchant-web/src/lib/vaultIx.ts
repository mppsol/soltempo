/**
 * Vault instruction builder + sender — `request_pullback_to_tempo`.
 *
 * Hand-built CPI rather than fetching the IDL at runtime; the
 * discriminator is precomputed (sha256("global:request_pullback_to_tempo")[..8])
 * and the account list mirrors the `RequestPullback` Anchor context
 * in `programs/vault/src/lib.rs` exactly.
 *
 * Anchor IDL approach is more flexible but requires more bundle weight
 * (the full anchor Program client) and a Provider. For a single
 * single-instruction call, raw web3.js is leaner.
 */

import {
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  HAS_SOLANA_AUTHORITY_KEY,
  SOLANA_AUTHORITY_KEYPAIR_RAW,
  USDC_SOLANA,
  VAULT_ADDRESS,
  VAULT_PROGRAM_ID,
} from "./config";
import { connection } from "./solana";

/**
 * Anchor instruction discriminator for `request_pullback_to_tempo`.
 * Computed as sha256("global:request_pullback_to_tempo")[..8].
 * If the upstream rust function is renamed, the on-chain ix will fail
 * with InstructionFallbackNotFound — re-derive and update.
 */
const REQUEST_PULLBACK_DISC = new Uint8Array([
  154, 53, 70, 29, 155, 195, 123, 134,
]);

let _authorityKeypair: Keypair | null = null;
export function getAuthorityKeypair(): Keypair {
  if (_authorityKeypair) return _authorityKeypair;
  if (!HAS_SOLANA_AUTHORITY_KEY) {
    throw new Error(
      "NEXT_PUBLIC_SOLANA_AUTHORITY_KEYPAIR not set — Withdraw is disabled",
    );
  }
  const arr = JSON.parse(SOLANA_AUTHORITY_KEYPAIR_RAW) as number[];
  _authorityKeypair = Keypair.fromSecretKey(Uint8Array.from(arr));
  return _authorityKeypair;
}

export function getAuthorityPubkey(): PublicKey | null {
  if (!HAS_SOLANA_AUTHORITY_KEY) return null;
  return getAuthorityKeypair().publicKey;
}

/**
 * Build the `request_pullback_to_tempo` instruction. Args layout
 * (Anchor/Borsh): amount(u64 LE = 8) | nonce([u8; 32]) = 40 bytes.
 *
 * Account order mirrors the RequestPullback context:
 *   1. vault          — readonly, non-signer (PDA)
 *   2. usdc_mint      — readonly, non-signer
 *   3. authority      — signer + writable (fee payer)
 */
export function buildRequestPullbackIx(
  amount: bigint,
  nonce: Uint8Array,
): TransactionInstruction {
  if (nonce.length !== 32) {
    throw new Error(`nonce must be 32 bytes, got ${nonce.length}`);
  }
  const authority = getAuthorityKeypair().publicKey;

  const data = new Uint8Array(8 + 8 + 32);
  data.set(REQUEST_PULLBACK_DISC, 0);
  // amount: u64 LE
  const amountView = new DataView(data.buffer, 8, 8);
  amountView.setBigUint64(0, amount, true);
  // nonce: 32 raw bytes
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

/**
 * Send `request_pullback_to_tempo` and return the tx signature once
 * confirmed. Generates a random 32-byte nonce.
 */
export async function sendRequestPullback(amount: bigint): Promise<{
  signature: string;
  nonce: Uint8Array;
}> {
  const authority = getAuthorityKeypair();
  const nonce = crypto.getRandomValues(new Uint8Array(32));

  const tx = new Transaction().add(buildRequestPullbackIx(amount, nonce));
  // Touch SystemProgram to make sure web3.js is happy with no other ixs
  // and so the `chain` import isn't tree-shaken from analysis. Cheap.
  void SystemProgram.programId;

  const { blockhash, lastValidBlockHeight } = await connection.getLatestBlockhash(
    "confirmed",
  );
  tx.recentBlockhash = blockhash;
  tx.lastValidBlockHeight = lastValidBlockHeight;
  tx.feePayer = authority.publicKey;
  tx.sign(authority);

  const sig = await connection.sendRawTransaction(tx.serialize(), {
    skipPreflight: false,
  });
  await connection.confirmTransaction(
    { signature: sig, blockhash, lastValidBlockHeight },
    "confirmed",
  );
  return { signature: sig, nonce };
}
