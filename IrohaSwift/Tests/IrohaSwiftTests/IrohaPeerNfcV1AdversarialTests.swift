import XCTest
@testable import IrohaSwift

final class IrohaPeerNfcV1AdversarialTests: XCTestCase {
    private let sessionID = Data((1...IrohaPeerNfcV1.sessionIDBytes).map(UInt8.init))

    func testAPDUDecoderRejectsNonCanonicalExtendedLengthAliases() throws {
        assertInvalidAPDU(Data([0x80, 0x10, 0x00, 0x00, 0x00, 0x00, 0xfe]))

        let hash = Data(repeating: 0x31, count: IrohaPeerNfcV1.hashBytes)
        let write = try IrohaPeerNfcAPDUCodecV1.encode(.writeIntent(
            sessionID: sessionID,
            intentWireHash: hash,
            offset: 0,
            bytes: Data([0x55])
        ))
        let body = write.subdata(in: 5..<write.count)
        var nonCanonical = Data([0x80, 0x13, 0x00, 0x00, 0x00, 0x00])
        nonCanonical.append(UInt8(body.count))
        nonCanonical.append(body)
        assertInvalidAPDU(nonCanonical)
    }

    func testExtremeOffsetsAndOversizedChunksLeaveIntentStateUnchanged() throws {
        let request = try message(kind: .request, byte: 0x41, count: 96)
        let intent = try message(kind: .intent, byte: 0x42, count: 128)
        let limits = IrohaPeerNfcLimitsV1(
            maximumReadChunkBytes: 128,
            maximumWriteChunkBytes: 128
        )
        var receiver = try IrohaPeerNfcReceiverSessionV1(
            sessionID: sessionID,
            receiveRequest: request.encoded,
            limits: limits
        )
        let begin = IrohaPeerNfcCommandV1.beginIntent(
            sessionID: sessionID,
            requestCanonicalHash: request.canonicalHash,
            intentHeader: intent.header.bytes
        )
        guard case .requiresDurableAdmission(let admission) =
            try receiver.prepareIntentAdmission(begin) else {
            return XCTFail("expected durable intent admission")
        }
        try receiver.installIntentAdmission(
            IrohaPeerNfcDurableIntentAdmissionV1(context: admission, limits: limits)
        )

        XCTAssertThrowsError(try receiver.handle(.writeIntent(
            sessionID: sessionID,
            intentWireHash: intent.wireHash,
            offset: UInt32.max,
            bytes: Data([0x01])
        ))) {
            XCTAssertEqual($0 as? IrohaPeerNfcErrorV1, .invalidOffset)
        }
        XCTAssertThrowsError(try receiver.handle(.writeIntent(
            sessionID: sessionID,
            intentWireHash: intent.wireHash,
            offset: 0,
            bytes: Data(repeating: 0x01, count: 129)
        ))) {
            XCTAssertEqual($0 as? IrohaPeerNfcErrorV1, .invalidOffset)
        }
        XCTAssertEqual(try receiver.status().receivedInboundBytes, 0)
    }

    func testReaderRejectsOversizeRequestBeforeReadOrHardwareBoundary() async throws {
        let request = try message(kind: .request, byte: 0x61, count: 300)
        let identity = try IrohaPeerNfcRequestIdentityV1(
            profile: request.profile,
            sessionID: sessionID,
            requestCanonicalHash: request.canonicalHash,
            requestWireHash: request.wireHash
        )
        let info = try IrohaPeerNfcInfoV1(
            phase: .requestReady,
            flags: [.idempotentWrites],
            identity: identity,
            requestLength: request.encoded.count,
            maximumReadChunkBytes: 240,
            maximumWriteChunkBytes: 240
        )
        let local = IrohaPeerNfcLimitsV1(
            maximumMessageBytes: 128,
            maximumReadChunkBytes: 128,
            maximumWriteChunkBytes: 128
        )
        let observed = LockedCommands()

        do {
            _ = try await IrohaPeerNfcReaderExchangeV1.run(
                profilePolicy: .sameProfile(.kagemushaV1),
                limits: local,
                transceive: { command in
                    observed.append(command)
                    switch command {
                    case .selectApplication:
                        return .init(statusWord: .success)
                    case .getInfo:
                        return .init(data: info.encode(), statusWord: .success)
                    default:
                        throw TestFailure.unexpected
                    }
                },
                loadOrCreateDurableCheckpoint: { _, _ in throw TestFailure.unexpected },
                preparePaymentCheckpoint: { _, _ in throw TestFailure.unexpected },
                updateDurableCheckpoint: { _ in throw TestFailure.unexpected }
            )
            XCTFail("expected local message-bound rejection")
        } catch let error as IrohaPeerNfcErrorV1 {
            XCTAssertEqual(
                error,
                .messageTooLarge(actual: request.encoded.count, maximum: 128)
            )
        }
        XCTAssertEqual(observed.types, ["select", "info"])
    }

    func testReaderRejectsUnexpectedControlResponseData() async throws {
        let observed = LockedCommands()
        do {
            _ = try await IrohaPeerNfcReaderExchangeV1.run(
                profilePolicy: .sameProfile(.kagemushaV1),
                transceive: { command in
                    observed.append(command)
                    return .init(data: Data([0]), statusWord: .success)
                },
                loadOrCreateDurableCheckpoint: { _, _ in throw TestFailure.unexpected },
                preparePaymentCheckpoint: { _, _ in throw TestFailure.unexpected },
                updateDurableCheckpoint: { _ in throw TestFailure.unexpected }
            )
            XCTFail("expected non-empty SELECT response rejection")
        } catch let error as IrohaPeerNfcErrorV1 {
            XCTAssertEqual(error, .invalidLength)
        }
        XCTAssertEqual(observed.types, ["select"])
    }

    private func message(
        kind: IrohaPeerWireKindV1,
        byte: UInt8,
        count: Int
    ) throws -> IrohaPeerWireMessageV1 {
        try IrohaPeerWireMessageV1(
            profile: .kagemushaV1,
            kind: kind,
            schemaVersion: 1,
            canonicalPayload: irohaPeerKagemushaStructuralArchiveV1(
                kind: kind,
                payload: Data(repeating: byte, count: count)
            )
        )
    }

    private func assertInvalidAPDU(
        _ apdu: Data,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        XCTAssertThrowsError(
            try IrohaPeerNfcAPDUCodecV1.decode(apdu),
            file: file,
            line: line
        ) {
            XCTAssertEqual($0 as? IrohaPeerNfcErrorV1, .invalidAPDU, file: file, line: line)
        }
    }
}

private final class LockedCommands: @unchecked Sendable {
    private let lock = NSLock()
    private var commands: [IrohaPeerNfcCommandV1] = []

    func append(_ command: IrohaPeerNfcCommandV1) {
        lock.lock()
        commands.append(command)
        lock.unlock()
    }

    var types: [String] {
        lock.lock()
        defer { lock.unlock() }
        return commands.map {
            switch $0 {
            case .selectApplication: return "select"
            case .getInfo: return "info"
            default: return "other"
            }
        }
    }
}

private enum TestFailure: Error {
    case unexpected
}
