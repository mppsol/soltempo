/**
 * Shared TypeScript types for soltempo.
 *
 * The CrossVMIntent type here mirrors the Solidity struct in
 * `contracts/buffer/src/CrossVMIntent.sol` and the Rust struct in
 * `programs/vault/src/lib.rs`. All three definitions must stay in sync.
 *
 * IMPORTANT: at v0.1, the Solidity side encodes via `abi.encode` (Solidity ABI),
 * the Rust side decodes via Borsh. These are incompatible. A canonical shared
 * encoding is a v0.2 deliverable. See README "Known TODOs" section.
 */

export type ChainId = "tempo" | "solana";

export const IntentKind = {
  DepositForYield: 0,
  PullbackForPayout: 1,
  ReceiptAck: 2,
} as const;

export type IntentKind = (typeof IntentKind)[keyof typeof IntentKind];

/**
 * Cross-VM settlement intent carried by CCIP message data.
 * Token amounts ride separately in CCIP `tokenAmounts`.
 */
export interface CrossVMIntent {
  /** CCIP chain selector of the destination chain. */
  sourceChain: bigint;
  /** Origin address: EVM address (left-padded) or Solana pubkey, both 32 bytes. */
  sourceAddress: `0x${string}`;
  /** Canonical merchant identifier. */
  merchant: `0x${string}`;
  /** USDC base units (6 decimals). */
  amount: bigint;
  /** Unique nonce — also becomes mppsol Receipt PDA seed. */
  nonce: `0x${string}`;
  /** Direction + purpose. */
  kind: IntentKind;
}

/**
 * Reference to an mppsol Receipt PDA emitted by mppsol_cpi.pay_with_receipt
 * on the Solana side. The keeper persists these to provide an audit trail
 * for cross-VM settlements.
 */
export interface MppsolReceiptRef {
  /** Solana base58-encoded address of the Receipt PDA. */
  address: string;
  /** The nonce that seeds the PDA. */
  nonce: `0x${string}`;
  /** USDC base units settled. */
  amount: bigint;
  /** Solana slot at which the receipt was created. */
  slot: bigint;
}

/**
 * Keeper view of a single cross-VM bridge cycle.
 */
export interface BridgeCycle {
  ccipMessageId: `0x${string}`;
  intent: CrossVMIntent;
  status:
    | "tempo-sent"
    | "ccip-in-flight"
    | "solana-received"
    | "kamino-allocated"
    | "settled-via-mppsol"
    | "ccip-pullback-in-flight"
    | "tempo-received";
  receipt?: MppsolReceiptRef;
}
