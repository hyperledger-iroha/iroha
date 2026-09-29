using System.Text.Json;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

/// <summary>
/// Encrypted identifier lookup with an independently authenticated plaintext opening.
/// This DTO does not encrypt input or establish that an encryption backend is available.
/// </summary>
[JsonConverter(typeof(IdentifierResolveRequestJsonConverter))]
public sealed record class ToriiIdentifierResolveRequest
{
    /// <summary>Exact registered identifier policy in <c>kind#rule</c> form.</summary>
    [JsonPropertyName("policy_id")]
    public required string PolicyId { get; init; }

    /// <summary>Nonempty encrypted input envelope as exact lowercase hexadecimal.</summary>
    [JsonPropertyName("encrypted_input")]
    public required string EncryptedInput { get; init; }

    /// <summary>Opening signed by the policy's independent output-opening authority.</summary>
    [JsonPropertyName("output_opening")]
    public required ToriiRamLfeOutputOpening OutputOpening { get; init; }
}

/// <summary>Independent authority's signed statement about opened plaintext output.</summary>
[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed record class ToriiRamLfeOutputOpening : IJsonOnDeserialized, IJsonOnSerializing
{
    /// <summary>Statement bound to the exact program, ciphertexts and evaluation parameters.</summary>
    [JsonPropertyName("payload")]
    public required ToriiRamLfeOutputOpeningPayload Payload { get; init; }

    /// <summary>Authority signature over the canonical payload, as lowercase hexadecimal.</summary>
    [JsonPropertyName("signature")]
    public required string Signature { get; init; }

    internal void Validate()
    {
        if (Payload is null)
        {
            throw new JsonException("output_opening.payload is required.");
        }

        Payload.Validate();
        IdentifierRequestWire.RequireLowerHex(Signature, "output_opening.signature");
    }

    void IJsonOnDeserialized.OnDeserialized() => Validate();
    void IJsonOnSerializing.OnSerializing() => Validate();
}

/// <summary>Typed wire payload signed by an independent RAM-LFE opening authority.</summary>
[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed record class ToriiRamLfeOutputOpeningPayload : IJsonOnDeserialized, IJsonOnSerializing
{
    /// <summary>Exact registered RAM-LFE program identifier.</summary>
    [JsonPropertyName("program_id")]
    public required string ProgramId { get; init; }

    /// <summary>Hash of the encrypted input envelope.</summary>
    [JsonPropertyName("input_ciphertext_hash")]
    public required string InputCiphertextHash { get; init; }

    /// <summary>Hash of the encrypted output envelope.</summary>
    [JsonPropertyName("output_ciphertext_hash")]
    public required string OutputCiphertextHash { get; init; }

    /// <summary>Digest of the registered encryption parameter set.</summary>
    [JsonPropertyName("parameter_digest")]
    public required string ParameterDigest { get; init; }

    /// <summary>Digest of the registered evaluation-key bundle.</summary>
    [JsonPropertyName("evaluation_key_digest")]
    public required string EvaluationKeyDigest { get; init; }

    /// <summary>Hash of the independently opened plaintext output.</summary>
    [JsonPropertyName("opened_output_hash")]
    public required string OpenedOutputHash { get; init; }

    /// <summary>Opening timestamp in milliseconds since the Unix epoch.</summary>
    [JsonPropertyName("opened_at_ms")]
    public required ulong OpenedAtMilliseconds { get; init; }

    /// <summary>Optional opening expiry in milliseconds since the Unix epoch.</summary>
    [JsonPropertyName("expires_at_ms"), JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public ulong? ExpiresAtMilliseconds { get; init; }

    internal void Validate()
    {
        ToriiIdentifierJson.RequireExactNonBlank(ProgramId, "output_opening.payload.program_id");
        IdentifierRequestWire.RequireLowerHex(InputCiphertextHash, "output_opening.payload.input_ciphertext_hash", 32);
        IdentifierRequestWire.RequireLowerHex(OutputCiphertextHash, "output_opening.payload.output_ciphertext_hash", 32);
        IdentifierRequestWire.RequireLowerHex(ParameterDigest, "output_opening.payload.parameter_digest", 32);
        IdentifierRequestWire.RequireLowerHex(EvaluationKeyDigest, "output_opening.payload.evaluation_key_digest", 32);
        IdentifierRequestWire.RequireLowerHex(OpenedOutputHash, "output_opening.payload.opened_output_hash", 32);
    }

    void IJsonOnDeserialized.OnDeserialized() => Validate();
    void IJsonOnSerializing.OnSerializing() => Validate();
}

internal static class IdentifierRequestWire
{
    internal static string RequireLowerHex(string? value, string field, int? bytes = null)
    {
        var exact = ToriiIdentifierJson.RequireExactNonBlank(value, field);
        if (exact.Length % 2 != 0 ||
            (bytes is not null && exact.Length != bytes * 2) ||
            exact.Any(c => c is not (>= '0' and <= '9') and not (>= 'a' and <= 'f')))
        {
            throw new JsonException($"{field} must be exact lowercase hexadecimal" +
                (bytes is null ? "." : $" containing {bytes} bytes."));
        }

        return exact;
    }
}

internal sealed class IdentifierResolveRequestJsonConverter : JsonConverter<ToriiIdentifierResolveRequest>
{
    public override bool HandleNull => true;

    public override ToriiIdentifierResolveRequest Read(
        ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        using var document = JsonDocument.ParseValue(ref reader);
        var root = document.RootElement;
        if (root.ValueKind != JsonValueKind.Object)
        {
            throw new JsonException("identifier resolve request must be an object.");
        }

        ToriiIdentifierJson.RejectDuplicateProperties(root, "identifier resolve request");
        foreach (var property in root.EnumerateObject())
        {
            if (property.Name is not ("policy_id" or "encrypted_input" or "output_opening"))
            {
                throw new JsonException($"identifier resolve request.{property.Name} is not supported.");
            }
        }

        var policyId = RequiredString(root, "policy_id");
        var ciphertext = RequiredString(root, "encrypted_input");
        if (!root.TryGetProperty("output_opening", out var opening) || opening.ValueKind == JsonValueKind.Null)
        {
            throw new JsonException("identifier resolve request.output_opening is required.");
        }

        var result = new ToriiIdentifierResolveRequest
        {
            PolicyId = policyId,
            EncryptedInput = ciphertext,
            OutputOpening = opening.Deserialize<ToriiRamLfeOutputOpening>(options)
                ?? throw new JsonException("identifier resolve request.output_opening is required."),
        };
        Validate(result);
        return result;
    }

    public override void Write(Utf8JsonWriter writer, ToriiIdentifierResolveRequest value, JsonSerializerOptions options)
    {
        Validate(value);
        writer.WriteStartObject();
        writer.WriteString("policy_id", value.PolicyId);
        writer.WriteString("encrypted_input", value.EncryptedInput);
        writer.WritePropertyName("output_opening");
        JsonSerializer.Serialize(writer, value.OutputOpening, options);
        writer.WriteEndObject();
    }

    private static string RequiredString(JsonElement root, string field)
    {
        if (!root.TryGetProperty(field, out var value) || value.ValueKind != JsonValueKind.String)
        {
            throw new JsonException($"identifier resolve request.{field} must be a string.");
        }

        return value.GetString()!;
    }

    private static void Validate(ToriiIdentifierResolveRequest? value)
    {
        if (value is null)
        {
            throw new JsonException("identifier resolve request is required.");
        }

        ToriiIdentifierJson.RequireExactPolicyId(value.PolicyId, "identifier resolve request.policy_id");
        IdentifierRequestWire.RequireLowerHex(value.EncryptedInput, "identifier resolve request.encrypted_input");
        if (value.OutputOpening is null)
        {
            throw new JsonException("identifier resolve request.output_opening is required.");
        }

        value.OutputOpening.Validate();
    }
}
