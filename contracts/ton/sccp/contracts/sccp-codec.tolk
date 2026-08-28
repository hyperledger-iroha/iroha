import "@stdlib/exotic-cells"
import "constants"
import "errors"
import "messages"

/// Return the live TON network global id (mainnet `-239`, testnet `-3`).
/// # Safety
/// `GLOBALID` only reads the current immutable execution configuration.
fun getNetworkGlobalId(): int32
    asm "GLOBALID"

const BLAKE2B_U64_MASK = 0xffffffffffffffff

fun blake2bIv(index: int): int {
    if (index == 0) {
        return 0x6a09e667f3bcc908;
    }
    if (index == 1) {
        return 0xbb67ae8584caa73b;
    }
    if (index == 2) {
        return 0x3c6ef372fe94f82b;
    }
    if (index == 3) {
        return 0xa54ff53a5f1d36f1;
    }
    if (index == 4) {
        return 0x510e527fade682d1;
    }
    if (index == 5) {
        return 0x9b05688c2b3e6c1f;
    }
    if (index == 6) {
        return 0x1f83d9abfb41bd6b;
    }
    return 0x5be0cd19137e2179;
}

fun blake2bSigma(round: int, index: int): int {
    val schedule = round == 0
        ? 0xfedcba9876543210
        : round == 1
            ? 0x357b20c16df984ae
            : round == 2
                ? 0x491763eadf250c8b
                : round == 3
                    ? 0x8f04a562ebcd1397
                    : round == 4
                        ? 0xd386cb1efa427509
                        : round == 5
                            ? 0x91ef57d438b0a6c2
                            : round == 6
                                ? 0xb8293670a4def15c
                                : round == 7
                                    ? 0xa2684f05931ce7bd
                                    : round == 8 ? 0x5a417d2c803b9ef6 : 0x0dc3e9bf5167482a;
    return (schedule >> (index * 4)) & 0xf;
}

fun rotateRight64(value: int, shift: int): int {
    return (value >> shift) | ((value << (64 - shift)) & BLAKE2B_U64_MASK);
}

fun blake2bMix(
    mutate state: array<int>,
    aIndex: int,
    bIndex: int,
    cIndex: int,
    dIndex: int,
    x: int,
    y: int,
) {
    var a = (state.get(aIndex) + state.get(bIndex) + x) & BLAKE2B_U64_MASK;
    var d = rotateRight64(state.get(dIndex) ^ a, 32);
    var c = (state.get(cIndex) + d) & BLAKE2B_U64_MASK;
    var b = rotateRight64(state.get(bIndex) ^ c, 24);
    a = (a + b + y) & BLAKE2B_U64_MASK;
    d = rotateRight64(d ^ a, 16);
    c = (c + d) & BLAKE2B_U64_MASK;
    b = rotateRight64(b ^ c, 63);
    state.set(a, aIndex);
    state.set(b, bIndex);
    state.set(c, cIndex);
    state.set(d, dIndex);
}

fun blake2bCompress(
    mutate chaining: array<int>,
    message: array<int>,
    counter: int,
    finalBlock: bool,
) {
    var state: array<int> = [];
    var index = 0;
    while (index < 8) {
        state.push(chaining.get(index));
        index += 1;
    }
    index = 0;
    while (index < 8) {
        state.push(blake2bIv(index));
        index += 1;
    }
    state.set(state.get(12) ^ (counter & BLAKE2B_U64_MASK), 12);
    if (finalBlock) {
        state.set(state.get(14) ^ BLAKE2B_U64_MASK, 14);
    }
    var round = 0;
    repeat (12) {
        val scheduleRound = round < 10 ? round : round - 10;
        blake2bMix(
            mutate state,
            0,
            4,
            8,
            12,
            message.get(blake2bSigma(scheduleRound, 0)),
            message.get(blake2bSigma(scheduleRound, 1)),
        );
        blake2bMix(
            mutate state,
            1,
            5,
            9,
            13,
            message.get(blake2bSigma(scheduleRound, 2)),
            message.get(blake2bSigma(scheduleRound, 3)),
        );
        blake2bMix(
            mutate state,
            2,
            6,
            10,
            14,
            message.get(blake2bSigma(scheduleRound, 4)),
            message.get(blake2bSigma(scheduleRound, 5)),
        );
        blake2bMix(
            mutate state,
            3,
            7,
            11,
            15,
            message.get(blake2bSigma(scheduleRound, 6)),
            message.get(blake2bSigma(scheduleRound, 7)),
        );
        blake2bMix(
            mutate state,
            0,
            5,
            10,
            15,
            message.get(blake2bSigma(scheduleRound, 8)),
            message.get(blake2bSigma(scheduleRound, 9)),
        );
        blake2bMix(
            mutate state,
            1,
            6,
            11,
            12,
            message.get(blake2bSigma(scheduleRound, 10)),
            message.get(blake2bSigma(scheduleRound, 11)),
        );
        blake2bMix(
            mutate state,
            2,
            7,
            8,
            13,
            message.get(blake2bSigma(scheduleRound, 12)),
            message.get(blake2bSigma(scheduleRound, 13)),
        );
        blake2bMix(
            mutate state,
            3,
            4,
            9,
            14,
            message.get(blake2bSigma(scheduleRound, 14)),
            message.get(blake2bSigma(scheduleRound, 15)),
        );
        round += 1;
    }
    index = 0;
    while (index < 8) {
        chaining.set(chaining.get(index) ^ state.get(index) ^ state.get(index + 8), index);
        index += 1;
    }
}

fun nextHashByte(mutate parts: array<slice>, mutate partIndex: int): int {
    while (partIndex < parts.size()) {
        var current = parts.get(partIndex);
        if (current.remainingBitsCount() != 0) {
            val value = current.loadUint(8);
            parts.set(current, partIndex);
            return value;
        }
        partIndex += 1;
    }
    throw SccpErrors.InvalidPayload;
}

/// Pure BLAKE2b-256. TVM's `HASHEXT 2` is BLAKE2b-512 and returns two stack words, so it cannot
/// be used for SCCP's 32-byte BLAKE2b parameterization.
fun blake2b256(mutatingParts: array<slice>): uint256 {
    var parts = mutatingParts;
    var totalBytes = 0;
    var index = 0;
    while (index < parts.size()) {
        val part = parts.get(index);
        assert (
            part.remainingRefsCount() == 0 && part.remainingBitsCount() % 8 == 0
        ) throw SccpErrors.InvalidPayload;
        totalBytes += part.remainingBitsCount() / 8;
        index += 1;
    }
    var chaining: array<int> = [];
    index = 0;
    while (index < 8) {
        chaining.push(blake2bIv(index));
        index += 1;
    }
    chaining.set(chaining.get(0) ^ 0x01010020, 0);
    var consumed = 0;
    var partIndex = 0;
    // BLAKE2 still compresses one final zero-length block for an empty input.
    var firstBlock = true;
    while (firstBlock || consumed < totalBytes) {
        firstBlock = false;
        val remaining = totalBytes - consumed;
        val blockBytes = remaining > 128 ? 128 : remaining;
        var message: array<int> = [];
        repeat (16) {
            message.push(0);
        }
        index = 0;
        while (index < blockBytes) {
            val wordIndex = index / 8;
            val shift = (index % 8) * 8;
            message.set(
                message.get(wordIndex) | (nextHashByte(mutate parts, mutate partIndex) << shift),
                wordIndex,
            );
            index += 1;
        }
        consumed += blockBytes;
        blake2bCompress(mutate chaining, message, consumed, consumed == totalBytes);
    }
    assert (partIndex <= parts.size()) throw SccpErrors.InvalidPayload;
    var digest = 0;
    var wordIndex = 0;
    while (wordIndex < 4) {
        var byteIndex = 0;
        while (byteIndex < 8) {
            digest = (digest << 8) | ((chaining.get(wordIndex) >> (byteIndex * 8)) & 0xff);
            byteIndex += 1;
        }
        wordIndex += 1;
    }
    return digest as uint256;
}

fun blake2bTwo(a: slice, b: slice): uint256 {
    var parts: array<slice> = [];
    parts.push(a);
    parts.push(b);
    return blake2b256(parts);
}

fun blake2bFive(a: slice, b: slice, c: slice, d: slice, e: slice): uint256 {
    var parts: array<slice> = [];
    parts.push(a);
    parts.push(b);
    parts.push(c);
    parts.push(d);
    parts.push(e);
    return blake2b256(parts);
}

/// # Safety
/// The caller provides one byte-aligned, reference-free slice.
@pure
fun sha256One(a: slice): uint256
    asm "1 PUSHINT" "0 HASHEXT"

/// # Safety
/// The caller provides one byte-aligned, reference-free slice.
@pure
fun keccak256One(a: slice): uint256
    asm "1 PUSHINT" "3 HASHEXT"

/// # Safety
/// Callers provide two byte-aligned, reference-free slices.
@pure
fun keccak256Two(a: slice, b: slice): uint256
    asm "2 PUSHINT" "3 HASHEXT"

/// # Safety
/// Callers provide seven byte-aligned, reference-free slices.
@pure
fun keccak256Seven(
    a: slice,
    b: slice,
    c: slice,
    d: slice,
    e: slice,
    f: slice,
    g: slice,
): uint256
    asm "7 PUSHINT" "3 HASHEXT"

fun builder.storeUint32Le(mutate self, value: uint32): self {
    self.storeUint(value & 0xff, 8);
    self.storeUint((value >> 8) & 0xff, 8);
    self.storeUint((value >> 16) & 0xff, 8);
    self.storeUint((value >> 24) & 0xff, 8);
    return self;
}

fun builder.storeUint64Le(mutate self, value: uint64): self {
    var remaining = value;
    repeat (8) {
        self.storeUint(remaining & 0xff, 8);
        remaining = remaining >> 8;
    }
    return self;
}

fun builder.storeUint128Le(mutate self, value: uint128): self {
    var remaining = value;
    repeat (16) {
        self.storeUint(remaining & 0xff, 8);
        remaining = remaining >> 8;
    }
    return self;
}

fun slice.loadUint32Le(mutate self): uint32 {
    val b0 = self.loadUint(8);
    val b1 = self.loadUint(8);
    val b2 = self.loadUint(8);
    val b3 = self.loadUint(8);
    return b0 | (b1 << 8) | (b2 << 16) | (b3 << 24);
}

fun slice.loadUint128Le(mutate self): uint128 {
    var value = 0;
    var shift = 0;
    repeat (16) {
        value = value | (self.loadUint(8) << shift);
        shift += 8;
    }
    return value as uint128;
}

/// Encode the canonical SCCP TON raw-account form: signed big-endian i32 workchain followed by
/// the 256-bit account id. This first route is deliberately basechain-only.
fun canonicalTonAddress36(value: address): cell {
    val (workchain, accountHash) = value.getWorkchainAndHash();
    assert (workchain == SCCP_WORKCHAIN) throw SccpErrors.WrongWorkchain;
    assert (accountHash != 0) throw SccpErrors.InvalidPayload;
    return beginCell().storeInt(workchain, 32).storeUint(accountHash, 256).endCell();
}

/// Encode a standard TON address for an internal, domain-separated identity.
///
/// Unlike the external SCCP account codec, internal replay/refund identities
/// must accept masterchain owners as well as basechain owners. The workchain
/// remains an explicit signed i32 so the same account hash cannot alias across
/// workchains. This helper is never used to admit an external route address.
fun canonicalInternalTonAddress36(value: address): cell {
    val (workchain, accountHash) = value.getWorkchainAndHash();
    assert (accountHash != 0) throw SccpErrors.InvalidPayload;
    return beginCell().storeInt(workchain, 32).storeUint(accountHash, 256).endCell();
}

/// Decode exactly one canonical 36-byte basechain TON account.
fun decodeCanonicalTonAddress36(value: slice): address {
    var input = value;
    assert (input.remainingBitsCount() == 36 * 8) throw SccpErrors.InvalidPayload;
    assert (input.remainingRefsCount() == 0) throw SccpErrors.InvalidPayload;
    val workchain = input.loadInt(32);
    assert (workchain == SCCP_WORKCHAIN) throw SccpErrors.WrongWorkchain;
    val accountHash = input.loadUint(256);
    input.assertEnd();
    assert (accountHash != 0) throw SccpErrors.InvalidPayload;
    return address.fromWorkchainAndHash(workchain, accountHash);
}

struct EdwardsPoint {
    x: int
    y: int
    z: int
    t: int
}

fun appendSliceBytes(mutate output: array<int>, value: slice) {
    var input = value;
    while (input.remainingBitsCount() != 0) {
        output.push(input.loadUint(8));
    }
    input.assertEnd();
}

/// Decode the unique recipient-cell representation: bytes 0..80 in the root and, only when
/// needed, bytes 80..148 in one ordinary tail cell. This makes the textual input representation
/// as canonical as the decoded I105 literal.
fun readTairaRecipientBytes(value: cell): (bool, array<int>) {
    var output: array<int> = [];
    val (initial, exotic) = value.beginParseSpecial();
    if (exotic || initial.remainingBitsCount() % 8 != 0) {
        return (false, output);
    }
    val headBits = initial.remainingBitsCount();
    val headBytes = headBits / 8;
    val refs = initial.remainingRefsCount();
    if (refs == 0) {
        if (headBytes == 0 || headBytes > SCCP_TAIRA_RECIPIENT_HEAD_BYTES) {
            return (false, output);
        }
        appendSliceBytes(mutate output, initial);
        return (true, output);
    }
    if (refs != 1 || headBytes != SCCP_TAIRA_RECIPIENT_HEAD_BYTES) {
        return (false, output);
    }
    val tailCell = initial.preloadRef();
    val (tailInitial, tailExotic) = tailCell.beginParseSpecial();
    val tailBits = tailInitial.remainingBitsCount();
    if (
        tailExotic || tailInitial.remainingRefsCount() != 0 || tailBits == 0 || tailBits % 8 != 0 ||
        headBytes + tailBits / 8 > SCCP_MAX_TAIRA_RECIPIENT_BYTES
    ) {
        return (false, output);
    }
    var head = initial;
    appendSliceBytes(mutate output, head.loadBits(headBits));
    head.loadRef();
    head.assertEnd();
    appendSliceBytes(mutate output, tailInitial);
    return (true, output);
}

fun asciiI105Digit(character: int): (bool, int) {
    var alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".literalSlice();
    var digit = 0;
    while (alphabet.remainingBitsCount() != 0) {
        if (alphabet.loadUint(8) == character) {
            return (true, digit);
        }
        digit += 1;
    }
    return (false, 0);
}

/// Map the published 47 three-byte UTF-8 Iroha-poem symbols to digits 58..104.
fun kanaI105Digit(code: int): (bool, int) {
    if (code == 0xefbdb2) {
        return (true, 58);
    }
    if (code == 0xefbe9b) {
        return (true, 59);
    }
    if (code == 0xefbe8a) {
        return (true, 60);
    }
    if (code == 0xefbe86) {
        return (true, 61);
    }
    if (code == 0xefbe8e) {
        return (true, 62);
    }
    if (code == 0xefbe8d) {
        return (true, 63);
    }
    if (code == 0xefbe84) {
        return (true, 64);
    }
    if (code == 0xefbe81) {
        return (true, 65);
    }
    if (code == 0xefbe98) {
        return (true, 66);
    }
    if (code == 0xefbe87) {
        return (true, 67);
    }
    if (code == 0xefbe99) {
        return (true, 68);
    }
    if (code == 0xefbda6) {
        return (true, 69);
    }
    if (code == 0xefbe9c) {
        return (true, 70);
    }
    if (code == 0xefbdb6) {
        return (true, 71);
    }
    if (code == 0xefbe96) {
        return (true, 72);
    }
    if (code == 0xefbe80) {
        return (true, 73);
    }
    if (code == 0xefbe9a) {
        return (true, 74);
    }
    if (code == 0xefbdbf) {
        return (true, 75);
    }
    if (code == 0xefbe82) {
        return (true, 76);
    }
    if (code == 0xefbe88) {
        return (true, 77);
    }
    if (code == 0xefbe85) {
        return (true, 78);
    }
    if (code == 0xefbe97) {
        return (true, 79);
    }
    if (code == 0xefbe91) {
        return (true, 80);
    }
    if (code == 0xefbdb3) {
        return (true, 81);
    }
    if (code == 0xe383b0) {
        return (true, 82);
    }
    if (code == 0xefbe89) {
        return (true, 83);
    }
    if (code == 0xefbdb5) {
        return (true, 84);
    }
    if (code == 0xefbdb8) {
        return (true, 85);
    }
    if (code == 0xefbe94) {
        return (true, 86);
    }
    if (code == 0xefbe8f) {
        return (true, 87);
    }
    if (code == 0xefbdb9) {
        return (true, 88);
    }
    if (code == 0xefbe8c) {
        return (true, 89);
    }
    if (code == 0xefbdba) {
        return (true, 90);
    }
    if (code == 0xefbdb4) {
        return (true, 91);
    }
    if (code == 0xefbe83) {
        return (true, 92);
    }
    if (code == 0xefbdb1) {
        return (true, 93);
    }
    if (code == 0xefbdbb) {
        return (true, 94);
    }
    if (code == 0xefbdb7) {
        return (true, 95);
    }
    if (code == 0xefbe95) {
        return (true, 96);
    }
    if (code == 0xefbe92) {
        return (true, 97);
    }
    if (code == 0xefbe90) {
        return (true, 98);
    }
    if (code == 0xefbdbc) {
        return (true, 99);
    }
    if (code == 0xe383b1) {
        return (true, 100);
    }
    if (code == 0xefbe8b) {
        return (true, 101);
    }
    if (code == 0xefbe93) {
        return (true, 102);
    }
    if (code == 0xefbdbe) {
        return (true, 103);
    }
    if (code == 0xefbdbd) {
        return (true, 104);
    }
    return (false, 0);
}

fun decodeI105Digits(value: array<int>): (bool, array<int>) {
    var digits: array<int> = [];
    if (
        value.size() <= 4 || value.get(0) != 0x74 || value.get(1) != 0x65 || value.get(2) != 0x73 ||
        value.get(3) != 0x74
    ) {
        return (false, digits);
    }
    var cursor = 4;
    while (cursor < value.size()) {
        val first = value.get(cursor);
        var found = false;
        var digit = 0;
        if (first < 0x80) {
            (found, digit) = asciiI105Digit(first);
            cursor += 1;
        } else {
            if (cursor + 2 >= value.size()) {
                return (false, digits);
            }
            val code = (first << 16) | (value.get(cursor + 1) << 8) | value.get(cursor + 2);
            (found, digit) = kanaI105Digit(code);
            cursor += 3;
        }
        if (!found) {
            return (false, digits);
        }
        digits.push(digit);
    }
    return (digits.size() > SCCP_I105_CHECKSUM_DIGITS, digits);
}

/// Decode exactly 36 canonical account bytes without ever materializing a 288-bit integer.
fun decodeI105SingleKey(digits: array<int>, payloadDigits: int): (bool, array<int>) {
    var canonical: array<int> = [];
    repeat (36) {
        canonical.push(0);
    }
    var digitIndex = 0;
    while (digitIndex < payloadDigits) {
        var carry = digits.get(digitIndex);
        var byteIndex = 35;
        repeat (36) {
            val accumulator = canonical.get(byteIndex) * SCCP_I105_BASE + carry;
            canonical.set(accumulator & 0xff, byteIndex);
            carry = accumulator >> 8;
            byteIndex -= 1;
        }
        if (carry != 0) {
            return (false, canonical);
        }
        digitIndex += 1;
    }
    val exactShape = canonical.get(0) == 0x02 && canonical.get(1) == 0x00 &&
    canonical.get(2) == 0x01 &&
    canonical.get(3) == 0x20;
    return (exactShape, canonical);
}

fun base105ReencodesExactly(
    canonical: array<int>,
    expected: array<int>,
    expectedLength: int,
): bool {
    var work: array<int> = [];
    var index = 0;
    while (index < canonical.size()) {
        work.push(canonical.get(index));
        index += 1;
    }
    var reversed: array<int> = [];
    var first = 0;
    while (first < work.size() && work.get(first) == 0) {
        first += 1;
    }
    while (first < work.size()) {
        var remainder = 0;
        index = first;
        while (index < work.size()) {
            val accumulator = (remainder << 8) | work.get(index);
            work.set(accumulator / SCCP_I105_BASE, index);
            remainder = accumulator % SCCP_I105_BASE;
            index += 1;
        }
        reversed.push(remainder);
        while (first < work.size() && work.get(first) == 0) {
            first += 1;
        }
    }
    if (reversed.size() != expectedLength) {
        return false;
    }
    index = 0;
    while (index < expectedLength) {
        if (expected.get(index) != reversed.get(expectedLength - 1 - index)) {
            return false;
        }
        index += 1;
    }
    return true;
}

fun i105PolymodStep(current: int, value: int): int {
    val top = current >> 25;
    var next = ((current & 0x01ffffff) << 5) ^ value;
    if ((top & 1) != 0) {
        next = next ^ 0x3b6a57b2;
    }
    if ((top & 2) != 0) {
        next = next ^ 0x26508e6d;
    }
    if ((top & 4) != 0) {
        next = next ^ 0x1ea119fa;
    }
    if ((top & 8) != 0) {
        next = next ^ 0x3d4233dd;
    }
    if ((top & 16) != 0) {
        next = next ^ 0x2a1462b3;
    }
    return next;
}

fun i105ChecksumMatches(canonical: array<int>, digits: array<int>, payloadDigits: int): bool {
    var polymod = 1;
    // expand_hrp("snx") = [3, 3, 3, 0, 19, 14, 24]
    polymod = i105PolymodStep(polymod, 3);
    polymod = i105PolymodStep(polymod, 3);
    polymod = i105PolymodStep(polymod, 3);
    polymod = i105PolymodStep(polymod, 0);
    polymod = i105PolymodStep(polymod, 19);
    polymod = i105PolymodStep(polymod, 14);
    polymod = i105PolymodStep(polymod, 24);
    var accumulator = 0;
    var bits = 0;
    var index = 0;
    while (index < canonical.size()) {
        accumulator = (accumulator << 8) | canonical.get(index);
        bits += 8;
        while (bits >= 5) {
            bits -= 5;
            polymod = i105PolymodStep(polymod, (accumulator >> bits) & 31);
        }
        accumulator = bits == 0 ? 0 : accumulator & ((1 << bits) - 1);
        index += 1;
    }
    if (bits != 0) {
        polymod = i105PolymodStep(polymod, (accumulator << (5 - bits)) & 31);
    }
    repeat (SCCP_I105_CHECKSUM_DIGITS) {
        polymod = i105PolymodStep(polymod, 0);
    }
    polymod = polymod ^ SCCP_I105_BECH32M_CONSTANT;
    index = 0;
    while (index < SCCP_I105_CHECKSUM_DIGITS) {
        val expected = (polymod >> (5 * (SCCP_I105_CHECKSUM_DIGITS - 1 - index))) & 31;
        if (digits.get(payloadDigits + index) != expected) {
            return false;
        }
        index += 1;
    }
    return true;
}

/// `mulDivMod` performs the 513-bit intermediate multiplication required for field arithmetic.
fun fieldMul(a: int, b: int): int {
    val (_, remainder) = mulDivMod(a, b, SCCP_ED25519_FIELD);
    return remainder;
}

fun fieldAdd(a: int, b: int): int {
    val sum = a + b;
    return sum >= SCCP_ED25519_FIELD ? sum - SCCP_ED25519_FIELD : sum;
}

fun fieldSub(a: int, b: int): int {
    return a >= b ? a - b : SCCP_ED25519_FIELD - (b - a);
}

fun fieldPow(baseValue: int, exponentValue: int): int {
    var result = 1;
    var base = baseValue;
    var exponent = exponentValue;
    while (exponent != 0) {
        if ((exponent & 1) != 0) {
            result = fieldMul(result, base);
        }
        base = fieldMul(base, base);
        exponent = exponent >> 1;
    }
    return result;
}

fun recoverEd25519X(y: int): (bool, int) {
    val y2 = fieldMul(y, y);
    val u = fieldSub(y2, 1);
    val v = fieldAdd(fieldMul(SCCP_ED25519_D, y2), 1);
    val v3 = fieldMul(fieldMul(v, v), v);
    val v7 = fieldMul(fieldMul(v3, v3), v);
    var x = fieldMul(fieldMul(u, v3), fieldPow(fieldMul(u, v7), SCCP_ED25519_SQRT_EXPONENT));
    var check = fieldMul(v, fieldMul(x, x));
    if (check != u) {
        if (check != (u == 0 ? 0 : SCCP_ED25519_FIELD - u)) {
            return (false, 0);
        }
        x = fieldMul(x, SCCP_ED25519_SQRT_M1);
        check = fieldMul(v, fieldMul(x, x));
        if (check != u) {
            return (false, 0);
        }
    }
    return (true, x);
}

fun addEd25519(left: EdwardsPoint, right: EdwardsPoint): EdwardsPoint {
    val a = fieldMul(fieldSub(left.y, left.x), fieldSub(right.y, right.x));
    val b = fieldMul(fieldAdd(left.y, left.x), fieldAdd(right.y, right.x));
    val c = fieldMul(SCCP_ED25519_TWO_D, fieldMul(left.t, right.t));
    val d = fieldMul(2, fieldMul(left.z, right.z));
    val e = fieldSub(b, a);
    val f = fieldSub(d, c);
    val g = fieldAdd(d, c);
    val h = fieldAdd(b, a);
    return {
        x: fieldMul(e, f),
        y: fieldMul(g, h),
        z: fieldMul(f, g),
        t: fieldMul(e, h),
    };
}

fun isEd25519PrimeSubgroup(x: int, y: int): bool {
    var result: EdwardsPoint = {
        x: 0,
        y: 1,
        z: 1,
        t: 0,
    };
    var multiple: EdwardsPoint = {
        x,
        y,
        z: 1,
        t: fieldMul(x, y),
    };
    var scalar = SCCP_ED25519_SUBGROUP_ORDER;
    while (scalar != 0) {
        if ((scalar & 1) != 0) {
            result = addEd25519(result, multiple);
        }
        multiple = addEd25519(multiple, multiple);
        scalar = scalar >> 1;
    }
    return result.z != 0 && result.x == 0 && result.y == result.z;
}

fun isAdmittedEd25519Key(canonical: array<int>): bool {
    var encoded = 0;
    var index = 4;
    var shift = 0;
    while (index < 36) {
        encoded = encoded | (canonical.get(index) << shift);
        shift += 8;
        index += 1;
    }
    val y = encoded & SCCP_ED25519_Y_MASK;
    if (
        y >= SCCP_ED25519_FIELD || y == 0 || y == 1 || y == SCCP_ED25519_FIELD - 1 ||
        y == SCCP_ED25519_TORSION_Y_1 ||
        y == SCCP_ED25519_TORSION_Y_2
    ) {
        return false;
    }
    var decompressed = false;
    var x = 0;
    (decompressed, x) = recoverEd25519X(y);
    if (!decompressed) {
        return false;
    }
    val sign = encoded >> 255;
    if ((x & 1) != sign) {
        x = SCCP_ED25519_FIELD - x;
    }
    return isEd25519PrimeSubgroup(x, y);
}

fun isCanonicalTairaRecipientBytes(value: array<int>): bool {
    val (symbolsValid, digits) = decodeI105Digits(value);
    if (!symbolsValid) {
        return false;
    }
    val payloadDigits = digits.size() - SCCP_I105_CHECKSUM_DIGITS;
    val (decoded, canonical) = decodeI105SingleKey(digits, payloadDigits);
    return decoded && base105ReencodesExactly(canonical, digits, payloadDigits) &&
    i105ChecksumMatches(canonical, digits, payloadDigits) &&
    isAdmittedEd25519Key(canonical);
}

/// Validate the exact discriminant-369 Taira I105 spelling, minimal base-105 representation,
/// Bech32m checksum, single-key AccountAddress shape, and prime-subgroup Ed25519 controller before
/// any irreversible Jetton burn.
fun isCanonicalTairaRecipient(value: cell): bool {
    val (shapeValid, bytes) = readTairaRecipientBytes(value);
    return shapeValid && isCanonicalTairaRecipientBytes(bytes);
}

fun validateTairaRecipient(value: cell): array<int> {
    val (shapeValid, bytes) = readTairaRecipientBytes(value);
    assert (shapeValid && isCanonicalTairaRecipientBytes(bytes)) throw SccpErrors.InvalidPayload;
    return bytes;
}

fun arrayAppendUint32Le(mutate output: array<int>, value: uint32) {
    output.push(value & 0xff);
    output.push((value >> 8) & 0xff);
    output.push((value >> 16) & 0xff);
    output.push((value >> 24) & 0xff);
}

fun cellFromByteRange(value: array<int>, start: int, length: int): cell {
    var encoded = beginCell();
    var index = start;
    while (index < start + length) {
        encoded.storeUint(value.get(index), 8);
        index += 1;
    }
    return encoded.endCell();
}

/// Construct the exact canonical TON-to-Taira transfer payload. Concatenating the four flat
/// cells yields the canonical SCCP bytes; segmentation is deterministic and never truncates a
/// variable-length I105 recipient.
fun buildTonToTairaPayload(
    sender: address,
    recipient: cell,
    nonce: uint64,
    routeRevision: uint32,
    amount: uint128,
): CanonicalPayloadCells {
    assert (routeRevision != 0) throw SccpErrors.InvalidPayload;
    assert (amount != 0 && amount <= SCCP_MAX_JETTON_AMOUNT) throw SccpErrors.InvalidAmount;
    val recipientBytes = validateTairaRecipient(recipient);
    val senderBytes = canonicalTonAddress36(sender);

    val header = beginCell()
        .storeUint(SCCP_PAYLOAD_DISCRIMINATOR, 8)
        .storeUint(SCCP_PAYLOAD_VERSION, 8)
        .storeUint32Le(SCCP_DOMAIN_TON)
        .storeUint32Le(SCCP_DOMAIN_SORA)
        .storeUint64Le(nonce)
        .storeUint32Le(routeRevision)
        .storeUint32Le(SCCP_DOMAIN_SORA)
        .storeUint(SCCP_CODEC_CANONICAL_TEXT, 8)
        .storeUint32Le(3)
        .storeSlice("xor".literalSlice())
        .storeUint128Le(amount)
        .endCell();
    var remainder: array<int> = [];
    remainder.push(SCCP_CODEC_TON_ADDRESS36);
    arrayAppendUint32Le(mutate remainder, 36);
    appendSliceBytes(mutate remainder, senderBytes.beginParse());
    remainder.push(SCCP_CODEC_CANONICAL_TEXT);
    arrayAppendUint32Le(mutate remainder, recipientBytes.size() as uint32);
    var index = 0;
    while (index < recipientBytes.size()) {
        remainder.push(recipientBytes.get(index));
        index += 1;
    }
    remainder.push(SCCP_CODEC_CANONICAL_TEXT);
    arrayAppendUint32Le(mutate remainder, 13);
    appendSliceBytes(mutate remainder, "taira_ton_xor".literalSlice());
    assert (
        remainder.size() <= SCCP_MAX_CANONICAL_PAYLOAD_BYTES - SCCP_CANONICAL_HEADER_BYTES
    ) throw SccpErrors.InvalidPayload;
    val firstLength = remainder.size() > SCCP_CANONICAL_CHUNK_BYTES
        ? SCCP_CANONICAL_CHUNK_BYTES
        : remainder.size();
    val afterFirst = remainder.size() - firstLength;
    val secondLength = afterFirst > SCCP_CANONICAL_CHUNK_BYTES
        ? SCCP_CANONICAL_CHUNK_BYTES
        : afterFirst;
    val thirdLength = afterFirst - secondLength;
    return {
        header,
        chunk1: cellFromByteRange(remainder, 0, firstLength),
        chunk2: cellFromByteRange(remainder, firstLength, secondLength),
        chunk3: cellFromByteRange(remainder, firstLength + secondLength, thirdLength),
    };
}

fun ordinaryByteLength(value: cell): int {
    val (input, exotic) = value.beginParseSpecial();
    assert (
        !exotic && input.remainingRefsCount() == 0 && input.remainingBitsCount() % 8 == 0
    ) throw SccpErrors.InvalidPayload;
    return input.remainingBitsCount() / 8;
}

/// Enforce the unique 50/100/100/remainder representation before hashing or emitting bytes.
fun assertCanonicalPayloadSegmentation(payload: CanonicalPayloadCells): int {
    val headerLength = ordinaryByteLength(payload.header);
    val firstLength = ordinaryByteLength(payload.chunk1);
    val secondLength = ordinaryByteLength(payload.chunk2);
    val thirdLength = ordinaryByteLength(payload.chunk3);
    assert (headerLength == SCCP_CANONICAL_HEADER_BYTES) throw SccpErrors.InvalidPayload;
    assert (
        firstLength <= SCCP_CANONICAL_CHUNK_BYTES && secondLength <= SCCP_CANONICAL_CHUNK_BYTES &&
        thirdLength <= 124
    ) throw SccpErrors.InvalidPayload;
    assert (
        (firstLength == SCCP_CANONICAL_CHUNK_BYTES || (secondLength == 0 && thirdLength == 0)) &&
        (secondLength == SCCP_CANONICAL_CHUNK_BYTES || thirdLength == 0)
    ) throw SccpErrors.InvalidPayload;
    val total = headerLength + firstLength + secondLength + thirdLength;
    assert (total <= SCCP_MAX_CANONICAL_PAYLOAD_BYTES) throw SccpErrors.InvalidPayload;
    return total;
}

/// Build the canonical four-cell snake authenticated by the TON source proof.
fun canonicalPayloadSnake(payload: CanonicalPayloadCells): cell {
    assertCanonicalPayloadSegmentation(payload);
    val tail = beginCell().storeSlice(payload.chunk3.beginParse()).endCell();
    val second = beginCell().storeSlice(payload.chunk2.beginParse()).storeRef(tail).endCell();
    val first = beginCell().storeSlice(payload.chunk1.beginParse()).storeRef(second).endCell();
    return beginCell().storeSlice(payload.header.beginParse()).storeRef(first).endCell();
}

fun canonicalPayloadByteLength(payload: CanonicalPayloadCells): int {
    return assertCanonicalPayloadSegmentation(payload);
}

fun sccpPayloadHash(payload: CanonicalPayloadCells): uint256 {
    assertCanonicalPayloadSegmentation(payload);
    return blake2bFive(
        "sccp:payload:v1".literalSlice(),
        payload.header.beginParse(),
        payload.chunk1.beginParse(),
        payload.chunk2.beginParse(),
        payload.chunk3.beginParse(),
    );
}

fun sccpLaneHash(laneBytes: cell): uint256 {
    val (lane, exotic) = laneBytes.beginParseSpecial();
    assert (
        !exotic && lane.remainingRefsCount() == 0 && lane.remainingBitsCount() % 8 == 0
    ) throw SccpErrors.InvalidConfiguration;
    return blake2bTwo("sccp:lane-id:v1".literalSlice(), lane);
}

/// Compute the exact SCCP V1 message id without representation-dependent cell hashes.
fun sccpMessageId(laneBytes: cell, payload: CanonicalPayloadCells): uint256 {
    val (lane, exotic) = laneBytes.beginParseSpecial();
    assert (
        !exotic && lane.remainingRefsCount() == 0 && lane.remainingBitsCount() % 8 == 0
    ) throw SccpErrors.InvalidConfiguration;
    val laneByteLength = lane.remainingBitsCount() / 8;
    val payloadByteLength = canonicalPayloadByteLength(payload);
    assert (payloadByteLength <= SCCP_MAX_CANONICAL_PAYLOAD_BYTES) throw SccpErrors.InvalidPayload;
    val prefix = beginCell()
        .storeSlice("sccp:lane-message-id:v1".literalSlice())
        .storeUint(1, 8)
        .storeUint32Le(laneByteLength)
        .endCell();
    val payloadLength = beginCell().storeUint32Le(payloadByteLength).endCell();
    return keccak256Seven(
        prefix.beginParse(),
        lane,
        payloadLength.beginParse(),
        payload.header.beginParse(),
        payload.chunk1.beginParse(),
        payload.chunk2.beginParse(),
        payload.chunk3.beginParse(),
    );
}

fun sccpSourceEventDigest(laneHash: uint256, messageId: uint256, payloadHash: uint256): uint256 {
    assert (laneHash != 0 && messageId != 0 && payloadHash != 0) throw SccpErrors.InvalidPayload;
    assert (
        laneHash != messageId && laneHash != payloadHash && messageId != payloadHash
    ) throw SccpErrors.HashRoleAlias;
    val preimage = beginCell()
        .storeSlice("sccp:source:event:v1".literalSlice())
        .storeUint(1, 8)
        .storeUint(laneHash, 256)
        .storeUint(messageId, 256)
        .storeUint(payloadHash, 256)
        .endCell();
    return keccak256One(preimage.beginParse());
}

struct ParsedTairaToTonTransfer {
    recipient: address
    amount: uint128
}

fun canonicalPayloadRemainderBytes(payload: CanonicalPayloadCells): array<int> {
    assertCanonicalPayloadSegmentation(payload);
    var output: array<int> = [];
    appendSliceBytes(mutate output, payload.chunk1.beginParse());
    appendSliceBytes(mutate output, payload.chunk2.beginParse());
    appendSliceBytes(mutate output, payload.chunk3.beginParse());
    return output;
}

fun arrayLoadByte(value: array<int>, mutate cursor: int): int {
    assert (cursor < value.size()) throw SccpErrors.InvalidPayload;
    val result = value.get(cursor);
    cursor += 1;
    return result;
}

fun arrayLoadUint32Le(value: array<int>, mutate cursor: int): uint32 {
    val b0 = arrayLoadByte(value, mutate cursor);
    val b1 = arrayLoadByte(value, mutate cursor);
    val b2 = arrayLoadByte(value, mutate cursor);
    val b3 = arrayLoadByte(value, mutate cursor);
    return (b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)) as uint32;
}

fun arrayLoadBytes(value: array<int>, mutate cursor: int, length: int): array<int> {
    assert (length >= 0 && cursor + length <= value.size()) throw SccpErrors.InvalidPayload;
    var result: array<int> = [];
    val end = cursor + length;
    while (cursor < end) {
        result.push(value.get(cursor));
        cursor += 1;
    }
    return result;
}

/// Strictly decode the destination-relevant fields of a canonical Taira-to-TON payload.
fun parseTairaToTonPayload(
    payloadCell: cell,
    expectedRouteRevision: uint32,
): ParsedTairaToTonTransfer {
    val payload = decodeCanonicalPayloadCells(payloadCell);
    var header = payload.header.beginParse();
    assert (
        header.remainingBitsCount() == SCCP_CANONICAL_HEADER_BYTES * 8 &&
        header.remainingRefsCount() == 0
    ) throw SccpErrors.InvalidPayload;
    assert (header.loadUint(8) == SCCP_PAYLOAD_DISCRIMINATOR) throw SccpErrors.InvalidPayload;
    assert (header.loadUint(8) == SCCP_PAYLOAD_VERSION) throw SccpErrors.InvalidPayload;
    assert (header.loadUint32Le() == SCCP_DOMAIN_SORA) throw SccpErrors.InvalidPayload;
    assert (header.loadUint32Le() == SCCP_DOMAIN_TON) throw SccpErrors.InvalidPayload;
    header.loadUint(64);
    assert (header.loadUint32Le() == expectedRouteRevision) throw SccpErrors.InvalidPayload;
    assert (header.loadUint32Le() == SCCP_DOMAIN_SORA) throw SccpErrors.InvalidPayload;
    assert (header.loadUint(8) == SCCP_CODEC_CANONICAL_TEXT) throw SccpErrors.InvalidPayload;
    assert (header.loadUint32Le() == 3) throw SccpErrors.InvalidPayload;
    assert (header.loadBits(24).bitsEqual("xor".literalSlice())) throw SccpErrors.InvalidPayload;
    val amount = header.loadUint128Le();
    header.assertEnd();
    assert (amount != 0 && amount <= SCCP_MAX_JETTON_AMOUNT) throw SccpErrors.InvalidAmount;

    val remainder = canonicalPayloadRemainderBytes(payload);
    var cursor = 0;
    assert (
        arrayLoadByte(remainder, mutate cursor) == SCCP_CODEC_CANONICAL_TEXT
    ) throw SccpErrors.InvalidPayload;
    val senderLength = arrayLoadUint32Le(remainder, mutate cursor) as int;
    assert (senderLength <= SCCP_MAX_TAIRA_RECIPIENT_BYTES) throw SccpErrors.InvalidPayload;
    val sender = arrayLoadBytes(remainder, mutate cursor, senderLength);
    assert (isCanonicalTairaRecipientBytes(sender)) throw SccpErrors.InvalidPayload;
    assert (
        arrayLoadByte(remainder, mutate cursor) == SCCP_CODEC_TON_ADDRESS36
    ) throw SccpErrors.InvalidPayload;
    assert (arrayLoadUint32Le(remainder, mutate cursor) == 36) throw SccpErrors.InvalidPayload;
    val recipientBytes = arrayLoadBytes(remainder, mutate cursor, 36);
    val recipient = decodeCanonicalTonAddress36(
        cellFromByteRange(recipientBytes, 0, recipientBytes.size()).beginParse(),
    );
    assert (
        arrayLoadByte(remainder, mutate cursor) == SCCP_CODEC_CANONICAL_TEXT
    ) throw SccpErrors.InvalidPayload;
    assert (arrayLoadUint32Le(remainder, mutate cursor) == 13) throw SccpErrors.InvalidPayload;
    val route = arrayLoadBytes(remainder, mutate cursor, 13);
    assert (
        cellFromByteRange(route, 0, route.size())
            .beginParse()
            .bitsEqual("taira_ton_xor".literalSlice())
    ) throw SccpErrors.InvalidPayload;
    assert (cursor == remainder.size()) throw SccpErrors.InvalidPayload;
    return { recipient, amount };
}

fun decodeCanonicalPayloadCells(payloadCell: cell): CanonicalPayloadCells {
    val (root, exotic) = payloadCell.beginParseSpecial();
    assert (
        !exotic && root.remainingBitsCount() == 0 && root.remainingRefsCount() == 4
    ) throw SccpErrors.InvalidPayload;
    val payload = CanonicalPayloadCells.fromCell(payloadCell);
    assertCanonicalPayloadSegmentation(payload);
    return payload;
}

/// Decode the TEP-74 `Either Cell ^Cell` remainder into exactly one payload slice.
fun decodeForwardPayload(payload: ForwardPayloadRemainder): slice {
    var encoded = payload;
    val referenced = encoded.loadMaybeRef();
    if (referenced != null) {
        encoded.assertEnd();
        return referenced!.beginParse();
    }
    return encoded;
}
