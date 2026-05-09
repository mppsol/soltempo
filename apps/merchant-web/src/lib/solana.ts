/**
 * Solana client — reads vault state and the vault's USDC ATA.
 *
 * We decode the Vault account by hand instead of pulling in the full
 * Anchor IDL machinery, because the layout is small and stable. The
 * field offsets match `programs/vault/src/lib.rs::Vault` byte-for-byte
 * (account discriminator = 8 bytes, then fields in declared order).
 *
 * If the on-chain Vault struct changes, update VAULT_LAYOUT below to
 * match — the layout is source of truth and decodeVault() will read
 * the right fields.
 */

import { Connection, PublicKey } from "@solana/web3.js";
import { getAssociatedTokenAddressSync, AccountLayout } from "@solana/spl-token";
import { SOLANA_RPC, USDC_SOLANA, VAULT_ADDRESS } from "./config";

export const connection = new Connection(SOLANA_RPC, "confirmed");

/**
 * Vault layout (Anchor `Vault` account in programs/vault/src/lib.rs).
 * Order matters — must mirror the Rust struct field declaration order
 * exactly, because Anchor serializes structs as concatenated fields.
 */
export const VAULT_LAYOUT = {
  discriminator: { offset: 0, size: 8 },
  authority: { offset: 8, size: 32 },
  merchant_id: { offset: 40, size: 32 },
  kamino_market: { offset: 72, size: 32 },
  usdc_mint: { offset: 104, size: 32 },
  total_deposits: { offset: 136, size: 8 }, // u64 LE — note: offset 0x88
  bump: { offset: 144, size: 1 },
  ccip_router: { offset: 145, size: 32 },
  expected_tempo_sender: { offset: 177, size: 32 },
  expected_tempo_chain_selector: { offset: 209, size: 8 }, // u64 LE
  trusted_keeper: { offset: 217, size: 32 },
} as const;

export interface VaultState {
  authority: PublicKey;
  merchantId: Uint8Array;
  kaminoMarket: PublicKey;
  usdcMint: PublicKey;
  totalDeposits: bigint;
  bump: number;
  ccipRouter: PublicKey;
  expectedTempoSender: Uint8Array;
  expectedTempoChainSelector: bigint;
  trustedKeeper: PublicKey;
}

function readU64LE(buf: Buffer, offset: number): bigint {
  // Buffer.readBigUInt64LE returns bigint directly, but works only on
  // Node Buffer. The browser polyfill via Buffer package handles this.
  return buf.readBigUInt64LE(offset);
}

export function decodeVault(data: Buffer): VaultState {
  const L = VAULT_LAYOUT;
  return {
    authority: new PublicKey(data.subarray(L.authority.offset, L.authority.offset + 32)),
    merchantId: new Uint8Array(data.subarray(L.merchant_id.offset, L.merchant_id.offset + 32)),
    kaminoMarket: new PublicKey(data.subarray(L.kamino_market.offset, L.kamino_market.offset + 32)),
    usdcMint: new PublicKey(data.subarray(L.usdc_mint.offset, L.usdc_mint.offset + 32)),
    totalDeposits: readU64LE(data, L.total_deposits.offset),
    bump: data[L.bump.offset],
    ccipRouter: new PublicKey(data.subarray(L.ccip_router.offset, L.ccip_router.offset + 32)),
    expectedTempoSender: new Uint8Array(
      data.subarray(L.expected_tempo_sender.offset, L.expected_tempo_sender.offset + 32),
    ),
    expectedTempoChainSelector: readU64LE(data, L.expected_tempo_chain_selector.offset),
    trustedKeeper: new PublicKey(
      data.subarray(L.trusted_keeper.offset, L.trusted_keeper.offset + 32),
    ),
  };
}

export async function readVaultState(): Promise<VaultState | null> {
  const info = await connection.getAccountInfo(VAULT_ADDRESS, "confirmed");
  if (!info) return null;
  return decodeVault(Buffer.from(info.data));
}

/**
 * Vault PDA's USDC ATA — bridged USDC lands here after CCIP / keeper
 * relays the cross-VM intent. This is where balance grows when
 * deposits are bridged.
 */
export const VAULT_USDC_ATA = getAssociatedTokenAddressSync(
  USDC_SOLANA,
  VAULT_ADDRESS,
  /* allowOwnerOffCurve */ true,
);

export async function readVaultUsdcBalance(): Promise<bigint> {
  const info = await connection.getAccountInfo(VAULT_USDC_ATA, "confirmed");
  if (!info) return 0n;
  // SPL token account amount is u64 LE at offset 64.
  const decoded = AccountLayout.decode(info.data);
  return decoded.amount;
}

export function fmtSolanaUsdc(amount: bigint): string {
  // 6 decimals
  const whole = amount / 1_000_000n;
  const frac = amount % 1_000_000n;
  return `${whole.toString()}.${frac.toString().padStart(6, "0")}`;
}
