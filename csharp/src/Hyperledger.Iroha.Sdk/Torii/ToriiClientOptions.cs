using Hyperledger.Iroha.Http;

namespace Hyperledger.Iroha.Torii;

/// <summary>Authentication and network trust inputs for one Torii client.</summary>
public sealed class ToriiClientOptions
{
    /// <summary>Optional HTTP bearer credential.</summary>
    public string? BearerToken { get; init; }

    /// <summary>Optional canonical request-signing credential.</summary>
    public CanonicalRequestCredentials? CanonicalRequestCredentials { get; init; }

    /// <summary>
    /// Exact genesis-derived network identity used by canonical authentication and locally
    /// verified signing drafts.
    /// </summary>
    public NetworkId? NetworkId { get; init; }
}
