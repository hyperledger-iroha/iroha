import XCTest
@testable import IrohaSwift

final class KagemushaNFCTests: XCTestCase {
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

    func testPayloadBudgetUsesAuthoritativeTwelveKiBLimit() throws {
        XCTAssertEqual(KagemushaNFCProtocol.maximumPayloadBytes, 12 * 1024)
        let maximum = Data(
            repeating: 0xA5,
            count: KagemushaNFCProtocol.maximumPayloadBytes
        )
        let info = try KagemushaNFCProtocol.encodeInfo(
            kind: .payment,
            payloadBytes: maximum
        )
        XCTAssertEqual(
            KagemushaNFCProtocol.decodeInfo(info)?.payloadLength,
            maximum.count
        )
        XCTAssertThrowsError(try KagemushaNFCProtocol.encodeInfo(
            kind: .payment,
            payloadBytes: maximum + Data([0])
        )) { error in
            XCTAssertEqual(error as? KagemushaNFCError, .invalidPayloadLength)
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

    func testCardStateMachineReadsRequestAndRejectsInvalidWriteSequences() throws {
        let request = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let machine = try KagemushaNFCCardStateMachine(receiveRequest: request)
        let candidatePayment = Data("PKK2P.candidate".utf8)
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

        let invalidAck = Data("PKK2A.invalid".utf8)
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
        let paymentBytes = Data(try KagemushaPeerTextCodec.encode(.payment(payment)).utf8)
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
        var commit: KagemushaNFCCardHandleResult?
        for command in commands { commit = machine.handle(command) }
        XCTAssertEqual(commit?.committedPayload, .payment(payment))
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
        let bytes = Data("PKK2P.not-a-valid-archive".utf8)
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
        let paymentBytes = Data("PKK2P.candidate".utf8)
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
