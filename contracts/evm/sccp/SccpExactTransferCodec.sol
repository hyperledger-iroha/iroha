// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.7.4;

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

    uint8 internal constant CODEC_CANONICAL_TEXT = 0;
    uint8 internal constant CODEC_EVM_ADDRESS20 = 1;
    uint8 internal constant CODEC_TRON_ADDRESS21 = 2;

    uint256 internal constant MAX_TEXT_BYTES = 256;
    uint256 internal constant MAX_U128 = (uint256(1) << 128) - 1;
    bytes private constant I105_BASE58_ALPHABET =
        "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    bytes private constant I105_KANA_ALPHABET =
        hex"efbdb2efbe9befbe8aefbe86efbe8eefbe8defbe84efbe81efbe98efbe87efbe99efbda6efbe9cefbdb6efbe96efbe80efbe9aefbdbfefbe82efbe88efbe85efbe97efbe91efbdb3e383b0efbe89efbdb5efbdb8efbe94efbe8fefbdb9efbe8cefbdbaefbdb4efbe83efbdb1efbdbbefbdb7efbe95efbe92efbe90efbdbce383b1efbe8befbe93efbdbeefbdbd";
    uint256 private constant I105_CHECKSUM_DIGITS = 6;
    uint256 private constant I105_BASE = 105;
    uint32 private constant BECH32M_CONST = 0x2bc830a3;
    uint256 private constant ED25519_FIELD =
        0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffed;
    uint256 private constant ED25519_D =
        0x52036cee2b6ffe738cc740797779e89800700a4d4141d8ab75eb4dca135978a3;
    uint256 private constant ED25519_SQRT_M1 =
        0x2b8324804fc1df0b2b4d00993dfbd7a72f431806ad2fe478c4ee1b274a0ea0b0;
    uint256 private constant ED25519_SQRT_EXPONENT =
        0x0ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffd;
    uint256 private constant ED25519_SUBGROUP_ORDER =
        0x1000000000000000000000000000000014def9dea2f79cd65812631a5cf5d3ed;
    uint256 private constant ED25519_TWO_D =
        0x2406d9dc56dffce7198e80f2eef3d13000e0149a8283b156ebd69b9426b2f159;
    uint256 private constant ED25519_Y_MASK = (uint256(1) << 255) - 1;
    uint256 private constant ED25519_TORSION_Y_1 =
        0x7a03ac9277fdc74ec6cc392cfa53202a0f67100d760b3cba4fd84d3d706a17c7;
    uint256 private constant ED25519_TORSION_Y_2 =
        0x05fc536d880238b13933c6d305acdfd5f098eff289f4c345b027b2c28f95e826;
    uint256 private constant SECP256K1_FIELD =
        0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f;
    uint256 private constant SECP256K1_SQRT_EXPONENT =
        0x3fffffffffffffffffffffffffffffffffffffffffffffffffffffffbfffff0c;

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

    struct EdwardsPoint {
        uint256 x;
        uint256 y;
        uint256 z;
        uint256 t;
    }

    function ethereumNetwork(uint8 profile) internal pure returns (bytes memory) {
        require(profile == 0x41, "Unsupported Ethereum profile");
        return abi.encodePacked(bytes1(0x01), bytes1(profile), u32le(1), u64le(1));
    }

    function bscNetwork(uint8 profile) internal pure returns (bytes memory) {
        require(profile == 0x42, "Unsupported BSC profile");
        return abi.encodePacked(bytes1(0x01), bytes1(profile), u32le(2), u64le(56));
    }

    function tronNetwork(uint8 profile) internal pure returns (bytes memory) {
        require(profile == 0x43, "Unsupported TRON profile");
        return abi.encodePacked(bytes1(0x01), bytes1(profile), u32le(3), u32le(0x2b6653dc));
    }

    function tairaNetwork() internal pure returns (bytes memory) {
        return hex"014000000000fc56984b2be7431d840e21514d1883f0";
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
        require(_isPrintableAsciiText(fields.assetId) && _isPrintableAsciiText(fields.routeId),
            "Noncanonical route text");
        bytes memory header = abi.encodePacked(
            bytes1(0x00), // SccpPayloadV1::Transfer
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
        return isCanonicalTextRange(value, 0, value.length);
    }

    /**
     * Validate the only irreversible-burn recipient admitted by SCCP V1.
     *
     * This is intentionally stricter than generic canonical text and generic
     * I105 validation. It requires Taira's named `test` sentinel (discriminant
     * 369), a minimal base-105/checksum round trip, the exact single-key
     * AccountAddress bytes `02 00 01 20 || ed25519_key`, and the same canonical,
     * decompressible Ed25519 key in the prime-order subgroup, matching Rust
     * admission including rejection of small-order and mixed-torsion points.
     */
    function isCanonicalTairaRecipient(bytes memory value) internal pure returns (bool) {
        return isCanonicalTairaRecipientRange(value, 0, value.length);
    }

    /** Validate an exact Taira recipient slice without allocating a copy. */
    function isCanonicalTairaRecipientRange(
        bytes memory value,
        uint256 start,
        uint256 length
    ) internal pure returns (bool) {
        if (length == 0 || length > MAX_TEXT_BYTES || start > value.length
            || value.length - start < length) return false;
        uint256 end = start + length;
        if (!_hasPrefix(value, start, end, "test")) return false;
        (bool symbolsValid, uint8[] memory digits, uint256 digitCount) =
            _decodeI105Symbols(value, start + 4, end);
        if (!symbolsValid || digitCount <= I105_CHECKSUM_DIGITS) return false;
        uint256 payloadDigits = digitCount - I105_CHECKSUM_DIGITS;
        bytes memory canonical = _decodeBase105(digits, payloadDigits);
        if (canonical.length != 36
            || canonical[0] != bytes1(0x02)
            || canonical[1] != bytes1(0x00)
            || canonical[2] != bytes1(0x01)
            || canonical[3] != bytes1(0x20)
            || !_base105ReencodesExactly(canonical, digits, payloadDigits)) return false;
        uint8[6] memory checksum = _i105Checksum(canonical);
        for (uint256 i = 0; i < I105_CHECKSUM_DIGITS; i++) {
            if (digits[payloadDigits + i] != checksum[i]) return false;
        }
        return _isAdmittedEd25519Key(canonical, 4);
    }

    /** Validate the sole canonical Taira AccountId provable by V1 routes. */
    function isCanonicalTairaAccountRange(
        bytes memory value,
        uint256 start,
        uint256 length
    ) internal pure returns (bool) {
        return isCanonicalTairaRecipientRange(value, start, length);
    }

    function _isPrintableAsciiText(bytes memory value) private pure returns (bool) {
        if (value.length == 0 || value.length > MAX_TEXT_BYTES) return false;
        for (uint256 i = 0; i < value.length; i++) {
            uint8 character = uint8(value[i]);
            if (character < 0x21 || character > 0x7e) return false;
        }
        return true;
    }

    /** Validate one SCCP canonical-text slice without allocating a copy. */
    function isCanonicalTextRange(
        bytes memory value,
        uint256 start,
        uint256 length
    ) internal pure returns (bool) {
        if (length == 0 || length > MAX_TEXT_BYTES || start > value.length
            || value.length - start < length) return false;
        bool printableAscii = true;
        for (uint256 i = 0; i < length; i++) {
            uint8 character = uint8(value[start + i]);
            if (character < 0x21 || character > 0x7e) {
                printableAscii = false;
                break;
            }
        }
        return printableAscii || isCanonicalI105Range(value, start, length);
    }

    /**
     * Validate an exact canonical I105 account literal.
     *
     * The non-ASCII canonical-text branch is deliberately closed: it accepts
     * only the 105 published symbols, performs a minimal base-105 round trip,
     * checks the six Bech32m digits over the decoded account bytes, and checks
     * the first-release AccountAddress controller layout and closed
     * Ed25519/secp256k1 key policy. Merely well-formed UTF-8 (including
     * arbitrary kana) is not sufficient.
     */
    function isCanonicalI105Range(
        bytes memory value,
        uint256 start,
        uint256 length
    ) internal pure returns (bool) {
        if (length == 0 || length > MAX_TEXT_BYTES || start > value.length
            || value.length - start < length) return false;
        uint256 end = start + length;
        if (_hasPrefix(value, start, end, "sora")) {
            return _isCanonicalI105Payload(value, start + 4, end);
        }
        if (_hasPrefix(value, start, end, "test")) {
            return _isCanonicalI105Payload(value, start + 4, end);
        }
        if (_hasPrefix(value, start, end, "dev")) {
            return _isCanonicalI105Payload(value, start + 3, end);
        }
        if (uint8(value[start]) != 0x6e) return false; // `n`

        // Custom discriminants use the shortest decimal `n<0..65535>` form.
        // Try every possible split because decimal characters are also base-105
        // symbols, and require one unambiguous, structurally valid result.
        uint256 discriminant;
        uint256 accepted;
        for (uint256 digits = 1; digits <= 5 && start + 1 + digits < end; digits++) {
            uint8 character = uint8(value[start + digits]);
            if (character < 0x30 || character > 0x39) break;
            if (digits == 1 && character == 0x30 && start + 1 + digits < end
                && uint8(value[start + 1 + digits]) >= 0x30
                && uint8(value[start + 1 + digits]) <= 0x39) break;
            discriminant = discriminant * 10 + uint256(character - 0x30);
            if (discriminant > uint256(type(uint16).max)) break;
            if (discriminant == 0 || discriminant == 369 || discriminant == 753) continue;
            if (_isCanonicalI105Payload(value, start + 1 + digits, end)) accepted++;
        }
        return accepted == 1;
    }

    function _isCanonicalI105Payload(
        bytes memory value,
        uint256 start,
        uint256 end
    ) private pure returns (bool) {
        (bool symbolsValid, uint8[] memory digits, uint256 digitCount) =
            _decodeI105Symbols(value, start, end);
        if (!symbolsValid || digitCount <= I105_CHECKSUM_DIGITS) return false;
        uint256 payloadDigits = digitCount - I105_CHECKSUM_DIGITS;
        bytes memory canonical = _decodeBase105(digits, payloadDigits);
        if (canonical.length == 0 || !_isCanonicalAccountAddress(canonical)
            || !_base105ReencodesExactly(canonical, digits, payloadDigits)) return false;
        uint8[6] memory checksum = _i105Checksum(canonical);
        for (uint256 i = 0; i < I105_CHECKSUM_DIGITS; i++) {
            if (digits[payloadDigits + i] != checksum[i]) return false;
        }
        return true;
    }

    function _decodeI105Symbols(bytes memory value, uint256 start, uint256 end)
        private pure returns (bool, uint8[] memory digits, uint256 count)
    {
        digits = new uint8[](end - start);
        uint256 cursor = start;
        while (cursor < end) {
            uint8 first = uint8(value[cursor]);
            bool found;
            uint8 digit;
            if (first < 0x80) {
                (found, digit) = _asciiI105Digit(first);
                cursor++;
            } else {
                if (end - cursor < 3) return (false, digits, 0);
                (found, digit) = _kanaI105Digit(
                    value[cursor], value[cursor + 1], value[cursor + 2]
                );
                cursor += 3;
            }
            if (!found) return (false, digits, 0);
            digits[count++] = digit;
        }
        return (true, digits, count);
    }

    function _asciiI105Digit(uint8 character) private pure returns (bool, uint8) {
        bytes memory alphabet = I105_BASE58_ALPHABET;
        for (uint256 i = 0; i < alphabet.length; i++) {
            if (uint8(alphabet[i]) == character) return (true, uint8(i));
        }
        return (false, 0);
    }

    function _kanaI105Digit(bytes1 a, bytes1 b, bytes1 c)
        private pure returns (bool, uint8)
    {
        bytes memory alphabet = I105_KANA_ALPHABET;
        for (uint256 i = 0; i < 47; i++) {
            uint256 offset = i * 3;
            if (alphabet[offset] == a && alphabet[offset + 1] == b
                && alphabet[offset + 2] == c) return (true, uint8(58 + i));
        }
        return (false, 0);
    }

    function _decodeBase105(uint8[] memory digits, uint256 length)
        private pure returns (bytes memory canonical)
    {
        uint256 leadingZeros;
        while (leadingZeros < length && digits[leadingZeros] == 0) leadingZeros++;
        bytes memory scratch = new bytes(length);
        uint256 used;
        for (uint256 i = leadingZeros; i < length; i++) {
            uint256 carry = digits[i];
            uint256 cursor = length;
            for (uint256 j = 0; j < used; j++) {
                cursor--;
                uint256 accumulator = uint256(uint8(scratch[cursor])) * I105_BASE + carry;
                scratch[cursor] = bytes1(uint8(accumulator));
                carry = accumulator >> 8;
            }
            while (carry != 0) {
                if (cursor == 0) return new bytes(0);
                cursor--;
                scratch[cursor] = bytes1(uint8(carry));
                carry >>= 8;
                used++;
            }
        }
        canonical = new bytes(leadingZeros + used);
        for (uint256 i = 0; i < used; i++) {
            canonical[leadingZeros + i] = scratch[length - used + i];
        }
    }

    function _base105ReencodesExactly(
        bytes memory canonical,
        uint8[] memory expected,
        uint256 expectedLength
    ) private pure returns (bool) {
        uint256 leadingZeros;
        while (leadingZeros < canonical.length && canonical[leadingZeros] == bytes1(0)) {
            leadingZeros++;
        }
        bytes memory work = new bytes(canonical.length);
        for (uint256 i = 0; i < canonical.length; i++) work[i] = canonical[i];
        uint8[] memory reversed = new uint8[](expectedLength + 1);
        uint256 count;
        uint256 first = leadingZeros;
        while (first < work.length) {
            uint256 remainder;
            for (uint256 i = first; i < work.length; i++) {
                uint256 accumulator = (remainder << 8) | uint256(uint8(work[i]));
                work[i] = bytes1(uint8(accumulator / I105_BASE));
                remainder = accumulator % I105_BASE;
            }
            reversed[count++] = uint8(remainder);
            while (first < work.length && work[first] == bytes1(0)) first++;
        }
        count += leadingZeros;
        if (count == 0) count = 1;
        if (count != expectedLength) return false;
        for (uint256 i = 0; i < leadingZeros; i++) {
            if (expected[i] != 0) return false;
        }
        for (uint256 i = leadingZeros; i < count; i++) {
            if (expected[i] != reversed[count - 1 - i]) return false;
        }
        return true;
    }

    function _i105Checksum(bytes memory canonical)
        private pure returns (uint8[6] memory checksum)
    {
        uint32 polymod = 1;
        // expand_hrp("snx") = [3, 3, 3, 0, 19, 14, 24]
        uint8[7] memory hrp = [uint8(3), 3, 3, 0, 19, 14, 24];
        for (uint256 i = 0; i < hrp.length; i++) polymod = _polymodStep(polymod, hrp[i]);
        uint256 accumulator;
        uint256 bits;
        for (uint256 i = 0; i < canonical.length; i++) {
            accumulator = (accumulator << 8) | uint256(uint8(canonical[i]));
            bits += 8;
            while (bits >= 5) {
                bits -= 5;
                polymod = _polymodStep(polymod, uint8((accumulator >> bits) & 31));
            }
            accumulator &= bits == 0 ? 0 : (uint256(1) << bits) - 1;
        }
        if (bits != 0) polymod = _polymodStep(polymod, uint8((accumulator << (5 - bits)) & 31));
        for (uint256 i = 0; i < I105_CHECKSUM_DIGITS; i++) polymod = _polymodStep(polymod, 0);
        polymod ^= BECH32M_CONST;
        for (uint256 i = 0; i < I105_CHECKSUM_DIGITS; i++) {
            checksum[i] = uint8((polymod >> (5 * (I105_CHECKSUM_DIGITS - 1 - i))) & 31);
        }
    }

    function _polymodStep(uint32 current, uint8 value) private pure returns (uint32) {
        uint32 top = current >> 25;
        uint32 next = ((current & 0x01ffffff) << 5) ^ uint32(value);
        if ((top & 1) != 0) next ^= 0x3b6a57b2;
        if ((top & 2) != 0) next ^= 0x26508e6d;
        if ((top & 4) != 0) next ^= 0x1ea119fa;
        if ((top & 8) != 0) next ^= 0x3d4233dd;
        if ((top & 16) != 0) next ^= 0x2a1462b3;
        return next;
    }

    function _isCanonicalAccountAddress(bytes memory canonical) private pure returns (bool) {
        if (canonical.length < 4) return false;
        uint8 header = uint8(canonical[0]);
        if (header == 0x02) return _isCanonicalSingleKey(canonical);
        if (header == 0x0a) return _isCanonicalMultisig(canonical);
        return false;
    }

    function _isAdmittedEd25519Key(bytes memory canonical, uint256 start)
        private pure returns (bool)
    {
        uint256 encoded;
        for (uint256 i = 0; i < 32; i++) {
            encoded |= uint256(uint8(canonical[start + i])) << (i * 8);
        }
        uint256 y = encoded & ED25519_Y_MASK;
        if (y >= ED25519_FIELD || _isSmallOrderEd25519Y(y)) return false;
        (bool decompressed, uint256 x) = _recoverEd25519X(y);
        if (!decompressed) return false;
        uint256 sign = encoded >> 255;
        if ((x & 1) != sign) x = ED25519_FIELD - x;
        return _isEd25519PrimeSubgroup(x, y);
    }

    function _isEd25519PrimeSubgroup(uint256 x, uint256 y)
        private pure returns (bool)
    {
        EdwardsPoint memory result = EdwardsPoint(0, 1, 1, 0);
        EdwardsPoint memory multiple = EdwardsPoint(x, y, 1, mulmod(x, y, ED25519_FIELD));
        uint256 scalar = ED25519_SUBGROUP_ORDER;
        while (scalar != 0) {
            if ((scalar & 1) != 0) result = _addEd25519(result, multiple);
            multiple = _addEd25519(multiple, multiple);
            scalar >>= 1;
        }
        return result.z != 0 && result.x == 0 && result.y == result.z;
    }

    function _addEd25519(EdwardsPoint memory left, EdwardsPoint memory right)
        private pure returns (EdwardsPoint memory result)
    {
        uint256 a = mulmod(
            addmod(left.y, ED25519_FIELD - left.x, ED25519_FIELD),
            addmod(right.y, ED25519_FIELD - right.x, ED25519_FIELD),
            ED25519_FIELD
        );
        uint256 b = mulmod(
            addmod(left.y, left.x, ED25519_FIELD),
            addmod(right.y, right.x, ED25519_FIELD),
            ED25519_FIELD
        );
        uint256 c = mulmod(
            ED25519_TWO_D,
            mulmod(left.t, right.t, ED25519_FIELD),
            ED25519_FIELD
        );
        uint256 d = mulmod(
            2,
            mulmod(left.z, right.z, ED25519_FIELD),
            ED25519_FIELD
        );
        uint256 e = addmod(b, ED25519_FIELD - a, ED25519_FIELD);
        uint256 f = addmod(d, ED25519_FIELD - c, ED25519_FIELD);
        uint256 g = addmod(d, c, ED25519_FIELD);
        uint256 h = addmod(b, a, ED25519_FIELD);
        result.x = mulmod(e, f, ED25519_FIELD);
        result.y = mulmod(g, h, ED25519_FIELD);
        result.z = mulmod(f, g, ED25519_FIELD);
        result.t = mulmod(e, h, ED25519_FIELD);
    }

    function _recoverEd25519X(uint256 y) private pure returns (bool, uint256 x) {
        uint256 y2 = mulmod(y, y, ED25519_FIELD);
        uint256 u = addmod(y2, ED25519_FIELD - 1, ED25519_FIELD);
        uint256 v = addmod(mulmod(ED25519_D, y2, ED25519_FIELD), 1, ED25519_FIELD);
        uint256 v3 = mulmod(mulmod(v, v, ED25519_FIELD), v, ED25519_FIELD);
        uint256 v7 = mulmod(mulmod(v3, v3, ED25519_FIELD), v, ED25519_FIELD);
        x = mulmod(
            mulmod(u, v3, ED25519_FIELD),
            _powModField(
                mulmod(u, v7, ED25519_FIELD), ED25519_SQRT_EXPONENT, ED25519_FIELD
            ),
            ED25519_FIELD
        );
        uint256 check = mulmod(v, mulmod(x, x, ED25519_FIELD), ED25519_FIELD);
        if (check != u) {
            uint256 negativeU = u == 0 ? 0 : ED25519_FIELD - u;
            if (check != negativeU) return (false, 0);
            x = mulmod(x, ED25519_SQRT_M1, ED25519_FIELD);
            check = mulmod(v, mulmod(x, x, ED25519_FIELD), ED25519_FIELD);
            if (check != u) return (false, 0);
        }
        return (true, x);
    }

    function _powModField(uint256 base, uint256 exponent, uint256 modulus)
        private pure returns (uint256 result)
    {
        result = 1;
        while (exponent != 0) {
            if ((exponent & 1) != 0) result = mulmod(result, base, modulus);
            base = mulmod(base, base, modulus);
            exponent >>= 1;
        }
    }

    function _isSmallOrderEd25519Y(uint256 y) private pure returns (bool) {
        // These five y coordinates cover all eight canonical E[8] encodings;
        // the sign bit selects the paired point where x is nonzero.
        return y == 0 || y == 1 || y == ED25519_FIELD - 1
            || y == ED25519_TORSION_Y_1 || y == ED25519_TORSION_Y_2;
    }

    function _isCanonicalSingleKey(bytes memory canonical) private pure returns (bool) {
        uint8 tag = uint8(canonical[1]);
        uint8 curve = uint8(canonical[2]);
        // Both V1 destination-supported key payloads fit the canonical u8
        // length tag. Rust's extended tag is reserved for larger algorithms
        // that these contracts intentionally reject fail-closed.
        if (tag != 0) return false;
        uint256 keyLength = uint8(canonical[3]);
        return 4 + keyLength == canonical.length
            && _isCanonicalKeyShape(canonical, 4, keyLength, curve);
    }

    function _isCanonicalMultisig(bytes memory canonical) private pure returns (bool) {
        if (canonical.length < 7 || uint8(canonical[1]) != 1 || uint8(canonical[2]) != 1) {
            return false;
        }
        uint256 threshold = _u16be(canonical, 3);
        uint256 members = _u16be(canonical, 5);
        if (threshold == 0 || members == 0) return false;
        uint256 cursor = 7;
        uint256 totalWeight;
        uint8 previousCurve;
        uint256 previousStart;
        uint256 previousLength;
        for (uint256 member = 0; member < members; member++) {
            if (cursor > canonical.length || canonical.length - cursor < 5) return false;
            uint8 curve = uint8(canonical[cursor++]);
            uint256 weight = _u16be(canonical, cursor); cursor += 2;
            uint256 keyLength = _u16be(canonical, cursor); cursor += 2;
            if (weight == 0 || cursor > canonical.length
                || canonical.length - cursor < keyLength
                || !_isCanonicalKeyShape(canonical, cursor, keyLength, curve)) return false;
            if (member != 0 && !_keyStrictlyFollows(
                canonical, previousCurve, previousStart, previousLength,
                curve, cursor, keyLength
            )) return false;
            totalWeight += weight;
            previousCurve = curve;
            previousStart = cursor;
            previousLength = keyLength;
            cursor += keyLength;
        }
        return cursor == canonical.length && threshold <= totalWeight;
    }

    function _isCanonicalKeyShape(
        bytes memory canonical,
        uint256 start,
        uint256 length,
        uint8 curve
    ) private pure returns (bool) {
        if (curve == 1) {
            return length == 32 && _isAdmittedEd25519Key(canonical, start);
        }
        return curve == 4 && length == 33
            && _isAdmittedSecp256k1Key(canonical, start);
    }

    function _isAdmittedSecp256k1Key(bytes memory canonical, uint256 start)
        private pure returns (bool)
    {
        uint8 prefix = uint8(canonical[start]);
        if (prefix != 2 && prefix != 3) return false;
        uint256 x;
        for (uint256 i = 1; i < 33; i++) {
            x = (x << 8) | uint256(uint8(canonical[start + i]));
        }
        if (x >= SECP256K1_FIELD) return false;
        uint256 x2 = mulmod(x, x, SECP256K1_FIELD);
        uint256 rhs = addmod(mulmod(x2, x, SECP256K1_FIELD), 7, SECP256K1_FIELD);
        uint256 y = _powModField(rhs, SECP256K1_SQRT_EXPONENT, SECP256K1_FIELD);
        if (mulmod(y, y, SECP256K1_FIELD) != rhs) return false;
        // A y=0 point has only the even compressed spelling. secp256k1 has no
        // such prime-subgroup point, but retaining the canonical SEC1 rule
        // keeps this check exact if the curve assumptions are ever revisited.
        return y != 0 || prefix == 2;
    }

    function _keyStrictlyFollows(
        bytes memory canonical,
        uint8 previousCurve,
        uint256 previousStart,
        uint256 previousLength,
        uint8 curve,
        uint256 start,
        uint256 length
    ) private pure returns (bool) {
        uint8 previousRank = _curveSortRank(previousCurve);
        uint8 rank = _curveSortRank(curve);
        if (previousRank == 0 || rank == 0) return false;
        if (rank != previousRank) return rank > previousRank;
        uint256 common = previousLength < length ? previousLength : length;
        for (uint256 i = 0; i < common; i++) {
            uint8 left = uint8(canonical[previousStart + i]);
            uint8 right = uint8(canonical[start + i]);
            if (left != right) return right > left;
        }
        return length > previousLength;
    }

    function _curveSortRank(uint8 curve) private pure returns (uint8) {
        // Rust sorts multisig members by the concatenation of the algorithm
        // name, one zero byte, and the public key. `ed25519` precedes
        // `secp256k1` lexicographically.
        if (curve == 1) return 1;
        if (curve == 4) return 2;
        return 0;
    }

    function _u16be(bytes memory value, uint256 start) private pure returns (uint256) {
        return (uint256(uint8(value[start])) << 8) | uint256(uint8(value[start + 1]));
    }

    function _hasPrefix(bytes memory value, uint256 start, uint256 end, bytes memory prefix)
        private pure returns (bool)
    {
        if (end - start < prefix.length) return false;
        for (uint256 i = 0; i < prefix.length; i++) {
            if (value[start + i] != prefix[i]) return false;
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
        if (finalBlock) v[14] ^= type(uint64).max;
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
        // BLAKE2b specifies these four additions modulo 2^64. This is the only
        // arithmetic in the SCCP contracts that intentionally wraps; every
        // token amount, nonce, length, and offset remains checked by Solidity.
        v[a] = _add64(_add64(v[a], v[b]), x);
        v[d] = _rotr(v[d] ^ v[a], 32);
        v[c] = _add64(v[c], v[d]);
        v[b] = _rotr(v[b] ^ v[c], 24);
        v[a] = _add64(_add64(v[a], v[b]), y);
        v[d] = _rotr(v[d] ^ v[a], 16);
        v[c] = _add64(v[c], v[d]);
        v[b] = _rotr(v[b] ^ v[c], 63);
    }

    function _add64(uint64 left, uint64 right) private pure returns (uint64 result) {
        // The explicit mask fixes BLAKE2b's intended modulo-2^64 behavior
        // independently of compiler arithmetic defaults. Value-moving paths
        // retain their separate overflow and range checks.
        assembly {
            result := and(add(left, right), 0xffffffffffffffff)
        }
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
