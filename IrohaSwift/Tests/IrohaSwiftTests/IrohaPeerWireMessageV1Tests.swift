import XCTest
@testable import IrohaSwift

final class IrohaPeerWireMessageV1Tests: XCTestCase {

    func testWireLimitHardCeilingsRejectLargerAllocationPolicies() {
        XCTAssertTrue(IrohaPeerWireLimitsV1.areValid(
            maximumCanonicalBytes: 32 * 1_024,
            maximumOfflineNoteEncodedBytes: 24_576,
            maximumKagemushaEncodedBytes: 24_576
        ))
        XCTAssertFalse(IrohaPeerWireLimitsV1.areValid(
            maximumCanonicalBytes: 32 * 1_024 + 1,
            maximumOfflineNoteEncodedBytes: 24_576,
            maximumKagemushaEncodedBytes: 24_576
        ))
        XCTAssertFalse(IrohaPeerWireLimitsV1.areValid(
            maximumCanonicalBytes: 32 * 1_024,
            maximumOfflineNoteEncodedBytes: 24_577,
            maximumKagemushaEncodedBytes: 24_576
        ))
        XCTAssertFalse(IrohaPeerWireLimitsV1.areValid(
            maximumCanonicalBytes: 32 * 1_024,
            maximumOfflineNoteEncodedBytes: 24_576,
            maximumKagemushaEncodedBytes: 24_577
        ))
    }

    func testEmptyCanonicalPayloadIsRejectedByProducerAndHeaderParser() throws {
        XCTAssertThrowsError(try IrohaPeerWireMessageV1(
            profile: .offlineNote,
            kind: .receiveRequest,
            schemaVersion: 1,
            canonicalPayload: Data()
        )) { error in
            XCTAssertEqual(error as? IrohaPeerWireMessageErrorV1, .emptyCanonicalPayload)
        }

        let valid = try IrohaPeerWireMessageV1(
            profile: .offlineNote,
            kind: .acknowledgement,
            schemaVersion: 1,
            canonicalPayload: Data([0x01])
        )
        var emptyHeader = valid.header.bytes
        writeUInt32BE(&emptyHeader, at: 12, 0)
        writeUInt32BE(&emptyHeader, at: 16, 0)
        XCTAssertThrowsError(try IrohaPeerWireMessageV1.inspectHeader(emptyHeader)) { error in
            XCTAssertEqual(error as? IrohaPeerWireMessageErrorV1, .emptyCanonicalPayload)
        }
    }

    func testIPM1HeaderLayoutAndDomainSeparatedHashes() throws {
        let canonical = Data("canonical-offline-payload".utf8)
        let message = try IrohaPeerWireMessageV1(
            profile: .offlineNote,
            kind: .payment,
            schemaVersion: 1,
            canonicalPayload: canonical
        )
        let encoded = message.encoded

        XCTAssertEqual(encoded.count, 84 + canonical.count)
        XCTAssertEqual(Data(encoded[0..<4]), Data("IPM1".utf8))
        XCTAssertEqual(encoded[4], 1)
        XCTAssertEqual(encoded[5], 0)
        XCTAssertEqual(readUInt16BE(encoded, 6), 1)
        XCTAssertEqual(encoded[8], 2)
        XCTAssertEqual(encoded[9], 0)
        XCTAssertEqual(readUInt16BE(encoded, 10), 1)
        XCTAssertEqual(readUInt32BE(encoded, 12), UInt32(canonical.count))
        XCTAssertEqual(readUInt32BE(encoded, 16), UInt32(canonical.count))

        var canonicalPreimage = Data("IROHA-PEER-PAYLOAD-V1\0".utf8)
        canonicalPreimage.append(contentsOf: [0, 1, 2, 0, 1])
        canonicalPreimage.append(canonical)
        let canonicalHash = Blake2b.hash256(canonicalPreimage)
        XCTAssertEqual(Data(encoded[20..<52]), canonicalHash)

        var wirePreimage = Data("IROHA-PEER-MESSAGE-V1\0".utf8)
        wirePreimage.append(encoded[0..<52])
        wirePreimage.append(canonical)
        let wireHash = Blake2b.hash256(wirePreimage)
        XCTAssertEqual(Data(encoded[52..<84]), wireHash)
        XCTAssertEqual(message.streamID, Data(wireHash.prefix(16)))

        let decoded = try IrohaPeerWireMessageV1.decode(encoded)
        XCTAssertEqual(decoded, message)
        XCTAssertEqual(decoded.canonicalPayload, canonical)
    }

    func testPeerCompressionPolicyRequiresSavingsAndFewerShards() throws {
        let compressible = Data(repeating: 0x41, count: 1_024)
        let compressed = try IrohaPeerWireMessageV1(
            profile: .offlineNote,
            kind: .receiveRequest,
            schemaVersion: 1,
            canonicalPayload: compressible,
            compressionPolicy: .peerOptimized
        )
        XCTAssertEqual(compressed.encoding, .zlib)
        XCTAssertEqual(
            compressed.encodedBody,
            Data([
                0x78, 0x9C, 0x73, 0x74, 0x1C, 0x05, 0xA3, 0x60, 0x14,
                0x8C, 0x54, 0x00, 0x00, 0xA4, 0x78, 0x04, 0x10
            ]),
            "Pinned RFC 1950 vector: zlib level-6 encoding of 1,024 ASCII A bytes"
        )
        XCTAssertGreaterThanOrEqual(compressible.count - compressed.encodedBody.count, 32)
        XCTAssertLessThan(
            shardCount(compressed.encodedBody.count),
            shardCount(compressible.count)
        )
        XCTAssertEqual(
            try IrohaPeerWireMessageV1.decode(compressed.encoded).canonicalPayload,
            compressible
        )

        // Compression saves bytes here, but both forms still occupy one 256-byte shard.
        let oneShard = Data(repeating: 0x41, count: 200)
        let unchanged = try IrohaPeerWireMessageV1(
            profile: .offlineNote,
            kind: .receiveRequest,
            schemaVersion: 1,
            canonicalPayload: oneShard,
            compressionPolicy: .peerOptimized
        )
        XCTAssertEqual(unchanged.encoding, .none)
        XCTAssertEqual(unchanged.encodedBody, oneShard)

        let disabled = try IrohaPeerWireMessageV1(
            profile: .offlineNote,
            kind: .receiveRequest,
            schemaVersion: 1,
            canonicalPayload: compressible,
            compressionPolicy: .disabled
        )
        XCTAssertEqual(disabled.encoding, .none)
    }

    func testDecoderRejectsNonCanonicalZlibAndTrailingInput() throws {
        let message = try IrohaPeerWireMessageV1(
            profile: .offlineNote,
            kind: .payment,
            schemaVersion: 1,
            canonicalPayload: Data(repeating: 0x41, count: 1_024),
            compressionPolicy: .peerOptimized
        )
        XCTAssertEqual(message.encoding, .zlib)

        var insufficientSavings = message.header.bytes
        writeUInt32BE(
            &insufficientSavings,
            at: 12,
            UInt32(message.encodedBody.count + 31)
        )
        XCTAssertThrowsError(try IrohaPeerWireMessageV1.inspectHeader(insufficientSavings)) { error in
            XCTAssertEqual(
                error as? IrohaPeerWireMessageErrorV1,
                .compressionPolicyNotSatisfied
            )
        }

        var sameShardCount = message.header.bytes
        writeUInt32BE(&sameShardCount, at: 12, 200)
        XCTAssertThrowsError(try IrohaPeerWireMessageV1.inspectHeader(sameShardCount)) { error in
            XCTAssertEqual(
                error as? IrohaPeerWireMessageErrorV1,
                .compressionPolicyNotSatisfied
            )
        }

        var emptyEncodedBody = message.header.bytes
        writeUInt32BE(&emptyEncodedBody, at: 16, 0)
        XCTAssertThrowsError(try IrohaPeerWireMessageV1.inspectHeader(emptyEncodedBody)) { error in
            XCTAssertEqual(
                error as? IrohaPeerWireMessageErrorV1,
                .emptyEncodedBody
            )
        }

        var trailingInput = message.encoded
        trailingInput.append(0)
        writeUInt32BE(&trailingInput, at: 16, UInt32(message.encodedBody.count + 1))
        refreshWireHash(&trailingInput)
        XCTAssertThrowsError(try IrohaPeerWireMessageV1.decode(trailingInput)) { error in
            XCTAssertEqual(error as? IrohaPeerWireMessageErrorV1, .decompressionFailed)
        }

        var nonCanonicalWrapper = message.encoded
        nonCanonicalWrapper[84] = 0x79
        refreshWireHash(&nonCanonicalWrapper)
        XCTAssertThrowsError(try IrohaPeerWireMessageV1.decode(nonCanonicalWrapper)) { error in
            XCTAssertEqual(error as? IrohaPeerWireMessageErrorV1, .decompressionFailed)
        }

        var invalidAdler32 = message.encoded
        invalidAdler32[invalidAdler32.count - 1] ^= 1
        refreshWireHash(&invalidAdler32)
        XCTAssertThrowsError(try IrohaPeerWireMessageV1.decode(invalidAdler32)) { error in
            XCTAssertEqual(error as? IrohaPeerWireMessageErrorV1, .decompressionFailed)
        }
    }

    func testProfileAndCanonicalLimitsAreEnforcedBeforeAllocation() throws {
        XCTAssertEqual(IrohaPeerWireLimitsV1.peerV1.maximumOfflineNoteEncodedBytes, 24_576)
        XCTAssertEqual(IrohaPeerWireLimitsV1.peerV1.maximumKagemushaEncodedBytes, 24_576)
        let retailBoundary = try IrohaPeerWireMessageV1(
            profile: .offlineNote,
            kind: .payment,
            schemaVersion: 1,
            canonicalPayload: Data(repeating: 0xA5, count: 24_576)
        )
        XCTAssertEqual(retailBoundary.encodedBody.count, 24_576)
        XCTAssertEqual(try IrohaPeerWireMessageV1.decode(retailBoundary.encoded), retailBoundary)
        XCTAssertThrowsError(try IrohaPeerWireMessageV1(
            profile: .offlineNote,
            kind: .payment,
            schemaVersion: 1,
            canonicalPayload: Data(repeating: 0xA5, count: 24_577)
        )) { error in
            XCTAssertEqual(
                error as? IrohaPeerWireMessageErrorV1,
                .encodedLengthOutOfRange(actual: 24_577, maximum: 24_576)
            )
        }

        let tight = IrohaPeerWireLimitsV1(
            maximumCanonicalBytes: 1_024,
            maximumOfflineNoteEncodedBytes: 300,
            maximumKagemushaEncodedBytes: 700
        )
        XCTAssertThrowsError(try IrohaPeerWireMessageV1(
            profile: .offlineNote,
            kind: .payment,
            schemaVersion: 1,
            canonicalPayload: Data(repeating: 1, count: 301),
            limits: tight
        )) { error in
            XCTAssertEqual(
                error as? IrohaPeerWireMessageErrorV1,
                .encodedLengthOutOfRange(actual: 301, maximum: 300)
            )
        }
        XCTAssertNoThrow(try IrohaPeerWireMessageV1(
            profile: .kagemushaRecursiveSpend,
            kind: .payment,
            schemaVersion: 0x0102,
            canonicalPayload: irohaPeerKagemushaStructuralArchiveV1(
                kind: .payment,
                payload: Data(repeating: 1, count: 253)
            ),
            limits: tight
        ))
        XCTAssertThrowsError(try IrohaPeerWireMessageV1(
            profile: .kagemushaRecursiveSpend,
            kind: .payment,
            schemaVersion: 0x0102,
            canonicalPayload: Data(repeating: 1, count: 1_025),
            limits: tight
        )) { error in
            XCTAssertEqual(
                error as? IrohaPeerWireMessageErrorV1,
                .canonicalLengthOutOfRange(actual: 1_025, maximum: 1_024)
            )
        }
    }

    func testWireAndCanonicalCorruptionAreRejected() throws {
        let message = try makeMessage(bytes: Data("bound-by-both-hashes".utf8))

        var bodyCorruption = message.encoded
        bodyCorruption[bodyCorruption.count - 1] ^= 1
        XCTAssertThrowsError(try IrohaPeerWireMessageV1.decode(bodyCorruption)) { error in
            XCTAssertEqual(error as? IrohaPeerWireMessageErrorV1, .wireHashMismatch)
        }

        var canonicalHashCorruption = message.encoded
        canonicalHashCorruption[20] ^= 1
        let body = Data(canonicalHashCorruption[84...])
        var wireInput = Data("IROHA-PEER-MESSAGE-V1\0".utf8)
        wireInput.append(canonicalHashCorruption[0..<52])
        wireInput.append(body)
        canonicalHashCorruption.replaceSubrange(52..<84, with: Blake2b.hash256(wireInput))
        XCTAssertThrowsError(try IrohaPeerWireMessageV1.decode(canonicalHashCorruption)) { error in
            XCTAssertEqual(error as? IrohaPeerWireMessageErrorV1, .canonicalHashMismatch)
        }
    }

    func testExpectedProfileAndKindProduceTypedRejections() throws {
        let message = try makeMessage(bytes: Data("typed-routing".utf8))
        XCTAssertThrowsError(try IrohaPeerWireMessageV1.decode(
            message.encoded,
            expectedProfile: .kagemushaRecursiveSpend
        )) { error in
            XCTAssertEqual(
                error as? IrohaPeerWireMessageErrorV1,
                .unexpectedProfile(expected: .kagemushaRecursiveSpend, actual: .offlineNote)
            )
        }
        XCTAssertThrowsError(try IrohaPeerWireMessageV1.decode(
            message.encoded,
            expectedKind: .acknowledgement
        )) { error in
            XCTAssertEqual(
                error as? IrohaPeerWireMessageErrorV1,
                .unexpectedKind(expected: .acknowledgement, actual: .payment)
            )
        }
    }

    func testKagemushaProfileRequiresExactNativeIndependentABI21Envelope() throws {
        let canonical = irohaPeerKagemushaStructuralArchiveV1(
            kind: .receiveRequest,
            payload: Data([0x51])
        )
        XCTAssertEqual(canonical.count, 49)
        XCTAssertEqual(
            canonical.hexEncodedString(),
            "4e5254300000bfd427e87daf1d5cfa39b7fb60a76859000100000000000000de8130dd3f67aeb502000000000000000051"
        )
        let message = try IrohaPeerWireMessageV1(
            profile: .kagemusha,
            kind: .receiveRequest,
            schemaVersion: 0x0102,
            canonicalPayload: canonical
        )
        XCTAssertEqual(try IrohaPeerWireMessageV1.decode(message.encoded), message)

        var wrongSchema = canonical
        wrongSchema[6] ^= 1
        var shortPadding = canonical
        shortPadding.remove(at: 40)
        var longPadding = canonical
        longPadding.insert(0, at: 40)
        var wrongChecksum = canonical
        wrongChecksum[31] ^= 1
        var wrongFlags = canonical
        wrongFlags[39] = 0
        var wrongCompression = canonical
        wrongCompression[22] = 1
        var trailing = canonical
        trailing.append(0)
        let bareSchema = noritoEncode(
            typeName: "KagemushaRecipientPaymentRequestV2",
            payload: Data([0x51]),
            flags: NoritoHeader.compactLen,
            payloadAlignment: 16
        )

        for invalid in [
            wrongSchema,
            shortPadding,
            longPadding,
            wrongChecksum,
            wrongFlags,
            wrongCompression,
            trailing,
            bareSchema
        ] {
            XCTAssertThrowsError(try IrohaPeerWireMessageV1(
                profile: .kagemusha,
                kind: .receiveRequest,
                schemaVersion: 0x0102,
                canonicalPayload: invalid
            )) {
                XCTAssertEqual(
                    $0 as? IrohaPeerWireMessageErrorV1,
                    .invalidCanonicalPayload(profile: .kagemusha, kind: .receiveRequest)
                )
            }
        }
        XCTAssertThrowsError(try IrohaPeerWireMessageV1(
            profile: .kagemusha,
            kind: .payment,
            schemaVersion: 0x0102,
            canonicalPayload: canonical
        ))

        let forged = rehashKagemushaMessage(message.encoded, canonical: wrongSchema)
        XCTAssertThrowsError(try IrohaPeerWireMessageV1.decode(forged)) {
            XCTAssertEqual(
                $0 as? IrohaPeerWireMessageErrorV1,
                .invalidCanonicalPayload(profile: .kagemusha, kind: .receiveRequest)
            )
        }
    }

    func testFirstReleaseProfileSchemaPairsAreEnforcedAtConstructionAndInspection() throws {
        XCTAssertThrowsError(try IrohaPeerWireMessageV1(
            profile: .offlineNote,
            kind: .payment,
            schemaVersion: 0x0102,
            canonicalPayload: Data([1])
        )) { error in
            XCTAssertEqual(
                error as? IrohaPeerWireMessageErrorV1,
                .schemaVersionMismatch(profile: .offlineNote, expected: 1, actual: 0x0102)
            )
        }
        XCTAssertThrowsError(try IrohaPeerWireMessageV1(
            profile: .kagemushaRecursiveSpend,
            kind: .payment,
            schemaVersion: 1,
            canonicalPayload: Data([1])
        )) { error in
            XCTAssertEqual(
                error as? IrohaPeerWireMessageErrorV1,
                .schemaVersionMismatch(
                    profile: .kagemushaRecursiveSpend,
                    expected: 0x0102,
                    actual: 1
                )
            )
        }

        let retail = try IrohaPeerWireMessageV1(
            profile: .offlineNote,
            kind: .payment,
            schemaVersion: 1,
            canonicalPayload: Data([1])
        )
        var hostileHeader = retail.header.bytes
        hostileHeader[10] = 0x01
        hostileHeader[11] = 0x02
        XCTAssertThrowsError(try IrohaPeerWireMessageV1.inspectHeader(hostileHeader)) { error in
            XCTAssertEqual(
                error as? IrohaPeerWireMessageErrorV1,
                .schemaVersionMismatch(profile: .offlineNote, expected: 1, actual: 0x0102)
            )
        }
    }

    private func makeMessage(bytes: Data) throws -> IrohaPeerWireMessageV1 {
        try IrohaPeerWireMessageV1(
            profile: .offlineNote,
            kind: .payment,
            schemaVersion: 1,
            canonicalPayload: bytes
        )
    }

    private func shardCount(_ count: Int) -> Int { (count + 255) / 256 }

    private func readUInt16BE(_ data: Data, _ offset: Int) -> UInt16 {
        UInt16(data[offset]) << 8 | UInt16(data[offset + 1])
    }

    private func readUInt32BE(_ data: Data, _ offset: Int) -> UInt32 {
        UInt32(data[offset]) << 24
            | UInt32(data[offset + 1]) << 16
            | UInt32(data[offset + 2]) << 8
            | UInt32(data[offset + 3])
    }

    private func writeUInt32BE(
        _ data: inout Data,
        at offset: Int,
        _ value: UInt32
    ) {
        data[offset] = UInt8(truncatingIfNeeded: value >> 24)
        data[offset + 1] = UInt8(truncatingIfNeeded: value >> 16)
        data[offset + 2] = UInt8(truncatingIfNeeded: value >> 8)
        data[offset + 3] = UInt8(truncatingIfNeeded: value)
    }

    private func refreshWireHash(_ message: inout Data) {
        var preimage = Data("IROHA-PEER-MESSAGE-V1\0".utf8)
        preimage.append(message[0..<52])
        preimage.append(message[84...])
        message.replaceSubrange(52..<84, with: Blake2b.hash256(preimage))
    }

    private func rehashKagemushaMessage(
        _ encoded: Data,
        canonical: Data
    ) -> Data {
        precondition(encoded.count == 84 + canonical.count)
        var result = encoded
        result.replaceSubrange(84..<result.count, with: canonical)
        var canonicalPreimage = Data("IROHA-PEER-PAYLOAD-V1\0".utf8)
        canonicalPreimage.append(contentsOf: [0, 2, 1, 1, 2])
        canonicalPreimage.append(canonical)
        result.replaceSubrange(20..<52, with: Blake2b.hash256(canonicalPreimage))
        refreshWireHash(&result)
        return result
    }
}
