import Foundation

/// Fixed selectors of the sole current finalized-load read route. They identify an original;
/// they do not authorize issuance, prove finality or select a different wallet incarnation.
public struct ToriiKagemushaWalletLoadSelectionV1: Equatable, Sendable {
    public let schemeID: Data
    public let walletID: Data
    public let requestID: Data

    public init(schemeID: Data, walletID: Data, requestID: Data) throws {
        for value in [schemeID, walletID, requestID] {
            guard value.count == 32, value.contains(where: { $0 != 0 }) else {
                throw ToriiClientError.invalidPayload("KAGEMUSHA load selectors must be nonzero 32-byte identities")
            }
        }
        self.schemeID = Data(schemeID)
        self.walletID = Data(walletID)
        self.requestID = Data(requestID)
    }

    var path: String {
        func hex(_ value: Data) -> String { value.map { String(format: "%02x", $0) }.joined() }
        return "/v1/kagemusha/\(hex(schemeID))/wallets/\(hex(walletID))/loads/\(hex(requestID))"
    }
}

/// Exact bounded HTTP original of `KagemushaWalletLoadIssuanceV1`, for Native admission.
///
/// This holder deliberately exposes no amount, pending verdict, voucher or balance. The
/// server's canonical issuance has an optional signed voucher; Native must decode it, bind
/// every original and verify load authorization before any state transition. A 200 response
/// containing only a pending unsigned body is never spendable value.
public struct ToriiKagemushaWalletLoadIssuanceOriginalV1: Sendable {
    /// Includes the maximum canonical account literal/frame (36 KiB), the fixed voucher body,
    /// a bounded 1 KiB voucher and Norito overhead, with a finite local transport allocation.
    public static let maximumBytes = 64 * 1024
    public let selection: ToriiKagemushaWalletLoadSelectionV1
    public let payerAccountID: String
    public let networkID: NetworkId
    public let canonicalResponseOriginal: Data

    init(selection: ToriiKagemushaWalletLoadSelectionV1, payerAccountID: String,
        networkID: NetworkId, expectedURL: URL?, response: HTTPURLResponse, bytes: Data) throws {
        guard response.statusCode == 200, let expectedURL,
              response.url?.absoluteString == expectedURL.absoluteString else {
            throw ToriiClientError.invalidResponse
        }
        guard response.value(forHTTPHeaderField: "Content-Type")?
            .trimmingCharacters(in: .whitespacesAndNewlines) == "application/x-norito" else {
            throw ToriiClientError.invalidPayload("KAGEMUSHA issuance requires exact application/x-norito")
        }
        let encoding = response.value(forHTTPHeaderField: "Content-Encoding")?
            .trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard encoding == nil || encoding == "identity" else {
            throw ToriiClientError.invalidPayload("KAGEMUSHA issuance must preserve the identity representation")
        }
        guard !bytes.isEmpty, bytes.count <= Self.maximumBytes else {
            throw ToriiClientError.invalidPayload("KAGEMUSHA issuance original is empty or exceeds its bound")
        }
        if let length = response.value(forHTTPHeaderField: "Content-Length") {
            let digits = length.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !digits.isEmpty, digits.utf8.allSatisfy({ (48...57).contains($0) }),
                  let count = Int(digits), count == bytes.count else {
                throw ToriiClientError.invalidPayload("KAGEMUSHA issuance original length differs")
            }
        }
        self.selection = selection
        self.payerAccountID = payerAccountID
        self.networkID = networkID
        self.canonicalResponseOriginal = Data(bytes)
    }
}
