//! Cross-language SCCP codec, I105 admission, and BLS12-381 signal vectors.

import "@acton/build"
import "@acton/crypto"
import "@acton/testing/expect"
import "@contracts/constants"
import "@contracts/jetton-utils"
import "@contracts/messages"
import "@contracts/proof-verifier"
import "@contracts/sccp-codec"
import "@contracts/storage"

fun repeatedRouteByte(value: uint8): uint256 {
    var word = 0;
    repeat (32) {
        word = (word << 8) | value;
    }
    return word;
}

fun routeHashGoldenConfig(): BridgeConfig {
    val codes = DeploymentCodeHashes {
        jettonMasterCodeHash: repeatedRouteByte(0x11),
        jettonWalletCodeHash: repeatedRouteByte(0x12),
        routeCodeHash: repeatedRouteByte(0x13),
    }.toCell();
    val guardians = MintBreakerGuardians {
        guardian0: repeatedRouteByte(0x21),
        guardian1: repeatedRouteByte(0x22),
        guardian2: repeatedRouteByte(0x23),
        tail: (MintBreakerGuardiansTail {
            guardian3: repeatedRouteByte(0x24),
            guardian4: repeatedRouteByte(0x25),
        }).toCell(),
    }.toCell();
    val sourceLaneBytes = canonicalSccpTonToSoraLaneBytesV1();
    val destinationLaneBytes = canonicalSccpSoraToTonLaneBytesV1();
    return {
        expectedGlobalId: SCCP_TON_MAINNET_GLOBAL_ID,
        routeRevision: 1,
        tairaToTonMultiplier: 1,
        maxWrappedSupply: 1000000,
        sourceLaneBytes,
        destinationLaneBytes,
        routeHashes: (RouteHashes {
            sourceLaneHash: sccpLaneHash(sourceLaneBytes),
            destinationLaneHash: sccpLaneHash(destinationLaneBytes),
            routeConfigurationHash: 1,
            tail: (RouteHashesTail {
                destinationBindingHash: 1,
                semanticProofProfileHash: repeatedRouteByte(0x18),
                deploymentCodeHashes: codes,
            }).toCell(),
        }).toCell(),
        finalityHashes: (FinalityHashes {
            soraFinalityAnchorHash: repeatedRouteByte(0x19),
            verifierCircuitHash: repeatedRouteByte(0x15),
            verifyingKeyHash: repeatedRouteByte(0x16),
            tail: (FinalityHashesTail {
                proofProfileCommitment: repeatedRouteByte(0x17),
                mintBreakerGuardians: guardians,
                embeddedVerifierCodeHash: repeatedRouteByte(0x14),
                verifyingKeyCellHash: 1,
                verifyingKey: beginCell().endCell(),
                masterMetadata: beginCell().endCell(),
            }).toCell(),
        }).toCell(),
    };
}

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
    val sourceLaneBytes = canonicalSccpTonToSoraLaneBytesV1();
    val destinationLaneBytes = canonicalSccpSoraToTonLaneBytesV1();
    val codeHashes = DeploymentCodeHashes {
        jettonMasterCodeHash: build("TairaXorJettonMaster").hash(),
        jettonWalletCodeHash: build("TairaXorJettonWallet").hash(),
        routeCodeHash: build("TairaXorSccpBridge").hash(),
    }.toCell();
    val routeTail: RouteHashesTail = {
        destinationBindingHash: 1,
        semanticProofProfileHash: 5,
        deploymentCodeHashes: codeHashes,
    };
    val draftRouteHashes: RouteHashes = {
        sourceLaneHash: sccpLaneHash(sourceLaneBytes),
        destinationLaneHash: sccpLaneHash(destinationLaneBytes),
        routeConfigurationHash: 1,
        tail: routeTail.toCell(),
    };
    val verifyingKey = pairingVerifyingKey();
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
        masterMetadata: beginCell().storeUint(0x53434350, 32).endCell(),
    };
    val finalityHashes: FinalityHashes = {
        soraFinalityAnchorHash: 6,
        verifierCircuitHash: 7,
        verifyingKeyHash: canonicalBls12381VerifyingKeyHash(verifyingKey),
        tail: finalityTail.toCell(),
    };
    val draft: BridgeConfig = {
        expectedGlobalId: SCCP_TON_MAINNET_GLOBAL_ID,
        routeRevision: 1,
        tairaToTonMultiplier: SCCP_TAIRA_TO_TON_MULTIPLIER,
        maxWrappedSupply: 1000000000000000,
        sourceLaneBytes,
        destinationLaneBytes,
        routeHashes: draftRouteHashes.toCell(),
        finalityHashes: finalityHashes.toCell(),
    };
    val destinationBindingHash = sccpTonDestinationBindingHashV1(draft);
    val bindingRouteHashes: RouteHashes = {
        sourceLaneHash: draftRouteHashes.sourceLaneHash,
        destinationLaneHash: draftRouteHashes.destinationLaneHash,
        routeConfigurationHash: 1,
        tail: (RouteHashesTail {
            destinationBindingHash,
            semanticProofProfileHash: 5,
            deploymentCodeHashes: codeHashes,
        }).toCell(),
    };
    val bindingDraft: BridgeConfig = {
        expectedGlobalId: draft.expectedGlobalId,
        routeRevision: draft.routeRevision,
        tairaToTonMultiplier: draft.tairaToTonMultiplier,
        maxWrappedSupply: draft.maxWrappedSupply,
        sourceLaneBytes: draft.sourceLaneBytes,
        destinationLaneBytes: draft.destinationLaneBytes,
        routeHashes: bindingRouteHashes.toCell(),
        finalityHashes: draft.finalityHashes,
    };
    val routeConfigurationHash = sccpExactTonRouteConfigurationHashV1(bindingDraft);
    return BridgeConfig {
        expectedGlobalId: bindingDraft.expectedGlobalId,
        routeRevision: bindingDraft.routeRevision,
        tairaToTonMultiplier: bindingDraft.tairaToTonMultiplier,
        maxWrappedSupply: bindingDraft.maxWrappedSupply,
        sourceLaneBytes: bindingDraft.sourceLaneBytes,
        destinationLaneBytes: bindingDraft.destinationLaneBytes,
        routeHashes: (RouteHashes {
            sourceLaneHash: bindingRouteHashes.sourceLaneHash,
            destinationLaneHash: bindingRouteHashes.destinationLaneHash,
            routeConfigurationHash,
            tail: bindingRouteHashes.tail,
        }).toCell(),
        finalityHashes: bindingDraft.finalityHashes,
    }.toCell();
}

fun sortedGuardianKeyPairs(): array<KeyPair> {
    var keys = [
        crypto.seedToKeyPair(1),
        crypto.seedToKeyPair(2),
        crypto.seedToKeyPair(3),
        crypto.seedToKeyPair(4),
        crypto.seedToKeyPair(5),
    ];
    var left = 0;
    while (left < SCCP_MINT_BREAKER_GUARDIAN_COUNT) {
        var right = left + 1;
        while (right < SCCP_MINT_BREAKER_GUARDIAN_COUNT) {
            if (keys.get(right).publicKey < keys.get(left).publicKey) {
                val saved = keys.get(left);
                keys.set(keys.get(right), left);
                keys.set(saved, right);
            }
            right += 1;
        }
        left += 1;
    }
    return keys;
}

fun breakerFixtureConfig(keys: array<KeyPair>): BridgeConfig {
    val guardianTail: MintBreakerGuardiansTail = {
        guardian3: keys.get(3).publicKey,
        guardian4: keys.get(4).publicKey,
    };
    val guardians: MintBreakerGuardians = {
        guardian0: keys.get(0).publicKey,
        guardian1: keys.get(1).publicKey,
        guardian2: keys.get(2).publicKey,
        tail: guardianTail.toCell(),
    };
    val verifyingKey = beginCell().storeUint(1, 8).endCell();
    val finalityTail: FinalityHashesTail = {
        proofProfileCommitment: 9,
        embeddedVerifierCodeHash: 10,
        verifyingKeyCellHash: verifyingKey.hash(),
        verifyingKey,
        mintBreakerGuardians: guardians.toCell(),
        masterMetadata: beginCell().storeUint(0x53434350, 32).endCell(),
    };
    val finality: FinalityHashes = {
        soraFinalityAnchorHash: 6,
        verifierCircuitHash: 7,
        verifyingKeyHash: 8,
        tail: finalityTail.toCell(),
    };
    val routeTail: RouteHashesTail = {
        destinationBindingHash: 4,
        semanticProofProfileHash: 5,
        deploymentCodeHashes: (DeploymentCodeHashes {
            jettonMasterCodeHash: 11,
            jettonWalletCodeHash: 12,
            routeCodeHash: 13,
        }).toCell(),
    };
    val hashes: RouteHashes = {
        sourceLaneHash: 1,
        destinationLaneHash: 2,
        routeConfigurationHash: 3,
        tail: routeTail.toCell(),
    };
    return {
        expectedGlobalId: -239,
        routeRevision: 1,
        tairaToTonMultiplier: SCCP_TAIRA_TO_TON_MULTIPLIER,
        maxWrappedSupply: 1000000,
        sourceLaneBytes: beginCell().storeUint(1, 8).endCell(),
        destinationLaneBytes: beginCell().storeUint(2, 8).endCell(),
        routeHashes: hashes.toCell(),
        finalityHashes: finality.toCell(),
    };
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

get fun `test completed mint replay acknowledges the current verified request`() {
    val record: MasterMintRecord = {
        queryId: 11,
        bridgeAddress: address.fromWorkchainAndHash(0, 0x1111),
        recipient: address.fromWorkchainAndHash(0, 0x2222),
        jettonAmount: 77,
        tail: (MasterMintRecordTail {
            payloadHash: 0x3333,
            payloadSha256: 0x5555,
        }).toCell(),
    };
    val ack = sccpMasterMintAckForRequest(record, 0x4444, 22);
    expect(ack.queryId).toEqual(22);
    expect(ack.messageId).toEqual(0x4444);
    expect(ack.recipient).toEqual(record.recipient);
    expect(ack.jettonAmount).toEqual(record.jettonAmount);
    expect(ack.payloadHash).toEqual(record.tail.load().payloadHash);
}

get fun `test finalize version and retry funding policies fail closed`() {
    expect(isCanonicalSccpMessageVersion(SCCP_MESSAGE_SCHEMA_VERSION)).toEqual(true);
    expect(isCanonicalSccpMessageVersion(SCCP_MESSAGE_SCHEMA_VERSION + 1)).toEqual(false);
    expect(sccpRetryValueIsSufficient(2 * SCCP_OPERATION_VALUE - 1)).toEqual(false);
    expect(sccpRetryValueIsSufficient(2 * SCCP_OPERATION_VALUE)).toEqual(true);
}

get fun `test pre-message reserves and bounded excesses preserve old balance`() {
    expect(sccpPreMessageReserveAmount(1000, 400, 100)).toEqual(600);
    expect(sccpPreMessageReserveAmount(450, 400, 100)).toEqual(100);
    expect(
        sccpBoundedInboundExcess(5 * SCCP_OPERATION_VALUE, 2 * SCCP_OPERATION_VALUE),
    ).toEqual(2 * SCCP_OPERATION_VALUE);
    expect(sccpBoundedInboundExcess(SCCP_OPERATION_VALUE, 0)).toEqual(0);
    expect(sccpPendingCapacityAvailable(SCCP_PENDING_OPERATION_CAP - 1)).toEqual(true);
    expect(sccpPendingCapacityAvailable(SCCP_PENDING_OPERATION_CAP)).toEqual(false);
    expect(sccpMintFitsSupplyCap(90, 10, 100)).toEqual(true);
    expect(sccpMintFitsSupplyCap(90, 11, 100)).toEqual(false);
    expect(sccpMintFitsSupplyCap(100, 1, 100)).toEqual(false);
    expect(sccpMintFitsSupplyCap(101, 1, 100)).toEqual(false);
    expect(sccpMintFitsSupplyCap(0, 0, 100)).toEqual(false);
    expect(sccpMintFitsSupplyCap(0, 1, 0)).toEqual(false);
}

@test.fail_with(86)
get fun `test pre-message reserve rejects impossible subtraction`() {
    sccpPreMessageReserveAmount(99, 100, 1);
}

get fun `test mint breaker is target-bound fixed-slot three-of-five`() {
    val keys = sortedGuardianKeyPairs();
    val config = breakerFixtureConfig(keys);
    val target = address.fromWorkchainAndHash(0, 0x1234);
    val otherTarget = address.fromWorkchainAndHash(0, 0x5678);
    val authorization = sccpMintBreakerAuthorization(config, target);
    val signature0 = beginCell()
        .storeSlice(crypto.sign(keys.get(0).privateKey, authorization))
        .endCell();
    val signature1 = beginCell()
        .storeSlice(crypto.sign(keys.get(1).privateKey, authorization))
        .endCell();
    val signature2 = beginCell()
        .storeSlice(crypto.sign(keys.get(2).privateKey, authorization))
        .endCell();
    val valid: SccpDisableMinting = {
        signature0,
        signature1,
        tail: (SccpDisableMintingTail {
            signature2,
            signature3: null,
            signature4: null,
        }).toCell(),
    };
    expect(verifyMintBreakerAuthorization(config, target, valid)).toEqual(true);
    expect(verifyMintBreakerAuthorization(config, otherTarget, valid)).toEqual(false);
    val onlyTwo: SccpDisableMinting = {
        signature0,
        signature1,
        tail: (SccpDisableMintingTail {
            signature2: null,
            signature3: null,
            signature4: null,
        }).toCell(),
    };
    expect(verifyMintBreakerAuthorization(config, target, onlyTwo)).toEqual(false);
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

get fun `test wallet burn retries bind the first debit tuple`() {
    val bridge = address.fromWorkchainAndHash(
        0,
        0x1111111111111111111111111111111111111111111111111111111111111111,
    );
    val recipient = address.fromWorkchainAndHash(
        0,
        0x2222222222222222222222222222222222222222222222222222222222222222,
    );
    val burn: WalletBurnRecord = {
        lockSequence: 5,
        queryId: 7,
        outboundNonce: 6,
        bridgeAddress: bridge,
        ownerAddress: recipient,
        jettonAmount: 10,
        tail: (WalletBurnRecordTail {
            payloadHash: 11,
            payloadSha256: 13,
            sourceEventDigest: 12,
        }).toCell(),
    };
    expect(
        sccpWalletBurnRecordMatches(
            burn,
            5,
            7,
            6,
            bridge,
            recipient,
            10,
            11,
            12,
        ),
    ).toEqual(true);
    expect(
        sccpWalletBurnRecordMatches(
            burn,
            5,
            7,
            6,
            bridge,
            recipient,
            20,
            11,
            12,
        ),
    ).toEqual(false);
}

get fun `test master burn replay key is scoped to authenticated bridge`() {
    val canonicalBridge = address.fromWorkchainAndHash(
        0,
        0x4444444444444444444444444444444444444444444444444444444444444444,
    );
    val attackerBridge = address.fromWorkchainAndHash(
        0,
        0x5555555555555555555555555555555555555555555555555555555555555555,
    );
    expect(sccpMasterBurnReplayId(canonicalBridge, 99)).toNotEqual(
        sccpMasterBurnReplayId(attackerBridge, 99),
    );
}

get fun `test TON mainnet lanes and guardian ordered route hashes match final V1 goldens`() {
    val config = routeHashGoldenConfig();
    val hashes = config.routeHashes.load();
    expect(hashes.sourceLaneHash).toEqual(
        0x53c771f37951401d26513e2519cf46b93c582f75529d3949a6c548217ec87b55,
    );
    expect(hashes.destinationLaneHash).toEqual(
        0x4876fd8bf67c75bbe3e96c884e82a8e794a2bd277f2f206cb19b96bedbf19bc0,
    );
    val binding = sccpTonDestinationBindingHashV1(config);
    expect(binding).toEqual(
        0x001c0af2bc2892344521e240b75cc136d7d497cb686722ffe932b4438266820c,
    );
    expect(sccpTonDeploymentConfigHashV1(config, binding)).toEqual(
        0x59259105963c49d671793b36c35539ce0f9b5e819ec8a0e6043fa00750f9b708,
    );
    expect(sccpTonAssetRouteConfigHashV1(config)).toEqual(
        0xed33173402e1a7de23b3052e2c2b22e876c359bae484127af8ee9773d4d78af6,
    );
    expect(sccpExactTonRouteConfigurationHashV1(config)).toEqual(
        0x795191a9fef7b24355e19f9265bcd0b36c3ea18833f1e87a59395d493422a3a4,
    );
}

get fun `test canonical deployment data starts from one empty mutable state`() {
    val config = initialStateFixtureConfig();
    val bridgeAddress = address.fromWorkchainAndHash(
        0,
        0x7777777777777777777777777777777777777777777777777777777777777777,
    );
    val bridge = canonicalBridgeInitialStorage(config);
    val loadedConfig = config.load();
    val routeHashes = loadedConfig.routeHashes.load();
    expect(bridge.storageVersion).toEqual(SCCP_STORAGE_VERSION);
    expect(bridge.routeConfigurationHash).toEqual(routeHashes.routeConfigurationHash);
    expect(routeHashes.tail.load().destinationBindingHash).toEqual(
        sccpTonDestinationBindingHashV1(loadedConfig),
    );
    expect(routeHashes.routeConfigurationHash).toEqual(
        sccpExactTonRouteConfigurationHashV1(loadedConfig),
    );
    expect(loadedConfig.sourceLaneBytes.hash()).toEqual(canonicalSccpTonToSoraLaneBytesV1().hash());
    expect(loadedConfig.destinationLaneBytes.hash()).toEqual(
        canonicalSccpSoraToTonLaneBytesV1().hash(),
    );
    expect(bridge.config.hash()).toEqual(config.hash());
    expect(bridge.bridgeConfigCellHash).toEqual(config.hash());
    expect(bridge.mintingDisabled).toEqual(false);
    expect(bridge.replay.hash()).toEqual(emptyBridgeReplayState().hash());
    expect(bridge.pending.hash()).toEqual(emptyBridgePendingState().hash());

    val master = canonicalMasterInitialStorage(config, bridgeAddress);
    expect(master.storageVersion).toEqual(SCCP_STORAGE_VERSION);
    expect(master.routeConfigurationHash).toEqual(routeHashes.routeConfigurationHash);
    expect(master.bridgeConfigCellHash).toEqual(config.hash());
    expect(master.totalSupply).toEqual(0);
    expect(master.metadata.hash()).toEqual(
        config.load().finalityHashes.load().tail.load().masterMetadata.hash(),
    );
    expect(master.bridgeConfig.hash()).toEqual(config.hash());
    expect(master.bridgeAddress).toEqual(bridgeAddress);
    expect(master.replay.hash()).toEqual(emptyMasterReplayState().hash());
    expect(master.pendingMintCount).toEqual(0);
    expect(master.mintingDisabled).toEqual(false);
    expect(loadedConfig.maxWrappedSupply).toEqual(1000000000000000);
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
        0x76070726615ac0915b69bf2d94018ba5b8335f3b26aa86e49606053067597e5e,
    );
    expect(sccpPayloadHashBlockCountV1(payload)).toEqual(2);
    var incremental = sccpPayloadHashBlockV1(payload, sccpBlake2bInitialStateV1(), 0);
    incremental = sccpPayloadHashBlockV1(payload, incremental, 1);
    expect(sccpPayloadHashFromStateV1(incremental)).toEqual(
        0x76070726615ac0915b69bf2d94018ba5b8335f3b26aa86e49606053067597e5e,
    );
    val lane = beginCell().storeSlice("lane".literalSlice()).endCell();
    expect(sccpLaneHash(lane)).toEqual(
        0x7f3f58178cd1d97f60aa761563b30da931bf8a5284d488b6204198f2e0e9df2f,
    );
    val messageId = sccpMessageId(lane, payload);
    expect(messageId).toEqual(
        0x83efac3e640cd3319da756d4c90ebb358260bd495de76349f1724675d4f7f641,
    );
    expect(sccpSourceEventDigest(sccpLaneHash(lane), messageId, sccpPayloadHash(payload))).toEqual(
        0x3554d5545dd97062e3c34779c723c271a07f8db5052ba2c967265c389e5ec277,
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
