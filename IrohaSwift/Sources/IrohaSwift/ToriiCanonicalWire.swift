import Foundation

/// Canonical scalar and public-key admission used by active Torii clients.
enum ToriiCanonicalWire {
    private struct BlsNormalPeerId {
        let literal: String
        let compressedKey: Data

        var orderingKey: Data {
            var bytes = Data([SigningAlgorithm.blsNormal.noritoDiscriminant])
            bytes.append(compressedKey)
            return bytes
        }
    }

    private static let blsKeyAdmissionMessage =
        Data("iroha:bls-normal-key-admission:v1".utf8)
    /// A valid compressed BLS-Normal signature used only to make the native
    /// bridge parse candidate public keys. Verification is intentionally
    /// performed with a key unrelated to this deterministic signing key;
    /// either Boolean result proves that both curve points passed the same
    /// parser used by Rust production.
    private static let blsKeyAdmissionSignature = Data(
        hexString:
            "93E02B6052719F607DACD3A088274F65596BD0D09920B61AB5DA61BBDC7F5049"
            + "334CF11213945D57E5AC7D055D042B7E024AA2B2F08F0A91260805272DC51051"
            + "C6E47AD4FA403B02B4510B647AE3D1770BAC0326A805BBEFD48056C8C121BDB8"
    )!
    private static let blsNormalPeerCache: NSCache<NSString, NSData> = {
        let cache = NSCache<NSString, NSData>()
        cache.countLimit = 512
        return cache
    }()

    static func isAsciiHex(_ character: Character, uppercaseOnly: Bool = false) -> Bool {
        if character >= "0" && character <= "9" {
            return true
        }
        if character >= "A" && character <= "F" {
            return true
        }
        return !uppercaseOnly && character >= "a" && character <= "f"
    }

    static func exactHex<K: CodingKey>(
        _ value: String,
        bytes: Int,
        key: K,
        container: KeyedDecodingContainer<K>,
        field: String,
        uppercaseOnly: Bool = false
    ) throws -> String {
        guard value.count == bytes * 2,
              value.allSatisfy({ isAsciiHex($0, uppercaseOnly: uppercaseOnly) }) else {
            throw DecodingError.dataCorruptedError(
                forKey: key,
                in: container,
                debugDescription: "\(field) must be exactly \(bytes) bytes of hex."
            )
        }
        return value
    }

    static func crc16(_ bytes: [UInt8]) -> UInt16 {
        var crc = UInt16.max
        for byte in bytes {
            crc ^= UInt16(byte) << 8
            for _ in 0..<8 {
                crc = (crc & 0x8000) != 0 ? (crc &<< 1) ^ 0x1021 : crc &<< 1
            }
        }
        return crc
    }

    static func isCanonicalHash(_ value: String) -> Bool {
        guard value.hasPrefix("hash:") else { return false }
        let components = value.dropFirst(5).split(
            separator: "#",
            maxSplits: 1,
            omittingEmptySubsequences: false
        )
        guard components.count == 2 else { return false }
        let body = String(components[0])
        let checksum = String(components[1])
        guard body.count == 64,
              body.allSatisfy({ isAsciiHex($0, uppercaseOnly: true) }),
              checksum.count == 4,
              checksum.allSatisfy({ isAsciiHex($0, uppercaseOnly: true) }),
              let bodyBytes = Data(hexString: body),
              bodyBytes.count == 32,
              let marker = bodyBytes.last,
              marker & 1 == 1,
              let parsedChecksum = UInt16(checksum, radix: 16) else {
            return false
        }
        return parsedChecksum == crc16(Array("hash:\(body)".utf8))
    }

    static func canonicalHash<K: CodingKey>(
        _ value: String,
        key: K,
        container: KeyedDecodingContainer<K>,
        field: String
    ) throws -> String {
        guard isCanonicalHash(value) else {
            throw DecodingError.dataCorruptedError(
                forKey: key,
                in: container,
                debugDescription: "\(field) has malformed hex, marker bit, or CRC16 checksum."
            )
        }
        return value
    }

    static func isCanonicalUnsignedDecimal(_ value: String, allowFraction: Bool) -> Bool {
        let parts = value.split(separator: ".", maxSplits: 1, omittingEmptySubsequences: false)
        guard !parts.isEmpty, parts.count <= (allowFraction ? 2 : 1) else { return false }
        let integer = parts[0]
        guard !integer.isEmpty,
              integer.allSatisfy({ $0 >= "0" && $0 <= "9" }),
              integer == "0" || integer.first != "0" else { return false }
        if parts.count == 2 {
            let fraction = parts[1]
            return !fraction.isEmpty && fraction.allSatisfy { $0 >= "0" && $0 <= "9" }
        }
        return true
    }

    static func unsignedDecimal<K: CodingKey>(
        _ value: String,
        key: K,
        container: KeyedDecodingContainer<K>,
        field: String,
        allowFraction: Bool = true
    ) throws -> String {
        guard isCanonicalUnsignedDecimal(value, allowFraction: allowFraction) else {
            throw DecodingError.dataCorruptedError(
                forKey: key,
                in: container,
                debugDescription: "\(field) must be a canonical unsigned decimal string."
            )
        }
        return value
    }

    static func quantity<K: CodingKey>(
        _ value: String,
        key: K,
        container: KeyedDecodingContainer<K>,
        field: String
    ) throws -> String {
        do {
            return try KotodamaNumericV1Codec.decodeQuantityJSON(value).canonicalString
        } catch {
            throw DecodingError.dataCorruptedError(
                forKey: key,
                in: container,
                debugDescription: "\(field) must be a canonical non-negative Kotodama V1 Quantity string."
            )
        }
    }

    static func u128<K: CodingKey>(
        _ value: String,
        key: K,
        container: KeyedDecodingContainer<K>,
        field: String
    ) throws -> String {
        let canonical = try unsignedDecimal(
            value,
            key: key,
            container: container,
            field: field,
            allowFraction: false
        )
        guard SccpUInt128.parse(canonical) != nil else {
            throw DecodingError.dataCorruptedError(
                forKey: key,
                in: container,
                debugDescription: "\(field) exceeds u128::MAX."
            )
        }
        return canonical
    }

    static func nonEmpty<K: CodingKey>(
        _ value: String,
        key: K,
        container: KeyedDecodingContainer<K>,
        field: String
    ) throws -> String {
        guard !value.isEmpty, value.trimmingCharacters(in: .whitespacesAndNewlines) == value else {
            throw DecodingError.dataCorruptedError(
                forKey: key,
                in: container,
                debugDescription: "\(field) must be a non-empty exact string."
            )
        }
        return value
    }

    private static func parseBlsNormalPeerId(_ value: String) -> BlsNormalPeerId? {
        let bare: String
        if value.hasPrefix("bls_normal:") {
            bare = String(value.dropFirst("bls_normal:".count))
        } else {
            bare = value
        }
        guard value.trimmingCharacters(in: .whitespacesAndNewlines) == value,
              bare.count == 102,
              bare.hasPrefix("ea0130")
        else {
            return nil
        }
        if let cached = blsNormalPeerCache.object(forKey: bare as NSString) {
            return BlsNormalPeerId(literal: bare, compressedKey: cached as Data)
        }
        let payloadStart = bare.index(bare.startIndex, offsetBy: 6)
        let payloadHex = String(bare[payloadStart...])
        guard payloadHex.count == 96,
              payloadHex.allSatisfy({ isAsciiHex($0, uppercaseOnly: true) }),
              let compressedKey = Data(hexString: payloadHex),
              compressedKey.count == 48,
              NoritoNativeBridge.shared.verifyDetached(
                  algorithm: .blsNormal,
                  publicKey: compressedKey,
                  message: blsKeyAdmissionMessage,
                  signature: blsKeyAdmissionSignature
              ) != nil
        else {
            return nil
        }
        blsNormalPeerCache.setObject(compressedKey as NSData, forKey: bare as NSString)
        return BlsNormalPeerId(literal: bare, compressedKey: compressedKey)
    }

    static func isCanonicalBlsNormalPeerId(_ value: String) -> Bool {
        parseBlsNormalPeerId(value) != nil
    }

}
