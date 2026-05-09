// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

import {IRouterClient} from "@chainlink/contracts-ccip/src/v0.8/ccip/interfaces/IRouterClient.sol";
import {Client} from "@chainlink/contracts-ccip/src/v0.8/ccip/libraries/Client.sol";
import {CCIPReceiver} from "@chainlink/contracts-ccip/src/v0.8/ccip/applications/CCIPReceiver.sol";
import {IERC20} from "@chainlink/contracts-ccip/src/v0.8/vendor/openzeppelin-solidity/v5.0.2/contracts/token/ERC20/IERC20.sol";

import {CrossVMIntent} from "./CrossVMIntent.sol";

/// @title Buffer
/// @notice Tempo-side treasury buffer for soltempo. Holds liquid USDC for
///         merchant payouts and emits cross-VM settlement intents (via
///         Chainlink CCIP) to a Solana vault when balance exceeds the
///         configured threshold. Receives pull-backs from Solana via CCIP
///         when payouts deplete the buffer.
///
/// @dev Designed for a single merchant per Buffer instance. Multi-tenant
///      treasury comes in v1.1+ once a single-merchant deployment is real.
contract Buffer is CCIPReceiver {
    /// @notice The merchant who owns this treasury.
    address public immutable merchant;

    /// @notice The USDC token on Tempo.
    IERC20 public immutable usdc;

    /// @notice CCIP chain selector for the Solana destination.
    uint64 public immutable solanaChainSelector;

    /// @notice The Solana vault program/PDA address (Solana addresses are 32 bytes).
    bytes public solanaVaultAddress;

    /// @notice Liquid USDC kept on Tempo for immediate payouts.
    uint256 public bufferTarget;

    /// @notice Monotonic nonce for outbound intents.
    uint256 public nonceCounter;

    event Deposit(address indexed from, uint256 amount);
    event Withdrawal(address indexed to, uint256 amount);
    event IntentSent(bytes32 indexed messageId, bytes32 indexed nonce, uint256 amount);
    event PullbackReceived(bytes32 indexed messageId, uint256 amount);
    event BufferTargetUpdated(uint256 oldTarget, uint256 newTarget);

    error OnlyMerchant();
    error InsufficientBalance();
    error BelowThreshold();
    error InvalidConfig();
    error AmountTooLarge();

    modifier onlyMerchant() {
        if (msg.sender != merchant) revert OnlyMerchant();
        _;
    }

    constructor(
        address ccipRouter,
        address _merchant,
        address _usdc,
        uint64 _solanaChainSelector,
        bytes memory _solanaVaultAddress,
        uint256 _bufferTarget
    ) CCIPReceiver(ccipRouter) {
        if (_merchant == address(0) || _usdc == address(0)) revert InvalidConfig();
        if (_solanaVaultAddress.length != 32) revert InvalidConfig();
        merchant = _merchant;
        usdc = IERC20(_usdc);
        solanaChainSelector = _solanaChainSelector;
        solanaVaultAddress = _solanaVaultAddress;
        bufferTarget = _bufferTarget;
    }

    /// @notice Merchant deposits USDC into the buffer.
    /// @dev Caller must `approve` the Buffer for `amount` of USDC first.
    function deposit(uint256 amount) external onlyMerchant {
        usdc.transferFrom(msg.sender, address(this), amount);
        emit Deposit(msg.sender, amount);
    }

    /// @notice Merchant withdraws USDC from the local buffer for an immediate payout.
    /// @dev If the buffer is insufficient, the keeper must first trigger a
    ///      pull-back from Solana before this call succeeds.
    function withdraw(uint256 amount, address to) external onlyMerchant {
        if (usdc.balanceOf(address(this)) < amount) revert InsufficientBalance();
        usdc.transfer(to, amount);
        emit Withdrawal(to, amount);
    }

    /// @notice Update the liquid buffer target.
    function setBufferTarget(uint256 newTarget) external onlyMerchant {
        emit BufferTargetUpdated(bufferTarget, newTarget);
        bufferTarget = newTarget;
    }

    /// @notice Bridge USDC above the buffer to the Solana vault for yield.
    /// @dev Anyone can call (typically the keeper). Only the buffer-excess is
    ///      bridged, so there's no merchant-funds-at-risk surface even with
    ///      open invocation. Caller pays the CCIP fee in native gas (msg.value).
    function sendIntentToSolana() external payable returns (bytes32 messageId) {
        uint256 balance = usdc.balanceOf(address(this));
        if (balance <= bufferTarget) revert BelowThreshold();
        uint256 amountToSend = balance - bufferTarget;
        if (amountToSend > type(uint128).max) revert AmountTooLarge();

        // Build CCIP token-transfer + intent payload
        Client.EVMTokenAmount[] memory tokenAmounts = new Client.EVMTokenAmount[](1);
        tokenAmounts[0] = Client.EVMTokenAmount({token: address(usdc), amount: amountToSend});

        bytes32 nonce = keccak256(abi.encodePacked(block.chainid, address(this), ++nonceCounter));
        CrossVMIntent.Intent memory intent = CrossVMIntent.Intent({
            sourceChain: uint64(block.chainid),
            amount: uint128(amountToSend),
            sourceAddress: bytes32(uint256(uint160(address(this)))),
            merchant: bytes32(uint256(uint160(merchant))),
            nonce: nonce,
            kind: CrossVMIntent.Kind.DepositForYield
        });

        Client.EVM2AnyMessage memory message = Client.EVM2AnyMessage({
            receiver: solanaVaultAddress,
            data: CrossVMIntent.encode(intent),
            tokenAmounts: tokenAmounts,
            extraArgs: "",          // default execution params
            feeToken: address(0)    // pay fee in native gas
        });

        IRouterClient router = IRouterClient(this.getRouter());
        usdc.approve(address(router), amountToSend);

        messageId = router.ccipSend{value: msg.value}(solanaChainSelector, message);
        emit IntentSent(messageId, nonce, amountToSend);
    }

    /// @notice Receive a pull-back from the Solana vault (CCIP-delivered).
    /// @dev CCIP transfers the tokens to this contract before invoking
    ///      `_ccipReceive`. We just emit the event; the merchant calls
    ///      `withdraw` separately to pull funds from the buffer.
    function _ccipReceive(Client.Any2EVMMessage memory message) internal override {
        // TODO(v0.2): validate message.sourceChainSelector and the abi.decode'd
        //              sender match the Solana vault address we expect.

        uint256 amount = 0;
        if (message.destTokenAmounts.length > 0) {
            amount = message.destTokenAmounts[0].amount;
        }
        emit PullbackReceived(message.messageId, amount);
    }

    /// @dev Allow contract to hold ETH for paying CCIP fees.
    receive() external payable {}
}
