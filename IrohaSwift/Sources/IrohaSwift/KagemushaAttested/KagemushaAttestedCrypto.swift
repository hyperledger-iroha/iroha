import CryptoKit
import Foundation

/// Domain-separation purposes of the attested-app suite.
///
/// Every tag is `iroha:kagemusha:v1:attested-app:<purpose>\0`.
enum KagemushaAttestedDomain: String, CaseIterable, Sendable {
    case schemeId = "scheme-id"
    case descriptor = "descriptor"
    case deviceId = "device-id"
    case cert = "cert"
    case transition = "transition"
    case request = "request"
    case ack = "ack"
    case voucher = "voucher"
    case crl = "crl"
    case crlDelta = "crl-delta"
    case delivery = "delivery"
    case enroll = "enroll"
    case sync = "sync"
    case accountProof = "account-proof"
    case loadBinding = "load-binding"
    case redemptionId = "redemption-id"

    /// Exact domain bytes, including the terminating NUL.
    var bytes: Data {
        Data("iroha:kagemusha:v1:attested-app:\(rawValue)\0".utf8)
    }
}

/// Primitive cryptography of the suite: SHA-256 and low-S ECDSA P-256 over 64-byte `r || s`.
enum KagemushaAttestedCrypto {
    static let publicKeyBytes = 65
    static let signatureBytes = 64
    static let digestBytes = 32

    /// P-256 group order `n`, big-endian.
    static let groupOrder: [UInt8] = attestedHexLiteral(
        "ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551")
    /// `floor(n / 2)`, big-endian. A signature is low-S when `1 <= s <= halfOrder`.
    static let halfOrder: [UInt8] = attestedHexLiteral(
        "7fffffff800000007fffffffffffffffde737d56d38bcf4279dce5617e3192a8")

    /// `SHA-256(parts...)`.
    static func sha256(_ parts: Data...) -> Data {
        sha256(parts)
    }

    static func sha256(_ parts: [Data]) -> Data {
        var hasher = SHA256()
        for part in parts { hasher.update(data: part) }
        return Data(hasher.finalize())
    }

    /// `H(D_purpose || parts...)`.
    static func domainHash(_ domain: KagemushaAttestedDomain, _ parts: Data...) -> Data {
        sha256([domain.bytes] + parts)
    }

    /// Whether `publicKey` is an uncompressed SEC1 point on P-256.
    static func isValidPublicKey(_ publicKey: Data) -> Bool {
        publicKey.count == publicKeyBytes && publicKey.first == 0x04
            && (try? P256.Signing.PublicKey(x963Representation: publicKey)) != nil
    }

    /// Normalize a raw `r || s` signature to low-S (`s -> n - s` when `s > n / 2`).
    static func normalizeLowS(_ signature: Data) -> Data {
        precondition(signature.count == signatureBytes)
        let bytes = [UInt8](signature)
        let s = Array(bytes[32..<64])
        guard compare(s, halfOrder) > 0 else { return signature }
        return Data(bytes[0..<32] + subtract(groupOrder, s))
    }

    /// Whether `signature` is `r || s` with `1 <= r < n` and `1 <= s <= n / 2`.
    static func isCanonicalLowS(_ signature: Data) -> Bool {
        guard signature.count == signatureBytes else { return false }
        let bytes = [UInt8](signature)
        let r = Array(bytes[0..<32])
        let s = Array(bytes[32..<64])
        return r.contains(where: { $0 != 0 }) && compare(r, groupOrder) < 0
            && s.contains(where: { $0 != 0 }) && compare(s, halfOrder) <= 0
    }

    /// Verify a low-S ECDSA P-256/SHA-256 signature over `message`. High-S is rejected.
    static func verify(signature: Data, message: Data, publicKey: Data) -> Bool {
        guard isCanonicalLowS(signature), publicKey.count == publicKeyBytes,
              publicKey.first == 0x04,
              let key = try? P256.Signing.PublicKey(x963Representation: publicKey),
              let parsed = try? P256.Signing.ECDSASignature(rawRepresentation: signature)
        else { return false }
        return key.isValidSignature(parsed, for: message)
    }

    /// Sign `message` with a software key and return low-S `r || s`.
    static func softwareSign(_ message: Data, key: P256.Signing.PrivateKey) throws -> Data {
        normalizeLowS(try key.signature(for: message).rawRepresentation)
    }

    /// Cryptographically random bytes.
    static func randomBytes(_ count: Int) -> Data {
        var generator = SystemRandomNumberGenerator()
        return Data((0..<count).map { _ in UInt8.random(in: .min ... .max, using: &generator) })
    }

    private static func compare(_ lhs: [UInt8], _ rhs: [UInt8]) -> Int {
        for (left, right) in zip(lhs, rhs) where left != right {
            return left < right ? -1 : 1
        }
        return 0
    }

    /// Big-endian `lhs - rhs` for `lhs >= rhs` of equal width.
    private static func subtract(_ lhs: [UInt8], _ rhs: [UInt8]) -> [UInt8] {
        var result = [UInt8](repeating: 0, count: lhs.count)
        var borrow = 0
        for index in stride(from: lhs.count - 1, through: 0, by: -1) {
            var value = Int(lhs[index]) - Int(rhs[index]) - borrow
            borrow = value < 0 ? 1 : 0
            if value < 0 { value += 256 }
            result[index] = UInt8(value)
        }
        return result
    }
}

private func attestedHexLiteral(_ hex: String) -> [UInt8] {
    var bytes: [UInt8] = []
    bytes.reserveCapacity(hex.utf8.count / 2)
    var high: UInt8?
    for scalar in hex.utf8 {
        let nibble: UInt8
        switch scalar {
        case 0x30...0x39: nibble = scalar - 0x30
        case 0x61...0x66: nibble = scalar - 0x61 + 10
        case 0x41...0x46: nibble = scalar - 0x41 + 10
        default: preconditionFailure("invalid hex literal")
        }
        if let value = high {
            bytes.append(value << 4 | nibble)
            high = nil
        } else {
            high = nibble
        }
    }
    precondition(high == nil, "odd hex literal")
    return bytes
}

extension Data {
    /// Lowercase hexadecimal.
    var kagemushaHex: String {
        map { String(format: "%02x", $0) }.joined()
    }

    /// Decode lowercase or uppercase hexadecimal; `nil` when malformed.
    init?(kagemushaHex hex: String) {
        guard hex.utf8.count % 2 == 0 else { return nil }
        var bytes = [UInt8]()
        bytes.reserveCapacity(hex.utf8.count / 2)
        var high: UInt8?
        for scalar in hex.utf8 {
            let nibble: UInt8
            switch scalar {
            case 0x30...0x39: nibble = scalar - 0x30
            case 0x61...0x66: nibble = scalar - 0x61 + 10
            case 0x41...0x46: nibble = scalar - 0x41 + 10
            default: return nil
            }
            if let value = high {
                bytes.append(value << 4 | nibble)
                high = nil
            } else {
                high = nibble
            }
        }
        self.init(bytes)
    }

    /// Unpadded base64url.
    var kagemushaBase64URL: String {
        base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    /// Strict unpadded base64url; rejects padding, foreign bytes and non-canonical tails.
    init?(kagemushaBase64URL text: String) {
        guard text.utf8.allSatisfy({ byte in
            (byte >= 0x41 && byte <= 0x5a) || (byte >= 0x61 && byte <= 0x7a)
                || (byte >= 0x30 && byte <= 0x39) || byte == 0x2d || byte == 0x5f
        }), text.utf8.count % 4 != 1 else { return nil }
        var padded = text.replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        padded.append(String(repeating: "=", count: (4 - padded.utf8.count % 4) % 4))
        guard let decoded = Data(base64Encoded: padded), decoded.kagemushaBase64URL == text else {
            return nil
        }
        self = decoded
    }

    /// Little-endian fixed-width encoding of `value`.
    static func kagemushaLE<T: FixedWidthInteger>(_ value: T) -> Data {
        var little = value.littleEndian
        return Swift.withUnsafeBytes(of: &little) { Data($0) }
    }
}
