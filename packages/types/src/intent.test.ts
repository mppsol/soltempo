import { test } from "node:test";
import assert from "node:assert/strict";

import {
  CrossVMIntent,
  INTENT_ENCODED_LENGTH,
  INTENT_VERSION,
  IntentKind,
  decodeIntent,
  encodeIntent,
} from "./index.js";

/**
 * Shared cross-language test vector — the matching Solidity test in
 * `contracts/buffer/test/Buffer.t.sol::CANONICAL_HEX_VECTOR` and the Rust
 * test in `programs/vault/src/lib.rs::CANONICAL_HEX_VECTOR` decode/encode
 * the same byte string. If any implementation diverges, its per-language
 * test fails.
 */
const CANONICAL_HEX_VECTOR =
  "010000000000000000010000000000000000000000003b9aca00000000000000000000000000beefbeefbeefbeefbeefbeefbeefbeefbeefbeef000000000000000000000000beefbeefbeefbeefbeefbeefbeefbeefbeefbeeffeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeed";

function hexToBytes(s: string): Uint8Array {
  const out = new Uint8Array(s.length / 2);
  for (let i = 0; i < out.length; i++) {
    out[i] = parseInt(s.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

function vectorIntent(): CrossVMIntent {
  return {
    sourceChain: 1n,
    amount: 1_000_000_000n,
    sourceAddress:
      "0x000000000000000000000000beefbeefbeefbeefbeefbeefbeefbeefbeefbeef",
    merchant:
      "0x000000000000000000000000beefbeefbeefbeefbeefbeefbeefbeefbeefbeef",
    nonce:
      "0xfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeed",
    kind: IntentKind.DepositForYield,
  };
}

test("encoded length is 122", () => {
  const encoded = encodeIntent(vectorIntent());
  assert.equal(encoded.length, INTENT_ENCODED_LENGTH);
  assert.equal(encoded.length, 122);
});

test("encodes to canonical vector", () => {
  const encoded = encodeIntent(vectorIntent());
  const expected = hexToBytes(CANONICAL_HEX_VECTOR);
  assert.deepEqual(encoded, expected);
});

test("decodes canonical vector", () => {
  const bytes = hexToBytes(CANONICAL_HEX_VECTOR);
  const decoded = decodeIntent(bytes);
  assert.deepEqual(decoded, vectorIntent());
});

test("round trip", () => {
  const original = vectorIntent();
  const encoded = encodeIntent(original);
  const decoded = decodeIntent(encoded);
  assert.deepEqual(decoded, original);
});

test("rejects wrong length", () => {
  assert.throws(() => decodeIntent(new Uint8Array([0x01, 0x00, 0x00])));
});

test("rejects wrong version", () => {
  const bytes = new Uint8Array(INTENT_ENCODED_LENGTH);
  bytes[0] = 0x99;
  assert.throws(() => decodeIntent(bytes));
});

test("rejects invalid kind", () => {
  const bytes = new Uint8Array(INTENT_ENCODED_LENGTH);
  bytes[0] = INTENT_VERSION;
  bytes[1] = 0x07;
  assert.throws(() => decodeIntent(bytes));
});

test("rejects amount > u128 max", () => {
  const intent = vectorIntent();
  intent.amount = 1n << 128n;
  assert.throws(() => encodeIntent(intent));
});

test("rejects sourceChain > u64 max", () => {
  const intent = vectorIntent();
  intent.sourceChain = 1n << 64n;
  assert.throws(() => encodeIntent(intent));
});
