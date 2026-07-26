using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;
using Hyperledger.Iroha.Address;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiAccountQueryJson
{
    internal static void ValidateAccountSummary(ToriiAccountSummary? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireCanonicalAccountId(response.Id, $"{context}.id");
    }

    internal static void ValidateAccountsPage(ToriiAccountsPage response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        ValidateItems(response.Items, $"{context}.items", ValidateAccountSummary);
        ValidateNonNegativeInt64(response.Total, $"{context}.total");
    }

    internal static void ValidateAssetBalance(ToriiAssetBalance? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireExactNonEmptyText(response.Asset, $"{context}.asset");
        RequireCanonicalAccountId(response.AccountId, $"{context}.account_id");
        RequireExactNonEmptyText(response.Scope, $"{context}.scope");
        RequireExactNonEmptyText(response.AssetName, $"{context}.asset_name");
        RequireOptionalExactNonEmptyText(response.AssetAlias, $"{context}.asset_alias");
        ValidateCanonicalQuantityText(response.Quantity, $"{context}.quantity");
    }

    internal static void ValidateAssetBalancesPage(ToriiAssetBalancesPage response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        ValidateItems(response.Items, $"{context}.items", ValidateAssetBalance);
        ValidateNonNegativeInt64(response.Total, $"{context}.total");
    }

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

    internal static void ValidateTransactionSummary(ToriiTransactionSummary? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireOptionalCanonicalAccountId(response.Authority, $"{context}.authority");
        ValidateOptionalPositiveInt64(response.TimestampMilliseconds, $"{context}.timestamp_ms");
        ToriiSseEventJson.RequireExactSizedHex(response.EntrypointHash, $"{context}.entrypoint_hash", 32);
    }

    internal static void ValidateTransactionsPage(ToriiTransactionsPage response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        ValidateItems(response.Items, $"{context}.items", ValidateTransactionSummary);
        ValidateNonNegativeInt64(response.Total, $"{context}.total");
    }

    internal static ToriiAccountSummary ReadAccountSummary(ref Utf8JsonReader reader, string context)
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
        string? id = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiAccountSummary { Id = RequireString(id, $"{context}.id") };
                    ValidateAccountSummary(response, context);
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
                case "id":
                    id = ReadOptionalString(ref reader, $"{context}.id");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiAccountsPage ReadAccountsPage(ref Utf8JsonReader reader, string context)
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
        List<ToriiAccountSummary>? items = null;
        long? total = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = new ToriiAccountsPage
                {
                    Items = RequireItems(items, context),
                    Total = RequirePageTotal(total, context),
                };
                ValidateAccountsPage(response, context);
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
                    items = ReadItems(ref reader, $"{context}.items", ReadAccountSummary);
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

    internal static ToriiAssetBalance ReadAssetBalance(ref Utf8JsonReader reader, string context)
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
        string? asset = null;
        string? accountId = null;
        string? scope = null;
        string? assetName = null;
        string? assetAlias = null;
        string? quantity = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiAssetBalance
                    {
                        Asset = RequireString(asset, $"{context}.asset"),
                        AccountId = RequireString(accountId, $"{context}.account_id"),
                        Scope = RequireString(scope, $"{context}.scope"),
                        AssetName = RequireString(assetName, $"{context}.asset_name"),
                        AssetAlias = assetAlias,
                        Quantity = RequireString(quantity, $"{context}.quantity"),
                    };
                    ValidateAssetBalance(response, context);
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
                case "asset":
                    asset = ReadOptionalString(ref reader, $"{context}.asset");
                    break;
                case "account_id":
                    accountId = ReadOptionalString(ref reader, $"{context}.account_id");
                    break;
                case "scope":
                    scope = ReadOptionalString(ref reader, $"{context}.scope");
                    break;
                case "asset_name":
                    assetName = ReadOptionalString(ref reader, $"{context}.asset_name");
                    break;
                case "asset_alias":
                    assetAlias = ReadOptionalString(ref reader, $"{context}.asset_alias");
                    break;
                case "quantity":
                    quantity = ReadOptionalString(ref reader, $"{context}.quantity");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiAssetBalancesPage ReadAssetBalancesPage(ref Utf8JsonReader reader, string context)
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
        List<ToriiAssetBalance>? items = null;
        long? total = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = new ToriiAssetBalancesPage
                {
                    Items = RequireItems(items, context),
                    Total = RequirePageTotal(total, context),
                };
                ValidateAssetBalancesPage(response, context);
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
                    items = ReadItems(ref reader, $"{context}.items", ReadAssetBalance);
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

    internal static ToriiTransactionSummary ReadTransactionSummary(ref Utf8JsonReader reader, string context)
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
        string? authority = null;
        long? timestampMilliseconds = null;
        string? entrypointHash = null;
        bool? resultOk = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiTransactionSummary
                    {
                        Authority = authority,
                        TimestampMilliseconds = timestampMilliseconds,
                        EntrypointHash = RequireString(entrypointHash, $"{context}.entrypoint_hash"),
                        ResultOk = RequireBool(resultOk, context, "result_ok"),
                    };
                    ValidateTransactionSummary(response, context);
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
                case "authority":
                    authority = ReadOptionalString(ref reader, $"{context}.authority");
                    break;
                case "timestamp_ms":
                    timestampMilliseconds = ReadNullableInt64(ref reader, $"{context}.timestamp_ms");
                    break;
                case "entrypoint_hash":
                    entrypointHash = ReadOptionalString(ref reader, $"{context}.entrypoint_hash");
                    break;
                case "result_ok":
                    resultOk = ReadBool(ref reader, $"{context}.result_ok");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiTransactionsPage ReadTransactionsPage(ref Utf8JsonReader reader, string context)
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
        List<ToriiTransactionSummary>? items = null;
        long? total = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = new ToriiTransactionsPage
                {
                    Items = RequireItems(items, context),
                    Total = RequirePageTotal(total, context),
                };
                ValidateTransactionsPage(response, context);
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
                    items = ReadItems(ref reader, $"{context}.items", ReadTransactionSummary);
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

    internal static void WriteAccountSummary(Utf8JsonWriter writer, ToriiAccountSummary response, string context)
    {
        ValidateAccountSummary(response, context);

        writer.WriteStartObject();
        writer.WriteString("id", response.Id);
        writer.WriteEndObject();
    }

    internal static void WriteAccountsPage(Utf8JsonWriter writer, ToriiAccountsPage response, string context)
    {
        ValidateAccountsPage(response, context);
        WritePage(writer, "items", response.Items, context, WriteAccountSummary, response.Total);
    }

    internal static void WriteAssetBalance(Utf8JsonWriter writer, ToriiAssetBalance response, string context)
    {
        ValidateAssetBalance(response, context);

        writer.WriteStartObject();
        writer.WriteString("asset", response.Asset);
        writer.WriteString("account_id", response.AccountId);
        writer.WriteString("scope", response.Scope);
        writer.WriteString("asset_name", response.AssetName);
        ToriiVpnJson.WriteNullableString(writer, "asset_alias", response.AssetAlias);
        writer.WriteString("quantity", response.Quantity);
        writer.WriteEndObject();
    }

    internal static void WriteAssetBalancesPage(
        Utf8JsonWriter writer,
        ToriiAssetBalancesPage response,
        string context)
    {
        ValidateAssetBalancesPage(response, context);
        WritePage(writer, "items", response.Items, context, WriteAssetBalance, response.Total);
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

    internal static void WriteTransactionSummary(
        Utf8JsonWriter writer,
        ToriiTransactionSummary response,
        string context)
    {
        ValidateTransactionSummary(response, context);

        writer.WriteStartObject();
        ToriiVpnJson.WriteNullableString(writer, "authority", response.Authority);
        WriteNullableNumber(writer, "timestamp_ms", response.TimestampMilliseconds);
        writer.WriteString("entrypoint_hash", response.EntrypointHash);
        writer.WriteBoolean("result_ok", response.ResultOk);
        writer.WriteEndObject();
    }

    internal static void WriteTransactionsPage(Utf8JsonWriter writer, ToriiTransactionsPage response, string context)
    {
        ValidateTransactionsPage(response, context);
        WritePage(writer, "items", response.Items, context, WriteTransactionSummary, response.Total);
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
            return AccountAddress.Parse(exact, AccountAddress.DefaultChainDiscriminant)
                .ToI105(AccountAddress.DefaultChainDiscriminant);
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

internal sealed class ToriiAccountSummaryJsonConverter : JsonConverter<ToriiAccountSummary>
{
    public override bool HandleNull => true;

    public override ToriiAccountSummary Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiAccountQueryJson.ReadAccountSummary(ref reader, "account summary");
    }

    public override void Write(Utf8JsonWriter writer, ToriiAccountSummary value, JsonSerializerOptions options)
    {
        ToriiAccountQueryJson.WriteAccountSummary(writer, value, "account summary");
    }
}

internal sealed class ToriiAccountsPageJsonConverter : JsonConverter<ToriiAccountsPage>
{
    public override bool HandleNull => true;

    public override ToriiAccountsPage Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiAccountQueryJson.ReadAccountsPage(ref reader, "accounts response");
    }

    public override void Write(Utf8JsonWriter writer, ToriiAccountsPage value, JsonSerializerOptions options)
    {
        ToriiAccountQueryJson.WriteAccountsPage(writer, value, "accounts response");
    }
}

internal sealed class ToriiAssetBalanceJsonConverter : JsonConverter<ToriiAssetBalance>
{
    public override bool HandleNull => true;

    public override ToriiAssetBalance Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiAccountQueryJson.ReadAssetBalance(ref reader, "account asset balance");
    }

    public override void Write(Utf8JsonWriter writer, ToriiAssetBalance value, JsonSerializerOptions options)
    {
        ToriiAccountQueryJson.WriteAssetBalance(writer, value, "account asset balance");
    }
}

internal sealed class ToriiAssetBalancesPageJsonConverter : JsonConverter<ToriiAssetBalancesPage>
{
    public override bool HandleNull => true;

    public override ToriiAssetBalancesPage Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiAccountQueryJson.ReadAssetBalancesPage(ref reader, "account asset balances response");
    }

    public override void Write(Utf8JsonWriter writer, ToriiAssetBalancesPage value, JsonSerializerOptions options)
    {
        ToriiAccountQueryJson.WriteAssetBalancesPage(writer, value, "account asset balances response");
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

internal sealed class ToriiTransactionSummaryJsonConverter : JsonConverter<ToriiTransactionSummary>
{
    public override bool HandleNull => true;

    public override ToriiTransactionSummary Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiAccountQueryJson.ReadTransactionSummary(ref reader, "account transaction summary");
    }

    public override void Write(Utf8JsonWriter writer, ToriiTransactionSummary value, JsonSerializerOptions options)
    {
        ToriiAccountQueryJson.WriteTransactionSummary(writer, value, "account transaction summary");
    }
}

internal sealed class ToriiTransactionsPageJsonConverter : JsonConverter<ToriiTransactionsPage>
{
    public override bool HandleNull => true;

    public override ToriiTransactionsPage Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiAccountQueryJson.ReadTransactionsPage(ref reader, "account transactions response");
    }

    public override void Write(Utf8JsonWriter writer, ToriiTransactionsPage value, JsonSerializerOptions options)
    {
        ToriiAccountQueryJson.WriteTransactionsPage(writer, value, "account transactions response");
    }
}
