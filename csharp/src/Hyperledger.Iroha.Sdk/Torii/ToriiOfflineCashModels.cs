using System.Text.Json;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

/// <summary>Asset-neutral first-release Offline Cash capability.</summary>
public sealed record class ToriiOfflineStatus
{
    [JsonPropertyName("cash_handoff_capability")]
    public string CashHandoffCapability { get; init; } = string.Empty;

    [JsonPropertyName("wire_version")]
    public uint WireVersion { get; init; }

    [JsonPropertyName("device_lifecycle_version")]
    public uint DeviceLifecycleVersion { get; init; }

    [JsonPropertyName("ready")]
    public bool Ready { get; init; }
}

/// <summary>Stable terminal Offline Cash V1 rejection metadata.</summary>
public sealed record OfflineCashOperationRejectionV1(string Code, ReadOnlyMemory<byte> DetailDigest);

/// <summary>
/// Structurally validated operation metadata that deliberately withholds any applied monetary
/// result until a caller-pinned finality verifier authenticates the complete response.
/// </summary>
public sealed class UnverifiedOfflineCashOperationStatusV1
{
    private readonly byte[] operationId;
    private readonly JsonElement source;

    internal UnverifiedOfflineCashOperationStatusV1(
        byte[] operationId,
        string kind,
        string state,
        OfflineCashOperationRejectionV1? rejection,
        JsonElement source)
    {
        this.operationId = operationId.ToArray();
        Kind = kind;
        State = state;
        Rejection = rejection;
        this.source = source.Clone();
    }

    public ReadOnlyMemory<byte> OperationId => operationId.ToArray();
    public string Kind { get; }
    public string State { get; }
    public OfflineCashOperationRejectionV1? Rejection { get; }

    /// <summary>Release the complete response only through an independently pinned verifier.</summary>
    public TResult VerifyAgainst<TAnchor, TResult>(
        TAnchor trustAnchor,
        Func<JsonElement, TAnchor, TResult> verifier)
        where TAnchor : notnull
    {
        ArgumentNullException.ThrowIfNull(trustAnchor);
        ArgumentNullException.ThrowIfNull(verifier);
        return verifier(source.Clone(), trustAnchor);
    }
}
