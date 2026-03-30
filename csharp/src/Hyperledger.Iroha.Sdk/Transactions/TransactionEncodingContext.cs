using System.Numerics;
using System.Text;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Transactions;

internal sealed class TransactionEncodingContext
{
    private const ushort DefaultNetworkPrefix = AccountAddress.DefaultChainDiscriminant;
    private const byte AssetDefinitionVersion = 1;

    private static readonly Dictionary<CurveId, ulong> PublicKeyMultihashCodes = new()
    {
        [CurveId.Ed25519] = 0xED,
        [CurveId.MlDsa] = 0xEE,
        [CurveId.Sm2] = 0x1306,
    };

    private static readonly Dictionary<char, int> Base58Alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
        .Select(static (character, index) => new KeyValuePair<char, int>(character, index))
        .ToDictionary();

    public TransactionEncodingContext(string authorityAccountId)
    {
        AuthorityAccountId = CanonicalizeAccountId(authorityAccountId);
    }

    public string AuthorityAccountId { get; }

    public byte[] EncodeChainId(string chainId)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(EncodeString(chainId.Trim()));
        return writer.ToArray();
    }

    public byte[] EncodeAccountId(string accountId)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(EncodeAccountController(accountId));
        return writer.ToArray();
    }

    public byte[] EncodeAccountController(string accountId)
    {
        var parsed = AccountAddress.Parse(CanonicalizeAccountId(accountId), DefaultNetworkPrefix);
        if (parsed.CurveIdentifier is null || parsed.PublicKey.Length == 0)
        {
            throw new ArgumentException("Multisig account controllers are not yet supported by the managed transaction encoder.", nameof(accountId));
        }

        if (!PublicKeyMultihashCodes.TryGetValue(parsed.CurveIdentifier.Value, out var multihashCode))
        {
            throw new ArgumentException($"Unsupported account curve `{parsed.CurveIdentifier}` for managed Norito encoding.", nameof(accountId));
        }

        var multihash = FormatPublicKeyMultihash(multihashCode, parsed.PublicKey);
        var keyPayload = EncodeString(multihash);

        var writer = new OfflineNoritoWriter();
        writer.WriteUInt32LittleEndian(0);
        writer.WriteField(keyPayload);
        return writer.ToArray();
    }

    public byte[] EncodeString(string value)
    {
        var bytes = Encoding.UTF8.GetBytes(value);
        var writer = new OfflineNoritoWriter();
        writer.WriteLength((ulong)bytes.Length);
        writer.WriteBytes(bytes);
        return writer.ToArray();
    }

    public byte[] EncodeName(string value)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(value);
        return EncodeString(value.Trim());
    }

    public byte[] EncodeOptionalString(string? value)
    {
        var normalized = string.IsNullOrWhiteSpace(value) ? null : value.Trim();
        var writer = new OfflineNoritoWriter();
        if (normalized is null)
        {
            writer.WriteByte(0);
            return writer.ToArray();
        }

        writer.WriteByte(1);
        writer.WriteField(EncodeString(normalized));
        return writer.ToArray();
    }

    public byte[] EncodeJson(JsonNode? value)
    {
        return EncodeString(WriteJsonNode(value));
    }

    public byte[] EncodeUInt32(uint value)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteUInt32LittleEndian(value);
        return writer.ToArray();
    }

    public byte[] EncodeUInt64(ulong value)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteUInt64LittleEndian(value);
        return writer.ToArray();
    }

    public byte[] EncodeHashLiteral(string literal)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(literal);

        var bytes = DecodeFixedBytesLiteral(literal, expectedLength: 32);

        // Match Rust's `Hash::prehashed(...)` behaviour, which guarantees an odd LSB.
        bytes[^1] |= 0x01;
        return bytes;
    }

    public byte[] EncodeFixedBytesLiteral(string literal, int expectedLength)
    {
        return DecodeFixedBytesLiteral(literal, expectedLength);
    }

    public byte[] EncodeOption<T>(T? value, Func<T, byte[]> encoder)
        where T : struct
    {
        var writer = new OfflineNoritoWriter();
        if (!value.HasValue)
        {
            writer.WriteByte(0);
            return writer.ToArray();
        }

        writer.WriteByte(1);
        var payload = encoder(value.Value);
        writer.WriteField(payload);
        return writer.ToArray();
    }

    public byte[] EncodeEmptyMetadata()
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteLength(0);
        return writer.ToArray();
    }

    public byte[] EncodeMetadata(IReadOnlyDictionary<string, JsonNode?> metadata)
    {
        var writer = new OfflineNoritoWriter();
        var orderedKeys = metadata.Keys.OrderBy(static key => key, StringComparer.Ordinal).ToArray();
        writer.WriteLength((ulong)orderedKeys.Length);
        foreach (var key in orderedKeys)
        {
            var entry = EncodeMetadataEntry(key, metadata[key]);
            writer.WriteField(entry);
        }

        return writer.ToArray();
    }

    public byte[] EncodeNumeric(string value)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(value);

        var trimmed = value.Trim();
        var sign = 1;
        if (trimmed[0] == '-')
        {
            sign = -1;
            trimmed = trimmed[1..];
        }
        else if (trimmed[0] == '+')
        {
            trimmed = trimmed[1..];
        }

        var parts = trimmed.Split('.', 2);
        if (parts.Length == 0 || parts.Any(static part => part.Length == 0 && part != string.Empty))
        {
            throw new ArgumentException($"Invalid numeric literal `{value}`.", nameof(value));
        }

        if (parts.Any(static part => part.Any(static character => character is < '0' or > '9')))
        {
            throw new ArgumentException($"Invalid numeric literal `{value}`.", nameof(value));
        }

        var digits = string.Concat(parts);
        if (digits.Length == 0)
        {
            throw new ArgumentException($"Invalid numeric literal `{value}`.", nameof(value));
        }

        var scale = parts.Length == 2 ? parts[1].Length : 0;
        if (scale > 28)
        {
            throw new ArgumentOutOfRangeException(nameof(value), "Iroha numerics support at most 28 fractional digits.");
        }

        var bigInteger = BigInteger.Parse(digits, System.Globalization.CultureInfo.InvariantCulture);
        if (sign < 0)
        {
            bigInteger = BigInteger.Negate(bigInteger);
        }

        var mantissaBytes = bigInteger.ToByteArray(isUnsigned: false, isBigEndian: false);

        var mantissa = new OfflineNoritoWriter();
        mantissa.WriteUInt32LittleEndian((uint)mantissaBytes.Length);
        mantissa.WriteBytes(mantissaBytes);

        var writer = new OfflineNoritoWriter();
        writer.WriteField(mantissa.ToArray());
        writer.WriteField(EncodeUInt32((uint)scale));
        return writer.ToArray();
    }

    public byte[] EncodeConstVec(ReadOnlySpan<byte> bytes)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteLength((ulong)bytes.Length);
        foreach (var value in bytes)
        {
            writer.WriteLength(1);
            writer.WriteByte(value);
        }

        return writer.ToArray();
    }

    public byte[] EncodeInstruction(TransactionInstruction instruction)
    {
        var framedInstruction = NoritoCodec.Encode(instruction.TypeName, instruction.EncodePayload(this));

        var writer = new OfflineNoritoWriter();
        writer.WriteField(EncodeString(instruction.WireId));
        writer.WriteField(EncodeBytesVec(framedInstruction));
        return writer.ToArray();
    }

    public byte[] EncodeInstructionsExecutable(IReadOnlyList<TransactionInstruction> instructions)
    {
        var instructionsWriter = new OfflineNoritoWriter();
        instructionsWriter.WriteLength((ulong)instructions.Count);
        foreach (var instruction in instructions)
        {
            instructionsWriter.WriteField(EncodeInstruction(instruction));
        }

        var executable = new OfflineNoritoWriter();
        executable.WriteUInt32LittleEndian(0);
        executable.WriteField(instructionsWriter.ToArray());
        return executable.ToArray();
    }

    public byte[] EncodeAssetId(string assetDefinitionId, string accountId, ulong? dataspaceId = null)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(EncodeAccountId(accountId));
        writer.WriteField(EncodeAssetDefinitionAddress(assetDefinitionId));
        writer.WriteField(EncodeAssetBalanceScope(dataspaceId));
        return writer.ToArray();
    }

    public byte[] EncodeAssetDefinitionId(string literal)
    {
        return EncodeAssetDefinitionAddress(literal);
    }

    public byte[] EncodeNftId(string nftId)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(nftId);

        var trimmed = nftId.Trim();
        var separatorIndex = trimmed.IndexOf('$');
        if (separatorIndex <= 0 || separatorIndex != trimmed.LastIndexOf('$') || separatorIndex == trimmed.Length - 1)
        {
            throw new ArgumentException($"Invalid NFT id `{nftId}`.", nameof(nftId));
        }

        var writer = new OfflineNoritoWriter();
        writer.WriteField(EncodeName(trimmed[(separatorIndex + 1)..]));
        writer.WriteField(EncodeName(trimmed[..separatorIndex]));
        return writer.ToArray();
    }

    public byte[] EncodeTriggerId(string triggerId)
    {
        return EncodeName(triggerId);
    }

    public void EnsureAuthorityMatchesPrivateKey(ReadOnlySpan<byte> privateKeySeed)
    {
        var publicKey = Ed25519Signer.GetPublicKey(privateKeySeed);
        var expected = AccountAddress.FromPublicKey(publicKey, "ed25519").ToI105(DefaultNetworkPrefix);
        if (!string.Equals(expected, AuthorityAccountId, StringComparison.Ordinal))
        {
            throw new InvalidOperationException(
                $"The signing key derives account `{expected}`, but the transaction authority is `{AuthorityAccountId}`.");
        }
    }

    private byte[] EncodeBytesVec(ReadOnlySpan<byte> bytes)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteLength((ulong)bytes.Length);
        writer.WriteBytes(bytes);
        return writer.ToArray();
    }

    private byte[] EncodeMetadataEntry(string key, JsonNode? value)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(EncodeString(key));
        var jsonString = EncodeJson(value);
        var jsonField = new OfflineNoritoWriter();
        jsonField.WriteField(jsonString);
        writer.WriteField(jsonField.ToArray());
        return writer.ToArray();
    }

    private static string WriteJsonNode(JsonNode? node)
    {
        if (node is null)
        {
            return "null";
        }

        var builder = new StringBuilder();
        WriteJsonNode(node, builder);
        return builder.ToString();
    }

    private static void WriteJsonNode(JsonNode node, StringBuilder builder)
    {
        switch (node)
        {
            case JsonValue value:
                builder.Append(value.ToJsonString());
                return;
            case JsonArray array:
                builder.Append('[');
                for (var index = 0; index < array.Count; index++)
                {
                    if (index > 0)
                    {
                        builder.Append(',');
                    }

                    if (array[index] is JsonNode item)
                    {
                        WriteJsonNode(item, builder);
                    }
                    else
                    {
                        builder.Append("null");
                    }
                }

                builder.Append(']');
                return;
            case JsonObject obj:
                builder.Append('{');
                var first = true;
                foreach (var pair in obj.OrderBy(static pair => pair.Key, StringComparer.Ordinal))
                {
                    if (!first)
                    {
                        builder.Append(',');
                    }

                    first = false;
                    builder.Append(JsonValue.Create(pair.Key)!.ToJsonString());
                    builder.Append(':');
                    if (pair.Value is JsonNode child)
                    {
                        WriteJsonNode(child, builder);
                    }
                    else
                    {
                        builder.Append("null");
                    }
                }

                builder.Append('}');
                return;
            default:
                builder.Append(node.ToJsonString());
                return;
        }
    }

    private byte[] EncodeAssetDefinitionAddress(string literal)
    {
        var trimmed = literal.Trim();
        if (trimmed.Length == 0 || trimmed.IndexOfAny([':', '#', '@', '$']) >= 0)
        {
            throw new ArgumentException($"Invalid asset definition id `{literal}`.", nameof(literal));
        }

        var payload = DecodeBase58(trimmed);
        if (payload.Length != 21 || payload[0] != AssetDefinitionVersion)
        {
            throw new ArgumentException($"Invalid asset definition id `{literal}`.", nameof(literal));
        }

        var uuidBytes = payload.AsSpan(1, 16);
        if ((uuidBytes[6] >> 4) != 0x4 || (uuidBytes[8] & 0xC0) != 0x80)
        {
            throw new ArgumentException($"Invalid asset definition id `{literal}`.", nameof(literal));
        }

        var writer = new OfflineNoritoWriter();
        foreach (var value in uuidBytes)
        {
            writer.WriteLength(1);
            writer.WriteByte(value);
        }

        return writer.ToArray();
    }

    private byte[] EncodeAssetBalanceScope(ulong? dataspaceId)
    {
        var writer = new OfflineNoritoWriter();
        if (!dataspaceId.HasValue)
        {
            writer.WriteUInt32LittleEndian(0);
            return writer.ToArray();
        }

        writer.WriteUInt32LittleEndian(1);
        var dataspaceWriter = new OfflineNoritoWriter();
        dataspaceWriter.WriteUInt64LittleEndian(dataspaceId.Value);
        writer.WriteField(dataspaceWriter.ToArray());
        return writer.ToArray();
    }

    private static string CanonicalizeAccountId(string accountId)
    {
        return AccountAddress.Parse(accountId.Trim(), DefaultNetworkPrefix).ToI105(DefaultNetworkPrefix);
    }

    private static string FormatPublicKeyMultihash(ulong functionCode, ReadOnlySpan<byte> payload)
    {
        var functionHex = Convert.ToHexString(EncodeVarint(functionCode)).ToLowerInvariant();
        var lengthHex = Convert.ToHexString(EncodeVarint((ulong)payload.Length)).ToLowerInvariant();
        var payloadHex = Convert.ToHexString(payload).ToUpperInvariant();
        return functionHex + lengthHex + payloadHex;
    }

    private static byte[] EncodeVarint(ulong value)
    {
        var bytes = new List<byte>();
        do
        {
            var current = (byte)(value & 0x7F);
            value >>= 7;
            if (value != 0)
            {
                current |= 0x80;
            }

            bytes.Add(current);
        }
        while (value != 0);

        return [.. bytes];
    }

    private static byte[] DecodeBase58(string literal)
    {
        var zeroCount = literal.TakeWhile(static character => character == '1').Count();
        var bytes = new List<byte> { 0 };

        foreach (var character in literal)
        {
            if (!Base58Alphabet.TryGetValue(character, out var value))
            {
                throw new ArgumentException($"Invalid asset definition id `{literal}`.", nameof(literal));
            }

            var carry = value;
            for (var index = 0; index < bytes.Count; index++)
            {
                var total = bytes[index] * 58 + carry;
                bytes[index] = (byte)(total & 0xFF);
                carry = total >> 8;
            }

            while (carry > 0)
            {
                bytes.Add((byte)(carry & 0xFF));
                carry >>= 8;
            }
        }

        var decoded = new byte[zeroCount + bytes.Count];
        for (var index = 0; index < zeroCount; index++)
        {
            decoded[index] = 0;
        }

        for (var index = 0; index < bytes.Count; index++)
        {
            decoded[decoded.Length - 1 - index] = bytes[index];
        }

        return decoded;
    }

    private static byte[] DecodeFixedBytesLiteral(string literal, int expectedLength)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(literal);

        var normalized = literal.Trim();
        if (normalized.StartsWith("0x", StringComparison.OrdinalIgnoreCase))
        {
            normalized = normalized[2..];
        }

        byte[] bytes;
        try
        {
            bytes = Convert.FromHexString(normalized);
        }
        catch (FormatException exception)
        {
            throw new ArgumentException($"Invalid fixed-byte literal `{literal}`.", nameof(literal), exception);
        }

        if (bytes.Length != expectedLength)
        {
            throw new ArgumentException($"Invalid fixed-byte literal `{literal}`.", nameof(literal));
        }

        return bytes;
    }
}
