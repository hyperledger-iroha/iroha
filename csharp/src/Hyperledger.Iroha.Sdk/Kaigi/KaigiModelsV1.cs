using System.Collections.ObjectModel;
using System.Text;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Kaigi;

/// <summary>Exact little-endian Pasta Fp bytes. Zero is valid; no reduction or hash marker is applied.</summary>
public sealed class KaigiAuthorizationScalarV1 : IEquatable<KaigiAuthorizationScalarV1>
{
    private static readonly byte[] Modulus = Convert.FromHexString(
        "01000000ED302D991BF94C09FC98462200000000000000000000000000000040");
    private readonly byte[] bytes;
    public const int EncodedLength = 32;

    public KaigiAuthorizationScalarV1(ReadOnlySpan<byte> littleEndianBytes)
    {
        if (littleEndianBytes.Length != EncodedLength)
            throw new ArgumentException("Kaigi scalar requires exactly 32 bytes.", nameof(littleEndianBytes));
        var less = false;
        for (var index = EncodedLength - 1; index >= 0; index--)
        {
            if (littleEndianBytes[index] == Modulus[index]) continue;
            less = littleEndianBytes[index] < Modulus[index];
            break;
        }
        if (!less) throw new ArgumentException("Kaigi scalar must be below the Pasta Fp modulus.", nameof(littleEndianBytes));
        bytes = littleEndianBytes.ToArray();
    }

    public byte[] ToLittleEndianBytes() => (byte[])bytes.Clone();
    internal ReadOnlySpan<byte> Bytes => bytes;
    public bool Equals(KaigiAuthorizationScalarV1? other) => other is not null && bytes.AsSpan().SequenceEqual(other.bytes);
    public override bool Equals(object? obj) => obj is KaigiAuthorizationScalarV1 other && Equals(other);
    public override int GetHashCode() { var hash = new HashCode(); hash.AddBytes(bytes); return hash.ToHashCode(); }
    public override string ToString() => Convert.ToHexString(bytes).ToLowerInvariant();
}

/// <summary>The final single-field participant commitment.</summary>
public sealed record KaigiParticipantCommitment
{
    public KaigiParticipantCommitment(KaigiAuthorizationScalarV1 commitment) => Commitment = commitment ?? throw new ArgumentNullException(nameof(commitment));
    public KaigiAuthorizationScalarV1 Commitment { get; }
}

/// <summary>The final single-field action nullifier.</summary>
public sealed record KaigiParticipantNullifier
{
    public KaigiParticipantNullifier(KaigiAuthorizationScalarV1 digest) => Digest = digest ?? throw new ArgumentNullException(nameof(digest));
    public KaigiAuthorizationScalarV1 Digest { get; }
}

/// <summary>Complete public authorization artifacts. Proof bytes are supplied by a prover; this SDK does not generate proofs.</summary>
public sealed class KaigiAuthorizationArtifactsV1
{
    private readonly byte[] rosterRoot;
    private readonly byte[] proof;
    public KaigiAuthorizationArtifactsV1(KaigiParticipantCommitment commitment, KaigiParticipantNullifier nullifier,
        ReadOnlySpan<byte> rosterRoot, ReadOnlySpan<byte> proof)
    {
        Commitment = commitment ?? throw new ArgumentNullException(nameof(commitment));
        Nullifier = nullifier ?? throw new ArgumentNullException(nameof(nullifier));
        this.rosterRoot = KaigiValidationV1.Hash(rosterRoot, nameof(rosterRoot));
        this.proof = KaigiValidationV1.Proof(proof);
    }
    public KaigiParticipantCommitment Commitment { get; }
    public KaigiParticipantNullifier Nullifier { get; }
    public byte[] RosterRoot => (byte[])rosterRoot.Clone();
    public byte[] Proof => (byte[])proof.Clone();
    internal ReadOnlySpan<byte> RootBytes => rosterRoot;
    internal ReadOnlySpan<byte> ProofBytes => proof;
}

/// <summary>Complete usage commitment and supplied proof. The V1 relation does not attest an encrypted log.</summary>
public sealed class KaigiUsageArtifactsV1
{
    private readonly byte[] proof;
    public KaigiUsageArtifactsV1(KaigiAuthorizationScalarV1 usageCommitment, ReadOnlySpan<byte> proof)
    {
        UsageCommitment = usageCommitment ?? throw new ArgumentNullException(nameof(usageCommitment));
        this.proof = KaigiValidationV1.Proof(proof);
    }
    public KaigiAuthorizationScalarV1 UsageCommitment { get; }
    public byte[] Proof => (byte[])proof.Clone();
    internal ReadOnlySpan<byte> ProofBytes => proof;
}

/// <summary>Canonical domain-scoped call identity.</summary>
public sealed record KaigiId
{
    public KaigiId(string domainId, string callName)
    {
        DomainId = KaigiValidationV1.Domain(domainId);
        CallName = KaigiValidationV1.Name(callName, nameof(callName));
    }
    public string DomainId { get; }
    public string CallName { get; }
    internal byte[] Encode(TransactionEncodingContext context)
    {
        var parts = DomainId.Split('.');
        return KaigiWireV1.Struct(KaigiWireV1.Struct(context.EncodeName(parts[0]), context.EncodeName(parts[1])), context.EncodeName(CallName));
    }
    public override string ToString() => $"{DomainId}:{CallName}";
}

public enum KaigiPrivacyMode { Transparent, ZkRosterV1 }
public enum KaigiRoomPolicy { Public, Authenticated }
public enum KaigiStatus { Active, Ended }

/// <summary>A pinned relay descriptor in a call's onion route.</summary>
public sealed class KaigiRelayHop
{
    private readonly byte[] hpkePublicKey;
    public KaigiRelayHop(string relayId, ReadOnlySpan<byte> hpkePublicKey, byte weight)
    {
        RelayId = TransactionEncodingContext.CanonicalizeAccountId(relayId, nameof(relayId));
        if (hpkePublicKey.Length is < 1 or > 4096) throw new ArgumentException("Kaigi HPKE descriptor requires 1..4096 bytes.", nameof(hpkePublicKey));
        if (weight == 0) throw new ArgumentOutOfRangeException(nameof(weight));
        this.hpkePublicKey = hpkePublicKey.ToArray();
        Weight = weight;
    }
    public string RelayId { get; }
    public byte[] HpkePublicKey => (byte[])hpkePublicKey.Clone();
    public byte Weight { get; }
    internal byte[] Encode(TransactionEncodingContext context) => KaigiWireV1.Struct(context.EncodeAccountId(RelayId), KaigiWireV1.BytesVector(hpkePublicKey), [Weight]);
}

public sealed class KaigiRelayManifest
{
    public KaigiRelayManifest(IEnumerable<KaigiRelayHop> hops, ulong expiryMs)
    {
        ArgumentNullException.ThrowIfNull(hops);
        var copy = hops.Take(9).ToArray();
        if (copy.Length is < 3 or > 8 || copy.Any(static hop => hop is null) || copy.Select(static hop => hop.RelayId).Distinct(StringComparer.Ordinal).Count() != copy.Length)
            throw new ArgumentException("Kaigi manifest requires 3..8 distinct relay accounts.", nameof(hops));
        if (expiryMs == 0) throw new ArgumentOutOfRangeException(nameof(expiryMs));
        Hops = Array.AsReadOnly(copy);
        ExpiryMs = expiryMs;
    }
    public IReadOnlyList<KaigiRelayHop> Hops { get; }
    public ulong ExpiryMs { get; }
    internal byte[] Encode(TransactionEncodingContext context) => KaigiWireV1.Struct(KaigiWireV1.Sequence(Hops.Select(hop => hop.Encode(context))), context.EncodeUInt64(ExpiryMs));
}

/// <summary>Complete first-release call configuration; private calls require authorization when encoded.</summary>
public sealed class NewKaigi
{
    private readonly IReadOnlyDictionary<string, JsonNode?> metadata;
    public NewKaigi(KaigiId id, string host, string? title = null, string? description = null,
        uint? maxParticipants = null, ulong gasRatePerMinute = 0, IReadOnlyDictionary<string, JsonNode?>? metadata = null,
        ulong? scheduledStartMs = null, string? billingAccount = null,
        KaigiPrivacyMode privacyMode = KaigiPrivacyMode.Transparent,
        KaigiRoomPolicy roomPolicy = KaigiRoomPolicy.Authenticated, KaigiRelayManifest? relayManifest = null)
    {
        Id = id ?? throw new ArgumentNullException(nameof(id));
        Host = TransactionEncodingContext.CanonicalizeAccountId(host, nameof(host));
        Title = KaigiValidationV1.Text(title); Description = KaigiValidationV1.Text(description);
        if (maxParticipants is 0 or > 4096) throw new ArgumentOutOfRangeException(nameof(maxParticipants));
        if (!Enum.IsDefined(privacyMode) || !Enum.IsDefined(roomPolicy)) throw new ArgumentException("Unknown Kaigi configuration enum.");
        BillingAccount = billingAccount is null ? null : TransactionEncodingContext.CanonicalizeAccountId(billingAccount, nameof(billingAccount));
        if (BillingAccount is not null && BillingAccount != Host) throw new ArgumentException("V1 billing account must equal the host.", nameof(billingAccount));
        MaxParticipants = maxParticipants; GasRatePerMinute = gasRatePerMinute; ScheduledStartMs = scheduledStartMs;
        PrivacyMode = privacyMode; RoomPolicy = roomPolicy; RelayManifest = relayManifest;
        this.metadata = KaigiValidationV1.Metadata(metadata ?? new Dictionary<string, JsonNode?>());
    }
    public KaigiId Id { get; }
    public string Host { get; }
    public string? Title { get; }
    public string? Description { get; }
    public uint? MaxParticipants { get; }
    public ulong GasRatePerMinute { get; }
    public IReadOnlyDictionary<string, JsonNode?> Metadata => KaigiValidationV1.Metadata(metadata);
    public ulong? ScheduledStartMs { get; }
    public string? BillingAccount { get; }
    public KaigiPrivacyMode PrivacyMode { get; }
    public KaigiRoomPolicy RoomPolicy { get; }
    public KaigiRelayManifest? RelayManifest { get; }
    internal byte[] Encode(TransactionEncodingContext context) => KaigiWireV1.Struct(
        Id.Encode(context), context.EncodeAccountId(Host), KaigiWireV1.Option(Title is null ? null : context.EncodeString(Title)),
        KaigiWireV1.Option(Description is null ? null : context.EncodeString(Description)), context.EncodeOption(MaxParticipants, context.EncodeUInt32),
        context.EncodeUInt64(GasRatePerMinute), context.EncodeMetadata(metadata), context.EncodeOption(ScheduledStartMs, context.EncodeUInt64),
        KaigiWireV1.Option(BillingAccount is null ? null : context.EncodeAccountId(BillingAccount)), context.EncodeUInt32((uint)PrivacyMode),
        context.EncodeUInt32((uint)RoomPolicy), KaigiWireV1.Option(RelayManifest?.Encode(context)));
}

internal static class KaigiValidationV1
{
    internal const int MaximumProofBytes = 64 * 1024 * 1024;
    private static readonly Encoding StrictUtf8 = new UTF8Encoding(false, true);
    internal static byte[] Hash(ReadOnlySpan<byte> bytes, string name)
    {
        if (bytes.Length != 32 || (bytes[^1] & 1) != 1) throw new ArgumentException("Kaigi roster root requires an exact canonical 32-byte Iroha Hash.", name);
        return bytes.ToArray();
    }
    internal static byte[] Proof(ReadOnlySpan<byte> bytes)
    {
        if (bytes.IsEmpty || bytes.Length > MaximumProofBytes) throw new ArgumentException("Kaigi proof requires 1..67108864 bytes.", nameof(bytes));
        return bytes.ToArray();
    }
    internal static string Name(string value, string name)
    {
        ArgumentNullException.ThrowIfNull(value);
        // TODO: share Rust's pinned NFC/UTS-46 owner before accepting Unicode identity names.
        if (value.Length is < 1 or > 255 || value.Any(static ch => ch is < '!' or > '~' or '@' or '#' or '$'))
            throw new ArgumentException("Kaigi identity names require canonical nonempty ASCII Name bytes.", name);
        return value;
    }
    internal static string Domain(string value)
    {
        ArgumentNullException.ThrowIfNull(value);
        var labels = value.Split('.');
        if (labels.Length != 2 || labels.Any(static label => label.Length is < 1 or > 63 || label[0] == '-' || label[^1] == '-' || label.Any(static ch => ch is not (>= 'a' and <= 'z') and not (>= '0' and <= '9') and not '-') || label.StartsWith("xn--", StringComparison.Ordinal) || (label.Length >= 4 && label[2..4] == "--")))
            throw new ArgumentException("Kaigi domain requires canonical ASCII domain.dataspace labels.", nameof(value));
        return value;
    }
    internal static string? Text(string? value)
    {
        if (value is not null && StrictUtf8.GetByteCount(value) > 1_048_576) throw new ArgumentException("Kaigi text exceeds the JSON record bound.", nameof(value));
        return value;
    }
    internal static IReadOnlyDictionary<string, JsonNode?> Metadata(IReadOnlyDictionary<string, JsonNode?> input)
    {
        var result = new Dictionary<string, JsonNode?>(StringComparer.Ordinal);
        foreach (var (key, value) in input)
        {
            Name(key, nameof(input));
            var clone = value?.DeepClone();
            _ = Text(TransactionEncodingContext.CanonicalJson(clone));
            result.Add(key, clone);
        }
        return new ReadOnlyDictionary<string, JsonNode?>(result);
    }
}

internal static class KaigiWireV1
{
    internal static byte[] Struct(params byte[][] fields)
    {
        var writer = new CanonicalNoritoWriter();
        foreach (var field in fields) writer.WriteField(field);
        return writer.ToArray();
    }
    internal static byte[] Option(byte[]? payload)
    {
        var writer = new CanonicalNoritoWriter(); writer.WriteByte(payload is null ? (byte)0 : (byte)1);
        if (payload is not null) writer.WriteField(payload);
        return writer.ToArray();
    }
    internal static byte[] BytesVector(ReadOnlySpan<byte> bytes)
    {
        var writer = new CanonicalNoritoWriter(); writer.WriteSequenceLength((ulong)bytes.Length); writer.WriteBytes(bytes); return writer.ToArray();
    }
    internal static byte[] Sequence(IEnumerable<byte[]> values)
    {
        var copy = values.ToArray(); var writer = new CanonicalNoritoWriter(); writer.WriteSequenceLength((ulong)copy.Length);
        foreach (var item in copy) writer.WriteField(item);
        return writer.ToArray();
    }
    internal static byte[][] Authorization(KaigiAuthorizationArtifactsV1? artifacts) => artifacts is null
        ? [Option(null), Option(null), Option(null), Option(null)]
        : [Option(Struct(artifacts.Commitment.Commitment.ToLittleEndianBytes())), Option(Struct(artifacts.Nullifier.Digest.ToLittleEndianBytes())),
            Option(artifacts.RootBytes.ToArray()), Option(BytesVector(artifacts.ProofBytes))];
}
