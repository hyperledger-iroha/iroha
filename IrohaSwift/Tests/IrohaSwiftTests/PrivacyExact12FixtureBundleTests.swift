import Foundation
import XCTest

@testable import IrohaSwift

final class PrivacyExact12FixtureBundleTests: XCTestCase {
    private static let fixtureFile = loadFixtureFile()
    private static let fixtureBase64 = String(fixtureFile.dropLast())
    private static let fixtureArchive = Data(base64Encoded: fixtureBase64)!

    func testRustFixtureDecodesAndByteIdenticallyReencodesWithoutNativeBridge() throws {
        let bundle = try PrivacyExact12FixtureCodecV1.decodeCanonicalBase64File(
            Self.fixtureFile
        )
        XCTAssertEqual(bundle.version, 1)
        XCTAssertEqual(bundle.rows.count, 12)
        XCTAssertEqual(bundle.rows.map(\.protocolId), PrivacyProtocolIdV1.allCases)
        XCTAssertEqual(
            Set(bundle.rows.map(\.submitProofWireId)),
            [PrivacyExact12FixtureCodecV1.submitProofWireId]
        )
        for row in bundle.rows {
            XCTAssertFalse(row.statementNorito.isEmpty)
            XCTAssertFalse(row.envelopeNorito.isEmpty)
            XCTAssertFalse(row.submitProofInstructionNorito.isEmpty)
            XCTAssertFalse(row.transactionIntentProjectionNorito.isEmpty)
            XCTAssertEqual(row.transactionIntentDigest.count, 32)
            XCTAssertFalse(row.unsignedTransactionPayloadNorito.isEmpty)
            XCTAssertFalse(row.signedTransactionVersionedNorito.isEmpty)
            XCTAssertEqual(row.signedTransactionHash.count, 32)
        }
        XCTAssertEqual(
            try PrivacyExact12FixtureCodecV1.encodeCanonicalArchive(bundle),
            Self.fixtureArchive
        )
        XCTAssertEqual(
            try PrivacyExact12FixtureCodecV1.encodeCanonicalBase64(bundle),
            Self.fixtureBase64
        )
        XCTAssertEqual(
            try PrivacyExact12FixtureCodecV1.requireCanonicalArchive(
                Self.fixtureArchive,
                expectedCanonicalArchive: Self.fixtureArchive
            ),
            bundle
        )
    }

    func testArchiveAndBase64ResourceLimitsFailBeforeDecode() {
        XCTAssertThrowsError(
            try PrivacyExact12FixtureCodecV1.decodeCanonicalArchive(Data())
        ) { error in
            XCTAssertEqual(error as? PrivacyExact12FixtureCodecErrorV1, .emptyArchive)
        }
        XCTAssertThrowsError(
            try PrivacyExact12FixtureCodecV1.decodeCanonicalArchive(
                Data(
                    repeating: 0,
                    count: PrivacyExact12FixtureCodecV1.maximumArchiveBytes + 1
                )
            )
        ) { error in
            XCTAssertEqual(
                error as? PrivacyExact12FixtureCodecErrorV1,
                .archiveTooLarge(
                    maximum: PrivacyExact12FixtureCodecV1.maximumArchiveBytes,
                    actual: PrivacyExact12FixtureCodecV1.maximumArchiveBytes + 1
                )
            )
        }
        let oversizedBase64 = String(
            repeating: "A",
            count: ((PrivacyExact12FixtureCodecV1.maximumArchiveBytes + 2) / 3) * 4 + 1
        )
        XCTAssertThrowsError(
            try PrivacyExact12FixtureCodecV1.decodeCanonicalBase64(oversizedBase64)
        )
    }

    func testTypedRowsRejectAliasesEmptyOversizedAndMalformedResources() throws {
        let canonical = try PrivacyExact12FixtureCodecV1.decodeCanonicalArchive(
            Self.fixtureArchive
        )
        let row = canonical.rows[0]

        XCTAssertThrowsError(
            try copyRow(row, submitProofWireId: "iroha.privacy.submit-proof.v1")
        ) { error in
            XCTAssertEqual(
                error as? PrivacyExact12FixtureCodecErrorV1,
                .invalidSubmitProofWireId
            )
        }
        XCTAssertThrowsError(try copyRow(row, statementNorito: Data()))
        XCTAssertThrowsError(try copyRow(row, envelopeNorito: Data()))
        XCTAssertThrowsError(try copyRow(row, submitProofInstructionNorito: Data()))
        XCTAssertThrowsError(try copyRow(row, transactionIntentProjectionNorito: Data()))
        XCTAssertThrowsError(try copyRow(row, unsignedTransactionPayloadNorito: Data()))
        XCTAssertThrowsError(try copyRow(row, signedTransactionVersionedNorito: Data()))

        XCTAssertThrowsError(
            try copyRow(
                row,
                statementNorito: Data(
                    repeating: 0,
                    count: PrivacyExact12FixtureCodecV1.maximumStatementBytes + 1
                )
            )
        )
        XCTAssertThrowsError(
            try copyRow(
                row,
                envelopeNorito: Data(
                    repeating: 0,
                    count: PrivacyExact12FixtureCodecV1.maximumEnvelopeBytes + 1
                )
            )
        )
        XCTAssertThrowsError(
            try copyRow(
                row,
                signedTransactionVersionedNorito: Data(
                    repeating: 0,
                    count: PrivacyExact12FixtureCodecV1.maximumSignedTransactionBytes + 1
                )
            )
        )
        XCTAssertThrowsError(try copyRow(row, transactionIntentDigest: Data(repeating: 1, count: 31)))
        XCTAssertThrowsError(try copyRow(row, transactionIntentDigest: Data(repeating: 1, count: 33)))
        XCTAssertThrowsError(try copyRow(row, transactionIntentDigest: Data(repeating: 0, count: 32)))
        XCTAssertThrowsError(try copyRow(row, signedTransactionHash: Data(repeating: 1, count: 31)))
        XCTAssertThrowsError(try copyRow(row, signedTransactionHash: Data(repeating: 0, count: 32)))

        XCTAssertThrowsError(
            try PrivacyExact12FixtureBundleV1(version: 2, rows: canonical.rows)
        ) { error in
            XCTAssertEqual(
                error as? PrivacyExact12FixtureCodecErrorV1,
                .unsupportedVersion(2)
            )
        }
        XCTAssertThrowsError(
            try PrivacyExact12FixtureBundleV1(
                version: 1,
                rows: Array(canonical.rows.dropLast())
            )
        ) { error in
            XCTAssertEqual(
                error as? PrivacyExact12FixtureCodecErrorV1,
                .invalidRowCount(11)
            )
        }
    }

    func testAggregateNestedResourceCeilingRejectsBeforeEncoding() throws {
        let canonical = try PrivacyExact12FixtureCodecV1.decodeCanonicalArchive(
            Self.fixtureArchive
        )
        var rows = canonical.rows
        for index in 0..<3 {
            rows[index] = try copyRow(
                rows[index],
                signedTransactionVersionedNorito: Data(
                    repeating: UInt8(index + 1),
                    count: PrivacyExact12FixtureCodecV1.maximumSignedTransactionBytes
                )
            )
        }
        XCTAssertThrowsError(
            try PrivacyExact12FixtureBundleV1(version: 1, rows: rows)
        ) { error in
            guard case PrivacyExact12FixtureCodecErrorV1.aggregateNestedBytesTooLarge = error else {
                return XCTFail("expected aggregateNestedBytesTooLarge, got \(error)")
            }
        }
    }

    func testNoncanonicalBase64SpellingsAndFixtureTextAreRejected() {
        let rejected = [
            Self.fixtureBase64 + "\n",
            " " + Self.fixtureBase64,
            Self.fixtureBase64 + " ",
            Self.fixtureBase64.replacingOccurrences(of: "+", with: "-"),
            String(Self.fixtureBase64.dropLast()),
        ]
        for encoded in rejected {
            XCTAssertThrowsError(
                try PrivacyExact12FixtureCodecV1.decodeCanonicalBase64(encoded),
                "noncanonical Base64 must reject"
            )
        }
        for fileText in [
            Self.fixtureBase64,
            Self.fixtureBase64 + "\r\n",
            Self.fixtureBase64 + "\n\n",
            String(Self.fixtureBase64.prefix(80)) + "\n"
                + String(Self.fixtureBase64.dropFirst(80)) + "\n",
        ] {
            XCTAssertThrowsError(
                try PrivacyExact12FixtureCodecV1.decodeCanonicalBase64File(fileText),
                "noncanonical fixture-file text must reject"
            )
        }
    }

    func testTruncationTrailingBytesHeaderAndChecksumMutationsReject() {
        var candidates = [
            Data(Self.fixtureArchive.dropLast()),
            Data(Self.fixtureArchive.dropFirst()),
            Data(Self.fixtureArchive.prefix(Self.fixtureArchive.count / 2)),
        ]
        var trailing = Self.fixtureArchive
        trailing.append(0)
        candidates.append(trailing)
        for index in [0, 4, 5, 6, 22, 23, 31, 39, Self.fixtureArchive.count - 1] {
            var mutated = Self.fixtureArchive
            mutated[index] ^= 0x80
            candidates.append(mutated)
        }
        for candidate in candidates {
            XCTAssertThrowsError(
                try PrivacyExact12FixtureCodecV1.decodeCanonicalArchive(candidate)
            )
        }
    }

    func testCanonicalOverlongLengthVarintRejectsAfterValidFrameChecksum() throws {
        let frame = try XCTUnwrap(noritoDecodeFrame(Self.fixtureArchive))
        XCTAssertEqual(frame.header.flags, NoritoHeader.compactLen)
        XCTAssertEqual(frame.paddingLength, 0)
        XCTAssertEqual(frame.payload.first, 4)
        var noncanonicalPayload = frame.payload
        noncanonicalPayload.replaceSubrange(0..<1, with: [0x84, 0x00])
        let noncanonicalArchive = noritoEncode(
            typeName: PrivacyExact12FixtureCodecV1.schemaName,
            payload: noncanonicalPayload,
            flags: NoritoHeader.compactLen,
            payloadAlignment: 8
        )
        XCTAssertThrowsError(
            try PrivacyExact12FixtureCodecV1.decodeCanonicalArchive(noncanonicalArchive)
        ) { error in
            guard case PrivacyExact12FixtureCodecErrorV1.nonCanonicalArchive = error else {
                return XCTFail("expected nonCanonicalArchive, got \(error)")
            }
        }
    }

    func testProtocolDiscriminantRequiresExactFourByteTagAndRejectsUnknownTag() throws {
        let unknown = try mutateFirstRowProtocolField(Self.fixtureArchive) { payload, offset in
            XCTAssertEqual(payload[offset], 4)
            payload[offset + 1] = 12
            payload[offset + 2] = 0
            payload[offset + 3] = 0
            payload[offset + 4] = 0
        }
        XCTAssertThrowsError(
            try PrivacyExact12FixtureCodecV1.decodeCanonicalArchive(unknown)
        ) { error in
            XCTAssertEqual(
                error as? PrivacyExact12FixtureCodecErrorV1,
                .unknownProtocolDiscriminant(12)
            )
        }

        for nonCanonicalLength in [UInt8(1), UInt8(3), UInt8(5)] {
            let malformed = try mutateFirstRowProtocolField(
                Self.fixtureArchive
            ) { payload, offset in
                XCTAssertEqual(payload[offset], 4)
                payload[offset] = nonCanonicalLength
            }
            XCTAssertThrowsError(
                try PrivacyExact12FixtureCodecV1.decodeCanonicalArchive(malformed),
                "protocol tag length \(nonCanonicalLength) must reject"
            )
        }
    }

    func testRowReorderAndProtocolSubstitutionRejectAtTypedBoundary() throws {
        let canonical = try PrivacyExact12FixtureCodecV1.decodeCanonicalArchive(
            Self.fixtureArchive
        )
        var reordered = canonical.rows
        reordered.swapAt(0, 1)
        XCTAssertThrowsError(
            try PrivacyExact12FixtureBundleV1(version: 1, rows: reordered)
        ) { error in
            guard case PrivacyExact12FixtureCodecErrorV1.protocolOrderMismatch = error else {
                return XCTFail("expected protocolOrderMismatch, got \(error)")
            }
        }

        let source = canonical.rows[0]
        let substituted = try copyRow(
            source,
            protocolId: .anonymousPgcKOutOfNV1
        )
        var rows = canonical.rows
        rows[0] = substituted
        XCTAssertThrowsError(
            try PrivacyExact12FixtureBundleV1(version: 1, rows: rows)
        )
    }

    func testCrossRowEnvelopeSubstitutionAndSignedPayloadMismatchReject() throws {
        let canonical = try PrivacyExact12FixtureCodecV1.decodeCanonicalArchive(
            Self.fixtureArchive
        )
        var rows = canonical.rows
        rows[0] = try copyRow(
            rows[0],
            envelopeNorito: rows[1].envelopeNorito
        )
        let envelopeSubstitution = try PrivacyExact12FixtureBundleV1(version: 1, rows: rows)
        XCTAssertThrowsError(
            try PrivacyExact12FixtureCodecV1.encodeCanonicalArchive(envelopeSubstitution)
        ) { error in
            guard case PrivacyExact12FixtureCodecErrorV1.invalidCrossFieldBinding = error else {
                return XCTFail("expected invalidCrossFieldBinding, got \(error)")
            }
        }

        rows = canonical.rows
        var signed = rows[2].signedTransactionVersionedNorito
        signed[signed.count - 2] ^= 1
        rows[2] = try copyRow(rows[2], signedTransactionVersionedNorito: signed)
        let signedMutation = try PrivacyExact12FixtureBundleV1(version: 1, rows: rows)
        XCTAssertThrowsError(
            try PrivacyExact12FixtureCodecV1.encodeCanonicalArchive(signedMutation)
        )
    }

    func testCanonicalInnerFrameAndCrossBindingMutationsReject() throws {
        let canonical = try PrivacyExact12FixtureCodecV1.decodeCanonicalArchive(
            Self.fixtureArchive
        )
        let row = canonical.rows[0]

        let badStatement = try reframe(
            row.statementNorito,
            typeName: "iroha.privacy.statement.v1",
            payloadAlignment: 16
        ) { payload in
            payload[0] ^= 1
        }
        try assertEncodingRejects(canonical, row: 0, replacement: copyRow(row, statementNorito: badStatement))

        let overlongStatement = try reframe(
            row.statementNorito,
            typeName: "iroha.privacy.statement.v1",
            payloadAlignment: 16
        ) { payload in
            var end = 4
            while payload[end] & 0x80 != 0 { end += 1 }
            payload[end] |= 0x80
            payload.insert(0, at: end + 1)
        }
        try assertEncodingRejects(
            canonical,
            row: 0,
            replacement: copyRow(row, statementNorito: overlongStatement)
        )

        let badEnvelope = try reframe(
            row.envelopeNorito,
            typeName: "iroha.privacy.proof-envelope.v1",
            payloadAlignment: 16
        ) { payload in
            // The first two compact fields are the four-byte protocol and
            // proof-system discriminants. Preserve every field length while
            // changing the proof system from its canonical row-0 value.
            precondition(payload[0] == 4 && payload[5] == 4)
            payload[6] ^= 1
        }
        try assertEncodingRejects(canonical, row: 0, replacement: copyRow(row, envelopeNorito: badEnvelope))

        let badEngine = try reframe(
            row.envelopeNorito,
            typeName: "iroha.privacy.proof-envelope.v1",
            payloadAlignment: 16
        ) { payload in
            // The third exact four-byte field is the native engine tag.
            precondition(payload[10] == 4)
            payload[11] = UInt8.max
        }
        try assertEncodingRejects(
            canonical,
            row: 0,
            replacement: copyRow(row, envelopeNorito: badEngine)
        )

        let badInstruction = try reframe(
            row.submitProofInstructionNorito,
            typeName: "iroha_data_model::isi::privacy::SubmitPrivacyProofV1",
            payloadAlignment: 16
        ) { payload in
            payload[payload.index(before: payload.endIndex)] ^= 1
        }
        try assertEncodingRejects(
            canonical,
            row: 0,
            replacement: copyRow(row, submitProofInstructionNorito: badInstruction)
        )

        let badProjection = try reframe(
            row.transactionIntentProjectionNorito,
            typeName: "iroha_data_model::transaction::signed::model::TransactionPayload",
            payloadAlignment: 8
        ) { payload in
            payload[payload.index(before: payload.endIndex)] ^= 1
        }
        try assertEncodingRejects(
            canonical,
            row: 0,
            replacement: copyRow(row, transactionIntentProjectionNorito: badProjection)
        )

        var badUnsigned = row.unsignedTransactionPayloadNorito
        let route = Data(PrivacyExact12FixtureCodecV1.submitProofWireId.utf8)
        let routeRange = try XCTUnwrap(badUnsigned.range(of: route))
        badUnsigned[routeRange.lowerBound] = 0x78
        try assertEncodingRejects(
            canonical,
            row: 0,
            replacement: copyRow(row, unsignedTransactionPayloadNorito: badUnsigned)
        )

        var badHash = row.signedTransactionHash
        badHash[0] ^= 1
        try assertEncodingRejects(
            canonical,
            row: 0,
            replacement: copyRow(row, signedTransactionHash: badHash)
        )
    }

    func testStaleOpaqueDigestNeedsAndFailsIndependentFixtureIdentityCheck() throws {
        let canonical = try PrivacyExact12FixtureCodecV1.decodeCanonicalArchive(
            Self.fixtureArchive
        )
        var rows = canonical.rows
        var staleDigest = rows[3].transactionIntentDigest
        staleDigest[0] ^= 1
        rows[3] = try copyRow(rows[3], transactionIntentDigest: staleDigest)
        let staleBundle = try PrivacyExact12FixtureBundleV1(version: 1, rows: rows)
        let staleArchive = try PrivacyExact12FixtureCodecV1.encodeCanonicalArchive(staleBundle)

        // The outer shape remains a canonical archive. Exact fixture identity
        // closes BLAKE3-derived bindings that pure Swift deliberately does not
        // approximate with a different hash.
        _ = try PrivacyExact12FixtureCodecV1.decodeCanonicalArchive(staleArchive)
        XCTAssertThrowsError(
            try PrivacyExact12FixtureCodecV1.requireCanonicalArchive(
                staleArchive,
                expectedCanonicalArchive: Self.fixtureArchive
            )
        ) { error in
            XCTAssertEqual(
                error as? PrivacyExact12FixtureCodecErrorV1,
                .fixtureIdentityMismatch
            )
        }
    }

    func testCanonicalBase64LengthUsesCheckedExactArithmetic() throws {
        XCTAssertEqual(
            try PrivacyExact12FixtureCodecV1.canonicalBase64EncodedLength(
                decodedByteCount: Self.fixtureArchive.count
            ),
            Self.fixtureBase64.utf8.count
        )
        XCTAssertEqual(
            try PrivacyExact12FixtureCodecV1.canonicalBase64EncodedLength(decodedByteCount: 0),
            0
        )
        XCTAssertThrowsError(
            try PrivacyExact12FixtureCodecV1.canonicalBase64EncodedLength(decodedByteCount: -1)
        )
    }

    private func copyRow(
        _ row: PrivacyExact12TypedFixtureRowV1,
        protocolId: PrivacyProtocolIdV1? = nil,
        statementNorito: Data? = nil,
        envelopeNorito: Data? = nil,
        submitProofWireId: String? = nil,
        submitProofInstructionNorito: Data? = nil,
        transactionIntentProjectionNorito: Data? = nil,
        transactionIntentDigest: Data? = nil,
        unsignedTransactionPayloadNorito: Data? = nil,
        signedTransactionVersionedNorito: Data? = nil,
        signedTransactionHash: Data? = nil
    ) throws -> PrivacyExact12TypedFixtureRowV1 {
        try PrivacyExact12TypedFixtureRowV1(
            protocolId: protocolId ?? row.protocolId,
            statementNorito: statementNorito ?? row.statementNorito,
            envelopeNorito: envelopeNorito ?? row.envelopeNorito,
            submitProofWireId: submitProofWireId ?? row.submitProofWireId,
            submitProofInstructionNorito:
                submitProofInstructionNorito ?? row.submitProofInstructionNorito,
            transactionIntentProjectionNorito:
                transactionIntentProjectionNorito ?? row.transactionIntentProjectionNorito,
            transactionIntentDigest: transactionIntentDigest ?? row.transactionIntentDigest,
            unsignedTransactionPayloadNorito:
                unsignedTransactionPayloadNorito ?? row.unsignedTransactionPayloadNorito,
            signedTransactionVersionedNorito:
                signedTransactionVersionedNorito ?? row.signedTransactionVersionedNorito,
            signedTransactionHash: signedTransactionHash ?? row.signedTransactionHash
        )
    }

    private func assertEncodingRejects(
        _ bundle: PrivacyExact12FixtureBundleV1,
        row rowIndex: Int,
        replacement: PrivacyExact12TypedFixtureRowV1,
        file: StaticString = #filePath,
        line: UInt = #line
    ) throws {
        var rows = bundle.rows
        rows[rowIndex] = replacement
        let mutated = try PrivacyExact12FixtureBundleV1(version: 1, rows: rows)
        XCTAssertThrowsError(
            try PrivacyExact12FixtureCodecV1.encodeCanonicalArchive(mutated),
            file: file,
            line: line
        )
    }

    private func reframe(
        _ archive: Data,
        typeName: String,
        payloadAlignment: Int,
        mutate: (inout Data) -> Void
    ) throws -> Data {
        let frame = try XCTUnwrap(noritoDecodeFrame(archive))
        var payload = frame.payload
        mutate(&payload)
        return noritoEncode(
            typeName: typeName,
            payload: payload,
            flags: NoritoHeader.compactLen,
            payloadAlignment: payloadAlignment
        )
    }

    private func mutateFirstRowProtocolField(
        _ archive: Data,
        mutate: (inout Data, Int) -> Void
    ) throws -> Data {
        let frame = try XCTUnwrap(noritoDecodeFrame(archive))
        var payload = frame.payload
        var cursor = 0
        let versionLength = try readCompactLength(payload, cursor: &cursor)
        cursor += versionLength
        _ = try readCompactLength(payload, cursor: &cursor)
        cursor += 8 // Exact12 row count is a canonical u64.
        _ = try readCompactLength(payload, cursor: &cursor)
        mutate(&payload, cursor)
        return noritoEncode(
            typeName: PrivacyExact12FixtureCodecV1.schemaName,
            payload: payload,
            flags: NoritoHeader.compactLen,
            payloadAlignment: 8
        )
    }

    private func readCompactLength(_ data: Data, cursor: inout Int) throws -> Int {
        var value: UInt64 = 0
        for byteIndex in 0..<10 {
            guard cursor < data.count else {
                throw PrivacyExact12FixtureCodecErrorV1.malformedArchive(
                    "truncated test fixture length"
                )
            }
            let byte = data[cursor]
            cursor += 1
            value |= UInt64(byte & 0x7f) << UInt64(byteIndex * 7)
            if byte & 0x80 == 0 {
                guard value <= UInt64(Int.max) else {
                    throw PrivacyExact12FixtureCodecErrorV1.malformedArchive(
                        "test fixture length overflow"
                    )
                }
                return Int(value)
            }
        }
        throw PrivacyExact12FixtureCodecErrorV1.malformedArchive(
            "test fixture length exceeds ten bytes"
        )
    }

    private static func loadFixtureFile() -> String {
        var directory = URL(
            fileURLWithPath: FileManager.default.currentDirectoryPath,
            isDirectory: true
        )
        while directory.path != "/" {
            let candidate = directory.appendingPathComponent(
                "fixtures/privacy/exact12_typed_fixture_bundle_v1.norito.b64"
            )
            if FileManager.default.fileExists(atPath: candidate.path) {
                guard let text = try? String(contentsOf: candidate, encoding: .utf8) else {
                    fatalError("Exact12 fixture is not readable UTF-8")
                }
                return text
            }
            directory.deleteLastPathComponent()
        }
        fatalError("cannot locate the Rust-derived Exact12 fixture")
    }
}
