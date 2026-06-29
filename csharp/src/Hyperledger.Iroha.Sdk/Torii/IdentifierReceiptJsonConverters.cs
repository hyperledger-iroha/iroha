using System.Globalization;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiIdentifierJson
{
    internal static string RequireExactPolicyId(string? value, string field)
    {
        var exact = RequireNonBlankWithoutSurroundingWhitespaceOrControl(value, field);
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
        value = RequireNonBlankWithoutSurroundingWhitespaceOrControl(value, field);
        if (ContainsWhitespace(value))
        {
            throw new JsonException($"{field} must not contain whitespace.");
        }

        return value;
    }

    private static string RequireNonBlankWithoutSurroundingWhitespaceOrControl(string? value, string field)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new JsonException($"{field} must not be empty.");
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must not contain surrounding whitespace.");
        }

        if (ContainsControlCharacter(value))
        {
            throw new JsonException($"{field} must not contain control characters.");
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

    internal static string ReadExactHex(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType != JsonTokenType.String)
        {
            throw new JsonException($"{field} must be a string.");
        }

        return RequireExactHex(reader.GetString(), field);
    }

    internal static string RequireExactHex(string? value, string field)
    {
        var exact = RequireExactNonBlank(value, field);
        var body = exact.StartsWith("0x", StringComparison.Ordinal)
            ? exact[2..]
            : exact;
        if (body.Length == 0 || body.Length % 2 != 0 || !IsHex(body))
        {
            throw new JsonException($"{field} must be an exact hex string.");
        }

        return exact;
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

    internal static bool IsCanonicalUnsignedDecimalText(string value)
    {
        if (value.Length == 0 || (value.Length > 1 && value[0] == '0'))
        {
            return false;
        }

        foreach (var character in value)
        {
            if (character < '0' || character > '9')
            {
                return false;
            }
        }

        return true;
    }

    internal static JsonNode? ReadOptionalNode(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            return null;
        }

        using var document = JsonDocument.ParseValue(ref reader);
        RejectDuplicateProperties(document.RootElement, field);
        return JsonNode.Parse(document.RootElement.GetRawText());
    }

    internal static JsonNode? ParseNodeRejectingDuplicateProperties(string json, string field)
    {
        using var document = JsonDocument.Parse(json);
        RejectDuplicateProperties(document.RootElement, field);
        return JsonNode.Parse(document.RootElement.GetRawText());
    }

    internal static void SkipRejectingDuplicateProperties(ref Utf8JsonReader reader, string field)
    {
        using var document = JsonDocument.ParseValue(ref reader);
        RejectDuplicateProperties(document.RootElement, field);
    }

    internal static bool IsDuplicatePropertyError(JsonException exception)
    {
        return exception.Message.Contains("must not appear more than once", StringComparison.Ordinal);
    }

    internal static void ValidatePolicySummary(ToriiIdentifierPolicySummary? value, string context)
    {
        if (value is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireExactPolicyId(value.PolicyId, $"{context}.policy_id");
        RequireExactNonBlank(value.Owner, $"{context}.owner");
        RequireExactNonBlank(value.Normalization, $"{context}.normalization");
        RequireExactNonBlank(value.ResolverPublicKey, $"{context}.resolver_public_key");
        RequireExactNonBlank(value.Backend, $"{context}.backend");
        RequireOptionalExactNonBlank(value.InputEncryption, $"{context}.input_encryption");
        RequireOptionalExactNonBlank(
            value.InputEncryptionPublicParameters,
            $"{context}.input_encryption_public_parameters");
    }

    internal static void RequireUniqueProperty(HashSet<string> seen, string propertyName, string context)
    {
        if (!seen.Add(propertyName))
        {
            throw new JsonException($"{context}.{propertyName} must not appear more than once.");
        }
    }

    internal static void RejectDuplicateProperties(JsonElement element, string field)
    {
        switch (element.ValueKind)
        {
            case JsonValueKind.Object:
                var seen = new HashSet<string>(StringComparer.Ordinal);
                foreach (var property in element.EnumerateObject())
                {
                    RequireUniqueProperty(seen, property.Name, field);
                    RejectDuplicateProperties(property.Value, $"{field}.{property.Name}");
                }

                break;
            case JsonValueKind.Array:
                var index = 0;
                foreach (var item in element.EnumerateArray())
                {
                    RejectDuplicateProperties(item, $"{field}[{index}]");
                    index++;
                }

                break;
        }
    }

    private static bool ContainsWhitespace(string value)
    {
        foreach (var character in value)
        {
            if (char.IsWhiteSpace(character))
            {
                return true;
            }
        }

        return false;
    }

    private static bool ContainsControlCharacter(string value)
    {
        foreach (var character in value)
        {
            if (char.IsControl(character))
            {
                return true;
            }
        }

        return false;
    }

    private static bool IsHex(string value)
    {
        foreach (var character in value)
        {
            var isHex =
                character is >= '0' and <= '9'
                || character is >= 'a' and <= 'f'
                || character is >= 'A' and <= 'F';
            if (!isHex)
            {
                return false;
            }
        }

        return true;
    }
}

internal sealed class ToriiIdentifierPolicySummaryJsonConverter : JsonConverter<ToriiIdentifierPolicySummary>
{
    public override bool HandleNull => true;

    public override ToriiIdentifierPolicySummary Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            throw new JsonException("identifier policy summary must not be null.");
        }

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
                    inputEncryptionPublicParametersDecoded = ToriiIdentifierJson.ReadOptionalNode(
                        ref reader,
                        "policy.input_encryption_public_parameters_decoded");
                    break;
                case "ram_fhe_profile":
                    ramFheProfile = ToriiIdentifierJson.ReadOptionalNode(ref reader, "policy.ram_fhe_profile");
                    break;
                case "note":
                    if (reader.TokenType == JsonTokenType.Null)
                    {
                        note = null;
                    }
                    else if (reader.TokenType == JsonTokenType.String)
                    {
                        note = reader.GetString();
                    }
                    else
                    {
                        throw new JsonException("policy.note must be a string.");
                    }
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"policy.{propertyName}");
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
        ToriiIdentifierJson.ValidatePolicySummary(value, "policy");

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

internal sealed class ToriiIdentifierPoliciesResponseJsonConverter :
    JsonConverter<ToriiIdentifierPoliciesResponse>
{
    public override bool HandleNull => true;

    public override ToriiIdentifierPoliciesResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            throw new JsonException("identifier policies response must not be null.");
        }

        if (reader.TokenType != JsonTokenType.StartObject)
        {
            throw new JsonException("identifier policies response must be an object.");
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);
        long? total = null;
        List<ToriiIdentifierPolicySummary>? items = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = new ToriiIdentifierPoliciesResponse
                {
                    Total = RequireTotal(total),
                    Items = RequireItems(items),
                };
                ValidateResponse(response);
                return response;
            }

            if (reader.TokenType != JsonTokenType.PropertyName)
            {
                throw new JsonException("identifier policies response property name expected.");
            }

            var propertyName = reader.GetString()
                ?? throw new JsonException("identifier policies response property name must be a string.");
            ToriiIdentifierJson.RequireUniqueProperty(seen, propertyName, "identifier policies response");
            if (!reader.Read())
            {
                throw new JsonException($"identifier policies response.{propertyName} is truncated.");
            }

            switch (propertyName)
            {
                case "total":
                    total = ToriiIdentifierJson.ReadNonNegativeInt64(ref reader, "identifier policies response.total");
                    break;
                case "items":
                    items = ReadItems(ref reader, options);
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(
                        ref reader,
                        $"identifier policies response.{propertyName}");
                    break;
            }
        }

        throw new JsonException("identifier policies response object is truncated.");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiIdentifierPoliciesResponse value,
        JsonSerializerOptions options)
    {
        ValidateResponse(value);

        writer.WriteStartObject();
        writer.WriteNumber("total", value.Total);
        writer.WritePropertyName("items");
        writer.WriteStartArray();
        for (var index = 0; index < value.Items.Count; index++)
        {
            JsonSerializer.Serialize(writer, value.Items[index], options);
        }
        writer.WriteEndArray();
        writer.WriteEndObject();
    }

    private static List<ToriiIdentifierPolicySummary>? ReadItems(
        ref Utf8JsonReader reader,
        JsonSerializerOptions options)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            return null;
        }

        if (reader.TokenType != JsonTokenType.StartArray)
        {
            throw new JsonException("identifier policies response.items must be an array.");
        }

        var items = new List<ToriiIdentifierPolicySummary>();
        var index = 0;
        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndArray)
            {
                return items;
            }

            if (reader.TokenType == JsonTokenType.Null)
            {
                throw new JsonException($"identifier policies response.items[{index}] must not be null.");
            }

            if (reader.TokenType != JsonTokenType.StartObject)
            {
                throw new JsonException($"identifier policies response.items[{index}] must be an object.");
            }

            items.Add(JsonSerializer.Deserialize<ToriiIdentifierPolicySummary>(ref reader, options)
                ?? throw new JsonException($"identifier policies response.items[{index}] must not be null."));
            index++;
        }

        throw new JsonException("identifier policies response.items array is truncated.");
    }

    private static void ValidateResponse(ToriiIdentifierPoliciesResponse? value)
    {
        if (value is null)
        {
            throw new JsonException("identifier policies response must not be null.");
        }

        if (value.Total < 0)
        {
            throw new JsonException("identifier policies response.total must be non-negative.");
        }

        if (value.Items is null)
        {
            throw new JsonException("identifier policies response.items is required.");
        }

        for (var index = 0; index < value.Items.Count; index++)
        {
            ToriiIdentifierJson.ValidatePolicySummary(
                value.Items[index],
                $"identifier policies response.items[{index}]");
        }
    }

    private static IReadOnlyList<ToriiIdentifierPolicySummary> RequireItems(
        IReadOnlyList<ToriiIdentifierPolicySummary>? items)
    {
        if (items is null)
        {
            throw new JsonException("identifier policies response.items is required.");
        }

        return items;
    }

    private static long RequireTotal(long? value)
    {
        if (!value.HasValue)
        {
            throw new JsonException("identifier policies response.total must not be null.");
        }

        return value.Value;
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

                ValidateLegacySignaturePayload(signaturePayload, "identifier receipt.signature_payload");

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
                    resolvedAtMilliseconds = ReadPositiveInt64(ref reader, "identifier receipt.resolved_at_ms");
                    break;
                case "expires_at_ms":
                    seenLegacyField = true;
                    expiresAtMilliseconds = reader.TokenType == JsonTokenType.Null
                        ? null
                        : ReadPositiveInt64(ref reader, "identifier receipt.expires_at_ms");
                    break;
                case "backend":
                    seenLegacyField = true;
                    backend = ToriiIdentifierJson.ReadExactString(ref reader, "identifier receipt.backend");
                    break;
                case "signature":
                    seenLegacyField = true;
                    signature = ToriiIdentifierJson.ReadExactHex(ref reader, "identifier receipt.signature");
                    break;
                case "signature_payload_hex":
                    seenLegacyField = true;
                    signaturePayloadHex = ToriiIdentifierJson.ReadExactHex(
                        ref reader,
                        "identifier receipt.signature_payload_hex");
                    break;
                case "signature_payload":
                    seenLegacyField = true;
                    signaturePayload = ToriiIdentifierJson.ReadOptionalNode(
                        ref reader,
                        "identifier receipt.signature_payload");
                    break;
                case "payload":
                    payload = ReadObjectNode(ref reader, "identifier receipt.payload");
                    break;
                case "attestation":
                    attestation = ReadObjectNode(ref reader, "identifier receipt.attestation");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(
                        ref reader,
                        $"identifier receipt.{propertyName}");
                    break;
            }
        }

        throw new JsonException("identifier resolve response object is truncated.");
    }

    private static void ValidateLegacySignaturePayload(JsonNode? payload, string context)
    {
        if (payload is not JsonObject payloadObject)
        {
            return;
        }

        ValidateSignaturePayloadObject(payloadObject, context);

        if (payloadObject.TryGetPropertyValue("payload", out var nestedPayload)
            && nestedPayload is JsonObject nestedPayloadObject)
        {
            ValidateSignaturePayloadObject(nestedPayloadObject, $"{context}.payload");
        }

        if (payloadObject.TryGetPropertyValue("attestation", out var attestation)
            && attestation is JsonObject attestationObject)
        {
            ValidateSignaturePayloadAttestation(attestationObject, $"{context}.attestation");
        }
    }

    private static void ValidateSignaturePayloadObject(JsonObject payload, string context)
    {
        ValidateOptionalPolicyId(payload, "policy_id", $"{context}.policy_id");
        ValidateOptionalExactString(payload, "account_id", $"{context}.account_id");
        ValidateOptionalExactString(payload, "opaque_id", $"{context}.opaque_id");
        ValidateOptionalExactString(payload, "receipt_hash", $"{context}.receipt_hash");
        ValidateOptionalExactString(payload, "uaid", $"{context}.uaid");
        ValidateSignaturePayloadAttestation(payload, context);

        if (TryGetOptionalObject(payload, "execution", $"{context}.execution", out var execution))
        {
            ValidateOptionalExactString(execution, "program_id", $"{context}.execution.program_id");
            ValidateOptionalExactString(execution, "program_digest", $"{context}.execution.program_digest");
            ValidateOptionalExactString(execution, "backend", $"{context}.execution.backend");
            ValidateOptionalExactString(execution, "verification_mode", $"{context}.execution.verification_mode");
            ValidateOptionalExactString(
                execution,
                "input_ciphertext_hash",
                $"{context}.execution.input_ciphertext_hash");
            ValidateOptionalExactString(
                execution,
                "output_ciphertext_hash",
                $"{context}.execution.output_ciphertext_hash");
            ValidateOptionalExactString(execution, "parameter_digest", $"{context}.execution.parameter_digest");
            ValidateOptionalExactString(
                execution,
                "evaluation_key_digest",
                $"{context}.execution.evaluation_key_digest");
            ValidateOptionalExactString(execution, "output_hash", $"{context}.execution.output_hash");
            ValidateOptionalExactString(
                execution,
                "associated_data_hash",
                $"{context}.execution.associated_data_hash");
            ValidateOptionalPositiveInt64(execution, "executed_at_ms", $"{context}.execution.executed_at_ms");
            ValidateOptionalPositiveInt64(execution, "expires_at_ms", $"{context}.execution.expires_at_ms");
        }

        if (TryGetOptionalObject(payload, "opening", $"{context}.opening", out var opening))
        {
            ValidateOptionalExactHexString(opening, "signature", $"{context}.opening.signature");
            if (TryGetOptionalObject(opening, "payload", $"{context}.opening.payload", out var openingPayload))
            {
                ValidateOptionalExactString(
                    openingPayload,
                    "program_id",
                    $"{context}.opening.payload.program_id");
                ValidateOptionalExactString(
                    openingPayload,
                    "input_ciphertext_hash",
                    $"{context}.opening.payload.input_ciphertext_hash");
                ValidateOptionalExactString(
                    openingPayload,
                    "output_ciphertext_hash",
                    $"{context}.opening.payload.output_ciphertext_hash");
                ValidateOptionalExactString(
                    openingPayload,
                    "parameter_digest",
                    $"{context}.opening.payload.parameter_digest");
                ValidateOptionalExactString(
                    openingPayload,
                    "evaluation_key_digest",
                    $"{context}.opening.payload.evaluation_key_digest");
                ValidateOptionalExactString(
                    openingPayload,
                    "opened_output_hash",
                    $"{context}.opening.payload.opened_output_hash");
                ValidateOptionalPositiveInt64(
                    openingPayload,
                    "opened_at_ms",
                    $"{context}.opening.payload.opened_at_ms");
                ValidateOptionalPositiveInt64(
                    openingPayload,
                    "expires_at_ms",
                    $"{context}.opening.payload.expires_at_ms");
            }
        }
    }

    private static void ValidateSignaturePayloadAttestation(JsonObject attestation, string context)
    {
        ValidateOptionalExactHexString(attestation, "signature", $"{context}.signature");

        var hasKind = attestation.TryGetPropertyValue("kind", out _);
        var hasProofBackend = attestation.TryGetPropertyValue("proof_backend", out _);
        var hasProofBase64 = attestation.TryGetPropertyValue("proof_b64", out _);
        if (!hasKind)
        {
            if (hasProofBackend || hasProofBase64)
            {
                throw new JsonException($"{context}.kind is required when proof fields are present.");
            }

            return;
        }

        var kind = RequireExactString(attestation, "kind", $"{context}.kind");
        switch (kind)
        {
            case "signed":
                if (hasProofBackend || hasProofBase64)
                {
                    throw new JsonException($"{context} signed attestations must not include proof fields.");
                }

                break;
            case "proof":
                _ = RequireExactString(attestation, "proof_backend", $"{context}.proof_backend");
                var proof = RequireExactString(attestation, "proof_b64", $"{context}.proof_b64");
                ValidateExactBase64(proof, $"{context}.proof_b64");
                break;
            default:
                throw new JsonException($"{context}.kind must be signed or proof.");
        }
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
        ValidateNestedResolvePayload(payload, execution);
        var kind = RequireExactString(attestation, "kind", "identifier receipt.attestation.kind");
        var signature = kind switch
        {
            "signed" => ReadSignedAttestationSignature(attestation),
            "proof" => ReadProofAttestation(attestation),
            _ => throw new JsonException("identifier receipt.attestation.kind must be signed or proof."),
        };

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
            ResolvedAtMilliseconds = RequirePositiveInt64(
                execution,
                "executed_at_ms",
                "identifier receipt.payload.execution.executed_at_ms"),
            ExpiresAtMilliseconds = ReadOptionalPositiveInt64(
                execution,
                "expires_at_ms",
                "identifier receipt.payload.execution.expires_at_ms"),
            Backend = RequireExactString(execution, "backend", "identifier receipt.payload.execution.backend"),
            Signature = signature,
            SignaturePayloadHex = string.Empty,
            SignaturePayload = signaturePayload,
        };
    }

    private static string ReadSignedAttestationSignature(JsonObject attestation)
    {
        if (attestation.ContainsKey("proof_backend") || attestation.ContainsKey("proof_b64"))
        {
            throw new JsonException("identifier receipt.attestation signed attestations must not include proof fields.");
        }

        return RequireExactHexString(attestation, "signature", "identifier receipt.attestation.signature");
    }

    private static string ReadProofAttestation(JsonObject attestation)
    {
        if (attestation.ContainsKey("signature"))
        {
            throw new JsonException("identifier receipt.attestation proof attestations must not include signature.");
        }

        _ = RequireExactString(attestation, "proof_backend", "identifier receipt.attestation.proof_backend");
        var proof = RequireExactString(attestation, "proof_b64", "identifier receipt.attestation.proof_b64");
        ValidateExactBase64(proof, "identifier receipt.attestation.proof_b64");
        return string.Empty;
    }

    private static void ValidateNestedResolvePayload(JsonObject payload, JsonObject execution)
    {
        ValidateOptionalExactString(execution, "program_id", "identifier receipt.payload.execution.program_id");
        ValidateOptionalExactString(execution, "program_digest", "identifier receipt.payload.execution.program_digest");
        ValidateOptionalExactString(execution, "backend", "identifier receipt.payload.execution.backend");
        ValidateOptionalExactString(execution, "verification_mode", "identifier receipt.payload.execution.verification_mode");
        ValidateOptionalExactString(
            execution,
            "input_ciphertext_hash",
            "identifier receipt.payload.execution.input_ciphertext_hash");
        ValidateOptionalExactString(
            execution,
            "output_ciphertext_hash",
            "identifier receipt.payload.execution.output_ciphertext_hash");
        ValidateOptionalExactString(execution, "parameter_digest", "identifier receipt.payload.execution.parameter_digest");
        ValidateOptionalExactString(
            execution,
            "evaluation_key_digest",
            "identifier receipt.payload.execution.evaluation_key_digest");
        ValidateOptionalExactString(execution, "output_hash", "identifier receipt.payload.execution.output_hash");
        ValidateOptionalExactString(
            execution,
            "associated_data_hash",
            "identifier receipt.payload.execution.associated_data_hash");

        if (!TryGetOptionalObject(payload, "opening", "identifier receipt.payload.opening", out var opening))
        {
            return;
        }

        ValidateOptionalExactHexString(opening, "signature", "identifier receipt.payload.opening.signature");
        if (!TryGetOptionalObject(opening, "payload", "identifier receipt.payload.opening.payload", out var openingPayload))
        {
            return;
        }

        ValidateOptionalExactString(
            openingPayload,
            "program_id",
            "identifier receipt.payload.opening.payload.program_id");
        ValidateOptionalExactString(
            openingPayload,
            "input_ciphertext_hash",
            "identifier receipt.payload.opening.payload.input_ciphertext_hash");
        ValidateOptionalExactString(
            openingPayload,
            "output_ciphertext_hash",
            "identifier receipt.payload.opening.payload.output_ciphertext_hash");
        ValidateOptionalExactString(
            openingPayload,
            "parameter_digest",
            "identifier receipt.payload.opening.payload.parameter_digest");
        ValidateOptionalExactString(
            openingPayload,
            "evaluation_key_digest",
            "identifier receipt.payload.opening.payload.evaluation_key_digest");
        ValidateOptionalExactString(
            openingPayload,
            "opened_output_hash",
            "identifier receipt.payload.opening.payload.opened_output_hash");
        ValidateOptionalPositiveInt64(
            openingPayload,
            "opened_at_ms",
            "identifier receipt.payload.opening.payload.opened_at_ms");
        ValidateOptionalPositiveInt64(
            openingPayload,
            "expires_at_ms",
            "identifier receipt.payload.opening.payload.expires_at_ms");
    }

    private static JsonObject ReadObjectNode(ref Utf8JsonReader reader, string field)
    {
        var node = ToriiIdentifierJson.ReadOptionalNode(ref reader, field);
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

    private static bool TryGetOptionalObject(
        JsonObject payload,
        string propertyName,
        string field,
        out JsonObject value)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var node) || node is null)
        {
            value = new JsonObject();
            return false;
        }

        if (node is JsonObject jsonObject)
        {
            value = jsonObject;
            return true;
        }

        throw new JsonException($"{field} must be an object.");
    }

    private static string RequirePolicyId(JsonObject payload, string propertyName, string field)
    {
        return ToriiIdentifierJson.RequireExactPolicyId(
            RequireExactString(payload, propertyName, field),
            field);
    }

    private static void ValidateOptionalPolicyId(JsonObject payload, string propertyName, string field)
    {
        if (payload.TryGetPropertyValue(propertyName, out var node) && node is not null)
        {
            _ = ToriiIdentifierJson.RequireExactPolicyId(RequireJsonString(node, field), field);
        }
    }

    private static string RequireJsonString(JsonNode node, string field)
    {
        if (node is JsonValue value && value.TryGetValue<string>(out var text))
        {
            return text;
        }

        throw new JsonException($"{field} must be a string.");
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

    private static string RequireExactHexString(JsonObject payload, string propertyName, string field)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var node))
        {
            throw new JsonException($"{field} is required.");
        }

        if (node is JsonValue value && value.TryGetValue<string>(out var text))
        {
            return ToriiIdentifierJson.RequireExactHex(text, field);
        }

        throw new JsonException($"{field} must be a string.");
    }

    private static void ValidateOptionalExactString(JsonObject payload, string propertyName, string field)
    {
        if (payload.TryGetPropertyValue(propertyName, out var node) && node is not null)
        {
            _ = RequireExactString(payload, propertyName, field);
        }
    }

    private static void ValidateOptionalExactHexString(JsonObject payload, string propertyName, string field)
    {
        if (payload.TryGetPropertyValue(propertyName, out var node) && node is not null)
        {
            _ = RequireExactHexString(payload, propertyName, field);
        }
    }

    private static long RequirePositiveInt64(JsonObject payload, string propertyName, string field)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var node))
        {
            throw new JsonException($"{field} is required.");
        }

        return ReadPositiveInt64(node, field);
    }

    private static long? ReadOptionalPositiveInt64(JsonObject payload, string propertyName, string field)
    {
        return payload.TryGetPropertyValue(propertyName, out var node) && node is not null
            ? ReadPositiveInt64(node, field)
            : null;
    }

    private static void ValidateOptionalPositiveInt64(JsonObject payload, string propertyName, string field)
    {
        if (payload.TryGetPropertyValue(propertyName, out var node) && node is not null)
        {
            _ = ReadPositiveInt64(node, field);
        }
    }

    private static long ReadPositiveInt64(ref Utf8JsonReader reader, string field)
    {
        return ValidatePositiveInt64(ToriiIdentifierJson.ReadNonNegativeInt64(ref reader, field), field);
    }

    private static long ReadPositiveInt64(JsonNode? node, string field)
    {
        return ValidatePositiveInt64(ReadNonNegativeInt64(node, field), field);
    }

    private static long ValidatePositiveInt64(long value, string field)
    {
        if (value <= 0)
        {
            throw new JsonException($"{field} must be positive.");
        }

        return value;
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
            if (text.StartsWith("-", StringComparison.Ordinal))
            {
                throw new JsonException($"{field} must be non-negative.");
            }
            if (!ToriiIdentifierJson.IsCanonicalUnsignedDecimalText(text)
                || !long.TryParse(text, NumberStyles.None, CultureInfo.InvariantCulture, out parsed))
            {
                throw new JsonException($"{field} must be canonical unsigned decimal text.");
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

    private static void ValidateExactBase64(string value, string field)
    {
        foreach (var character in value)
        {
            if (char.IsWhiteSpace(character))
            {
                throw new JsonException($"{field} must not contain whitespace.");
            }
        }

        byte[] bytes;
        try
        {
            bytes = Convert.FromBase64String(value);
        }
        catch (FormatException exception)
        {
            throw new JsonException($"{field} must be valid base64.", exception);
        }

        if (bytes.Length == 0)
        {
            throw new JsonException($"{field} must not decode to empty bytes.");
        }

        if (!string.Equals(Convert.ToBase64String(bytes), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must be canonical base64 text.");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiIdentifierResolveResponse value,
        JsonSerializerOptions options)
    {
        ValidatePositiveInt64(value.ResolvedAtMilliseconds, "identifier receipt.resolved_at_ms");
        if (value.ExpiresAtMilliseconds is long expiresAtMilliseconds)
        {
            ValidatePositiveInt64(expiresAtMilliseconds, "identifier receipt.expires_at_ms");
        }

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
