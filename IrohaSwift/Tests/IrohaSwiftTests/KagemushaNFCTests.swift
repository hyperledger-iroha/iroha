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
            ("request", 824, 4, 6),
            ("acknowledgement", 471, 3, 5),
            ("payment-depth-1-hop-1", 6_677, 31, 33),
            ("payment-depth-8-hop-8", 6_848, 32, 34),
            ("payment-depth-16-hop-8", 7_040, 32, 34),
            ("payment-depth-32-hop-8", 7_424, 34, 36),
            ("payment-depth-64-hop-8", 8_192, 38, 40),
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
        for invalid in ["", "F", "F0504B4Z", "F050 4B", "F0504B45",
                        "F0504B45504B524E464301020304050607"] {
            XCTAssertThrowsError(
                try KagemushaNFCProtocol.applicationIdentifier(hex: invalid),
                invalid
            )
        }
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

    func testEveryCanonicalNFCChunkRoundTripsAndReassemblesOutOfOrder() throws {
        let request = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let payment = try KagemushaPeerTransportTestFixtures.payment(request: request)
        let acknowledgement = try KagemushaPeerTransportTestFixtures.acknowledgement(
            request: request,
            payment: payment
        )
        let payloads: [KagemushaPeerPayload] = [
            .receiveRequest(request),
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
        let request = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let machine = try KagemushaNFCCardStateMachine(receiveRequest: request)
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
        let request = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let payment = try KagemushaPeerTransportTestFixtures.payment(request: request)
        let acknowledgement = try KagemushaPeerTransportTestFixtures.acknowledgement(
            request: request,
            payment: payment
        )
        let machine = try KagemushaNFCCardStateMachine(receiveRequest: request)
        let paymentBytes = payment.archive
        let commands = try KagemushaNFCProtocol.writePayloadCommands(
            kind: .payment,
            payloadBytes: paymentBytes,
            maximumChunkLength: 97
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
        guard KagemushaRecursiveSpend.hasRequiredNativeSymbols else {
            XCTAssertNil(commit.committedPayload)
            XCTAssertEqual(commit.rejectionReason, .invalidCommittedPayload)
            XCTAssertEqual(KagemushaNFCProtocol.responseStatus(commit.response), 0x6A80)
            XCTAssertTrue(machine.isReadable)
            XCTAssertTrue(machine.hasPendingWrite)
            return
        }
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

    func testInvalidDigestCommitDoesNotBecomeTypedPayment() throws {
        let request = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let machine = try KagemushaNFCCardStateMachine(receiveRequest: request)
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
    }

    func testSelectingAnotherApplicationClearsSelectionAndPendingWrite() throws {
        let request = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let machine = try KagemushaNFCCardStateMachine(receiveRequest: request)
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
