//! Canonical non-deployable fixture for the final-V1 bridge/master StateInit golden.

import "@acton/build"
import "@acton/crypto"
import "@contracts/constants"
import "@contracts/proof-verifier"
import "@contracts/sccp-codec"
import "@contracts/storage"

const SCCP_STATEINIT_GOLDEN_ROUTE_REVISION = 1
const SCCP_STATEINIT_GOLDEN_MAX_WRAPPED_SUPPLY = 1000000000000000

fun stateInitGoldenG1(): cell {
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

fun stateInitGoldenG2(): cell {
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

fun stateInitGoldenVerifyingKey(): cell {
    val g1 = stateInitGoldenG1();
    val g2 = stateInitGoldenG2();
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

fun stateInitGoldenGuardians(): Cell<MintBreakerGuardians> {
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
    return MintBreakerGuardians {
        guardian0: keys.get(0).publicKey,
        guardian1: keys.get(1).publicKey,
        guardian2: keys.get(2).publicKey,
        tail: (MintBreakerGuardiansTail {
            guardian3: keys.get(3).publicKey,
            guardian4: keys.get(4).publicKey,
        }).toCell(),
    }.toCell();
}

fun stateInitGoldenBridgeConfig(): Cell<BridgeConfig> {
    val sourceLaneBytes = canonicalSccpTonToSoraLaneBytesV1();
    val destinationLaneBytes = canonicalSccpSoraToTonLaneBytesV1();
    val verifyingKey = stateInitGoldenVerifyingKey();
    val codeHashes = DeploymentCodeHashes {
        jettonMasterCodeHash: build("TairaXorJettonMaster").hash(),
        jettonWalletCodeHash: build("TairaXorJettonWallet").hash(),
        routeCodeHash: build("TairaXorSccpBridge").hash(),
    }.toCell();
    val draftRouteHashes: RouteHashes = {
        sourceLaneHash: sccpLaneHash(sourceLaneBytes),
        destinationLaneHash: sccpLaneHash(destinationLaneBytes),
        routeConfigurationHash: 1,
        tail: (RouteHashesTail {
            destinationBindingHash: 1,
            semanticProofProfileHash: 0x1818181818181818181818181818181818181818181818181818181818181818,
            deploymentCodeHashes: codeHashes,
        }).toCell(),
    };
    val finalityHashes: FinalityHashes = {
        soraFinalityAnchorHash: 0x1919191919191919191919191919191919191919191919191919191919191919,
        verifierCircuitHash: 0x1515151515151515151515151515151515151515151515151515151515151515,
        verifyingKeyHash: canonicalBls12381VerifyingKeyHash(verifyingKey),
        tail: (FinalityHashesTail {
            proofProfileCommitment: 0x1717171717171717171717171717171717171717171717171717171717171717,
            mintBreakerGuardians: stateInitGoldenGuardians(),
            embeddedVerifierCodeHash: 0x1414141414141414141414141414141414141414141414141414141414141414,
            verifyingKeyCellHash: verifyingKey.hash(),
            verifyingKey,
            masterMetadata: OnchainMetadataReply { contentDict: [] }.toCell(),
        }).toCell(),
    };
    val draft: BridgeConfig = {
        expectedGlobalId: SCCP_TON_MAINNET_GLOBAL_ID,
        routeRevision: SCCP_STATEINIT_GOLDEN_ROUTE_REVISION,
        tairaToTonMultiplier: SCCP_TAIRA_TO_TON_MULTIPLIER,
        maxWrappedSupply: SCCP_STATEINIT_GOLDEN_MAX_WRAPPED_SUPPLY,
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
            semanticProofProfileHash: draftRouteHashes.tail.load().semanticProofProfileHash,
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

fun stateInitGoldenBridgeState(): ContractState {
    val config = stateInitGoldenBridgeConfig();
    return {
        code: build("TairaXorSccpBridge"),
        data: canonicalBridgeInitialStorage(config).toCell(),
    };
}

fun stateInitGoldenBridgeAddress(): address {
    return AutoDeployAddress {
        workchain: SCCP_WORKCHAIN,
        stateInit: stateInitGoldenBridgeState(),
    }.calculateAddress();
}

fun stateInitGoldenMasterState(): ContractState {
    val config = stateInitGoldenBridgeConfig();
    return {
        code: build("TairaXorJettonMaster"),
        data: canonicalMasterInitialStorage(config, stateInitGoldenBridgeAddress()).toCell(),
    };
}

fun stateInitGoldenMasterAddress(): address {
    return AutoDeployAddress {
        workchain: SCCP_WORKCHAIN,
        stateInit: stateInitGoldenMasterState(),
    }.calculateAddress();
}
