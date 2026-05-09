// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

/// @title CrossVMIntent
/// @notice Canonical wire-format for cross-VM settlement intents bridged
///         between EVM L1s (Tempo, Arc, Megaeth) and Solana via mppsol
///         primitives. Both sides read/write the same fixed 122-byte layout.
///
/// @dev v0.1 encoding (version byte = 0x01):
///
///        Offset  Size  Field
///        ─────────────────────────────────────────────────────────────
///        0       1     version          (= 0x01)
///        1       1     kind             (0=DepositForYield, 1=PullbackForPayout, 2=ReceiptAck)
///        2       8     sourceChain      (BE u64 — CCIP chain selector)
///        10      16    amount           (BE u128 — USDC base units, 6 decimals)
///        26      32    sourceAddress    (EVM address left-padded, OR Solana pubkey)
///        58      32    merchant         (canonical merchant identifier)
///        90      32    nonce            (also seeds the mppsol Receipt PDA)
///        ─────────────────────────────────────────────────────────────
///        Total: 122 bytes, big-endian, no padding.
///
///         The matching Rust implementation lives in
///        `programs/vault/src/lib.rs` (CrossVMIntentPayload). The matching
///        TS implementation lives in `packages/types/src/index.ts`. All
///        three are tested against a shared hex vector — see
///        `test/Buffer.t.sol::test_canonicalEncoding_vector`.
library CrossVMIntent {
    /// Wire-format version. Bump when the layout changes.
    uint8 internal constant VERSION = 0x01;

    /// Encoded length in bytes — exact, no variable fields.
    uint256 internal constant ENCODED_LENGTH = 122;

    /// Direction + purpose of the cross-VM intent.
    enum Kind {
        DepositForYield,    // 0 — Tempo → Solana: deposit USDC for yield allocation
        PullbackForPayout,  // 1 — Solana → Tempo: return USDC for merchant payout
        ReceiptAck          // 2 — Solana → Tempo: ack with mppsol Receipt PDA reference
    }

    /// Cross-VM intent payload carried by CCIP message data.
    /// Token amounts also ride separately in CCIP `tokenAmounts` for the
    /// CCIP token-transfer flow; the `amount` field here is the canonical
    /// settlement amount the receiver should treat as authoritative.
    struct Intent {
        uint64 sourceChain;     // CCIP chain selector of origin
        uint128 amount;         // USDC base units (6 decimals); cap = 2^128 - 1
        bytes32 sourceAddress;  // EVM address (left-padded) or Solana pubkey
        bytes32 merchant;       // canonical merchant identifier
        bytes32 nonce;          // unique per intent — also seeds mppsol Receipt PDA
        Kind kind;
    }

    error InvalidLength(uint256 got);
    error UnsupportedVersion(uint8 got);
    error InvalidKind(uint8 got);

    /// Encode an intent to the canonical 122-byte layout.
    function encode(Intent memory intent) internal pure returns (bytes memory) {
        return bytes.concat(
            bytes1(VERSION),
            bytes1(uint8(intent.kind)),
            bytes8(intent.sourceChain),
            bytes16(intent.amount),
            intent.sourceAddress,
            intent.merchant,
            intent.nonce
        );
    }

    /// Decode a canonical 122-byte buffer into an Intent.
    function decode(bytes memory data) internal pure returns (Intent memory intent) {
        if (data.length != ENCODED_LENGTH) revert InvalidLength(data.length);
        if (uint8(data[0]) != VERSION) revert UnsupportedVersion(uint8(data[0]));

        uint8 kindByte = uint8(data[1]);
        if (kindByte > uint8(Kind.ReceiptAck)) revert InvalidKind(kindByte);
        intent.kind = Kind(kindByte);

        intent.sourceChain = _readUint64(data, 2);
        intent.amount = _readUint128(data, 10);
        intent.sourceAddress = _readBytes32(data, 26);
        intent.merchant = _readBytes32(data, 58);
        intent.nonce = _readBytes32(data, 90);
    }

    function _readUint64(bytes memory data, uint256 offset) private pure returns (uint64 v) {
        for (uint256 i = 0; i < 8; i++) {
            v = (v << 8) | uint64(uint8(data[offset + i]));
        }
    }

    function _readUint128(bytes memory data, uint256 offset) private pure returns (uint128 v) {
        for (uint256 i = 0; i < 16; i++) {
            v = (v << 8) | uint128(uint8(data[offset + i]));
        }
    }

    function _readBytes32(bytes memory data, uint256 offset) private pure returns (bytes32 v) {
        // Solidity `bytes memory` has a 32-byte length prefix; actual data starts at
        // memory(data) + 32. So the byte at logical offset `o` lives at memory(data) + 32 + o.
        // mload reads 32 bytes starting at the given pointer.
        assembly {
            v := mload(add(data, add(32, offset)))
        }
    }
}
