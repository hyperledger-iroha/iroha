using System.Text.Json;
using System.Text.Json.Serialization;
using Hyperledger.Iroha.Address;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiAccountAliasLookupJson
{
    internal static void ValidateAccountAliasLookupItem(ToriiAccountAliasLookupItem? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        ToriiSseEventJson.RequireExactTokenText(response.Alias, $"{context}.alias");
        ToriiSseEventJson.RequireExactTokenText(response.Dataspace, $"{context}.dataspace");
        ToriiSseEventJson.RequireOptionalExactTokenText(response.Domain, $"{context}.domain");
    }

    internal static void ValidateAccountAliasLookupResponse(
        ToriiAccountAliasLookupResponse response,
        string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        RequireCanonicalAccountId(response.AccountId, $"{context}.account_id");
        ToriiSseEventJson.RequireOptionalExactTokenText(response.Source, $"{context}.source");
        ValidateNonNegativeInt64(response.Total, $"{context}.total");

        if (response.Items is null)
        {
            throw new JsonException($"{context}.items must not be null.");
        }

        for (var index = 0; index < response.Items.Count; index++)
        {
            ValidateAccountAliasLookupItem(response.Items[index], $"{context}.items[{index}]");
        }
    }

    internal static ToriiAccountAliasLookupItem ReadAccountAliasLookupItem(
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
        string? alias = null;
        string? dataspace = null;
        string? domain = null;
        bool? isPrimary = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiAccountAliasLookupItem
                    {
                        Alias = RequireString(alias, $"{context}.alias"),
                        Dataspace = RequireString(dataspace, $"{context}.dataspace"),
                        Domain = domain,
                        IsPrimary = RequireBool(isPrimary, context, "is_primary"),
                    };
                    ValidateAccountAliasLookupItem(response, context);
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
                case "alias":
                    alias = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.alias");
                    break;
                case "dataspace":
                    dataspace = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.dataspace");
                    break;
                case "domain":
                    domain = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.domain");
                    break;
                case "is_primary":
                    isPrimary = ReadBool(ref reader, $"{context}.is_primary");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiAccountAliasLookupResponse ReadAccountAliasLookupResponse(
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
        string? accountId = null;
        List<ToriiAccountAliasLookupItem>? items = null;
        long? total = null;
        string? source = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiAccountAliasLookupResponse
                    {
                        AccountId = RequireString(accountId, $"{context}.account_id"),
                        Items = RequireItems(items, context),
                        Total = RequireTotal(total, context),
                        Source = source,
                    };
                    ValidateAccountAliasLookupResponse(response, context);
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
                case "account_id":
                    accountId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.account_id");
                    break;
                case "items":
                    items = ReadAccountAliasLookupItems(ref reader, $"{context}.items");
                    break;
                case "total":
                    total = ReadInt64(ref reader, $"{context}.total");
                    break;
                case "source":
                    source = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.source");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static void WriteAccountAliasLookupItem(
        Utf8JsonWriter writer,
        ToriiAccountAliasLookupItem response,
        string context)
    {
        ValidateAccountAliasLookupItem(response, context);

        writer.WriteStartObject();
        writer.WriteString("alias", response.Alias);
        writer.WriteString("dataspace", response.Dataspace);
        ToriiVpnJson.WriteNullableString(writer, "domain", response.Domain);
        writer.WriteBoolean("is_primary", response.IsPrimary);
        writer.WriteEndObject();
    }

    internal static void WriteAccountAliasLookupResponse(
        Utf8JsonWriter writer,
        ToriiAccountAliasLookupResponse response,
        string context)
    {
        ValidateAccountAliasLookupResponse(response, context);

        writer.WriteStartObject();
        writer.WriteString("account_id", response.AccountId);
        writer.WritePropertyName("items");
        writer.WriteStartArray();
        for (var index = 0; index < response.Items.Count; index++)
        {
            WriteAccountAliasLookupItem(writer, response.Items[index], $"{context}.items[{index}]");
        }

        writer.WriteEndArray();
        writer.WriteNumber("total", response.Total);
        ToriiVpnJson.WriteNullableString(writer, "source", response.Source);
        writer.WriteEndObject();
    }

    internal static JsonException DirectMetadataErrorToJsonException(ArgumentException error, string context)
    {
        var field = error.ParamName switch
        {
            "Alias" => "alias",
            "Dataspace" => "dataspace",
            "Domain" => "domain",
            "AccountId" => "account_id",
            "Source" => "source",
            _ => error.ParamName,
        };
        return new JsonException($"{context}.{field}: {error.Message}", error);
    }

    private static List<ToriiAccountAliasLookupItem>? ReadAccountAliasLookupItems(
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

        var items = new List<ToriiAccountAliasLookupItem>();
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

            items.Add(ReadAccountAliasLookupItem(ref reader, $"{context}[{index}]"));
            index++;
        }

        throw new JsonException($"{context} array is incomplete.");
    }

    private static IReadOnlyList<ToriiAccountAliasLookupItem> RequireItems(
        IReadOnlyList<ToriiAccountAliasLookupItem>? items,
        string context)
    {
        if (items is null)
        {
            throw new JsonException($"{context}.items must not be null.");
        }

        return items;
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

    private static string RequireString(string? value, string field)
    {
        if (value is null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        return value;
    }

    private static long ReadInt64(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType != JsonTokenType.Number || !reader.TryGetInt64(out var value))
        {
            throw new JsonException($"{field} must be an integer.");
        }

        return value;
    }

    private static void ValidateNonNegativeInt64(long value, string field)
    {
        if (value < 0)
        {
            throw new JsonException($"{field} must be non-negative.");
        }
    }

    private static long RequireTotal(long? value, string context)
    {
        if (!value.HasValue)
        {
            throw new JsonException($"{context}.total must not be null.");
        }

        return value.Value;
    }

    private static string RequireCanonicalAccountId(string? value, string field)
    {
        var text = ToriiSseEventJson.RequireExactTokenText(value, field);
        try
        {
            _ = AccountAddress.Parse(text);
            return text;
        }
        catch (AccountAddressException exception)
        {
            throw new JsonException($"{field} must be a canonical I105 account id.", exception);
        }
    }
}

internal sealed class ToriiAccountAliasLookupItemJsonConverter : JsonConverter<ToriiAccountAliasLookupItem>
{
    public override bool HandleNull => true;

    public override ToriiAccountAliasLookupItem Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiAccountAliasLookupJson.ReadAccountAliasLookupItem(ref reader, "account alias lookup item");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiAccountAliasLookupItem value,
        JsonSerializerOptions options)
    {
        ToriiAccountAliasLookupJson.WriteAccountAliasLookupItem(writer, value, "account alias lookup item");
    }
}

internal sealed class ToriiAccountAliasLookupResponseJsonConverter : JsonConverter<ToriiAccountAliasLookupResponse>
{
    public override bool HandleNull => true;

    public override ToriiAccountAliasLookupResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiAccountAliasLookupJson.ReadAccountAliasLookupResponse(ref reader, "account alias lookup response");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiAccountAliasLookupResponse value,
        JsonSerializerOptions options)
    {
        ToriiAccountAliasLookupJson.WriteAccountAliasLookupResponse(writer, value, "account alias lookup response");
    }
}
