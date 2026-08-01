import Foundation
import XCTest

@testable import IrohaSwift

final class ProofAttachmentNoritoTests: XCTestCase {
    func testProofAttachmentNoritoEncodingMatchesManualLayout() throws {
        let attachment = try ProofAttachment(
            backend: "test",
            proof: Data([0x01, 0x02]),
            verifyingKey: .reference(.init(backend: "test", name: "vk"))
        )
        let payload = try attachment.noritoPayload()
        let expectedAttachment = manualProofAttachmentPayload(attachment)
        XCTAssertEqual(payload, expectedAttachment)

    }

    func testProofAttachmentRejectsInvalidCommitmentLength() throws {
        XCTAssertThrowsError(
            try ProofAttachment(
                backend: "test",
                proof: Data([0x01]),
                verifyingKey: .reference(.init(backend: "test", name: "vk")),
                verifyingKeyCommitment: Data(repeating: 0xAA, count: 31)
            )
        ) { error in
            guard case let ProofAttachmentError.invalidVerifyingKeyCommitmentLength(expected, actual) = error else {
                return XCTFail("expected invalidVerifyingKeyCommitmentLength error")
            }
            XCTAssertEqual(expected, 32)
            XCTAssertEqual(actual, 31)
        }
    }

    func testProofAttachmentRejectsEmptyVerifyingKeyBackend() {
        XCTAssertThrowsError(
            try ProofAttachment(
                backend: "test",
                proof: Data([0x01]),
                verifyingKey: .reference(.init(backend: "  ", name: "vk"))
            )
        ) { error in
            guard case ProofAttachmentError.emptyVerifyingKeyBackend = error else {
                return XCTFail("expected emptyVerifyingKeyBackend error")
            }
        }
    }

    func testProofAttachmentRejectsEmptyVerifyingKeyName() {
        XCTAssertThrowsError(
            try ProofAttachment(
                backend: "test",
                proof: Data([0x01]),
                verifyingKey: .reference(.init(backend: "halo2/ipa", name: " "))
            )
        ) { error in
            guard case ProofAttachmentError.emptyVerifyingKeyName = error else {
                return XCTFail("expected emptyVerifyingKeyName error")
            }
        }
    }

    func testProofAttachmentAcceptsCanonicalColonNamespace() throws {
        let attachment = try ProofAttachment(
            backend: "halo2/ipa",
            proof: Data([0x01]),
            verifyingKey: .reference(.init(
                backend: "halo2/ipa",
                name: "halo2/ipa::transfer_v1"
            ))
        )
        XCTAssertEqual(
            attachment.verifyingKey,
            .reference(.init(backend: "halo2/ipa", name: "halo2/ipa::transfer_v1"))
        )
    }

    func testProofAttachmentRejectsNoncanonicalVerifierIdentifiersWithoutNormalization() {
        let invalid = [
            " leading", "trailing ", "Uppercase", ".hidden", "trailing_", "a..b",
            "a//b", "a:::b", "a/:b", "a:/b", "a/.b", "a./b", "a:.b", "a.:b",
            "a\\b", "a\u{200B}b"
        ]
        for name in invalid {
            XCTAssertThrowsError(
                try ProofAttachment(
                    backend: "halo2/ipa",
                    proof: Data([0x01]),
                    verifyingKey: .reference(.init(backend: "halo2/ipa", name: name))
                ),
                "identifier \(name.debugDescription) must fail closed"
            ) { error in
                guard case ProofAttachmentError.invalidVerifyingKeyIdentifier = error else {
                    return XCTFail("expected invalidVerifyingKeyIdentifier for \(name.debugDescription), got \(error)")
                }
            }
        }
    }

    func testProofAttachmentRejectsOversizedVerifierIdentifier() {
        let oversized = String(repeating: "a", count: 257)
        XCTAssertThrowsError(
            try ProofAttachment(
                backend: "halo2/ipa",
                proof: Data([0x01]),
                verifyingKey: .reference(.init(backend: "halo2/ipa", name: oversized))
            )
        ) { error in
            guard case let ProofAttachmentError.verifyingKeyIdentifierTooLong(field, maximum, actual) = error else {
                return XCTFail("expected verifyingKeyIdentifierTooLong, got \(error)")
            }
            XCTAssertEqual(field, "verifyingKey.name")
            XCTAssertEqual(maximum, 256)
            XCTAssertEqual(actual, 257)
        }
    }

    func testProofAttachmentRejectsZeroVerifyingKeyCommitment() {
        XCTAssertThrowsError(
            try ProofAttachment(
                backend: "test",
                proof: Data([0x01]),
                verifyingKey: .reference(.init(backend: "test", name: "vk")),
                verifyingKeyCommitment: Data(repeating: 0, count: 32)
            )
        ) { error in
            guard case ProofAttachmentError.zeroVerifyingKeyCommitment = error else {
                return XCTFail("expected zeroVerifyingKeyCommitment, got \(error)")
            }
        }
    }

    func testProofBoxLimitAppliesToCompleteEncodedField() throws {
        let maximum = 64 * 1024 * 1024
        let backendBytes = "halo2/ipa::transfer_v1".utf8.count
        XCTAssertEqual(ProofAttachment.maximumEncodedProofBoxBytesV1, maximum)
        XCTAssertEqual(
            try ProofAttachment.maximumProofByteCountV1(
                forBackend: "halo2/ipa::transfer_v1"
            ),
            maximum - 36
        )
        let maximumProofBytes = try ProofAttachment.maximumProofByteCount(
            backendUTF8Count: backendBytes
        )
        XCTAssertEqual(maximumProofBytes, maximum - 36)
        XCTAssertEqual(
            try ProofAttachment.canonicalProofBoxEncodedLength(
                backendUTF8Count: backendBytes,
                proofByteCount: maximumProofBytes
            ),
            maximum
        )
        XCTAssertEqual(
            try ProofAttachment.canonicalProofBoxEncodedLength(
                backendUTF8Count: backendBytes,
                proofByteCount: maximumProofBytes + 1
            ),
            maximum + 1
        )
    }

    func testProofBoxCompactLengthTransitionsAreExact() throws {
        XCTAssertEqual(
            try ProofAttachment.canonicalProofBoxEncodedLength(
                backendUTF8Count: 127,
                proofByteCount: 0
            ),
            139
        )
        XCTAssertEqual(
            try ProofAttachment.canonicalProofBoxEncodedLength(
                backendUTF8Count: 128,
                proofByteCount: 0
            ),
            141
        )
        XCTAssertEqual(
            try ProofAttachment.canonicalProofBoxEncodedLength(
                backendUTF8Count: 16_383,
                proofByteCount: 0
            ),
            16_397
        )
        XCTAssertEqual(
            try ProofAttachment.canonicalProofBoxEncodedLength(
                backendUTF8Count: 16_384,
                proofByteCount: 0
            ),
            16_399
        )

        XCTAssertEqual(
            try ProofAttachment.canonicalProofBoxEncodedLength(
                backendUTF8Count: 0,
                proofByteCount: 119
            ),
            130
        )
        XCTAssertEqual(
            try ProofAttachment.canonicalProofBoxEncodedLength(
                backendUTF8Count: 0,
                proofByteCount: 120
            ),
            132
        )
        XCTAssertEqual(
            try ProofAttachment.canonicalProofBoxEncodedLength(
                backendUTF8Count: 0,
                proofByteCount: 16_375
            ),
            16_387
        )
        XCTAssertEqual(
            try ProofAttachment.canonicalProofBoxEncodedLength(
                backendUTF8Count: 0,
                proofByteCount: 16_376
            ),
            16_389
        )
    }

    func testProofAttachmentEncodesCanonicalLanePrivacyThirdTail() throws {
        let rawSibling = Data(repeating: 0x22, count: 32)
        let merkle = try ProofAttachment.LanePrivacyProof.MerkleWitness(
            leaf: Data(repeating: 0xAA, count: 32),
            leafIndex: 1,
            auditPath: [rawSibling, Data(repeating: 0x44, count: 32)]
        )
        XCTAssertEqual(merkle.auditPath[0].last, 0x23)
        XCTAssertEqual(merkle.auditPath[1].last, 0x45)
        let lane = ProofAttachment.LanePrivacyProof(
            commitmentId: 7,
            witness: .merkle(merkle)
        )
        let attachment = try ProofAttachment(
            backend: "halo2/ipa",
            proof: Data([0x01, 0x02]),
            verifyingKey: .reference(.init(backend: "halo2/ipa", name: "vk_transfer")),
            lanePrivacy: lane
        )

        var expected = Data()
        expected.append(manualField(encodeString("halo2/ipa")))
        expected.append(manualField(manualProofBoxPayload(backend: "halo2/ipa", bytes: Data([0x01, 0x02]))))
        expected.append(manualField(manualVerifyingKeyIdPayload(backend: "halo2/ipa", name: "vk_transfer")))
        expected.append(manualField(manualOptionPayload(nil)))
        expected.append(manualField(manualOptionPayload(nil)))
        expected.append(manualField(manualOptionPayload(manualLanePrivacyPayload(lane))))

        XCTAssertEqual(try attachment.noritoPayload(), expected)
    }

    func testLanePrivacyRejectsInvalidMerkleResources() {
        XCTAssertThrowsError(
            try ProofAttachment.LanePrivacyProof.MerkleWitness(
                leaf: Data(repeating: 0xAA, count: 32),
                leafIndex: 0,
                auditPath: []
            )
        ) { error in
            guard case ProofAttachmentError.invalidLanePrivacyPathLength = error else {
                return XCTFail("expected invalidLanePrivacyPathLength, got \(error)")
            }
        }
        XCTAssertThrowsError(
            try ProofAttachment.LanePrivacyProof.MerkleWitness(
                leaf: Data(repeating: 0xAA, count: 32),
                leafIndex: 2,
                auditPath: [Data(repeating: 0x22, count: 32)]
            )
        ) { error in
            guard case ProofAttachmentError.invalidLanePrivacyLeafIndex = error else {
                return XCTFail("expected invalidLanePrivacyLeafIndex, got \(error)")
            }
        }
        XCTAssertThrowsError(
            try ProofAttachment.LanePrivacyProof.MerkleWitness(
                leaf: Data(repeating: 0xAA, count: 32),
                leafIndex: 0,
                auditPath: [Data(repeating: 0x22, count: 31)]
            )
        ) { error in
            guard case ProofAttachmentError.invalidLanePrivacyHashLength = error else {
                return XCTFail("expected invalidLanePrivacyHashLength, got \(error)")
            }
        }
    }

    func testProofAttachmentJsonUsesReferenceOnly() throws {
        let attachment = try ProofAttachment(
            backend: "test",
            proof: Data([0x01]),
            verifyingKey: .reference(.init(backend: "test", name: "vk"))
        )
        let object = try JSONSerialization.jsonObject(with: attachment.encodedJSON()) as? [String: Any]
        XCTAssertNotNil(object?["vk_ref"])
        XCTAssertEqual(object?["envelope_hash_hex"] as? String, IrohaHash.hash(Data([0x01])).hexLowercased())
    }

    func testProofAttachmentJsonAddsCanonicalEnvelopeHash() throws {
        let attachment = try ProofAttachment(
            backend: "test",
            proof: Data([0x01]),
            verifyingKey: .reference(.init(backend: "test", name: "vk"))
        )
        let object = try JSONSerialization.jsonObject(with: attachment.encodedJSON()) as? [String: Any]
        XCTAssertEqual(object?["envelope_hash_hex"] as? String, IrohaHash.hash(Data([0x01])).hexLowercased())
    }

    func testProofAttachmentRejectsEnvelopeHashMismatch() {
        XCTAssertThrowsError(
            try ProofAttachment(
                backend: "test",
                proof: Data([0x01]),
                verifyingKey: .reference(.init(backend: "test", name: "vk")),
                envelopeHash: Data(repeating: 0xAB, count: 32)
            )
        ) { error in
            guard case ProofAttachmentError.envelopeHashMismatch = error else {
                return XCTFail("expected envelopeHashMismatch error")
            }
        }
    }

    func testProofAttachmentRejectsVerifyingKeyBackendMismatch() {
        XCTAssertThrowsError(
            try ProofAttachment(
                backend: "halo2/ipa",
                proof: Data([0x01]),
                verifyingKey: .reference(.init(backend: "stark/fri", name: "vk"))
            )
        ) { error in
            guard case let ProofAttachmentError.verifyingKeyBackendMismatch(expected, actual) = error else {
                return XCTFail("expected verifyingKeyBackendMismatch error")
            }
            XCTAssertEqual(expected, "halo2/ipa")
            XCTAssertEqual(actual, "stark/fri")
        }
    }

    private func manualProofAttachmentPayload(_ attachment: ProofAttachment) -> Data {
        let proofBox = manualProofBoxPayload(backend: attachment.backend, bytes: attachment.proof)
        let vkRef = manualVerifyingKeyIdPayload(backend: "test", name: "vk")

        var writer = Data()
        writer.append(manualField(encodeString(attachment.backend)))
        writer.append(manualField(proofBox))
        writer.append(manualField(vkRef))
        return writer
    }

    private func manualProofBoxPayload(backend: String, bytes: Data) -> Data {
        var payload = Data()
        payload.append(manualField(encodeString(backend)))
        payload.append(manualField(encodeBytesVec(bytes)))
        return payload
    }

    private func manualVerifyingKeyIdPayload(backend: String, name: String) -> Data {
        var payload = Data()
        payload.append(manualField(encodeString(backend)))
        payload.append(manualField(encodeString(name)))
        return payload
    }

    private func manualLanePrivacyPayload(_ lane: ProofAttachment.LanePrivacyProof) -> Data {
        var payload = Data()
        var commitmentId = lane.commitmentId.littleEndian
        payload.append(manualField(Data(bytes: &commitmentId, count: 2)))

        var witness = Data()
        switch lane.witness {
        case .merkle(let merkle):
            var tag = UInt32(0).littleEndian
            witness.append(Data(bytes: &tag, count: 4))

            var merklePayload = Data()
            merklePayload.append(manualField(manualFixedBytes(merkle.leaf)))
            var proof = Data()
            var leafIndex = merkle.leafIndex.littleEndian
            proof.append(manualField(Data(bytes: &leafIndex, count: 4)))
            var auditPath = Data()
            auditPath.append(u64le(UInt64(merkle.auditPath.count)))
            for sibling in merkle.auditPath {
                auditPath.append(manualField(manualOptionPayload(sibling)))
            }
            proof.append(manualField(auditPath))
            merklePayload.append(manualField(proof))
            witness.append(manualField(merklePayload))
        }
        payload.append(manualField(witness))
        return payload
    }

    private func manualFixedBytes(_ bytes: Data) -> Data {
        var out = Data()
        for byte in bytes {
            out.append(u64le(1))
            out.append(byte)
        }
        return out
    }

    private func manualOptionPayload(_ payload: Data?) -> Data {
        guard let payload else { return Data([0x00]) }
        var out = Data([0x01])
        out.append(u64le(UInt64(payload.count)))
        out.append(payload)
        return out
    }

    private func manualField(_ payload: Data) -> Data {
        var out = Data()
        out.append(u64le(UInt64(payload.count)))
        out.append(payload)
        return out
    }

    private func encodeString(_ value: String) -> Data {
        let bytes = Data(value.utf8)
        var out = Data()
        out.append(u64le(UInt64(bytes.count)))
        out.append(bytes)
        return out
    }

    /// Encode Vec<u8> as flat blob: [u64 count][raw bytes].
    /// Rust Vec<u8> NoritoSerialize has a special case that writes bytes flat.
    private func encodeBytesVec(_ bytes: Data) -> Data {
        var out = Data()
        out.append(u64le(UInt64(bytes.count)))
        out.append(bytes)
        return out
    }

    private func u64le(_ value: UInt64) -> Data {
        var le = value.littleEndian
        return Data(bytes: &le, count: MemoryLayout<UInt64>.size)
    }
}
