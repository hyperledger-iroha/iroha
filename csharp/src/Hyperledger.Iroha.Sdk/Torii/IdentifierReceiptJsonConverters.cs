using System.Globalization;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiIdentifierJson
{
    internal static string RequireExactPolicyId(string? value, string field)
    {
        var exact = RequireExactNonBlank(value, field);
        var separator = exact.IndexOf('#', StringComparison.Ordinal);
        if (separator <= 0 || separator != exact.LastIndexOf('#') || separator == exact.Length - 1)
        {
            throw new JsonException($"{field} must use `kind#rule`.");
        }

        RequireExactNonBlank(exact[..separator], $"{field}.kind");
        RequireExactNonBlank(exact[(separator + 1)..], $"{field}.rule");
        return exact;
    }

    internal static string RequireExactNonBlank(string? value, string field)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new JsonException($"{field} must not be empty.");
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must not contain surrounding whitespace.");
        }

        return value;
    }

    internal static string? RequireOptionalExactNonBlank(string? value, string field)
    {
        return value is null ? null : RequireExactNonBlank(value, field);
    }

    internal static string ReadExactString(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType != JsonTokenType.String)
        {
            throw new JsonException($"{field} must be a string.");
        }

        return RequireExactNonBlank(reader.GetString(), field);
    }

    internal static string? ReadOptionalExactString(ref Utf8JsonReader reader, string field)
    {
        return reader.TokenType == JsonTokenType.Null
            ? null
            : ReadExactString(ref reader, field);
    }

    internal static string ReadPolicyId(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType != JsonTokenType.String)
        {
            throw new JsonException($"{field} must be a string.");
        }

        return RequireExactPolicyId(reader.GetString(), field);
    }

    internal static long ReadNonNegativeInt64(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType != JsonTokenType.Number || !reader.TryGetInt64(out var value))
        {
            throw new JsonException($"{field} must be an integer.");
        }

        if (value < 0)
        {
            throw new JsonException($"{field} must be non-negative.");
        }

        return value;
    }

    internal static JsonNode? ReadOptionalNode(ref Utf8JsonReader reader)
    {
        return reader.TokenType == JsonTokenType.Null ? null : JsonNode.Parse(ref reader);
    }

    internal static void RequireUniqueProperty(HashSet<string> seen, string propertyName, string context)
    {
        if (!seen.Add(propertyName))
        {
            throw new JsonException($"{context}.{propertyName} must not appear more than once.");
        }
    }
}

internal sealed class ToriiIdentifierPolicySummaryJsonConverter : JsonConverter<ToriiIdentifierPolicySummary>
{
    public override ToriiIdentifierPolicySummary Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        if (reader.TokenType != JsonTokenType.StartObject)
        {
            throw new JsonException("identifier policy summary must be an object.");
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);
        string? policyId = null;
        string? owner = null;
        bool? active = null;
        string? normalization = null;
        string? resolverPublicKey = null;
        string? backend = null;
        string? inputEncryption = null;
        string? inputEncryptionPublicParameters = null;
        JsonNode? inputEncryptionPublicParametersDecoded = null;
        JsonNode? ramFheProfile = null;
        string? note = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                return new ToriiIdentifierPolicySummary
                {
                    PolicyId = policyId ?? throw new JsonException("policy.policy_id is required."),
                    Owner = owner ?? throw new JsonException("policy.owner is required."),
                    Active = active ?? throw new JsonException("policy.active is required."),
                    Normalization = normalization ?? throw new JsonException("policy.normalization is required."),
                    ResolverPublicKey = resolverPublicKey ?? throw new JsonException("policy.resolver_public_key is required."),
                    Backend = backend ?? throw new JsonException("policy.backend is required."),
                    InputEncryption = inputEncryption,
                    InputEncryptionPublicParameters = inputEncryptionPublicParameters,
                    InputEncryptionPublicParametersDecoded = inputEncryptionPublicParametersDecoded,
                    RamFheProfile = ramFheProfile,
                    Note = note,
                };
            }

            if (reader.TokenType != JsonTokenType.PropertyName)
            {
                throw new JsonException("identifier policy summary property name expected.");
            }

            var propertyName = reader.GetString() ?? throw new JsonException("identifier policy summary property name must be a string.");
            ToriiIdentifierJson.RequireUniqueProperty(seen, propertyName, "policy");
            if (!reader.Read())
            {
                throw new JsonException($"policy.{propertyName} is truncated.");
            }

            switch (propertyName)
            {
                case "policy_id":
                    policyId = ToriiIdentifierJson.ReadPolicyId(ref reader, "policy.policy_id");
                    break;
                case "owner":
                    owner = ToriiIdentifierJson.ReadExactString(ref reader, "policy.owner");
                    break;
                case "active":
                    active = reader.TokenType == JsonTokenType.True
                        ? true
                        : reader.TokenType == JsonTokenType.False
                            ? false
                            : throw new JsonException("policy.active must be a boolean.");
                    break;
                case "normalization":
                    normalization = ToriiIdentifierJson.ReadExactString(ref reader, "policy.normalization");
                    break;
                case "resolver_public_key":
                    resolverPublicKey = ToriiIdentifierJson.ReadExactString(ref reader, "policy.resolver_public_key");
                    break;
                case "backend":
                    backend = ToriiIdentifierJson.ReadExactString(ref reader, "policy.backend");
                    break;
                case "input_encryption":
                    inputEncryption = ToriiIdentifierJson.ReadOptionalExactString(ref reader, "policy.input_encryption");
                    break;
                case "input_encryption_public_parameters":
                    inputEncryptionPublicParameters = ToriiIdentifierJson.ReadOptionalExactString(
                        ref reader,
                        "policy.input_encryption_public_parameters");
                    break;
                case "input_encryption_public_parameters_decoded":
                    inputEncryptionPublicParametersDecoded = ToriiIdentifierJson.ReadOptionalNode(ref reader);
                    break;
                case "ram_fhe_profile":
                    ramFheProfile = ToriiIdentifierJson.ReadOptionalNode(ref reader);
                    break;
                case "note":
                    note = reader.TokenType == JsonTokenType.Null
                        ? null
                        : reader.GetString() ?? throw new JsonException("policy.note must be a string.");
                    break;
                default:
                    reader.Skip();
                    break;
            }
        }

        throw new JsonException("identifier policy summary object is truncated.");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiIdentifierPolicySummary value,
        JsonSerializerOptions options)
    {
        writer.WriteStartObject();
        writer.WriteString("policy_id", value.PolicyId);
        writer.WriteString("owner", value.Owner);
        writer.WriteBoolean("active", value.Active);
        writer.WriteString("normalization", value.Normalization);
        writer.WriteString("resolver_public_key", value.ResolverPublicKey);
        writer.WriteString("backend", value.Backend);
        WriteNullableString(writer, "input_encryption", value.InputEncryption);
        WriteNullableString(writer, "input_encryption_public_parameters", value.InputEncryptionPublicParameters);
        WriteNullableNode(writer, "input_encryption_public_parameters_decoded", value.InputEncryptionPublicParametersDecoded, options);
        WriteNullableNode(writer, "ram_fhe_profile", value.RamFheProfile, options);
        WriteNullableString(writer, "note", value.Note);
        writer.WriteEndObject();
    }

    private static void WriteNullableString(Utf8JsonWriter writer, string propertyName, string? value)
    {
        writer.WritePropertyName(propertyName);
        if (value is null)
        {
            writer.WriteNullValue();
            return;
        }

        writer.WriteStringValue(value);
    }

    private static void WriteNullableNode(
        Utf8JsonWriter writer,
        string propertyName,
        JsonNode? value,
        JsonSerializerOptions options)
    {
        writer.WritePropertyName(propertyName);
        if (value is null)
        {
            writer.WriteNullValue();
            return;
        }

        value.WriteTo(writer, options);
    }
}

internal sealed class ToriiIdentifierResolveResponseJsonConverter : JsonConverter<ToriiIdentifierResolveResponse>
{
    public override ToriiIdentifierResolveResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        if (reader.TokenType != JsonTokenType.StartObject)
        {
            throw new JsonException("identifier resolve response must be an object.");
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);
        string? policyId = null;
        string? opaqueId = null;
        string? receiptHash = null;
        string? uaid = null;
        string? accountId = null;
        long? resolvedAtMilliseconds = null;
        long? expiresAtMilliseconds = null;
        string? backend = null;
        string? signature = null;
        string? signaturePayloadHex = null;
        JsonNode? signaturePayload = null;
        JsonObject? payload = null;
        JsonObject? attestation = null;
        var seenLegacyField = false;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                if (payload is not null || attestation is not null)
                {
                    if (seenLegacyField)
                    {
                        throw new JsonException(
                            "identifier receipt must not mix nested payload/attestation with legacy receipt fields.");
                    }

                    return ReadNestedResolveResponse(payload, attestation);
                }

                return new ToriiIdentifierResolveResponse
                {
                    PolicyId = policyId ?? throw new JsonException("identifier receipt.policy_id is required."),
                    OpaqueId = opaqueId ?? throw new JsonException("identifier receipt.opaque_id is required."),
                    ReceiptHash = receiptHash ?? throw new JsonException("identifier receipt.receipt_hash is required."),
                    Uaid = uaid ?? throw new JsonException("identifier receipt.uaid is required."),
                    AccountId = accountId ?? throw new JsonException("identifier receipt.account_id is required."),
                    ResolvedAtMilliseconds = resolvedAtMilliseconds ?? throw new JsonException("identifier receipt.resolved_at_ms is required."),
                    ExpiresAtMilliseconds = expiresAtMilliseconds,
                    Backend = backend ?? throw new JsonException("identifier receipt.backend is required."),
                    Signature = signature ?? throw new JsonException("identifier receipt.signature is required."),
                    SignaturePayloadHex = signaturePayloadHex ?? throw new JsonException("identifier receipt.signature_payload_hex is required."),
                    SignaturePayload = signaturePayload,
                };
            }

            if (reader.TokenType != JsonTokenType.PropertyName)
            {
                throw new JsonException("identifier resolve response property name expected.");
            }

            var propertyName = reader.GetString() ?? throw new JsonException("identifier resolve response property name must be a string.");
            ToriiIdentifierJson.RequireUniqueProperty(seen, propertyName, "identifier receipt");
            if (!reader.Read())
            {
                throw new JsonException($"identifier receipt.{propertyName} is truncated.");
            }

            switch (propertyName)
            {
                case "policy_id":
                    seenLegacyField = true;
                    policyId = ToriiIdentifierJson.ReadPolicyId(ref reader, "identifier receipt.policy_id");
                    break;
                case "opaque_id":
                    seenLegacyField = true;
                    opaqueId = ToriiIdentifierJson.ReadExactString(ref reader, "identifier receipt.opaque_id");
                    break;
                case "receipt_hash":
                    seenLegacyField = true;
                    receiptHash = ToriiIdentifierJson.ReadExactString(ref reader, "identifier receipt.receipt_hash");
                    break;
                case "uaid":
                    seenLegacyField = true;
                    uaid = ToriiIdentifierJson.ReadExactString(ref reader, "identifier receipt.uaid");
                    break;
                case "account_id":
                    seenLegacyField = true;
                    accountId = ToriiIdentifierJson.ReadExactString(ref reader, "identifier receipt.account_id");
                    break;
                case "resolved_at_ms":
                    seenLegacyField = true;
                    resolvedAtMilliseconds = ToriiIdentifierJson.ReadNonNegativeInt64(
                        ref reader,
                        "identifier receipt.resolved_at_ms");
                    break;
                case "expires_at_ms":
                    seenLegacyField = true;
                    expiresAtMilliseconds = reader.TokenType == JsonTokenType.Null
                        ? null
                        : ToriiIdentifierJson.ReadNonNegativeInt64(ref reader, "identifier receipt.expires_at_ms");
                    break;
                case "backend":
                    seenLegacyField = true;
                    backend = ToriiIdentifierJson.ReadExactString(ref reader, "identifier receipt.backend");
                    break;
                case "signature":
                    seenLegacyField = true;
                    signature = ToriiIdentifierJson.ReadExactString(ref reader, "identifier receipt.signature");
                    break;
                case "signature_payload_hex":
                    seenLegacyField = true;
                    signaturePayloadHex = ToriiIdentifierJson.ReadExactString(
                        ref reader,
                        "identifier receipt.signature_payload_hex");
                    break;
                case "signature_payload":
                    seenLegacyField = true;
                    signaturePayload = ToriiIdentifierJson.ReadOptionalNode(ref reader);
                    break;
                case "payload":
                    payload = ReadObjectNode(ref reader, "identifier receipt.payload");
                    break;
                case "attestation":
                    attestation = ReadObjectNode(ref reader, "identifier receipt.attestation");
                    break;
                default:
                    reader.Skip();
                    break;
            }
        }

        throw new JsonException("identifier resolve response object is truncated.");
    }

    private static ToriiIdentifierResolveResponse ReadNestedResolveResponse(
        JsonObject? payload,
        JsonObject? attestation)
    {
        if (payload is null)
        {
            throw new JsonException("identifier receipt.payload is required.");
        }

        if (attestation is null)
        {
            throw new JsonException("identifier receipt.attestation is required.");
        }

        var execution = RequireObject(payload, "execution", "identifier receipt.payload.execution");
        var kind = RequireExactString(attestation, "kind", "identifier receipt.attestation.kind");
        var signature = kind switch
        {
            "signed" => RequireExactString(attestation, "signature", "identifier receipt.attestation.signature"),
            "proof" => string.Empty,
            _ => throw new JsonException("identifier receipt.attestation.kind must be signed or proof."),
        };

        if (kind == "proof" && attestation.ContainsKey("signature"))
        {
            throw new JsonException("identifier receipt.attestation proof attestations must not include signature.");
        }

        var signaturePayload = new JsonObject
        {
            ["payload"] = payload.DeepClone(),
            ["attestation"] = attestation.DeepClone(),
        };

        return new ToriiIdentifierResolveResponse
        {
            PolicyId = RequirePolicyId(payload, "policy_id", "identifier receipt.payload.policy_id"),
            OpaqueId = RequireExactString(payload, "opaque_id", "identifier receipt.payload.opaque_id"),
            ReceiptHash = RequireExactString(payload, "receipt_hash", "identifier receipt.payload.receipt_hash"),
            Uaid = RequireExactString(payload, "uaid", "identifier receipt.payload.uaid"),
            AccountId = RequireExactString(payload, "account_id", "identifier receipt.payload.account_id"),
            ResolvedAtMilliseconds = RequireNonNegativeInt64(
                execution,
                "executed_at_ms",
                "identifier receipt.payload.execution.executed_at_ms"),
            ExpiresAtMilliseconds = ReadOptionalNonNegativeInt64(
                execution,
                "expires_at_ms",
                "identifier receipt.payload.execution.expires_at_ms"),
            Backend = RequireExactString(execution, "backend", "identifier receipt.payload.execution.backend"),
            Signature = signature,
            SignaturePayloadHex = string.Empty,
            SignaturePayload = signaturePayload,
        };
    }

    private static JsonObject ReadObjectNode(ref Utf8JsonReader reader, string field)
    {
        var node = JsonNode.Parse(ref reader);
        return node as JsonObject ?? throw new JsonException($"{field} must be an object.");
    }

    private static JsonObject RequireObject(JsonObject payload, string propertyName, string field)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var node))
        {
            throw new JsonException($"{field} is required.");
        }

        return node as JsonObject ?? throw new JsonException($"{field} must be an object.");
    }

    private static string RequirePolicyId(JsonObject payload, string propertyName, string field)
    {
        return ToriiIdentifierJson.RequireExactPolicyId(
            RequireExactString(payload, propertyName, field),
            field);
    }

    private static string RequireExactString(JsonObject payload, string propertyName, string field)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var node))
        {
            throw new JsonException($"{field} is required.");
        }

        if (node is JsonValue value && value.TryGetValue<string>(out var text))
        {
            return ToriiIdentifierJson.RequireExactNonBlank(text, field);
        }

        throw new JsonException($"{field} must be a string.");
    }

    private static long RequireNonNegativeInt64(JsonObject payload, string propertyName, string field)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var node))
        {
            throw new JsonException($"{field} is required.");
        }

        return ReadNonNegativeInt64(node, field);
    }

    private static long? ReadOptionalNonNegativeInt64(JsonObject payload, string propertyName, string field)
    {
        return payload.TryGetPropertyValue(propertyName, out var node) && node is not null
            ? ReadNonNegativeInt64(node, field)
            : null;
    }

    private static long ReadNonNegativeInt64(JsonNode? node, string field)
    {
        if (node is not JsonValue value)
        {
            throw new JsonException($"{field} must be an integer.");
        }

        long parsed;
        if (value.TryGetValue<long>(out var signedInteger))
        {
            parsed = signedInteger;
        }
        else if (value.TryGetValue<ulong>(out var unsignedInteger)
            && unsignedInteger <= long.MaxValue)
        {
            parsed = (long)unsignedInteger;
        }
        else if (value.TryGetValue<string>(out var text))
        {
            text = ToriiIdentifierJson.RequireExactNonBlank(text, field);
            if (!long.TryParse(text, NumberStyles.None, CultureInfo.InvariantCulture, out parsed))
            {
                throw new JsonException($"{field} must be an integer.");
            }
        }
        else
        {
            throw new JsonException($"{field} must be an integer.");
        }

        if (parsed < 0)
        {
            throw new JsonException($"{field} must be non-negative.");
        }

        return parsed;
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiIdentifierResolveResponse value,
        JsonSerializerOptions options)
    {
        writer.WriteStartObject();
        writer.WriteString("policy_id", value.PolicyId);
        writer.WriteString("opaque_id", value.OpaqueId);
        writer.WriteString("receipt_hash", value.ReceiptHash);
        writer.WriteString("uaid", value.Uaid);
        writer.WriteString("account_id", value.AccountId);
        writer.WriteNumber("resolved_at_ms", value.ResolvedAtMilliseconds);
        writer.WritePropertyName("expires_at_ms");
        if (value.ExpiresAtMilliseconds is null)
        {
            writer.WriteNullValue();
        }
        else
        {
            writer.WriteNumberValue(value.ExpiresAtMilliseconds.Value);
        }

        writer.WriteString("backend", value.Backend);
        writer.WriteString("signature", value.Signature);
        writer.WriteString("signature_payload_hex", value.SignaturePayloadHex);
        writer.WritePropertyName("signature_payload");
        if (value.SignaturePayload is null)
        {
            writer.WriteNullValue();
        }
        else
        {
            value.SignaturePayload.WriteTo(writer, options);
        }

        writer.WriteEndObject();
    }
}
