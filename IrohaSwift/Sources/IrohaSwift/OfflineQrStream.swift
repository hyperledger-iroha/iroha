import Foundation

public enum OfflineQrStreamError: Error, LocalizedError {
    case invalidMagic
    case unsupportedVersion(UInt8)
    case invalidLength(String)
    case checksumMismatch
    case invalidEnvelope(String)
    case invalidStreamId

    public var errorDescription: String? {
        switch self {
        case .invalidMagic:
            return "QR stream magic mismatch"
        case .unsupportedVersion(let version):
            return "Unsupported QR stream version \(version)"
        case .invalidLength(let field):
            return "Invalid QR stream length for \(field)"
        case .checksumMismatch:
            return "QR stream frame checksum mismatch"
        case .invalidEnvelope(let reason):
            return "QR stream envelope invalid: \(reason)"
        case .invalidStreamId:
            return "QR stream id mismatch"
        }
    }
}

public enum OfflineQrStreamFrameKind: UInt8, Sendable {
    case header = 0
    case data = 1
    case parity = 2
}

public enum OfflineQrStreamFrameEncoding: Sendable {
    case binary
    case base64
}

public enum OfflineQrPayloadKind: UInt16, Sendable {
    case unspecified = 0
    case offlineToOnlineTransfer = 1
    case offlineSpendReceipt = 2
    case offlineEnvelope = 3
}

public struct OfflineQrStreamOptions: Sendable, Equatable {
    public var chunkSize: Int
    public var parityGroup: Int

    public init(chunkSize: Int = 360, parityGroup: Int = 0) {
        self.chunkSize = chunkSize
        self.parityGroup = parityGroup
    }
}

public struct OfflineQrStreamEnvelope: Sendable, Equatable {
    public static let version: UInt8 = 1
    public static let encodingBinary: UInt8 = 0

    public let flags: UInt8
    public let encoding: UInt8
    public let parityGroup: UInt8
    public let chunkSize: UInt16
    public let dataChunks: UInt16
    public let parityChunks: UInt16
    public let payloadKind: UInt16
    public let payloadLength: UInt32
    public let payloadHash: Data

    public var streamId: Data {
        payloadHash.prefix(16)
    }

    public init(
        flags: UInt8 = 0,
        encoding: UInt8 = OfflineQrStreamEnvelope.encodingBinary,
        parityGroup: UInt8,
        chunkSize: UInt16,
        dataChunks: UInt16,
        parityChunks: UInt16,
        payloadKind: UInt16,
        payloadLength: UInt32,
        payloadHash: Data
    ) throws {
        guard payloadHash.count == 32 else {
            throw OfflineQrStreamError.invalidEnvelope("payload_hash must be 32 bytes")
        }
        self.flags = flags
        self.encoding = encoding
        self.parityGroup = parityGroup
        self.chunkSize = chunkSize
        self.dataChunks = dataChunks
        self.parityChunks = parityChunks
        self.payloadKind = payloadKind
        self.payloadLength = payloadLength
        self.payloadHash = payloadHash
    }

    public func encode() -> Data {
        var data = Data()
        data.append(OfflineQrStreamEnvelope.version)
        data.append(flags)
        data.append(encoding)
        data.append(parityGroup)
        data.appendUInt16LE(chunkSize)
        data.appendUInt16LE(dataChunks)
        data.appendUInt16LE(parityChunks)
        data.appendUInt16LE(payloadKind)
        data.appendUInt32LE(payloadLength)
        data.append(payloadHash)
        return data
    }

    public static func decode(_ data: Data) throws -> OfflineQrStreamEnvelope {
        let bytes = [UInt8](data)
        let minLength = 1 + 1 + 1 + 1 + 2 + 2 + 2 + 2 + 4 + 32
        guard bytes.count >= minLength else {
            throw OfflineQrStreamError.invalidLength("envelope")
        }
        let version = bytes[0]
        guard version == OfflineQrStreamEnvelope.version else {
            throw OfflineQrStreamError.unsupportedVersion(version)
        }
        var offset = 1
        let flags = bytes[offset]
        offset += 1
        let encoding = bytes[offset]
        offset += 1
        let parityGroup = bytes[offset]
        offset += 1
        let chunkSize = readUInt16LE(bytes, offset)
        offset += 2
        let dataChunks = readUInt16LE(bytes, offset)
        offset += 2
        let parityChunks = readUInt16LE(bytes, offset)
        offset += 2
        let payloadKind = readUInt16LE(bytes, offset)
        offset += 2
        let payloadLength = readUInt32LE(bytes, offset)
        offset += 4
        let hashEnd = offset + 32
        guard bytes.count >= hashEnd else {
            throw OfflineQrStreamError.invalidLength("payload_hash")
        }
        let payloadHash = Data(bytes[offset..<hashEnd])
        return try OfflineQrStreamEnvelope(
            flags: flags,
            encoding: encoding,
            parityGroup: parityGroup,
            chunkSize: chunkSize,
            dataChunks: dataChunks,
            parityChunks: parityChunks,
            payloadKind: payloadKind,
            payloadLength: payloadLength,
            payloadHash: payloadHash
        )
    }
}

public struct OfflineQrStreamFrame: Sendable, Equatable {
    public static let magic: [UInt8] = [0x49, 0x51]
    public static let version: UInt8 = 1

    public let kind: OfflineQrStreamFrameKind
    public let streamId: Data
    public let index: UInt16
    public let total: UInt16
    public let payload: Data

    public init(kind: OfflineQrStreamFrameKind, streamId: Data, index: UInt16, total: UInt16, payload: Data) throws {
        guard streamId.count == 16 else {
            throw OfflineQrStreamError.invalidLength("stream_id")
        }
        self.kind = kind
        self.streamId = streamId
        self.index = index
        self.total = total
        self.payload = payload
    }

    public func encode() -> Data {
        var data = Data()
        data.append(contentsOf: OfflineQrStreamFrame.magic)
        data.append(OfflineQrStreamFrame.version)
        data.append(kind.rawValue)
        data.append(streamId)
        data.appendUInt16LE(index)
        data.appendUInt16LE(total)
        data.appendUInt16LE(UInt16(payload.count))
        data.append(payload)
        let crc = OfflineQrStreamChecksum.crc32(data: crcPayload())
        data.appendUInt32LE(crc)
        return data
    }

    public static func decode(_ data: Data) throws -> OfflineQrStreamFrame {
        let bytes = [UInt8](data)
        let headerLength = 2 + 1 + 1 + 16 + 2 + 2 + 2
        guard bytes.count >= headerLength + 4 else {
            throw OfflineQrStreamError.invalidLength("frame")
        }
        guard bytes[0] == OfflineQrStreamFrame.magic[0], bytes[1] == OfflineQrStreamFrame.magic[1] else {
            throw OfflineQrStreamError.invalidMagic
        }
        let version = bytes[2]
        guard version == OfflineQrStreamFrame.version else {
            throw OfflineQrStreamError.unsupportedVersion(version)
        }
        let kindRaw = bytes[3]
        guard let kind = OfflineQrStreamFrameKind(rawValue: kindRaw) else {
            throw OfflineQrStreamError.invalidEnvelope("unknown frame kind")
        }
        let streamStart = 4
        let streamEnd = streamStart + 16
        guard bytes.count >= streamEnd else {
            throw OfflineQrStreamError.invalidLength("stream_id")
        }
        let streamId = Data(bytes[streamStart..<streamEnd])
        var offset = streamEnd
        let index = readUInt16LE(bytes, offset)
        offset += 2
        let total = readUInt16LE(bytes, offset)
        offset += 2
        let payloadLength = Int(readUInt16LE(bytes, offset))
        offset += 2
        let payloadEnd = offset + payloadLength
        guard payloadEnd + 4 <= bytes.count else {
            throw OfflineQrStreamError.invalidLength("payload")
        }
        let payload = Data(bytes[offset..<payloadEnd])
        let crcStart = payloadEnd
        let crc = readUInt32LE(bytes, crcStart)
        let crcPayload = bytes[2..<payloadEnd]
        let computed = OfflineQrStreamChecksum.crc32(bytes: Array(crcPayload))
        guard crc == computed else {
            throw OfflineQrStreamError.checksumMismatch
        }
        return try OfflineQrStreamFrame(kind: kind, streamId: streamId, index: index, total: total, payload: payload)
    }

    private func crcPayload() -> Data {
        var data = Data()
        data.append(OfflineQrStreamFrame.version)
        data.append(kind.rawValue)
        data.append(streamId)
        data.appendUInt16LE(index)
        data.appendUInt16LE(total)
        data.appendUInt16LE(UInt16(payload.count))
        data.append(payload)
        return data
    }
}

public struct OfflineQrStreamDecodeResult: Sendable, Equatable {
    public let payload: Data?
    public let receivedChunks: Int
    public let totalChunks: Int
    public let recoveredChunks: Int

    public var progress: Double {
        guard totalChunks > 0 else { return 0 }
        return Double(receivedChunks) / Double(totalChunks)
    }

    public var isComplete: Bool {
        payload != nil
    }
}

public final class OfflineQrStreamEncoder {
    public static func encodeFrames(
        payload: Data,
        payloadKind: OfflineQrPayloadKind = .unspecified,
        options: OfflineQrStreamOptions = OfflineQrStreamOptions()
    ) throws -> [OfflineQrStreamFrame] {
        let chunkSize = options.chunkSize
        guard chunkSize > 0, chunkSize <= Int(UInt16.max) else {
            throw OfflineQrStreamError.invalidLength("chunk_size")
        }
        guard payload.count <= Int(UInt32.max) else {
            throw OfflineQrStreamError.invalidLength("payload_length")
        }
        let dataChunks = Int(ceil(Double(payload.count) / Double(chunkSize)))
        guard dataChunks <= Int(UInt16.max) else {
            throw OfflineQrStreamError.invalidLength("data_chunks")
        }
        let parityGroup = max(0, options.parityGroup)
        guard parityGroup <= Int(UInt8.max) else {
            throw OfflineQrStreamError.invalidLength("parity_group")
        }
        let parityChunks = parityGroup > 0 ? Int(ceil(Double(dataChunks) / Double(parityGroup))) : 0
        guard parityChunks <= Int(UInt16.max) else {
            throw OfflineQrStreamError.invalidLength("parity_chunks")
        }
        let payloadHash = Blake2b.hash256(payload)
        let envelope = try OfflineQrStreamEnvelope(
            parityGroup: UInt8(parityGroup),
            chunkSize: UInt16(chunkSize),
            dataChunks: UInt16(dataChunks),
            parityChunks: UInt16(parityChunks),
            payloadKind: payloadKind.rawValue,
            payloadLength: UInt32(payload.count),
            payloadHash: payloadHash
        )
        var frames: [OfflineQrStreamFrame] = []
        let header = try OfflineQrStreamFrame(
            kind: .header,
            streamId: envelope.streamId,
            index: 0,
            total: 1,
            payload: envelope.encode()
        )
        frames.append(header)
        for index in 0..<dataChunks {
            let start = index * chunkSize
            let end = min(payload.count, start + chunkSize)
            let chunk = payload.subdata(in: start..<end)
            let frame = try OfflineQrStreamFrame(
                kind: .data,
                streamId: envelope.streamId,
                index: UInt16(index),
                total: UInt16(dataChunks),
                payload: chunk
            )
            frames.append(frame)
        }
        if parityGroup > 0 {
            for groupIndex in 0..<parityChunks {
                let parity = xorParity(
                    payload: payload,
                    chunkSize: chunkSize,
                    dataChunks: dataChunks,
                    groupIndex: groupIndex,
                    groupSize: parityGroup
                )
                let frame = try OfflineQrStreamFrame(
                    kind: .parity,
                    streamId: envelope.streamId,
                    index: UInt16(groupIndex),
                    total: UInt16(parityChunks),
                    payload: parity
                )
                frames.append(frame)
            }
        }
        return frames
    }

    public static func encodeFrameBytes(
        payload: Data,
        payloadKind: OfflineQrPayloadKind = .unspecified,
        options: OfflineQrStreamOptions = OfflineQrStreamOptions()
    ) throws -> [Data] {
        try encodeFrames(payload: payload, payloadKind: payloadKind, options: options).map { $0.encode() }
    }

    private static func xorParity(
        payload: Data,
        chunkSize: Int,
        dataChunks: Int,
        groupIndex: Int,
        groupSize: Int
    ) -> Data {
        var parity = [UInt8](repeating: 0, count: chunkSize)
        let startIndex = groupIndex * groupSize
        let endIndex = min(dataChunks, startIndex + groupSize)
        guard startIndex < endIndex else {
            return Data(parity)
        }
        for chunkIndex in startIndex..<endIndex {
            let start = chunkIndex * chunkSize
            let end = min(payload.count, start + chunkSize)
            let chunk = payload.subdata(in: start..<end)
            for (offset, byte) in chunk.enumerated() {
                parity[offset] ^= byte
            }
        }
        return Data(parity)
    }
}

public final class OfflineQrStreamDecoder {
    private var envelope: OfflineQrStreamEnvelope?
    private var dataChunks: [Data?] = []
    private var parityChunks: [Data?] = []
    private var pendingFrames: [OfflineQrStreamFrame] = []
    private var recovered: Set<Int> = []

    public init() {}

    public func ingest(frameBytes: Data) throws -> OfflineQrStreamDecodeResult {
        let frame = try OfflineQrStreamFrame.decode(frameBytes)
        try ingest(frame: frame)
        if let payload = try finalizeIfComplete() {
            return OfflineQrStreamDecodeResult(
                payload: payload,
                receivedChunks: dataChunks.compactMap { $0 }.count,
                totalChunks: dataChunks.count,
                recoveredChunks: recovered.count
            )
        }
        return OfflineQrStreamDecodeResult(
            payload: nil,
            receivedChunks: dataChunks.compactMap { $0 }.count,
            totalChunks: dataChunks.count,
            recoveredChunks: recovered.count
        )
    }

    private func ingest(frame: OfflineQrStreamFrame) throws {
        switch frame.kind {
        case .header:
            let envelope = try OfflineQrStreamEnvelope.decode(frame.payload)
            let streamId = envelope.streamId
            guard streamId == frame.streamId else {
                throw OfflineQrStreamError.invalidStreamId
            }
            self.envelope = envelope
            dataChunks = Array(repeating: nil, count: Int(envelope.dataChunks))
            parityChunks = Array(repeating: nil, count: Int(envelope.parityChunks))
            if !pendingFrames.isEmpty {
                let buffered = pendingFrames
                pendingFrames.removeAll()
                for bufferedFrame in buffered where bufferedFrame.streamId == streamId {
                    try ingest(frame: bufferedFrame)
                }
            }
        case .data, .parity:
            guard let envelope = envelope else {
                pendingFrames.append(frame)
                return
            }
            guard frame.streamId == envelope.streamId else {
                return
            }
            switch frame.kind {
            case .data:
                storeData(frame: frame, envelope: envelope)
            case .parity:
                storeParity(frame: frame, envelope: envelope)
            case .header:
                break
            }
            try recoverMissing(envelope: envelope)
        }
    }

    private func storeData(frame: OfflineQrStreamFrame, envelope: OfflineQrStreamEnvelope) {
        let index = Int(frame.index)
        guard index < dataChunks.count else { return }
        if dataChunks[index] == nil {
            dataChunks[index] = frame.payload
        }
    }

    private func storeParity(frame: OfflineQrStreamFrame, envelope: OfflineQrStreamEnvelope) {
        let index = Int(frame.index)
        guard index < parityChunks.count else { return }
        if parityChunks[index] == nil {
            parityChunks[index] = frame.payload
        }
    }

    private func recoverMissing(envelope: OfflineQrStreamEnvelope) throws {
        let groupSize = Int(envelope.parityGroup)
        guard groupSize > 0 else { return }
        let chunkSize = Int(envelope.chunkSize)
        for groupIndex in 0..<parityChunks.count {
            guard let parity = parityChunks[groupIndex] else { continue }
            let startIndex = groupIndex * groupSize
            let endIndex = min(dataChunks.count, startIndex + groupSize)
            guard startIndex < endIndex else { continue }
            var missingIndex: Int?
            var xor = [UInt8](parity)
            for dataIndex in startIndex..<endIndex {
                if let chunk = dataChunks[dataIndex] {
                    var padded = [UInt8](repeating: 0, count: chunkSize)
                    let bytes = [UInt8](chunk)
                    for offset in 0..<min(bytes.count, chunkSize) {
                        padded[offset] = bytes[offset]
                    }
                    for offset in 0..<chunkSize {
                        xor[offset] ^= padded[offset]
                    }
                } else {
                    if missingIndex == nil {
                        missingIndex = dataIndex
                    } else {
                        missingIndex = nil
                        break
                    }
                }
            }
            if let missingIndex = missingIndex {
                let length = expectedChunkLength(envelope: envelope, index: missingIndex)
                let recoveredChunk = Data(xor.prefix(length))
                dataChunks[missingIndex] = recoveredChunk
                recovered.insert(missingIndex)
            }
        }
    }

    private func expectedChunkLength(envelope: OfflineQrStreamEnvelope, index: Int) -> Int {
        let chunkSize = Int(envelope.chunkSize)
        let total = Int(envelope.dataChunks)
        guard total > 0 else { return 0 }
        if index < total - 1 {
            return chunkSize
        }
        let payloadLength = Int(envelope.payloadLength)
        let tail = payloadLength - chunkSize * (total - 1)
        return max(0, min(chunkSize, tail))
    }

    private func finalizeIfComplete() throws -> Data? {
        guard let envelope = envelope else { return nil }
        if dataChunks.contains(where: { $0 == nil }) {
            return nil
        }
        var payload = Data()
        for (index, chunk) in dataChunks.enumerated() {
            guard let chunk = chunk else { continue }
            let length = expectedChunkLength(envelope: envelope, index: index)
            if chunk.count > length {
                payload.append(chunk.prefix(length))
            } else {
                payload.append(chunk)
            }
        }
        if payload.count != Int(envelope.payloadLength) {
            payload = payload.prefix(Int(envelope.payloadLength))
        }
        let hash = Blake2b.hash256(payload)
        guard hash == envelope.payloadHash else {
            throw OfflineQrStreamError.invalidEnvelope("payload_hash mismatch")
        }
        return payload
    }
}

public struct OfflineQrStreamColor: Sendable, Equatable {
    public let red: Double
    public let green: Double
    public let blue: Double

    public init(red: Double, green: Double, blue: Double) {
        self.red = red
        self.green = green
        self.blue = blue
    }
}

public struct OfflineQrStreamFrameStyle: Sendable, Equatable {
    public let petalPhase: Double
    public let accentStrength: Double
    public let gradientAngle: Double
}

public struct OfflineQrStreamPlaybackStyle: Sendable, Equatable {
    public let petalPhase: Double
    public let accentStrength: Double
    public let gradientAngle: Double
    public let driftOffset: Double
    public let progressAlpha: Double
}

public struct OfflineQrStreamTheme: Sendable, Equatable {
    public let name: String
    public let backgroundStart: OfflineQrStreamColor
    public let backgroundEnd: OfflineQrStreamColor
    public let accent: OfflineQrStreamColor
    public let petal: OfflineQrStreamColor
    public let petalCount: Int
    public let pulsePeriod: Double

    public static let sakura = OfflineQrStreamTheme(
        name: "sakura",
        backgroundStart: OfflineQrStreamColor(red: 0.98, green: 0.94, blue: 0.96),
        backgroundEnd: OfflineQrStreamColor(red: 1.00, green: 0.98, blue: 0.99),
        accent: OfflineQrStreamColor(red: 0.92, green: 0.48, blue: 0.60),
        petal: OfflineQrStreamColor(red: 0.98, green: 0.80, blue: 0.86),
        petalCount: 6,
        pulsePeriod: 48
    )

    public static let sakuraStorm = OfflineQrStreamTheme(
        name: "sakura-storm",
        backgroundStart: OfflineQrStreamColor(red: 0.05, green: 0.02, blue: 0.08),
        backgroundEnd: OfflineQrStreamColor(red: 0.02, green: 0.01, blue: 0.04),
        accent: OfflineQrStreamColor(red: 0.95, green: 0.71, blue: 0.87),
        petal: OfflineQrStreamColor(red: 0.98, green: 0.92, blue: 0.97),
        petalCount: 8,
        pulsePeriod: 36
    )

    public func frameStyle(frameIndex: Int, totalFrames: Int) -> OfflineQrStreamFrameStyle {
        let safeTotal = max(totalFrames, 1)
        let phase = Double(frameIndex % safeTotal) / Double(safeTotal)
        let pulse = (sin((Double(frameIndex) / pulsePeriod) * Double.pi * 2) + 1) / 2
        let angle = phase * 360.0
        return OfflineQrStreamFrameStyle(petalPhase: phase, accentStrength: pulse, gradientAngle: angle)
    }
}

public struct OfflineQrStreamPlaybackSkin: Sendable, Equatable {
    public let name: String
    public let theme: OfflineQrStreamTheme
    public let frameRate: Double
    public let petalDriftSpeed: Double
    public let progressOverlayAlpha: Double
    public let reducedMotion: Bool
    public let lowPower: Bool

    public static let sakura = OfflineQrStreamPlaybackSkin(
        name: "sakura",
        theme: .sakura,
        frameRate: 12,
        petalDriftSpeed: 1.0,
        progressOverlayAlpha: 0.4,
        reducedMotion: false,
        lowPower: false
    )

    public static let sakuraReducedMotion = OfflineQrStreamPlaybackSkin(
        name: "sakura-reduced-motion",
        theme: .sakura,
        frameRate: 6,
        petalDriftSpeed: 0.0,
        progressOverlayAlpha: 0.25,
        reducedMotion: true,
        lowPower: false
    )

    public static let sakuraLowPower = OfflineQrStreamPlaybackSkin(
        name: "sakura-low-power",
        theme: OfflineQrStreamTheme(
            name: "sakura-low-power",
            backgroundStart: OfflineQrStreamTheme.sakura.backgroundStart,
            backgroundEnd: OfflineQrStreamTheme.sakura.backgroundEnd,
            accent: OfflineQrStreamTheme.sakura.accent,
            petal: OfflineQrStreamTheme.sakura.petal,
            petalCount: 4,
            pulsePeriod: 72
        ),
        frameRate: 8,
        petalDriftSpeed: 0.4,
        progressOverlayAlpha: 0.3,
        reducedMotion: false,
        lowPower: true
    )

    public static let sakuraStorm = OfflineQrStreamPlaybackSkin(
        name: "sakura-storm",
        theme: .sakuraStorm,
        frameRate: 12,
        petalDriftSpeed: 0.6,
        progressOverlayAlpha: 0.34,
        reducedMotion: false,
        lowPower: false
    )

    public func frameStyle(frameIndex: Int, totalFrames: Int, progress: Double) -> OfflineQrStreamPlaybackStyle {
        let safeTotal = max(totalFrames, 1)
        let phase = Double(frameIndex % safeTotal) / Double(safeTotal)
        let pulse = (sin((Double(frameIndex) / theme.pulsePeriod) * Double.pi * 2) + 1) / 2
        let angle = phase * 360.0
        let drift = reducedMotion ? 0.0 : sin(phase * Double.pi * 2) * petalDriftSpeed
        let progressClamped = min(max(progress, 0), 1)
        return OfflineQrStreamPlaybackStyle(
            petalPhase: phase,
            accentStrength: pulse,
            gradientAngle: angle,
            driftOffset: drift,
            progressAlpha: progressOverlayAlpha * progressClamped
        )
    }
}

public final class OfflineQrStreamScanSession {
    private let decoder = OfflineQrStreamDecoder()

    public init() {}

    public func ingest(frameBytes: Data) throws -> OfflineQrStreamDecodeResult {
        try decoder.ingest(frameBytes: frameBytes)
    }

    public func ingest(frameString: String, encoding: OfflineQrStreamFrameEncoding = .base64) throws -> OfflineQrStreamDecodeResult {
        let bytes = try OfflineQrStreamTextCodec.decode(frameString, encoding: encoding)
        return try ingest(frameBytes: bytes)
    }
}

public enum OfflineQrStreamTextCodec {
    private static let prefix = "iroha:qr1:"

    public static func encode(_ data: Data, encoding: OfflineQrStreamFrameEncoding) -> String {
        switch encoding {
        case .binary:
            return data.base64EncodedString()
        case .base64:
            return prefix + data.base64EncodedString()
        }
    }

    public static func decode(_ value: String, encoding: OfflineQrStreamFrameEncoding) throws -> Data {
        switch encoding {
        case .binary:
            guard let decoded = Data(base64Encoded: value) else {
                throw OfflineQrStreamError.invalidEnvelope("frame text is not base64")
            }
            return decoded
        case .base64:
            guard let stripped = value.trimmingCharacters(in: .whitespacesAndNewlines).stripPrefix(prefix) else {
                throw OfflineQrStreamError.invalidEnvelope("qr text prefix missing")
            }
            guard let decoded = Data(base64Encoded: stripped) else {
                throw OfflineQrStreamError.invalidEnvelope("frame text is not base64")
            }
            return decoded
        }
    }
}

private extension String {
    func stripPrefix(_ prefix: String) -> String? {
        guard hasPrefix(prefix) else { return nil }
        return String(dropFirst(prefix.count))
    }
}

private enum OfflineQrStreamChecksum {
    private static let table: [UInt32] = {
        var table = [UInt32](repeating: 0, count: 256)
        for i in 0..<256 {
            var c = UInt32(i)
            for _ in 0..<8 {
                if c & 1 == 1 {
                    c = 0xEDB88320 ^ (c >> 1)
                } else {
                    c = c >> 1
                }
            }
            table[i] = c
        }
        return table
    }()

    static func crc32(data: Data) -> UInt32 {
        crc32(bytes: [UInt8](data))
    }

    static func crc32(bytes: [UInt8]) -> UInt32 {
        var crc: UInt32 = 0xFFFFFFFF
        for byte in bytes {
            let index = Int((crc ^ UInt32(byte)) & 0xFF)
            crc = table[index] ^ (crc >> 8)
        }
        return crc ^ 0xFFFFFFFF
    }
}

private extension Data {
    mutating func appendUInt16LE(_ value: UInt16) {
        var le = value.littleEndian
        Swift.withUnsafeBytes(of: &le) { append(contentsOf: $0) }
    }

    mutating func appendUInt32LE(_ value: UInt32) {
        var le = value.littleEndian
        Swift.withUnsafeBytes(of: &le) { append(contentsOf: $0) }
    }
}

private func readUInt16LE(_ bytes: [UInt8], _ offset: Int) -> UInt16 {
    let b0 = UInt16(bytes[offset])
    let b1 = UInt16(bytes[offset + 1]) << 8
    return b0 | b1
}

private func readUInt32LE(_ bytes: [UInt8], _ offset: Int) -> UInt32 {
    var value: UInt32 = 0
    for i in 0..<4 {
        value |= UInt32(bytes[offset + i]) << (UInt32(i) * 8)
    }
    return value
}
