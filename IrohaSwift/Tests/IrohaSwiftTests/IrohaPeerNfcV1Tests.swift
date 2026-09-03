import XCTest
@testable import IrohaSwift

final class IrohaPeerNfcV1Tests: XCTestCase {
    private let sessionID = Data((1...IrohaPeerNfcV1.sessionIDBytes).map(UInt8.init))
    private let zeroHash = Data(repeating: 0, count: IrohaPeerNfcV1.hashBytes)

    func testFiveMessageInstructionAndPhaseOrderIsFrozen() {
        XCTAssertEqual(
            IrohaPeerNfcInstructionV1.allCases.map(\.rawValue),
            [0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25]
        )
        XCTAssertEqual(IrohaPeerNfcPhaseV1.allCases.map(\.rawValue), [1, 2, 3, 4, 5, 6])
        XCTAssertEqual(IrohaPeerWireKindV1.allCases.map(\.rawValue), [1, 2, 3, 4, 5])
    }

    func testEveryFiveMessageAPDURoundTripsWithUInt32Offsets() throws {
        let request = try message(kind: .request, byte: 0x31, count: 96)
        let intent = try message(kind: .intent, byte: 0x32, count: 128)
        let ticket = try message(kind: .ticket, byte: 0x33, count: 112)
        let payment = try message(kind: .payment, byte: 0x34, count: 192)
        let acknowledgement = try message(kind: .acknowledgement, byte: 0x35, count: 80)
        let offset: UInt32 = 0x0102_0304
        let commands: [IrohaPeerNfcCommandV1] = [
            .selectApplication,
            .getInfo,
            .readRequest(
                sessionID: sessionID,
                requestCanonicalHash: request.canonicalHash,
                offset: offset,
                length: 240
            ),
            .beginIntent(
                sessionID: sessionID,
                requestCanonicalHash: request.canonicalHash,
                intentHeader: intent.header.bytes
            ),
            .writeIntent(
                sessionID: sessionID,
                intentWireHash: intent.wireHash,
                offset: offset,
                bytes: Data(repeating: 0x41, count: 300)
            ),
            .commitIntent(
                sessionID: sessionID,
                requestCanonicalHash: request.canonicalHash,
                intentWireHash: intent.wireHash
            ),
            .readTicket(
                sessionID: sessionID,
                intentWireHash: intent.wireHash,
                offset: offset,
                length: 240
            ),
            .beginPayment(
                sessionID: sessionID,
                ticketWireHash: ticket.wireHash,
                paymentHeader: payment.header.bytes
            ),
            .writePayment(
                sessionID: sessionID,
                paymentWireHash: payment.wireHash,
                offset: offset,
                bytes: Data(repeating: 0x42, count: 300)
            ),
            .commitPayment(
                sessionID: sessionID,
                ticketWireHash: ticket.wireHash,
                paymentWireHash: payment.wireHash
            ),
            .readAcknowledgement(
                sessionID: sessionID,
                paymentWireHash: payment.wireHash,
                offset: offset,
                length: 240
            ),
            .confirmAcknowledgement(
                sessionID: sessionID,
                paymentWireHash: payment.wireHash,
                acknowledgementWireHash: acknowledgement.wireHash
            ),
            .getStatus(
                sessionID: sessionID,
                requestCanonicalHash: request.canonicalHash
            ),
        ]

        for command in commands {
            XCTAssertEqual(
                try IrohaPeerNfcAPDUCodecV1.decode(IrohaPeerNfcAPDUCodecV1.encode(command)),
                command
            )
        }
    }

    func testStatusRoundTripsEveryFiveMessagePhase() throws {
        let request = try message(kind: .request, byte: 0x51, count: 96)
        let intent = try message(kind: .intent, byte: 0x52, count: 128)
        let ticket = try message(kind: .ticket, byte: 0x53, count: 112)
        let payment = try message(kind: .payment, byte: 0x54, count: 192)
        let acknowledgement = try message(kind: .acknowledgement, byte: 0x55, count: 80)
        let identity = try identity(for: request)

        for phase in IrohaPeerNfcPhaseV1.allCases {
            let inbound: IrohaPeerWireMessageV1?
            let received: Int
            let nextMissing: Int
            let outbound: IrohaPeerWireMessageV1?
            switch phase {
            case .requestReady:
                inbound = nil
                received = 0
                nextMissing = 0
                outbound = nil
            case .intentReceiving:
                inbound = intent
                received = 7
                nextMissing = 7
                outbound = nil
            case .ticketReady:
                inbound = intent
                received = intent.encoded.count
                nextMissing = intent.encoded.count
                outbound = ticket
            case .paymentReceiving:
                inbound = payment
                received = 9
                nextMissing = 9
                outbound = ticket
            case .acknowledgementReady, .complete:
                inbound = payment
                received = payment.encoded.count
                nextMissing = payment.encoded.count
                outbound = acknowledgement
            }
            let status = try IrohaPeerNfcStatusV1(
                phase: phase,
                flags: phase == .requestReady
                    ? [.idempotentWrites]
                    : [.idempotentWrites, .durableState],
                identity: identity,
                inboundKind: inbound?.kind,
                inboundLength: inbound?.encoded.count ?? 0,
                receivedInboundBytes: received,
                nextMissingInboundOffset: nextMissing,
                inboundWireHash: inbound?.wireHash ?? zeroHash,
                outboundKind: outbound?.kind,
                outboundLength: outbound?.encoded.count ?? 0,
                outboundWireHash: outbound?.wireHash ?? zeroHash,
                maximumReadChunkBytes: 240,
                maximumWriteChunkBytes: 240
            )
            XCTAssertEqual(status.encode().count, IrohaPeerNfcV1.statusBytes)
            XCTAssertEqual(try IrohaPeerNfcStatusV1.decode(status.encode()), status)
        }
    }

    func testIntentReassemblyAcceptsOutOfOrderAndIdenticalOverlap() throws {
        let request = try message(kind: .request, byte: 0x61, count: 96)
        let intent = try message(kind: .intent, byte: 0x62, count: 128)
        var receiver = try IrohaPeerNfcReceiverSessionV1(
            sessionID: sessionID,
            receiveRequest: request.encoded,
            profilePolicy: .init(profile: .kagemushaV1)
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
            IrohaPeerNfcDurableIntentAdmissionV1(context: admission)
        )
        let split = intent.encoded.count / 2
        let suffix = intent.encoded.subdata(in: split..<intent.encoded.count)
        let prefix = intent.encoded.subdata(in: 0..<split)

        try receiver.handle(.writeIntent(
            sessionID: sessionID,
            intentWireHash: intent.wireHash,
            offset: UInt32(split),
            bytes: suffix
        ))
        XCTAssertEqual(try receiver.status().receivedInboundBytes, suffix.count)
        XCTAssertEqual(try receiver.status().nextMissingInboundOffset, 0)
        try receiver.handle(.writeIntent(
            sessionID: sessionID,
            intentWireHash: intent.wireHash,
            offset: UInt32(split),
            bytes: suffix
        ))
        try receiver.handle(.writeIntent(
            sessionID: sessionID,
            intentWireHash: intent.wireHash,
            offset: 0,
            bytes: prefix
        ))
        XCTAssertEqual(
            try receiver.status().receivedInboundBytes,
            intent.encoded.count
        )
        XCTAssertEqual(
            try receiver.status().nextMissingInboundOffset,
            intent.encoded.count
        )

        var conflicting = Data(suffix.prefix(1))
        conflicting[0] ^= 0xff
        XCTAssertThrowsError(try receiver.handle(.writeIntent(
            sessionID: sessionID,
            intentWireHash: intent.wireHash,
            offset: UInt32(split),
            bytes: conflicting
        ))) {
            XCTAssertEqual($0 as? IrohaPeerNfcErrorV1, .conflictingReplay)
        }
    }

    func testOpaqueIntentCannotCrossDurableTicketBoundary() throws {
        let request = try message(kind: .request, byte: 0x71, count: 96)
        let intent = try message(kind: .intent, byte: 0x72, count: 128)
        var receiver = try IrohaPeerNfcReceiverSessionV1(
            sessionID: sessionID,
            receiveRequest: request.encoded,
            profilePolicy: .init(profile: .kagemushaV1)
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
            IrohaPeerNfcDurableIntentAdmissionV1(context: admission)
        )
        try receiver.handle(.writeIntent(
            sessionID: sessionID,
            intentWireHash: intent.wireHash,
            offset: 0,
            bytes: intent.encoded
        ))
        XCTAssertThrowsError(try receiver.prepareIntentCommit(
            .commitIntent(
                sessionID: sessionID,
                requestCanonicalHash: request.canonicalHash,
                intentWireHash: intent.wireHash
            )
        )) {
            XCTAssertEqual($0 as? IrohaPeerNfcErrorV1, .continuityMismatch)
        }
        XCTAssertEqual(receiver.phase, .intentReceiving)
    }

    private func identity(
        for request: IrohaPeerWireMessageV1
    ) throws -> IrohaPeerNfcRequestIdentityV1 {
        try IrohaPeerNfcRequestIdentityV1(
            profile: request.profile,
            sessionID: sessionID,
            requestCanonicalHash: request.canonicalHash,
            requestWireHash: request.wireHash
        )
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
}
