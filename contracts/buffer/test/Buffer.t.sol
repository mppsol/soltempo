// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";
import {Buffer} from "../src/Buffer.sol";
import {CrossVMIntent} from "../src/CrossVMIntent.sol";

/// @notice Tests for Buffer construction + CrossVMIntent canonical encoding.
///         The hex vector in `test_canonicalEncoding_vector` is the
///         shared cross-language test vector — the matching Rust test in
///         `programs/vault` and the TS test in `packages/types/` both
///         decode/encode against this exact byte string. If any of the three
///         diverges, the test fails on that side.
contract BufferTest is Test {
    address constant MERCHANT = address(0xBEEF);
    address constant USDC = address(0xC0DE);
    address constant ROUTER = address(0xCC1B);

    /// Shared cross-language test vector — see CrossVMIntent.sol for the layout.
    /// Matches `CANONICAL_HEX_VECTOR` in:
    ///   - programs/vault/src/lib.rs
    ///   - packages/types/src/index.ts
    bytes constant CANONICAL_HEX_VECTOR =
        hex"010000000000000000010000000000000000000000003b9aca00000000000000000000000000beefbeefbeefbeefbeefbeefbeefbeefbeefbeef000000000000000000000000beefbeefbeefbeefbeefbeefbeefbeefbeefbeeffeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeed";

    function _solanaAddr() internal pure returns (bytes memory) {
        return abi.encodePacked(bytes32(uint256(0xA1B2C3D4)));
    }

    function _vectorIntent() internal pure returns (CrossVMIntent.Intent memory) {
        return CrossVMIntent.Intent({
            sourceChain: 1,
            amount: 1_000_000_000,
            sourceAddress: bytes32(uint256(0xbeefbeefbeefbeefbeefbeefbeefbeefbeefbeef)),
            merchant: bytes32(uint256(0xbeefbeefbeefbeefbeefbeefbeefbeefbeefbeef)),
            nonce: bytes32(uint256(0xfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeed)),
            kind: CrossVMIntent.Kind.DepositForYield
        });
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

    function test_canonicalEncoding_length() public pure {
        bytes memory encoded = CrossVMIntent.encode(_vectorIntent());
        assertEq(encoded.length, CrossVMIntent.ENCODED_LENGTH);
        assertEq(encoded.length, 122);
    }

    function test_canonicalEncoding_vector() public pure {
        bytes memory encoded = CrossVMIntent.encode(_vectorIntent());
        assertEq(encoded, CANONICAL_HEX_VECTOR);
    }

    function test_canonicalEncoding_decodeVector() public pure {
        CrossVMIntent.Intent memory decoded = CrossVMIntent.decode(CANONICAL_HEX_VECTOR);
        assertEq(decoded.sourceChain, 1);
        assertEq(decoded.amount, 1_000_000_000);
        assertEq(decoded.sourceAddress, bytes32(uint256(0xbeefbeefbeefbeefbeefbeefbeefbeefbeefbeef)));
        assertEq(decoded.merchant, bytes32(uint256(0xbeefbeefbeefbeefbeefbeefbeefbeefbeefbeef)));
        assertEq(decoded.nonce, bytes32(uint256(0xfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeed)));
        assertEq(uint256(decoded.kind), uint256(CrossVMIntent.Kind.DepositForYield));
    }

    function test_canonicalEncoding_roundtrip() public pure {
        CrossVMIntent.Intent memory original = _vectorIntent();
        bytes memory encoded = CrossVMIntent.encode(original);
        CrossVMIntent.Intent memory decoded = CrossVMIntent.decode(encoded);
        assertEq(decoded.sourceChain, original.sourceChain);
        assertEq(decoded.amount, original.amount);
        assertEq(decoded.sourceAddress, original.sourceAddress);
        assertEq(decoded.merchant, original.merchant);
        assertEq(decoded.nonce, original.nonce);
        assertEq(uint256(decoded.kind), uint256(original.kind));
    }

    function test_decode_invalidLength_reverts() public {
        bytes memory wrong = hex"01020304";
        vm.expectRevert(abi.encodeWithSelector(CrossVMIntent.InvalidLength.selector, uint256(4)));
        CrossVMIntent.decode(wrong);
    }

    function test_decode_invalidVersion_reverts() public {
        bytes memory wrongVersion = new bytes(122);
        wrongVersion[0] = bytes1(uint8(0x99));
        vm.expectRevert(abi.encodeWithSelector(CrossVMIntent.UnsupportedVersion.selector, uint8(0x99)));
        CrossVMIntent.decode(wrongVersion);
    }

    function test_decode_invalidKind_reverts() public {
        bytes memory wrongKind = new bytes(122);
        wrongKind[0] = bytes1(uint8(0x01));
        wrongKind[1] = bytes1(uint8(0x07));  // kind > 2
        vm.expectRevert(abi.encodeWithSelector(CrossVMIntent.InvalidKind.selector, uint8(0x07)));
        CrossVMIntent.decode(wrongKind);
    }
}
