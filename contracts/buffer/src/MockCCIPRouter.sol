// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

import {IRouterClient} from "@chainlink/contracts-ccip/src/v0.8/ccip/interfaces/IRouterClient.sol";
import {Client} from "@chainlink/contracts-ccip/src/v0.8/ccip/libraries/Client.sol";
import {IERC20} from "@chainlink/contracts-ccip/src/v0.8/vendor/openzeppelin-solidity/v5.0.2/contracts/token/ERC20/IERC20.sol";

/// @title MockCCIPRouter
/// @notice Demo-only mock of Chainlink's CCIP router for Tempo testnet
///         (Moderato), where the real CCIP router is not yet deployed
///         (Andantino was decommissioned per docs.chain.link/ccip/directory
///         /testnet/chain/tempo-testnet, and Moderato hasn't been added).
///
///         Buffer.sol points at this address as its router. When Buffer
///         calls ccipSend, the mock pulls the tokens (mimicking real CCIP
///         taking custody) and emits MockMessageSent. An off-chain keeper
///         observes the event and executes the destination-chain side
///         (calls vault.ccip_receive on Solana devnet with the
///         reconstructed message + transfers the bridged USDC).
///
/// @dev    NOT FOR PRODUCTION. The trust model is "trusted off-chain
///         keeper" instead of Chainlink CCIP's secured router. Swap the
///         router address to the real Chainlink router when CCIP on
///         Tempo testnet ships — no Buffer.sol changes needed.
contract MockCCIPRouter is IRouterClient {
    uint256 public messageCounter;

    event MockMessageSent(
        bytes32 indexed messageId,
        uint64 indexed destChainSelector,
        address indexed sender,
        bytes data,
        uint256 amount,
        address token,
        bytes receiver
    );

    /// @notice Fee is always 0 for the demo. Real CCIP charges in LINK or
    ///         native gas; for the trusted-keeper variant the keeper
    ///         absorbs the destination-chain costs out-of-band.
    function getFee(uint64, Client.EVM2AnyMessage memory)
        external
        pure
        returns (uint256)
    {
        return 0;
    }

    /// @notice Pulls tokens from sender (mimicking CCIP's custody) and
    ///         emits an event the keeper can observe. Returns a
    ///         deterministic messageId.
    function ccipSend(uint64 destChainSelector, Client.EVM2AnyMessage memory message)
        external
        payable
        returns (bytes32 messageId)
    {
        uint256 amount = 0;
        address token = address(0);
        if (message.tokenAmounts.length > 0) {
            token = message.tokenAmounts[0].token;
            amount = message.tokenAmounts[0].amount;
            // Sender (Buffer) approved us in sendIntentToSolana before calling.
            IERC20(token).transferFrom(msg.sender, address(this), amount);
        }

        unchecked {
            messageCounter++;
        }
        messageId = keccak256(
            abi.encodePacked(block.chainid, address(this), messageCounter)
        );

        emit MockMessageSent(
            messageId,
            destChainSelector,
            msg.sender,
            message.data,
            amount,
            token,
            message.receiver
        );
    }

    function isChainSupported(uint64) external pure returns (bool) {
        return true;
    }

    function getSupportedTokens(uint64)
        external
        pure
        returns (address[] memory)
    {
        return new address[](0);
    }

    /// @notice Demo-only: lets the keeper pull tokens out of the mock
    ///         router after observing MockMessageSent. In real CCIP this
    ///         flow doesn't exist; tokens are minted on the destination
    ///         chain by the offramp.
    function withdrawForRelay(address token, address to, uint256 amount) external {
        // No access control on the mock — keeper is trusted in this demo.
        // For a real testnet deployment that's not throwaway, gate this on
        // a configured keeper address.
        IERC20(token).transfer(to, amount);
    }
}
