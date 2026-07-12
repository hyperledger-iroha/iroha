import CryptoKit
import XCTest
@testable import IrohaSwift

final class KagemushaTopUpParityTests: XCTestCase {
    func testFirstReleaseApiDoesNotExposeProofOutputOnlyFoldBuilder() throws {
        let packageRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let checkedFiles = [
            packageRoot.appendingPathComponent("Sources/IrohaSwift/KagemushaRecursiveSpendRequestCodecs.swift"),
            packageRoot.appendingPathComponent("README.md")
        ]
        for file in checkedFiles {
            let contents = try String(contentsOf: file, encoding: .utf8)
            XCTAssertFalse(contents.contains("hopProofOutputArchives"), "\(file.path) exposes proof-output-only fold inputs")
            XCTAssertFalse(
                contents.contains("buildVerifiedFoldRecordBundle(hopProofOutputArchives"),
                "\(file.path) documents a proof-output-only fold builder"
            )
        }
    }

    func testVerifierRecordArchiveUsesMarkedSchemaHashAndThirtyTwoBitStatus() throws {
        let verifierKey = verifierKey()
        let record = try KagemushaRecursiveSpendRequestCodecs
            .encodeConfidentialTransferV2VerifierRecordArchive(verifierKey: verifierKey)
        let payload = try KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            record,
            schema: KagemushaRecursiveSpendRequestCodecs.verifyingKeyRecordWireName,
            field: "record"
        )
        let fields = try compactFields(payload)

        XCTAssertEqual(fields.count, 17)
        XCTAssertEqual(
            fields[6],
            IrohaHash.hash(PrivacyConfidentialWitnessCodecs.confidentialTransferPublicInputsSchema())
        )
        XCTAssertEqual(fields[6].last.map { $0 & 1 }, 1)
        XCTAssertEqual(
            fields[7],
            verifyingKeyCommitment(
                backend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                bytes: verifierKey
            )
        )
        XCTAssertEqual(fields.last, Data([1, 0, 0, 0]))
    }

    func testBuildsVerifiedFoldRecordBundleFromSyntheticTransferHop() throws {
        let verifierKey = verifierKey()
        let vkHash = verifyingKeyCommitment(backend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                                            bytes: verifierKey)
        let envelope = openVerifyEnvelope(vkHash: vkHash)
        let proofOutput = privacyBuildResult(proof: envelope)
        let verifierRecord = try KagemushaRecursiveSpendRequestCodecs
            .encodeConfidentialTransferV2VerifierRecordArchive(verifierKey: verifierKey)
        let ref = try KagemushaRecursiveSpendVerifierRecordRef(
            verifierKeyId: "halo2/ipa:transfer-v2",
            recordBytes: verifierRecord
        )
        let hop = try KagemushaVerifiedFoldHopEvidence(
            proofOutputArchive: proofOutput,
            verifierRecord: ref,
            chainId: "swift-kagemusha-chain",
            assetDefinitionId: assetDefinitionId(),
            rootAfter: fixed32(0x08)
        )

        let bundle = try KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(hops: [hop])
        let payload = try KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            bundle,
            schema: KagemushaRecursiveSpendRequestCodecs.recordBundleWireName,
            field: "recordBundle"
        )
        XCTAssertEqual(
            try KagemushaRecursiveSpendRequestCodecs.readVerifiedFoldRecordBundleHopCount(
                payload,
                field: "recordBundle"
            ),
            1
        )
    }

    func testVerifiedFoldRecordBundleUsesPackedDirectAndGenericFixed32Framing() throws {
        let rootBefore = fixed32(0x21)
        let rootAfter = fixed32(0x22)
        let verifierKey = verifierKey()
        let vkHash = verifyingKeyCommitment(
            backend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
            bytes: verifierKey
        )
        let envelope = openVerifyEnvelope(
            vkHash: vkHash,
            proof: zk1Proof(rootBefore: rootBefore)
        )
        let verifierRecord = try KagemushaRecursiveSpendRequestCodecs
            .encodeConfidentialTransferV2VerifierRecordArchive(verifierKey: verifierKey)
        let hop = try KagemushaVerifiedFoldHopEvidence(
            proofOutputArchive: privacyBuildResult(proof: envelope),
            verifierRecord: KagemushaRecursiveSpendVerifierRecordRef(
                verifierKeyId: "halo2/ipa:transfer-v2",
                recordBytes: verifierRecord
            ),
            chainId: "swift-kagemusha-chain",
            assetDefinitionId: assetDefinitionId(),
            rootAfter: rootAfter
        )

        let archive = try KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(hops: [hop])
        let payload = try KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            archive,
            schema: KagemushaRecursiveSpendRequestCodecs.recordBundleWireName,
            field: "recordBundle"
        )
        let recordBundleFields = try compactFields(payload)
        let bundleFields = try compactFields(recordBundleFields[0])
        var expectedAssetBytes = Data((0..<16).map { UInt8(0x01 + $0) })
        expectedAssetBytes[6] = (expectedAssetBytes[6] & 0x0f) | 0x40
        expectedAssetBytes[8] = (expectedAssetBytes[8] & 0x3f) | 0x80
        XCTAssertEqual(try constVecBytes(bundleFields[1], expectedCount: 16), expectedAssetBytes)
        var steps = KagemushaTestCompactReader(bundleFields[2])
        XCTAssertEqual(try steps.readUInt64LE(), 1)
        let stepFields = try compactFields(steps.readField())
        XCTAssertEqual(steps.remaining, 0)

        XCTAssertEqual(stepFields[0], rootBefore)
        XCTAssertEqual(stepFields[0].count, 32)
        XCTAssertEqual(stepFields[3], rootAfter)
        XCTAssertEqual(stepFields[3].count, 32)
        XCTAssertEqual(try fixed32VectorValues(stepFields[1]).count, 1)
        XCTAssertEqual(try fixed32VectorValues(stepFields[2]).count, 1)

        let attachmentFields = try compactFields(stepFields[4])
        XCTAssertEqual(attachmentFields.count, 5)
        XCTAssertEqual(try optionFixed32(attachmentFields[3]), vkHash)
        let envelopeHash = try optionFixed32(attachmentFields[4])
        XCTAssertEqual(envelopeHash, IrohaHash.hash(envelope))
        XCTAssertEqual(envelopeHash.last.map { $0 & 1 }, 1)
    }

    func testBuildRedeemProofAttachmentEmitsCanonicalArchiveAndComposesWithRedeemRequest() throws {
        let fixture = try unshieldProofFixture()
        let unshieldSchemaHash = IrohaHash.hash(
            PrivacyConfidentialWitnessCodecs.confidentialUnshieldPublicInputsSchema()
        )
        XCTAssertEqual(unshieldSchemaHash, Data([
            0x43, 0xcd, 0xb5, 0x63, 0x9d, 0x62, 0xc1, 0xc5,
            0x39, 0xb2, 0xc5, 0x76, 0x81, 0x8a, 0x38, 0xa6,
            0x81, 0xc8, 0xbe, 0x9d, 0x8f, 0x9f, 0x97, 0x23,
            0x3a, 0x56, 0xa4, 0x90, 0x6f, 0x28, 0x24, 0xbd,
        ]))
        XCTAssertNotEqual(
            Blake2b.hash256(PrivacyConfidentialWitnessCodecs.confidentialUnshieldPublicInputsSchema()),
            unshieldSchemaHash
        )

        let attachment = try KagemushaRecursiveSpendRequestCodecs.buildRedeemProofAttachmentStructurally(
            unshieldProofOutputArchive: fixture.proofOutputArchive,
            unshieldVerifierRecord: fixture.verifierRecord
        )
        let payload = try KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            attachment,
            schema: KagemushaRecursiveSpendRequestCodecs.proofAttachmentWireName,
            field: "redeemProof"
        )
        let fields = try compactFields(payload)

        XCTAssertEqual(fields.count, 5)
        XCTAssertEqual(
            fields[0],
            OfflineCompactNorito.encodeString(KagemushaRecursiveSpendProver.recursiveAggregationProofBackend)
        )
        let proofBoxFields = try compactFields(fields[1])
        XCTAssertEqual(proofBoxFields.count, 2)
        XCTAssertEqual(
            proofBoxFields[0],
            OfflineCompactNorito.encodeString(KagemushaRecursiveSpendProver.recursiveAggregationProofBackend)
        )
        XCTAssertEqual(proofBoxFields[1], byteVec(fixture.envelopeArchive))
        let verifierKeyIdFields = try compactFields(fields[2])
        XCTAssertEqual(verifierKeyIdFields.count, 2)
        XCTAssertEqual(
            verifierKeyIdFields[0],
            OfflineCompactNorito.encodeString(KagemushaRecursiveSpendProver.recursiveAggregationProofBackend)
        )
        XCTAssertEqual(verifierKeyIdFields[1], OfflineCompactNorito.encodeString("unshield-v3"))
        XCTAssertEqual(try optionFixed32Payload(fields[3]), fixture.commitment)
        let canonicalEnvelopeHash = IrohaHash.hash(fixture.envelopeArchive)
        XCTAssertEqual(
            try optionFixed32Payload(fields[4]),
            canonicalEnvelopeHash
        )
        XCTAssertNotEqual(Blake2b.hash256(fixture.envelopeArchive), canonicalEnvelopeHash)
        XCTAssertThrowsError(try fixed32Payload(fixture.commitment))
        XCTAssertThrowsError(try fixed32Payload(canonicalEnvelopeHash))
        let redeemRequest = try KagemushaRecursiveSpendRedeemRequest(
            bundle: sharedRecursiveSpendArchive(name: "init_bundle"),
            recipient: sampleRecipient(),
            publicAmount: "7",
            redeemProof: attachment,
            lineageWitness: sharedRecursiveSpendArchive(name: "lineage_witness_append_result"),
            lineageVerifierRecord: syntheticLineageVerifierRecord()
        )
        let redeemArchive = try KagemushaRecursiveSpendRequestCodecs.encodeRedeemRequest(redeemRequest)
        let redeemPayload = try KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            redeemArchive,
            schema: KagemushaRecursiveSpendRequestCodecs.redeemRequestWireName,
            field: "redeemRequest"
        )
        XCTAssertEqual(try compactFields(redeemPayload)[3], payload)
    }

    func testBuildRedeemProofAttachmentRejectsInvalidUnshieldEvidence() throws {
        let fixture = try unshieldProofFixture()

        let invalidBuildResults = [
            privacyBuildResult(
                proof: fixture.envelopeArchive,
                algorithmId: "unshield",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef,
                version: 2
            ),
            privacyBuildResult(
                proof: fixture.envelopeArchive,
                algorithmId: "unshield",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef,
                status: 1
            ),
            privacyBuildResult(
                proof: fixture.envelopeArchive,
                algorithmId: "unshield",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef,
                errorCode: 5
            ),
            privacyBuildResult(
                proof: fixture.envelopeArchive,
                algorithmId: "unshield",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef,
                message: "rejected"
            ),
            privacyBuildResult(
                proof: fixture.envelopeArchive,
                algorithmId: "confidential-transfer-v2",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef
            ),
            privacyBuildResult(
                proof: fixture.envelopeArchive,
                algorithmId: "unshield",
                entrypoint: "buildConfidentialTransferProofV2",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef
            ),
            privacyBuildResult(
                proof: fixture.envelopeArchive,
                algorithmId: "unshield",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialTransferV2VerifierRef
            ),
            privacyBuildResult(
                proof: fixture.envelopeArchive,
                algorithmId: "unshield",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef,
                publicInputs: Data([0x01])
            ),
            privacyBuildResult(
                proof: Data(),
                algorithmId: "unshield",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef
            ),
            privacyBuildResult(
                proof: fixture.envelopeArchive,
                algorithmId: "unshield",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef,
                verified: true
            ),
            privacyBuildResult(
                proof: fixture.envelopeArchive,
                algorithmId: "unshield",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef,
                trailingField: Data([0x00])
            ),
            privacyBuildResult(
                proof: fixture.envelopeArchive,
                algorithmId: "unshield",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef,
                flags: 0
            ),
            privacyBuildResult(
                proof: fixture.envelopeArchive,
                algorithmId: "unshield",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef,
                schemaByte: 0x41
            ),
            archiveWithHeaderPadding(
                privacyBuildResult(
                    proof: fixture.envelopeArchive,
                    algorithmId: "unshield",
                    entrypoint: "buildConfidentialUnshieldProofV3",
                    vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef
                ),
                count: NoritoHeader.maxHeaderPadding + 1
            ),
            Data([0x00])
        ]
        for invalidBuildResult in invalidBuildResults {
            assertRedeemProofAttachmentRejected(
                proofOutputArchive: invalidBuildResult,
                verifierRecord: fixture.verifierRecord,
                expected: .invalidArchive("unshieldProofOutputArchive")
            )
        }

        assertRedeemProofAttachmentRejected(
            proofOutputArchive: privacyBuildResult(
                proof: fixture.envelopeArchive,
                algorithmId: "unshield",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: " invalid"
            ),
            verifierRecord: fixture.verifierRecord,
            expected: .invalidField("unshieldProofOutputArchive.vk_ref")
        )

        let invalidRecords = [
            try unshieldVerifierRecord(verifierKey: verifierKey(), status: 2),
            try unshieldVerifierRecord(
                verifierKey: verifierKey(),
                circuitId: KagemushaRecursiveSpendRequestCodecs.confidentialTransferV2CircuitId
            ),
            try unshieldVerifierRecord(
                verifierKey: verifierKey(),
                schema: PrivacyConfidentialWitnessCodecs.confidentialTransferPublicInputsSchema()
            ),
            try unshieldVerifierRecord(
                verifierKey: verifierKey(),
                publicInputsSchemaHash: Blake2b.hash256(
                    PrivacyConfidentialWitnessCodecs.confidentialUnshieldPublicInputsSchema()
                )
            ),
            try unshieldVerifierRecord(
                verifierKey: Data((0..<96).map { UInt8(($0 * 11 + 5) & 0xff) })
            ),
            try unshieldVerifierRecord(
                verifierKey: verifierKey(),
                namespace: "privacy"
            ),
            try unshieldVerifierRecord(
                verifierKey: verifierKey(),
                backendTag: VerifyingKeyBackendTag.halo2Bn254.rawValue
            ),
            try unshieldVerifierRecord(
                verifierKey: verifierKey(),
                curve: "vesta"
            ),
            try unshieldVerifierRecord(
                verifierKey: verifierKey(),
                verifierKeyId: "test:unshield-v3"
            ),
            try unshieldVerifierRecord(
                verifierKey: verifierKey(),
                maxProofBytes: 0
            ),
            try unshieldVerifierRecord(
                verifierKey: verifierKey(),
                maxProofBytes: UInt32(192 * 1024 + 1)
            ),
            try unshieldVerifierRecord(
                verifierKey: verifierKey(),
                maxProofBytes: UInt32(fixture.envelopeArchive.count - 1)
            )
        ]
        for invalidRecord in invalidRecords {
            assertRedeemProofAttachmentRejected(
                proofOutputArchive: fixture.proofOutputArchive,
                verifierRecord: invalidRecord,
                expected: .invalidArchive("unshieldVerifierRecord")
            )
        }
    }

    func testBuildRedeemProofAttachmentRequiresExactSuccessfulNativeVerification() throws {
        let fixture = try unshieldProofFixture()
        let successfulVerifyResult = privacyBuildResult(
            proof: fixture.envelopeArchive,
            algorithmId: "unshield",
            entrypoint: "buildConfidentialUnshieldProofV3",
            vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef,
            verified: true,
            schemaByte: 0x56
        )
        var observedVerifyRequest: Data?
        XCTAssertNoThrow(
            try KagemushaRecursiveSpendRequestCodecs.buildRedeemProofAttachment(
                unshieldProofOutputArchive: fixture.proofOutputArchive,
                unshieldVerifierRecord: fixture.verifierRecord,
                verifyProof: { request in
                    observedVerifyRequest = request
                    return successfulVerifyResult
                }
            )
        )
        XCTAssertNotNil(observedVerifyRequest)

        let invalidVerifyResults = [
            privacyBuildResult(
                proof: fixture.envelopeArchive,
                algorithmId: "unshield",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef,
                version: 2,
                verified: true,
                schemaByte: 0x56
            ),
            privacyBuildResult(
                proof: Data(),
                algorithmId: "unshield",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef,
                status: 1,
                errorCode: 6,
                message: "privacy proof verification failed",
                schemaByte: 0x56
            ),
            privacyBuildResult(
                proof: fixture.envelopeArchive,
                algorithmId: "confidential-transfer-v2",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef,
                verified: true,
                schemaByte: 0x56
            ),
            privacyBuildResult(
                proof: fixture.envelopeArchive,
                algorithmId: "unshield",
                entrypoint: "buildConfidentialTransferProofV2",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef,
                verified: true,
                schemaByte: 0x56
            ),
            privacyBuildResult(
                proof: fixture.envelopeArchive,
                algorithmId: "unshield",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialTransferV2VerifierRef,
                verified: true,
                schemaByte: 0x56
            ),
            privacyBuildResult(
                proof: fixture.envelopeArchive,
                algorithmId: "unshield",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef,
                publicInputs: Data([0x01]),
                verified: true,
                schemaByte: 0x56
            ),
            privacyBuildResult(
                proof: Data([0x01]),
                algorithmId: "unshield",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef,
                verified: true,
                schemaByte: 0x56
            ),
            privacyBuildResult(
                proof: fixture.envelopeArchive,
                algorithmId: "unshield",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef,
                verified: false,
                schemaByte: 0x56
            ),
            privacyBuildResult(
                proof: fixture.envelopeArchive,
                algorithmId: "unshield",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef,
                verified: true,
                trailingField: Data([0x00]),
                schemaByte: 0x56
            ),
            privacyBuildResult(
                proof: fixture.envelopeArchive,
                algorithmId: "unshield",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef,
                verified: true,
                flags: 0,
                schemaByte: 0x56
            ),
            privacyBuildResult(
                proof: fixture.envelopeArchive,
                algorithmId: "unshield",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef,
                verified: true,
                schemaByte: 0x55
            )
        ]
        for invalidVerifyResult in invalidVerifyResults {
            XCTAssertThrowsError(
                try KagemushaRecursiveSpendRequestCodecs.buildRedeemProofAttachment(
                    unshieldProofOutputArchive: fixture.proofOutputArchive,
                    unshieldVerifierRecord: fixture.verifierRecord,
                    verifyProof: { _ in invalidVerifyResult }
                )
            ) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveSpendRequestCodecError,
                    .invalidArchive("unshieldProofVerification")
                )
            }
        }

        let allZeroProofEnvelope = openVerifyEnvelope(
            vkHash: fixture.commitment,
            proof: Data(repeating: 0, count: 64),
            circuitId: KagemushaRecursiveSpendRequestCodecs.confidentialUnshieldV3CircuitId,
            schema: PrivacyConfidentialWitnessCodecs.confidentialUnshieldPublicInputsSchema()
        )
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRequestCodecs.buildRedeemProofAttachment(
                unshieldProofOutputArchive: unshieldProofOutput(envelope: allZeroProofEnvelope),
                unshieldVerifierRecord: fixture.verifierRecord,
                verifyProof: { _ in invalidVerifyResults[1] }
            )
        )

        let falselySuccessfulZeroProofResult = privacyBuildResult(
            proof: allZeroProofEnvelope,
            algorithmId: "unshield",
            entrypoint: "buildConfidentialUnshieldProofV3",
            vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef,
            verified: true,
            schemaByte: 0x56
        )
        var zeroProofVerifyWasCalled = false
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRequestCodecs.buildRedeemProofAttachment(
                unshieldProofOutputArchive: unshieldProofOutput(envelope: allZeroProofEnvelope),
                unshieldVerifierRecord: fixture.verifierRecord,
                verifyProof: { _ in
                    zeroProofVerifyWasCalled = true
                    return falselySuccessfulZeroProofResult
                }
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("unshield proof")
            )
        }
        XCTAssertTrue(zeroProofVerifyWasCalled)
    }

    func testPublicBuildRedeemProofAttachmentFailsClosedForCanonicalKeySubstitutionAndZeroProof() throws {
        let fixture = try unshieldProofFixture()
        let substitutedKey = Data((0..<96).map { UInt8(($0 * 17 + 9) & 0xff) })
        let substitutedCommitment = verifyingKeyCommitment(
            backend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
            bytes: substitutedKey
        )
        let substitutedEnvelope = openVerifyEnvelope(
            vkHash: substitutedCommitment,
            circuitId: KagemushaRecursiveSpendRequestCodecs.confidentialUnshieldV3CircuitId,
            schema: PrivacyConfidentialWitnessCodecs.confidentialUnshieldPublicInputsSchema()
        )
        let substitutedRecord = try unshieldVerifierRecord(verifierKey: substitutedKey)

        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRequestCodecs.buildRedeemProofAttachment(
                unshieldProofOutputArchive: unshieldProofOutput(envelope: substitutedEnvelope),
                unshieldVerifierRecord: substitutedRecord
            )
        )

        let allZeroProofEnvelope = openVerifyEnvelope(
            vkHash: fixture.commitment,
            proof: Data(repeating: 0, count: 64),
            circuitId: KagemushaRecursiveSpendRequestCodecs.confidentialUnshieldV3CircuitId,
            schema: PrivacyConfidentialWitnessCodecs.confidentialUnshieldPublicInputsSchema()
        )
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRequestCodecs.buildRedeemProofAttachment(
                unshieldProofOutputArchive: unshieldProofOutput(envelope: allZeroProofEnvelope),
                unshieldVerifierRecord: fixture.verifierRecord
            )
        )
    }

    func testBuildRedeemProofAttachmentRejectsAdversarialUnshieldEnvelopes() throws {
        let fixture = try unshieldProofFixture()
        let circuitId = KagemushaRecursiveSpendRequestCodecs.confidentialUnshieldV3CircuitId
        let schema = PrivacyConfidentialWitnessCodecs.confidentialUnshieldPublicInputsSchema()

        let malformedEnvelopes = [
            Data([0x00]),
            openVerifyEnvelope(
                vkHash: fixture.commitment,
                circuitId: circuitId,
                schema: schema,
                backendTag: VerifyingKeyBackendTag.halo2Bn254.rawValue
            ),
            openVerifyEnvelope(
                vkHash: Data(repeating: 0, count: 32),
                circuitId: circuitId,
                schema: schema
            ),
            openVerifyEnvelope(
                vkHash: fixture.commitment,
                proof: Data(),
                circuitId: circuitId,
                schema: schema
            ),
            openVerifyEnvelope(
                vkHash: fixture.commitment,
                circuitId: circuitId,
                schema: schema,
                aux: Data([0x01])
            ),
            openVerifyEnvelope(
                vkHash: fixture.commitment,
                circuitId: circuitId,
                schema: schema,
                trailingField: Data([0x00])
            ),
            openVerifyEnvelope(
                vkHash: fixture.commitment,
                circuitId: circuitId,
                schema: schema,
                flags: 0
            ),
            openVerifyEnvelope(
                vkHash: fixture.commitment,
                circuitId: circuitId,
                schema: schema,
                archiveTypeName: "test.WrongOpenVerifyEnvelope"
            ),
            archiveWithHeaderPadding(
                openVerifyEnvelope(
                    vkHash: fixture.commitment,
                    circuitId: circuitId,
                    schema: schema
                ),
                count: NoritoHeader.maxHeaderPadding + 1
            )
        ]
        for malformedEnvelope in malformedEnvelopes {
            assertRedeemProofAttachmentRejected(
                proofOutputArchive: unshieldProofOutput(envelope: malformedEnvelope),
                verifierRecord: fixture.verifierRecord,
                expected: .invalidArchive("unshield proof")
            )
        }

        let crossWiredEnvelopes = [
            openVerifyEnvelope(
                vkHash: fixture.commitment,
                circuitId: KagemushaRecursiveSpendRequestCodecs.confidentialTransferV2CircuitId,
                schema: schema
            ),
            openVerifyEnvelope(
                vkHash: fixture.commitment,
                circuitId: circuitId,
                schema: PrivacyConfidentialWitnessCodecs.confidentialTransferPublicInputsSchema()
            )
        ]
        for crossWiredEnvelope in crossWiredEnvelopes {
            assertRedeemProofAttachmentRejected(
                proofOutputArchive: unshieldProofOutput(envelope: crossWiredEnvelope),
                verifierRecord: fixture.verifierRecord,
                expected: .invalidArchive("unshieldVerifierRecord")
            )
        }
    }

    func testBuildRedeemProofAttachmentValidatesInlineKeyAndProofCapBoundaries() throws {
        let fixture = try unshieldProofFixture()
        let key = verifierKey()
        let otherKey = Data((0..<96).map { UInt8(($0 * 13 + 7) & 0xff) })

        let invalidInlineKeys = [
            try unshieldVerifierRecord(verifierKey: key, inlineKey: .absent),
            try unshieldVerifierRecord(
                verifierKey: key,
                inlineKey: .explicit(backend: "test", bytes: key)
            ),
            try unshieldVerifierRecord(
                verifierKey: key,
                inlineKey: .explicit(
                    backend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                    bytes: Data()
                )
            ),
            try unshieldVerifierRecord(
                verifierKey: key,
                vkLen: UInt32(key.count - 1)
            ),
            try unshieldVerifierRecord(
                verifierKey: key,
                inlineKey: .explicit(
                    backend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                    bytes: otherKey
                )
            )
        ]
        for invalidInlineKey in invalidInlineKeys {
            assertRedeemProofAttachmentRejected(
                proofOutputArchive: fixture.proofOutputArchive,
                verifierRecord: invalidInlineKey,
                expected: .invalidArchive("unshieldVerifierRecord.key")
            )
        }

        let exactProofCap = try unshieldVerifierRecord(
            verifierKey: key,
            maxProofBytes: UInt32(fixture.envelopeArchive.count)
        )
        XCTAssertNoThrow(
            try KagemushaRecursiveSpendRequestCodecs.buildRedeemProofAttachmentStructurally(
                unshieldProofOutputArchive: fixture.proofOutputArchive,
                unshieldVerifierRecord: exactProofCap
            )
        )
    }

    func testBuildRedeemProofAttachmentRejectsNoncanonicalTypedArchivePadding() throws {
        let fixture = try unshieldProofFixture()

        assertRedeemProofAttachmentRejected(
            proofOutputArchive: archiveWithHeaderPadding(fixture.proofOutputArchive, count: 1),
            verifierRecord: fixture.verifierRecord,
            expected: .invalidArchive("unshieldProofOutputArchive")
        )
        assertRedeemProofAttachmentRejected(
            proofOutputArchive: unshieldProofOutput(
                envelope: archiveWithHeaderPadding(fixture.envelopeArchive, count: 1)
            ),
            verifierRecord: fixture.verifierRecord,
            expected: .invalidArchive("unshield proof")
        )
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendVerifierRecordRef(
                verifierKeyId: fixture.verifierRecord.verifierKeyId,
                recordBytes: archiveWithHeaderPadding(
                    fixture.verifierRecord.recordBytes,
                    count: 1
                )
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("recordBytes")
            )
        }
    }

    func testBuildRedeemProofAttachmentRejectsElementFramedDirectFixed32Fields() throws {
        let fixture = try unshieldProofFixture()
        let envelopePayload = try KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            fixture.envelopeArchive,
            schema: KagemushaRecursiveSpendRequestCodecs.openVerifyEnvelopeWireName,
            field: "envelope"
        )
        var envelopeFields = try compactFields(envelopePayload)
        envelopeFields[2] = encodeFixed32Payload(fixture.commitment)
        let elementFramedVkHashEnvelope = verifierRecordArchive(
            fields: envelopeFields,
            typeName: KagemushaRecursiveSpendRequestCodecs.openVerifyEnvelopeWireName
        )
        assertRedeemProofAttachmentRejected(
            proofOutputArchive: unshieldProofOutput(envelope: elementFramedVkHashEnvelope),
            verifierRecord: fixture.verifierRecord,
            expected: .invalidArchive("unshield proof.vk_hash")
        )

        let recordPayload = try KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            fixture.verifierRecord.recordBytes,
            schema: KagemushaRecursiveSpendRequestCodecs.verifyingKeyRecordWireName,
            field: "record"
        )
        var recordFields = try compactFields(recordPayload)
        recordFields[6] = encodeFixed32Payload(
            IrohaHash.hash(PrivacyConfidentialWitnessCodecs.confidentialUnshieldPublicInputsSchema())
        )
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendVerifierRecordRef(
                verifierKeyId: fixture.verifierRecord.verifierKeyId,
                recordBytes: verifierRecordArchive(fields: recordFields)
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("verifierRecord.public_inputs_schema_hash")
            )
        }

        recordFields = try compactFields(recordPayload)
        recordFields[7] = encodeFixed32Payload(fixture.commitment)
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendVerifierRecordRef(
                verifierKeyId: fixture.verifierRecord.verifierKeyId,
                recordBytes: verifierRecordArchive(fields: recordFields)
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("verifierRecord.commitment")
            )
        }
    }

    func testBuildRedeemProofAttachmentEnforcesVerifierLifecycleWindow() throws {
        let fixture = try unshieldProofFixture()
        let windowed = try unshieldVerifierRecord(
            verifierKey: verifierKey(),
            activationHeight: 10,
            withdrawHeight: 20
        )

        for height in [nil, UInt64(9), UInt64(20)] {
            assertRedeemProofAttachmentRejected(
                proofOutputArchive: fixture.proofOutputArchive,
                verifierRecord: windowed,
                blockHeight: height,
                expected: .invalidArchive("unshieldVerifierRecord")
            )
        }
        for height in [UInt64(10), UInt64(19)] {
            XCTAssertNoThrow(
                try KagemushaRecursiveSpendRequestCodecs.buildRedeemProofAttachmentStructurally(
                    unshieldProofOutputArchive: fixture.proofOutputArchive,
                    unshieldVerifierRecord: windowed,
                    blockHeight: height
                )
            )
        }

        XCTAssertThrowsError(
            try unshieldVerifierRecord(
                verifierKey: verifierKey(),
                activationHeight: 20,
                withdrawHeight: 20
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("verifierRecord")
            )
        }
    }

    func testBuildRedeemProofAttachmentRejectsMalformedVerifierRecordArchivesAndIds() throws {
        let fixture = try unshieldProofFixture()
        let canonicalArchive = fixture.verifierRecord.recordBytes
        let canonicalPayload = try KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            canonicalArchive,
            schema: KagemushaRecursiveSpendRequestCodecs.verifyingKeyRecordWireName,
            field: "record"
        )
        let canonicalFields = try compactFields(canonicalPayload)

        XCTAssertThrowsError(
            try KagemushaRecursiveSpendVerifierRecordRef(
                verifierKeyId: fixture.verifierRecord.verifierKeyId,
                recordBytes: verifierRecordArchive(fields: canonicalFields, flags: 0)
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("verifierRecord")
            )
        }

        XCTAssertThrowsError(
            try KagemushaRecursiveSpendVerifierRecordRef(
                verifierKeyId: fixture.verifierRecord.verifierKeyId,
                recordBytes: verifierRecordArchive(fields: canonicalFields + [Data([0])])
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("verifierRecord")
            )
        }

        var invalidOptionFields = canonicalFields
        invalidOptionFields[2] = Data([2])
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendVerifierRecordRef(
                verifierKeyId: fixture.verifierRecord.verifierKeyId,
                recordBytes: verifierRecordArchive(fields: invalidOptionFields)
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("optionString")
            )
        }

        for verifierKeyId in ["halo2/ipa:Unshield-v3", "halo2//ipa:unshield-v3"] {
            XCTAssertThrowsError(
                try KagemushaRecursiveSpendVerifierRecordRef(
                    verifierKeyId: verifierKeyId,
                    recordBytes: canonicalArchive
                )
            ) { error in
                guard case let KagemushaRecursiveSpendRequestCodecError.invalidField(field) = error else {
                    return XCTFail("unexpected error: \(error)")
                }
                XCTAssertTrue(field.hasPrefix("verifierKeyId."))
            }
        }

        let wrongSchemaArchive = verifierRecordArchive(
            fields: canonicalFields,
            typeName: "test.WrongVerifierRecord"
        )
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendVerifierRecordRef(
                verifierKeyId: fixture.verifierRecord.verifierKeyId,
                recordBytes: wrongSchemaArchive
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("recordBytes")
            )
        }

        let excessivePaddingArchive = archiveWithHeaderPadding(
            canonicalArchive,
            count: NoritoHeader.maxHeaderPadding + 1
        )
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendVerifierRecordRef(
                verifierKeyId: fixture.verifierRecord.verifierKeyId,
                recordBytes: excessivePaddingArchive
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("recordBytes")
            )
        }

    }

    func testRejectsLegacyOneByteVerifierRecordStatus() throws {
        let verifierKey = verifierKey()
        let verifierRecord = try oneByteStatusRecord(
            KagemushaRecursiveSpendRequestCodecs
                .encodeConfidentialTransferV2VerifierRecordArchive(verifierKey: verifierKey)
        )
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendVerifierRecordRef(
                verifierKeyId: "halo2/ipa:transfer-v2",
                recordBytes: verifierRecord
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("truncated")
            )
        }
    }

    func testVerifiedFoldRecordBundleRejectsAdversarialHopEvidence() throws {
        let first = try transferHop(rootBefore: fixed32(0x31), rootAfter: fixed32(0x32))
        let linked = try transferHop(rootBefore: fixed32(0x32), rootAfter: fixed32(0x33))
        let unlinked = try transferHop(rootBefore: fixed32(0x34), rootAfter: fixed32(0x35))

        XCTAssertThrowsError(try KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(hops: [
            try transferHop(
                rootBefore: fixed32(0x40),
                rootAfter: fixed32(0x41),
                extraColumns: [fixed32(0x42)]
            )
        ]))
        XCTAssertThrowsError(try KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(hops: [
            try transferHop(rootBefore: fixed32(0x50), rootAfter: fixed32(0x50))
        ]))
        XCTAssertThrowsError(try KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(hops: [
            first,
            unlinked
        ]))
        XCTAssertThrowsError(try KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(hops: [
            first,
            try transferHop(
                rootBefore: fixed32(0x32),
                rootAfter: fixed32(0x33),
                chainId: "swift-kagemusha-other-chain"
            )
        ]))
        XCTAssertThrowsError(try KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(hops: [
            first,
            try transferHop(
                rootBefore: fixed32(0x32),
                rootAfter: fixed32(0x33),
                asset: assetDefinitionId(seed: 0x41)
            )
        ]))
        XCTAssertNoThrow(try KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(hops: [
            first,
            linked
        ]))

        let inactive = try transferHop(
            rootBefore: fixed32(0x60),
            rootAfter: fixed32(0x61),
            verifierStatus: 2
        )
        XCTAssertThrowsError(try KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(hops: [
            inactive
        ]))

        let wrongAlgorithm = try transferHop(
            rootBefore: fixed32(0x70),
            rootAfter: fixed32(0x71),
            algorithmId: "unshield",
            entrypoint: "buildConfidentialUnshieldProofV3"
        )
        XCTAssertThrowsError(try KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(hops: [
            wrongAlgorithm
        ]))

        let verifierKey = verifierKey()
        let otherKey = Data((0..<96).map { UInt8(($0 * 11 + 5) & 0xff) })
        let envelope = openVerifyEnvelope(
            vkHash: verifyingKeyCommitment(
                backend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                bytes: verifierKey
            ),
            proof: zk1Proof(rootBefore: fixed32(0x80))
        )
        let mismatchedRecord = try KagemushaRecursiveSpendRequestCodecs
            .encodeConfidentialTransferV2VerifierRecordArchive(verifierKey: otherKey)
        let mismatchedRef = try KagemushaRecursiveSpendVerifierRecordRef(
            verifierKeyId: "halo2/ipa:transfer-v2",
            recordBytes: mismatchedRecord
        )
        let mismatchedHop = try KagemushaVerifiedFoldHopEvidence(
            proofOutputArchive: privacyBuildResult(proof: envelope),
            verifierRecord: mismatchedRef,
            chainId: "swift-kagemusha-chain",
            assetDefinitionId: assetDefinitionId(),
            rootAfter: fixed32(0x81)
        )
        XCTAssertThrowsError(try KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(hops: [
            mismatchedHop
        ]))
    }

    private func assetDefinitionId(seed: UInt8 = 0x01) -> String {
        var bytes = Data((0..<16).map { UInt8(Int(seed) + $0) })
        bytes[6] = (bytes[6] & 0x0f) | 0x40
        bytes[8] = (bytes[8] & 0x3f) | 0x80
        return AssetDefinitionAddress.encode(uuidBytes: bytes)!
    }

    private func verifierKey() -> Data {
        Data((0..<96).map { UInt8(($0 * 7 + 3) & 0xff) })
    }

    private struct UnshieldProofFixture {
        let envelopeArchive: Data
        let proofOutputArchive: Data
        let verifierRecord: KagemushaRecursiveSpendVerifierRecordRef
        let commitment: Data
    }

    private enum InlineVerifierKey {
        case canonical
        case absent
        case explicit(backend: String, bytes: Data)
    }

    private func unshieldProofFixture() throws -> UnshieldProofFixture {
        let verifierKey = verifierKey()
        let commitment = verifyingKeyCommitment(
            backend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
            bytes: verifierKey
        )
        let envelope = openVerifyEnvelope(
            vkHash: commitment,
            circuitId: KagemushaRecursiveSpendRequestCodecs.confidentialUnshieldV3CircuitId,
            schema: PrivacyConfidentialWitnessCodecs.confidentialUnshieldPublicInputsSchema()
        )
        return try UnshieldProofFixture(
            envelopeArchive: envelope,
            proofOutputArchive: privacyBuildResult(
                proof: envelope,
                algorithmId: "unshield",
                entrypoint: "buildConfidentialUnshieldProofV3",
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef
            ),
            verifierRecord: unshieldVerifierRecord(verifierKey: verifierKey),
            commitment: commitment
        )
    }

    private func unshieldVerifierRecord(
        verifierKey: Data,
        circuitId: String = KagemushaRecursiveSpendRequestCodecs.confidentialUnshieldV3CircuitId,
        schema: Data = PrivacyConfidentialWitnessCodecs.confidentialUnshieldPublicInputsSchema(),
        publicInputsSchemaHash: Data? = nil,
        status: UInt32 = 1,
        maxProofBytes: UInt32 = 196_608,
        namespace: String = "offline_kagemusha",
        backendTag: UInt32 = VerifyingKeyBackendTag.halo2IpaPasta.rawValue,
        curve: String = "pallas",
        verifierKeyId: String = "halo2/ipa:unshield-v3",
        vkLen: UInt32? = nil,
        activationHeight: UInt64? = nil,
        withdrawHeight: UInt64? = nil,
        inlineKey: InlineVerifierKey = .canonical
    ) throws -> KagemushaRecursiveSpendVerifierRecordRef {
        let transferRecord = try KagemushaRecursiveSpendRequestCodecs
            .encodeConfidentialTransferV2VerifierRecordArchive(
                verifierKey: verifierKey
            )
        let payload = try KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            transferRecord,
            schema: KagemushaRecursiveSpendRequestCodecs.verifyingKeyRecordWireName,
            field: "record"
        )
        var fields = try compactFields(payload)
        fields[1] = OfflineCompactNorito.encodeString(circuitId)
        fields[3] = OfflineCompactNorito.encodeString(namespace)
        fields[4] = OfflineCompactNorito.encodeUInt32(backendTag)
        fields[5] = OfflineCompactNorito.encodeString(curve)
        fields[6] = publicInputsSchemaHash ?? IrohaHash.hash(schema)
        fields[8] = OfflineCompactNorito.encodeUInt32(vkLen ?? UInt32(verifierKey.count))
        fields[9] = OfflineCompactNorito.encodeUInt32(maxProofBytes)
        fields[13] = activationHeight.map {
            optionPayload(OfflineCompactNorito.encodeUInt64($0))
        } ?? Data([0])
        fields[14] = withdrawHeight.map {
            optionPayload(OfflineCompactNorito.encodeUInt64($0))
        } ?? Data([0])
        switch inlineKey {
        case .canonical:
            break
        case .absent:
            fields[15] = Data([0])
        case let .explicit(backend, bytes):
            fields[15] = optionPayload(verifyingKeyBoxPayload(backend: backend, bytes: bytes))
        }
        fields[16] = OfflineCompactNorito.encodeUInt32(status)
        var writer = OfflineCompactNoritoWriter()
        fields.forEach { writer.writeField($0) }
        return try KagemushaRecursiveSpendVerifierRecordRef(
            verifierKeyId: verifierKeyId,
            recordBytes: noritoEncode(
                typeName: KagemushaRecursiveSpendRequestCodecs.verifyingKeyRecordWireName,
                payload: writer.data,
                flags: NoritoHeader.compactLen
            )
        )
    }

    private func transferHop(
        rootBefore: Data,
        rootAfter: Data,
        chainId: String = "swift-kagemusha-chain",
        asset: String? = nil,
        extraColumns: [Data] = [],
        verifierStatus: UInt32 = 1,
        algorithmId: String = "confidential-transfer-v2",
        entrypoint: String = "buildConfidentialTransferProofV2"
    ) throws -> KagemushaVerifiedFoldHopEvidence {
        let verifierKey = verifierKey()
        let vkHash = verifyingKeyCommitment(
            backend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
            bytes: verifierKey
        )
        let envelope = openVerifyEnvelope(
            vkHash: vkHash,
            proof: zk1Proof(rootBefore: rootBefore, extraColumns: extraColumns)
        )
        var verifierRecord = try KagemushaRecursiveSpendRequestCodecs
            .encodeConfidentialTransferV2VerifierRecordArchive(verifierKey: verifierKey)
        if verifierStatus != 1 {
            verifierRecord = try statusRecord(verifierRecord, status: verifierStatus)
        }
        let ref = try KagemushaRecursiveSpendVerifierRecordRef(
            verifierKeyId: "halo2/ipa:transfer-v2",
            recordBytes: verifierRecord
        )
        return try KagemushaVerifiedFoldHopEvidence(
            proofOutputArchive: privacyBuildResult(
                proof: envelope,
                algorithmId: algorithmId,
                entrypoint: entrypoint
            ),
            verifierRecord: ref,
            chainId: chainId,
            assetDefinitionId: asset ?? assetDefinitionId(),
            rootAfter: rootAfter
        )
    }

    private func openVerifyEnvelope(
        vkHash: Data,
        proof: Data? = nil,
        circuitId: String = KagemushaRecursiveSpendRequestCodecs.confidentialTransferV2CircuitId,
        schema: Data = PrivacyConfidentialWitnessCodecs.confidentialTransferPublicInputsSchema(),
        backendTag: UInt32 = VerifyingKeyBackendTag.halo2IpaPasta.rawValue,
        aux: Data = Data(),
        trailingField: Data? = nil,
        flags: UInt8 = NoritoHeader.compactLen,
        archiveTypeName: String = KagemushaRecursiveSpendRequestCodecs.openVerifyEnvelopeWireName
    ) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeUInt32(backendTag))
        writer.writeField(OfflineCompactNorito.encodeString(circuitId))
        writer.writeField(vkHash)
        writer.writeField(byteVec(schema))
        writer.writeField(byteVec(proof ?? zk1Proof()))
        writer.writeField(byteVec(aux))
        if let trailingField {
            writer.writeField(trailingField)
        }
        return noritoEncode(
            typeName: archiveTypeName,
            payload: writer.data,
            flags: flags
        )
    }

    private func privacyBuildResult(
        proof: Data,
        algorithmId: String = "confidential-transfer-v2",
        entrypoint: String = "buildConfidentialTransferProofV2",
        vkRef: String = PrivacyConfidentialWitnessCodecs.confidentialTransferV2VerifierRef,
        version: UInt32 = 1,
        status: UInt32 = 0,
        errorCode: UInt32 = 0,
        message: String = "",
        publicInputs: Data = Data(),
        verified: Bool = false,
        trailingField: Data? = nil,
        flags: UInt8 = NoritoHeader.compactLen,
        schemaByte: UInt8 = 0x42
    ) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeUInt32(version))
        writer.writeField(OfflineCompactNorito.encodeUInt32(status))
        writer.writeField(OfflineCompactNorito.encodeUInt32(errorCode))
        writer.writeField(OfflineCompactNorito.encodeString(message))
        writer.writeField(OfflineCompactNorito.encodeString(algorithmId))
        writer.writeField(OfflineCompactNorito.encodeString(entrypoint))
        writer.writeField(OfflineCompactNorito.encodeString(vkRef))
        writer.writeField(byteVec(publicInputs))
        writer.writeField(byteVec(proof))
        writer.writeField(Data([verified ? 1 : 0]))
        if let trailingField {
            writer.writeField(trailingField)
        }
        var archive = noritoEncode(
            typeName: "connect_norito_bridge::PrivacyBuildProofResultV1",
            payload: writer.data,
            flags: flags
        )
        archive.replaceSubrange(6..<22, with: Data(repeating: schemaByte, count: 16))
        return archive
    }

    private func unshieldProofOutput(envelope: Data) -> Data {
        privacyBuildResult(
            proof: envelope,
            algorithmId: "unshield",
            entrypoint: "buildConfidentialUnshieldProofV3",
            vkRef: PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef
        )
    }

    private func assertRedeemProofAttachmentRejected(
        proofOutputArchive: Data,
        verifierRecord: KagemushaRecursiveSpendVerifierRecordRef,
        blockHeight: UInt64? = nil,
        expected: KagemushaRecursiveSpendRequestCodecError,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRequestCodecs.buildRedeemProofAttachmentStructurally(
                unshieldProofOutputArchive: proofOutputArchive,
                unshieldVerifierRecord: verifierRecord,
                blockHeight: blockHeight
            ),
            file: file,
            line: line
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                expected,
                file: file,
                line: line
            )
        }
    }

    private func verifyingKeyBoxPayload(backend: String, bytes: Data) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(backend))
        writer.writeField(byteVec(bytes))
        return writer.data
    }

    private func optionPayload(_ payload: Data) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeUInt8(1)
        writer.writeField(payload)
        return writer.data
    }

    private func encodeFixed32Payload(_ bytes: Data) -> Data {
        precondition(bytes.count == 32)
        var writer = OfflineCompactNoritoWriter()
        for byte in bytes {
            writer.writeField(Data([byte]))
        }
        return writer.data
    }

    private func optionFixed32Payload(_ payload: Data) throws -> Data {
        try fixed32Payload(optionSomePayload(payload))
    }

    private func fixed32Payload(_ payload: Data) throws -> Data {
        let fields = try compactFields(payload)
        guard fields.count == 32, fields.allSatisfy({ $0.count == 1 }) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("testFixed32")
        }
        return Data(fields.map { $0[$0.startIndex] })
    }

    private func zk1Proof(rootBefore: Data? = nil, extraColumns: [Data] = []) -> Data {
        var instance = Data()
        appendUInt32LE(UInt32(9 + extraColumns.count), to: &instance)
        appendUInt32LE(1, to: &instance)
        var columns = [
            fixed32(0x00),
            fixed32(0x00),
            fixed32(0x03),
            fixed32(0x00),
            fixed32(0x04),
            fixed32(0x00),
            rootBefore ?? fixed32(0x05),
            fixed32(0x06),
            fixed32(0x07),
        ]
        columns.append(contentsOf: extraColumns)
        columns.forEach { instance.append($0) }

        var out = Data([0x5a, 0x4b, 0x31, 0x00])
        appendTlv(tag: "PROF", payload: Data([0xaa]), to: &out)
        appendTlv(tag: "I10P", payload: instance, to: &out)
        return out
    }

    private func oneByteStatusRecord(_ record: Data) throws -> Data {
        let payload = try KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            record,
            schema: KagemushaRecursiveSpendRequestCodecs.verifyingKeyRecordWireName,
            field: "record"
        )
        var fields = try compactFields(payload)
        fields[fields.count - 1] = Data([1])
        var writer = OfflineCompactNoritoWriter()
        fields.forEach { writer.writeField($0) }
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.verifyingKeyRecordWireName,
            payload: writer.data,
            flags: NoritoHeader.compactLen
        )
    }

    private func verifierRecordArchive(
        fields: [Data],
        flags: UInt8 = NoritoHeader.compactLen,
        typeName: String = KagemushaRecursiveSpendRequestCodecs.verifyingKeyRecordWireName
    ) -> Data {
        var writer = OfflineCompactNoritoWriter()
        fields.forEach { writer.writeField($0) }
        return noritoEncode(typeName: typeName, payload: writer.data, flags: flags)
    }

    private func archiveWithHeaderPadding(_ archive: Data, count: Int) -> Data {
        precondition(count >= 0)
        var padded = archive
        padded.insert(
            contentsOf: Data(repeating: 0, count: count),
            at: NoritoHeader.encodedLength
        )
        return padded
    }

    private func statusRecord(_ record: Data, status: UInt32) throws -> Data {
        let payload = try KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            record,
            schema: KagemushaRecursiveSpendRequestCodecs.verifyingKeyRecordWireName,
            field: "record"
        )
        var fields = try compactFields(payload)
        fields[fields.count - 1] = OfflineCompactNorito.encodeUInt32(status)
        var writer = OfflineCompactNoritoWriter()
        fields.forEach { writer.writeField($0) }
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.verifyingKeyRecordWireName,
            payload: writer.data,
            flags: NoritoHeader.compactLen
        )
    }

    private func compactFields(_ payload: Data) throws -> [Data] {
        var reader = KagemushaTestCompactReader(payload)
        var fields: [Data] = []
        while reader.remaining > 0 {
            fields.append(try reader.readField())
        }
        return fields
    }

    private func optionSomePayload(_ payload: Data) throws -> Data {
        guard payload.first == 1 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("testOption")
        }
        var reader = KagemushaTestCompactReader(Data(payload.dropFirst()))
        let value = try reader.readField()
        guard reader.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("testOption")
        }
        return value
    }

    private func sharedRecursiveSpendArchive(name: String) throws -> Data {
        var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        for _ in 0..<10 {
            let candidate = directory
                .appendingPathComponent("fixtures/kagemusha_recursive_spend_abi6/archives.json")
            if FileManager.default.fileExists(atPath: candidate.path) {
                let root = try XCTUnwrap(
                    JSONSerialization.jsonObject(with: Data(contentsOf: candidate)) as? [String: Any]
                )
                let archives = try XCTUnwrap(root["archives"] as? [[String: Any]])
                let entry = try XCTUnwrap(archives.first { $0["name"] as? String == name })
                return try XCTUnwrap(Data(base64Encoded: try XCTUnwrap(entry["bytes_base64"] as? String)))
            }
            directory.deleteLastPathComponent()
        }
        throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("testFixture")
    }

    private func sampleRecipient() throws -> String {
        try AccountAddress
            .fromAccount(publicKey: Data(repeating: 0x2a, count: 32), algorithm: "ed25519")
            .toI105(networkPrefix: 0x02F1)
    }

    private func syntheticLineageVerifierRecord() throws -> KagemushaRecursiveSpendVerifierRecordRef {
        try canonicalKagemushaVerifierRecordRef(
            verifierKeyId: "halo2/ipa:kagemusha-recursive-spend-lineage-test"
        )
    }

    private func byteVec(_ bytes: Data) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeUInt64LE(UInt64(bytes.count))
        writer.writeBytes(bytes)
        return writer.data
    }

    private func fixed32VectorValues(_ payload: Data) throws -> [Data] {
        var reader = KagemushaTestCompactReader(payload)
        let count = Int(try reader.readUInt64LE())
        var values: [Data] = []
        values.reserveCapacity(count)
        for _ in 0..<count {
            let item = try reader.readField()
            XCTAssertEqual(item.count, 64)
            values.append(try constVecFixed32(item))
        }
        XCTAssertEqual(reader.remaining, 0)
        return values
    }

    private func optionFixed32(_ payload: Data) throws -> Data {
        XCTAssertEqual(payload.count, 66)
        XCTAssertEqual(payload.prefix(2), Data([1, 64]))
        return try constVecFixed32(Data(payload.dropFirst(2)))
    }

    private func constVecFixed32(_ payload: Data) throws -> Data {
        try constVecBytes(payload, expectedCount: 32)
    }

    private func constVecBytes(_ payload: Data, expectedCount: Int) throws -> Data {
        var reader = KagemushaTestCompactReader(payload)
        var value = Data()
        while reader.remaining > 0 {
            let byteField = try reader.readField()
            XCTAssertEqual(byteField.count, 1)
            value.append(byteField)
        }
        XCTAssertEqual(value.count, expectedCount)
        return value
    }

    private func fixed32(_ byte: UInt8) -> Data {
        Data(repeating: byte, count: 32)
    }

    private func appendTlv(tag: String, payload: Data, to out: inout Data) {
        out.append(Data(tag.utf8))
        appendUInt32LE(UInt32(payload.count), to: &out)
        out.append(payload)
    }

    private func appendUInt32LE(_ value: UInt32, to out: inout Data) {
        out.append(UInt8(value & 0xff))
        out.append(UInt8((value >> 8) & 0xff))
        out.append(UInt8((value >> 16) & 0xff))
        out.append(UInt8((value >> 24) & 0xff))
    }

    private func verifyingKeyCommitment(backend: String, bytes: Data) -> Data {
        var preimage = Data("iroha:zk:v1:vk".utf8)
        appendUInt64BE(UInt64(backend.utf8.count), to: &preimage)
        preimage.append(Data(backend.utf8))
        appendUInt64BE(UInt64(bytes.count), to: &preimage)
        preimage.append(bytes)
        return Data(SHA256.hash(data: preimage))
    }

    private func appendUInt64BE(_ value: UInt64, to out: inout Data) {
        out.append(UInt8((value >> 56) & 0xff))
        out.append(UInt8((value >> 48) & 0xff))
        out.append(UInt8((value >> 40) & 0xff))
        out.append(UInt8((value >> 32) & 0xff))
        out.append(UInt8((value >> 24) & 0xff))
        out.append(UInt8((value >> 16) & 0xff))
        out.append(UInt8((value >> 8) & 0xff))
        out.append(UInt8(value & 0xff))
    }
}

private struct KagemushaTestCompactReader {
    private let data: Data
    private var offset: Int = 0

    init(_ data: Data) {
        self.data = data
    }

    var remaining: Int {
        data.count - offset
    }

    mutating func readField() throws -> Data {
        try readBytes(Int(readLength()))
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

    private mutating func readLength() throws -> UInt64 {
        var value: UInt64 = 0
        var shift: UInt64 = 0
        while true {
            guard offset < data.count else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("testReader")
            }
            let byte = data[offset]
            offset += 1
            value |= UInt64(byte & 0x7f) << shift
            if (byte & 0x80) == 0 {
                return value
            }
            shift += 7
        }
    }

    private mutating func readBytes(_ count: Int) throws -> Data {
        guard offset + count <= data.count else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("testReader")
        }
        defer { offset += count }
        return Data(data[offset..<(offset + count)])
    }
}
