namespace Hyperledger.Iroha.Transactions;

/// <summary>Validates canonical V1 Bech32m contract-address literals.</summary>
internal static class ContractAddressV1
{
    private const uint Bech32mConstant = 0x2BC830A3;
    private const string Charset = "qpzry9x8gf2tvdw0s3jn54khce6mua7l";
    private static readonly uint[] Generators =
    [
        0x3B6A57B2,
        0x26508E6D,
        0x1EA119FA,
        0x3D4233DD,
        0x2A1462B3,
    ];

    internal static bool IsCanonical(string? literal)
    {
        if (string.IsNullOrEmpty(literal)
            || literal.Length > 90
            || literal.Any(character => character is < (char)33 or > (char)126)
            || literal.Any(character => character is >= 'A' and <= 'Z'))
        {
            return false;
        }

        var separator = literal.LastIndexOf('1');
        if (separator <= 0 || separator > 83 || literal.Length - separator - 1 < 6)
        {
            return false;
        }

        var hrp = literal[..separator];
        var values = new List<byte>(literal.Length - separator - 1);
        foreach (var character in literal[(separator + 1)..])
        {
            var value = Charset.IndexOf(character, StringComparison.Ordinal);
            if (value < 0)
            {
                return false;
            }
            values.Add((byte)value);
        }

        var checksumInput = HrpExpand(hrp);
        checksumInput.AddRange(values);
        if (Polymod(checksumInput) != Bech32mConstant)
        {
            return false;
        }

        var payload = DecodePayload(values.Take(values.Count - 6));
        return payload is { Length: 29 } && payload[0] == 1;
    }

    private static List<byte> HrpExpand(string hrp)
    {
        var result = new List<byte>(hrp.Length * 2 + 1);
        result.AddRange(hrp.Select(character => (byte)(character >> 5)));
        result.Add(0);
        result.AddRange(hrp.Select(character => (byte)(character & 0x1F)));
        return result;
    }

    private static uint Polymod(IEnumerable<byte> values)
    {
        uint checksum = 1;
        foreach (var value in values)
        {
            var top = checksum >> 25;
            checksum = ((checksum & 0x01FF_FFFF) << 5) ^ value;
            for (var index = 0; index < Generators.Length; index++)
            {
                if (((top >> index) & 1) != 0)
                {
                    checksum ^= Generators[index];
                }
            }
        }
        return checksum;
    }

    private static byte[]? DecodePayload(IEnumerable<byte> values)
    {
        var output = new List<byte>();
        uint accumulator = 0;
        var bits = 0;
        foreach (var value in values)
        {
            accumulator = (accumulator << 5) | value;
            bits += 5;
            while (bits >= 8)
            {
                bits -= 8;
                output.Add((byte)((accumulator >> bits) & 0xFF));
            }
            accumulator &= bits == 0 ? 0 : (1U << bits) - 1;
        }
        if (bits >= 5 || (bits > 0 && accumulator != 0))
        {
            return null;
        }
        return output.ToArray();
    }
}
