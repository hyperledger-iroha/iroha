import CryptoKit
import Foundation

public struct KagemushaNFCMessages: Equatable, Sendable {
    public let readerPrompt: String
    public let cardPrompt: String
    public let success: String
    public let failure: String

    public init(
        readerPrompt: String,
        cardPrompt: String,
        success: String,
        failure: String
    ) {
        self.readerPrompt = readerPrompt
        self.cardPrompt = cardPrompt
        self.success = success
        self.failure = failure
    }

    public static let english = Self(
        readerPrompt: "Hold the top of this iPhone near the other iPhone.",
        cardPrompt: "Hold the top of both iPhones together and keep still.",
        success: "Kagemusha payment confirmed.",
        failure: "NFC exchange failed. Keep both phones together and try again."
    )
}

public struct KagemushaNFCConfiguration: Equatable, Sendable {
    public let applicationIdentifier: Data
    public let chainDiscriminant: UInt16
    public let cardSessionEnabled: Bool
    public let messages: KagemushaNFCMessages

    public init(
        chainDiscriminant: UInt16,
        applicationIdentifierHex: String =
            KagemushaPeerTransportContract.nfcApplicationIdentifierHex,
        cardSessionEnabled: Bool = false,
        messages: KagemushaNFCMessages = .english
    ) throws {
        applicationIdentifier = try KagemushaNFCProtocol.applicationIdentifier(
            hex: applicationIdentifierHex
        )
        self.chainDiscriminant = chainDiscriminant
        self.cardSessionEnabled = cardSessionEnabled
        self.messages = messages
    }

    public init(
        applicationIdentifier: Data,
        chainDiscriminant: UInt16,
        cardSessionEnabled: Bool = false,
        messages: KagemushaNFCMessages = .english
    ) throws {
        self.applicationIdentifier = try KagemushaNFCProtocol.validateApplicationIdentifier(
            applicationIdentifier
        )
        self.chainDiscriminant = chainDiscriminant
        self.cardSessionEnabled = cardSessionEnabled
        self.messages = messages
    }
}

public enum KagemushaNFCAvailabilityReason: String, Equatable, Sendable {
    case unsupportedDevice = "unsupported_device"
    case missingEntitlementOrProfile = "missing_entitlement_or_profile"
    case ineligibleDevice = "ineligible_device"
    case disabledByApplication = "disabled_by_application"
}

public struct KagemushaNFCAvailability: Equatable, Sendable {
    public let isAvailable: Bool
    public let reason: KagemushaNFCAvailabilityReason?

    public static let available = Self(isAvailable: true, reason: nil)

    public static func unavailable(_ reason: KagemushaNFCAvailabilityReason) -> Self {
        Self(isAvailable: false, reason: reason)
    }
}

public enum KagemushaNFCError: Error, Equatable, LocalizedError, Sendable {
    case unavailable
    case missingEntitlementOrProfile
    case ineligibleDevice
    case timedOut
    case invalidApplicationIdentifier
    case invalidOffset
    case invalidPayloadLength
    case invalidChunkLength
    case malformedCommand
    case invalidPeer
    case peerRejected(statusWord: UInt16?)
    case incompletePayload
    case checksumMismatch
    case acknowledgementPending
    case invalidState
    case completedSession
    case cancelled

    public var errorDescription: String? {
        switch self {
        case .unavailable:
            return "NFC is unavailable on this device."
        case .missingEntitlementOrProfile:
            return "This build is not entitled for NFC card emulation."
        case .ineligibleDevice:
            return "This device is not eligible for NFC card emulation."
        case .timedOut:
            return "The NFC exchange timed out."
        case .invalidApplicationIdentifier:
            return "The NFC application identifier is invalid."
        case .invalidOffset:
            return "The NFC APDU offset is out of bounds."
        case .invalidPayloadLength:
            return "The NFC payload length is out of bounds."
        case .invalidChunkLength:
            return "The NFC APDU chunk length is out of bounds."
        case .malformedCommand:
            return "The NFC APDU is malformed."
        case .invalidPeer:
            return "The NFC peer returned an invalid Kagemusha payload."
        case .peerRejected:
            return "The NFC peer rejected the exchange."
        case .incompletePayload:
            return "The NFC payload is incomplete."
        case .checksumMismatch:
            return "The NFC payload checksum does not match."
        case .acknowledgementPending:
            return "The cash handoff is final; its delivery receipt is pending."
        case .invalidState:
            return "The NFC exchange operation is invalid in its current state."
        case .completedSession:
            return "The NFC exchange is already complete."
        case .cancelled:
            return "The NFC exchange was cancelled."
        }
    }

    /// Whether an already-prepared payment may be retransmitted without
    /// constructing a different spend.
    public var shouldRetryPreparedPaymentTransfer: Bool {
        switch self {
        case .acknowledgementPending, .timedOut:
            return true
        case .peerRejected(let statusWord):
            return statusWord == nil || statusWord == 0x6985
        default:
            return false
        }
    }

    /// Once the wallet commits `cash_handoff_v1`, transport failure cannot
    /// roll back the spend. Preserve and retransmit only the exact committed
    /// payment while recovering the acknowledgement receipt.
    static func afterCommittedPayment(_ error: Self) -> Self {
        error == .acknowledgementPending ? error : .acknowledgementPending
    }
}

public enum KagemushaNFCEvent: Equatable, Sendable {
    case sessionStarted
    case peerConnected
    case receiveRequestRead(KagemushaRecipientReceiveOfferV2)
    case paymentPrepared(KagemushaRecursiveSpendPeerPaymentV4)
    case paymentCommitted(KagemushaRecursiveSpendPeerPaymentV4)
    case acknowledgementReady(KagemushaReceiverAcknowledgement)
    case acknowledgementRead
    case bytesTransferred(completed: Int, total: Int)
}

public struct KagemushaNFCPayloadInfo: Equatable, Sendable {
    public let transportVersion: UInt8
    public let kind: KagemushaPeerPayloadKind
    public let payloadLength: Int
    public let maximumChunkLength: Int
    public let sha256: Data
}

public enum KagemushaNFCCommand: Equatable, Sendable {
    case select
    case selectOtherApplication
    case getInfo
    case readChunk(offset: Int, requestedLength: Int)
    case writeMetadata(kind: KagemushaPeerPayloadKind, payloadLength: Int, sha256: Data)
    case writeChunk(offset: Int, bytes: Data)
    case commit
    case unsupported
    case invalid
}

/// Pure ISO-7816 framing shared by the iOS reader/card adapters and test peers.
public enum KagemushaNFCProtocol {
    /// First-release V4 carries typed Norito bytes with an explicit 32-bit
    /// offset inside every read/write command body. Retired P1/P2 16-bit
    /// offset commands are rejected as non-canonical.
    public static let rawTransportVersion: UInt8 = 4
    public static let minimumApplicationIdentifierBytes = 5
    public static let maximumApplicationIdentifierBytes = 16
    static let maximumApplicationIdentifierPaddingBytes = 8
    public static let safeChunkBytes = 220
    public static let maximumExtendedReadChunkBytes = 1_024
    public static let maximumExtendedWriteChunkBytes = 16_384
    public static let maximumPayloadBytes =
        KagemushaPeerTransportContract.maximumArchiveBytes
    static let sparseFragmentAllowance = 64
    static let maximumSparseFragmentCount =
        (maximumPayloadBytes + safeChunkBytes - 1) / safeChunkBytes
            + sparseFragmentAllowance

    public static let statusSuccess = Data([0x90, 0x00])
    public static let statusWrongData = Data([0x6A, 0x80])
    public static let statusNotFound = Data([0x6A, 0x82])
    public static let statusConditionsNotSatisfied = Data([0x69, 0x85])
    public static let statusUnsupported = Data([0x6D, 0x00])

    static let instructionClass: UInt8 = 0x80
    static let instructionGetInfo: UInt8 = 0x10
    static let instructionReadChunk: UInt8 = 0x11
    static let instructionWriteMetadata: UInt8 = 0x20
    static let instructionWriteChunk: UInt8 = 0x21
    static let instructionCommit: UInt8 = 0x22
    static let offsetBytes = 4
    static let readRequestBytes = offsetBytes + 2

    static func sparseFragmentBudget(payloadLength: Int) -> Int {
        let canonicalFragments =
            (payloadLength + safeChunkBytes - 1) / safeChunkBytes
        return min(
            canonicalFragments + sparseFragmentAllowance,
            maximumSparseFragmentCount
        )
    }

    public static var defaultApplicationIdentifier: Data {
        // The constant is compile-time controlled and tested below; force-free
        // parsing here would only make this otherwise infallible property optional.
        (try? applicationIdentifier(
            hex: KagemushaPeerTransportContract.nfcApplicationIdentifierHex
        )) ?? Data()
    }

    public static func applicationIdentifier(hex rawValue: String) throws -> Data {
        let minimumEncodedLength = minimumApplicationIdentifierBytes * 2
        let maximumEncodedLength = maximumApplicationIdentifierBytes * 2
        let rawEncodedLength = rawValue.utf8.count
        guard rawEncodedLength <= maximumEncodedLength
                + maximumApplicationIdentifierPaddingBytes,
              rawValue.unicodeScalars.allSatisfy({ scalar in
                  switch scalar.value {
                  case 9, 10, 13, 32, 48...57, 65...70, 97...102: return true
                  default: return false
                  }
              }) else {
            throw KagemushaNFCError.invalidApplicationIdentifier
        }
        let value = rawValue.trimmingCharacters(
            in: CharacterSet(charactersIn: " \t\r\n")
        )
        let encodedLength = value.utf8.count
        let validEncodedLengths = minimumEncodedLength...maximumEncodedLength
        guard validEncodedLengths.contains(encodedLength),
              rawEncodedLength - encodedLength <= maximumApplicationIdentifierPaddingBytes,
              encodedLength.isMultiple(of: 2),
              value.unicodeScalars.allSatisfy({ scalar in
                  switch scalar.value {
                  case 48...57, 65...70, 97...102: return true
                  default: return false
                  }
              }) else {
            throw KagemushaNFCError.invalidApplicationIdentifier
        }
        var output = Data()
        output.reserveCapacity(encodedLength / 2)
        var index = value.startIndex
        while index < value.endIndex {
            let next = value.index(index, offsetBy: 2)
            guard let byte = UInt8(value[index..<next], radix: 16) else {
                throw KagemushaNFCError.invalidApplicationIdentifier
            }
            output.append(byte)
            index = next
        }
        return try validateApplicationIdentifier(output)
    }

    public static func applicationIdentifierHex(_ value: Data) throws -> String {
        try validateApplicationIdentifier(value)
            .map { String(format: "%02X", $0) }
            .joined()
    }

    @discardableResult
    public static func validateApplicationIdentifier(_ value: Data) throws -> Data {
        guard (minimumApplicationIdentifierBytes...maximumApplicationIdentifierBytes)
                .contains(value.count) else {
            throw KagemushaNFCError.invalidApplicationIdentifier
        }
        return value
    }

    public static func selectApplicationCommand(
        applicationIdentifier: Data = defaultApplicationIdentifier
    ) throws -> Data {
        let identifier = try validateApplicationIdentifier(applicationIdentifier)
        return Data([0x00, 0xA4, 0x04, 0x00, UInt8(identifier.count)])
            + identifier + Data([0x00])
    }

    public static func getInfoCommand() -> Data {
        Data([
            instructionClass,
            instructionGetInfo,
            rawTransportVersion,
            0x00,
            0x00,
        ])
    }

    public static func readChunkCommand(
        offset: Int,
        length: Int = safeChunkBytes
    ) throws -> Data {
        try requireChunkLength(length, maximum: maximumExtendedReadChunkBytes)
        try requireTransferRange(
            offset: offset,
            length: length,
            maximumChunkLength: maximumExtendedReadChunkBytes
        )
        return try dataCommand(
            instruction: instructionReadChunk,
            data: uint32(offset) + uint16(length)
        )
    }

    public static func writeMetadataCommand(
        kind: KagemushaPeerPayloadKind,
        payloadBytes: Data
    ) throws -> Data {
        try requirePayloadLength(payloadBytes.count)
        let metadata = Data([rawTransportVersion, kind.rawValue])
            + uint32(payloadBytes.count)
            + sha256(payloadBytes)
        return try dataCommand(instruction: instructionWriteMetadata, data: metadata)
    }

    public static func writeChunkCommand(offset: Int, bytes: Data) throws -> Data {
        try requireChunkLength(bytes.count, maximum: maximumExtendedWriteChunkBytes)
        try requireTransferRange(
            offset: offset,
            length: bytes.count,
            maximumChunkLength: maximumExtendedWriteChunkBytes
        )
        return try dataCommand(
            instruction: instructionWriteChunk,
            data: uint32(offset) + bytes
        )
    }

    public static func commitCommand() -> Data {
        Data([instructionClass, instructionCommit, rawTransportVersion, 0, 0])
    }

    /// Builds the canonical bulk-write sequence. Full chunks are never smaller
    /// than 220 bytes; only the final chunk may be shorter.
    public static func writePayloadCommands(
        kind: KagemushaPeerPayloadKind,
        payloadBytes: Data,
        maximumChunkLength: Int = safeChunkBytes
    ) throws -> [Data] {
        try requirePayloadLength(payloadBytes.count)
        guard maximumChunkLength >= safeChunkBytes else {
            throw KagemushaNFCError.invalidChunkLength
        }
        try requireChunkLength(
            maximumChunkLength,
            maximum: maximumExtendedWriteChunkBytes
        )
        var commands = [try writeMetadataCommand(kind: kind, payloadBytes: payloadBytes)]
        for offset in stride(from: 0, to: payloadBytes.count, by: maximumChunkLength) {
            commands.append(try writeChunkCommand(
                offset: offset,
                bytes: payloadBytes.subdata(
                    in: offset..<min(offset + maximumChunkLength, payloadBytes.count)
                )
            ))
        }
        commands.append(commitCommand())
        return commands
    }

    public static func parseCommand(
        _ command: Data?,
        applicationIdentifier: Data = defaultApplicationIdentifier
    ) -> KagemushaNFCCommand {
        guard let command, command.count >= 4 else { return .invalid }
        if isSelectCommand(command, applicationIdentifier: applicationIdentifier) {
            return .select
        }
        if isSelectCommand(command) { return .selectOtherApplication }
        guard command[0] == instructionClass else { return .unsupported }
        let instruction = command[1]
        let canonicalParameters = command[2] == rawTransportVersion && command[3] == 0
        switch instruction {
        case instructionGetInfo:
            return canonicalParameters && isNoDataCommand(command) ? .getInfo : .invalid
        case instructionReadChunk:
            guard canonicalParameters,
                  let data = commandData(command),
                  data.count == readRequestBytes else { return .invalid }
            let offset = Int(data.uint32BE(at: 0))
            let length = Int(data.uint16BE(at: offsetBytes))
            guard transferRangeIsValid(
                offset: offset,
                length: length,
                maximumChunkLength: maximumExtendedReadChunkBytes
            ) else { return .invalid }
            return .readChunk(offset: offset, requestedLength: length)
        case instructionWriteMetadata:
            guard canonicalParameters,
                  let data = commandData(command),
                  data.count == 38,
                  data[0] == rawTransportVersion,
                  let kind = KagemushaPeerPayloadKind(rawValue: data[1]) else {
                return .invalid
            }
            let length = Int(data.uint32BE(at: 2))
            guard (1...maximumPayloadBytes).contains(length),
                  data[6..<38].contains(where: { $0 != 0 }) else {
                return .invalid
            }
            return .writeMetadata(
                kind: kind,
                payloadLength: length,
                sha256: data.subdata(in: 6..<38)
            )
        case instructionWriteChunk:
            guard canonicalParameters,
                  let data = commandData(command),
                  data.count > offsetBytes,
                  data.count <= offsetBytes + maximumExtendedWriteChunkBytes else {
                return .invalid
            }
            let offset = Int(data.uint32BE(at: 0))
            let chunk = data.subdata(in: offsetBytes..<data.count)
            guard transferRangeIsValid(
                offset: offset,
                length: chunk.count,
                maximumChunkLength: maximumExtendedWriteChunkBytes
            ) else { return .invalid }
            return .writeChunk(offset: offset, bytes: chunk)
        case instructionCommit:
            return canonicalParameters && isNoDataCommand(command) ? .commit : .invalid
        default:
            return .unsupported
        }
    }

    public static func encodeInfo(
        kind: KagemushaPeerPayloadKind,
        payloadBytes: Data,
        maximumChunkLength: Int = safeChunkBytes
    ) throws -> Data {
        try requirePayloadLength(payloadBytes.count)
        try requireChunkLength(
            maximumChunkLength,
            maximum: maximumExtendedReadChunkBytes
        )
        return Data([rawTransportVersion, kind.rawValue])
            + uint32(payloadBytes.count)
            + uint16(maximumChunkLength)
            + sha256(payloadBytes)
    }

    public static func decodeInfo(_ data: Data) -> KagemushaNFCPayloadInfo? {
        guard data.count == 40,
              data[0] == rawTransportVersion,
              let kind = KagemushaPeerPayloadKind(rawValue: data[1]) else {
            return nil
        }
        let length = Int(data.uint32BE(at: 2))
        let chunkLength = Int(data.uint16BE(at: 6))
        guard (1...maximumPayloadBytes).contains(length),
              (1...maximumExtendedReadChunkBytes).contains(chunkLength),
              data[8..<40].contains(where: { $0 != 0 }) else {
            return nil
        }
        return KagemushaNFCPayloadInfo(
            transportVersion: data[0],
            kind: kind,
            payloadLength: length,
            maximumChunkLength: chunkLength,
            sha256: data.subdata(in: 8..<40)
        )
    }

    public static func response(_ data: Data = Data()) -> Data {
        data + statusSuccess
    }

    public static func responseStatus(_ response: Data) -> UInt16? {
        guard response.count >= 2 else { return nil }
        return UInt16(response[response.count - 2]) << 8
            | UInt16(response[response.count - 1])
    }

    public static func responseData(_ response: Data) -> Data {
        guard response.count >= 2 else { return Data() }
        return response.subdata(in: 0..<(response.count - 2))
    }

    public static func sha256(_ data: Data) -> Data {
        Data(SHA256.hash(data: data))
    }

    private static func isSelectCommand(
        _ command: Data,
        applicationIdentifier: Data
    ) -> Bool {
        guard command.count >= 5,
              command[0] == 0,
              command[1] == 0xA4,
              command[2] == 0x04,
              command[3] == 0,
              isValidApplicationIdentifier(applicationIdentifier) else {
            return false
        }
        let count = Int(command[4])
        let end = 5 + count
        guard command.count == end || (command.count == end + 1 && command[end] == 0),
              end <= command.count else {
            return false
        }
        return command.subdata(in: 5..<end) == applicationIdentifier
    }

    private static func isSelectCommand(_ command: Data) -> Bool {
        guard command.count >= 5,
              command[0] == 0,
              command[1] == 0xA4,
              command[2] == 0x04,
              command[3] == 0 else {
            return false
        }
        let count = Int(command[4])
        let end = 5 + count
        return count > 0 && (command.count == end
            || (command.count == end + 1 && command[end] == 0))
    }

    private static func isValidApplicationIdentifier(_ value: Data) -> Bool {
        (try? validateApplicationIdentifier(value)) != nil
    }

    private static func isNoDataCommand(_ command: Data) -> Bool {
        command.count == 4 || (command.count == 5 && command[4] == 0)
    }

    private static func commandData(_ command: Data) -> Data? {
        guard command.count >= 5 else { return nil }
        if command[4] > 0 {
            let length = Int(command[4])
            guard command.count == 5 + length else { return nil }
            return command.subdata(in: 5..<command.count)
        }
        guard command.count >= 7 else { return nil }
        let length = Int(command[5]) << 8 | Int(command[6])
        guard length > 0, command.count == 7 + length else { return nil }
        return command.subdata(in: 7..<command.count)
    }

    private static func dataCommand(instruction: UInt8, data: Data) throws -> Data {
        guard !data.isEmpty, data.count <= Int(UInt16.max) else {
            throw KagemushaNFCError.invalidChunkLength
        }
        if data.count <= Int(UInt8.max) {
            return Data([
                instructionClass,
                instruction,
                rawTransportVersion,
                0,
                UInt8(data.count),
            ]) + data
        }
        return Data([
            instructionClass,
            instruction,
            rawTransportVersion,
            0,
            0,
            UInt8((data.count >> 8) & 0xFF),
            UInt8(data.count & 0xFF),
        ]) + data
    }

    private static func transferRangeIsValid(
        offset: Int,
        length: Int,
        maximumChunkLength: Int
    ) -> Bool {
        guard offset >= 0,
              (1...maximumChunkLength).contains(length),
              offset < maximumPayloadBytes else { return false }
        let (end, overflow) = offset.addingReportingOverflow(length)
        return !overflow && end <= maximumPayloadBytes
    }

    private static func requireTransferRange(
        offset: Int,
        length: Int,
        maximumChunkLength: Int
    ) throws {
        guard transferRangeIsValid(
            offset: offset,
            length: length,
            maximumChunkLength: maximumChunkLength
        ) else {
            throw KagemushaNFCError.invalidOffset
        }
    }

    private static func requirePayloadLength(_ length: Int) throws {
        guard (1...maximumPayloadBytes).contains(length) else {
            throw KagemushaNFCError.invalidPayloadLength
        }
    }

    private static func requireChunkLength(_ length: Int, maximum: Int) throws {
        guard (1...maximum).contains(length) else {
            throw KagemushaNFCError.invalidChunkLength
        }
    }

    private static func uint16(_ value: Int) -> Data {
        Data([UInt8((value >> 8) & 0xFF), UInt8(value & 0xFF)])
    }

    private static func uint32(_ value: Int) -> Data {
        Data([
            UInt8((value >> 24) & 0xFF),
            UInt8((value >> 16) & 0xFF),
            UInt8((value >> 8) & 0xFF),
            UInt8(value & 0xFF),
        ])
    }
}

public enum KagemushaNFCCardRejectionReason: Equatable, Sendable {
    case conditionsNotSatisfied
    case wrongData
    case unsupportedCommand
    case incompletePayload
    case checksumMismatch
    case invalidCommittedPayload
}

public struct KagemushaNFCCardHandleResult: Equatable, Sendable {
    public let response: Data
    public let committedPayload: KagemushaPeerPayload?
    public let acknowledgementReadRange: Range<Int>?
    public let rejectionReason: KagemushaNFCCardRejectionReason?

    public init(
        response: Data,
        committedPayload: KagemushaPeerPayload? = nil,
        acknowledgementReadRange: Range<Int>? = nil,
        rejectionReason: KagemushaNFCCardRejectionReason? = nil
    ) {
        self.response = response
        self.committedPayload = committedPayload
        self.acknowledgementReadRange = acknowledgementReadRange
        self.rejectionReason = rejectionReason
    }
}

/// Deterministic card-side state machine. It contains no CoreNFC dependency and
/// can therefore be fuzzed and reused by other Apple transports.
public final class KagemushaNFCCardStateMachine: @unchecked Sendable {
    public let applicationIdentifier: Data

    private let lock = NSLock()
    private let chainDiscriminant: UInt16
    private var currentPayload: KagemushaPeerPayload
    private var currentPayloadBytes: Data
    private var currentInfo: Data
    private var applicationSelected = false
    private var readable = true
    private var pendingWrite: KagemushaNFCPayloadAssembler?
    private var acknowledgementReadTracker: KagemushaNFCReadTracker?
    private var paymentCommitted = false
    private var completed = false

    public init(
        applicationIdentifier: Data = KagemushaNFCProtocol.defaultApplicationIdentifier,
        chainDiscriminant: UInt16,
        receiveRequest: KagemushaRecipientReceiveOfferV2
    ) throws {
        self.applicationIdentifier = try KagemushaNFCProtocol
            .validateApplicationIdentifier(applicationIdentifier)
        self.chainDiscriminant = chainDiscriminant
        currentPayload = .receiveRequest(receiveRequest)
        currentPayloadBytes = currentPayload.archive
        currentInfo = try KagemushaNFCProtocol.encodeInfo(
            kind: .receiveRequest,
            payloadBytes: currentPayloadBytes
        )
    }

    public var currentPayloadKind: KagemushaPeerPayloadKind {
        lock.withLock { currentPayload.kind }
    }

    public var hasPendingWrite: Bool { lock.withLock { pendingWrite != nil } }
    public var isReadable: Bool { lock.withLock { readable } }
    public var hasCompleted: Bool { lock.withLock { completed } }

    public func handle(_ command: Data) -> KagemushaNFCCardHandleResult {
        lock.withLock { handleLocked(command) }
    }

    public func publishAcknowledgement(
        _ acknowledgement: KagemushaReceiverAcknowledgement
    ) throws {
        try lock.withLock {
            guard !completed,
                  paymentCommitted,
                  !readable,
                  currentPayload.kind == .receiveRequest else {
                throw KagemushaNFCError.invalidState
            }
            let payload = KagemushaPeerPayload.acknowledgement(acknowledgement)
            let payloadBytes = payload.archive
            currentPayload = payload
            currentPayloadBytes = payloadBytes
            currentInfo = try KagemushaNFCProtocol.encodeInfo(
                kind: .acknowledgement,
                payloadBytes: payloadBytes
            )
            acknowledgementReadTracker = try KagemushaNFCReadTracker(
                expectedLength: payloadBytes.count
            )
            pendingWrite = nil
            readable = true
        }
    }

    @discardableResult
    public func markAcknowledgementBytesRead(_ range: Range<Int>) -> Bool {
        lock.withLock {
            guard !completed,
                  currentPayload.kind == .acknowledgement,
                  let tracker = acknowledgementReadTracker else {
                return false
            }
            guard range.lowerBound >= 0,
                  range.upperBound > range.lowerBound,
                  range.upperBound <= tracker.expectedLength else {
                return false
            }
            guard tracker.mark(
                offset: range.lowerBound,
                length: range.upperBound - range.lowerBound
            ) else {
                if tracker.isCleared {
                    acknowledgementReadTracker = nil
                    readable = false
                }
                return false
            }
            let isComplete = tracker.isComplete
            if isComplete {
                tracker.clear()
                acknowledgementReadTracker = nil
                completed = true
                readable = false
                pendingWrite = nil
            }
            return isComplete
        }
    }

    private func handleLocked(_ command: Data) -> KagemushaNFCCardHandleResult {
        switch KagemushaNFCProtocol.parseCommand(
            command,
            applicationIdentifier: applicationIdentifier
        ) {
        case .select:
            applicationSelected = true
            pendingWrite = nil
            return .init(response: KagemushaNFCProtocol.response())
        case .selectOtherApplication:
            applicationSelected = false
            pendingWrite = nil
            return .init(
                response: KagemushaNFCProtocol.statusNotFound,
                rejectionReason: .wrongData
            )
        case .getInfo:
            guard applicationSelected, readable else {
                return rejection(.conditionsNotSatisfied)
            }
            return .init(response: KagemushaNFCProtocol.response(currentInfo))
        case .readChunk(let offset, let requestedLength):
            guard applicationSelected else {
                return rejection(.conditionsNotSatisfied)
            }
            guard readable,
                  offset >= 0,
                  offset < currentPayloadBytes.count else {
                return rejection(readable ? .wrongData : .conditionsNotSatisfied)
            }
            let end = min(
                offset + min(requestedLength, KagemushaNFCProtocol.maximumExtendedReadChunkBytes),
                currentPayloadBytes.count
            )
            return .init(
                response: KagemushaNFCProtocol.response(
                    currentPayloadBytes.subdata(in: offset..<end)
                ),
                acknowledgementReadRange: currentPayload.kind == .acknowledgement
                    ? offset..<end : nil
            )
        case .writeMetadata(let kind, let length, let digest):
            guard applicationSelected,
                  !completed, readable, currentPayload.kind == .receiveRequest,
                  pendingWrite == nil else {
                return rejection(.conditionsNotSatisfied)
            }
            guard kind == .payment else { return rejection(.wrongData) }
            do {
                pendingWrite = try KagemushaNFCPayloadAssembler(
                    kind: kind,
                    expectedLength: length,
                    expectedSHA256: digest
                )
                return .init(response: KagemushaNFCProtocol.response())
            } catch {
                return rejection(.wrongData)
            }
        case .writeChunk(let offset, let bytes):
            guard applicationSelected, let pendingWrite else {
                return rejection(.conditionsNotSatisfied)
            }
            guard pendingWrite.write(offset: offset, bytes: bytes) else {
                if pendingWrite.isCleared {
                    self.pendingWrite = nil
                }
                return rejection(.wrongData)
            }
            return .init(response: KagemushaNFCProtocol.response())
        case .commit:
            guard applicationSelected, let pendingWrite else {
                return rejection(.conditionsNotSatisfied)
            }
            let bytes: Data
            do {
                bytes = try pendingWrite.commit()
            } catch KagemushaNFCError.incompletePayload {
                if pendingWrite.isCleared {
                    self.pendingWrite = nil
                }
                return rejection(.incompletePayload)
            } catch KagemushaNFCError.checksumMismatch {
                self.pendingWrite = nil
                return rejection(.checksumMismatch)
            } catch {
                self.pendingWrite = nil
                return rejection(.wrongData)
            }
            guard let payload = try? KagemushaPeerPayload.decode(
                      archive: bytes,
                      kind: .payment,
                      chainDiscriminant: chainDiscriminant
                  ),
                  case .payment = payload else {
                self.pendingWrite = nil
                return rejection(.invalidCommittedPayload)
            }
            self.pendingWrite = nil
            readable = false
            paymentCommitted = true
            return .init(
                response: KagemushaNFCProtocol.response(),
                committedPayload: payload
            )
        case .unsupported:
            return rejection(.unsupportedCommand)
        case .invalid:
            return rejection(.wrongData)
        }
    }

    private func rejection(
        _ reason: KagemushaNFCCardRejectionReason
    ) -> KagemushaNFCCardHandleResult {
        let response: Data
        switch reason {
        case .conditionsNotSatisfied:
            response = KagemushaNFCProtocol.statusConditionsNotSatisfied
        case .unsupportedCommand:
            response = KagemushaNFCProtocol.statusUnsupported
        default:
            response = KagemushaNFCProtocol.statusWrongData
        }
        return .init(response: response, rejectionReason: reason)
    }
}

final class KagemushaNFCPayloadAssembler {
    let kind: KagemushaPeerPayloadKind
    let expectedLength: Int
    private(set) var expectedSHA256: Data
    private let sparseBytes = KagemushaNFCSparseByteStore()
    private var covered = IndexSet()
    // The canonical 220-byte writer needs ceil(length / 220) fragments. A
    // fixed allowance admits modest overlap/out-of-order splitting without
    // allowing attacker-selected one-byte writes to create unbounded nodes.
    private let fragmentBudget: Int
    private var cleared = false

    init(
        kind: KagemushaPeerPayloadKind,
        expectedLength: Int,
        expectedSHA256: Data
    ) throws {
        guard (1...KagemushaNFCProtocol.maximumPayloadBytes).contains(expectedLength),
              expectedSHA256.count == 32,
              expectedSHA256.contains(where: { $0 != 0 }) else {
            throw KagemushaNFCError.invalidPayloadLength
        }
        self.kind = kind
        self.expectedLength = expectedLength
        self.expectedSHA256 = Data(expectedSHA256)
        fragmentBudget = KagemushaNFCProtocol.sparseFragmentBudget(
            payloadLength: expectedLength
        )
    }

    /// Unique payload bytes retained by this assembler.
    ///
    /// Metadata construction leaves this at zero even for the 32 MiB protocol
    /// maximum. The complete payload is allocated only after exact coverage is
    /// established by `commit()`.
    var bufferedByteCount: Int { sparseBytes.bufferedByteCount }
    var isCleared: Bool { cleared }

    var isComplete: Bool {
        !cleared
            && bufferedByteCount == expectedLength
            && covered.count == expectedLength
            && covered.first == 0
            && covered.last == expectedLength - 1
    }

    @discardableResult
    func write(offset: Int, bytes chunk: Data) -> Bool {
        guard !cleared,
              offset >= 0,
              offset < expectedLength,
              !chunk.isEmpty,
              chunk.count <= KagemushaNFCProtocol.maximumExtendedWriteChunkBytes,
              chunk.count <= expectedLength - offset else {
            return false
        }

        let end = offset + chunk.count
        guard let overlaps = sparseBytes.matchingSegments(
            overlapping: offset..<end,
            incoming: chunk,
            writeOffset: offset
        ) else { return false }

        var proposedFragmentCount = 0
        var budgetCursor = offset
        for segment in overlaps {
            if budgetCursor < segment.offset {
                proposedFragmentCount += 1
            }
            budgetCursor = max(budgetCursor, min(segment.end, end))
        }
        if budgetCursor < end {
            proposedFragmentCount += 1
        }
        if sparseBytes.fragmentCount + proposedFragmentCount > fragmentBudget {
            clear()
            return false
        }

        // Prepare every uncovered fragment before mutating the assembler. The
        // ordered coverage set coalesces adjacency while the immutable byte
        // fragments avoid repeatedly copying an ever-growing full segment.
        var additions: [(offset: Int, bytes: Data)] = []
        var cursor = offset
        for segment in overlaps {
            if cursor < segment.offset {
                let gapEnd = min(segment.offset, end)
                additions.append((
                    cursor,
                    copyChunkRange(chunk, absoluteRange: cursor..<gapEnd, writeOffset: offset)
                ))
            }
            cursor = max(cursor, min(segment.end, end))
        }
        if cursor < end {
            additions.append((
                cursor,
                copyChunkRange(chunk, absoluteRange: cursor..<end, writeOffset: offset)
            ))
        }
        guard !additions.isEmpty else {
            return true
        }
        for addition in additions {
            sparseBytes.insert(offset: addition.offset, bytes: addition.bytes)
        }
        covered.insert(integersIn: offset..<end)
        return true
    }

    func commit() throws -> Data {
        guard !cleared else {
            throw KagemushaNFCError.invalidState
        }
        guard isComplete else {
            throw KagemushaNFCError.incompletePayload
        }
        var bytes = Data()
        var succeeded = false
        defer {
            if !succeeded {
                bytes.resetBytes(in: bytes.startIndex..<bytes.endIndex)
            }
            // Exact coverage makes commit a one-shot operation. Both success
            // and terminal validation failure consume and zeroize the state.
            clear()
        }
        bytes.reserveCapacity(expectedLength)
        sparseBytes.appendAll(to: &bytes)
        guard bytes.count == expectedLength else {
            throw KagemushaNFCError.incompletePayload
        }
        guard KagemushaNFCProtocol.sha256(bytes) == expectedSHA256 else {
            throw KagemushaNFCError.checksumMismatch
        }
        succeeded = true
        return bytes
    }

    /// Zeroizes all retained payload and digest bytes and makes the assembler empty.
    func clear() {
        sparseBytes.clear()
        covered.removeAll()
        expectedSHA256.resetBytes(in: expectedSHA256.startIndex..<expectedSHA256.endIndex)
        cleared = true
    }

    deinit {
        clear()
    }

    private func copyChunkRange(
        _ chunk: Data,
        absoluteRange: Range<Int>,
        writeOffset: Int
    ) -> Data {
        let lower = chunk.index(
            chunk.startIndex,
            offsetBy: absoluteRange.lowerBound - writeOffset
        )
        let upper = chunk.index(
            chunk.startIndex,
            offsetBy: absoluteRange.upperBound - writeOffset
        )
        return chunk.subdata(in: lower..<upper)
    }
}

final class KagemushaNFCReadTracker {
    let expectedLength: Int
    private var read = IndexSet()
    private let rangeBudget: Int
    private(set) var isCleared = false

    init(expectedLength: Int) throws {
        guard (1...KagemushaNFCProtocol.maximumPayloadBytes).contains(expectedLength) else {
            throw KagemushaNFCError.invalidPayloadLength
        }
        self.expectedLength = expectedLength
        rangeBudget = KagemushaNFCProtocol.sparseFragmentBudget(
            payloadLength: expectedLength
        )
    }

    var isComplete: Bool {
        !isCleared
            && read.count == expectedLength
            && read.first == 0
            && read.last == expectedLength - 1
    }

    @discardableResult
    func mark(offset: Int, length: Int) -> Bool {
        guard !isCleared,
              offset >= 0, offset < expectedLength,
              length > 0, length <= expectedLength - offset else {
            return false
        }
        read.insert(integersIn: offset..<(offset + length))
        if read.rangeView.count > rangeBudget {
            clear()
            return false
        }
        return true
    }

    func clear() {
        read = IndexSet()
        isCleared = true
    }
}

private struct KagemushaNFCStoredByteSegment {
    let offset: Int
    let end: Int
}

/// AVL-ordered sparse payload fragments. Coverage coalescing is kept separate
/// so adjacent writes never require copying bytes already accepted.
private final class KagemushaNFCSparseByteStore {
    private final class Node {
        let offset: Int
        var bytes: Data
        var left: Node?
        var right: Node?
        var height = 1
        var maximumEnd: Int

        init(offset: Int, bytes: Data) {
            self.offset = offset
            self.bytes = bytes
            maximumEnd = offset + bytes.count
        }

        var end: Int { offset + bytes.count }
    }

    private var root: Node?
    private(set) var bufferedByteCount = 0
    private(set) var fragmentCount = 0

    /// Returns non-owning segment metadata only after validating every
    /// overlapping byte. Keeping all `Data` access inside this store ensures a
    /// terminal `clear()` can uniquely zeroize each node buffer.
    func matchingSegments(
        overlapping range: Range<Int>,
        incoming: Data,
        writeOffset: Int
    ) -> [KagemushaNFCStoredByteSegment]? {
        var result: [KagemushaNFCStoredByteSegment] = []
        return collect(
            root,
            overlapping: range,
            incoming: incoming,
            writeOffset: writeOffset,
            into: &result
        ) ? result : nil
    }

    func insert(offset: Int, bytes: Data) {
        precondition(!bytes.isEmpty)
        root = insert(root, offset: offset, bytes: bytes)
        bufferedByteCount += bytes.count
        fragmentCount += 1
    }

    func appendAll(to output: inout Data) {
        append(root, to: &output)
    }

    func clear() {
        zeroize(root)
        root = nil
        bufferedByteCount = 0
        fragmentCount = 0
    }

    private func collect(
        _ node: Node?,
        overlapping range: Range<Int>,
        incoming: Data,
        writeOffset: Int,
        into result: inout [KagemushaNFCStoredByteSegment]
    ) -> Bool {
        guard let node else { return true }
        if let left = node.left, left.maximumEnd > range.lowerBound {
            guard collect(
                left,
                overlapping: range,
                incoming: incoming,
                writeOffset: writeOffset,
                into: &result
            ) else { return false }
        }
        if node.offset < range.upperBound, node.end > range.lowerBound {
            let overlapStart = max(range.lowerBound, node.offset)
            let overlapEnd = min(range.upperBound, node.end)
            for destination in overlapStart..<overlapEnd {
                let existingIndex = node.bytes.index(
                    node.bytes.startIndex,
                    offsetBy: destination - node.offset
                )
                let incomingIndex = incoming.index(
                    incoming.startIndex,
                    offsetBy: destination - writeOffset
                )
                guard node.bytes[existingIndex] == incoming[incomingIndex] else {
                    return false
                }
            }
            result.append(.init(offset: node.offset, end: node.end))
        }
        if node.offset < range.upperBound {
            guard collect(
                node.right,
                overlapping: range,
                incoming: incoming,
                writeOffset: writeOffset,
                into: &result
            ) else { return false }
        }
        return true
    }

    private func insert(_ node: Node?, offset: Int, bytes: Data) -> Node {
        guard let node else { return Node(offset: offset, bytes: bytes) }
        if offset < node.offset {
            node.left = insert(node.left, offset: offset, bytes: bytes)
        } else {
            precondition(offset != node.offset)
            node.right = insert(node.right, offset: offset, bytes: bytes)
        }
        refresh(node)
        return rebalance(node)
    }

    private func append(_ node: Node?, to output: inout Data) {
        guard let node else { return }
        append(node.left, to: &output)
        output.append(node.bytes)
        append(node.right, to: &output)
    }

    private func zeroize(_ node: Node?) {
        guard let node else { return }
        zeroize(node.left)
        zeroize(node.right)
        node.bytes.resetBytes(in: node.bytes.startIndex..<node.bytes.endIndex)
        node.left = nil
        node.right = nil
    }

    private func refresh(_ node: Node) {
        node.height = max(height(node.left), height(node.right)) + 1
        node.maximumEnd = max(
            node.end,
            max(
                node.left?.maximumEnd ?? node.end,
                node.right?.maximumEnd ?? node.end
            )
        )
    }

    private func rebalance(_ node: Node) -> Node {
        let balance = height(node.left) - height(node.right)
        if balance > 1 {
            if let left = node.left, height(left.left) < height(left.right) {
                node.left = rotateLeft(left)
            }
            return rotateRight(node)
        }
        if balance < -1 {
            if let right = node.right, height(right.right) < height(right.left) {
                node.right = rotateRight(right)
            }
            return rotateLeft(node)
        }
        return node
    }

    private func rotateLeft(_ node: Node) -> Node {
        guard let pivot = node.right else { return node }
        node.right = pivot.left
        pivot.left = node
        refresh(node)
        refresh(pivot)
        return pivot
    }

    private func rotateRight(_ node: Node) -> Node {
        guard let pivot = node.left else { return node }
        node.left = pivot.right
        pivot.right = node
        refresh(node)
        refresh(pivot)
        return pivot
    }

    private func height(_ node: Node?) -> Int {
        node?.height ?? 0
    }
}

private extension NSLock {
    func withLock<T>(_ body: () throws -> T) rethrows -> T {
        lock()
        defer { unlock() }
        return try body()
    }
}

private extension Data {
    func uint16BE(at offset: Int) -> UInt16 {
        UInt16(self[offset]) << 8 | UInt16(self[offset + 1])
    }

    func uint32BE(at offset: Int) -> UInt32 {
        UInt32(self[offset]) << 24
            | UInt32(self[offset + 1]) << 16
            | UInt32(self[offset + 2]) << 8
            | UInt32(self[offset + 3])
    }
}

#if os(iOS) && canImport(CoreNFC)
@preconcurrency import CoreNFC

/// iOS ISO-7816 reader adapter. The app supplies all localized messages and
/// the closure that constructs the payment from the typed receive request.
public final class KagemushaNFCReader: NSObject, @unchecked Sendable {
    public static var isAvailable: Bool { NFCTagReaderSession.readingAvailable }

    private let configuration: KagemushaNFCConfiguration
    private let lock = NSLock()
    private var connection: KagemushaNFCTagConnection?

    public init(configuration: KagemushaNFCConfiguration) {
        self.configuration = configuration
    }

    public func cancel() {
        lock.withLock { connection?.cancel(message: configuration.messages.failure) }
    }

    public func sendPayment(
        onEvent: @escaping @Sendable (KagemushaNFCEvent) -> Void = { _ in },
        commitPaymentForHandoff: @escaping @Sendable (
            KagemushaRecipientReceiveOfferV2
        ) async throws -> KagemushaRecursiveSpendPeerPaymentV4
    ) async throws -> KagemushaPeerSendResult {
        let connection = KagemushaNFCTagConnection()
        lock.withLock { self.connection = connection }
        var committedPayment: KagemushaRecursiveSpendPeerPaymentV4?
        defer { lock.withLock { self.connection = nil } }
        do {
            let tag = try await connection.connect(
                alertMessage: configuration.messages.readerPrompt
            )
            onEvent(.sessionStarted)
            try await sendRaw(
                try KagemushaNFCProtocol.selectApplicationCommand(
                    applicationIdentifier: configuration.applicationIdentifier
                ),
                to: tag
            )
            onEvent(.peerConnected)
            let requestPayload = try await readPayload(
                expectedKind: .receiveRequest,
                from: tag,
                onEvent: onEvent
            )
            guard case .receiveRequest(let request) = requestPayload else {
                throw KagemushaNFCError.invalidPeer
            }
            onEvent(.receiveRequestRead(request))
            // This closure must atomically consume the parents and durably
            // bind/sign the exact payment before returning. From this point the
            // cash handoff is final even when no transport byte is delivered.
            let payment = try await commitPaymentForHandoff(request)
            committedPayment = payment
            onEvent(.paymentPrepared(payment))
            onEvent(.paymentCommitted(payment))
            try await writePayload(
                .payment(payment),
                to: tag,
                onEvent: onEvent,
                onCommitAttempted: {}
            )
            let acknowledgementPayload = try await readPayload(
                expectedKind: .acknowledgement,
                from: tag,
                onEvent: onEvent
            )
            guard case .acknowledgement(let acknowledgement) = acknowledgementPayload else {
                throw KagemushaNFCError.invalidPeer
            }
            onEvent(.acknowledgementReady(acknowledgement))
            connection.finish(message: configuration.messages.success)
            return KagemushaPeerSendResult(
                payment: payment,
                acknowledgement: acknowledgement
            )
        } catch let error as KagemushaNFCError {
            connection.cancel(message: configuration.messages.failure)
            if committedPayment != nil {
                throw KagemushaNFCError.afterCommittedPayment(error)
            }
            throw error
        } catch {
            connection.cancel(message: configuration.messages.failure)
            if committedPayment != nil {
                throw KagemushaNFCError.acknowledgementPending
            }
            throw KagemushaNFCError.invalidPeer
        }
    }

    private func readPayload(
        expectedKind: KagemushaPeerPayloadKind,
        from tag: NFCISO7816Tag,
        onEvent: @escaping @Sendable (KagemushaNFCEvent) -> Void
    ) async throws -> KagemushaPeerPayload {
        let infoBytes = try await sendRaw(KagemushaNFCProtocol.getInfoCommand(), to: tag)
        guard let info = KagemushaNFCProtocol.decodeInfo(infoBytes),
              info.kind == expectedKind else {
            throw KagemushaNFCError.invalidPeer
        }
        var bytes = Data()
        while bytes.count < info.payloadLength {
            let requested = min(
                info.maximumChunkLength,
                info.payloadLength - bytes.count
            )
            let chunk = try await sendRaw(
                try KagemushaNFCProtocol.readChunkCommand(
                    offset: bytes.count,
                    length: requested
                ),
                to: tag
            )
            guard chunk.count == requested else { throw KagemushaNFCError.invalidPeer }
            bytes.append(chunk)
            onEvent(.bytesTransferred(completed: bytes.count, total: info.payloadLength))
        }
        guard info.transportVersion == KagemushaNFCProtocol.rawTransportVersion,
              KagemushaNFCProtocol.sha256(bytes) == info.sha256 else {
            throw KagemushaNFCError.checksumMismatch
        }
        do {
            return try KagemushaPeerPayload.decode(
                archive: bytes,
                kind: expectedKind,
                chainDiscriminant: configuration.chainDiscriminant
            )
        } catch {
            throw KagemushaNFCError.invalidPeer
        }
    }

    private func writePayload(
        _ payload: KagemushaPeerPayload,
        to tag: NFCISO7816Tag,
        onEvent: @escaping @Sendable (KagemushaNFCEvent) -> Void,
        onCommitAttempted: () -> Void
    ) async throws {
        let bytes = payload.archive
        let commands = try KagemushaNFCProtocol.writePayloadCommands(
            kind: payload.kind,
            payloadBytes: bytes
        )
        var transferred = 0
        for (index, command) in commands.enumerated() {
            if index == commands.count - 1 { onCommitAttempted() }
            _ = try await sendRaw(command, to: tag)
            if index > 0, index < commands.count - 1 {
                transferred = min(
                    bytes.count,
                    transferred + KagemushaNFCProtocol.safeChunkBytes
                )
                onEvent(.bytesTransferred(completed: transferred, total: bytes.count))
            }
        }
    }

    @discardableResult
    private func sendRaw(_ bytes: Data, to tag: NFCISO7816Tag) async throws -> Data {
        guard let apdu = NFCISO7816APDU(data: bytes) else {
            throw KagemushaNFCError.malformedCommand
        }
        let response: NFCISO7816ResponseAPDU = try await tag.sendCommand(apdu: apdu)
        let status = UInt16(response.statusWord1) << 8 | UInt16(response.statusWord2)
        guard status == 0x9000 else {
            throw KagemushaNFCError.peerRejected(statusWord: status)
        }
        return response.payload ?? Data()
    }
}

private final class KagemushaNFCTagConnection: NSObject, @unchecked Sendable,
    NFCTagReaderSessionDelegate
{
    private let lock = NSLock()
    private var continuation: CheckedContinuation<NFCISO7816Tag, Error>?
    private var session: NFCTagReaderSession?

    func connect(alertMessage: String) async throws -> NFCISO7816Tag {
        guard NFCTagReaderSession.readingAvailable else {
            throw KagemushaNFCError.unavailable
        }
        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                lock.withLock {
                    self.continuation = continuation
                    guard let session = NFCTagReaderSession(
                        pollingOption: [.iso14443],
                        delegate: self,
                        queue: nil
                    ) else {
                        self.continuation = nil
                        continuation.resume(throwing: KagemushaNFCError.unavailable)
                        return
                    }
                    session.alertMessage = alertMessage
                    self.session = session
                    session.begin()
                }
            }
        } onCancel: {
            self.cancel(message: "")
        }
    }

    func finish(message: String) {
        lock.withLock {
            session?.alertMessage = message
            session?.invalidate()
            session = nil
        }
    }

    func cancel(message: String) {
        lock.withLock {
            if message.isEmpty { session?.invalidate() }
            else { session?.invalidate(errorMessage: message) }
            session = nil
            continuation?.resume(throwing: KagemushaNFCError.cancelled)
            continuation = nil
        }
    }

    func tagReaderSessionDidBecomeActive(_ session: NFCTagReaderSession) {}

    func tagReaderSession(
        _ session: NFCTagReaderSession,
        didInvalidateWithError error: Error
    ) {
        let continuation = lock.withLock { () -> CheckedContinuation<NFCISO7816Tag, Error>? in
            let value = self.continuation
            self.continuation = nil
            self.session = nil
            return value
        }
        continuation?.resume(throwing: error)
    }

    func tagReaderSession(_ session: NFCTagReaderSession, didDetect tags: [NFCTag]) {
        guard tags.count == 1, case .iso7816(let tag) = tags[0] else {
            session.restartPolling()
            return
        }
        let tagBox = KagemushaUncheckedISO7816Tag(tag)
        session.connect(to: tags[0]) { [weak self] error in
            guard let self else { return }
            if error != nil {
                self.restartPolling()
                return
            }
            let continuation = self.lock.withLock {
                let value = self.continuation
                self.continuation = nil
                return value
            }
            continuation?.resume(returning: tagBox.value)
        }
    }

    private func restartPolling() {
        lock.withLock { session?.restartPolling() }
    }
}

private final class KagemushaUncheckedISO7816Tag: @unchecked Sendable {
    let value: NFCISO7816Tag
    init(_ value: NFCISO7816Tag) { self.value = value }
}

/// iOS 17.4+ card-emulation adapter. Availability is controlled explicitly by
/// `KagemushaNFCConfiguration`; the SDK never reads application build flags or
/// bundle metadata.
public final class KagemushaNFCCardSession: @unchecked Sendable {
    private let configuration: KagemushaNFCConfiguration
    private let lock = NSLock()
    private var runtime: AnyObject?

    public init(configuration: KagemushaNFCConfiguration) {
        self.configuration = configuration
    }

    public static func availability(
        configuration: KagemushaNFCConfiguration
    ) async -> KagemushaNFCAvailability {
        guard configuration.cardSessionEnabled else {
            return .unavailable(.disabledByApplication)
        }
        guard #available(iOS 17.4, *), CardSession.isSupported else {
            return .unavailable(.unsupportedDevice)
        }
        return await CardSession.isEligible
            ? .available : .unavailable(.ineligibleDevice)
    }

    public func cancel() {
        if #available(iOS 17.4, *),
           let runtime = lock.withLock({ runtime }) as? KagemushaNFCCardRuntime {
            runtime.cancel()
        }
        lock.withLock { runtime = nil }
    }

    public func receivePayment(
        receiveRequest: KagemushaRecipientReceiveOfferV2,
        onEvent: @escaping @Sendable (KagemushaNFCEvent) -> Void = { _ in },
        acceptPayment: @escaping @Sendable (
            KagemushaRecursiveSpendPeerPaymentV4
        ) async throws -> KagemushaReceiverAcknowledgement
    ) async throws -> KagemushaRecursiveSpendPeerPaymentV4 {
        guard configuration.cardSessionEnabled else {
            throw KagemushaNFCError.missingEntitlementOrProfile
        }
        guard #available(iOS 17.4, *) else { throw KagemushaNFCError.unavailable }
        let runtime = try KagemushaNFCCardRuntime(
            configuration: configuration,
            receiveRequest: receiveRequest,
            onEvent: onEvent,
            acceptPayment: acceptPayment
        )
        lock.withLock { self.runtime = runtime }
        defer { lock.withLock { self.runtime = nil } }
        return try await runtime.run()
    }
}

@available(iOS 17.4, *)
private final class KagemushaNFCCardRuntime {
    private let configuration: KagemushaNFCConfiguration
    private let stateMachine: KagemushaNFCCardStateMachine
    private let onEvent: @Sendable (KagemushaNFCEvent) -> Void
    private let acceptPayment: @Sendable (
        KagemushaRecursiveSpendPeerPaymentV4
    ) async throws -> KagemushaReceiverAcknowledgement
    private var session: CardSession?
    private var acceptedPayment: KagemushaRecursiveSpendPeerPaymentV4?

    init(
        configuration: KagemushaNFCConfiguration,
        receiveRequest: KagemushaRecipientReceiveOfferV2,
        onEvent: @escaping @Sendable (KagemushaNFCEvent) -> Void,
        acceptPayment: @escaping @Sendable (
            KagemushaRecursiveSpendPeerPaymentV4
        ) async throws -> KagemushaReceiverAcknowledgement
    ) throws {
        self.configuration = configuration
        stateMachine = try KagemushaNFCCardStateMachine(
            applicationIdentifier: configuration.applicationIdentifier,
            chainDiscriminant: configuration.chainDiscriminant,
            receiveRequest: receiveRequest
        )
        self.onEvent = onEvent
        self.acceptPayment = acceptPayment
    }

    func cancel() { session?.invalidate() }

    func run() async throws -> KagemushaRecursiveSpendPeerPaymentV4 {
        guard CardSession.isSupported else { throw KagemushaNFCError.unavailable }
        guard await CardSession.isEligible else { throw KagemushaNFCError.ineligibleDevice }
        let session: CardSession
        do { session = try await CardSession() }
        catch { throw KagemushaNFCError.missingEntitlementOrProfile }
        self.session = session
        session.alertMessage = configuration.messages.cardPrompt
        for try await event in session.eventStream {
            switch event {
            case .sessionStarted:
                try await session.startEmulation()
                onEvent(.sessionStarted)
            case .readerDetected:
                onEvent(.peerConnected)
            case .received(let apdu):
                try await respond(to: apdu)
                if stateMachine.hasCompleted, let acceptedPayment {
                    onEvent(.acknowledgementRead)
                    session.invalidate()
                    return acceptedPayment
                }
            case .readerDeselected:
                break
            case .sessionInvalidated(let reason):
                throw map(reason)
            @unknown default:
                throw KagemushaNFCError.invalidPeer
            }
        }
        throw KagemushaNFCError.cancelled
    }

    private func respond(to apdu: CardSession.APDU) async throws {
        let result = stateMachine.handle(apdu.payload)
        if case .payment(let payment) = result.committedPayload {
            do {
                let acknowledgement = try await acceptPayment(payment)
                try stateMachine.publishAcknowledgement(acknowledgement)
                acceptedPayment = payment
                onEvent(.paymentCommitted(payment))
                onEvent(.acknowledgementReady(acknowledgement))
            } catch {
                try await apdu.respond(response: KagemushaNFCProtocol.statusConditionsNotSatisfied)
                throw error
            }
        }
        try await apdu.respond(response: result.response)
        if let range = result.acknowledgementReadRange {
            _ = stateMachine.markAcknowledgementBytesRead(range)
        }
    }

    private func map(_ error: CardSession.Error) -> KagemushaNFCError {
        switch error {
        case .userInvalidated, .invalidated, .emulationStopped:
            return .cancelled
        case .maxSessionDurationReached:
            return .timedOut
        case .accessNotAccepted:
            return .missingEntitlementOrProfile
        case .systemEligibilityFailed:
            return .ineligibleDevice
        case .systemNotAvailable, .radioDisabled:
            return .unavailable
        case .transmissionError:
            return acceptedPayment == nil ? .invalidPeer : .acknowledgementPending
        @unknown default:
            return .unavailable
        }
    }
}
#else
public final class KagemushaNFCReader: @unchecked Sendable {
    public static var isAvailable: Bool { false }
    public init(configuration: KagemushaNFCConfiguration) { _ = configuration }
    public func cancel() {}
    public func sendPayment(
        onEvent: @escaping @Sendable (KagemushaNFCEvent) -> Void = { _ in },
        commitPaymentForHandoff: @escaping @Sendable (
            KagemushaRecipientReceiveOfferV2
        ) async throws -> KagemushaRecursiveSpendPeerPaymentV4
    ) async throws -> KagemushaPeerSendResult {
        _ = onEvent
        _ = commitPaymentForHandoff
        throw KagemushaNFCError.unavailable
    }
}

public final class KagemushaNFCCardSession: @unchecked Sendable {
    public init(configuration: KagemushaNFCConfiguration) { _ = configuration }
    public static func availability(
        configuration: KagemushaNFCConfiguration
    ) async -> KagemushaNFCAvailability {
        _ = configuration
        return .unavailable(.unsupportedDevice)
    }
    public func cancel() {}
    public func receivePayment(
        receiveRequest: KagemushaRecipientReceiveOfferV2,
        onEvent: @escaping @Sendable (KagemushaNFCEvent) -> Void = { _ in },
        acceptPayment: @escaping @Sendable (
            KagemushaRecursiveSpendPeerPaymentV4
        ) async throws -> KagemushaReceiverAcknowledgement
    ) async throws -> KagemushaRecursiveSpendPeerPaymentV4 {
        _ = receiveRequest
        _ = onEvent
        _ = acceptPayment
        throw KagemushaNFCError.unavailable
    }
}
#endif
