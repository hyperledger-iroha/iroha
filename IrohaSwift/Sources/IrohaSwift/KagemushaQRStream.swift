import CryptoKit
import Foundation

public enum KagemushaQRStreamError: Error, Equatable, LocalizedError, Sendable {
    case invalidOptions
    case payloadTooLarge(actual: Int, maximum: Int)
    case malformedFrame
    case nonCanonicalFrame
    case checksumMismatch
    case wrongStream
    case conflictingFrame
    case invalidHeader
    case digestMismatch
    case kindMismatch(expected: KagemushaPeerPayloadKind, actual: KagemushaPeerPayloadKind)
    case invalidPayload

    public var errorDescription: String? {
        switch self {
        case .invalidOptions:
            return "Kagemusha QR stream options are outside the supported bounds."
        case .payloadTooLarge(let actual, let maximum):
            return "Kagemusha QR payload is \(actual) bytes; the limit is \(maximum)."
        case .malformedFrame:
            return "Kagemusha QR stream frame is malformed."
        case .nonCanonicalFrame:
            return "Kagemusha QR stream frame is not canonically encoded."
        case .checksumMismatch:
            return "Kagemusha QR stream frame checksum does not match."
        case .wrongStream:
            return "Kagemusha QR frame belongs to a different stream."
        case .conflictingFrame:
            return "Kagemusha QR stream contains conflicting duplicate frames."
        case .invalidHeader:
            return "Kagemusha QR stream header is invalid."
        case .digestMismatch:
            return "Kagemusha QR stream payload digest does not match."
        case .kindMismatch(let expected, let actual):
            return "Kagemusha QR header declares \(expected), but the archive is \(actual)."
        case .invalidPayload:
            return "Kagemusha QR stream does not contain a valid peer archive."
        }
    }
}

public struct KagemushaQRStreamOptions: Equatable, Sendable {
    public static let minimumChunkSize = 64
    public static let maximumChunkSize = 512
    public static let minimumParityGroup = 2
    public static let maximumParityGroup = 16

    public static var standard: Self {
        Self(uncheckedChunkSize: 256, parityGroup: 4)
    }

    public let chunkSize: Int
    public let parityGroup: Int

    public init(chunkSize: Int = 256, parityGroup: Int = 4) throws {
        guard (Self.minimumChunkSize...Self.maximumChunkSize).contains(chunkSize),
              (Self.minimumParityGroup...Self.maximumParityGroup).contains(parityGroup) else {
            throw KagemushaQRStreamError.invalidOptions
        }
        self.chunkSize = chunkSize
        self.parityGroup = parityGroup
    }

    private init(uncheckedChunkSize: Int, parityGroup: Int) {
        chunkSize = uncheckedChunkSize
        self.parityGroup = parityGroup
    }
}

public struct KagemushaQRDecodeResult: Equatable, Sendable {
    public let payload: KagemushaPeerPayload?
    public let payloadKind: KagemushaPeerPayloadKind?
    public let receivedDataFrames: Int
    public let totalDataFrames: Int
    public let recoveredDataFrames: Int

    public var isComplete: Bool { payload != nil }

    public var progress: Double {
        guard totalDataFrames > 0 else { return 0 }
        return min(1, Double(receivedDataFrames) / Double(totalDataFrames))
    }
}

/// Fixed-XOR parity QR framing for canonical Kagemusha peer archives.
///
/// Each parity frame recovers at most one missing chunk in its fixed group.
public enum KagemushaQRStreamCodec {
    /// Exact upper bound for an unpadded base64url frame, including `PKKQ1.`.
    /// This is checked before allocating the decoded frame bytes.
    static let maximumFrameTextBytes =
        KagemushaPeerTransportContract.qrStreamTextPrefix.utf8.count
            + (KagemushaQRStreamFrame.maximumEncodedBytes * 4 + 2) / 3

    public static func encode(
        _ payload: KagemushaPeerPayload,
        options: KagemushaQRStreamOptions = .standard
    ) throws -> [String] {
        let archive = payload.archive
        guard !archive.isEmpty,
              archive.count <= KagemushaPeerTransportContract.maximumArchiveBytes else {
            throw KagemushaQRStreamError.payloadTooLarge(
                actual: archive.count,
                maximum: KagemushaPeerTransportContract.maximumArchiveBytes
            )
        }
        let header = try KagemushaQRStreamEnvelope(
            kind: payload.kind,
            payload: archive,
            options: options
        )
        let streamID = header.streamID
        var frames = [try KagemushaQRStreamFrame(
            kind: .header,
            streamID: streamID,
            index: 0,
            total: 1,
            payload: header.encode()
        )]

        var chunks: [Data] = []
        chunks.reserveCapacity(header.dataChunks)
        for offset in stride(from: 0, to: archive.count, by: options.chunkSize) {
            chunks.append(
                archive.subdata(in: offset..<min(offset + options.chunkSize, archive.count))
            )
        }
        for (index, chunk) in chunks.enumerated() {
            frames.append(try KagemushaQRStreamFrame(
                kind: .data,
                streamID: streamID,
                index: index,
                total: chunks.count,
                payload: chunk
            ))
        }

        for group in 0..<header.parityChunks {
            let start = group * options.parityGroup
            let end = min(start + options.parityGroup, chunks.count)
            var parity = Data(repeating: 0, count: options.chunkSize)
            for chunk in chunks[start..<end] {
                for index in chunk.indices {
                    parity[index] ^= chunk[index]
                }
            }
            frames.append(try KagemushaQRStreamFrame(
                kind: .parity,
                streamID: streamID,
                index: group,
                total: header.parityChunks,
                payload: parity
            ))
        }

        return frames.map { KagemushaPeerTransportContract.qrStreamTextPrefix +
            KagemushaPeerTextCodec.base64URLEncode($0.encode())
        }
    }

    static func decodeFrameText(_ value: String) throws -> KagemushaQRStreamFrame {
        let prefix = KagemushaPeerTransportContract.qrStreamTextPrefix
        guard value.utf8.count <= maximumFrameTextBytes,
              value.hasPrefix(prefix) else {
            throw KagemushaQRStreamError.nonCanonicalFrame
        }
        let body = String(value.dropFirst(prefix.count))
        guard let bytes = KagemushaPeerTextCodec.base64URLDecode(body) else {
            throw KagemushaQRStreamError.nonCanonicalFrame
        }
        guard prefix + KagemushaPeerTextCodec.base64URLEncode(bytes) == value else {
            throw KagemushaQRStreamError.nonCanonicalFrame
        }
        return try KagemushaQRStreamFrame.decode(bytes)
    }
}

public final class KagemushaQRStreamDecoder: @unchecked Sendable {
    private static let maximumDataFrames =
        (KagemushaPeerTransportContract.maximumArchiveBytes
            + KagemushaQRStreamOptions.minimumChunkSize - 1)
            / KagemushaQRStreamOptions.minimumChunkSize
    private static let maximumParityFrames =
        (maximumDataFrames + KagemushaQRStreamOptions.minimumParityGroup - 1)
            / KagemushaQRStreamOptions.minimumParityGroup

    private let lock = NSLock()
    private let chainDiscriminant: UInt16
    private var streamID: Data?
    private var envelope: KagemushaQRStreamEnvelope?
    private var dataFrames: [Int: Data] = [:]
    private var dataFrameTotals: [Int: Int] = [:]
    private var parityFrames: [Int: Data] = [:]
    private var parityFrameTotals: [Int: Int] = [:]
    private var recovered = Set<Int>()
    private var completedPayload: KagemushaPeerPayload?

    private struct State {
        let streamID: Data?
        let envelope: KagemushaQRStreamEnvelope?
        let dataFrames: [Int: Data]
        let dataFrameTotals: [Int: Int]
        let parityFrames: [Int: Data]
        let parityFrameTotals: [Int: Int]
        let recovered: Set<Int>
        let completedPayload: KagemushaPeerPayload?
    }

    public init(chainDiscriminant: UInt16) {
        self.chainDiscriminant = chainDiscriminant
    }

    public func reset() {
        lock.lock()
        defer { lock.unlock() }
        streamID = nil
        envelope = nil
        dataFrames.removeAll(keepingCapacity: false)
        dataFrameTotals.removeAll(keepingCapacity: false)
        parityFrames.removeAll(keepingCapacity: false)
        parityFrameTotals.removeAll(keepingCapacity: false)
        recovered.removeAll(keepingCapacity: false)
        completedPayload = nil
    }

    public func ingest(_ frameText: String) throws -> KagemushaQRDecodeResult {
        let frame = try KagemushaQRStreamCodec.decodeFrameText(frameText)
        lock.lock()
        defer { lock.unlock() }
        let previousState = state
        do {
            return try ingestLocked(frame)
        } catch {
            // An invalid frame is a failed transaction: stream selection,
            // buffered frames, parity recovery, and completion all roll back.
            // A subsequent valid frame from this or another stream therefore
            // observes exactly the state that existed before the bad input.
            state = previousState
            throw error
        }
    }

    private var state: State {
        get {
            State(
                streamID: streamID,
                envelope: envelope,
                dataFrames: dataFrames,
                dataFrameTotals: dataFrameTotals,
                parityFrames: parityFrames,
                parityFrameTotals: parityFrameTotals,
                recovered: recovered,
                completedPayload: completedPayload
            )
        }
        set {
            streamID = newValue.streamID
            envelope = newValue.envelope
            dataFrames = newValue.dataFrames
            dataFrameTotals = newValue.dataFrameTotals
            parityFrames = newValue.parityFrames
            parityFrameTotals = newValue.parityFrameTotals
            recovered = newValue.recovered
            completedPayload = newValue.completedPayload
        }
    }

    private func ingestLocked(_ frame: KagemushaQRStreamFrame) throws
        -> KagemushaQRDecodeResult
    {
        if let streamID {
            guard streamID == frame.streamID else {
                throw KagemushaQRStreamError.wrongStream
            }
        } else {
            streamID = frame.streamID
        }

        switch frame.kind {
        case .header:
            let decoded = try KagemushaQRStreamEnvelope.decode(frame.payload)
            guard decoded.streamID == frame.streamID else {
                throw KagemushaQRStreamError.digestMismatch
            }
            if let envelope, envelope != decoded {
                throw KagemushaQRStreamError.conflictingFrame
            }
            envelope = decoded
        case .data:
            guard frame.total <= Self.maximumDataFrames else {
                throw KagemushaQRStreamError.malformedFrame
            }
            try store(
                frame.payload,
                index: frame.index,
                total: frame.total,
                in: &dataFrames,
                totals: &dataFrameTotals
            )
        case .parity:
            guard frame.total <= Self.maximumParityFrames else {
                throw KagemushaQRStreamError.malformedFrame
            }
            try store(
                frame.payload,
                index: frame.index,
                total: frame.total,
                in: &parityFrames,
                totals: &parityFrameTotals
            )
        }

        if let envelope {
            try validateBufferedFrames(against: envelope)
            try recoverSingleMissingFrames(envelope)
            completedPayload = try completedPayload ?? finalizeIfComplete(envelope)
        }
        return result()
    }

    private func store(
        _ payload: Data,
        index: Int,
        total: Int,
        in frames: inout [Int: Data],
        totals: inout [Int: Int]
    ) throws {
        guard index >= 0, index < total else {
            throw KagemushaQRStreamError.malformedFrame
        }
        if let existing = frames[index] {
            guard existing == payload, totals[index] == total else {
                throw KagemushaQRStreamError.conflictingFrame
            }
        } else {
            frames[index] = payload
            totals[index] = total
        }
    }

    private func validateBufferedFrames(
        against envelope: KagemushaQRStreamEnvelope
    ) throws {
        for (index, payload) in dataFrames {
            guard index < envelope.dataChunks,
                  dataFrameTotals[index] == envelope.dataChunks,
                  payload.count == envelope.expectedDataChunkLength(index: index) else {
                throw KagemushaQRStreamError.malformedFrame
            }
        }
        for (index, payload) in parityFrames {
            guard index < envelope.parityChunks,
                  parityFrameTotals[index] == envelope.parityChunks,
                  payload.count == envelope.chunkSize else {
                throw KagemushaQRStreamError.malformedFrame
            }
        }
    }

    private func recoverSingleMissingFrames(
        _ envelope: KagemushaQRStreamEnvelope
    ) throws {
        for group in 0..<envelope.parityChunks {
            guard let parity = parityFrames[group] else { continue }
            let start = group * envelope.parityGroup
            let end = min(start + envelope.parityGroup, envelope.dataChunks)
            let missing = (start..<end).filter { dataFrames[$0] == nil }
            guard missing.count == 1 else { continue }
            var recoveredChunk = parity
            for index in start..<end where index != missing[0] {
                guard let chunk = dataFrames[index] else {
                    throw KagemushaQRStreamError.malformedFrame
                }
                for byteIndex in chunk.indices {
                    recoveredChunk[byteIndex] ^= chunk[byteIndex]
                }
            }
            recoveredChunk = recoveredChunk.prefix(
                envelope.expectedDataChunkLength(index: missing[0])
            )
            dataFrames[missing[0]] = recoveredChunk
            dataFrameTotals[missing[0]] = envelope.dataChunks
            recovered.insert(missing[0])
        }
    }

    private func finalizeIfComplete(
        _ envelope: KagemushaQRStreamEnvelope
    ) throws -> KagemushaPeerPayload? {
        guard dataFrames.count == envelope.dataChunks else { return nil }
        var archive = Data()
        archive.reserveCapacity(envelope.totalBytes)
        for index in 0..<envelope.dataChunks {
            guard let chunk = dataFrames[index] else { return nil }
            archive.append(chunk)
        }
        guard archive.count == envelope.totalBytes else {
            throw KagemushaQRStreamError.malformedFrame
        }
        guard Data(SHA256.hash(data: archive)) == envelope.payloadDigest else {
            throw KagemushaQRStreamError.digestMismatch
        }
        do {
            let payload = try KagemushaPeerPayload.decode(
                archive: archive,
                kind: envelope.payloadKind,
                chainDiscriminant: chainDiscriminant
            )
            guard payload.kind == envelope.payloadKind else {
                throw KagemushaQRStreamError.kindMismatch(
                    expected: envelope.payloadKind,
                    actual: payload.kind
                )
            }
            return payload
        } catch let error as KagemushaQRStreamError {
            throw error
        } catch {
            throw KagemushaQRStreamError.invalidPayload
        }
    }

    private func result() -> KagemushaQRDecodeResult {
        KagemushaQRDecodeResult(
            payload: completedPayload,
            payloadKind: envelope?.payloadKind,
            receivedDataFrames: dataFrames.count,
            totalDataFrames: envelope?.dataChunks ?? 0,
            recoveredDataFrames: recovered.count
        )
    }
}

enum KagemushaQRStreamFrameKind: UInt8, Sendable {
    case header = 0
    case data = 1
    case parity = 2
}

struct KagemushaQRStreamEnvelope: Equatable, Sendable {
    static let version: UInt8 = 1
    static let encodedLength = 46

    let payloadKind: KagemushaPeerPayloadKind
    let parityGroup: Int
    let chunkSize: Int
    let dataChunks: Int
    let parityChunks: Int
    let totalBytes: Int
    let payloadDigest: Data

    var streamID: Data { Data(payloadDigest.prefix(16)) }

    init(
        kind: KagemushaPeerPayloadKind,
        payload: Data,
        options: KagemushaQRStreamOptions
    ) throws {
        guard !payload.isEmpty,
              payload.count <= KagemushaPeerTransportContract.maximumArchiveBytes else {
            throw KagemushaQRStreamError.payloadTooLarge(
                actual: payload.count,
                maximum: KagemushaPeerTransportContract.maximumArchiveBytes
            )
        }
        payloadKind = kind
        parityGroup = options.parityGroup
        chunkSize = options.chunkSize
        dataChunks = (payload.count + options.chunkSize - 1) / options.chunkSize
        parityChunks = (dataChunks + options.parityGroup - 1) / options.parityGroup
        totalBytes = payload.count
        payloadDigest = Data(SHA256.hash(data: payload))
    }

    private init(
        payloadKind: KagemushaPeerPayloadKind,
        parityGroup: Int,
        chunkSize: Int,
        dataChunks: Int,
        parityChunks: Int,
        totalBytes: Int,
        payloadDigest: Data
    ) throws {
        guard (KagemushaQRStreamOptions.minimumChunkSize...KagemushaQRStreamOptions.maximumChunkSize)
                .contains(chunkSize),
              (KagemushaQRStreamOptions.minimumParityGroup...KagemushaQRStreamOptions.maximumParityGroup)
                .contains(parityGroup),
              (1...KagemushaPeerTransportContract.maximumArchiveBytes).contains(totalBytes),
              dataChunks == (totalBytes + chunkSize - 1) / chunkSize,
              parityChunks == (dataChunks + parityGroup - 1) / parityGroup,
              payloadDigest.count == 32,
              payloadDigest.contains(where: { $0 != 0 }) else {
            throw KagemushaQRStreamError.invalidHeader
        }
        self.payloadKind = payloadKind
        self.parityGroup = parityGroup
        self.chunkSize = chunkSize
        self.dataChunks = dataChunks
        self.parityChunks = parityChunks
        self.totalBytes = totalBytes
        self.payloadDigest = payloadDigest
    }

    func expectedDataChunkLength(index: Int) -> Int {
        guard index == dataChunks - 1 else { return chunkSize }
        return totalBytes - index * chunkSize
    }

    func encode() -> Data {
        var data = Data([Self.version, payloadKind.rawValue, UInt8(parityGroup), 0])
        data.appendUInt16BE(UInt16(chunkSize))
        data.appendUInt16BE(UInt16(dataChunks))
        data.appendUInt16BE(UInt16(parityChunks))
        data.appendUInt32BE(UInt32(totalBytes))
        data.append(payloadDigest)
        return data
    }

    static func decode(_ data: Data) throws -> Self {
        guard data.count == encodedLength,
              data[0] == version,
              let kind = KagemushaPeerPayloadKind(rawValue: data[1]),
              data[3] == 0 else {
            throw KagemushaQRStreamError.invalidHeader
        }
        return try Self(
            payloadKind: kind,
            parityGroup: Int(data[2]),
            chunkSize: Int(data.uint16BE(at: 4)),
            dataChunks: Int(data.uint16BE(at: 6)),
            parityChunks: Int(data.uint16BE(at: 8)),
            totalBytes: Int(data.uint32BE(at: 10)),
            payloadDigest: data.subdata(in: 14..<46)
        )
    }
}

struct KagemushaQRStreamFrame: Equatable, Sendable {
    static let magic = Data([0x4B, 0x51])
    static let version: UInt8 = 1
    static let fixedOverhead = 30
    static let maximumEncodedBytes = fixedOverhead + KagemushaQRStreamOptions.maximumChunkSize

    let kind: KagemushaQRStreamFrameKind
    let streamID: Data
    let index: Int
    let total: Int
    let payload: Data

    init(
        kind: KagemushaQRStreamFrameKind,
        streamID: Data,
        index: Int,
        total: Int,
        payload: Data
    ) throws {
        guard streamID.count == 16,
              streamID.contains(where: { $0 != 0 }),
              index >= 0,
              index < total,
              total > 0,
              total <= Int(UInt16.max),
              !payload.isEmpty,
              payload.count <= KagemushaQRStreamOptions.maximumChunkSize else {
            throw KagemushaQRStreamError.malformedFrame
        }
        switch kind {
        case .header:
            guard index == 0, total == 1,
                  payload.count == KagemushaQRStreamEnvelope.encodedLength else {
                throw KagemushaQRStreamError.malformedFrame
            }
        case .data, .parity:
            break
        }
        self.kind = kind
        self.streamID = streamID
        self.index = index
        self.total = total
        self.payload = payload
    }

    func encode() -> Data {
        var data = Self.magic
        data.append(Self.version)
        data.append(kind.rawValue)
        data.append(streamID)
        data.appendUInt16BE(UInt16(index))
        data.appendUInt16BE(UInt16(total))
        data.appendUInt16BE(UInt16(payload.count))
        data.append(payload)
        data.appendUInt32BE(KagemushaCRC32.checksum(data.dropFirst(2)))
        return data
    }

    static func decode(_ data: Data) throws -> Self {
        guard data.count >= fixedOverhead,
              data.count <= maximumEncodedBytes,
              data.prefix(2) == magic,
              data[2] == version,
              let kind = KagemushaQRStreamFrameKind(rawValue: data[3]) else {
            throw KagemushaQRStreamError.malformedFrame
        }
        let payloadLength = Int(data.uint16BE(at: 24))
        let payloadEnd = 26 + payloadLength
        guard payloadEnd + 4 == data.count else {
            throw KagemushaQRStreamError.malformedFrame
        }
        let expectedChecksum = data.uint32BE(at: payloadEnd)
        guard expectedChecksum == KagemushaCRC32.checksum(data[2..<payloadEnd]) else {
            throw KagemushaQRStreamError.checksumMismatch
        }
        return try Self(
            kind: kind,
            streamID: data.subdata(in: 4..<20),
            index: Int(data.uint16BE(at: 20)),
            total: Int(data.uint16BE(at: 22)),
            payload: data.subdata(in: 26..<payloadEnd)
        )
    }
}

enum KagemushaCRC32 {
    static func checksum<C: Collection>(_ bytes: C) -> UInt32 where C.Element == UInt8 {
        var crc: UInt32 = 0xFFFF_FFFF
        for byte in bytes {
            crc ^= UInt32(byte)
            for _ in 0..<8 {
                crc = (crc & 1) == 0 ? crc >> 1 : (crc >> 1) ^ 0xEDB8_8320
            }
        }
        return crc ^ 0xFFFF_FFFF
    }
}

private extension Data {
    mutating func appendUInt16BE(_ value: UInt16) {
        append(UInt8(truncatingIfNeeded: value >> 8))
        append(UInt8(truncatingIfNeeded: value))
    }

    mutating func appendUInt32BE(_ value: UInt32) {
        append(UInt8(truncatingIfNeeded: value >> 24))
        append(UInt8(truncatingIfNeeded: value >> 16))
        append(UInt8(truncatingIfNeeded: value >> 8))
        append(UInt8(truncatingIfNeeded: value))
    }

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
