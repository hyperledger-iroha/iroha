using System.Text.Json;
using Hyperledger.Iroha.Http;

namespace Hyperledger.Iroha.Torii;

/// <summary>
/// Immutable trust context required by every Torii operation that creates local signatures.
/// </summary>
public sealed class ToriiLocalSigningContext
{
    /// <summary>
    /// Creates a signing context for the public Taira testnet. Call the two-argument
    /// overload for any other network; this overload never falls back to a legacy profile.
    /// </summary>
    public ToriiLocalSigningContext(NetworkId networkId)
        : this(networkId, Address.AccountAddress.TairaTestnetChainDiscriminant)
    {
    }

    /// <summary>
    /// Creates a signing context bound to one exact canonical network identifier and I105
    /// chain discriminant.
    /// </summary>
    public ToriiLocalSigningContext(NetworkId networkId, ushort chainDiscriminant)
    {
        ArgumentNullException.ThrowIfNull(networkId);
        NetworkId = networkId;
        ChainDiscriminant = chainDiscriminant;
    }

    /// <summary>Exact network identity expected in every server-prepared signing payload.</summary>
    public NetworkId NetworkId { get; }

    /// <summary>Exact I105 chain discriminant required for locally signed account identities.</summary>
    public ushort ChainDiscriminant { get; }
}

public sealed class ToriiClientOptions
{
    private JsonSerializerOptions jsonSerializerOptions = new(JsonSerializerDefaults.Web);

    public string? BearerToken { get; init; }

    public CanonicalRequestCredentials? CanonicalRequestCredentials { get; init; }

    /// <summary>
    /// Immutable network trust context for canonical HTTP auth and locally signed drafts.
    /// </summary>
    public ToriiLocalSigningContext? LocalSigningContext { get; init; }

    public JsonSerializerOptions JsonSerializerOptions
    {
        get => new(jsonSerializerOptions);
        init
        {
            ArgumentNullException.ThrowIfNull(value);
            jsonSerializerOptions = new JsonSerializerOptions(value);
        }
    }

    internal ToriiClientOptions Snapshot() => new()
    {
        BearerToken = BearerToken,
        CanonicalRequestCredentials = CanonicalRequestCredentials,
        LocalSigningContext = LocalSigningContext,
        JsonSerializerOptions = jsonSerializerOptions,
    };
}
