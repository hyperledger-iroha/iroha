import Foundation
import CryptoKit

public enum ToriiCanonicalRequestError: Error, Equatable {
    case missingAccountId
    case missingNonce
    case invalidAccountId
    case invalidNonce
    case queryTooLarge
    case tooManyQueryPairs
    case accountTooLarge
    case methodTooLarge
    case invalidMethod
    case pathTooLarge
    case invalidPath
}

/// Helpers for building canonical request signatures accepted by Torii app endpoints.
public enum ToriiCanonicalRequest {
    public static let maxQueryPairsV1 = canonicalRequestMaxQueryPairsV1
    public static let maxRawQueryBytesV1 = canonicalRequestMaxRawQueryBytesV1
    public static let maxAccountLiteralBytesV1 = 36 * 1024
    public static let maxAliasLiteralBytesV1 = canonicalRequestMaxAliasLiteralBytesV1
    public static let maxMethodBytesV1 = 32
    public static let maxPathBytesV1 = 64 * 1024

    public static let headerAccount = "X-Iroha-Account"
    public static let headerSignature = "X-Iroha-Signature"
    public static let headerTimestampMs = "X-Iroha-Timestamp-Ms"
    public static let headerNonce = "X-Iroha-Nonce"

    /// Canonicalise a bounded raw query string by decoding, sorting, and re-encoding.
    public static func canonicalQueryString(from raw: String?) throws -> String {
        try validateCanonicalQuery(raw)
        guard let raw, !raw.isEmpty else { return "" }
        var pairs: [(String, String)] = []
        for component in raw.split(separator: "&", omittingEmptySubsequences: false) {
            guard !component.isEmpty else { continue }
            let parts = component.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
            let key = parts.first.map(String.init) ?? ""
            let value = parts.count > 1 ? String(parts[1]) : ""
            let decodedKey = decodeFormComponent(key)
            let decodedValue = decodeFormComponent(value)
            pairs.append((decodedKey, decodedValue))
        }
        pairs.sort { lhs, rhs in
            let keyOrder = compareUTF8(lhs.0, rhs.0)
            return keyOrder == 0 ? compareUTF8(lhs.1, rhs.1) < 0 : keyOrder < 0
        }
        return pairs
            .map { key, value in
                "\(encodeFormComponent(key))=\(encodeFormComponent(value))"
            }
            .joined(separator: "&")
    }

    /// Build the canonical request bytes for signing.
    public static func canonicalRequestMessage(method: String,
                                               url: URL,
                                               body: Data? = nil) throws -> Data {
        let exactMethod: String
        let target: CanonicalRequestV1Target
        do {
            exactMethod = try canonicalRequestV1Method(method)
            target = try canonicalRequestV1Target(url)
        } catch let failure as CanonicalRequestV1TargetValidationError {
            throw canonicalRequestError(from: failure)
        }
        let query = try canonicalQueryString(from: target.query)
        let digest = SHA256.hash(data: body ?? Data())
        let rendered = "\(exactMethod.uppercased())\n\(target.path)\n\(query)\n\(hexString(from: digest))"
        return Data(rendered.utf8)
    }

    /// Build canonical request bytes bound to one exact genesis-derived network.
    public static func signatureMessage(networkId: NetworkId,
                                        method: String,
                                        url: URL,
                                        body: Data? = nil,
                                        timestampMs: UInt64,
                                        nonce: String) throws -> Data {
        try validateNonce(nonce)
        var message = Data("iroha.app.request.network.v1\0".utf8)
        message.append(networkId.bytes)
        message.append(try canonicalRequestMessage(method: method, url: url, body: body))
        message.append(Data("\n\(timestampMs)\n\(nonce)".utf8))
        return message
    }

    /// Build canonical signing headers including freshness metadata.
    public static func buildHeaders(method: String,
                                    url: URL,
                                    body: Data? = nil,
                                    accountId: String,
                                    privateKey: Data,
                                    networkId: NetworkId,
                                    timestampMs: UInt64 = UInt64(Date().timeIntervalSince1970 * 1000),
                                    nonce: String = UUID().uuidString.replacingOccurrences(of: "-", with: "")) throws -> [String: String] {
        try validateAccount(accountId)
        guard let accountHeaderValue = canonicalRequestAccountHeaderValue(accountId) else {
            throw ToriiCanonicalRequestError.invalidAccountId
        }
        let message = try signatureMessage(
            networkId: networkId,
            method: method,
            url: url,
            body: body,
            timestampMs: timestampMs,
            nonce: nonce
        )
        let signer = try SigningKey.ed25519(privateKey: privateKey)
        let signature = try signer.sign(message)
        return [
            headerAccount: accountHeaderValue,
            headerSignature: signature.base64EncodedString(),
            headerTimestampMs: String(timestampMs),
            headerNonce: nonce,
        ]
    }

    private static func encodeFormComponent(_ value: String) -> String {
        let hex = Array("0123456789ABCDEF".utf8)
        var encoded: [UInt8] = []
        encoded.reserveCapacity(value.utf8.count)
        for byte in value.utf8 {
            switch byte {
            case UInt8(ascii: "A")...UInt8(ascii: "Z"),
                 UInt8(ascii: "a")...UInt8(ascii: "z"),
                 UInt8(ascii: "0")...UInt8(ascii: "9"),
                 UInt8(ascii: "*"), UInt8(ascii: "-"), UInt8(ascii: "."), UInt8(ascii: "_"):
                encoded.append(byte)
            case UInt8(ascii: " "):
                encoded.append(UInt8(ascii: "+"))
            default:
                encoded.append(UInt8(ascii: "%"))
                encoded.append(hex[Int(byte >> 4)])
                encoded.append(hex[Int(byte & 0x0f)])
            }
        }
        return String(decoding: encoded, as: UTF8.self)
    }

    private static func validateAccount(_ value: String) throws {
        guard !value.isEmpty else {
            throw ToriiCanonicalRequestError.missingAccountId
        }
        guard value.utf8.count <= maxAccountLiteralBytesV1 else {
            throw ToriiCanonicalRequestError.accountTooLarge
        }
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw ToriiCanonicalRequestError.missingAccountId
        }
        guard trimmed == value else {
            throw ToriiCanonicalRequestError.invalidAccountId
        }
        guard canonicalRequestAccountHeaderValue(value) != nil else {
            throw ToriiCanonicalRequestError.invalidAccountId
        }
    }

    private static func validateNonce(_ value: String) throws {
        guard !value.isEmpty else {
            throw ToriiCanonicalRequestError.missingNonce
        }
        guard value.utf8.count <= 256,
              value.utf8.allSatisfy({ $0 >= 0x21 && $0 <= 0x7e }) else {
            throw ToriiCanonicalRequestError.invalidNonce
        }
    }

    private static func validateCanonicalQuery(_ raw: String?) throws {
        do {
            try validateCanonicalRequestV1Query(raw)
        } catch CanonicalRequestV1QueryValidationError.queryTooLarge {
            throw ToriiCanonicalRequestError.queryTooLarge
        } catch CanonicalRequestV1QueryValidationError.tooManyQueryPairs {
            throw ToriiCanonicalRequestError.tooManyQueryPairs
        }
    }

    private static func canonicalRequestError(
        from failure: CanonicalRequestV1TargetValidationError
    ) -> ToriiCanonicalRequestError {
        switch failure {
        case .methodTooLarge:
            return .methodTooLarge
        case .invalidMethod:
            return .invalidMethod
        case .pathTooLarge:
            return .pathTooLarge
        case .invalidPath:
            return .invalidPath
        }
    }

    private static func decodeFormComponent(_ value: String) -> String {
        let raw = Array(value.utf8)
        var decoded: [UInt8] = []
        decoded.reserveCapacity(raw.count)
        var index = 0
        while index < raw.count {
            let byte = raw[index]
            if byte == UInt8(ascii: "+") {
                decoded.append(UInt8(ascii: " "))
                index += 1
            } else if byte == UInt8(ascii: "%"), index + 2 < raw.count,
                      let high = hexValue(raw[index + 1]),
                      let low = hexValue(raw[index + 2]) {
                decoded.append((high << 4) | low)
                index += 3
            } else {
                decoded.append(byte)
                index += 1
            }
        }
        return String(decoding: decoded, as: UTF8.self)
    }

    private static func hexValue(_ byte: UInt8) -> UInt8? {
        switch byte {
        case UInt8(ascii: "0")...UInt8(ascii: "9"):
            return byte - UInt8(ascii: "0")
        case UInt8(ascii: "A")...UInt8(ascii: "F"):
            return byte - UInt8(ascii: "A") + 10
        case UInt8(ascii: "a")...UInt8(ascii: "f"):
            return byte - UInt8(ascii: "a") + 10
        default:
            return nil
        }
    }

    private static func compareUTF8(_ left: String, _ right: String) -> Int {
        let leftBytes = Array(left.utf8)
        let rightBytes = Array(right.utf8)
        for index in 0..<min(leftBytes.count, rightBytes.count) where leftBytes[index] != rightBytes[index] {
            return Int(leftBytes[index]) - Int(rightBytes[index])
        }
        return leftBytes.count - rightBytes.count
    }

    private static func hexString<D: Sequence>(from bytes: D) -> String where D.Element == UInt8 {
        bytes.map { String(format: "%02x", $0) }.joined()
    }
}
