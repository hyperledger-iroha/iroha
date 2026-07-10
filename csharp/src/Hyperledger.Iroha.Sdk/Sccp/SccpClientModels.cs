using System.Buffers;
using System.Buffers.Binary;
using System.Text.Json;
using System.Text.Json.Serialization;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Sccp;

/// <summary>Closed bridge payload kinds admitted in SCCP V1.</summary>
public enum SccpPayloadKindV1
{
    AssetRegister,
    RouteActivate,
    Transfer,
    TokenAdd,
    TokenPause,
    TokenResume,
}

public static class SccpPayloadKindV1Extensions
{
    public static string WireKey(this SccpPayloadKindV1 kind) => kind switch
    {
        SccpPayloadKindV1.AssetRegister => "asset_register",
        SccpPayloadKindV1.RouteActivate => "route_activate",
        SccpPayloadKindV1.Transfer => "transfer",
        SccpPayloadKindV1.TokenAdd => "token_add",
        SccpPayloadKindV1.TokenPause => "token_pause",
        SccpPayloadKindV1.TokenResume => "token_resume",
        _ => throw new ArgumentOutOfRangeException(nameof(kind)),
    };

    public static SccpPayloadKindV1 ParseWireKey(string value)
    {
        ArgumentNullException.ThrowIfNull(value);
        foreach (var candidate in Enum.GetValues<SccpPayloadKindV1>())
        {
            if (string.Equals(candidate.WireKey(), value, StringComparison.Ordinal))
            {
                return candidate;
            }
        }

        throw new ArgumentException("SCCP payload kind is unknown or retired.", nameof(value));
    }
}

/// <summary>Request payload for <c>POST /v1/bridge/proofs/submit</c>.</summary>
public sealed class SccpBridgeProofSubmitRequest
{
    public SccpBridgeProofSubmitRequest(
        string authority,
        string messageBundleBase64,
        string? publicKeyHex = null,
        string? signatureBase64 = null,
        string? networkIdHex = null,
        string? verifierAddressHex = null,
        string? bridgeAddressHex = null,
        string? verifierCodeHashHex = null,
        string? verifierKeyHashHex = null,
        string? tronVerifierAddress = null,
        string? proofBytesHex = null,
        ulong? creationTimeMs = null)
    {
        Authority = SccpSubmitValidation.Authority(authority);
        (PublicKeyHex, SignatureBase64) = SccpSubmitValidation.DetachedSigner(
            publicKeyHex,
            signatureBase64,
            Authority);
        SccpSubmitValidation.CanonicalNoritoBase64(messageBundleBase64, "message_bundle_b64");
        MessageBundleBase64 = messageBundleBase64;
        NetworkIdHex = SccpSubmitValidation.OptionalHex(networkIdHex, 32, "network_id_hex");
        VerifierAddressHex = SccpSubmitValidation.OptionalHex(verifierAddressHex, 20, "verifier_address_hex");
        BridgeAddressHex = SccpSubmitValidation.OptionalHex(bridgeAddressHex, 20, "bridge_address_hex");
        VerifierCodeHashHex = SccpSubmitValidation.OptionalHex(verifierCodeHashHex, 32, "verifier_code_hash_hex");
        VerifierKeyHashHex = SccpSubmitValidation.OptionalHex(verifierKeyHashHex, 32, "verifier_key_hash_hex");
        TronVerifierAddress = SccpSubmitValidation.OptionalText(tronVerifierAddress, 128, "tron_verifier_address");
        ProofBytesHex = SccpSubmitValidation.OptionalProofHex(proofBytesHex);
        if (creationTimeMs == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(creationTimeMs), "creation_time_ms must be positive.");
        }

        CreationTimeMs = creationTimeMs;
        var evm = VerifierAddressHex is not null || BridgeAddressHex is not null;
        var tron = TronVerifierAddress is not null;
        var destination = NetworkIdHex is not null || evm || tron
            || VerifierCodeHashHex is not null || VerifierKeyHashHex is not null;
        if ((ProofBytesHex is not null) != destination)
        {
            throw new ArgumentException(
                "proof_bytes_hex and complete destination material must be supplied together.");
        }

        if (destination
            && (evm == tron || NetworkIdHex is null || VerifierCodeHashHex is null || VerifierKeyHashHex is null))
        {
            throw new ArgumentException(
                "Destination material must select exactly one complete EVM or TRON family.");
        }

        if (evm && (VerifierAddressHex is null || BridgeAddressHex is null))
        {
            throw new ArgumentException("Complete EVM SCCP destination material is required.");
        }
    }

    [JsonPropertyName("authority")]
    public string Authority { get; }

    [JsonPropertyName("public_key_hex")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? PublicKeyHex { get; }

    [JsonPropertyName("signature_b64")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? SignatureBase64 { get; }

    [JsonPropertyName("message_bundle_b64")]
    public string MessageBundleBase64 { get; }

    [JsonPropertyName("network_id_hex")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? NetworkIdHex { get; }

    [JsonPropertyName("verifier_address_hex")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? VerifierAddressHex { get; }

    [JsonPropertyName("bridge_address_hex")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? BridgeAddressHex { get; }

    [JsonPropertyName("verifier_code_hash_hex")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? VerifierCodeHashHex { get; }

    [JsonPropertyName("verifier_key_hash_hex")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? VerifierKeyHashHex { get; }

    [JsonPropertyName("tron_verifier_address")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? TronVerifierAddress { get; }

    [JsonPropertyName("proof_bytes_hex")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? ProofBytesHex { get; }

    [JsonPropertyName("creation_time_ms")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public ulong? CreationTimeMs { get; }
}

/// <summary>Native-proof-only request for <c>POST /v1/bridge/messages</c>.</summary>
public sealed class SccpBridgeMessageSubmitRequest
{
    public SccpBridgeMessageSubmitRequest(
        string authority,
        string nativeProofBase64,
        string? publicKeyHex = null,
        string? signatureBase64 = null,
        ulong? creationTimeMs = null)
    {
        Authority = SccpSubmitValidation.Authority(authority);
        (PublicKeyHex, SignatureBase64) = SccpSubmitValidation.DetachedSigner(
            publicKeyHex,
            signatureBase64,
            Authority);
        SccpSubmitValidation.CanonicalNoritoBase64(nativeProofBase64, "native_proof_b64");
        NativeProofBase64 = nativeProofBase64;
        if (creationTimeMs == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(creationTimeMs), "creation_time_ms must be positive.");
        }

        CreationTimeMs = creationTimeMs;
    }

    [JsonPropertyName("authority")]
    public string Authority { get; }

    [JsonPropertyName("public_key_hex")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? PublicKeyHex { get; }

    [JsonPropertyName("signature_b64")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? SignatureBase64 { get; }

    [JsonPropertyName("native_proof_b64")]
    public string NativeProofBase64 { get; }

    [JsonPropertyName("creation_time_ms")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public ulong? CreationTimeMs { get; }
}

/// <summary>Optional request-bound checks for a bridge response.</summary>
public sealed record SccpBridgeResponseExpectation(
    SccpPayloadKindV1? PayloadKind = null,
    string? MessageIdHex = null,
    uint? CounterpartyDomain = null,
    SccpNetworkV1? CounterpartyChain = null,
    ulong? CreationTimeMs = null)
{
    public void Validate()
    {
        if (MessageIdHex is not null)
        {
            SccpSubmitValidation.ResponseHash(MessageIdHex, nameof(MessageIdHex));
        }

        if (CounterpartyDomain is not null and (0 or > 5))
        {
            throw new ArgumentOutOfRangeException(nameof(CounterpartyDomain));
        }

        if (CounterpartyChain is { } chain && !chain.IsExternal())
        {
            throw new ArgumentException("Expected counterparty chain must be external.", nameof(CounterpartyChain));
        }

        if (CounterpartyDomain is { } domain && CounterpartyChain is { } profile
            && domain != profile.DomainId())
        {
            throw new ArgumentException("Expected SCCP counterparty profile/domain mismatch.");
        }

        if (CreationTimeMs == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(CreationTimeMs));
        }
    }
}

/// <summary>Exact unified two-phase response from either SCCP submit endpoint.</summary>
public sealed record SccpBridgeSubmitResponse(
    bool Submitted,
    SccpPayloadKindV1 PayloadKind,
    string MessageIdHex,
    string Backend,
    uint CounterpartyDomain,
    SccpNetworkV1 CounterpartyChain,
    string ManifestHashHex,
    ulong RangeStartHeight,
    ulong RangeEndHeight,
    ulong CreationTimeMs,
    string? TxHashHex,
    string? TransactionPayloadBase64,
    string? SigningMessageBase64)
{
    private static readonly HashSet<string> Fields =
    [
        "submitted", "payload_kind", "message_id_hex", "backend", "counterparty_domain",
        "counterparty_chain", "manifest_hash_hex", "range_start_height", "range_end_height",
        "creation_time_ms", "tx_hash_hex", "transaction_payload_b64", "signing_message_b64",
    ];

    public static SccpBridgeSubmitResponse Parse(
        ReadOnlyMemory<byte> json,
        SccpBridgeResponseExpectation? expectation = null)
    {
        using var document = SccpJson.Parse(json, "bridge submit response");
        var root = document.RootElement;
        SccpJson.ExactFields(root, Fields, "bridge submit response");
        var submitted = SccpJson.Boolean(root, "submitted");
        var payloadKind = SccpPayloadKindV1Extensions.ParseWireKey(SccpJson.Text(root, "payload_kind"));
        var messageId = SccpSubmitValidation.ResponseHash(SccpJson.Text(root, "message_id_hex"), "message_id_hex");
        var backend = SccpJson.Text(root, "backend");
        if (backend.Length > 128 || !System.Text.RegularExpressions.Regex.IsMatch(
                backend,
                "^bridge/[a-z0-9/_-]+$",
                System.Text.RegularExpressions.RegexOptions.CultureInvariant))
        {
            throw new ArgumentException("backend must be a canonical bridge backend label.");
        }

        var domain = SccpJson.UInt32(root, "counterparty_domain", 1, 5);
        var chain = SccpNetworkV1Extensions.ParseProfileKey(SccpJson.Text(root, "counterparty_chain"));
        if (!chain.IsExternal() || chain.DomainId() != domain)
        {
            throw new ArgumentException(
                "counterparty_chain and counterparty_domain must identify one exact external network.");
        }

        var manifest = SccpSubmitValidation.ResponseHash(SccpJson.Text(root, "manifest_hash_hex"), "manifest_hash_hex");
        var start = SccpJson.UInt64(root, "range_start_height", 1);
        var end = SccpJson.UInt64(root, "range_end_height", start);
        var creation = SccpJson.UInt64(root, "creation_time_ms", 1);
        var txHash = SccpJson.OptionalText(root, "tx_hash_hex");
        if (txHash is not null)
        {
            txHash = SccpSubmitValidation.ResponseHash(txHash, "tx_hash_hex");
        }

        var payloadBase64 = SccpJson.OptionalText(root, "transaction_payload_b64");
        var signingBase64 = SccpJson.OptionalText(root, "signing_message_b64");
        if (submitted)
        {
            if (txHash is null || payloadBase64 is not null || signingBase64 is not null)
            {
                throw new ArgumentException(
                    "Submitted SCCP response must contain tx_hash_hex and no signing payload.");
            }
        }
        else
        {
            if (txHash is not null || payloadBase64 is null || signingBase64 is null)
            {
                throw new ArgumentException(
                    "Prepared SCCP response requires transaction_payload_b64 and signing_message_b64.");
            }

            var payload = SccpSubmitValidation.CanonicalBase64(
                payloadBase64,
                "transaction_payload_b64",
                maximumBytes: SccpSubmitValidation.MaximumArtifactBytes);
            var signing = SccpSubmitValidation.CanonicalBase64(
                signingBase64,
                "signing_message_b64",
                exactBytes: IrohaHash.Length);
            if (!signing.AsSpan().SequenceEqual(IrohaHash.Hash(payload)))
            {
                throw new ArgumentException(
                    "signing_message_b64 must be the exact transaction-payload prehash.");
            }
        }

        var response = new SccpBridgeSubmitResponse(
            submitted,
            payloadKind,
            messageId,
            backend,
            domain,
            chain,
            manifest,
            start,
            end,
            creation,
            txHash,
            payloadBase64,
            signingBase64);
        response.RequireExpectation(expectation);
        return response;
    }

    private void RequireExpectation(SccpBridgeResponseExpectation? expectation)
    {
        if (expectation is null)
        {
            return;
        }

        expectation.Validate();
        if (expectation.PayloadKind is { } payloadKind && payloadKind != PayloadKind
            || expectation.MessageIdHex is { } messageId && messageId != MessageIdHex
            || expectation.CounterpartyDomain is { } domain && domain != CounterpartyDomain
            || expectation.CounterpartyChain is { } chain && chain != CounterpartyChain
            || expectation.CreationTimeMs is { } creation && creation != CreationTimeMs)
        {
            throw new ArgumentException("Bridge submit response does not match its request expectation.");
        }
    }
}

internal static class SccpSubmitValidation
{
    internal const int MaximumArtifactBytes = 16 * 1024 * 1024;

    internal static string Authority(string value)
    {
        ArgumentNullException.ThrowIfNull(value);
        AccountAddress address;
        try
        {
            address = AccountAddress.Parse(value, AccountAddress.DefaultChainDiscriminant);
        }
        catch (Exception error) when (error is ArgumentException or FormatException)
        {
            throw new ArgumentException("authority must be a canonical AccountId.", nameof(value), error);
        }

        if (!string.Equals(address.ToI105(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException("authority must be a canonical AccountId.", nameof(value));
        }

        return value;
    }

    internal static (string? PublicKey, string? Signature) DetachedSigner(
        string? publicKeyHex,
        string? signatureBase64,
        string authority)
    {
        if ((publicKeyHex is null) != (signatureBase64 is null))
        {
            throw new ArgumentException("public_key_hex and signature_b64 must be supplied together.");
        }

        if (publicKeyHex is null || signatureBase64 is null)
        {
            return (null, null);
        }

        if (publicKeyHex.Length != 64 || publicKeyHex.Any(static item =>
                !char.IsAsciiDigit(item) && item is not (>= 'a' and <= 'f')))
        {
            throw new ArgumentException("public_key_hex must be canonical lowercase Ed25519 hex.");
        }

        var publicKey = Convert.FromHexString(publicKeyHex);
        if (!CanonicalEd25519Point(publicKey))
        {
            throw new ArgumentException("public_key_hex must be one canonical Ed25519 public key.");
        }

        var address = AccountAddress.Parse(authority, AccountAddress.DefaultChainDiscriminant);
        if (!string.Equals(address.Algorithm, "ed25519", StringComparison.Ordinal)
            || !address.PublicKey.AsSpan().SequenceEqual(publicKey))
        {
            throw new ArgumentException("public_key_hex does not match authority.");
        }

        var signature = CanonicalBase64(signatureBase64, "signature_b64", exactBytes: 64);
        if (!CanonicalEd25519Point(signature.AsSpan(0, 32))
            || !CanonicalEd25519Scalar(signature.AsSpan(32, 32)))
        {
            throw new ArgumentException("signature_b64 must contain one canonical Ed25519 signature.");
        }

        return (publicKeyHex, signatureBase64);
    }

    internal static byte[] CanonicalNoritoBase64(string value, string field)
    {
        var archive = CanonicalBase64(value, field, maximumBytes: MaximumArtifactBytes);
        if (archive.Length < NoritoHeader.EncodedLength
            || !archive.AsSpan(0, 4).SequenceEqual("NRT0"u8)
            || archive[4] != 0 || archive[5] != 0
            || archive.AsSpan(6, 16).IndexOfAnyExcept((byte)0) < 0
            || archive[22] != (byte)NoritoCompression.None)
        {
            throw new ArgumentException($"{field} must contain one canonical uncompressed Norito envelope.");
        }

        var payloadLength = BinaryPrimitives.ReadUInt64LittleEndian(archive.AsSpan(23, 8));
        if (payloadLength > int.MaxValue || payloadLength > (ulong)(archive.Length - NoritoHeader.EncodedLength))
        {
            throw new ArgumentException($"{field} contains an invalid Norito payload length.");
        }

        var padding = archive.Length - NoritoHeader.EncodedLength - (int)payloadLength;
        if (padding is not (0 or 8)
            || archive.AsSpan(NoritoHeader.EncodedLength, padding).IndexOfAnyExcept((byte)0) >= 0)
        {
            throw new ArgumentException($"{field} must use canonical Norito header alignment.");
        }

        var payload = archive.AsSpan(NoritoHeader.EncodedLength + padding, (int)payloadLength);
        var checksum = BinaryPrimitives.ReadUInt64LittleEndian(archive.AsSpan(31, 8));
        if (Crc64Ecma.Compute(payload) != checksum)
        {
            throw new ArgumentException($"{field} has an invalid Norito checksum.");
        }

        return archive;
    }

    internal static byte[] CanonicalBase64(
        string value,
        string field,
        int? exactBytes = null,
        int maximumBytes = MaximumArtifactBytes)
    {
        ArgumentNullException.ThrowIfNull(value);
        byte[] decoded;
        try
        {
            decoded = Convert.FromBase64String(value);
        }
        catch (FormatException error)
        {
            throw new ArgumentException($"{field} must be canonical padded base64.", field, error);
        }

        if (value.Length == 0 || value != value.Trim() || decoded.Length == 0
            || decoded.Length > maximumBytes
            || !string.Equals(Convert.ToBase64String(decoded), value, StringComparison.Ordinal))
        {
            throw new ArgumentException($"{field} must be canonical nonempty padded base64.", field);
        }

        if (exactBytes is { } expected && decoded.Length != expected)
        {
            throw new ArgumentException($"{field} must contain exactly {expected} bytes.", field);
        }

        return decoded;
    }

    internal static string? OptionalHex(string? value, int bytes, string field)
    {
        if (value is null)
        {
            return null;
        }

        if (value.Length != 2 + bytes * 2 || !value.StartsWith("0x", StringComparison.Ordinal)
            || value.AsSpan(2).ContainsAnyExcept("0123456789abcdef")
            || value.AsSpan(2).IndexOfAnyExcept('0') < 0)
        {
            throw new ArgumentException(
                $"{field} must be canonical lowercase nonzero 0x-prefixed {bytes}-byte hex.",
                field);
        }

        return value;
    }

    internal static string ResponseHash(string value, string field)
    {
        ArgumentNullException.ThrowIfNull(value);
        if (value.Length != 64 || value.AsSpan().ContainsAnyExcept("0123456789abcdef")
            || value.AsSpan().IndexOfAnyExcept('0') < 0)
        {
            throw new ArgumentException(
                $"{field} must be canonical lowercase nonzero 32-byte hex.",
                field);
        }

        return value;
    }

    internal static string? OptionalText(string? value, int maximumBytes, string field)
    {
        if (value is null)
        {
            return null;
        }

        if (value.Length == 0 || value != value.Trim()
            || System.Text.Encoding.UTF8.GetByteCount(value) > maximumBytes)
        {
            throw new ArgumentException($"{field} must be canonical nonempty text.", field);
        }

        return value;
    }

    internal static string? OptionalProofHex(string? value)
    {
        if (value is null)
        {
            return null;
        }

        if (value.Length != 768 || value.AsSpan().ContainsAnyExcept("0123456789abcdef")
            || value.AsSpan().IndexOfAnyExcept('0') < 0)
        {
            throw new ArgumentException(
                "proof_bytes_hex must be canonical lowercase nonzero 384-byte hex.",
                nameof(value));
        }

        var proof = Convert.FromHexString(value);
        if (proof.AsSpan(0, 31).IndexOfAnyExcept((byte)0) >= 0 || proof[31] != 1
            || proof.AsSpan(32, 32).IndexOfAnyExcept((byte)0) < 0
            || proof.AsSpan(64, 32).IndexOfAnyExcept((byte)0) >= 0
            || proof.AsSpan(96, 32).IndexOfAnyExcept((byte)0) < 0)
        {
            throw new ArgumentException("proof_bytes_hex has invalid SCCP public inputs.", nameof(value));
        }

        return value;
    }

    private static bool CanonicalEd25519Scalar(ReadOnlySpan<byte> value)
    {
        ReadOnlySpan<byte> order =
        [
            0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58,
            0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x10,
        ];
        for (var index = 31; index >= 0; index--)
        {
            if (value[index] < order[index])
            {
                return true;
            }

            if (value[index] > order[index])
            {
                return false;
            }
        }

        return false;
    }

    private static bool CanonicalEd25519Point(ReadOnlySpan<byte> value)
    {
        if (value.Length != 32)
        {
            return false;
        }

        Span<byte> y = stackalloc byte[32];
        value.CopyTo(y);
        y[31] &= 0x7f;
        ReadOnlySpan<byte> prime =
        [
            0xed, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f,
        ];
        var less = false;
        for (var index = 31; index >= 0; index--)
        {
            if (y[index] < prime[index])
            {
                less = true;
                break;
            }

            if (y[index] > prime[index])
            {
                return false;
            }
        }

        if (!less)
        {
            return false;
        }

        string[] smallOrder =
        [
            "0000000000000000000000000000000000000000000000000000000000000000",
            "0100000000000000000000000000000000000000000000000000000000000000",
            "ecffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
            "0000000000000000000000000000000000000000000000000000000000000080",
        ];
        var hex = Convert.ToHexString(value).ToLowerInvariant();
        return !smallOrder.Contains(hex, StringComparer.Ordinal);
    }
}

internal static class SccpJson
{
    internal static JsonDocument Parse(ReadOnlyMemory<byte> json, string label)
    {
        try
        {
            var reader = new Utf8JsonReader(json.Span, new JsonReaderOptions
            {
                AllowTrailingCommas = false,
                CommentHandling = JsonCommentHandling.Disallow,
                MaxDepth = 128,
            });
            var objects = new Stack<HashSet<string>>();
            while (reader.Read())
            {
                switch (reader.TokenType)
                {
                    case JsonTokenType.StartObject:
                        objects.Push(new HashSet<string>(StringComparer.Ordinal));
                        break;
                    case JsonTokenType.PropertyName:
                        var property = reader.GetString()
                            ?? throw new JsonException("JSON property name must not be null.");
                        if (objects.Count == 0 || !objects.Peek().Add(property))
                        {
                            throw new JsonException($"Duplicate JSON property `{property}`.");
                        }

                        break;
                    case JsonTokenType.EndObject:
                        objects.Pop();
                        break;
                }
            }

            return JsonDocument.Parse(json);
        }
        catch (JsonException error)
        {
            throw new ArgumentException($"{label} must be strict UTF-8 JSON without duplicate keys.", label, error);
        }
    }

    internal static void ExactFields(JsonElement value, HashSet<string> fields, string label)
    {
        if (value.ValueKind != JsonValueKind.Object)
        {
            throw new ArgumentException($"{label} must be a JSON object.");
        }

        var observed = new HashSet<string>(StringComparer.Ordinal);
        foreach (var property in value.EnumerateObject())
        {
            if (!fields.Contains(property.Name))
            {
                throw new ArgumentException($"{label} contains unknown or retired field `{property.Name}`.");
            }

            observed.Add(property.Name);
        }

        foreach (var field in fields)
        {
            if (!observed.Contains(field))
            {
                throw new ArgumentException($"{label} is missing required field `{field}`.");
            }
        }
    }

    internal static string Text(JsonElement value, string field)
    {
        var property = value.GetProperty(field);
        if (property.ValueKind != JsonValueKind.String)
        {
            throw new ArgumentException($"{field} must be a string.");
        }

        var result = property.GetString()!;
        if (result.Length == 0 || result != result.Trim())
        {
            throw new ArgumentException($"{field} must be canonical nonempty text.");
        }

        return result;
    }

    internal static string? OptionalText(JsonElement value, string field) =>
        value.GetProperty(field).ValueKind == JsonValueKind.Null ? null : Text(value, field);

    internal static bool Boolean(JsonElement value, string field) => value.GetProperty(field).ValueKind switch
    {
        JsonValueKind.True => true,
        JsonValueKind.False => false,
        _ => throw new ArgumentException($"{field} must be boolean."),
    };

    internal static ulong UInt64(JsonElement value, string field, ulong minimum)
    {
        var property = value.GetProperty(field);
        if (property.ValueKind != JsonValueKind.Number || !property.TryGetUInt64(out var result)
            || result < minimum || property.GetRawText() != result.ToString(System.Globalization.CultureInfo.InvariantCulture))
        {
            throw new ArgumentException($"{field} must be a canonical unsigned integer >= {minimum}.");
        }

        return result;
    }

    internal static uint UInt32(JsonElement value, string field, uint minimum, uint maximum)
    {
        var result = UInt64(value, field, minimum);
        if (result > maximum)
        {
            throw new ArgumentOutOfRangeException(field);
        }

        return (uint)result;
    }
}
