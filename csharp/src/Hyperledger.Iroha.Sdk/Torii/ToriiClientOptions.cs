using System.Text.Json;
using Hyperledger.Iroha.Http;

namespace Hyperledger.Iroha.Torii;

public sealed class ToriiClientOptions
{
    private JsonSerializerOptions jsonSerializerOptions = new(JsonSerializerDefaults.Web);

    public string? BearerToken { get; init; }

    public CanonicalRequestCredentials? CanonicalRequestCredentials { get; init; }

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
        JsonSerializerOptions = jsonSerializerOptions,
    };
}
