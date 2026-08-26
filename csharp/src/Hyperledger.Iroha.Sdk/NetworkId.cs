using System.Globalization;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha;

/// <summary>
/// Exact deployment identity derived from the consensus hash of the genesis header.
/// </summary>
[JsonConverter(typeof(NetworkIdJsonConverter))]
public sealed class NetworkId : IEquatable<NetworkId>
{
    public const int ByteLength = 32;

    private const int CanonicalLiteralLength = ByteLength * 2;
    private readonly byte[] bytes;
    private readonly string literal;

    private NetworkId(string literal, byte[] bytes)
    {
        this.literal = literal;
        this.bytes = bytes;
    }

    /// <summary>Parses one exact 64-character lowercase hexadecimal network id.</summary>
    public static NetworkId Parse(string value)
    {
        ArgumentNullException.ThrowIfNull(value);
        if (value.Length != CanonicalLiteralLength
            || value.Any(static item => !IsLowerHex(item)))
        {
            throw new FormatException(
                "Network id must be exactly 64 lowercase hexadecimal characters.");
        }

        var decoded = Convert.FromHexString(value);
        if ((decoded[^1] & 1) == 0)
        {
            throw new FormatException("Network id hash marker bit must be set.");
        }

        return new NetworkId(value, decoded);
    }

    /// <summary>Returns a defensive copy of the exact 32-byte genesis hash.</summary>
    public byte[] ToBytes() => bytes.ToArray();

    internal ReadOnlySpan<byte> AsSpan() => bytes;

    internal string ToNoritoJsonLiteral()
    {
        var body = Convert.ToHexString(bytes);
        var checksum = Crc16(Encoding.ASCII.GetBytes($"hash:{body}"));
        return $"hash:{body}#{checksum:X4}";
    }

    internal static NetworkId ParseNoritoJsonLiteral(string value)
    {
        ArgumentNullException.ThrowIfNull(value);
        if (value.Length != 74
            || !value.StartsWith("hash:", StringComparison.Ordinal)
            || value[69] != '#')
        {
            throw new FormatException(
                "NetworkId JSON must be a canonical checksummed Norito Hash literal.");
        }

        var body = value.AsSpan(5, 64);
        var checksum = value.AsSpan(70, 4);
        if (body.IndexOfAnyExcept("0123456789ABCDEF".AsSpan()) >= 0
            || checksum.IndexOfAnyExcept("0123456789ABCDEF".AsSpan()) >= 0
            || !ushort.TryParse(
                checksum,
                NumberStyles.HexNumber,
                CultureInfo.InvariantCulture,
                out var supplied)
            || supplied != Crc16(Encoding.ASCII.GetBytes($"hash:{body.ToString()}")))
        {
            throw new FormatException(
                "NetworkId JSON has a malformed or invalid Norito Hash checksum.");
        }

        var decoded = Convert.FromHexString(body);
        if ((decoded[^1] & 1) == 0)
        {
            throw new FormatException("NetworkId JSON hash marker bit must be set.");
        }
        return new NetworkId(Convert.ToHexString(decoded).ToLowerInvariant(), decoded);
    }

    public bool Equals(NetworkId? other) =>
        other is not null && bytes.AsSpan().SequenceEqual(other.bytes);

    public override bool Equals(object? obj) => obj is NetworkId other && Equals(other);

    public override int GetHashCode() => StringComparer.Ordinal.GetHashCode(literal);

    public override string ToString() => literal;

    public static bool operator ==(NetworkId? left, NetworkId? right) =>
        ReferenceEquals(left, right) || left?.Equals(right) == true;

    public static bool operator !=(NetworkId? left, NetworkId? right) => !(left == right);

    private static bool IsLowerHex(char value) =>
        value is >= '0' and <= '9' or >= 'a' and <= 'f';

    private static ushort Crc16(ReadOnlySpan<byte> bytes)
    {
        var crc = 0xffff;
        foreach (var value in bytes)
        {
            crc ^= value << 8;
            for (var bit = 0; bit < 8; bit++)
            {
                crc = (crc & 0x8000) != 0
                    ? ((crc << 1) ^ 0x1021) & 0xffff
                    : (crc << 1) & 0xffff;
            }
        }
        return (ushort)crc;
    }
}

internal sealed class NetworkIdJsonConverter : JsonConverter<NetworkId>
{
    public override NetworkId Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        if (reader.TokenType != JsonTokenType.String)
        {
            throw new JsonException("NetworkId must be a canonical string literal.");
        }
        try
        {
            return NetworkId.ParseNoritoJsonLiteral(reader.GetString()!);
        }
        catch (FormatException error)
        {
            throw new JsonException(
                "NetworkId JSON must be a canonical checksummed Norito Hash literal with its marker bit set.",
                error);
        }
    }

    public override void Write(Utf8JsonWriter writer, NetworkId value, JsonSerializerOptions options) =>
        writer.WriteStringValue(value.ToNoritoJsonLiteral());
}
