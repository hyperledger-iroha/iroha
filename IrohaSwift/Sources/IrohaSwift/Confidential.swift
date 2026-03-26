import Foundation

public enum ConfidentialKeyDerivationError: Error, Sendable {
    case missingSeed
    case invalidSpendKeyLength
    case invalidKeyLength(label: String, expected: Int, actual: Int)
    case invalidHexEncoding(field: String)
    case invalidBase64Encoding(field: String)
}

extension ConfidentialKeyDerivationError: LocalizedError {
    public var errorDescription: String? {
        switch self {
        case .missingSeed:
            return "Provide either seedHex or seedBase64."
        case .invalidSpendKeyLength:
            return "Invalid spend key length; expected 32 bytes."
        case let .invalidKeyLength(label, expected, actual):
            return "Invalid \(label) length; expected \(expected) bytes but got \(actual)."
        case let .invalidHexEncoding(field):
            return "Invalid hex encoding for \(field)."
        case let .invalidBase64Encoding(field):
            return "Invalid base64 encoding for \(field)."
        }
    }
}

public struct ConfidentialKeyset: Equatable, Sendable {
    public let spendKey: Data
    public let nullifierKey: Data
    public let incomingViewKey: Data
    public let outgoingViewKey: Data
    public let fullViewKey: Data

    public var spendKeyHex: String { spendKey.map { String(format: "%02x", $0) }.joined() }
    public var nullifierKeyHex: String { nullifierKey.map { String(format: "%02x", $0) }.joined() }
    public var incomingViewKeyHex: String { incomingViewKey.map { String(format: "%02x", $0) }.joined() }
    public var outgoingViewKeyHex: String { outgoingViewKey.map { String(format: "%02x", $0) }.joined() }
    public var fullViewKeyHex: String { fullViewKey.map { String(format: "%02x", $0) }.joined() }

    public func asHexDictionary() -> [String: String] {
        [
            "sk_spend": spendKeyHex,
            "nk": nullifierKeyHex,
            "ivk": incomingViewKeyHex,
            "ovk": outgoingViewKeyHex,
            "fvk": fullViewKeyHex,
        ]
    }

    public init(spendKey: Data,
                nullifierKey: Data,
                incomingViewKey: Data,
                outgoingViewKey: Data,
                fullViewKey: Data) throws {
        func validate(_ data: Data, label: String) throws -> Data {
            if data.count != 32 {
                throw ConfidentialKeyDerivationError.invalidKeyLength(label: label,
                                                                       expected: 32,
                                                                       actual: data.count)
            }
            return data
        }

        self.spendKey = try validate(spendKey, label: "spendKey")
        self.nullifierKey = try validate(nullifierKey, label: "nullifierKey")
        self.incomingViewKey = try validate(incomingViewKey, label: "incomingViewKey")
        self.outgoingViewKey = try validate(outgoingViewKey, label: "outgoingViewKey")
        self.fullViewKey = try validate(fullViewKey, label: "fullViewKey")
    }

    public static func derive(from spendKey: Data) throws -> ConfidentialKeyset {
        guard spendKey.count == 32 else {
            throw ConfidentialKeyDerivationError.invalidSpendKeyLength
        }

        let prk = hmacSHA3(key: ConfidentialConstants.salt, data: spendKey)
        let nk = hkdfExpand(prk: prk, info: ConfidentialConstants.infoNk)
        let ivk = hkdfExpand(prk: prk, info: ConfidentialConstants.infoIvk)
        let ovk = hkdfExpand(prk: prk, info: ConfidentialConstants.infoOvk)
        let fvk = hkdfExpand(prk: prk, info: ConfidentialConstants.infoFvk)

        return try ConfidentialKeyset(
            spendKey: spendKey,
            nullifierKey: nk,
            incomingViewKey: ivk,
            outgoingViewKey: ovk,
            fullViewKey: fvk
        )
    }

    public static func derive(seedHex: String? = nil, seedBase64: String? = nil) throws -> ConfidentialKeyset {
        if let seedHex, !seedHex.isEmpty {
            guard let spendKey = Data(hexString: seedHex) else {
                throw ConfidentialKeyDerivationError.invalidHexEncoding(field: "seed_hex")
            }
            return try derive(from: spendKey)
        }
        if let seedBase64, !seedBase64.isEmpty {
            guard let spendKey = Data(base64Encoded: seedBase64) else {
                throw ConfidentialKeyDerivationError.invalidBase64Encoding(field: "seed_b64")
            }
            return try derive(from: spendKey)
        }
        throw ConfidentialKeyDerivationError.missingSeed
    }
}

private enum ConfidentialConstants {
    static let salt = Data("iroha:confidential:key-derivation:v1".utf8)
    static let infoNk = Data("iroha:confidential:nk".utf8)
    static let infoIvk = Data("iroha:confidential:ivk".utf8)
    static let infoOvk = Data("iroha:confidential:ovk".utf8)
    static let infoFvk = Data("iroha:confidential:fvk".utf8)
    static let blockSize = 72 // rate for SHA3-512 in bytes
}

private func hkdfExpand(prk: Data, info: Data) -> Data {
    var buffer = Data()
    buffer.reserveCapacity(ConfidentialConstants.blockSize + info.count + 1)
    buffer.append(info)
    buffer.append(0x01)
    return hmacSHA3(key: prk, data: buffer).prefix(32)
}

private func hmacSHA3(key: Data, data: Data) -> Data {
    var keyMaterial = key
    if keyMaterial.count > ConfidentialConstants.blockSize {
        keyMaterial = sha3_512(keyMaterial)
    }
    if keyMaterial.count < ConfidentialConstants.blockSize {
        keyMaterial.append(contentsOf: repeatElement(0, count: ConfidentialConstants.blockSize - keyMaterial.count))
    }

    var ipad = Data(count: ConfidentialConstants.blockSize)
    var opad = Data(count: ConfidentialConstants.blockSize)
    for i in 0..<ConfidentialConstants.blockSize {
        let byte = keyMaterial[i]
        ipad[i] = byte ^ 0x36
        opad[i] = byte ^ 0x5c
    }

    var inner = Data()
    inner.reserveCapacity(ipad.count + data.count)
    inner.append(ipad)
    inner.append(data)
    let innerDigest = sha3_512(inner)

    var outer = Data()
    outer.reserveCapacity(opad.count + innerDigest.count)
    outer.append(opad)
    outer.append(innerDigest)
    return sha3_512(outer)
}

// MARK: - SHA3-512 implementation

private func sha3_512(_ data: Data) -> Data {
    return keccak(rate: 72, outputLength: 64, data: data, suffix: 0x06)
}

private func keccak(rate: Int, outputLength: Int, data: Data, suffix: UInt8) -> Data {
    var state = [UInt64](repeating: 0, count: 25)
    let rateInBytes = rate
    var offset = 0
    let bytes = [UInt8](data)

    while offset + rateInBytes <= bytes.count {
        absorb(block: Array(bytes[offset..<(offset + rateInBytes)]), into: &state)
        keccakF(&state)
        offset += rateInBytes
    }

    var block = [UInt8](repeating: 0, count: rateInBytes)
    let remaining = bytes.count - offset
    if remaining > 0 {
        for i in 0..<remaining {
            block[i] = bytes[offset + i]
        }
    }
    block[remaining] ^= suffix
    block[rateInBytes - 1] ^= 0x80
    absorb(block: block, into: &state)
    keccakF(&state)

    var output = [UInt8]()
    output.reserveCapacity(outputLength)
    while output.count < outputLength {
        for word in state.prefix(rateInBytes / 8) {
            var little = word.littleEndian
            withUnsafeBytes(of: &little) { ptr in
                for byte in ptr {
                    if output.count < outputLength {
                        output.append(byte)
                    }
                }
            }
            if output.count >= outputLength {
                break
            }
        }
        if output.count >= outputLength {
            break
        }
        keccakF(&state)
    }
    return Data(output)
}

private func absorb(block: [UInt8], into state: inout [UInt64]) {
    for lane in 0..<(block.count / 8) {
        var value: UInt64 = 0
        let offset = lane * 8
        for j in 0..<8 {
            value |= UInt64(block[offset + j]) << (UInt64(j) * 8)
        }
        state[lane] ^= value
    }
}

private func keccakF(_ state: inout [UInt64]) {
    for round in 0..<24 {
        theta(&state)
        rhoPi(&state)
        chi(&state)
        state[0] ^= KeccakConstants.roundConstants[round]
    }
}

private func theta(_ state: inout [UInt64]) {
    var c = [UInt64](repeating: 0, count: 5)
    for x in 0..<5 {
        c[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20]
    }
    for x in 0..<5 {
        let d = rotateLeft(c[(x + 1) % 5], by: 1) ^ c[(x + 4) % 5]
        for y in stride(from: x, to: state.count, by: 5) {
            state[y] ^= d
        }
    }
}

private func rhoPi(_ state: inout [UInt64]) {
    var current = state[1]
    for t in 0..<24 {
        let targetIndex = KeccakConstants.piLane[t]
        let shift = KeccakConstants.rotationConstants[t]
        let temp = state[targetIndex]
        state[targetIndex] = rotateLeft(current, by: shift)
        current = temp
    }
}

private func chi(_ state: inout [UInt64]) {
    for y in stride(from: 0, to: 25, by: 5) {
        var row = [UInt64](repeating: 0, count: 5)
        for x in 0..<5 { row[x] = state[y + x] }
        for x in 0..<5 {
            state[y + x] ^= (~row[(x + 1) % 5]) & row[(x + 2) % 5]
        }
    }
}

private func rotateLeft(_ value: UInt64, by: Int) -> UInt64 {
    let shift = UInt64(by)
    return (value << shift) | (value >> (64 - shift))
}

private enum KeccakConstants {
    static let roundConstants: [UInt64] = [
        0x0000000000000001, 0x0000000000008082, 0x800000000000808A, 0x8000000080008000,
        0x000000000000808B, 0x0000000080000001, 0x8000000080008081, 0x8000000000008009,
        0x000000000000008A, 0x0000000000000088, 0x0000000080008009, 0x000000008000000A,
        0x000000008000808B, 0x800000000000008B, 0x8000000000008089, 0x8000000000008003,
        0x8000000000008002, 0x8000000000000080, 0x000000000000800A, 0x800000008000000A,
        0x8000000080008081, 0x8000000000008080, 0x0000000080000001, 0x8000000080008008,
    ]

    static let rotationConstants: [Int] = [
        1, 3, 6, 10, 15, 21, 28, 36, 45, 55, 2, 14, 27, 41, 56, 8, 25, 43, 62, 18, 39, 61, 20, 44,
    ]

    static let piLane: [Int] = [
        10, 7, 11, 17, 18, 3, 5, 16, 8, 21, 24, 4, 15, 23, 19, 13, 12, 2, 20, 14, 22, 9, 6, 1,
    ]
}
