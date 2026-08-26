using System.Buffers.Binary;
using System.Text.Json;
using System.Text.Json.Serialization;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Torii;

/// <summary>
/// One canonical reason why Offline Cash V1 is not production-ready.
/// </summary>
public sealed record class ToriiKagemushaReadinessBlocker
{
    [JsonPropertyName("code")]
    public string Code { get; init; } = string.Empty;

    [JsonPropertyName("message")]
    public string Message { get; init; } = string.Empty;
}

/// <summary>
/// Universally compiled, asset-neutral Offline Cash V1 capability projection.
/// </summary>
public sealed record class ToriiOfflineStatus
{
    [JsonPropertyName("mandatory")]
    public bool Mandatory { get; init; }

    [JsonPropertyName("cash_handoff_capability")]
    public string CashHandoffCapability { get; init; } = string.Empty;

    [JsonPropertyName("required_bridge_abi_version")]
    public uint RequiredBridgeAbiVersion { get; init; }

    [JsonPropertyName("max_hops")]
    public uint MaxHops { get; init; }

    [JsonPropertyName("ready")]
    public bool Ready { get; init; }

    [JsonPropertyName("assets")]
    public JsonElement[] Assets { get; init; } = [];

    [JsonPropertyName("blockers")]
    public ToriiKagemushaReadinessBlocker[] Blockers { get; init; } = [];
}

/// <summary>
/// A canonical externally-produced V4 top-up request archive for Torii transport.
/// </summary>
public sealed class ToriiKagemushaTopUpRequestV4
{
    private readonly byte[] norito;

    public ToriiKagemushaTopUpRequestV4(ReadOnlySpan<byte> norito)
    {
        var validated = ToriiKagemushaTransport.RequireNoritoRequestArchive(
            norito,
            ToriiKagemushaTransport.TopUpRequestSchemaName,
            fieldCount: 8,
            operationIdFieldIndex: 6,
            ToriiKagemushaTransport.MaxTopUpNoritoRequestBytes,
            nameof(norito));
        OperationId = validated.OperationId;
        IssuedAtMilliseconds = validated.IssuedAtMilliseconds;
        NetworkId = validated.NetworkId;
        this.norito = validated.Archive;
    }

    public ToriiKagemushaTopUpRequestV4(string operationId, ReadOnlySpan<byte> norito)
        : this(norito)
    {
        var exactOperationId = ToriiKagemushaTransport.RequireOperationId(
            operationId,
            nameof(operationId));
        if (!string.Equals(exactOperationId, OperationId, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "Kagemusha operation id must match the signed Norito request body.",
                nameof(operationId));
        }
    }

    public int Version => ToriiKagemushaTransport.ManifestVersion;

    public string OperationId { get; }

    public ulong IssuedAtMilliseconds { get; }

    public NetworkId NetworkId { get; }

    public byte[] Norito => norito.ToArray();
}

/// <summary>
/// A canonical externally-produced V4 redemption request archive for Torii transport.
/// </summary>
public sealed class ToriiKagemushaRedeemRequestV4
{
    private readonly byte[] norito;

    public ToriiKagemushaRedeemRequestV4(ReadOnlySpan<byte> norito)
    {
        var validated = ToriiKagemushaTransport.RequireNoritoRequestArchive(
            norito,
            ToriiKagemushaTransport.RedeemRequestSchemaName,
            fieldCount: 10,
            operationIdFieldIndex: 8,
            ToriiKagemushaTransport.MaxRedeemNoritoRequestBytes,
            nameof(norito));
        OperationId = validated.OperationId;
        IssuedAtMilliseconds = validated.IssuedAtMilliseconds;
        NetworkId = validated.NetworkId;
        this.norito = validated.Archive;
    }

    public ToriiKagemushaRedeemRequestV4(string operationId, ReadOnlySpan<byte> norito)
        : this(norito)
    {
        var exactOperationId = ToriiKagemushaTransport.RequireOperationId(
            operationId,
            nameof(operationId));
        if (!string.Equals(exactOperationId, OperationId, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "Kagemusha operation id must match the signed Norito request body.",
                nameof(operationId));
        }
    }

    public int Version => ToriiKagemushaTransport.ManifestVersion;

    public string OperationId { get; }

    public ulong IssuedAtMilliseconds { get; }

    public NetworkId NetworkId { get; }

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

    public ulong SubmittedAtMilliseconds { get; init; }
}

/// <summary>
/// Terminal top-up projection. Anchor and finality proof remain typed JSON
/// documents because this SDK intentionally does not ship a native prover.
/// </summary>
public sealed record class ToriiKagemushaTopUpResultV4
{
    public required string TransactionHash { get; init; }

    public ulong FinalizedBlockHeight { get; init; }

    public ulong ServerTimeMilliseconds { get; init; }

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

    public ulong ServerTimeMilliseconds { get; init; }
}

/// <summary>
/// Stable Torii error attached to a rejected operation.
/// </summary>
public sealed record class ToriiKagemushaOperationError
{
    public required string Code { get; init; }

    public required string Message { get; init; }
}

/// <summary>
/// Pollable status returned by <c>/v1/offline/operations/{operation_id}</c>.
/// </summary>
public sealed record class ToriiKagemushaOperationStatus
{
    public required string OperationId { get; init; }

    public required ToriiKagemushaOperationState State { get; init; }

    public ToriiKagemushaOperationKind? Kind { get; init; }

    public string? TransactionHash { get; init; }

    public ulong? SubmittedAtMilliseconds { get; init; }

    public ToriiKagemushaTopUpResultV4? TopUpResult { get; init; }

    public ToriiKagemushaRedeemResultV4? RedeemResult { get; init; }

    public ToriiKagemushaOperationError? Error { get; init; }
}

internal static class ToriiKagemushaTransport
{
    internal const int BridgeAbiVersion = 22;
    internal const int ManifestVersion = 4;
    internal const int MaxHops = 8;
    internal const int MaxTopUpNoritoRequestBytes = 512 * 1024;
    internal const int MaxRedeemNoritoRequestBytes = 48 * 1024 * 1024;
    internal const string TopUpRequestSchemaName = "iroha.torii.v1.offline.top_up.request";
    internal const string RedeemRequestSchemaName = "iroha.torii.v1.offline.redeem.request";

    private const int RequestHeaderPaddingBytes = 8;
    private const int AuthorizationFieldCount = 10;
    private const int AuthorizationOperationIdFieldIndex = 3;
    private const int AuthorizationIssuedAtFieldIndex = 4;
    private const int TopUpCurrentNoteFieldIndex = 3;
    private const int CurrentNoteFieldCount = 5;
    private const int RedeemBundleFieldIndex = 1;
    private const int RecursiveBundleFieldCount = 3;
    private const int RecursiveStatementFieldIndex = 0;
    private const int RecursiveStatementFieldCount = 13;

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

    internal static ValidatedRequestArchive RequireNoritoRequestArchive(
        ReadOnlySpan<byte> value,
        string schemaName,
        int fieldCount,
        int operationIdFieldIndex,
        int maximumBytes,
        string parameterName)
    {
        if (value.Length <= NoritoHeader.EncodedLength + RequestHeaderPaddingBytes
            || value.Length > maximumBytes)
        {
            throw new ArgumentException(
                $"Kagemusha V4 request must be a canonical Norito archive no larger than {maximumBytes} bytes.",
                parameterName);
        }

        byte[] payload;
        byte flags;
        try
        {
            (payload, flags) = NoritoCodec.Decode(schemaName, value);
        }
        catch (ArgumentException exception)
        {
            throw new ArgumentException(
                "Kagemusha V4 request must use its exact canonical Norito schema and framing.",
                parameterName,
                exception);
        }

        var paddingLength = value.Length - NoritoHeader.EncodedLength - payload.Length;
        if (flags != NoritoCodec.CanonicalLayoutFlags
            || paddingLength != RequestHeaderPaddingBytes)
        {
            throw new ArgumentException(
                "Kagemusha V4 request must use compact canonical Norito framing and exact alignment.",
                parameterName);
        }

        var reader = new CanonicalNoritoReader(
            payload,
            "Kagemusha V4 request",
            parameterName);
        var fields = new byte[fieldCount][];
        for (var index = 0; index < fields.Length; index++)
        {
            fields[index] = reader.ReadField($"fields[{index}]").ToArray();
        }
        reader.RequireEnd();

        if (fields[0].Length != sizeof(ushort)
            || BinaryPrimitives.ReadUInt16LittleEndian(fields[0]) != ManifestVersion)
        {
            throw new ArgumentException(
                $"Kagemusha request version must be exactly {ManifestVersion}.",
                parameterName);
        }

        var operationIdBytes = RequireOperationIdBytes(
            fields[operationIdFieldIndex],
            "request operation id",
            parameterName);
        var authorization = new CanonicalNoritoReader(
            fields[^1],
            "Kagemusha request authorization",
            parameterName);
        var authorizationFields = new byte[AuthorizationFieldCount][];
        for (var index = 0; index < authorizationFields.Length; index++)
        {
            authorizationFields[index] = authorization.ReadField($"fields[{index}]").ToArray();
        }
        authorization.RequireEnd();

        var authorizationOperationId = RequireOperationIdBytes(
            authorizationFields[AuthorizationOperationIdFieldIndex],
            "authorization operation id",
            parameterName);
        if (!authorizationOperationId.AsSpan().SequenceEqual(operationIdBytes))
        {
            throw new ArgumentException(
                "Kagemusha request and authorization operation ids must match exactly.",
                parameterName);
        }

        var issuedAtBytes = authorizationFields[AuthorizationIssuedAtFieldIndex];
        if (issuedAtBytes.Length != sizeof(ulong))
        {
            throw new ArgumentException(
                "Kagemusha authorization issued_at_ms must be an exact UInt64.",
                parameterName);
        }
        var issuedAtMilliseconds = BinaryPrimitives.ReadUInt64LittleEndian(issuedAtBytes);
        if (issuedAtMilliseconds == 0)
        {
            throw new ArgumentException(
                "Kagemusha authorization issued_at_ms must be at least 1.",
                parameterName);
        }

        var networkId = RequireRequestNetworkId(
            fields,
            schemaName,
            parameterName);

        return new ValidatedRequestArchive(
            value.ToArray(),
            Convert.ToHexString(operationIdBytes).ToLowerInvariant(),
            issuedAtMilliseconds,
            networkId);
    }

    private static NetworkId RequireRequestNetworkId(
        byte[][] fields,
        string schemaName,
        string parameterName)
    {
        byte[] networkIdBytes;
        if (string.Equals(schemaName, TopUpRequestSchemaName, StringComparison.Ordinal))
        {
            var currentNoteFields = ReadStructFields(
                fields[TopUpCurrentNoteFieldIndex],
                CurrentNoteFieldCount,
                "Kagemusha top-up current note",
                parameterName);
            networkIdBytes = currentNoteFields[0];
        }
        else if (string.Equals(schemaName, RedeemRequestSchemaName, StringComparison.Ordinal))
        {
            var bundleFields = ReadStructFields(
                fields[RedeemBundleFieldIndex],
                RecursiveBundleFieldCount,
                "Kagemusha redemption bundle",
                parameterName);
            var statementFields = ReadStructFields(
                bundleFields[RecursiveStatementFieldIndex],
                RecursiveStatementFieldCount,
                "Kagemusha redemption statement",
                parameterName);
            networkIdBytes = statementFields[0];
        }
        else
        {
            throw new ArgumentException(
                "Kagemusha request uses an unsupported schema.",
                parameterName);
        }

        if (networkIdBytes.Length != NetworkId.ByteLength)
        {
            throw new ArgumentException(
                "Kagemusha signed request NetworkId must contain exactly 32 bytes.",
                parameterName);
        }
        try
        {
            return NetworkId.Parse(Convert.ToHexString(networkIdBytes).ToLowerInvariant());
        }
        catch (FormatException error)
        {
            throw new ArgumentException(
                "Kagemusha signed request NetworkId must set the Iroha hash marker bit.",
                parameterName,
                error);
        }
    }

    private static byte[][] ReadStructFields(
        ReadOnlySpan<byte> value,
        int fieldCount,
        string context,
        string parameterName)
    {
        var reader = new CanonicalNoritoReader(value, context, parameterName);
        var fields = new byte[fieldCount][];
        for (var index = 0; index < fields.Length; index++)
        {
            fields[index] = reader.ReadField($"fields[{index}]").ToArray();
        }
        reader.RequireEnd();
        return fields;
    }

    private static byte[] RequireOperationIdBytes(
        byte[] value,
        string field,
        string parameterName)
    {
        if (value.Length != 32 || value.All(static item => item == 0))
        {
            throw new ArgumentException(
                $"Kagemusha {field} must be exactly 32 non-zero bytes.",
                parameterName);
        }
        return value;
    }

    internal sealed record ValidatedRequestArchive(
        byte[] Archive,
        string OperationId,
        ulong IssuedAtMilliseconds,
        NetworkId NetworkId);
}
