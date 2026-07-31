using System.Text.Json;
using Hyperledger.Iroha.Http;

namespace Hyperledger.Iroha.Torii;

/// <summary>
/// Immutable trust context required by Torii operations that return bytes for local signing.
/// </summary>
public sealed class ToriiLocalSigningContext
{
    /// <summary>Creates a signing context bound to one exact canonical chain identifier.</summary>
    public ToriiLocalSigningContext(string chainId)
    {
        ArgumentNullException.ThrowIfNull(chainId);
        if (chainId.Length is 0 or > 128
            || !IsAsciiAlphanumeric(chainId[0])
            || !IsAsciiAlphanumeric(chainId[^1])
            || chainId.Any(static character =>
                !IsAsciiAlphanumeric(character) && character is not ('.' or '_' or ':' or '-')))
        {
            throw new ArgumentException(
                "chainId must be exact canonical ASCII ChainId text.",
                nameof(chainId));
        }

        ChainId = chainId;
    }

    /// <summary>Exact chain identity expected in every server-prepared signing payload.</summary>
    public string ChainId { get; }

    private static bool IsAsciiAlphanumeric(char value) =>
        value is >= '0' and <= '9'
            or >= 'A' and <= 'Z'
            or >= 'a' and <= 'z';
}

public sealed class ToriiClientOptions
{
    private JsonSerializerOptions jsonSerializerOptions = new(JsonSerializerDefaults.Web);

    public string? BearerToken { get; init; }

    public CanonicalRequestCredentials? CanonicalRequestCredentials { get; init; }

    /// <summary>
    /// Immutable chain trust context for endpoints that return bytes to be signed locally.
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
