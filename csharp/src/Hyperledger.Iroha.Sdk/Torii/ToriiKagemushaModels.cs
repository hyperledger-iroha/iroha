using System.Text.Json;
using System.Text.Json.Serialization;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Torii;

/// <summary>
/// Asset-neutral first-release offline protocol capability.
/// </summary>
public sealed record class ToriiOfflineStatus
{
    [JsonPropertyName("cash_handoff_capability")]
    public string CashHandoffCapability { get; init; } = string.Empty;

    [JsonPropertyName("required_bridge_abi_version")]
    public uint RequiredBridgeAbiVersion { get; init; }

    [JsonPropertyName("max_hops")]
    public uint MaxHops { get; init; }

    [JsonPropertyName("ready")]
    public bool Ready { get; init; }
}

/// <summary>
/// A canonical externally-produced V4 top-up request archive for Torii transport.
/// </summary>
public sealed class ToriiKagemushaTopUpRequestV4
{
    private readonly byte[] norito;

    public ToriiKagemushaTopUpRequestV4(string operationId, ReadOnlySpan<byte> norito)
    {
        OperationId = ToriiKagemushaTransport.RequireOperationId(operationId, nameof(operationId));
        this.norito = ToriiKagemushaTransport.RequireNoritoArchive(
            norito,
            ToriiKagemushaTransport.MaxTopUpNoritoRequestBytes,
            ToriiKagemushaTransport.TopUpRequestSchemaName,
            nameof(norito));
    }

    public int Version => ToriiKagemushaTransport.ManifestVersion;

    public string OperationId { get; }

    public byte[] Norito => norito.ToArray();
}

/// <summary>
/// A canonical externally-produced V4 redemption request archive for Torii transport.
/// </summary>
public sealed class ToriiKagemushaRedeemRequestV4
{
    private readonly byte[] norito;

    public ToriiKagemushaRedeemRequestV4(string operationId, ReadOnlySpan<byte> norito)
    {
        OperationId = ToriiKagemushaTransport.RequireOperationId(operationId, nameof(operationId));
        this.norito = ToriiKagemushaTransport.RequireNoritoArchive(
            norito,
            ToriiKagemushaTransport.MaxRedeemNoritoRequestBytes,
            ToriiKagemushaTransport.RedeemRequestSchemaName,
            nameof(norito));
    }

    public int Version => ToriiKagemushaTransport.ManifestVersion;

    public string OperationId { get; }

    public byte[] Norito => norito.ToArray();
}

public enum ToriiKagemushaOperationKind
{
    TopUp,
    Redeem,
}

public enum ToriiKagemushaOperationState
{
    Pending,
    Applied,
    Rejected,
}

/// <summary>
/// Initial reference returned after Torii accepts an offline command.
/// </summary>
public sealed record class ToriiKagemushaOperationReference
{
    public required string OperationId { get; init; }

    public required ToriiKagemushaOperationKind Kind { get; init; }

    public ToriiKagemushaOperationState State { get; init; } = ToriiKagemushaOperationState.Pending;

    public required string TransactionHash { get; init; }

    public required string StatusUri { get; init; }

    public required ulong SubmittedAtMilliseconds { get; init; }
}

/// <summary>
/// Terminal top-up projection. Anchor and finality proof remain typed JSON
/// documents because this SDK intentionally does not ship a native prover.
/// </summary>
/// <remarks>
/// Operation-status decoding validates their portable structure and mutual
/// bindings, but does not authenticate the embedded Commit-QC signature.
/// Consumers must verify that signature against a separately trusted,
/// release-pinned validator roster before treating this evidence as consensus
/// finality.
/// </remarks>
public sealed record class ToriiKagemushaTopUpResultV4
{
    public required string TransactionHash { get; init; }

    public ulong FinalizedBlockHeight { get; init; }

    public required JsonElement Anchor { get; init; }

    public required JsonElement FinalityProof { get; init; }
}

/// <summary>
/// Terminal redemption projection.
/// </summary>
public sealed record class ToriiKagemushaRedeemResultV4
{
    public required string TransactionHash { get; init; }

    public ulong FinalizedBlockHeight { get; init; }
}

/// <summary>
/// Stable Torii error attached to a rejected operation.
/// </summary>
public sealed record class ToriiKagemushaOperationError
{
    public required string Code { get; init; }

    public required string Message { get; init; }

    public JsonElement? Details { get; init; }
}

/// <summary>
/// Pollable status returned by <c>/v1/offline/operations/{operation_id}</c>.
/// </summary>
public sealed record class ToriiKagemushaOperationStatus
{
    public required string OperationId { get; init; }

    public required ToriiKagemushaOperationState State { get; init; }

    public required ToriiKagemushaOperationKind Kind { get; init; }

    public required string TransactionHash { get; init; }

    /// <summary>
    /// The active submission time carried only by Pending responses. It repeats while their
    /// transaction hash is unchanged and may replace both values for a newer exact retry
    /// attempt. Applied and Rejected responses omit this field.
    /// </summary>
    public ulong? SubmittedAtMilliseconds { get; init; }

    public ToriiKagemushaTopUpResultV4? TopUpResult { get; init; }

    public ToriiKagemushaRedeemResultV4? RedeemResult { get; init; }

    public ToriiKagemushaOperationError? Error { get; init; }
}

internal static class ToriiKagemushaTransport
{
    internal const int BridgeAbiVersion = 23;
    internal const int ManifestVersion = 4;
    internal const int MaxHops = 8;
    internal const int MaxReadinessJsonResponseBytes = 4 * 1024;
    internal const int MaxOperationReferenceJsonResponseBytes = 4 * 1024;
    internal const int MaxOperationStatusJsonResponseBytes = 16 * 1024 * 1024;
    internal const int MaxTopUpNoritoRequestBytes = 512 * 1024;
    internal const int MaxRedeemNoritoRequestBytes = 48 * 1024 * 1024;
    internal const string TopUpRequestSchemaName = "iroha.torii.v1.offline.top_up.request";
    internal const string RedeemRequestSchemaName = "iroha.torii.v1.offline.redeem.request";

    private const int RequiredHeaderPaddingBytes = 8;

    internal static string RequireOperationId(string? value, string parameterName)
    {
        ArgumentNullException.ThrowIfNull(value, parameterName);
        if (value.Length != 64
            || value.All(static character => character == '0')
            || value.Any(static character => character is not (>= '0' and <= '9') and not (>= 'a' and <= 'f')))
        {
            throw new ArgumentException(
                "Kagemusha operation id must be non-zero lowercase 32-byte hexadecimal.",
                parameterName);
        }

        return value;
    }

    internal static byte[] RequireNoritoArchive(
        ReadOnlySpan<byte> value,
        int maximumBytes,
        string expectedSchemaName,
        string parameterName)
    {
        if (value.Length < NoritoHeader.EncodedLength || value.Length > maximumBytes)
        {
            throw new ArgumentException(
                $"Kagemusha V4 request must be a canonical Norito archive between {NoritoHeader.EncodedLength} and {maximumBytes} bytes.",
                parameterName);
        }

        byte[] payload;
        byte flags;
        try
        {
            (payload, flags) = NoritoCodec.Decode(expectedSchemaName, value);
        }
        catch (ArgumentException error)
        {
            throw new ArgumentException(
                "Kagemusha V4 request must be a schema-bound canonical Norito archive.",
                parameterName,
                error);
        }
        if (payload.Length == 0
            || flags != NoritoCodec.CanonicalLayoutFlags
            || value.Length != NoritoHeader.EncodedLength + RequiredHeaderPaddingBytes + payload.Length)
        {
            throw new ArgumentException(
                "Kagemusha V4 request must use the schema-bound compact layout with exactly eight zero padding bytes and a non-empty payload.",
                parameterName);
        }

        return value.ToArray();
    }
}
