import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaVerifierRecordFixturesTests: XCTestCase {
    func testCanonicalFixturesMatchTheirFirstReleaseCircuitProfiles() throws {
        let unshield = try KagemushaRecursiveSpendVerifierRecordRef(
            verifierKeyId: "halo2/ipa:unshield-v3",
            recordBytes: canonicalKagemushaVerifierRecordArchive(seed: 0x71)
        )
        XCTAssertEqual(
            unshield.metadata.circuitId,
            KagemushaRecursiveSpendRequestCodecs.confidentialUnshieldV3CircuitId
        )
        XCTAssertEqual(unshield.metadata.namespace, "offline_kagemusha")
        XCTAssertEqual(unshield.metadata.maxProofBytes, 196_608)
        XCTAssertTrue(unshield.metadata.isActiveStatus)

        let lineage = try canonicalKagemushaVerifierRecordRef(
            verifierKeyId: "halo2/ipa:kagemusha-recursive-spend-lineage-test"
        )
        XCTAssertEqual(
            lineage.metadata.circuitId,
            KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1
        )
        XCTAssertEqual(lineage.metadata.namespace, "offline_kagemusha")
        XCTAssertEqual(lineage.metadata.maxProofBytes, 4_096)
        XCTAssertTrue(lineage.metadata.isActiveStatus)
    }

    func testCanonicalFixturesRejectEmptyVerifierKeys() {
        XCTAssertThrowsError(
            try canonicalKagemushaVerifierRecordArchive(
                seed: 0x71,
                verifierKeyLength: 0
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidField("canonicalVerifierRecord")
            )
        }
    }
}

/// Build a canonical active unshield verifier record for SDK tests.
func canonicalKagemushaVerifierRecordArchive(
    seed: UInt8,
    verifierKeyLength: Int = 96
) throws -> Data {
    try canonicalKagemushaVerifierRecordArchive(
        seed: seed,
        verifierKeyLength: verifierKeyLength,
        circuitId: KagemushaRecursiveSpendRequestCodecs.confidentialUnshieldV3CircuitId,
        ownerManifestId: "confidential-v3",
        publicInputsSchemaHash: IrohaHash.hash(
            PrivacyConfidentialWitnessCodecs.confidentialUnshieldPublicInputsSchema()
        ),
        maxProofBytes: 196_608
    )
}

/// Build the canonical active Reserved-lineage verifier record used by request tests.
func canonicalKagemushaVerifierRecordRef(
    verifierKeyId: String,
    seed: UInt8 = 0x69,
    verifierKeyLength: Int = 96
) throws -> KagemushaRecursiveSpendVerifierRecordRef {
    let ownerManifestId = verifierKeyId.split(separator: ":", maxSplits: 1).last.map(String.init)
    let archive = try canonicalKagemushaVerifierRecordArchive(
        seed: seed,
        verifierKeyLength: verifierKeyLength,
        circuitId: KagemushaRecursiveSpendProver
            .recursiveSpendLineageOneHopProofCircuitIdV1,
        ownerManifestId: ownerManifestId,
        publicInputsSchemaHash: Data([
            // Blake2b-256/IrohaHash of the canonical Rust
            // KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_SCHEMA.
            0x63, 0xeb, 0xe0, 0xdd, 0x60, 0xc2, 0x2b, 0xbe,
            0x1a, 0xc4, 0x7e, 0xc1, 0x84, 0xb4, 0x9d, 0x00,
            0x86, 0x27, 0x7f, 0x18, 0x4a, 0xf4, 0x9e, 0x07,
            0x25, 0x01, 0x48, 0xee, 0x81, 0x17, 0xd9, 0x7d,
        ]),
        maxProofBytes: 4_096
    )
    return try KagemushaRecursiveSpendVerifierRecordRef(
        verifierKeyId: verifierKeyId,
        recordBytes: archive
    )
}

private func canonicalKagemushaVerifierRecordArchive(
    seed: UInt8,
    verifierKeyLength: Int,
    circuitId: String,
    ownerManifestId: String?,
    publicInputsSchemaHash: Data,
    maxProofBytes: UInt32
) throws -> Data {
    guard verifierKeyLength > 0,
          verifierKeyLength <= Int(UInt32.max),
          publicInputsSchemaHash.count == 32,
          maxProofBytes > 0,
          maxProofBytes <= 196_608 else {
        throw KagemushaRecursiveSpendRequestCodecError.invalidField(
            "canonicalVerifierRecord"
        )
    }

    // Start from the SDK's canonical encoder so commitment and inline-key
    // framing stay identical to production, then select the circuit profile.
    let baseArchive = try KagemushaRecursiveSpendRequestCodecs
        .encodeConfidentialTransferV2VerifierRecordArchive(
            verifierKey: Data(repeating: seed, count: verifierKeyLength),
            maxProofBytes: maxProofBytes
        )
    let payload = try KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
        baseArchive,
        schema: KagemushaRecursiveSpendRequestCodecs.verifyingKeyRecordWireName,
        field: "canonicalVerifierRecord"
    )
    var reader = KagemushaVerifierRecordFixtureReader(payload)
    var fields: [Data] = []
    while reader.remaining > 0 {
        fields.append(try reader.readField())
    }
    guard fields.count == 17 else {
        throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
            "canonicalVerifierRecord"
        )
    }

    fields[0] = OfflineCompactNorito.encodeUInt32(3)
    fields[1] = OfflineCompactNorito.encodeString(circuitId)
    fields[2] = kagemushaVerifierRecordOptionString(ownerManifestId)
    fields[6] = publicInputsSchemaHash
    fields[9] = OfflineCompactNorito.encodeUInt32(maxProofBytes)

    var writer = OfflineCompactNoritoWriter()
    fields.forEach { writer.writeField($0) }
    return noritoEncode(
        typeName: KagemushaRecursiveSpendRequestCodecs.verifyingKeyRecordWireName,
        payload: writer.data,
        flags: NoritoHeader.compactLen
    )
}

private func kagemushaVerifierRecordOptionString(_ value: String?) -> Data {
    guard let value else { return Data([0]) }
    var writer = OfflineCompactNoritoWriter()
    writer.writeUInt8(1)
    writer.writeField(OfflineCompactNorito.encodeString(value))
    return writer.data
}

private struct KagemushaVerifierRecordFixtureReader {
    private let data: Data
    private var offset = 0

    init(_ data: Data) {
        self.data = data
    }

    var remaining: Int { data.count - offset }

    mutating func readField() throws -> Data {
        let length = try readLength()
        guard length <= UInt64(remaining), length <= UInt64(Int.max) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                "canonicalVerifierRecord"
            )
        }
        let end = offset + Int(length)
        defer { offset = end }
        return Data(data[offset..<end])
    }

    private mutating func readLength() throws -> UInt64 {
        var value: UInt64 = 0
        var shift: UInt64 = 0
        while shift < 64 {
            guard offset < data.count else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                    "canonicalVerifierRecord"
                )
            }
            let byte = data[offset]
            offset += 1
            let payload = UInt64(byte & 0x7f)
            guard shift < 63 || payload <= 1 else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                    "canonicalVerifierRecord"
                )
            }
            value |= payload << shift
            if byte & 0x80 == 0 { return value }
            shift += 7
        }
        throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
            "canonicalVerifierRecord"
        )
    }
}
