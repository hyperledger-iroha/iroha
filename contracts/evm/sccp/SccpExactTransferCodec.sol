// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.7.4;

/**
 * @title SccpExactTransferCodec
 * @dev Canonical SCCP V1 transfer, exact-lane, and hash construction shared by
 * concrete EVM/TVM route contracts. Every function is internal; this library
 * cannot expose a generic source-emission entry point.
 */
library SccpExactTransferCodec {
    bytes internal constant PAYLOAD_HASH_PREFIX = "sccp:payload:v1";
    bytes internal constant LANE_HASH_PREFIX = "sccp:lane-id:v1";
    bytes internal constant MESSAGE_ID_PREFIX = "sccp:lane-message-id:v1";
    bytes internal constant SOURCE_EVENT_PREFIX = "sccp:source:event:v1";

    uint8 internal constant CODEC_CANONICAL_TEXT = 1;
    uint8 internal constant CODEC_EVM_ADDRESS20 = 2;
    uint8 internal constant CODEC_TRON_ADDRESS21 = 5;

    uint256 internal constant MAX_TEXT_BYTES = 256;
    uint256 internal constant MAX_U128 = (uint256(1) << 128) - 1;

    struct TransferFields {
        uint32 sourceDomain;
        uint32 destinationDomain;
        uint64 nonce;
        uint32 routeRevision;
        uint32 assetHomeDomain;
        bytes assetId;
        uint256 amount;
        uint8 senderCodec;
        bytes sender;
        uint8 recipientCodec;
        bytes recipient;
        bytes routeId;
    }

    function ethereumNetwork(uint8 profile) internal pure returns (bytes memory) {
        require(profile == 2 || profile == 3, "Unsupported Ethereum profile");
        uint64 chainId = profile == 2 ? uint64(1) : uint64(11155111);
        return abi.encodePacked(bytes1(0x01), bytes1(profile), u32le(1), u64le(chainId));
    }

    function bscNetwork(uint8 profile) internal pure returns (bytes memory) {
        require(profile == 4 || profile == 5, "Unsupported BSC profile");
        uint64 chainId = profile == 4 ? uint64(56) : uint64(97);
        return abi.encodePacked(bytes1(0x01), bytes1(profile), u32le(2), u64le(chainId));
    }

    function tronNetwork(uint8 profile) internal pure returns (bytes memory) {
        uint32 networkId;
        if (profile == 10) networkId = 0x2b6653dc;
        else if (profile == 11) networkId = 0xcd8690dc;
        else if (profile == 12) networkId = 0x94a9059e;
        else revert("Unsupported TRON profile");
        return abi.encodePacked(bytes1(0x01), bytes1(profile), u32le(5), u32le(networkId));
    }

    function tairaNetwork() internal pure returns (bytes memory) {
        return hex"010100000000809574f5fee75e69bfcf52451e42d50f";
    }

    function lane(bytes memory source, bytes memory target) internal pure returns (bytes memory) {
        return abi.encodePacked(bytes1(0x01), u32le(uint32(source.length)), source,
            u32le(uint32(target.length)), target);
    }

    function laneHash(bytes memory canonicalLane) internal pure returns (bytes32) {
        return blake2b256(abi.encodePacked(LANE_HASH_PREFIX, canonicalLane));
    }

    /** Hash one lane on EVM networks through the EIP-152 BLAKE2F precompile. */
    function laneHashEvm(bytes memory canonicalLane) internal view returns (bytes32) {
        return blake2b256Evm(abi.encodePacked(LANE_HASH_PREFIX, canonicalLane));
    }

    function messageId(bytes memory canonicalLane, bytes memory payload)
        internal
        pure
        returns (bytes32)
    {
        return keccak256(abi.encodePacked(MESSAGE_ID_PREFIX, bytes1(0x01),
            u32le(uint32(canonicalLane.length)), canonicalLane,
            u32le(uint32(payload.length)), payload));
    }

    function payloadHash(bytes memory payload) internal pure returns (bytes32) {
        return blake2b256(abi.encodePacked(PAYLOAD_HASH_PREFIX, payload));
    }

    /** Hash one payload on EVM networks through the EIP-152 BLAKE2F precompile. */
    function payloadHashEvm(bytes memory payload) internal view returns (bytes32) {
        return blake2b256Evm(abi.encodePacked(PAYLOAD_HASH_PREFIX, payload));
    }

    function sourceEventDigest(bytes32 exactLaneHash, bytes32 exactMessageId, bytes32 exactPayloadHash)
        internal
        pure
        returns (bytes32)
    {
        require(exactLaneHash != bytes32(0) && exactMessageId != bytes32(0)
            && exactPayloadHash != bytes32(0), "Zero SCCP hash role");
        require(exactLaneHash != exactMessageId && exactLaneHash != exactPayloadHash
            && exactMessageId != exactPayloadHash, "SCCP hash role collision");
        return keccak256(abi.encodePacked(SOURCE_EVENT_PREFIX, bytes1(0x01), exactLaneHash,
            exactMessageId, exactPayloadHash));
    }

    function transferPayload(TransferFields memory fields) internal pure returns (bytes memory) {
        require(fields.amount != 0 && fields.amount <= MAX_U128, "Amount exceeds SCCP u128");
        require(fields.routeRevision != 0, "Route revision is required");
        require(isCanonicalText(fields.assetId) && isCanonicalText(fields.routeId),
            "Noncanonical route text");
        bytes memory header = abi.encodePacked(
            bytes1(0x02), // SccpPayloadV1::Transfer
            bytes1(0x01),
            u32le(fields.sourceDomain),
            u32le(fields.destinationDomain),
            u64le(fields.nonce),
            u32le(fields.routeRevision),
            u32le(fields.assetHomeDomain),
            bytes1(CODEC_CANONICAL_TEXT),
            vec(fields.assetId),
            u128le(uint128(fields.amount))
        );
        bytes memory accounts = abi.encodePacked(
            bytes1(fields.senderCodec),
            vec(fields.sender),
            bytes1(fields.recipientCodec),
            vec(fields.recipient)
        );
        return abi.encodePacked(
            header,
            accounts,
            bytes1(CODEC_CANONICAL_TEXT),
            vec(fields.routeId)
        );
    }

    function isCanonicalText(bytes memory value) internal pure returns (bool) {
        if (value.length == 0 || value.length > MAX_TEXT_BYTES) return false;
        for (uint256 i = 0; i < value.length; i++) {
            uint8 character = uint8(value[i]);
            if (character < 0x21 || character > 0x7e) return false;
        }
        return true;
    }

    function vec(bytes memory value) internal pure returns (bytes memory) {
        require(value.length <= uint256(type(uint32).max), "SCCP vector too long");
        return abi.encodePacked(u32le(uint32(value.length)), value);
    }

    function u32le(uint32 value) internal pure returns (bytes4) {
        return bytes4(
            (uint32(uint8(value)) << 24)
            | (uint32(uint8(value >> 8)) << 16)
            | (uint32(uint8(value >> 16)) << 8)
            | uint32(uint8(value >> 24))
        );
    }

    function u64le(uint64 value) internal pure returns (bytes8) {
        uint64 reversed;
        for (uint256 i = 0; i < 8; i++) {
            reversed |= uint64(uint8(value >> uint64(i * 8))) << uint64((7 - i) * 8);
        }
        return bytes8(reversed);
    }

    function u128le(uint128 value) internal pure returns (bytes16) {
        uint128 reversed;
        for (uint256 i = 0; i < 16; i++) {
            reversed |= uint128(uint8(value >> uint128(i * 8))) << uint128((15 - i) * 8);
        }
        return bytes16(reversed);
    }

    /**
     * Compute BLAKE2b-256 with EIP-152's compression precompile at address 0x09.
     *
     * This path is EVM-only. TVM assigns address 0x09 to BatchValidateSign, so
     * TRON routes must continue to call the software `blake2b256` function.
     */
    function blake2b256Evm(bytes memory input) internal view returns (bytes32 output) {
        bytes memory state =
            hex"28c9bdf267e6096a3ba7ca8485ae67bb2bf894fe72f36e3cf1361d5f3af54fa5d182e6ad7f520e511f6c3e2b8c68059b6bbd41fbabd9831f79217e1319cde05b";
        uint256 blocks = input.length == 0 ? 1 : (input.length + 127) / 128;
        uint64 counterLow = 0;
        uint64 counterHigh = 0;
        for (uint256 blockIndex = 0; blockIndex < blocks; blockIndex++) {
            uint256 inputOffset = blockIndex * 128;
            uint256 remaining = input.length > inputOffset ? input.length - inputOffset : 0;
            uint256 blockLength = remaining > 128 ? 128 : remaining;
            uint64 previous = counterLow;
            counterLow += uint64(blockLength);
            if (counterLow < previous) counterHigh += 1;

            bytes memory arguments = new bytes(213);
            arguments[3] = bytes1(uint8(12));
            for (uint256 i = 0; i < 64; i++) arguments[4 + i] = state[i];
            for (uint256 i = 0; i < blockLength; i++) {
                arguments[68 + i] = input[inputOffset + i];
            }
            _writeU64Le(arguments, 196, counterLow);
            _writeU64Le(arguments, 204, counterHigh);
            arguments[212] = blockIndex + 1 == blocks ? bytes1(uint8(1)) : bytes1(uint8(0));

            bytes memory nextState = new bytes(64);
            bool success;
            uint256 returnSize;
            assembly {
                success := staticcall(gas(), 0x09, add(arguments, 32), 213, add(nextState, 32), 64)
                returnSize := returndatasize()
            }
            require(success && returnSize == 64, "BLAKE2F precompile unavailable");
            state = nextState;
        }
        assembly { output := mload(add(state, 32)) }
    }

    function _writeU64Le(bytes memory target, uint256 offset, uint64 value) private pure {
        for (uint256 i = 0; i < 8; i++) {
            target[offset + i] = bytes1(uint8(value >> uint64(i * 8)));
        }
    }

    function blake2b256(bytes memory input) internal pure returns (bytes32) {
        uint64[8] memory h;
        for (uint256 i = 0; i < 8; i++) h[i] = _iv(i);
        h[0] ^= uint64(0x01010020);
        uint256 blocks = input.length == 0 ? 1 : (input.length + 127) / 128;
        uint64 counterLow = 0;
        uint64 counterHigh = 0;
        for (uint256 blockIndex = 0; blockIndex < blocks; blockIndex++) {
            uint256 offset = blockIndex * 128;
            uint256 remaining = input.length > offset ? input.length - offset : 0;
            uint256 blockLength = remaining > 128 ? 128 : remaining;
            uint64 previous = counterLow;
            counterLow += uint64(blockLength);
            if (counterLow < previous) counterHigh += 1;
            uint64[16] memory words = _loadBlock(input, offset);
            _compress(h, words, counterLow, counterHigh, blockIndex + 1 == blocks);
        }
        bytes memory digest = new bytes(32);
        for (uint256 i = 0; i < 4; i++) {
            for (uint256 j = 0; j < 8; j++) digest[i * 8 + j] = bytes1(uint8(h[i] >> (j * 8)));
        }
        bytes32 output;
        assembly { output := mload(add(digest, 32)) }
        return output;
    }

    function _loadBlock(bytes memory input, uint256 offset)
        private pure returns (uint64[16] memory words)
    {
        for (uint256 wordIndex = 0; wordIndex < 16; wordIndex++) {
            uint64 word;
            for (uint256 byteIndex = 0; byteIndex < 8; byteIndex++) {
                uint256 position = offset + wordIndex * 8 + byteIndex;
                if (position < input.length) {
                    word |= uint64(uint8(input[position])) << uint64(byteIndex * 8);
                }
            }
            words[wordIndex] = word;
        }
    }

    function _compress(
        uint64[8] memory h,
        uint64[16] memory m,
        uint64 counterLow,
        uint64 counterHigh,
        bool finalBlock
    ) private pure {
        uint64[16] memory v;
        for (uint256 i = 0; i < 8; i++) { v[i] = h[i]; v[i + 8] = _iv(i); }
        v[12] ^= counterLow;
        v[13] ^= counterHigh;
        if (finalBlock) v[14] ^= uint64(-1);
        for (uint256 round = 0; round < 12; round++) {
            uint8[16] memory s = _sigma(round);
            _g(v, 0, 4, 8, 12, m[s[0]], m[s[1]]);
            _g(v, 1, 5, 9, 13, m[s[2]], m[s[3]]);
            _g(v, 2, 6, 10, 14, m[s[4]], m[s[5]]);
            _g(v, 3, 7, 11, 15, m[s[6]], m[s[7]]);
            _g(v, 0, 5, 10, 15, m[s[8]], m[s[9]]);
            _g(v, 1, 6, 11, 12, m[s[10]], m[s[11]]);
            _g(v, 2, 7, 8, 13, m[s[12]], m[s[13]]);
            _g(v, 3, 4, 9, 14, m[s[14]], m[s[15]]);
        }
        for (uint256 i = 0; i < 8; i++) h[i] ^= v[i] ^ v[i + 8];
    }

    function _g(uint64[16] memory v, uint256 a, uint256 b, uint256 c, uint256 d, uint64 x, uint64 y)
        private pure
    {
        v[a] = v[a] + v[b] + x;
        v[d] = _rotr(v[d] ^ v[a], 32);
        v[c] = v[c] + v[d];
        v[b] = _rotr(v[b] ^ v[c], 24);
        v[a] = v[a] + v[b] + y;
        v[d] = _rotr(v[d] ^ v[a], 16);
        v[c] = v[c] + v[d];
        v[b] = _rotr(v[b] ^ v[c], 63);
    }

    function _rotr(uint64 value, uint256 shift) private pure returns (uint64) {
        return (value >> uint64(shift)) | (value << uint64(64 - shift));
    }

    function _iv(uint256 index) private pure returns (uint64) {
        if (index == 0) return 0x6a09e667f3bcc908;
        if (index == 1) return 0xbb67ae8584caa73b;
        if (index == 2) return 0x3c6ef372fe94f82b;
        if (index == 3) return 0xa54ff53a5f1d36f1;
        if (index == 4) return 0x510e527fade682d1;
        if (index == 5) return 0x9b05688c2b3e6c1f;
        if (index == 6) return 0x1f83d9abfb41bd6b;
        if (index == 7) return 0x5be0cd19137e2179;
        revert("BLAKE2b IV index");
    }

    function _sigma(uint256 round) private pure returns (uint8[16] memory s) {
        uint8[160] memory all = [
            0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,
            14,10,4,8,9,15,13,6,1,12,0,2,11,7,5,3,
            11,8,12,0,5,2,15,13,10,14,3,6,7,1,9,4,
            7,9,3,1,13,12,11,14,2,6,5,10,4,0,15,8,
            9,0,5,7,2,4,10,15,14,1,11,12,6,8,3,13,
            2,12,6,10,0,11,8,3,4,13,7,5,15,14,1,9,
            12,5,1,15,14,13,4,10,0,7,6,3,9,2,8,11,
            13,11,7,14,12,1,3,9,5,0,15,4,8,6,2,10,
            6,15,14,9,11,3,0,8,12,2,13,7,1,4,10,5,
            10,2,8,4,7,6,1,5,15,11,9,14,3,12,13,0
        ];
        uint256 offset = (round % 10) * 16;
        for (uint256 i = 0; i < 16; i++) s[i] = all[offset + i];
    }
}
