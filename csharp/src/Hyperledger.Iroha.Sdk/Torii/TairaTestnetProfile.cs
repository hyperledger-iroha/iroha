namespace Hyperledger.Iroha.Torii;

/// <summary>Stable, non-secret public metadata for the SORA Taira testnet.</summary>
public static class TairaTestnetProfile
{
    /// <summary>Public Torii origin.</summary>
    public static Uri ToriiBaseUri { get; } = new("https://taira.sora.org", UriKind.Absolute);

    /// <summary>Stable semantic chain UUID; this is not a transaction-signing <see cref="NetworkId"/>.</summary>
    public const string ChainId = "fc56984b-2be7-431d-840e-21514d1883f0";

    /// <summary>Canonical I105 address discriminant for Taira.</summary>
    public const ushort I105Discriminant = 369;

    /// <summary>Canonical Digital Shekel asset-definition ID used by Kagemusha on Taira.</summary>
    public const string KagemushaAssetDefinitionId = "7ZepsJTHCVLKsrFFNZGSRGZgvBhv";

    /// <summary>Canonical Digital Shekel alias used by Kagemusha on Taira.</summary>
    public const string KagemushaAssetAlias = "ds#boi.is";

    /// <summary>Canonical Digital Shekel fixed-point scale used by Kagemusha on Taira.</summary>
    public const uint KagemushaAssetScale = 2;

    /// <summary>Public Taira XOR asset-definition ID used for transaction fees.</summary>
    public const string XorAssetDefinitionId = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";

    /// <summary>Public Taira XOR alias used for transaction fees.</summary>
    public const string XorAssetAlias = "xor#universal";

    /// <summary>Public Taira XOR fee-asset fixed-point scale.</summary>
    public const uint XorAssetScale = 9;

    /// <summary>
    /// Creates Taira client options bound to the caller-supplied deployed genesis identity.
    /// Taira resets can change <see cref="NetworkId"/>, so this profile never guesses it.
    /// </summary>
    public static ToriiClientOptions CreateClientOptions(NetworkId deployedNetworkId)
    {
        ArgumentNullException.ThrowIfNull(deployedNetworkId);
        return new ToriiClientOptions
        {
            NetworkId = deployedNetworkId,
        };
    }

    /// <summary>Creates a Taira Torii client with an exact caller-supplied network identity.</summary>
    public static ToriiClient CreateClient(NetworkId deployedNetworkId) =>
        new(ToriiBaseUri, CreateClientOptions(deployedNetworkId));

    /// <summary>
    /// Creates a Taira Torii client over a caller-owned transport for read-only operations.
    /// </summary>
    public static ToriiClient CreateClient(
        NetworkId deployedNetworkId,
        HttpClient httpClient) =>
        new(ToriiBaseUri, httpClient, CreateClientOptions(deployedNetworkId));
}
