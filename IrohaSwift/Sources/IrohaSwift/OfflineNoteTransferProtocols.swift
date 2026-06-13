import CryptoKit
import Foundation

public enum OfflineNoteNfcPayloadKind: UInt8, CaseIterable, Sendable {
    case receiveRequest = 1
    case paymentToken = 2
    case receiptAck = 3

    public var qrPayloadKind: OfflineQrPayloadKind {
        switch self {
        case .receiveRequest:
            return .offlineReceiveRequest
        case .paymentToken:
            return .offlinePaymentToken
        case .receiptAck:
            return .offlineReceiptAck
        }
    }
}

public struct OfflineNoteNfcPayloadInfo: Equatable, Sendable {
    public let kind: OfflineNoteNfcPayloadKind
    public let payloadLength: Int
    public let maxChunkLength: Int
    public let sha256: Data
}

public enum OfflineNoteNfcCommand: Equatable, Sendable {
    case select
    case getInfo
    case readChunk(offset: Int, requestedLength: Int)
    case writeMeta(kind: OfflineNoteNfcPayloadKind, payloadLength: Int, sha256: Data)
    case writeChunk(offset: Int, bytes: Data)
    case commit
    case unsupported
    case invalid
}

public enum OfflineNoteNfcApduError: Error, LocalizedError, Equatable {
    case invalidOffset
    case invalidPayloadLength
    case invalidChunkLength
    case incompletePayload
    case checksumMismatch
    case completedSession

    public var errorDescription: String? {
        switch self {
        case .invalidOffset:
            return "Offline Note NFC APDU offset is out of bounds."
        case .invalidPayloadLength:
            return "Offline Note NFC payload length is out of bounds."
        case .invalidChunkLength:
            return "Offline Note NFC chunk length is out of bounds."
        case .incompletePayload:
            return "Offline Note NFC payload is incomplete."
        case .checksumMismatch:
            return "Offline Note NFC payload checksum did not match."
        case .completedSession:
            return "Offline Note NFC session is already complete."
        }
    }
}

public enum OfflineNoteNfcAidError: Error, LocalizedError, Equatable {
    case missing
    case invalidHexLength
    case invalidHexCharacter
    case invalidLength(actual: Int, minimum: Int, maximum: Int)

    public var errorDescription: String? {
        switch self {
        case .missing:
            return "Offline Note NFC AID is missing."
        case .invalidHexLength:
            return "Offline Note NFC AID hex must contain an even number of characters."
        case .invalidHexCharacter:
            return "Offline Note NFC AID hex contains non-hex characters."
        case .invalidLength(let actual, let minimum, let maximum):
            return "Offline Note NFC AID must be \(minimum)-\(maximum) bytes, got \(actual)."
        }
    }
}

public enum OfflineNoteNfcApduProtocol {
    public static let aid = Data([0xF0, 0x49, 0x52, 0x4F, 0x48, 0x41, 0x32])
    public static let aidHex = "F049524F484132"
    public static let minAidBytes = 5
    public static let maxAidBytes = 16
    public static let androidSafeChunkBytes = 240
    public static let maxExtendedReadChunkBytes = 1_024
    public static let maxExtendedWriteChunkBytes = 16_384
    public static let maxIncomingPayloadBytes = 64 * 1024

    public static let statusSuccess = Data([0x90, 0x00])
    public static let statusWrongData = Data([0x6A, 0x80])
    public static let statusNotFound = Data([0x6A, 0x82])
    public static let statusConditionsNotSatisfied = Data([0x69, 0x85])
    public static let statusUnsupported = Data([0x6D, 0x00])

    private static let cla: UInt8 = 0x80
    private static let insGetInfo: UInt8 = 0x10
    private static let insReadChunk: UInt8 = 0x11
    private static let insWriteMeta: UInt8 = 0x20
    private static let insWriteChunk: UInt8 = 0x21
    private static let insCommit: UInt8 = 0x22

    public static func selectAidAPDUData() -> Data {
        selectAidAPDUData(aid: aid)
    }

    public static func selectAidAPDUData(aid: Data) -> Data {
        precondition(isValidAid(aid), "Offline Note NFC AID must be \(minAidBytes)-\(maxAidBytes) bytes.")
        return Data([0x00, 0xA4, 0x04, 0x00, UInt8(aid.count)]) + aid + Data([0x00])
    }

    public static func aidHex(for aid: Data) -> String {
        aid.map { String(format: "%02X", $0) }.joined()
    }

    public static func validateAid(_ aid: Data) throws -> Data {
        guard aid.count >= minAidBytes, aid.count <= maxAidBytes else {
            throw OfflineNoteNfcAidError.invalidLength(
                actual: aid.count,
                minimum: minAidBytes,
                maximum: maxAidBytes
            )
        }
        return aid
    }

    public static func isValidAid(_ aid: Data) -> Bool {
        (try? validateAid(aid)) != nil
    }

    public static func aidData(hexString rawValue: String) throws -> Data {
        let trimmed = rawValue.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw OfflineNoteNfcAidError.missing
        }
        guard trimmed.allSatisfy(\.isHexDigit) else {
            throw OfflineNoteNfcAidError.invalidHexCharacter
        }
        guard trimmed.count.isMultiple(of: 2) else {
            throw OfflineNoteNfcAidError.invalidHexLength
        }
        guard let data = Data(hexString: trimmed) else {
            throw OfflineNoteNfcAidError.invalidHexCharacter
        }
        return try validateAid(data)
    }

    public static func normalizedAidHex(_ rawValue: String) throws -> String {
        aidHex(for: try aidData(hexString: rawValue))
    }

    public static func getInfoAPDUData() -> Data {
        Data([cla, insGetInfo, 0x00, 0x00, 0x00])
    }

    public static func readChunkAPDUData(
        offset: Int,
        length: Int = androidSafeChunkBytes
    ) throws -> Data {
        try requireValidOffset(offset)
        try requireChunkLength(length, maxChunkLength: maxExtendedReadChunkBytes)
        if length <= UInt8.max {
            return Data([
                cla,
                insReadChunk,
                UInt8((offset >> 8) & 0xff),
                UInt8(offset & 0xff),
                UInt8(length),
            ])
        }
        return Data([
            cla,
            insReadChunk,
            UInt8((offset >> 8) & 0xff),
            UInt8(offset & 0xff),
            0x00,
            UInt8((length >> 8) & 0xff),
            UInt8(length & 0xff),
        ])
    }

    public static func writeMetaAPDUData(
        kind: OfflineNoteNfcPayloadKind,
        payloadBytes: Data
    ) throws -> Data {
        try requirePayloadLength(payloadBytes.count)
        let meta = Data([kind.rawValue])
            + int32(payloadBytes.count)
            + sha256(payloadBytes)
        return Data([cla, insWriteMeta, 0x00, 0x00, UInt8(meta.count)]) + meta
    }

    public static func writeChunkAPDUData(offset: Int, bytes: Data) throws -> Data {
        try requireValidOffset(offset)
        try requireChunkLength(bytes.count, maxChunkLength: maxExtendedWriteChunkBytes)
        var apdu = Data()
        apdu.reserveCapacity(commandHeaderLength(forPayloadLength: bytes.count) + bytes.count)
        appendCommandHeader(&apdu, instruction: insWriteChunk, offset: offset, length: bytes.count)
        apdu.append(bytes)
        return apdu
    }

    public static func writeChunkAPDUData(
        offset: Int,
        payloadBytes: Data,
        range: Range<Data.Index>
    ) throws -> Data {
        try requireValidOffset(offset)
        try requireValidPayloadRange(range, in: payloadBytes)
        let length = payloadBytes.distance(from: range.lowerBound, to: range.upperBound)
        try requireChunkLength(length, maxChunkLength: maxExtendedWriteChunkBytes)
        var apdu = Data()
        apdu.reserveCapacity(commandHeaderLength(forPayloadLength: length) + length)
        appendCommandHeader(&apdu, instruction: insWriteChunk, offset: offset, length: length)
        apdu.append(contentsOf: payloadBytes[range])
        return apdu
    }

    public static func commitAPDUData() -> Data {
        Data([cla, insCommit, 0x00, 0x00, 0x00])
    }

    public static func writePayloadAPDUs(
        kind: OfflineNoteNfcPayloadKind,
        payloadBytes: Data,
        maxChunkLength: Int = androidSafeChunkBytes
    ) throws -> [Data] {
        try requirePayloadLength(payloadBytes.count)
        try requireChunkLength(maxChunkLength, maxChunkLength: maxExtendedWriteChunkBytes)
        var apdus = [try writeMetaAPDUData(kind: kind, payloadBytes: payloadBytes)]
        var offset = 0
        while offset < payloadBytes.count {
            let end = min(offset + maxChunkLength, payloadBytes.count)
            apdus.append(try writeChunkAPDUData(offset: offset, payloadBytes: payloadBytes, range: offset..<end))
            offset = end
        }
        apdus.append(commitAPDUData())
        return apdus
    }

    public static func readPayloadAPDUs(
        payloadLength: Int,
        maxChunkLength: Int = androidSafeChunkBytes
    ) throws -> [Data] {
        try requirePayloadLength(payloadLength)
        try requireChunkLength(maxChunkLength, maxChunkLength: maxExtendedReadChunkBytes)
        var apdus: [Data] = []
        var offset = 0
        while offset < payloadLength {
            apdus.append(try readChunkAPDUData(offset: offset, length: maxChunkLength))
            offset += maxChunkLength
        }
        return apdus
    }

    public static func parseCommand(_ apdu: Data?) -> OfflineNoteNfcCommand {
        parseCommand(apdu, aid: aid)
    }

    public static func parseCommand(_ apdu: Data?, aid: Data) -> OfflineNoteNfcCommand {
        guard let apdu, apdu.count >= 4 else { return .invalid }
        if isSelectAid(apdu, aid: aid) { return .select }
        guard apdu[apdu.startIndex] == cla else { return .unsupported }
        let instruction = apdu[apdu.startIndex + 1]
        let offset = (Int(apdu[apdu.startIndex + 2]) << 8) | Int(apdu[apdu.startIndex + 3])
        switch instruction {
        case insGetInfo:
            return offset == 0 && isNoDataAPDU(apdu) ? .getInfo : .invalid
        case insReadChunk:
            return isReadChunkAPDU(apdu)
                ? .readChunk(offset: offset, requestedLength: requestedReadChunkLength(apdu))
                : .invalid
        case insWriteMeta:
            guard offset == 0 else { return .invalid }
            guard let data = commandData(apdu) else { return .invalid }
            return parseWriteMeta(data)
        case insWriteChunk:
            guard let data = commandData(apdu),
                  !data.isEmpty,
                  data.count <= maxExtendedWriteChunkBytes else {
                return .invalid
            }
            return .writeChunk(offset: offset, bytes: data)
        case insCommit:
            return offset == 0 && isNoDataAPDU(apdu) ? .commit : .invalid
        default:
            return .unsupported
        }
    }

    public static func encodeInfo(
        kind: OfflineNoteNfcPayloadKind,
        payloadBytes: Data,
        maxChunkLength: Int = androidSafeChunkBytes
    ) throws -> Data {
        try requirePayloadLength(payloadBytes.count)
        try requireChunkLength(maxChunkLength, maxChunkLength: maxExtendedReadChunkBytes)
        return Data([kind.rawValue])
            + int32(payloadBytes.count)
            + uint16(maxChunkLength)
            + sha256(payloadBytes)
    }

    public static func decodeInfo(_ data: Data) -> OfflineNoteNfcPayloadInfo? {
        guard data.count == 39,
              let kind = OfflineNoteNfcPayloadKind(rawValue: data[data.startIndex]) else {
            return nil
        }
        let payloadLength = readInt32(data, offset: 1)
        let maxChunkLength = readUInt16(data, offset: 5)
        guard payloadLength > 0,
              payloadLength <= maxIncomingPayloadBytes,
              maxChunkLength > 0,
              maxChunkLength <= maxExtendedReadChunkBytes else {
            return nil
        }
        return OfflineNoteNfcPayloadInfo(
            kind: kind,
            payloadLength: payloadLength,
            maxChunkLength: maxChunkLength,
            sha256: data.subdata(in: 7..<39)
        )
    }

    public static func response(_ data: Data = Data()) -> Data {
        data + statusSuccess
    }

    public static func responseStatus(_ response: Data) -> UInt16? {
        guard response.count >= 2 else { return nil }
        let hi = UInt16(response[response.index(response.endIndex, offsetBy: -2)])
        let lo = UInt16(response[response.index(before: response.endIndex)])
        return (hi << 8) | lo
    }

    public static func responseData(_ response: Data) -> Data {
        guard response.count >= 2 else { return Data() }
        return response.subdata(in: response.startIndex..<response.index(response.endIndex, offsetBy: -2))
    }

    public static func sha256(_ data: Data) -> Data {
        Data(SHA256.hash(data: data))
    }

    public static func payloadDigestMatches(_ payloadBytes: Data, expectedSha256: Data) -> Bool {
        sha256(payloadBytes) == expectedSha256
    }

    public static func requestedReadChunkLength(_ apdu: Data) -> Int {
        guard apdu.count >= 5,
              apdu[apdu.startIndex] == cla,
              apdu[apdu.startIndex + 1] == insReadChunk else {
            return androidSafeChunkBytes
        }
        let length = Int(apdu[apdu.startIndex + 4])
        if length == 0, apdu.count >= 7 {
            let extendedLength = (Int(apdu[apdu.startIndex + 5]) << 8)
                | Int(apdu[apdu.startIndex + 6])
            return min(max(extendedLength, 1), maxExtendedReadChunkBytes)
        }
        return min(max(length, 1), androidSafeChunkBytes)
    }

    public static func iosFastWriteChunkLength(peerSupportsExtendedChunks: Bool) -> Int {
        peerSupportsExtendedChunks ? maxExtendedWriteChunkBytes : androidSafeChunkBytes
    }

    public static func isSelectAidAPDU(
        _ apdu: Data?,
        aid: Data = OfflineNoteNfcApduProtocol.aid
    ) -> Bool {
        guard let apdu else { return false }
        return isSelectAid(apdu, aid: aid)
    }

    private static func isSelectAid(_ apdu: Data, aid: Data) -> Bool {
        guard apdu.count >= 5,
              apdu[apdu.startIndex] == 0x00,
              apdu[apdu.startIndex + 1] == 0xA4,
              apdu[apdu.startIndex + 2] == 0x04,
              apdu[apdu.startIndex + 3] == 0x00 else {
            return false
        }
        let length = Int(apdu[apdu.startIndex + 4])
        let payloadEnd = apdu.startIndex + 5 + length
        guard apdu.endIndex == payloadEnd || apdu.endIndex == payloadEnd + 1 else { return false }
        guard apdu.endIndex == payloadEnd || apdu[payloadEnd] == 0x00 else { return false }
        return apdu.subdata(in: (apdu.startIndex + 5)..<payloadEnd) == aid
    }

    private static func commandData(_ apdu: Data) -> Data? {
        if apdu.count == 4 { return Data() }
        guard apdu.count >= 5 else { return nil }
        let length = Int(apdu[apdu.startIndex + 4])
        if length == 0 {
            if apdu.count == 5 { return Data() }
            guard apdu.count >= 7 else { return nil }
            let extendedLength = (Int(apdu[apdu.startIndex + 5]) << 8)
                | Int(apdu[apdu.startIndex + 6])
            guard extendedLength > 0,
                  apdu.count == 7 + extendedLength else {
                return nil
            }
            return apdu.subdata(in: (apdu.startIndex + 7)..<apdu.endIndex)
        }
        guard apdu.count == 5 + length else { return nil }
        return apdu.subdata(in: (apdu.startIndex + 5)..<apdu.endIndex)
    }

    private static func isNoDataAPDU(_ apdu: Data) -> Bool {
        apdu.count == 4 || (apdu.count == 5 && apdu[apdu.startIndex + 4] == 0x00)
    }

    private static func isReadChunkAPDU(_ apdu: Data) -> Bool {
        if apdu.count == 4 { return false }
        if apdu.count == 5 { return apdu[apdu.startIndex + 4] != 0x00 }
        guard apdu.count == 7,
              apdu[apdu.startIndex + 4] == 0x00 else {
            return false
        }
        let extendedLength = (Int(apdu[apdu.startIndex + 5]) << 8) | Int(apdu[apdu.startIndex + 6])
        return extendedLength > 0 && extendedLength <= maxExtendedReadChunkBytes
    }

    private static func parseWriteMeta(_ data: Data) -> OfflineNoteNfcCommand {
        guard data.count == 37,
              let kind = OfflineNoteNfcPayloadKind(rawValue: data[data.startIndex]) else {
            return .invalid
        }
        let payloadLength = readInt32(data, offset: 1)
        guard payloadLength > 0,
              payloadLength <= maxIncomingPayloadBytes else {
            return .invalid
        }
        return .writeMeta(
            kind: kind,
            payloadLength: payloadLength,
            sha256: data.subdata(in: (data.startIndex + 5)..<(data.startIndex + 37))
        )
    }

    private static func requireValidOffset(_ offset: Int) throws {
        guard (0...0xffff).contains(offset) else {
            throw OfflineNoteNfcApduError.invalidOffset
        }
    }

    private static func requirePayloadLength(_ length: Int) throws {
        guard length > 0, length <= maxIncomingPayloadBytes else {
            throw OfflineNoteNfcApduError.invalidPayloadLength
        }
    }

    private static func requireChunkLength(_ length: Int, maxChunkLength: Int) throws {
        guard length > 0, length <= maxChunkLength else {
            throw OfflineNoteNfcApduError.invalidChunkLength
        }
    }

    private static func requireValidPayloadRange(
        _ range: Range<Data.Index>,
        in payloadBytes: Data
    ) throws {
        guard range.lowerBound >= payloadBytes.startIndex,
              range.upperBound <= payloadBytes.endIndex else {
            throw OfflineNoteNfcApduError.invalidOffset
        }
    }

    private static func appendCommandHeader(
        _ data: inout Data,
        instruction: UInt8,
        offset: Int,
        length: Int
    ) {
        data.append(cla)
        data.append(instruction)
        data.append(UInt8((offset >> 8) & 0xff))
        data.append(UInt8(offset & 0xff))
        if length <= UInt8.max {
            data.append(UInt8(length))
        } else {
            data.append(0x00)
            data.append(UInt8((length >> 8) & 0xff))
            data.append(UInt8(length & 0xff))
        }
    }

    private static func commandHeaderLength(forPayloadLength length: Int) -> Int {
        length <= UInt8.max ? 5 : 7
    }

    private static func int32(_ value: Int) -> Data {
        Data([
            UInt8((value >> 24) & 0xff),
            UInt8((value >> 16) & 0xff),
            UInt8((value >> 8) & 0xff),
            UInt8(value & 0xff),
        ])
    }

    private static func uint16(_ value: Int) -> Data {
        Data([UInt8((value >> 8) & 0xff), UInt8(value & 0xff)])
    }

    private static func readInt32(_ data: Data, offset: Int) -> Int {
        (Int(data[data.startIndex + offset]) << 24)
            | (Int(data[data.startIndex + offset + 1]) << 16)
            | (Int(data[data.startIndex + offset + 2]) << 8)
            | Int(data[data.startIndex + offset + 3])
    }

    private static func readUInt16(_ data: Data, offset: Int) -> Int {
        (Int(data[data.startIndex + offset]) << 8) | Int(data[data.startIndex + offset + 1])
    }
}

public final class OfflineNoteNfcPayloadAssembler {
    public let kind: OfflineNoteNfcPayloadKind
    public let expectedLength: Int
    public let expectedSha256: Data
    private var bytes: Data
    private var written: [Bool]
    private var writtenCount = 0

    public convenience init(info: OfflineNoteNfcPayloadInfo) throws {
        try self.init(kind: info.kind, expectedLength: info.payloadLength, expectedSha256: info.sha256)
    }

    public init(kind: OfflineNoteNfcPayloadKind, expectedLength: Int, expectedSha256: Data) throws {
        guard expectedLength > 0,
              expectedLength <= OfflineNoteNfcApduProtocol.maxIncomingPayloadBytes,
              expectedSha256.count == 32 else {
            throw OfflineNoteNfcApduError.invalidPayloadLength
        }
        self.kind = kind
        self.expectedLength = expectedLength
        self.expectedSha256 = expectedSha256
        self.bytes = Data(repeating: 0, count: expectedLength)
        self.written = Array(repeating: false, count: expectedLength)
    }

    public var isComplete: Bool {
        writtenCount == expectedLength
    }

    public var writtenByteCount: Int {
        writtenCount
    }

    @discardableResult
    public func write(offset: Int, chunk: Data) -> Bool {
        guard offset >= 0,
              offset <= expectedLength,
              !chunk.isEmpty,
              chunk.count <= OfflineNoteNfcApduProtocol.maxExtendedWriteChunkBytes else {
            return false
        }
        guard chunk.count <= expectedLength - offset else { return false }
        let end = offset + chunk.count
        for index in 0..<chunk.count {
            let writeIndex = offset + index
            if written[writeIndex],
               bytes[bytes.startIndex + writeIndex] != chunk[chunk.startIndex + index] {
                return false
            }
        }
        bytes.replaceSubrange(offset..<end, with: chunk)
        for index in offset..<end where !written[index] {
            written[index] = true
            writtenCount += 1
        }
        return true
    }

    public func commit() throws -> Data {
        guard isComplete else {
            throw OfflineNoteNfcApduError.incompletePayload
        }
        guard OfflineNoteNfcApduProtocol.payloadDigestMatches(bytes, expectedSha256: expectedSha256) else {
            throw OfflineNoteNfcApduError.checksumMismatch
        }
        return bytes
    }
}

public final class OfflineNoteNfcPayloadReadTracker {
    public let expectedLength: Int
    private var read: [Bool]
    private var readCount = 0

    public init(expectedLength: Int) throws {
        guard expectedLength > 0,
              expectedLength <= OfflineNoteNfcApduProtocol.maxIncomingPayloadBytes else {
            throw OfflineNoteNfcApduError.invalidPayloadLength
        }
        self.expectedLength = expectedLength
        self.read = Array(repeating: false, count: expectedLength)
    }

    public var isComplete: Bool {
        readCount == expectedLength
    }

    public var readByteCount: Int {
        readCount
    }

    @discardableResult
    public func markRead(offset: Int, length: Int) -> Bool {
        guard offset >= 0,
              offset < expectedLength,
              length > 0,
              length <= OfflineNoteNfcApduProtocol.maxExtendedReadChunkBytes,
              length <= expectedLength - offset else {
            return false
        }
        let end = offset + length
        for index in offset..<end where !read[index] {
            read[index] = true
            readCount += 1
        }
        return true
    }
}

public enum OfflineNoteNfcCardIncomingWriteDecision: Equatable, Sendable {
    case accept
    case rejectNotReady
    case rejectWrongKind
}

public enum OfflineNoteNfcCardSessionWritePolicy {
    public static func decision(
        currentKind: OfflineNoteNfcPayloadKind,
        incomingKind: OfflineNoteNfcPayloadKind,
        hasPendingWrite: Bool,
        readable: Bool,
        didComplete: Bool
    ) -> OfflineNoteNfcCardIncomingWriteDecision {
        guard !didComplete, !hasPendingWrite, readable, currentKind == .receiveRequest else {
            return .rejectNotReady
        }
        guard incomingKind == .paymentToken else {
            return .rejectWrongKind
        }
        return .accept
    }

    public static func shouldAcceptIncomingWrite(
        currentKind: OfflineNoteNfcPayloadKind,
        incomingKind: OfflineNoteNfcPayloadKind,
        hasPendingWrite: Bool,
        readable: Bool,
        didComplete: Bool
    ) -> Bool {
        decision(
            currentKind: currentKind,
            incomingKind: incomingKind,
            hasPendingWrite: hasPendingWrite,
            readable: readable,
            didComplete: didComplete
        ) == .accept
    }
}

public enum OfflineNoteNfcReaderExchangePolicy {
    public static func shouldBeginTagExchange(
        hasActiveSession: Bool,
        didComplete: Bool,
        isExchangeInFlight: Bool
    ) -> Bool {
        hasActiveSession && !didComplete && !isExchangeInFlight
    }
}

public enum OfflineNoteNfcReaderPayloadReadPolicy {
    public static func acceptsChunkResponse(
        responseLength: Int,
        requestedLength: Int,
        remainingLength: Int
    ) -> Bool {
        guard responseLength > 0,
              requestedLength > 0,
              requestedLength <= OfflineNoteNfcApduProtocol.maxExtendedReadChunkBytes,
              remainingLength > 0 else {
            return false
        }
        return responseLength == min(requestedLength, remainingLength)
    }
}

public struct OfflineNoteNfcCardSessionCommittedPayload: Equatable, Sendable {
    public let kind: OfflineNoteNfcPayloadKind
    public let payloadBytes: Data

    public init(kind: OfflineNoteNfcPayloadKind, payloadBytes: Data) {
        self.kind = kind
        self.payloadBytes = payloadBytes
    }
}

public enum OfflineNoteNfcCardSessionRejectionReason: Equatable, Sendable {
    case conditionsNotSatisfied
    case wrongData
    case unsupportedCommand
    case incompletePayload
    case checksumMismatch
    case invalidCommittedPayload
}

public struct OfflineNoteNfcCardSessionHandleResult: Equatable, Sendable {
    public let response: Data
    public let committedPayload: OfflineNoteNfcCardSessionCommittedPayload?
    public let receiptAckReadRange: Range<Int>?
    public let rejectionReason: OfflineNoteNfcCardSessionRejectionReason?

    public init(
        response: Data,
        committedPayload: OfflineNoteNfcCardSessionCommittedPayload? = nil,
        receiptAckReadRange: Range<Int>? = nil,
        rejectionReason: OfflineNoteNfcCardSessionRejectionReason? = nil
    ) {
        self.response = response
        self.committedPayload = committedPayload
        self.receiptAckReadRange = receiptAckReadRange
        self.rejectionReason = rejectionReason
    }
}

public struct OfflineNoteNfcPayloadReadProgress: Equatable, Sendable {
    public let readByteCount: Int
    public let expectedLength: Int
    public let isComplete: Bool

    public init(readByteCount: Int, expectedLength: Int, isComplete: Bool) {
        self.readByteCount = readByteCount
        self.expectedLength = expectedLength
        self.isComplete = isComplete
    }
}

public final class OfflineNoteNfcCardSessionStateMachine {
    public typealias CommittedPayloadValidator = (OfflineNoteNfcPayloadKind, Data) -> Bool

    public let applicationIdentifier: Data
    private let committedPayloadValidator: CommittedPayloadValidator
    private var currentKind: OfflineNoteNfcPayloadKind
    private var currentPayloadBytes: Data
    private var currentPayloadInfo: Data
    private var currentPayloadReadTracker: OfflineNoteNfcPayloadReadTracker?
    private var readable = true
    private var pendingWrite: OfflineNoteNfcPayloadAssembler?
    private var didComplete = false

    public init(
        applicationIdentifier: Data = OfflineNoteNfcApduProtocol.aid,
        initialKind: OfflineNoteNfcPayloadKind = .receiveRequest,
        initialPayloadBytes: Data,
        committedPayloadValidator: @escaping CommittedPayloadValidator = { _, _ in true }
    ) throws {
        self.applicationIdentifier = try OfflineNoteNfcApduProtocol.validateAid(applicationIdentifier)
        self.committedPayloadValidator = committedPayloadValidator
        self.currentKind = initialKind
        self.currentPayloadBytes = initialPayloadBytes
        self.currentPayloadInfo = try OfflineNoteNfcApduProtocol.encodeInfo(
            kind: initialKind,
            payloadBytes: initialPayloadBytes
        )
        self.currentPayloadReadTracker = initialKind == .receiptAck
            ? try OfflineNoteNfcPayloadReadTracker(expectedLength: initialPayloadBytes.count)
            : nil
    }

    public var currentPayloadKind: OfflineNoteNfcPayloadKind {
        currentKind
    }

    public var currentPayloadLength: Int {
        currentPayloadBytes.count
    }

    public var isReadable: Bool {
        readable
    }

    public var hasPendingWrite: Bool {
        pendingWrite != nil
    }

    public var hasCompleted: Bool {
        didComplete
    }

    public var receiptAckReadProgress: OfflineNoteNfcPayloadReadProgress? {
        guard currentKind == .receiptAck,
              let tracker = currentPayloadReadTracker else {
            return nil
        }
        return OfflineNoteNfcPayloadReadProgress(
            readByteCount: tracker.readByteCount,
            expectedLength: tracker.expectedLength,
            isComplete: tracker.isComplete
        )
    }

    public func handle(_ commandAPDU: Data) -> OfflineNoteNfcCardSessionHandleResult {
        switch OfflineNoteNfcApduProtocol.parseCommand(commandAPDU, aid: applicationIdentifier) {
        case .select:
            pendingWrite = nil
            return OfflineNoteNfcCardSessionHandleResult(response: OfflineNoteNfcApduProtocol.response())
        case .getInfo:
            guard readable else {
                return OfflineNoteNfcCardSessionHandleResult(
                    response: OfflineNoteNfcApduProtocol.statusConditionsNotSatisfied,
                    rejectionReason: .conditionsNotSatisfied
                )
            }
            return OfflineNoteNfcCardSessionHandleResult(
                response: OfflineNoteNfcApduProtocol.response(currentPayloadInfo)
            )
        case .readChunk(let offset, let requestedLength):
            return readChunk(offset: offset, requestedLength: requestedLength)
        case .writeMeta(let kind, let payloadLength, let sha256):
            return beginWrite(kind: kind, payloadLength: payloadLength, sha256: sha256)
        case .writeChunk(let offset, let bytes):
            return writeChunk(offset: offset, bytes: bytes)
        case .commit:
            return commitWrite()
        case .unsupported:
            return OfflineNoteNfcCardSessionHandleResult(
                response: OfflineNoteNfcApduProtocol.statusUnsupported,
                rejectionReason: .unsupportedCommand
            )
        case .invalid:
            return OfflineNoteNfcCardSessionHandleResult(
                response: OfflineNoteNfcApduProtocol.statusWrongData,
                rejectionReason: .wrongData
            )
        }
    }

    @discardableResult
    public func markReceiptAckBytesRead(_ range: Range<Int>) -> Bool {
        guard !didComplete,
              currentKind == .receiptAck,
              let tracker = currentPayloadReadTracker,
              tracker.markRead(offset: range.lowerBound, length: range.upperBound - range.lowerBound) else {
            return false
        }
        if tracker.isComplete {
            didComplete = true
            readable = false
            pendingWrite = nil
        }
        return tracker.isComplete
    }

    public func publishPayload(
        kind: OfflineNoteNfcPayloadKind,
        payloadBytes: Data
    ) throws {
        guard !didComplete else {
            throw OfflineNoteNfcApduError.completedSession
        }
        let payloadInfo = try OfflineNoteNfcApduProtocol.encodeInfo(kind: kind, payloadBytes: payloadBytes)
        let payloadReadTracker = kind == .receiptAck
            ? try OfflineNoteNfcPayloadReadTracker(expectedLength: payloadBytes.count)
            : nil
        currentKind = kind
        currentPayloadBytes = payloadBytes
        currentPayloadInfo = payloadInfo
        currentPayloadReadTracker = payloadReadTracker
        pendingWrite = nil
        readable = true
    }

    public func markPayloadProcessing() {
        readable = false
        currentPayloadReadTracker = nil
    }

    private func readChunk(offset: Int, requestedLength: Int) -> OfflineNoteNfcCardSessionHandleResult {
        guard readable else {
            return OfflineNoteNfcCardSessionHandleResult(
                response: OfflineNoteNfcApduProtocol.statusConditionsNotSatisfied,
                rejectionReason: .conditionsNotSatisfied
            )
        }
        guard offset >= 0, offset < currentPayloadBytes.count else {
            return OfflineNoteNfcCardSessionHandleResult(
                response: OfflineNoteNfcApduProtocol.statusWrongData,
                rejectionReason: .wrongData
            )
        }
        let chunkLength = min(max(requestedLength, 1), OfflineNoteNfcApduProtocol.maxExtendedReadChunkBytes)
        let end = min(offset + chunkLength, currentPayloadBytes.count)
        let receiptAckReadRange = currentKind == .receiptAck ? offset..<end : nil
        return OfflineNoteNfcCardSessionHandleResult(
            response: OfflineNoteNfcApduProtocol.response(currentPayloadBytes.subdata(in: offset..<end)),
            receiptAckReadRange: receiptAckReadRange
        )
    }

    private func beginWrite(
        kind: OfflineNoteNfcPayloadKind,
        payloadLength: Int,
        sha256: Data
    ) -> OfflineNoteNfcCardSessionHandleResult {
        switch OfflineNoteNfcCardSessionWritePolicy.decision(
            currentKind: currentKind,
            incomingKind: kind,
            hasPendingWrite: pendingWrite != nil,
            readable: readable,
            didComplete: didComplete
        ) {
        case .accept:
            break
        case .rejectNotReady:
            return OfflineNoteNfcCardSessionHandleResult(
                response: OfflineNoteNfcApduProtocol.statusConditionsNotSatisfied,
                rejectionReason: .conditionsNotSatisfied
            )
        case .rejectWrongKind:
            return OfflineNoteNfcCardSessionHandleResult(
                response: OfflineNoteNfcApduProtocol.statusWrongData,
                rejectionReason: .wrongData
            )
        }
        guard payloadLength <= OfflineNoteNfcApduProtocol.maxIncomingPayloadBytes else {
            return OfflineNoteNfcCardSessionHandleResult(
                response: OfflineNoteNfcApduProtocol.statusWrongData,
                rejectionReason: .wrongData
            )
        }
        do {
            pendingWrite = try OfflineNoteNfcPayloadAssembler(
                kind: kind,
                expectedLength: payloadLength,
                expectedSha256: sha256
            )
            return OfflineNoteNfcCardSessionHandleResult(response: OfflineNoteNfcApduProtocol.response())
        } catch {
            return OfflineNoteNfcCardSessionHandleResult(
                response: OfflineNoteNfcApduProtocol.statusWrongData,
                rejectionReason: .wrongData
            )
        }
    }

    private func writeChunk(offset: Int, bytes: Data) -> OfflineNoteNfcCardSessionHandleResult {
        guard let pendingWrite else {
            return OfflineNoteNfcCardSessionHandleResult(
                response: OfflineNoteNfcApduProtocol.statusConditionsNotSatisfied,
                rejectionReason: .conditionsNotSatisfied
            )
        }
        guard pendingWrite.write(offset: offset, chunk: bytes) else {
            return OfflineNoteNfcCardSessionHandleResult(
                response: OfflineNoteNfcApduProtocol.statusWrongData,
                rejectionReason: .wrongData
            )
        }
        return OfflineNoteNfcCardSessionHandleResult(response: OfflineNoteNfcApduProtocol.response())
    }

    private func commitWrite() -> OfflineNoteNfcCardSessionHandleResult {
        guard let pendingWrite else {
            return OfflineNoteNfcCardSessionHandleResult(
                response: OfflineNoteNfcApduProtocol.statusConditionsNotSatisfied,
                rejectionReason: .conditionsNotSatisfied
            )
        }
        let payloadBytes: Data
        do {
            payloadBytes = try pendingWrite.commit()
        } catch OfflineNoteNfcApduError.checksumMismatch {
            return OfflineNoteNfcCardSessionHandleResult(
                response: OfflineNoteNfcApduProtocol.statusWrongData,
                rejectionReason: .checksumMismatch
            )
        } catch OfflineNoteNfcApduError.incompletePayload {
            return OfflineNoteNfcCardSessionHandleResult(
                response: OfflineNoteNfcApduProtocol.statusWrongData,
                rejectionReason: .incompletePayload
            )
        } catch {
            return OfflineNoteNfcCardSessionHandleResult(
                response: OfflineNoteNfcApduProtocol.statusWrongData,
                rejectionReason: .wrongData
            )
        }
        guard committedPayloadValidator(pendingWrite.kind, payloadBytes) else {
            return OfflineNoteNfcCardSessionHandleResult(
                response: OfflineNoteNfcApduProtocol.statusWrongData,
                rejectionReason: .invalidCommittedPayload
            )
        }
        let committedPayload = OfflineNoteNfcCardSessionCommittedPayload(
            kind: pendingWrite.kind,
            payloadBytes: payloadBytes
        )
        self.pendingWrite = nil
        readable = false
        currentPayloadReadTracker = nil
        return OfflineNoteNfcCardSessionHandleResult(
            response: OfflineNoteNfcApduProtocol.response(),
            committedPayload: committedPayload
        )
    }
}

public enum OfflineNoteNearbyMessageKind: String, Codable, Sendable {
    case receiveRequest = "receive_request"
    case payment
    case receiptAck = "receipt_ack"
    case rejected
}

public enum OfflineNoteNearbyTransportPolicy {
    public static let exchangeTimeoutSeconds: UInt64 = 90
    public static let invitationTimeoutSeconds: TimeInterval = 20
    public static let localNetworkPreflightTimeoutSeconds: TimeInterval = 12
    public static let maxPeerDisplayNameUTF8Bytes = 63
    public static let defaultPeerDisplayNamePrefix = "iroha"
    public static let receiptAckDisconnectGraceNanoseconds: UInt64 = 1_500_000_000

    public static func peerDisplayName(
        prefix: String = defaultPeerDisplayNamePrefix,
        uuid: UUID = UUID()
    ) -> String {
        let suffix = String(uuid.uuidString.prefix(8)).lowercased()
        let sanitizedPrefix = sanitizedPeerDisplayNamePrefix(prefix)
        let candidate = "\(sanitizedPrefix)-\(suffix)"
        guard candidate.utf8.count > maxPeerDisplayNameUTF8Bytes else {
            return candidate
        }
        let maxPrefixLength = Swift.max(1, maxPeerDisplayNameUTF8Bytes - suffix.utf8.count - 1)
        let clippedPrefix = String(sanitizedPrefix.prefix(maxPrefixLength)).trimmingCharacters(
            in: CharacterSet(charactersIn: "-")
        )
        let finalPrefix = clippedPrefix.isEmpty ? defaultPeerDisplayNamePrefix : clippedPrefix
        return "\(finalPrefix)-\(suffix)"
    }

    private static func sanitizedPeerDisplayNamePrefix(_ prefix: String) -> String {
        let folded = prefix.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        var output = ""
        var previousWasSeparator = false
        for scalar in folded.unicodeScalars {
            if ("a"..."z").contains(String(scalar)) || ("0"..."9").contains(String(scalar)) {
                output.unicodeScalars.append(scalar)
                previousWasSeparator = false
            } else if scalar == "-" || scalar == "_" {
                if !previousWasSeparator, !output.isEmpty {
                    output.append("-")
                    previousWasSeparator = true
                }
            }
        }
        let trimmed = output.trimmingCharacters(in: CharacterSet(charactersIn: "-"))
        return trimmed.isEmpty ? defaultPeerDisplayNamePrefix : trimmed
    }

    public static func requiresDisconnectGraceAfterSending(_ kind: OfflineNoteNearbyMessageKind) -> Bool {
        kind == .receiptAck
    }

    public static func disconnectGraceNanosecondsAfterSending(_ kind: OfflineNoteNearbyMessageKind) -> UInt64 {
        requiresDisconnectGraceAfterSending(kind) ? receiptAckDisconnectGraceNanoseconds : 0
    }
}

public enum OfflineNoteNearbyDiscoveryPolicy {
    public static let protocolKey = "protocol"
    public static let protocolVersion = "offline-bearer-cash-v1"
    public static let bonjourServiceName = bonjourServiceName(for: OfflineNoteTransferHandoff.nearbyServiceName)

    public static func bonjourServiceName(for serviceName: String) -> String {
        "_\(serviceName)._tcp"
    }

    public static var discoveryInfo: [String: String] {
        [protocolKey: protocolVersion]
    }

    public static func isExpectedDiscoveryInfo(_ discoveryInfo: [String: String]?) -> Bool {
        guard let value = discoveryInfo?[protocolKey] else {
            return false
        }
        return value == protocolVersion
    }
}

public enum OfflineNoteNearbyPeerSelection {
    public static func peerKey(displayName: String, selectionHash: Int) -> String {
        "\(displayName.utf8.count):\(displayName)#\(selectionHash)"
    }

    public static func shouldAcceptInvitation(
        didFinish: Bool,
        connectedPeerName: String?
    ) -> Bool {
        !didFinish && connectedPeerName == nil
    }

    public static func shouldInvite(
        foundPeerName: String,
        didFinish: Bool,
        connectedPeerName: String?,
        invitedPeerNames: Set<String>
    ) -> Bool {
        !didFinish
            && connectedPeerName == nil
            && !invitedPeerNames.contains(foundPeerName)
    }

    public static func shouldUseConnectedPeer(
        peerName: String,
        didFinish: Bool,
        connectedPeerName: String?
    ) -> Bool {
        !didFinish && (connectedPeerName == nil || connectedPeerName == peerName)
    }

    public static func shouldAcceptMessage(
        from peerName: String,
        didFinish: Bool,
        connectedPeerName: String?
    ) -> Bool {
        !didFinish && connectedPeerName == peerName
    }

    public static func shouldFailDisconnect(
        from peerName: String,
        didFinish: Bool,
        connectedPeerName: String?
    ) -> Bool {
        !didFinish && connectedPeerName == peerName
    }
}

public enum OfflineNoteNearbyMessageHandlingPolicy {
    public static func shouldAcceptSenderReceiveRequest(
        didFinish: Bool,
        isProcessingReceiveRequest: Bool,
        hasQueuedPaymentPayload: Bool
    ) -> Bool {
        !didFinish && !isProcessingReceiveRequest && !hasQueuedPaymentPayload
    }

    public static func shouldAcceptSenderReceiptAck(
        didFinish: Bool,
        hasQueuedPaymentPayload: Bool
    ) -> Bool {
        !didFinish && hasQueuedPaymentPayload
    }

    public static func shouldAcceptReceiverPayment(
        didFinish: Bool,
        isProcessingPayment: Bool
    ) -> Bool {
        !didFinish && !isProcessingPayment
    }
}

public extension OfflineNoteNearbyMessageKind {
    var requiresDisconnectGraceAfterSend: Bool {
        OfflineNoteNearbyTransportPolicy.requiresDisconnectGraceAfterSending(self)
    }

    var recommendedDisconnectGraceNanosecondsAfterSend: UInt64 {
        OfflineNoteNearbyTransportPolicy.disconnectGraceNanosecondsAfterSending(self)
    }
}

public enum OfflineNoteNearbyError: Error, LocalizedError, Equatable {
    case invalidMessage
    case pairingMismatch
    case cancelled

    public var errorDescription: String? {
        switch self {
        case .invalidMessage:
            return "Offline Note nearby message is invalid."
        case .pairingMismatch:
            return "Nearby pairing challenge did not match."
        case .cancelled:
            return "Nearby transfer was cancelled."
        }
    }
}

public struct OfflineNoteNearbyPairingChallenge: Codable, Equatable, Hashable, Sendable {
    public static let allAssetNames = [
        "nearby_pairing_stars",
        "nearby_pairing_bird",
        "nearby_pairing_mask",
    ]

    public static let allChoices = allAssetNames.compactMap { try? OfflineNoteNearbyPairingChallenge(assetName: $0) }

    public let assetName: String

    public init(assetName: String) throws {
        let trimmed = assetName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard Self.allAssetNames.contains(trimmed) else {
            throw OfflineNoteNearbyError.invalidMessage
        }
        self.assetName = trimmed
    }

    public init(from decoder: Decoder) throws {
        if let assetName = try? decoder.singleValueContainer().decode(String.self) {
            try self.init(assetName: assetName)
            return
        }
        let unknownContainer = try decoder.container(keyedBy: AnyCodingKey.self)
        guard unknownContainer.allKeys.count == 1,
              unknownContainer.allKeys.first?.stringValue == CodingKeys.assetName.rawValue,
              let assetNameKey = AnyCodingKey(stringValue: CodingKeys.assetName.rawValue) else {
            throw OfflineNoteNearbyError.invalidMessage
        }
        try self.init(assetName: unknownContainer.decode(String.self, forKey: assetNameKey))
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(assetName)
    }

    public static func random() -> OfflineNoteNearbyPairingChallenge {
        allChoices.randomElement() ?? allChoices[0]
    }

    private enum CodingKeys: String, CodingKey {
        case assetName
    }

    private struct AnyCodingKey: CodingKey {
        let stringValue: String
        let intValue: Int? = nil

        init?(stringValue: String) {
            self.stringValue = stringValue
        }

        init?(intValue: Int) {
            return nil
        }
    }
}

public struct OfflineNoteNearbyEnvelope: Codable, Equatable, Sendable {
    public static let maxEncodedBytes = 96 * 1024

    public let kind: OfflineNoteNearbyMessageKind
    public let payload: Data
    public let contentType: String
    public let pairingChallenge: OfflineNoteNearbyPairingChallenge?

    public init(
        kind: OfflineNoteNearbyMessageKind,
        payload: Data,
        contentType: String,
        pairingChallenge: OfflineNoteNearbyPairingChallenge? = nil
    ) throws {
        self.kind = kind
        self.payload = payload
        self.contentType = contentType
        self.pairingChallenge = pairingChallenge
        try validateForTransport()
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.kind = try container.decode(OfflineNoteNearbyMessageKind.self, forKey: .kind)
        let payloadText = try container.decode(String.self, forKey: .payload)
        guard let payload = Self.base64UrlDecode(payloadText) else {
            throw OfflineNoteNearbyError.invalidMessage
        }
        self.payload = payload
        self.contentType = try container.decode(String.self, forKey: .contentType)
        self.pairingChallenge = try container.decodeIfPresent(
            OfflineNoteNearbyPairingChallenge.self,
            forKey: .pairingChallenge
        )
        try validateForTransport()
    }

    public func encode(to encoder: Encoder) throws {
        try validateForTransport()
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(kind, forKey: .kind)
        try container.encode(Self.base64UrlEncode(payload), forKey: .payload)
        try container.encode(contentType, forKey: .contentType)
        try container.encodeIfPresent(pairingChallenge, forKey: .pairingChallenge)
    }

    public func encoded() throws -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        let data = try encoder.encode(self)
        guard data.count <= Self.maxEncodedBytes else {
            throw OfflineNoteNearbyError.invalidMessage
        }
        return data
    }

    public static func decode(_ data: Data) throws -> OfflineNoteNearbyEnvelope {
        guard data.count <= maxEncodedBytes else {
            throw OfflineNoteNearbyError.invalidMessage
        }
        let allowedKeys: Set<String> = ["kind", "payload", "contentType", "pairingChallenge"]
        guard let object = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              object.keys.allSatisfy({ allowedKeys.contains($0) }) else {
            throw OfflineNoteNearbyError.invalidMessage
        }
        return try JSONDecoder().decode(OfflineNoteNearbyEnvelope.self, from: data)
    }

    public func paymentToken() throws -> OfflineNotePaymentToken {
        guard kind == .payment,
              contentType == OfflineNoteTransferHandoff.paymentTokenContentType else {
            throw OfflineNoteNearbyError.invalidMessage
        }
        return try OfflineNotePaymentTokenCodec.decodeNorito(payload)
    }

    public func receiveRequest() throws -> OfflineNoteReceiveRequest {
        guard kind == .receiveRequest,
              contentType == OfflineNoteTransferHandoff.receiveRequestContentType else {
            throw OfflineNoteNearbyError.invalidMessage
        }
        return try OfflineNoteReceiveRequestCodec.decodeNorito(payload)
    }

    public func receiptAck() throws -> OfflineNoteReceiptAck {
        guard kind == .receiptAck,
              contentType == OfflineNoteTransferHandoff.receiptAckContentType else {
            throw OfflineNoteNearbyError.invalidMessage
        }
        return try OfflineNoteReceiptAckCodec.decodeNorito(payload)
    }

    public var requiresDisconnectGraceAfterSend: Bool {
        kind.requiresDisconnectGraceAfterSend
    }

    public var recommendedDisconnectGraceNanosecondsAfterSend: UInt64 {
        kind.recommendedDisconnectGraceNanosecondsAfterSend
    }

    private func validateForTransport() throws {
        guard !payload.isEmpty,
              payload.count <= OfflineNoteNfcApduProtocol.maxIncomingPayloadBytes,
              !contentType.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw OfflineNoteNearbyError.invalidMessage
        }
        switch kind {
        case .receiveRequest:
            guard pairingChallenge != nil,
                  Self.isContentType(
                    contentType,
                    oneOf: [
                        OfflineNoteTransferHandoff.receiveRequestContentType,
                        OfflineNoteTransferHandoff.textReceiveRequestContentType,
                    ]
                  ) else {
                throw OfflineNoteNearbyError.invalidMessage
            }
            if contentType == OfflineNoteTransferHandoff.textReceiveRequestContentType {
                try requireTextPayload(.receiveRequest)
            } else {
                guard (try? OfflineNoteReceiveRequestCodec.decodeNorito(payload)) != nil else {
                    throw OfflineNoteNearbyError.invalidMessage
                }
            }
        case .payment:
            guard pairingChallenge == nil,
                  Self.isContentType(
                    contentType,
                    oneOf: [
                        OfflineNoteTransferHandoff.paymentTokenContentType,
                        OfflineNoteTransferHandoff.textPaymentTokenContentType,
                    ]
                  ) else {
                throw OfflineNoteNearbyError.invalidMessage
            }
            if contentType == OfflineNoteTransferHandoff.paymentTokenContentType {
                guard (try? OfflineNotePaymentTokenCodec.decodeNorito(payload)) != nil else {
                    throw OfflineNoteNearbyError.invalidMessage
                }
            } else {
                try requireTextPayload(.paymentToken)
            }
        case .receiptAck:
            guard pairingChallenge == nil,
                  Self.isContentType(
                    contentType,
                    oneOf: [
                        OfflineNoteTransferHandoff.receiptAckContentType,
                        OfflineNoteTransferHandoff.textReceiptAckContentType,
                    ]
                  ) else {
                throw OfflineNoteNearbyError.invalidMessage
            }
            if contentType == OfflineNoteTransferHandoff.textReceiptAckContentType {
                try requireTextPayload(.receiptAck)
            } else {
                guard (try? OfflineNoteReceiptAckCodec.decodeNorito(payload)) != nil else {
                    throw OfflineNoteNearbyError.invalidMessage
                }
            }
        case .rejected:
            guard pairingChallenge == nil else { throw OfflineNoteNearbyError.invalidMessage }
        }
    }

    private func requireTextPayload(_ expectedKind: OfflineNoteTextPayloadKind) throws {
        guard let payloadText = String(data: payload, encoding: .utf8),
              OfflineNoteTransferHandoff.isValidDeviceToDeviceTextPayload(payloadText, expectedKind: expectedKind) else {
            throw OfflineNoteNearbyError.invalidMessage
        }
    }

    private static func isContentType(_ value: String, oneOf allowedValues: [String]) -> Bool {
        allowedValues.contains(value)
    }

    private enum CodingKeys: String, CodingKey {
        case kind
        case payload
        case contentType
        case pairingChallenge
    }

    private static func base64UrlEncode(_ data: Data) -> String {
        OfflineNoteTextTransferContract.base64URLEncodedString(data)
    }

    private static func base64UrlDecode(_ value: String) -> Data? {
        OfflineNoteTextTransferContract.base64URLDecodedData(value)
    }
}
