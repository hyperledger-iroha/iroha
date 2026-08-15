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
    case invalidMethod
    case pathTooLarge
    case invalidPath
}

enum CanonicalRequestV1TargetValidationError: Error {
    case methodTooLarge
    case invalidMethod
    case pathTooLarge
    case invalidPath
}

struct CanonicalRequestV1Target {
    let path: String
    let query: String?
}

let canonicalRequestMaxAliasLiteralBytesV1 = 3 * 255 + 2
private let canonicalRequestMaxMethodBytesV1 = 32
private let canonicalRequestMaxPathBytesV1 = 64 * 1024
let canonicalRequestMaxQueryPairsV1 = 64
let canonicalRequestMaxRawQueryBytesV1 = 64 * 1024

enum CanonicalRequestV1QueryValidationError: Error {
    case queryTooLarge
    case tooManyQueryPairs
}

func validateCanonicalRequestV1Query(_ raw: String?) throws {
    guard let raw, !raw.isEmpty else { return }
    guard raw.utf8.count <= canonicalRequestMaxRawQueryBytesV1 else {
        throw CanonicalRequestV1QueryValidationError.queryTooLarge
    }
    var pairCount = 0
    for component in raw.split(separator: "&", omittingEmptySubsequences: false) where !component.isEmpty {
        pairCount += 1
        guard pairCount <= canonicalRequestMaxQueryPairsV1 else {
            throw CanonicalRequestV1QueryValidationError.tooManyQueryPairs
        }
    }
}

private func canonicalRequestV1TransportQuery(_ raw: String?) -> String? {
    guard let raw, !raw.isEmpty,
          let url = URL(string: "https://canonical.invalid/?\(raw)"),
          let components = URLComponents(url: url, resolvingAgainstBaseURL: false) else {
        return raw
    }
    return components.percentEncodedQuery
}

func canonicalRequestAccountHeaderValue(_ accountId: String) -> String? {
    if let address = try? AccountAddress.parseEncoded(accountId),
       let canonicalHex = try? address.canonicalHex() {
        return canonicalHex
    }
    guard !accountId.hasPrefix("0x"),
          accountId.utf8.count <= canonicalRequestMaxAliasLiteralBytesV1,
          isCanonicalRequestAliasShape(accountId) else {
        return nil
    }
    return accountId
}

private func isCanonicalRequestAliasShape(_ value: String) -> Bool {
    let parts = value.split(separator: "@", omittingEmptySubsequences: false)
    guard parts.count == 2 else { return false }
    let scope = parts[1].split(separator: ".", omittingEmptySubsequences: false)
    return (scope.count == 1 || scope.count == 2)
        && isCanonicalRequestAliasSegment(parts[0])
        && scope.allSatisfy(isCanonicalRequestAliasSegment)
}

private func isCanonicalRequestAliasSegment(_ value: Substring) -> Bool {
    let bytes = Array(value.utf8)
    guard (1...63).contains(bytes.count),
          bytes[0] != UInt8(ascii: "-"),
          bytes[bytes.count - 1] != UInt8(ascii: "-"),
          bytes.allSatisfy({ byte in
              (UInt8(ascii: "a")...UInt8(ascii: "z")).contains(byte)
                  || (UInt8(ascii: "0")...UInt8(ascii: "9")).contains(byte)
                  || byte == UInt8(ascii: "-")
                  || byte == UInt8(ascii: "_")
          }) else {
        return false
    }
    return bytes.count < 4
        || bytes[2] != UInt8(ascii: "-")
        || bytes[3] != UInt8(ascii: "-")
        || bytes.starts(with: Array("xn--".utf8))
}

func canonicalRequestV1Method(_ method: String) throws -> String {
    guard method.utf8.count <= canonicalRequestMaxMethodBytesV1 else {
        throw CanonicalRequestV1TargetValidationError.methodTooLarge
    }
    let bytes = method.utf8
    guard !bytes.isEmpty,
          bytes.allSatisfy({ byte in
              switch byte {
              case UInt8(ascii: "A")...UInt8(ascii: "Z"),
                   UInt8(ascii: "a")...UInt8(ascii: "z"),
                   UInt8(ascii: "0")...UInt8(ascii: "9"),
                   UInt8(ascii: "!"), UInt8(ascii: "#"), UInt8(ascii: "$"),
                   UInt8(ascii: "%"), UInt8(ascii: "&"), UInt8(ascii: "'"),
                   UInt8(ascii: "*"), UInt8(ascii: "+"), UInt8(ascii: "-"),
                   UInt8(ascii: "."), UInt8(ascii: "^"), UInt8(ascii: "_"),
                   UInt8(ascii: "`"), UInt8(ascii: "|"), UInt8(ascii: "~"):
                  return true
              default:
                  return false
              }
          }) else {
        throw CanonicalRequestV1TargetValidationError.invalidMethod
    }
    return method
}

func canonicalRequestV1Path(_ path: String) throws -> String {
    let bytes = Array(path.utf8)
    guard bytes.count <= canonicalRequestMaxPathBytesV1 else {
        throw CanonicalRequestV1TargetValidationError.pathTooLarge
    }
    guard !bytes.isEmpty,
          bytes[0] == UInt8(ascii: "/"),
          bytes.count == 1 || bytes[1] != UInt8(ascii: "/"),
          bytes.allSatisfy(isCanonicalRequestRawPathByte),
          hasValidCanonicalRequestPercentEscapes(bytes),
          !hasCanonicalRequestDotSegment(bytes) else {
        throw CanonicalRequestV1TargetValidationError.invalidPath
    }
    return path
}

private func isCanonicalRequestRawPathByte(_ byte: UInt8) -> Bool {
    switch byte {
    case UInt8(ascii: "A")...UInt8(ascii: "Z"),
         UInt8(ascii: "a")...UInt8(ascii: "z"),
         UInt8(ascii: "0")...UInt8(ascii: "9"),
         UInt8(ascii: "!"), UInt8(ascii: "$"), UInt8(ascii: "%"),
         UInt8(ascii: "&"), UInt8(ascii: "'"), UInt8(ascii: "("),
         UInt8(ascii: ")"), UInt8(ascii: "*"), UInt8(ascii: "+"),
         UInt8(ascii: ","), UInt8(ascii: "-"), UInt8(ascii: "."),
         UInt8(ascii: "/"), UInt8(ascii: ":"), UInt8(ascii: ";"),
         UInt8(ascii: "="), UInt8(ascii: "@"), UInt8(ascii: "_"),
         UInt8(ascii: "~"):
        return true
    default:
        return false
    }
}

func canonicalRequestV1Target(_ url: URL) throws -> CanonicalRequestV1Target {
    guard let components = URLComponents(url: url, resolvingAgainstBaseURL: false),
          components.percentEncodedFragment == nil else {
        throw CanonicalRequestV1TargetValidationError.invalidPath
    }

    if let scheme = components.scheme {
        guard (scheme.caseInsensitiveCompare("http") == .orderedSame
                || scheme.caseInsensitiveCompare("https") == .orderedSame),
              components.host?.isEmpty == false else {
            throw CanonicalRequestV1TargetValidationError.invalidPath
        }
    } else if components.host != nil || components.port != nil || components.user != nil {
        throw CanonicalRequestV1TargetValidationError.invalidPath
    }

    let rawPath = components.percentEncodedPath
    let path = rawPath.isEmpty && components.scheme != nil ? "/" : rawPath
    return CanonicalRequestV1Target(
        path: try canonicalRequestV1Path(path),
        query: components.percentEncodedQuery
    )
}

private func hasValidCanonicalRequestPercentEscapes(_ bytes: [UInt8]) -> Bool {
    var index = 0
    while index < bytes.count {
        if bytes[index] == UInt8(ascii: "%") {
            guard index + 2 < bytes.count,
                  canonicalRequestHexValue(bytes[index + 1]) != nil,
                  canonicalRequestHexValue(bytes[index + 2]) != nil else {
                return false
            }
            index += 3
        } else {
            index += 1
        }
    }
    return true
}

private func hasCanonicalRequestDotSegment(_ bytes: [UInt8]) -> Bool {
    for segment in bytes.split(separator: UInt8(ascii: "/"), omittingEmptySubsequences: false) {
        var decoded: [UInt8] = []
        var index = segment.startIndex
        while index < segment.endIndex {
            if segment[index] == UInt8(ascii: "%"),
               segment.distance(from: index, to: segment.endIndex) >= 3,
               let high = canonicalRequestHexValue(segment[segment.index(after: index)]),
               let low = canonicalRequestHexValue(segment[segment.index(index, offsetBy: 2)]) {
                decoded.append((high << 4) | low)
                index = segment.index(index, offsetBy: 3)
            } else {
                decoded.append(segment[index])
                index = segment.index(after: index)
            }
        }
        if decoded == [UInt8(ascii: ".")] || decoded == [UInt8(ascii: "."), UInt8(ascii: ".")] {
            return true
        }
    }
    return false
}

private func canonicalRequestHexValue(_ byte: UInt8) -> UInt8? {
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

@available(macOS 10.15, iOS 13.0, *)
public struct CanonicalRequest {
    public static let maxQueryPairsV1 = canonicalRequestMaxQueryPairsV1
    public static let maxRawQueryBytesV1 = canonicalRequestMaxRawQueryBytesV1
    public static let maxAccountLiteralBytesV1 = 36 * 1024
    public static let maxAliasLiteralBytesV1 = canonicalRequestMaxAliasLiteralBytesV1
    public static let maxMethodBytesV1 = canonicalRequestMaxMethodBytesV1
    public static let maxPathBytesV1 = canonicalRequestMaxPathBytesV1

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

    /// Canonicalise a raw query after enforcing the V1 byte and pair caps.
    public static func canonicalQueryString(from raw: String?) throws -> String {
        try validateCanonicalQuery(raw)
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
                                        body: Data = Data()) throws -> Data {
        let exactMethod: String
        let exactPath: String
        do {
            exactMethod = try canonicalRequestV1Method(method)
            exactPath = try canonicalRequestV1Path(path)
        } catch let failure as CanonicalRequestV1TargetValidationError {
            throw canonicalRequestError(from: failure)
        }
        let canonicalQuery = try canonicalQueryString(from: query)
        let hash = SHA256.hash(data: body)
        let bodyHex = hash.compactMap { String(format: "%02x", $0) }.joined()
        let rendered = "\(exactMethod.uppercased())\n\(exactPath)\n\(canonicalQuery)\n\(bodyHex)"
        return Data(rendered.utf8)
    }

    public static func signatureMessage(networkId: NetworkId,
                                        method: String,
                                        path: String,
                                        query: String? = nil,
                                        body: Data = Data(),
                                        timestampMs: UInt64,
                                        nonce: String) throws -> Data {
        try validateNonce(nonce)
        let base = try canonicalMessage(method: method, path: path, query: query, body: body)
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
            query: canonicalRequestV1TransportQuery(query),
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
        do {
            try validateCanonicalRequestV1Query(raw)
        } catch CanonicalRequestV1QueryValidationError.queryTooLarge {
            throw CanonicalRequestError.queryTooLarge
        } catch CanonicalRequestV1QueryValidationError.tooManyQueryPairs {
            throw CanonicalRequestError.tooManyQueryPairs
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

    private static func canonicalRequestError(
        from failure: CanonicalRequestV1TargetValidationError
    ) -> CanonicalRequestError {
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
}
