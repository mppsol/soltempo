// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

/// @title Buffer
/// @notice Tempo-side treasury buffer. Holds liquid balance for merchant
///         payouts and emits bridge intents for amounts above the configured
///         threshold.
contract Buffer {
    address public immutable merchant;
    uint256 public bufferTarget;

    constructor(address _merchant, uint256 _bufferTarget) {
        merchant = _merchant;
        bufferTarget = _bufferTarget;
    }
}
