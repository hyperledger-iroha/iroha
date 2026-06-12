import XCTest
@testable import IrohaSwift

final class OfflineNoteNfcApduPayloadBudgetTests: XCTestCase {
    func testNfcAidContractParsesNormalizesAndRejectsMalformedValues() throws {
        let bokoloAid = try OfflineNoteNfcApduProtocol.aidData(
            hexString: "  f0504b45504b524e464301  "
        )

        XCTAssertEqual(OfflineNoteNfcApduProtocol.aidHex(for: bokoloAid), "F0504B45504B524E464301")
        XCTAssertEqual(
            try OfflineNoteNfcApduProtocol.normalizedAidHex("f0504b45504b524e464301"),
            "F0504B45504B524E464301"
        )
        XCTAssertTrue(OfflineNoteNfcApduProtocol.isValidAid(bokoloAid))
        XCTAssertEqual(
            OfflineNoteNfcApduProtocol.parseCommand(
                OfflineNoteNfcApduProtocol.selectAidAPDUData(aid: bokoloAid),
                aid: bokoloAid
            ),
            .select
        )

        let cases: [(String, OfflineNoteNfcAidError)] = [
            ("", .missing),
            ("F", .invalidHexLength),
            ("F0504B45504B524E46430Z", .invalidHexCharacter),
            ("F050 4B45504B524E464301", .invalidHexCharacter),
            ("F0504B45", .invalidLength(actual: 4, minimum: 5, maximum: 16)),
            (
                "F0504B45504B524E464301020304050607",
                .invalidLength(actual: 17, minimum: 5, maximum: 16)
            ),
        ]

        for (rawValue, expectedError) in cases {
            XCTAssertThrowsError(try OfflineNoteNfcApduProtocol.aidData(hexString: rawValue)) { error in
                XCTAssertEqual(error as? OfflineNoteNfcAidError, expectedError, rawValue)
            }
        }
    }

    func testNfcApduAcceptsWalletDeviceTransferBudget() throws {
        XCTAssertEqual(OfflineNoteNfcApduProtocol.maxIncomingPayloadBytes, 64 * 1024)

        let payload = Data(repeating: 0xA5, count: OfflineNoteNfcApduProtocol.maxIncomingPayloadBytes)
        let infoBytes = try OfflineNoteNfcApduProtocol.encodeInfo(
            kind: .paymentToken,
            payloadBytes: payload
        )
        let info = try XCTUnwrap(OfflineNoteNfcApduProtocol.decodeInfo(infoBytes))

        XCTAssertEqual(info.payloadLength, payload.count)
        XCTAssertTrue(OfflineNoteNfcApduProtocol.payloadDigestMatches(payload, expectedSha256: info.sha256))
    }

    func testNfcApduRejectsPayloadsAboveWalletDeviceTransferBudget() throws {
        let payload = Data(repeating: 0x5A, count: OfflineNoteNfcApduProtocol.maxIncomingPayloadBytes + 1)

        XCTAssertThrowsError(
            try OfflineNoteNfcApduProtocol.encodeInfo(
                kind: .paymentToken,
                payloadBytes: payload
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteNfcApduError, .invalidPayloadLength)
        }
    }

    func testNfcApduParserRejectsMalformedAndOversizedCommands() {
        let malformedCommands: [(Data?, OfflineNoteNfcCommand)] = [
            (nil, .invalid),
            (Data(), .invalid),
            (Data([0x80, 0x11, 0x00, 0x00]), .invalid),
            (Data([0x80, 0x11, 0x00, 0x00, 0x00, 0x01]), .invalid),
            (Data([0x80, 0x11, 0x00, 0x00, 0x00, 0x04, 0x01]), .invalid),
            (Data([0x80, 0x21, 0x00, 0x00, 0x00, 0x00, 0x00]), .invalid),
            (Data([0x80, 0xFF, 0x00, 0x00, 0x00]), .unsupported),
        ]

        for (apdu, expected) in malformedCommands {
            XCTAssertEqual(OfflineNoteNfcApduProtocol.parseCommand(apdu), expected, "\(String(describing: apdu))")
        }

        let zeroLengthMeta = Data([OfflineNoteNfcPayloadKind.paymentToken.rawValue])
            + Data([0x00, 0x00, 0x00, 0x00])
            + Data(repeating: 0xA5, count: 32)
        XCTAssertEqual(
            OfflineNoteNfcApduProtocol.parseCommand(
                Data([0x80, 0x20, 0x00, 0x00, UInt8(zeroLengthMeta.count)]) + zeroLengthMeta
            ),
            .invalid
        )
    }

    func testNfcPayloadAssemblerRejectsContradictoryOverwritesAndInvalidCommits() throws {
        let payload = Data("abcdef".utf8)
        let assembler = try OfflineNoteNfcPayloadAssembler(
            kind: .paymentToken,
            expectedLength: payload.count,
            expectedSha256: OfflineNoteNfcApduProtocol.sha256(payload)
        )

        XCTAssertFalse(assembler.write(offset: 0, chunk: Data()))
        XCTAssertFalse(assembler.write(offset: payload.count, chunk: Data([0x00])))
        XCTAssertTrue(assembler.write(offset: 0, chunk: Data("abc".utf8)))
        XCTAssertTrue(assembler.write(offset: 1, chunk: Data("bc".utf8)))
        XCTAssertFalse(assembler.write(offset: 1, chunk: Data("ZZ".utf8)))
        XCTAssertThrowsError(try assembler.commit()) { error in
            XCTAssertEqual(error as? OfflineNoteNfcApduError, .incompletePayload)
        }

        XCTAssertTrue(assembler.write(offset: 3, chunk: Data("def".utf8)))
        XCTAssertEqual(try assembler.commit(), payload)

        let tamperedAssembler = try OfflineNoteNfcPayloadAssembler(
            kind: .paymentToken,
            expectedLength: payload.count,
            expectedSha256: OfflineNoteNfcApduProtocol.sha256(payload)
        )
        XCTAssertTrue(tamperedAssembler.write(offset: 0, chunk: Data("abcdeg".utf8)))
        XCTAssertThrowsError(try tamperedAssembler.commit()) { error in
            XCTAssertEqual(error as? OfflineNoteNfcApduError, .checksumMismatch)
        }
    }

    func testNfcCardSessionStateMachineRejectsInvalidSequencesAndTracksAckReads() throws {
        let receiveRequest = Data("receive-request".utf8)
        let payment = Data("payment-token".utf8)
        let ack = Data("ack".utf8)
        let stateMachine = try OfflineNoteNfcCardSessionStateMachine(
            initialKind: .receiveRequest,
            initialPayloadBytes: receiveRequest
        )

        XCTAssertEqual(
            stateMachine.handle(OfflineNoteNfcApduProtocol.commitAPDUData()).rejectionReason,
            .conditionsNotSatisfied
        )
        XCTAssertEqual(
            stateMachine.handle(try OfflineNoteNfcApduProtocol.writeChunkAPDUData(offset: 0, bytes: payment)).rejectionReason,
            .conditionsNotSatisfied
        )
        XCTAssertEqual(
            stateMachine.handle(
                try OfflineNoteNfcApduProtocol.writeMetaAPDUData(kind: .receiptAck, payloadBytes: ack)
            ).rejectionReason,
            .wrongData
        )

        XCTAssertNil(
            stateMachine.handle(
                try OfflineNoteNfcApduProtocol.writeMetaAPDUData(kind: .paymentToken, payloadBytes: payment)
            ).rejectionReason
        )
        XCTAssertTrue(stateMachine.hasPendingWrite)
        XCTAssertEqual(
            stateMachine.handle(
                try OfflineNoteNfcApduProtocol.writeMetaAPDUData(kind: .paymentToken, payloadBytes: payment)
            ).rejectionReason,
            .conditionsNotSatisfied
        )
        XCTAssertNil(
            stateMachine.handle(
                try OfflineNoteNfcApduProtocol.writeChunkAPDUData(offset: 0, bytes: payment)
            ).rejectionReason
        )
        XCTAssertEqual(
            stateMachine.handle(
                try OfflineNoteNfcApduProtocol.writeChunkAPDUData(offset: 0, bytes: Data("tampered".utf8))
            ).rejectionReason,
            .wrongData
        )

        let commit = stateMachine.handle(OfflineNoteNfcApduProtocol.commitAPDUData())
        XCTAssertNil(commit.rejectionReason)
        XCTAssertEqual(commit.committedPayload?.kind, .paymentToken)
        XCTAssertEqual(commit.committedPayload?.payloadBytes, payment)
        XCTAssertFalse(stateMachine.isReadable)
        XCTAssertFalse(stateMachine.hasPendingWrite)
        XCTAssertEqual(
            stateMachine.handle(OfflineNoteNfcApduProtocol.getInfoAPDUData()).rejectionReason,
            .conditionsNotSatisfied
        )

        try stateMachine.publishPayload(kind: .receiptAck, payloadBytes: ack)
        XCTAssertEqual(stateMachine.currentPayloadKind, .receiptAck)
        XCTAssertEqual(stateMachine.receiptAckReadProgress?.readByteCount, 0)

        let firstRead = stateMachine.handle(try OfflineNoteNfcApduProtocol.readChunkAPDUData(offset: 0, length: 2))
        XCTAssertEqual(OfflineNoteNfcApduProtocol.responseData(firstRead.response), Data("ac".utf8))
        XCTAssertEqual(firstRead.receiptAckReadRange, 0..<2)
        XCTAssertFalse(stateMachine.markReceiptAckBytesRead(0..<2))
        XCTAssertEqual(stateMachine.receiptAckReadProgress?.readByteCount, 2)
        XCTAssertFalse(stateMachine.hasCompleted)

        let secondRead = stateMachine.handle(try OfflineNoteNfcApduProtocol.readChunkAPDUData(offset: 2, length: 2))
        XCTAssertEqual(OfflineNoteNfcApduProtocol.responseData(secondRead.response), Data("k".utf8))
        XCTAssertEqual(secondRead.receiptAckReadRange, 2..<3)
        XCTAssertTrue(stateMachine.markReceiptAckBytesRead(2..<3))
        XCTAssertTrue(stateMachine.hasCompleted)
        XCTAssertEqual(stateMachine.receiptAckReadProgress?.readByteCount, ack.count)
    }
}
