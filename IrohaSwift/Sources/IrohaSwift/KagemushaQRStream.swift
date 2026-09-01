import CryptoKit
import Foundation

public enum KagemushaQRStreamError: Error, Equatable, LocalizedError, Sendable {
    case invalidOptions
    case payloadTooLarge(actual: Int, maximum: Int)
    case tooManyFrames(actual: Int, maximum: Int)
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
        case .tooManyFrames(let actual, let maximum):
            return "Kagemusha QR stream requires \(actual) frames; the limit is \(maximum)."
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
    /// Maximum number of frames in one stream, including its header.
    public static let maximumStreamFrames = 4_096

    /// Exact upper bound for an unpadded base64url frame, including `PKKQ1.`.
    /// This is checked before allocating the decoded frame bytes.
    static let maximumFrameTextBytes =
        KagemushaPeerTransportContract.qrStreamTextPrefix.utf8.count
            + (KagemushaQRStreamFrame.maximumEncodedBytes * 4 + 2) / 3

    public static func encode(
        _ payload: KagemushaPeerPayload,
        options: KagemushaQRStreamOptions = .standard
    ) throws -> [String] {
        var archive = payload.archive
        defer { archive.zeroize() }
        guard !archive.isEmpty,
              archive.count <= KagemushaPeerTransportContract.maximumArchiveBytes else {
            throw KagemushaQRStreamError.payloadTooLarge(
                actual: archive.count,
                maximum: KagemushaPeerTransportContract.maximumArchiveBytes
            )
        }
        var header = try KagemushaQRStreamEnvelope(
            kind: payload.kind,
            payload: archive,
            options: options
        )
        defer { header.zeroize() }
        var streamID = header.streamID
        defer { streamID.zeroize() }
        var frames = [try KagemushaQRStreamFrame(
            kind: .header,
            streamID: streamID,
            index: 0,
            total: 1,
            payload: header.encode()
        )]
        frames.reserveCapacity(1 + header.dataChunks + header.parityChunks)

        var chunks: [Data] = []
        chunks.reserveCapacity(header.dataChunks)
        defer {
            for index in chunks.indices { chunks[index].zeroize() }
            for index in frames.indices { frames[index].zeroize() }
        }
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

        return frames.map { frame in
            var encoded = frame.encode()
            defer { encoded.zeroize() }
            return KagemushaPeerTransportContract.qrStreamTextPrefix +
                KagemushaPeerTextCodec.base64URLEncode(encoded)
        }
    }

    static func preflightStreamFrameCount(
        payloadBytes: Int,
        options: KagemushaQRStreamOptions
    ) throws -> Int {
        guard (1...KagemushaPeerTransportContract.maximumArchiveBytes)
            .contains(payloadBytes) else {
            throw KagemushaQRStreamError.payloadTooLarge(
                actual: payloadBytes,
                maximum: KagemushaPeerTransportContract.maximumArchiveBytes
            )
        }
        let dataChunks = (payloadBytes + options.chunkSize - 1) / options.chunkSize
        let parityChunks = (dataChunks + options.parityGroup - 1) / options.parityGroup
        let frameCount = 1 + dataChunks + parityChunks
        guard frameCount <= maximumStreamFrames else {
            throw KagemushaQRStreamError.tooManyFrames(
                actual: frameCount,
                maximum: maximumStreamFrames
            )
        }
        return frameCount
    }

    static func decodeFrameText(_ value: String) throws -> KagemushaQRStreamFrame {
        let prefix = KagemushaPeerTransportContract.qrStreamTextPrefix
        guard value.utf8.count <= maximumFrameTextBytes,
              value.hasPrefix(prefix) else {
            throw KagemushaQRStreamError.nonCanonicalFrame
        }
        let body = String(value.dropFirst(prefix.count))
        guard var bytes = KagemushaPeerTextCodec.base64URLDecode(body) else {
            throw KagemushaQRStreamError.nonCanonicalFrame
        }
        defer { bytes.zeroize() }
        guard prefix + KagemushaPeerTextCodec.base64URLEncode(bytes) == value else {
            throw KagemushaQRStreamError.nonCanonicalFrame
        }
        return try KagemushaQRStreamFrame.decode(bytes)
    }
}

public final class KagemushaQRStreamDecoder: @unchecked Sendable {
    private let lock = NSLock()
    private let chainDiscriminant: UInt16
    private var streamID: Data?
    private var envelope: KagemushaQRStreamEnvelope?
    private var dataFrames: [Int: Data] = [:]
    private var parityFrames: [Int: Data] = [:]
    private var recovered = Set<Int>()
    private var completedPayload: KagemushaPeerPayload?

    public init(chainDiscriminant: UInt16) {
        self.chainDiscriminant = chainDiscriminant
    }

    public func reset() {
        lock.lock()
        defer { lock.unlock() }
        resetLocked()
    }

    public func ingest(_ frameText: String) throws -> KagemushaQRDecodeResult {
        var frame = try KagemushaQRStreamCodec.decodeFrameText(frameText)
        defer { frame.zeroize() }
        lock.lock()
        defer { lock.unlock() }
        return try ingestLocked(frame)
    }

    private func ingestLocked(_ frame: KagemushaQRStreamFrame) throws
        -> KagemushaQRDecodeResult
    {
        guard let header = envelope else {
            guard frame.kind == .header else {
                throw KagemushaQRStreamError.malformedFrame
            }
            var decoded = try KagemushaQRStreamEnvelope.decode(frame.payload)
            var decodedStreamID = decoded.streamID
            defer { decodedStreamID.zeroize() }
            guard decodedStreamID == frame.streamID else {
                decoded.zeroize()
                throw KagemushaQRStreamError.digestMismatch
            }
            streamID = frame.streamID
            envelope = decoded
            return result()
        }

        if let streamID {
            guard streamID == frame.streamID else {
                throw KagemushaQRStreamError.wrongStream
            }
        }

        switch frame.kind {
        case .header:
            var decoded = try KagemushaQRStreamEnvelope.decode(frame.payload)
            defer { decoded.zeroize() }
            var decodedStreamID = decoded.streamID
            defer { decodedStreamID.zeroize() }
            guard decodedStreamID == frame.streamID else {
                throw KagemushaQRStreamError.digestMismatch
            }
            if header != decoded {
                throw KagemushaQRStreamError.conflictingFrame
            }
        case .data:
            try ingestData(frame, header: header)
        case .parity:
            try ingestParity(frame, header: header)
        }
        return result()
    }

    private func ingestData(
        _ frame: KagemushaQRStreamFrame,
        header: KagemushaQRStreamEnvelope
    ) throws {
        guard frame.total == header.dataChunks,
              frame.index >= 0,
              frame.index < header.dataChunks,
              frame.payload.count == header.expectedDataChunkLength(index: frame.index) else {
            throw KagemushaQRStreamError.malformedFrame
        }
        if let existing = dataFrames[frame.index] {
            guard existing == frame.payload else {
                throw KagemushaQRStreamError.conflictingFrame
            }
            return
        }
        dataFrames[frame.index] = frame.payload
        try finishNewFrame(
            kind: .data,
            index: frame.index,
            parityGroup: frame.index / header.parityGroup,
            header: header
        )
    }

    private func ingestParity(
        _ frame: KagemushaQRStreamFrame,
        header: KagemushaQRStreamEnvelope
    ) throws {
        guard frame.total == header.parityChunks,
              frame.index >= 0,
              frame.index < header.parityChunks,
              frame.payload.count == header.chunkSize else {
            throw KagemushaQRStreamError.malformedFrame
        }
        if let existing = parityFrames[frame.index] {
            guard existing == frame.payload else {
                throw KagemushaQRStreamError.conflictingFrame
            }
            return
        }
        parityFrames[frame.index] = frame.payload
        try finishNewFrame(
            kind: .parity,
            index: frame.index,
            parityGroup: frame.index,
            header: header
        )
    }

    private func finishNewFrame(
        kind: KagemushaQRStreamFrameKind,
        index: Int,
        parityGroup: Int,
        header: KagemushaQRStreamEnvelope
    ) throws {
        var recoveredIndex: Int? = nil
        do {
            recoveredIndex = try recoverSingleMissingFrame(header, group: parityGroup)
        } catch {
            if let recoveredIndex {
                removeAndZeroize(recoveredIndex, from: &dataFrames)
                recovered.remove(recoveredIndex)
            }
            switch kind {
            case .data:
                removeAndZeroize(index, from: &dataFrames)
            case .parity:
                removeAndZeroize(index, from: &parityFrames)
            case .header:
                break
            }
            throw error
        }
        guard completedPayload == nil, dataFrames.count == header.dataChunks else {
            return
        }
        do {
            completedPayload = try finalizeComplete(header)
        } catch {
            // Exact coverage means another final-frame retry would repeat the
            // whole archive allocation/hash/decode. Consume the failed stream.
            resetLocked()
            throw error
        }
    }

    private func recoverSingleMissingFrame(
        _ envelope: KagemushaQRStreamEnvelope,
        group: Int
    ) throws -> Int? {
        guard let parity = parityFrames[group] else { return nil }
        let start = group * envelope.parityGroup
        let end = min(start + envelope.parityGroup, envelope.dataChunks)
        let missing = (start..<end).filter { dataFrames[$0] == nil }
        guard missing.count == 1 else { return nil }
        let missingIndex = missing[0]
        var recoveredChunk = parity
        for index in start..<end where index != missingIndex {
            guard let chunk = dataFrames[index] else {
                throw KagemushaQRStreamError.malformedFrame
            }
            for byteIndex in chunk.indices {
                recoveredChunk[byteIndex] ^= chunk[byteIndex]
            }
        }
        recoveredChunk = recoveredChunk.prefix(
            envelope.expectedDataChunkLength(index: missingIndex)
        )
        dataFrames[missingIndex] = recoveredChunk
        recovered.insert(missingIndex)
        return missingIndex
    }

    private func finalizeComplete(
        _ envelope: KagemushaQRStreamEnvelope
    ) throws -> KagemushaPeerPayload {
        guard dataFrames.count == envelope.dataChunks else {
            throw KagemushaQRStreamError.malformedFrame
        }
        var archive = Data()
        defer { archive.zeroize() }
        archive.reserveCapacity(envelope.totalBytes)
        for index in 0..<envelope.dataChunks {
            guard let chunk = dataFrames[index] else {
                throw KagemushaQRStreamError.malformedFrame
            }
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

    private func resetLocked() {
        streamID?.zeroize()
        streamID = nil
        envelope?.zeroize()
        envelope = nil
        while let index = dataFrames.keys.first {
            removeAndZeroize(index, from: &dataFrames)
        }
        while let index = parityFrames.keys.first {
            removeAndZeroize(index, from: &parityFrames)
        }
        recovered.removeAll(keepingCapacity: false)
        completedPayload = nil
    }

    private func removeAndZeroize(_ index: Int, from frames: inout [Int: Data]) {
        guard var bytes = frames.removeValue(forKey: index) else { return }
        bytes.zeroize()
    }
}

enum KagemushaQRStreamFrameKind: UInt8, Sendable {
    case header = 0
    case data = 1
    case parity = 2
}

struct KagemushaQRStreamEnvelope: Equatable, Sendable {
    static let version: UInt8 = 1
    static let encodedLength = 50

    let payloadKind: KagemushaPeerPayloadKind
    let parityGroup: Int
    let chunkSize: Int
    let dataChunks: Int
    let parityChunks: Int
    let totalBytes: Int
    private(set) var payloadDigest: Data

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
        _ = try KagemushaQRStreamCodec.preflightStreamFrameCount(
            payloadBytes: payload.count,
            options: options
        )
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
              1 + dataChunks + parityChunks <= KagemushaQRStreamCodec.maximumStreamFrames,
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
        data.appendUInt32BE(UInt32(dataChunks))
        data.appendUInt32BE(UInt32(parityChunks))
        data.appendUInt32BE(UInt32(totalBytes))
        data.append(payloadDigest)
        return data
    }

    mutating func zeroize() {
        payloadDigest.zeroize()
    }

    static func decode(_ data: Data) throws -> Self {
        guard data.count == encodedLength,
              data[0] == version,
              let kind = KagemushaPeerPayloadKind(rawValue: data[1]),
              data[3] == 0 else {
            throw KagemushaQRStreamError.invalidHeader
        }
        var digest = data.subdata(in: 18..<50)
        defer { digest.zeroize() }
        return try Self(
            payloadKind: kind,
            parityGroup: Int(data[2]),
            chunkSize: Int(data.uint16BE(at: 4)),
            dataChunks: Int(data.uint32BE(at: 6)),
            parityChunks: Int(data.uint32BE(at: 10)),
            totalBytes: Int(data.uint32BE(at: 14)),
            payloadDigest: digest
        )
    }
}

struct KagemushaQRStreamFrame: Equatable, Sendable {
    static let magic = Data([0x4B, 0x51])
    static let version: UInt8 = 1
    static let fixedOverhead = 34
    static let maximumEncodedBytes = fixedOverhead + KagemushaQRStreamOptions.maximumChunkSize
    let kind: KagemushaQRStreamFrameKind
    private(set) var streamID: Data
    let index: Int
    let total: Int
    private(set) var payload: Data

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
              total < KagemushaQRStreamCodec.maximumStreamFrames,
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
        data.appendUInt32BE(UInt32(index))
        data.appendUInt32BE(UInt32(total))
        data.appendUInt16BE(UInt16(payload.count))
        data.append(payload)
        data.appendUInt32BE(KagemushaCRC32.checksum(data.dropFirst(2)))
        return data
    }

    mutating func zeroize() {
        streamID.zeroize()
        payload.zeroize()
    }

    static func decode(_ data: Data) throws -> Self {
        guard data.count >= fixedOverhead,
              data.count <= maximumEncodedBytes,
              data.prefix(2) == magic,
              data[2] == version,
              let kind = KagemushaQRStreamFrameKind(rawValue: data[3]) else {
            throw KagemushaQRStreamError.malformedFrame
        }
        let payloadLength = Int(data.uint16BE(at: 28))
        let payloadEnd = 30 + payloadLength
        guard payloadEnd + 4 == data.count else {
            throw KagemushaQRStreamError.malformedFrame
        }
        let expectedChecksum = data.uint32BE(at: payloadEnd)
        guard expectedChecksum == KagemushaCRC32.checksum(data[2..<payloadEnd]) else {
            throw KagemushaQRStreamError.checksumMismatch
        }
        var decodedStreamID = data.subdata(in: 4..<20)
        var decodedPayload = data.subdata(in: 30..<payloadEnd)
        defer {
            decodedStreamID.zeroize()
            decodedPayload.zeroize()
        }
        return try Self(
            kind: kind,
            streamID: decodedStreamID,
            index: Int(data.uint32BE(at: 20)),
            total: Int(data.uint32BE(at: 24)),
            payload: decodedPayload
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
    mutating func zeroize() {
        resetBytes(in: startIndex..<endIndex)
    }

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
