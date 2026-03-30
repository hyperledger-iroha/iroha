using System.Text.Json;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

public sealed class ToriiPipelineEvent
{
    [JsonPropertyName("category")]
    public string Category { get; set; } = string.Empty;

    [JsonPropertyName("event")]
    public string Event { get; set; } = string.Empty;

    [JsonPropertyName("status")]
    public string? Status { get; set; }

    [JsonPropertyName("hash")]
    public string? Hash { get; set; }

    [JsonPropertyName("lane_id")]
    public ulong? LaneId { get; set; }

    [JsonPropertyName("dataspace_id")]
    public ulong? DataspaceId { get; set; }

    [JsonPropertyName("block_height")]
    public ulong? BlockHeight { get; set; }

    [JsonPropertyName("kind")]
    public string? Kind { get; set; }

    [JsonPropertyName("details")]
    public string? Details { get; set; }

    [JsonPropertyName("height")]
    public ulong? Height { get; set; }

    [JsonPropertyName("epoch_id")]
    public ulong? EpochId { get; set; }

    [JsonPropertyName("global_state_root")]
    public string? GlobalStateRoot { get; set; }

    [JsonPropertyName("block_hash")]
    public string? BlockHash { get; set; }

    [JsonPropertyName("view")]
    public ulong? View { get; set; }

    [JsonPropertyName("epoch")]
    public ulong? Epoch { get; set; }

    [JsonPropertyName("read_count")]
    public ulong? ReadCount { get; set; }

    [JsonPropertyName("write_count")]
    public ulong? WriteCount { get; set; }

    [JsonIgnore]
    public string? LastEventId { get; set; }

    [JsonIgnore]
    public string? SseEventName { get; set; }

    [JsonIgnore]
    public int? RetryMilliseconds { get; set; }

    [JsonExtensionData]
    public Dictionary<string, JsonElement>? AdditionalProperties { get; set; }
}
