import Foundation

/// Stable, non-secret public metadata for the SORA Taira testnet.
public enum TairaTestnetProfile {
    /// Public Torii origin.
    public static let toriiBaseURL = URL(string: "https://taira.sora.org")!
    /// Stable semantic chain UUID; this is not a transaction-signing `NetworkId`.
    public static let chainId = "fc56984b-2be7-431d-840e-21514d1883f0"
    /// Canonical I105 address discriminant for Taira.
    public static let i105Discriminant: UInt16 = 369
    /// Canonical Digital Shekel asset-definition ID used by Kagemusha on Taira.
    public static let kagemushaAssetDefinitionId = "7ZepsJTHCVLKsrFFNZGSRGZgvBhv"
    /// Canonical Digital Shekel alias used by Kagemusha on Taira.
    public static let kagemushaAssetAlias = "ds#boi.is"
    /// Canonical Digital Shekel fixed-point scale used by Kagemusha on Taira.
    public static let kagemushaAssetScale: UInt32 = 2
    /// Public Taira XOR asset-definition ID used for transaction fees.
    public static let xorAssetDefinitionId = "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
    /// Public Taira XOR alias used for transaction fees.
    public static let xorAssetAlias = "xor#universal"
    /// Public Taira XOR fee-asset fixed-point scale.
    public static let xorAssetScale: UInt32 = 9

    /// Creates a Taira client bound to the caller-supplied deployed genesis identity.
    ///
    /// Taira resets can change `NetworkId`; this profile never guesses it from the
    /// stable semantic chain label or from an unauthenticated server response.
    public static func makeClient(
        deployedNetworkId: NetworkId,
        session: URLSession = .shared,
        defaultHeaders: [String: String] = [:]
    ) -> ToriiClient {
        ToriiClient(
            baseURL: toriiBaseURL,
            session: session,
            defaultHeaders: defaultHeaders,
            localSigningContext: ToriiLocalSigningContext(networkId: deployedNetworkId)
        )
    }
}
