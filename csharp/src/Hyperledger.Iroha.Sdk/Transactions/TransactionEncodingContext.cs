using System.Text;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Numeric;

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
        var bytes = Encoding.UTF8.GetBytes(RequireCanonicalChainId(chainId, nameof(chainId)));
        var writer = new OfflineNoritoWriter();
        writer.WriteCompactLength((ulong)bytes.Length);
        writer.WriteBytes(bytes);
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
        var parsed = AccountAddress.Parse(CanonicalizeDefaultAccountId(accountId), DefaultNetworkPrefix);
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
        return EncodeString(RequireExactNonBlank(value, nameof(value)));
    }

    public byte[] EncodeOptionalString(string? value)
    {
        var writer = new OfflineNoritoWriter();
        if (value is null)
        {
            writer.WriteByte(0);
            return writer.ToArray();
        }

        value = RequireExactNonBlank(value, nameof(value));
        writer.WriteByte(1);
        writer.WriteField(EncodeString(value));
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

    public byte[] EncodeQuantity(NumericV1.QuantityValue value)
    {
        ArgumentNullException.ThrowIfNull(value);
        var mantissaBytes = value.Mantissa.IsZero
            ? Array.Empty<byte>()
            : value.Mantissa.ToByteArray(isUnsigned: false, isBigEndian: false);

        var mantissa = new OfflineNoritoWriter();
        mantissa.WriteUInt32LittleEndian((uint)mantissaBytes.Length);
        mantissa.WriteBytes(mantissaBytes);

        var writer = new OfflineNoritoWriter();
        writer.WriteField(mantissa.ToArray());
        writer.WriteField(EncodeUInt32((uint)value.Scale));
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
        var framedInstruction = instruction.EncodeFramedPayload(this);

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

    public byte[] EncodeExecutableBatch(IReadOnlyList<TransactionBatchEntry> entries)
    {
        if (entries.Count == 0)
        {
            throw new ArgumentException("Executable batches must contain at least one item.", nameof(entries));
        }

        var sequence = new OfflineNoritoWriter();
        sequence.WriteLength((ulong)entries.Count);
        foreach (var entry in entries)
        {
            var item = new OfflineNoritoWriter();
            switch (entry)
            {
                case TransactionBatchEntry.InstructionEntry instruction:
                    item.WriteUInt32LittleEndian(0);
                    item.WriteField(EncodeInstruction(instruction.Value));
                    break;
                case TransactionBatchEntry.ContractCallEntry call:
                    item.WriteUInt32LittleEndian(1);
                    item.WriteField(EncodeContractInvocation(call.Invocation));
                    break;
                default:
                    throw new ArgumentException("Unknown executable batch entry.", nameof(entries));
            }
            sequence.WriteField(item.ToArray());
        }

        var executable = new OfflineNoritoWriter();
        executable.WriteUInt32LittleEndian(4);
        executable.WriteField(sequence.ToArray());
        return executable.ToArray();
    }

    private byte[] EncodeContractInvocation(TransactionContractInvocation invocation)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(EncodeString(invocation.ContractAddress));
        writer.WriteField(invocation.ExpectedCodeHashSpan);
        writer.WriteField(EncodeString(invocation.Entrypoint));
        var arguments = new OfflineNoritoWriter();
        if (invocation.HasArguments)
        {
            arguments.WriteByte(1);
            arguments.WriteField(EncodeBytesVec(invocation.ArgumentsSpan));
        }
        else
        {
            arguments.WriteByte(0);
        }
        writer.WriteField(arguments.ToArray());
        return writer.ToArray();
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

    public byte[] EncodeFeePaymentIntent(FeePaymentIntent feePayment)
    {
        ArgumentNullException.ThrowIfNull(feePayment);

        var writer = new OfflineNoritoWriter();
        writer.WriteUInt32LittleEndian(feePayment.PayerTag);
        writer.WriteField(feePayment switch
        {
            AuthorityFeePaymentIntent authority => EncodeAuthorityFeePayment(authority),
            SponsorFeePaymentIntent sponsor => EncodeSponsorFeePayment(sponsor),
            _ => throw new ArgumentException("Unknown fee payment intent subtype.", nameof(feePayment)),
        });
        return writer.ToArray();
    }

    public byte[] EncodeNftId(string nftId)
    {
        var exactNftId = RequireExactNonBlank(nftId, nameof(nftId));

        var separatorIndex = exactNftId.IndexOf('$');
        if (separatorIndex <= 0 || separatorIndex != exactNftId.LastIndexOf('$') || separatorIndex == exactNftId.Length - 1)
        {
            throw new ArgumentException($"Invalid NFT id `{nftId}`.", nameof(nftId));
        }

        var writer = new OfflineNoritoWriter();
        writer.WriteField(EncodeName(exactNftId[(separatorIndex + 1)..]));
        writer.WriteField(EncodeName(exactNftId[..separatorIndex]));
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
                    var keyNode = JsonValue.Create(pair.Key)
                        ?? throw new InvalidOperationException("JSON object key must not be null.");
                    builder.Append(keyNode.ToJsonString());
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

    internal static string CanonicalizeAssetDefinitionId(
        string literal,
        string paramName = "assetDefinitionId")
    {
        var exactLiteral = RequireExactNonBlank(literal, paramName);
        if (exactLiteral.IndexOfAny([':', '#', '@', '$']) >= 0)
        {
            throw new ArgumentException($"Invalid asset definition id `{literal}`.", paramName);
        }

        var payload = DecodeBase58(exactLiteral, paramName);
        if (payload.Length != 21 || payload[0] != AssetDefinitionVersion)
        {
            throw new ArgumentException($"Invalid asset definition id `{literal}`.", paramName);
        }

        var uuidBytes = payload.AsSpan(1, 16);
        if ((uuidBytes[6] >> 4) != 0x4 || (uuidBytes[8] & 0xC0) != 0x80)
        {
            throw new ArgumentException($"Invalid asset definition id `{literal}`.", paramName);
        }

        return exactLiteral;
    }

    private byte[] EncodeAssetDefinitionAddress(string literal)
    {
        var exactLiteral = CanonicalizeAssetDefinitionId(literal, nameof(literal));
        var payload = DecodeBase58(exactLiteral, nameof(literal));
        var uuidBytes = payload.AsSpan(1, 16);

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

    internal static string CanonicalizeAccountId(string accountId, string paramName = "accountId")
    {
        var exact = RequireExactNonBlank(accountId, paramName);
        try
        {
            return AccountAddress.Parse(exact, DefaultNetworkPrefix).ToI105(DefaultNetworkPrefix);
        }
        catch (AccountAddressException exception)
        {
            throw new ArgumentException("Account id must be a canonical I105 account id.", paramName, exception);
        }
    }

    private static string CanonicalizeDefaultAccountId(string accountId)
    {
        try
        {
            return AccountAddress.Parse(
                RequireExactNonBlank(accountId, nameof(accountId)),
                DefaultNetworkPrefix).ToI105(DefaultNetworkPrefix);
        }
        catch (AccountAddressException exception)
        {
            throw new ArgumentException("Account id must be a canonical I105 account id.", nameof(accountId), exception);
        }
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

    private static byte[] DecodeBase58(string literal, string paramName)
    {
        var zeroCount = literal.TakeWhile(static character => character == '1').Count();
        var bytes = new List<byte> { 0 };

        foreach (var character in literal)
        {
            if (!Base58Alphabet.TryGetValue(character, out var value))
            {
                throw new ArgumentException($"Invalid asset definition id `{literal}`.", paramName);
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

    private byte[] EncodeAuthorityFeePayment(AuthorityFeePaymentIntent payment)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(EncodeFeeChargeLimits(payment.ChargeLimits));
        writer.WriteField(EncodeOption(payment.GasLimit, EncodeUInt64));
        return writer.ToArray();
    }

    private byte[] EncodeSponsorFeePayment(SponsorFeePaymentIntent payment)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(EncodeFeeSponsorProgramId(payment.ProgramId));
        writer.WriteField(EncodeUInt64(payment.ProgramRevision));
        writer.WriteField(EncodeFeeChargeLimits(payment.ChargeLimits));
        writer.WriteField(EncodeOption(payment.GasLimit, EncodeUInt64));
        return writer.ToArray();
    }

    private byte[] EncodeFeeSponsorProgramId(FeeSponsorProgramId programId)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(EncodeAccountId(programId.Sponsor));
        writer.WriteField(EncodeName(programId.Name));
        return writer.ToArray();
    }

    private byte[] EncodeFeeChargeLimits(IReadOnlyList<FeeChargeLimit> limits)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteLength((ulong)limits.Count);
        foreach (var limit in limits)
        {
            writer.WriteField(EncodeFeeChargeLimit(limit));
        }
        return writer.ToArray();
    }

    private byte[] EncodeFeeChargeLimit(FeeChargeLimit limit)
    {
        var kind = new OfflineNoritoWriter();
        kind.WriteUInt32LittleEndian(limit.Kind switch
        {
            FeeChargeKind.Nexus => 0,
            FeeChargeKind.PipelineGas => 1,
            _ => throw new ArgumentOutOfRangeException(nameof(limit), "Unknown fee charge kind."),
        });

        var writer = new OfflineNoritoWriter();
        writer.WriteField(kind.ToArray());
        writer.WriteField(EncodeAssetDefinitionId(limit.AssetDefinitionId));
        writer.WriteField(EncodeQuantity(NumericV1.QuantityValue.ParseCanonical(limit.MaxAmount)));
        return writer.ToArray();
    }

    private static byte[] DecodeFixedBytesLiteral(string literal, int expectedLength)
    {
        var normalized = RequireExactNonBlank(literal, nameof(literal));

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

    private static string RequireExactNonBlank(string? value, string paramName)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException("Value cannot be null or whitespace.", paramName);
        }
        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException("Value must not contain surrounding whitespace.", paramName);
        }
        if (value.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Value must not contain whitespace.", paramName);
        }
        if (value.Any(char.IsControl))
        {
            throw new ArgumentException("Value must not contain control characters.", paramName);
        }
        return value;
    }

    private static string RequireCanonicalChainId(string? value, string paramName)
    {
        const int maxChainIdBytes = 128;
        if (string.IsNullOrEmpty(value) || value.Length > maxChainIdBytes)
        {
            throw new ArgumentException(
                $"Chain ID must contain 1..{maxChainIdBytes} ASCII bytes.",
                paramName);
        }
        if (!IsAsciiLetterOrDigit(value[0]) || !IsAsciiLetterOrDigit(value[^1]))
        {
            throw new ArgumentException(
                "Chain ID must begin and end with an ASCII alphanumeric character.",
                paramName);
        }
        if (value.Any(character =>
                !IsAsciiLetterOrDigit(character)
                && character is not ('.' or '_' or ':' or '-')))
        {
            throw new ArgumentException("Chain ID contains a non-canonical character.", paramName);
        }
        return value;
    }

    private static bool IsAsciiLetterOrDigit(char value)
    {
        return value is (>= 'a' and <= 'z')
            or (>= 'A' and <= 'Z')
            or (>= '0' and <= '9');
    }
}
