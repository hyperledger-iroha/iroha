//! Cross-language SCCP codec, I105 admission, and BLS12-381 signal vectors.

import "@acton/testing/expect"
import "@contracts/constants"
import "@contracts/jetton-utils"
import "@contracts/proof-verifier"
import "@contracts/sccp-codec"
import "@contracts/storage"

fun canonicalRecipientFromSlice(value: slice): cell {
    var input = value;
    if (input.remainingBitsCount() <= SCCP_TAIRA_RECIPIENT_HEAD_BYTES * 8) {
        return beginCell().storeSlice(input).endCell();
    }
    val head = input.loadBits(SCCP_TAIRA_RECIPIENT_HEAD_BYTES * 8);
    val tail = beginCell().storeSlice(input).endCell();
    return beginCell().storeSlice(head).storeRef(tail).endCell();
}

fun validTairaRecipient(): cell {
    return canonicalRecipientFromSlice(
        "testuﾛ1PYﾛ9ｵﾆﾘﾐ3Yf8wﾜｿﾋﾉajｼｱ6eﾑbHｱﾜｶBｳdUｺcヰｲnﾌNP21YC".literalSlice(),
    );
}

fun vectorG1(
    a: int,
    b: int,
    c: int,
    d: int,
    e: int,
    f: int,
    g: int,
    h: int,
): cell {
    return beginCell()
        .storeUint(a, 48)
        .storeUint(b, 48)
        .storeUint(c, 48)
        .storeUint(d, 48)
        .storeUint(e, 48)
        .storeUint(f, 48)
        .storeUint(g, 48)
        .storeUint(h, 48)
        .endCell();
}

fun vectorG2(
    a: int,
    b: int,
    c: int,
    d: int,
    e: int,
    f: int,
    g: int,
    h: int,
): cell {
    return beginCell()
        .storeUint(a, 96)
        .storeUint(b, 96)
        .storeUint(c, 96)
        .storeUint(d, 96)
        .storeUint(e, 96)
        .storeUint(f, 96)
        .storeUint(g, 96)
        .storeUint(h, 96)
        .endCell();
}

fun vectorVerifyingKey(): cell {
    val alpha = vectorG1(
        183011293753374,
        153766608626527,
        266145638431761,
        75466329404921,
        230334637350991,
        89164996524245,
        97502668511874,
        101038412596841,
    );
    val beta = vectorG2(
        46408002589580840726232933963,
        47345780740936842582462518758,
        56525246792141261468662636595,
        33301243340976382265661917167,
        6429032730315346955197275799,
        1992263385287759932813953247,
        65732417395468142527703105096,
        30733837601032853270691364217,
    );
    val gamma = vectorG2(
        45765300665918301128790692768,
        42137484379184671317244818970,
        56280794141317933024253112594,
        6059577009407902906031811454,
        709198854518927808499549479,
        14165060894806320894179293954,
        55805278558791767872231965478,
        52000413785700509085167893944,
    );
    val delta = vectorG2(
        49789121803873806817509672708,
        18726252031417989091448011696,
        74063063928443088317504713506,
        19263010647859157794393177,
        1686241465902126740349150760,
        15941168503749452830055877516,
        5211477145183535490501541634,
        41311053173765659121573254739,
    );
    val ic = vectorG1(
        178265951040853,
        56473986965074,
        106075490475640,
        243422637637155,
        231625369907622,
        96827964420860,
        167399507869996,
        45547050341048,
    );
    var points: map<uint8, cell> = [];
    var index = 0;
    while (index < 12) {
        points.set(index as uint8, ic);
        index += 1;
    }
    val tail: Groth16Bls12381VerificationKeyTail = { deltaG2: delta, ic: points };
    val key: Groth16Bls12381VerificationKey = {
        version: 1,
        publicSignalCount: 11,
        alphaG1: alpha,
        betaG2: beta,
        gammaG2: gamma,
        tail: tail.toCell(),
    };
    return key.toCell();
}

fun pairingVerifyingKey(): cell {
    val seed = Groth16Bls12381VerificationKey.fromCell(vectorVerifyingKey());
    var points: map<uint8, cell> = [];
    var index = 0;
    while (index < 12) {
        points.set(index as uint8, seed.alphaG1);
        index += 1;
    }
    val tail: Groth16Bls12381VerificationKeyTail = { deltaG2: seed.betaG2, ic: points };
    val key: Groth16Bls12381VerificationKey = {
        version: 1,
        publicSignalCount: 11,
        alphaG1: seed.alphaG1,
        betaG2: seed.betaG2,
        gammaG2: seed.betaG2,
        tail: tail.toCell(),
    };
    return key.toCell();
}

fun initialStateFixtureConfig(): Cell<BridgeConfig> {
    val routeTail: RouteHashesTail = { destinationBindingHash: 4, semanticProofProfileHash: 5 };
    val routeHashes: RouteHashes = {
        sourceLaneHash: 1,
        destinationLaneHash: 2,
        routeConfigurationHash: 3,
        tail: routeTail.toCell(),
    };
    val verifyingKey = beginCell().storeUint(12, 8).endCell();
    val guardianTail: MintBreakerGuardiansTail = { guardian3: 4, guardian4: 5 };
    val guardians: MintBreakerGuardians = {
        guardian0: 1,
        guardian1: 2,
        guardian2: 3,
        tail: guardianTail.toCell(),
    };
    val finalityTail: FinalityHashesTail = {
        proofProfileCommitment: 9,
        embeddedVerifierCodeHash: 10,
        verifyingKeyCellHash: verifyingKey.hash(),
        verifyingKey,
        mintBreakerGuardians: guardians.toCell(),
    };
    val finalityHashes: FinalityHashes = {
        soraFinalityAnchorHash: 6,
        verifierCircuitHash: 7,
        verifyingKeyHash: 8,
        tail: finalityTail.toCell(),
    };
    return BridgeConfig {
        expectedGlobalId: -239,
        routeRevision: 1,
        tairaToTonMultiplier: SCCP_TAIRA_TO_TON_MULTIPLIER,
        maxWrappedSupply: 100,
        sourceLaneBytes: beginCell().storeUint(14, 8).endCell(),
        destinationLaneBytes: beginCell().storeUint(1, 8).endCell(),
        routeHashes: routeHashes.toCell(),
        finalityHashes: finalityHashes.toCell(),
    }.toCell();
}

fun positiveDestinationSignals(): cell {
    val tail3: DestinationPublicSignals3 = {
        routeConfigurationHash: signalRouteConfigurationHash(10),
        soraFinalityAnchorHash: signalSoraFinalityAnchorHash(11),
    };
    val tail2: DestinationPublicSignals2 = {
        sourceDomain: signalSourceDomain(SCCP_DOMAIN_SORA),
        statementHash: signalStatementHash(8),
        destinationBindingHash: signalDestinationBindingHash(9),
        next: tail3.toCell(),
    };
    val tail1: DestinationPublicSignals1 = {
        commitmentRoot: 4,
        finalityHeight: 5,
        finalityBlockHash: 6,
        next: tail2.toCell(),
    };
    val head: DestinationPublicSignals0 = {
        messageId: signalMessageId(1),
        payloadHash: signalPayloadHash(2),
        targetDomain: signalTargetDomain(SCCP_DOMAIN_TON),
        next: tail1.toCell(),
    };
    return head.toCell();
}

fun positiveProofForSignals(publicSignalsCell: cell, verifyingKeyCell: cell): cell {
    val signals0 = DestinationPublicSignals0.fromCell(publicSignalsCell);
    val signals1 = signals0.next.load();
    val signals2 = signals1.next.load();
    val signals3 = signals2.next.load();
    val key = Groth16Bls12381VerificationKey.fromCell(verifyingKeyCell);
    val keyTail = key.tail.load();
    val generator = ordinaryPointSlice(key.alphaG1, 384);

    // The fixture key uses the group generators for alpha/beta/gamma/delta and every IC
    // point. A=(3+sum(signals))*G1, B=G2, C=G1 therefore satisfies the four-pair
    // Groth16 equation exactly.
    val first = blsG1MultiExp6(
        ordinaryPointSlice(keyTail.ic.mustGet(0), 384),
        1,
        ordinaryPointSlice(keyTail.ic.mustGet(1), 384),
        signals0.messageId,
        ordinaryPointSlice(keyTail.ic.mustGet(2), 384),
        signals0.payloadHash,
        ordinaryPointSlice(keyTail.ic.mustGet(3), 384),
        signals0.targetDomain,
        ordinaryPointSlice(keyTail.ic.mustGet(4), 384),
        signals1.commitmentRoot,
        ordinaryPointSlice(keyTail.ic.mustGet(5), 384),
        signals1.finalityHeight,
    );
    val second = blsG1MultiExp6(
        ordinaryPointSlice(keyTail.ic.mustGet(6), 384),
        signals1.finalityBlockHash,
        ordinaryPointSlice(keyTail.ic.mustGet(7), 384),
        signals2.sourceDomain,
        ordinaryPointSlice(keyTail.ic.mustGet(8), 384),
        signals2.statementHash,
        ordinaryPointSlice(keyTail.ic.mustGet(9), 384),
        signals2.destinationBindingHash,
        ordinaryPointSlice(keyTail.ic.mustGet(10), 384),
        signals3.routeConfigurationHash,
        ordinaryPointSlice(keyTail.ic.mustGet(11), 384),
        signals3.soraFinalityAnchorHash,
    );
    val vkX = blsG1Add(first, second);
    val proof: Groth16Bls12381Proof = {
        a: beginCell().storeSlice(blsG1Add(blsG1Add(vkX, generator), generator)).endCell(),
        b: key.betaG2,
        c: key.alphaG1,
    };
    return proof.toCell();
}

get fun `test sequential identical rejected deposits have unique refund identities`() {
    val owner = address.fromWorkchainAndHash(
        0,
        0x0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20,
    );
    val first = sccpRefundIdentity(7, 99, owner, 123456789);
    val second = sccpRefundIdentity(8, 99, owner, 123456789);
    expect(first).toNotEqual(second);
    expect(first).toEqual(sccpRefundIdentity(7, 99, owner, 123456789));
    // Invalid bridge payloads must remain refundable even at the largest
    // amount representable by TON's VarUInteger 16 `coins` type.
    expect(sccpRefundIdentity(9, 99, owner, 0xffffffffffffffffffffffffffffff)).toNotEqual(0);
}

get fun `test completed mint replay acknowledges the current verified request`() {
    val record: MasterMintRecord = {
        status: SCCP_COMPLETED,
        queryId: 11,
        bridgeAddress: address.fromWorkchainAndHash(0, 0x1111),
        recipient: address.fromWorkchainAndHash(0, 0x2222),
        jettonAmount: 77,
        payloadHash: 0x3333,
    };
    val ack = sccpMasterMintAckForRequest(record, 0x4444, 22);
    expect(ack.queryId).toEqual(22);
    expect(ack.messageId).toEqual(0x4444);
    expect(ack.recipient).toEqual(record.recipient);
    expect(ack.jettonAmount).toEqual(record.jettonAmount);
    expect(ack.payloadHash).toEqual(record.payloadHash);
}

get fun `test finalize version and retry funding policies fail closed`() {
    expect(isCanonicalSccpMessageVersion(SCCP_MESSAGE_SCHEMA_VERSION)).toEqual(true);
    expect(isCanonicalSccpMessageVersion(SCCP_MESSAGE_SCHEMA_VERSION + 1)).toEqual(false);
    expect(sccpRetryValueIsSufficient(2 * SCCP_OPERATION_VALUE - 1)).toEqual(false);
    expect(sccpRetryValueIsSufficient(2 * SCCP_OPERATION_VALUE)).toEqual(true);
}

get fun `test master mint supply cap admits the boundary and rejects overruns`() {
    expect(sccpMintFitsSupplyCap(90, 10, 100)).toEqual(true);
    expect(sccpMintFitsSupplyCap(90, 11, 100)).toEqual(false);
    expect(sccpMintFitsSupplyCap(100, 1, 100)).toEqual(false);
    expect(sccpMintFitsSupplyCap(101, 1, 100)).toEqual(false);
    expect(sccpMintFitsSupplyCap(0, 0, 0)).toEqual(false);
}

get fun `test transfer funding and recoverable nonce policies fail closed`() {
    expect(sccpTransferRequestValueIsSufficient(SCCP_OPERATION_VALUE, 0)).toEqual(true);
    expect(
        sccpTransferRequestValueIsSufficient(
            4 * SCCP_OPERATION_VALUE - 1,
            2 * SCCP_OPERATION_VALUE,
        ),
    ).toEqual(false);
    expect(
        sccpTransferRequestValueIsSufficient(
            4 * SCCP_OPERATION_VALUE,
            2 * SCCP_OPERATION_VALUE,
        ),
    ).toEqual(true);
    expect(
        sccpInternalTransferValueIsSufficient(
            3 * SCCP_OPERATION_VALUE - 1,
            2 * SCCP_OPERATION_VALUE,
        ),
    ).toEqual(false);
    expect(
        sccpInternalTransferValueIsSufficient(
            3 * SCCP_OPERATION_VALUE,
            2 * SCCP_OPERATION_VALUE,
        ),
    ).toEqual(true);
    expect(sccpOutboundNonceCanAdvance(7, 7)).toEqual(true);
    expect(sccpOutboundNonceCanAdvance(6, 7)).toEqual(false);
    expect(sccpOutboundNonceCanAdvance(8, 7)).toEqual(false);
    expect(sccpOutboundNonceCanAdvance(0xffffffffffffffff, 0xffffffffffffffff)).toEqual(false);
}

get fun `test wallet burn and refund retries bind the first debit tuple`() {
    val bridge = address.fromWorkchainAndHash(
        0,
        0x1111111111111111111111111111111111111111111111111111111111111111,
    );
    val recipient = address.fromWorkchainAndHash(
        0,
        0x2222222222222222222222222222222222222222222222222222222222222222,
    );
    val otherRecipient = address.fromWorkchainAndHash(
        0,
        0x3333333333333333333333333333333333333333333333333333333333333333,
    );
    val burn: WalletBurnRecord = {
        status: SCCP_PENDING_BURN,
        queryId: 7,
        bridgeAddress: bridge,
        jettonAmount: 10,
        payloadHash: 11,
        sourceEventDigest: 12,
    };
    expect(
        sccpWalletBurnRecordMatches(
            burn,
            SCCP_PENDING_BURN,
            7,
            bridge,
            10,
            11,
            12,
        ),
    ).toEqual(true);
    expect(
        sccpWalletBurnRecordMatches(
            burn,
            SCCP_PENDING_BURN,
            7,
            bridge,
            20,
            11,
            12,
        ),
    ).toEqual(false);

    val refund: WalletRefundRecord = {
        status: SCCP_PENDING_BURN,
        queryId: 8,
        bridgeAddress: bridge,
        recipientOwner: recipient,
        jettonAmount: 30,
    };
    expect(
        sccpWalletRefundRecordMatches(
            refund,
            SCCP_PENDING_BURN,
            8,
            bridge,
            recipient,
            30,
        ),
    ).toEqual(true);
    expect(
        sccpWalletRefundRecordMatches(
            refund,
            SCCP_PENDING_BURN,
            8,
            bridge,
            recipient,
            31,
        ),
    ).toEqual(false);
    expect(
        sccpWalletRefundRecordMatches(
            refund,
            SCCP_PENDING_BURN,
            8,
            bridge,
            otherRecipient,
            30,
        ),
    ).toEqual(false);
}

get fun `test destination refund replay key is scoped to authenticated bridge`() {
    val canonicalBridge = address.fromWorkchainAndHash(
        0,
        0x4444444444444444444444444444444444444444444444444444444444444444,
    );
    val attackerBridge = address.fromWorkchainAndHash(
        0,
        0x5555555555555555555555555555555555555555555555555555555555555555,
    );
    val canonical = sccpWalletRefundDestinationReplayId(canonicalBridge, 99);
    expect(canonical).toEqual(sccpWalletRefundDestinationReplayId(canonicalBridge, 99));
    expect(canonical).toNotEqual(sccpWalletRefundDestinationReplayId(attackerBridge, 99));
    expect(sccpMasterBurnReplayId(canonicalBridge, 99)).toNotEqual(
        sccpMasterBurnReplayId(attackerBridge, 99),
    );
    val masterchainOwner = address.fromWorkchainAndHash(
        -1,
        0x4444444444444444444444444444444444444444444444444444444444444444,
    );
    expect(sccpRefundIdentity(0, 7, masterchainOwner, 1)).toNotEqual(
        sccpRefundIdentity(0, 7, canonicalBridge, 1),
    );
}

get fun `test canonical deployment data starts from one empty mutable state`() {
    val config = initialStateFixtureConfig();
    val masterAddress = address.fromWorkchainAndHash(
        0,
        0x7777777777777777777777777777777777777777777777777777777777777777,
    );
    val bridge = canonicalBridgeInitialStorage(config, masterAddress);
    expect(bridge.storageVersion).toEqual(SCCP_STORAGE_VERSION);
    expect(bridge.routeConfigurationHash).toEqual(3);
    expect(bridge.config.hash()).toEqual(config.hash());
    expect(bridge.masterAddress).toEqual(masterAddress);
    expect(bridge.refundSequence).toEqual(0);
    expect(bridge.nonces.exists(masterAddress)).toEqual(false);
    expect(bridge.replay.hash()).toEqual(emptyBridgeReplayState().hash());
    expect(bridge.pending.hash()).toEqual(emptyBridgePendingState().hash());

    val metadata = beginCell().storeUint(0x53434350, 32).endCell();
    val master = canonicalMasterInitialStorage(metadata, config);
    expect(master.storageVersion).toEqual(SCCP_STORAGE_VERSION);
    expect(master.routeConfigurationHash).toEqual(3);
    expect(master.totalSupply).toEqual(0);
    expect(master.metadata.hash()).toEqual(metadata.hash());
    expect(master.bridgeConfig.hash()).toEqual(config.hash());
    expect(master.replay.hash()).toEqual(emptyMasterReplayState().hash());
}

get fun `test canonical Taira I105 recipient passes complete source admission`() {
    expect(isCanonicalTairaRecipient(validTairaRecipient())).toEqual(true);
}

get fun `test I105 checksum network and nonminimal mutations fail closed`() {
    val badChecksum = canonicalRecipientFromSlice(
        "testuﾛ1PYﾛ9ｵﾆﾘﾐ3Yf8wﾜｿﾋﾉajｼｱ6eﾑbHｱﾜｶBｳdUｺcヰｲnﾌNP21YD".literalSlice(),
    );
    val wrongNetwork = canonicalRecipientFromSlice(
        "sorauﾛ1PYﾛ9ｵﾆﾘﾐ3Yf8wﾜｿﾋﾉajｼｱ6eﾑbHｱﾜｶBｳdUｺcヰｲnﾌNP21YC".literalSlice(),
    );
    val nonminimal = canonicalRecipientFromSlice(
        "test1uﾛ1PYﾛ9ｵﾆﾘﾐ3Yf8wﾜｿﾋﾉajｼｱ6eﾑbHｱﾜｶBｳdUｺcヰｲnﾌNP21YC".literalSlice(),
    );
    expect(isCanonicalTairaRecipient(badChecksum)).toEqual(false);
    expect(isCanonicalTairaRecipient(wrongNetwork)).toEqual(false);
    expect(isCanonicalTairaRecipient(nonminimal)).toEqual(false);
}

get fun `test I105 wrong curve and small-order Ed25519 controller fail closed`() {
    // Both literals have valid I105 checksums; only their decoded controller material is invalid.
    val wrongCurve = canonicalRecipientFromSlice(
        "testuﾛ5ﾘwｹｼﾏLqﾎｳｽMﾌwmﾓVｿkZbW4ﾈ7jVcMFﾙcP9ｿrﾙﾏﾍｺLDQ5VR".literalSlice(),
    );
    val smallOrder = canonicalRecipientFromSlice(
        "testuﾛ1NcﾐMkRAﾄMCﾘﾜRnヰSRﾐﾋhﾐdJﾏTｴ7ｼvｾSﾒﾖmﾉ9vﾒﾘDH7VEV".literalSlice(),
    );
    expect(isCanonicalTairaRecipient(wrongCurve)).toEqual(false);
    expect(isCanonicalTairaRecipient(smallOrder)).toEqual(false);
}

get fun `test recipient snake rejects overlength and ambiguous split`() {
    val tooLongTail = beginCell().storeUint(0, 256).storeUint(0, 256).storeUint(0, 40).endCell();
    val tooLong = beginCell()
        .storeUint(0, 256)
        .storeUint(0, 256)
        .storeUint(0, 128)
        .storeRef(tooLongTail)
        .endCell();
    val ambiguous = beginCell()
        .storeUint(0, 256)
        .storeUint(0, 256)
        .storeUint(0, 120)
        .storeRef(beginCell().storeUint(0, 15 * 8).endCell())
        .endCell();
    expect(isCanonicalTairaRecipient(tooLong)).toEqual(false);
    expect(isCanonicalTairaRecipient(ambiguous)).toEqual(false);
}

get fun `test codec seven is signed i32 big endian plus account hash`() {
    val accountHash = 0x0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20;
    val encoded = canonicalTonAddress36(address.fromWorkchainAndHash(0, accountHash));
    val expected = beginCell().storeUint(0, 32).storeUint(accountHash, 256).endCell();
    expect(encoded.beginParse().bitsEqual(expected.beginParse())).toEqual(true);
}

get fun `test canonical four-cell payload and hashes match Rust vectors`() {
    val accountHash = 0x0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20;
    val payload = buildTonToTairaPayload(
        address.fromWorkchainAndHash(0, accountHash),
        validTairaRecipient(),
        9,
        7,
        123456789,
    );
    expect(ordinaryByteLength(payload.header)).toEqual(50);
    expect(ordinaryByteLength(payload.chunk1)).toEqual(100);
    expect(ordinaryByteLength(payload.chunk2)).toEqual(58);
    expect(ordinaryByteLength(payload.chunk3)).toEqual(0);
    expect(canonicalPayloadByteLength(payload)).toEqual(208);
    expect(sccpPayloadHash(payload)).toEqual(
        0xa5b2070d585a783ee4450eb9694cb864fe8cb6a57622c6ec29f15ea168b8afff,
    );
    val lane = beginCell().storeSlice("lane".literalSlice()).endCell();
    expect(sccpLaneHash(lane)).toEqual(
        0x7f3f58178cd1d97f60aa761563b30da931bf8a5284d488b6204198f2e0e9df2f,
    );
    val messageId = sccpMessageId(lane, payload);
    expect(messageId).toEqual(
        0x515d4b69a9303452d5d580c4bca71978a60fd3cb4dc906dff5c414d64117c24d,
    );
    expect(sccpSourceEventDigest(sccpLaneHash(lane), messageId, sccpPayloadHash(payload))).toEqual(
        0xdeb773d913725a4b7557329552b29501ae2de915321d02d59488036b513703ed,
    );
}

get fun `test all eleven BLS12-381 SHA256 signal words match Rust vectors`() {
    expect(signalMessageId(1)).toEqual(
        0x38233bedf9a99fc596a893ac8bf0a54259ec746eaac94496ac6aab46bddf9865,
    );
    expect(signalPayloadHash(2)).toEqual(
        0x50e47a35a9dd26fb214618cb6bc80c9a43ff8d6f2140598cbd64335690a7df4b,
    );
    expect(signalTargetDomain(3)).toEqual(
        0x138c9b3073bb270f5c9276cf11bd8347e651baa6d4f8bafbe05888a4b18cc65f,
    );
    expect(signalCommitmentRoot(4)).toEqual(
        0x48479cbdf01e0f652b7ee77b4d52f2532b8219127ae9164aa0b8ca466fbeea33,
    );
    expect(signalFinalityHeight(5)).toEqual(
        0x4337fd2cc86f4069ddc7d9299295e14ad47df28d07c6e30e51e0ca60a045fbea,
    );
    expect(signalFinalityBlockHash(6)).toEqual(
        0x55f0ebbd28b5a253a0a159c3b02de5231e2b145f49428db98238d5273f0a9537,
    );
    expect(signalSourceDomain(7)).toEqual(
        0x11de378b213429cb66e55361bf87c82db99ba97e9f2a1b7d61b365fe59e472fb,
    );
    expect(signalStatementHash(8)).toEqual(
        0x632e88b4a9f1aac23582295df957ba41e99a0cabbb8c9d7639cec9e0575109b5,
    );
    expect(signalDestinationBindingHash(9)).toEqual(
        0x4ca329795ff859e41e41d4e0ad98fe63a05038f40ddd03f08664ade7483fcc95,
    );
    expect(signalRouteConfigurationHash(10)).toEqual(
        0x57d6ec7115c11cc182f98740ce557084044efb19c13e15b58db6617b5a600aa0,
    );
    expect(signalSoraFinalityAnchorHash(11)).toEqual(
        0x4f59d560e129108ff1283a7682257ba55cdc96082c9b7a2a3955b9c0300c4abb,
    );
}

get fun `test canonical BLS12-381 key SHA256 matches Rust byte commitment`() {
    expect(canonicalBls12381VerifyingKeyHash(vectorVerifyingKey())).toEqual(
        0x7877d1290baec55477c6797800c6f7f8798bf088efb98ad7c6702a7699c3db74,
    );
}

get fun `test BLS12-381 Groth16 pairing accepts valid proof and rejects changed relation`() {
    val verifyingKey = pairingVerifyingKey();
    val publicSignals = positiveDestinationSignals();
    val policy: DestinationProofPolicy = {
        messageId: 1,
        payloadHash: 2,
        statementHash: 8,
        routeConfigurationHash: 10,
        destinationBindingHash: 9,
        semanticProofProfileHash: 12,
        soraFinalityAnchorHash: 11,
        verifyingKeyHash: canonicalBls12381VerifyingKeyHash(verifyingKey),
        verifyingKeyCellHash: verifyingKey.hash(),
    };
    val validProof = positiveProofForSignals(publicSignals, verifyingKey);
    expect(
        verifyTairaDestinationProof(policy, publicSignals, validProof, verifyingKey),
    ).toEqual(true);

    val decoded = Groth16Bls12381Proof.fromCell(validProof);
    val changedProof: Groth16Bls12381Proof = {
        a: decoded.a,
        b: decoded.b,
        c: beginCell()
            .storeSlice(
                blsG1Add(
                    ordinaryPointSlice(decoded.c, 384),
                    ordinaryPointSlice(decoded.c, 384),
                ),
            )
            .endCell(),
    };
    expect(
        verifyTairaDestinationProof(policy, publicSignals, changedProof.toCell(), verifyingKey),
    ).toEqual(false);
}
