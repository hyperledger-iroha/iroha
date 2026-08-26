using System.Collections.ObjectModel;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Transactions;

/// <summary>
/// Exact unsigned transaction payload sent to the account-signed fee quote endpoint.
/// </summary>
public sealed class UnsignedTransactionPayload
{
    private readonly JsonObject executable;
    private readonly ReadOnlyDictionary<string, JsonNode?> metadata;

    internal UnsignedTransactionPayload(
        NetworkId networkId,
        string authority,
        ulong creationTimeMilliseconds,
        JsonObject executable,
        ulong timeToLiveMilliseconds,
        uint? nonce,
        FeePaymentIntent feePayment,
        TransactionAdmissionIntent admissionIntent,
        IReadOnlyDictionary<string, JsonNode?> metadata)
    {
        Domain = new TransactionNetworkDomain(
            networkId ?? throw new ArgumentNullException(nameof(networkId)));
        Authority = authority;
        CreationTimeMilliseconds = creationTimeMilliseconds;
        this.executable = (JsonObject)executable.DeepClone();
        TimeToLiveMilliseconds = timeToLiveMilliseconds;
        Nonce = nonce;
        FeePayment = feePayment;
        AdmissionIntent = admissionIntent;

        var metadataSnapshot = new Dictionary<string, JsonNode?>(StringComparer.Ordinal);
        foreach (var (key, value) in metadata)
        {
            metadataSnapshot[key] = value?.DeepClone();
        }
        this.metadata = new ReadOnlyDictionary<string, JsonNode?>(metadataSnapshot);
    }

    /// <summary>Exact signed replay-protection domain.</summary>
    [JsonPropertyName("domain")]
    public TransactionNetworkDomain Domain { get; }

    /// <summary>Canonical transaction authority.</summary>
    [JsonPropertyName("authority")]
    public string Authority { get; }

    /// <summary>Unix creation time in milliseconds.</summary>
    [JsonPropertyName("creation_time_ms")]
    public ulong CreationTimeMilliseconds { get; }

    /// <summary>Canonical Norito JSON executable.</summary>
    [JsonPropertyName("instructions")]
    public JsonObject Executable => (JsonObject)executable.DeepClone();

    /// <summary>Required positive signature-bound transaction lifetime in milliseconds.</summary>
    [JsonPropertyName("time_to_live_ms")]
    public ulong TimeToLiveMilliseconds { get; }

    /// <summary>Optional non-zero replay nonce.</summary>
    [JsonPropertyName("nonce")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public uint? Nonce { get; }

    /// <summary>Requested or quoted signature-bound fee payment.</summary>
    [JsonPropertyName("fee_payment")]
    public FeePaymentIntent FeePayment { get; }

    /// <summary>Required signature-bound admission protocol.</summary>
    [JsonPropertyName("admission_intent")]
    public TransactionAdmissionIntent AdmissionIntent { get; }

    /// <summary>Exact transaction metadata.</summary>
    [JsonPropertyName("metadata")]
    public IReadOnlyDictionary<string, JsonNode?> Metadata => metadata;
}

/// <summary>Canonical JSON representation of <c>TransactionDomain::Network</c>.</summary>
public sealed class TransactionNetworkDomain
{
    internal TransactionNetworkDomain(NetworkId networkId)
    {
        Value = networkId.ToNoritoJsonLiteral();
    }

    [JsonPropertyName("kind")]
    public string Kind => "network";

    [JsonPropertyName("value")]
    public string Value { get; }
}
