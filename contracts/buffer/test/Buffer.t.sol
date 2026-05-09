// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";
import {Buffer} from "../src/Buffer.sol";
import {CrossVMIntent} from "../src/CrossVMIntent.sol";

/// @notice Minimal Buffer tests — full CCIP integration tests live alongside
///         the chainlink-local mock router and require additional setup.
///         Once `forge install smartcontractkit/chainlink-local` is run, the
///         CCIPLocalSimulator can drive end-to-end send/receive tests in this
///         file.
contract BufferTest is Test {
    address constant MERCHANT = address(0xBEEF);
    address constant USDC = address(0xC0DE);
    address constant ROUTER = address(0xCC1B);

    function _solanaAddr() internal pure returns (bytes memory) {
        // 32-byte placeholder representing a Solana program/PDA address.
        return abi.encodePacked(bytes32(uint256(0xA1B2C3D4)));
    }

    function test_construction() public {
        Buffer b = new Buffer(ROUTER, MERCHANT, USDC, 124615329519749607, _solanaAddr(), 1_000e6);
        assertEq(b.merchant(), MERCHANT);
        assertEq(address(b.usdc()), USDC);
        assertEq(b.bufferTarget(), 1_000e6);
        assertEq(b.solanaChainSelector(), 124615329519749607);
    }

    function test_invalidConfig_reverts() public {
        vm.expectRevert(Buffer.InvalidConfig.selector);
        new Buffer(ROUTER, address(0), USDC, 1, _solanaAddr(), 0);

        vm.expectRevert(Buffer.InvalidConfig.selector);
        new Buffer(ROUTER, MERCHANT, address(0), 1, _solanaAddr(), 0);

        bytes memory wrongLength = hex"deadbeef";
        vm.expectRevert(Buffer.InvalidConfig.selector);
        new Buffer(ROUTER, MERCHANT, USDC, 1, wrongLength, 0);
    }

    function test_intentEncoding_roundtrip() public pure {
        CrossVMIntent.Intent memory intent = CrossVMIntent.Intent({
            sourceChain: 124615329519749607,
            sourceAddress: bytes32(uint256(uint160(MERCHANT))),
            merchant: bytes32(uint256(uint160(MERCHANT))),
            amount: 1_000_000_000,
            nonce: keccak256("test-nonce"),
            kind: CrossVMIntent.Kind.DepositForYield
        });
        bytes memory encoded = CrossVMIntent.encode(intent);
        CrossVMIntent.Intent memory decoded = CrossVMIntent.decode(encoded);
        assertEq(decoded.amount, intent.amount);
        assertEq(decoded.nonce, intent.nonce);
        assertEq(uint256(decoded.kind), uint256(intent.kind));
    }
}
