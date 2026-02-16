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

        let list = OfflineProofAttachmentList(attachments: [attachment])
        let listPayload = try list.noritoPayload()
        var expectedList = Data()
        expectedList.append(u64le(1))
        expectedList.append(u64le(UInt64(expectedAttachment.count)))
        expectedList.append(expectedAttachment)
        XCTAssertEqual(listPayload, expectedList)
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

    func testProofAttachmentRejectsVerifyingKeySeparator() {
        XCTAssertThrowsError(
            try ProofAttachment(
                backend: "test",
                proof: Data([0x01]),
                verifyingKey: .reference(.init(backend: "halo2:ipa", name: "vk"))
            )
        ) { error in
            guard case ProofAttachmentError.invalidVerifyingKeySeparator = error else {
                return XCTFail("expected invalidVerifyingKeySeparator error")
            }
        }
    }

    func testProofAttachmentRejectsInlineVerifyingKeyBytes() {
        XCTAssertThrowsError(
            try ProofAttachment(
                backend: "test",
                proof: Data([0x01]),
                verifyingKey: .inline(.init(backend: "halo2/ipa", bytes: Data()))
            )
        ) { error in
            guard case ProofAttachmentError.emptyVerifyingKeyBytes = error else {
                return XCTFail("expected emptyVerifyingKeyBytes error")
            }
        }
    }

    private func manualProofAttachmentPayload(_ attachment: ProofAttachment) -> Data {
        let proofBox = manualProofBoxPayload(backend: attachment.backend, bytes: attachment.proof)
        let vkRef = manualVerifyingKeyIdPayload(backend: "test", name: "vk")
        let vkRefOption = manualOptionPayload(vkRef)
        let vkInlineOption = manualOptionPayload(nil)

        var writer = Data()
        writer.append(manualField(encodeString(attachment.backend)))
        writer.append(manualField(proofBox))
        writer.append(manualField(vkRefOption))
        writer.append(manualField(vkInlineOption))
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
