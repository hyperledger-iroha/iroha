import Foundation

// Generic multihash public-key validation shared by Sumeragi status, validator staking and
// governance decoders.

/// Canonical ordering key of one public-key multihash: the Norito algorithm discriminant, then
/// the raw key payload bytes.
struct GovernancePublicKeyOrderV1: Comparable {
    let algorithm: UInt8
    let payload: [UInt8]

    static func < (lhs: Self, rhs: Self) -> Bool {
        if lhs.algorithm != rhs.algorithm {
            return lhs.algorithm < rhs.algorithm
        }
        return lhs.payload.lexicographicallyPrecedes(rhs.payload)
    }
}

/// Canonical unsigned LEB128 varint at `start`, with its end offset; `nil` for a truncated,
/// overlong or non-minimal encoding.
private func governancePublicKeyVarintV1(
    _ bytes: [UInt8],
    from start: Int
) -> (UInt64, Int)? {
    var value: UInt64 = 0
    var shift = 0
    var index = start
    while index < bytes.count, shift <= 63 {
        let byte = bytes[index]
        index += 1
        let chunk = UInt64(byte & 0x7f)
        guard shift != 63 || chunk <= 1 else { return nil }
        value |= chunk << shift
        if byte & 0x80 == 0 {
            guard index - start == 1 || chunk != 0 else { return nil }
            return (value, index)
        }
        shift += 7
    }
    return nil
}

/// Whether `payload` has the exact length and envelope of an `algorithm` public key.
private func governancePublicKeyShapeV1(
    algorithm: SigningAlgorithm,
    payload: Data
) -> Bool {
    switch algorithm {
    case .ed25519:
        return payload.count == 32
    case .secp256k1:
        return payload.count == 33 && (payload.first == 0x02 || payload.first == 0x03)
    case .blsNormal:
        return payload.count == 48
    case .blsSmall:
        return payload.count == 96
    case .mlDsa:
        return payload.count == 1_952 && payload.contains(where: { $0 != 0 })
    case .gost2012_256A, .gost2012_256B, .gost2012_256C:
        return payload.count == 64 && payload.contains(where: { $0 != 0 })
    case .gost2012_512A, .gost2012_512B:
        return payload.count == 128 && payload.contains(where: { $0 != 0 })
    case .sm2:
        guard payload.count >= 67 else { return false }
        let distidLength = (Int(payload[payload.startIndex]) << 8)
            | Int(payload[payload.index(after: payload.startIndex)])
        guard distidLength <= Int(UInt16.max) / 8,
              payload.count == 2 + distidLength + 65 else { return false }
        let distidStart = payload.index(payload.startIndex, offsetBy: 2)
        let distidEnd = payload.index(distidStart, offsetBy: distidLength)
        guard String(data: payload[distidStart..<distidEnd], encoding: .utf8) != nil else {
            return false
        }
        let sec1 = payload[distidEnd...]
        return sec1.first == 0x04 && sec1.dropFirst().contains(where: { $0 != 0 })
    }
}

/// Parse an exact canonical public-key multihash literal (Sumeragi status keys, validator
/// staking keys and governance signer lists) into its ordering key.
///
/// The literal must be hex of `varint code || varint length || payload` with a supported
/// algorithm code, the exact payload shape of that algorithm, a valid prime-order Ed25519 key
/// for Ed25519, and the canonical spelling `CanonicalNorito.publicKeyMultihash` produces.
///
/// - Throws: `DecodingError.dataCorrupted` at `codingPath` for any other literal.
func governancePublicKeyOrderV1(
    _ literal: String,
    codingPath: [CodingKey]
) throws -> GovernancePublicKeyOrderV1 {
    func invalid() -> DecodingError {
        DecodingError.dataCorrupted(.init(
            codingPath: codingPath,
            debugDescription: "expected an exact canonical public-key multihash"
        ))
    }
    guard literal.utf8.count <= 1_048_576,
          let encoded = Data(hexString: literal) else { throw invalid() }
    let bytes = [UInt8](encoded)
    guard let (code, codeEnd) = governancePublicKeyVarintV1(bytes, from: 0),
          let (length, payloadStart) = governancePublicKeyVarintV1(bytes, from: codeEnd),
          length > 0,
          length == UInt64(bytes.count - payloadStart) else { throw invalid() }
    let algorithm: SigningAlgorithm
    switch code {
    case 0xed: algorithm = .ed25519
    case 0xe7: algorithm = .secp256k1
    case 0xea: algorithm = .blsNormal
    case 0xeb: algorithm = .blsSmall
    case 0xee: algorithm = .mlDsa
    case 0x1200: algorithm = .gost2012_256A
    case 0x1201: algorithm = .gost2012_256B
    case 0x1202: algorithm = .gost2012_256C
    case 0x1203: algorithm = .gost2012_512A
    case 0x1204: algorithm = .gost2012_512B
    case 0x1306: algorithm = .sm2
    default: throw invalid()
    }
    let payload = Data(bytes[payloadStart...])
    guard CanonicalNorito.publicKeyMultihash(algorithm: algorithm, payload: payload) == literal,
          governancePublicKeyShapeV1(algorithm: algorithm, payload: payload),
          algorithm != .ed25519 || Ed25519PublicKeyAdmission.isValidPublicKey(payload) else {
        throw invalid()
    }
    return GovernancePublicKeyOrderV1(
        algorithm: algorithm.noritoDiscriminant,
        payload: [UInt8](payload)
    )
}
