//! Emulator regressions for the direct-wallet SCCP mint, burn, and refund state machines.

import "@acton/build"
import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@contracts/constants"
import "@contracts/errors"
import "@contracts/jetton-utils"
import "@contracts/messages"
import "@contracts/proof-verifier"
import "@contracts/replay-forest"
import "@contracts/sccp-codec"
import "@contracts/storage"
import "@wrappers/TairaXorJettonMaster.gen"
import "@wrappers/TairaXorJettonWallet.gen"
import "@wrappers/TairaXorSccpBridge.gen"

fun securityEmptyReplayWitness(): SccpReplayWitnessV1 {
    return {
        expectedShardRoot: SCCP_REPLAY_EMPTY_ROOT_V1,
        priorRecordDigest: 0,
        siblingBitmap: 0,
        siblings: null,
    };
}

fun securityBurnWitnesses(bridgeWitness: SccpReplayWitnessV1): SccpBurnReplayWitnessesV1 {
    val empty = securityEmptyReplayWitness();
    return {
        lockDebit: empty.toCell(),
        bridge: bridgeWitness.toCell(),
        walletAuthorization: empty.toCell(),
        tail: (SccpBurnReplayWitnessesTailV1 { master: empty.toCell() }).toCell(),
    };
}

fun securityCanonicalRecipientFromSlice(value: slice): cell {
    var input = value;
    if (input.remainingBitsCount() <= SCCP_TAIRA_RECIPIENT_HEAD_BYTES * 8) {
        return beginCell().storeSlice(input).endCell();
    }
    val head = input.loadBits(SCCP_TAIRA_RECIPIENT_HEAD_BYTES * 8);
    val tail = beginCell().storeSlice(input).endCell();
    return beginCell().storeSlice(head).storeRef(tail).endCell();
}

fun securityValidTairaRecipient(): cell {
    return securityCanonicalRecipientFromSlice(
        "testuﾛ1PYﾛ9ｵﾆﾘﾐ3Yf8wﾜｿﾋﾉajｼｱ6eﾑbHｱﾜｶBｳdUｺcヰｲnﾌNP21YC".literalSlice(),
    );
}

fun securityG1(): cell {
    return beginCell()
        .storeUint(183011293753374, 48)
        .storeUint(153766608626527, 48)
        .storeUint(266145638431761, 48)
        .storeUint(75466329404921, 48)
        .storeUint(230334637350991, 48)
        .storeUint(89164996524245, 48)
        .storeUint(97502668511874, 48)
        .storeUint(101038412596841, 48)
        .endCell();
}

fun securityG2(): cell {
    return beginCell()
        .storeUint(46408002589580840726232933963, 96)
        .storeUint(47345780740936842582462518758, 96)
        .storeUint(56525246792141261468662636595, 96)
        .storeUint(33301243340976382265661917167, 96)
        .storeUint(6429032730315346955197275799, 96)
        .storeUint(1992263385287759932813953247, 96)
        .storeUint(65732417395468142527703105096, 96)
        .storeUint(30733837601032853270691364217, 96)
        .endCell();
}

fun securityVerifyingKey(): cell {
    val g1 = securityG1();
    val g2 = securityG2();
    var points: map<uint8, cell> = [];
    var index = 0;
    while (index < SCCP_GROTH16_VK_IC_COUNT) {
        points.set(index as uint8, g1);
        index += 1;
    }
    return Groth16Bls12381VerificationKey {
        version: 1,
        publicSignalCount: SCCP_GROTH16_PUBLIC_SIGNAL_COUNT,
        alphaG1: g1,
        betaG2: g2,
        gammaG2: g2,
        tail: (Groth16Bls12381VerificationKeyTail {
            deltaG2: g2,
            ic: points,
        }).toCell(),
    }.toCell();
}

fun securityGuardians(a: uint256 = 1, b: uint256 = 2): Cell<MintBreakerGuardians> {
    return MintBreakerGuardians {
        guardian0: a,
        guardian1: b,
        guardian2: 3,
        tail: (MintBreakerGuardiansTail {
            guardian3: 4,
            guardian4: 5,
        }).toCell(),
    }.toCell();
}

fun securityBridgeConfigWithGuardians(
    guardians: Cell<MintBreakerGuardians>,
    cap: coins = 1000000000000000,
): Cell<BridgeConfig> {
    val sourceLane = canonicalSccpTonToSoraLaneBytesV1();
    val destinationLane = canonicalSccpSoraToTonLaneBytesV1();
    val verifyingKey = securityVerifyingKey();
    val codeHashes = DeploymentCodeHashes {
        jettonMasterCodeHash: build("TairaXorJettonMaster").hash(),
        jettonWalletCodeHash: build("TairaXorJettonWallet").hash(),
        routeCodeHash: build("TairaXorSccpBridge").hash(),
    }.toCell();
    val draftRouteHashes: RouteHashes = {
        sourceLaneHash: sccpLaneHash(sourceLane),
        destinationLaneHash: sccpLaneHash(destinationLane),
        routeConfigurationHash: 1,
        tail: (RouteHashesTail {
            destinationBindingHash: 1,
            semanticProofProfileHash: 5,
            deploymentCodeHashes: codeHashes,
        }).toCell(),
    };
    val finalityHashes: FinalityHashes = {
        soraFinalityAnchorHash: 6,
        verifierCircuitHash: 7,
        verifyingKeyHash: canonicalBls12381VerifyingKeyHash(verifyingKey),
        tail: (FinalityHashesTail {
            proofProfileCommitment: 9,
            mintBreakerGuardians: guardians,
            embeddedVerifierCodeHash: 10,
            verifyingKeyCellHash: verifyingKey.hash(),
            verifyingKey,
            masterMetadata: OnchainMetadataReply { contentDict: [] }.toCell(),
        }).toCell(),
    };
    val draft: BridgeConfig = {
        expectedGlobalId: SCCP_TON_MAINNET_GLOBAL_ID,
        routeRevision: 1,
        tairaToTonMultiplier: SCCP_TAIRA_TO_TON_MULTIPLIER,
        maxWrappedSupply: cap,
        sourceLaneBytes: sourceLane,
        destinationLaneBytes: destinationLane,
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

fun securityBridgeConfig(): Cell<BridgeConfig> {
    return securityBridgeConfigWithGuardians(securityGuardians());
}

fun securityTairaToTonPayload(recipient: address, amount: uint128): CanonicalPayloadCells {
    val sender = validateTairaRecipient(securityValidTairaRecipient());
    val header = beginCell()
        .storeUint(SCCP_PAYLOAD_DISCRIMINATOR, 8)
        .storeUint(SCCP_PAYLOAD_VERSION, 8)
        .storeUint32Le(SCCP_DOMAIN_SORA)
        .storeUint32Le(SCCP_DOMAIN_TON)
        .storeUint64Le(7)
        .storeUint32Le(1)
        .storeUint32Le(SCCP_DOMAIN_SORA)
        .storeUint(SCCP_CODEC_CANONICAL_TEXT, 8)
        .storeUint32Le(3)
        .storeSlice("xor".literalSlice())
        .storeUint128Le(amount)
        .endCell();
    var remainder: array<int> = [];
    remainder.push(SCCP_CODEC_CANONICAL_TEXT);
    arrayAppendUint32Le(mutate remainder, sender.size() as uint32);
    var index = 0;
    while (index < sender.size()) {
        remainder.push(sender.get(index));
        index += 1;
    }
    remainder.push(SCCP_CODEC_TON_ADDRESS36);
    arrayAppendUint32Le(mutate remainder, 36);
    appendSliceBytes(mutate remainder, canonicalTonAddress36(recipient).beginParse());
    remainder.push(SCCP_CODEC_CANONICAL_TEXT);
    arrayAppendUint32Le(mutate remainder, 13);
    appendSliceBytes(mutate remainder, "taira_ton_xor".literalSlice());
    val firstLength = remainder.size() > SCCP_CANONICAL_CHUNK_BYTES
        ? SCCP_CANONICAL_CHUNK_BYTES
        : remainder.size();
    val remaining = remainder.size() - firstLength;
    val secondLength = remaining > SCCP_CANONICAL_CHUNK_BYTES
        ? SCCP_CANONICAL_CHUNK_BYTES
        : remaining;
    return {
        header,
        chunk1: cellFromByteRange(remainder, 0, firstLength),
        chunk2: cellFromByteRange(remainder, firstLength, secondLength),
        chunk3: cellFromByteRange(
            remainder,
            firstLength + secondLength,
            remaining - secondLength,
        ),
    };
}

fun securitySignals(
    messageId: uint256,
    payloadHash: uint256,
    statementHash: uint256,
    config: BridgeConfig,
): cell {
    val hashes = config.routeHashes.load();
    val tail3: DestinationPublicSignals3 = {
        routeConfigurationHash: signalRouteConfigurationHash(hashes.routeConfigurationHash),
        soraFinalityAnchorHash: signalSoraFinalityAnchorHash(
            config.finalityHashes.load().soraFinalityAnchorHash,
        ),
    };
    val tail2: DestinationPublicSignals2 = {
        sourceDomain: signalSourceDomain(SCCP_DOMAIN_SORA),
        statementHash: signalStatementHash(statementHash),
        destinationBindingHash: signalDestinationBindingHash(
            hashes.tail.load().destinationBindingHash,
        ),
        next: tail3.toCell(),
    };
    val tail1: DestinationPublicSignals1 = {
        commitmentRoot: 4,
        finalityHeight: 5,
        finalityBlockHash: 6,
        next: tail2.toCell(),
    };
    return DestinationPublicSignals0 {
        messageId: signalMessageId(messageId),
        payloadHash: signalPayloadHash(payloadHash),
        targetDomain: signalTargetDomain(SCCP_DOMAIN_TON),
        next: tail1.toCell(),
    }.toCell();
}

fun securityProof(publicSignalsCell: cell, verifyingKeyCell: cell): cell {
    val signals0 = DestinationPublicSignals0.fromCell(publicSignalsCell);
    val signals1 = signals0.next.load();
    val signals2 = signals1.next.load();
    val signals3 = signals2.next.load();
    val key = Groth16Bls12381VerificationKey.fromCell(verifyingKeyCell);
    val keyTail = key.tail.load();
    val generator = ordinaryPointSlice(key.alphaG1, 384);
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
    return Groth16Bls12381Proof {
        a: beginCell().storeSlice(blsG1Add(blsG1Add(vkX, generator), generator)).endCell(),
        b: key.betaG2,
        c: key.alphaG1,
    }.toCell();
}

fun securityDeployBridge(bridge: TairaXorSccpBridge, from: address): SendResultList {
    return net.send(
        from,
        createMessage({
            bounce: BounceMode.NoBounce,
            value: ton("20"),
            dest: { stateInit: bridge.stateInit! },
            body: TopUpTons {},
        }),
    );
}

fun securityDeployMaster(master: TairaXorJettonMaster, from: address): SendResultList {
    return net.send(
        from,
        createMessage({
            bounce: BounceMode.NoBounce,
            value: ton("20"),
            dest: { stateInit: master.stateInit! },
            body: TopUpTons {},
        }),
    );
}

fun securityMint(
    bridge: TairaXorSccpBridge,
    relayer: address,
    recipient: address,
    amount: uint128,
): SendResultList {
    val config = securityBridgeConfig().load();
    val payload = securityTairaToTonPayload(recipient, amount);
    val payloadHash = sccpPayloadHash(payload);
    val messageId = sccpMessageId(config.destinationLaneBytes, payload);
    val statementHash = 8;
    val signals = securitySignals(messageId, payloadHash, statementHash, config);
    val verifyingKey = config.finalityHashes.load().tail.load().verifyingKey;
    val empty = securityEmptyReplayWitness();
    return bridge.sendSccpFinalizeFromTaira(
        relayer,
        11,
        SCCP_MESSAGE_SCHEMA_VERSION,
        messageId,
        statementHash,
        signals,
        securityProof(signals, verifyingKey),
        payload.toCell(),
        {
            bridge: empty.toCell(),
            master: empty.toCell(),
            wallet: empty.toCell(),
        },
        { value: sccpBridgeFinalizeMinimum() },
    );
}

get fun `test canonical bridge derives and authenticates the sole master`() {
    val deployer = testing.treasury("sccp-derived-master");
    val config = securityBridgeConfig();
    val bridge = TairaXorSccpBridge.fromStorage(canonicalBridgeInitialStorage(config));
    val master = TairaXorJettonMaster.fromStorage(
        canonicalMasterInitialStorage(config, bridge.address),
    );
    expect(securityDeployBridge(bridge, deployer.address)).toHaveSuccessfulDeploy({
        to: bridge.address,
    });
    expect(securityDeployMaster(master, deployer.address)).toHaveSuccessfulDeploy({
        to: master.address,
    });
    expect(bridge.getSccpMasterAddress()).toEqual(master.address);
    expect(master.getSccpBridgeAddress()).toEqual(bridge.address);
    val (masterCodeHash, masterDataHash) = bridge.getSccpMasterInitialStateHashes();
    val (readCodeHash, readDataHash) = master.getSccpInitialStateHashes();
    expect(masterCodeHash).toEqual(readCodeHash);
    expect(masterDataHash).toEqual(readDataHash);
    expect(bridge.getSccpBridgeConfigCellHash()).toEqual(config.hash());
    expect(master.getSccpBridgeConfigCellHash()).toEqual(config.hash());
    val loadedConfig = config.load();
    val destinationBindingHash = loadedConfig.routeHashes.load().tail.load().destinationBindingHash;
    expect(bridge.getSccpDestinationBindingHash()).toEqual(destinationBindingHash);
    expect(master.getSccpDestinationBindingHash()).toEqual(destinationBindingHash);
    val expectedCodes = loadedConfig.routeHashes.load().tail.load().deploymentCodeHashes.load();
    val (
        bridgeMasterCode,
        bridgeWalletCode,
        bridgeRouteCode,
    ) = bridge.getSccpDeploymentCodeHashes();
    val (
        masterMasterCode,
        masterWalletCode,
        masterRouteCode,
    ) = master.getSccpDeploymentCodeHashes();
    expect(bridgeMasterCode).toEqual(expectedCodes.jettonMasterCodeHash);
    expect(bridgeWalletCode).toEqual(expectedCodes.jettonWalletCodeHash);
    expect(bridgeRouteCode).toEqual(expectedCodes.routeCodeHash);
    expect(masterMasterCode).toEqual(bridgeMasterCode);
    expect(masterWalletCode).toEqual(bridgeWalletCode);
    expect(masterRouteCode).toEqual(bridgeRouteCode);
    val guardians = loadedConfig.finalityHashes.load().tail.load().mintBreakerGuardians.load();
    val guardianTail = guardians.tail.load();
    val (bg0, bg1, bg2, bg3, bg4) = bridge.getSccpMintBreakerGuardians();
    val (mg0, mg1, mg2, mg3, mg4) = master.getSccpMintBreakerGuardians();
    expect(bg0).toEqual(guardians.guardian0);
    expect(bg1).toEqual(guardians.guardian1);
    expect(bg2).toEqual(guardians.guardian2);
    expect(bg3).toEqual(guardianTail.guardian3);
    expect(bg4).toEqual(guardianTail.guardian4);
    expect(mg0).toEqual(bg0);
    expect(mg1).toEqual(bg1);
    expect(mg2).toEqual(bg2);
    expect(mg3).toEqual(bg3);
    expect(mg4).toEqual(bg4);
    expect(master.getJettonData().totalSupply).toEqual(0);
    expect(master.getSccpPendingMintCount()).toEqual(0);
    val (bridgeMints, bridgeBurns) = bridge.getSccpPendingCounts();
    expect(bridgeMints).toEqual(0);
    expect(bridgeBurns).toEqual(0);
}

get fun `test funded bridge mint and direct wallet burn complete every replay boundary`() {
    val deployer = testing.treasury("sccp-funded-flow");
    val owner = testing.treasury("sccp-funded-owner");
    val config = securityBridgeConfig();
    val bridge = TairaXorSccpBridge.fromStorage(canonicalBridgeInitialStorage(config));
    val master = TairaXorJettonMaster.fromStorage(
        canonicalMasterInitialStorage(config, bridge.address),
    );
    expect(securityDeployBridge(bridge, deployer.address)).toHaveSuccessfulDeploy({
        to: bridge.address,
    });
    expect(securityDeployMaster(master, deployer.address)).toHaveSuccessfulDeploy({
        to: master.address,
    });

    val mint = securityMint(bridge, deployer.address, owner.address, 100);
    expect(mint).toHaveSuccessfulTx<SccpFinalizeFromTaira>({ to: bridge.address });
    expect(mint).toHaveSuccessfulTx<SccpMasterMint>({ to: master.address });
    val wallet = TairaXorJettonWallet.fromAddress(master.getWalletAddress(owner.address));
    expect(mint).toHaveSuccessfulDeploy({ to: wallet.address });
    expect(mint).toHaveSuccessfulTx<SccpWalletMint>({ to: wallet.address });
    expect(mint).toHaveSuccessfulTx<SccpWalletMintAck>({ to: master.address });
    expect(mint).toHaveSuccessfulTx<SccpMasterMintAck>({ to: bridge.address });
    expect(wallet.getWalletData().jettonBalance).toEqual(100);
    expect(master.getJettonData().totalSupply).toEqual(100);

    val burn = wallet.sendSccpWalletPrepareBurn(
        owner.address,
        12,
        0,
        100,
        securityValidTairaRecipient(),
        securityBurnWitnesses(securityEmptyReplayWitness()),
        { value: sccpWalletBurnPrepareMinimum() },
    );
    expect(burn).toHaveSuccessfulTx<SccpWalletPrepareBurn>({ to: wallet.address });
    expect(burn).toHaveSuccessfulTx<SccpBridgeBurnPrepare>({ to: bridge.address });
    expect(burn).toHaveSuccessfulTx<SccpWalletBurn>({ to: wallet.address });
    expect(burn).toHaveSuccessfulTx<SccpMasterBurn>({ to: master.address });
    expect(burn).toHaveSuccessfulTx<SccpMasterBurnAck>({ to: wallet.address });
    expect(burn).toHaveSuccessfulTx<SccpMasterBurnAck>({ to: bridge.address });
    expect(wallet.getWalletData().jettonBalance).toEqual(0);
    expect(master.getJettonData().totalSupply).toEqual(0);
    val (nonce, nextLock, locked) = wallet.getSccpOutboundState();
    expect(nonce).toEqual(1);
    expect(nextLock).toEqual(1);
    expect(locked).toEqual(0);
    val (walletPending, walletLocks) = wallet.getSccpPendingCounts();
    expect(walletPending).toEqual(0);
    expect(walletLocks).toEqual(0);
    val (_, _, lockLeaves, _) = wallet.getSccpReplayState(SCCP_REPLAY_TON_WALLET_BURN_LOCK, 0);
    val (_, _, authLeaves, _) = wallet.getSccpReplayState(
        SCCP_REPLAY_TON_WALLET_BURN_AUTHORIZATION,
        0,
    );
    val (_, _, bridgeLeaves, _) = bridge.getSccpReplayState(
        SCCP_REPLAY_TON_BRIDGE_OUTBOUND_BURN,
        0,
    );
    val (_, _, masterLeaves, _) = master.getSccpReplayState(SCCP_REPLAY_TON_MASTER_BURN, 0);
    expect(lockLeaves).toEqual(1);
    expect(authLeaves).toEqual(1);
    expect(bridgeLeaves).toEqual(1);
    expect(masterLeaves).toEqual(1);
}

get fun `test bridge rejects the jetton master as recipient before reserving mint state`() {
    val deployer = testing.treasury("sccp-master-recipient");
    val config = securityBridgeConfig();
    val bridge = TairaXorSccpBridge.fromStorage(canonicalBridgeInitialStorage(config));
    val master = TairaXorJettonMaster.fromStorage(
        canonicalMasterInitialStorage(config, bridge.address),
    );
    expect(securityDeployBridge(bridge, deployer.address)).toHaveSuccessfulDeploy({
        to: bridge.address,
    });
    expect(securityDeployMaster(master, deployer.address)).toHaveSuccessfulDeploy({
        to: master.address,
    });

    val rejected = securityMint(bridge, deployer.address, master.address, 100);
    expect(rejected).toHaveFailedTx<SccpFinalizeFromTaira>({
        to: bridge.address,
        exitCode: SccpErrors.InvalidPayload,
    });
    val (bridgeMints, bridgeBurns) = bridge.getSccpPendingCounts();
    expect(bridgeMints).toEqual(0);
    expect(bridgeBurns).toEqual(0);
    expect(master.getSccpPendingMintCount()).toEqual(0);
    expect(master.getJettonData().totalSupply).toEqual(0);
    val (_, _, bridgeLeaves, _) = bridge.getSccpReplayState(
        SCCP_REPLAY_TON_BRIDGE_INBOUND_MINT,
        0,
    );
    val (_, _, masterLeaves, _) = master.getSccpReplayState(
        SCCP_REPLAY_TON_MASTER_MINT,
        0,
    );
    expect(bridgeLeaves).toEqual(0);
    expect(masterLeaves).toEqual(0);
}

get fun `test stale bridge witness bounces before 0x31 and fresh 0x37 refunds once`() {
    val deployer = testing.treasury("sccp-refund-flow");
    val owner = testing.treasury("sccp-refund-owner");
    val relayer = testing.treasury("sccp-refund-relayer");
    val config = securityBridgeConfig();
    val bridge = TairaXorSccpBridge.fromStorage(canonicalBridgeInitialStorage(config));
    val master = TairaXorJettonMaster.fromStorage(
        canonicalMasterInitialStorage(config, bridge.address),
    );
    securityDeployBridge(bridge, deployer.address);
    securityDeployMaster(master, deployer.address);
    securityMint(bridge, deployer.address, owner.address, 10);
    val wallet = TairaXorJettonWallet.fromAddress(master.getWalletAddress(owner.address));

    var stale = securityEmptyReplayWitness();
    stale.expectedShardRoot = 1;
    val rejected = wallet.sendSccpWalletPrepareBurn(
        owner.address,
        21,
        0,
        10,
        securityValidTairaRecipient(),
        securityBurnWitnesses(stale),
        { value: sccpWalletBurnPrepareMinimum() },
    );
    expect(rejected).toHaveFailedTx<SccpBridgeBurnPrepare>({
        to: bridge.address,
        exitCode: SccpErrors.StaleReplayWitness,
    });
    expect(rejected).toHaveSuccessfulTx({
        from: bridge.address,
        to: wallet.address,
        bounced: true,
    });
    expect(wallet.getWalletData().jettonBalance).toEqual(0);
    val (_, _, bridgeLeaves, _) = bridge.getSccpReplayState(
        SCCP_REPLAY_TON_BRIDGE_OUTBOUND_BURN,
        0,
    );
    expect(bridgeLeaves).toEqual(0);
    val (_, _, lockedBeforeRefund) = wallet.getSccpOutboundState();
    expect(lockedBeforeRefund).toEqual(1);

    val refund = wallet.sendSccpWalletRefundBurnLock(
        relayer.address,
        0,
        securityEmptyReplayWitness(),
        { value: sccpMaximumComputeFee() },
    );
    expect(refund).toHaveSuccessfulTx<SccpWalletRefundBurnLock>({ to: wallet.address });
    expect(wallet.getWalletData().jettonBalance).toEqual(10);
    val (_, _, lockedAfterRefund) = wallet.getSccpOutboundState();
    expect(lockedAfterRefund).toEqual(0);
    val duplicate = wallet.sendSccpWalletRefundBurnLock(
        relayer.address,
        0,
        securityEmptyReplayWitness(),
        { value: sccpMaximumComputeFee() },
    );
    expect(duplicate).toHaveSuccessfulTx<SccpWalletRefundBurnLock>({ to: wallet.address });
    expect(wallet.getWalletData().jettonBalance).toEqual(10);
}

get fun `test unauthorized and underfunded entries cannot poison replay`() {
    val deployer = testing.treasury("sccp-poison-deployer");
    val attacker = testing.treasury("sccp-poison-attacker");
    val victim = testing.treasury("sccp-poison-victim");
    val config = securityBridgeConfig();
    val bridge = TairaXorSccpBridge.fromStorage(canonicalBridgeInitialStorage(config));
    val master = TairaXorJettonMaster.fromStorage(
        canonicalMasterInitialStorage(config, bridge.address),
    );
    securityDeployBridge(bridge, deployer.address);
    securityDeployMaster(master, deployer.address);
    val empty = securityEmptyReplayWitness();
    val poison = bridge.sendSccpBridgeBurnPrepare(
        attacker.address,
        0,
        1,
        1,
        0,
        victim.address,
        1,
        {
            payloadHash: 1,
            payloadSha256: 1,
            tairaRecipient: securityValidTairaRecipient(),
            replayWitnesses: (SccpBurnPrepareRetryWitnessesV1 {
                bridge: empty.toCell(),
                walletAuthorization: empty.toCell(),
                master: empty.toCell(),
            }).toCell(),
        },
        { value: sccpBridgeBurnPrepareMinimum() },
    );
    expect(poison).toHaveFailedTx<SccpBridgeBurnPrepare>({
        to: bridge.address,
        exitCode: SccpErrors.NotValidWallet,
    });
    val (_, _, bridgeLeaves, _) = bridge.getSccpReplayState(
        SCCP_REPLAY_TON_BRIDGE_OUTBOUND_BURN,
        0,
    );
    expect(bridgeLeaves).toEqual(0);

    val unauthorizedMint = master.sendSccpMasterMint(
        attacker.address,
        1,
        1,
        victim.address,
        1,
        {
            payloadHash: 1,
            payloadSha256: 1,
            replayWitnesses: (SccpReplayWitnessPairV1 {
                first: empty.toCell(),
                second: empty.toCell(),
            }).toCell(),
        },
        { value: sccpMasterMintMinimum() },
    );
    expect(unauthorizedMint).toHaveFailedTx<SccpMasterMint>({
        to: master.address,
        exitCode: SccpErrors.NotBridge,
    });

    val belowMinimum = bridge.sendSccpRetry(
        attacker.address,
        1,
        1,
        { first: empty.toCell(), second: empty.toCell() },
        { value: sccpBridgeRetryMinimum() - 1 },
    );
    expect(belowMinimum).toHaveFailedTx<SccpRetry>({
        to: bridge.address,
        exitCode: SccpErrors.NotEnoughGas,
    });
}

get fun `test funding readback is network-derived bounded and reserve preserving`() {
    val deployer = testing.treasury("sccp-funding-readback");
    val config = securityBridgeConfig();
    val bridge = TairaXorSccpBridge.fromStorage(canonicalBridgeInitialStorage(config));
    val master = TairaXorJettonMaster.fromStorage(
        canonicalMasterInitialStorage(config, bridge.address),
    );
    securityDeployBridge(bridge, deployer.address);
    securityDeployMaster(master, deployer.address);
    val (finalizeMinimum, retryMinimum, bridgeReserve) = bridge.getSccpFundingParameters();
    val (masterMintMinimum, masterBurnMinimum, masterReserve) = master.getSccpFundingParameters();
    expect(finalizeMinimum).toEqual(sccpBridgeFinalizeMinimum());
    expect(retryMinimum).toEqual(sccpBridgeRetryMinimum());
    expect(masterMintMinimum).toEqual(sccpMasterMintMinimum());
    expect(masterBurnMinimum).toEqual(sccpMasterBurnMinimum());
    expect(bridgeReserve).toEqual(sccpBridgeStorageReserve());
    expect(masterReserve).toEqual(sccpMasterStorageReserve());
    expect(finalizeMinimum > masterMintMinimum).toEqual(true);
    expect(retryMinimum > masterMintMinimum).toEqual(true);
    expect(bridgeReserve > 0 && masterReserve > 0).toEqual(true);
    val before = testing.getAccountBalance(bridge.address);
    val topUp = bridge.sendTopUpTons(deployer.address, { value: ton("1") });
    expect(topUp).toHaveSuccessfulTx<TopUpTons>({ to: bridge.address });
    expect(testing.getAccountBalance(bridge.address) > before).toEqual(true);
}

@test.fail_with(78)
get fun `test duplicate guardian keys cannot enter canonical StateInit`() {
    canonicalBridgeInitialStorage(
        securityBridgeConfigWithGuardians(securityGuardians(1, 1)),
    );
}

@test.fail_with(78)
get fun `test guardian substitution order cannot enter canonical StateInit`() {
    canonicalBridgeInitialStorage(
        securityBridgeConfigWithGuardians(securityGuardians(2, 1)),
    );
}

@test.fail_with(51)
get fun `test amount equal to two to the 120 is rejected`() {
    sccpCheckedScaleAmount(1 << 120, 1, SCCP_MAX_JETTON_AMOUNT);
}
