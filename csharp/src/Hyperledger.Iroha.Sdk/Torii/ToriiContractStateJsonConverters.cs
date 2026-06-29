using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiContractStateJson
{
    internal static void ValidateContractStateEntry(ToriiContractStateEntry? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        ToriiSseEventJson.RequireExactTokenText(response.Path, $"{context}.path");
        ToriiSseEventJson.RequireOptionalExactNonEmptyText(response.DecodeError, $"{context}.decode_error");
        if (!response.Found)
        {
            if (response.ValueBase64 is not null || response.ValueLength is not null || response.ValueJson is not null)
            {
                throw new JsonException($"{context} not-found entries must not contain value material.");
            }

            return;
        }

        if (response.ValueBase64 is null && response.ValueLength is not null)
        {
            throw new JsonException($"{context}.value_len requires value_b64.");
        }

        if (response.ValueBase64 is null)
        {
            return;
        }

        var valueBytes = DecodeExactBase64AllowEmpty(response.ValueBase64, $"{context}.value_b64");
        if (response.ValueLength is not null && response.ValueLength.Value != (ulong)valueBytes.Length)
        {
            throw new JsonException($"{context}.value_len must match decoded value_b64 length.");
        }
    }

    internal static void ValidateContractStateResponse(ToriiContractStateResponse response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ToriiSseEventJson.RequireOptionalExactTokenText(response.Path, $"{context}.path");
        ToriiSseEventJson.RequireOptionalExactTokenText(response.Prefix, $"{context}.prefix");
        if (response.Paths is not null)
        {
            for (var index = 0; index < response.Paths.Count; index++)
            {
                ToriiSseEventJson.RequireExactTokenText(response.Paths[index], $"{context}.paths[{index}]");
            }
        }

        if (response.Entries is null)
        {
            throw new JsonException($"{context}.entries is required.");
        }

        if ((ulong)response.Entries.Count > response.Limit)
        {
            throw new JsonException($"{context}.entries item count must be less than or equal to limit.");
        }

        if (response.NextOffset.HasValue && response.NextOffset.Value <= response.Offset)
        {
            throw new JsonException($"{context}.next_offset must be greater than offset.");
        }

        for (var index = 0; index < response.Entries.Count; index++)
        {
            ValidateContractStateEntry(response.Entries[index], $"{context}.entries[{index}]");
        }
    }

    internal static void WriteContractStateEntry(
        Utf8JsonWriter writer,
        ToriiContractStateEntry response,
        string context)
    {
        ValidateContractStateEntry(response, context);

        writer.WriteStartObject();
        writer.WriteString("path", response.Path);
        writer.WriteBoolean("found", response.Found);
        ToriiVpnJson.WriteNullableString(writer, "value_b64", response.ValueBase64);
        WriteNullableUInt64(writer, "value_len", response.ValueLength);
        writer.WritePropertyName("value_json");
        if (response.ValueJson is null)
        {
            writer.WriteNullValue();
        }
        else
        {
            response.ValueJson.WriteTo(writer);
        }

        ToriiVpnJson.WriteNullableString(writer, "decode_error", response.DecodeError);
        writer.WriteEndObject();
    }

    internal static void WriteContractStateResponse(
        Utf8JsonWriter writer,
        ToriiContractStateResponse response,
        string context)
    {
        ValidateContractStateResponse(response, context);

        writer.WriteStartObject();
        ToriiVpnJson.WriteNullableString(writer, "path", response.Path);
        writer.WritePropertyName("paths");
        if (response.Paths is null)
        {
            writer.WriteNullValue();
        }
        else
        {
            writer.WriteStartArray();
            foreach (var path in response.Paths)
            {
                writer.WriteStringValue(path);
            }

            writer.WriteEndArray();
        }

        ToriiVpnJson.WriteNullableString(writer, "prefix", response.Prefix);
        writer.WritePropertyName("entries");
        writer.WriteStartArray();
        for (var index = 0; index < response.Entries.Count; index++)
        {
            WriteContractStateEntry(writer, response.Entries[index], $"{context}.entries[{index}]");
        }

        writer.WriteEndArray();
        writer.WriteNumber("offset", response.Offset);
        writer.WriteNumber("limit", response.Limit);
        WriteNullableUInt64(writer, "next_offset", response.NextOffset);
        writer.WriteEndObject();
    }

    internal static ToriiContractStateEntry ReadContractStateEntry(
        ref Utf8JsonReader reader,
        string context)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        if (reader.TokenType != JsonTokenType.StartObject)
        {
            throw new JsonException($"{context} must be an object.");
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);
        string? path = null;
        bool? found = null;
        string? valueBase64 = null;
        ulong? valueLength = null;
        JsonNode? valueJson = null;
        string? decodeError = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiContractStateEntry
                    {
                        Path = RequireString(path, $"{context}.path"),
                        Found = RequireBool(found, context, "found"),
                        ValueBase64 = valueBase64,
                        ValueLength = valueLength,
                        ValueJson = valueJson,
                        DecodeError = decodeError,
                    };
                    ValidateContractStateEntry(response, context);
                    return response;
                }
                catch (ArgumentException error) when (error.ParamName is not null)
                {
                    throw DirectMetadataErrorToJsonException(error, context);
                }
            }

            if (reader.TokenType != JsonTokenType.PropertyName)
            {
                throw new JsonException($"{context} property name expected.");
            }

            var propertyName = reader.GetString() ?? throw new JsonException($"{context} property name must be a string.");
            ToriiIdentifierJson.RequireUniqueProperty(seen, propertyName, context);
            if (!reader.Read())
            {
                throw new JsonException($"{context}.{propertyName} is truncated.");
            }

            switch (propertyName)
            {
                case "path":
                    path = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.path");
                    break;
                case "found":
                    found = ReadBool(ref reader, $"{context}.found");
                    break;
                case "value_b64":
                    valueBase64 = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.value_b64");
                    break;
                case "value_len":
                    valueLength = ReadOptionalUInt64(ref reader, $"{context}.value_len");
                    break;
                case "value_json":
                    valueJson = ToriiIdentifierJson.ReadOptionalNode(ref reader, $"{context}.value_json");
                    break;
                case "decode_error":
                    decodeError = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.decode_error");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiContractStateResponse ReadContractStateResponse(
        ref Utf8JsonReader reader,
        string context)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        if (reader.TokenType != JsonTokenType.StartObject)
        {
            throw new JsonException($"{context} must be an object.");
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);
        string? path = null;
        List<string>? paths = null;
        string? prefix = null;
        List<ToriiContractStateEntry>? entries = null;
        ulong? offset = null;
        ulong? limit = null;
        ulong? nextOffset = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiContractStateResponse
                    {
                        Path = path,
                        Paths = paths,
                        Prefix = prefix,
                        Entries = RequireEntries(entries, context),
                        Offset = RequireUInt64(offset, context, "offset"),
                        Limit = RequireUInt64(limit, context, "limit"),
                        NextOffset = nextOffset,
                    };
                    ValidateContractStateResponse(response, context);
                    return response;
                }
                catch (ArgumentException error) when (error.ParamName is not null)
                {
                    throw DirectMetadataErrorToJsonException(error, context);
                }
            }

            if (reader.TokenType != JsonTokenType.PropertyName)
            {
                throw new JsonException($"{context} property name expected.");
            }

            var propertyName = reader.GetString() ?? throw new JsonException($"{context} property name must be a string.");
            ToriiIdentifierJson.RequireUniqueProperty(seen, propertyName, context);
            if (!reader.Read())
            {
                throw new JsonException($"{context}.{propertyName} is truncated.");
            }

            switch (propertyName)
            {
                case "path":
                    path = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.path");
                    break;
                case "paths":
                    paths = ReadStringList(ref reader, $"{context}.paths");
                    break;
                case "prefix":
                    prefix = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.prefix");
                    break;
                case "entries":
                    entries = ReadContractStateEntryList(ref reader, $"{context}.entries");
                    break;
                case "offset":
                    offset = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.offset");
                    break;
                case "limit":
                    limit = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.limit");
                    break;
                case "next_offset":
                    nextOffset = ReadOptionalUInt64(ref reader, $"{context}.next_offset");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static JsonException DirectMetadataErrorToJsonException(ArgumentException error, string context)
    {
        var paramName = error.ParamName ?? "metadata";
        var field = paramName switch
        {
            nameof(ToriiContractStateEntry.Path) => "path",
            nameof(ToriiContractStateEntry.ValueBase64) => "value_b64",
            nameof(ToriiContractStateEntry.DecodeError) => "decode_error",
            nameof(ToriiContractStateResponse.Prefix) => "prefix",
            nameof(ToriiContractStateResponse.Entries) => "entries",
            nameof(ToriiContractStateResponse.Offset) => "offset",
            nameof(ToriiContractStateResponse.Limit) => "limit",
            nameof(ToriiContractStateResponse.NextOffset) => "next_offset",
            _ when paramName.StartsWith(
                nameof(ToriiContractStateResponse.Paths) + "[",
                StringComparison.Ordinal) => "paths" + paramName[nameof(ToriiContractStateResponse.Paths).Length..],
            _ => paramName,
        };

        return new JsonException($"{context}.{field}: {error.Message}", error);
    }

    private static IReadOnlyList<ToriiContractStateEntry> RequireEntries(
        IReadOnlyList<ToriiContractStateEntry>? entries,
        string context)
    {
        if (entries is null)
        {
            throw new JsonException($"{context}.entries is required.");
        }

        return entries;
    }

    private static List<string>? ReadStringList(ref Utf8JsonReader reader, string context)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            return null;
        }

        if (reader.TokenType != JsonTokenType.StartArray)
        {
            throw new JsonException($"{context} must be an array.");
        }

        var values = new List<string>();
        var index = 0;
        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndArray)
            {
                return values;
            }

            values.Add(RequireString(
                ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}[{index}]"),
                $"{context}[{index}]"));
            index++;
        }

        throw new JsonException($"{context} array is incomplete.");
    }

    private static List<ToriiContractStateEntry>? ReadContractStateEntryList(
        ref Utf8JsonReader reader,
        string context)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            return null;
        }

        if (reader.TokenType != JsonTokenType.StartArray)
        {
            throw new JsonException($"{context} must be an array.");
        }

        var entries = new List<ToriiContractStateEntry>();
        var index = 0;
        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndArray)
            {
                return entries;
            }

            if (reader.TokenType == JsonTokenType.Null)
            {
                throw new JsonException($"{context}[{index}] must not be null.");
            }

            entries.Add(ReadContractStateEntry(ref reader, $"{context}[{index}]"));
            index++;
        }

        throw new JsonException($"{context} array is incomplete.");
    }

    private static bool ReadBool(ref Utf8JsonReader reader, string field)
    {
        return reader.TokenType switch
        {
            JsonTokenType.True => true,
            JsonTokenType.False => false,
            _ => throw new JsonException($"{field} must be a boolean."),
        };
    }

    private static bool RequireBool(bool? value, string context, string propertyName)
    {
        if (!value.HasValue)
        {
            throw new JsonException($"{context}.{propertyName} must not be null.");
        }

        return value.Value;
    }

    private static ulong? ReadOptionalUInt64(ref Utf8JsonReader reader, string field)
    {
        return reader.TokenType == JsonTokenType.Null ? null : ToriiAccountFaucetJson.ReadUInt64(ref reader, field);
    }

    private static ulong RequireUInt64(ulong? value, string context, string propertyName)
    {
        if (!value.HasValue)
        {
            throw new JsonException($"{context}.{propertyName} must not be null.");
        }

        return value.Value;
    }

    private static string RequireString(string? value, string field)
    {
        if (value is null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        return value;
    }

    private static void WriteNullableUInt64(Utf8JsonWriter writer, string propertyName, ulong? value)
    {
        if (value.HasValue)
        {
            writer.WriteNumber(propertyName, value.Value);
            return;
        }

        writer.WriteNull(propertyName);
    }

    private static byte[] DecodeExactBase64AllowEmpty(string value, string field)
    {
        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must not contain surrounding whitespace.");
        }

        foreach (var character in value)
        {
            if (char.IsWhiteSpace(character))
            {
                throw new JsonException($"{field} must not contain whitespace.");
            }

            if (char.IsControl(character))
            {
                throw new JsonException($"{field} must not contain control characters.");
            }
        }

        byte[] bytes;
        try
        {
            bytes = Convert.FromBase64String(value);
        }
        catch (FormatException error)
        {
            throw new JsonException($"{field} must be valid base64.", error);
        }

        if (!string.Equals(Convert.ToBase64String(bytes), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must be canonical base64 text.");
        }

        return bytes;
    }
}

internal sealed class ToriiContractStateEntryJsonConverter : JsonConverter<ToriiContractStateEntry>
{
    public override bool HandleNull => true;

    public override ToriiContractStateEntry Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiContractStateJson.ReadContractStateEntry(ref reader, "contract state entry");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiContractStateEntry value,
        JsonSerializerOptions options)
    {
        ToriiContractStateJson.WriteContractStateEntry(writer, value, "contract state entry");
    }
}

internal sealed class ToriiContractStateResponseJsonConverter : JsonConverter<ToriiContractStateResponse>
{
    public override bool HandleNull => true;

    public override ToriiContractStateResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiContractStateJson.ReadContractStateResponse(ref reader, "contract state response");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiContractStateResponse value,
        JsonSerializerOptions options)
    {
        ToriiContractStateJson.WriteContractStateResponse(writer, value, "contract state response");
    }
}
