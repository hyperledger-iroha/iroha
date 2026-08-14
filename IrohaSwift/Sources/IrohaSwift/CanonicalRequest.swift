import Foundation
import CryptoKit

@available(macOS 10.15, iOS 13.0, *)
public enum CanonicalRequestError: Error, Equatable {
    case missingAccountId
    case missingSigningKey
    case missingNonce
    case invalidAccountId
    case invalidNonce
    case queryTooLarge
    case tooManyQueryPairs
    case accountTooLarge
    case methodTooLarge
    case pathTooLarge
}

func canonicalRequestAccountHeaderValue(_ accountId: String) -> String? {
    if let address = try? AccountAddress.parseEncoded(accountId),
       let canonicalHex = try? address.canonicalHex() {
        return canonicalHex
    }
    guard accountId.utf8.allSatisfy({ $0 <= 0x7f }) else {
        return nil
    }
    return accountId
}

@available(macOS 10.15, iOS 13.0, *)
public struct CanonicalRequest {
    public static let maxQueryPairsV1 = 64
    public static let maxRawQueryBytesV1 = 64 * 1024
    public static let maxAccountLiteralBytesV1 = 36 * 1024
    public static let maxMethodBytesV1 = 32
    public static let maxPathBytesV1 = 64 * 1024

    private static func percentEncode(_ value: String) -> String {
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

    public static func canonicalQueryString(from raw: String?) -> String {
        guard let raw = raw, !raw.isEmpty else { return "" }
        var pairs: [(String, String)] = []
        for part in raw.split(separator: "&", omittingEmptySubsequences: false) {
            guard !part.isEmpty else { continue }
            let components = part.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
            let name = String(components.first ?? Substring())
            let value = components.count > 1 ? String(components[1]) : ""
            let decodedName = decodeFormComponent(name)
            let decodedValue = decodeFormComponent(value)
            pairs.append((decodedName, decodedValue))
        }
        pairs.sort { lhs, rhs in
            let keyOrder = compareUTF8(lhs.0, rhs.0)
            return keyOrder == 0 ? compareUTF8(lhs.1, rhs.1) < 0 : keyOrder < 0
        }
        return pairs
            .map { "\(percentEncode($0.0))=\(percentEncode($0.1))" }
            .joined(separator: "&")
    }

    public static func canonicalMessage(method: String,
                                        path: String,
                                        query: String? = nil,
                                        body: Data = Data()) -> Data {
        let canonicalQuery = canonicalQueryString(from: query)
        let hash = SHA256.hash(data: body)
        let bodyHex = hash.compactMap { String(format: "%02x", $0) }.joined()
        let rendered = "\(method.uppercased())\n\(path)\n\(canonicalQuery)\n\(bodyHex)"
        return Data(rendered.utf8)
    }

    public static func signatureMessage(networkId: NetworkId,
                                        method: String,
                                        path: String,
                                        query: String? = nil,
                                        body: Data = Data(),
                                        timestampMs: UInt64,
                                        nonce: String) throws -> Data {
        guard method.utf8.count <= maxMethodBytesV1 else {
            throw CanonicalRequestError.methodTooLarge
        }
        guard path.utf8.count <= maxPathBytesV1 else {
            throw CanonicalRequestError.pathTooLarge
        }
        try validateCanonicalQuery(query)
        try validateNonce(nonce)
        let base = canonicalMessage(method: method, path: path, query: query, body: body)
        var message = Data("iroha.app.request.network.v1\0".utf8)
        message.append(networkId.bytes)
        message.append(base)
        message.append(Data("\n\(timestampMs)\n\(nonce)".utf8))
        return message
    }

    public static func signingHeaders(accountId: String,
                                      networkId: NetworkId,
                                      method: String,
                                      path: String,
                                      query: String? = nil,
                                      body: Data = Data(),
                                      signer: SigningKey?,
                                      timestampMs: UInt64 = UInt64(Date().timeIntervalSince1970 * 1000),
                                      nonce: String = UUID().uuidString.replacingOccurrences(of: "-", with: "")) throws -> [String: String] {
        guard !accountId.isEmpty else {
            throw CanonicalRequestError.missingAccountId
        }
        guard accountId.utf8.count <= maxAccountLiteralBytesV1 else {
            throw CanonicalRequestError.accountTooLarge
        }
        guard !accountId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw CanonicalRequestError.missingAccountId
        }
        guard accountId.trimmingCharacters(in: .whitespacesAndNewlines) == accountId else {
            throw CanonicalRequestError.invalidAccountId
        }
        guard let accountHeaderValue = canonicalRequestAccountHeaderValue(accountId) else {
            throw CanonicalRequestError.invalidAccountId
        }
        guard let signer = signer else {
            throw CanonicalRequestError.missingSigningKey
        }
        let message = try signatureMessage(
            networkId: networkId,
            method: method,
            path: path,
            query: query,
            body: body,
            timestampMs: timestampMs,
            nonce: nonce
        )
        let signature = try signer.sign(message)
        return [
            "X-Iroha-Account": accountHeaderValue,
            "X-Iroha-Signature": Data(signature).base64EncodedString(),
            "X-Iroha-Timestamp-Ms": String(timestampMs),
            "X-Iroha-Nonce": nonce,
        ]
    }

    private static func validateCanonicalQuery(_ raw: String?) throws {
        guard let raw, !raw.isEmpty else { return }
        guard raw.utf8.count <= maxRawQueryBytesV1 else {
            throw CanonicalRequestError.queryTooLarge
        }
        var pairCount = 0
        for component in raw.split(separator: "&", omittingEmptySubsequences: false) where !component.isEmpty {
            pairCount += 1
            guard pairCount <= maxQueryPairsV1 else {
                throw CanonicalRequestError.tooManyQueryPairs
            }
        }
    }

    private static func validateNonce(_ nonce: String) throws {
        guard !nonce.isEmpty else {
            throw CanonicalRequestError.missingNonce
        }
        guard nonce.utf8.count <= 256,
              nonce.utf8.allSatisfy({ $0 >= 0x21 && $0 <= 0x7e }) else {
            throw CanonicalRequestError.invalidNonce
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
}
