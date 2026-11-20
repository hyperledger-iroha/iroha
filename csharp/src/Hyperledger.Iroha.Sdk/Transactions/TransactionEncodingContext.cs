using System.Text;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Numeric;

namespace Hyperledger.Iroha.Transactions;

/// <summary>
/// Encodes validated transaction inputs in one explicit Norito layout: the compact layout (the
/// default, and the only layout a ledger selects today) or the dense layout.
/// </summary>
/// <remarks>
/// The transaction envelope shapes are written by <see cref="TransactionWireCodec"/>; this class
/// validates and converts caller inputs to the wire model.
/// </remarks>
internal sealed class TransactionEncodingContext
{
    private const byte AssetDefinitionVersion = 1;

    private static readonly Dictionary<CurveId, byte> PublicKeyAlgorithmTags = new()
    {
        [CurveId.Ed25519] = 0,
        [CurveId.Secp256k1] = 1,
        [CurveId.BlsNormal] = 2,
        [CurveId.BlsSmall] = 3,
        [CurveId.MlDsa] = 4,
        [CurveId.Gost256A] = 5,
        [CurveId.Gost256B] = 6,
        [CurveId.Gost256C] = 7,
        [CurveId.Gost512A] = 8,
        [CurveId.Gost512B] = 9,
        [CurveId.Sm2] = 10,
    };

    private static readonly Dictionary<char, int> Base58Alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
        .Select(static (character, index) => new KeyValuePair<char, int>(character, index))
        .ToDictionary();

    public TransactionEncodingContext(
        string authorityAccountId,
        NoritoWireLayout layout = NoritoWireLayout.Compact)
    {
        AuthorityAccountId = CanonicalizeAccountId(authorityAccountId);
        Layout = NoritoWireLayoutRules.RequireTyped(layout, nameof(layout));
    }

    public string AuthorityAccountId { get; }

    /// <summary>The layout every value this context encodes is written in.</summary>
    public NoritoWireLayout Layout { get; }

    /// <summary>A writer in this context's layout.</summary>
    internal CanonicalNoritoWriter NewWriter() => new(Layout);

    public byte[] EncodeNetworkDomain(NetworkId networkId)
    {
        ArgumentNullException.ThrowIfNull(networkId);
        return TransactionWireCodec.EncodeDomain(Layout, networkId.AsSpan());
    }

    public byte[] EncodeAccountId(string accountId)
    {
        return EncodeAccountController(accountId);
    }

    public byte[] EncodeAccountController(string accountId)
    {
        return TransactionWireCodec.EncodeAccountController(Layout, ToWireController(accountId));
    }

    /// <summary>The wire-level controller of a canonical, natively admitted account id.</summary>
    internal WireAccountController ToWireController(string accountId)
    {
        var parsed = AccountAddress.Parse(CanonicalizeAccountId(accountId));
        if (parsed.GetMultisigPolicy() is { } policy)
        {
            var members = new WireMultisigMember[policy.Members.Count];
            for (var index = 0; index < members.Length; index++)
            {
                var member = policy.Members[index];
                members[index] = new WireMultisigMember(
                    TagPublicKey(member.Curve, member.PublicKey),
                    member.Weight);
            }

            return new WireMultisigController(policy.Version, policy.Threshold, Array.AsReadOnly(members));
        }

        return new WireSingleKeyController(TagPublicKey(parsed.CurveIdentifier!.Value, parsed.PublicKey));
    }

    private static byte[] TagPublicKey(CurveId curve, ReadOnlySpan<byte> publicKey)
    {
        if (!PublicKeyAlgorithmTags.TryGetValue(curve, out var algorithmTag))
            throw new ArgumentException($"Unsupported account curve `{curve}` for managed Norito encoding.", nameof(curve));
        var compactKey = new byte[publicKey.Length + 1];
        compactKey[0] = algorithmTag;
        publicKey.CopyTo(compactKey.AsSpan(1));
        return compactKey;
    }

    public byte[] EncodeString(string value)
    {
        return TransactionWireCodec.EncodeString(Layout, value);
    }

    public byte[] EncodeName(string value)
    {
        NameNative.ValidateCanonical(value, nameof(value));
        return EncodeString(value);
    }

    internal static string CanonicalizeDomainId(string value, string paramName)
    {
        var exact = RequireExactNonBlank(value, paramName);
        DomainIdNative.ValidateCanonical(exact, paramName);
        return exact;
    }

    public byte[] EncodeDomainId(string value)
    {
        var exact = CanonicalizeDomainId(value, nameof(value));
        var separator = exact.IndexOf('.');
        var writer = NewWriter();
        writer.WriteField(EncodeName(exact[..separator]));
        writer.WriteField(EncodeName(exact[(separator + 1)..]));
        return writer.ToArray();
    }

    public byte[] EncodeOptionalString(string? value)
    {
        if (value is null)
        {
            return TransactionWireCodec.EncodeNone(Layout);
        }

        value = RequireExactNonBlank(value, nameof(value));
        return TransactionWireCodec.EncodeSome(Layout, EncodeString(value));
    }

    public byte[] EncodeJson(JsonNode? value)
    {
        return TransactionWireCodec.EncodeJsonText(Layout, CanonicalJson(value));
    }

    internal static string CanonicalJson(JsonNode? value) => WriteJsonNode(value);

    public byte[] EncodeUInt32(uint value)
    {
        return TransactionWireCodec.EncodeUInt32(Layout, value);
    }

    public byte[] EncodeUInt64(ulong value)
    {
        return TransactionWireCodec.EncodeUInt64(Layout, value);
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
        return value.HasValue
            ? TransactionWireCodec.EncodeSome(Layout, encoder(value.Value))
            : TransactionWireCodec.EncodeNone(Layout);
    }

    public byte[] EncodeEmptyMetadata()
    {
        return TransactionWireCodec.EncodeMetadata(Layout, []);
    }

    public byte[] EncodeMetadata(IReadOnlyDictionary<string, JsonNode?> metadata)
    {
        return TransactionWireCodec.EncodeMetadata(Layout, ToWireMetadata(metadata));
    }

    /// <summary>The wire-level metadata entries in key order, each value as canonical JSON text.</summary>
    internal static IReadOnlyList<WireMetadataEntry> ToWireMetadata(IReadOnlyDictionary<string, JsonNode?> metadata)
    {
        ArgumentNullException.ThrowIfNull(metadata);
        var orderedKeys = metadata.Keys.OrderBy(static key => key, StringComparer.Ordinal).ToArray();
        var entries = new WireMetadataEntry[orderedKeys.Length];
        for (var index = 0; index < orderedKeys.Length; index++)
        {
            entries[index] = new WireMetadataEntry(orderedKeys[index], CanonicalJson(metadata[orderedKeys[index]]));
        }

        return Array.AsReadOnly(entries);
    }

    public byte[] EncodeQuantity(NumericV1.QuantityValue value)
    {
        return TransactionWireCodec.EncodeNumeric(Layout, ToWireNumeric(value));
    }

    private static WireNumeric ToWireNumeric(NumericV1.QuantityValue value)
    {
        ArgumentNullException.ThrowIfNull(value);
        return new WireNumeric(value.Mantissa, checked((uint)value.Scale));
    }

    public byte[] EncodeConstVec(ReadOnlySpan<byte> bytes)
    {
        return TransactionWireCodec.EncodeConstVec(Layout, bytes);
    }

    public byte[] EncodeInstruction(TransactionInstruction instruction)
    {
        return TransactionWireCodec.EncodeInstruction(Layout, ToWireInstruction(instruction));
    }

    /// <summary>
    /// The instruction's envelope in this context's layout: its wire id and its complete Norito
    /// frame (compact) or its bare payload (dense).
    /// </summary>
    internal WireInstruction ToWireInstruction(TransactionInstruction instruction)
    {
        ArgumentNullException.ThrowIfNull(instruction);
        return new WireInstruction(instruction.WireId, instruction.EncodeEnvelopePayload(this));
    }

    public byte[] EncodeInstructionsExecutable(IReadOnlyList<TransactionInstruction> instructions)
    {
        return TransactionWireCodec.EncodeExecutable(Layout, ToWireInstructionsExecutable(instructions));
    }

    internal WireExecutable ToWireInstructionsExecutable(IReadOnlyList<TransactionInstruction> instructions)
    {
        ArgumentNullException.ThrowIfNull(instructions);
        var envelopes = new WireInstruction[instructions.Count];
        for (var index = 0; index < envelopes.Length; index++)
        {
            envelopes[index] = ToWireInstruction(instructions[index]);
        }

        return new WireInstructionsExecutable(Array.AsReadOnly(envelopes));
    }

    public byte[] EncodeExecutableBatch(IReadOnlyList<TransactionBatchEntry> entries)
    {
        return TransactionWireCodec.EncodeExecutable(Layout, ToWireBatchExecutable(entries));
    }

    internal WireExecutable ToWireBatchExecutable(IReadOnlyList<TransactionBatchEntry> entries)
    {
        ArgumentNullException.ThrowIfNull(entries);
        if (entries.Count == 0)
        {
            throw new ArgumentException("Executable batches must contain at least one item.", nameof(entries));
        }

        var items = new WireBatchItem[entries.Count];
        for (var index = 0; index < items.Length; index++)
        {
            items[index] = entries[index] switch
            {
                TransactionBatchEntry.InstructionEntry instruction =>
                    new WireBatchInstruction(ToWireInstruction(instruction.Value)),
                TransactionBatchEntry.ContractCallEntry call =>
                    new WireBatchContractCall(ToWireContractInvocation(call.Invocation)),
                _ => throw new ArgumentException("Unknown executable batch entry.", nameof(entries)),
            };
        }

        return new WireBatchExecutable(Array.AsReadOnly(items));
    }

    private static WireContractInvocation ToWireContractInvocation(TransactionContractInvocation invocation)
    {
        return new WireContractInvocation(
            invocation.ContractAddress,
            invocation.ExpectedCodeHashSpan.ToArray(),
            invocation.Entrypoint,
            invocation.HasArguments ? invocation.ArgumentsSpan.ToArray() : null);
    }

    public byte[] EncodeAssetId(string assetDefinitionId, string accountId, ulong? dataspaceId = null)
    {
        var writer = NewWriter();
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
        return TransactionWireCodec.EncodeFeePayment(Layout, ToWireFeePayment(feePayment));
    }

    /// <summary>The wire-level fee payment of a validated intent.</summary>
    internal WireFeePayment ToWireFeePayment(FeePaymentIntent feePayment)
    {
        ArgumentNullException.ThrowIfNull(feePayment);

        var limits = new WireFeeChargeLimit[feePayment.ChargeLimits.Count];
        for (var index = 0; index < limits.Length; index++)
        {
            limits[index] = ToWireFeeChargeLimit(feePayment.ChargeLimits[index]);
        }

        return feePayment switch
        {
            AuthorityFeePaymentIntent => new WireAuthorityFeePayment(Array.AsReadOnly(limits), feePayment.GasLimit),
            SponsorFeePaymentIntent sponsor => new WireSponsorFeePayment(
                ToWireController(sponsor.ProgramId.Sponsor),
                RequireCanonicalName(sponsor.ProgramId.Name),
                sponsor.ProgramRevision,
                Array.AsReadOnly(limits),
                feePayment.GasLimit),
            _ => throw new ArgumentException("Unknown fee payment intent subtype.", nameof(feePayment)),
        };
    }

    private static string RequireCanonicalName(string value)
    {
        NameNative.ValidateCanonical(value, nameof(value));
        return value;
    }

    internal static string CanonicalizeNftId(string nftId, string paramName)
    {
        var exact = RequireExactNonBlank(nftId, paramName);
        var separator = exact.IndexOf('$');
        if (separator <= 0 || separator != exact.LastIndexOf('$') || separator == exact.Length - 1)
            throw new ArgumentException("NFT ID must use exact name$domain.dataspace spelling.", paramName);
        CanonicalizeDomainId(exact[(separator + 1)..], paramName);
        return exact;
    }

    public byte[] EncodeNftId(string nftId)
    {
        var exact = CanonicalizeNftId(nftId, nameof(nftId));
        var separator = exact.IndexOf('$');
        var writer = NewWriter();
        writer.WriteField(EncodeDomainId(exact[(separator + 1)..]));
        writer.WriteField(EncodeName(exact[..separator]));
        return writer.ToArray();
    }

    public byte[] EncodeTriggerId(string triggerId)
    {
        var writer = NewWriter();
        writer.WriteField(EncodeName(triggerId));
        return writer.ToArray();
    }

    public void EnsureAuthorityMatchesPrivateKey(ReadOnlySpan<byte> privateKeySeed)
    {
        var publicKey = Ed25519Signer.GetPublicKey(privateKeySeed);
        var authority = AccountAddress.Parse(AuthorityAccountId);
        if (authority.CurveIdentifier != CurveId.Ed25519
            || !authority.PublicKey.AsSpan().SequenceEqual(publicKey))
        {
            throw new InvalidOperationException(
                $"The signing key does not control transaction authority `{AuthorityAccountId}`.");
        }
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
        var uuidBytes = payload.AsSpan(1, TransactionWireCodec.AssetDefinitionIdLength);

        var writer = NewWriter();
        writer.WriteByteElements(uuidBytes);

        return writer.ToArray();
    }

    private byte[] EncodeAssetBalanceScope(ulong? dataspaceId)
    {
        var writer = NewWriter();
        if (!dataspaceId.HasValue)
        {
            writer.WriteEnumTag(0);
            return writer.ToArray();
        }

        writer.WriteEnumTag(1);
        var dataspaceWriter = NewWriter();
        dataspaceWriter.WriteField(EncodeUInt64(dataspaceId.Value));
        writer.WriteField(dataspaceWriter.ToArray());
        return writer.ToArray();
    }

    internal static string CanonicalizeAccountId(string accountId, string paramName = "accountId")
    {
        var exact = RequireExactNonBlank(accountId, paramName);
        try
        {
            _ = AccountAddress.Parse(exact);
            return exact;
        }
        catch (AccountAddressException exception)
        {
            throw new ArgumentException("Account id must be a canonical I105 account id.", paramName, exception);
        }
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

    private static WireFeeChargeLimit ToWireFeeChargeLimit(FeeChargeLimit limit)
    {
        var kind = limit.Kind switch
        {
            FeeChargeKind.Nexus => 0u,
            FeeChargeKind.PipelineGas => 1u,
            _ => throw new ArgumentOutOfRangeException(nameof(limit), "Unknown fee charge kind."),
        };

        var exactLiteral = CanonicalizeAssetDefinitionId(limit.AssetDefinitionId, nameof(limit));
        var assetDefinitionId = DecodeBase58(exactLiteral, nameof(limit))
            .AsSpan(1, TransactionWireCodec.AssetDefinitionIdLength)
            .ToArray();

        return new WireFeeChargeLimit(
            kind,
            assetDefinitionId,
            ToWireNumeric(NumericV1.QuantityValue.ParseCanonical(limit.MaxAmount)));
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
}
