using System.Text.Json;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Offline;

internal sealed class OfflineReadinessBlockerJsonConverter : JsonConverter<OfflineReadinessBlocker>
{
    public override bool HandleNull => true;

    public override OfflineReadinessBlocker Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        if (reader.TokenType != JsonTokenType.StartObject)
        {
            throw new JsonException("Offline readiness blocker must be an object.");
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);
        string? code = null;
        string? message = null;
        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                if (!seen.Contains("code") || !seen.Contains("message"))
                {
                    throw new JsonException("Offline readiness blocker requires code and message.");
                }

                try
                {
                    return new OfflineReadinessBlocker(code!, message!);
                }
                catch (ArgumentException exception)
                {
                    throw new JsonException("Offline readiness blocker is invalid.", exception);
                }
            }

            var name = OfflineReadinessJson.RequireUniqueProperty(ref reader, seen, "Offline readiness blocker");
            if (!reader.Read())
            {
                throw new JsonException("Offline readiness blocker property is truncated.");
            }
            switch (name)
            {
                case "code":
                    code = OfflineReadinessJson.ReadString(ref reader, "Offline readiness blocker.code");
                    break;
                case "message":
                    message = OfflineReadinessJson.ReadString(ref reader, "Offline readiness blocker.message");
                    break;
                default:
                    reader.Skip();
                    break;
            }
        }

        throw new JsonException("Offline readiness blocker object is truncated.");
    }

    public override void Write(
        Utf8JsonWriter writer,
        OfflineReadinessBlocker value,
        JsonSerializerOptions options)
    {
        ArgumentNullException.ThrowIfNull(value);
        writer.WriteStartObject();
        writer.WriteString("code", value.Code);
        writer.WriteString("message", value.Message);
        writer.WriteEndObject();
    }
}

internal sealed class OfflineVerifierIdJsonConverter : JsonConverter<OfflineVerifierId>
{
    public override bool HandleNull => true;

    public override OfflineVerifierId Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        if (reader.TokenType != JsonTokenType.StartObject)
        {
            throw new JsonException("Offline verifier id must be an object.");
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);
        string? backend = null;
        string? name = null;
        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                if (!seen.Contains("backend") || !seen.Contains("name"))
                {
                    throw new JsonException("Offline verifier id requires backend and name.");
                }
                try
                {
                    return new OfflineVerifierId(backend!, name!);
                }
                catch (ArgumentException exception)
                {
                    throw new JsonException("Offline verifier id is invalid.", exception);
                }
            }

            var property = OfflineReadinessJson.RequireUniqueProperty(
                ref reader,
                seen,
                "Offline verifier id");
            if (!reader.Read())
            {
                throw new JsonException("Offline verifier id property is truncated.");
            }
            switch (property)
            {
                case "backend":
                    backend = OfflineReadinessJson.ReadString(ref reader, "Offline verifier id.backend");
                    break;
                case "name":
                    name = OfflineReadinessJson.ReadString(ref reader, "Offline verifier id.name");
                    break;
                default:
                    reader.Skip();
                    break;
            }
        }
        throw new JsonException("Offline verifier id object is truncated.");
    }

    public override void Write(Utf8JsonWriter writer, OfflineVerifierId value, JsonSerializerOptions options)
    {
        ArgumentNullException.ThrowIfNull(value);
        writer.WriteStartObject();
        writer.WriteString("backend", value.Backend);
        writer.WriteString("name", value.Name);
        writer.WriteEndObject();
    }
}

internal sealed class OfflineActiveTransferVerifierJsonConverter
    : JsonConverter<OfflineActiveTransferVerifier>
{
    private static readonly string[] RequiredProperties =
    [
        "id",
        "version",
        "circuit_id",
        "commitment",
        "public_inputs_schema_hash",
        "max_proof_bytes",
        "activation_height",
        "withdrawal_height",
    ];

    public override bool HandleNull => true;

    public override OfflineActiveTransferVerifier Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        if (reader.TokenType != JsonTokenType.StartObject)
        {
            throw new JsonException("Offline active transfer verifier must be an object.");
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);
        OfflineVerifierId? id = null;
        uint version = 0;
        string? circuitId = null;
        string? commitment = null;
        string? publicInputsSchemaHash = null;
        uint maxProofBytes = 0;
        ulong activationHeight = 0;
        ulong? withdrawalHeight = null;
        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                foreach (var required in RequiredProperties)
                {
                    if (!seen.Contains(required))
                    {
                        throw new JsonException(
                            $"Offline active transfer verifier requires `{required}`.");
                    }
                }
                try
                {
                    return new OfflineActiveTransferVerifier(
                        id!,
                        version,
                        circuitId!,
                        commitment!,
                        publicInputsSchemaHash!,
                        maxProofBytes,
                        activationHeight,
                        withdrawalHeight);
                }
                catch (ArgumentException exception)
                {
                    throw new JsonException("Offline active transfer verifier is invalid.", exception);
                }
            }

            var property = OfflineReadinessJson.RequireUniqueProperty(
                ref reader,
                seen,
                "Offline active transfer verifier");
            if (!reader.Read())
            {
                throw new JsonException("Offline active transfer verifier property is truncated.");
            }
            switch (property)
            {
                case "id":
                    id = JsonSerializer.Deserialize<OfflineVerifierId>(ref reader, options)
                        ?? throw new JsonException("Offline active transfer verifier.id must not be null.");
                    break;
                case "version":
                    if (reader.TokenType != JsonTokenType.Number || !reader.TryGetUInt32(out version))
                    {
                        throw new JsonException("Offline active transfer verifier.version must be a u32 integer.");
                    }
                    break;
                case "circuit_id":
                    circuitId = OfflineReadinessJson.ReadString(
                        ref reader,
                        "Offline active transfer verifier.circuit_id");
                    break;
                case "commitment":
                    commitment = OfflineReadinessJson.ReadString(
                        ref reader,
                        "Offline active transfer verifier.commitment");
                    break;
                case "public_inputs_schema_hash":
                    publicInputsSchemaHash = OfflineReadinessJson.ReadString(
                        ref reader,
                        "Offline active transfer verifier.public_inputs_schema_hash");
                    break;
                case "max_proof_bytes":
                    if (reader.TokenType != JsonTokenType.Number
                        || !reader.TryGetUInt32(out maxProofBytes))
                    {
                        throw new JsonException(
                            "Offline active transfer verifier.max_proof_bytes must be a u32 integer.");
                    }
                    break;
                case "activation_height":
                    if (reader.TokenType != JsonTokenType.Number
                        || !reader.TryGetUInt64(out activationHeight))
                    {
                        throw new JsonException(
                            "Offline active transfer verifier.activation_height must be a u64 integer.");
                    }
                    break;
                case "withdrawal_height":
                    if (reader.TokenType == JsonTokenType.Null)
                    {
                        withdrawalHeight = null;
                    }
                    else if (reader.TokenType == JsonTokenType.Number
                        && reader.TryGetUInt64(out var decodedWithdrawalHeight))
                    {
                        withdrawalHeight = decodedWithdrawalHeight;
                    }
                    else
                    {
                        throw new JsonException(
                            "Offline active transfer verifier.withdrawal_height must be null or a u64 integer.");
                    }
                    break;
                default:
                    reader.Skip();
                    break;
            }
        }
        throw new JsonException("Offline active transfer verifier object is truncated.");
    }

    public override void Write(
        Utf8JsonWriter writer,
        OfflineActiveTransferVerifier value,
        JsonSerializerOptions options)
    {
        ArgumentNullException.ThrowIfNull(value);
        writer.WriteStartObject();
        writer.WritePropertyName("id");
        JsonSerializer.Serialize(writer, value.Id, options);
        writer.WriteNumber("version", value.Version);
        writer.WriteString("circuit_id", value.CircuitId);
        writer.WriteString("commitment", value.Commitment);
        writer.WriteString("public_inputs_schema_hash", value.PublicInputsSchemaHash);
        writer.WriteNumber("max_proof_bytes", value.MaxProofBytes);
        writer.WriteNumber("activation_height", value.ActivationHeight);
        if (value.WithdrawalHeight.HasValue)
        {
            writer.WriteNumber("withdrawal_height", value.WithdrawalHeight.Value);
        }
        else
        {
            writer.WriteNull("withdrawal_height");
        }
        writer.WriteEndObject();
    }
}

internal sealed class OfflineReadinessJsonConverter : JsonConverter<OfflineReadiness>
{
    public override bool HandleNull => true;

    public override OfflineReadiness Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        if (reader.TokenType != JsonTokenType.StartObject)
        {
            throw new JsonException("Offline readiness response must be an object.");
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);
        string? assetDefinitionId = null;
        uint? assetScale = null;
        ulong evaluatedBlockHeight = 0;
        string? evaluatedBlockHash = null;
        OfflineActiveTransferVerifier? activeTransferVerifier = null;
        OfflineActiveTransferVerifier? activeTopUpShieldVerifier = null;
        bool ready = false;
        List<OfflineReadinessBlocker>? blockers = null;
        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                foreach (var required in new[]
                {
                    "asset_definition_id",
                    "asset_scale",
                    "evaluated_block_height",
                    "evaluated_block_hash",
                    "active_transfer_verifier",
                    "active_topup_shield_verifier",
                    "ready",
                    "blockers",
                })
                {
                    if (!seen.Contains(required))
                    {
                        throw new JsonException($"Offline readiness response requires `{required}`.");
                    }
                }

                try
                {
                    return new OfflineReadiness(
                        assetDefinitionId!,
                        assetScale,
                        evaluatedBlockHeight,
                        evaluatedBlockHash!,
                        activeTransferVerifier,
                        activeTopUpShieldVerifier,
                        ready,
                        blockers!);
                }
                catch (ArgumentException exception)
                {
                    throw new JsonException("Offline readiness response is invalid.", exception);
                }
            }

            var name = OfflineReadinessJson.RequireUniqueProperty(ref reader, seen, "Offline readiness response");
            if (!reader.Read())
            {
                throw new JsonException("Offline readiness response property is truncated.");
            }
            switch (name)
            {
                case "asset_definition_id":
                    assetDefinitionId = OfflineReadinessJson.ReadString(
                        ref reader,
                        "Offline readiness response.asset_definition_id");
                    break;
                case "asset_scale":
                    if (reader.TokenType == JsonTokenType.Null)
                    {
                        assetScale = null;
                    }
                    else if (reader.TokenType == JsonTokenType.Number
                        && reader.TryGetUInt32(out var decodedAssetScale))
                    {
                        assetScale = decodedAssetScale;
                    }
                    else
                    {
                        throw new JsonException(
                            "Offline readiness response.asset_scale must be null or a u32 integer.");
                    }
                    break;
                case "evaluated_block_height":
                    if (reader.TokenType != JsonTokenType.Number || !reader.TryGetUInt64(out evaluatedBlockHeight))
                    {
                        throw new JsonException(
                            "Offline readiness response.evaluated_block_height must be a lossless u64 integer.");
                    }
                    break;
                case "evaluated_block_hash":
                    evaluatedBlockHash = OfflineReadinessJson.ReadString(
                        ref reader,
                        "Offline readiness response.evaluated_block_hash");
                    break;
                case "active_transfer_verifier":
                    activeTransferVerifier = reader.TokenType == JsonTokenType.Null
                        ? null
                        : JsonSerializer.Deserialize<OfflineActiveTransferVerifier>(ref reader, options)
                            ?? throw new JsonException(
                                "Offline readiness response.active_transfer_verifier must not decode to null.");
                    break;
                case "active_topup_shield_verifier":
                    activeTopUpShieldVerifier = reader.TokenType == JsonTokenType.Null
                        ? null
                        : JsonSerializer.Deserialize<OfflineActiveTransferVerifier>(ref reader, options)
                            ?? throw new JsonException(
                                "Offline readiness response.active_topup_shield_verifier must not decode to null.");
                    break;
                case "ready":
                    if (reader.TokenType is not (JsonTokenType.True or JsonTokenType.False))
                    {
                        throw new JsonException("Offline readiness response.ready must be a boolean.");
                    }
                    ready = reader.GetBoolean();
                    break;
                case "blockers":
                    blockers = ReadBlockers(ref reader, options);
                    break;
                default:
                    reader.Skip();
                    break;
            }
        }

        throw new JsonException("Offline readiness response object is truncated.");
    }

    public override void Write(Utf8JsonWriter writer, OfflineReadiness value, JsonSerializerOptions options)
    {
        ArgumentNullException.ThrowIfNull(value);
        writer.WriteStartObject();
        writer.WriteString("asset_definition_id", value.AssetDefinitionId);
        if (value.AssetScale.HasValue)
        {
            writer.WriteNumber("asset_scale", value.AssetScale.Value);
        }
        else
        {
            writer.WriteNull("asset_scale");
        }
        writer.WriteNumber("evaluated_block_height", value.EvaluatedBlockHeight);
        writer.WriteString("evaluated_block_hash", value.EvaluatedBlockHash);
        writer.WritePropertyName("active_transfer_verifier");
        if (value.ActiveTransferVerifier is null)
        {
            writer.WriteNullValue();
        }
        else
        {
            JsonSerializer.Serialize(writer, value.ActiveTransferVerifier, options);
        }
        writer.WritePropertyName("active_topup_shield_verifier");
        if (value.ActiveTopUpShieldVerifier is null)
        {
            writer.WriteNullValue();
        }
        else
        {
            JsonSerializer.Serialize(writer, value.ActiveTopUpShieldVerifier, options);
        }
        writer.WriteBoolean("ready", value.Ready);
        writer.WritePropertyName("blockers");
        writer.WriteStartArray();
        foreach (var blocker in value.Blockers)
        {
            JsonSerializer.Serialize(writer, blocker, options);
        }
        writer.WriteEndArray();
        writer.WriteEndObject();
    }

    private static List<OfflineReadinessBlocker> ReadBlockers(
        ref Utf8JsonReader reader,
        JsonSerializerOptions options)
    {
        if (reader.TokenType != JsonTokenType.StartArray)
        {
            throw new JsonException("Offline readiness response.blockers must be an array.");
        }

        var blockers = new List<OfflineReadinessBlocker>();
        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndArray)
            {
                return blockers;
            }
            var blocker = JsonSerializer.Deserialize<OfflineReadinessBlocker>(ref reader, options)
                ?? throw new JsonException("Offline readiness response.blockers must not contain null.");
            blockers.Add(blocker);
        }
        throw new JsonException("Offline readiness response.blockers is truncated.");
    }
}

internal static class OfflineReadinessJson
{
    internal static string RequireUniqueProperty(
        ref Utf8JsonReader reader,
        HashSet<string> seen,
        string context)
    {
        if (reader.TokenType != JsonTokenType.PropertyName)
        {
            throw new JsonException($"{context} must contain only named properties.");
        }
        var name = reader.GetString()
            ?? throw new JsonException($"{context} contains an invalid property name.");
        if (!seen.Add(name))
        {
            throw new JsonException($"{context}.{name} must not appear more than once.");
        }
        return name;
    }

    internal static string ReadString(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType != JsonTokenType.String)
        {
            throw new JsonException($"{field} must be a string.");
        }
        return reader.GetString() ?? throw new JsonException($"{field} must not be null.");
    }
}
