import "@stdlib/exotic-cells"
import "constants"
import "errors"
import "messages"

/// One final-V1 replay forest. Only roots which differ from the canonical
/// empty root are stored; the key type bounds the forest to exactly 256 shards.
struct SccpReplayForestV1 {
    nonemptyShardRoots: map<uint8, uint256>
    leafCount: uint64
    updateSequence: uint64
}

/// Semantic material committed by one occupied TON replay leaf.
struct SccpTonReplayRecordV1 {
    operation: uint8
    replayId: uint256
    payloadSha256: uint256
    amountScale9: uint128
    principal: address
    auxiliaryIdentitySha256: uint256
}

/// Result of admitting one exact record. `occupiedNow` is false only after a
/// strict membership witness proves an identical, already occupied leaf.
struct SccpReplayAdmissionV1 {
    occupiedNow: bool
    domainHash: uint256
    shard: uint8
    key: uint256
    recordDigest: uint256
    oldRoot: uint256
    newRoot: uint256
    leafCount: uint64
    updateSequence: uint64
}

const SCCP_REPLAY_DEPTH_V1 = 248
/// Final-V1 network-profile bytes. These are distinct from transfer-domain
/// identifiers (`SCCP_DOMAIN_*`) and no legacy/test profile is accepted.
const SCCP_REPLAY_NETWORK_SORA_V1 = SCCP_NETWORK_PROFILE_SORA_V1
const SCCP_REPLAY_NETWORK_TON_V1 = SCCP_NETWORK_PROFILE_TON_V1
const SCCP_REPLAY_ACTOR_TON_V1 = 3
const SCCP_REPLAY_PRINCIPAL_TON_V1 = 4

// Golden values shared with `fixtures/sccp/replay_forest_v1.json`.
const SCCP_REPLAY_EMPTY_LEAF_V1 = 0x6841d062186b649a505eb694ebce936fe978c5530596882a70c6e04303c88d43
const SCCP_REPLAY_EMPTY_ROOT_V1 = 0xcefd4f39c0d2ba5c33835008c6c3e7bca47d6ea1c4da5bfc8a63f09dbc66651f

/// # Safety
/// The caller provides one byte-aligned, reference-free slice.
@pure
fun sccpReplaySha256One(value: slice): uint256
    asm "1 PUSHINT" "0 HASHEXT"

/// # Safety
/// The caller provides two byte-aligned, reference-free slices.
@pure
fun sccpReplaySha256Two(first: slice, second: slice): uint256
    asm "2 PUSHINT" "0 HASHEXT"

/// # Safety
/// The caller provides four byte-aligned, reference-free slices.
@pure
fun sccpReplaySha256Four(first: slice, second: slice, third: slice, fourth: slice): uint256
    asm "4 PUSHINT" "0 HASHEXT"

/// # Safety
/// The caller provides five byte-aligned, reference-free slices.
@pure
fun sccpReplaySha256Five(
    first: slice,
    second: slice,
    third: slice,
    fourth: slice,
    fifth: slice,
): uint256
    asm "5 PUSHINT" "0 HASHEXT"

/// # Safety
/// The caller provides eight byte-aligned, reference-free slices.
@pure
fun sccpReplaySha256Eight(
    first: slice,
    second: slice,
    third: slice,
    fourth: slice,
    fifth: slice,
    sixth: slice,
    seventh: slice,
    eighth: slice,
): uint256
    asm "8 PUSHINT" "0 HASHEXT"

/// Canonical signed-i32-big-endian workchain plus raw 32-byte account id.
/// Fixed-width zero account ids are intentionally not reinterpreted as absent.
fun sccpReplayTonAddress36(value: address): cell {
    val encoded = value as slice;
    assert (
        encoded.remainingBitsCount() == 267 && encoded.remainingRefsCount() == 0 &&
        encoded.preloadUint(3) == 0b100
    ) throw SccpErrors.InvalidReplayRecord;
    val (workchain, accountHash) = value.getWorkchainAndHash();
    return beginCell().storeInt(workchain, 32).storeUint(accountHash, 256).endCell();
}

/// Decode the one accepted witness-header cell shape before any field is used.
/// The typed decoder then independently enforces complete field consumption.
fun sccpReplayLoadWitnessV1(
    witnessCell: Cell<SccpReplayWitnessV1>,
): SccpReplayWitnessV1 {
    val raw = witnessCell as cell;
    val (encodedValue, exotic) = raw.beginParseSpecial();
    var encoded = encodedValue;
    assert (
        !exotic && encoded.remainingBitsCount() == 3 * 256 + 1 && encoded.remainingRefsCount() <= 1
    ) throw SccpErrors.NonCanonicalReplayWitness;
    val encodedRefCount = encoded.remainingRefsCount();
    encoded.skipBits(3 * 256);
    val siblingRefPresent = encoded.loadUint(1);
    assert (
        encoded.remainingBitsCount() == 0 && encodedRefCount == siblingRefPresent
    ) throw SccpErrors.NonCanonicalReplayWitness;
    val witness = witnessCell.load();
    assert (
        encoded.remainingRefsCount() == (witness.siblings == null ? 0 : 1) &&
        ((witness.siblingBitmap == 0) == (witness.siblings == null))
    ) throw SccpErrors.NonCanonicalReplayWitness;
    return witness;
}

fun sccpReplayOperationIsInbound(operation: uint8): bool {
    return operation == 0x30 || operation == 0x32 || operation == 0x34 || operation == 0x36 ||
    operation == 0x37;
}

fun sccpReplayOperationIsOutbound(operation: uint8): bool {
    return operation == 0x31 || operation == 0x33 || operation == 0x35;
}

/// Hash the exact final-V1 TON replay domain.
fun sccpTonReplayDomainHashV1(
    operation: uint8,
    routeRevision: uint32,
    routeConfigurationHash: uint256,
    actor: address,
): uint256 {
    assert (
        routeRevision != 0 && routeConfigurationHash != 0 &&
        (sccpReplayOperationIsInbound(operation) || sccpReplayOperationIsOutbound(operation))
    ) throw SccpErrors.InvalidReplayDomain;
    val source = sccpReplayOperationIsInbound(operation)
        ? SCCP_REPLAY_NETWORK_SORA_V1
        : SCCP_REPLAY_NETWORK_TON_V1;
    val target = sccpReplayOperationIsInbound(operation)
        ? SCCP_REPLAY_NETWORK_TON_V1
        : SCCP_REPLAY_NETWORK_SORA_V1;
    val actorBytes = sccpReplayTonAddress36(actor);
    val preimage = beginCell()
        .storeSlice("SCCP-REPLAY-SMT-V1".literalSlice())
        .storeUint(0x00, 8)
        .storeUint(source, 32)
        .storeUint(target, 32)
        .storeUint(operation, 8)
        .storeUint(routeRevision, 32)
        .storeUint(routeConfigurationHash, 256)
        .storeUint(SCCP_REPLAY_ACTOR_TON_V1, 8)
        .storeUint(36, 16)
        .storeSlice(actorBytes.beginParse())
        .endCell();
    return sccpReplaySha256One(preimage.beginParse());
}

/// SHA-256 of the exact four flat canonical SCCP payload cells.
fun sccpReplayPayloadSha256V1(payload: CanonicalPayloadCells): uint256 {
    val (header, headerExotic) = payload.header.beginParseSpecial();
    val (chunk1, chunk1Exotic) = payload.chunk1.beginParseSpecial();
    val (chunk2, chunk2Exotic) = payload.chunk2.beginParseSpecial();
    val (chunk3, chunk3Exotic) = payload.chunk3.beginParseSpecial();
    assert (
        !headerExotic && !chunk1Exotic && !chunk2Exotic && !chunk3Exotic &&
        header.remainingRefsCount() == 0 &&
        chunk1.remainingRefsCount() == 0 &&
        chunk2.remainingRefsCount() == 0 &&
        chunk3.remainingRefsCount() == 0 &&
        header.remainingBitsCount() == SCCP_CANONICAL_HEADER_BYTES * 8 &&
        chunk1.remainingBitsCount() % 8 == 0 &&
        chunk2.remainingBitsCount() % 8 == 0 &&
        chunk3.remainingBitsCount() % 8 == 0
    ) throw SccpErrors.InvalidPayload;
    val firstLength = chunk1.remainingBitsCount() / 8;
    val secondLength = chunk2.remainingBitsCount() / 8;
    val thirdLength = chunk3.remainingBitsCount() / 8;
    assert (
        firstLength <= SCCP_CANONICAL_CHUNK_BYTES && secondLength <= SCCP_CANONICAL_CHUNK_BYTES &&
        thirdLength <= 124 &&
        (firstLength == SCCP_CANONICAL_CHUNK_BYTES || (secondLength == 0 && thirdLength == 0)) &&
        (secondLength == SCCP_CANONICAL_CHUNK_BYTES || thirdLength == 0) &&
        SCCP_CANONICAL_HEADER_BYTES + firstLength + secondLength + thirdLength <=
        SCCP_MAX_CANONICAL_PAYLOAD_BYTES
    ) throw SccpErrors.InvalidPayload;
    return sccpReplaySha256Four(header, chunk1, chunk2, chunk3);
}

/// One canonical SHA-256 layer over a 32-byte auxiliary word.
fun sccpReplayAuxiliaryWordSha256V1(value: uint256): uint256 {
    return sccpReplaySha256One(beginCell().storeUint(value, 256).endCell().beginParse());
}

/// One canonical SHA-256 layer over a 36-byte TON account.
fun sccpReplayAuxiliaryAddressSha256V1(value: address): uint256 {
    return sccpReplaySha256One(sccpReplayTonAddress36(value).beginParse());
}

/// Route-local identity shared by the 0x36 debit and 0x37 rejected-lock
/// recredit. A checked wallet sequence makes identities unique even when an
/// owner repeats query ids, nonces are gapped, or payload bytes are equal.
fun sccpReplayWalletBurnLockIdV1(
    walletAddress: address,
    lockSequence: uint64,
): uint256 {
    val first = beginCell()
        .storeSlice("SCCP-TON-BURN-LOCK-V1".literalSlice())
        .storeSlice(sccpReplayTonAddress36(walletAddress).beginParse())
        .endCell();
    val second = beginCell().storeUint(lockSequence, 64).endCell();
    val identity = sccpReplaySha256Two(first.beginParse(), second.beginParse());
    assert (identity != 0) throw SccpErrors.InvalidReplayRecord;
    return identity;
}

fun sccpReplayKeyV1(domainHash: uint256, replayId: uint256): uint256 {
    val preimage = beginCell()
        .storeSlice("SCCP-REPLAY-SMT-V1".literalSlice())
        .storeUint(0x01, 8)
        .storeUint(domainHash, 256)
        .storeUint(replayId, 256)
        .endCell();
    return sccpReplaySha256One(preimage.beginParse());
}

fun sccpTonReplayRecordDigestV1(record: SccpTonReplayRecordV1): uint256 {
    assert (
        (sccpReplayOperationIsInbound(record.operation) ||
        sccpReplayOperationIsOutbound(record.operation)) &&
        record.replayId != 0 &&
        record.payloadSha256 != 0 &&
        record.amountScale9 != 0 &&
        record.auxiliaryIdentitySha256 != 0
    ) throw SccpErrors.InvalidReplayRecord;
    val principalBytes = sccpReplayTonAddress36(record.principal);
    val principalPreimage = beginCell()
        .storeSlice("SCCP-REPLAY-SMT-V1".literalSlice())
        .storeUint(0x03, 8)
        .storeUint(SCCP_REPLAY_PRINCIPAL_TON_V1, 8)
        .storeUint(36, 16)
        .storeSlice(principalBytes.beginParse())
        .endCell();
    val principalDigest = sccpReplaySha256One(principalPreimage.beginParse());
    val auxiliaryPreimage = beginCell()
        .storeSlice("SCCP-REPLAY-SMT-V1".literalSlice())
        .storeUint(0x04, 8)
        .storeUint(record.operation, 8)
        .storeUint(record.auxiliaryIdentitySha256, 256)
        .endCell();
    val auxiliaryDigest = sccpReplaySha256One(auxiliaryPreimage.beginParse());
    val first = beginCell()
        .storeSlice("SCCP-REPLAY-SMT-V1".literalSlice())
        .storeUint(0x02, 8)
        .storeUint(record.operation, 8)
        .storeUint(record.replayId, 256)
        .storeUint(record.payloadSha256, 256)
        .storeUint(record.amountScale9, 128)
        .endCell();
    val second = beginCell()
        .storeUint(principalDigest, 256)
        .storeUint(auxiliaryDigest, 256)
        .endCell();
    return sccpReplaySha256Two(first.beginParse(), second.beginParse());
}

fun sccpReplayParentV1(level: int, left: uint256, right: uint256): uint256 {
    val preimage = beginCell()
        .storeSlice("SCCP-REPLAY-SMT-V1".literalSlice())
        .storeUint(0x12, 8)
        .storeUint(level, 16)
        .storeUint(left, 256)
        .storeUint(right, 256)
        .endCell();
    return sccpReplaySha256One(preimage.beginParse());
}

fun sccpReplayOccupiedLeafV1(key: uint256, recordDigest: uint256): uint256 {
    val preimage = beginCell()
        .storeSlice("SCCP-REPLAY-SMT-V1".literalSlice())
        .storeUint(0x11, 8)
        .storeUint(key, 256)
        .storeUint(recordDigest, 256)
        .endCell();
    return sccpReplaySha256One(preimage.beginParse());
}

fun sccpReplayPopcountV1(bitmap: uint256): int {
    var value = bitmap;
    var count = 0;
    while (value != 0) {
        count += value & 1;
        value = value >> 1;
    }
    return count;
}

/// Fold one canonical witness. The sibling snake is uniquely chunked as three
/// hashes in every nonterminal cell and one to three in the final cell.
fun sccpReplayFoldV1(
    key: uint256,
    leaf: uint256,
    witness: SccpReplayWitnessV1,
): uint256 {
    assert (
        witness.expectedShardRoot != 0 && witness.siblingBitmap >> SCCP_REPLAY_DEPTH_V1 == 0
    ) throw SccpErrors.NonCanonicalReplayWitness;
    val explicitCount = sccpReplayPopcountV1(witness.siblingBitmap);
    assert (explicitCount <= SCCP_REPLAY_DEPTH_V1) throw SccpErrors.NonCanonicalReplayWitness;
    // At most ceil(248 / 3) = 83 sibling cells are traversed.
    val expectedCells = (explicitCount + 2) / 3;
    assert (expectedCells <= 83) throw SccpErrors.NonCanonicalReplayWitness;
    var siblings = beginCell().endCell().beginParse();
    if (explicitCount == 0) {
        assert (witness.siblings == null) throw SccpErrors.NonCanonicalReplayWitness;
    } else {
        assert (witness.siblings != null) throw SccpErrors.NonCanonicalReplayWitness;
        val (initialSiblings, initialExotic) = witness.siblings!.beginParseSpecial();
        assert (!initialExotic) throw SccpErrors.NonCanonicalReplayWitness;
        siblings = initialSiblings;
    }

    var root = leaf;
    var empty = SCCP_REPLAY_EMPTY_LEAF_V1;
    var supplied = 0;
    var traversedCells = explicitCount == 0 ? 0 : 1;
    var level = 0;
    while (level < SCCP_REPLAY_DEPTH_V1) {
        var sibling = empty;
        if (((witness.siblingBitmap >> level) & 1) != 0) {
            if (supplied % 3 == 0) {
                val remaining = explicitCount - supplied;
                val inThisCell = remaining > 3 ? 3 : remaining;
                assert (
                    siblings.remainingBitsCount() == inThisCell * 256 &&
                    siblings.remainingRefsCount() == (remaining > 3 ? 1 : 0)
                ) throw SccpErrors.NonCanonicalReplayWitness;
            }
            sibling = siblings.loadUint(256);
            assert (sibling != empty) throw SccpErrors.NonCanonicalReplayWitness;
            supplied += 1;
            if (supplied < explicitCount && supplied % 3 == 0) {
                val next = siblings.loadRef();
                siblings.assertEnd();
                traversedCells += 1;
                assert (traversedCells <= expectedCells) throw SccpErrors.NonCanonicalReplayWitness;
                val (nextSiblings, nextExotic) = next.beginParseSpecial();
                assert (!nextExotic) throw SccpErrors.NonCanonicalReplayWitness;
                siblings = nextSiblings;
            }
        }
        root = ((key >> level) & 1) != 0
            ? sccpReplayParentV1(level, sibling, root)
            : sccpReplayParentV1(level, root, sibling);
        empty = sccpReplayParentV1(level, empty, empty);
        level += 1;
    }
    assert (
        supplied == explicitCount && traversedCells == expectedCells
    ) throw SccpErrors.NonCanonicalReplayWitness;
    siblings.assertEnd();
    return root;
}

/// Fold the same canonical path for an empty and occupied leaf in one pass.
/// Reusing the default-chain hash is required to keep a fresh depth-248
/// admission below TON's transaction gas ceiling.
fun sccpReplayFoldPairV1(
    key: uint256,
    firstLeaf: uint256,
    secondLeaf: uint256,
    witness: SccpReplayWitnessV1,
): (uint256, uint256) {
    assert (
        witness.expectedShardRoot != 0 && witness.siblingBitmap >> SCCP_REPLAY_DEPTH_V1 == 0
    ) throw SccpErrors.NonCanonicalReplayWitness;
    val explicitCount = sccpReplayPopcountV1(witness.siblingBitmap);
    assert (explicitCount <= SCCP_REPLAY_DEPTH_V1) throw SccpErrors.NonCanonicalReplayWitness;
    val expectedCells = (explicitCount + 2) / 3;
    assert (expectedCells <= 83) throw SccpErrors.NonCanonicalReplayWitness;
    var siblings = beginCell().endCell().beginParse();
    if (explicitCount == 0) {
        assert (witness.siblings == null) throw SccpErrors.NonCanonicalReplayWitness;
    } else {
        assert (witness.siblings != null) throw SccpErrors.NonCanonicalReplayWitness;
        val (initialSiblings, initialExotic) = witness.siblings!.beginParseSpecial();
        assert (!initialExotic) throw SccpErrors.NonCanonicalReplayWitness;
        siblings = initialSiblings;
    }

    var firstRoot = firstLeaf;
    var secondRoot = secondLeaf;
    var empty = SCCP_REPLAY_EMPTY_LEAF_V1;
    var supplied = 0;
    var traversedCells = explicitCount == 0 ? 0 : 1;
    var level = 0;
    while (level < SCCP_REPLAY_DEPTH_V1) {
        var sibling = empty;
        if (((witness.siblingBitmap >> level) & 1) != 0) {
            if (supplied % 3 == 0) {
                val remaining = explicitCount - supplied;
                val inThisCell = remaining > 3 ? 3 : remaining;
                assert (
                    siblings.remainingBitsCount() == inThisCell * 256 &&
                    siblings.remainingRefsCount() == (remaining > 3 ? 1 : 0)
                ) throw SccpErrors.NonCanonicalReplayWitness;
            }
            sibling = siblings.loadUint(256);
            assert (sibling != empty) throw SccpErrors.NonCanonicalReplayWitness;
            supplied += 1;
            if (supplied < explicitCount && supplied % 3 == 0) {
                val next = siblings.loadRef();
                siblings.assertEnd();
                traversedCells += 1;
                assert (traversedCells <= expectedCells) throw SccpErrors.NonCanonicalReplayWitness;
                val (nextSiblings, nextExotic) = next.beginParseSpecial();
                assert (!nextExotic) throw SccpErrors.NonCanonicalReplayWitness;
                siblings = nextSiblings;
            }
        }
        if (((key >> level) & 1) != 0) {
            firstRoot = sccpReplayParentV1(level, sibling, firstRoot);
            secondRoot = sccpReplayParentV1(level, sibling, secondRoot);
        } else {
            firstRoot = sccpReplayParentV1(level, firstRoot, sibling);
            secondRoot = sccpReplayParentV1(level, secondRoot, sibling);
        }
        empty = sccpReplayParentV1(level, empty, empty);
        level += 1;
    }
    assert (
        supplied == explicitCount && traversedCells == expectedCells
    ) throw SccpErrors.NonCanonicalReplayWitness;
    siblings.assertEnd();
    return (firstRoot, secondRoot);
}

fun emptySccpReplayForestV1(): Cell<SccpReplayForestV1> {
    return SccpReplayForestV1 {
        nonemptyShardRoots: [],
        leafCount: 0,
        updateSequence: 0,
    }.toCell();
}

fun sccpReplayValidateForestV1(forest: SccpReplayForestV1) {
    assert (
        forest.updateSequence == forest.leafCount &&
        ((forest.leafCount == 0) == forest.nonemptyShardRoots.isEmpty())
    ) throw SccpErrors.InvalidReplayForest;
}

fun sccpReplayShardRootV1(forest: SccpReplayForestV1, shard: uint8): uint256 {
    sccpReplayValidateForestV1(forest);
    if (!forest.nonemptyShardRoots.exists(shard)) {
        return SCCP_REPLAY_EMPTY_ROOT_V1;
    }
    val root = forest.nonemptyShardRoots.mustGet(shard);
    assert (root != 0 && root != SCCP_REPLAY_EMPTY_ROOT_V1) throw SccpErrors.InvalidReplayForest;
    return root;
}

/// Occupy a fresh leaf or prove exact membership for an identical retry.
fun sccpReplayAdmitV1(
    mutate forest: SccpReplayForestV1,
    domainHash: uint256,
    record: SccpTonReplayRecordV1,
    witnessCell: Cell<SccpReplayWitnessV1>,
): SccpReplayAdmissionV1 {
    assert (domainHash != 0) throw SccpErrors.InvalidReplayDomain;
    val witness = sccpReplayLoadWitnessV1(witnessCell);
    val recordDigest = sccpTonReplayRecordDigestV1(record);
    val key = sccpReplayKeyV1(domainHash, record.replayId);
    val shard = (key >> SCCP_REPLAY_DEPTH_V1) as uint8;
    val currentRoot = sccpReplayShardRootV1(forest, shard);
    assert (witness.expectedShardRoot == currentRoot) throw SccpErrors.StaleReplayWitness;

    if (witness.priorRecordDigest == 0) {
        val (oldRoot, newRoot) = sccpReplayFoldPairV1(
            key,
            SCCP_REPLAY_EMPTY_LEAF_V1,
            sccpReplayOccupiedLeafV1(key, recordDigest),
            witness,
        );
        assert (oldRoot == currentRoot) throw SccpErrors.InvalidReplayPath;
        assert (
            newRoot != 0 && newRoot != oldRoot && forest.leafCount != 0xffffffffffffffff &&
            forest.updateSequence != 0xffffffffffffffff
        ) throw SccpErrors.ReplayCounterExhausted;
        forest.nonemptyShardRoots.set(shard, newRoot);
        forest.leafCount += 1;
        forest.updateSequence += 1;
        return {
            occupiedNow: true,
            domainHash,
            shard,
            key,
            recordDigest,
            oldRoot,
            newRoot,
            leafCount: forest.leafCount,
            updateSequence: forest.updateSequence,
        };
    }

    assert (witness.priorRecordDigest == recordDigest) throw SccpErrors.Replay;
    val membershipRoot = sccpReplayFoldV1(
        key,
        sccpReplayOccupiedLeafV1(key, recordDigest),
        witness,
    );
    assert (membershipRoot == currentRoot) throw SccpErrors.InvalidReplayPath;
    return {
        occupiedNow: false,
        domainHash,
        shard,
        key,
        recordDigest,
        oldRoot: currentRoot,
        newRoot: currentRoot,
        leafCount: forest.leafCount,
        updateSequence: forest.updateSequence,
    };
}

fun sccpReplayVerifyNonMembershipV1(
    forest: SccpReplayForestV1,
    domainHash: uint256,
    replayId: uint256,
    witnessCell: Cell<SccpReplayWitnessV1>,
): bool {
    if (domainHash == 0 || replayId == 0) {
        return false;
    }
    val witness = sccpReplayLoadWitnessV1(witnessCell);
    if (witness.priorRecordDigest != 0) {
        return false;
    }
    val key = sccpReplayKeyV1(domainHash, replayId);
    val root = sccpReplayShardRootV1(forest, (key >> SCCP_REPLAY_DEPTH_V1) as uint8);
    return witness.expectedShardRoot == root &&
    sccpReplayFoldV1(key, SCCP_REPLAY_EMPTY_LEAF_V1, witness) == root;
}

fun sccpReplayVerifyMembershipV1(
    forest: SccpReplayForestV1,
    domainHash: uint256,
    record: SccpTonReplayRecordV1,
    witnessCell: Cell<SccpReplayWitnessV1>,
): bool {
    if (domainHash == 0) {
        return false;
    }
    val witness = sccpReplayLoadWitnessV1(witnessCell);
    val digest = sccpTonReplayRecordDigestV1(record);
    if (witness.priorRecordDigest != digest) {
        return false;
    }
    val key = sccpReplayKeyV1(domainHash, record.replayId);
    val root = sccpReplayShardRootV1(forest, (key >> SCCP_REPLAY_DEPTH_V1) as uint8);
    return witness.expectedShardRoot == root &&
    sccpReplayFoldV1(key, sccpReplayOccupiedLeafV1(key, digest), witness) == root;
}

/// Emit the authenticated delta needed to rebuild off-chain witness indexes.
fun emitSccpReplayDeltaV1(operation: uint8, admission: SccpReplayAdmissionV1) {
    assert (admission.occupiedNow) throw SccpErrors.InvalidReplayForest;
    val event = createExternalLogMessage({
        dest: createAddressNone(),
        body: SccpReplayDeltaV1 {
            operation,
            domainHash: admission.domainHash,
            shard: admission.shard,
            key: admission.key,
            tail: (SccpReplayDeltaTailV1 {
                recordDigest: admission.recordDigest,
                oldRoot: admission.oldRoot,
                newRoot: admission.newRoot,
                leafCount: admission.leafCount,
                updateSequence: admission.updateSequence,
            }).toCell(),
        },
    });
    event.send(SEND_MODE_REGULAR);
}
