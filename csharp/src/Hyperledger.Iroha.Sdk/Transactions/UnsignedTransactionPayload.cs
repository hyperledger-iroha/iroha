using System.Collections.ObjectModel;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Transactions;

/// <summary>
/// Exact unsigned transaction payload sent to the account-signed fee quote endpoint.
/// </summary>
public sealed class UnsignedTransactionPayload
{
    private readonly JsonObject instructions;
    private readonly ReadOnlyDictionary<string, JsonNode?> metadata;

    internal UnsignedTransactionPayload(
        string chain,
        string authority,
        ulong creationTimeMilliseconds,
        JsonObject instructions,
        ulong? timeToLiveMilliseconds,
        uint? nonce,
        FeePaymentIntent feePayment,
        IReadOnlyDictionary<string, JsonNode?> metadata)
    {
        Chain = chain;
        Authority = authority;
        CreationTimeMilliseconds = creationTimeMilliseconds;
        this.instructions = (JsonObject)instructions.DeepClone();
        TimeToLiveMilliseconds = timeToLiveMilliseconds;
        Nonce = nonce;
        FeePayment = feePayment;

        var metadataSnapshot = new Dictionary<string, JsonNode?>(StringComparer.Ordinal);
        foreach (var (key, value) in metadata)
        {
            metadataSnapshot[key] = value?.DeepClone();
        }
        this.metadata = new ReadOnlyDictionary<string, JsonNode?>(metadataSnapshot);
    }

    /// <summary>Unique chain identifier.</summary>
    [JsonPropertyName("chain")]
    public string Chain { get; }

    /// <summary>Canonical transaction authority.</summary>
    [JsonPropertyName("authority")]
    public string Authority { get; }

    /// <summary>Unix creation time in milliseconds.</summary>
    [JsonPropertyName("creation_time_ms")]
    public ulong CreationTimeMilliseconds { get; }

    /// <summary>Canonical Norito JSON executable.</summary>
    [JsonPropertyName("instructions")]
    public JsonObject Instructions => (JsonObject)instructions.DeepClone();

    /// <summary>Optional transaction time-to-live in milliseconds.</summary>
    [JsonPropertyName("time_to_live_ms")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public ulong? TimeToLiveMilliseconds { get; }

    /// <summary>Optional non-zero replay nonce.</summary>
    [JsonPropertyName("nonce")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public uint? Nonce { get; }

    /// <summary>Requested or quoted signature-bound fee payment.</summary>
    [JsonPropertyName("fee_payment")]
    public FeePaymentIntent FeePayment { get; }

    /// <summary>Exact transaction metadata.</summary>
    [JsonPropertyName("metadata")]
    public IReadOnlyDictionary<string, JsonNode?> Metadata => metadata;
}
