import Foundation

/// Failures raised while constructing exact-network Torii operator authentication.
public enum ToriiOperatorRequestError: Error, Equatable, LocalizedError, Sendable {
    /// The nonce was empty, non-ASCII, contained whitespace, or exceeded the protocol limit.
    case invalidNonce
    /// The signing key returned no signature bytes.
    case emptySignature

    public var errorDescription: String? {
        switch self {
        case .invalidNonce:
            return "Operator request nonce must contain 1...256 printable ASCII bytes."
        case .emptySignature:
            return "Operator signing key returned an empty signature."
        }
    }
}

/// Immutable exact-network signing context for Torii operator requests.
///
/// A context is pinned to the genesis-derived ``NetworkId`` supplied at
/// construction. Each request still receives fresh timestamp and nonce values;
/// callers cannot replace the network identity or precompute authentication
/// headers after the context has been installed on a client.
public final class ToriiOperatorSigningContext: @unchecked Sendable {
    /// Exact network identity included in every operator signature domain.
    public let networkId: NetworkId
    /// Canonical Iroha public-key multihash sent to Torii.
    public let publicKey: String

    private let signingKey: SigningKey

    /// Construct an immutable operator signer from an existing SDK signing key.
    public init(networkId: NetworkId, signingKey: SigningKey) throws {
        self.networkId = networkId
        self.signingKey = signingKey
        self.publicKey = CanonicalNorito.publicKeyMultihash(
            algorithm: signingKey.algorithm,
            payload: try signingKey.publicKey()
        )
    }

    /// Build fresh operator authentication for one exact request.
    ///
    /// The signature covers the uppercase method, percent-encoded path,
    /// canonical sorted query, SHA-256 body digest, timestamp, and nonce under
    /// the operator-request domain and this context's immutable network ID.
    public func buildHeaders(
        method: String,
        url: URL,
        body: Data = Data(),
        timestampMs: UInt64 = UInt64(max(0, Date().timeIntervalSince1970 * 1_000).rounded()),
        nonce: String = UUID().uuidString.replacingOccurrences(of: "-", with: "")
    ) throws -> [String: String] {
        guard (1...256).contains(nonce.utf8.count),
              nonce.utf8.allSatisfy({ (0x21...0x7E).contains($0) }) else {
            throw ToriiOperatorRequestError.invalidNonce
        }

        var message = Data("iroha.operator.http-request.network.v1\0".utf8)
        message.append(networkId.bytes)
        message.append(
            try ToriiCanonicalRequest.canonicalRequestMessage(
                method: method,
                url: url,
                body: body
            )
        )
        message.append(Data("\n\(timestampMs)\n\(nonce)".utf8))

        let signature = try signingKey.sign(message)
        guard !signature.isEmpty else {
            throw ToriiOperatorRequestError.emptySignature
        }
        return [
            "X-Iroha-Operator-Public-Key": publicKey,
            "X-Iroha-Operator-Timestamp-Ms": String(timestampMs),
            "X-Iroha-Operator-Nonce": nonce,
            "X-Iroha-Operator-Signature": signature.base64EncodedString(),
        ]
    }
}
