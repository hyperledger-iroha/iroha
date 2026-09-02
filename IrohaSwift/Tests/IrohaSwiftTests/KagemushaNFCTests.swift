import XCTest
@testable import IrohaSwift

final class KagemushaNFCTests: XCTestCase {
    private func hexData(_ value: String) -> Data {
        precondition(value.count.isMultiple(of: 2))
        var output = Data()
        output.reserveCapacity(value.count / 2)
        var index = value.startIndex
        while index < value.endIndex {
            let next = value.index(index, offsetBy: 2)
            output.append(UInt8(value[index..<next], radix: 16)!)
            index = next
        }
        return output
    }

    func testMeasuredReleaseArchivesStayWithinTheSafeNFCChunkBudget() {
        let samples: [(String, Int, Int, Int)] = [
            ("receive-offer", 12_435, 57, 59),
            ("acknowledgement", 471, 3, 5),
            ("payment-v4-peer-hop-1", 12_896, 59, 61),
        ]
        for (label, archiveBytes, expectedChunks, expectedCommands) in samples {
            let chunks = (archiveBytes + KagemushaNFCProtocol.safeChunkBytes - 1)
                / KagemushaNFCProtocol.safeChunkBytes
            XCTAssertEqual(chunks, expectedChunks, label)
            XCTAssertEqual(chunks + 2, expectedCommands, label)
            XCTAssertLessThanOrEqual(
                archiveBytes,
                KagemushaNFCProtocol.maximumPayloadBytes,
                label
            )
        }
    }

    func testApplicationIdentifierContractIsExactAndStrict() throws {
        let identifier = try KagemushaNFCProtocol.applicationIdentifier(
            hex: " f0504b45504b524e464301 "
        )
        XCTAssertEqual(identifier, KagemushaNFCProtocol.defaultApplicationIdentifier)
        XCTAssertEqual(
            try KagemushaNFCProtocol.applicationIdentifierHex(identifier),
            "F0504B45504B524E464301"
        )
        XCTAssertEqual(
            KagemushaNFCProtocol.parseCommand(
                try KagemushaNFCProtocol.selectApplicationCommand()
            ),
            .select
        )
        let otherIdentifier = Data([0xF0, 0x50, 0x4B, 0x45, 0x51])
        XCTAssertEqual(
            KagemushaNFCProtocol.parseCommand(
                try KagemushaNFCProtocol.selectApplicationCommand(
                    applicationIdentifier: otherIdentifier
                )
            ),
            .selectOtherApplication
        )
        for invalid in ["", "F", "F049524Z", "F049 52", "F049524F",
                        "F0504B45504B524E464301020304050607"] {
            XCTAssertThrowsError(
                try KagemushaNFCProtocol.applicationIdentifier(hex: invalid),
                invalid
            )
        }
        XCTAssertThrowsError(try KagemushaNFCProtocol.applicationIdentifier(
            hex: String(repeating: "AA", count: 17)
        ))
        XCTAssertEqual(
            try KagemushaNFCProtocol.applicationIdentifier(
                hex: String(repeating: "AA", count: 16)
            ).count,
            16
        )
        XCTAssertThrowsError(try KagemushaNFCProtocol.applicationIdentifier(
            hex: String(repeating: "AA", count: 16)
                + String(
                    repeating: " ",
                    count: KagemushaNFCProtocol.maximumApplicationIdentifierPaddingBytes + 1
                )
        ))
        XCTAssertThrowsError(try KagemushaNFCProtocol.applicationIdentifier(
            hex: String(repeating: "AA", count: 5)
                + String(
                    repeating: " ",
                    count: KagemushaNFCProtocol.maximumApplicationIdentifierPaddingBytes + 1
                )
        ))
        XCTAssertThrowsError(try KagemushaNFCProtocol.applicationIdentifier(
            hex: "\u{2003}" + String(repeating: "AA", count: 5)
        ))
        XCTAssertThrowsError(try KagemushaNFCProtocol.applicationIdentifier(
            hex: String(repeating: " ", count: 1_000_000)
        ))
    }

    func testAPDUParserRejectsTruncationTrailingBytesAndInvalidLengths() throws {
        let validPayload = Data("payload".utf8)
        let validMetadata = try KagemushaNFCProtocol.writeMetadataCommand(
            kind: .payment,
            payloadBytes: validPayload
        )
        var legacyMetadata = validMetadata
        legacyMetadata[5] = 1
        XCTAssertEqual(
            KagemushaNFCProtocol.parseCommand(validMetadata),
            .writeMetadata(
                kind: .payment,
                payloadLength: validPayload.count,
                sha256: KagemushaNFCProtocol.sha256(validPayload)
            )
        )

        let malformed: [Data?] = [
            nil,
            Data(),
            Data([0x80, 0x10, 0, 0, 1]),
            Data([0x80, 0x11, 0, 0, 0]),
            Data([0x80, 0x11, 0, 0, 0, 0]),
            Data([0x80, 0x11, 0, 0, 0, 0, 0]),
            Data([0x80, 0x21, 0, 0, 2, 1]),
            legacyMetadata,
            validMetadata + Data([0]),
        ]
        for command in malformed {
            XCTAssertEqual(KagemushaNFCProtocol.parseCommand(command), .invalid)
        }
        XCTAssertEqual(
            KagemushaNFCProtocol.parseCommand(Data([0x80, 0xFF, 0, 0, 0])),
            .unsupported
        )
        XCTAssertEqual(
            KagemushaNFCProtocol.parseCommand(Data([0x00, 0x10, 0, 0, 0])),
            .unsupported
        )
    }

    func testPayloadBudgetUsesAuthoritativeRawArchiveLimit() throws {
        XCTAssertEqual(KagemushaNFCProtocol.rawTransportVersion, 4)
        XCTAssertEqual(KagemushaRecursiveSpend.maximumPeerArchiveBytesV2, 32 * 1024)
        XCTAssertEqual(KagemushaRecursiveSpend.maximumPeerArchiveBytesV4, 32 * 1024 * 1024)
        XCTAssertEqual(
            KagemushaPeerTransportContract.maximumArchiveBytes,
            KagemushaPeerTransportContract.maximumArchiveBytesV4
        )
        XCTAssertEqual(KagemushaNFCProtocol.maximumPayloadBytes, 32 * 1024 * 1024)
        let maximum = Data(
            repeating: 0xA5,
            count: KagemushaNFCProtocol.maximumPayloadBytes
        )
        let info = try KagemushaNFCProtocol.encodeInfo(
            kind: .payment,
            payloadBytes: maximum
        )
        let decoded = try XCTUnwrap(KagemushaNFCProtocol.decodeInfo(info))
        XCTAssertEqual(decoded.transportVersion, 4)
        XCTAssertEqual(decoded.payloadLength, maximum.count)
        XCTAssertThrowsError(try KagemushaNFCProtocol.encodeInfo(
            kind: .payment,
            payloadBytes: maximum + Data([0])
        )) { error in
            XCTAssertEqual(error as? KagemushaNFCError, .invalidPayloadLength)
        }
    }

    func testBulkWriterRequiresCanonicalMinimumButAllowsASmallerFinalChunk() throws {
        let payload = Data(repeating: 0xA5, count: KagemushaNFCProtocol.safeChunkBytes + 1)
        XCTAssertThrowsError(try KagemushaNFCProtocol.writePayloadCommands(
            kind: .payment,
            payloadBytes: payload,
            maximumChunkLength: KagemushaNFCProtocol.safeChunkBytes - 1
        )) { error in
            XCTAssertEqual(error as? KagemushaNFCError, .invalidChunkLength)
        }

        let commands = try KagemushaNFCProtocol.writePayloadCommands(
            kind: .payment,
            payloadBytes: payload,
            maximumChunkLength: KagemushaNFCProtocol.safeChunkBytes
        )
        XCTAssertEqual(commands.count, 4)
        guard case let .writeChunk(firstOffset, firstBytes) =
                KagemushaNFCProtocol.parseCommand(commands[1]),
              case let .writeChunk(finalOffset, finalBytes) =
                KagemushaNFCProtocol.parseCommand(commands[2]) else {
            return XCTFail("missing canonical bulk-write chunks")
        }
        XCTAssertEqual(firstOffset, 0)
        XCTAssertEqual(firstBytes.count, KagemushaNFCProtocol.safeChunkBytes)
        XCTAssertEqual(finalOffset, KagemushaNFCProtocol.safeChunkBytes)
        XCTAssertEqual(finalBytes, Data([0xA5]))
    }

    func testAPDUV4GoldensStreamBeyondLegacyLimitAndRejectDowngrade() throws {
        let smallArchive = hexData(
            "4e5254300000dbb6585e2e75a835bced19003aab7acd000100000000000000de8130dd3f67aeb502519897f659"
        )
        let smallCommands = try KagemushaNFCProtocol.writePayloadCommands(
            kind: .receiveRequest,
            payloadBytes: smallArchive,
            maximumChunkLength: KagemushaNFCProtocol.safeChunkBytes
        )
        XCTAssertEqual(
            smallCommands[0],
            hexData(
                "802004002604010000002d16b35168fd7dce091904f3b0b2597831528dbf9c19bd154b8eba509b92b1f84c"
            )
        )
        XCTAssertEqual(
            smallCommands[1],
            hexData(
                "8021040031000000004e5254300000dbb6585e2e75a835bced19003aab7acd000100000000000000de8130dd3f67aeb502519897f659"
            )
        )
        XCTAssertEqual(smallCommands[2], hexData("8022040000"))

        let payload = Data((0..<70_003).map { UInt8(truncatingIfNeeded: $0 * 29 + 7) })
        let commands = try KagemushaNFCProtocol.writePayloadCommands(
            kind: .payment,
            payloadBytes: payload,
            maximumChunkLength: KagemushaNFCProtocol.maximumExtendedWriteChunkBytes
        )
        guard case let .writeMetadata(kind, length, digest) =
                KagemushaNFCProtocol.parseCommand(commands[0]) else {
            return XCTFail("missing V4 metadata")
        }
        XCTAssertEqual(length, payload.count)
        let assembler = try KagemushaNFCPayloadAssembler(
            kind: kind,
            expectedLength: length,
            expectedSHA256: digest
        )
        for command in commands.dropFirst().dropLast().reversed() {
            guard case let .writeChunk(offset, bytes) =
                    KagemushaNFCProtocol.parseCommand(command) else {
                return XCTFail("missing V4 chunk")
            }
            XCTAssertTrue(assembler.write(offset: offset, bytes: bytes))
        }
        XCTAssertEqual(try assembler.commit(), payload)

        let atFFFF = try KagemushaNFCProtocol.writeChunkCommand(
            offset: 0xFFFF,
            bytes: Data([0x5A])
        )
        let at10000 = try KagemushaNFCProtocol.writeChunkCommand(
            offset: 0x1_0000,
            bytes: Data([0x5A])
        )
        XCTAssertEqual(atFFFF, hexData("80210400050000ffff5a"))
        XCTAssertEqual(at10000, hexData("8021040005000100005a"))
        XCTAssertEqual(
            try KagemushaNFCProtocol.readChunkCommand(offset: 0xFFFF, length: 1_024),
            hexData("80110400060000ffff0400")
        )
        XCTAssertEqual(
            KagemushaNFCProtocol.parseCommand(atFFFF),
            .writeChunk(offset: 0xFFFF, bytes: Data([0x5A]))
        )
        XCTAssertEqual(
            KagemushaNFCProtocol.parseCommand(at10000),
            .writeChunk(offset: 0x1_0000, bytes: Data([0x5A]))
        )

        // Retired V2 P1/P2 offsets, including offset zero, are never V4 commands.
        for retired in [
            Data([0x80, 0x21, 0, 0, 1, 0x5A]),
            Data([0x80, 0x21, 0xFF, 0xFF, 1, 0x5A]),
            Data([0x80, 0x11, 0, 0, 0]),
            Data(at10000.dropLast()),
        ] {
            XCTAssertEqual(KagemushaNFCProtocol.parseCommand(retired), .invalid)
        }
        XCTAssertThrowsError(try KagemushaNFCProtocol.writeChunkCommand(
            offset: KagemushaNFCProtocol.maximumPayloadBytes,
            bytes: Data([1])
        )) { error in
            XCTAssertEqual(error as? KagemushaNFCError, .invalidOffset)
        }
    }

    func testPayloadAssemblerAcceptsIdempotentOverlapAndRejectsContradiction() throws {
        let payload = Data("abcdef".utf8)
        let assembler = try KagemushaNFCPayloadAssembler(
            kind: .payment,
            expectedLength: payload.count,
            expectedSHA256: KagemushaNFCProtocol.sha256(payload)
        )
        XCTAssertFalse(assembler.write(offset: 0, bytes: Data()))
        XCTAssertFalse(assembler.write(offset: payload.count, bytes: Data([1])))
        XCTAssertTrue(assembler.write(offset: 0, bytes: Data("abc".utf8)))
        XCTAssertTrue(assembler.write(offset: 1, bytes: Data("bc".utf8)))
        XCTAssertFalse(assembler.write(offset: 1, bytes: Data("ZZ".utf8)))
        XCTAssertThrowsError(try assembler.commit()) { error in
            XCTAssertEqual(error as? KagemushaNFCError, .incompletePayload)
        }
        XCTAssertTrue(assembler.write(offset: 3, bytes: Data("def".utf8)))
        XCTAssertEqual(try assembler.commit(), payload)

        let corrupt = try KagemushaNFCPayloadAssembler(
            kind: .payment,
            expectedLength: payload.count,
            expectedSHA256: KagemushaNFCProtocol.sha256(payload)
        )
        XCTAssertTrue(corrupt.write(offset: 0, bytes: Data("abcdeg".utf8)))
        XCTAssertThrowsError(try corrupt.commit()) { error in
            XCTAssertEqual(error as? KagemushaNFCError, .checksumMismatch)
        }
    }

    func testPayloadAssemblerBuffersOnlyAcceptedSparseBytes() throws {
        let maximum = try KagemushaNFCPayloadAssembler(
            kind: .payment,
            expectedLength: KagemushaNFCProtocol.maximumPayloadBytes,
            expectedSHA256: Data(repeating: 0xA5, count: 32)
        )
        XCTAssertEqual(maximum.bufferedByteCount, 0)
        XCTAssertFalse(maximum.isComplete)
        XCTAssertTrue(maximum.write(
            offset: KagemushaNFCProtocol.maximumPayloadBytes - 3,
            bytes: Data([7, 8, 9])
        ))
        XCTAssertEqual(maximum.bufferedByteCount, 3)
        maximum.clear()
        XCTAssertEqual(maximum.bufferedByteCount, 0)

        let payload = Data("abcdefgh".utf8)
        let assembler = try KagemushaNFCPayloadAssembler(
            kind: .payment,
            expectedLength: payload.count,
            expectedSHA256: KagemushaNFCProtocol.sha256(payload)
        )
        XCTAssertTrue(assembler.write(offset: 4, bytes: Data("efgh".utf8)))
        XCTAssertEqual(assembler.bufferedByteCount, 4)
        XCTAssertTrue(assembler.write(offset: 2, bytes: Data("cdef".utf8)))
        XCTAssertEqual(assembler.bufferedByteCount, 6)
        XCTAssertTrue(assembler.write(offset: 3, bytes: Data("def".utf8)))
        XCTAssertEqual(assembler.bufferedByteCount, 6)
        XCTAssertFalse(assembler.write(offset: 3, bytes: Data("dXf".utf8)))
        XCTAssertEqual(assembler.bufferedByteCount, 6)
        XCTAssertTrue(assembler.write(offset: 0, bytes: Data("ab".utf8)))
        XCTAssertEqual(assembler.bufferedByteCount, payload.count)
        XCTAssertTrue(assembler.isComplete)
        XCTAssertEqual(try assembler.commit(), payload)
    }

    func testPayloadAssemblerFragmentBudgetFailureIsTerminal() throws {
        let assembler = try KagemushaNFCPayloadAssembler(
            kind: .payment,
            expectedLength: 131,
            expectedSHA256: Data(repeating: 1, count: 32)
        )
        var accepted = 0
        for offset in stride(from: 0, to: 131, by: 2) {
            if assembler.write(offset: offset, bytes: Data([UInt8(truncatingIfNeeded: offset)])) {
                accepted += 1
            } else {
                XCTAssertEqual(offset, 130)
                break
            }
        }
        XCTAssertEqual(accepted, 65)
        XCTAssertEqual(assembler.bufferedByteCount, 0)
        XCTAssertTrue(assembler.isCleared)
        XCTAssertEqual(assembler.expectedSHA256, Data(repeating: 0, count: 32))
        XCTAssertFalse(assembler.write(offset: 1, bytes: Data([1])))
        XCTAssertThrowsError(try assembler.commit()) { error in
            XCTAssertEqual(error as? KagemushaNFCError, .invalidState)
        }
    }

    func testCompleteBadDigestCommitIsTerminal() throws {
        let payload = Data("abcdef".utf8)
        let assembler = try KagemushaNFCPayloadAssembler(
            kind: .payment,
            expectedLength: payload.count,
            expectedSHA256: KagemushaNFCProtocol.sha256(Data("abcdeg".utf8))
        )
        XCTAssertTrue(assembler.write(offset: 0, bytes: payload))
        XCTAssertThrowsError(try assembler.commit()) { error in
            XCTAssertEqual(error as? KagemushaNFCError, .checksumMismatch)
        }
        XCTAssertEqual(assembler.bufferedByteCount, 0)
        XCTAssertTrue(assembler.isCleared)
        XCTAssertEqual(assembler.expectedSHA256, Data(repeating: 0, count: 32))
        XCTAssertThrowsError(try assembler.commit()) { error in
            XCTAssertEqual(error as? KagemushaNFCError, .invalidState)
        }
    }

    func testIncompleteCommitIsRetryableAndSuccessConsumesAssembler() throws {
        let payload = Data("abcdefgh".utf8)
        let assembler = try KagemushaNFCPayloadAssembler(
            kind: .payment,
            expectedLength: payload.count,
            expectedSHA256: KagemushaNFCProtocol.sha256(payload)
        )
        XCTAssertTrue(assembler.write(offset: 0, bytes: Data("abcd".utf8)))
        XCTAssertThrowsError(try assembler.commit()) { error in
            XCTAssertEqual(error as? KagemushaNFCError, .incompletePayload)
        }
        XCTAssertEqual(assembler.bufferedByteCount, 4)
        XCTAssertFalse(assembler.isCleared)
        XCTAssertTrue(assembler.write(offset: 4, bytes: Data("efgh".utf8)))
        XCTAssertEqual(try assembler.commit(), payload)
        XCTAssertEqual(assembler.bufferedByteCount, 0)
        XCTAssertTrue(assembler.isCleared)
        XCTAssertEqual(assembler.expectedSHA256, Data(repeating: 0, count: 32))
        XCTAssertFalse(assembler.write(offset: 0, bytes: payload))
        XCTAssertThrowsError(try assembler.commit()) { error in
            XCTAssertEqual(error as? KagemushaNFCError, .invalidState)
        }
    }

    func testReadTrackerFragmentBudgetFailureIsTerminal() throws {
        let tracker = try KagemushaNFCReadTracker(expectedLength: 131)
        var accepted = 0
        for offset in stride(from: 0, to: 131, by: 2) {
            if tracker.mark(offset: offset, length: 1) {
                accepted += 1
            } else {
                XCTAssertEqual(offset, 130)
                break
            }
        }
        XCTAssertEqual(accepted, 65)
        XCTAssertTrue(tracker.isCleared)
        XCTAssertFalse(tracker.isComplete)
        XCTAssertFalse(tracker.mark(offset: 1, length: 1))
    }

    func testEveryCanonicalNFCChunkRoundTripsAndReassemblesOutOfOrder() throws {
        let offer = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let request = try offer.project(
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        ).request
        let payment = try KagemushaPeerTransportTestFixtures.payment(request: request)
        let acknowledgement = try KagemushaPeerTransportTestFixtures.acknowledgement(
            request: request,
            payment: payment
        )
        let payloads: [KagemushaPeerPayload] = [
            .receiveRequest(offer),
            .payment(payment),
            .acknowledgement(acknowledgement),
        ]

        for payload in payloads {
            let bytes = payload.archive
            let commands = try KagemushaNFCProtocol.writePayloadCommands(
                kind: payload.kind,
                payloadBytes: bytes,
                maximumChunkLength: KagemushaNFCProtocol.safeChunkBytes
            )
            guard case let .writeMetadata(kind, length, digest) =
                    KagemushaNFCProtocol.parseCommand(commands[0]) else {
                return XCTFail("missing canonical metadata for \(payload.kind)")
            }
            XCTAssertEqual(kind, payload.kind)
            XCTAssertEqual(length, bytes.count)
            let assembler = try KagemushaNFCPayloadAssembler(
                kind: kind,
                expectedLength: length,
                expectedSHA256: digest
            )
            let chunks = try commands.dropFirst().dropLast().map { command -> (Int, Data) in
                guard case let .writeChunk(offset, chunk) =
                        KagemushaNFCProtocol.parseCommand(command) else {
                    throw KagemushaNFCError.malformedCommand
                }
                XCTAssertEqual(
                    try KagemushaNFCProtocol.writeChunkCommand(
                        offset: offset,
                        bytes: chunk
                    ),
                    command
                )
                return (offset, chunk)
            }
            for (offset, chunk) in chunks.reversed() {
                XCTAssertTrue(assembler.write(offset: offset, bytes: chunk))
                XCTAssertTrue(assembler.write(offset: offset, bytes: chunk))
            }
            XCTAssertEqual(try assembler.commit(), bytes, "\(payload.kind)")

            let incomplete = try KagemushaNFCPayloadAssembler(
                kind: kind,
                expectedLength: length,
                expectedSHA256: digest
            )
            for (offset, chunk) in chunks.dropFirst() {
                XCTAssertTrue(incomplete.write(offset: offset, bytes: chunk))
            }
            XCTAssertThrowsError(try incomplete.commit()) { error in
                XCTAssertEqual(error as? KagemushaNFCError, .incompletePayload)
            }
        }
    }

    func testDeclaredNFCPayloadLengthIsCappedBeforeAssemblerAllocation() throws {
        let payload = Data("bounded".utf8)
        var metadata = try KagemushaNFCProtocol.writeMetadataCommand(
            kind: .payment,
            payloadBytes: payload
        )
        let oversized = UInt32(KagemushaNFCProtocol.maximumPayloadBytes + 1)
        metadata[7] = UInt8(truncatingIfNeeded: oversized >> 24)
        metadata[8] = UInt8(truncatingIfNeeded: oversized >> 16)
        metadata[9] = UInt8(truncatingIfNeeded: oversized >> 8)
        metadata[10] = UInt8(truncatingIfNeeded: oversized)
        XCTAssertEqual(KagemushaNFCProtocol.parseCommand(metadata), .invalid)
        XCTAssertThrowsError(try KagemushaNFCPayloadAssembler(
            kind: .payment,
            expectedLength: Int(oversized),
            expectedSHA256: KagemushaNFCProtocol.sha256(payload)
        )) { error in
            XCTAssertEqual(error as? KagemushaNFCError, .invalidPayloadLength)
        }
    }

    func testCardStateMachineReadsRequestAndRejectsInvalidWriteSequences() throws {
        let offer = try KagemushaPeerTransportTestFixtures.receiveRequest()
        _ = try offer.project(
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        ).request
        let machine = try KagemushaNFCCardStateMachine(chainDiscriminant: SccpV1.tairaI105DiscriminantV1, receiveRequest: offer)
        let candidatePayment = Data("invalid-candidate".utf8)
        XCTAssertEqual(
            machine.handle(KagemushaNFCProtocol.getInfoCommand()).rejectionReason,
            .conditionsNotSatisfied
        )
        XCTAssertEqual(
            machine.handle(try KagemushaNFCProtocol.readChunkCommand(
                offset: 0,
                length: 1
            )).rejectionReason,
            .conditionsNotSatisfied
        )
        XCTAssertEqual(
            machine.handle(try KagemushaNFCProtocol.writeMetadataCommand(
                kind: .payment,
                payloadBytes: candidatePayment
            )).rejectionReason,
            .conditionsNotSatisfied
        )
        let otherIdentifier = Data([0xF0, 0x50, 0x4B, 0x45, 0x51])
        XCTAssertEqual(
            machine.handle(try KagemushaNFCProtocol.selectApplicationCommand(
                applicationIdentifier: otherIdentifier
            )).response,
            KagemushaNFCProtocol.statusNotFound
        )
        XCTAssertEqual(
            machine.handle(KagemushaNFCProtocol.commitCommand()).rejectionReason,
            .conditionsNotSatisfied
        )
        XCTAssertEqual(
            machine.handle(try KagemushaNFCProtocol.writeChunkCommand(
                offset: 0,
                bytes: Data([1])
            )).rejectionReason,
            .conditionsNotSatisfied
        )

        XCTAssertNil(machine.handle(
            try KagemushaNFCProtocol.selectApplicationCommand()
        ).rejectionReason)
        let infoResult = machine.handle(KagemushaNFCProtocol.getInfoCommand())
        let info = try XCTUnwrap(KagemushaNFCProtocol.decodeInfo(
            KagemushaNFCProtocol.responseData(infoResult.response)
        ))
        XCTAssertEqual(info.kind, .receiveRequest)
        let chunk = machine.handle(try KagemushaNFCProtocol.readChunkCommand(
            offset: 0,
            length: min(info.maximumChunkLength, info.payloadLength)
        ))
        XCTAssertEqual(KagemushaNFCProtocol.responseStatus(chunk.response), 0x9000)
        XCTAssertFalse(KagemushaNFCProtocol.responseData(chunk.response).isEmpty)

        let invalidAck = Data("invalid-ack".utf8)
        XCTAssertEqual(machine.handle(
            try KagemushaNFCProtocol.writeMetadataCommand(
                kind: .acknowledgement,
                payloadBytes: invalidAck
            )
        ).rejectionReason, .wrongData)
    }

    func testCardStateMachineCommitsTypedPaymentAndTracksEveryAckByte() throws {
        try requireNativeTestCapability(
            KagemushaRecursiveSpend.hasRequiredNativeSymbols,
            "ABI-23 Kagemusha bridge is not linked in this test host"
        )
        let offer = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let request = try offer.project(
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        ).request
        let payment = try KagemushaPeerTransportTestFixtures.payment(request: request)
        let acknowledgement = try KagemushaPeerTransportTestFixtures.acknowledgement(
            request: request,
            payment: payment
        )
        let machine = try KagemushaNFCCardStateMachine(chainDiscriminant: SccpV1.tairaI105DiscriminantV1, receiveRequest: offer)
        let paymentBytes = payment.archive
        let commands = try KagemushaNFCProtocol.writePayloadCommands(
            kind: .payment,
            payloadBytes: paymentBytes,
            maximumChunkLength: KagemushaNFCProtocol.safeChunkBytes
        )
        XCTAssertEqual(
            KagemushaNFCProtocol.responseStatus(machine.handle(
                try KagemushaNFCProtocol.selectApplicationCommand()
            ).response),
            0x9000
        )
        let metadata = try XCTUnwrap(commands.first)
        let commitCommand = try XCTUnwrap(commands.last)
        let chunks = Array(commands.dropFirst().dropLast())
        XCTAssertNil(machine.handle(metadata).rejectionReason)
        for command in chunks.reversed() {
            XCTAssertNil(machine.handle(command).rejectionReason)
        }
        if let duplicate = chunks.first {
            XCTAssertNil(machine.handle(duplicate).rejectionReason)
        }
        let commit = machine.handle(commitCommand)
        XCTAssertEqual(commit.committedPayload, .payment(payment))
        XCTAssertFalse(machine.isReadable)
        XCTAssertFalse(machine.hasPendingWrite)

        try machine.publishAcknowledgement(acknowledgement)
        XCTAssertTrue(machine.isReadable)
        let info = try XCTUnwrap(KagemushaNFCProtocol.decodeInfo(
            KagemushaNFCProtocol.responseData(
                machine.handle(KagemushaNFCProtocol.getInfoCommand()).response
            )
        ))
        XCTAssertEqual(info.kind, .acknowledgement)
        var offset = 0
        while offset < info.payloadLength {
            let length = min(73, info.payloadLength - offset)
            let result = machine.handle(try KagemushaNFCProtocol.readChunkCommand(
                offset: offset,
                length: length
            ))
            let range = try XCTUnwrap(result.acknowledgementReadRange)
            XCTAssertEqual(range, offset..<(offset + length))
            let completed = machine.markAcknowledgementBytesRead(range)
            XCTAssertEqual(completed, offset + length == info.payloadLength)
            offset += length
        }
        XCTAssertTrue(machine.hasCompleted)
        XCTAssertFalse(machine.isReadable)
    }

    func testCardStateDropsTerminallyFragmentedAcknowledgementTracker() throws {
        try requireNativeTestCapability(
            KagemushaRecursiveSpend.hasRequiredNativeSymbols,
            "ABI-23 Kagemusha bridge is not linked in this test host"
        )
        let offer = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let request = try offer.project(
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        ).request
        let payment = try KagemushaPeerTransportTestFixtures.payment(request: request)
        let acknowledgement = try KagemushaPeerTransportTestFixtures.acknowledgement(
            request: request,
            payment: payment
        )
        let machine = try KagemushaNFCCardStateMachine(
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1,
            receiveRequest: offer
        )
        let commands = try KagemushaNFCProtocol.writePayloadCommands(
            kind: .payment,
            payloadBytes: payment.archive
        )
        XCTAssertNil(machine.handle(
            try KagemushaNFCProtocol.selectApplicationCommand()
        ).rejectionReason)
        for command in commands {
            XCTAssertNil(machine.handle(command).rejectionReason)
        }
        try machine.publishAcknowledgement(acknowledgement)
        let info = try XCTUnwrap(KagemushaNFCProtocol.decodeInfo(
            KagemushaNFCProtocol.responseData(
                machine.handle(KagemushaNFCProtocol.getInfoCommand()).response
            )
        ))
        let rangeBudget = KagemushaNFCProtocol.sparseFragmentBudget(
            payloadLength: info.payloadLength
        )
        XCTAssertGreaterThan(info.payloadLength, rangeBudget * 2)
        XCTAssertFalse(machine.markAcknowledgementBytesRead(Int.min..<Int.max))
        XCTAssertTrue(machine.isReadable)

        var accepted = 0
        for offset in stride(from: 0, to: info.payloadLength, by: 2) {
            let result = machine.handle(try KagemushaNFCProtocol.readChunkCommand(
                offset: offset,
                length: 1
            ))
            let range = try XCTUnwrap(result.acknowledgementReadRange)
            XCTAssertFalse(machine.markAcknowledgementBytesRead(range))
            if !machine.isReadable {
                break
            }
            accepted += 1
        }
        XCTAssertEqual(accepted, rangeBudget)
        XCTAssertFalse(machine.hasCompleted)
        XCTAssertFalse(machine.isReadable)
        XCTAssertFalse(machine.markAcknowledgementBytesRead(1..<2))
        XCTAssertEqual(
            machine.handle(
                try KagemushaNFCProtocol.readChunkCommand(offset: 0, length: 1)
            ).rejectionReason,
            .conditionsNotSatisfied
        )
    }

    func testInvalidDigestCommitDoesNotBecomeTypedPayment() throws {
        let request = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let machine = try KagemushaNFCCardStateMachine(chainDiscriminant: SccpV1.tairaI105DiscriminantV1, receiveRequest: request)
        let bytes = Data("not-a-valid-archive".utf8)
        var metadata = try KagemushaNFCProtocol.writeMetadataCommand(
            kind: .payment,
            payloadBytes: bytes
        )
        metadata[metadata.count - 1] ^= 1
        XCTAssertNil(machine.handle(
            try KagemushaNFCProtocol.selectApplicationCommand()
        ).rejectionReason)
        XCTAssertNil(machine.handle(metadata).rejectionReason)
        XCTAssertNil(machine.handle(try KagemushaNFCProtocol.writeChunkCommand(
            offset: 0,
            bytes: bytes
        )).rejectionReason)
        let result = machine.handle(KagemushaNFCProtocol.commitCommand())
        XCTAssertEqual(result.rejectionReason, .checksumMismatch)
        XCTAssertNil(result.committedPayload)
        XCTAssertFalse(machine.hasPendingWrite)
        XCTAssertEqual(
            machine.handle(KagemushaNFCProtocol.commitCommand()).rejectionReason,
            .conditionsNotSatisfied
        )
    }

    func testCardStateDropsBudgetTerminatedWrite() throws {
        let request = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let machine = try KagemushaNFCCardStateMachine(
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1,
            receiveRequest: request
        )
        let payload = Data(repeating: 0xA5, count: 131)
        let metadata = try KagemushaNFCProtocol.writeMetadataCommand(
            kind: .payment,
            payloadBytes: payload
        )
        XCTAssertNil(machine.handle(
            try KagemushaNFCProtocol.selectApplicationCommand()
        ).rejectionReason)
        XCTAssertNil(machine.handle(metadata).rejectionReason)
        for offset in stride(from: 0, through: 128, by: 2) {
            XCTAssertNil(machine.handle(
                try KagemushaNFCProtocol.writeChunkCommand(
                    offset: offset,
                    bytes: Data([0xA5])
                )
            ).rejectionReason)
        }
        XCTAssertEqual(machine.handle(
            try KagemushaNFCProtocol.writeChunkCommand(
                offset: 130,
                bytes: Data([0xA5])
            )
        ).rejectionReason, .wrongData)
        XCTAssertFalse(machine.hasPendingWrite)
        XCTAssertNil(machine.handle(metadata).rejectionReason)
    }

    func testSelectingAnotherApplicationClearsSelectionAndPendingWrite() throws {
        let request = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let machine = try KagemushaNFCCardStateMachine(chainDiscriminant: SccpV1.tairaI105DiscriminantV1, receiveRequest: request)
        let paymentBytes = Data("invalid-candidate".utf8)
        XCTAssertNil(machine.handle(
            try KagemushaNFCProtocol.selectApplicationCommand()
        ).rejectionReason)
        XCTAssertNil(machine.handle(
            try KagemushaNFCProtocol.writeMetadataCommand(
                kind: .payment,
                payloadBytes: paymentBytes
            )
        ).rejectionReason)
        XCTAssertTrue(machine.hasPendingWrite)

        let otherIdentifier = Data([0xF0, 0x50, 0x4B, 0x45, 0x51])
        XCTAssertEqual(machine.handle(
            try KagemushaNFCProtocol.selectApplicationCommand(
                applicationIdentifier: otherIdentifier
            )
        ).response, KagemushaNFCProtocol.statusNotFound)
        XCTAssertFalse(machine.hasPendingWrite)

        let deselectedCommands = try [
            KagemushaNFCProtocol.getInfoCommand(),
            KagemushaNFCProtocol.readChunkCommand(offset: 0, length: 1),
            KagemushaNFCProtocol.writeMetadataCommand(
                kind: .payment,
                payloadBytes: paymentBytes
            ),
            KagemushaNFCProtocol.writeChunkCommand(
                offset: 0,
                bytes: Data([1])
            ),
            KagemushaNFCProtocol.commitCommand(),
        ]
        for command in deselectedCommands {
            XCTAssertEqual(
                machine.handle(command).rejectionReason,
                .conditionsNotSatisfied
            )
        }

        XCTAssertNil(machine.handle(
            try KagemushaNFCProtocol.selectApplicationCommand()
        ).rejectionReason)
        XCTAssertEqual(
            KagemushaNFCProtocol.responseStatus(
                machine.handle(KagemushaNFCProtocol.getInfoCommand()).response
            ),
            0x9000
        )
    }

    func testDeliveryAmbiguityRetryClassificationIsNarrow() {
        XCTAssertTrue(KagemushaNFCError.acknowledgementPending
            .shouldRetryPreparedPaymentTransfer)
        XCTAssertTrue(KagemushaNFCError.timedOut.shouldRetryPreparedPaymentTransfer)
        XCTAssertTrue(KagemushaNFCError.peerRejected(statusWord: 0x6985)
            .shouldRetryPreparedPaymentTransfer)
        XCTAssertFalse(KagemushaNFCError.peerRejected(statusWord: 0x6A80)
            .shouldRetryPreparedPaymentTransfer)
        XCTAssertFalse(KagemushaNFCError.checksumMismatch
            .shouldRetryPreparedPaymentTransfer)
        XCTAssertFalse(KagemushaNFCError.invalidPeer.shouldRetryPreparedPaymentTransfer)
    }

    func testEveryPostCommitFailureBecomesAcknowledgementPending() {
        let failures: [KagemushaNFCError] = [
            .peerRejected(statusWord: 0x6A80),
            .peerRejected(statusWord: 0x6985),
            .invalidPeer,
            .checksumMismatch,
            .timedOut,
            .cancelled,
        ]
        for failure in failures {
            XCTAssertEqual(
                KagemushaNFCError.afterCommittedPayment(failure),
                .acknowledgementPending,
                "post-commit failure \(failure) must remain an ambiguous delivery"
            )
        }
        XCTAssertEqual(
            KagemushaNFCError.afterCommittedPayment(.acknowledgementPending),
            .acknowledgementPending
        )
    }
}
