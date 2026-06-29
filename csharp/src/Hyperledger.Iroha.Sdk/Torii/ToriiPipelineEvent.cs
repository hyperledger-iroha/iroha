using System.Text.Json;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

[JsonConverter(typeof(ToriiPipelineEventJsonConverter))]
public sealed class ToriiPipelineEvent
{
    private string category = string.Empty;
    private string eventName = string.Empty;
    private string? status;
    private string? hash;
    private string? kind;
    private string? details;
    private string? globalStateRoot;
    private string? blockHash;
    private string? lastEventId;
    private string? sseEventName;
    private int? retryMilliseconds;
    private Dictionary<string, JsonElement>? additionalProperties;

    [JsonPropertyName("category")]
    public string Category
    {
        get => category;
        set => category = ToriiSseDirectMetadata.RequireExactValue(value, "Pipeline", nameof(Category));
    }

    [JsonPropertyName("event")]
    public string Event
    {
        get => eventName;
        set => eventName = ToriiSseDirectMetadata.RequireExactTokenText(value, nameof(Event));
    }

    [JsonPropertyName("status")]
    public string? Status
    {
        get => status;
        set => status = ToriiSseDirectMetadata.RequireOptionalExactTokenText(value, nameof(Status));
    }

    [JsonPropertyName("hash")]
    public string? Hash
    {
        get => hash;
        set => hash = ToriiSseDirectMetadata.RequireOptionalExactSizedHex(value, nameof(Hash), 32);
    }

    [JsonPropertyName("lane_id")]
    public ulong? LaneId { get; set; }

    [JsonPropertyName("dataspace_id")]
    public ulong? DataspaceId { get; set; }

    [JsonPropertyName("block_height")]
    public ulong? BlockHeight { get; set; }

    [JsonPropertyName("kind")]
    public string? Kind
    {
        get => kind;
        set => kind = ToriiSseDirectMetadata.RequireOptionalExactTokenText(value, nameof(Kind));
    }

    [JsonPropertyName("details")]
    public string? Details
    {
        get => details;
        set => details = ToriiSseDirectMetadata.RequireOptionalExactNonEmptyText(value, nameof(Details));
    }

    [JsonPropertyName("height")]
    public ulong? Height { get; set; }

    [JsonPropertyName("epoch_id")]
    public ulong? EpochId { get; set; }

    [JsonPropertyName("global_state_root")]
    public string? GlobalStateRoot
    {
        get => globalStateRoot;
        set => globalStateRoot = ToriiSseDirectMetadata.RequireOptionalExactSizedHex(
            value,
            nameof(GlobalStateRoot),
            32);
    }

    [JsonPropertyName("block_hash")]
    public string? BlockHash
    {
        get => blockHash;
        set => blockHash = ToriiSseDirectMetadata.RequireOptionalExactSizedHex(value, nameof(BlockHash), 32);
    }

    [JsonPropertyName("view")]
    public ulong? View { get; set; }

    [JsonPropertyName("epoch")]
    public ulong? Epoch { get; set; }

    [JsonPropertyName("read_count")]
    public ulong? ReadCount { get; set; }

    [JsonPropertyName("write_count")]
    public ulong? WriteCount { get; set; }

    [JsonIgnore]
    public string? LastEventId
    {
        get => lastEventId;
        set => lastEventId = ToriiSseDirectMetadata.RequireOptionalExactNonEmptyText(value, nameof(LastEventId));
    }

    [JsonIgnore]
    public string? SseEventName
    {
        get => sseEventName;
        set => sseEventName = ToriiSseDirectMetadata.RequireOptionalExactNonEmptyText(value, nameof(SseEventName));
    }

    [JsonIgnore]
    public int? RetryMilliseconds
    {
        get => retryMilliseconds;
        set
        {
            if (value < 0)
            {
                throw new ArgumentOutOfRangeException(
                    nameof(RetryMilliseconds),
                    "Retry milliseconds must be non-negative.");
            }

            retryMilliseconds = value;
        }
    }

    [JsonExtensionData]
    public Dictionary<string, JsonElement>? AdditionalProperties
    {
        get => ToriiJsonElementDictionarySnapshot.Copy(additionalProperties);
        set => additionalProperties = ToriiJsonElementDictionarySnapshot.Copy(value);
    }
}

internal static class ToriiSseDirectMetadata
{
    internal static string RequireExactValue(string? value, string expected, string paramName)
    {
        var exact = RequireExactTokenText(value, paramName);
        if (!string.Equals(exact, expected, StringComparison.Ordinal))
        {
            throw new ArgumentException($"Value must be {expected}.", paramName);
        }

        return exact;
    }

    internal static string RequireProofEventName(string? value, string paramName)
    {
        var exact = RequireExactTokenText(value, paramName);
        if (!exact.StartsWith("Proof", StringComparison.Ordinal))
        {
            throw new ArgumentException("Value must be a proof event.", paramName);
        }

        return exact;
    }

    internal static string RequireExactTokenText(string? value, string paramName)
    {
        var exact = RequireExactNonEmptyText(value, paramName);
        if (exact.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Value must not contain whitespace.", paramName);
        }

        return exact;
    }

    internal static string? RequireOptionalExactTokenText(string? value, string paramName)
    {
        return value is null ? null : RequireExactTokenText(value, paramName);
    }

    internal static string? RequireOptionalExactNonEmptyText(string? value, string paramName)
    {
        return value is null ? null : RequireExactNonEmptyText(value, paramName);
    }

    internal static string? RequireOptionalExactSizedHex(string? value, string paramName, int expectedBytes)
    {
        return value is null ? null : RequireExactSizedHex(value, paramName, expectedBytes);
    }

    internal static string RequireExactSizedHex(string? value, string paramName, int expectedBytes)
    {
        var exact = RequireExactNonEmptyText(value, paramName);
        if (exact.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Value must not contain whitespace.", paramName);
        }

        if (exact.Length != expectedBytes * 2 || !exact.All(IsLowercaseHexCharacter))
        {
            throw new ArgumentException(
                $"Value must be an exact lowercase {expectedBytes}-byte hex string.",
                paramName);
        }

        return exact;
    }

    private static string RequireExactNonEmptyText(string? value, string paramName)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException("Value must be a non-empty string.", paramName);
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException("Value must not contain surrounding whitespace.", paramName);
        }

        if (value.Any(char.IsControl))
        {
            throw new ArgumentException("Value must not contain control characters.", paramName);
        }

        return value;
    }

    private static bool IsLowercaseHexCharacter(char value)
    {
        return value is (>= '0' and <= '9') or (>= 'a' and <= 'f');
    }
}

internal static class ToriiJsonElementDictionarySnapshot
{
    internal static Dictionary<string, JsonElement>? Copy(IDictionary<string, JsonElement>? values)
    {
        if (values is null)
        {
            return null;
        }

        var snapshot = new Dictionary<string, JsonElement>(StringComparer.Ordinal);
        foreach (var (key, value) in values)
        {
            snapshot[key] = value.Clone();
        }

        return snapshot;
    }
}
