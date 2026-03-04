import Foundation

public enum NoritoCompression: UInt8 { case none = 0 }

public struct NoritoHeader {
    public static let magic = Data([0x4E, 0x52, 0x54, 0x30]) // "NRT0"
    public static let versionMajor: UInt8 = 0
    public static let versionMinor: UInt8 = 0
    public static let encodedLength = 4 + 1 + 1 + 16 + 1 + 8 + 8 + 1

    public let schema: [UInt8] // 16 bytes
    public let compression: NoritoCompression
    public let length: UInt64
    public let checksum: UInt64
    public let flags: UInt8

    public func encode() -> Data {
        var out = Data()
        out.append(NoritoHeader.magic)
        out.append(contentsOf: [NoritoHeader.versionMajor, NoritoHeader.versionMinor])
        out.append(contentsOf: schema)
        out.append(compression.rawValue)
        out.append(contentsOf: withUnsafeBytes(of: length.littleEndian, Array.init))
        out.append(contentsOf: withUnsafeBytes(of: checksum.littleEndian, Array.init))
        out.append(flags)
        return out
    }
}

struct NoritoFrame {
    let header: NoritoHeader
    let payload: Data
    let paddingLength: Int
}

// FNV-1a 64-bit (little-endian duplication to 16 bytes)
public func noritoSchemaHash(forTypeName name: String) -> [UInt8] {
    let FNV_OFFSET: UInt64 = 0xcbf29ce484222325
    let FNV_PRIME: UInt64 = 0x100000001b3
    var hash = FNV_OFFSET
    for b in name.utf8 { hash ^= UInt64(b); hash = hash &* FNV_PRIME }
    let part = withUnsafeBytes(of: hash.littleEndian, Array.init)
    return part + part
}

// CRC64 (reflected, init/xor = all-ones) to match Rust crc64fast output.
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
public func noritoEncode(typeName: String, payload: Data, flags: UInt8 = 0) -> Data {
    let schema = noritoSchemaHash(forTypeName: typeName)
    let checksum = crc64ECMA(payload)
    let header = NoritoHeader(schema: schema,
                              compression: .none,
                              length: UInt64(payload.count),
                              checksum: checksum,
                              flags: flags)
    var out = Data()
    out.append(header.encode())
    out.append(payload)
    return out
}

func noritoDecodeFrame(_ data: Data) -> NoritoFrame? {
    let headerLength = NoritoHeader.encodedLength
    guard data.count >= headerLength else { return nil }
    guard data.prefix(4) == NoritoHeader.magic else { return nil }
    let major = data[4]
    let minor = data[5]
    guard major == NoritoHeader.versionMajor, minor == NoritoHeader.versionMinor else {
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
    guard payloadLength <= UInt64(Int.max) else { return nil }
    let payloadLen = Int(payloadLength)
    let payloadStart = data.count - payloadLen
    guard payloadStart >= headerLength else { return nil }
    let paddingLength = payloadStart - headerLength
    if paddingLength > 0 {
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
                              flags: flags)
    return NoritoFrame(header: header, payload: payload, paddingLength: paddingLength)
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
