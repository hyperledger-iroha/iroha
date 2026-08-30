using System.Globalization;
using System.Text;

namespace Hyperledger.Iroha.Address;

public sealed class AccountAddress : IEquatable<AccountAddress>
{
    public const ushort DefaultChainDiscriminant = 0x02F1;
    public const ushort DevChainDiscriminant = 0x0000;
    public const ushort TestChainDiscriminant = 0x0171;

    private const byte DefaultHeaderVersion = 0;
    private const byte DefaultNormalizationVersion = 1;
    private const byte SingleKeyControllerTag = 0x00;
    private const byte MultisigControllerTag = 0x01;
    private const byte ExtendedSingleKeyControllerTag = 0x02;
    private const int MlDsa65PublicKeyLength = 1_952;
    private const int I105ChecksumLength = 6;
    private const int I105Base = 105;
    private const int Bech32mConst = 0x2BC830A3;

    private static readonly string[] CanonicalI105Alphabet =
    [
        .. "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".Select(static ch => ch.ToString()),
        "ｲ", "ﾛ", "ﾊ", "ﾆ", "ﾎ", "ﾍ", "ﾄ", "ﾁ", "ﾘ", "ﾇ", "ﾙ", "ｦ", "ﾜ", "ｶ", "ﾖ", "ﾀ", "ﾚ", "ｿ",
        "ﾂ", "ﾈ", "ﾅ", "ﾗ", "ﾑ", "ｳ", "ヰ", "ﾉ", "ｵ", "ｸ", "ﾔ", "ﾏ", "ｹ", "ﾌ", "ｺ", "ｴ", "ﾃ", "ｱ",
        "ｻ", "ｷ", "ﾕ", "ﾒ", "ﾐ", "ｼ", "ヱ", "ﾋ", "ﾓ", "ｾ", "ｽ",
    ];

    private static readonly IReadOnlyDictionary<string, int> I105Index = BuildI105Index();

    private readonly byte[] canonicalBytes;
    private readonly byte[] controllerBytes;
    private readonly byte[]? publicKey;

    private AccountAddress(
        byte headerVersion,
        byte normalizationVersion,
        AddressClass addressClass,
        byte[] canonicalBytes,
        byte[] controllerBytes,
        CurveId? curveIdentifier,
        string? algorithm,
        byte[]? publicKey)
    {
        HeaderVersion = headerVersion;
        NormalizationVersion = normalizationVersion;
        AddressClass = addressClass;
        this.canonicalBytes = canonicalBytes;
        this.controllerBytes = controllerBytes;
        CurveIdentifier = curveIdentifier;
        Algorithm = algorithm;
        this.publicKey = publicKey;
    }

    public string? Algorithm { get; }

    public AddressClass AddressClass { get; }

    public string CanonicalHex => $"0x{ToLowerHex(canonicalBytes)}";

    public CurveId? CurveIdentifier { get; }

    public byte HeaderVersion { get; }

    public byte NormalizationVersion { get; }

    public byte[] PublicKey => publicKey is null ? [] : [.. publicKey];

    public byte[] CanonicalBytes() => [.. canonicalBytes];

    public byte[] ControllerBytes() => [.. controllerBytes];

    /// <inheritdoc />
    public bool Equals(AccountAddress? other) =>
        other is not null && canonicalBytes.AsSpan().SequenceEqual(other.canonicalBytes);

    /// <inheritdoc />
    public override bool Equals(object? obj) => obj is AccountAddress other && Equals(other);

    /// <inheritdoc />
    public override int GetHashCode()
    {
        var hash = new HashCode();
        foreach (var value in canonicalBytes)
        {
            hash.Add(value);
        }
        return hash.ToHashCode();
    }

    public static bool operator ==(AccountAddress? left, AccountAddress? right) =>
        Equals(left, right);

    public static bool operator !=(AccountAddress? left, AccountAddress? right) =>
        !Equals(left, right);

    public static AccountAddress FromCanonicalBytes(ReadOnlySpan<byte> payload)
    {
        if (payload.IsEmpty)
        {
            throw NewError(AccountAddressErrorCode.InvalidLength, "invalid length for address payload");
        }

        var header = payload[0];
        var headerVersion = (byte)((header >> 5) & 0b111);
        var classBits = (byte)((header >> 3) & 0b11);
        var normalizationVersion = (byte)((header >> 1) & 0b11);
        var extensionFlag = (header & 0b1) != 0;

        if (extensionFlag)
        {
            throw NewError(AccountAddressErrorCode.InvalidHeaderVersion, "address header extension flag is reserved");
        }

        if (headerVersion != DefaultHeaderVersion)
        {
            throw NewError(AccountAddressErrorCode.InvalidHeaderVersion, $"unsupported address header version: {headerVersion}");
        }

        if (normalizationVersion != DefaultNormalizationVersion)
        {
            throw NewError(AccountAddressErrorCode.InvalidNormVersion, $"unsupported normalization version: {normalizationVersion}");
        }

        var canonicalBytes = payload.ToArray();
        return classBits switch
        {
            (byte)AddressClass.SingleKey => ParseSingleKey(canonicalBytes, headerVersion, normalizationVersion),
            (byte)AddressClass.MultiSig => ParseMultisig(canonicalBytes, headerVersion, normalizationVersion),
            _ => throw NewError(AccountAddressErrorCode.UnknownAddressClass, $"unknown address class: {classBits}"),
        };
    }

    public static AccountAddress FromPublicKey(
        ReadOnlySpan<byte> publicKey,
        CurveId curveIdentifier = CurveId.Ed25519)
    {
        if (!Enum.IsDefined(typeof(CurveId), curveIdentifier))
        {
            throw NewError(
                AccountAddressErrorCode.UnknownCurve,
                $"unknown curve id: {(byte)curveIdentifier}");
        }

        if (publicKey.Length > ushort.MaxValue)
        {
            throw NewError(AccountAddressErrorCode.InvalidPublicKey, "public key length must be between 1 and 65535 bytes");
        }
        ValidateControllerPublicKey(curveIdentifier, publicKey);
        if (publicKey.IsEmpty)
        {
            throw NewError(AccountAddressErrorCode.InvalidPublicKey, "public key length must be between 1 and 65535 bytes");
        }

        var extended = publicKey.Length > byte.MaxValue;
        var keyOffset = extended ? 4 : 3;
        var controllerBytes = new byte[keyOffset + publicKey.Length];
        controllerBytes[0] = extended ? ExtendedSingleKeyControllerTag : SingleKeyControllerTag;
        controllerBytes[1] = (byte)curveIdentifier;
        if (extended)
        {
            controllerBytes[2] = (byte)(publicKey.Length >> 8);
            controllerBytes[3] = (byte)publicKey.Length;
        }
        else
        {
            controllerBytes[2] = (byte)publicKey.Length;
        }
        publicKey.CopyTo(controllerBytes.AsSpan(keyOffset));

        var canonicalBytes = new byte[1 + controllerBytes.Length];
        canonicalBytes[0] = EncodeHeader(AddressClass.SingleKey);
        controllerBytes.CopyTo(canonicalBytes.AsSpan(1));

        return new AccountAddress(
            DefaultHeaderVersion,
            DefaultNormalizationVersion,
            AddressClass.SingleKey,
            canonicalBytes,
            controllerBytes,
            curveIdentifier,
            CurveIdToAlgorithm(curveIdentifier),
            publicKey.ToArray());
    }

    public static AccountAddress Parse(string encoded, ushort? expectedDiscriminant = null)
    {
        ArgumentNullException.ThrowIfNull(encoded);

        var trimmed = encoded.Trim();
        if (trimmed.Length == 0)
        {
            throw NewError(AccountAddressErrorCode.InvalidLength, "invalid length for address payload");
        }

        if (!string.Equals(trimmed, encoded, StringComparison.Ordinal))
        {
            throw NewError(AccountAddressErrorCode.UnsupportedAddressFormat, "account address literals must not contain surrounding whitespace");
        }

        if (encoded.Contains('@', StringComparison.Ordinal))
        {
            throw NewError(AccountAddressErrorCode.UnsupportedAddressFormat, "account address literals must not include @domain; use canonical I105 form");
        }

        if (encoded.StartsWith("0x", StringComparison.OrdinalIgnoreCase))
        {
            throw NewError(AccountAddressErrorCode.UnsupportedAddressFormat, "canonical hex account addresses are not accepted; use canonical I105 form");
        }

        var (discriminant, payload) = DecodeI105(encoded, expectedDiscriminant);
        var address = FromCanonicalBytes(payload);
        if (!string.Equals(address.ToI105(discriminant), encoded, StringComparison.Ordinal))
        {
            throw NewError(AccountAddressErrorCode.UnsupportedAddressFormat, "account address literals must use canonical I105 form");
        }

        return address;
    }

    public string ToI105(ushort chainDiscriminant = DefaultChainDiscriminant)
    {
        var digits = EncodeBaseN(canonicalBytes, I105Base);
        var checksum = ComputeI105ChecksumDigits(canonicalBytes);
        var builder = new StringBuilder();
        builder.Append(GetI105Sentinel(chainDiscriminant));
        foreach (var digit in digits)
        {
            builder.Append(CanonicalI105Alphabet[digit]);
        }

        foreach (var digit in checksum)
        {
            builder.Append(CanonicalI105Alphabet[digit]);
        }

        return builder.ToString();
    }

    public override string ToString() => ToI105();

    private static AccountAddress ParseMultisig(byte[] canonicalBytes, byte headerVersion, byte normalizationVersion)
    {
        if (canonicalBytes.Length < 8)
        {
            throw NewError(AccountAddressErrorCode.InvalidLength, "invalid length for multisig address payload");
        }

        if (canonicalBytes[1] != MultisigControllerTag)
        {
            throw NewError(AccountAddressErrorCode.UnknownControllerTag, $"unknown multisig controller tag: {canonicalBytes[1]}");
        }

        var cursor = 2;
        var policyVersion = canonicalBytes[cursor++];
        if (policyVersion != 1)
        {
            throw NewError(AccountAddressErrorCode.InvalidLength, $"unsupported multisig policy version: {policyVersion}");
        }
        var threshold = ReadUInt16(canonicalBytes, ref cursor);
        var memberCount = ReadUInt16(canonicalBytes, ref cursor);
        if (threshold == 0 || memberCount == 0)
        {
            throw NewError(AccountAddressErrorCode.InvalidLength, "invalid multisig threshold or member count");
        }
        var totalWeight = 0UL;
        for (var index = 0; index < memberCount; index++)
        {
            if (cursor >= canonicalBytes.Length)
            {
                throw NewError(AccountAddressErrorCode.InvalidLength, "truncated multisig member");
            }
            var curveRaw = canonicalBytes[cursor++];
            if (!Enum.IsDefined(typeof(CurveId), curveRaw))
            {
                throw NewError(AccountAddressErrorCode.UnknownCurve, $"unknown curve id: {curveRaw}");
            }
            var weight = ReadUInt16(canonicalBytes, ref cursor);
            var keyLength = ReadUInt16(canonicalBytes, ref cursor);
            if (weight == 0 || keyLength == 0 || cursor + keyLength > canonicalBytes.Length)
            {
                throw NewError(AccountAddressErrorCode.InvalidLength, "invalid multisig member payload");
            }
            ValidateControllerPublicKey(
                (CurveId)curveRaw,
                canonicalBytes.AsSpan(cursor, keyLength));
            cursor += keyLength;
            totalWeight += weight;
        }
        if (cursor != canonicalBytes.Length)
        {
            throw NewError(AccountAddressErrorCode.UnexpectedTrailingBytes, "unexpected trailing bytes in canonical payload");
        }
        if (totalWeight < threshold)
        {
            throw NewError(AccountAddressErrorCode.InvalidLength, "multisig threshold exceeds total weight");
        }

        return new AccountAddress(
            headerVersion,
            normalizationVersion,
            AddressClass.MultiSig,
            canonicalBytes,
            canonicalBytes[1..],
            null,
            null,
            null);
    }

    private static AccountAddress ParseSingleKey(byte[] canonicalBytes, byte headerVersion, byte normalizationVersion)
    {
        if (canonicalBytes.Length < 4)
        {
            throw NewError(AccountAddressErrorCode.InvalidLength, "invalid length for single-key address payload");
        }

        var tag = canonicalBytes[1];
        if (tag != SingleKeyControllerTag && tag != ExtendedSingleKeyControllerTag)
        {
            throw NewError(AccountAddressErrorCode.UnknownControllerTag, $"unknown controller payload tag: {tag}");
        }

        if (!Enum.IsDefined(typeof(CurveId), canonicalBytes[2]))
        {
            throw NewError(AccountAddressErrorCode.UnknownCurve, $"unknown curve id: {canonicalBytes[2]}");
        }

        var curveId = (CurveId)canonicalBytes[2];
        var extended = tag == ExtendedSingleKeyControllerTag;
        if (extended && canonicalBytes.Length < 5)
        {
            throw NewError(AccountAddressErrorCode.InvalidLength, "invalid extended single-key address payload");
        }
        var length = extended
            ? (canonicalBytes[3] << 8) | canonicalBytes[4]
            : canonicalBytes[3];
        if (extended && length <= byte.MaxValue)
        {
            throw NewError(AccountAddressErrorCode.InvalidLength, "short public keys must use the compact controller tag");
        }
        var keyOffset = extended ? 5 : 4;
        var expectedLength = keyOffset + length;
        if (expectedLength != canonicalBytes.Length)
        {
            throw NewError(AccountAddressErrorCode.UnexpectedTrailingBytes, "unexpected trailing bytes in canonical payload");
        }
        ValidateControllerPublicKey(curveId, canonicalBytes.AsSpan(keyOffset, length));

        return new AccountAddress(
            headerVersion,
            normalizationVersion,
            AddressClass.SingleKey,
            canonicalBytes,
            canonicalBytes[1..],
            curveId,
            CurveIdToAlgorithm(curveId),
            canonicalBytes[keyOffset..].ToArray());
    }

    private static ushort ReadUInt16(byte[] bytes, ref int cursor)
    {
        if (cursor + 1 >= bytes.Length)
        {
            throw NewError(AccountAddressErrorCode.InvalidLength, "truncated canonical address payload");
        }
        var value = (ushort)((bytes[cursor] << 8) | bytes[cursor + 1]);
        cursor += 2;
        return value;
    }

    private static void ValidateControllerPublicKey(CurveId curveId, ReadOnlySpan<byte> publicKey)
    {
        if (curveId == CurveId.Ed25519
            && (publicKey.Length != 32 || IsAllZero(publicKey)))
        {
            throw NewError(
                AccountAddressErrorCode.InvalidPublicKey,
                "invalid Ed25519 public key: expected a nonzero 32-byte key");
        }

        if (curveId == CurveId.MlDsa
            && (publicKey.Length != MlDsa65PublicKeyLength || IsAllZero(publicKey)))
        {
            throw NewError(
                AccountAddressErrorCode.InvalidPublicKey,
                "invalid ML-DSA public key: expected a nonzero 1952-byte ML-DSA-65 key");
        }
    }

    private static bool IsAllZero(ReadOnlySpan<byte> bytes)
    {
        foreach (var value in bytes)
        {
            if (value != 0)
            {
                return false;
            }
        }
        return true;
    }

    private static byte EncodeHeader(AddressClass addressClass)
    {
        byte header = 0;
        header |= (byte)(DefaultHeaderVersion << 5);
        header |= (byte)(((byte)addressClass & 0b11) << 3);
        header |= (byte)(DefaultNormalizationVersion << 1);
        return header;
    }

    private static string CurveIdToAlgorithm(CurveId curveId)
    {
        return curveId switch
        {
            CurveId.Ed25519 => "ed25519",
            CurveId.MlDsa => "ml-dsa",
            CurveId.Gost256A => "gost3410-2012-256-paramset-a",
            CurveId.Gost256B => "gost3410-2012-256-paramset-b",
            CurveId.Gost256C => "gost3410-2012-256-paramset-c",
            CurveId.Gost512A => "gost3410-2012-512-paramset-a",
            CurveId.Gost512B => "gost3410-2012-512-paramset-b",
            CurveId.Sm2 => "sm2",
            _ => throw NewError(AccountAddressErrorCode.UnknownCurve, $"unknown curve id: {curveId}"),
        };
    }

    private static (ushort Discriminant, byte[] Payload) DecodeI105(string encoded, ushort? expectedDiscriminant)
    {
        var (discriminant, payload) = SplitI105Sentinel(encoded);
        if (expectedDiscriminant.HasValue && expectedDiscriminant.Value != discriminant)
        {
            throw NewError(
                AccountAddressErrorCode.UnexpectedNetworkPrefix,
                $"unexpected i105 chain discriminant: expected {expectedDiscriminant.Value}, found {discriminant}");
        }

        var digits = payload.EnumerateRunes().Select(LookupI105Digit).ToArray();
        if (digits.Length <= I105ChecksumLength)
        {
            throw NewError(AccountAddressErrorCode.I105TooShort, "i105 address too short");
        }

        var dataDigits = digits[..^I105ChecksumLength];
        var checksumDigits = digits[^I105ChecksumLength..];
        var canonicalBytes = DecodeBaseN(dataDigits, I105Base);
        var expectedChecksum = ComputeI105ChecksumDigits(canonicalBytes);
        if (!expectedChecksum.SequenceEqual(checksumDigits))
        {
            throw NewError(AccountAddressErrorCode.ChecksumMismatch, "i105 checksum mismatch");
        }

        return (discriminant, canonicalBytes);
    }

    private static int LookupI105Digit(Rune rune)
    {
        var key = rune.ToString();
        if (I105Index.TryGetValue(key, out var digit))
        {
            return digit;
        }

        throw NewError(AccountAddressErrorCode.InvalidI105Char, $"invalid i105 alphabet symbol: {key}");
    }

    private static (ushort Discriminant, string Payload) SplitI105Sentinel(string encoded)
    {
        if (encoded.StartsWith("sora", StringComparison.Ordinal))
        {
            return (DefaultChainDiscriminant, encoded["sora".Length..]);
        }

        if (encoded.StartsWith("test", StringComparison.Ordinal))
        {
            return (TestChainDiscriminant, encoded["test".Length..]);
        }

        if (encoded.StartsWith("dev", StringComparison.Ordinal))
        {
            return (DevChainDiscriminant, encoded["dev".Length..]);
        }

        if (encoded.StartsWith("n", StringComparison.Ordinal))
        {
            var digits = new StringBuilder();
            foreach (var ch in encoded[1..])
            {
                if (!char.IsAsciiDigit(ch))
                {
                    break;
                }

                digits.Append(ch);
            }

            if (digits.Length == 0)
            {
                throw NewError(AccountAddressErrorCode.MissingI105Sentinel, "i105 address is missing the expected chain-discriminant sentinel");
            }

            if (!ushort.TryParse(digits.ToString(), NumberStyles.None, CultureInfo.InvariantCulture, out var discriminant))
            {
                throw NewError(AccountAddressErrorCode.InvalidI105Discriminant, "i105 chain discriminant must fit in an unsigned 16-bit integer");
            }

            return (discriminant, encoded[(1 + digits.Length)..]);
        }

        throw NewError(AccountAddressErrorCode.MissingI105Sentinel, "i105 address is missing the expected chain-discriminant sentinel");
    }

    private static string GetI105Sentinel(ushort discriminant)
    {
        return discriminant switch
        {
            DefaultChainDiscriminant => "sora",
            TestChainDiscriminant => "test",
            DevChainDiscriminant => "dev",
            _ => $"n{discriminant}",
        };
    }

    private static int[] EncodeBaseN(byte[] data, int @base)
    {
        if (@base < 2)
        {
            throw NewError(AccountAddressErrorCode.InvalidI105Base, "invalid base for encoding");
        }

        if (data.Length == 0)
        {
            return [0];
        }

        var value = data.ToArray();
        var leading = 0;
        while (leading < value.Length && value[leading] == 0)
        {
            leading += 1;
        }

        var digits = new List<int>();
        var start = leading;
        while (start < value.Length)
        {
            var remainder = 0;
            for (var index = start; index < value.Length; index += 1)
            {
                var accumulator = (remainder << 8) | value[index];
                value[index] = (byte)(accumulator / @base);
                remainder = accumulator % @base;
            }

            digits.Add(remainder);
            while (start < value.Length && value[start] == 0)
            {
                start += 1;
            }
        }

        digits.AddRange(Enumerable.Repeat(0, leading));
        if (digits.Count == 0)
        {
            digits.Add(0);
        }

        digits.Reverse();
        return [.. digits];
    }

    private static byte[] DecodeBaseN(IReadOnlyList<int> digits, int @base)
    {
        if (@base < 2)
        {
            throw NewError(AccountAddressErrorCode.InvalidI105Base, "invalid base for decoding");
        }

        if (digits.Count == 0)
        {
            throw NewError(AccountAddressErrorCode.InvalidLength, "invalid length for address payload");
        }

        var value = digits.ToArray();
        foreach (var digit in value)
        {
            if (digit < 0 || digit >= @base)
            {
                throw NewError(AccountAddressErrorCode.InvalidI105Digit, $"invalid digit {digit} for base {@base}");
            }
        }

        var leading = 0;
        while (leading < value.Length && value[leading] == 0)
        {
            leading += 1;
        }

        var output = new List<byte>();
        var start = leading;
        while (start < value.Length)
        {
            var remainder = 0;
            for (var index = start; index < value.Length; index += 1)
            {
                var accumulator = (remainder * @base) + value[index];
                value[index] = accumulator / 256;
                remainder = accumulator % 256;
            }

            output.Add((byte)remainder);
            while (start < value.Length && value[start] == 0)
            {
                start += 1;
            }
        }

        output.AddRange(Enumerable.Repeat((byte)0, leading));
        output.Reverse();
        return [.. output];
    }

    private static int[] ComputeI105ChecksumDigits(byte[] canonicalBytes)
    {
        var base32Digits = ConvertToBase32(canonicalBytes);
        var values = ExpandHrp("snx");
        values.AddRange(base32Digits);
        values.AddRange(Enumerable.Repeat(0, I105ChecksumLength));
        var polymod = Bech32Polymod(values) ^ Bech32mConst;

        var checksumDigits = new int[I105ChecksumLength];
        for (var index = 0; index < checksumDigits.Length; index += 1)
        {
            checksumDigits[index] = (polymod >> (5 * (I105ChecksumLength - 1 - index))) & 0x1F;
        }

        return checksumDigits;
    }

    private static List<int> ConvertToBase32(byte[] data)
    {
        var accumulator = 0;
        var bits = 0;
        var output = new List<int>();
        foreach (var value in data)
        {
            accumulator = (accumulator << 8) | value;
            bits += 8;
            while (bits >= 5)
            {
                bits -= 5;
                output.Add((accumulator >> bits) & 0x1F);
            }
        }

        if (bits > 0)
        {
            output.Add((accumulator << (5 - bits)) & 0x1F);
        }

        return output;
    }

    private static List<int> ExpandHrp(string hrp)
    {
        var output = new List<int>();
        foreach (var ch in hrp)
        {
            output.Add(ch >> 5);
        }

        output.Add(0);
        output.AddRange(hrp.Select(static ch => ch & 0x1F));
        return output;
    }

    private static int Bech32Polymod(IEnumerable<int> values)
    {
        ReadOnlySpan<int> generators = [0x3B6A57B2, 0x26508E6D, 0x1EA119FA, 0x3D4233DD, 0x2A1462B3];
        var checksum = 1;
        foreach (var value in values)
        {
            var top = checksum >> 25;
            checksum = ((checksum & 0x1FF_FFFF) << 5) ^ value;
            for (var index = 0; index < generators.Length; index += 1)
            {
                if (((top >> index) & 1) != 0)
                {
                    checksum ^= generators[index];
                }
            }
        }

        return checksum;
    }

    private static IReadOnlyDictionary<string, int> BuildI105Index()
    {
        var values = new Dictionary<string, int>(StringComparer.Ordinal);
        for (var index = 0; index < CanonicalI105Alphabet.Length; index += 1)
        {
            values[CanonicalI105Alphabet[index]] = index;
        }

        return values;
    }

    private static AccountAddressException NewError(AccountAddressErrorCode code, string message)
    {
        return new AccountAddressException(code, message);
    }

    private static string ToLowerHex(ReadOnlySpan<byte> value)
    {
        return Convert.ToHexString(value).ToLowerInvariant();
    }
}
