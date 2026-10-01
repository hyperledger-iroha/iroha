using System.Globalization;
using System.Text;
using System.Text.Json;

namespace Hyperledger.Iroha.Norito;

/// <summary>Exact marked Norito Hash literals shared by contract metadata owners.</summary>
internal static class CanonicalHashLiteral
{
    internal static string Parse(string value, string context)
    {
        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{context} must not contain surrounding whitespace.");
        }
        if (value.Any(char.IsControl))
        {
            throw new JsonException($"{context} must not contain control characters.");
        }
        if (value.Length != 74
            || !value.StartsWith("hash:", StringComparison.Ordinal)
            || value[69] != '#')
        {
            throw new JsonException($"{context} must be a canonical checksummed Norito Hash literal.");
        }
        var body = value.Substring(5, 64);
        var checksum = value.Substring(70, 4);
        if (body.Any(character => !IsUpperHex(character))
            || checksum.Any(character => !IsUpperHex(character))
            || !ushort.TryParse(checksum, NumberStyles.HexNumber, CultureInfo.InvariantCulture, out var supplied)
            || supplied != Crc16(Encoding.ASCII.GetBytes($"hash:{body}")))
        {
            throw new JsonException($"{context} has a malformed or invalid Norito Hash checksum.");
        }
        var normalized = body.ToLowerInvariant();
        ValidateMarkerBit(normalized, context);
        return normalized;
    }

    internal static string Format(string value, string context)
    {
        ValidateHex(value, context);
        var body = value.ToUpperInvariant();
        var checksum = Crc16(Encoding.ASCII.GetBytes($"hash:{body}"));
        return $"hash:{body}#{checksum:X4}";
    }

    internal static void ValidateHex(string? value, string context)
    {
        if (value is null)
        {
            return;
        }
        if (value.Length != 64 || value.Any(character => !IsLowerHex(character)))
        {
            throw new JsonException($"{context} must be canonical lowercase 64-hex.");
        }
        ValidateMarkerBit(value, context);
    }

    private static void ValidateMarkerBit(string value, string context)
    {
        if (!byte.TryParse(value.AsSpan(value.Length - 2), NumberStyles.HexNumber, CultureInfo.InvariantCulture, out var last)
            || (last & 1) != 1)
        {
            throw new JsonException($"{context} must set the Iroha Hash marker bit.");
        }
    }

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

    private static bool IsUpperHex(char value)
    {
        return value is >= '0' and <= '9' or >= 'A' and <= 'F';
    }

    private static bool IsLowerHex(char value)
    {
        return value is >= '0' and <= '9' or >= 'a' and <= 'f';
    }
}
