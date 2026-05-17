import CryptoKit
import Foundation

public enum OfflineNoteV2NfcPayloadKind: UInt8, CaseIterable, Sendable {
    case receiveChallenge = 1
    case paymentToken = 2
    case receiptAck = 3

    public var qrPayloadKind: OfflineQrPayloadKind {
        switch self {
        case .receiveChallenge:
            return .offlineReceiveChallengeV2
        case .paymentToken:
            return .offlinePaymentTokenV2
        case .receiptAck:
            return .offlineReceiptAckV2
        }
    }
}

public struct OfflineNoteV2NfcPayloadInfo: Equatable, Sendable {
    public let kind: OfflineNoteV2NfcPayloadKind
    public let payloadLength: Int
    public let maxChunkLength: Int
    public let sha256: Data
}

public enum OfflineNoteV2NfcCommand: Equatable, Sendable {
    case select
    case getInfo
    case readChunk(offset: Int, requestedLength: Int)
    case writeMeta(kind: OfflineNoteV2NfcPayloadKind, payloadLength: Int, sha256: Data)
    case writeChunk(offset: Int, bytes: Data)
    case commit
    case unsupported
    case invalid
}

public enum OfflineNoteV2NfcApduError: Error, LocalizedError, Equatable {
    case invalidOffset
    case invalidPayloadLength
    case invalidChunkLength
    case incompletePayload
    case checksumMismatch

    public var errorDescription: String? {
        switch self {
        case .invalidOffset:
            return "Offline Note V2 NFC APDU offset is out of bounds."
        case .invalidPayloadLength:
            return "Offline Note V2 NFC payload length is out of bounds."
        case .invalidChunkLength:
            return "Offline Note V2 NFC chunk length is out of bounds."
        case .incompletePayload:
            return "Offline Note V2 NFC payload is incomplete."
        case .checksumMismatch:
            return "Offline Note V2 NFC payload checksum did not match."
        }
    }
}

public enum OfflineNoteV2NfcApduProtocol {
    public static let aid = Data([0xF0, 0x49, 0x52, 0x4F, 0x48, 0x41, 0x32])
    public static let aidHex = "F049524F484132"
    public static let protocolVersion: UInt8 = 1
    public static let androidSafeChunkBytes = 240
    public static let maxExtendedReadChunkBytes = 1_024
    public static let maxExtendedWriteChunkBytes = 16_384
    public static let maxIncomingPayloadBytes = 64 * 1_024

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
        Data([0x00, 0xA4, 0x04, 0x00, UInt8(aid.count)]) + aid + Data([0x00])
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
        kind: OfflineNoteV2NfcPayloadKind,
        payloadBytes: Data
    ) throws -> Data {
        try requirePayloadLength(payloadBytes.count)
        let meta = Data([protocolVersion, kind.rawValue])
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
        let length = payloadBytes.distance(from: range.lowerBound, to: range.upperBound)
        try requireChunkLength(length, maxChunkLength: maxExtendedWriteChunkBytes)
        var apdu = Data()
        apdu.reserveCapacity(commandHeaderLength(forPayloadLength: length) + length)
        appendCommandHeader(&apdu, instruction: insWriteChunk, offset: offset, length: length)
        payloadBytes.withUnsafeBytes { rawBuffer in
            guard let baseAddress = rawBuffer.bindMemory(to: UInt8.self).baseAddress else { return }
            let lowerBound = payloadBytes.distance(from: payloadBytes.startIndex, to: range.lowerBound)
            apdu.append(baseAddress.advanced(by: lowerBound), count: length)
        }
        return apdu
    }

    public static func commitAPDUData() -> Data {
        Data([cla, insCommit, 0x00, 0x00, 0x00])
    }

    public static func writePayloadAPDUs(
        kind: OfflineNoteV2NfcPayloadKind,
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

    public static func parseCommand(_ apdu: Data?) -> OfflineNoteV2NfcCommand {
        guard let apdu, apdu.count >= 4 else { return .invalid }
        if isSelectAid(apdu) { return .select }
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
        kind: OfflineNoteV2NfcPayloadKind,
        payloadBytes: Data,
        maxChunkLength: Int = androidSafeChunkBytes
    ) throws -> Data {
        try requirePayloadLength(payloadBytes.count)
        try requireChunkLength(maxChunkLength, maxChunkLength: maxExtendedReadChunkBytes)
        return Data([protocolVersion, kind.rawValue])
            + int32(payloadBytes.count)
            + uint16(maxChunkLength)
            + sha256(payloadBytes)
    }

    public static func decodeInfo(_ data: Data) -> OfflineNoteV2NfcPayloadInfo? {
        guard data.count == 40,
              data[data.startIndex] == protocolVersion,
              let kind = OfflineNoteV2NfcPayloadKind(rawValue: data[data.startIndex + 1]) else {
            return nil
        }
        let payloadLength = readInt32(data, offset: 2)
        let maxChunkLength = readUInt16(data, offset: 6)
        guard payloadLength > 0,
              payloadLength <= maxIncomingPayloadBytes,
              maxChunkLength > 0,
              maxChunkLength <= maxExtendedReadChunkBytes else {
            return nil
        }
        return OfflineNoteV2NfcPayloadInfo(
            kind: kind,
            payloadLength: payloadLength,
            maxChunkLength: maxChunkLength,
            sha256: data.subdata(in: 8..<40)
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

    private static func isSelectAid(_ apdu: Data) -> Bool {
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
        if apdu.count == 4 { return true }
        if apdu.count == 5 { return apdu[apdu.startIndex + 4] != 0x00 }
        guard apdu.count == 7,
              apdu[apdu.startIndex + 4] == 0x00 else {
            return false
        }
        let extendedLength = (Int(apdu[apdu.startIndex + 5]) << 8) | Int(apdu[apdu.startIndex + 6])
        return extendedLength > 0 && extendedLength <= maxExtendedReadChunkBytes
    }

    private static func parseWriteMeta(_ data: Data) -> OfflineNoteV2NfcCommand {
        guard data.count == 38,
              data[data.startIndex] == protocolVersion,
              let kind = OfflineNoteV2NfcPayloadKind(rawValue: data[data.startIndex + 1]) else {
            return .invalid
        }
        let payloadLength = readInt32(data, offset: 2)
        guard payloadLength > 0,
              payloadLength <= maxIncomingPayloadBytes else {
            return .invalid
        }
        return .writeMeta(
            kind: kind,
            payloadLength: payloadLength,
            sha256: data.subdata(in: (data.startIndex + 6)..<(data.startIndex + 38))
        )
    }

    private static func requireValidOffset(_ offset: Int) throws {
        guard (0...0xffff).contains(offset) else {
            throw OfflineNoteV2NfcApduError.invalidOffset
        }
    }

    private static func requirePayloadLength(_ length: Int) throws {
        guard length > 0, length <= maxIncomingPayloadBytes else {
            throw OfflineNoteV2NfcApduError.invalidPayloadLength
        }
    }

    private static func requireChunkLength(_ length: Int, maxChunkLength: Int) throws {
        guard length > 0, length <= maxChunkLength else {
            throw OfflineNoteV2NfcApduError.invalidChunkLength
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

public final class OfflineNoteV2NfcPayloadAssembler {
    public let kind: OfflineNoteV2NfcPayloadKind
    public let expectedLength: Int
    public let expectedSha256: Data
    private var bytes: Data
    private var written: [Bool]
    private var writtenCount = 0

    public convenience init(info: OfflineNoteV2NfcPayloadInfo) throws {
        try self.init(kind: info.kind, expectedLength: info.payloadLength, expectedSha256: info.sha256)
    }

    public init(kind: OfflineNoteV2NfcPayloadKind, expectedLength: Int, expectedSha256: Data) throws {
        guard expectedLength > 0,
              expectedLength <= OfflineNoteV2NfcApduProtocol.maxIncomingPayloadBytes,
              expectedSha256.count == 32 else {
            throw OfflineNoteV2NfcApduError.invalidPayloadLength
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

    @discardableResult
    public func write(offset: Int, chunk: Data) -> Bool {
        guard offset >= 0,
              offset <= expectedLength,
              !chunk.isEmpty,
              chunk.count <= OfflineNoteV2NfcApduProtocol.maxExtendedWriteChunkBytes else {
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
            throw OfflineNoteV2NfcApduError.incompletePayload
        }
        guard OfflineNoteV2NfcApduProtocol.payloadDigestMatches(bytes, expectedSha256: expectedSha256) else {
            throw OfflineNoteV2NfcApduError.checksumMismatch
        }
        return bytes
    }
}

public enum OfflineNoteV2NearbyMessageKind: String, Codable, Sendable {
    case challenge
    case payment
    case receiptAck = "receipt_ack"
    case rejected
}

public enum OfflineNoteV2NearbyError: Error, LocalizedError, Equatable {
    case invalidMessage
    case pairingMismatch
    case cancelled

    public var errorDescription: String? {
        switch self {
        case .invalidMessage:
            return "Offline Note V2 nearby message is invalid."
        case .pairingMismatch:
            return "Nearby pairing challenge did not match."
        case .cancelled:
            return "Nearby transfer was cancelled."
        }
    }
}

public struct OfflineNoteV2NearbyPairingChallenge: Codable, Equatable, Hashable, Sendable {
    public static let allAssetNames = [
        "nearby_pairing_stars",
        "nearby_pairing_bird",
        "nearby_pairing_mask",
    ]

    public static let allChoices = allAssetNames.compactMap { try? OfflineNoteV2NearbyPairingChallenge(assetName: $0) }

    public let assetName: String

    public init(assetName: String) throws {
        let trimmed = assetName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard Self.allAssetNames.contains(trimmed) else {
            throw OfflineNoteV2NearbyError.invalidMessage
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
            throw OfflineNoteV2NearbyError.invalidMessage
        }
        try self.init(assetName: unknownContainer.decode(String.self, forKey: assetNameKey))
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(assetName)
    }

    public static func random() -> OfflineNoteV2NearbyPairingChallenge {
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

public struct OfflineNoteV2NearbyEnvelope: Codable, Equatable, Sendable {
    public static let version = 1

    public let version: Int
    public let kind: OfflineNoteV2NearbyMessageKind
    public let payload: Data
    public let contentType: String
    public let pairingChallenge: OfflineNoteV2NearbyPairingChallenge?

    public init(
        kind: OfflineNoteV2NearbyMessageKind,
        payload: Data,
        contentType: String,
        pairingChallenge: OfflineNoteV2NearbyPairingChallenge? = nil
    ) throws {
        self.version = Self.version
        self.kind = kind
        self.payload = payload
        self.contentType = contentType
        self.pairingChallenge = pairingChallenge
        try validateForTransport()
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.version = try container.decode(Int.self, forKey: .version)
        self.kind = try container.decode(OfflineNoteV2NearbyMessageKind.self, forKey: .kind)
        let payloadText = try container.decode(String.self, forKey: .payload)
        guard let payload = Self.base64UrlDecode(payloadText) else {
            throw OfflineNoteV2NearbyError.invalidMessage
        }
        self.payload = payload
        self.contentType = try container.decode(String.self, forKey: .contentType)
        self.pairingChallenge = try container.decodeIfPresent(
            OfflineNoteV2NearbyPairingChallenge.self,
            forKey: .pairingChallenge
        )
        guard version == Self.version else {
            throw OfflineNoteV2NearbyError.invalidMessage
        }
        try validateForTransport()
    }

    public func encode(to encoder: Encoder) throws {
        try validateForTransport()
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(version, forKey: .version)
        try container.encode(kind, forKey: .kind)
        try container.encode(Self.base64UrlEncode(payload), forKey: .payload)
        try container.encode(contentType, forKey: .contentType)
        try container.encodeIfPresent(pairingChallenge, forKey: .pairingChallenge)
    }

    public func encoded() throws -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        return try encoder.encode(self)
    }

    public static func decode(_ data: Data) throws -> OfflineNoteV2NearbyEnvelope {
        let allowedKeys: Set<String> = ["version", "kind", "payload", "contentType", "pairingChallenge"]
        guard let object = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              object.keys.allSatisfy({ allowedKeys.contains($0) }) else {
            throw OfflineNoteV2NearbyError.invalidMessage
        }
        return try JSONDecoder().decode(OfflineNoteV2NearbyEnvelope.self, from: data)
    }

    public func paymentToken() throws -> OfflineNoteV2PaymentToken {
        guard kind == .payment,
              contentType == OfflineNoteV2TransferHandoff.paymentTokenContentType else {
            throw OfflineNoteV2NearbyError.invalidMessage
        }
        return try OfflineNoteV2PaymentTokenCodec.decodeNorito(payload)
    }

    private func validateForTransport() throws {
        guard !payload.isEmpty,
              payload.count <= OfflineNoteV2NfcApduProtocol.maxIncomingPayloadBytes,
              !contentType.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw OfflineNoteV2NearbyError.invalidMessage
        }
        switch kind {
        case .challenge:
            guard pairingChallenge != nil,
                  contentType == OfflineNoteV2TransferHandoff.receiveChallengeContentType else {
                throw OfflineNoteV2NearbyError.invalidMessage
            }
        case .payment:
            guard pairingChallenge == nil,
                  contentType == OfflineNoteV2TransferHandoff.paymentTokenContentType,
                  (try? OfflineNoteV2PaymentTokenCodec.decodeNorito(payload)) != nil else {
                throw OfflineNoteV2NearbyError.invalidMessage
            }
        case .receiptAck:
            guard pairingChallenge == nil,
                  contentType == OfflineNoteV2TransferHandoff.receiptAckContentType else {
                throw OfflineNoteV2NearbyError.invalidMessage
            }
        case .rejected:
            guard pairingChallenge == nil else { throw OfflineNoteV2NearbyError.invalidMessage }
        }
    }

    private enum CodingKeys: String, CodingKey {
        case version
        case kind
        case payload
        case contentType
        case pairingChallenge
    }

    private static func base64UrlEncode(_ data: Data) -> String {
        data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .trimmingCharacters(in: CharacterSet(charactersIn: "="))
    }

    private static func base64UrlDecode(_ value: String) -> Data? {
        guard !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !value.contains("="),
              value.unicodeScalars.allSatisfy({ scalar in
                  let byte = scalar.value
                  return (65...90).contains(byte)
                      || (97...122).contains(byte)
                      || (48...57).contains(byte)
                      || byte == 45
                      || byte == 95
              }) else {
            return nil
        }
        var normalized = value
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        let padding = (4 - normalized.count % 4) % 4
        normalized.append(String(repeating: "=", count: padding))
        return Data(base64Encoded: normalized)
    }
}
