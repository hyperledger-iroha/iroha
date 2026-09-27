// SCCP v1 verification primitives shared by the TON minter: roster digests
// (§3.7), EIP-712 attestation digests (§3.6), signature sets (§3.8),
// commitment and history paths (§3.4, §3.5), transfer and control leaves, and
// the §3.2 transfer payload in both directions.

import "sccp-cells"
import "sccp-constants"
import "sccp-crypto"
import "sccp-errors"
import "sccp-messages"

// ---------------------------------------------------------------------------
// Rosters (§3.7).

/// `⌊2n/3⌋ + 1`.
@pure
fun sccpThreshold(n: int): int {
    return (2 * n) / 3 + 1;
}

/// Recomputes the §3.7 roster digest while checking the `n` range, the `t`
/// formula, `valid_until > valid_from`, `generation ≥ 1` and the member order
/// (zero members first, then nonzero members strictly ascending). The members
/// list must be canonical and made of ordinary cells (`BadSnake` otherwise).
/// Throws `BadRoster` on any other violation.
fun sccpRosterDigest(
    networkId: int,
    generation: int,
    validFromMs: int,
    validUntilMs: int,
    n: int,
    t: int,
    members: cell,
): uint256 {
    assert ((n >= SCCP_MIN_ROSTER_SIZE) & (n <= SCCP_MAX_ROSTER_SIZE)) throw SccpErrors.BadRoster;
    assert (t == sccpThreshold(n)) throw SccpErrors.BadRoster;
    assert (generation >= 1) throw SccpErrors.BadRoster;
    assert (validUntilMs > validFromMs) throw SccpErrors.BadRoster;
    val (values, parts) = sccpLoadMembers(members, n);
    var previous = 0;
    var i = 0;
    while (i < n) {
        val member = values.get(i);
        if (member == 0) {
            assert (previous == 0) throw SccpErrors.BadRoster;
        } else {
            assert (member > previous) throw SccpErrors.BadRoster;
            previous = member;
        }
        i += 1;
    }
    var preimage: tuple = [];
    preimage.push(
        beginCell()
            .storeUint(SCCP_TAG_ROSTER_V1, SCCP_TAG_ROSTER_V1_BITS)
            .storeUint(networkId, 256)
            .storeUint(generation, 64)
            .storeUint(validFromMs, 64)
            .storeUint(validUntilMs, 64)
            .storeUint(n, 8)
            .storeUint(t, 8),
    );
    i = 0;
    while (i < parts.size()) {
        preimage.push(parts.get(i));
        i += 1;
    }
    return keccak256Parts(preimage);
}

/// §5.1.5 validity bounds of a roster installed at `nowMs` (initial roster at
/// `sccp_init`, or `next` at a rotation); throws `BadRosterValidity`.
fun sccpCheckRosterValidity(validFromMs: int, validUntilMs: int, nowMs: int) {
    assert (validFromMs <= nowMs + SCCP_MAX_CLOCK_SKEW_MS) throw SccpErrors.BadRosterValidity;
    assert (
        validUntilMs - validFromMs <= SCCP_MAX_ROSTER_VALIDITY_MS
    ) throw SccpErrors.BadRosterValidity;
    assert (nowMs < validUntilMs) throw SccpErrors.BadRosterValidity;
    assert (validUntilMs <= nowMs + SCCP_MAX_ROSTER_VALIDITY_MS) throw SccpErrors.BadRosterValidity;
}

// ---------------------------------------------------------------------------
// Attestations (§3.6).

/// `DOMAIN_SEPARATOR` for `salt = networkId`.
@pure
fun sccpDomainSeparator(networkId: int): uint256 {
    return keccak256Builders2(
        beginCell().storeUint(SCCP_DOMAIN_TYPEHASH, 256).storeUint(SCCP_NAME_HASH, 256),
        beginCell().storeUint(SCCP_VERSION_HASH, 256).storeUint(networkId, 256),
    );
}

/// EIP-712 digest of an attestation statement.
fun sccpAttestationDigest(networkId: int, a: Attestation, tail: AttestationTail): uint256 {
    var words: tuple = [];
    words.push(
        beginCell()
            .storeUint(SCCP_ATTESTATION_TYPEHASH, 256)
            .storeUint(a.height, 256)
            .storeUint(a.epoch, 256),
    );
    words.push(
        beginCell()
            .storeUint(a.timestampMs, 256)
            .storeUint(a.blockHash, 256)
            .storeUint(a.sccpRoot, 256),
    );
    words.push(
        beginCell()
            .storeUint(a.messageCount, 256)
            .storeUint(tail.historyRoot, 256)
            .storeUint(a.historySize, 256),
    );
    words.push(
        beginCell().storeUint(tail.rosterDigest, 256).storeUint(tail.nextRosterDigest, 256),
    );
    val structHash = keccak256Parts(words);
    return keccak256Builder(
        beginCell()
            .storeUint(0x1901, 16)
            .storeUint(sccpDomainSeparator(networkId), 256)
            .storeUint(structHash, 256),
    );
}

/// §3.6.1 invariants checked by every verifier; throws `BadAttestation`.
fun sccpCheckAttestation(a: Attestation, tail: AttestationTail) {
    assert ((a.messageCount == 0) == (a.sccpRoot == 0)) throw SccpErrors.BadAttestation;
    assert ((a.historySize == 0) == (tail.historyRoot == 0)) throw SccpErrors.BadAttestation;
    assert (a.messageCount <= SCCP_MAX_BLOCK_LEAVES) throw SccpErrors.BadAttestation;
}

/// Verifies a §3.8 signature set over `digest` against the stored members.
///
/// `signatures#_ signer_bitmap:uint32 first:^SignatureCell`; one 521-bit
/// `signature_cell#_ r:uint256 s:uint256 v:uint8 next:(Maybe ^SignatureCell)`
/// per set bit, in ascending bit order. Bits ≥ `n` must be zero, every set bit
/// must address a nonzero member equal to the recovered address, and at least
/// `t` bits must be set. `members` is a canonical stored `^MemberChunk` list
/// (validated when it was installed), walked once up to the highest set bit.
fun sccpVerifySignatures(digest: uint256, signatures: cell, members: cell, n: int, t: int) {
    var cs = signatures.beginParse();
    assert (
        (cs.remainingBitsCount() == 32) & (cs.remainingRefsCount() == 1)
    ) throw SccpErrors.BadSignatures;
    var pending = cs.loadUint(32);
    assert ((pending >> n) == 0) throw SccpErrors.BadSignatures;
    var sig = cs.preloadRef().beginParse();
    var member = members.beginParse();
    var inChunk = 0;
    var count = 0;
    var more = true;
    while (pending != 0) {
        if (inChunk == SCCP_MEMBERS_PER_CHUNK) {
            member = member.preloadRef().beginParse();
            inChunk = 0;
        }
        val signer = member.loadUint(160);
        inChunk += 1;
        if ((pending & 1) != 0) {
            assert (more & (sig.remainingBitsCount() == 521)) throw SccpErrors.BadSignatures;
            val r = sig.loadUint(256);
            val s = sig.loadUint(256);
            val v = sig.loadUint(8);
            more = sig.loadBool();
            assert (sig.remainingRefsCount() == (more ? 1 : 0)) throw SccpErrors.BadSignatures;
            // §3.8: nonzero member, 1 ≤ r < N, 1 ≤ s ≤ HALF_N, v ∈ {27, 28}
            assert (
                (signer != 0) & (r >= 1) & (r < SECP256K1_N) & (s >= 1) & (s <= SECP256K1_HALF_N) &
                ((v == 27) | (v == 28))
            ) throw SccpErrors.BadSignatures;
            val (_, x, y, ok) = ecrecoverRaw(digest, v - 27, r, s);
            assert (ok) throw SccpErrors.BadSignatures;
            assert (sccpAddressOfPoint(x!, y!) == signer) throw SccpErrors.BadSignatures;
            count += 1;
            if (more) {
                sig = sig.preloadRef().beginParse();
            }
        }
        pending = pending >> 1;
    }
    assert (!more) throw SccpErrors.BadSignatures;
    assert (count >= t) throw SccpErrors.TooFewSignatures;
}

// ---------------------------------------------------------------------------
// Commitment and history trees (§3.4, §3.5).

/// `node(l, r) = keccak256("SCCP/NODE/V1" ‖ l ‖ r)`.
@inline
fun sccpNode(left: int, right: int): uint256 {
    return keccak256Builder(
        beginCell()
            .storeUint(SCCP_TAG_NODE_V1, SCCP_TAG_NODE_V1_BITS)
            .storeUint(left, 256)
            .storeUint(right, 256),
    );
}

/// Positional promote-odd Merkle root (§3.4); throws `BadProof`.
fun sccpMerkleRoot(leaf: int, index: int, count: int, siblings: array<int>): int {
    assert ((count >= 1) & (index >= 0) & (index < count)) throw SccpErrors.BadProof;
    var h = leaf;
    var k = 0;
    var i = index;
    var c = count;
    val len = siblings.size();
    while (c > 1) {
        if ((i & 1) != 0) {
            assert (k < len) throw SccpErrors.BadProof;
            h = sccpNode(siblings.get(k), h);
            k += 1;
        } else if (i + 1 < c) {
            assert (k < len) throw SccpErrors.BadProof;
            h = sccpNode(h, siblings.get(k));
            k += 1;
        }
        i = i >> 1;
        c = (c + 1) >> 1;
    }
    assert (k == len) throw SccpErrors.BadProof;
    return h;
}

/// `history_leaf(height, sccp_root, message_count)`.
@pure
fun sccpHistoryLeaf(height: int, sccpRoot: int, messageCount: int): uint256 {
    return keccak256Builder(
        beginCell()
            .storeUint(SCCP_TAG_HISTORY_V1, SCCP_TAG_HISTORY_V1_BITS)
            .storeUint(height, 64)
            .storeUint(sccpRoot, 256)
            .storeUint(messageCount, 32),
    );
}

/// Verifies that `leaf` is committed by the attestation: directly against
/// `A.sccpRoot`, or (historical mode) against the proven block and that
/// block's history leaf against `A.historyRoot`. Throws `BadProof`.
fun sccpVerifyLeafInclusion(
    leaf: int,
    leafIndex: int,
    pathLen: int,
    path: cell?,
    a: Attestation,
    tail: AttestationTail,
    history: Cell<HistoryProof>?,
) {
    val siblings = sccpLoadHashPath(path, pathLen, SCCP_MAX_BLOCK_PATH);
    if (history == null) {
        assert (a.messageCount >= 1) throw SccpErrors.BadProof;
        assert (
            sccpMerkleRoot(leaf, leafIndex, a.messageCount, siblings) == a.sccpRoot
        ) throw SccpErrors.BadProof;
        return;
    }
    val h = history!.load();
    assert (
        (h.messageCount >= 1) & (h.messageCount <= SCCP_MAX_BLOCK_LEAVES)
    ) throw SccpErrors.BadProof;
    assert (h.sccpRoot != 0) throw SccpErrors.BadProof;
    assert (
        sccpMerkleRoot(leaf, leafIndex, h.messageCount, siblings) == h.sccpRoot
    ) throw SccpErrors.BadProof;
    assert (
        (a.historySize >= 1) & (a.historySize <= SCCP_MAX_HISTORY_SIZE)
    ) throw SccpErrors.BadProof;
    val historySiblings = sccpLoadHashPath(h.path, h.pathLen, SCCP_MAX_HISTORY_PATH);
    val historyLeaf = sccpHistoryLeaf(h.height, h.sccpRoot, h.messageCount);
    assert (
        sccpMerkleRoot(historyLeaf, h.leafIndex, a.historySize, historySiblings) == tail.historyRoot
    ) throw SccpErrors.BadProof;
}

// ---------------------------------------------------------------------------
// Leaves and message ids (§3.3, §3.4).

/// `message_id` of a Taira→TON payload (lane `(sora-taira, ton-mainnet)`).
@pure
fun sccpInboundMessageId(networkId: int, payloadHash: int): uint256 {
    return keccak256Builder(
        beginCell()
            .storeUint(SCCP_TAG_MESSAGE_V1, SCCP_TAG_MESSAGE_V1_BITS)
            .storeUint(SCCP_TAG_TAIRA, 8)
            .storeUint(networkId, 256)
            .storeUint(SCCP_TAG_TON, 8)
            .storeUint(SCCP_TON_IDENTITY_WORD, 256)
            .storeUint(payloadHash, 256),
    );
}

/// `message_id` of a TON→Taira payload (lane `(ton-mainnet, sora-taira)`).
@pure
fun sccpOutboundMessageId(networkId: int, payloadHash: int): uint256 {
    return keccak256Builder(
        beginCell()
            .storeUint(SCCP_TAG_MESSAGE_V1, SCCP_TAG_MESSAGE_V1_BITS)
            .storeUint(SCCP_TAG_TON, 8)
            .storeUint(SCCP_TON_IDENTITY_WORD, 256)
            .storeUint(SCCP_TAG_TAIRA, 8)
            .storeUint(networkId, 256)
            .storeUint(payloadHash, 256),
    );
}

/// `transfer_leaf = keccak256("SCCP/LEAF/V1" ‖ message_id ‖ destination_word)`.
@pure
fun sccpTransferLeaf(messageId: int, destinationWord: int): uint256 {
    return keccak256Builder(
        beginCell()
            .storeUint(SCCP_TAG_LEAF_V1, SCCP_TAG_LEAF_V1_BITS)
            .storeUint(messageId, 256)
            .storeUint(destinationWord, 256),
    );
}

/// Control leaf of §3.4 for `target = ton-mainnet` (126-byte preimage).
@pure
fun sccpControlLeaf(
    networkId: int,
    destinationWord: int,
    routeRevision: int,
    controlNonce: int,
    paused: bool,
): uint256 {
    return keccak256Builder(
        beginCell()
            .storeUint(SCCP_TAG_CONTROL_V1, SCCP_TAG_CONTROL_V1_BITS)
            .storeUint(SCCP_TAG_TAIRA, 8)
            .storeUint(networkId, 256)
            .storeUint(SCCP_TAG_TON, 8)
            .storeUint(SCCP_TON_IDENTITY_WORD, 256)
            .storeUint(destinationWord, 256)
            .storeUint(routeRevision, 32)
            .storeUint(controlNonce, 64)
            .storeUint(paused ? 1 : 0, 8),
    );
}

// ---------------------------------------------------------------------------
// Transfer payloads (§3.2).

/// Fields of a validated Taira→TON payload.
struct InboundPayload {
    nonce: uint64
    deadlineMs: uint64
    amount: int
    /// Account id of the `addr_std` workchain-0 recipient.
    recipientHash: uint256
    payloadHash: uint256
}

/// `payload_hash = keccak256("SCCP/PAYLOAD/V1" ‖ payload)` over chunk parts.
fun sccpPayloadHash(chunks: tuple): uint256 {
    var parts: tuple = [];
    parts.push(beginCell().storeUint(SCCP_TAG_PAYLOAD_V1, SCCP_TAG_PAYLOAD_V1_BITS));
    var i = 0;
    val size = chunks.size();
    while (i < size) {
        parts.push(chunks.get(i));
        i += 1;
    }
    return keccak256Parts(parts);
}

/// Bytes of a Taira→TON payload before the sender bytes (kind … sender_len).
const SCCP_INBOUND_PREFIX_BYTES = 59
/// Bytes after the sender (recipient codec … route id).
const SCCP_INBOUND_SUFFIX_BYTES = 55

/// Parses and validates a Taira→TON payload for this deployment (§3.2 and
/// §5.1.3 step 4): kind, version, domains, revision, nonzero deadline, asset,
/// amount (nonzero, below 2^96), codecs and lengths, the `ton_account36`
/// recipient (workchain 0, nonzero, not the minter itself), the route id and
/// no trailing bytes. The deadline itself is checked by the caller.
///
/// The fixed prefix always lies in the first 127-byte chunk; the fixed suffix
/// spans at most the last two chunks and is joined into one slice.
fun sccpParseInboundPayload(payload: cell, routeRevision: int, minterHash: int): InboundPayload {
    val (chunks, total) = sccpParseSnakeBytes(
        payload,
        SCCP_MAX_PAYLOAD_BYTES,
        SccpErrors.BadPayload,
    );
    assert (
        total > SCCP_INBOUND_PREFIX_BYTES + SCCP_INBOUND_SUFFIX_BYTES
    ) throw SccpErrors.BadPayload;
    var s = chunks.get(0);
    assert (s.loadUint(8) == SCCP_PAYLOAD_KIND_TRANSFER) throw SccpErrors.BadPayload;
    assert (s.loadUint(8) == SCCP_PAYLOAD_VERSION) throw SccpErrors.BadPayload;
    assert (s.loadUint(32) == SCCP_DOMAIN_TAIRA) throw SccpErrors.BadPayload;
    assert (s.loadUint(32) == SCCP_DOMAIN_TON) throw SccpErrors.BadPayload;
    val nonce = s.loadUint(64);
    assert (s.loadUint(32) == routeRevision) throw SccpErrors.BadPayload;
    val deadlineMs = s.loadUint(64);
    assert (deadlineMs != 0) throw SccpErrors.BadPayload;
    assert (s.loadUint(32) == SCCP_DOMAIN_TAIRA) throw SccpErrors.BadPayload;
    assert (s.loadUint(8) == SCCP_CODEC_CANONICAL_TEXT) throw SccpErrors.BadPayload;
    assert (s.loadUint(16) == SCCP_ASSET_ID_XOR_LEN) throw SccpErrors.BadPayload;
    assert (s.loadUint(24) == SCCP_ASSET_ID_XOR) throw SccpErrors.BadPayload;
    val amount = s.loadUint(128);
    assert ((amount != 0) & (amount < SCCP_TON_AMOUNT_LIMIT)) throw SccpErrors.BadAmount;
    assert (s.loadUint(8) == SCCP_CODEC_TAIRA_ACCOUNT) throw SccpErrors.BadPayload;
    val senderLen = s.loadUint(16);
    assert (
        (senderLen >= 1) & (senderLen <= SCCP_MAX_TAIRA_ACCOUNT_BYTES)
    ) throw SccpErrors.BadPayload;
    val offset = SCCP_INBOUND_PREFIX_BYTES + senderLen;
    assert (total == offset + SCCP_INBOUND_SUFFIX_BYTES) throw SccpErrors.BadPayload;
    val (index, within) = divMod(offset, SCCP_SNAKE_CHUNK_BYTES);
    var suffix = chunks.get(index);
    suffix.skipBits(within * 8);
    if (index + 1 < chunks.size()) {
        suffix = beginCell().storeSlice(suffix).storeSlice(chunks.get(index + 1)).toSlice();
    }
    assert (suffix.loadUint(8) == SCCP_CODEC_TON_ACCOUNT36) throw SccpErrors.BadPayload;
    assert (suffix.loadUint(16) == SCCP_TON_ACCOUNT36_LEN) throw SccpErrors.BadPayload;
    val workchain = suffix.loadUint(32);
    val recipientHash = suffix.loadUint(256);
    assert (workchain == 0) throw SccpErrors.BadRecipient;
    assert (recipientHash != 0) throw SccpErrors.BadRecipient;
    assert (recipientHash != minterHash) throw SccpErrors.BadRecipient;
    assert (suffix.loadUint(8) == SCCP_CODEC_CANONICAL_TEXT) throw SccpErrors.BadPayload;
    assert (suffix.loadUint(16) == SCCP_ROUTE_ID_TON_LEN) throw SccpErrors.BadPayload;
    assert (
        suffix.loadUint(SCCP_ROUTE_ID_TON_LEN * 8) == SCCP_ROUTE_ID_TON
    ) throw SccpErrors.BadPayload;
    return {
        nonce,
        deadlineMs,
        amount,
        recipientHash,
        payloadHash: sccpPayloadHash(chunks as tuple),
    };
}

/// A built TON→Taira payload.
struct OutboundPayload {
    /// Canonical `SnakeBytes`.
    cell: cell
    payloadHash: uint256
}

/// Builds the TON→Taira payload of a burn (§5.1.7): `dest_domain 0`,
/// `deadline_ms 0`, sender = `ton_account36` of the owner, recipient = the
/// validated `taira_account` chunks.
fun sccpBuildOutboundPayload(
    nonce: int,
    routeRevision: int,
    amount: int,
    ownerHash: int,
    recipientChunks: array<slice>,
    recipientLen: int,
): OutboundPayload {
    // kind … recipient_len: 98 bytes in the first chunk
    var w: SnakeWriter = {
        done: [],
        cur: beginCell()
            .storeUint(SCCP_PAYLOAD_KIND_TRANSFER, 8)
            .storeUint(SCCP_PAYLOAD_VERSION, 8)
            .storeUint(SCCP_DOMAIN_TON, 32)
            .storeUint(SCCP_DOMAIN_TAIRA, 32)
            .storeUint(nonce, 64)
            .storeUint(routeRevision, 32)
            .storeUint(0, 64)
            .storeUint(SCCP_DOMAIN_TAIRA, 32)
            .storeUint(SCCP_CODEC_CANONICAL_TEXT, 8)
            .storeUint(SCCP_ASSET_ID_XOR_LEN, 16)
            .storeUint(SCCP_ASSET_ID_XOR, 24)
            .storeUint(amount, 128)
            .storeUint(SCCP_CODEC_TON_ACCOUNT36, 8)
            .storeUint(SCCP_TON_ACCOUNT36_LEN, 16)
            .storeUint(0, 32)
            .storeUint(ownerHash, 256)
            .storeUint(SCCP_CODEC_TAIRA_ACCOUNT, 8)
            .storeUint(recipientLen, 16),
    };
    var i = 0;
    while (i < recipientChunks.size()) {
        w.writeSlice(recipientChunks.get(i));
        i += 1;
    }
    // route_id codec ‖ len ‖ "taira_ton_xor" (16 bytes)
    w.writeUint(
        (SCCP_CODEC_CANONICAL_TEXT << 120) | (SCCP_ROUTE_ID_TON_LEN << 104) | SCCP_ROUTE_ID_TON,
        16,
    );
    val chunks = w.chunks();
    return { cell: sccpSnakeCellFromChunks(chunks), payloadHash: sccpPayloadHash(chunks as tuple) };
}
