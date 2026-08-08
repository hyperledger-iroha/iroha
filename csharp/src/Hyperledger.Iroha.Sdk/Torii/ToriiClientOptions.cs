using System.Text.Json;
using Hyperledger.Iroha.Http;

namespace Hyperledger.Iroha.Torii;

/// <summary>
/// Immutable trust context required by Torii operations that return bytes for local signing.
/// </summary>
public sealed class ToriiLocalSigningContext
{
    /// <summary>Creates a signing context bound to one exact canonical network identifier.</summary>
    public ToriiLocalSigningContext(NetworkId networkId)
    {
        ArgumentNullException.ThrowIfNull(networkId);
        NetworkId = networkId;
    }

    /// <summary>Exact network identity expected in every server-prepared signing payload.</summary>
    public NetworkId NetworkId { get; }
}

public sealed class ToriiClientOptions
{
    private JsonSerializerOptions jsonSerializerOptions = new(JsonSerializerDefaults.Web);

    public string? BearerToken { get; init; }

    public CanonicalRequestCredentials? CanonicalRequestCredentials { get; init; }

    /// <summary>
    /// Immutable network trust context for endpoints that return bytes to be signed locally.
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
