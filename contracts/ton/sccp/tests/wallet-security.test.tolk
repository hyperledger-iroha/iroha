/// Emulator regressions for TON wallet bounce and native-value isolation.

import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@contracts/constants"
import "@contracts/errors"
import "@contracts/jetton-utils"
import "@contracts/messages"
import "@contracts/proof-verifier"
import "@contracts/sccp-codec"
import "@contracts/storage"
import "@wrappers/TairaXorJettonMaster.gen"
import "@wrappers/TairaXorJettonWallet.gen"
import "@wrappers/TairaXorSccpBridge.gen"

fun securityTestCanonicalRecipientFromSlice(value: slice): cell {
    var input = value;
    if (input.remainingBitsCount() <= SCCP_TAIRA_RECIPIENT_HEAD_BYTES * 8) {
        return beginCell().storeSlice(input).endCell();
    }
    val head = input.loadBits(SCCP_TAIRA_RECIPIENT_HEAD_BYTES * 8);
    val tail = beginCell().storeSlice(input).endCell();
    return beginCell().storeSlice(head).storeRef(tail).endCell();
}

fun securityTestValidTairaRecipient(): cell {
    return securityTestCanonicalRecipientFromSlice(
        "testuﾛ1PYﾛ9ｵﾆﾘﾐ3Yf8wﾜｿﾋﾉajｼｱ6eﾑbHｱﾜｶBｳdUｺcヰｲnﾌNP21YC".literalSlice(),
    );
}

fun securityTestG1(): cell {
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

fun securityTestG2(): cell {
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

fun securityTestVerifyingKey(): cell {
    val g1 = securityTestG1();
    val g2 = securityTestG2();
    var points: map<uint8, cell> = [];
    var index = 0;
    while (index < SCCP_GROTH16_VK_IC_COUNT) {
        points.set(index as uint8, g1);
        index += 1;
    }
    val tail: Groth16Bls12381VerificationKeyTail = { deltaG2: g2, ic: points };
    return Groth16Bls12381VerificationKey {
        version: 1,
        publicSignalCount: SCCP_GROTH16_PUBLIC_SIGNAL_COUNT,
        alphaG1: g1,
        betaG2: g2,
        gammaG2: g2,
        tail: tail.toCell(),
    }.toCell();
}

fun securityTestBridgeConfig(): Cell<BridgeConfig> {
    val sourceLane = beginCell().storeSlice("security-source".literalSlice()).endCell();
    val destinationLane = beginCell().storeSlice("security-destination".literalSlice()).endCell();
    val routeTail: RouteHashesTail = { destinationBindingHash: 4, semanticProofProfileHash: 5 };
    val routeHashes: RouteHashes = {
        sourceLaneHash: sccpLaneHash(sourceLane),
        destinationLaneHash: sccpLaneHash(destinationLane),
        routeConfigurationHash: 3,
        tail: routeTail.toCell(),
    };
    val verifyingKey = securityTestVerifyingKey();
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
        verifyingKeyHash: canonicalBls12381VerifyingKeyHash(verifyingKey),
        tail: finalityTail.toCell(),
    };
    return BridgeConfig {
        expectedGlobalId: -239,
        routeRevision: 1,
        tairaToTonMultiplier: SCCP_TAIRA_TO_TON_MULTIPLIER,
        maxWrappedSupply: 100,
        sourceLaneBytes: sourceLane,
        destinationLaneBytes: destinationLane,
        routeHashes: routeHashes.toCell(),
        finalityHashes: finalityHashes.toCell(),
    }.toCell();
}

fun deployWalletForSecurityTest(
    wallet: TairaXorJettonWallet,
    from: address,
    value: coins,
): SendResultList {
    val deployment = createMessage({
        bounce: BounceMode.NoBounce,
        value,
        dest: { stateInit: wallet.stateInit! },
        body: TopUpTons {},
    });
    return net.send(from, deployment);
}

fun deployMasterForSecurityTest(
    master: TairaXorJettonMaster,
    from: address,
    value: coins,
): SendResultList {
    val deployment = createMessage({
        bounce: BounceMode.NoBounce,
        value,
        dest: { stateInit: master.stateInit! },
        body: TopUpTons {},
    });
    return net.send(from, deployment);
}

fun deployBridgeForSecurityTest(
    bridge: TairaXorSccpBridge,
    from: address,
    value: coins,
): SendResultList {
    val deployment = createMessage({
        bounce: BounceMode.NoBounce,
        value,
        dest: { stateInit: bridge.stateInit! },
        body: TopUpTons {},
    });
    return net.send(from, deployment);
}

get fun `test master rejects recorded supply above immutable cap`() {
    val deployer = testing.treasury("sccp-over-cap-master-deployer");
    var masterStorage = canonicalMasterInitialStorage(
        beginCell().storeUint(0x53434350, 32).endCell(),
        securityTestBridgeConfig(),
    );
    masterStorage.totalSupply = 101;
    val master = TairaXorJettonMaster.fromStorage(masterStorage);
    expect(
        deployMasterForSecurityTest(master, deployer.address, ton("3")),
    ).toHaveSuccessfulDeploy({ to: master.address });

    val trace = master.sendSccpMasterBurn(
        deployer.address,
        1,
        0x1111,
        deployer.address,
        1,
        { payloadHash: 0x2222, sourceEventDigest: 0x3333 },
        { value: ton("1") },
    );
    expect(trace).toHaveFailedTx<SccpMasterBurn>({
        from: deployer.address,
        to: master.address,
        exitCode: SccpErrors.InvalidConfiguration,
    });
    expect(master.getJettonData().totalSupply).toEqual(101);
}

get fun `test rich SCCP burn bounce retains the exact pending debit`() {
    val owner = testing.treasury("sccp-rich-bounce-owner");
    val missingMaster = address.fromWorkchainAndHash(
        0,
        0xabababababababababababababababababababababababababababababababab,
    );
    val wallet = TairaXorJettonWallet.fromStorage({
        jettonBalance: 100,
        ownerAddress: owner.address,
        masterAddress: missingMaster,
        replay: emptyWalletReplayState(),
    });
    val deployment = deployWalletForSecurityTest(wallet, owner.address, ton("2"));
    expect(deployment).toHaveSuccessfulDeploy({ to: wallet.address });

    val first = wallet.sendSccpWalletBurn(
        owner.address,
        7,
        0x1111,
        10,
        0x2222,
        0x3333,
        { value: ton("1") },
    );
    expect(first).toHaveSuccessfulTx<SccpWalletBurn>({
        from: owner.address,
        to: wallet.address,
    });
    expect(first).toHaveSuccessfulTx({
        from: missingMaster,
        to: wallet.address,
        bounced: true,
    });
    expect(wallet.getWalletData().jettonBalance).toEqual(90);

    // An exact retry debits no second time, and its independent bounce cannot
    // release the debit reserved for another in-flight attempt.
    val retry = wallet.sendSccpWalletBurn(
        owner.address,
        7,
        0x1111,
        10,
        0x2222,
        0x3333,
        { value: ton("1") },
    );
    expect(retry).toHaveSuccessfulTx({
        from: missingMaster,
        to: wallet.address,
        bounced: true,
    });
    expect(wallet.getWalletData().jettonBalance).toEqual(90);

    val changed = wallet.sendSccpWalletBurn(
        owner.address,
        7,
        0x1111,
        11,
        0x2222,
        0x3333,
        { value: ton("1") },
    );
    expect(changed).toHaveFailedTx<SccpWalletBurn>({
        from: owner.address,
        to: wallet.address,
        exitCode: SccpErrors.Replay,
    });
    expect(wallet.getWalletData().jettonBalance).toEqual(90);
}

get fun `test excess return cannot sweep preexisting recipient wallet tons`() {
    val attacker = testing.treasury("sccp-excess-attacker");
    val victim = testing.treasury("sccp-excess-victim");
    val master = address.fromWorkchainAndHash(
        0,
        0xcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd,
    );
    val source = TairaXorJettonWallet.fromStorage({
        jettonBalance: 0,
        ownerAddress: attacker.address,
        masterAddress: master,
        replay: emptyWalletReplayState(),
    });
    val destination = TairaXorJettonWallet.fromStorage({
        jettonBalance: 0,
        ownerAddress: victim.address,
        masterAddress: master,
        replay: emptyWalletReplayState(),
    });
    expect(deployWalletForSecurityTest(source, attacker.address, ton("2"))).toHaveSuccessfulDeploy({
        to: source.address,
    });
    val sourceMint = source.sendSccpWalletMint(
        master,
        1,
        0xaaaa,
        attacker.address,
        10,
        0xbbbb,
        { value: ton("1") },
    );
    expect(sourceMint).toHaveSuccessfulTx<SccpWalletMint>({ to: source.address });
    expect(source.getWalletData().jettonBalance).toEqual(10);
    expect(
        deployWalletForSecurityTest(destination, victim.address, ton("5")),
    ).toHaveSuccessfulDeploy({
        to: destination.address,
    });
    val balanceBefore = testing.getAccountBalance(destination.address);
    val trace = destination.sendInternalTransferStep(
        source.address,
        9,
        1,
        attacker.address,
        attacker.address,
        0,
        PayloadInline { value: beginCell().endCell().beginParse() },
        { value: ton("1") },
    );
    expect(trace).toHaveSuccessfulTx<InternalTransferStep>({ to: destination.address });
    val balanceAfter = testing.getAccountBalance(destination.address);
    expect(balanceAfter >= balanceBefore - SCCP_OPERATION_VALUE).toEqual(true);
    expect(destination.getWalletData().jettonBalance).toEqual(1);
}

get fun `test unrelated wallet cannot preempt canonical master burn replay`() {
    val deployer = testing.treasury("sccp-master-deployer");
    val attacker = testing.treasury("sccp-burn-preemption-attacker");
    val custody = testing.treasury("sccp-burn-preemption-custody");
    val config = securityTestBridgeConfig();
    var masterStorage = canonicalMasterInitialStorage(
        beginCell().storeUint(0x53434350, 32).endCell(),
        config,
    );
    // Seed supply only for this isolated replay test. Production StateInit is
    // separately pinned to canonical zero supply by deployment evidence.
    masterStorage.totalSupply = 20;
    val master = TairaXorJettonMaster.fromStorage(masterStorage);
    expect(
        deployMasterForSecurityTest(master, deployer.address, ton("3")),
    ).toHaveSuccessfulDeploy({ to: master.address });

    val attackerWallet = TairaXorJettonWallet.fromStorage({
        jettonBalance: 0,
        ownerAddress: attacker.address,
        masterAddress: master.address,
        replay: emptyWalletReplayState(),
    });
    val custodyWallet = TairaXorJettonWallet.fromStorage({
        jettonBalance: 0,
        ownerAddress: custody.address,
        masterAddress: master.address,
        replay: emptyWalletReplayState(),
    });
    expect(
        deployWalletForSecurityTest(attackerWallet, attacker.address, ton("2")),
    ).toHaveSuccessfulDeploy({ to: attackerWallet.address });
    expect(
        deployWalletForSecurityTest(custodyWallet, custody.address, ton("2")),
    ).toHaveSuccessfulDeploy({ to: custodyWallet.address });

    attackerWallet.sendSccpWalletMint(
        master.address,
        1,
        0xa1,
        attacker.address,
        10,
        0xa2,
        { value: ton("1") },
    );
    custodyWallet.sendSccpWalletMint(
        master.address,
        2,
        0xc1,
        custody.address,
        10,
        0xc2,
        { value: ton("1") },
    );
    expect(attackerWallet.getWalletData().jettonBalance).toEqual(10);
    expect(custodyWallet.getWalletData().jettonBalance).toEqual(10);

    val sharedMessageId = 0x9999;
    val attackerBurn = attackerWallet.sendSccpWalletBurn(
        attacker.address,
        3,
        sharedMessageId,
        1,
        0xa3,
        0xa4,
        { value: ton("1") },
    );
    expect(attackerBurn).toHaveSuccessfulTx<SccpMasterBurn>({ to: master.address });
    expect(master.getJettonData().totalSupply).toEqual(19);

    val custodyBurn = custodyWallet.sendSccpWalletBurn(
        custody.address,
        4,
        sharedMessageId,
        10,
        0xc3,
        0xc4,
        { value: ton("1") },
    );
    expect(custodyBurn).toHaveSuccessfulTx<SccpMasterBurn>({ to: master.address });
    expect(master.getJettonData().totalSupply).toEqual(9);
    expect(attackerWallet.getWalletData().jettonBalance).toEqual(9);
    expect(custodyWallet.getWalletData().jettonBalance).toEqual(0);
}

get fun `test masterchain owner deposit is refunded exactly`() {
    val deployer = testing.treasury("sccp-masterchain-refund-deployer");
    val owner = address.fromWorkchainAndHash(
        -1,
        0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee,
    );
    val config = securityTestBridgeConfig();
    var masterStorage = canonicalMasterInitialStorage(
        beginCell().storeUint(0x53434350, 32).endCell(),
        config,
    );
    masterStorage.totalSupply = 10;
    val master = TairaXorJettonMaster.fromStorage(masterStorage);
    expect(
        deployMasterForSecurityTest(master, deployer.address, ton("3")),
    ).toHaveSuccessfulDeploy({ to: master.address });

    val bridge = TairaXorSccpBridge.fromStorage(
        canonicalBridgeInitialStorage(config, master.address),
    );
    expect(
        deployBridgeForSecurityTest(bridge, deployer.address, ton("3")),
    ).toHaveSuccessfulDeploy({ to: bridge.address });

    val ownerWallet = TairaXorJettonWallet.fromStorage({
        jettonBalance: 0,
        ownerAddress: owner,
        masterAddress: master.address,
        replay: emptyWalletReplayState(),
    });
    val custodyWallet = TairaXorJettonWallet.fromStorage({
        jettonBalance: 0,
        ownerAddress: bridge.address,
        masterAddress: master.address,
        replay: emptyWalletReplayState(),
    });
    expect(
        deployWalletForSecurityTest(ownerWallet, deployer.address, ton("2")),
    ).toHaveSuccessfulDeploy({ to: ownerWallet.address });
    expect(
        deployWalletForSecurityTest(custodyWallet, deployer.address, ton("2")),
    ).toHaveSuccessfulDeploy({ to: custodyWallet.address });
    ownerWallet.sendSccpWalletMint(
        master.address,
        1,
        0xe1,
        bridge.address,
        10,
        0xe2,
        { value: ton("1") },
    );
    expect(ownerWallet.getWalletData().jettonBalance).toEqual(10);

    val trace = ownerWallet.sendAskToTransfer(
        owner,
        2,
        10,
        bridge.address,
        null,
        null,
        2 * SCCP_OPERATION_VALUE,
        PayloadInline {
            value: (SccpOutboundBurnRequest {
                version: SCCP_MESSAGE_SCHEMA_VERSION,
                expectedNonce: 0,
                tairaRecipient: securityTestValidTairaRecipient(),
            })
                .toCell()
                .beginParse(),
        },
        { value: ton("2") },
    );
    expect(trace).toHaveSuccessfulTx<TransferNotificationForRecipient>({ to: bridge.address });
    expect(trace).toHaveSuccessfulTx<SccpWalletRefundTransfer>({ to: ownerWallet.address });
    expect(trace).toHaveSuccessfulTx<SccpWalletRefundAck>({ to: bridge.address });
    expect(ownerWallet.getWalletData().jettonBalance).toEqual(10);
    expect(custodyWallet.getWalletData().jettonBalance).toEqual(0);
    expect(master.getJettonData().totalSupply).toEqual(10);
}
