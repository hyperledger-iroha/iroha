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
        ulong evaluatedBlockHeight = 0;
        string? evaluatedBlockHash = null;
        bool ready = false;
        List<OfflineReadinessBlocker>? blockers = null;
        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                foreach (var required in new[]
                {
                    "asset_definition_id",
                    "evaluated_block_height",
                    "evaluated_block_hash",
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
                        evaluatedBlockHeight,
                        evaluatedBlockHash!,
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
        writer.WriteNumber("evaluated_block_height", value.EvaluatedBlockHeight);
        writer.WriteString("evaluated_block_hash", value.EvaluatedBlockHash);
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
