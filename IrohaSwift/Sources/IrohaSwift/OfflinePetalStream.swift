import Foundation

public enum OfflinePetalStreamError: Error, LocalizedError {
    case invalidOptions(String)
    case payloadTooLarge
    case capacityExceeded
    case invalidHeader(String)
    case checksumMismatch
    case anchorContrast

    public var errorDescription: String? {
        switch self {
        case .invalidOptions(let reason):
            return "Petal stream options invalid: \(reason)"
        case .payloadTooLarge:
            return "Petal stream payload length exceeds 65535 bytes"
        case .capacityExceeded:
            return "Petal stream grid too small for payload"
        case .invalidHeader(let reason):
            return "Petal stream header invalid: \(reason)"
        case .checksumMismatch:
            return "Petal stream checksum mismatch"
        case .anchorContrast:
            return "Petal stream anchor contrast too low"
        }
    }
}

public struct OfflinePetalStreamOptions: Sendable, Equatable {
    public var gridSize: UInt16
    public var border: UInt8
    public var anchorSize: UInt8

    public init(gridSize: UInt16 = 0, border: UInt8 = 1, anchorSize: UInt8 = 3) {
        self.gridSize = gridSize
        self.border = border
        self.anchorSize = anchorSize
    }
}

public struct OfflinePetalStreamGrid: Sendable, Equatable {
    public let gridSize: UInt16
    public let cells: [Bool]

    public init(gridSize: UInt16, cells: [Bool]) throws {
        let expected = Int(gridSize) * Int(gridSize)
        guard gridSize > 0, cells.count == expected else {
            throw OfflinePetalStreamError.invalidOptions("grid size mismatch")
        }
        self.gridSize = gridSize
        self.cells = cells
    }

    public func get(x: UInt16, y: UInt16) -> Bool? {
        guard x < gridSize, y < gridSize else {
            return nil
        }
        let idx = Int(y) * Int(gridSize) + Int(x)
        return cells[idx]
    }
}

public struct OfflinePetalStreamSampleGrid: Sendable, Equatable {
    public let gridSize: UInt16
    public let samples: [UInt8]

    public init(gridSize: UInt16, samples: [UInt8]) throws {
        let expected = Int(gridSize) * Int(gridSize)
        guard gridSize > 0, samples.count == expected else {
            throw OfflinePetalStreamError.invalidOptions("sample grid size mismatch")
        }
        self.gridSize = gridSize
        self.samples = samples
    }
}

public enum OfflinePetalStreamEncoder {
    public static let gridSizes: [UInt16] = [33, 37, 41, 45, 49, 53, 57, 61, 65, 69]

    public static func encodeGrid(
        _ payload: Data,
        options: OfflinePetalStreamOptions = OfflinePetalStreamOptions()
    ) throws -> OfflinePetalStreamGrid {
        if payload.count > Int(UInt16.max) {
            throw OfflinePetalStreamError.payloadTooLarge
        }
        let resolved = try resolveGridSize(payloadLength: payload.count, options: options)
        let capacity = try capacityBits(gridSize: resolved, options: options)
        let bitsNeeded = (headerLength + payload.count) * 8
        if bitsNeeded > capacity {
            throw OfflinePetalStreamError.capacityExceeded
        }
        let header = try encodeHeader(payload)
        var bits: [Bool] = []
        bits.reserveCapacity(bitsNeeded)
        pushBytesAsBits(header, into: &bits)
        pushBytesAsBits(payload, into: &bits)

        let totalCells = Int(resolved) * Int(resolved)
        var cells = [Bool](repeating: false, count: totalCells)
        var bitIndex = 0
        for y in 0..<resolved {
            for x in 0..<resolved {
                let idx = Int(y) * Int(resolved) + Int(x)
                switch cellRole(x: x, y: y, gridSize: resolved, options: options) {
                case .border, .anchorDark:
                    cells[idx] = true
                case .anchorLight:
                    cells[idx] = false
                case .data:
                    if bitIndex < bits.count {
                        cells[idx] = bits[bitIndex]
                        bitIndex += 1
                    }
                }
            }
        }
        return try OfflinePetalStreamGrid(gridSize: resolved, cells: cells)
    }

    public static func encodeGrids(
        _ payloads: [Data],
        options: OfflinePetalStreamOptions = OfflinePetalStreamOptions()
    ) throws -> (gridSize: UInt16, grids: [OfflinePetalStreamGrid]) {
        let maxLen = payloads.map(\.count).max() ?? 0
        let resolved = options.gridSize == 0
            ? try resolveGridSize(payloadLength: maxLen, options: options)
            : options.gridSize
        let fixed = OfflinePetalStreamOptions(
            gridSize: resolved,
            border: options.border,
            anchorSize: options.anchorSize
        )
        let grids = try payloads.map { try encodeGrid($0, options: fixed) }
        return (resolved, grids)
    }
}

public enum OfflinePetalStreamDecoder {
    public static func decodeGrid(
        _ grid: OfflinePetalStreamGrid,
        options: OfflinePetalStreamOptions = OfflinePetalStreamOptions()
    ) throws -> Data {
        let resolved = try resolveGridSizeForDecode(gridSize: grid.gridSize, options: options)
        guard resolved == grid.gridSize else {
            throw OfflinePetalStreamError.invalidOptions("grid size mismatch")
        }
        let capacity = try capacityBits(gridSize: resolved, options: options)
        var bits: [Bool] = []
        bits.reserveCapacity(capacity)
        for y in 0..<resolved {
            for x in 0..<resolved {
                if cellRole(x: x, y: y, gridSize: resolved, options: options) == .data {
                    if let bit = grid.get(x: x, y: y) {
                        bits.append(bit)
                    }
                }
            }
        }
        let bytes = bitsToBytes(bits)
        return try decodePayload(bytes)
    }

    public static func decodeSamples(
        _ samples: OfflinePetalStreamSampleGrid,
        options: OfflinePetalStreamOptions = OfflinePetalStreamOptions()
    ) throws -> Data {
        let resolved = try resolveGridSizeForDecode(gridSize: samples.gridSize, options: options)
        guard resolved == samples.gridSize else {
            throw OfflinePetalStreamError.invalidOptions("sample grid size mismatch")
        }
        var darkSum = 0
        var lightSum = 0
        var darkCount = 0
        var lightCount = 0
        for y in 0..<resolved {
            for x in 0..<resolved {
                let idx = Int(y) * Int(resolved) + Int(x)
                let value = Int(samples.samples[idx])
                switch cellRole(x: x, y: y, gridSize: resolved, options: options) {
                case .anchorDark:
                    darkSum += value
                    darkCount += 1
                case .anchorLight:
                    lightSum += value
                    lightCount += 1
                case .border, .data:
                    break
                }
            }
        }
        guard darkCount > 0, lightCount > 0 else {
            throw OfflinePetalStreamError.invalidOptions("anchor sampling failed")
        }
        let darkAvg = Double(darkSum) / Double(darkCount)
        let lightAvg = Double(lightSum) / Double(lightCount)
        guard darkAvg < lightAvg else {
            throw OfflinePetalStreamError.anchorContrast
        }
        let threshold = UInt8(((darkAvg + lightAvg) / 2.0).rounded())
        var cells = [Bool](repeating: false, count: samples.samples.count)
        for (idx, sample) in samples.samples.enumerated() {
            cells[idx] = sample < threshold
        }
        let grid = try OfflinePetalStreamGrid(gridSize: resolved, cells: cells)
        return try decodeGrid(grid, options: options)
    }
}

public enum OfflinePetalStreamSampler {
    public static func sampleGridFromRGBA(
        rgba: Data,
        width: Int,
        height: Int,
        gridSize: UInt16
    ) throws -> OfflinePetalStreamSampleGrid {
        guard width > 0, height > 0 else {
            throw OfflinePetalStreamError.invalidOptions("image dimensions must be positive")
        }
        guard gridSize > 0 else {
            throw OfflinePetalStreamError.invalidOptions("grid size is zero")
        }
        guard rgba.count >= width * height * 4 else {
            throw OfflinePetalStreamError.invalidOptions("image data length is too small")
        }
        let size = min(width, height)
        let offsetX = (width - size) / 2
        let offsetY = (height - size) / 2
        let cellSize = max(1, size / Int(gridSize))
        let bytes = [UInt8](rgba)
        var samples: [UInt8] = []
        samples.reserveCapacity(Int(gridSize) * Int(gridSize))
        for y in 0..<gridSize {
            for x in 0..<gridSize {
                var sum = 0
                var count = 0
                for oy in [0.25, 0.5, 0.75] {
                    for ox in [0.25, 0.5, 0.75] {
                        let px = min(
                            width - 1,
                            offsetX + Int((Double(x) + ox) * Double(cellSize))
                        )
                        let py = min(
                            height - 1,
                            offsetY + Int((Double(y) + oy) * Double(cellSize))
                        )
                        let idx = (py * width + px) * 4
                        if idx + 2 >= bytes.count {
                            continue
                        }
                        let r = Int(bytes[idx])
                        let g = Int(bytes[idx + 1])
                        let b = Int(bytes[idx + 2])
                        let luma = (77 * r + 150 * g + 29 * b) >> 8
                        sum += luma
                        count += 1
                    }
                }
                let avg = count == 0 ? 0 : sum / count
                samples.append(UInt8(avg))
            }
        }
        return try OfflinePetalStreamSampleGrid(gridSize: gridSize, samples: samples)
    }
}

public final class OfflinePetalStreamScanSession {
    public let qrSession: OfflineQrStreamScanSession
    public private(set) var gridSize: UInt16?
    private let options: OfflinePetalStreamOptions

    public init(
        qrSession: OfflineQrStreamScanSession = OfflineQrStreamScanSession(),
        options: OfflinePetalStreamOptions = OfflinePetalStreamOptions()
    ) {
        self.qrSession = qrSession
        self.options = options
        self.gridSize = options.gridSize == 0 ? nil : options.gridSize
    }

    public func ingest(sampleGrid: OfflinePetalStreamSampleGrid) throws -> OfflineQrStreamDecodeResult {
        if gridSize == nil {
            gridSize = sampleGrid.gridSize
        }
        let payload = try OfflinePetalStreamDecoder.decodeSamples(sampleGrid, options: options)
        return try qrSession.ingest(frameBytes: payload)
    }
}

private enum CellRole {
    case border
    case anchorDark
    case anchorLight
    case data
}

private let headerLength = 9
private let magic: [UInt8] = [0x50, 0x53]
private let version: UInt8 = 1

private func resolveGridSize(
    payloadLength: Int,
    options: OfflinePetalStreamOptions
) throws -> UInt16 {
    guard options.border > 0 else {
        throw OfflinePetalStreamError.invalidOptions("border must be > 0")
    }
    guard options.anchorSize > 0 else {
        throw OfflinePetalStreamError.invalidOptions("anchorSize must be > 0")
    }
    let bitsNeeded = (headerLength + payloadLength) * 8
    if options.gridSize != 0 {
        let capacity = try capacityBits(gridSize: options.gridSize, options: options)
        guard bitsNeeded <= capacity else {
            throw OfflinePetalStreamError.capacityExceeded
        }
        return options.gridSize
    }
    for candidate in OfflinePetalStreamEncoder.gridSizes {
        let capacity = try capacityBits(gridSize: candidate, options: options)
        if bitsNeeded <= capacity {
            return candidate
        }
    }
    throw OfflinePetalStreamError.capacityExceeded
}

private func resolveGridSizeForDecode(
    gridSize: UInt16,
    options: OfflinePetalStreamOptions
) throws -> UInt16 {
    if options.gridSize != 0 {
        return options.gridSize
    }
    guard gridSize > 0 else {
        throw OfflinePetalStreamError.invalidOptions("grid size is zero")
    }
    return gridSize
}

private func capacityBits(
    gridSize: UInt16,
    options: OfflinePetalStreamOptions
) throws -> Int {
    let border = Int(options.border)
    let anchor = Int(options.anchorSize)
    let grid = Int(gridSize)
    guard grid > 0 else {
        throw OfflinePetalStreamError.invalidOptions("grid size must be > 0")
    }
    let minGrid = border * 2 + anchor * 2 + 1
    guard grid >= minGrid else {
        throw OfflinePetalStreamError.invalidOptions("grid size too small for anchors")
    }
    let total = grid * grid
    let borderCells = grid * 4 - 4
    let anchorCells = anchor * anchor * 4
    let dataCells = max(0, total - borderCells - anchorCells)
    return dataCells
}

private func cellRole(
    x: UInt16,
    y: UInt16,
    gridSize: UInt16,
    options: OfflinePetalStreamOptions
) -> CellRole {
    let border = UInt16(options.border)
    let anchor = UInt16(options.anchorSize)
    if x < border || y < border || x >= gridSize - border || y >= gridSize - border {
        return .border
    }
    let right = gridSize - border - anchor
    let bottom = gridSize - border - anchor
    let inLeft = x >= border && x < border + anchor
    let inRight = x >= right && x < right + anchor
    let inTop = y >= border && y < border + anchor
    let inBottom = y >= bottom && y < bottom + anchor
    if inLeft && inTop {
        return .anchorDark
    }
    if inLeft && inBottom {
        return .anchorDark
    }
    if inRight && inTop {
        return .anchorLight
    }
    if inRight && inBottom {
        return .anchorLight
    }
    return .data
}

private func encodeHeader(_ payload: Data) throws -> Data {
    guard payload.count <= Int(UInt16.max) else {
        throw OfflinePetalStreamError.payloadTooLarge
    }
    var data = Data()
    data.append(contentsOf: magic)
    data.append(version)
    data.appendUInt16LE(UInt16(payload.count))
    data.appendUInt32LE(OfflinePetalStreamChecksum.crc32(data: payload))
    return data
}

private func decodePayload(_ bytes: Data) throws -> Data {
    let raw = [UInt8](bytes)
    guard raw.count >= headerLength else {
        throw OfflinePetalStreamError.invalidHeader("header too short")
    }
    guard raw[0] == magic[0], raw[1] == magic[1] else {
        throw OfflinePetalStreamError.invalidHeader("magic mismatch")
    }
    guard raw[2] == version else {
        throw OfflinePetalStreamError.invalidHeader("unsupported version")
    }
    let payloadLength = Int(readUInt16LE(raw, 3))
    let crc = readUInt32LE(raw, 5)
    let start = headerLength
    let end = start + payloadLength
    guard end <= raw.count else {
        throw OfflinePetalStreamError.invalidHeader("payload length exceeds data")
    }
    let payload = bytes.subdata(in: start..<end)
    let expected = OfflinePetalStreamChecksum.crc32(data: payload)
    guard expected == crc else {
        throw OfflinePetalStreamError.checksumMismatch
    }
    return payload
}

private func pushBytesAsBits(_ bytes: Data, into out: inout [Bool]) {
    for byte in bytes {
        for bit in (0..<8).reversed() {
            out.append(byte & (1 << bit) != 0)
        }
    }
}

private func bitsToBytes(_ bits: [Bool]) -> Data {
    var out = Data()
    var index = 0
    while index < bits.count {
        var value: UInt8 = 0
        for offset in 0..<8 where index + offset < bits.count {
            if bits[index + offset] {
                value |= 1 << (7 - offset)
            }
        }
        out.append(value)
        index += 8
    }
    return out
}

private enum OfflinePetalStreamChecksum {
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
        var crc: UInt32 = 0xFFFFFFFF
        for byte in data {
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
