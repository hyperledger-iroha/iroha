using System.Text.Json.Nodes;

namespace Hyperledger.Iroha.Torii;

public sealed record class ToriiServerSentEvent
{
    public string? Event { get; init; }

    public string? Id { get; init; }

    public int? RetryMilliseconds { get; init; }

    public string? RawData { get; init; }

    public JsonNode? JsonData { get; init; }

    public string? Comment { get; init; }

    public bool IsComment => Comment is not null && RawData is null;
}
