import Foundation
import CryptoKit

@available(macOS 10.15, iOS 13.0, *)
public enum CanonicalRequestError: Error {
    case missingAccountId
    case missingSigningKey
    case missingNonce
}

@available(macOS 10.15, iOS 13.0, *)
public struct CanonicalRequest {
    private static let unreserved = CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~")

    private static func percentEncode(_ value: String) -> String {
        let encoded = value.addingPercentEncoding(withAllowedCharacters: unreserved) ?? value
        return encoded.replacingOccurrences(of: "%20", with: "+")
    }

    public static func canonicalQueryString(from raw: String?) -> String {
        guard let raw = raw, !raw.isEmpty else { return "" }
        var pairs: [(String, String)] = []
        for part in raw.split(separator: "&", omittingEmptySubsequences: false) {
            let components = part.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
            let name = String(components.first ?? Substring())
            let value = components.count > 1 ? String(components[1]) : ""
            let decodedName = name.replacingOccurrences(of: "+", with: " ").removingPercentEncoding ?? name
            let decodedValue = value.replacingOccurrences(of: "+", with: " ").removingPercentEncoding ?? value
            pairs.append((decodedName, decodedValue))
        }
        pairs.sort { lhs, rhs in
            if lhs.0 == rhs.0 {
                return lhs.1 < rhs.1
            }
            return lhs.0 < rhs.0
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

    public static func signatureMessage(method: String,
                                        path: String,
                                        query: String? = nil,
                                        body: Data = Data(),
                                        timestampMs: UInt64,
                                        nonce: String) throws -> Data {
        guard !nonce.isEmpty else {
            throw CanonicalRequestError.missingNonce
        }
        let base = canonicalMessage(method: method, path: path, query: query, body: body)
        let rendered = "\(String(decoding: base, as: UTF8.self))\n\(timestampMs)\n\(nonce)"
        return Data(rendered.utf8)
    }

    public static func signingHeaders(accountId: String,
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
        guard let signer = signer else {
            throw CanonicalRequestError.missingSigningKey
        }
        let message = try signatureMessage(
            method: method,
            path: path,
            query: query,
            body: body,
            timestampMs: timestampMs,
            nonce: nonce
        )
        let signature = try signer.sign(message)
        return [
            "X-Iroha-Account": accountId,
            "X-Iroha-Signature": Data(signature).base64EncodedString(),
            "X-Iroha-Timestamp-Ms": String(timestampMs),
            "X-Iroha-Nonce": nonce,
        ]
    }
}
