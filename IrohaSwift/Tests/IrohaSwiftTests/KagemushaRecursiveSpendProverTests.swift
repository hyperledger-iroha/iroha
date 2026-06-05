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

    func testSharedRecursiveSpendAbi6FixtureMatchesSdkSurface() throws {
        let manifest = try Self.sharedRecursiveSpendManifest()
        XCTAssertEqual(
            manifest["schema"] as? String,
            "iroha.kagemusha.recursive_spend.abi6.fixture_manifest.v1"
        )
        XCTAssertEqual(
            manifest["bridge_abi_version"] as? Int,
            Int(KagemushaRecursiveSpendProver.requiredBridgeAbiVersion)
        )

        let circuitIds = try XCTUnwrap(manifest["proof_circuit_ids"] as? [String: Any])
        XCTAssertEqual(
            circuitIds["recursive_aggregation"] as? String,
            KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
        )
        XCTAssertEqual(
            circuitIds["reserved_lineage"] as? String,
            KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1
        )
        XCTAssertEqual(
            circuitIds["reserved_lineage_one_hop"] as? String,
            KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1
        )
        XCTAssertEqual(
            circuitIds["reserved_lineage_append"] as? String,
            KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1
        )

        let limits = try XCTUnwrap(manifest["limits"] as? [String: Any])
        XCTAssertEqual(
            limits["compact_token_max_hops"] as? Int,
            Int(KagemushaRecursiveSpendProver.compactTokenMaxHops)
        )
        XCTAssertEqual(
            limits["reserved_lineage_witnessless_max_hops"] as? Int,
            Int(KagemushaRecursiveSpendProver.recursiveSpendLineageWitnesslessMaxHopsV1)
        )
        XCTAssertEqual(
            limits["previous_proof_open_envelopes_required_count"] as? Int,
            Int(KagemushaRecursiveSpendProver.recursivePreviousProofOpenEnvelopesRequiredCountV1)
        )
        XCTAssertEqual(
            limits["previous_proof_open_envelopes_max_bytes"] as? Int,
            Int(KagemushaRecursiveSpendProver.recursivePreviousProofOpenEnvelopesMaxBytes)
        )
        XCTAssertEqual(
            limits["pallas_open_envelope_max_transcript_label_bytes"] as? Int,
            Int(KagemushaRecursiveSpendProver.recursivePallasOpenEnvelopeMaxTranscriptLabelBytes)
        )
        XCTAssertEqual(
            limits["native_archive_max_bytes"] as? Int,
            Int(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes)
        )

        let domains = try XCTUnwrap(manifest["domains"] as? [String: Any])
        XCTAssertEqual(
            domains["transition_profile"] as? String,
            KagemushaRecursiveSpendProver.recursiveSpendTransitionProfileDomain
        )
        XCTAssertEqual(
            domains["lineage_append_boundary_final_note_binding"] as? String,
            KagemushaRecursiveSpendProver.recursiveSpendLineageAppendBoundaryFinalNoteBindingDomainV1
        )

        let operations = try XCTUnwrap(manifest["operations"] as? [[String: Any]])
        XCTAssertEqual(manifest["operation_count"] as? Int, operations.count)
        XCTAssertEqual(operations.count, 9)
        XCTAssertEqual(
            Set(operations.compactMap { $0["symbol"] as? String }),
            Set<String>([
                "connect_norito_kagemusha_recursive_spend_init",
                "connect_norito_kagemusha_recursive_spend_append",
                "connect_norito_kagemusha_recursive_spend_transition_profile_init",
                "connect_norito_kagemusha_recursive_spend_transition_profile_append",
                "connect_norito_kagemusha_recursive_spend_lineage_append_boundary",
                "connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result",
                "connect_norito_kagemusha_recursive_spend_lineage_witness_append_result",
                "connect_norito_kagemusha_recursive_spend_verify",
                "connect_norito_kagemusha_recursive_spend_redeem"
            ])
        )
        let appendWitness = try XCTUnwrap(operations.first { $0["name"] as? String == "lineage_witness_append_result" })
        XCTAssertEqual((appendWitness["input_archives"] as? [String])?.count, 3)
        XCTAssertEqual(appendWitness["output_archive"] as? String, "KagemushaRecursiveSpendLineageWitnessV1")

        let payloadBenchmarks = try XCTUnwrap(manifest["payload_benchmarks"] as? [String: Any])
        XCTAssertEqual(payloadBenchmarks["semantic_payload_bytes"] as? Int, 1751)
        XCTAssertEqual(payloadBenchmarks["reserved_lineage_payload_bytes"] as? Int, 3847)
        XCTAssertEqual(payloadBenchmarks["reserved_lineage_transition_profile_bytes"] as? Int, 2817)

        let archiveFixture = try Self.sharedRecursiveSpendArchives()
        XCTAssertEqual(
            archiveFixture["schema"] as? String,
            "iroha.kagemusha.recursive_spend.abi6.archive_fixtures.v1"
        )
        let archives = try XCTUnwrap(archiveFixture["archives"] as? [[String: Any]])
        XCTAssertEqual(archives.count, 13)
        XCTAssertEqual(
            Set(archives.compactMap { $0["name"] as? String }),
            Set<String>([
                "init_request",
                "init_bundle",
                "transition_profile_init",
                "append_request",
                "append_bundle",
                "transition_profile_append",
                "lineage_append_boundary",
                "lineage_witness_from_init_result",
                "lineage_witness_append_result",
                "verify_request",
                "verify_result",
                "redeem_request",
                "redeem_instruction"
            ])
        )
        let requestArchiveFields = try XCTUnwrap(archiveFixture["request_archive_fields"] as? [[String: Any]])
        let fieldRecordsByType = Dictionary(
            uniqueKeysWithValues: requestArchiveFields.compactMap { entry -> (String, [[String: Any]])? in
                guard
                    let type = entry["norito_type"] as? String,
                    let fields = entry["fields"] as? [[String: Any]]
                else {
                    return nil
                }
                return (type, fields)
            }
        )
        let fieldsByType = Dictionary(
            uniqueKeysWithValues: fieldRecordsByType.map { entry in
                (entry.key, entry.value.compactMap { $0["name"] as? String })
            }
        )
        XCTAssertEqual(
            Set(fieldsByType.keys),
            Set([
                "KagemushaRecursiveSpendInitRequestV1",
                "KagemushaRecursiveSpendAppendRequestV1",
                "KagemushaRecursiveSpendVerifyRequestV1",
                "KagemushaRecursiveSpendRedeemRequestV1"
            ])
        )
        XCTAssertEqual(
            fieldsByType["KagemushaRecursiveSpendInitRequestV1"],
            [
                "record_bundle",
                "pallas_open_envelopes_archive",
                "current_note",
                "lineage_verifier_key",
                "lineage_proving_key_archive",
                "block_height"
            ]
        )
        XCTAssertEqual(
            fieldsByType["KagemushaRecursiveSpendAppendRequestV1"],
            [
                "previous_bundle",
                "record_bundle",
                "pallas_open_envelopes_archive",
                "current_note",
                "output_proof_circuit_id",
                "previous_lineage_verifier_record",
                "previous_recursive_proof_open_envelopes_archive",
                "lineage_verifier_key",
                "lineage_proving_key_archive",
                "block_height"
            ]
        )
        XCTAssertEqual(
            fieldsByType["KagemushaRecursiveSpendVerifyRequestV1"],
            ["bundle", "lineage_verifier_record", "block_height"]
        )
        XCTAssertEqual(
            fieldsByType["KagemushaRecursiveSpendRedeemRequestV1"],
            [
                "bundle",
                "recipient",
                "public_amount",
                "redeem_proof",
                "lineage_witness",
                "lineage_verifier_record",
                "block_height"
            ]
        )
        for requestType in fieldsByType.keys {
            let blockHeight = try XCTUnwrap(
                fieldRecordsByType[requestType]?.first { $0["name"] as? String == "block_height" }
            )
            XCTAssertEqual(blockHeight["type"] as? String, "Option<u64>")
            XCTAssertEqual(blockHeight["norito_default"] as? Bool, true)
            XCTAssertEqual(blockHeight["semantics"] as? String, "verifier_record_activation_height")
        }
        let redeemArchive = try XCTUnwrap(archives.first { $0["name"] as? String == "redeem_request" })
        XCTAssertEqual(redeemArchive["operation"] as? String, "redeem")
        XCTAssertEqual(redeemArchive["norito_type"] as? String, "KagemushaRecursiveSpendRedeemRequestV1")
        XCTAssertGreaterThan(redeemArchive["byte_len"] as? Int ?? 0, 0)
        XCTAssertEqual((redeemArchive["sha256_hex"] as? String)?.count, 64)
        let redeemInstructionArchive = try XCTUnwrap(archives.first { $0["name"] as? String == "redeem_instruction" })
        XCTAssertEqual(redeemInstructionArchive["norito_type"] as? String, "RedeemKagemushaRecursive")

        XCTAssertEqual(
            KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(previousHopCount: 1),
            circuitIds["reserved_lineage_append"] as? String
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(previousHopCount: 63),
            circuitIds["reserved_lineage_append"] as? String
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(previousHopCount: 64),
            circuitIds["recursive_aggregation"] as? String
        )
        XCTAssertFalse(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(previousHopCount: 0))
        XCTAssertTrue(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(previousHopCount: 63))
        XCTAssertFalse(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(previousHopCount: 64))
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1,
                hopCount: 2
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                hopCount: 65
            )
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
        XCTAssertEqual(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes, 64 * 1024 * 1024)
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
        XCTAssertTrue(KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForInit())
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(
                outputCircuitId: nil
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(
                outputCircuitId: ""
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(
                outputCircuitId: "unknown-kagemusha-recursive-spend-circuit"
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

    func testRejectsOversizedNativeOutput() {
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.call(
                requestArchive: Data([0x01]),
                bridgeAvailable: true
            ) { _ in
                Data(
                    repeating: 0x7f,
                    count: KagemushaRecursiveSpendProver.nativeArchiveMaxBytes + 1
                )
            }
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .oversizedNativeOutput)
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

    private static func sharedRecursiveSpendManifest() throws -> [String: Any] {
        try sharedRecursiveSpendFixture(named: "manifest.json")
    }

    private static func sharedRecursiveSpendArchives() throws -> [String: Any] {
        try sharedRecursiveSpendFixture(named: "archives.json")
    }

    private static func sharedRecursiveSpendFixture(named fileName: String) throws -> [String: Any] {
        var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        for _ in 0..<10 {
            let candidate = directory
                .appendingPathComponent("fixtures")
                .appendingPathComponent("kagemusha_recursive_spend_abi6")
                .appendingPathComponent(fileName)
            if FileManager.default.fileExists(atPath: candidate.path) {
                let data = try Data(contentsOf: candidate)
                return try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
            }
            directory.deleteLastPathComponent()
        }
        throw NSError(
            domain: "KagemushaRecursiveSpendProverTests",
            code: 1,
            userInfo: [NSLocalizedDescriptionKey: "missing shared recursive spend ABI-6 fixture \(fileName)"]
        )
    }
}
