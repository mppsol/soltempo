/**
 * Shared TypeScript types for soltempo.
 *
 * The CrossVMIntent type and its encode/decode functions mirror the
 * Solidity library in `contracts/buffer/src/CrossVMIntent.sol` and the
 * Rust implementation in `programs/vault/src/lib.rs`. All three
 * implementations are tested against the same hex vector — see
 * `intent.test.ts`.
 *
 * Wire format (v0.1, version byte = 0x01):
 *
 *   Offset  Size  Field
 *   ─────────────────────────────────────────────────────────
 *   0       1     version          (= 0x01)
 *   1       1     kind             (0=DepositForYield, 1=PullbackForPayout, 2=ReceiptAck)
 *   2       8     sourceChain      (BE u64 — CCIP chain selector)
 *   10      16    amount           (BE u128 — USDC base units, 6 decimals)
 *   26      32    sourceAddress
 *   58      32    merchant
 *   90      32    nonce
 *   ─────────────────────────────────────────────────────────
 *   Total: 122 bytes, big-endian, no padding.
 */

export type ChainId = "tempo" | "solana";

export const INTENT_VERSION = 0x01;
export const INTENT_ENCODED_LENGTH = 122;

export const IntentKind = {
  DepositForYield: 0,
  PullbackForPayout: 1,
  ReceiptAck: 2,
} as const;

export type IntentKind = (typeof IntentKind)[keyof typeof IntentKind];

/** A 32-byte hex string with the `0x` prefix (66 chars total). */
export type Hex32 = `0x${string}`;

/**
 * Cross-VM settlement intent carried by CCIP message data.
 * Token amounts also ride separately in CCIP `tokenAmounts` for the
 * CCIP token-transfer flow; the `amount` field here is the canonical
 * settlement amount the receiver should treat as authoritative.
 */
export interface CrossVMIntent {
  sourceChain: bigint;
  amount: bigint;
  sourceAddress: Hex32;
  merchant: Hex32;
  nonce: Hex32;
  kind: IntentKind;
}

const U128_MAX = (1n << 128n) - 1n;
const U64_MAX = (1n << 64n) - 1n;

export function encodeIntent(intent: CrossVMIntent): Uint8Array {
  if (intent.sourceChain < 0n || intent.sourceChain > U64_MAX) {
    throw new RangeError(`sourceChain out of u64 range: ${intent.sourceChain}`);
  }
  if (intent.amount < 0n || intent.amount > U128_MAX) {
    throw new RangeError(`amount out of u128 range: ${intent.amount}`);
  }
  if (intent.kind < 0 || intent.kind > IntentKind.ReceiptAck) {
    throw new RangeError(`invalid kind: ${intent.kind}`);
  }

  const buf = new Uint8Array(INTENT_ENCODED_LENGTH);
  const view = new DataView(buf.buffer);

  buf[0] = INTENT_VERSION;
  buf[1] = intent.kind;
  view.setBigUint64(2, intent.sourceChain, false); // big-endian
  setBigUint128(view, 10, intent.amount);
  setHex32(buf, 26, intent.sourceAddress);
  setHex32(buf, 58, intent.merchant);
  setHex32(buf, 90, intent.nonce);

  return buf;
}

export function decodeIntent(data: Uint8Array): CrossVMIntent {
  if (data.length !== INTENT_ENCODED_LENGTH) {
    throw new Error(
      `expected ${INTENT_ENCODED_LENGTH} bytes, got ${data.length}`,
    );
  }
  if (data[0] !== INTENT_VERSION) {
    throw new Error(`unsupported version: 0x${data[0]!.toString(16)}`);
  }
  const kindByte = data[1]!;
  if (kindByte > IntentKind.ReceiptAck) {
    throw new Error(`invalid kind: ${kindByte}`);
  }

  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
  return {
    kind: kindByte as IntentKind,
    sourceChain: view.getBigUint64(2, false),
    amount: getBigUint128(view, 10),
    sourceAddress: hexFromBytes(data, 26),
    merchant: hexFromBytes(data, 58),
    nonce: hexFromBytes(data, 90),
  };
}

function setBigUint128(view: DataView, offset: number, value: bigint): void {
  const high = value >> 64n;
  const low = value & U64_MAX;
  view.setBigUint64(offset, high, false);
  view.setBigUint64(offset + 8, low, false);
}

function getBigUint128(view: DataView, offset: number): bigint {
  const high = view.getBigUint64(offset, false);
  const low = view.getBigUint64(offset + 8, false);
  return (high << 64n) | low;
}

function setHex32(buf: Uint8Array, offset: number, hex: Hex32): void {
  if (hex.length !== 66 || !hex.startsWith("0x")) {
    throw new Error(`expected 32-byte hex (66 chars including 0x), got ${hex}`);
  }
  for (let i = 0; i < 32; i++) {
    buf[offset + i] = parseInt(hex.slice(2 + i * 2, 4 + i * 2), 16);
  }
}

function hexFromBytes(buf: Uint8Array, offset: number): Hex32 {
  let s = "0x";
  for (let i = 0; i < 32; i++) {
    s += buf[offset + i]!.toString(16).padStart(2, "0");
  }
  return s as Hex32;
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
  nonce: Hex32;
  /** USDC base units settled. */
  amount: bigint;
  /** Solana slot at which the receipt was created. */
  slot: bigint;
}

/** Keeper view of a single cross-VM bridge cycle. */
export interface BridgeCycle {
  ccipMessageId: Hex32;
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
