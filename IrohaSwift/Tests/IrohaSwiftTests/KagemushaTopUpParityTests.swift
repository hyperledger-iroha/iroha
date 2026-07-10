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
        let record = try KagemushaRecursiveSpendRequestCodecs
            .encodeConfidentialTransferV2VerifierRecordArchive(verifierKey: verifierKey())
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

    func testRejectsLegacyOneByteVerifierRecordStatus() throws {
        let verifierKey = verifierKey()
        let vkHash = verifyingKeyCommitment(backend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                                            bytes: verifierKey)
        let envelope = openVerifyEnvelope(vkHash: vkHash)
        let proofOutput = privacyBuildResult(proof: envelope)
        let verifierRecord = try oneByteStatusRecord(
            KagemushaRecursiveSpendRequestCodecs
                .encodeConfidentialTransferV2VerifierRecordArchive(verifierKey: verifierKey)
        )
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

        XCTAssertThrowsError(
            try KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(hops: [hop])
        )
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

    private func openVerifyEnvelope(vkHash: Data, proof: Data? = nil) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeUInt32(VerifyingKeyBackendTag.halo2IpaPasta.rawValue))
        writer.writeField(OfflineCompactNorito.encodeString(
            KagemushaRecursiveSpendRequestCodecs.confidentialTransferV2CircuitId
        ))
        writer.writeField(vkHash)
        writer.writeField(byteVec(PrivacyConfidentialWitnessCodecs.confidentialTransferPublicInputsSchema()))
        writer.writeField(byteVec(proof ?? zk1Proof()))
        writer.writeField(byteVec(Data()))
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.openVerifyEnvelopeWireName,
            payload: writer.data,
            flags: NoritoHeader.compactLen
        )
    }

    private func privacyBuildResult(
        proof: Data,
        algorithmId: String = "confidential-transfer-v2",
        entrypoint: String = "buildConfidentialTransferProofV2"
    ) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeUInt32(1))
        writer.writeField(OfflineCompactNorito.encodeUInt32(0))
        writer.writeField(OfflineCompactNorito.encodeUInt32(0))
        writer.writeField(OfflineCompactNorito.encodeString(""))
        writer.writeField(OfflineCompactNorito.encodeString(algorithmId))
        writer.writeField(OfflineCompactNorito.encodeString(entrypoint))
        writer.writeField(OfflineCompactNorito.encodeString("halo2-ipa-pasta:confidential_transfer_v2"))
        writer.writeField(byteVec(Data()))
        writer.writeField(byteVec(proof))
        writer.writeField(Data([0]))
        var archive = noritoEncode(
            typeName: "connect_norito_bridge::PrivacyBuildProofResultV1",
            payload: writer.data,
            flags: NoritoHeader.compactLen
        )
        archive.replaceSubrange(6..<22, with: Data(repeating: 0x42, count: 16))
        return archive
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

    private func statusRecord(_ record: Data, status: UInt32) throws -> Data {
        let payload = try KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            record,
            schema: KagemushaRecursiveSpendRequestCodecs.verifyingKeyRecordWireName,
            field: "record"
        )
        var fields = try compactFields(payload)
        var statusBytes = Data()
        appendUInt32LE(status, to: &statusBytes)
        fields[fields.count - 1] = statusBytes
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
