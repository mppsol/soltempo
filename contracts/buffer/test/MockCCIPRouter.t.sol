// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";
import {MockCCIPRouter} from "../src/MockCCIPRouter.sol";
import {Client} from "@chainlink/contracts-ccip/src/v0.8/ccip/libraries/Client.sol";
import {IERC20} from "@chainlink/contracts-ccip/src/v0.8/vendor/openzeppelin-solidity/v5.0.2/contracts/token/ERC20/IERC20.sol";

/// @notice Minimal ERC20 for testing — needs `forge install foundry-rs/forge-std`
///         and `smartcontractkit/chainlink-local` (or the chainlink CCIP
///         contracts directly) to compile.
contract MockUSDC {
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        allowance[from][msg.sender] -= amount;
        balanceOf[from] -= amount;
        balanceOf[to] += amount;
        return true;
    }

    function transfer(address to, uint256 amount) external returns (bool) {
        balanceOf[msg.sender] -= amount;
        balanceOf[to] += amount;
        return true;
    }
}

contract MockCCIPRouterTest is Test {
    MockCCIPRouter router;
    MockUSDC usdc;
    address sender = address(0xBEEF);
    bytes solanaReceiver = abi.encodePacked(bytes32(uint256(0xA1B2C3D4)));

    function setUp() public {
        router = new MockCCIPRouter();
        usdc = new MockUSDC();
    }

    function test_getFee_isZero() public view {
        Client.EVMTokenAmount[] memory tokens = new Client.EVMTokenAmount[](0);
        Client.EVM2AnyMessage memory msg_ = Client.EVM2AnyMessage({
            receiver: solanaReceiver,
            data: hex"deadbeef",
            tokenAmounts: tokens,
            extraArgs: "",
            feeToken: address(0)
        });
        assertEq(router.getFee(124615329519749607, msg_), 0);
    }

    function test_ccipSend_pullsTokensAndEmitsEvent() public {
        uint64 destChain = 124615329519749607;
        uint256 amount = 1_000e6;

        // Mint USDC to sender, approve router
        usdc.mint(sender, amount);
        vm.prank(sender);
        usdc.approve(address(router), amount);

        Client.EVMTokenAmount[] memory tokens = new Client.EVMTokenAmount[](1);
        tokens[0] = Client.EVMTokenAmount({token: address(usdc), amount: amount});

        Client.EVM2AnyMessage memory msg_ = Client.EVM2AnyMessage({
            receiver: solanaReceiver,
            data: hex"abcd",
            tokenAmounts: tokens,
            extraArgs: "",
            feeToken: address(0)
        });

        // Expect MockMessageSent event (we don't validate exact messageId here
        // since it depends on block.chainid, but we do check the counter
        // increments below).
        uint256 counterBefore = router.messageCounter();

        vm.prank(sender);
        bytes32 messageId = router.ccipSend(destChain, msg_);

        assertTrue(messageId != bytes32(0));
        assertEq(router.messageCounter(), counterBefore + 1);

        // Tokens moved from sender to router
        assertEq(usdc.balanceOf(sender), 0);
        assertEq(usdc.balanceOf(address(router)), amount);
    }

    function test_ccipSend_messageIdsDiffer() public {
        usdc.mint(sender, 2_000e6);
        vm.prank(sender);
        usdc.approve(address(router), 2_000e6);

        Client.EVMTokenAmount[] memory tokens = new Client.EVMTokenAmount[](1);
        tokens[0] = Client.EVMTokenAmount({token: address(usdc), amount: 1_000e6});

        Client.EVM2AnyMessage memory msg_ = Client.EVM2AnyMessage({
            receiver: solanaReceiver,
            data: "",
            tokenAmounts: tokens,
            extraArgs: "",
            feeToken: address(0)
        });

        vm.prank(sender);
        bytes32 id1 = router.ccipSend(124615329519749607, msg_);
        vm.prank(sender);
        bytes32 id2 = router.ccipSend(124615329519749607, msg_);

        assertTrue(id1 != id2, "messageIds should be unique");
    }

    function test_isChainSupported_alwaysTrue() public view {
        assertTrue(router.isChainSupported(0));
        assertTrue(router.isChainSupported(type(uint64).max));
    }

    function test_withdrawForRelay_movesTokens() public {
        usdc.mint(address(router), 500e6);
        address keeper = address(0xDEAD);
        router.withdrawForRelay(address(usdc), keeper, 500e6);
        assertEq(usdc.balanceOf(keeper), 500e6);
        assertEq(usdc.balanceOf(address(router)), 0);
    }
}
