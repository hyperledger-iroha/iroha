import Foundation

/// Transport-neutral NFC V1 constants for the sole five-message KAGEMUSHA `IPM1` exchange.
///
/// V1 has one application identifier, one command set, and no codec negotiation
/// or fallback.
public enum IrohaPeerNfcV1 {
    /// ISO/IEC 7816 application identifier `F0504B45504B524E464301`.
    public static let applicationIdentifierHex = "F0504B45504B524E464301"
    public static let applicationIdentifier = decodeHex(applicationIdentifierHex)
    public static let buildProfileMarker = "IrohaPeerNfcV1.AID.F0504B45504B524E464301"
    public static let commandClass: UInt8 = 0x80
    public static let wireVersion: UInt8 = 1
    public static let sessionIDBytes = 16
    public static let hashBytes = 32
    public static let maximumChunkBytes = 4_096
    public static let maximumMessageBytes =
        IrohaPeerWireMessageV1.headerBytes + KagemushaWireV1.maximumPaymentBytes
    public static let infoBytes = 98
    public static let statusBytes = 178

    private static func decodeHex(_ value: String) -> Data {
        precondition(value.count.isMultiple(of: 2))
        var bytes = Data()
        bytes.reserveCapacity(value.count / 2)
        var cursor = value.startIndex
        while cursor < value.endIndex {
            let next = value.index(cursor, offsetBy: 2)
            guard let byte = UInt8(value[cursor..<next], radix: 16) else {
                preconditionFailure("Iroha peer NFC AID must be hexadecimal")
            }
            bytes.append(byte)
            cursor = next
        }
        return bytes
    }
    public static let intentAdmissionBytes = 244
    public static let ticketedPaymentAdmissionBytes = 308

    fileprivate static let infoMagic = Data("INF1".utf8)
    fileprivate static let statusMagic = Data("NST1".utf8)
    fileprivate static let intentAdmissionMagic = Data("IAA1".utf8)
    fileprivate static let ticketedPaymentAdmissionMagic = Data("ITP1".utf8)
    fileprivate static let durableTicketMagic = Data("IDT1".utf8)
    fileprivate static let durableAckMagic = Data("IDA1".utf8)
    fileprivate static let senderCheckpointMagic = Data("ISC1".utf8)
}

/// The only proprietary instructions accepted by Iroha peer NFC V1.
public enum IrohaPeerNfcInstructionV1: UInt8, CaseIterable, Sendable {
    case getInfo = 0x10
    case readRequest = 0x11
    case beginIntent = 0x12
    case writeIntent = 0x13
    case commitIntent = 0x14
    case readTicket = 0x15
    case beginPayment = 0x20
    case writePayment = 0x21
    case commitPayment = 0x22
    case readAcknowledgement = 0x23
    case confirmAcknowledgement = 0x24
    case getStatus = 0x25
}

/// Receiver phases exposed by `GET_INFO` and `GET_STATUS`.
public enum IrohaPeerNfcPhaseV1: UInt8, CaseIterable, Sendable {
    case requestReady = 1
    case intentReceiving = 2
    case ticketReady = 3
    case paymentReceiving = 4
    case acknowledgementReady = 5
    case complete = 6
}

/// Capabilities and durability boundaries advertised in NFC status records.
public struct IrohaPeerNfcFlagsV1: OptionSet, Equatable, Sendable {
    public let rawValue: UInt8

    public init(rawValue: UInt8) {
        self.rawValue = rawValue
    }

    /// Replayed or partially overlapping `WRITE` commands are accepted only
    /// when every already-received byte is identical.
    public static let idempotentWrites = IrohaPeerNfcFlagsV1(rawValue: 1 << 0)
    /// The current monotonic receiver prefix crossed its required durable boundary.
    public static let durableState = IrohaPeerNfcFlagsV1(rawValue: 1 << 1)

    fileprivate static let known: IrohaPeerNfcFlagsV1 = [
        .idempotentWrites,
        .durableState,
    ]
}

/// Bounded message and APDU chunk sizes for a peer NFC endpoint.
public struct IrohaPeerNfcLimitsV1: Equatable, Sendable {
    public let maximumMessageBytes: Int
    public let maximumReadChunkBytes: Int
    public let maximumWriteChunkBytes: Int

    public init(
        maximumMessageBytes: Int = IrohaPeerNfcV1.maximumMessageBytes,
        maximumReadChunkBytes: Int = IrohaPeerNfcV1.maximumChunkBytes,
        maximumWriteChunkBytes: Int = IrohaPeerNfcV1.maximumChunkBytes
    ) {
        precondition(
            Self.areValid(
                maximumMessageBytes: maximumMessageBytes,
                maximumReadChunkBytes: maximumReadChunkBytes,
                maximumWriteChunkBytes: maximumWriteChunkBytes
            )
        )
        self.maximumMessageBytes = maximumMessageBytes
        self.maximumReadChunkBytes = maximumReadChunkBytes
        self.maximumWriteChunkBytes = maximumWriteChunkBytes
    }

    public static let `default` = IrohaPeerNfcLimitsV1()

    static func areValid(
        maximumMessageBytes: Int,
        maximumReadChunkBytes: Int,
        maximumWriteChunkBytes: Int
    ) -> Bool {
        (IrohaPeerWireMessageV1.headerBytes...IrohaPeerNfcV1.maximumMessageBytes)
            .contains(maximumMessageBytes) &&
            (1...IrohaPeerNfcV1.maximumChunkBytes).contains(maximumReadChunkBytes) &&
            (1...IrohaPeerNfcV1.maximumChunkBytes).contains(maximumWriteChunkBytes)
    }
}

/// One immutable application profile for all five IPM1 messages in an NFC handoff.
public struct IrohaPeerNfcProfilePolicyV1: Equatable, Sendable {
    public let profile: IrohaPeerPayloadProfile

    public init(profile: IrohaPeerPayloadProfile) {
        precondition(profile != .reject)
        self.profile = profile
    }

    /// Convenience spelling equivalent to `init(profile:)`.
    public static func sameProfile(
        _ profile: IrohaPeerPayloadProfile
    ) -> IrohaPeerNfcProfilePolicyV1 {
        IrohaPeerNfcProfilePolicyV1(profile: profile)
    }

    public func accepts(_ candidate: IrohaPeerPayloadProfile) -> Bool {
        profile == candidate
    }
}

public enum IrohaPeerNfcErrorV1: Error, Equatable, LocalizedError, Sendable {
    case invalidApplicationIdentifier
    case invalidCommandClass(UInt8)
    case unsupportedInstruction(UInt8)
    case invalidAPDU
    case invalidLength
    case messageTooLarge(actual: Int, maximum: Int)
    case invalidOffset
    case invalidSession
    case invalidHash
    case invalidProfile
    case invalidKind
    case invalidFlags
    case invalidIPM1
    case continuityMismatch
    case conflictingReplay
    case stateMismatch
    case durableIntentAdmissionRequired
    case durableAdmissionRequired
    case durableTicketRequired
    case durableCommitRequired
    case acknowledgementNotDurable
    case peerRejected(operation: IrohaPeerNfcOperationV1)
    case peerPersistenceFailure(operation: IrohaPeerNfcOperationV1)

    public var errorDescription: String? {
        switch self {
        case .invalidApplicationIdentifier:
            return "The Iroha peer NFC application identifier is invalid."
        case .invalidCommandClass(let value):
            return String(format: "Unsupported NFC command class %02X.", value)
        case .unsupportedInstruction(let value):
            return String(format: "Unsupported NFC instruction %02X.", value)
        case .invalidAPDU: return "The NFC command APDU is malformed."
        case .invalidLength: return "The NFC record length is invalid."
        case .messageTooLarge(let actual, let maximum):
            return "The NFC message is \(actual) bytes; the limit is \(maximum)."
        case .invalidOffset: return "The NFC chunk offset is invalid."
        case .invalidSession: return "The NFC session identifier is invalid."
        case .invalidHash: return "The NFC message hash is invalid."
        case .invalidProfile: return "The NFC payload profile is invalid."
        case .invalidKind: return "The NFC IPM1 message kind is invalid."
        case .invalidFlags: return "The NFC status flags are invalid."
        case .invalidIPM1: return "The NFC payload is not a valid IPM1 message."
        case .continuityMismatch: return "The NFC transfer continuity does not match."
        case .conflictingReplay: return "The NFC retry conflicts with already-received bytes."
        case .stateMismatch: return "The NFC command is not valid in the current phase."
        case .durableIntentAdmissionRequired:
            return "BEGIN_AUTHORIZATION requires application-provided durable admission."
        case .durableAdmissionRequired:
            return "BEGIN_PAYMENT requires application-provided durable admission."
        case .durableTicketRequired:
            return "COMMIT_AUTHORIZATION requires a durably issued acceptance ticket."
        case .durableCommitRequired:
            return "COMMIT requires an application-provided durable acknowledgement."
        case .acknowledgementNotDurable:
            return "The acknowledgement must be durable before it can be confirmed."
        case .peerRejected(let operation):
            return "The nearby NFC peer rejected \(operation.rawValue)."
        case .peerPersistenceFailure(let operation):
            return "The nearby NFC peer could not durably persist \(operation.rawValue)."
        }
    }
}

/// Stable command names used by typed peer failures without exposing APDU
/// bytes or collapsing persistence failures into generic state errors.
public enum IrohaPeerNfcOperationV1: String, CaseIterable, Sendable {
    case selectApplication
    case getInfo
    case readRequest
    case beginIntent
    case writeIntent
    case commitIntent
    case readTicket
    case beginPayment
    case writePayment
    case commitPayment
    case readAcknowledgement
    case confirmAcknowledgement
    case getStatus
}

/// Marker for a transport error that makes delivery of the final CONFIRM
/// response unknowable. Other errors remain deterministic failures.
public protocol IrohaPeerNfcAmbiguousResponseErrorV1: Error, Sendable {}

/// Stable request identity shared by the NFC reader and card endpoint.
public struct IrohaPeerNfcRequestIdentityV1: Equatable, Sendable {
    public let profile: IrohaPeerPayloadProfile
    public let sessionID: Data
    public let requestCanonicalHash: Data
    public let requestWireHash: Data

    public init(
        profile: IrohaPeerPayloadProfile,
        sessionID: Data,
        requestCanonicalHash: Data,
        requestWireHash: Data
    ) throws {
        guard profile != .reject else { throw IrohaPeerNfcErrorV1.invalidProfile }
        try nfcRequireSession(sessionID)
        try nfcRequireHash(requestCanonicalHash)
        try nfcRequireHash(requestWireHash)
        self.profile = profile
        self.sessionID = Data(sessionID)
        self.requestCanonicalHash = Data(requestCanonicalHash)
        self.requestWireHash = Data(requestWireHash)
    }
}

/// Fixed 98-byte response to `GET_INFO`.
public struct IrohaPeerNfcInfoV1: Equatable, Sendable {
    public let phase: IrohaPeerNfcPhaseV1
    public let flags: IrohaPeerNfcFlagsV1
    public let identity: IrohaPeerNfcRequestIdentityV1
    public let requestLength: Int
    public let maximumReadChunkBytes: Int
    public let maximumWriteChunkBytes: Int

    public init(
        phase: IrohaPeerNfcPhaseV1,
        flags: IrohaPeerNfcFlagsV1,
        identity: IrohaPeerNfcRequestIdentityV1,
        requestLength: Int,
        maximumReadChunkBytes: Int,
        maximumWriteChunkBytes: Int
    ) throws {
        guard flags.subtracting(.known).isEmpty else {
            throw IrohaPeerNfcErrorV1.invalidFlags
        }
        guard requestLength > IrohaPeerWireMessageV1.headerBytes,
              requestLength <= IrohaPeerNfcV1.maximumMessageBytes else {
            throw IrohaPeerNfcErrorV1.invalidLength
        }
        try nfcRequireChunkLimit(maximumReadChunkBytes)
        try nfcRequireChunkLimit(maximumWriteChunkBytes)
        let mustBeDurable = phase != .requestReady
        guard flags.contains(.idempotentWrites),
              flags.contains(.durableState) == mustBeDurable else {
            throw IrohaPeerNfcErrorV1.invalidFlags
        }
        self.phase = phase
        self.flags = flags
        self.identity = identity
        self.requestLength = requestLength
        self.maximumReadChunkBytes = maximumReadChunkBytes
        self.maximumWriteChunkBytes = maximumWriteChunkBytes
    }

    public func encode() -> Data {
        var output = IrohaPeerNfcV1.infoMagic
        output.append(IrohaPeerNfcV1.wireVersion)
        output.append(phase.rawValue)
        output.nfcAppendUInt16BE(identity.profile.rawValue)
        output.append(flags.rawValue)
        output.append(0)
        output.append(identity.sessionID)
        output.nfcAppendUInt32BE(UInt32(requestLength))
        output.append(identity.requestCanonicalHash)
        output.append(identity.requestWireHash)
        output.nfcAppendUInt16BE(UInt16(maximumReadChunkBytes))
        output.nfcAppendUInt16BE(UInt16(maximumWriteChunkBytes))
        precondition(output.count == IrohaPeerNfcV1.infoBytes)
        return output
    }

    public static func decode(_ data: Data) throws -> Self {
        guard data.count == IrohaPeerNfcV1.infoBytes else {
            throw IrohaPeerNfcErrorV1.invalidLength
        }
        guard data.prefix(4) == IrohaPeerNfcV1.infoMagic,
              data[4] == IrohaPeerNfcV1.wireVersion else {
            throw IrohaPeerNfcErrorV1.invalidAPDU
        }
        guard let phase = IrohaPeerNfcPhaseV1(rawValue: data[5]),
              let profile = IrohaPeerPayloadProfile(rawValue: data.nfcUInt16BE(at: 6)),
              profile != .reject,
              data[9] == 0 else {
            throw IrohaPeerNfcErrorV1.invalidAPDU
        }
        let identity = try IrohaPeerNfcRequestIdentityV1(
            profile: profile,
            sessionID: data.subdata(in: 10..<26),
            requestCanonicalHash: data.subdata(in: 30..<62),
            requestWireHash: data.subdata(in: 62..<94)
        )
        return try Self(
            phase: phase,
            flags: IrohaPeerNfcFlagsV1(rawValue: data[8]),
            identity: identity,
            requestLength: Int(data.nfcUInt32BE(at: 26)),
            maximumReadChunkBytes: Int(data.nfcUInt16BE(at: 94)),
            maximumWriteChunkBytes: Int(data.nfcUInt16BE(at: 96))
        )
    }
}

/// Fixed 178-byte status carrying the active inbound and outbound IPM1 slots.
public struct IrohaPeerNfcStatusV1: Equatable, Sendable {
    public let phase: IrohaPeerNfcPhaseV1
    public let flags: IrohaPeerNfcFlagsV1
    public let identity: IrohaPeerNfcRequestIdentityV1
    public let inboundKind: IrohaPeerPayloadKind?
    public let inboundLength: Int
    public let receivedInboundBytes: Int
    public let nextMissingInboundOffset: Int
    public let inboundWireHash: Data
    public let outboundKind: IrohaPeerPayloadKind?
    public let outboundLength: Int
    public let outboundWireHash: Data
    public let maximumReadChunkBytes: Int
    public let maximumWriteChunkBytes: Int

    public init(
        phase: IrohaPeerNfcPhaseV1,
        flags: IrohaPeerNfcFlagsV1,
        identity: IrohaPeerNfcRequestIdentityV1,
        inboundKind: IrohaPeerPayloadKind?,
        inboundLength: Int,
        receivedInboundBytes: Int,
        nextMissingInboundOffset: Int,
        inboundWireHash: Data,
        outboundKind: IrohaPeerPayloadKind?,
        outboundLength: Int,
        outboundWireHash: Data,
        maximumReadChunkBytes: Int,
        maximumWriteChunkBytes: Int
    ) throws {
        guard flags.subtracting(.known).isEmpty,
              flags.contains(.idempotentWrites) else {
            throw IrohaPeerNfcErrorV1.invalidFlags
        }
        try nfcRequireChunkLimit(maximumReadChunkBytes)
        try nfcRequireChunkLimit(maximumWriteChunkBytes)
        let zeroHash = Data(repeating: 0, count: IrohaPeerNfcV1.hashBytes)
        let inboundIsAbsent = inboundKind == nil
            && inboundLength == 0
            && receivedInboundBytes == 0
            && nextMissingInboundOffset == 0
            && inboundWireHash == zeroHash
        let inboundIsPresent = inboundKind != nil
            && inboundLength > IrohaPeerWireMessageV1.headerBytes
            && inboundLength <= IrohaPeerNfcV1.maximumMessageBytes
            && (0...inboundLength).contains(receivedInboundBytes)
            && (0...inboundLength).contains(nextMissingInboundOffset)
            && (receivedInboundBytes == inboundLength)
                == (nextMissingInboundOffset == inboundLength)
            && inboundWireHash.count == IrohaPeerNfcV1.hashBytes
            && inboundWireHash != zeroHash
        let outboundIsAbsent = outboundKind == nil
            && outboundLength == 0
            && outboundWireHash == zeroHash
        let outboundIsPresent = outboundKind != nil
            && outboundLength > IrohaPeerWireMessageV1.headerBytes
            && outboundLength <= IrohaPeerNfcV1.maximumMessageBytes
            && outboundWireHash.count == IrohaPeerNfcV1.hashBytes
            && outboundWireHash != zeroHash
        let durable = flags.contains(.durableState)
        let isValid: Bool
        switch phase {
        case .requestReady:
            isValid = inboundIsAbsent && outboundIsAbsent && !durable
        case .intentReceiving:
            isValid = inboundIsPresent && outboundIsAbsent && durable
                && inboundKind == .intent
        case .ticketReady:
            isValid = inboundIsPresent && outboundIsPresent && durable
                && inboundKind == .intent
                && receivedInboundBytes == inboundLength
                && outboundKind == .ticket
        case .paymentReceiving:
            isValid = inboundIsPresent && outboundIsPresent && durable
                && inboundKind == .payment
                && outboundKind == .ticket
        case .acknowledgementReady, .complete:
            isValid = inboundIsPresent && outboundIsPresent && durable
                && inboundKind == .payment
                && receivedInboundBytes == inboundLength
                && outboundKind == .acknowledgement
        }
        guard isValid else { throw IrohaPeerNfcErrorV1.invalidLength }
        self.phase = phase
        self.flags = flags
        self.identity = identity
        self.inboundKind = inboundKind
        self.inboundLength = inboundLength
        self.receivedInboundBytes = receivedInboundBytes
        self.nextMissingInboundOffset = nextMissingInboundOffset
        self.inboundWireHash = Data(inboundWireHash)
        self.outboundKind = outboundKind
        self.outboundLength = outboundLength
        self.outboundWireHash = Data(outboundWireHash)
        self.maximumReadChunkBytes = maximumReadChunkBytes
        self.maximumWriteChunkBytes = maximumWriteChunkBytes
    }

    public func encode() -> Data {
        var output = IrohaPeerNfcV1.statusMagic
        output.append(IrohaPeerNfcV1.wireVersion)
        output.append(phase.rawValue)
        output.nfcAppendUInt16BE(identity.profile.rawValue)
        output.append(flags.rawValue)
        output.append(0)
        output.append(identity.sessionID)
        output.append(identity.requestCanonicalHash)
        output.append(identity.requestWireHash)
        output.append(inboundKind?.rawValue ?? 0)
        output.append(0)
        output.nfcAppendUInt32BE(UInt32(inboundLength))
        output.nfcAppendUInt32BE(UInt32(receivedInboundBytes))
        output.nfcAppendUInt32BE(UInt32(nextMissingInboundOffset))
        output.append(inboundWireHash)
        output.append(outboundKind?.rawValue ?? 0)
        output.append(0)
        output.nfcAppendUInt32BE(UInt32(outboundLength))
        output.append(outboundWireHash)
        output.nfcAppendUInt16BE(UInt16(maximumReadChunkBytes))
        output.nfcAppendUInt16BE(UInt16(maximumWriteChunkBytes))
        precondition(output.count == IrohaPeerNfcV1.statusBytes)
        return output
    }

    public static func decode(_ data: Data) throws -> Self {
        guard data.count == IrohaPeerNfcV1.statusBytes else {
            throw IrohaPeerNfcErrorV1.invalidLength
        }
        guard data.prefix(4) == IrohaPeerNfcV1.statusMagic,
              data[4] == IrohaPeerNfcV1.wireVersion,
              let phase = IrohaPeerNfcPhaseV1(rawValue: data[5]),
              let profile = IrohaPeerPayloadProfile(rawValue: data.nfcUInt16BE(at: 6)),
              profile != .reject,
              data[9] == 0,
              data[91] == 0,
              data[137] == 0 else {
            throw IrohaPeerNfcErrorV1.invalidAPDU
        }
        let inboundCode = data[90]
        let outboundCode = data[136]
        let inboundKind = inboundCode == 0
            ? nil
            : IrohaPeerPayloadKind(rawValue: inboundCode)
        let outboundKind = outboundCode == 0
            ? nil
            : IrohaPeerPayloadKind(rawValue: outboundCode)
        guard inboundCode == 0 || inboundKind != nil,
              outboundCode == 0 || outboundKind != nil else {
            throw IrohaPeerNfcErrorV1.invalidKind
        }
        return try Self(
            phase: phase,
            flags: IrohaPeerNfcFlagsV1(rawValue: data[8]),
            identity: IrohaPeerNfcRequestIdentityV1(
                profile: profile,
                sessionID: data.subdata(in: 10..<26),
                requestCanonicalHash: data.subdata(in: 26..<58),
                requestWireHash: data.subdata(in: 58..<90)
            ),
            inboundKind: inboundKind,
            inboundLength: Int(data.nfcUInt32BE(at: 92)),
            receivedInboundBytes: Int(data.nfcUInt32BE(at: 96)),
            nextMissingInboundOffset: Int(data.nfcUInt32BE(at: 100)),
            inboundWireHash: data.subdata(in: 104..<136),
            outboundKind: outboundKind,
            outboundLength: Int(data.nfcUInt32BE(at: 138)),
            outboundWireHash: data.subdata(in: 142..<174),
            maximumReadChunkBytes: Int(data.nfcUInt16BE(at: 174)),
            maximumWriteChunkBytes: Int(data.nfcUInt16BE(at: 176))
        )
    }
}

/// Typed command representation. All chunk offsets are UInt32 values encoded
/// in the APDU command body in network (big-endian) byte order.
public enum IrohaPeerNfcCommandV1: Equatable, Sendable {
    case selectApplication
    case getInfo
    case readRequest(sessionID: Data, requestCanonicalHash: Data, offset: UInt32, length: Int)
    case beginIntent(sessionID: Data, requestCanonicalHash: Data, intentHeader: Data)
    case writeIntent(sessionID: Data, intentWireHash: Data, offset: UInt32, bytes: Data)
    case commitIntent(sessionID: Data, requestCanonicalHash: Data, intentWireHash: Data)
    case readTicket(sessionID: Data, intentWireHash: Data, offset: UInt32, length: Int)
    case beginPayment(
        sessionID: Data,
        ticketWireHash: Data,
        paymentHeader: Data
    )
    case writePayment(sessionID: Data, paymentWireHash: Data, offset: UInt32, bytes: Data)
    case commitPayment(sessionID: Data, ticketWireHash: Data, paymentWireHash: Data)
    case readAcknowledgement(sessionID: Data, paymentWireHash: Data, offset: UInt32, length: Int)
    case confirmAcknowledgement(sessionID: Data, paymentWireHash: Data, acknowledgementWireHash: Data)
    case getStatus(sessionID: Data, requestCanonicalHash: Data)

    public var operation: IrohaPeerNfcOperationV1 {
        switch self {
        case .selectApplication: return .selectApplication
        case .getInfo: return .getInfo
        case .readRequest: return .readRequest
        case .beginIntent: return .beginIntent
        case .writeIntent: return .writeIntent
        case .commitIntent: return .commitIntent
        case .readTicket: return .readTicket
        case .beginPayment: return .beginPayment
        case .writePayment: return .writePayment
        case .commitPayment: return .commitPayment
        case .readAcknowledgement: return .readAcknowledgement
        case .confirmAcknowledgement: return .confirmAcknowledgement
        case .getStatus: return .getStatus
        }
    }
}

/// ISO/IEC 7816 status words returned by the transport-neutral card core.
public enum IrohaPeerNfcStatusWordV1: UInt16, Sendable {
    case success = 0x9000
    case storageFailure = 0x6581
    case wrongLength = 0x6700
    case securityStatusNotSatisfied = 0x6982
    case conditionsNotSatisfied = 0x6985
    case wrongData = 0x6A80
    case notFound = 0x6A82
    case instructionNotSupported = 0x6D00
    case classNotSupported = 0x6E00
}

/// APDU response with data and an ISO/IEC 7816 status word.
public struct IrohaPeerNfcAPDUResponseV1: Equatable, Sendable {
    public let data: Data
    public let statusWord: IrohaPeerNfcStatusWordV1

    public init(data: Data = Data(), statusWord: IrohaPeerNfcStatusWordV1) {
        self.data = Data(data)
        self.statusWord = statusWord
    }

    /// Card-emulation response bytes (`data || SW1 || SW2`).
    public var encoded: Data {
        data + Data([
            UInt8(truncatingIfNeeded: statusWord.rawValue >> 8),
            UInt8(truncatingIfNeeded: statusWord.rawValue),
        ])
    }
}

/// Strict raw APDU codec used by both CoreNFC adapters and test transports.
public enum IrohaPeerNfcAPDUCodecV1 {
    public static func encode(_ command: IrohaPeerNfcCommandV1) throws -> Data {
        switch command {
        case .selectApplication:
            return try encodeEnvelope(
                cla: 0x00,
                instruction: 0xA4,
                p1: 0x04,
                p2: 0x00,
                data: IrohaPeerNfcV1.applicationIdentifier,
                expectedResponseLength: 256
            )
        case .getInfo:
            return try encodeProprietary(
                instruction: .getInfo,
                expectedResponseLength: IrohaPeerNfcV1.infoBytes
            )
        case .readRequest(let sessionID, let requestHash, let offset, let length):
            try validateRead(sessionID: sessionID, hash: requestHash, length: length)
            return try encodeProprietary(
                instruction: .readRequest,
                data: sessionID + requestHash + nfcUInt32BE(offset),
                expectedResponseLength: length
            )
        case .beginIntent(let sessionID, let requestHash, let intentHeader):
            try nfcRequireSession(sessionID)
            try nfcRequireHash(requestHash)
            _ = try inspectHeader(intentHeader, kind: .intent)
            return try encodeProprietary(
                instruction: .beginIntent,
                data: sessionID + requestHash + intentHeader
            )
        case .writeIntent(let sessionID, let intentHash, let offset, let bytes):
            try validateWrite(
                sessionID: sessionID, hash: intentHash, bytes: bytes)
            return try encodeProprietary(
                instruction: .writeIntent,
                data: sessionID + intentHash + nfcUInt32BE(offset) + bytes
            )
        case .commitIntent(let sessionID, let requestHash, let intentHash):
            try validateControl(
                sessionID: sessionID, firstHash: requestHash, secondHash: intentHash)
            return try encodeProprietary(
                instruction: .commitIntent,
                data: sessionID + requestHash + intentHash
            )
        case .readTicket(let sessionID, let intentHash, let offset, let length):
            try validateRead(sessionID: sessionID, hash: intentHash, length: length)
            return try encodeProprietary(
                instruction: .readTicket,
                data: sessionID + intentHash + nfcUInt32BE(offset),
                expectedResponseLength: length
            )
        case .beginPayment(let sessionID, let ticketHash, let paymentHeader):
            try nfcRequireSession(sessionID)
            try nfcRequireHash(ticketHash)
            _ = try inspectHeader(paymentHeader, kind: .payment)
            return try encodeProprietary(
                instruction: .beginPayment,
                data: sessionID + ticketHash + paymentHeader
            )
        case .writePayment(let sessionID, let paymentHash, let offset, let bytes):
            try validateWrite(sessionID: sessionID, hash: paymentHash, bytes: bytes)
            return try encodeProprietary(
                instruction: .writePayment,
                data: sessionID + paymentHash + nfcUInt32BE(offset) + bytes
            )
        case .commitPayment(let sessionID, let ticketHash, let paymentHash):
            try validateControl(sessionID: sessionID, firstHash: ticketHash, secondHash: paymentHash)
            return try encodeProprietary(
                instruction: .commitPayment,
                data: sessionID + ticketHash + paymentHash
            )
        case .readAcknowledgement(let sessionID, let paymentHash, let offset, let length):
            try validateRead(sessionID: sessionID, hash: paymentHash, length: length)
            return try encodeProprietary(
                instruction: .readAcknowledgement,
                data: sessionID + paymentHash + nfcUInt32BE(offset),
                expectedResponseLength: length
            )
        case .confirmAcknowledgement(let sessionID, let paymentHash, let ackHash):
            try validateControl(sessionID: sessionID, firstHash: paymentHash, secondHash: ackHash)
            return try encodeProprietary(
                instruction: .confirmAcknowledgement,
                data: sessionID + paymentHash + ackHash
            )
        case .getStatus(let sessionID, let requestHash):
            try nfcRequireSession(sessionID)
            try nfcRequireHash(requestHash)
            return try encodeProprietary(
                instruction: .getStatus,
                data: sessionID + requestHash,
                expectedResponseLength: IrohaPeerNfcV1.statusBytes
            )
        }
    }

    public static func decode(_ apdu: Data) throws -> IrohaPeerNfcCommandV1 {
        let envelope = try decodeEnvelope(apdu)
        if envelope.cla == 0x00, envelope.instruction == 0xA4 {
            guard envelope.p1 == 0x04,
                  envelope.p2 == 0x00,
                  envelope.data == IrohaPeerNfcV1.applicationIdentifier,
                  envelope.expectedResponseLength == 256 else {
                throw IrohaPeerNfcErrorV1.invalidApplicationIdentifier
            }
            return .selectApplication
        }
        guard envelope.cla == IrohaPeerNfcV1.commandClass else {
            throw IrohaPeerNfcErrorV1.invalidCommandClass(envelope.cla)
        }
        guard envelope.p1 == 0, envelope.p2 == 0 else {
            throw IrohaPeerNfcErrorV1.invalidAPDU
        }
        guard let instruction = IrohaPeerNfcInstructionV1(rawValue: envelope.instruction) else {
            throw IrohaPeerNfcErrorV1.unsupportedInstruction(envelope.instruction)
        }
        switch instruction {
        case .getInfo:
            guard envelope.data.isEmpty,
                  envelope.expectedResponseLength == IrohaPeerNfcV1.infoBytes else {
                throw IrohaPeerNfcErrorV1.invalidAPDU
            }
            return .getInfo
        case .readRequest, .readTicket, .readAcknowledgement:
            guard envelope.data.count == 52,
                  let length = envelope.expectedResponseLength else {
                throw IrohaPeerNfcErrorV1.invalidAPDU
            }
            let sessionID = envelope.data.subdata(in: 0..<16)
            let hash = envelope.data.subdata(in: 16..<48)
            try validateRead(sessionID: sessionID, hash: hash, length: length)
            let offset = envelope.data.nfcUInt32BE(at: 48)
            if instruction == .readRequest {
                return .readRequest(
                    sessionID: sessionID,
                    requestCanonicalHash: hash,
                    offset: offset,
                    length: length
                )
            }
            if instruction == .readTicket {
                return .readTicket(
                    sessionID: sessionID,
                    intentWireHash: hash,
                    offset: offset,
                    length: length
                )
            }
            return .readAcknowledgement(
                sessionID: sessionID,
                paymentWireHash: hash,
                offset: offset,
                length: length
            )
        case .beginIntent:
            guard envelope.expectedResponseLength == nil,
                  envelope.data.count == 16 + 32 + IrohaPeerWireMessageV1.headerBytes else {
                throw IrohaPeerNfcErrorV1.invalidAPDU
            }
            let sessionID = envelope.data.subdata(in: 0..<16)
            let requestHash = envelope.data.subdata(in: 16..<48)
            let intentHeader = envelope.data.subdata(in: 48..<envelope.data.count)
            try nfcRequireSession(sessionID)
            try nfcRequireHash(requestHash)
            _ = try inspectHeader(intentHeader, kind: .intent)
            return .beginIntent(
                sessionID: sessionID,
                requestCanonicalHash: requestHash,
                intentHeader: intentHeader
            )
        case .beginPayment:
            guard envelope.expectedResponseLength == nil,
                  envelope.data.count == 16 + 32 + IrohaPeerWireMessageV1.headerBytes else {
                throw IrohaPeerNfcErrorV1.invalidAPDU
            }
            let sessionID = envelope.data.subdata(in: 0..<16)
            let ticketHash = envelope.data.subdata(in: 16..<48)
            let paymentHeader = envelope.data.subdata(in: 48..<envelope.data.count)
            try nfcRequireSession(sessionID)
            try nfcRequireHash(ticketHash)
            _ = try inspectHeader(paymentHeader, kind: .payment)
            return .beginPayment(
                sessionID: sessionID,
                ticketWireHash: ticketHash,
                paymentHeader: paymentHeader
            )
        case .writeIntent, .writePayment:
            guard envelope.expectedResponseLength == nil,
                  envelope.data.count > 52,
                  envelope.data.count <= 52 + IrohaPeerNfcV1.maximumChunkBytes else {
                throw IrohaPeerNfcErrorV1.invalidAPDU
            }
            let sessionID = envelope.data.subdata(in: 0..<16)
            let messageHash = envelope.data.subdata(in: 16..<48)
            let bytes = envelope.data.subdata(in: 52..<envelope.data.count)
            try validateWrite(sessionID: sessionID, hash: messageHash, bytes: bytes)
            if instruction == .writeIntent {
                return .writeIntent(
                    sessionID: sessionID,
                    intentWireHash: messageHash,
                    offset: envelope.data.nfcUInt32BE(at: 48),
                    bytes: bytes
                )
            }
            return .writePayment(
                sessionID: sessionID,
                paymentWireHash: messageHash,
                offset: envelope.data.nfcUInt32BE(at: 48),
                bytes: bytes
            )
        case .commitIntent, .commitPayment, .confirmAcknowledgement:
            guard envelope.expectedResponseLength == nil,
                  envelope.data.count == 80 else {
                throw IrohaPeerNfcErrorV1.invalidAPDU
            }
            let sessionID = envelope.data.subdata(in: 0..<16)
            let firstHash = envelope.data.subdata(in: 16..<48)
            let secondHash = envelope.data.subdata(in: 48..<80)
            try validateControl(sessionID: sessionID, firstHash: firstHash, secondHash: secondHash)
            if instruction == .commitIntent {
                return .commitIntent(
                    sessionID: sessionID,
                    requestCanonicalHash: firstHash,
                    intentWireHash: secondHash
                )
            }
            if instruction == .commitPayment {
                return .commitPayment(
                    sessionID: sessionID,
                    ticketWireHash: firstHash,
                    paymentWireHash: secondHash
                )
            }
            return .confirmAcknowledgement(
                sessionID: sessionID,
                paymentWireHash: firstHash,
                acknowledgementWireHash: secondHash
            )
        case .getStatus:
            guard envelope.data.count == 48,
                  envelope.expectedResponseLength == IrohaPeerNfcV1.statusBytes else {
                throw IrohaPeerNfcErrorV1.invalidAPDU
            }
            let sessionID = envelope.data.subdata(in: 0..<16)
            let requestHash = envelope.data.subdata(in: 16..<48)
            try nfcRequireSession(sessionID)
            try nfcRequireHash(requestHash)
            return .getStatus(sessionID: sessionID, requestCanonicalHash: requestHash)
        }
    }

    private struct Envelope {
        let cla: UInt8
        let instruction: UInt8
        let p1: UInt8
        let p2: UInt8
        let data: Data
        let expectedResponseLength: Int?
    }

    private static func encodeProprietary(
        instruction: IrohaPeerNfcInstructionV1,
        data: Data = Data(),
        expectedResponseLength: Int? = nil
    ) throws -> Data {
        try encodeEnvelope(
            cla: IrohaPeerNfcV1.commandClass,
            instruction: instruction.rawValue,
            p1: 0,
            p2: 0,
            data: data,
            expectedResponseLength: expectedResponseLength
        )
    }

    private static func encodeEnvelope(
        cla: UInt8,
        instruction: UInt8,
        p1: UInt8,
        p2: UInt8,
        data: Data,
        expectedResponseLength: Int?
    ) throws -> Data {
        guard data.count <= Int(UInt16.max),
              expectedResponseLength.map({ (1...65_536).contains($0) }) ?? true else {
            throw IrohaPeerNfcErrorV1.invalidLength
        }
        var output = Data([cla, instruction, p1, p2])
        if data.isEmpty {
            guard let expectedResponseLength else { return output }
            if expectedResponseLength <= 256 {
                output.append(expectedResponseLength == 256 ? 0 : UInt8(expectedResponseLength))
            } else {
                output.append(0)
                output.nfcAppendUInt16BE(expectedResponseLength == 65_536 ? 0 : UInt16(expectedResponseLength))
            }
            return output
        }
        let useExtended = data.count > Int(UInt8.max)
            || (expectedResponseLength.map { $0 > 256 } ?? false)
        if useExtended {
            output.append(0)
            output.nfcAppendUInt16BE(UInt16(data.count))
            output.append(data)
            if let expectedResponseLength {
                output.nfcAppendUInt16BE(expectedResponseLength == 65_536 ? 0 : UInt16(expectedResponseLength))
            }
        } else {
            output.append(UInt8(data.count))
            output.append(data)
            if let expectedResponseLength {
                output.append(expectedResponseLength == 256 ? 0 : UInt8(expectedResponseLength))
            }
        }
        return output
    }

    private static func decodeEnvelope(_ apdu: Data) throws -> Envelope {
        guard apdu.count >= 4 else { throw IrohaPeerNfcErrorV1.invalidAPDU }
        let header = (apdu[0], apdu[1], apdu[2], apdu[3])
        guard apdu.count > 4 else {
            return Envelope(
                cla: header.0,
                instruction: header.1,
                p1: header.2,
                p2: header.3,
                data: Data(),
                expectedResponseLength: nil
            )
        }
        let firstLength = Int(apdu[4])
        if firstLength != 0 {
            if apdu.count == 5 {
                return Envelope(
                    cla: header.0,
                    instruction: header.1,
                    p1: header.2,
                    p2: header.3,
                    data: Data(),
                    expectedResponseLength: firstLength
                )
            }
            if apdu.count == 5 + firstLength {
                return Envelope(
                    cla: header.0,
                    instruction: header.1,
                    p1: header.2,
                    p2: header.3,
                    data: apdu.subdata(in: 5..<apdu.count),
                    expectedResponseLength: nil
                )
            }
            if apdu.count == 6 + firstLength {
                let rawLe = Int(apdu[apdu.count - 1])
                return Envelope(
                    cla: header.0,
                    instruction: header.1,
                    p1: header.2,
                    p2: header.3,
                    data: apdu.subdata(in: 5..<(5 + firstLength)),
                    expectedResponseLength: rawLe == 0 ? 256 : rawLe
                )
            }
            throw IrohaPeerNfcErrorV1.invalidAPDU
        }
        if apdu.count == 5 {
            return Envelope(
                cla: header.0,
                instruction: header.1,
                p1: header.2,
                p2: header.3,
                data: Data(),
                expectedResponseLength: 256
            )
        }
        guard apdu.count >= 7 else { throw IrohaPeerNfcErrorV1.invalidAPDU }
        let extendedLength = Int(apdu.nfcUInt16BE(at: 5))
        if apdu.count == 7 {
            // Case 2E is canonical only when the response length cannot be
            // represented by the one-byte Case 2S form. Rejecting alternate
            // encodings avoids parser differentials between NFC stacks.
            guard extendedLength == 0 || extendedLength > 256 else {
                throw IrohaPeerNfcErrorV1.invalidAPDU
            }
            return Envelope(
                cla: header.0,
                instruction: header.1,
                p1: header.2,
                p2: header.3,
                data: Data(),
                expectedResponseLength: extendedLength == 0 ? 65_536 : extendedLength
            )
        }
        guard extendedLength > 0 else { throw IrohaPeerNfcErrorV1.invalidAPDU }
        if apdu.count == 7 + extendedLength {
            guard extendedLength > Int(UInt8.max) else {
                throw IrohaPeerNfcErrorV1.invalidAPDU
            }
            return Envelope(
                cla: header.0,
                instruction: header.1,
                p1: header.2,
                p2: header.3,
                data: apdu.subdata(in: 7..<apdu.count),
                expectedResponseLength: nil
            )
        }
        if apdu.count == 9 + extendedLength {
            let rawLe = Int(apdu.nfcUInt16BE(at: 7 + extendedLength))
            let expectedResponseLength = rawLe == 0 ? 65_536 : rawLe
            guard extendedLength > Int(UInt8.max)
                    || expectedResponseLength > 256 else {
                throw IrohaPeerNfcErrorV1.invalidAPDU
            }
            return Envelope(
                cla: header.0,
                instruction: header.1,
                p1: header.2,
                p2: header.3,
                data: apdu.subdata(in: 7..<(7 + extendedLength)),
                expectedResponseLength: expectedResponseLength
            )
        }
        throw IrohaPeerNfcErrorV1.invalidAPDU
    }

    private static func validateRead(sessionID: Data, hash: Data, length: Int) throws {
        try nfcRequireSession(sessionID)
        try nfcRequireHash(hash)
        try nfcRequireChunkLimit(length)
    }

    private static func validateControl(
        sessionID: Data,
        firstHash: Data,
        secondHash: Data
    ) throws {
        try nfcRequireSession(sessionID)
        try nfcRequireHash(firstHash)
        try nfcRequireHash(secondHash)
    }

    private static func validateWrite(sessionID: Data, hash: Data, bytes: Data) throws {
        try nfcRequireSession(sessionID)
        try nfcRequireHash(hash)
        guard !bytes.isEmpty, bytes.count <= IrohaPeerNfcV1.maximumChunkBytes else {
            throw IrohaPeerNfcErrorV1.invalidLength
        }
    }

    private static func inspectHeader(
        _ data: Data,
        kind: IrohaPeerPayloadKind
    ) throws -> IrohaPeerWireHeaderV1 {
        do {
            return try IrohaPeerWireMessageV1.inspectHeader(data, expectedKind: kind)
        } catch {
            throw IrohaPeerNfcErrorV1.invalidIPM1
        }
    }
}

/// Metadata accepted by `BEGIN_AUTHORIZATION` before message 2 is reassembled.
public struct IrohaPeerNfcIntentDescriptorV1: Equatable, Sendable {
    public let profile: IrohaPeerPayloadProfile
    public let schemaVersion: UInt16
    public let messageLength: Int
    public let canonicalHash: Data
    public let wireHash: Data
    public let header: Data

    public init(
        intentHeader: Data,
        limits: IrohaPeerNfcLimitsV1 = .default
    ) throws {
        let inspected: IrohaPeerWireHeaderV1
        do {
            inspected = try IrohaPeerWireMessageV1.inspectHeader(
                intentHeader,
                expectedKind: .intent
            )
        } catch {
            throw IrohaPeerNfcErrorV1.invalidIPM1
        }
        let messageLength = IrohaPeerWireMessageV1.headerBytes + inspected.encodedLength
        guard inspected.canonicalLength > 0,
              inspected.encodedLength > 0,
              messageLength <= limits.maximumMessageBytes else {
            throw IrohaPeerNfcErrorV1.messageTooLarge(
                actual: messageLength,
                maximum: limits.maximumMessageBytes
            )
        }
        self.profile = inspected.profile
        self.schemaVersion = inspected.schemaVersion
        self.messageLength = messageLength
        self.canonicalHash = Data(inspected.canonicalHash)
        self.wireHash = Data(inspected.wireHash)
        self.header = Data(intentHeader)
    }
}

/// Exact BEGIN_AUTHORIZATION transition proposed to application durability.
public struct IrohaPeerNfcIntentAdmissionContextV1: Equatable, Sendable {
    public let identity: IrohaPeerNfcRequestIdentityV1
    public let profilePolicy: IrohaPeerNfcProfilePolicyV1
    public let descriptor: IrohaPeerNfcIntentDescriptorV1

    public init(
        identity: IrohaPeerNfcRequestIdentityV1,
        profilePolicy: IrohaPeerNfcProfilePolicyV1,
        descriptor: IrohaPeerNfcIntentDescriptorV1
    ) throws {
        guard profilePolicy.profile == identity.profile,
              profilePolicy.accepts(descriptor.profile) else {
            throw IrohaPeerNfcErrorV1.invalidProfile
        }
        self.identity = identity
        self.profilePolicy = profilePolicy
        self.descriptor = descriptor
    }
}

/// Fixed IAA1 record that must be persisted before BEGIN_AUTHORIZATION succeeds.
/// Partial intent chunks are deliberately excluded and restart at offset zero.
public struct IrohaPeerNfcDurableIntentAdmissionV1: Equatable, Sendable {
    public let context: IrohaPeerNfcIntentAdmissionContextV1

    public var identity: IrohaPeerNfcRequestIdentityV1 { context.identity }
    public var profilePolicy: IrohaPeerNfcProfilePolicyV1 { context.profilePolicy }
    public var descriptor: IrohaPeerNfcIntentDescriptorV1 { context.descriptor }
    public var intentHeader: Data { descriptor.header }

    public init(
        context: IrohaPeerNfcIntentAdmissionContextV1,
        limits: IrohaPeerNfcLimitsV1 = .default
    ) throws {
        let validated = try IrohaPeerNfcIntentAdmissionContextV1(
            identity: context.identity,
            profilePolicy: context.profilePolicy,
            descriptor: IrohaPeerNfcIntentDescriptorV1(
                intentHeader: context.descriptor.header,
                limits: limits
            )
        )
        guard validated == context else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
        self.context = validated
    }

    public var encoded: Data {
        var output = IrohaPeerNfcV1.intentAdmissionMagic
        output.append(IrohaPeerNfcV1.wireVersion)
        output.nfcAppendUInt16BE(identity.profile.rawValue)
        output.append(0)
        output.append(identity.sessionID)
        output.append(identity.requestCanonicalHash)
        output.append(identity.requestWireHash)
        output.nfcAppendUInt16BE(descriptor.profile.rawValue)
        output.nfcAppendUInt16BE(descriptor.schemaVersion)
        output.nfcAppendUInt32BE(UInt32(descriptor.messageLength))
        output.append(descriptor.canonicalHash)
        output.append(descriptor.wireHash)
        output.append(descriptor.header)
        precondition(output.count == IrohaPeerNfcV1.intentAdmissionBytes)
        return output
    }

    public static func decode(
        _ data: Data,
        profilePolicy: IrohaPeerNfcProfilePolicyV1? = nil,
        limits: IrohaPeerNfcLimitsV1 = .default
    ) throws -> Self {
        guard data.count == IrohaPeerNfcV1.intentAdmissionBytes,
              data.prefix(4) == IrohaPeerNfcV1.intentAdmissionMagic,
              data[4] == IrohaPeerNfcV1.wireVersion,
              data[7] == 0,
              let requestProfile = IrohaPeerPayloadProfile(
                rawValue: data.nfcUInt16BE(at: 5)
              ), requestProfile != .reject,
              let intentProfile = IrohaPeerPayloadProfile(
                rawValue: data.nfcUInt16BE(at: 88)
              ), intentProfile != .reject else {
            throw IrohaPeerNfcErrorV1.invalidLength
        }
        let identity = try IrohaPeerNfcRequestIdentityV1(
            profile: requestProfile,
            sessionID: data.subdata(in: 8..<24),
            requestCanonicalHash: data.subdata(in: 24..<56),
            requestWireHash: data.subdata(in: 56..<88)
        )
        let effectivePolicy = profilePolicy ?? .init(profile: requestProfile)
        let descriptor = try IrohaPeerNfcIntentDescriptorV1(
            intentHeader: data.subdata(in: 160..<244),
            limits: limits
        )
        guard effectivePolicy.profile == requestProfile,
              descriptor.profile == intentProfile,
              descriptor.schemaVersion == data.nfcUInt16BE(at: 90),
              descriptor.messageLength == Int(data.nfcUInt32BE(at: 92)),
              descriptor.canonicalHash == data.subdata(in: 96..<128),
              descriptor.wireHash == data.subdata(in: 128..<160) else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
        return try Self(
            context: IrohaPeerNfcIntentAdmissionContextV1(
                identity: identity,
                profilePolicy: effectivePolicy,
                descriptor: descriptor
            ),
            limits: limits
        )
    }
}

public enum IrohaPeerNfcIntentAdmissionDispositionV1: Equatable, Sendable {
    case requiresDurableAdmission(IrohaPeerNfcIntentAdmissionContextV1)
    case alreadyAdmitted
}

/// Fully reassembled message 2 presented to the native ticket issuer.
public struct IrohaPeerNfcIntentCommitContextV1: Equatable, Sendable {
    public let identity: IrohaPeerNfcRequestIdentityV1
    public let profilePolicy: IrohaPeerNfcProfilePolicyV1
    public let receiveRequest: IrohaPeerWireMessageV1
    public let intent: IrohaPeerWireMessageV1

    public init(
        identity: IrohaPeerNfcRequestIdentityV1,
        profilePolicy: IrohaPeerNfcProfilePolicyV1,
        receiveRequest: IrohaPeerWireMessageV1,
        intent: IrohaPeerWireMessageV1
    ) throws {
        guard profilePolicy.profile == identity.profile,
              profilePolicy.accepts(receiveRequest.profile),
              profilePolicy.accepts(intent.profile) else {
            throw IrohaPeerNfcErrorV1.invalidProfile
        }
        guard receiveRequest.kind == .request,
              intent.kind == .intent else {
            throw IrohaPeerNfcErrorV1.invalidKind
        }
        try nfcValidateIntentBinding(
            request: receiveRequest,
            intent: intent
        )
        self.identity = identity
        self.profilePolicy = profilePolicy
        self.receiveRequest = receiveRequest
        self.intent = intent
    }
}

/// Durable receiver record for messages 2 and 3. Ticket issuance may return
/// success only after this exact record and its inbox reservation are durable.
public struct IrohaPeerNfcDurableAcceptanceTicketV1: Equatable, Sendable {
    public let identity: IrohaPeerNfcRequestIdentityV1
    public let intent: IrohaPeerWireMessageV1
    public let ticket: IrohaPeerWireMessageV1

    public init(
        context: IrohaPeerNfcIntentCommitContextV1,
        ticket: Data,
        limits: IrohaPeerNfcLimitsV1 = .default
    ) throws {
        let decoded = try nfcDecodeMessage(
            ticket,
            expectedProfile: context.profilePolicy.profile,
            expectedKind: .ticket,
            limits: limits
        )
        try nfcValidatePreTicketBindings(
            request: context.receiveRequest,
            intent: context.intent,
            ticket: decoded
        )
        identity = context.identity
        intent = context.intent
        self.ticket = decoded
    }

    public var encoded: Data {
        var output = IrohaPeerNfcV1.durableTicketMagic
        output.append(IrohaPeerNfcV1.wireVersion)
        output.nfcAppendUInt16BE(identity.profile.rawValue)
        output.append(0)
        output.append(identity.sessionID)
        output.append(identity.requestCanonicalHash)
        output.append(identity.requestWireHash)
        output.nfcAppendUInt32BE(UInt32(intent.encoded.count))
        output.nfcAppendUInt32BE(UInt32(ticket.encoded.count))
        output.append(intent.encoded)
        output.append(ticket.encoded)
        return output
    }

    public static func decode(
        _ data: Data,
        receiveRequest: Data,
        profilePolicy: IrohaPeerNfcProfilePolicyV1? = nil,
        limits: IrohaPeerNfcLimitsV1 = .default
    ) throws -> Self {
        let fixedBytes = 96
        guard data.count > fixedBytes,
              data.prefix(4) == IrohaPeerNfcV1.durableTicketMagic,
              data[4] == IrohaPeerNfcV1.wireVersion,
              data[7] == 0,
              let profile = IrohaPeerPayloadProfile(rawValue: data.nfcUInt16BE(at: 5)),
              profile != .reject else {
            throw IrohaPeerNfcErrorV1.invalidLength
        }
        let intentLength = Int(data.nfcUInt32BE(at: 88))
        let ticketLength = Int(data.nfcUInt32BE(at: 92))
        guard intentLength > IrohaPeerWireMessageV1.headerBytes,
              ticketLength > IrohaPeerWireMessageV1.headerBytes,
              intentLength <= limits.maximumMessageBytes,
              ticketLength <= limits.maximumMessageBytes,
              fixedBytes <= data.count - intentLength,
              fixedBytes + intentLength <= data.count - ticketLength,
              fixedBytes + intentLength + ticketLength == data.count else {
            throw IrohaPeerNfcErrorV1.invalidLength
        }
        let request = try nfcDecodeMessage(
            receiveRequest,
            expectedProfile: profile,
            expectedKind: .request,
            limits: limits
        )
        let identity = try IrohaPeerNfcRequestIdentityV1(
            profile: profile,
            sessionID: data.subdata(in: 8..<24),
            requestCanonicalHash: data.subdata(in: 24..<56),
            requestWireHash: data.subdata(in: 56..<88)
        )
        guard request.canonicalHash == identity.requestCanonicalHash,
              request.wireHash == identity.requestWireHash else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
        let effectivePolicy = profilePolicy ?? .init(profile: profile)
        let intentStart = fixedBytes
        let ticketStart = intentStart + intentLength
        let intent = try nfcDecodeMessage(
            data.subdata(in: intentStart..<ticketStart),
            expectedProfile: profile,
            expectedKind: .intent,
            limits: limits
        )
        let context = try IrohaPeerNfcIntentCommitContextV1(
            identity: identity,
            profilePolicy: effectivePolicy,
            receiveRequest: request,
            intent: intent
        )
        return try Self(
            context: context,
            ticket: data.subdata(in: ticketStart..<data.count),
            limits: limits
        )
    }
}

public enum IrohaPeerNfcIntentCommitDispositionV1: Equatable, Sendable {
    case requiresDurableTicket(IrohaPeerNfcIntentCommitContextV1)
    case alreadyIssued
}

/// Metadata accepted by `BEGIN_PAYMENT`. The complete IPM1 header is carried
/// in that command so profile, kind, lengths, and both IPM1 hashes are checked
/// before the first payment byte is buffered.
public struct IrohaPeerNfcPaymentDescriptorV1: Equatable, Sendable {
    public let profile: IrohaPeerPayloadProfile
    public let schemaVersion: UInt16
    public let messageLength: Int
    public let canonicalHash: Data
    public let wireHash: Data
    public let header: Data

    public init(paymentHeader: Data, limits: IrohaPeerNfcLimitsV1 = .default) throws {
        let inspected: IrohaPeerWireHeaderV1
        do {
            inspected = try IrohaPeerWireMessageV1.inspectHeader(
                paymentHeader,
                expectedKind: .payment
            )
        } catch {
            throw IrohaPeerNfcErrorV1.invalidIPM1
        }
        let messageLength = IrohaPeerWireMessageV1.headerBytes + inspected.encodedLength
        guard inspected.canonicalLength > 0,
              inspected.encodedLength > 0,
              messageLength <= limits.maximumMessageBytes else {
            throw IrohaPeerNfcErrorV1.messageTooLarge(
                actual: messageLength,
                maximum: limits.maximumMessageBytes
            )
        }
        self.profile = inspected.profile
        self.schemaVersion = inspected.schemaVersion
        self.messageLength = messageLength
        self.canonicalHash = Data(inspected.canonicalHash)
        self.wireHash = Data(inspected.wireHash)
        self.header = Data(paymentHeader)
    }
}

/// Validated BEGIN_PAYMENT metadata that must be durable before any payment
/// byte is accepted.
public struct IrohaPeerNfcPaymentAdmissionContextV1: Equatable, Sendable {
    public let identity: IrohaPeerNfcRequestIdentityV1
    public let profilePolicy: IrohaPeerNfcProfilePolicyV1
    public let intentWireHash: Data
    public let ticketWireHash: Data
    public let descriptor: IrohaPeerNfcPaymentDescriptorV1

    public init(
        identity: IrohaPeerNfcRequestIdentityV1,
        profilePolicy: IrohaPeerNfcProfilePolicyV1,
        intentWireHash: Data,
        ticketWireHash: Data,
        descriptor: IrohaPeerNfcPaymentDescriptorV1
    ) throws {
        guard profilePolicy.profile == identity.profile,
              profilePolicy.accepts(descriptor.profile) else {
            throw IrohaPeerNfcErrorV1.invalidProfile
        }
        try nfcRequireHash(intentWireHash)
        try nfcRequireHash(ticketWireHash)
        self.identity = identity
        self.profilePolicy = profilePolicy
        self.intentWireHash = Data(intentWireHash)
        self.ticketWireHash = Data(ticketWireHash)
        self.descriptor = descriptor
    }

    public init(
        identity: IrohaPeerNfcRequestIdentityV1,
        profilePolicy: IrohaPeerNfcProfilePolicyV1,
        durableAcceptanceTicket: IrohaPeerNfcDurableAcceptanceTicketV1,
        paymentHeader: Data,
        limits: IrohaPeerNfcLimitsV1 = .default
    ) throws {
        try self.init(
            identity: identity,
            profilePolicy: profilePolicy,
            intentWireHash: durableAcceptanceTicket.intent.wireHash,
            ticketWireHash: durableAcceptanceTicket.ticket.wireHash,
            descriptor: IrohaPeerNfcPaymentDescriptorV1(
                paymentHeader: paymentHeader,
                limits: limits
            )
        )
    }

    public var paymentHeader: Data { descriptor.header }
}

/// Fixed 308-byte ITP1 ticket-bound payment admission record. The
/// callback input context is intentionally separate from this persistable
/// output so BEGIN_PAYMENT cannot succeed merely because validation ran.
public struct IrohaPeerNfcDurablePaymentAdmissionV1: Equatable, Sendable {
    public let context: IrohaPeerNfcPaymentAdmissionContextV1

    public var identity: IrohaPeerNfcRequestIdentityV1 { context.identity }
    public var profilePolicy: IrohaPeerNfcProfilePolicyV1 { context.profilePolicy }
    public var descriptor: IrohaPeerNfcPaymentDescriptorV1 { context.descriptor }
    public var paymentHeader: Data { context.paymentHeader }

    public init(
        context: IrohaPeerNfcPaymentAdmissionContextV1,
        limits: IrohaPeerNfcLimitsV1 = .default
    ) throws {
        let validated = try IrohaPeerNfcPaymentAdmissionContextV1(
            identity: context.identity,
            profilePolicy: context.profilePolicy,
            intentWireHash: context.intentWireHash,
            ticketWireHash: context.ticketWireHash,
            descriptor: IrohaPeerNfcPaymentDescriptorV1(
                paymentHeader: context.paymentHeader,
                limits: limits
            )
        )
        guard validated == context else { throw IrohaPeerNfcErrorV1.continuityMismatch }
        self.context = validated
    }

    public var encoded: Data {
        var output = IrohaPeerNfcV1.ticketedPaymentAdmissionMagic
        output.append(IrohaPeerNfcV1.wireVersion)
        output.nfcAppendUInt16BE(identity.profile.rawValue)
        output.append(0)
        output.append(identity.sessionID)
        output.append(identity.requestCanonicalHash)
        output.append(identity.requestWireHash)
        output.append(context.intentWireHash)
        output.append(context.ticketWireHash)
        output.nfcAppendUInt16BE(descriptor.profile.rawValue)
        output.nfcAppendUInt16BE(descriptor.schemaVersion)
        output.nfcAppendUInt32BE(UInt32(descriptor.messageLength))
        output.append(descriptor.canonicalHash)
        output.append(descriptor.wireHash)
        output.append(descriptor.header)
        precondition(output.count == IrohaPeerNfcV1.ticketedPaymentAdmissionBytes)
        return output
    }

    public static func decode(
        _ data: Data,
        durableAcceptanceTicket: IrohaPeerNfcDurableAcceptanceTicketV1,
        profilePolicy: IrohaPeerNfcProfilePolicyV1? = nil,
        limits: IrohaPeerNfcLimitsV1 = .default
    ) throws -> Self {
        guard data.count == IrohaPeerNfcV1.ticketedPaymentAdmissionBytes,
              data.prefix(4) == IrohaPeerNfcV1.ticketedPaymentAdmissionMagic,
              data[4] == IrohaPeerNfcV1.wireVersion,
              data[7] == 0,
              let requestProfile = IrohaPeerPayloadProfile(
                rawValue: data.nfcUInt16BE(at: 5)
              ), requestProfile != .reject,
              let paymentProfile = IrohaPeerPayloadProfile(
                rawValue: data.nfcUInt16BE(at: 152)
              ), paymentProfile != .reject else {
            throw IrohaPeerNfcErrorV1.invalidLength
        }
        let effectivePolicy = profilePolicy ?? .init(profile: requestProfile)
        let identity = try IrohaPeerNfcRequestIdentityV1(
            profile: requestProfile,
            sessionID: data.subdata(in: 8..<24),
            requestCanonicalHash: data.subdata(in: 24..<56),
            requestWireHash: data.subdata(in: 56..<88)
        )
        let context = try IrohaPeerNfcPaymentAdmissionContextV1(
            identity: identity,
            profilePolicy: effectivePolicy,
            durableAcceptanceTicket: durableAcceptanceTicket,
            paymentHeader: data.subdata(in: 224..<308),
            limits: limits
        )
        guard effectivePolicy.profile == requestProfile,
              durableAcceptanceTicket.identity == identity,
              context.intentWireHash == data.subdata(in: 88..<120),
              context.ticketWireHash == data.subdata(in: 120..<152),
              context.descriptor.profile == paymentProfile,
              context.descriptor.schemaVersion == data.nfcUInt16BE(at: 154),
              context.descriptor.messageLength == Int(data.nfcUInt32BE(at: 156)),
              context.descriptor.canonicalHash == data.subdata(in: 160..<192),
              context.descriptor.wireHash == data.subdata(in: 192..<224) else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
        return try Self(context: context, limits: limits)
    }
}

/// A fully validated payment presented to application persistence at COMMIT.
public struct IrohaPeerNfcCommitContextV1: Equatable, Sendable {
    public let identity: IrohaPeerNfcRequestIdentityV1
    public let profilePolicy: IrohaPeerNfcProfilePolicyV1
    public let receiveRequest: IrohaPeerWireMessageV1
    public let intent: IrohaPeerWireMessageV1
    public let ticket: IrohaPeerWireMessageV1
    public let payment: IrohaPeerWireMessageV1

    public init(
        identity: IrohaPeerNfcRequestIdentityV1,
        profilePolicy: IrohaPeerNfcProfilePolicyV1,
        receiveRequest: IrohaPeerWireMessageV1,
        durableAcceptanceTicket: IrohaPeerNfcDurableAcceptanceTicketV1,
        payment: IrohaPeerWireMessageV1
    ) throws {
        guard profilePolicy.profile == identity.profile,
              profilePolicy.accepts(payment.profile) else {
            throw IrohaPeerNfcErrorV1.invalidProfile
        }
        guard receiveRequest.kind == .request,
              durableAcceptanceTicket.intent.kind == .intent,
              durableAcceptanceTicket.ticket.kind == .ticket,
              payment.kind == .payment else {
            throw IrohaPeerNfcErrorV1.invalidKind
        }
        guard durableAcceptanceTicket.identity == identity else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
        try nfcValidateCommittedPaymentBindings(
            request: receiveRequest,
            intent: durableAcceptanceTicket.intent,
            ticket: durableAcceptanceTicket.ticket,
            payment: payment
        )
        self.identity = identity
        self.profilePolicy = profilePolicy
        self.receiveRequest = receiveRequest
        intent = durableAcceptanceTicket.intent
        ticket = durableAcceptanceTicket.ticket
        self.payment = payment
    }
}

/// Opaque, persistable receiver record proving that payment ingest and its
/// exact IPM1 acknowledgement crossed the application's durability boundary.
///
/// A COMMIT handler must store `encoded` durably before returning this value.
/// Reloading it reconstructs `ACKNOWLEDGEMENT_READY` after RF loss or relaunch.
public struct IrohaPeerNfcDurableAcknowledgementV1: Equatable, Sendable {
    public let identity: IrohaPeerNfcRequestIdentityV1
    public let intentWireHash: Data
    public let ticketWireHash: Data
    public let payment: IrohaPeerWireMessageV1
    public let acknowledgement: IrohaPeerWireMessageV1

    public var paymentProfile: IrohaPeerPayloadProfile { payment.profile }
    public var paymentLength: Int { payment.encoded.count }
    public var paymentWireHash: Data { payment.wireHash }

    public init(
        context: IrohaPeerNfcCommitContextV1,
        acknowledgement: Data,
        limits: IrohaPeerNfcLimitsV1 = .default
    ) throws {
        guard context.payment.encoded.count <= limits.maximumMessageBytes else {
            throw IrohaPeerNfcErrorV1.messageTooLarge(
                actual: context.payment.encoded.count,
                maximum: limits.maximumMessageBytes
            )
        }
        let decoded = try nfcDecodeMessage(
            acknowledgement,
            expectedProfile: nil,
            expectedKind: .acknowledgement,
            limits: limits
        )
        guard context.profilePolicy.accepts(decoded.profile) else {
            throw IrohaPeerNfcErrorV1.invalidProfile
        }
        try nfcValidateCompleteBindings(
            request: context.receiveRequest,
            intent: context.intent,
            ticket: context.ticket,
            payment: context.payment,
            acknowledgement: decoded
        )
        self.identity = context.identity
        intentWireHash = context.intent.wireHash
        ticketWireHash = context.ticket.wireHash
        payment = context.payment
        self.acknowledgement = decoded
    }

    public var encoded: Data {
        var output = IrohaPeerNfcV1.durableAckMagic
        output.append(IrohaPeerNfcV1.wireVersion)
        output.nfcAppendUInt16BE(identity.profile.rawValue)
        output.append(0)
        output.append(identity.sessionID)
        output.append(identity.requestCanonicalHash)
        output.append(identity.requestWireHash)
        output.append(intentWireHash)
        output.append(ticketWireHash)
        output.nfcAppendUInt32BE(UInt32(paymentLength))
        output.nfcAppendUInt32BE(UInt32(acknowledgement.encoded.count))
        output.append(payment.encoded)
        output.append(acknowledgement.encoded)
        return output
    }

    public static func decode(
        _ data: Data,
        receiveRequest: Data,
        durableAcceptanceTicket: IrohaPeerNfcDurableAcceptanceTicketV1,
        profilePolicy: IrohaPeerNfcProfilePolicyV1? = nil,
        limits: IrohaPeerNfcLimitsV1 = .default
    ) throws -> Self {
        let fixedBytes = 160
        guard data.count > fixedBytes,
              data.prefix(4) == IrohaPeerNfcV1.durableAckMagic,
              data[4] == IrohaPeerNfcV1.wireVersion,
              data[7] == 0,
              let profile = IrohaPeerPayloadProfile(rawValue: data.nfcUInt16BE(at: 5)),
              profile != .reject else {
            throw IrohaPeerNfcErrorV1.invalidLength
        }
        let paymentLength = Int(data.nfcUInt32BE(at: 152))
        let acknowledgementLength = Int(data.nfcUInt32BE(at: 156))
        guard paymentLength > IrohaPeerWireMessageV1.headerBytes,
              paymentLength <= limits.maximumMessageBytes,
              acknowledgementLength > IrohaPeerWireMessageV1.headerBytes,
              acknowledgementLength <= limits.maximumMessageBytes,
              fixedBytes <= data.count - paymentLength,
              fixedBytes + paymentLength <= data.count - acknowledgementLength,
              fixedBytes + paymentLength + acknowledgementLength == data.count else {
            throw IrohaPeerNfcErrorV1.invalidLength
        }
        let identity = try IrohaPeerNfcRequestIdentityV1(
            profile: profile,
            sessionID: data.subdata(in: 8..<24),
            requestCanonicalHash: data.subdata(in: 24..<56),
            requestWireHash: data.subdata(in: 56..<88)
        )
        let paymentEnd = fixedBytes + paymentLength
        let payment = try nfcDecodeMessage(
            data.subdata(in: fixedBytes..<paymentEnd),
            expectedProfile: profile,
            expectedKind: .payment,
            limits: limits
        )
        let acknowledgement = try nfcDecodeMessage(
            data.subdata(in: paymentEnd..<data.count),
            expectedProfile: profile,
            expectedKind: .acknowledgement,
            limits: limits
        )
        let effectivePolicy = profilePolicy ?? .init(profile: profile)
        guard effectivePolicy.profile == profile,
              durableAcceptanceTicket.identity == identity,
              durableAcceptanceTicket.intent.wireHash
                == data.subdata(in: 88..<120),
              durableAcceptanceTicket.ticket.wireHash == data.subdata(in: 120..<152),
              effectivePolicy.accepts(payment.profile),
              effectivePolicy.accepts(acknowledgement.profile) else {
            throw IrohaPeerNfcErrorV1.invalidProfile
        }
        let request = try nfcDecodeMessage(
            receiveRequest,
            expectedProfile: profile,
            expectedKind: .request,
            limits: limits
        )
        guard request.canonicalHash == identity.requestCanonicalHash,
              request.wireHash == identity.requestWireHash else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
        let context = try IrohaPeerNfcCommitContextV1(
            identity: identity,
            profilePolicy: effectivePolicy,
            receiveRequest: request,
            durableAcceptanceTicket: durableAcceptanceTicket,
            payment: payment
        )
        return try Self(
            context: context,
            acknowledgement: acknowledgement.encoded,
            limits: limits
        )
    }
}

public enum IrohaPeerNfcCommitDispositionV1: Equatable, Sendable {
    /// The application must atomically ingest the payment and persist an ACK,
    /// then install the returned durable record before replying with `9000`.
    case requiresDurableCommit(IrohaPeerNfcCommitContextV1)
    /// The exact COMMIT was already durably accepted; replay is idempotent.
    case alreadyCommitted
}

public enum IrohaPeerNfcPaymentAdmissionDispositionV1: Equatable, Sendable {
    case requiresDurableAdmission(IrohaPeerNfcPaymentAdmissionContextV1)
    case alreadyAdmitted
}

private struct IrohaPeerNfcSparseInboundBufferV1: Sendable {
    private var bytes: Data
    private var received: [Bool]

    init(length: Int) {
        precondition(length > 0)
        bytes = Data(repeating: 0, count: length)
        received = Array(repeating: false, count: length)
    }

    var length: Int { bytes.count }

    var contiguousPrefixLength: Int {
        received.firstIndex(of: false) ?? received.count
    }

    var receivedByteCount: Int { received.lazy.filter { $0 }.count }

    var isComplete: Bool { !received.contains(false) }

    var completeBytes: Data? { isComplete ? bytes : nil }

    mutating func install(offset: UInt32, chunk: Data, maximumChunkBytes: Int) throws {
        guard !chunk.isEmpty, chunk.count <= maximumChunkBytes,
              chunk.count <= bytes.count,
              let start = Int(exactly: offset),
              start >= 0, start <= bytes.count - chunk.count else {
            throw IrohaPeerNfcErrorV1.invalidOffset
        }
        for (relativeIndex, value) in chunk.enumerated() {
            let target = start + relativeIndex
            if received[target] {
                guard bytes[target] == value else {
                    throw IrohaPeerNfcErrorV1.conflictingReplay
                }
            } else {
                bytes[target] = value
                received[target] = true
            }
        }
    }
}

/// Transport-neutral receiver/card state for one NFC transfer session.
public struct IrohaPeerNfcReceiverSessionV1: Sendable {
    public typealias DurableIntentAdmissionHandler =
        (IrohaPeerNfcIntentAdmissionContextV1) throws ->
            IrohaPeerNfcDurableIntentAdmissionV1
    public typealias DurableTicketHandler =
        (IrohaPeerNfcIntentCommitContextV1) throws ->
            IrohaPeerNfcDurableAcceptanceTicketV1
    public typealias DurableAdmissionHandler =
        (IrohaPeerNfcPaymentAdmissionContextV1) throws ->
            IrohaPeerNfcDurablePaymentAdmissionV1
    public typealias DurableCommitHandler =
        (IrohaPeerNfcCommitContextV1) throws -> IrohaPeerNfcDurableAcknowledgementV1

    private struct PendingIntent: Sendable {
        let descriptor: IrohaPeerNfcIntentDescriptorV1
        var buffer: IrohaPeerNfcSparseInboundBufferV1
    }

    private struct PendingPayment: Sendable {
        let descriptor: IrohaPeerNfcPaymentDescriptorV1
        var buffer: IrohaPeerNfcSparseInboundBufferV1
    }

    public let identity: IrohaPeerNfcRequestIdentityV1
    public let profilePolicy: IrohaPeerNfcProfilePolicyV1
    public let receiveRequest: IrohaPeerWireMessageV1
    public let limits: IrohaPeerNfcLimitsV1

    private var pendingIntent: PendingIntent?
    private var durableAcceptanceTicket: IrohaPeerNfcDurableAcceptanceTicketV1?
    private var pendingPayment: PendingPayment?
    private var durableAcknowledgement: IrohaPeerNfcDurableAcknowledgementV1?
    private var acknowledgementConfirmed = false

    public init(
        sessionID: Data,
        receiveRequest: Data,
        restoredIntentAdmission: IrohaPeerNfcDurableIntentAdmissionV1? = nil,
        restoredAcceptanceTicket: IrohaPeerNfcDurableAcceptanceTicketV1? = nil,
        durableAcknowledgement: IrohaPeerNfcDurableAcknowledgementV1? = nil,
        restoredPaymentAdmission: IrohaPeerNfcDurablePaymentAdmissionV1? = nil,
        profilePolicy: IrohaPeerNfcProfilePolicyV1? = nil,
        limits: IrohaPeerNfcLimitsV1 = .default
    ) throws {
        try nfcRequireSession(sessionID)
        let request = try nfcDecodeMessage(
            receiveRequest,
            expectedProfile: nil,
            expectedKind: .request,
            limits: limits
        )
        let identity = try IrohaPeerNfcRequestIdentityV1(
            profile: request.profile,
            sessionID: sessionID,
            requestCanonicalHash: request.canonicalHash,
            requestWireHash: request.wireHash
        )
        let effectivePolicy = profilePolicy ?? .init(profile: request.profile)
        guard effectivePolicy.profile == request.profile else {
            throw IrohaPeerNfcErrorV1.invalidProfile
        }
        let initialPendingIntent: PendingIntent?
        if let restoredIntentAdmission {
            guard restoredAcceptanceTicket == nil,
                  restoredPaymentAdmission == nil,
                  durableAcknowledgement == nil else {
                throw IrohaPeerNfcErrorV1.continuityMismatch
            }
            let validated = try IrohaPeerNfcDurableIntentAdmissionV1.decode(
                restoredIntentAdmission.encoded,
                profilePolicy: effectivePolicy,
                limits: limits
            )
            guard validated == restoredIntentAdmission,
                  validated.identity == identity else {
                throw IrohaPeerNfcErrorV1.continuityMismatch
            }
            initialPendingIntent = PendingIntent(
                descriptor: validated.descriptor,
                buffer: IrohaPeerNfcSparseInboundBufferV1(
                    length: validated.descriptor.messageLength
                )
            )
        } else {
            initialPendingIntent = nil
        }
        let validatedAcceptanceTicket: IrohaPeerNfcDurableAcceptanceTicketV1?
        if let restoredAcceptanceTicket {
            let validated = try IrohaPeerNfcDurableAcceptanceTicketV1.decode(
                restoredAcceptanceTicket.encoded,
                receiveRequest: request.encoded,
                profilePolicy: effectivePolicy,
                limits: limits
            )
            guard validated == restoredAcceptanceTicket,
                  validated.identity == identity else {
                throw IrohaPeerNfcErrorV1.continuityMismatch
            }
            validatedAcceptanceTicket = validated
        } else {
            validatedAcceptanceTicket = nil
        }
        if let durableAcknowledgement {
            guard let validatedAcceptanceTicket else {
                throw IrohaPeerNfcErrorV1.continuityMismatch
            }
            let validatedAcknowledgement = try IrohaPeerNfcDurableAcknowledgementV1.decode(
                durableAcknowledgement.encoded,
                receiveRequest: request.encoded,
                durableAcceptanceTicket: validatedAcceptanceTicket,
                profilePolicy: effectivePolicy,
                limits: limits
            )
            guard validatedAcknowledgement == durableAcknowledgement,
                  durableAcknowledgement.identity == identity,
                  effectivePolicy.accepts(durableAcknowledgement.paymentProfile),
                  effectivePolicy.accepts(
                    durableAcknowledgement.acknowledgement.profile
                  ) else {
                throw IrohaPeerNfcErrorV1.continuityMismatch
            }
        }
        var initialPendingPayment: PendingPayment?
        if let restoredPaymentAdmission {
            guard let validatedAcceptanceTicket else {
                throw IrohaPeerNfcErrorV1.continuityMismatch
            }
            let validated = try IrohaPeerNfcDurablePaymentAdmissionV1.decode(
                restoredPaymentAdmission.encoded,
                durableAcceptanceTicket: validatedAcceptanceTicket,
                profilePolicy: effectivePolicy,
                limits: limits
            )
            guard validated == restoredPaymentAdmission,
                  validated.identity == identity,
                  validated.profilePolicy == effectivePolicy else {
                throw IrohaPeerNfcErrorV1.continuityMismatch
            }
            if let durableAcknowledgement {
                guard validated.descriptor.profile == durableAcknowledgement.paymentProfile,
                      validated.descriptor.messageLength == durableAcknowledgement.paymentLength,
                      validated.descriptor.wireHash == durableAcknowledgement.paymentWireHash else {
                    throw IrohaPeerNfcErrorV1.continuityMismatch
                }
                initialPendingPayment = nil
            } else {
                initialPendingPayment = PendingPayment(
                    descriptor: validated.descriptor,
                    buffer: IrohaPeerNfcSparseInboundBufferV1(
                        length: validated.descriptor.messageLength)
                )
            }
        } else {
            initialPendingPayment = nil
        }
        self.identity = identity
        self.receiveRequest = request
        self.profilePolicy = effectivePolicy
        self.limits = limits
        self.pendingIntent = initialPendingIntent
        self.durableAcceptanceTicket = validatedAcceptanceTicket
        self.pendingPayment = initialPendingPayment
        self.durableAcknowledgement = durableAcknowledgement
    }

    public var phase: IrohaPeerNfcPhaseV1 {
        if acknowledgementConfirmed { return .complete }
        if durableAcknowledgement != nil { return .acknowledgementReady }
        if pendingPayment != nil { return .paymentReceiving }
        if durableAcceptanceTicket != nil { return .ticketReady }
        if pendingIntent != nil { return .intentReceiving }
        return .requestReady
    }

    public func info() throws -> IrohaPeerNfcInfoV1 {
        try IrohaPeerNfcInfoV1(
            phase: phase,
            flags: currentFlags,
            identity: identity,
            requestLength: receiveRequest.encoded.count,
            maximumReadChunkBytes: limits.maximumReadChunkBytes,
            maximumWriteChunkBytes: limits.maximumWriteChunkBytes
        )
    }

    public func status() throws -> IrohaPeerNfcStatusV1 {
        let zeroHash = Data(repeating: 0, count: IrohaPeerNfcV1.hashBytes)
        let inboundKind: IrohaPeerPayloadKind?
        let inboundLength: Int
        let receivedInboundBytes: Int
        let nextMissingInboundOffset: Int
        let inboundWireHash: Data
        let outboundKind: IrohaPeerPayloadKind?
        let outboundLength: Int
        let outboundWireHash: Data

        if let durableAcknowledgement {
            inboundKind = .payment
            inboundLength = durableAcknowledgement.paymentLength
            receivedInboundBytes = durableAcknowledgement.paymentLength
            nextMissingInboundOffset = durableAcknowledgement.paymentLength
            inboundWireHash = durableAcknowledgement.paymentWireHash
            outboundKind = .acknowledgement
            outboundLength = durableAcknowledgement.acknowledgement.encoded.count
            outboundWireHash = durableAcknowledgement.acknowledgement.wireHash
        } else if let pendingPayment {
            inboundKind = .payment
            inboundLength = pendingPayment.descriptor.messageLength
            receivedInboundBytes = pendingPayment.buffer.receivedByteCount
            nextMissingInboundOffset = pendingPayment.buffer.contiguousPrefixLength
            inboundWireHash = pendingPayment.descriptor.wireHash
            outboundKind = .ticket
            outboundLength = durableAcceptanceTicket!.ticket.encoded.count
            outboundWireHash = durableAcceptanceTicket!.ticket.wireHash
        } else if let durableAcceptanceTicket {
            inboundKind = .intent
            inboundLength = durableAcceptanceTicket.intent.encoded.count
            receivedInboundBytes = durableAcceptanceTicket.intent.encoded.count
            nextMissingInboundOffset = durableAcceptanceTicket.intent.encoded.count
            inboundWireHash = durableAcceptanceTicket.intent.wireHash
            outboundKind = .ticket
            outboundLength = durableAcceptanceTicket.ticket.encoded.count
            outboundWireHash = durableAcceptanceTicket.ticket.wireHash
        } else if let pendingIntent {
            inboundKind = .intent
            inboundLength = pendingIntent.descriptor.messageLength
            receivedInboundBytes = pendingIntent.buffer.receivedByteCount
            nextMissingInboundOffset = pendingIntent.buffer.contiguousPrefixLength
            inboundWireHash = pendingIntent.descriptor.wireHash
            outboundKind = nil
            outboundLength = 0
            outboundWireHash = zeroHash
        } else {
            inboundKind = nil
            inboundLength = 0
            receivedInboundBytes = 0
            nextMissingInboundOffset = 0
            inboundWireHash = zeroHash
            outboundKind = nil
            outboundLength = 0
            outboundWireHash = zeroHash
        }
        return try IrohaPeerNfcStatusV1(
            phase: phase,
            flags: currentFlags,
            identity: identity,
            inboundKind: inboundKind,
            inboundLength: inboundLength,
            receivedInboundBytes: receivedInboundBytes,
            nextMissingInboundOffset: nextMissingInboundOffset,
            inboundWireHash: inboundWireHash,
            outboundKind: outboundKind,
            outboundLength: outboundLength,
            outboundWireHash: outboundWireHash,
            maximumReadChunkBytes: limits.maximumReadChunkBytes,
            maximumWriteChunkBytes: limits.maximumWriteChunkBytes
        )
    }

    /// Handles non-durable commands. Intent commit, payment admission,
    /// and payment commit cross explicit application durability boundaries.
    public mutating func handle(_ command: IrohaPeerNfcCommandV1) throws -> Data {
        switch command {
        case .selectApplication:
            return Data()
        case .getInfo:
            return try info().encode()
        case .readRequest(let sessionID, let requestHash, let offset, let length):
            try requireRequestContinuity(sessionID: sessionID, requestHash: requestHash)
            return try readSlice(
                receiveRequest.encoded,
                offset: offset,
                requestedLength: length
            )
        case .beginIntent:
            throw IrohaPeerNfcErrorV1.durableIntentAdmissionRequired
        case .writeIntent(let sessionID, let intentHash, let offset, let bytes):
            try writeIntent(
                sessionID: sessionID,
                intentHash: intentHash,
                offset: offset,
                bytes: bytes
            )
            return Data()
        case .commitIntent:
            throw IrohaPeerNfcErrorV1.durableTicketRequired
        case .readTicket(let sessionID, let intentHash, let offset, let length):
            guard let durableAcceptanceTicket else {
                throw IrohaPeerNfcErrorV1.stateMismatch
            }
            try requireSession(sessionID)
            guard intentHash == durableAcceptanceTicket.intent.wireHash else {
                throw IrohaPeerNfcErrorV1.continuityMismatch
            }
            return try readSlice(
                durableAcceptanceTicket.ticket.encoded,
                offset: offset,
                requestedLength: length
            )
        case .beginPayment:
            throw IrohaPeerNfcErrorV1.durableAdmissionRequired
        case .writePayment(let sessionID, let paymentHash, let offset, let bytes):
            try writePayment(
                sessionID: sessionID,
                paymentHash: paymentHash,
                offset: offset,
                bytes: bytes
            )
            return Data()
        case .commitPayment:
            throw IrohaPeerNfcErrorV1.durableCommitRequired
        case .readAcknowledgement(let sessionID, let paymentHash, let offset, let length):
            guard let durableAcknowledgement else {
                throw IrohaPeerNfcErrorV1.stateMismatch
            }
            try requireSession(sessionID)
            guard paymentHash == durableAcknowledgement.paymentWireHash else {
                throw IrohaPeerNfcErrorV1.continuityMismatch
            }
            return try readSlice(
                durableAcknowledgement.acknowledgement.encoded,
                offset: offset,
                requestedLength: length
            )
        case .confirmAcknowledgement(let sessionID, let paymentHash, let ackHash):
            try confirmAcknowledgement(
                sessionID: sessionID,
                paymentHash: paymentHash,
                acknowledgementHash: ackHash
            )
            return Data()
        case .getStatus(let sessionID, let requestHash):
            try requireRequestContinuity(sessionID: sessionID, requestHash: requestHash)
            return try status().encode()
        }
    }

    public func prepareIntentAdmission(
        _ command: IrohaPeerNfcCommandV1
    ) throws -> IrohaPeerNfcIntentAdmissionDispositionV1 {
        guard case .beginIntent(
            let sessionID, let requestHash, let intentHeader
        ) = command else {
            throw IrohaPeerNfcErrorV1.stateMismatch
        }
        try requireRequestContinuity(sessionID: sessionID, requestHash: requestHash)
        let descriptor = try IrohaPeerNfcIntentDescriptorV1(
            intentHeader: intentHeader,
            limits: limits
        )
        guard profilePolicy.accepts(descriptor.profile) else {
            throw IrohaPeerNfcErrorV1.invalidProfile
        }
        if let durableAcceptanceTicket {
            guard durableAcceptanceTicket.intent.header.bytes == descriptor.header,
                  durableAcceptanceTicket.intent.wireHash == descriptor.wireHash else {
                throw IrohaPeerNfcErrorV1.conflictingReplay
            }
            return .alreadyAdmitted
        }
        guard pendingPayment == nil, durableAcknowledgement == nil else {
            throw IrohaPeerNfcErrorV1.stateMismatch
        }
        if let pendingIntent {
            guard pendingIntent.descriptor == descriptor else {
                throw IrohaPeerNfcErrorV1.conflictingReplay
            }
            return .alreadyAdmitted
        }
        return .requiresDurableAdmission(
            try IrohaPeerNfcIntentAdmissionContextV1(
                identity: identity,
                profilePolicy: profilePolicy,
                descriptor: descriptor
            )
        )
    }

    /// Installs only the byte-exact IAA1 record returned for the current BEGIN command.
    public mutating func installIntentAdmission(
        _ record: IrohaPeerNfcDurableIntentAdmissionV1
    ) throws {
        let validated = try IrohaPeerNfcDurableIntentAdmissionV1.decode(
            record.encoded,
            profilePolicy: profilePolicy,
            limits: limits
        )
        guard validated == record, validated.identity == identity else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
        if let pendingIntent {
            guard pendingIntent.descriptor == validated.descriptor else {
                throw IrohaPeerNfcErrorV1.conflictingReplay
            }
            return
        }
        guard durableAcceptanceTicket == nil,
              pendingPayment == nil,
              durableAcknowledgement == nil else {
            throw IrohaPeerNfcErrorV1.stateMismatch
        }
        pendingIntent = PendingIntent(
            descriptor: validated.descriptor,
            buffer: IrohaPeerNfcSparseInboundBufferV1(
                length: validated.descriptor.messageLength
            )
        )
    }

    private mutating func writeIntent(
        sessionID: Data,
        intentHash: Data,
        offset: UInt32,
        bytes: Data
    ) throws {
        try requireSession(sessionID)
        if let durableAcceptanceTicket {
            let message = durableAcceptanceTicket.intent.encoded
            guard intentHash == durableAcceptanceTicket.intent.wireHash,
                  bytes.count <= message.count,
                  let start = Int(exactly: offset),
                  start >= 0, start <= message.count - bytes.count,
                  message.subdata(in: start..<(start + bytes.count)) == bytes else {
                throw IrohaPeerNfcErrorV1.conflictingReplay
            }
            return
        }
        guard var pendingIntent else {
            throw IrohaPeerNfcErrorV1.stateMismatch
        }
        guard intentHash == pendingIntent.descriptor.wireHash else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
        try pendingIntent.buffer.install(
            offset: offset,
            chunk: bytes,
            maximumChunkBytes: limits.maximumWriteChunkBytes
        )
        self.pendingIntent = pendingIntent
    }

    public func prepareIntentCommit(
        _ command: IrohaPeerNfcCommandV1
    ) throws -> IrohaPeerNfcIntentCommitDispositionV1 {
        guard case .commitIntent(
            let sessionID, let requestHash, let intentHash
        ) = command else {
            throw IrohaPeerNfcErrorV1.stateMismatch
        }
        try requireRequestContinuity(sessionID: sessionID, requestHash: requestHash)
        if let durableAcceptanceTicket {
            guard intentHash == durableAcceptanceTicket.intent.wireHash else {
                throw IrohaPeerNfcErrorV1.continuityMismatch
            }
            return .alreadyIssued
        }
        guard let pendingIntent,
              intentHash == pendingIntent.descriptor.wireHash,
              let bytes = pendingIntent.buffer.completeBytes else {
            throw IrohaPeerNfcErrorV1.stateMismatch
        }
        let intent = try nfcDecodeMessage(
            bytes,
            expectedProfile: profilePolicy.profile,
            expectedKind: .intent,
            limits: limits
        )
        guard intent.header.bytes == pendingIntent.descriptor.header,
              intent.wireHash == pendingIntent.descriptor.wireHash else {
            throw IrohaPeerNfcErrorV1.invalidHash
        }
        return .requiresDurableTicket(
            try IrohaPeerNfcIntentCommitContextV1(
                identity: identity,
                profilePolicy: profilePolicy,
                receiveRequest: receiveRequest,
                intent: intent
            )
        )
    }

    public mutating func installAcceptanceTicket(
        _ record: IrohaPeerNfcDurableAcceptanceTicketV1
    ) throws {
        let validated = try IrohaPeerNfcDurableAcceptanceTicketV1.decode(
            record.encoded,
            receiveRequest: receiveRequest.encoded,
            profilePolicy: profilePolicy,
            limits: limits
        )
        guard validated == record, validated.identity == identity else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
        if let durableAcceptanceTicket {
            guard durableAcceptanceTicket == validated else {
                throw IrohaPeerNfcErrorV1.conflictingReplay
            }
            return
        }
        guard let pendingIntent,
              pendingIntent.descriptor.wireHash
                == validated.intent.wireHash,
              pendingIntent.buffer.completeBytes == validated.intent.encoded else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
        durableAcceptanceTicket = validated
        self.pendingIntent = nil
    }

    public func preparePaymentAdmission(
        _ command: IrohaPeerNfcCommandV1
    ) throws -> IrohaPeerNfcPaymentAdmissionDispositionV1 {
        guard case .beginPayment(let sessionID, let ticketHash, let header) = command else {
            throw IrohaPeerNfcErrorV1.stateMismatch
        }
        try requireSession(sessionID)
        guard let durableAcceptanceTicket,
              ticketHash == durableAcceptanceTicket.ticket.wireHash else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
        let descriptor = try IrohaPeerNfcPaymentDescriptorV1(
            paymentHeader: header,
            limits: limits
        )
        guard profilePolicy.accepts(descriptor.profile) else {
            throw IrohaPeerNfcErrorV1.invalidProfile
        }
        if let durableAcknowledgement {
            guard durableAcknowledgement.payment.header.bytes == descriptor.header,
                  durableAcknowledgement.paymentWireHash == descriptor.wireHash else {
                throw IrohaPeerNfcErrorV1.conflictingReplay
            }
            return .alreadyAdmitted
        }
        if let pendingPayment {
            guard pendingPayment.descriptor == descriptor else {
                throw IrohaPeerNfcErrorV1.conflictingReplay
            }
            return .alreadyAdmitted
        }
        return .requiresDurableAdmission(
            try IrohaPeerNfcPaymentAdmissionContextV1(
                identity: identity,
                profilePolicy: profilePolicy,
                intentWireHash: durableAcceptanceTicket.intent.wireHash,
                ticketWireHash: durableAcceptanceTicket.ticket.wireHash,
                descriptor: descriptor
            )
        )
    }

    public mutating func installPaymentAdmission(
        _ record: IrohaPeerNfcDurablePaymentAdmissionV1
    ) throws {
        guard let durableAcceptanceTicket else {
            throw IrohaPeerNfcErrorV1.stateMismatch
        }
        let validated = try IrohaPeerNfcDurablePaymentAdmissionV1.decode(
            record.encoded,
            durableAcceptanceTicket: durableAcceptanceTicket,
            profilePolicy: profilePolicy,
            limits: limits
        )
        guard validated == record,
              validated.identity == identity,
              validated.profilePolicy == profilePolicy else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
        if durableAcknowledgement != nil {
            throw IrohaPeerNfcErrorV1.stateMismatch
        }
        if let pendingPayment {
            guard pendingPayment.descriptor == validated.descriptor else {
                throw IrohaPeerNfcErrorV1.conflictingReplay
            }
            return
        }
        pendingPayment = PendingPayment(
            descriptor: validated.descriptor,
            buffer: IrohaPeerNfcSparseInboundBufferV1(
                length: validated.descriptor.messageLength)
        )
    }

    public func prepareCommit(
        _ command: IrohaPeerNfcCommandV1
    ) throws -> IrohaPeerNfcCommitDispositionV1 {
        guard case .commitPayment(let sessionID, let ticketHash, let paymentHash) = command else {
            throw IrohaPeerNfcErrorV1.stateMismatch
        }
        try requireSession(sessionID)
        guard let durableAcceptanceTicket,
              ticketHash == durableAcceptanceTicket.ticket.wireHash else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
        if let durableAcknowledgement {
            guard paymentHash == durableAcknowledgement.paymentWireHash else {
                throw IrohaPeerNfcErrorV1.continuityMismatch
            }
            return .alreadyCommitted
        }
        guard let pendingPayment else { throw IrohaPeerNfcErrorV1.stateMismatch }
        guard paymentHash == pendingPayment.descriptor.wireHash else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
        guard let paymentBytes = pendingPayment.buffer.completeBytes else {
            throw IrohaPeerNfcErrorV1.stateMismatch
        }
        let payment = try nfcDecodeMessage(
            paymentBytes,
            expectedProfile: nil,
            expectedKind: .payment,
            limits: limits
        )
        guard profilePolicy.accepts(payment.profile),
              payment.header.bytes == pendingPayment.descriptor.header,
              payment.wireHash == pendingPayment.descriptor.wireHash else {
            throw IrohaPeerNfcErrorV1.invalidHash
        }
        return .requiresDurableCommit(
            try IrohaPeerNfcCommitContextV1(
                identity: identity,
                profilePolicy: profilePolicy,
                receiveRequest: receiveRequest,
                durableAcceptanceTicket: durableAcceptanceTicket,
                payment: payment
            )
        )
    }

    /// Installs only a record for the exact pending payment. Call this after
    /// the application has atomically persisted the payment outcome and ACK.
    public mutating func installDurableAcknowledgement(
        _ record: IrohaPeerNfcDurableAcknowledgementV1
    ) throws {
        guard let durableAcceptanceTicket,
              record.identity == identity,
              record.intentWireHash == durableAcceptanceTicket.intent.wireHash,
              record.ticketWireHash == durableAcceptanceTicket.ticket.wireHash else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
        let validated = try IrohaPeerNfcDurableAcknowledgementV1.decode(
            record.encoded,
            receiveRequest: receiveRequest.encoded,
            durableAcceptanceTicket: durableAcceptanceTicket,
            profilePolicy: profilePolicy,
            limits: limits
        )
        guard validated == record else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
        guard profilePolicy.accepts(record.paymentProfile),
              profilePolicy.accepts(record.acknowledgement.profile) else {
            throw IrohaPeerNfcErrorV1.invalidProfile
        }
        if let durableAcknowledgement {
            guard durableAcknowledgement == record else {
                throw IrohaPeerNfcErrorV1.conflictingReplay
            }
            return
        }
        guard let pendingPayment,
              pendingPayment.buffer.completeBytes == record.payment.encoded,
              record.paymentProfile == pendingPayment.descriptor.profile,
              record.paymentLength == pendingPayment.descriptor.messageLength,
              record.paymentWireHash == pendingPayment.descriptor.wireHash else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
        durableAcknowledgement = record
        self.pendingPayment = nil
    }

    /// Decodes and executes a raw APDU. A successful COMMIT response is never
    /// produced until `durableCommit` returns an exact persisted record.
    public mutating func process(
        apdu: Data,
        admitIntent: DurableIntentAdmissionHandler? = nil,
        issueAcceptanceTicket: DurableTicketHandler? = nil,
        admitPayment: DurableAdmissionHandler? = nil,
        durableCommit: DurableCommitHandler? = nil
    ) -> IrohaPeerNfcAPDUResponseV1 {
        let command: IrohaPeerNfcCommandV1
        do {
            command = try IrohaPeerNfcAPDUCodecV1.decode(apdu)
        } catch let error as IrohaPeerNfcErrorV1 {
            return Self.failureResponse(for: error)
        } catch {
            return IrohaPeerNfcAPDUResponseV1(statusWord: .wrongData)
        }
        if case .beginIntent = command {
            do {
                switch try prepareIntentAdmission(command) {
                case .alreadyAdmitted:
                    return IrohaPeerNfcAPDUResponseV1(statusWord: .success)
                case .requiresDurableAdmission(let context):
                    guard let admitIntent else {
                        throw IrohaPeerNfcErrorV1.durableIntentAdmissionRequired
                    }
                    let record: IrohaPeerNfcDurableIntentAdmissionV1
                    do { record = try admitIntent(context) } catch {
                        return IrohaPeerNfcAPDUResponseV1(statusWord: .storageFailure)
                    }
                    guard record.context == context else {
                        return IrohaPeerNfcAPDUResponseV1(
                            statusWord: .securityStatusNotSatisfied
                        )
                    }
                    try installIntentAdmission(record)
                    return IrohaPeerNfcAPDUResponseV1(statusWord: .success)
                }
            } catch let error as IrohaPeerNfcErrorV1 {
                return Self.failureResponse(for: error)
            } catch {
                return IrohaPeerNfcAPDUResponseV1(statusWord: .storageFailure)
            }
        } else if case .commitIntent = command {
            do {
                switch try prepareIntentCommit(command) {
                case .alreadyIssued:
                    return IrohaPeerNfcAPDUResponseV1(statusWord: .success)
                case .requiresDurableTicket(let context):
                    guard let issueAcceptanceTicket else {
                        throw IrohaPeerNfcErrorV1.durableTicketRequired
                    }
                    let record: IrohaPeerNfcDurableAcceptanceTicketV1
                    do { record = try issueAcceptanceTicket(context) } catch {
                        return IrohaPeerNfcAPDUResponseV1(statusWord: .storageFailure)
                    }
                    try installAcceptanceTicket(record)
                    return IrohaPeerNfcAPDUResponseV1(statusWord: .success)
                }
            } catch let error as IrohaPeerNfcErrorV1 {
                return Self.failureResponse(for: error)
            } catch {
                return IrohaPeerNfcAPDUResponseV1(statusWord: .storageFailure)
            }
        } else if case .beginPayment = command {
            do {
                switch try preparePaymentAdmission(command) {
                case .alreadyAdmitted:
                    return IrohaPeerNfcAPDUResponseV1(statusWord: .success)
                case .requiresDurableAdmission(let context):
                    guard let admitPayment else {
                        throw IrohaPeerNfcErrorV1.durableAdmissionRequired
                    }
                    let record: IrohaPeerNfcDurablePaymentAdmissionV1
                    do { record = try admitPayment(context) } catch {
                        return IrohaPeerNfcAPDUResponseV1(statusWord: .storageFailure)
                    }
                    guard record.context == context else {
                        return IrohaPeerNfcAPDUResponseV1(
                            statusWord: .securityStatusNotSatisfied
                        )
                    }
                    try installPaymentAdmission(record)
                    return IrohaPeerNfcAPDUResponseV1(statusWord: .success)
                }
            } catch let error as IrohaPeerNfcErrorV1 {
                return Self.failureResponse(for: error)
            } catch {
                return IrohaPeerNfcAPDUResponseV1(statusWord: .storageFailure)
            }
        } else if case .commitPayment = command {
            do {
                switch try prepareCommit(command) {
                case .alreadyCommitted:
                    return IrohaPeerNfcAPDUResponseV1(statusWord: .success)
                case .requiresDurableCommit(let context):
                    guard let durableCommit else {
                        throw IrohaPeerNfcErrorV1.durableCommitRequired
                    }
                    let record: IrohaPeerNfcDurableAcknowledgementV1
                    do {
                        record = try durableCommit(context)
                    } catch {
                        return IrohaPeerNfcAPDUResponseV1(statusWord: .storageFailure)
                    }
                    try installDurableAcknowledgement(record)
                    return IrohaPeerNfcAPDUResponseV1(statusWord: .success)
                }
            } catch let error as IrohaPeerNfcErrorV1 {
                return Self.failureResponse(for: error)
            } catch {
                return IrohaPeerNfcAPDUResponseV1(statusWord: .storageFailure)
            }
        }
        do {
            return IrohaPeerNfcAPDUResponseV1(
                data: try handle(command),
                statusWord: .success
            )
        } catch let error as IrohaPeerNfcErrorV1 {
            return Self.failureResponse(for: error)
        } catch {
            return IrohaPeerNfcAPDUResponseV1(statusWord: .wrongData)
        }
    }

    private var currentFlags: IrohaPeerNfcFlagsV1 {
        var flags: IrohaPeerNfcFlagsV1 = [.idempotentWrites]
        if phase != .requestReady { flags.insert(.durableState) }
        return flags
    }

    private mutating func writePayment(
        sessionID: Data,
        paymentHash: Data,
        offset: UInt32,
        bytes: Data
    ) throws {
        try requireSession(sessionID)
        guard var pendingPayment else { throw IrohaPeerNfcErrorV1.stateMismatch }
        guard paymentHash == pendingPayment.descriptor.wireHash else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
        try pendingPayment.buffer.install(
            offset: offset,
            chunk: bytes,
            maximumChunkBytes: limits.maximumWriteChunkBytes
        )
        self.pendingPayment = pendingPayment
    }

    private mutating func confirmAcknowledgement(
        sessionID: Data,
        paymentHash: Data,
        acknowledgementHash: Data
    ) throws {
        try requireSession(sessionID)
        guard let durableAcknowledgement else {
            throw IrohaPeerNfcErrorV1.acknowledgementNotDurable
        }
        guard paymentHash == durableAcknowledgement.paymentWireHash,
              acknowledgementHash == durableAcknowledgement.acknowledgement.wireHash else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
        acknowledgementConfirmed = true
    }

    private func requireSession(_ sessionID: Data) throws {
        try nfcRequireSession(sessionID)
        guard sessionID == identity.sessionID else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
    }

    private func requireRequestContinuity(sessionID: Data, requestHash: Data) throws {
        try requireSession(sessionID)
        try nfcRequireHash(requestHash)
        guard requestHash == identity.requestCanonicalHash else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
    }

    private func readSlice(
        _ message: Data,
        offset: UInt32,
        requestedLength: Int
    ) throws -> Data {
        guard requestedLength > 0,
              requestedLength <= limits.maximumReadChunkBytes else {
            throw IrohaPeerNfcErrorV1.invalidLength
        }
        guard let start = Int(exactly: offset), start < message.count else {
            throw IrohaPeerNfcErrorV1.invalidOffset
        }
        let end = start + min(requestedLength, message.count - start)
        return message.subdata(in: start..<end)
    }

    private static func failureResponse(
        for error: IrohaPeerNfcErrorV1
    ) -> IrohaPeerNfcAPDUResponseV1 {
        let statusWord: IrohaPeerNfcStatusWordV1
        switch error {
        case .invalidCommandClass:
            statusWord = .classNotSupported
        case .unsupportedInstruction:
            statusWord = .instructionNotSupported
        case .invalidLength, .messageTooLarge:
            statusWord = .wrongLength
        case .invalidSession, .invalidHash, .invalidProfile, .invalidKind,
             .continuityMismatch, .invalidIPM1:
            statusWord = .securityStatusNotSatisfied
        case .stateMismatch, .durableIntentAdmissionRequired,
             .durableAdmissionRequired, .durableTicketRequired,
             .durableCommitRequired,
             .acknowledgementNotDurable, .conflictingReplay, .peerRejected,
             .peerPersistenceFailure:
            statusWord = .conditionsNotSatisfied
        case .invalidApplicationIdentifier:
            statusWord = .notFound
        case .invalidAPDU, .invalidOffset, .invalidFlags:
            statusWord = .wrongData
        }
        return IrohaPeerNfcAPDUResponseV1(statusWord: statusWord)
    }
}

/// Helpers for the request-reading phase before a payment checkpoint exists.
public enum IrohaPeerNfcReaderPlanningV1 {
    public static func getStatusCommand(
        for info: IrohaPeerNfcInfoV1
    ) -> IrohaPeerNfcCommandV1 {
        .getStatus(
            sessionID: info.identity.sessionID,
            requestCanonicalHash: info.identity.requestCanonicalHash
        )
    }

    public static func readRequestCommand(
        for info: IrohaPeerNfcInfoV1,
        offset: Int,
        localLimits: IrohaPeerNfcLimitsV1 = .default
    ) throws -> IrohaPeerNfcCommandV1 {
        guard info.requestLength <= localLimits.maximumMessageBytes else {
            throw IrohaPeerNfcErrorV1.messageTooLarge(
                actual: info.requestLength,
                maximum: localLimits.maximumMessageBytes
            )
        }
        guard offset >= 0, offset < info.requestLength, offset <= Int(UInt32.max) else {
            throw IrohaPeerNfcErrorV1.invalidOffset
        }
        return .readRequest(
            sessionID: info.identity.sessionID,
            requestCanonicalHash: info.identity.requestCanonicalHash,
            offset: UInt32(offset),
            length: min(
                min(
                    localLimits.maximumReadChunkBytes,
                    info.maximumReadChunkBytes
                ),
                info.requestLength - offset
            )
        )
    }

    @discardableResult
    public static func validateReceiveRequest(
        _ data: Data,
        against info: IrohaPeerNfcInfoV1,
        limits: IrohaPeerNfcLimitsV1 = .default
    ) throws -> IrohaPeerWireMessageV1 {
        guard data.count == info.requestLength,
              data.count <= limits.maximumMessageBytes else {
            throw IrohaPeerNfcErrorV1.invalidLength
        }
        let message = try nfcDecodeMessage(
            data,
            expectedProfile: info.identity.profile,
            expectedKind: .request,
            limits: limits
        )
        guard message.canonicalHash == info.identity.requestCanonicalHash,
              message.wireHash == info.identity.requestWireHash else {
            throw IrohaPeerNfcErrorV1.invalidHash
        }
        return message
    }
}

/// Sender state persisted before message 2 and advanced atomically after
/// message 3. Recovery always reuses the exact intent, ticket, payment,
/// and acknowledgement bytes; it never creates a replacement monetary value.
public struct IrohaPeerNfcSenderCheckpointV1: Equatable, Sendable {
    public let identity: IrohaPeerNfcRequestIdentityV1
    public let profilePolicy: IrohaPeerNfcProfilePolicyV1
    public let receiveRequest: IrohaPeerWireMessageV1
    public let intent: IrohaPeerWireMessageV1
    public let ticket: IrohaPeerWireMessageV1?
    public let payment: IrohaPeerWireMessageV1?
    public let durableAcknowledgement: IrohaPeerWireMessageV1?

    public init(
        sessionID: Data,
        receiveRequest: Data,
        intent: Data,
        ticket: Data? = nil,
        payment: Data? = nil,
        durableAcknowledgement: Data? = nil,
        profilePolicy: IrohaPeerNfcProfilePolicyV1? = nil,
        limits: IrohaPeerNfcLimitsV1 = .default
    ) throws {
        try nfcRequireSession(sessionID)
        let requestMessage = try nfcDecodeMessage(
            receiveRequest,
            expectedProfile: nil,
            expectedKind: .request,
            limits: limits
        )
        let effectivePolicy = profilePolicy ?? .init(profile: requestMessage.profile)
        guard effectivePolicy.profile == requestMessage.profile else {
            throw IrohaPeerNfcErrorV1.invalidProfile
        }
        let intentMessage = try nfcDecodeMessage(
            intent,
            expectedProfile: nil,
            expectedKind: .intent,
            limits: limits
        )
        guard effectivePolicy.accepts(intentMessage.profile) else {
            throw IrohaPeerNfcErrorV1.invalidProfile
        }
        try nfcValidateIntentBinding(
            request: requestMessage,
            intent: intentMessage
        )
        guard (ticket == nil) == (payment == nil) else {
            throw IrohaPeerNfcErrorV1.stateMismatch
        }
        let ticketMessage = try ticket.map {
            try nfcDecodeMessage(
                $0,
                expectedProfile: effectivePolicy.profile,
                expectedKind: .ticket,
                limits: limits
            )
        }
        let paymentMessage = try payment.map {
            try nfcDecodeMessage(
                $0,
                expectedProfile: effectivePolicy.profile,
                expectedKind: .payment,
                limits: limits
            )
        }
        if let ticketMessage, let paymentMessage {
            try nfcValidateCommittedPaymentBindings(
                request: requestMessage,
                intent: intentMessage,
                ticket: ticketMessage,
                payment: paymentMessage
            )
        }
        let ackMessage: IrohaPeerWireMessageV1?
        if let durableAcknowledgement {
            ackMessage = try nfcDecodeMessage(
                durableAcknowledgement,
                expectedProfile: nil,
                expectedKind: .acknowledgement,
                limits: limits
            )
            guard effectivePolicy.accepts(ackMessage!.profile) else {
                throw IrohaPeerNfcErrorV1.invalidProfile
            }
            guard let ticketMessage, let paymentMessage else {
                throw IrohaPeerNfcErrorV1.stateMismatch
            }
            try nfcValidateCompleteBindings(
                request: requestMessage,
                intent: intentMessage,
                ticket: ticketMessage,
                payment: paymentMessage,
                acknowledgement: ackMessage!
            )
        } else {
            ackMessage = nil
        }
        self.identity = try IrohaPeerNfcRequestIdentityV1(
            profile: requestMessage.profile,
            sessionID: sessionID,
            requestCanonicalHash: requestMessage.canonicalHash,
            requestWireHash: requestMessage.wireHash
        )
        self.profilePolicy = effectivePolicy
        self.receiveRequest = requestMessage
        self.intent = intentMessage
        self.ticket = ticketMessage
        self.payment = paymentMessage
        self.durableAcknowledgement = ackMessage
    }

    public var encoded: Data {
        let ticketBytes = ticket?.encoded ?? Data()
        let paymentBytes = payment?.encoded ?? Data()
        let ack = durableAcknowledgement?.encoded ?? Data()
        var output = IrohaPeerNfcV1.senderCheckpointMagic
        output.append(IrohaPeerNfcV1.wireVersion)
        output.nfcAppendUInt16BE(identity.profile.rawValue)
        output.append(0)
        output.append(identity.sessionID)
        output.nfcAppendUInt32BE(UInt32(receiveRequest.encoded.count))
        output.nfcAppendUInt32BE(UInt32(intent.encoded.count))
        output.nfcAppendUInt32BE(UInt32(ticketBytes.count))
        output.nfcAppendUInt32BE(UInt32(paymentBytes.count))
        output.nfcAppendUInt32BE(UInt32(ack.count))
        output.append(receiveRequest.encoded)
        output.append(intent.encoded)
        output.append(ticketBytes)
        output.append(paymentBytes)
        output.append(ack)
        return output
    }

    public static func decode(
        _ data: Data,
        profilePolicy: IrohaPeerNfcProfilePolicyV1? = nil,
        limits: IrohaPeerNfcLimitsV1 = .default
    ) throws -> Self {
        let fixedBytes = 44
        guard data.count > fixedBytes,
              data.prefix(4) == IrohaPeerNfcV1.senderCheckpointMagic,
              data[4] == IrohaPeerNfcV1.wireVersion,
              data[7] == 0,
              let profile = IrohaPeerPayloadProfile(rawValue: data.nfcUInt16BE(at: 5)),
              profile != .reject else {
            throw IrohaPeerNfcErrorV1.invalidLength
        }
        let requestLength = Int(data.nfcUInt32BE(at: 24))
        let intentLength = Int(data.nfcUInt32BE(at: 28))
        let ticketLength = Int(data.nfcUInt32BE(at: 32))
        let paymentLength = Int(data.nfcUInt32BE(at: 36))
        let acknowledgementLength = Int(data.nfcUInt32BE(at: 40))
        guard requestLength > IrohaPeerWireMessageV1.headerBytes,
              intentLength > IrohaPeerWireMessageV1.headerBytes,
              (ticketLength == 0) == (paymentLength == 0),
              ticketLength == 0 || ticketLength > IrohaPeerWireMessageV1.headerBytes,
              paymentLength == 0 || paymentLength > IrohaPeerWireMessageV1.headerBytes,
              acknowledgementLength == 0
                || acknowledgementLength > IrohaPeerWireMessageV1.headerBytes,
              requestLength <= limits.maximumMessageBytes,
              intentLength <= limits.maximumMessageBytes,
              ticketLength <= limits.maximumMessageBytes,
              paymentLength <= limits.maximumMessageBytes,
              acknowledgementLength <= limits.maximumMessageBytes,
              fixedBytes <= data.count - requestLength,
              fixedBytes + requestLength <= data.count - intentLength,
              fixedBytes + requestLength + intentLength <= data.count - ticketLength,
              fixedBytes + requestLength + intentLength + ticketLength
                <= data.count - paymentLength,
              fixedBytes + requestLength + intentLength + ticketLength + paymentLength
                <= data.count - acknowledgementLength,
              fixedBytes + requestLength + intentLength + ticketLength + paymentLength
                + acknowledgementLength == data.count else {
            throw IrohaPeerNfcErrorV1.invalidLength
        }
        let requestStart = fixedBytes
        let intentStart = requestStart + requestLength
        let ticketStart = intentStart + intentLength
        let paymentStart = ticketStart + ticketLength
        let acknowledgementStart = paymentStart + paymentLength
        let checkpoint = try Self(
            sessionID: data.subdata(in: 8..<24),
            receiveRequest: data.subdata(in: requestStart..<intentStart),
            intent: data.subdata(in: intentStart..<ticketStart),
            ticket: ticketLength == 0
                ? nil
                : data.subdata(in: ticketStart..<paymentStart),
            payment: paymentLength == 0
                ? nil
                : data.subdata(in: paymentStart..<acknowledgementStart),
            durableAcknowledgement: acknowledgementLength == 0
                ? nil
                : data.subdata(in: acknowledgementStart..<data.count),
            profilePolicy: profilePolicy,
            limits: limits
        )
        guard checkpoint.identity.profile == profile else {
            throw IrohaPeerNfcErrorV1.invalidProfile
        }
        return checkpoint
    }

    fileprivate func addingTicketAndPayment(
        ticket: Data,
        payment: Data,
        limits: IrohaPeerNfcLimitsV1
    ) throws -> Self {
        guard self.ticket == nil, self.payment == nil,
              durableAcknowledgement == nil else {
            throw IrohaPeerNfcErrorV1.stateMismatch
        }
        return try Self(
            sessionID: identity.sessionID,
            receiveRequest: receiveRequest.encoded,
            intent: intent.encoded,
            ticket: ticket,
            payment: payment,
            profilePolicy: profilePolicy,
            limits: limits
        )
    }

    fileprivate func addingDurableAcknowledgement(
        _ acknowledgement: Data,
        limits: IrohaPeerNfcLimitsV1
    ) throws -> Self {
        guard let ticket, let payment else {
            throw IrohaPeerNfcErrorV1.stateMismatch
        }
        return try Self(
            sessionID: identity.sessionID,
            receiveRequest: receiveRequest.encoded,
            intent: intent.encoded,
            ticket: ticket.encoded,
            payment: payment.encoded,
            durableAcknowledgement: acknowledgement,
            profilePolicy: profilePolicy,
            limits: limits
        )
    }
}

/// One deterministic next step from a status-reconciled sender.
public enum IrohaPeerNfcSenderActionV1: Equatable, Sendable {
    case send(IrohaPeerNfcCommandV1)
    /// Ask sender hardware to create and durably persist message 4 against
    /// the complete validated message-3 ticket.
    case preparePayment(ticket: Data)
    /// Persist the supplied complete, validated ACK via `persistAcknowledgement`.
    case persistAcknowledgement(Data)
    /// The peer is complete and this exact ACK was already durable locally.
    case complete(Data)
}

/// Status-driven sender reducer designed for RF loss between any two APDUs.
///
/// On a second tap, read `GET_INFO`, require the same identity, then pass
/// `GET_STATUS` here. Receiver progress is authoritative; the exact durable
/// payment checkpoint supplies bytes for replay without creating a new debit.
public struct IrohaPeerNfcTwoTapReducerV1: Sendable {
    public typealias AcknowledgementPersistenceHandler = (Data) throws -> Void

    public private(set) var checkpoint: IrohaPeerNfcSenderCheckpointV1
    public let limits: IrohaPeerNfcLimitsV1

    private var ticketBuffer = Data()
    private var expectedTicketLength: Int?
    private var expectedTicketHash: Data?
    private var acknowledgementBuffer = Data()
    private var expectedAcknowledgementLength: Int?
    private var expectedAcknowledgementHash: Data?

    public init(
        checkpoint: IrohaPeerNfcSenderCheckpointV1,
        limits: IrohaPeerNfcLimitsV1 = .default
    ) {
        self.checkpoint = checkpoint
        self.limits = limits
    }

    public func requireSamePeer(_ info: IrohaPeerNfcInfoV1) throws {
        guard info.identity == checkpoint.identity,
              info.requestLength == checkpoint.receiveRequest.encoded.count else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
    }

    public mutating func nextAction(
        observing status: IrohaPeerNfcStatusV1
    ) throws -> IrohaPeerNfcSenderActionV1 {
        try validateContinuity(status)
        let intent = checkpoint.intent
        switch status.phase {
        case .requestReady:
            guard checkpoint.ticket == nil,
                  checkpoint.payment == nil,
                  checkpoint.durableAcknowledgement == nil else {
                throw IrohaPeerNfcErrorV1.continuityMismatch
            }
            resetTicketBuffer()
            resetAcknowledgementBuffer()
            return .send(.beginIntent(
                sessionID: checkpoint.identity.sessionID,
                requestCanonicalHash: checkpoint.identity.requestCanonicalHash,
                intentHeader: intent.header.bytes
            ))
        case .intentReceiving:
            guard checkpoint.ticket == nil,
                  checkpoint.payment == nil,
                  checkpoint.durableAcknowledgement == nil,
                  status.inboundKind == .intent,
                  status.inboundLength == intent.encoded.count,
                  status.inboundWireHash == intent.wireHash else {
                throw IrohaPeerNfcErrorV1.continuityMismatch
            }
            resetTicketBuffer()
            resetAcknowledgementBuffer()
            if status.receivedInboundBytes < status.inboundLength {
                let offset = status.nextMissingInboundOffset
                let count = min(
                    min(limits.maximumWriteChunkBytes, status.maximumWriteChunkBytes),
                    status.inboundLength - offset
                )
                guard count > 0, offset <= Int(UInt32.max) else {
                    throw IrohaPeerNfcErrorV1.invalidOffset
                }
                return .send(.writeIntent(
                    sessionID: checkpoint.identity.sessionID,
                    intentWireHash: intent.wireHash,
                    offset: UInt32(offset),
                    bytes: intent.encoded.subdata(in: offset..<(offset + count))
                ))
            }
            return .send(.commitIntent(
                sessionID: checkpoint.identity.sessionID,
                requestCanonicalHash: checkpoint.identity.requestCanonicalHash,
                intentWireHash: intent.wireHash
            ))
        case .ticketReady:
            try requireIntentAndTicketMetadata(status)
            resetAcknowledgementBuffer()
            if let ticket = checkpoint.ticket, let payment = checkpoint.payment {
                guard ticket.encoded.count == status.outboundLength,
                      ticket.wireHash == status.outboundWireHash else {
                    throw IrohaPeerNfcErrorV1.continuityMismatch
                }
                resetTicketBuffer()
                return .send(.beginPayment(
                    sessionID: checkpoint.identity.sessionID,
                    ticketWireHash: ticket.wireHash,
                    paymentHeader: payment.header.bytes
                ))
            }
            guard checkpoint.ticket == nil, checkpoint.payment == nil else {
                throw IrohaPeerNfcErrorV1.continuityMismatch
            }
            configureTicketBuffer(for: status)
            if ticketBuffer.count == status.outboundLength {
                return .preparePayment(ticket: ticketBuffer)
            }
            let offset = ticketBuffer.count
            let count = min(
                min(limits.maximumReadChunkBytes, status.maximumReadChunkBytes),
                status.outboundLength - offset
            )
            guard count > 0, offset <= Int(UInt32.max) else {
                throw IrohaPeerNfcErrorV1.invalidOffset
            }
            return .send(.readTicket(
                sessionID: checkpoint.identity.sessionID,
                intentWireHash: intent.wireHash,
                offset: UInt32(offset),
                length: count
            ))
        case .paymentReceiving:
            guard let ticket = checkpoint.ticket,
                  let payment = checkpoint.payment,
                  checkpoint.durableAcknowledgement == nil,
                  status.outboundKind == .ticket,
                  status.outboundWireHash == ticket.wireHash,
                  status.inboundKind == .payment,
                  status.inboundLength == payment.encoded.count,
                  status.inboundWireHash == payment.wireHash else {
                throw IrohaPeerNfcErrorV1.continuityMismatch
            }
            resetTicketBuffer()
            resetAcknowledgementBuffer()
            if status.receivedInboundBytes < status.inboundLength {
                let offset = status.nextMissingInboundOffset
                let count = min(
                    min(
                        limits.maximumWriteChunkBytes,
                        status.maximumWriteChunkBytes
                    ),
                    status.inboundLength - offset
                )
                guard offset <= Int(UInt32.max) else {
                    throw IrohaPeerNfcErrorV1.invalidOffset
                }
                return .send(.writePayment(
                    sessionID: checkpoint.identity.sessionID,
                    paymentWireHash: payment.wireHash,
                    offset: UInt32(offset),
                    bytes: payment.encoded.subdata(in: offset..<(offset + count))
                ))
            }
            return .send(.commitPayment(
                sessionID: checkpoint.identity.sessionID,
                ticketWireHash: ticket.wireHash,
                paymentWireHash: payment.wireHash
            ))
        case .acknowledgementReady:
            try requirePaymentAndAcknowledgementMetadata(status)
            guard let payment = checkpoint.payment else {
                throw IrohaPeerNfcErrorV1.continuityMismatch
            }
            if let durableAcknowledgement = checkpoint.durableAcknowledgement {
                guard status.outboundKind == .acknowledgement,
                      durableAcknowledgement.encoded.count == status.outboundLength,
                      durableAcknowledgement.wireHash == status.outboundWireHash else {
                    throw IrohaPeerNfcErrorV1.continuityMismatch
                }
                return .send(.confirmAcknowledgement(
                    sessionID: checkpoint.identity.sessionID,
                    paymentWireHash: payment.wireHash,
                    acknowledgementWireHash: durableAcknowledgement.wireHash
                ))
            }
            configureAcknowledgementBuffer(for: status)
            if acknowledgementBuffer.count == status.outboundLength {
                return .persistAcknowledgement(acknowledgementBuffer)
            }
            let offset = acknowledgementBuffer.count
            let count = min(
                min(
                    limits.maximumReadChunkBytes,
                    status.maximumReadChunkBytes
                ),
                status.outboundLength - offset
            )
            return .send(.readAcknowledgement(
                sessionID: checkpoint.identity.sessionID,
                paymentWireHash: payment.wireHash,
                offset: UInt32(offset),
                length: count
            ))
        case .complete:
            try requirePaymentAndAcknowledgementMetadata(status)
            guard let durableAcknowledgement = checkpoint.durableAcknowledgement,
                  status.outboundKind == .acknowledgement,
                  durableAcknowledgement.encoded.count == status.outboundLength,
                  durableAcknowledgement.wireHash == status.outboundWireHash else {
                throw IrohaPeerNfcErrorV1.acknowledgementNotDurable
            }
            return .complete(durableAcknowledgement.encoded)
        }
    }

    /// Supplies the response to the last `READ_TICKET` action. Ticket bytes are
    /// transient until application hardware returns and durably stores the
    /// exact ticket-bound payment checkpoint.
    @discardableResult
    public mutating func consumeTicketChunk(_ bytes: Data) throws -> Bool {
        guard let expectedTicketLength,
              let expectedTicketHash,
              !bytes.isEmpty,
              bytes.count <= limits.maximumReadChunkBytes,
              ticketBuffer.count < expectedTicketLength,
              bytes.count <= expectedTicketLength - ticketBuffer.count else {
            throw IrohaPeerNfcErrorV1.invalidLength
        }
        ticketBuffer.append(bytes)
        guard ticketBuffer.count == expectedTicketLength else {
            return false
        }
        let ticket = try nfcDecodeMessage(
            ticketBuffer,
            expectedProfile: checkpoint.profilePolicy.profile,
            expectedKind: .ticket,
            limits: limits
        )
        guard ticket.wireHash == expectedTicketHash else {
            throw IrohaPeerNfcErrorV1.invalidHash
        }
        try nfcValidatePreTicketBindings(
            request: checkpoint.receiveRequest,
            intent: checkpoint.intent,
            ticket: ticket
        )
        return true
    }

    /// Installs the exact ticket-and-payment checkpoint only after application
    /// hardware has committed and durably stored it. No payment is synthesized
    /// inside the NFC transport.
    public mutating func installPreparedPaymentCheckpoint(
        _ candidate: IrohaPeerNfcSenderCheckpointV1
    ) throws {
        guard checkpoint.ticket == nil,
              checkpoint.payment == nil,
              checkpoint.durableAcknowledgement == nil,
              let expectedTicketLength,
              let expectedTicketHash,
              ticketBuffer.count == expectedTicketLength else {
            throw IrohaPeerNfcErrorV1.stateMismatch
        }
        let ticket = try nfcDecodeMessage(
            ticketBuffer,
            expectedProfile: checkpoint.profilePolicy.profile,
            expectedKind: .ticket,
            limits: limits
        )
        guard ticket.wireHash == expectedTicketHash,
              candidate.identity == checkpoint.identity,
              candidate.profilePolicy == checkpoint.profilePolicy,
              candidate.receiveRequest == checkpoint.receiveRequest,
              candidate.intent == checkpoint.intent,
              candidate.ticket == ticket,
              candidate.payment != nil,
              candidate.durableAcknowledgement == nil else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
        checkpoint = candidate
        resetTicketBuffer()
    }

    /// Supplies the response to the last READ_ACK action. Partial ACK bytes
    /// are intentionally restartable rather than treated as durable state.
    @discardableResult
    public mutating func consumeAcknowledgementChunk(_ bytes: Data) throws -> Bool {
        guard let expectedAcknowledgementLength,
              let expectedAcknowledgementHash,
              !bytes.isEmpty,
              bytes.count <= limits.maximumReadChunkBytes,
              acknowledgementBuffer.count < expectedAcknowledgementLength,
              bytes.count <= expectedAcknowledgementLength - acknowledgementBuffer.count else {
            throw IrohaPeerNfcErrorV1.invalidLength
        }
        acknowledgementBuffer.append(bytes)
        guard acknowledgementBuffer.count == expectedAcknowledgementLength else {
            return false
        }
        let acknowledgement = try nfcDecodeMessage(
            acknowledgementBuffer,
            expectedProfile: nil,
            expectedKind: .acknowledgement,
            limits: limits
        )
        guard checkpoint.profilePolicy.accepts(acknowledgement.profile),
              acknowledgement.wireHash == expectedAcknowledgementHash else {
            throw IrohaPeerNfcErrorV1.invalidHash
        }
        return true
    }

    /// Persists the full sender checkpoint including ACK before allowing
    /// `CONFIRM_ACK`. If persistence throws, the reducer remains ACK-pending.
    public mutating func persistAcknowledgement(
        using persist: AcknowledgementPersistenceHandler
    ) throws {
        let candidate = try acknowledgementCheckpointCandidate()
        try persist(candidate.encoded)
        try installPersistedCheckpoint(candidate)
    }

    /// Builds the exact checkpoint that includes the complete validated ACK.
    /// The caller may persist this value asynchronously, then install it with
    /// `installPersistedCheckpoint(_:)`. Separating those operations keeps the
    /// transport from confirming an ACK before an actor/database write ends.
    public func acknowledgementCheckpointCandidate()
        throws -> IrohaPeerNfcSenderCheckpointV1 {
        guard let expectedAcknowledgementLength,
              let expectedAcknowledgementHash,
              acknowledgementBuffer.count == expectedAcknowledgementLength else {
            throw IrohaPeerNfcErrorV1.stateMismatch
        }
        let candidate = try checkpoint.addingDurableAcknowledgement(
            acknowledgementBuffer,
            limits: limits
        )
        guard candidate.durableAcknowledgement?.wireHash == expectedAcknowledgementHash else {
            throw IrohaPeerNfcErrorV1.invalidHash
        }
        return candidate
    }

    /// Installs only the byte-identical checkpoint previously returned by
    /// `acknowledgementCheckpointCandidate()`. This is the in-memory half of
    /// the sender durability boundary and must follow successful persistence.
    public mutating func installPersistedCheckpoint(
        _ candidate: IrohaPeerNfcSenderCheckpointV1
    ) throws {
        let expected = try acknowledgementCheckpointCandidate()
        guard candidate == expected else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
        checkpoint = candidate
        resetAcknowledgementBuffer()
    }

    private func validateContinuity(_ status: IrohaPeerNfcStatusV1) throws {
        guard status.identity == checkpoint.identity else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
    }

    private func requireIntentAndTicketMetadata(
        _ status: IrohaPeerNfcStatusV1
    ) throws {
        let intent = checkpoint.intent
        guard status.inboundKind == .intent,
              status.inboundLength == intent.encoded.count,
              status.receivedInboundBytes == intent.encoded.count,
              status.nextMissingInboundOffset == intent.encoded.count,
              status.inboundWireHash == intent.wireHash,
              status.outboundKind == .ticket,
              status.outboundLength > IrohaPeerWireMessageV1.headerBytes,
              status.outboundLength <= limits.maximumMessageBytes else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
    }

    private func requirePaymentAndAcknowledgementMetadata(
        _ status: IrohaPeerNfcStatusV1
    ) throws {
        guard checkpoint.ticket != nil,
              let payment = checkpoint.payment,
              status.inboundKind == .payment,
              status.inboundLength == payment.encoded.count,
              status.receivedInboundBytes == payment.encoded.count,
              status.nextMissingInboundOffset == payment.encoded.count,
              status.inboundWireHash == payment.wireHash,
              status.outboundKind == .acknowledgement,
              status.outboundLength > IrohaPeerWireMessageV1.headerBytes,
              status.outboundLength <= limits.maximumMessageBytes else {
            throw IrohaPeerNfcErrorV1.continuityMismatch
        }
    }

    private mutating func configureTicketBuffer(
        for status: IrohaPeerNfcStatusV1
    ) {
        if expectedTicketLength != status.outboundLength
            || expectedTicketHash != status.outboundWireHash {
            ticketBuffer.removeAll(keepingCapacity: false)
            expectedTicketLength = status.outboundLength
            expectedTicketHash = status.outboundWireHash
        }
    }

    private mutating func resetTicketBuffer() {
        ticketBuffer.removeAll(keepingCapacity: false)
        expectedTicketLength = nil
        expectedTicketHash = nil
    }

    private mutating func configureAcknowledgementBuffer(
        for status: IrohaPeerNfcStatusV1
    ) {
        if expectedAcknowledgementLength != status.outboundLength
            || expectedAcknowledgementHash != status.outboundWireHash {
            acknowledgementBuffer.removeAll(keepingCapacity: false)
            expectedAcknowledgementLength = status.outboundLength
            expectedAcknowledgementHash = status.outboundWireHash
        }
    }

    private mutating func resetAcknowledgementBuffer() {
        acknowledgementBuffer.removeAll(keepingCapacity: false)
        expectedAcknowledgementLength = nil
        expectedAcknowledgementHash = nil
    }
}

private func nfcDecodeMessage(
    _ data: Data,
    expectedProfile: IrohaPeerPayloadProfile?,
    expectedKind: IrohaPeerPayloadKind,
    limits: IrohaPeerNfcLimitsV1
) throws -> IrohaPeerWireMessageV1 {
    guard data.count > IrohaPeerWireMessageV1.headerBytes else {
        throw IrohaPeerNfcErrorV1.invalidLength
    }
    guard data.count <= limits.maximumMessageBytes else {
        throw IrohaPeerNfcErrorV1.messageTooLarge(
            actual: data.count,
            maximum: limits.maximumMessageBytes
        )
    }
    do {
        let message = try IrohaPeerWireMessageV1.decode(
            data,
            expectedProfile: expectedProfile,
            expectedKind: expectedKind
        )
        guard !message.canonicalPayload.isEmpty else {
            throw IrohaPeerNfcErrorV1.invalidLength
        }
        return message
    } catch let error as IrohaPeerNfcErrorV1 {
        throw error
    } catch let error as IrohaPeerWireMessageErrorV1 {
        switch error {
        case .unexpectedProfile, .invalidProfile:
            throw IrohaPeerNfcErrorV1.invalidProfile
        case .unexpectedKind, .invalidKind:
            throw IrohaPeerNfcErrorV1.invalidKind
        case .canonicalHashMismatch, .wireHashMismatch:
            throw IrohaPeerNfcErrorV1.invalidHash
        case .canonicalLengthOutOfRange, .encodedLengthOutOfRange, .lengthMismatch:
            throw IrohaPeerNfcErrorV1.invalidLength
        default:
            throw IrohaPeerNfcErrorV1.invalidIPM1
        }
    } catch {
        throw IrohaPeerNfcErrorV1.invalidIPM1
    }
}

private func nfcValidateIntentBinding(
    request: IrohaPeerWireMessageV1,
    intent: IrohaPeerWireMessageV1
) throws {
    do {
        let requestValue = try KagemushaNoritoV1.decodePaymentRequestShapeExact(
            request.canonicalPayload)
        _ = try KagemushaNoritoV1.decodeAcceptanceIntentShapeExact(
            intent.canonicalPayload,
            against: requestValue
        )
    } catch {
        throw IrohaPeerNfcErrorV1.continuityMismatch
    }
}

private func nfcValidatePreTicketBindings(
    request: IrohaPeerWireMessageV1,
    intent: IrohaPeerWireMessageV1,
    ticket: IrohaPeerWireMessageV1
) throws {
    do {
        let requestValue = try KagemushaNoritoV1.decodePaymentRequestShapeExact(
            request.canonicalPayload)
        let intentValue = try KagemushaNoritoV1
            .decodeAcceptanceIntentShapeExact(
                intent.canonicalPayload,
                against: requestValue
            )
        let ticketValue = try KagemushaNoritoV1.decodeAcceptanceTicketShapeExact(
            ticket.canonicalPayload,
            against: requestValue,
            intent: intentValue
        )
        _ = try KagemushaNoritoV1.validatePreTicketExchangeShape(
            request: requestValue,
            intent: intentValue,
            ticket: ticketValue
        )
    } catch {
        throw IrohaPeerNfcErrorV1.continuityMismatch
    }
}

private func nfcValidateCommittedPaymentBindings(
    request: IrohaPeerWireMessageV1,
    intent: IrohaPeerWireMessageV1,
    ticket: IrohaPeerWireMessageV1,
    payment: IrohaPeerWireMessageV1
) throws {
    do {
        let requestValue = try KagemushaNoritoV1.decodePaymentRequestShapeExact(
            request.canonicalPayload)
        let intentValue = try KagemushaNoritoV1
            .decodeAcceptanceIntentShapeExact(
                intent.canonicalPayload,
                against: requestValue
            )
        let ticketValue = try KagemushaNoritoV1.decodeAcceptanceTicketShapeExact(
            ticket.canonicalPayload,
            against: requestValue,
            intent: intentValue
        )
        let paymentValue = try KagemushaNoritoV1.decodePaymentShapeExact(
            payment.canonicalPayload,
            against: requestValue,
            intent: intentValue,
            ticket: ticketValue
        )
        _ = try KagemushaNoritoV1.validateCommittedPaymentShape(
            request: requestValue,
            intent: intentValue,
            ticket: ticketValue,
            payment: paymentValue
        )
    } catch {
        throw IrohaPeerNfcErrorV1.continuityMismatch
    }
}

private func nfcValidateCompleteBindings(
    request: IrohaPeerWireMessageV1,
    intent: IrohaPeerWireMessageV1,
    ticket: IrohaPeerWireMessageV1,
    payment: IrohaPeerWireMessageV1,
    acknowledgement: IrohaPeerWireMessageV1
) throws {
    do {
        let requestValue = try KagemushaNoritoV1.decodePaymentRequestShapeExact(
            request.canonicalPayload)
        let intentValue = try KagemushaNoritoV1
            .decodeAcceptanceIntentShapeExact(
                intent.canonicalPayload,
                against: requestValue
            )
        let ticketValue = try KagemushaNoritoV1.decodeAcceptanceTicketShapeExact(
            ticket.canonicalPayload,
            against: requestValue,
            intent: intentValue
        )
        let paymentValue = try KagemushaNoritoV1.decodePaymentShapeExact(
            payment.canonicalPayload,
            against: requestValue,
            intent: intentValue,
            ticket: ticketValue
        )
        let acknowledgementValue = try KagemushaNoritoV1
            .decodeAcknowledgementShapeExact(
                acknowledgement.canonicalPayload,
                against: requestValue,
                intent: intentValue,
                ticket: ticketValue,
                payment: paymentValue
            )
        _ = try KagemushaNoritoV1.validateCompleteExchangeShape(
            request: requestValue,
            intent: intentValue,
            ticket: ticketValue,
            payment: paymentValue,
            acknowledgement: acknowledgementValue
        )
    } catch {
        throw IrohaPeerNfcErrorV1.continuityMismatch
    }
}

/// Complete sender-side result. `checkpoint` contains the durable ACK and is
/// safe to reload if the final `CONFIRM_ACK` response was lost.
public enum IrohaPeerNfcConfirmationStateV1: Equatable, Sendable {
    case confirmed
    case responseUnknown
}

public struct IrohaPeerNfcReaderExchangeResultV1: Equatable, Sendable {
    public let checkpoint: IrohaPeerNfcSenderCheckpointV1
    public let acknowledgement: IrohaPeerWireMessageV1
    public let confirmationState: IrohaPeerNfcConfirmationStateV1

    public init(
        checkpoint: IrohaPeerNfcSenderCheckpointV1,
        acknowledgement: IrohaPeerWireMessageV1,
        confirmationState: IrohaPeerNfcConfirmationStateV1 = .confirmed
    ) {
        self.checkpoint = checkpoint
        self.acknowledgement = acknowledgement
        self.confirmationState = confirmationState
    }
}

/// Transport-neutral, status-reconciled reader exchange.
///
/// `loadOrCreateDurableCheckpoint` atomically stores message 2 before it is
/// transmitted. After message 3 is read and validated,
/// `preparePaymentCheckpoint` is the hardware boundary that irreversibly commits
/// message 4 and stores the exact ticket-and-payment checkpoint before
/// `BEGIN_PAYMENT`. `updateDurableCheckpoint` performs the final monotonic
/// ACK-bearing update before `CONFIRM_ACK` is emitted.
public enum IrohaPeerNfcReaderExchangeV1 {
    public typealias Transceive = @Sendable (
        IrohaPeerNfcCommandV1
    ) async throws -> IrohaPeerNfcAPDUResponseV1
    public typealias LoadOrCreateDurableCheckpoint = @Sendable (
        IrohaPeerNfcInfoV1,
        IrohaPeerWireMessageV1
    ) async throws -> IrohaPeerNfcSenderCheckpointV1
    public typealias PreparePaymentCheckpoint = @Sendable (
        IrohaPeerNfcSenderCheckpointV1,
        IrohaPeerWireMessageV1
    ) async throws -> IrohaPeerNfcSenderCheckpointV1
    public typealias UpdateDurableCheckpoint = @Sendable (Data) async throws -> Void

    /// Covers all five protocol-maximum messages at the minimum one-byte chunk,
    /// plus SELECT/INFO, phase probes, controls, and durable transitions.
    public static let defaultMaximumActions =
        5 * IrohaPeerNfcV1.maximumMessageBytes + 24

    private struct ActionBudget {
        var remaining: Int

        mutating func consume() throws {
            guard remaining > 0 else {
                throw IrohaPeerNfcErrorV1.stateMismatch
            }
            remaining -= 1
        }
    }

    public static func run(
        restoredCheckpoint: Data? = nil,
        profilePolicy: IrohaPeerNfcProfilePolicyV1,
        limits: IrohaPeerNfcLimitsV1 = .default,
        maximumActions: Int = IrohaPeerNfcReaderExchangeV1.defaultMaximumActions,
        transceive: Transceive,
        loadOrCreateDurableCheckpoint: LoadOrCreateDurableCheckpoint,
        preparePaymentCheckpoint: PreparePaymentCheckpoint,
        updateDurableCheckpoint: UpdateDurableCheckpoint
    ) async throws -> IrohaPeerNfcReaderExchangeResultV1 {
        precondition(maximumActions > 0)
        var actionBudget = ActionBudget(remaining: maximumActions)

        try actionBudget.consume()
        try requireEmptySuccess(await transceive(.selectApplication), operation: .selectApplication)
        try actionBudget.consume()
        let infoResponse = try await requireSuccess(transceive(.getInfo), operation: .getInfo)
        let info = try IrohaPeerNfcInfoV1.decode(infoResponse.data)
        guard info.identity.profile == profilePolicy.profile else {
            throw IrohaPeerNfcErrorV1.invalidProfile
        }
        guard info.requestLength <= limits.maximumMessageBytes else {
            throw IrohaPeerNfcErrorV1.messageTooLarge(
                actual: info.requestLength,
                maximum: limits.maximumMessageBytes
            )
        }
        if restoredCheckpoint == nil {
            // Only a newly advertised request may authorize value creation.
            // Other phases belong to an already-started exchange and require
            // its exact durable sender checkpoint to resume without re-debit.
            guard info.phase == .requestReady else {
                throw IrohaPeerNfcErrorV1.stateMismatch
            }
        }

        let checkpoint: IrohaPeerNfcSenderCheckpointV1
        if let restoredCheckpoint {
            checkpoint = try IrohaPeerNfcSenderCheckpointV1.decode(
                restoredCheckpoint,
                profilePolicy: profilePolicy,
                limits: limits
            )
            let continuity = IrohaPeerNfcTwoTapReducerV1(
                checkpoint: checkpoint,
                limits: limits
            )
            try continuity.requireSamePeer(info)
        } else {
            var requestBytes = Data()
            requestBytes.reserveCapacity(info.requestLength)
            while requestBytes.count < info.requestLength {
                let command = try IrohaPeerNfcReaderPlanningV1.readRequestCommand(
                    for: info,
                    offset: requestBytes.count,
                    localLimits: limits
                )
                guard case .readRequest(_, _, _, let requestedLength) = command else {
                    throw IrohaPeerNfcErrorV1.stateMismatch
                }
                try actionBudget.consume()
                let response = try await requireSuccess(
                    transceive(command), operation: command.operation
                )
                guard !response.data.isEmpty,
                      response.data.count <= requestedLength,
                      response.data.count <= info.requestLength - requestBytes.count else {
                    throw IrohaPeerNfcErrorV1.invalidLength
                }
                requestBytes.append(response.data)
            }
            let request = try IrohaPeerNfcReaderPlanningV1.validateReceiveRequest(
                requestBytes,
                against: info,
                limits: limits
            )
            // Message 2 must already be durable before it can be transmitted.
            try actionBudget.consume()
            checkpoint = try await loadOrCreateDurableCheckpoint(info, request)
            let continuity = IrohaPeerNfcTwoTapReducerV1(
                checkpoint: checkpoint,
                limits: limits
            )
            guard checkpoint.profilePolicy == profilePolicy,
                  checkpoint.receiveRequest == request,
                  checkpoint.ticket == nil,
                  checkpoint.payment == nil,
                  checkpoint.durableAcknowledgement == nil else {
                throw IrohaPeerNfcErrorV1.continuityMismatch
            }
            try continuity.requireSamePeer(info)
        }

        var reducer = IrohaPeerNfcTwoTapReducerV1(
            checkpoint: checkpoint,
            limits: limits
        )
        while true {
            let statusCommand = IrohaPeerNfcReaderPlanningV1.getStatusCommand(for: info)
            try actionBudget.consume()
            let statusResponse = try await requireSuccess(
                transceive(statusCommand), operation: statusCommand.operation
            )
            let status = try IrohaPeerNfcStatusV1.decode(statusResponse.data)
            let action = try reducer.nextAction(observing: status)
            try actionBudget.consume()

            switch action {
            case .send(let command):
                let rawResponse: IrohaPeerNfcAPDUResponseV1
                do {
                    rawResponse = try await transceive(command)
                } catch {
                    if error is any IrohaPeerNfcAmbiguousResponseErrorV1,
                       case .confirmAcknowledgement = command,
                       let acknowledgement = reducer.checkpoint.durableAcknowledgement {
                        return IrohaPeerNfcReaderExchangeResultV1(
                            checkpoint: reducer.checkpoint,
                            acknowledgement: acknowledgement,
                            confirmationState: .responseUnknown
                        )
                    }
                    throw error
                }
                let response = try requireSuccess(
                    rawResponse,
                    operation: command.operation
                )
                switch command {
                case .readTicket(_, _, _, let requestedLength):
                    guard response.data.count <= requestedLength else {
                        throw IrohaPeerNfcErrorV1.invalidLength
                    }
                    _ = try reducer.consumeTicketChunk(response.data)
                case .readAcknowledgement(_, _, _, let requestedLength):
                    guard response.data.count <= requestedLength else {
                        throw IrohaPeerNfcErrorV1.invalidLength
                    }
                    _ = try reducer.consumeAcknowledgementChunk(response.data)
                default:
                    guard response.data.isEmpty else {
                        throw IrohaPeerNfcErrorV1.invalidLength
                    }
                }
                if case .confirmAcknowledgement = command {
                    guard let acknowledgement = reducer.checkpoint.durableAcknowledgement else {
                        throw IrohaPeerNfcErrorV1.acknowledgementNotDurable
                    }
                    return IrohaPeerNfcReaderExchangeResultV1(
                        checkpoint: reducer.checkpoint,
                        acknowledgement: acknowledgement
                    )
                }

            case .preparePayment(let ticketBytes):
                let ticket = try nfcDecodeMessage(
                    ticketBytes,
                    expectedProfile: profilePolicy.profile,
                    expectedKind: .ticket,
                    limits: limits
                )
                let candidate = try await preparePaymentCheckpoint(
                    reducer.checkpoint,
                    ticket
                )
                try reducer.installPreparedPaymentCheckpoint(candidate)

            case .persistAcknowledgement:
                let candidate = try reducer.acknowledgementCheckpointCandidate()
                try await updateDurableCheckpoint(candidate.encoded)
                try reducer.installPersistedCheckpoint(candidate)

            case .complete:
                guard let acknowledgement = reducer.checkpoint.durableAcknowledgement else {
                    throw IrohaPeerNfcErrorV1.acknowledgementNotDurable
                }
                return IrohaPeerNfcReaderExchangeResultV1(
                    checkpoint: reducer.checkpoint,
                    acknowledgement: acknowledgement
                )
            }
        }
    }

    private static func requireSuccess(
        _ response: IrohaPeerNfcAPDUResponseV1,
        operation: IrohaPeerNfcOperationV1
    ) throws -> IrohaPeerNfcAPDUResponseV1 {
        switch response.statusWord {
        case .success:
            return response
        case .storageFailure:
            throw IrohaPeerNfcErrorV1.peerPersistenceFailure(operation: operation)
        case .wrongLength:
            throw IrohaPeerNfcErrorV1.invalidLength
        case .securityStatusNotSatisfied:
            throw IrohaPeerNfcErrorV1.continuityMismatch
        case .conditionsNotSatisfied:
            throw IrohaPeerNfcErrorV1.peerRejected(operation: operation)
        case .wrongData:
            throw IrohaPeerNfcErrorV1.invalidAPDU
        case .notFound:
            throw IrohaPeerNfcErrorV1.invalidApplicationIdentifier
        case .instructionNotSupported:
            throw IrohaPeerNfcErrorV1.unsupportedInstruction(0)
        case .classNotSupported:
            throw IrohaPeerNfcErrorV1.invalidCommandClass(0)
        }
    }

    private static func requireEmptySuccess(
        _ response: IrohaPeerNfcAPDUResponseV1,
        operation: IrohaPeerNfcOperationV1
    ) throws {
        let successful = try requireSuccess(response, operation: operation)
        guard successful.data.isEmpty else {
            throw IrohaPeerNfcErrorV1.invalidLength
        }
    }
}

private func nfcRequireSession(_ data: Data) throws {
    guard data.count == IrohaPeerNfcV1.sessionIDBytes,
          data.contains(where: { $0 != 0 }) else {
        throw IrohaPeerNfcErrorV1.invalidSession
    }
}

private func nfcRequireHash(_ data: Data) throws {
    guard data.count == IrohaPeerNfcV1.hashBytes,
          data.contains(where: { $0 != 0 }) else {
        throw IrohaPeerNfcErrorV1.invalidHash
    }
}

private func nfcRequireChunkLimit(_ value: Int) throws {
    guard (1...IrohaPeerNfcV1.maximumChunkBytes).contains(value) else {
        throw IrohaPeerNfcErrorV1.invalidLength
    }
}

private func nfcUInt32BE(_ value: UInt32) -> Data {
    var data = Data()
    data.nfcAppendUInt32BE(value)
    return data
}

private extension Data {
    mutating func nfcAppendUInt16BE(_ value: UInt16) {
        append(UInt8(truncatingIfNeeded: value >> 8))
        append(UInt8(truncatingIfNeeded: value))
    }

    mutating func nfcAppendUInt32BE(_ value: UInt32) {
        append(UInt8(truncatingIfNeeded: value >> 24))
        append(UInt8(truncatingIfNeeded: value >> 16))
        append(UInt8(truncatingIfNeeded: value >> 8))
        append(UInt8(truncatingIfNeeded: value))
    }

    func nfcUInt16BE(at offset: Int) -> UInt16 {
        UInt16(self[offset]) << 8 | UInt16(self[offset + 1])
    }

    func nfcUInt32BE(at offset: Int) -> UInt32 {
        UInt32(self[offset]) << 24
            | UInt32(self[offset + 1]) << 16
            | UInt32(self[offset + 2]) << 8
            | UInt32(self[offset + 3])
    }
}
