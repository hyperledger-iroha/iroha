using System.Buffers.Binary;
using System.IO.Compression;
using System.Text;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Kagemusha;

/// <summary>Stable application profile identifiers carried by IPM1.</summary>
public enum IrohaPeerPayloadProfileV1 : ushort
{
    KagemushaV1 = 1,
}

/// <summary>The only three KAGEMUSHA V1 peer-message identifiers carried by IPM1.</summary>
public enum IrohaPeerPayloadKindV1 : byte
{
    Request = 1,
    Payment = 2,
    Acknowledgement = 3,
}

public enum IrohaPeerContentEncodingV1 : byte
{
    None = 0,
    Zlib = 1,
}

public enum IrohaPeerWireCompressionPolicyV1
{
    Disabled,
    PeerOptimized,
}

public sealed record IrohaPeerWireLimitsV1
{
    public static IrohaPeerWireLimitsV1 PeerV1 { get; } = new();

    public IrohaPeerWireLimitsV1(
        int maximumCanonicalBytes = Kagemusha.MaximumPaymentBytes,
        int maximumKagemushaEncodedBytes = Kagemusha.MaximumPaymentBytes)
    {
        if (maximumCanonicalBytes is < 1 or > Kagemusha.MaximumPaymentBytes)
            throw new ArgumentOutOfRangeException(nameof(maximumCanonicalBytes));
        if (maximumKagemushaEncodedBytes is < 1 or > Kagemusha.MaximumPaymentBytes)
            throw new ArgumentOutOfRangeException(nameof(maximumKagemushaEncodedBytes));
        MaximumCanonicalBytes = maximumCanonicalBytes;
        MaximumKagemushaEncodedBytes = maximumKagemushaEncodedBytes;
    }

    public int MaximumCanonicalBytes { get; }
    public int MaximumKagemushaEncodedBytes { get; }
}

/// <summary>Exact canonical profile bytes; no proof or signature authority is inferred.</summary>
public sealed class IrohaPeerCanonicalPayloadV1 : IEquatable<IrohaPeerCanonicalPayloadV1>
{
    private readonly byte[] bytes;

    public IrohaPeerCanonicalPayloadV1(
        IrohaPeerPayloadProfileV1 profile,
        IrohaPeerPayloadKindV1 kind,
        ushort schemaVersion,
        ReadOnlySpan<byte> bytes)
    {
        if (profile != IrohaPeerPayloadProfileV1.KagemushaV1
            || schemaVersion != IrohaPeerWireMessageV1.ArchiveSchemaVersion
            || !Enum.IsDefined(kind))
            throw new ArgumentException("Unsupported peer payload profile, kind, or schema version.");
        if (bytes.IsEmpty || bytes.Length > MaximumFor(kind))
            throw new ArgumentOutOfRangeException(nameof(bytes));
        ValidateCanonicalFraming(kind, bytes);
        Profile = profile;
        Kind = kind;
        SchemaVersion = schemaVersion;
        this.bytes = bytes.ToArray();
    }

    public IrohaPeerPayloadProfileV1 Profile { get; }
    public IrohaPeerPayloadKindV1 Kind { get; }
    public ushort SchemaVersion { get; }
    public int ByteCount => bytes.Length;
    public byte[] Bytes() => bytes.ToArray();

    public bool Equals(IrohaPeerCanonicalPayloadV1? other) => other is not null
        && Profile == other.Profile
        && Kind == other.Kind
        && SchemaVersion == other.SchemaVersion
        && bytes.AsSpan().SequenceEqual(other.bytes);

    public override bool Equals(object? obj) => Equals(obj as IrohaPeerCanonicalPayloadV1);

    public override int GetHashCode()
    {
        var hash = new HashCode();
        hash.Add(Profile);
        hash.Add(Kind);
        hash.Add(SchemaVersion);
        foreach (var value in bytes) hash.Add(value);
        return hash.ToHashCode();
    }

    private static int MaximumFor(IrohaPeerPayloadKindV1 kind) => kind switch
    {
        IrohaPeerPayloadKindV1.Request => Kagemusha.MaximumRequestBytes,
        IrohaPeerPayloadKindV1.Payment => Kagemusha.MaximumPaymentBytes,
        IrohaPeerPayloadKindV1.Acknowledgement => Kagemusha.MaximumAcknowledgementBytes,
        _ => throw new ArgumentOutOfRangeException(nameof(kind)),
    };

    private static void ValidateCanonicalFraming(IrohaPeerPayloadKindV1 kind, ReadOnlySpan<byte> archive)
    {
        var schema = kind switch
        {
            IrohaPeerPayloadKindV1.Request =>
                "iroha_data_model::kagemusha::kagemusha_v1::KagemushaPaymentRequestV1",
            IrohaPeerPayloadKindV1.Payment =>
                "iroha_data_model::kagemusha::kagemusha_v1::KagemushaPaymentV1",
            IrohaPeerPayloadKindV1.Acknowledgement =>
                "iroha_data_model::kagemusha::kagemusha_v1::KagemushaAcknowledgementV1",
            _ => throw new ArgumentOutOfRangeException(nameof(kind)),
        };
        var (payload, flags) = NoritoCodec.Decode(schema, archive);
        if (flags != NoritoCodec.CanonicalLayoutFlags)
            throw new ArgumentException("Peer payload must use canonical compact Norito framing.", nameof(archive));
        var padding = kind is IrohaPeerPayloadKindV1.Request
            or IrohaPeerPayloadKindV1.Payment
            ? 8
            : 0;
        var encoded = NoritoCodec.Encode(schema, payload, NoritoCodec.CanonicalLayoutFlags);
        if (padding != 0)
        {
            var aligned = new byte[encoded.Length + padding];
            encoded.AsSpan(0, NoritoHeader.EncodedLength).CopyTo(aligned);
            encoded.AsSpan(NoritoHeader.EncodedLength).CopyTo(aligned.AsSpan(NoritoHeader.EncodedLength + padding));
            encoded = aligned;
        }
        if (!archive.SequenceEqual(encoded))
            throw new ArgumentException("Peer payload is not canonical for its three-message kind.", nameof(archive));
    }
}

/// <summary>Immutable, transport-neutral IPM1 envelope for one KAGEMUSHA payload.</summary>
public sealed class IrohaPeerWireMessageV1 : IEquatable<IrohaPeerWireMessageV1>
{
    public const byte Version = 1;
    public const ushort ArchiveSchemaVersion = 1;
    public const int HeaderLength = 84;

    private static ReadOnlySpan<byte> Magic => "IPM1"u8;
    private static readonly byte[] CanonicalDomain = System.Text.Encoding.UTF8.GetBytes("IROHA-PEER-PAYLOAD-V1\0");
    private static readonly byte[] MessageDomain = System.Text.Encoding.UTF8.GetBytes("IROHA-PEER-MESSAGE-V1\0");

    private readonly byte[] canonicalHash;
    private readonly byte[] wireHash;
    private readonly byte[] encodedBody;

    public IrohaPeerWireMessageV1(
        IrohaPeerCanonicalPayloadV1 canonicalPayload,
        IrohaPeerWireCompressionPolicyV1 compressionPolicy = IrohaPeerWireCompressionPolicyV1.Disabled,
        IrohaPeerWireLimitsV1? limits = null)
    {
        ArgumentNullException.ThrowIfNull(canonicalPayload);
        limits ??= IrohaPeerWireLimitsV1.PeerV1;
        if (canonicalPayload.ByteCount > limits.MaximumCanonicalBytes)
            throw new ArgumentOutOfRangeException(nameof(canonicalPayload));
        CanonicalPayload = canonicalPayload;
        canonicalHash = ComputeCanonicalHash(canonicalPayload);
        var canonical = canonicalPayload.Bytes();
        var compressed = compressionPolicy == IrohaPeerWireCompressionPolicyV1.PeerOptimized
            ? Compress(canonical)
            : null;
        var useCompressed = compressed is not null
            && IsCanonicalCompressedLength(canonical.Length, compressed.Length)
            && compressed.Length <= limits.MaximumKagemushaEncodedBytes;
        Encoding = useCompressed ? IrohaPeerContentEncodingV1.Zlib : IrohaPeerContentEncodingV1.None;
        encodedBody = useCompressed ? compressed! : canonical;
        if (encodedBody.Length > limits.MaximumKagemushaEncodedBytes)
            throw new ArgumentOutOfRangeException(nameof(canonicalPayload));
        wireHash = Blake2b.Hash256(Concat(
            MessageDomain,
            HeaderPrefix(Encoding, canonicalPayload, encodedBody.Length, canonicalHash),
            encodedBody));
    }

    private IrohaPeerWireMessageV1(
        IrohaPeerCanonicalPayloadV1 canonicalPayload,
        IrohaPeerContentEncodingV1 encoding,
        byte[] canonicalHash,
        byte[] wireHash,
        byte[] encodedBody)
    {
        CanonicalPayload = canonicalPayload;
        Encoding = encoding;
        this.canonicalHash = canonicalHash;
        this.wireHash = wireHash;
        this.encodedBody = encodedBody;
    }

    public IrohaPeerCanonicalPayloadV1 CanonicalPayload { get; }
    public IrohaPeerContentEncodingV1 Encoding { get; }
    public byte[] CanonicalHash() => canonicalHash.ToArray();
    public byte[] WireHash() => wireHash.ToArray();
    public byte[] StreamId() => wireHash[..16];
    public byte[] EncodedBody() => encodedBody.ToArray();

    public byte[] Encode()
    {
        var output = new byte[checked(HeaderLength + encodedBody.Length)];
        HeaderPrefix(Encoding, CanonicalPayload, encodedBody.Length, canonicalHash).CopyTo(output, 0);
        wireHash.CopyTo(output, 52);
        encodedBody.CopyTo(output, HeaderLength);
        return output;
    }

    public static IrohaPeerWireMessageV1 Decode(
        ReadOnlySpan<byte> data,
        IrohaPeerPayloadProfileV1? expectedProfile = null,
        IrohaPeerPayloadKindV1? expectedKind = null,
        IrohaPeerWireLimitsV1? limits = null)
    {
        limits ??= IrohaPeerWireLimitsV1.PeerV1;
        if (data.Length < HeaderLength || !data[..4].SequenceEqual(Magic) || data[4] != Version)
            throw new ArgumentException("Malformed IPM1 peer message.", nameof(data));
        if (!Enum.IsDefined((IrohaPeerContentEncodingV1)data[5]))
            throw new ArgumentException("Unsupported peer content encoding.", nameof(data));
        var encoding = (IrohaPeerContentEncodingV1)data[5];
        var profile = (IrohaPeerPayloadProfileV1)BinaryPrimitives.ReadUInt16BigEndian(data[6..8]);
        var kind = (IrohaPeerPayloadKindV1)data[8];
        if (profile != IrohaPeerPayloadProfileV1.KagemushaV1 || !Enum.IsDefined(kind)
            || data[9] != 0 || BinaryPrimitives.ReadUInt16BigEndian(data[10..12]) != ArchiveSchemaVersion)
            throw new ArgumentException("Invalid peer payload metadata.", nameof(data));
        if (expectedProfile is not null && expectedProfile != profile
            || expectedKind is not null && expectedKind != kind)
            throw new ArgumentException("Unexpected peer payload profile or kind.", nameof(data));
        var canonicalLength = CheckedLength(
            BinaryPrimitives.ReadUInt32BigEndian(data[12..16]), limits.MaximumCanonicalBytes);
        var encodedLength = CheckedLength(
            BinaryPrimitives.ReadUInt32BigEndian(data[16..20]), limits.MaximumKagemushaEncodedBytes);
        if (data.Length != HeaderLength + encodedLength
            || encoding == IrohaPeerContentEncodingV1.None && canonicalLength != encodedLength
            || encoding == IrohaPeerContentEncodingV1.Zlib
                && !IsCanonicalCompressedLength(canonicalLength, encodedLength))
            throw new ArgumentException("Peer message length or compression is noncanonical.", nameof(data));

        var canonicalDigest = data[20..52].ToArray();
        var messageDigest = data[52..84].ToArray();
        var body = data[84..].ToArray();
        var expectedWire = Blake2b.Hash256(Concat(MessageDomain, data[..52].ToArray(), body));
        if (!messageDigest.AsSpan().SequenceEqual(expectedWire))
            throw new ArgumentException("Peer message wire hash differs.", nameof(data));
        var canonical = encoding == IrohaPeerContentEncodingV1.None
            ? body.ToArray()
            : Decompress(body, canonicalLength);
        var payload = new IrohaPeerCanonicalPayloadV1(profile, kind, ArchiveSchemaVersion, canonical);
        if (!canonicalDigest.AsSpan().SequenceEqual(ComputeCanonicalHash(payload)))
            throw new ArgumentException("Peer canonical payload hash differs.", nameof(data));
        return new IrohaPeerWireMessageV1(payload, encoding, canonicalDigest, messageDigest, body);
    }

    public bool Equals(IrohaPeerWireMessageV1? other) => other is not null
        && CanonicalPayload.Equals(other.CanonicalPayload)
        && Encoding == other.Encoding
        && canonicalHash.AsSpan().SequenceEqual(other.canonicalHash)
        && wireHash.AsSpan().SequenceEqual(other.wireHash)
        && encodedBody.AsSpan().SequenceEqual(other.encodedBody);

    public override bool Equals(object? obj) => Equals(obj as IrohaPeerWireMessageV1);
    public override int GetHashCode() => HashCode.Combine(CanonicalPayload, Encoding, BinaryPrimitives.ReadInt32LittleEndian(wireHash));

    private static byte[] ComputeCanonicalHash(IrohaPeerCanonicalPayloadV1 payload)
    {
        Span<byte> metadata = stackalloc byte[5];
        BinaryPrimitives.WriteUInt16BigEndian(metadata[..2], (ushort)payload.Profile);
        metadata[2] = (byte)payload.Kind;
        BinaryPrimitives.WriteUInt16BigEndian(metadata[3..], payload.SchemaVersion);
        return Blake2b.Hash256(Concat(CanonicalDomain, metadata.ToArray(), payload.Bytes()));
    }

    private static byte[] HeaderPrefix(
        IrohaPeerContentEncodingV1 encoding,
        IrohaPeerCanonicalPayloadV1 payload,
        int encodedLength,
        byte[] canonicalHash)
    {
        var output = new byte[52];
        Magic.CopyTo(output);
        output[4] = Version;
        output[5] = (byte)encoding;
        BinaryPrimitives.WriteUInt16BigEndian(output.AsSpan(6, 2), (ushort)payload.Profile);
        output[8] = (byte)payload.Kind;
        output[9] = 0;
        BinaryPrimitives.WriteUInt16BigEndian(output.AsSpan(10, 2), payload.SchemaVersion);
        BinaryPrimitives.WriteUInt32BigEndian(output.AsSpan(12, 4), checked((uint)payload.ByteCount));
        BinaryPrimitives.WriteUInt32BigEndian(output.AsSpan(16, 4), checked((uint)encodedLength));
        canonicalHash.CopyTo(output, 20);
        return output;
    }

    private static int CheckedLength(uint value, int maximum)
    {
        if (value == 0 || value > maximum) throw new ArgumentOutOfRangeException(nameof(value));
        return checked((int)value);
    }

    private static bool IsCanonicalCompressedLength(int canonicalLength, int encodedLength) =>
        canonicalLength - encodedLength >= 32 && ShardCount(encodedLength) < ShardCount(canonicalLength);

    private static int ShardCount(int value) => checked((value + 255) / 256);

    private static byte[] Compress(byte[] canonical)
    {
        using var output = new MemoryStream(canonical.Length);
        using (var zlib = new ZLibStream(output, CompressionLevel.Optimal, leaveOpen: true))
            zlib.Write(canonical);
        return output.ToArray();
    }

    private static byte[] Decompress(byte[] encoded, int expectedLength)
    {
        if (encoded.Length < 6 || encoded[0] != 0x78 || encoded[1] != 0x9c)
            throw new ArgumentException("Invalid canonical zlib peer body.", nameof(encoded));
        using var input = new MemoryStream(encoded, writable: false);
        using var zlib = new ZLibStream(input, CompressionMode.Decompress);
        using var output = new MemoryStream(expectedLength);
        var buffer = new byte[Math.Min(16 * 1024, expectedLength)];
        while (true)
        {
            var count = zlib.Read(buffer, 0, buffer.Length);
            if (count == 0) break;
            if (output.Length + count > expectedLength)
                throw new ArgumentException("Expanded peer body exceeds its declared length.", nameof(encoded));
            output.Write(buffer, 0, count);
        }
        if (output.Length != expectedLength)
            throw new ArgumentException("Expanded peer body length differs.", nameof(encoded));
        return output.ToArray();
    }

    private static byte[] Concat(params byte[][] values)
    {
        var result = new byte[values.Sum(static value => value.Length)];
        var offset = 0;
        foreach (var value in values)
        {
            value.CopyTo(result, offset);
            offset += value.Length;
        }
        return result;
    }
}

public static class IrohaPeerKagemushaAdapterV1
{
    public static IrohaPeerWireMessageV1 Wrap(
        IrohaPeerPayloadKindV1 kind,
        ReadOnlySpan<byte> canonicalPayload,
        IrohaPeerWireCompressionPolicyV1 compressionPolicy = IrohaPeerWireCompressionPolicyV1.Disabled,
        IrohaPeerWireLimitsV1? limits = null) => new(
            new IrohaPeerCanonicalPayloadV1(
                IrohaPeerPayloadProfileV1.KagemushaV1,
                kind,
                IrohaPeerWireMessageV1.ArchiveSchemaVersion,
                canonicalPayload),
            compressionPolicy,
            limits);

    public static byte[] Decode(IrohaPeerWireMessageV1 message)
    {
        ArgumentNullException.ThrowIfNull(message);
        if (message.CanonicalPayload.Profile != IrohaPeerPayloadProfileV1.KagemushaV1
            || message.CanonicalPayload.SchemaVersion != IrohaPeerWireMessageV1.ArchiveSchemaVersion)
            throw new ArgumentException("Unexpected KAGEMUSHA peer profile.", nameof(message));
        return message.CanonicalPayload.Bytes();
    }
}
