//! Final-V1 TON replay-forest golden and canonical-witness regressions.

import "@acton/testing/expect"
import "@contracts/constants"
import "@contracts/errors"
import "@contracts/messages"
import "@contracts/replay-forest"

fun replayTestActor(): address {
    // The all-zero 32-byte account is still a present, fixed-width identity.
    return address.fromWorkchainAndHash(-1, 0);
}

fun replayTestDomain(): uint256 {
    return sccpTonReplayDomainHashV1(
        SCCP_REPLAY_TON_BRIDGE_INBOUND_MINT,
        7,
        0x4444444444444444444444444444444444444444444444444444444444444444,
        replayTestActor(),
    );
}

fun replayTestRecord(): SccpTonReplayRecordV1 {
    return {
        operation: SCCP_REPLAY_TON_BRIDGE_INBOUND_MINT,
        replayId: 0x1111111111111111111111111111111111111111111111111111111111111111,
        payloadSha256: 0x2222222222222222222222222222222222222222222222222222222222222222,
        amountScale9: 9,
        principal: replayTestActor(),
        auxiliaryIdentitySha256: 0x5555555555555555555555555555555555555555555555555555555555555555,
    };
}

fun replayTestEmptyWitness(): SccpReplayWitnessV1 {
    return {
        expectedShardRoot: SCCP_REPLAY_EMPTY_ROOT_V1,
        priorRecordDigest: 0,
        siblingBitmap: 0,
        siblings: null,
    };
}

fun replayTestFoldExitCode(witness: SccpReplayWitnessV1): int {
    try {
        sccpReplayFoldV1(
            sccpReplayKeyV1(replayTestDomain(), replayTestRecord().replayId),
            SCCP_REPLAY_EMPTY_LEAF_V1,
            witness,
        );
        return 0;
    } catch (errorCode) {
        return errorCode;
    }
}

fun replayTestAdmitExitCode(rawWitness: cell): int {
    var forest = emptySccpReplayForestV1().load();
    try {
        sccpReplayAdmitV1(
            mutate forest,
            replayTestDomain(),
            replayTestRecord(),
            rawWitness as Cell<SccpReplayWitnessV1>,
        );
        return forest.leafCount as int;
    } catch (errorCode) {
        return errorCode;
    }
}

get fun `test TON replay hashes match independent final V1 golden`() {
    val actorBytes = sccpReplayTonAddress36(replayTestActor());
    expect(actorBytes.beginParse().loadUint(32)).toEqual(0xffffffff);
    expect(actorBytes.beginParse().remainingBitsCount()).toEqual(36 * 8);
    expect(replayTestDomain()).toEqual(
        0x996ac6a7faa29e3bd41b6af354673b81f582dec2463fe442431e01174abbae87,
    );
    val key = sccpReplayKeyV1(replayTestDomain(), replayTestRecord().replayId);
    expect(key).toEqual(
        0x0c165fb181b2bc7bb6fadda1b51f0985229a3ddcf783ba56db0e691ccd0cf076,
    );
    expect((key >> SCCP_REPLAY_DEPTH_V1) as uint8).toEqual(12);
    val digest = sccpTonReplayRecordDigestV1(replayTestRecord());
    expect(digest).toEqual(
        0x47d14b8f9825981ea24647fdf1a6f9da2e779b32ee2e5705888756572f5c4b73,
    );
    expect(sccpReplayAuxiliaryWordSha256V1(replayTestRecord().auxiliaryIdentitySha256)).toEqual(
        0x84126d0dd850199be29021aadbaee68cb9199047b1cb7ec9894ddb1e3562783c,
    );
    expect(sccpReplayAuxiliaryAddressSha256V1(replayTestActor())).toEqual(
        0x44ceaca0476e8ab026f92789c0f15f0f38ab8c9b0187a4df9cf58379848f70ac,
    );
    expect(
        sccpReplayFoldV1(
            key,
            sccpReplayOccupiedLeafV1(key, digest),
            replayTestEmptyWitness(),
        ),
    ).toEqual(0xae8127b12c22ba814b1303f2b176cd139334c6661869e37a0bb0c2661b12b026);
}

get fun `test canonical payload commitment uses one SHA256 layer`() {
    val payload: CanonicalPayloadCells = {
        header: beginCell()
            .storeUint(0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 256)
            .storeUint(0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 144)
            .endCell(),
        chunk1: beginCell().storeUint(0xbbccdd, 24).endCell(),
        chunk2: beginCell().endCell(),
        chunk3: beginCell().endCell(),
    };
    expect(sccpReplayPayloadSha256V1(payload)).toEqual(
        0x81f6d653370ba3f9e2554c177a91d0a56284de75f8bfcb387bf770790458e83e,
    );
}

get fun `test fresh occupation and identical membership keep counters stable`() {
    var forest = emptySccpReplayForestV1().load();
    val record = replayTestRecord();
    val fresh = sccpReplayAdmitV1(
        mutate forest,
        replayTestDomain(),
        record,
        replayTestEmptyWitness().toCell(),
    );
    expect(fresh.occupiedNow).toEqual(true);
    expect(fresh.leafCount).toEqual(1);
    expect(fresh.updateSequence).toEqual(1);
    expect(fresh.newRoot).toEqual(
        0xae8127b12c22ba814b1303f2b176cd139334c6661869e37a0bb0c2661b12b026,
    );
    val membership: SccpReplayWitnessV1 = {
        expectedShardRoot: fresh.newRoot,
        priorRecordDigest: fresh.recordDigest,
        siblingBitmap: 0,
        siblings: null,
    };
    val retry = sccpReplayAdmitV1(
        mutate forest,
        replayTestDomain(),
        record,
        membership.toCell(),
    );
    expect(retry.occupiedNow).toEqual(false);
    expect(retry.leafCount).toEqual(1);
    expect(retry.updateSequence).toEqual(1);
    expect(
        sccpReplayVerifyMembershipV1(
            forest,
            replayTestDomain(),
            record,
            membership.toCell(),
        ),
    ).toEqual(true);
}

get fun `test witness header and sibling snake reject every alternate shape`() {
    val emptyWitness = replayTestEmptyWitness();
    val trailingBit = beginCell()
        .storeUint(emptyWitness.expectedShardRoot, 256)
        .storeUint(0, 256)
        .storeUint(0, 256)
        .storeUint(0, 1)
        .storeUint(1, 1)
        .endCell();
    expect(replayTestAdmitExitCode(trailingBit)).toEqual(
        SccpErrors.NonCanonicalReplayWitness as int,
    );
    val extraRef = beginCell()
        .storeUint(emptyWitness.expectedShardRoot, 256)
        .storeUint(0, 256)
        .storeUint(0, 256)
        .storeUint(1, 1)
        .storeRef(beginCell().endCell())
        .storeRef(beginCell().endCell())
        .endCell();
    expect(replayTestAdmitExitCode(extraRef)).toEqual(SccpErrors.NonCanonicalReplayWitness as int);
    val missingRef = beginCell()
        .storeUint(emptyWitness.expectedShardRoot, 256)
        .storeUint(0, 256)
        .storeUint(0, 256)
        .storeUint(1, 1)
        .endCell();
    expect(replayTestAdmitExitCode(missingRef)).toEqual(
        SccpErrors.NonCanonicalReplayWitness as int,
    );

    var malformed = emptyWitness;
    malformed.siblingBitmap = 1 << SCCP_REPLAY_DEPTH_V1;
    expect(replayTestFoldExitCode(malformed)).toEqual(
        SccpErrors.NonCanonicalReplayWitness as int,
    );
    malformed.siblingBitmap = 0;
    malformed.siblings = beginCell().storeUint(0, 1).endCell();
    expect(replayTestFoldExitCode(malformed)).toEqual(
        SccpErrors.NonCanonicalReplayWitness as int,
    );
    malformed.siblingBitmap = 1;
    malformed.siblings = beginCell().endCell();
    expect(replayTestFoldExitCode(malformed)).toEqual(
        SccpErrors.NonCanonicalReplayWitness as int,
    );
    malformed.siblings = beginCell().storeUint(SCCP_REPLAY_EMPTY_LEAF_V1, 256).endCell();
    expect(replayTestFoldExitCode(malformed)).toEqual(
        SccpErrors.NonCanonicalReplayWitness as int,
    );

    val splitWrong = beginCell()
        .storeUint(1, 256)
        .storeUint(2, 256)
        .storeRef(beginCell().storeUint(3, 256).storeUint(4, 256).endCell())
        .endCell();
    malformed.siblingBitmap = 0b1111;
    malformed.siblings = splitWrong;
    expect(replayTestFoldExitCode(malformed)).toEqual(
        SccpErrors.NonCanonicalReplayWitness as int,
    );
}

get fun `test swapped leaf-up siblings cannot authenticate the claimed root`() {
    val record = replayTestRecord();
    val domain = replayTestDomain();
    val key = sccpReplayKeyV1(domain, record.replayId);
    val canonicalWitness: SccpReplayWitnessV1 = {
        expectedShardRoot: 1,
        priorRecordDigest: 0,
        siblingBitmap: 0b11,
        siblings: beginCell().storeUint(1, 256).storeUint(2, 256).endCell(),
    };
    val canonicalRoot = sccpReplayFoldV1(key, SCCP_REPLAY_EMPTY_LEAF_V1, canonicalWitness);
    var forest = emptySccpReplayForestV1().load();
    forest.nonemptyShardRoots.set((key >> SCCP_REPLAY_DEPTH_V1) as uint8, canonicalRoot);
    forest.leafCount = 1;
    forest.updateSequence = 1;
    val swapped: SccpReplayWitnessV1 = {
        expectedShardRoot: canonicalRoot,
        priorRecordDigest: 0,
        siblingBitmap: 0b11,
        siblings: beginCell().storeUint(2, 256).storeUint(1, 256).endCell(),
    };
    var errorCode = 0;
    try {
        sccpReplayAdmitV1(mutate forest, domain, record, swapped.toCell());
    } catch (caught) {
        errorCode = caught;
    }
    expect(errorCode).toEqual(SccpErrors.InvalidReplayPath as int);
}

get fun `test TON anycast identity is not a replay actor alias`() {
    // addr_std$10, Some(Anycast), depth=1, one rewrite bit, wc=0, 256-bit account.
    val anycast = beginCell()
        .storeUint(0b101, 3)
        .storeUint(1, 5)
        .storeUint(0, 1)
        .storeInt(0, 8)
        .storeUint(7, 256)
        .endCell()
        .beginParse() as address;
    var errorCode = 0;
    try {
        sccpReplayTonAddress36(anycast);
    } catch (caught) {
        errorCode = caught;
    }
    expect(errorCode).toEqual(SccpErrors.InvalidReplayRecord as int);
}
