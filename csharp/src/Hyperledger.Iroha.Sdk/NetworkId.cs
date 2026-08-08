using System.Globalization;
using System.Text;

namespace Hyperledger.Iroha;

/// <summary>
/// Exact deployment identity derived from the consensus hash of the genesis header.
/// </summary>
public sealed class NetworkId : IEquatable<NetworkId>
{
    public const int ByteLength = 32;

    private const int CanonicalLiteralLength = 74;
    private readonly byte[] bytes;
    private readonly string literal;

    private NetworkId(string literal, byte[] bytes)
    {
        this.literal = literal;
        this.bytes = bytes;
    }

    /// <summary>Parses one exact checksummed uppercase Norito Hash literal.</summary>
    public static NetworkId Parse(string value)
    {
        ArgumentNullException.ThrowIfNull(value);
        if (value.Length != CanonicalLiteralLength
            || !value.StartsWith("hash:", StringComparison.Ordinal)
            || value[69] != '#')
        {
            throw new FormatException(
                "Network id must be a canonical checksummed Norito Hash literal.");
        }

        var body = value.Substring(5, 64);
        var checksum = value.Substring(70, 4);
        if (body.Any(static item => !IsUpperHex(item))
            || checksum.Any(static item => !IsUpperHex(item))
            || !ushort.TryParse(
                checksum,
                NumberStyles.HexNumber,
                CultureInfo.InvariantCulture,
                out var supplied)
            || supplied != Crc16(Encoding.ASCII.GetBytes($"hash:{body}")))
        {
            throw new FormatException(
                "Network id has a malformed or invalid Norito Hash checksum.");
        }

        var decoded = Convert.FromHexString(body);
        if ((decoded[^1] & 1) == 0)
        {
            throw new FormatException("Network id Hash marker bit must be set.");
        }

        return new NetworkId(value, decoded);
    }

    /// <summary>Returns a defensive copy of the exact 32-byte genesis hash.</summary>
    public byte[] ToBytes() => bytes.ToArray();

    internal ReadOnlySpan<byte> AsSpan() => bytes;

    public bool Equals(NetworkId? other) =>
        other is not null && bytes.AsSpan().SequenceEqual(other.bytes);

    public override bool Equals(object? obj) => obj is NetworkId other && Equals(other);

    public override int GetHashCode() => StringComparer.Ordinal.GetHashCode(literal);

    public override string ToString() => literal;

    public static bool operator ==(NetworkId? left, NetworkId? right) =>
        ReferenceEquals(left, right) || left?.Equals(right) == true;

    public static bool operator !=(NetworkId? left, NetworkId? right) => !(left == right);

    private static bool IsUpperHex(char value) =>
        value is >= '0' and <= '9' or >= 'A' and <= 'F';

    private static ushort Crc16(ReadOnlySpan<byte> value)
    {
        var crc = 0xffff;
        foreach (var item in value)
        {
            crc ^= item << 8;
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
