import Foundation
import CryptoKit
import XCTest
@testable import IrohaSwift

final class KagemushaRecursiveSpendRequestCodecsTests: XCTestCase {
    func testDecodeVerifyResultReadsAbi6AndAbi7Fields() throws {
        let abi6 = try KagemushaRecursiveSpendRequestCodecs.decodeVerifyResult(
            Self.sharedRecursiveSpendArchive(abi: .abi6, name: "verify_result")
        )
        XCTAssertFalse(abi6.valid)
        XCTAssertEqual(abi6.hopCount, 2)
        XCTAssertEqual(abi6.encodedBytes, 4011)
        XCTAssertEqual(abi6.reason, "fixture recursive proof is not a production proof")
        XCTAssertFalse(abi6.chainAdmissible)
        XCTAssertEqual(abi6.chainAdmissionReason, "offline verification failed")
        XCTAssertFalse(abi6.witnesslessRedeemSupported)
        XCTAssertTrue(abi6.lineageWitnessRequired)

        let abi7 = try KagemushaRecursiveSpendRequestCodecs.decodeVerifyResult(
            Self.sharedRecursiveSpendArchive(abi: .abi7, name: "verify_result")
        )
        XCTAssertGreaterThanOrEqual(abi7.hopCount, 1)
        XCTAssertGreaterThan(abi7.encodedBytes, 0)
        XCTAssertEqual(abi7.chainAdmissionReason.isEmpty, abi7.chainAdmissible)
        XCTAssertEqual(!abi7.lineageWitnessRequired, abi7.witnesslessRedeemSupported)
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRequestCodecs.decodeVerifyResult(
                Self.recursiveSpendVerifyResultWithTrailingField()
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("verifyResult")
            )
        }
    }

    func testLineageWitnessRejectsTrailingFields() throws {
        let malformedWitnesses: [(Data, KagemushaRecursiveSpendRequestCodecError)] = [
            (try Self.recursiveSpendLineageWitnessWithTrailingField(), .invalidArchive("lineageWitness")),
            (
                try Self.recursiveSpendLineageWitnessWithTrailingPreviousProofsField(),
                .invalidArchive("lineageWitness.previousRecursiveProofs")
            ),
            (
                try Self.recursiveSpendLineageWitnessWithTrailingPreviousProofField(),
                .invalidArchive("lineageWitness.previousRecursiveProofs")
            ),
            (
                try Self.recursiveSpendLineageWitnessWithTrailingPreviousVerifierKeyIdField(),
                .invalidArchive("lineageWitness.previousRecursiveProofs.verifierKeyId")
            ),
            (
                try Self.recursiveSpendLineageWitnessWithPreviousProofField(
                    fieldIndex: 1,
                    replacement: Data()
                ),
                .invalidArchive("lineageWitness.previousRecursiveProofs.proof_public_inputs")
            ),
            (
                try Self.recursiveSpendLineageWitnessWithPreviousProofField(
                    fieldIndex: 2,
                    replacement: Data(repeating: 0, count: 32)
                ),
                .invalidArchive("lineageWitness.previousRecursiveProofs.proof_public_inputs_hash")
            ),
            (
                try Self.recursiveSpendLineageWitnessWithPreviousProofField(
                    fieldIndex: 2,
                    replacement: Data(repeating: 0x44, count: 32)
                ),
                .invalidArchive("lineageWitness.previousRecursiveProofs.proof_public_inputs_hash")
            ),
            (
                try Self.recursiveSpendLineageWitnessWithPreviousProofBoxBackend("halo2/kzg"),
                .invalidArchive("lineageWitness.previousRecursiveProofs.proof_backend")
            ),
            (
                try Self.recursiveSpendLineageWitnessWithPreviousProofBoxBackendAndEmptyProofBytes(
                    "halo2/kzg"
                ),
                .invalidArchive("lineageWitness.previousRecursiveProofs.proof_backend")
            ),
            (
                try Self.recursiveSpendLineageWitnessWithEmptyPreviousProofBytes(),
                .invalidArchive("lineageWitness.previousRecursiveProofs.proof_bytes")
            )
        ]
        for (archive, expectedError) in malformedWitnesses {
            XCTAssertThrowsError(
                try KagemushaRecursiveSpendRequestCodecs.lineageWitnessHasReservedPreviousProof(archive)
            ) { error in
                XCTAssertEqual(error as? KagemushaRecursiveSpendRequestCodecError, expectedError)
            }
        }
    }

    func testDecodeBundleExtractsLineageSummariesFromFixtureArchives() throws {
        let initBundle = try KagemushaRecursiveSpendRequestCodecs.decodeBundle(
            Self.sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle")
        )
        XCTAssertEqual(initBundle.hopCount, 1)
        XCTAssertEqual(
            initBundle.proofCircuitId,
            KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1
        )
        XCTAssertEqual(initBundle.chainId, "kagemusha-recursive-spend-abi-chain")
        XCTAssertEqual(initBundle.asset, "686w6ABhTWPaCrWNjjXs7X1SW6w9")
        let fallbackAssetBundle = try KagemushaRecursiveSpendRequestCodecs.decodeBundle(
            Self.recursiveSpendBundleWithAccumulatorField(
                fieldIndex: 2,
                replacement: Self.fixedArrayPayload(0x01, count: 16)
            )
        )
        XCTAssertEqual(fallbackAssetBundle.asset, "hex:01010101010101010101010101010101")
        XCTAssertTrue(initBundle.initialRoot.contains { $0 != 0 })
        XCTAssertTrue(initBundle.finalRoot.contains { $0 != 0 })
        XCTAssertEqual(initBundle.currentNote.amount, "7")
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveSpendAccumulatorDomain,
            "iroha:kagemusha:v1:recursive-spend-accumulator"
        )
        XCTAssertGreaterThanOrEqual(initBundle.topupAnchorNullifiers.count, 2)
        let malformedTopupAnchorSets: [[Data]] = [
            [],
            [Data(repeating: 0, count: 32)],
            [
                initBundle.topupAnchorNullifiers[0],
                initBundle.topupAnchorNullifiers[1],
                Data(repeating: 0x34, count: 32)
            ],
            [initBundle.topupAnchorNullifiers[0], initBundle.topupAnchorNullifiers[0]],
            [initBundle.topupAnchorNullifiers[1], initBundle.topupAnchorNullifiers[0]],
            [initBundle.currentNote.noteCommitment],
            [initBundle.currentNote.spendNullifier]
        ]
        for nullifiers in malformedTopupAnchorSets {
            XCTAssertThrowsError(
                try KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                    Self.recursiveSpendBundleWithTopupAnchorNullifiers(nullifiers)
                )
            ) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveSpendRequestCodecError,
                    .invalidArchive("bundle.accumulator.topup_anchor_nullifiers")
                )
            }
        }

        let appendBundle = try KagemushaRecursiveSpendRequestCodecs.decodeBundle(
            Self.sharedRecursiveSpendArchive(abi: .abi7, name: "append_bundle")
        )
        XCTAssertGreaterThanOrEqual(appendBundle.hopCount, initBundle.hopCount)
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
                appendBundle.proofCircuitId
            )
        )
        XCTAssertEqual(appendBundle.asset, "7Y5nGzchCJcxcv98NUoBfwBR1nTk")
        XCTAssertTrue(appendBundle.currentNote.noteCommitment.contains { $0 != 0 })
        XCTAssertTrue(appendBundle.currentNote.spendNullifier.contains { $0 != 0 })
        XCTAssertNotEqual(appendBundle.currentNote.amount, "0")
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                Self.recursiveSpendBundleWithProofCircuitId(
                    "kagemusha-recursive-spend-lineage-badhop-v1"
                )
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("bundle.proof_circuit_id")
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                Self.recursiveSpendBundleWithProofBackend("halo2/kzg")
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("bundle.proof_backend")
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                Self.recursiveSpendBundleWithProofBoxBackend("halo2/kzg")
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("bundle.proof_backend")
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                Self.recursiveSpendBundleWithProofBoxBackendAndEmptyProofBytes("halo2/kzg")
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("bundle.proof_backend")
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                Self.recursiveSpendBundleWithTrailingRecursiveProofField()
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("bundle")
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                Self.recursiveSpendBundleWithTrailingVerifierKeyIdField()
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("bundle")
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                Self.recursiveSpendBundleWithTrailingProofBoxField()
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("bundle")
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                Self.recursiveSpendBundleWithEmptyProofBytes()
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("bundle.proof_bytes")
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                Self.recursiveSpendBundleWithEmptyProofPublicInputs()
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("bundle.proof_public_inputs")
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                Self.recursiveSpendBundleWithZeroProofPublicInputsHash()
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("bundle.proof_public_inputs_hash")
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                Self.recursiveSpendBundleWithMismatchedProofPublicInputsHash()
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("bundle.proof_public_inputs_hash")
            )
        }
        let malformedCurrentNotes: [(Data, KagemushaRecursiveSpendRequestCodecError)] = [
            (
                try Self.recursiveSpendBundleWithCurrentNoteField(
                    fieldIndex: 0,
                    replacement: Data(repeating: 0, count: 32)
                ),
                .invalidField("noteCommitment")
            ),
            (
                try Self.recursiveSpendBundleWithCurrentNoteField(
                    fieldIndex: 1,
                    replacement: Data(repeating: 0, count: 32)
                ),
                .invalidField("spendNullifier")
            ),
            (
                try Self.recursiveSpendBundleWithEqualCurrentNoteNullifier(),
                .invalidField("spendNullifier")
            ),
            (
                try Self.recursiveSpendBundleWithCurrentNoteField(
                    fieldIndex: 2,
                    replacement: Self.zeroNumericPayload()
                ),
                .invalidField("amount")
            ),
            (
                try Self.recursiveSpendBundleWithCurrentNoteField(
                    fieldIndex: 0,
                    replacement: Self.fixedArrayPayload(0x04, count: 31)
                ),
                .invalidArchive("fixedArray")
            ),
            (
                try Self.recursiveSpendBundleWithCurrentNoteField(
                    fieldIndex: 0,
                    replacement: Self.fixedArrayPayload(0x04, count: 33)
                ),
                .invalidArchive("fixedArray")
            ),
            (
                try Self.recursiveSpendBundleWithCurrentNoteField(
                    fieldIndex: 0,
                    replacement: Self.countPrefixedFixedArrayPayload(0x04, count: 32)
                ),
                .invalidArchive("fixedArray")
            ),
            (
                try Self.recursiveSpendBundleWithCurrentNoteField(
                    fieldIndex: 1,
                    replacement: Self.fixedArrayPayload(0x05, count: 31)
                ),
                .invalidArchive("fixedArray")
            ),
            (
                try Self.recursiveSpendBundleWithCurrentNoteField(
                    fieldIndex: 1,
                    replacement: Self.fixedArrayPayload(0x05, count: 33)
                ),
                .invalidArchive("fixedArray")
            ),
            (
                try Self.recursiveSpendBundleWithCurrentNoteField(
                    fieldIndex: 1,
                    replacement: Self.countPrefixedFixedArrayPayload(0x05, count: 32)
                ),
                .invalidArchive("fixedArray")
            ),
            (
                try Self.recursiveSpendBundleWithCurrentNoteField(
                    fieldIndex: 2,
                    replacement: Self.numericPayload(Data([1]), scale: 1)
                ),
                .invalidField("numeric")
            ),
            (
                try Self.recursiveSpendBundleWithCurrentNoteField(
                    fieldIndex: 2,
                    replacement: Self.numericPayloadWithScalePayload(
                        Self.countPrefixedFixedArrayPayload(0x16, count: 4)
                    )
                ),
                .invalidArchive("field")
            ),
            (
                try Self.recursiveSpendBundleWithCurrentNoteField(
                    fieldIndex: 2,
                    replacement: Self.numericPayloadWithMantissaPayload(Data([2, 0, 0, 0, 1]))
                ),
                .invalidArchive("truncated")
            ),
            (
                try Self.recursiveSpendBundleWithCurrentNoteField(
                    fieldIndex: 2,
                    replacement: Self.numericPayload(Data([0xff]))
                ),
                .invalidField("amount")
            ),
            (
                try Self.recursiveSpendBundleWithCurrentNoteField(
                    fieldIndex: 2,
                    replacement: Self.numericPayload(Data(repeating: 0, count: 16) + Data([1]))
                ),
                .invalidField("amount")
            ),
            (
                try Self.recursiveSpendBundleWithCurrentNoteField(
                    fieldIndex: 2,
                    replacement: Self.numericPayloadWithTrailingField()
                ),
                .invalidArchive("field")
            ),
        ]
        for (archive, expectedError) in malformedCurrentNotes {
            XCTAssertThrowsError(
                try KagemushaRecursiveSpendRequestCodecs.decodeBundle(archive)
            ) { error in
                XCTAssertEqual(error as? KagemushaRecursiveSpendRequestCodecError, expectedError)
            }
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                Self.recursiveSpendBundleWithTrailingBundleField()
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("bundle")
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                Self.recursiveSpendBundleWithTrailingCurrentNoteField()
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("field")
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                Self.recursiveSpendBundleWithAccumulatorField(
                    fieldIndex: 0,
                    replacement: Self.noritoString(
                        "iroha:kagemusha:v1:recursive-spend-accumulator-digest",
                        flags: NoritoHeader.compactLen
                    )
                )
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("bundle.accumulator.domain")
            )
        }
        let malformedAccumulatorFields: [(Int, Data, KagemushaRecursiveSpendRequestCodecError)] = [
            (0, Self.noritoString(" iroha:kagemusha:v1:recursive-spend-accumulator", flags: NoritoHeader.compactLen), .invalidArchive("bundle.accumulator.domain")),
            (0, Self.noritoString("iroha:Kagemusha:v1:recursive-spend-accumulator", flags: NoritoHeader.compactLen), .invalidArchive("bundle.accumulator.domain")),
            (1, Self.noritoString("kagemusha-recursive-spend-abi-chain", flags: NoritoHeader.compactLen), .invalidArchive("bundle.accumulator.chain_id")),
            (1, Self.accumulatorChainIdPayload(""), .invalidField("bundle.accumulator.chain_id")),
            (1, Self.accumulatorChainIdPayload(" kagemusha-recursive-spend-abi-chain"), .invalidField("bundle.accumulator.chain_id")),
            (1, Self.accumulatorChainIdPayload("kagemusha-recursive-spend-abi-chain "), .invalidField("bundle.accumulator.chain_id")),
            (1, Self.accumulatorChainIdPayload("kagemusha recursive-spend-abi-chain"), .invalidField("bundle.accumulator.chain_id")),
            (3, Data(repeating: 0, count: 32), .invalidArchive("bundle.accumulator.initial_root")),
            (4, Data(repeating: 0, count: 32), .invalidArchive("bundle.accumulator.final_root")),
            (4, initBundle.initialRoot, .invalidArchive("bundle.accumulator.final_root")),
            (2, Self.encodeFields(Array(repeating: Data([0x01]), count: 15), flags: NoritoHeader.compactLen), .invalidArchive("fixedArray")),
            (2, Self.countPrefixedFixedArrayPayload(0x01, count: 16), .invalidArchive("fixedArray")),
            (2, Self.encodeFields(Array(repeating: Data([0x01]), count: 17), flags: NoritoHeader.compactLen), .invalidArchive("fixedArray")),
            (3, Self.encodeFields(Array(repeating: Data([0x02]), count: 31), flags: NoritoHeader.compactLen), .invalidArchive("fixedArray")),
            (3, Self.countPrefixedFixedArrayPayload(0x02, count: 32), .invalidArchive("fixedArray")),
            (3, Self.encodeFields(Array(repeating: Data([0x02]), count: 33), flags: NoritoHeader.compactLen), .invalidArchive("fixedArray")),
            (4, Self.encodeFields(Array(repeating: Data([0x03]), count: 31), flags: NoritoHeader.compactLen), .invalidArchive("fixedArray")),
            (4, Self.countPrefixedFixedArrayPayload(0x03, count: 32), .invalidArchive("fixedArray")),
            (4, Self.encodeFields(Array(repeating: Data([0x03]), count: 33), flags: NoritoHeader.compactLen), .invalidArchive("fixedArray")),
            (6, Data([0, 0, 0, 0]), .invalidArchive("bundle.accumulator.hop_count")),
            (6, Self.countPrefixedFixedArrayPayload(0x06, count: 4), .invalidArchive("bundle.accumulator.hop_count")),
            (6, Data([65, 0, 0, 0]), .invalidArchive("bundle.accumulator.hop_count")),
            (7, Data(repeating: 0, count: 32), .invalidArchive("bundle.accumulator.lineage_digest")),
            (7, Self.fixedArrayPayload(0x07, count: 31), .invalidArchive("fixedArray")),
            (7, Self.countPrefixedFixedArrayPayload(0x07, count: 32), .invalidArchive("fixedArray")),
            (7, Self.fixedArrayPayload(0x07, count: 33), .invalidArchive("fixedArray")),
            (8, Data(repeating: 0x7d, count: 32), .invalidArchive("bundle.accumulator.aggregation_transcript_digest")),
            (8, Data(repeating: 0, count: 32), .invalidArchive("bundle.accumulator.aggregation_transcript_digest")),
            (9, Data(repeating: 0, count: 32), .invalidArchive("bundle.accumulator.nullifier_digest")),
            (10, Data(repeating: 0, count: 32), .invalidArchive("bundle.accumulator.output_commitment_digest")),
            (11, Data(repeating: 0, count: 32), .invalidArchive("bundle.accumulator.fold_digest")),
            (12, Data(repeating: 0, count: 32), .invalidArchive("bundle.accumulator.recursive_proof_chain_digest")),
            (13, Data(repeating: 0, count: 32), .invalidArchive("bundle.accumulator.transition_profile_binding_digest")),
            (14, Data(repeating: 0x7e, count: 32), .invalidArchive("bundle.accumulator.append_opening_preflight_digest")),
            (14, Self.fixedArrayPayload(0x0e, count: 31), .invalidArchive("fixedArray")),
            (14, Self.countPrefixedFixedArrayPayload(0x0e, count: 32), .invalidArchive("fixedArray")),
            (14, Self.fixedArrayPayload(0x0e, count: 33), .invalidArchive("fixedArray")),
            (15, Data(repeating: 0x7f, count: 32), .invalidArchive("bundle.accumulator.append_boundary_digest")),
            (16, Data(repeating: 0, count: 32), .invalidArchive("bundle.accumulator.verifier_params_fingerprint")),
            (17, Data(repeating: 0, count: 32), .invalidArchive("bundle.accumulator.fixed_window_table_schedule_digest")),
            (18, Data(repeating: 0, count: 32), .invalidArchive("bundle.accumulator.fixed_window_shared_table_manifest_digest")),
            (19, Data(repeating: 0, count: 32), .invalidArchive("bundle.accumulator.fixed_window_table_base_digest")),
            (20, Data(repeating: 0, count: 32), .invalidArchive("bundle.accumulator.verifier_witness_batch_digest")),
            (20, Self.fixedArrayPayload(0x14, count: 31), .invalidArchive("fixedArray")),
            (20, Self.countPrefixedFixedArrayPayload(0x14, count: 32), .invalidArchive("fixedArray")),
            (20, Self.fixedArrayPayload(0x14, count: 33), .invalidArchive("fixedArray")),
            (21, Data([3, 0, 0, 0]), .invalidArchive("bundle.accumulator.verifier_opening_len")),
            (21, Self.countPrefixedFixedArrayPayload(0x15, count: 4), .invalidArchive("bundle.accumulator.verifier_opening_len")),
        ]
        for (fieldIndex, replacement, expectedError) in malformedAccumulatorFields {
            XCTAssertThrowsError(
                try KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                    Self.recursiveSpendBundleWithAccumulatorField(
                        fieldIndex: fieldIndex,
                        replacement: replacement
                    )
                )
            ) { error in
                XCTAssertEqual(error as? KagemushaRecursiveSpendRequestCodecError, expectedError)
            }
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRequestCodecs.decodeBundle(
                Self.recursiveSpendBundleWithTrailingAccumulatorField()
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("bundle")
            )
        }
    }

    func testTypedEncodersWriteExpectedRequestSchemasAndLayouts() throws {
        let recordBundle = Self.syntheticRecordBundleArchive()
        let pallasOpenEnvelopes = Self.syntheticPallasOpenEnvelopesArchive()
        let (lineageVerifierKey, lineageProvingKeyArchive) = try Self.sharedInitLineageKeyMaterial()
        let note = try Self.sampleNote()

        let initArchive = try KagemushaRecursiveSpendRequestCodecs.encodeInitRequest(
            KagemushaRecursiveSpendInitRequest(
                recordBundle: recordBundle,
                pallasOpenEnvelopes: pallasOpenEnvelopes,
                currentNote: note,
                lineageVerifierKey: lineageVerifierKey,
                lineageProvingKeyArchive: lineageProvingKeyArchive,
                blockHeight: 7
            )
        )
        try Self.assertArchiveSchema(
            initArchive,
            KagemushaRecursiveSpendRequestCodecs.initRequestWireName
        )

        let initFields = try Self.requestFields(
            initArchive,
            schema: KagemushaRecursiveSpendRequestCodecs.initRequestWireName
        )
        XCTAssertEqual(initFields.count, 6)
        XCTAssertEqual(
            try Self.compactPayload(
                recordBundle,
                schema: KagemushaRecursiveSpendRequestCodecs.recordBundleWireName
            ),
            initFields[0]
        )
        XCTAssertEqual(pallasOpenEnvelopes, try Self.readBytesVecPayload(initFields[1]))

        let noteFields = try Self.fieldPayloads(initFields[2])
        XCTAssertEqual(noteFields.count, 3)
        XCTAssertEqual(note.noteCommitment, try Self.readFixedArrayPayload(noteFields[0], expectedSize: 32))
        XCTAssertEqual(note.spendNullifier, try Self.readFixedArrayPayload(noteFields[1], expectedSize: 32))
        XCTAssertEqual(noteFields[0].count, 64)
        XCTAssertEqual(noteFields[1].count, 64)

        let lineageKeyFields = try Self.fieldPayloads(Self.optionSomePayload(initFields[3]))
        XCTAssertEqual(lineageKeyFields.count, 2)
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
            try Self.readStringPayload(lineageKeyFields[0])
        )
        XCTAssertEqual(lineageVerifierKey, try Self.readBytesVecPayload(lineageKeyFields[1]))
        XCTAssertEqual(lineageProvingKeyArchive, try Self.readBytesVecPayload(Self.optionSomePayload(initFields[4])))
        XCTAssertEqual(UInt64(7), try Self.readUInt64Payload(Self.optionSomePayload(initFields[5])))

        try Self.assertArchiveSchema(
            KagemushaRecursiveSpendRequestCodecs.encodeAppendRequest(
                KagemushaRecursiveSpendAppendRequest(
                    previousBundle: Self.sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
                    recordBundle: recordBundle,
                    pallasOpenEnvelopes: pallasOpenEnvelopes,
                    currentNote: try Self.sampleNote(seed: 0x31),
                    outputProofCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                    previousLineageVerifierRecord: try Self.sampleVerifierRecord(),
                    blockHeight: 8
                )
            ),
            KagemushaRecursiveSpendRequestCodecs.appendRequestWireName
        )
        let redeemBundle = try Self.sharedRecursiveSpendArchive(abi: .abi7, name: "append_bundle")
        let redeemProof = Self.syntheticArchive(
            schema: KagemushaRecursiveSpendRequestCodecs.proofAttachmentWireName
        )
        let lineageWitness = try Self.sharedRecursiveSpendArchive(
            abi: .abi6,
            name: "lineage_witness_append_result"
        )
        let lineageVerifierRecord = try Self.sampleVerifierRecord()
        let changeOutput = Data((0..<32).map { UInt8(0x80 + $0) })
        let redeemArchive = try KagemushaRecursiveSpendRequestCodecs.encodeRedeemRequest(
            KagemushaRecursiveSpendRedeemRequest(
                bundle: redeemBundle,
                recipient: try Self.sampleRecipient(),
                publicAmount: "6",
                redeemProof: redeemProof,
                lineageWitness: lineageWitness,
                changeOutput: changeOutput,
                lineageVerifierRecord: lineageVerifierRecord,
                blockHeight: 10
            )
        )
        try Self.assertArchiveSchema(
            redeemArchive,
            KagemushaRecursiveSpendRequestCodecs.redeemRequestWireName
        )
        let redeemFields = try Self.requestFields(
            redeemArchive,
            schema: KagemushaRecursiveSpendRequestCodecs.redeemRequestWireName
        )
        XCTAssertEqual(redeemFields.count, 8)
        XCTAssertEqual(
            try Self.compactPayload(redeemBundle, schema: KagemushaRecursiveSpendRequestCodecs.bundleWireName),
            redeemFields[0]
        )
        XCTAssertEqual(
            try Self.compactPayload(redeemProof, schema: KagemushaRecursiveSpendRequestCodecs.proofAttachmentWireName),
            redeemFields[3]
        )
        XCTAssertEqual(
            try Self.compactPayload(lineageWitness, schema: KagemushaRecursiveSpendRequestCodecs.lineageWitnessWireName),
            try Self.optionSomePayload(redeemFields[4])
        )
        XCTAssertEqual(changeOutput, try Self.readFixedArrayPayload(Self.optionSomePayload(redeemFields[5]), expectedSize: 32))
        XCTAssertEqual(
            try Self.compactPayload(
                lineageVerifierRecord.recordBytes,
                schema: KagemushaRecursiveSpendRequestCodecs.verifyingKeyRecordWireName
            ),
            try Self.optionSomePayload(redeemFields[6])
        )
        XCTAssertEqual(UInt64(10), try Self.readUInt64Payload(Self.optionSomePayload(redeemFields[7])))

        let exactRedeemFields = try Self.requestFields(
            KagemushaRecursiveSpendRequestCodecs.encodeRedeemRequest(
                KagemushaRecursiveSpendRedeemRequest(
                    bundle: Self.sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
                    recipient: try Self.sampleRecipient(),
                    publicAmount: "7",
                    redeemProof: redeemProof,
                    lineageVerifierRecord: lineageVerifierRecord
                )
            ),
            schema: KagemushaRecursiveSpendRequestCodecs.redeemRequestWireName
        )
        XCTAssertEqual(exactRedeemFields.count, 8)
        try Self.assertOptionNone(exactRedeemFields[4])
        try Self.assertOptionNone(exactRedeemFields[5])
        XCTAssertEqual(
            try Self.compactPayload(
                lineageVerifierRecord.recordBytes,
                schema: KagemushaRecursiveSpendRequestCodecs.verifyingKeyRecordWireName
            ),
            try Self.optionSomePayload(exactRedeemFields[6])
        )
        try Self.assertOptionNone(exactRedeemFields[7])

        let verifyFields = try Self.requestFields(
            KagemushaRecursiveSpendRequestCodecs.encodeVerifyRequest(
                try KagemushaRecursiveSpendVerifyRequest(
                    bundle: Self.sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
                    lineageVerifierRecord: lineageVerifierRecord
                )
            ),
            schema: KagemushaRecursiveSpendRequestCodecs.verifyRequestWireName
        )
        XCTAssertEqual(verifyFields.count, 3)
        XCTAssertEqual(
            try Self.compactPayload(
                Self.sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
                schema: KagemushaRecursiveSpendRequestCodecs.bundleWireName
            ),
            verifyFields[0]
        )
        XCTAssertEqual(
            try Self.compactPayload(
                lineageVerifierRecord.recordBytes,
                schema: KagemushaRecursiveSpendRequestCodecs.verifyingKeyRecordWireName
            ),
            try Self.optionSomePayload(verifyFields[1])
        )
        try Self.assertOptionNone(verifyFields[2])
    }

    func testTypedRequestsRejectMalformedInputsBeforeNativeDispatch() throws {
        for amount in [
            "",
            "0",
            "00",
            "01",
            "0007",
            "-1",
            "+1",
            "1.0",
            "1e3",
            "7 ",
            " 7",
            "\t7",
            "7\n",
            Self.u128MaxPlusOne,
            Self.u128TooManyDigits
        ] {
            XCTAssertThrowsError(
                try KagemushaRecursiveSpendableNoteDescriptor(
                    noteCommitment: Data(repeating: 4, count: 32),
                    spendNullifier: Data(repeating: 5, count: 32),
                    amount: amount
                )
            )
            XCTAssertThrowsError(
                try KagemushaRecursiveSpendRedeemRequest(
                    bundle: Self.sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
                    recipient: Self.sampleRecipient(),
                    publicAmount: amount,
                    redeemProof: Self.syntheticArchive(
                        schema: KagemushaRecursiveSpendRequestCodecs.proofAttachmentWireName
                    )
                )
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendableNoteDescriptor(
                noteCommitment: Data(repeating: 1, count: 31),
                spendNullifier: Data(repeating: 2, count: 32),
                amount: "1"
            )
        )
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendableNoteDescriptor(
                noteCommitment: Data(repeating: 3, count: 32),
                spendNullifier: Data(repeating: 3, count: 32),
                amount: "1"
            )
        )
        func assertRedeemRequestInvalidField(
            _ expectedField: String,
            _ makeRequest: () throws -> KagemushaRecursiveSpendRedeemRequest
        ) {
            XCTAssertThrowsError(try makeRequest()) { error in
                guard case let KagemushaRecursiveSpendRequestCodecError.invalidField(field) = error else {
                    XCTFail("Expected invalidField(\(expectedField)), got \(error)")
                    return
                }
                XCTAssertEqual(field, expectedField)
            }
        }
        for changeOutput in [Data(repeating: 1, count: 31), Data(repeating: 0, count: 32)] {
            assertRedeemRequestInvalidField("changeOutput") {
                try KagemushaRecursiveSpendRedeemRequest(
                    bundle: Self.sharedRecursiveSpendArchive(abi: .abi7, name: "append_bundle"),
                    recipient: Self.sampleRecipient(),
                    publicAmount: "7",
                    redeemProof: Self.syntheticArchive(
                        schema: KagemushaRecursiveSpendRequestCodecs.proofAttachmentWireName
                    ),
                    changeOutput: changeOutput
                )
            }
        }
        let partialBundle = try Self.sharedRecursiveSpendArchive(abi: .abi7, name: "append_bundle")
        let partialSummary = try KagemushaRecursiveSpendRequestCodecs.decodeBundle(partialBundle)
        XCTAssertFalse(partialSummary.topupAnchorNullifiers.isEmpty)
        for changeOutput in [
            partialSummary.currentNote.noteCommitment,
            partialSummary.currentNote.spendNullifier,
            partialSummary.topupAnchorNullifiers[0]
        ] {
            assertRedeemRequestInvalidField("changeOutput") {
                try KagemushaRecursiveSpendRedeemRequest(
                    bundle: partialBundle,
                    recipient: Self.sampleRecipient(),
                    publicAmount: "6",
                    redeemProof: Self.syntheticArchive(
                        schema: KagemushaRecursiveSpendRequestCodecs.proofAttachmentWireName
                    ),
                    changeOutput: changeOutput
                )
            }
        }
        assertRedeemRequestInvalidField("changeOutput") {
            try KagemushaRecursiveSpendRedeemRequest(
                bundle: Self.sharedRecursiveSpendArchive(abi: .abi7, name: "append_bundle"),
                recipient: Self.sampleRecipient(),
                publicAmount: "6",
                redeemProof: Self.syntheticArchive(
                    schema: KagemushaRecursiveSpendRequestCodecs.proofAttachmentWireName
                )
            )
        }
        assertRedeemRequestInvalidField("lineageWitness") {
            try KagemushaRecursiveSpendRedeemRequest(
                bundle: Self.sharedRecursiveSpendArchive(abi: .abi7, name: "append_bundle"),
                recipient: Self.sampleRecipient(),
                publicAmount: "7",
                redeemProof: Self.syntheticArchive(
                    schema: KagemushaRecursiveSpendRequestCodecs.proofAttachmentWireName
                )
            )
        }
        assertRedeemRequestInvalidField("lineageVerifierRecord") {
            try KagemushaRecursiveSpendRedeemRequest(
                bundle: Self.sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
                recipient: Self.sampleRecipient(),
                publicAmount: "7",
                redeemProof: Self.syntheticArchive(
                    schema: KagemushaRecursiveSpendRequestCodecs.proofAttachmentWireName
                )
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRedeemRequest(
                bundle: Self.sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
                recipient: Self.sampleRecipient(),
                publicAmount: "7",
                redeemProof: Self.syntheticArchive(
                    schema: KagemushaRecursiveSpendRequestCodecs.proofAttachmentWireName
                ),
                lineageWitness: Self.syntheticArchive(
                    schema: KagemushaRecursiveSpendRequestCodecs.lineageWitnessWireName
                ),
                lineageVerifierRecord: try Self.sampleVerifierRecord()
            )
        ) { error in
            guard case let KagemushaRecursiveSpendRequestCodecError.invalidArchive(field) = error else {
                XCTFail("Expected invalidArchive(lineageWitness), got \(error)")
                return
            }
            XCTAssertFalse(field.isEmpty)
        }
        assertRedeemRequestInvalidField("lineageVerifierRecord") {
            try KagemushaRecursiveSpendRedeemRequest(
                bundle: Self.sharedRecursiveSpendArchive(abi: .abi7, name: "append_bundle"),
                recipient: Self.sampleRecipient(),
                publicAmount: "7",
                redeemProof: Self.syntheticArchive(
                    schema: KagemushaRecursiveSpendRequestCodecs.proofAttachmentWireName
                ),
                lineageVerifierRecord: try Self.sampleVerifierRecord()
            )
        }
        assertRedeemRequestInvalidField("lineageVerifierRecord") {
            try KagemushaRecursiveSpendRedeemRequest(
                bundle: Self.sharedRecursiveSpendArchive(abi: .abi7, name: "append_bundle"),
                recipient: Self.sampleRecipient(),
                publicAmount: "7",
                redeemProof: Self.syntheticArchive(
                    schema: KagemushaRecursiveSpendRequestCodecs.proofAttachmentWireName
                ),
                lineageWitness: Self.sharedRecursiveSpendArchive(
                    abi: .abi6,
                    name: "lineage_witness_append_result"
                )
            )
        }
        assertRedeemRequestInvalidField("lineageVerifierRecord") {
            try KagemushaRecursiveSpendRedeemRequest(
                bundle: Self.sharedRecursiveSpendArchive(abi: .abi7, name: "append_bundle"),
                recipient: Self.sampleRecipient(),
                publicAmount: "7",
                redeemProof: Self.syntheticArchive(
                    schema: KagemushaRecursiveSpendRequestCodecs.proofAttachmentWireName
                ),
                lineageWitness: Self.sharedRecursiveSpendArchive(
                    abi: .abi6,
                    name: "lineage_witness_from_init_result"
                ),
                lineageVerifierRecord: try Self.sampleVerifierRecord()
            )
        }
        assertRedeemRequestInvalidField("publicAmount") {
            try KagemushaRecursiveSpendRedeemRequest(
                bundle: Self.sharedRecursiveSpendArchive(abi: .abi7, name: "append_bundle"),
                recipient: Self.sampleRecipient(),
                publicAmount: "8",
                redeemProof: Self.syntheticArchive(
                    schema: KagemushaRecursiveSpendRequestCodecs.proofAttachmentWireName
                )
            )
        }
        assertRedeemRequestInvalidField("publicAmount") {
            try KagemushaRecursiveSpendRedeemRequest(
                bundle: Self.sharedRecursiveSpendArchive(abi: .abi7, name: "append_bundle"),
                recipient: Self.sampleRecipient(),
                publicAmount: "7",
                redeemProof: Self.syntheticArchive(
                    schema: KagemushaRecursiveSpendRequestCodecs.proofAttachmentWireName
                ),
                changeOutput: Data(repeating: 0x42, count: 32)
            )
        }
        assertRedeemRequestInvalidField("publicAmount") {
            try KagemushaRecursiveSpendRedeemRequest(
                bundle: Self.sharedRecursiveSpendArchive(abi: .abi7, name: "append_bundle"),
                recipient: Self.sampleRecipient(),
                publicAmount: "8",
                redeemProof: Self.syntheticArchive(
                    schema: KagemushaRecursiveSpendRequestCodecs.proofAttachmentWireName
                ),
                changeOutput: Data(repeating: 0x43, count: 32)
            )
        }
        func assertVerifyRequestInvalidField(
            _ expectedField: String,
            _ makeRequest: () throws -> KagemushaRecursiveSpendVerifyRequest
        ) {
            XCTAssertThrowsError(try makeRequest()) { error in
                guard case let KagemushaRecursiveSpendRequestCodecError.invalidField(field) = error else {
                    XCTFail("Expected invalidField(\(expectedField)), got \(error)")
                    return
                }
                XCTAssertEqual(field, expectedField)
            }
        }
        assertVerifyRequestInvalidField(
            "lineageVerifierRecord",
            {
                try KagemushaRecursiveSpendVerifyRequest(
                    bundle: Self.sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle")
                )
            }
        )
        assertVerifyRequestInvalidField(
            "lineageVerifierRecord",
            {
                try KagemushaRecursiveSpendVerifyRequest(
                    bundle: Self.sharedRecursiveSpendArchive(abi: .abi7, name: "append_bundle"),
                    lineageVerifierRecord: try Self.sampleVerifierRecord()
                )
            }
        )
        let recordBundle = Self.syntheticRecordBundleArchive()
        let pallasOpenEnvelopes = Self.syntheticPallasOpenEnvelopesArchive()
        let (lineageVerifierKey, lineageProvingKeyArchive) = try Self.sharedInitLineageKeyMaterial()
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendInitRequest(
                recordBundle: recordBundle,
                pallasOpenEnvelopes: pallasOpenEnvelopes,
                currentNote: Self.sampleNote(),
                lineageVerifierKey: nil,
                lineageProvingKeyArchive: nil
            )
        )

        XCTAssertThrowsError(
            try KagemushaRecursiveSpendInitRequest(
                recordBundle: recordBundle,
                pallasOpenEnvelopes: pallasOpenEnvelopes,
                currentNote: Self.sampleNote(),
                lineageVerifierKey: Data(repeating: 0x5a, count: 64),
                lineageProvingKeyArchive: lineageProvingKeyArchive
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidField("lineageVerifierKey")
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendInitRequest(
                recordBundle: recordBundle,
                pallasOpenEnvelopes: pallasOpenEnvelopes,
                currentNote: Self.sampleNote(),
                lineageVerifierKey: lineageVerifierKey,
                lineageProvingKeyArchive: Self.syntheticArchive(schema: "test.LineageProvingKeyArchive")
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidField("lineageProvingKeyArchive")
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendAppendRequest(
                previousBundle: Self.sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
                recordBundle: recordBundle,
                pallasOpenEnvelopes: pallasOpenEnvelopes,
                currentNote: Self.sampleNote(seed: 0x41),
                outputProofCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousLineageVerifierRecord: Self.sampleVerifierRecord(),
                lineageVerifierKey: lineageVerifierKey,
                lineageProvingKeyArchive: lineageProvingKeyArchive
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidField("lineageKeyArtifacts")
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendAppendRequest(
                previousBundle: Self.sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
                recordBundle: recordBundle,
                pallasOpenEnvelopes: pallasOpenEnvelopes,
                currentNote: Self.sampleNote(seed: 0x44),
                outputProofCircuitId: "kagemusha-recursive-spend-invalid-output-v1",
                previousLineageVerifierRecord: Self.sampleVerifierRecord(),
                lineageVerifierKey: lineageVerifierKey,
                lineageProvingKeyArchive: lineageProvingKeyArchive
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidField("outputProofCircuitId")
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendAppendRequest(
                previousBundle: Self.sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
                recordBundle: recordBundle,
                pallasOpenEnvelopes: pallasOpenEnvelopes,
                currentNote: Self.sampleNote(seed: 0x47),
                outputProofCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousProofOpenEnvelopes: Self.syntheticPallasOpenEnvelopesArchive()
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidField("previousLineageVerifierRecord")
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendAppendRequest(
                previousBundle: Self.sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
                recordBundle: recordBundle,
                pallasOpenEnvelopes: pallasOpenEnvelopes,
                currentNote: Self.sampleNote(seed: 0x42),
                outputProofCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousLineageVerifierRecord: Self.sampleVerifierRecord(),
                previousProofOpenEnvelopes: Self.syntheticPallasOpenEnvelopesArchive()
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidField("previousProofOpenEnvelopes")
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendAppendRequest(
                previousBundle: Self.sharedRecursiveSpendArchive(abi: .abi7, name: "append_bundle"),
                recordBundle: recordBundle,
                pallasOpenEnvelopes: pallasOpenEnvelopes,
                currentNote: Self.sampleNote(seed: 0x43),
                outputProofCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousLineageVerifierRecord: Self.sampleVerifierRecord()
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidField("previousLineageVerifierRecord")
            )
        }

        XCTAssertThrowsError(
            try KagemushaRecursiveSpendInitRequest(
                recordBundle: recordBundle,
                pallasOpenEnvelopes: Self.syntheticArchive(schema: "test.PallasOpenEnvelopes"),
                currentNote: Self.sampleNote(),
                lineageVerifierKey: lineageVerifierKey,
                lineageProvingKeyArchive: lineageProvingKeyArchive
            )
        )
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendInitRequest(
                recordBundle: recordBundle,
                pallasOpenEnvelopes: Self.syntheticPallasOpenEnvelopesArchive(count: 2),
                currentNote: Self.sampleNote(),
                lineageVerifierKey: lineageVerifierKey,
                lineageProvingKeyArchive: lineageProvingKeyArchive
            )
        )
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendInitRequest(
                recordBundle: recordBundle,
                pallasOpenEnvelopes: Self.syntheticPallasOpenEnvelopesArchive(
                    includeDomainTag: false
                ),
                currentNote: Self.sampleNote(),
                lineageVerifierKey: lineageVerifierKey,
                lineageProvingKeyArchive: lineageProvingKeyArchive
            )
        )
        for transcriptLabel in ["", String(repeating: "\u{00e9}", count: 65)] {
            XCTAssertThrowsError(
                try KagemushaRecursiveSpendInitRequest(
                    recordBundle: recordBundle,
                    pallasOpenEnvelopes: Self.syntheticPallasOpenEnvelopesArchive(
                        transcriptLabel: transcriptLabel
                    ),
                    currentNote: Self.sampleNote(),
                    lineageVerifierKey: lineageVerifierKey,
                    lineageProvingKeyArchive: lineageProvingKeyArchive
                )
            ) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveSpendRequestCodecError,
                    .invalidArchive("pallasOpenEnvelopes")
                )
            }
        }
        let malformedPallasMetadataArchives: [(String, Data)] = [
            (
                "vk_commitment",
                Self.syntheticPallasOpenEnvelopesArchive(
                    vkCommitmentPayload: Self.fixedArrayPayload(0x70, count: 32)
                )
            ),
            (
                "vk_commitment",
                Self.syntheticPallasOpenEnvelopesArchive(
                    vkCommitmentOptionPayload: Self.requiredOptionPayloadWithTrailingByte(Self.fixed32(0x70))
                )
            ),
            (
                "vk_commitment",
                Self.syntheticPallasOpenEnvelopesArchive(
                    vkCommitmentOptionPayload: Self.requiredOptionPayloadWithUnknownTag()
                )
            ),
            (
                "vk_commitment",
                Self.syntheticPallasOpenEnvelopesArchive(
                    vkCommitmentOptionPayload: Self.requiredOptionPayloadWithDeclaredLengthTooLong(Self.fixed32(0x70))
                )
            ),
            (
                "public_inputs_schema_hash",
                Self.syntheticPallasOpenEnvelopesArchive(
                    publicInputsSchemaHashPayload: Self.fixedArrayPayload(0x71, count: 32)
                )
            ),
            (
                "public_inputs_schema_hash",
                Self.syntheticPallasOpenEnvelopesArchive(
                    publicInputsSchemaHashOptionPayload: Self.requiredOptionPayloadWithTrailingByte(Self.fixed32(0x71))
                )
            ),
            (
                "public_inputs_schema_hash",
                Self.syntheticPallasOpenEnvelopesArchive(
                    publicInputsSchemaHashOptionPayload: Self.requiredOptionPayloadWithUnknownTag()
                )
            ),
            (
                "public_inputs_schema_hash",
                Self.syntheticPallasOpenEnvelopesArchive(
                    publicInputsSchemaHashOptionPayload: Self.requiredOptionPayloadWithDeclaredLengthTooLong(Self.fixed32(0x71))
                )
            ),
            (
                "domain_tag",
                Self.syntheticPallasOpenEnvelopesArchive(
                    domainTagPayload: Self.fixedArrayPayload(0x72, count: 32)
                )
            ),
            (
                "domain_tag",
                Self.syntheticPallasOpenEnvelopesArchive(
                    domainTagOptionPayload: Self.requiredOptionPayloadWithTrailingByte(Self.fixed32(0x72))
                )
            ),
            (
                "domain_tag",
                Self.syntheticPallasOpenEnvelopesArchive(
                    domainTagOptionPayload: Self.requiredOptionPayloadWithUnknownTag()
                )
            ),
            (
                "domain_tag",
                Self.syntheticPallasOpenEnvelopesArchive(
                    domainTagOptionPayload: Self.requiredOptionPayloadWithDeclaredLengthTooLong(Self.fixed32(0x72))
                )
            )
        ]
        for (metadataField, archive) in malformedPallasMetadataArchives {
            XCTAssertThrowsError(
                try KagemushaRecursiveSpendInitRequest(
                    recordBundle: recordBundle,
                    pallasOpenEnvelopes: archive,
                    currentNote: Self.sampleNote(),
                    lineageVerifierKey: lineageVerifierKey,
                    lineageProvingKeyArchive: lineageProvingKeyArchive
                ),
                metadataField
            ) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveSpendRequestCodecError,
                    .invalidArchive("pallasOpenEnvelopes.\(metadataField)"),
                    metadataField
                )
            }
        }

        var corrupted = Self.syntheticPallasOpenEnvelopesArchive()
        corrupted[corrupted.count - 1] ^= 0x01
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendInitRequest(
                recordBundle: recordBundle,
                pallasOpenEnvelopes: corrupted,
                currentNote: Self.sampleNote(),
                lineageVerifierKey: lineageVerifierKey,
                lineageProvingKeyArchive: lineageProvingKeyArchive
            )
        )
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendVerifyRequest(
                bundle: Self.sharedRecursiveSpendArchive(abi: .abi6, name: "verify_result")
            )
        )

        var tamperedBundle = try Self.sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle")
        tamperedBundle[tamperedBundle.count - 1] ^= 0x01
        XCTAssertThrowsError(try KagemushaRecursiveSpendRequestCodecs.decodeBundle(tamperedBundle))

        XCTAssertThrowsError(
            try KagemushaRecursiveSpendAppendRequest(
                previousBundle: Self.sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
                recordBundle: recordBundle,
                pallasOpenEnvelopes: pallasOpenEnvelopes,
                currentNote: Self.sampleNote(seed: 0x41),
                outputProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1,
                previousLineageVerifierRecord: Self.sampleVerifierRecord(),
                previousProofOpenEnvelopes: nil,
                lineageVerifierKey: Data(repeating: 0x6b, count: 64),
                lineageProvingKeyArchive: Self.syntheticArchive(schema: "test.LineageProvingKeyArchive")
            )
        )
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendAppendRequest(
                previousBundle: Self.sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
                recordBundle: recordBundle,
                pallasOpenEnvelopes: pallasOpenEnvelopes,
                currentNote: Self.sampleNote(seed: 0x41),
                outputProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1,
                previousLineageVerifierRecord: Self.sampleVerifierRecord(),
                previousProofOpenEnvelopes: Self.syntheticPallasOpenEnvelopesArchive(count: 2),
                lineageVerifierKey: Data(repeating: 0x6b, count: 64),
                lineageProvingKeyArchive: Self.syntheticArchive(schema: "test.LineageProvingKeyArchive")
            )
        )
        for transcriptLabel in ["", String(repeating: "\u{00e9}", count: 65)] {
            XCTAssertThrowsError(
                try KagemushaRecursiveSpendAppendRequest(
                    previousBundle: Self.sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
                    recordBundle: recordBundle,
                    pallasOpenEnvelopes: pallasOpenEnvelopes,
                    currentNote: Self.sampleNote(seed: 0x41),
                    outputProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1,
                    previousLineageVerifierRecord: Self.sampleVerifierRecord(),
                    previousProofOpenEnvelopes: Self.syntheticPallasOpenEnvelopesArchive(
                        transcriptLabel: transcriptLabel
                    ),
                    lineageVerifierKey: Data(repeating: 0x6b, count: 64),
                    lineageProvingKeyArchive: Self.syntheticArchive(schema: "test.LineageProvingKeyArchive")
                )
            ) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveSpendRequestCodecError,
                    .invalidArchive("previousProofOpenEnvelopes")
                )
            }
        }
        for (metadataField, archive) in malformedPallasMetadataArchives {
            XCTAssertThrowsError(
                try KagemushaRecursiveSpendAppendRequest(
                    previousBundle: Self.sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
                    recordBundle: recordBundle,
                    pallasOpenEnvelopes: pallasOpenEnvelopes,
                    currentNote: Self.sampleNote(seed: 0x41),
                    outputProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1,
                    previousLineageVerifierRecord: Self.sampleVerifierRecord(),
                    previousProofOpenEnvelopes: archive,
                    lineageVerifierKey: Data(repeating: 0x6b, count: 64),
                    lineageProvingKeyArchive: Self.syntheticArchive(schema: "test.LineageProvingKeyArchive")
                ),
                metadataField
            ) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveSpendRequestCodecError,
                    .invalidArchive("previousProofOpenEnvelopes.\(metadataField)"),
                    metadataField
                )
            }
        }
    }

    private static func sampleNote(seed: UInt8 = 0x21) throws -> KagemushaRecursiveSpendableNoteDescriptor {
        try KagemushaRecursiveSpendableNoteDescriptor(
            noteCommitment: Data(repeating: seed, count: 32),
            spendNullifier: Data(repeating: seed &+ 1, count: 32),
            amount: "17"
        )
    }

    private static func sampleVerifierRecord() throws -> KagemushaRecursiveSpendVerifierRecordRef {
        try KagemushaRecursiveSpendVerifierRecordRef(
            verifierKeyId: "halo2/ipa:kagemusha-recursive-spend-lineage-test",
            recordBytes: syntheticArchive(schema: KagemushaRecursiveSpendRequestCodecs.verifyingKeyRecordWireName)
        )
    }

    private static func sharedInitLineageKeyMaterial() throws -> (Data, Data) {
        let verifierKey = lineageVerifierKey(
            circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
            seed: 0xA1
        )
        let provingKeyArchive = lineageProvingKeyArchive(
            circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
            verifierKey: verifierKey,
            seed: 0xA2
        )
        return (verifierKey, provingKeyArchive)
    }

    private static func sampleRecipient() throws -> String {
        try AccountAddress
            .fromAccount(publicKey: Data(repeating: 0x2a, count: 32), algorithm: "ed25519")
            .toI105(networkPrefix: 0x02F1)
    }

    private static func assertArchiveSchema(_ archive: Data, _ schema: String) throws {
        let frame = try XCTUnwrap(noritoDecodeFrame(archive))
        XCTAssertEqual(frame.header.schema, noritoSchemaHash(forTypeName: schema))
        XCTAssertEqual(frame.header.flags, NoritoHeader.compactLen)
        XCTAssertFalse(frame.payload.isEmpty)
    }

    private static func compactPayload(_ archive: Data, schema: String) throws -> Data {
        let frame = try XCTUnwrap(noritoDecodeFrame(archive))
        XCTAssertEqual(frame.header.schema, noritoSchemaHash(forTypeName: schema))
        XCTAssertEqual(frame.header.flags, NoritoHeader.compactLen)
        return frame.payload
    }

    private static func requestFields(_ archive: Data, schema: String) throws -> [Data] {
        try fieldPayloads(compactPayload(archive, schema: schema))
    }

    private static func recursiveSpendBundleWithAccumulatorField(
        fieldIndex: Int,
        replacement: Data
    ) throws -> Data {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
            schema: KagemushaRecursiveSpendRequestCodecs.bundleWireName
        )
        var bundleFields = try fieldPayloads(payload)
        var accumulatorFields = try fieldPayloads(bundleFields[0])
        accumulatorFields[fieldIndex] = replacement
        bundleFields[0] = encodeFields(accumulatorFields, flags: NoritoHeader.compactLen)
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.bundleWireName,
            payload: encodeFields(bundleFields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendBundleWithTopupAnchorNullifiers(
        _ nullifiers: [Data]
    ) throws -> Data {
        try recursiveSpendBundleWithAccumulatorField(
            fieldIndex: 5,
            replacement: encodeSequence(nullifiers)
        )
    }

    private static func recursiveSpendBundleWithTrailingBundleField() throws -> Data {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
            schema: KagemushaRecursiveSpendRequestCodecs.bundleWireName
        )
        var bundleFields = try fieldPayloads(payload)
        bundleFields.append(
            noritoString("ignored-extra-bundle-field", flags: NoritoHeader.compactLen)
        )
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.bundleWireName,
            payload: encodeFields(bundleFields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendVerifyResultWithTrailingField() throws -> Data {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi7, name: "verify_result"),
            schema: KagemushaRecursiveSpendRequestCodecs.verifyResultWireName
        )
        var fields = try fieldPayloads(payload)
        fields.append(Data([0x01]))
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.verifyResultWireName,
            payload: encodeFields(fields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendLineageWitnessWithTrailingField() throws -> Data {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "lineage_witness_append_result"),
            schema: KagemushaRecursiveSpendRequestCodecs.lineageWitnessWireName
        )
        var fields = try fieldPayloads(payload)
        fields.append(noritoString("ignored-extra-lineage-witness-field", flags: NoritoHeader.compactLen))
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.lineageWitnessWireName,
            payload: encodeFields(fields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendLineageWitnessWithTrailingPreviousProofsField() throws -> Data {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "lineage_witness_append_result"),
            schema: KagemushaRecursiveSpendRequestCodecs.lineageWitnessWireName
        )
        var fields = try fieldPayloads(payload)
        fields[3].append(
            noritoField(
                noritoString("ignored-extra-previous-proofs-field", flags: NoritoHeader.compactLen),
                flags: NoritoHeader.compactLen
            )
        )
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.lineageWitnessWireName,
            payload: encodeFields(fields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendLineageWitnessWithTrailingPreviousProofField() throws -> Data {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "lineage_witness_append_result"),
            schema: KagemushaRecursiveSpendRequestCodecs.lineageWitnessWireName
        )
        var fields = try fieldPayloads(payload)
        var previousProofs = try sequencePayloads(fields[3])
        XCTAssertFalse(previousProofs.isEmpty)
        var previousProofFields = try fieldPayloads(previousProofs[0])
        previousProofFields.append(
            noritoString("ignored-extra-previous-proof-field", flags: NoritoHeader.compactLen)
        )
        previousProofs[0] = encodeFields(previousProofFields, flags: NoritoHeader.compactLen)
        fields[3] = encodeSequence(previousProofs)
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.lineageWitnessWireName,
            payload: encodeFields(fields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendLineageWitnessWithTrailingPreviousVerifierKeyIdField() throws -> Data {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "lineage_witness_append_result"),
            schema: KagemushaRecursiveSpendRequestCodecs.lineageWitnessWireName
        )
        var fields = try fieldPayloads(payload)
        var previousProofs = try sequencePayloads(fields[3])
        XCTAssertFalse(previousProofs.isEmpty)
        var previousProofFields = try fieldPayloads(previousProofs[0])
        var verifierKeyIdFields = try fieldPayloads(previousProofFields[0])
        verifierKeyIdFields.append(
            noritoString("ignored-extra-previous-verifier-key-field", flags: NoritoHeader.compactLen)
        )
        previousProofFields[0] = encodeFields(verifierKeyIdFields, flags: NoritoHeader.compactLen)
        previousProofs[0] = encodeFields(previousProofFields, flags: NoritoHeader.compactLen)
        fields[3] = encodeSequence(previousProofs)
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.lineageWitnessWireName,
            payload: encodeFields(fields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendLineageWitnessWithPreviousProofField(
        fieldIndex: Int,
        replacement: Data
    ) throws -> Data {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "lineage_witness_append_result"),
            schema: KagemushaRecursiveSpendRequestCodecs.lineageWitnessWireName
        )
        var fields = try fieldPayloads(payload)
        var previousProofs = try sequencePayloads(fields[3])
        XCTAssertFalse(previousProofs.isEmpty)
        var previousProofFields = try fieldPayloads(previousProofs[0])
        previousProofFields[fieldIndex] = replacement
        previousProofs[0] = encodeFields(previousProofFields, flags: NoritoHeader.compactLen)
        fields[3] = encodeSequence(previousProofs)
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.lineageWitnessWireName,
            payload: encodeFields(fields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendLineageWitnessWithPreviousProofBoxBackend(
        _ proofBackend: String
    ) throws -> Data {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "lineage_witness_append_result"),
            schema: KagemushaRecursiveSpendRequestCodecs.lineageWitnessWireName
        )
        var fields = try fieldPayloads(payload)
        var previousProofs = try sequencePayloads(fields[3])
        XCTAssertFalse(previousProofs.isEmpty)
        var previousProofFields = try fieldPayloads(previousProofs[0])
        var proofBoxFields = try fieldPayloads(previousProofFields[3])
        proofBoxFields[0] = noritoString(proofBackend, flags: NoritoHeader.compactLen)
        previousProofFields[3] = encodeFields(proofBoxFields, flags: NoritoHeader.compactLen)
        previousProofs[0] = encodeFields(previousProofFields, flags: NoritoHeader.compactLen)
        fields[3] = encodeSequence(previousProofs)
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.lineageWitnessWireName,
            payload: encodeFields(fields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendLineageWitnessWithPreviousProofBoxBackendAndEmptyProofBytes(
        _ proofBackend: String
    ) throws -> Data {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "lineage_witness_append_result"),
            schema: KagemushaRecursiveSpendRequestCodecs.lineageWitnessWireName
        )
        var fields = try fieldPayloads(payload)
        var previousProofs = try sequencePayloads(fields[3])
        XCTAssertFalse(previousProofs.isEmpty)
        var previousProofFields = try fieldPayloads(previousProofs[0])
        var proofBoxFields = try fieldPayloads(previousProofFields[3])
        proofBoxFields[0] = noritoString(proofBackend, flags: NoritoHeader.compactLen)
        var emptyProofBytes = Data()
        appendUInt64LE(0, to: &emptyProofBytes)
        proofBoxFields[1] = emptyProofBytes
        previousProofFields[3] = encodeFields(proofBoxFields, flags: NoritoHeader.compactLen)
        previousProofs[0] = encodeFields(previousProofFields, flags: NoritoHeader.compactLen)
        fields[3] = encodeSequence(previousProofs)
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.lineageWitnessWireName,
            payload: encodeFields(fields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendLineageWitnessWithEmptyPreviousProofBytes() throws -> Data {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "lineage_witness_append_result"),
            schema: KagemushaRecursiveSpendRequestCodecs.lineageWitnessWireName
        )
        var fields = try fieldPayloads(payload)
        var previousProofs = try sequencePayloads(fields[3])
        XCTAssertFalse(previousProofs.isEmpty)
        var previousProofFields = try fieldPayloads(previousProofs[0])
        var proofBoxFields = try fieldPayloads(previousProofFields[3])
        var emptyProofBytes = Data()
        appendUInt64LE(0, to: &emptyProofBytes)
        proofBoxFields[1] = emptyProofBytes
        previousProofFields[3] = encodeFields(proofBoxFields, flags: NoritoHeader.compactLen)
        previousProofs[0] = encodeFields(previousProofFields, flags: NoritoHeader.compactLen)
        fields[3] = encodeSequence(previousProofs)
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.lineageWitnessWireName,
            payload: encodeFields(fields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendBundleWithTrailingAccumulatorField() throws -> Data {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
            schema: KagemushaRecursiveSpendRequestCodecs.bundleWireName
        )
        var bundleFields = try fieldPayloads(payload)
        var accumulatorFields = try fieldPayloads(bundleFields[0])
        accumulatorFields.append(
            noritoString("ignored-extra-accumulator-field", flags: NoritoHeader.compactLen)
        )
        bundleFields[0] = encodeFields(accumulatorFields, flags: NoritoHeader.compactLen)
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.bundleWireName,
            payload: encodeFields(bundleFields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendBundleWithCurrentNoteField(
        fieldIndex: Int,
        replacement: Data
    ) throws -> Data {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
            schema: KagemushaRecursiveSpendRequestCodecs.bundleWireName
        )
        var bundleFields = try fieldPayloads(payload)
        var accumulatorFields = try fieldPayloads(bundleFields[0])
        var currentNoteFields = try fieldPayloads(accumulatorFields[22])
        currentNoteFields[fieldIndex] = replacement
        accumulatorFields[22] = encodeFields(currentNoteFields, flags: NoritoHeader.compactLen)
        bundleFields[0] = encodeFields(accumulatorFields, flags: NoritoHeader.compactLen)
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.bundleWireName,
            payload: encodeFields(bundleFields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendBundleWithTrailingCurrentNoteField() throws -> Data {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
            schema: KagemushaRecursiveSpendRequestCodecs.bundleWireName
        )
        var bundleFields = try fieldPayloads(payload)
        var accumulatorFields = try fieldPayloads(bundleFields[0])
        var currentNoteFields = try fieldPayloads(accumulatorFields[22])
        currentNoteFields.append(
            noritoString("ignored-extra-current-note-field", flags: NoritoHeader.compactLen)
        )
        accumulatorFields[22] = encodeFields(currentNoteFields, flags: NoritoHeader.compactLen)
        bundleFields[0] = encodeFields(accumulatorFields, flags: NoritoHeader.compactLen)
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.bundleWireName,
            payload: encodeFields(bundleFields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendBundleWithEqualCurrentNoteNullifier() throws -> Data {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
            schema: KagemushaRecursiveSpendRequestCodecs.bundleWireName
        )
        var bundleFields = try fieldPayloads(payload)
        var accumulatorFields = try fieldPayloads(bundleFields[0])
        var currentNoteFields = try fieldPayloads(accumulatorFields[22])
        currentNoteFields[1] = currentNoteFields[0]
        accumulatorFields[22] = encodeFields(currentNoteFields, flags: NoritoHeader.compactLen)
        bundleFields[0] = encodeFields(accumulatorFields, flags: NoritoHeader.compactLen)
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.bundleWireName,
            payload: encodeFields(bundleFields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func zeroNumericPayload() -> Data {
        numericPayload(Data())
    }

    private static func numericPayload(_ mantissa: Data, scale: UInt32 = 0) -> Data {
        var mantissaPayload = Data()
        appendUInt32LE(UInt32(mantissa.count), to: &mantissaPayload)
        mantissaPayload.append(mantissa)
        var scalePayload = Data()
        appendUInt32LE(scale, to: &scalePayload)
        return encodeFields([mantissaPayload, scalePayload], flags: NoritoHeader.compactLen)
    }

    private static func accumulatorChainIdPayload(_ value: String) -> Data {
        encodeFields([noritoString(value, flags: NoritoHeader.compactLen)], flags: NoritoHeader.compactLen)
    }

    private static func numericPayloadWithMantissaPayload(_ mantissaPayload: Data) -> Data {
        var scalePayload = Data()
        appendUInt32LE(0, to: &scalePayload)
        return encodeFields([mantissaPayload, scalePayload], flags: NoritoHeader.compactLen)
    }

    private static func numericPayloadWithScalePayload(_ scalePayload: Data) -> Data {
        var mantissaPayload = Data()
        appendUInt32LE(1, to: &mantissaPayload)
        mantissaPayload.append(1)
        return encodeFields([mantissaPayload, scalePayload], flags: NoritoHeader.compactLen)
    }

    private static func numericPayloadWithTrailingField() -> Data {
        var extraPayload = Data()
        appendUInt32LE(0x42, to: &extraPayload)
        return numericPayload(Data([1]))
            + encodeFields([extraPayload], flags: NoritoHeader.compactLen)
    }

    private static func fixedArrayPayload(_ value: UInt8, count: Int) -> Data {
        encodeFields(Array(repeating: Data([value]), count: count), flags: NoritoHeader.compactLen)
    }

    private static func countPrefixedFixedArrayPayload(_ value: UInt8, count: Int) -> Data {
        var payload = Data()
        appendUInt64LE(UInt64(count), to: &payload)
        payload.append(fixedArrayPayload(value, count: count))
        return payload
    }

    private static func recursiveSpendBundleWithProofCircuitId(_ proofCircuitId: String) throws
        -> Data
    {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
            schema: KagemushaRecursiveSpendRequestCodecs.bundleWireName
        )
        let expected = Data(
            KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1.utf8
        )
        let replacement = Data(proofCircuitId.utf8)
        precondition(replacement.count == expected.count)
        let (mutatedPayload, replacementCount) = replacingAll(
            payload,
            expected,
            with: replacement
        )
        precondition(replacementCount == 2)
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.bundleWireName,
            payload: mutatedPayload,
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendBundleWithProofBackend(_ proofBackend: String) throws
        -> Data
    {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
            schema: KagemushaRecursiveSpendRequestCodecs.bundleWireName
        )
        let expected = Data(KagemushaRecursiveSpendProver.recursiveAggregationProofBackend.utf8)
        let replacement = Data(proofBackend.utf8)
        precondition(replacement.count == expected.count)
        let (mutatedPayload, replacementCount) = replacingAll(
            payload,
            expected,
            with: replacement
        )
        precondition(replacementCount == 2)
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.bundleWireName,
            payload: mutatedPayload,
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendBundleWithProofBoxBackend(_ proofBackend: String) throws
        -> Data
    {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
            schema: KagemushaRecursiveSpendRequestCodecs.bundleWireName
        )
        var bundleFields = try fieldPayloads(payload)
        var proofFields = try fieldPayloads(bundleFields[1])
        var proofBoxFields = try fieldPayloads(proofFields[3])
        proofBoxFields[0] = noritoString(proofBackend, flags: NoritoHeader.compactLen)
        proofFields[3] = encodeFields(proofBoxFields, flags: NoritoHeader.compactLen)
        bundleFields[1] = encodeFields(proofFields, flags: NoritoHeader.compactLen)
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.bundleWireName,
            payload: encodeFields(bundleFields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendBundleWithProofBoxBackendAndEmptyProofBytes(
        _ proofBackend: String
    ) throws -> Data {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
            schema: KagemushaRecursiveSpendRequestCodecs.bundleWireName
        )
        var bundleFields = try fieldPayloads(payload)
        var proofFields = try fieldPayloads(bundleFields[1])
        var proofBoxFields = try fieldPayloads(proofFields[3])
        proofBoxFields[0] = noritoString(proofBackend, flags: NoritoHeader.compactLen)
        proofBoxFields[1] = byteVecPayload(Data())
        proofFields[3] = encodeFields(proofBoxFields, flags: NoritoHeader.compactLen)
        bundleFields[1] = encodeFields(proofFields, flags: NoritoHeader.compactLen)
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.bundleWireName,
            payload: encodeFields(bundleFields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendBundleWithTrailingVerifierKeyIdField() throws -> Data {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
            schema: KagemushaRecursiveSpendRequestCodecs.bundleWireName
        )
        var bundleFields = try fieldPayloads(payload)
        var proofFields = try fieldPayloads(bundleFields[1])
        var verifierKeyIdFields = try fieldPayloads(proofFields[0])
        verifierKeyIdFields.append(
            noritoString("ignored-extra-verifier-key-field", flags: NoritoHeader.compactLen)
        )
        proofFields[0] = encodeFields(verifierKeyIdFields, flags: NoritoHeader.compactLen)
        bundleFields[1] = encodeFields(proofFields, flags: NoritoHeader.compactLen)
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.bundleWireName,
            payload: encodeFields(bundleFields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendBundleWithTrailingRecursiveProofField() throws -> Data {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
            schema: KagemushaRecursiveSpendRequestCodecs.bundleWireName
        )
        var bundleFields = try fieldPayloads(payload)
        var proofFields = try fieldPayloads(bundleFields[1])
        proofFields.append(
            noritoString("ignored-extra-recursive-proof-field", flags: NoritoHeader.compactLen)
        )
        bundleFields[1] = encodeFields(proofFields, flags: NoritoHeader.compactLen)
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.bundleWireName,
            payload: encodeFields(bundleFields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendBundleWithTrailingProofBoxField() throws -> Data {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
            schema: KagemushaRecursiveSpendRequestCodecs.bundleWireName
        )
        var bundleFields = try fieldPayloads(payload)
        var proofFields = try fieldPayloads(bundleFields[1])
        var proofBoxFields = try fieldPayloads(proofFields[3])
        proofBoxFields.append(
            noritoString("ignored-extra-proof-box-field", flags: NoritoHeader.compactLen)
        )
        proofFields[3] = encodeFields(proofBoxFields, flags: NoritoHeader.compactLen)
        bundleFields[1] = encodeFields(proofFields, flags: NoritoHeader.compactLen)
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.bundleWireName,
            payload: encodeFields(bundleFields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendBundleWithEmptyProofBytes() throws -> Data {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
            schema: KagemushaRecursiveSpendRequestCodecs.bundleWireName
        )
        var bundleFields = try fieldPayloads(payload)
        var proofFields = try fieldPayloads(bundleFields[1])
        var proofBoxFields = try fieldPayloads(proofFields[3])
        proofBoxFields[1] = byteVecPayload(Data())
        proofFields[3] = encodeFields(proofBoxFields, flags: NoritoHeader.compactLen)
        bundleFields[1] = encodeFields(proofFields, flags: NoritoHeader.compactLen)
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.bundleWireName,
            payload: encodeFields(bundleFields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendBundleWithEmptyProofPublicInputs() throws -> Data {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
            schema: KagemushaRecursiveSpendRequestCodecs.bundleWireName
        )
        var bundleFields = try fieldPayloads(payload)
        var proofFields = try fieldPayloads(bundleFields[1])
        proofFields[1] = Data()
        bundleFields[1] = encodeFields(proofFields, flags: NoritoHeader.compactLen)
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.bundleWireName,
            payload: encodeFields(bundleFields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendBundleWithZeroProofPublicInputsHash() throws -> Data {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
            schema: KagemushaRecursiveSpendRequestCodecs.bundleWireName
        )
        var bundleFields = try fieldPayloads(payload)
        var proofFields = try fieldPayloads(bundleFields[1])
        proofFields[2] = Data(repeating: 0, count: 32)
        bundleFields[1] = encodeFields(proofFields, flags: NoritoHeader.compactLen)
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.bundleWireName,
            payload: encodeFields(bundleFields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func recursiveSpendBundleWithMismatchedProofPublicInputsHash() throws -> Data {
        let payload = try compactPayload(
            sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
            schema: KagemushaRecursiveSpendRequestCodecs.bundleWireName
        )
        var bundleFields = try fieldPayloads(payload)
        var proofFields = try fieldPayloads(bundleFields[1])
        var mismatchedHash = proofFields[2]
        let first = mismatchedHash.startIndex
        mismatchedHash[first] = mismatchedHash[first] ^ 0x01
        proofFields[2] = mismatchedHash
        bundleFields[1] = encodeFields(proofFields, flags: NoritoHeader.compactLen)
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.bundleWireName,
            payload: encodeFields(bundleFields, flags: NoritoHeader.compactLen),
            flags: NoritoHeader.compactLen
        )
    }

    private static func fieldPayloads(_ payload: Data) throws -> [Data] {
        var reader = TestCompactReader(data: payload)
        var fields: [Data] = []
        while reader.remaining > 0 {
            fields.append(try reader.readField())
        }
        return fields
    }

    private static func encodeFields(_ fields: [Data], flags: UInt8) -> Data {
        fields.reduce(into: Data()) { out, field in
            out.append(noritoField(field, flags: flags))
        }
    }

    private static func sequencePayloads(_ payload: Data) throws -> [Data] {
        var reader = TestCompactReader(data: payload)
        let count = try reader.readUInt64LE()
        guard count <= UInt64(Int.max) else {
            XCTFail("test sequence count is too large")
            return []
        }
        var fields: [Data] = []
        for _ in 0..<Int(count) {
            fields.append(try reader.readField())
        }
        XCTAssertEqual(reader.remaining, 0)
        return fields
    }

    private static func encodeSequence(_ fields: [Data]) -> Data {
        var encoded = Data()
        appendUInt64LE(UInt64(fields.count), to: &encoded)
        for field in fields {
            encoded.append(noritoField(field, flags: NoritoHeader.compactLen))
        }
        return encoded
    }

    private static func noritoField(_ payload: Data, flags: UInt8) -> Data {
        var encoded = noritoLength(payload.count, flags: flags)
        encoded.append(payload)
        return encoded
    }

    private static func noritoString(_ value: String, flags: UInt8) -> Data {
        let bytes = Data(value.utf8)
        var encoded = noritoLength(bytes.count, flags: flags)
        encoded.append(bytes)
        return encoded
    }

    private static func byteVecPayload(_ value: Data) -> Data {
        var encoded = Data()
        appendUInt64LE(UInt64(value.count), to: &encoded)
        encoded.append(value)
        return encoded
    }

    private static func replacingAll(_ data: Data, _ expected: Data, with replacement: Data)
        -> (Data, Int)
    {
        precondition(!expected.isEmpty)
        precondition(expected.count == replacement.count)
        var output = Data()
        var index = data.startIndex
        var replacements = 0
        while index < data.endIndex {
            let remaining = data.distance(from: index, to: data.endIndex)
            if remaining >= expected.count {
                let end = data.index(index, offsetBy: expected.count)
                if data[index..<end].elementsEqual(expected) {
                    output.append(replacement)
                    index = end
                    replacements += 1
                    continue
                }
            }
            output.append(data[index])
            index = data.index(after: index)
        }
        return (output, replacements)
    }

    private static func noritoLength(_ value: Int, flags: UInt8) -> Data {
        guard (flags & NoritoHeader.compactLen) != 0 else {
            var encoded = Data()
            appendUInt64LE(UInt64(value), to: &encoded)
            return encoded
        }
        var encoded = Data()
        var remaining = UInt64(value)
        while remaining >= 0x80 {
            encoded.append(UInt8(remaining & 0x7f) | 0x80)
            remaining >>= 7
        }
        encoded.append(UInt8(remaining))
        return encoded
    }

    private static func appendUInt64LE(_ value: UInt64, to data: inout Data) {
        var littleEndian = value.littleEndian
        withUnsafeBytes(of: &littleEndian) { bytes in
            data.append(contentsOf: bytes)
        }
    }

    private static func appendUInt32LE(_ value: UInt32, to data: inout Data) {
        var littleEndian = value.littleEndian
        withUnsafeBytes(of: &littleEndian) { bytes in
            data.append(contentsOf: bytes)
        }
    }

    private static func appendUInt16LE(_ value: UInt16, to data: inout Data) {
        var littleEndian = value.littleEndian
        withUnsafeBytes(of: &littleEndian) { bytes in
            data.append(contentsOf: bytes)
        }
    }

    private static func appendUInt64BE(_ value: UInt64, to data: inout Data) {
        var bigEndian = value.bigEndian
        withUnsafeBytes(of: &bigEndian) { bytes in
            data.append(contentsOf: bytes)
        }
    }

    private static func readBytesVecPayload(_ payload: Data) throws -> Data {
        var reader = TestCompactReader(data: payload)
        let length = try Int(reader.readUInt64LE())
        let bytes = try reader.readBytes(length)
        XCTAssertEqual(reader.remaining, 0)
        return bytes
    }

    private static func readFixedArrayPayload(_ payload: Data, expectedSize: Int) throws -> Data {
        var reader = TestCompactReader(data: payload)
        var bytes = Data()
        while reader.remaining > 0 {
            XCTAssertEqual(try reader.readLength(), 1)
            bytes.append(try reader.readUInt8())
        }
        XCTAssertEqual(bytes.count, expectedSize)
        return bytes
    }

    private static func readStringPayload(_ payload: Data) throws -> String {
        var reader = TestCompactReader(data: payload)
        let length = try reader.readLength()
        let bytes = try reader.readBytes(length)
        XCTAssertEqual(reader.remaining, 0)
        return try XCTUnwrap(String(data: bytes, encoding: .utf8))
    }

    private static func readUInt64Payload(_ payload: Data) throws -> UInt64 {
        var reader = TestCompactReader(data: payload)
        let value = try reader.readUInt64LE()
        XCTAssertEqual(reader.remaining, 0)
        return value
    }

    private static func optionSomePayload(_ payload: Data) throws -> Data {
        var reader = TestCompactReader(data: payload)
        XCTAssertEqual(try reader.readUInt8(), 1)
        let value = try reader.readField()
        XCTAssertEqual(reader.remaining, 0)
        return value
    }

    private static func assertOptionNone(_ payload: Data) throws {
        var reader = TestCompactReader(data: payload)
        XCTAssertEqual(try reader.readUInt8(), 0)
        XCTAssertEqual(reader.remaining, 0)
    }

    private static func syntheticArchive(schema: String) -> Data {
        noritoEncode(
            typeName: schema,
            payload: Data([0x01, 0x02, 0x03]),
            flags: NoritoHeader.compactLen
        )
    }

    private static func syntheticRecordBundleArchive(hopCount: Int = 1) -> Data {
        precondition(hopCount >= 1)
        let stepPayload = encodeFields(
            (0..<6).map { Data([UInt8(0xa0 + $0)]) },
            flags: NoritoHeader.compactLen
        )
        var stepsPayload = Data()
        appendUInt64LE(UInt64(hopCount), to: &stepsPayload)
        for _ in 0..<hopCount {
            stepsPayload.append(noritoField(stepPayload, flags: NoritoHeader.compactLen))
        }
        let bundlePayload = encodeFields(
            [
                Data([0x41]),
                Data([0x42]),
                stepsPayload
            ],
            flags: NoritoHeader.compactLen
        )
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.recordBundleWireName,
            payload: encodeFields(
                [
                    bundlePayload,
                    Data()
                ],
                flags: NoritoHeader.compactLen
            ),
            flags: NoritoHeader.compactLen
        )
    }

    private static func syntheticPallasOpenEnvelopesArchive(
        count: Int = 1,
        includeDomainTag: Bool = true,
        transcriptLabel: String = "pallas-open",
        vkCommitmentPayload: Data? = nil,
        publicInputsSchemaHashPayload: Data? = nil,
        domainTagPayload: Data? = nil,
        vkCommitmentOptionPayload: Data? = nil,
        publicInputsSchemaHashOptionPayload: Data? = nil,
        domainTagOptionPayload: Data? = nil
    ) -> Data {
        let envelope = syntheticPallasOpenEnvelopePayload(
            includeDomainTag: includeDomainTag,
            transcriptLabel: transcriptLabel,
            vkCommitmentPayload: vkCommitmentPayload,
            publicInputsSchemaHashPayload: publicInputsSchemaHashPayload,
            domainTagPayload: domainTagPayload,
            vkCommitmentOptionPayload: vkCommitmentOptionPayload,
            publicInputsSchemaHashOptionPayload: publicInputsSchemaHashOptionPayload,
            domainTagOptionPayload: domainTagOptionPayload
        )
        var payload = Data()
        appendUInt64LE(UInt64(count), to: &payload)
        for _ in 0..<count {
            payload.append(noritoField(envelope, flags: NoritoHeader.compactLen))
        }
        var archive = noritoEncode(
            typeName: "test.PallasOpenEnvelopeVector",
            payload: payload,
            flags: NoritoHeader.compactLen
        )
        archive.replaceSubrange(
            archive.index(archive.startIndex, offsetBy: 6)..<archive.index(archive.startIndex, offsetBy: 22),
            with: pallasOpenEnvelopeVectorSchemaHash
        )
        return archive
    }

    private static func syntheticPallasOpenEnvelopePayload(
        includeDomainTag: Bool,
        transcriptLabel: String,
        vkCommitmentPayload: Data?,
        publicInputsSchemaHashPayload: Data?,
        domainTagPayload: Data?,
        vkCommitmentOptionPayload: Data?,
        publicInputsSchemaHashOptionPayload: Data?,
        domainTagOptionPayload: Data?
    ) -> Data {
        let n: UInt32 = 4
        let params = encodeFields(
            [
                uint16Payload(1),
                uint16Payload(1),
                uint32Payload(n),
                fixed32SequencePayload(count: Int(n), seed: 0x10),
                fixed32SequencePayload(count: Int(n), seed: 0x20),
                fixed32(0x30)
            ],
            flags: NoritoHeader.compactLen
        )
        let publicValue = encodeFields(
            [
                uint16Payload(1),
                uint16Payload(1),
                uint32Payload(n),
                fixed32(0x31),
                fixed32(0x32),
                fixed32(0x33)
            ],
            flags: NoritoHeader.compactLen
        )
        let proof = encodeFields(
            [
                uint16Payload(1),
                fixed32SequencePayload(count: 2, seed: 0x40),
                fixed32SequencePayload(count: 2, seed: 0x50),
                fixed32(0x60),
                fixed32(0x61)
            ],
            flags: NoritoHeader.compactLen
        )
        let vkOptionPayload = vkCommitmentOptionPayload
            ?? requiredOptionPayload(vkCommitmentPayload ?? fixed32(0x70))
        let publicInputsSchemaOptionPayload = publicInputsSchemaHashOptionPayload
            ?? requiredOptionPayload(publicInputsSchemaHashPayload ?? fixed32(0x71))
        let domainOptionPayload = domainTagOptionPayload
            ?? requiredOptionPayload(includeDomainTag ? (domainTagPayload ?? fixed32(0x72)) : nil)
        return encodeFields(
            [
                params,
                publicValue,
                proof,
                noritoString(transcriptLabel, flags: NoritoHeader.compactLen),
                vkOptionPayload,
                publicInputsSchemaOptionPayload,
                domainOptionPayload
            ],
            flags: NoritoHeader.compactLen
        )
    }

    private static func fixed32SequencePayload(count: Int, seed: UInt8) -> Data {
        var payload = Data()
        appendUInt64LE(UInt64(count), to: &payload)
        for index in 0..<count {
            payload.append(noritoField(fixed32(seed &+ UInt8(index)), flags: NoritoHeader.compactLen))
        }
        return payload
    }

    private static func requiredOptionPayload(_ payload: Data?) -> Data {
        guard let payload else { return Data([0]) }
        var out = Data([1])
        out.append(noritoLength(payload.count, flags: NoritoHeader.compactLen))
        out.append(payload)
        return out
    }

    private static func requiredOptionPayloadWithTrailingByte(_ payload: Data) -> Data {
        var out = requiredOptionPayload(payload)
        out.append(0x7f)
        return out
    }

    private static func requiredOptionPayloadWithUnknownTag() -> Data {
        Data([0x02])
    }

    private static func requiredOptionPayloadWithDeclaredLengthTooLong(_ payload: Data) -> Data {
        var out = Data([1])
        out.append(noritoLength(payload.count + 1, flags: NoritoHeader.compactLen))
        out.append(payload)
        return out
    }

    private static func fixed32(_ seed: UInt8) -> Data {
        Data((0..<32).map { UInt8((Int(seed) + $0) & 0xff) })
    }

    private static func lineageVerifierKey(circuitId: String, seed: UInt8) -> Data {
        var verifierKey = Data([0x5A, 0x4B, 0x31, 0x00])
        verifierKey.append(zk1Tlv(tag: "IPAK", payload: Data([8, 0, 0, 0])))
        verifierKey.append(zk1Tlv(tag: "CID1", payload: Data(circuitId.utf8)))
        verifierKey.append(zk1Tlv(tag: "H2VK", payload: Data(repeating: seed, count: 32)))
        return verifierKey
    }

    private static func zk1Tlv(tag: String, payload: Data) -> Data {
        var encoded = Data(tag.utf8)
        appendUInt32LE(UInt32(payload.count), to: &encoded)
        encoded.append(payload)
        return encoded
    }

    private static func lineageProvingKeyArchive(
        circuitId: String,
        verifierKey: Data,
        seed: UInt8
    ) -> Data {
        var versionPayload = Data()
        appendUInt16LE(1, to: &versionPayload)
        let verifierKeyCommitment = verifierKeyCommitment(verifierKey: verifierKey)
        let payload = encodeFields(
            [
                versionPayload,
                noritoString(circuitId, flags: NoritoHeader.compactLen),
                verifierKeyCommitment,
                byteVecPayload(Data(repeating: seed, count: 64))
            ],
            flags: NoritoHeader.compactLen
        )
        return noritoFrameFromSchemaHash(
            kagemushaLineageProvingKeyArchiveSchemaHash,
            payload: payload,
            flags: NoritoHeader.compactLen
        )
    }

    private static func verifierKeyCommitment(verifierKey: Data) -> Data {
        let backend = Data(KagemushaRecursiveSpendProver.recursiveAggregationProofBackend.utf8)
        var preimage = Data("iroha:zk:v1:vk".utf8)
        appendUInt64BE(UInt64(backend.count), to: &preimage)
        preimage.append(backend)
        appendUInt64BE(UInt64(verifierKey.count), to: &preimage)
        preimage.append(verifierKey)
        return Data(SHA256.hash(data: preimage))
    }

    private static func noritoFrameFromSchemaHash(
        _ schemaHash: Data,
        payload: Data,
        flags: UInt8
    ) -> Data {
        precondition(schemaHash.count == 16)
        var frame = Data()
        frame.append(NoritoHeader.magic)
        frame.append(NoritoHeader.versionMajor)
        frame.append(NoritoHeader.versionMinor)
        frame.append(schemaHash)
        frame.append(NoritoCompression.none.rawValue)
        appendUInt64LE(UInt64(payload.count), to: &frame)
        appendUInt64LE(crc64ECMA(payload), to: &frame)
        frame.append(flags)
        frame.append(payload)
        return frame
    }

    private static func uint16Payload(_ value: UInt16) -> Data {
        var payload = Data()
        appendUInt16LE(value, to: &payload)
        return payload
    }

    private static func uint32Payload(_ value: UInt32) -> Data {
        var payload = Data()
        appendUInt32LE(value, to: &payload)
        return payload
    }

    private static func sharedRecursiveSpendArchive(abi: FixtureAbi, name: String) throws -> Data {
        let root = try sharedRecursiveSpendFixture(named: "archives.json", abi: abi)
        let archives = try XCTUnwrap(root["archives"] as? [[String: Any]])
        let entry = try XCTUnwrap(archives.first { $0["name"] as? String == name })
        return try XCTUnwrap(Data(base64Encoded: try XCTUnwrap(entry["bytes_base64"] as? String)))
    }

    private static func sharedRecursiveSpendFixture(named fileName: String, abi: FixtureAbi) throws -> [String: Any] {
        var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        for _ in 0..<10 {
            let candidate = directory
                .appendingPathComponent("fixtures")
                .appendingPathComponent(abi.directory)
                .appendingPathComponent(fileName)
            if FileManager.default.fileExists(atPath: candidate.path) {
                let data = try Data(contentsOf: candidate)
                return try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
            }
            directory.deleteLastPathComponent()
        }
        throw NSError(
            domain: "KagemushaRecursiveSpendRequestCodecsTests",
            code: 1,
            userInfo: [NSLocalizedDescriptionKey: "missing shared recursive spend fixture \(abi.directory)/\(fileName)"]
        )
    }

    private enum FixtureAbi {
        case abi6
        case abi7

        var directory: String {
            switch self {
            case .abi6: return "kagemusha_recursive_spend_abi6"
            case .abi7: return "kagemusha_recursive_spend_abi7"
            }
        }
    }

    private static let pallasOpenEnvelopeVectorSchemaHash = Data([
        0xfe, 0x38, 0x26, 0x32, 0x8f, 0x08, 0x17, 0x71,
        0x75, 0x0f, 0x24, 0xfe, 0x11, 0x02, 0x60, 0xca
    ])

    private static let kagemushaLineageProvingKeyArchiveSchemaHash = Data([
        0xc8, 0x84, 0x89, 0x61, 0x8a, 0x01, 0x2c, 0x28,
        0x3f, 0xf3, 0xbb, 0x2e, 0xba, 0xbc, 0x77, 0x75,
    ])

    private static let u128MaxPlusOne = "340282366920938463463374607431768211456"
    private static let u128TooManyDigits = String(repeating: "9", count: 40)
}

private struct TestCompactReader {
    private let data: Data
    private(set) var offset = 0

    init(data: Data) {
        self.data = data
    }

    var remaining: Int {
        data.count - offset
    }

    mutating func readUInt8() throws -> UInt8 {
        guard offset < data.count else { throw TestReadError.truncated }
        let value = data[data.startIndex + offset]
        offset += 1
        return value
    }

    mutating func readUInt64LE() throws -> UInt64 {
        let bytes = try readBytes(8)
        var value: UInt64 = 0
        bytes.withUnsafeBytes { buffer in
            guard let base = buffer.baseAddress else { return }
            memcpy(&value, base, 8)
        }
        return UInt64(littleEndian: value)
    }

    mutating func readBytes(_ count: Int) throws -> Data {
        guard count >= 0, offset + count <= data.count else { throw TestReadError.truncated }
        let start = data.startIndex + offset
        offset += count
        return Data(data[start..<(start + count)])
    }

    mutating func readField() throws -> Data {
        try readBytes(readLength())
    }

    mutating func readLength() throws -> Int {
        var value: UInt64 = 0
        var shift: UInt64 = 0
        for _ in 0..<10 {
            let byte = try readUInt8()
            value |= UInt64(byte & 0x7f) << shift
            if (byte & 0x80) == 0 {
                return Int(value)
            }
            shift += 7
        }
        throw TestReadError.truncated
    }
}

private enum TestReadError: Error {
    case truncated
}
