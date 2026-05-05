using System.Text.Json;
using Hyperledger.Iroha.Http;

namespace Hyperledger.Iroha.Torii;

public sealed class ToriiClientOptions
{
    public string? BearerToken { get; init; }

    public CanonicalRequestCredentials? CanonicalRequestCredentials { get; init; }

    public JsonSerializerOptions JsonSerializerOptions { get; init; } = new(JsonSerializerDefaults.Web);
}
