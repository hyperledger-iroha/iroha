import Foundation
import CryptoKit

/// Helpers for building canonical request signatures accepted by Torii app endpoints.
public enum ToriiCanonicalRequest {
    public static let headerAccount = "X-Iroha-Account"
    public static let headerSignature = "X-Iroha-Signature"

    /// Canonicalise a raw query string by decoding, sorting, and re-encoding.
    public static func canonicalQueryString(from raw: String?) -> String {
        guard let raw, !raw.isEmpty else { return "" }
        var pairs: [(String, String)] = []
        for component in raw.split(separator: "&", omittingEmptySubsequences: false) {
            let parts = component.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
            let key = parts.first.map(String.init) ?? ""
            let value = parts.count > 1 ? String(parts[1]) : ""
            let decodedKey = key.replacingOccurrences(of: "+", with: " ").removingPercentEncoding ?? key
            let decodedValue = value.replacingOccurrences(of: "+", with: " ").removingPercentEncoding ?? value
            pairs.append((decodedKey, decodedValue))
        }
        pairs.sort { lhs, rhs in
            if lhs.0 == rhs.0 {
                return lhs.1 < rhs.1
            }
            return lhs.0 < rhs.0
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
                                               body: Data? = nil) -> Data {
        let query = canonicalQueryString(from: url.query)
        let path = url.path.isEmpty ? "/" : url.path
        let upperMethod = method.uppercased()
        let digest = SHA256.hash(data: body ?? Data())
        let rendered = "\(upperMethod)\n\(path)\n\(query)\n\(hexString(from: digest))"
        return Data(rendered.utf8)
    }

    /// Build canonical signing headers (`X-Iroha-Account`/`X-Iroha-Signature`).
    public static func buildHeaders(method: String,
                                    url: URL,
                                    body: Data? = nil,
                                    accountId: String,
                                    privateKey: Data) throws -> [String: String] {
        let message = canonicalRequestMessage(method: method, url: url, body: body)
        let signer = try SigningKey.ed25519(privateKey: privateKey)
        let signature = try signer.sign(message)
        return [
            headerAccount: accountId,
            headerSignature: signature.base64EncodedString(),
        ]
    }

    private static func encodeFormComponent(_ value: String) -> String {
        let allowed = CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~")
        let encoded = value.addingPercentEncoding(withAllowedCharacters: allowed) ?? value
        return encoded.replacingOccurrences(of: "%20", with: "+")
    }

    private static func hexString<D: Sequence>(from bytes: D) -> String where D.Element == UInt8 {
        bytes.map { String(format: "%02x", $0) }.joined()
    }
}
