using System.Text.Json;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

public sealed class ToriiProofEvent
{
    [JsonPropertyName("category")]
    public string Category { get; set; } = string.Empty;

    [JsonPropertyName("event")]
    public string Event { get; set; } = string.Empty;

    [JsonPropertyName("backend")]
    public string? Backend { get; set; }

    [JsonPropertyName("proof_hash")]
    public string? ProofHash { get; set; }

    [JsonPropertyName("call_hash")]
    public string? CallHash { get; set; }

    [JsonPropertyName("envelope_hash")]
    public string? EnvelopeHash { get; set; }

    [JsonPropertyName("vk_ref")]
    public string? VerificationKeyReference { get; set; }

    [JsonPropertyName("vk_commitment")]
    public string? VerificationKeyCommitment { get; set; }

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
    public string? PrunedBy { get; set; }

    [JsonPropertyName("origin")]
    public string? Origin { get; set; }

    [JsonPropertyName("removed")]
    public List<ToriiProofRemovedRecord>? Removed { get; set; }

    [JsonIgnore]
    public string? LastEventId { get; set; }

    [JsonIgnore]
    public string? SseEventName { get; set; }

    [JsonIgnore]
    public int? RetryMilliseconds { get; set; }

    [JsonExtensionData]
    public Dictionary<string, JsonElement>? AdditionalProperties { get; set; }
}

public sealed class ToriiProofRemovedRecord
{
    [JsonPropertyName("backend")]
    public string Backend { get; set; } = string.Empty;

    [JsonPropertyName("proof_hash")]
    public string ProofHash { get; set; } = string.Empty;
}
