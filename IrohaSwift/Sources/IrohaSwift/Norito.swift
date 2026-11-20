import Foundation
import CryptoKit

public enum NoritoCompression: UInt8 { case none = 0 }

/// A Norito payload layout (`norito.md`, "Dense layout (header minor 1)").
///
/// A frame declares its layout in the header (minor version and flag byte); a
/// headerless payload's owner names it explicitly. Nothing infers a layout from
/// payload bytes, and a payload of one layout never decodes as another.
public enum NoritoWireLayout: Sendable, Equatable, Hashable, CaseIterable {
    /// Minor `0` with fixed-width `u64` per-value lengths (header flags `0x00`).
    case fixed
    /// Minor `0` with compact varint per-value lengths (header flags `0x02`).
    /// This is the layout every current ledger wire uses.
    case compact
    /// Minor `1`: varint lengths and counts, raw byte arrays, varint enum tags
    /// and no header alignment padding.
    ///
    /// TODO(dense-activation): no ledger definition selects dense yet
    /// (`norito.md`, `specs/live_runtime_upgrades.md`).
    case dense

    /// Header minor version that declares this layout.
    public var minor: UInt8 {
        switch self {
        case .fixed, .compact:
            return NoritoHeader.versionMinor
        case .dense:
            return NoritoHeader.versionMinorDense
        }
    }

    /// Header flag byte that declares this layout.
    public var headerFlags: UInt8 {
        switch self {
        case .fixed, .dense:
            return 0
        case .compact:
            return NoritoHeader.compactLen
        }
    }

    /// The layout a header's minor version and flag byte declare, or `nil` for an
    /// unknown minor version or a flag byte that minor version does not admit
    /// (reserved bits with minor `0`, any flag with minor `1`).
    public init?(headerMinor: UInt8, flags: UInt8) {
        switch (headerMinor, flags) {
        case (NoritoHeader.versionMinor, 0):
            self = .fixed
        case (NoritoHeader.versionMinor, NoritoHeader.compactLen):
            self = .compact
        case (NoritoHeader.versionMinorDense, 0):
            self = .dense
        default:
            return nil
        }
    }
}

public struct NoritoHeader {
    public static let magic = Data([0x4E, 0x52, 0x54, 0x30]) // "NRT0"
    public static let versionMajor: UInt8 = 0
    /// Minor version of the current layouts (fixed-width and `COMPACT_LEN`).
    public static let versionMinor: UInt8 = 0
    /// Minor version of the dense layout (`NoritoWireLayout.dense`).
    public static let versionMinorDense: UInt8 = 1
    public static let encodedLength = 4 + 1 + 1 + 16 + 1 + 8 + 8 + 1
    public static let maxHeaderPadding = 64
    public static let compactLen: UInt8 = 0x02
    /// Norito v1 minor `0` accepts only fixed-width (0x00) or compact
    /// (COMPACT_LEN, 0x02) per-value length prefixes; every other bit is
    /// rejected. A minor `1` (dense) header admits no flag at all.
    public static let supportedFlags: UInt8 = compactLen

    public let schema: [UInt8] // 16 bytes
    public let compression: NoritoCompression
    public let length: UInt64
    public let checksum: UInt64
    public let flags: UInt8
    /// Header minor version. Together with `flags` it declares the payload layout.
    public let minor: UInt8

    init(
        schema: [UInt8],
        compression: NoritoCompression,
        length: UInt64,
        checksum: UInt64,
        flags: UInt8,
        minor: UInt8 = NoritoHeader.versionMinor
    ) {
        self.schema = schema
        self.compression = compression
        self.length = length
        self.checksum = checksum
        self.flags = flags
        self.minor = minor
    }

    /// The payload layout this header declares, or `nil` when the minor version
    /// and flag byte name no supported layout.
    public var layout: NoritoWireLayout? {
        NoritoWireLayout(headerMinor: minor, flags: flags)
    }

    public func encode() -> Data {
        precondition(layout != nil, "Unsupported Norito layout flags")
        var out = Data()
        out.append(NoritoHeader.magic)
        out.append(contentsOf: [NoritoHeader.versionMajor, minor])
        out.append(contentsOf: schema)
        out.append(compression.rawValue)
        out.append(contentsOf: withUnsafeBytes(of: length.littleEndian, Array.init))
        out.append(contentsOf: withUnsafeBytes(of: checksum.littleEndian, Array.init))
        out.append(flags)
        return out
    }

    /// Whether `flags` is a valid minor-`0` flag byte.
    public static func isSupported(flags: UInt8) -> Bool {
        (flags & ~supportedFlags) == 0
    }
}

struct NoritoFrame {
    let header: NoritoHeader
    let payload: Data
    let paddingLength: Int
    /// The layout the header declares. Decode `payload` in this layout only.
    let layout: NoritoWireLayout
}

// Domain-separated SHA-256 truncated to 16 bytes.
public func noritoSchemaHash(forTypeName name: String) -> [UInt8] {
    var input = Data("norito:v1:type-name\u{0000}".utf8)
    input.append(contentsOf: name.utf8)
    return Array(SHA256.hash(data: input).prefix(16))
}

// CRC-64/XZ (reflected, init/xor = all-ones) to match Rust Norito's checksum.
private let CRC64_TABLE: [UInt64] = {
    let poly: UInt64 = 0xC96C5795D7870F42
    var table = [UInt64](repeating: 0, count: 256)
    for i in 0..<256 {
        var crc = UInt64(i)
        for _ in 0..<8 {
            if (crc & 1) != 0 {
                crc = (crc >> 1) ^ poly
            } else {
                crc >>= 1
            }
        }
        table[i] = crc
    }
    return table
}()

public func crc64ECMA(_ data: Data) -> UInt64 {
    var crc: UInt64 = 0xFFFF_FFFF_FFFF_FFFF
    for byte in data {
        let idx = Int((crc ^ UInt64(byte)) & 0xFF)
        crc = CRC64_TABLE[idx] ^ (crc >> 8)
    }
    return crc ^ 0xFFFF_FFFF_FFFF_FFFF
}

/// Build a Norito envelope for an already-serialized payload.
///
/// `payloadAlignment` must be the exact alignment of Rust's archived payload
/// type. Norito places zero padding between its fixed-size header and the
/// payload so the first payload byte has that alignment. Callers that exchange
/// an archive with Rust must pass the authoritative static alignment for the
/// schema instead of inferring it from the payload bytes.
///
/// `flags` selects one of the two minor-`0` layouts: `0x00` is
/// `NoritoWireLayout.fixed` and `COMPACT_LEN` is `NoritoWireLayout.compact`.
public func noritoEncode(
    typeName: String,
    payload: Data,
    flags: UInt8 = 0,
    payloadAlignment: Int = 1
) -> Data {
    guard let layout = NoritoWireLayout(headerMinor: NoritoHeader.versionMinor, flags: flags) else {
        preconditionFailure("Unsupported Norito layout flags")
    }
    return noritoEncode(
        typeName: typeName,
        payload: payload,
        layout: layout,
        payloadAlignment: payloadAlignment
    )
}

/// Build a Norito envelope for a payload that is already serialized in `layout`.
///
/// The header declares the layout: minor `0` with flags `0x00` or `0x02` for the
/// current layouts, minor `1` with flags `0x00` for the dense layout. A dense
/// frame has no alignment padding, so its payload starts at byte 40 for every
/// type and `payloadAlignment` does not affect it.
public func noritoEncode(
    typeName: String,
    payload: Data,
    layout: NoritoWireLayout,
    payloadAlignment: Int = 1
) -> Data {
    guard let alignmentPadding = noritoHeaderPaddingLength(payloadAlignment: payloadAlignment) else {
        preconditionFailure(
            "Norito payload alignment must be a power of two no greater than the header-padding limit"
        )
    }
    let paddingLength = layout == .dense ? 0 : alignmentPadding
    let schema = noritoSchemaHash(forTypeName: typeName)
    let checksum = crc64ECMA(payload)
    let header = NoritoHeader(schema: schema,
                              compression: .none,
                              length: UInt64(payload.count),
                              checksum: checksum,
                              flags: layout.headerFlags,
                              minor: layout.minor)
    var out = Data()
    out.append(header.encode())
    if paddingLength > 0 {
        out.append(Data(repeating: 0, count: paddingLength))
    }
    out.append(payload)
    return out
}

func noritoHeaderPaddingLength(payloadAlignment: Int) -> Int? {
    guard payloadAlignment > 0,
          payloadAlignment <= NoritoHeader.maxHeaderPadding,
          payloadAlignment.isPowerOfTwo else {
        return nil
    }
    return (payloadAlignment - (NoritoHeader.encodedLength % payloadAlignment))
        % payloadAlignment
}

/// Decode one frame of a current layout (header minor `0`).
///
/// A dense frame (minor `1`) is refused here: a decoder written for the current
/// layouts must never read a dense payload. Callers that implement the dense
/// layout use `noritoDecodeFrame(_:layout:)` or `noritoDecodeDeclaredFrame(_:)`.
func noritoDecodeFrame(_ data: Data) -> NoritoFrame? {
    guard let frame = noritoDecodeDeclaredFrame(data), frame.layout != .dense else {
        return nil
    }
    return frame
}

/// Decode one frame whose header declares exactly `layout`.
///
/// A frame of any other layout is refused; no alternate layout is attempted.
func noritoDecodeFrame(_ data: Data, layout: NoritoWireLayout) -> NoritoFrame? {
    guard let frame = noritoDecodeDeclaredFrame(data), frame.layout == layout else {
        return nil
    }
    return frame
}

/// Decode one frame of whichever supported layout its header declares.
///
/// The header's minor version and flag byte are the only layout selector:
/// unknown minor versions, reserved flag bits and any flag on a dense header
/// are refused, and a dense frame must carry its payload at byte 40 with no
/// padding. The caller must decode `payload` in the returned `layout`.
func noritoDecodeDeclaredFrame(_ data: Data) -> NoritoFrame? {
    let headerLength = NoritoHeader.encodedLength
    guard data.count >= headerLength else { return nil }
    guard data.prefix(4) == NoritoHeader.magic else { return nil }
    let major = data[4]
    let minor = data[5]
    guard major == NoritoHeader.versionMajor else {
        return nil
    }
    let schema = [UInt8](data[6..<22])
    guard let compression = NoritoCompression(rawValue: data[22]) else {
        return nil
    }
    guard let payloadLength = data.readUInt64LE(at: 23) else {
        return nil
    }
    guard let checksum = data.readUInt64LE(at: 31) else {
        return nil
    }
    let flags = data[39]
    guard let layout = NoritoWireLayout(headerMinor: minor, flags: flags) else {
        return nil
    }
    guard payloadLength <= UInt64(Int.max) else { return nil }
    let payloadLen = Int(payloadLength)
    let payloadStart = data.count - payloadLen
    guard payloadStart >= headerLength else { return nil }
    let paddingLength = payloadStart - headerLength
    guard paddingLength <= NoritoHeader.maxHeaderPadding else { return nil }
    if paddingLength > 0 {
        // The dense layout has no alignment padding: its payload starts at byte 40.
        guard layout != .dense else { return nil }
        let padding = data[headerLength..<payloadStart]
        if padding.contains(where: { $0 != 0 }) {
            return nil
        }
    }
    let payload = Data(data[payloadStart..<data.count])
    guard crc64ECMA(payload) == checksum else { return nil }
    let header = NoritoHeader(schema: schema,
                              compression: compression,
                              length: payloadLength,
                              checksum: checksum,
                              flags: flags,
                              minor: minor)
    return NoritoFrame(
        header: header,
        payload: payload,
        paddingLength: paddingLength,
        layout: layout
    )
}

private extension Data {
    func readUInt64LE(at offset: Int) -> UInt64? {
        guard offset >= 0, offset + 8 <= count else {
            return nil
        }
        var value: UInt64 = 0
        self[offset..<(offset + 8)].withUnsafeBytes { buffer in
            guard let base = buffer.baseAddress else { return }
            memcpy(&value, base, 8)
        }
        return UInt64(littleEndian: value)
    }
}

private extension Int {
    var isPowerOfTwo: Bool {
        self & (self - 1) == 0
    }
}
