using System.Text.Json;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

[JsonConverter(typeof(ToriiProofEventJsonConverter))]
public sealed class ToriiProofEvent
{
    private string category = string.Empty;
    private string eventName = string.Empty;
    private string? backend;
    private string? proofHash;
    private string? callHash;
    private string? envelopeHash;
    private string? verificationKeyReference;
    private string? verificationKeyCommitment;
    private string? prunedBy;
    private string? origin;
    private List<ToriiProofRemovedRecord>? removed;
    private string? lastEventId;
    private string? sseEventName;
    private int? retryMilliseconds;
    private Dictionary<string, JsonElement>? additionalProperties;

    [JsonPropertyName("category")]
    public string Category
    {
        get => category;
        set => category = ToriiSseDirectMetadata.RequireExactValue(value, "Data", nameof(Category));
    }

    [JsonPropertyName("event")]
    public string Event
    {
        get => eventName;
        set => eventName = ToriiSseDirectMetadata.RequireProofEventName(value, nameof(Event));
    }

    [JsonPropertyName("backend")]
    public string? Backend
    {
        get => backend;
        set => backend = ToriiSseDirectMetadata.RequireOptionalExactTokenText(value, nameof(Backend));
    }

    [JsonPropertyName("proof_hash")]
    public string? ProofHash
    {
        get => proofHash;
        set => proofHash = ToriiSseDirectMetadata.RequireOptionalExactSizedHex(value, nameof(ProofHash), 32);
    }

    [JsonPropertyName("call_hash")]
    public string? CallHash
    {
        get => callHash;
        set => callHash = ToriiSseDirectMetadata.RequireOptionalExactSizedHex(value, nameof(CallHash), 32);
    }

    [JsonPropertyName("envelope_hash")]
    public string? EnvelopeHash
    {
        get => envelopeHash;
        set => envelopeHash = ToriiSseDirectMetadata.RequireOptionalExactSizedHex(value, nameof(EnvelopeHash), 32);
    }

    [JsonPropertyName("vk_ref")]
    public string? VerificationKeyReference
    {
        get => verificationKeyReference;
        set => verificationKeyReference = ToriiSseDirectMetadata.RequireOptionalExactTokenText(
            value,
            nameof(VerificationKeyReference));
    }

    [JsonPropertyName("vk_commitment")]
    public string? VerificationKeyCommitment
    {
        get => verificationKeyCommitment;
        set => verificationKeyCommitment = ToriiSseDirectMetadata.RequireOptionalExactSizedHex(
            value,
            nameof(VerificationKeyCommitment),
            32);
    }

    [JsonPropertyName("removed_count")]
    public ulong? RemovedCount { get; set; }

    [JsonPropertyName("remaining")]
    public ulong? Remaining { get; set; }

    [JsonPropertyName("cap")]
    public ulong? Cap { get; set; }

    [JsonPropertyName("grace_blocks")]
    public ulong? GraceBlocks { get; set; }

    [JsonPropertyName("prune_batch")]
    public ulong? PruneBatch { get; set; }

    [JsonPropertyName("pruned_at_height")]
    public ulong? PrunedAtHeight { get; set; }

    [JsonPropertyName("pruned_by")]
    public string? PrunedBy
    {
        get => prunedBy;
        set => prunedBy = ToriiSseDirectMetadata.RequireOptionalExactTokenText(value, nameof(PrunedBy));
    }

    [JsonPropertyName("origin")]
    public string? Origin
    {
        get => origin;
        set => origin = ToriiSseDirectMetadata.RequireOptionalExactTokenText(value, nameof(Origin));
    }

    [JsonPropertyName("removed")]
    public List<ToriiProofRemovedRecord>? Removed
    {
        get => CopyRemovedRecords(removed);
        set => removed = CopyRemovedRecords(value);
    }

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

    private static List<ToriiProofRemovedRecord>? CopyRemovedRecords(
        IReadOnlyList<ToriiProofRemovedRecord>? values)
    {
        if (values is null)
        {
            return null;
        }

        var snapshot = new List<ToriiProofRemovedRecord>(values.Count);
        foreach (var value in values)
        {
            if (value is null)
            {
                throw new ArgumentException("Removed records must not contain null entries.", nameof(values));
            }

            snapshot.Add(new ToriiProofRemovedRecord
            {
                Backend = value.Backend,
                ProofHash = value.ProofHash,
            });
        }

        return snapshot;
    }
}

[JsonConverter(typeof(ToriiProofRemovedRecordJsonConverter))]
public sealed class ToriiProofRemovedRecord
{
    private string backend = string.Empty;
    private string proofHash = string.Empty;

    [JsonPropertyName("backend")]
    public string Backend
    {
        get => backend;
        set => backend = ToriiSseDirectMetadata.RequireExactTokenText(value, nameof(Backend));
    }

    [JsonPropertyName("proof_hash")]
    public string ProofHash
    {
        get => proofHash;
        set => proofHash = ToriiSseDirectMetadata.RequireExactSizedHex(value, nameof(ProofHash), 32);
    }
}
