// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

/// @title CrossVMIntent
/// @notice Canonical encoding for cross-VM settlement intents bridged
///         between EVM L1s (Tempo, Arc, Megaeth) and Solana via mppsol
///         primitives.
///
/// @dev IMPORTANT: the encoding here uses Solidity `abi.encode` which is
///      NOT compatible with Solana's Borsh serialization. The Solana side
///      currently expects Borsh; before deployment, both sides must agree
///      on a single canonical encoding. The pragmatic v0.2 plan is a
///      manual byte-packed format that both sides can read — see the
///      matching Rust types in `programs/vault/src/lib.rs`.
library CrossVMIntent {
    /// Direction + purpose of the cross-VM intent.
    enum Kind {
        DepositForYield,    // 0 — Tempo → Solana: deposit USDC for yield allocation
        PullbackForPayout,  // 1 — Solana → Tempo: return USDC for merchant payout
        ReceiptAck          // 2 — Solana → Tempo: ack with mppsol Receipt PDA reference
    }

    /// Cross-VM intent payload carried by CCIP message data.
    /// Token amounts are carried separately in CCIP `tokenAmounts`.
    struct Intent {
        uint64 sourceChain;       // CCIP chain selector of origin
        bytes32 sourceAddress;    // EVM address (left-padded) or Solana pubkey
        bytes32 merchant;         // canonical merchant identifier
        uint256 amount;           // USDC base units (6 decimals)
        bytes32 nonce;            // unique per intent — also becomes mppsol Receipt PDA seed
        Kind kind;
    }

    function encode(Intent memory intent) internal pure returns (bytes memory) {
        return abi.encode(intent);
    }

    function decode(bytes memory data) internal pure returns (Intent memory) {
        return abi.decode(data, (Intent));
    }
}
