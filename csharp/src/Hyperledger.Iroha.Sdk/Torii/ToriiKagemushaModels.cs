using System.Buffers.Binary;
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
/// A canonical externally-produced ABI-23/V4 top-up request archive for Torii transport.
/// </summary>
public sealed class ToriiKagemushaTopUpRequestV4
{
    private readonly byte[] norito;

    public ToriiKagemushaTopUpRequestV4(ReadOnlySpan<byte> norito)
    {
        (this.norito, Identity) =
            ToriiKagemushaTransport.RequireNoritoRequestArchive(
                norito,
                ToriiKagemushaTransport.MaxTopUpNoritoRequestBytes,
                ToriiKagemushaTransport.TopUpRequestSchemaName,
                fieldCount: 8,
                operationIdFieldIndex: 6,
                nameof(norito));
    }

    public int Version => ToriiKagemushaTransport.ManifestVersion;

    public ToriiKagemushaOperationIdentity Identity { get; }

    public byte[] Norito => norito.ToArray();
}

/// <summary>
/// A canonical externally-produced ABI-23/V4 redemption request archive for Torii transport.
/// </summary>
public sealed class ToriiKagemushaRedeemRequestV4
{
    private readonly byte[] norito;

    public ToriiKagemushaRedeemRequestV4(ReadOnlySpan<byte> norito)
    {
        (this.norito, Identity) =
            ToriiKagemushaTransport.RequireNoritoRequestArchive(
                norito,
                ToriiKagemushaTransport.MaxRedeemNoritoRequestBytes,
                ToriiKagemushaTransport.RedeemRequestSchemaName,
                fieldCount: 10,
                operationIdFieldIndex: 8,
                nameof(norito));
    }

    public int Version => ToriiKagemushaTransport.ManifestVersion;

    public ToriiKagemushaOperationIdentity Identity { get; }

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
/// Complete immutable identity of one authorized Kagemusha operation.
/// </summary>
public sealed class ToriiKagemushaOperationIdentity
    : IEquatable<ToriiKagemushaOperationIdentity>
{
    public ToriiKagemushaOperationIdentity(
        string operationId,
        string requestAuthorityDigest,
        string canonicalRequestDigest,
        ToriiKagemushaOperationKind kind,
        ulong issuedAtMilliseconds,
        ulong expiresAtMilliseconds)
    {
        OperationId = ToriiKagemushaTransport.RequireOperationId(
            operationId,
            nameof(operationId));
        RequestAuthorityDigest = ToriiKagemushaTransport.RequireMarkedHash(
            requestAuthorityDigest,
            nameof(requestAuthorityDigest));
        CanonicalRequestDigest = ToriiKagemushaTransport.RequireMarkedHash(
            canonicalRequestDigest,
            nameof(canonicalRequestDigest));
        if (kind is not (ToriiKagemushaOperationKind.TopUp
                or ToriiKagemushaOperationKind.Redeem))
        {
            throw new ArgumentOutOfRangeException(nameof(kind));
        }
        if (issuedAtMilliseconds == 0
            || expiresAtMilliseconds <= issuedAtMilliseconds
            || expiresAtMilliseconds - issuedAtMilliseconds
                > ToriiKagemushaTransport.MaxAuthorizationLifetimeMilliseconds)
        {
            throw new ArgumentOutOfRangeException(
                nameof(expiresAtMilliseconds),
                "Kagemusha operation expiry must follow issue time by at most 300000ms.");
        }
        Kind = kind;
        IssuedAtMilliseconds = issuedAtMilliseconds;
        ExpiresAtMilliseconds = expiresAtMilliseconds;
    }

    public string OperationId { get; }

    public string RequestAuthorityDigest { get; }

    public string CanonicalRequestDigest { get; }

    public ToriiKagemushaOperationKind Kind { get; }

    public ulong IssuedAtMilliseconds { get; }

    public ulong ExpiresAtMilliseconds { get; }

    public bool Equals(ToriiKagemushaOperationIdentity? other) =>
        other is not null
        && string.Equals(OperationId, other.OperationId, StringComparison.Ordinal)
        && string.Equals(
            RequestAuthorityDigest,
            other.RequestAuthorityDigest,
            StringComparison.Ordinal)
        && string.Equals(
            CanonicalRequestDigest,
            other.CanonicalRequestDigest,
            StringComparison.Ordinal)
        && Kind == other.Kind
        && IssuedAtMilliseconds == other.IssuedAtMilliseconds
        && ExpiresAtMilliseconds == other.ExpiresAtMilliseconds;

    public override bool Equals(object? obj) =>
        obj is ToriiKagemushaOperationIdentity other && Equals(other);

    public override int GetHashCode() => HashCode.Combine(
        OperationId,
        RequestAuthorityDigest,
        CanonicalRequestDigest,
        Kind,
        IssuedAtMilliseconds,
        ExpiresAtMilliseconds);
}

/// <summary>
/// Initial reference returned after Torii accepts an offline command.
/// </summary>
public sealed record class ToriiKagemushaOperationReference
{
    public required ToriiKagemushaOperationIdentity Identity { get; init; }

    public ToriiKagemushaOperationState State { get; init; } = ToriiKagemushaOperationState.Pending;

    public required string TransactionHash { get; init; }

    public required string StatusUri { get; init; }
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
    public required ToriiKagemushaOperationIdentity Identity { get; init; }

    public required ToriiKagemushaOperationState State { get; init; }

    public required string TransactionHash { get; init; }

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
    internal const ulong MaxAuthorizationLifetimeMilliseconds = 300_000;
    internal const string TopUpRequestSchemaName = "iroha.torii.v1.offline.top_up.request";
    internal const string RedeemRequestSchemaName = "iroha.torii.v1.offline.redeem.request";

    private const int RequiredHeaderPaddingBytes = 8;
    private const ushort RequestWireVersion = 4;
    private const int RequestAuthorizationFieldCount = 10;
    private const int RequestAuthorizationOperationIdFieldIndex = 3;
    private const int RequestAuthorizationIssuedAtFieldIndex = 4;
    private const int RequestAuthorizationExpiresAtFieldIndex = 5;
    private const int RequestAuthorizationNonceFieldIndex = 6;
    private const string AccountIdSchemaName = "iroha_data_model::account::model::AccountId";

    internal static string RequireOperationId(string? value, string parameterName)
    {
        ArgumentNullException.ThrowIfNull(value, parameterName);
        if (value.Length != 64
            || value.Any(static character => character is not (>= '0' and <= '9') and not (>= 'a' and <= 'f'))
            || "13579bdf".IndexOf(value[^1]) < 0)
        {
            throw new ArgumentException(
                "Kagemusha operation id must be canonical marker-bearing lowercase 32-byte hexadecimal.",
                parameterName);
        }

        return value;
    }

    internal static string RequireMarkedHash(string? value, string parameterName)
    {
        ArgumentNullException.ThrowIfNull(value, parameterName);
        if (value.Length != 64
            || value.Any(static character => character is not (>= '0' and <= '9') and not (>= 'a' and <= 'f'))
            || "13579bdf".IndexOf(value[^1]) < 0)
        {
            throw new ArgumentException(
                "Kagemusha digest must be canonical marker-bearing lowercase 32-byte hexadecimal.",
                parameterName);
        }
        return value;
    }

    internal static (byte[] Archive, ToriiKagemushaOperationIdentity Identity)
        RequireNoritoRequestArchive(
        ReadOnlySpan<byte> value,
        int maximumBytes,
        string expectedSchemaName,
        int fieldCount,
        int operationIdFieldIndex,
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

        if (fieldCount <= 1
            || operationIdFieldIndex < 0
            || operationIdFieldIndex >= fieldCount - 1)
        {
            throw new InvalidOperationException("Kagemusha request field layout is invalid.");
        }

        var reader = new CanonicalNoritoReader(payload, "Kagemusha V4 request", parameterName);
        ReadOnlySpan<byte> operationId = default;
        ReadOnlySpan<byte> authorization = default;
        for (var index = 0; index < fieldCount; index++)
        {
            var field = reader.ReadField($"field[{index}]");
            if (index == 0)
            {
                if (field.Length != sizeof(ushort)
                    || BinaryPrimitives.ReadUInt16LittleEndian(field) != RequestWireVersion)
                {
                    throw new ArgumentException(
                        "Kagemusha request must use first-release wire version 4.",
                        parameterName);
                }
            }
            if (index == operationIdFieldIndex)
            {
                operationId = field;
            }
            if (index == fieldCount - 1)
            {
                authorization = field;
            }
        }
        reader.RequireEnd();
        if (operationId.Length != 32 || operationId.IndexOfAnyExcept((byte)0) < 0)
        {
            throw new ArgumentException(
                "Kagemusha request operation id must be a non-zero 32-byte value.",
                parameterName);
        }

        var authorizationReader = new CanonicalNoritoReader(
            authorization,
            "Kagemusha V4 request authorization",
            parameterName);
        ReadOnlySpan<byte> authorizationOperationId = default;
        ReadOnlySpan<byte> authorityPayload = default;
        ReadOnlySpan<byte> issuedAtField = default;
        ReadOnlySpan<byte> expiresAtField = default;
        ReadOnlySpan<byte> nonce = default;
        for (var index = 0; index < RequestAuthorizationFieldCount; index++)
        {
            var field = authorizationReader.ReadField($"field[{index}]");
            if (index == 0)
            {
                authorityPayload = field;
            }
            if (index == RequestAuthorizationOperationIdFieldIndex)
            {
                authorizationOperationId = field;
            }
            if (index == RequestAuthorizationIssuedAtFieldIndex)
            {
                issuedAtField = field;
            }
            if (index == RequestAuthorizationExpiresAtFieldIndex)
            {
                expiresAtField = field;
            }
            if (index == RequestAuthorizationNonceFieldIndex)
            {
                nonce = field;
            }
        }
        authorizationReader.RequireEnd();
        if (authorityPayload.IsEmpty)
        {
            throw new ArgumentException(
                "Kagemusha request authorization authority must not be empty.",
                parameterName);
        }
        if (nonce.Length != IrohaHash.Length || nonce.IndexOfAnyExcept((byte)0) < 0)
        {
            throw new ArgumentException(
                "Kagemusha request authorization nonce must be exactly 32 non-zero bytes.",
                parameterName);
        }
        if (issuedAtField.Length != sizeof(ulong) || expiresAtField.Length != sizeof(ulong))
        {
            throw new ArgumentException(
                "Kagemusha request authorization times must be exactly eight little-endian bytes.",
                parameterName);
        }
        var issuedAtMilliseconds = BinaryPrimitives.ReadUInt64LittleEndian(issuedAtField);
        var expiresAtMilliseconds = BinaryPrimitives.ReadUInt64LittleEndian(expiresAtField);
        if (issuedAtMilliseconds == 0
            || expiresAtMilliseconds <= issuedAtMilliseconds
            || expiresAtMilliseconds - issuedAtMilliseconds
                > MaxAuthorizationLifetimeMilliseconds)
        {
            throw new ArgumentException(
                "Kagemusha request authorization expiry must follow issue time by at most 300000ms.",
                parameterName);
        }

        var accountArchive = NoritoCodec.Encode(
            AccountIdSchemaName,
            authorityPayload,
            NoritoCodec.CanonicalLayoutFlags);
        var (accountPayloadCheck, accountFlags) = NoritoCodec.Decode(
            AccountIdSchemaName,
            accountArchive);
        if (accountFlags != NoritoCodec.CanonicalLayoutFlags
            || !accountPayloadCheck.AsSpan().SequenceEqual(authorityPayload))
        {
            throw new InvalidOperationException("Canonical AccountId framing was not self-consistent.");
        }

        var derivedOperationId = HashAccountIdentity(
            "iroha:offline:kagemusha:operation-id:v4\0"u8,
            accountArchive,
            nonce);
        if (!authorizationOperationId.SequenceEqual(operationId)
            || !derivedOperationId.AsSpan().SequenceEqual(operationId))
        {
            throw new ArgumentException(
                "Kagemusha request operation id must equal the canonical authority-and-nonce derivation.",
                parameterName);
        }

        var requestAuthorityDigest = HashAccountIdentity(
            "iroha:offline:kagemusha:operation-outcome-authority:v4\0"u8,
            accountArchive,
            ReadOnlySpan<byte>.Empty);
        var kind = string.Equals(expectedSchemaName, TopUpRequestSchemaName, StringComparison.Ordinal)
            ? ToriiKagemushaOperationKind.TopUp
            : ToriiKagemushaOperationKind.Redeem;
        var kindTag = kind == ToriiKagemushaOperationKind.TopUp ? "top_up"u8 : "redeem"u8;
        var canonicalRequestDigest = HashCanonicalRequest(kindTag, value);
        var identity = new ToriiKagemushaOperationIdentity(
            Convert.ToHexString(derivedOperationId).ToLowerInvariant(),
            Convert.ToHexString(requestAuthorityDigest).ToLowerInvariant(),
            Convert.ToHexString(canonicalRequestDigest).ToLowerInvariant(),
            kind,
            issuedAtMilliseconds,
            expiresAtMilliseconds);

        return (
            value.ToArray(),
            identity);
    }

    private static byte[] HashAccountIdentity(
        ReadOnlySpan<byte> domain,
        ReadOnlySpan<byte> accountArchive,
        ReadOnlySpan<byte> suffix)
    {
        var preimage = new byte[checked(
            domain.Length + sizeof(ulong) + accountArchive.Length + suffix.Length)];
        var offset = 0;
        domain.CopyTo(preimage.AsSpan(offset));
        offset += domain.Length;
        BinaryPrimitives.WriteUInt64LittleEndian(
            preimage.AsSpan(offset, sizeof(ulong)),
            checked((ulong)accountArchive.Length));
        offset += sizeof(ulong);
        accountArchive.CopyTo(preimage.AsSpan(offset));
        offset += accountArchive.Length;
        suffix.CopyTo(preimage.AsSpan(offset));
        return IrohaHash.Hash(preimage);
    }

    private static byte[] HashCanonicalRequest(
        ReadOnlySpan<byte> kindTag,
        ReadOnlySpan<byte> requestArchive)
    {
        ReadOnlySpan<byte> domain =
            "iroha:offline:kagemusha:operation-request:v4\0"u8;
        var preimage = new byte[checked(
            domain.Length + kindTag.Length + sizeof(ulong) + requestArchive.Length)];
        var offset = 0;
        domain.CopyTo(preimage.AsSpan(offset));
        offset += domain.Length;
        kindTag.CopyTo(preimage.AsSpan(offset));
        offset += kindTag.Length;
        BinaryPrimitives.WriteUInt64LittleEndian(
            preimage.AsSpan(offset, sizeof(ulong)),
            checked((ulong)requestArchive.Length));
        offset += sizeof(ulong);
        requestArchive.CopyTo(preimage.AsSpan(offset));
        return IrohaHash.Hash(preimage);
    }
}
