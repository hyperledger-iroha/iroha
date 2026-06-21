import Foundation
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
        XCTAssertFalse(initBundle.asset.isEmpty)
        XCTAssertTrue(initBundle.initialRoot.contains { $0 != 0 })
        XCTAssertTrue(initBundle.finalRoot.contains { $0 != 0 })
        XCTAssertEqual(initBundle.currentNote.amount, "7")

        let appendBundle = try KagemushaRecursiveSpendRequestCodecs.decodeBundle(
            Self.sharedRecursiveSpendArchive(abi: .abi7, name: "append_bundle")
        )
        XCTAssertGreaterThanOrEqual(appendBundle.hopCount, initBundle.hopCount)
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
                appendBundle.proofCircuitId
            )
        )
        XCTAssertTrue(appendBundle.currentNote.noteCommitment.contains { $0 != 0 })
        XCTAssertTrue(appendBundle.currentNote.spendNullifier.contains { $0 != 0 })
        XCTAssertNotEqual(appendBundle.currentNote.amount, "0")
    }

    func testTypedEncodersWriteExpectedRequestSchemasAndLayouts() throws {
        let recordBundle = Self.syntheticArchive(
            schema: KagemushaRecursiveSpendRequestCodecs.recordBundleWireName
        )
        let pallasOpenEnvelopes = Self.syntheticArchive(schema: "test.PallasOpenEnvelopes")
        let lineageVerifierKey = Data((0..<64).map { UInt8($0 + 1) })
        let lineageProvingKeyArchive = Self.syntheticArchive(schema: "test.LineageProvingKeyArchive")
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
                    bundle: redeemBundle,
                    recipient: try Self.sampleRecipient(),
                    publicAmount: "7",
                    redeemProof: redeemProof
                )
            ),
            schema: KagemushaRecursiveSpendRequestCodecs.redeemRequestWireName
        )
        XCTAssertEqual(exactRedeemFields.count, 8)
        try Self.assertOptionNone(exactRedeemFields[4])
        try Self.assertOptionNone(exactRedeemFields[5])
        try Self.assertOptionNone(exactRedeemFields[6])
        try Self.assertOptionNone(exactRedeemFields[7])

        let verifyFields = try Self.requestFields(
            KagemushaRecursiveSpendRequestCodecs.encodeVerifyRequest(
                KagemushaRecursiveSpendVerifyRequest(
                    bundle: Self.sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle")
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
        try Self.assertOptionNone(verifyFields[1])
        try Self.assertOptionNone(verifyFields[2])
    }

    func testTypedRequestsRejectMalformedInputsBeforeNativeDispatch() throws {
        for amount in ["", "0", "01", "-1", "+1", "1.0", "1e3", Self.u128MaxPlusOne] {
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
        for changeOutput in [Data(repeating: 1, count: 31), Data(repeating: 0, count: 32)] {
            XCTAssertThrowsError(
                try KagemushaRecursiveSpendRedeemRequest(
                    bundle: Self.sharedRecursiveSpendArchive(abi: .abi7, name: "append_bundle"),
                    recipient: Self.sampleRecipient(),
                    publicAmount: "7",
                    redeemProof: Self.syntheticArchive(
                        schema: KagemushaRecursiveSpendRequestCodecs.proofAttachmentWireName
                    ),
                    changeOutput: changeOutput
                )
            )
        }
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
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendInitRequest(
                recordBundle: Self.syntheticArchive(
                    schema: KagemushaRecursiveSpendRequestCodecs.recordBundleWireName
                ),
                pallasOpenEnvelopes: Self.syntheticArchive(schema: "test.PallasOpenEnvelopes"),
                currentNote: Self.sampleNote(),
                lineageVerifierKey: nil,
                lineageProvingKeyArchive: nil
            )
        )

        var corrupted = Self.syntheticArchive(schema: "test.PallasOpenEnvelopes")
        corrupted[corrupted.count - 1] ^= 0x01
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendInitRequest(
                recordBundle: Self.syntheticArchive(
                    schema: KagemushaRecursiveSpendRequestCodecs.recordBundleWireName
                ),
                pallasOpenEnvelopes: corrupted,
                currentNote: Self.sampleNote(),
                lineageVerifierKey: Data(repeating: 0x5a, count: 64),
                lineageProvingKeyArchive: Self.syntheticArchive(schema: "test.LineageProvingKeyArchive")
            )
        )
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRequestCodecs.encodeVerifyRequest(
                KagemushaRecursiveSpendVerifyRequest(
                    bundle: Self.sharedRecursiveSpendArchive(abi: .abi6, name: "verify_result")
                )
            )
        )

        var tamperedBundle = try Self.sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle")
        tamperedBundle[tamperedBundle.count - 1] ^= 0x01
        XCTAssertThrowsError(try KagemushaRecursiveSpendRequestCodecs.decodeBundle(tamperedBundle))

        XCTAssertThrowsError(
            try KagemushaRecursiveSpendAppendRequest(
                previousBundle: Self.sharedRecursiveSpendArchive(abi: .abi6, name: "init_bundle"),
                recordBundle: Self.syntheticArchive(
                    schema: KagemushaRecursiveSpendRequestCodecs.recordBundleWireName
                ),
                pallasOpenEnvelopes: Self.syntheticArchive(schema: "test.PallasOpenEnvelopes"),
                currentNote: Self.sampleNote(seed: 0x41),
                outputProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1,
                previousLineageVerifierRecord: Self.sampleVerifierRecord(),
                previousProofOpenEnvelopes: nil,
                lineageVerifierKey: Data(repeating: 0x6b, count: 64),
                lineageProvingKeyArchive: Self.syntheticArchive(schema: "test.LineageProvingKeyArchive")
            )
        )
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

    private static func fieldPayloads(_ payload: Data) throws -> [Data] {
        var reader = TestCompactReader(data: payload)
        var fields: [Data] = []
        while reader.remaining > 0 {
            fields.append(try reader.readField())
        }
        return fields
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

    private static let u128MaxPlusOne = "340282366920938463463374607431768211456"
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
