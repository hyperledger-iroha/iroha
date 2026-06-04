import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaRecursiveSpendProverTests: XCTestCase {
    func testPreferredModeDefaultsToRecursiveWhenAvailable() {
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.preferredMode(recursiveSpendAvailable: true),
            .recursiveSpendV1
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.preferredMode(recursiveSpendAvailable: false),
            .checkedPrefoldV1
        )
    }

    func testExportsStableCircuitIds() {
        XCTAssertEqual(KagemushaRecursiveSpendProver.requiredBridgeAbiVersion, 6)
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
            "kagemusha-recursive-aggregation-v1"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
            "kagemusha-recursive-spend-lineage-v1"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
            "kagemusha-recursive-spend-lineage-onehop-v1"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1,
            "kagemusha-recursive-spend-lineage-append-v1"
        )
        XCTAssertEqual(KagemushaRecursiveSpendProver.compactTokenMaxHops, 64)
        XCTAssertEqual(KagemushaRecursiveSpendProver.recursiveSpendLineageWitnesslessMaxHopsV1, 64)
        XCTAssertTrue(KagemushaRecursiveSpendProver.recursiveSpendLineageTransitionCircuitWiredV1)
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursivePreviousProofOpenEnvelopesRequiredCountV1,
            1
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursivePreviousProofOpenEnvelopesMaxBytes,
            8 * 1024 * 1024
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursivePallasOpenEnvelopeMaxTranscriptLabelBytes,
            128
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveSpendTransitionProfileDomain,
            "iroha:kagemusha:v1:recursive-spend-transition-profile"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveSpendTransitionProfileDigestDomain,
            "iroha:kagemusha:v1:recursive-spend-transition-profile-digest"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveSpendTransitionProfileBindingDigestDomain,
            "iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveSpendLineageAppendOpeningsPreflightDomainV1,
            "iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveSpendLineageAppendBoundaryDomainV1,
            "iroha:kagemusha:recursive-spend-lineage-append-boundary:v1"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveSpendLineageAppendBoundaryChainAssetBindingDomainV1,
            "iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveSpendLineageAppendBoundaryFinalNoteBindingDomainV1,
            "iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1"
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                hopCount: 1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
                hopCount: 1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1,
                hopCount: 2
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresLineageWitnessForRedeem(
                circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                hopCount: 1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                hopCount: KagemushaRecursiveSpendProver.recursiveSpendLineageWitnesslessMaxHopsV1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresLineageWitnessForRedeem(
                circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                hopCount: KagemushaRecursiveSpendProver.recursiveSpendLineageWitnesslessMaxHopsV1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                circuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                hopCount: 1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                hopCount: 0
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresLineageWitnessForRedeem(
                circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                hopCount: 2
            )
        )
        for (circuitId, hopCount) in [
            (KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1, UInt32.max),
            (KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1, 0),
            (KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1, UInt32.max),
            ("", 1),
            ("unknown-kagemusha-recursive-spend-circuit", UInt32.max),
        ] {
            XCTAssertFalse(
                KagemushaRecursiveSpendProver.canRedeemWitnessless(
                    circuitId: circuitId,
                    hopCount: hopCount
                )
            )
            XCTAssertTrue(
                KagemushaRecursiveSpendProver.requiresLineageWitnessForRedeem(
                    circuitId: circuitId,
                    hopCount: hopCount
                )
            )
        }
        XCTAssertFalse(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(previousHopCount: 0))
        XCTAssertTrue(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(previousHopCount: 1))
        XCTAssertTrue(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(previousHopCount: 63))
        XCTAssertFalse(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(previousHopCount: 64))
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(previousHopCount: UInt32.max)
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.normalizedAppendOutputCircuitId(nil),
            KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.normalizedAppendOutputCircuitId(""),
            KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.normalizedAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1
            ),
            KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.normalizedAppendOutputCircuitId(
                "unknown-kagemusha-recursive-spend-circuit"
            ),
            "unknown-kagemusha-recursive-spend-circuit"
        )
        XCTAssertTrue(KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(nil))
        XCTAssertTrue(KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(""))
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isLineageProofCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isLineageProofCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isLineageProofCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.isLineageAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isLineageAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(
                "unknown-kagemusha-recursive-spend-circuit"
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
                KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
                "unknown-kagemusha-recursive-spend-circuit"
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
                previousProofCircuitId: "unknown-kagemusha-recursive-spend-circuit"
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                outputCircuitId: ""
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1
            ),
            "semantic previous proofs cannot select Reserved-lineage output"
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
                previousProofCircuitId: "unknown-kagemusha-recursive-spend-circuit",
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                outputCircuitId: "unknown-kagemusha-recursive-spend-circuit"
            )
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(previousHopCount: 1),
            KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(previousHopCount: 63),
            KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(previousHopCount: 64),
            KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
            "preferred append selector falls back at the witnessless hop cap"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(previousHopCount: 0),
            KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(nil, previousHopCount: 1)
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousHopCount: KagemushaRecursiveSpendProver.compactTokenMaxHops - 1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousHopCount: 0
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousHopCount: KagemushaRecursiveSpendProver.compactTokenMaxHops
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                previousHopCount: 63
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                previousHopCount: 64
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                "unknown-kagemusha-recursive-spend-circuit",
                previousHopCount: 1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                previousProofCircuitId: "unknown-kagemusha-recursive-spend-circuit",
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                previousHopCount: 1
            ),
            "semantic previous proofs cannot select Reserved-lineage output"
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                outputCircuitId: "unknown-kagemusha-recursive-spend-circuit",
                previousHopCount: 1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousHopCount: 0
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                previousHopCount: 64
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                previousHopCount: 0
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
                outputCircuitId: nil,
                previousHopCount: 1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
                outputCircuitId: "",
                previousHopCount: 1
            )
        )
    }

    func testRejectsEmptyRequestArchivesBeforeBridgeCall() {
        let helpers: [(String, (Data) throws -> Data)] = [
            ("init", KagemushaRecursiveSpendProver.initSpend),
            ("append", KagemushaRecursiveSpendProver.appendSpend),
            ("transitionProfileInit", KagemushaRecursiveSpendProver.transitionProfileInit),
            ("transitionProfileAppend", KagemushaRecursiveSpendProver.transitionProfileAppend),
            ("lineageAppendBoundary", KagemushaRecursiveSpendProver.lineageAppendBoundary),
            ("verify", KagemushaRecursiveSpendProver.verifySpend),
            ("redeem", KagemushaRecursiveSpendProver.redeemSpend)
        ]

        for (label, helper) in helpers {
            XCTAssertThrowsError(try helper(Data()), "helper \(label) should reject empty archives") { error in
                XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .emptyRequestArchive)
            }
        }

        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                requestArchive: Data(),
                bundleArchive: Data([0x01])
            )
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .emptyRequestArchive)
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                requestArchive: Data([0x01]),
                bundleArchive: Data()
            )
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .emptyRequestArchive)
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                previousWitnessArchive: Data(),
                requestArchive: Data([0x01]),
                bundleArchive: Data([0x02])
            )
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .emptyRequestArchive)
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                previousWitnessArchive: Data([0x01]),
                requestArchive: Data(),
                bundleArchive: Data([0x02])
            )
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .emptyRequestArchive)
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                previousWitnessArchive: Data([0x01]),
                requestArchive: Data([0x02]),
                bundleArchive: Data()
            )
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .emptyRequestArchive)
        }
    }

    func testRejectsEmptyNativeOutput() {
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.call(
                requestArchive: Data([0x01]),
                bridgeAvailable: true
            ) { _ in
                Data()
            }
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .proofRejected)
        }
    }

    func testNilNativeOutputIsBridgeUnavailable() {
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.call(
                requestArchive: Data([0x01]),
                bridgeAvailable: true
            ) { _ in
                nil
            }
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .bridgeUnavailable)
        }
    }

    func testNativeKagemushaRejectionMapsToProofRejected() {
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.call(
                requestArchive: Data([0x01]),
                bridgeAvailable: true
            ) { _ in
                throw NativeBridgeError.kagemushaProve
            }
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .proofRejected)
        }
    }

    func testUnexpectedNativeRejectionMapsToProofRejected() {
        enum LocalError: Error {
            case rejected
        }

        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.call(
                requestArchive: Data([0x01]),
                bridgeAvailable: true
            ) { _ in
                throw LocalError.rejected
            }
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .proofRejected)
        }
    }

    func testNativeAvailabilityProbeRequiresMalformedArchiveRejection() {
        #if canImport(Darwin)
        XCTAssertTrue(
            NoritoNativeBridge.isExpectedKagemushaMalformedProbeResult(
                status: NoritoNativeBridge.expectedKagemushaProbeStatus,
                outPtr: nil,
                outLen: 0
            )
        )
        XCTAssertFalse(
            NoritoNativeBridge.isExpectedKagemushaMalformedProbeResult(
                status: 0,
                outPtr: nil,
                outLen: 0
            )
        )
        XCTAssertFalse(
            NoritoNativeBridge.isExpectedKagemushaMalformedProbeResult(
                status: -1,
                outPtr: nil,
                outLen: 0
            )
        )
        XCTAssertFalse(
            NoritoNativeBridge.isExpectedKagemushaMalformedProbeResult(
                status: NoritoNativeBridge.expectedKagemushaProbeStatus,
                outPtr: nil,
                outLen: 1
            )
        )
        let output = UnsafeMutablePointer<UInt8>.allocate(capacity: 1)
        defer { output.deallocate() }
        XCTAssertFalse(
            NoritoNativeBridge.isExpectedKagemushaMalformedProbeResult(
                status: NoritoNativeBridge.expectedKagemushaProbeStatus,
                outPtr: output,
                outLen: 0
            )
        )
        #endif
    }

    func testRejectsMalformedArchivesWhenBridgeIsAvailable() throws {
        guard KagemushaRecursiveSpendProver.isNativeAvailable else {
            throw XCTSkip("Native Kagemusha recursive spend prover is unavailable.")
        }

        let helpers: [(String, (Data) throws -> Data)] = [
            ("init", KagemushaRecursiveSpendProver.initSpend),
            ("append", KagemushaRecursiveSpendProver.appendSpend),
            ("transitionProfileInit", KagemushaRecursiveSpendProver.transitionProfileInit),
            ("transitionProfileAppend", KagemushaRecursiveSpendProver.transitionProfileAppend),
            ("verify", KagemushaRecursiveSpendProver.verifySpend),
            ("redeem", KagemushaRecursiveSpendProver.redeemSpend)
        ]

        for (label, helper) in helpers {
            XCTAssertThrowsError(try helper(Data([0x01, 0x02])), "helper \(label) should reject malformed archives") { error in
                XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .proofRejected)
            }
        }

        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                requestArchive: Data([0x01, 0x02]),
                bundleArchive: Data([0x03, 0x04])
            )
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .proofRejected)
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                previousWitnessArchive: Data([0x01, 0x02]),
                requestArchive: Data([0x03, 0x04]),
                bundleArchive: Data([0x05, 0x06])
            )
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .proofRejected)
        }
    }
}
