using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;
using Hyperledger.Iroha.Address;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiAccountQueryJson
{
    internal static void ValidateAccountPermission(ToriiAccountPermission? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireExactNonEmptyText(response.Name, $"{context}.name");
    }

    internal static void ValidateAccountPermissionsPage(ToriiAccountPermissionsPage response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        ValidateItems(response.Items, $"{context}.items", ValidateAccountPermission);
        ValidateNonNegativeInt64(response.Total, $"{context}.total");
    }

    internal static ToriiAccountPermission ReadAccountPermission(ref Utf8JsonReader reader, string context)
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
        string? name = null;
        JsonNode? payload = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiAccountPermission
                    {
                        Name = RequireString(name, $"{context}.name"),
                        Payload = payload,
                    };
                    ValidateAccountPermission(response, context);
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
                case "name":
                    name = ReadOptionalString(ref reader, $"{context}.name");
                    break;
                case "payload":
                    payload = ToriiIdentifierJson.ReadOptionalNode(ref reader, $"{context}.payload");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiAccountPermissionsPage ReadAccountPermissionsPage(
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
        List<ToriiAccountPermission>? items = null;
        long? total = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = new ToriiAccountPermissionsPage
                {
                    Items = RequireItems(items, context),
                    Total = RequirePageTotal(total, context),
                };
                ValidateAccountPermissionsPage(response, context);
                return response;
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
                case "items":
                    items = ReadItems(ref reader, $"{context}.items", ReadAccountPermission);
                    break;
                case "total":
                    total = ReadInt64(ref reader, $"{context}.total");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static void WriteAccountPermission(
        Utf8JsonWriter writer,
        ToriiAccountPermission response,
        string context)
    {
        ValidateAccountPermission(response, context);

        writer.WriteStartObject();
        writer.WriteString("name", response.Name);
        writer.WritePropertyName("payload");
        if (response.Payload is null)
        {
            writer.WriteNullValue();
        }
        else
        {
            response.Payload.WriteTo(writer);
        }
        writer.WriteEndObject();
    }

    internal static void WriteAccountPermissionsPage(
        Utf8JsonWriter writer,
        ToriiAccountPermissionsPage response,
        string context)
    {
        ValidateAccountPermissionsPage(response, context);
        WritePage(writer, "items", response.Items, context, WriteAccountPermission, response.Total);
    }

    internal static JsonException DirectMetadataErrorToJsonException(ArgumentException error, string context)
    {
        var field = error.ParamName switch
        {
            "Id" => "id",
            "Asset" => "asset",
            "AccountId" => "account_id",
            "Scope" => "scope",
            "AssetName" => "asset_name",
            "AssetAlias" => "asset_alias",
            "Quantity" => "quantity",
            "Name" => "name",
            "Authority" => "authority",
            "TimestampMilliseconds" => "timestamp_ms",
            "EntrypointHash" => "entrypoint_hash",
            _ => error.ParamName,
        };
        return new JsonException($"{context}.{field}: {error.Message}", error);
    }

    private delegate T ReadItem<T>(ref Utf8JsonReader reader, string context);

    private delegate void WriteItem<T>(Utf8JsonWriter writer, T item, string context);

    private static List<T>? ReadItems<T>(
        ref Utf8JsonReader reader,
        string context,
        ReadItem<T> readItem)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            return null;
        }

        if (reader.TokenType != JsonTokenType.StartArray)
        {
            throw new JsonException($"{context} must be an array.");
        }

        var items = new List<T>();
        var index = 0;
        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndArray)
            {
                return items;
            }

            if (reader.TokenType == JsonTokenType.Null)
            {
                throw new JsonException($"{context}[{index}] must not be null.");
            }

            items.Add(readItem(ref reader, $"{context}[{index}]"));
            index++;
        }

        throw new JsonException($"{context} array is incomplete.");
    }

    private static void ValidateItems<T>(
        IReadOnlyList<T>? items,
        string context,
        Action<T?, string> validateItem)
        where T : class
    {
        if (items is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        for (var index = 0; index < items.Count; index++)
        {
            validateItem(items[index], $"{context}[{index}]");
        }
    }

    private static IReadOnlyList<T> RequireItems<T>(IReadOnlyList<T>? items, string context)
    {
        if (items is null)
        {
            throw new JsonException($"{context}.items must not be null.");
        }

        return items;
    }

    private static void WritePage<T>(
        Utf8JsonWriter writer,
        string itemsPropertyName,
        IReadOnlyList<T> items,
        string context,
        WriteItem<T> writeItem,
        long total)
    {
        writer.WriteStartObject();
        writer.WritePropertyName(itemsPropertyName);
        writer.WriteStartArray();
        for (var index = 0; index < items.Count; index++)
        {
            writeItem(writer, items[index], $"{context}.{itemsPropertyName}[{index}]");
        }
        writer.WriteEndArray();
        writer.WriteNumber("total", total);
        writer.WriteEndObject();
    }

    private static string? ReadOptionalString(ref Utf8JsonReader reader, string field)
    {
        return ToriiAccountFaucetJson.ReadOptionalString(ref reader, field);
    }

    private static string RequireString(string? value, string field)
    {
        if (value is null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        return value;
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

    private static long ReadInt64(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType != JsonTokenType.Number || !reader.TryGetInt64(out var value))
        {
            throw new JsonException($"{field} must be an integer.");
        }

        return value;
    }

    private static long? ReadNullableInt64(ref Utf8JsonReader reader, string field)
    {
        return reader.TokenType == JsonTokenType.Null ? null : ReadInt64(ref reader, field);
    }

    private static long RequirePageTotal(long? value, string context)
    {
        if (!value.HasValue)
        {
            throw new JsonException($"{context}.total must not be null.");
        }

        return value.Value;
    }

    private static string RequireExactNonEmptyText(string? value, string field)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new JsonException($"{field} must be a non-empty string.");
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

    private static void RequireOptionalExactNonEmptyText(string? value, string field)
    {
        if (value is not null)
        {
            RequireExactNonEmptyText(value, field);
        }
    }

    private static string RequireCanonicalAccountId(string? value, string field)
    {
        var exact = RequireExactNonEmptyText(value, field);
        if (exact.Any(char.IsWhiteSpace))
        {
            throw new JsonException($"{field} must not contain whitespace.");
        }

        try
        {
            _ = AccountAddress.Parse(exact);
            return exact;
        }
        catch (AccountAddressException exception)
        {
            throw new JsonException($"{field} must be a canonical I105 account id.", exception);
        }
    }

    private static void RequireOptionalCanonicalAccountId(string? value, string field)
    {
        if (value is not null)
        {
            RequireCanonicalAccountId(value, field);
        }
    }

    private static void ValidateCanonicalQuantityText(string? value, string field)
    {
        _ = RequireExactNonEmptyText(value, field);
        _ = ToriiQuantityJson.RequireCanonicalQuantity(value, field);
    }

    private static void ValidateOptionalNonNegativeInt64(long? value, string field)
    {
        if (value is long integer)
        {
            ValidateNonNegativeInt64(integer, field);
        }
    }

    private static void ValidateNonNegativeInt64(long value, string field)
    {
        if (value < 0)
        {
            throw new JsonException($"{field} must be non-negative.");
        }
    }

    private static void ValidateOptionalPositiveInt64(long? value, string field)
    {
        if (value is long integer)
        {
            ValidatePositiveInt64(integer, field);
        }
    }

    private static void ValidatePositiveInt64(long value, string field)
    {
        if (value <= 0)
        {
            throw new JsonException($"{field} must be positive.");
        }
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

    private static void WriteNullableNumber(Utf8JsonWriter writer, string propertyName, long? value)
    {
        if (value is long integer)
        {
            writer.WriteNumber(propertyName, integer);
        }
        else
        {
            writer.WriteNull(propertyName);
        }
    }
}

internal sealed class ToriiAccountPermissionJsonConverter : JsonConverter<ToriiAccountPermission>
{
    public override bool HandleNull => true;

    public override ToriiAccountPermission Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiAccountQueryJson.ReadAccountPermission(ref reader, "account permission");
    }

    public override void Write(Utf8JsonWriter writer, ToriiAccountPermission value, JsonSerializerOptions options)
    {
        ToriiAccountQueryJson.WriteAccountPermission(writer, value, "account permission");
    }
}

internal sealed class ToriiAccountPermissionsPageJsonConverter : JsonConverter<ToriiAccountPermissionsPage>
{
    public override bool HandleNull => true;

    public override ToriiAccountPermissionsPage Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiAccountQueryJson.ReadAccountPermissionsPage(ref reader, "account permissions response");
    }

    public override void Write(Utf8JsonWriter writer, ToriiAccountPermissionsPage value, JsonSerializerOptions options)
    {
        ToriiAccountQueryJson.WriteAccountPermissionsPage(writer, value, "account permissions response");
    }
}
