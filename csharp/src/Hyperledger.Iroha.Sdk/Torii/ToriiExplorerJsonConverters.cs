using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;
using Hyperledger.Iroha.Address;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiExplorerJson
{
    internal static void ValidateExplorerBlock(ToriiExplorerBlock? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        ToriiSseEventJson.RequireExactSizedHex(response.Hash, $"{context}.hash", 32);
        RequireExactNonEmptyText(response.CreatedAt, $"{context}.created_at");
        ToriiSseEventJson.RequireOptionalExactSizedHex(response.PreviousBlockHash, $"{context}.prev_block_hash", 32);
        ToriiSseEventJson.RequireOptionalExactSizedHex(response.TransactionsHash, $"{context}.transactions_hash", 32);
    }

    internal static void ValidateExplorerTransaction(ToriiExplorerTransaction? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireCanonicalAccountId(response.Authority, $"{context}.authority");
        ToriiSseEventJson.RequireExactSizedHex(response.Hash, $"{context}.hash", 32);
        RequireExactNonEmptyText(response.CreatedAt, $"{context}.created_at");
        RequireExactNonEmptyText(response.Executable, $"{context}.executable");
        RequireExactNonEmptyText(response.Status, $"{context}.status");
    }

    internal static void ValidateExplorerTransactionDetail(
        ToriiExplorerTransactionDetail response,
        string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        RequireCanonicalAccountId(response.Authority, $"{context}.authority");
        ToriiSseEventJson.RequireExactSizedHex(response.Hash, $"{context}.hash", 32);
        RequireExactNonEmptyText(response.CreatedAt, $"{context}.created_at");
        RequireExactNonEmptyText(response.Executable, $"{context}.executable");
        RequireExactNonEmptyText(response.Status, $"{context}.status");
        RequireExactEvenLengthHex(response.Signature, $"{context}.signature");

        if (response.RejectionReason is not null)
        {
            ValidateExplorerTransactionRejection(response.RejectionReason, $"{context}.rejection_reason");
        }
    }

    internal static void ValidateExplorerTransactionRejection(
        ToriiExplorerTransactionRejection? response,
        string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireExactHex(response.Encoded, $"{context}.encoded");
        RequireExactNonEmptyText(response.Message, $"{context}.message");
    }

    internal static void ValidateExplorerInstruction(ToriiExplorerInstruction? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireCanonicalAccountId(response.Authority, $"{context}.authority");
        RequireExactNonEmptyText(response.CreatedAt, $"{context}.created_at");
        RequireExactNonEmptyText(response.Kind, $"{context}.kind");
        ToriiSseEventJson.RequireExactSizedHex(response.TransactionHash, $"{context}.transaction_hash", 32);
        RequireExactNonEmptyText(response.TransactionStatus, $"{context}.transaction_status");

        if (response.InstructionBox is null)
        {
            throw new JsonException($"{context}.box must not be null.");
        }

        ValidateExplorerInstructionBox(response.InstructionBox, $"{context}.box");
    }

    internal static void ValidateExplorerInstructionBox(ToriiExplorerInstructionBox response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        RequireExactHex(response.Encoded, $"{context}.encoded");
        if (response.Json is null)
        {
            throw new JsonException($"{context}.json must not be null.");
        }

        ValidateExplorerInstructionJson(response.Json, $"{context}.json");
    }

    internal static void ValidateExplorerInstructionJson(ToriiExplorerInstructionJson response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        RequireExactNonEmptyText(response.Kind, $"{context}.kind");
        RequireExactNonEmptyText(response.WireId, $"{context}.wire_id");
        RequireExactEvenLengthHex(response.Encoded, $"{context}.encoded");
    }

    internal static void ValidateExplorerBlocksPage(ToriiExplorerBlocksPage response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        ValidateExplorerPagination(response.Pagination, $"{context}.pagination");
        ValidateExplorerItems(response.Items, $"{context}.items", ValidateExplorerBlock);
    }

    internal static void ValidateExplorerTransactionsPage(ToriiExplorerTransactionsPage response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        ValidateExplorerPagination(response.Pagination, $"{context}.pagination");
        ValidateExplorerItems(response.Items, $"{context}.items", ValidateExplorerTransaction);
    }

    internal static void ValidateExplorerLatestTransactionsResponse(
        ToriiExplorerLatestTransactionsResponse response,
        string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        RequireExactNonEmptyText(response.SampledAt, $"{context}.sampled_at");
        ValidateExplorerItems(response.Items, $"{context}.items", ValidateExplorerTransaction);
    }

    internal static void ValidateExplorerInstructionsPage(ToriiExplorerInstructionsPage response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        ValidateExplorerPagination(response.Pagination, $"{context}.pagination");
        ValidateExplorerItems(response.Items, $"{context}.items", ValidateExplorerInstruction);
    }

    internal static void ValidateExplorerLatestInstructionsResponse(
        ToriiExplorerLatestInstructionsResponse response,
        string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        RequireExactNonEmptyText(response.SampledAt, $"{context}.sampled_at");
        ValidateExplorerItems(response.Items, $"{context}.items", ValidateExplorerInstruction);
    }

    internal static void ValidateExplorerPagination(ToriiExplorerPaginationMeta? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        if (response.Page == 0)
        {
            throw new JsonException($"{context}.page must be positive.");
        }

        if (response.PerPage == 0)
        {
            throw new JsonException($"{context}.per_page must be positive.");
        }
    }

    internal static void ValidateExplorerCursor(ToriiExplorerCursorMeta? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        if (response.Limit is 0 or > ToriiExplorerDirectMetadata.ExplorerCursorLimitMaximum)
        {
            throw new JsonException(
                $"{context}.limit must be between 1 and {ToriiExplorerDirectMetadata.ExplorerCursorLimitMaximum}.");
        }

        if (response.NextCursor is not null)
        {
            try
            {
                ToriiExplorerDirectMetadata.RequireCanonicalExplorerCursor(
                    response.NextCursor,
                    nameof(ToriiExplorerCursorMeta.NextCursor));
            }
            catch (ArgumentException error)
            {
                throw DirectMetadataErrorToJsonException(error, context);
            }
        }

        if (response.HasMore != (response.NextCursor is not null))
        {
            throw new JsonException(
                $"{context}.has_more must be true exactly when next_cursor is present.");
        }
    }

    internal static void ValidateExplorerCursorPage<T>(
        ToriiExplorerCursorMeta? pagination,
        IReadOnlyList<T>? items,
        string context)
    {
        ValidateExplorerCursor(pagination, $"{context}.pagination");
        if (items is null)
        {
            throw new JsonException($"{context}.items must not be null.");
        }

        if (items.Count > pagination!.Limit)
        {
            throw new JsonException($"{context}.items must not contain more entries than pagination.limit.");
        }
    }

    internal static void ValidateExplorerAccountsPage(ToriiExplorerAccountsPage response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        ValidateExplorerCursorPage(response.Pagination, response.Items, context);
        ValidateExplorerItems(response.Items, $"{context}.items", ValidateExplorerAccount);
    }

    internal static void ValidateExplorerAccount(ToriiExplorerAccount? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireCanonicalAccountId(response.Id, $"{context}.id");
        RequireCanonicalAccountId(response.I105Address, $"{context}.i105_address");
    }

    internal static void ValidateExplorerDomainsPage(ToriiExplorerDomainsPage response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        ValidateExplorerCursorPage(response.Pagination, response.Items, context);
        ValidateExplorerItems(response.Items, $"{context}.items", ValidateExplorerDomain);
    }

    internal static void ValidateExplorerDomain(ToriiExplorerDomain? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireExactTokenText(response.Id, $"{context}.id");
        RequireOptionalExactTokenText(response.Logo, $"{context}.logo");
        RequireCanonicalAccountId(response.OwnedBy, $"{context}.owned_by");
    }

    internal static void ValidateExplorerAssetDefinitionsPage(
        ToriiExplorerAssetDefinitionsPage response,
        string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        ValidateExplorerCursorPage(response.Pagination, response.Items, context);
        ValidateExplorerItems(response.Items, $"{context}.items", ValidateExplorerAssetDefinition);
    }

    internal static void ValidateExplorerAssetDefinition(ToriiExplorerAssetDefinition? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireExactTokenText(response.Id, $"{context}.id");
        RequireOptionalExactTokenText(response.OwningDomain, $"{context}.owning_domain");
        RequireExactTokenText(response.Mintable, $"{context}.mintable");
        RequireOptionalExactTokenText(response.Logo, $"{context}.logo");
        RequireCanonicalAccountId(response.OwnedBy, $"{context}.owned_by");
        ValidateCanonicalQuantityText(response.TotalQuantity, $"{context}.total_quantity");
        ValidateOptionalCanonicalQuantityText(response.LockedQuantity, $"{context}.locked_quantity");
        ValidateOptionalCanonicalQuantityText(
            response.CirculatingQuantity,
            $"{context}.circulating_quantity");
    }

    internal static void ValidateExplorerAssetDefinitionEconometrics(
        ToriiExplorerAssetDefinitionEconometrics response,
        string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        RequireExactTokenText(response.DefinitionId, $"{context}.definition_id");
        ValidateExplorerItems(response.VelocityWindows, $"{context}.velocity_windows", ValidateExplorerVelocityWindow);
        ValidateExplorerItems(response.IssuanceWindows, $"{context}.issuance_windows", ValidateExplorerIssuanceWindow);
        ValidateExplorerItems(response.IssuanceSeries, $"{context}.issuance_series", ValidateExplorerIssuanceSeriesPoint);
    }

    internal static void ValidateExplorerVelocityWindow(
        ToriiExplorerEconometricsVelocityWindow? response,
        string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireExactTokenText(response.Key, $"{context}.key");
        ValidateCanonicalQuantityText(response.Amount, $"{context}.amount");
    }

    internal static void ValidateExplorerIssuanceWindow(
        ToriiExplorerEconometricsIssuanceWindow? response,
        string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireExactTokenText(response.Key, $"{context}.key");
        ValidateCanonicalQuantityText(response.Minted, $"{context}.minted");
        ValidateCanonicalQuantityText(response.Burned, $"{context}.burned");
        ValidateCanonicalQuantityText(response.Net, $"{context}.net");
    }

    internal static void ValidateExplorerIssuanceSeriesPoint(
        ToriiExplorerEconometricsIssuanceSeriesPoint? response,
        string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        ValidateCanonicalQuantityText(response.Minted, $"{context}.minted");
        ValidateCanonicalQuantityText(response.Burned, $"{context}.burned");
        ValidateCanonicalQuantityText(response.Net, $"{context}.net");
    }

    internal static void ValidateExplorerAssetDefinitionSnapshot(
        ToriiExplorerAssetDefinitionSnapshot response,
        string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        RequireExactTokenText(response.DefinitionId, $"{context}.definition_id");
        ValidateCanonicalQuantityText(response.TotalSupply, $"{context}.total_supply");
        ValidateExplorerItems(response.TopHolders, $"{context}.top_holders", ValidateExplorerTopHolder);
        if (response.Distribution is null)
        {
            throw new JsonException($"{context}.distribution must not be null.");
        }

        ValidateExplorerDistributionSnapshot(response.Distribution, $"{context}.distribution");
    }

    internal static void ValidateExplorerTopHolder(ToriiExplorerEconometricsTopHolder? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireCanonicalAccountId(response.AccountId, $"{context}.account_id");
        ValidateCanonicalQuantityText(response.Balance, $"{context}.balance");
    }

    internal static void ValidateExplorerDistributionSnapshot(
        ToriiExplorerEconometricsDistributionSnapshot? response,
        string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        ValidateFiniteUnitIntervalDouble(response.Gini, $"{context}.gini");
        ValidateFiniteNonNegativeDouble(response.Hhi, $"{context}.hhi");
        ValidateFiniteNonNegativeDouble(response.Theil, $"{context}.theil");
        ValidateFiniteNonNegativeDouble(response.Entropy, $"{context}.entropy");
        ValidateFiniteUnitIntervalDouble(response.EntropyNormalized, $"{context}.entropy_normalized");
        ValidateFiniteUnitIntervalDouble(response.Top1, $"{context}.top1");
        ValidateFiniteUnitIntervalDouble(response.Top5, $"{context}.top5");
        ValidateFiniteUnitIntervalDouble(response.Top10, $"{context}.top10");
        ValidateOptionalCanonicalQuantityText(response.Median, $"{context}.median");
        ValidateOptionalCanonicalQuantityText(response.P90, $"{context}.p90");
        ValidateOptionalCanonicalQuantityText(response.P99, $"{context}.p99");
        ValidateExplorerItems(response.Lorenz, $"{context}.lorenz", ValidateExplorerLorenzPoint);
    }

    internal static void ValidateExplorerLorenzPoint(
        ToriiExplorerEconometricsLorenzPoint? response,
        string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        ValidateFiniteUnitIntervalDouble(response.Population, $"{context}.population");
        ValidateFiniteUnitIntervalDouble(response.Share, $"{context}.share");
    }

    internal static void ValidateExplorerAssetsPage(ToriiExplorerAssetsPage response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        ValidateExplorerCursorPage(response.Pagination, response.Items, context);
        ValidateExplorerItems(response.Items, $"{context}.items", ValidateExplorerAsset);
    }

    internal static void ValidateExplorerAsset(ToriiExplorerAsset? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireExactTokenText(response.Id, $"{context}.id");
        RequireExactTokenText(response.DefinitionId, $"{context}.definition_id");
        RequireCanonicalAccountId(response.AccountId, $"{context}.account_id");
        ValidateCanonicalQuantityText(response.Value, $"{context}.value");
    }

    internal static void ValidateExplorerNftsPage(ToriiExplorerNftsPage response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        ValidateExplorerCursorPage(response.Pagination, response.Items, context);
        ValidateExplorerItems(response.Items, $"{context}.items", ValidateExplorerNft);
    }

    internal static void ValidateExplorerNft(ToriiExplorerNft? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireExactTokenText(response.Id, $"{context}.id");
        RequireCanonicalAccountId(response.OwnedBy, $"{context}.owned_by");
    }

    internal static void ValidateExplorerRwasPage(ToriiExplorerRwasPage response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        ValidateExplorerCursorPage(response.Pagination, response.Items, context);
        ValidateExplorerItems(response.Items, $"{context}.items", ValidateExplorerRwa);
    }

    internal static void ValidateExplorerRwa(ToriiExplorerRwa? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireExactTokenText(response.Id, $"{context}.id");
        RequireCanonicalAccountId(response.OwnedBy, $"{context}.owned_by");
        ValidateCanonicalQuantityText(response.Quantity, $"{context}.quantity");
        ValidateCanonicalQuantityText(response.HeldQuantity, $"{context}.held_quantity");
        RequireExactNonEmptyText(response.PrimaryReference, $"{context}.primary_reference");
        RequireOptionalExactTokenText(response.Status, $"{context}.status");
        ValidateExplorerItems(response.Parents, $"{context}.parents", ValidateExplorerRwaParent);
    }

    internal static void ValidateExplorerRwaParent(ToriiExplorerRwaParent? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireExactTokenText(response.Rwa, $"{context}.rwa");
        ValidateCanonicalQuantityText(response.Quantity, $"{context}.quantity");
    }

    internal static JsonObject ReadObject(ref Utf8JsonReader reader, string context)
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
        var payload = new JsonObject();
        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                return payload;
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

            payload[propertyName] = ReadJsonNode(ref reader, $"{context}.{propertyName}");
        }

        throw new JsonException($"{context} is truncated.");
    }

    internal static void RequireExactProperties(
        JsonObject payload,
        string context,
        params string[] requiredProperties)
    {
        var required = new HashSet<string>(requiredProperties, StringComparer.Ordinal);
        foreach (var property in payload)
        {
            if (!required.Contains(property.Key))
            {
                throw new JsonException($"{context}.{property.Key} is not supported.");
            }
        }

        foreach (var propertyName in requiredProperties)
        {
            if (!payload.ContainsKey(propertyName))
            {
                throw new JsonException($"{context}.{propertyName} must be present.");
            }
        }
    }

    internal static string ReadRequiredString(JsonObject payload, string propertyName, string field)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var value) || value is null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        if (value is JsonValue jsonValue && jsonValue.TryGetValue<string>(out var text))
        {
            return text;
        }

        throw new JsonException($"{field} must be a string.");
    }

    internal static string? ReadOptionalString(JsonObject payload, string propertyName, string field)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var value) || value is null)
        {
            return null;
        }

        if (value is JsonValue jsonValue && jsonValue.TryGetValue<string>(out var text))
        {
            return text;
        }

        throw new JsonException($"{field} must be a string.");
    }

    internal static ulong ReadUInt64OrDefault(JsonObject payload, string propertyName, string field)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var value) || value is null)
        {
            return 0;
        }

        if (value is JsonValue jsonValue && jsonValue.TryGetValue<ulong>(out var number))
        {
            return number;
        }

        throw new JsonException($"{field} must be an unsigned integer.");
    }

    internal static ulong ReadRequiredUInt64(JsonObject payload, string propertyName, string field)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var value) || value is null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        if (value is JsonValue jsonValue && jsonValue.TryGetValue<ulong>(out var number))
        {
            return number;
        }

        throw new JsonException($"{field} must be an unsigned integer.");
    }

    internal static ulong? ReadOptionalUInt64(JsonObject payload, string propertyName, string field)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var value) || value is null)
        {
            return null;
        }

        if (value is JsonValue jsonValue && jsonValue.TryGetValue<ulong>(out var number))
        {
            return number;
        }

        throw new JsonException($"{field} must be an unsigned integer.");
    }

    internal static uint ReadUInt32OrDefault(JsonObject payload, string propertyName, string field)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var value) || value is null)
        {
            return 0;
        }

        if (value is JsonValue jsonValue && jsonValue.TryGetValue<uint>(out var number))
        {
            return number;
        }

        throw new JsonException($"{field} must be an unsigned integer.");
    }

    internal static uint ReadRequiredUInt32(JsonObject payload, string propertyName, string field)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var value) || value is null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        if (value is JsonValue jsonValue && jsonValue.TryGetValue<uint>(out var number))
        {
            return number;
        }

        throw new JsonException($"{field} must be an unsigned integer.");
    }

    internal static ushort ReadRequiredUInt16(JsonObject payload, string propertyName, string field)
    {
        var number = ReadRequiredUInt32(payload, propertyName, field);
        if (number > ushort.MaxValue)
        {
            throw new JsonException($"{field} must fit in an unsigned 16-bit integer.");
        }

        return (ushort)number;
    }

    internal static bool ReadRequiredBool(JsonObject payload, string propertyName, string field)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var value) || value is null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        if (value is JsonValue jsonValue && jsonValue.TryGetValue<bool>(out var boolean))
        {
            return boolean;
        }

        throw new JsonException($"{field} must be a boolean.");
    }

    internal static double ReadRequiredDouble(JsonObject payload, string propertyName, string field)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var value) || value is null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        if (value is JsonValue jsonValue && jsonValue.TryGetValue<double>(out var number))
        {
            return number;
        }

        throw new JsonException($"{field} must be a number.");
    }

    internal static JsonNode? ReadOptionalNode(JsonObject payload, string propertyName)
    {
        return payload.TryGetPropertyValue(propertyName, out var value) ? value?.DeepClone() : null;
    }

    internal static ToriiExplorerPaginationMeta ReadRequiredPagination(
        JsonObject payload,
        string propertyName,
        string field)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var value) || value is null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        if (value is not JsonObject)
        {
            throw new JsonException($"{field} must be an object.");
        }

        try
        {
            return value.Deserialize<ToriiExplorerPaginationMeta>() ?? throw new JsonException($"{field} must not be null.");
        }
        catch (JsonException exception)
        {
            throw RewriteContext(exception, "explorer pagination", field);
        }
    }

    internal static ToriiExplorerCursorMeta ReadRequiredCursorPagination(
        JsonObject payload,
        string propertyName,
        string field)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var value) || value is null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        if (value is not JsonObject)
        {
            throw new JsonException($"{field} must be an object.");
        }

        try
        {
            return value.Deserialize<ToriiExplorerCursorMeta>() ?? throw new JsonException($"{field} must not be null.");
        }
        catch (JsonException exception)
        {
            throw RewriteContext(exception, "explorer cursor pagination", field);
        }
    }

    internal static IReadOnlyList<T> ReadRequiredItems<T>(
        JsonObject payload,
        string propertyName,
        string field,
        string itemContext)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var value) || value is null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        if (value is not JsonArray items)
        {
            throw new JsonException($"{field} must be an array.");
        }

        var result = new List<T>(items.Count);
        for (var index = 0; index < items.Count; index++)
        {
            var item = items[index];
            if (item is null)
            {
                throw new JsonException($"{field}[{index}] must not be null.");
            }

            try
            {
                result.Add(item.Deserialize<T>() ?? throw new JsonException($"{field}[{index}] must not be null."));
            }
            catch (JsonException exception)
            {
                throw RewriteContext(exception, itemContext, $"{field}[{index}]");
            }
        }

        return result;
    }

    internal static IReadOnlyList<T> ReadItemsOrDefault<T>(
        JsonObject payload,
        string propertyName,
        string field,
        string itemContext)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var value))
        {
            return Array.Empty<T>();
        }

        if (value is null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        if (value is not JsonArray items)
        {
            throw new JsonException($"{field} must be an array.");
        }

        var result = new List<T>(items.Count);
        for (var index = 0; index < items.Count; index++)
        {
            var item = items[index];
            if (item is null)
            {
                throw new JsonException($"{field}[{index}] must not be null.");
            }

            try
            {
                result.Add(item.Deserialize<T>() ?? throw new JsonException($"{field}[{index}] must not be null."));
            }
            catch (JsonException exception)
            {
                throw RewriteContext(exception, itemContext, $"{field}[{index}]");
            }
        }

        return result;
    }

    internal static T? ReadOptionalObject<T>(
        JsonObject payload,
        string propertyName,
        string field,
        string nestedContext)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var value) || value is null)
        {
            return default;
        }

        if (value is not JsonObject)
        {
            throw new JsonException($"{field} must be an object.");
        }

        try
        {
            return value.Deserialize<T>();
        }
        catch (JsonException exception)
        {
            throw RewriteContext(exception, nestedContext, field);
        }
    }

    internal static T ReadRequiredObject<T>(
        JsonObject payload,
        string propertyName,
        string field,
        string nestedContext)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var value) || value is null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        if (value is not JsonObject)
        {
            throw new JsonException($"{field} must be an object.");
        }

        try
        {
            return value.Deserialize<T>() ?? throw new JsonException($"{field} must not be null.");
        }
        catch (JsonException exception)
        {
            throw RewriteContext(exception, nestedContext, field);
        }
    }

    internal static void WriteNullableString(Utf8JsonWriter writer, string propertyName, string? value)
    {
        if (value is null)
        {
            writer.WriteNull(propertyName);
            return;
        }

        writer.WriteString(propertyName, value);
    }

    internal static JsonException DirectMetadataErrorToJsonException(ArgumentException error, string context)
    {
        var field = error.ParamName switch
        {
            "Hash" => "hash",
            "CreatedAt" => "created_at",
            "PreviousBlockHash" => "prev_block_hash",
            "TransactionsHash" => "transactions_hash",
            "Authority" => "authority",
            "Executable" => "executable",
            "Status" => "status",
            "Kind" => "kind",
            "WireId" => "wire_id",
            "Encoded" => "encoded",
            "Message" => "message",
            "Json" => "json",
            "InstructionBox" => "box",
            "TransactionHash" => "transaction_hash",
            "TransactionStatus" => "transaction_status",
            "Signature" => "signature",
            "Page" => "page",
            "PerPage" => "per_page",
            "Limit" => "limit",
            "NextCursor" => "next_cursor",
            "SampledAt" => "sampled_at",
            "Id" => "id",
            "OwningDomain" => "owning_domain",
            "I105Address" => "i105_address",
            "Logo" => "logo",
            "OwnedBy" => "owned_by",
            "Mintable" => "mintable",
            "DefinitionId" => "definition_id",
            "TotalQuantity" => "total_quantity",
            "LockedQuantity" => "locked_quantity",
            "CirculatingQuantity" => "circulating_quantity",
            "Key" => "key",
            "Amount" => "amount",
            "Minted" => "minted",
            "Burned" => "burned",
            "Net" => "net",
            "Population" => "population",
            "Share" => "share",
            "Gini" => "gini",
            "Hhi" => "hhi",
            "Theil" => "theil",
            "Entropy" => "entropy",
            "EntropyNormalized" => "entropy_normalized",
            "Top1" => "top1",
            "Top5" => "top5",
            "Top10" => "top10",
            "Median" => "median",
            "P90" => "p90",
            "P99" => "p99",
            "AccountId" => "account_id",
            "Balance" => "balance",
            "TotalSupply" => "total_supply",
            "Distribution" => "distribution",
            "Value" => "value",
            "Rwa" => "rwa",
            "Quantity" => "quantity",
            "HeldQuantity" => "held_quantity",
            "PrimaryReference" => "primary_reference",
            "CanonicalId" => "canonical_id",
            "Literal" => "literal",
            "NetworkPrefix" => "network_prefix",
            "ErrorCorrection" => "error_correction",
            "Modules" => "modules",
            "QrVersion" => "qr_version",
            "Svg" => "svg",
            "HeadCreatedAt" => "head_created_at",
            "BlockCreatedAt" => "block_created_at",
            _ => error.ParamName,
        };
        return new JsonException($"{context}.{field}: {error.Message}", error);
    }

    internal static void WriteItems<T>(
        Utf8JsonWriter writer,
        string propertyName,
        IReadOnlyList<T> values,
        JsonSerializerOptions options)
    {
        writer.WritePropertyName(propertyName);
        writer.WriteStartArray();
        foreach (var value in values)
        {
            JsonSerializer.Serialize(writer, value, options);
        }

        writer.WriteEndArray();
    }

    private static void RequireExactNonEmptyText(string? value, string field)
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
    }

    private static void RequireExactTokenText(string? value, string field)
    {
        RequireExactNonEmptyText(value, field);
        var text = value ?? throw new JsonException($"{field} must not be null.");
        if (ContainsWhitespace(text))
        {
            throw new JsonException($"{field} must not contain whitespace.");
        }
    }

    private static string RequireCanonicalAccountId(string? value, string field)
    {
        RequireExactNonEmptyText(value, field);
        var text = value ?? throw new JsonException($"{field} must not be null.");
        if (ContainsWhitespace(text))
        {
            throw new JsonException($"{field} must not contain whitespace.");
        }

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

    private static void RequireOptionalExactTokenText(string? value, string field)
    {
        if (value is not null)
        {
            RequireExactTokenText(value, field);
        }
    }

    private static void ValidateExplorerItems<T>(
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

    private static void ValidateCanonicalQuantityText(string? value, string field)
    {
        RequireExactNonEmptyText(value, field);
        _ = ToriiQuantityJson.RequireCanonicalQuantity(value, field);
    }

    private static void ValidateOptionalCanonicalQuantityText(string? value, string field)
    {
        if (value is not null)
        {
            ValidateCanonicalQuantityText(value, field);
        }
    }

    private static void ValidateFiniteNonNegativeDouble(double value, string field)
    {
        if (!double.IsFinite(value) || value < 0)
        {
            throw new JsonException($"{field} must be a finite non-negative number.");
        }
    }

    private static void ValidateFiniteUnitIntervalDouble(double value, string field)
    {
        if (!double.IsFinite(value) || value < 0 || value > 1)
        {
            throw new JsonException($"{field} must be a finite number from 0 to 1.");
        }
    }

    private static JsonNode? ReadJsonNode(ref Utf8JsonReader reader, string context)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            return null;
        }

        if (reader.TokenType == JsonTokenType.StartObject)
        {
            var seen = new HashSet<string>(StringComparer.Ordinal);
            var payload = new JsonObject();
            while (reader.Read())
            {
                if (reader.TokenType == JsonTokenType.EndObject)
                {
                    return payload;
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

                payload[propertyName] = ReadJsonNode(ref reader, $"{context}.{propertyName}");
            }

            throw new JsonException($"{context} JSON object is incomplete.");
        }

        if (reader.TokenType == JsonTokenType.StartArray)
        {
            var values = new JsonArray();
            var index = 0;
            while (reader.Read())
            {
                if (reader.TokenType == JsonTokenType.EndArray)
                {
                    return values;
                }

                values.Add(ReadJsonNode(ref reader, $"{context}[{index}]"));
                index++;
            }

            throw new JsonException($"{context} array is incomplete.");
        }

        return JsonNode.Parse(ref reader);
    }

    private static void RequireExactHex(string? value, string field)
    {
        RequireExactNonEmptyText(value, field);
        var exact = value ?? throw new JsonException($"{field} must not be null.");
        if (ContainsWhitespace(exact))
        {
            throw new JsonException($"{field} must not contain whitespace.");
        }

        var body = exact.StartsWith("0x", StringComparison.Ordinal) ? exact[2..] : exact;
        if (body.Length == 0 || body.Length % 2 != 0 || !IsHex(body))
        {
            throw new JsonException($"{field} must be an exact hex string.");
        }
    }

    private static void RequireExactEvenLengthHex(string? value, string field)
    {
        RequireExactNonEmptyText(value, field);
        var exact = value ?? throw new JsonException($"{field} must not be null.");
        if (ContainsWhitespace(exact))
        {
            throw new JsonException($"{field} must not contain whitespace.");
        }

        if (exact.Length % 2 != 0 || !IsLowercaseHex(exact))
        {
            throw new JsonException($"{field} must be an exact lowercase even-length hex string.");
        }
    }

    internal static JsonException RewriteContext(JsonException exception, string from, string to)
    {
        var message = exception.Message;
        if (message.StartsWith(from, StringComparison.Ordinal))
        {
            message = to + message[from.Length..];
        }

        return new JsonException(message, exception);
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

    private static bool IsLowercaseHex(string value)
    {
        foreach (var character in value)
        {
            var isHex =
                character is >= '0' and <= '9'
                || character is >= 'a' and <= 'f';
            if (!isHex)
            {
                return false;
            }
        }

        return true;
    }
}

internal sealed class ToriiExplorerPaginationMetaJsonConverter : JsonConverter<ToriiExplorerPaginationMeta>
{
    public override bool HandleNull => true;

    public override ToriiExplorerPaginationMeta Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer pagination");
        try
        {
            var response = new ToriiExplorerPaginationMeta
            {
                Page = ToriiExplorerJson.ReadRequiredUInt64(payload, "page", "explorer pagination.page"),
                PerPage = ToriiExplorerJson.ReadRequiredUInt64(payload, "per_page", "explorer pagination.per_page"),
                TotalPages = ToriiExplorerJson.ReadRequiredUInt64(payload, "total_pages", "explorer pagination.total_pages"),
                TotalItems = ToriiExplorerJson.ReadRequiredUInt64(payload, "total_items", "explorer pagination.total_items"),
            };
            ToriiExplorerJson.ValidateExplorerPagination(response, "explorer pagination");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, "explorer pagination");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerPaginationMeta value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerPagination(value, "explorer pagination");

        writer.WriteStartObject();
        writer.WriteNumber("page", value.Page);
        writer.WriteNumber("per_page", value.PerPage);
        writer.WriteNumber("total_pages", value.TotalPages);
        writer.WriteNumber("total_items", value.TotalItems);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerCursorMetaJsonConverter : JsonConverter<ToriiExplorerCursorMeta>
{
    public override bool HandleNull => true;

    public override ToriiExplorerCursorMeta Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer cursor pagination");
        ToriiExplorerJson.RequireExactProperties(
            payload,
            "explorer cursor pagination",
            "limit",
            "next_cursor",
            "has_more");
        try
        {
            var response = new ToriiExplorerCursorMeta
            {
                Limit = ToriiExplorerJson.ReadRequiredUInt32(
                    payload,
                    "limit",
                    "explorer cursor pagination.limit"),
                NextCursor = ToriiExplorerJson.ReadOptionalString(
                    payload,
                    "next_cursor",
                    "explorer cursor pagination.next_cursor"),
                HasMore = ToriiExplorerJson.ReadRequiredBool(
                    payload,
                    "has_more",
                    "explorer cursor pagination.has_more"),
            };
            ToriiExplorerJson.ValidateExplorerCursor(response, "explorer cursor pagination");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, "explorer cursor pagination");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerCursorMeta value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerCursor(value, "explorer cursor pagination");

        writer.WriteStartObject();
        writer.WriteNumber("limit", value.Limit);
        ToriiExplorerJson.WriteNullableString(writer, "next_cursor", value.NextCursor);
        writer.WriteBoolean("has_more", value.HasMore);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerAccountJsonConverter : JsonConverter<ToriiExplorerAccount>
{
    public override bool HandleNull => true;

    public override ToriiExplorerAccount Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer account");
        try
        {
            var response = new ToriiExplorerAccount
            {
                Id = ToriiExplorerJson.ReadRequiredString(payload, "id", "explorer account.id"),
                I105Address = ToriiExplorerJson.ReadRequiredString(
                    payload,
                    "i105_address",
                    "explorer account.i105_address"),
                NetworkPrefix = ToriiExplorerJson.ReadRequiredUInt16(
                    payload,
                    "network_prefix",
                    "explorer account.network_prefix"),
                Metadata = ToriiExplorerJson.ReadOptionalNode(payload, "metadata"),
                OwnedDomains = ToriiExplorerJson.ReadRequiredUInt32(
                    payload,
                    "owned_domains",
                    "explorer account.owned_domains"),
                OwnedAssets = ToriiExplorerJson.ReadRequiredUInt32(
                    payload,
                    "owned_assets",
                    "explorer account.owned_assets"),
                OwnedNfts = ToriiExplorerJson.ReadRequiredUInt32(payload, "owned_nfts", "explorer account.owned_nfts"),
            };
            ToriiExplorerJson.ValidateExplorerAccount(response, "explorer account");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, "explorer account");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerAccount value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerAccount(value, "explorer account");

        writer.WriteStartObject();
        writer.WriteString("id", value.Id);
        writer.WriteString("i105_address", value.I105Address);
        writer.WriteNumber("network_prefix", value.NetworkPrefix);
        writer.WritePropertyName("metadata");
        JsonSerializer.Serialize(writer, value.Metadata, options);
        writer.WriteNumber("owned_domains", value.OwnedDomains);
        writer.WriteNumber("owned_assets", value.OwnedAssets);
        writer.WriteNumber("owned_nfts", value.OwnedNfts);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerAccountsPageJsonConverter : JsonConverter<ToriiExplorerAccountsPage>
{
    public override bool HandleNull => true;

    public override ToriiExplorerAccountsPage Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer accounts page");
        ToriiExplorerJson.RequireExactProperties(payload, "explorer accounts page", "pagination", "items");
        var response = new ToriiExplorerAccountsPage
        {
            Pagination = ToriiExplorerJson.ReadRequiredCursorPagination(
                payload,
                "pagination",
                "explorer accounts page.pagination"),
            Items = ToriiExplorerJson.ReadRequiredItems<ToriiExplorerAccount>(
                payload,
                "items",
                "explorer accounts page.items",
                "explorer account"),
        };
        ToriiExplorerJson.ValidateExplorerAccountsPage(response, "explorer accounts page");
        return response;
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerAccountsPage value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerAccountsPage(value, "explorer accounts page");

        writer.WriteStartObject();
        writer.WritePropertyName("pagination");
        JsonSerializer.Serialize(writer, value.Pagination, options);
        ToriiExplorerJson.WriteItems(writer, "items", value.Items, options);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerDomainJsonConverter : JsonConverter<ToriiExplorerDomain>
{
    public override bool HandleNull => true;

    public override ToriiExplorerDomain Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer domain");
        try
        {
            var response = new ToriiExplorerDomain
            {
                Id = ToriiExplorerJson.ReadRequiredString(payload, "id", "explorer domain.id"),
                Logo = ToriiExplorerJson.ReadOptionalString(payload, "logo", "explorer domain.logo"),
                Metadata = ToriiExplorerJson.ReadOptionalNode(payload, "metadata"),
                OwnedBy = ToriiExplorerJson.ReadRequiredString(payload, "owned_by", "explorer domain.owned_by"),
                Accounts = ToriiExplorerJson.ReadRequiredUInt32(payload, "accounts", "explorer domain.accounts"),
                Assets = ToriiExplorerJson.ReadRequiredUInt32(payload, "assets", "explorer domain.assets"),
                Nfts = ToriiExplorerJson.ReadRequiredUInt32(payload, "nfts", "explorer domain.nfts"),
            };
            ToriiExplorerJson.ValidateExplorerDomain(response, "explorer domain");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, "explorer domain");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerDomain value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerDomain(value, "explorer domain");

        writer.WriteStartObject();
        writer.WriteString("id", value.Id);
        ToriiExplorerJson.WriteNullableString(writer, "logo", value.Logo);
        writer.WritePropertyName("metadata");
        JsonSerializer.Serialize(writer, value.Metadata, options);
        writer.WriteString("owned_by", value.OwnedBy);
        writer.WriteNumber("accounts", value.Accounts);
        writer.WriteNumber("assets", value.Assets);
        writer.WriteNumber("nfts", value.Nfts);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerDomainsPageJsonConverter : JsonConverter<ToriiExplorerDomainsPage>
{
    public override bool HandleNull => true;

    public override ToriiExplorerDomainsPage Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer domains page");
        ToriiExplorerJson.RequireExactProperties(payload, "explorer domains page", "pagination", "items");
        var response = new ToriiExplorerDomainsPage
        {
            Pagination = ToriiExplorerJson.ReadRequiredCursorPagination(
                payload,
                "pagination",
                "explorer domains page.pagination"),
            Items = ToriiExplorerJson.ReadRequiredItems<ToriiExplorerDomain>(
                payload,
                "items",
                "explorer domains page.items",
                "explorer domain"),
        };
        ToriiExplorerJson.ValidateExplorerDomainsPage(response, "explorer domains page");
        return response;
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerDomainsPage value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerDomainsPage(value, "explorer domains page");

        writer.WriteStartObject();
        writer.WritePropertyName("pagination");
        JsonSerializer.Serialize(writer, value.Pagination, options);
        ToriiExplorerJson.WriteItems(writer, "items", value.Items, options);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerAssetDefinitionJsonConverter : JsonConverter<ToriiExplorerAssetDefinition>
{
    public override bool HandleNull => true;

    public override ToriiExplorerAssetDefinition Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer asset definition");
        try
        {
            ToriiExplorerJson.RequireExactProperties(
                payload,
                "explorer asset definition",
                "id",
                "owning_domain",
                "mintable",
                "logo",
                "metadata",
                "owned_by",
                "assets",
                "total_quantity",
                "locked_quantity",
                "circulating_quantity");
            var response = new ToriiExplorerAssetDefinition
            {
                Id = ToriiExplorerJson.ReadRequiredString(payload, "id", "explorer asset definition.id"),
                OwningDomain = ToriiExplorerJson.ReadOptionalString(
                    payload,
                    "owning_domain",
                    "explorer asset definition.owning_domain"),
                Mintable = ToriiExplorerJson.ReadRequiredString(
                    payload,
                    "mintable",
                    "explorer asset definition.mintable"),
                Logo = ToriiExplorerJson.ReadOptionalString(payload, "logo", "explorer asset definition.logo"),
                Metadata = ToriiExplorerJson.ReadOptionalNode(payload, "metadata"),
                OwnedBy = ToriiExplorerJson.ReadRequiredString(payload, "owned_by", "explorer asset definition.owned_by"),
                Assets = ToriiExplorerJson.ReadRequiredUInt32(payload, "assets", "explorer asset definition.assets"),
                TotalQuantity = ToriiExplorerJson.ReadRequiredString(
                    payload,
                    "total_quantity",
                    "explorer asset definition.total_quantity"),
                LockedQuantity = ToriiExplorerJson.ReadOptionalString(
                    payload,
                    "locked_quantity",
                    "explorer asset definition.locked_quantity"),
                CirculatingQuantity = ToriiExplorerJson.ReadOptionalString(
                    payload,
                    "circulating_quantity",
                    "explorer asset definition.circulating_quantity"),
            };
            ToriiExplorerJson.ValidateExplorerAssetDefinition(response, "explorer asset definition");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, "explorer asset definition");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerAssetDefinition value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerAssetDefinition(value, "explorer asset definition");

        writer.WriteStartObject();
        writer.WriteString("id", value.Id);
        ToriiExplorerJson.WriteNullableString(writer, "owning_domain", value.OwningDomain);
        writer.WriteString("mintable", value.Mintable);
        ToriiExplorerJson.WriteNullableString(writer, "logo", value.Logo);
        writer.WritePropertyName("metadata");
        JsonSerializer.Serialize(writer, value.Metadata, options);
        writer.WriteString("owned_by", value.OwnedBy);
        writer.WriteNumber("assets", value.Assets);
        writer.WriteString("total_quantity", value.TotalQuantity);
        ToriiExplorerJson.WriteNullableString(writer, "locked_quantity", value.LockedQuantity);
        ToriiExplorerJson.WriteNullableString(writer, "circulating_quantity", value.CirculatingQuantity);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerAssetDefinitionsPageJsonConverter :
    JsonConverter<ToriiExplorerAssetDefinitionsPage>
{
    public override bool HandleNull => true;

    public override ToriiExplorerAssetDefinitionsPage Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer asset definitions page");
        ToriiExplorerJson.RequireExactProperties(
            payload,
            "explorer asset definitions page",
            "pagination",
            "items");
        var response = new ToriiExplorerAssetDefinitionsPage
        {
            Pagination = ToriiExplorerJson.ReadRequiredCursorPagination(
                payload,
                "pagination",
                "explorer asset definitions page.pagination"),
            Items = ToriiExplorerJson.ReadRequiredItems<ToriiExplorerAssetDefinition>(
                payload,
                "items",
                "explorer asset definitions page.items",
                "explorer asset definition"),
        };
        ToriiExplorerJson.ValidateExplorerAssetDefinitionsPage(response, "explorer asset definitions page");
        return response;
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerAssetDefinitionsPage value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerAssetDefinitionsPage(value, "explorer asset definitions page");

        writer.WriteStartObject();
        writer.WritePropertyName("pagination");
        JsonSerializer.Serialize(writer, value.Pagination, options);
        ToriiExplorerJson.WriteItems(writer, "items", value.Items, options);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerEconometricsVelocityWindowJsonConverter :
    JsonConverter<ToriiExplorerEconometricsVelocityWindow>
{
    public override bool HandleNull => true;

    public override ToriiExplorerEconometricsVelocityWindow Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer velocity window");
        try
        {
            var response = new ToriiExplorerEconometricsVelocityWindow
            {
                Key = ToriiExplorerJson.ReadRequiredString(payload, "key", "explorer velocity window.key"),
                StartMilliseconds = ToriiExplorerJson.ReadRequiredUInt64(
                    payload,
                    "start_ms",
                    "explorer velocity window.start_ms"),
                EndMilliseconds = ToriiExplorerJson.ReadRequiredUInt64(
                    payload,
                    "end_ms",
                    "explorer velocity window.end_ms"),
                Transfers = ToriiExplorerJson.ReadRequiredUInt64(
                    payload,
                    "transfers",
                    "explorer velocity window.transfers"),
                UniqueSenders = ToriiExplorerJson.ReadRequiredUInt64(
                    payload,
                    "unique_senders",
                    "explorer velocity window.unique_senders"),
                UniqueReceivers = ToriiExplorerJson.ReadRequiredUInt64(
                    payload,
                    "unique_receivers",
                    "explorer velocity window.unique_receivers"),
                Amount = ToriiExplorerJson.ReadRequiredString(payload, "amount", "explorer velocity window.amount"),
            };
            ToriiExplorerJson.ValidateExplorerVelocityWindow(response, "explorer velocity window");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, "explorer velocity window");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerEconometricsVelocityWindow value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerVelocityWindow(value, "explorer velocity window");

        writer.WriteStartObject();
        writer.WriteString("key", value.Key);
        writer.WriteNumber("start_ms", value.StartMilliseconds);
        writer.WriteNumber("end_ms", value.EndMilliseconds);
        writer.WriteNumber("transfers", value.Transfers);
        writer.WriteNumber("unique_senders", value.UniqueSenders);
        writer.WriteNumber("unique_receivers", value.UniqueReceivers);
        writer.WriteString("amount", value.Amount);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerEconometricsIssuanceWindowJsonConverter :
    JsonConverter<ToriiExplorerEconometricsIssuanceWindow>
{
    public override bool HandleNull => true;

    public override ToriiExplorerEconometricsIssuanceWindow Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer issuance window");
        try
        {
            var response = new ToriiExplorerEconometricsIssuanceWindow
            {
                Key = ToriiExplorerJson.ReadRequiredString(payload, "key", "explorer issuance window.key"),
                StartMilliseconds = ToriiExplorerJson.ReadRequiredUInt64(
                    payload,
                    "start_ms",
                    "explorer issuance window.start_ms"),
                EndMilliseconds = ToriiExplorerJson.ReadRequiredUInt64(
                    payload,
                    "end_ms",
                    "explorer issuance window.end_ms"),
                MintCount = ToriiExplorerJson.ReadRequiredUInt64(
                    payload,
                    "mint_count",
                    "explorer issuance window.mint_count"),
                BurnCount = ToriiExplorerJson.ReadRequiredUInt64(
                    payload,
                    "burn_count",
                    "explorer issuance window.burn_count"),
                Minted = ToriiExplorerJson.ReadRequiredString(payload, "minted", "explorer issuance window.minted"),
                Burned = ToriiExplorerJson.ReadRequiredString(payload, "burned", "explorer issuance window.burned"),
                Net = ToriiExplorerJson.ReadRequiredString(payload, "net", "explorer issuance window.net"),
            };
            ToriiExplorerJson.ValidateExplorerIssuanceWindow(response, "explorer issuance window");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, "explorer issuance window");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerEconometricsIssuanceWindow value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerIssuanceWindow(value, "explorer issuance window");

        writer.WriteStartObject();
        writer.WriteString("key", value.Key);
        writer.WriteNumber("start_ms", value.StartMilliseconds);
        writer.WriteNumber("end_ms", value.EndMilliseconds);
        writer.WriteNumber("mint_count", value.MintCount);
        writer.WriteNumber("burn_count", value.BurnCount);
        writer.WriteString("minted", value.Minted);
        writer.WriteString("burned", value.Burned);
        writer.WriteString("net", value.Net);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerEconometricsIssuanceSeriesPointJsonConverter :
    JsonConverter<ToriiExplorerEconometricsIssuanceSeriesPoint>
{
    public override bool HandleNull => true;

    public override ToriiExplorerEconometricsIssuanceSeriesPoint Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer issuance series point");
        try
        {
            var response = new ToriiExplorerEconometricsIssuanceSeriesPoint
            {
                BucketStartMilliseconds = ToriiExplorerJson.ReadRequiredUInt64(
                    payload,
                    "bucket_start_ms",
                    "explorer issuance series point.bucket_start_ms"),
                Minted = ToriiExplorerJson.ReadRequiredString(payload, "minted", "explorer issuance series point.minted"),
                Burned = ToriiExplorerJson.ReadRequiredString(payload, "burned", "explorer issuance series point.burned"),
                Net = ToriiExplorerJson.ReadRequiredString(payload, "net", "explorer issuance series point.net"),
            };
            ToriiExplorerJson.ValidateExplorerIssuanceSeriesPoint(response, "explorer issuance series point");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, "explorer issuance series point");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerEconometricsIssuanceSeriesPoint value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerIssuanceSeriesPoint(value, "explorer issuance series point");

        writer.WriteStartObject();
        writer.WriteNumber("bucket_start_ms", value.BucketStartMilliseconds);
        writer.WriteString("minted", value.Minted);
        writer.WriteString("burned", value.Burned);
        writer.WriteString("net", value.Net);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerAssetDefinitionEconometricsJsonConverter :
    JsonConverter<ToriiExplorerAssetDefinitionEconometrics>
{
    public override bool HandleNull => true;

    public override ToriiExplorerAssetDefinitionEconometrics Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer asset definition econometrics");
        try
        {
            var response = new ToriiExplorerAssetDefinitionEconometrics
            {
                DefinitionId = ToriiExplorerJson.ReadRequiredString(
                    payload,
                    "definition_id",
                    "explorer asset definition econometrics.definition_id"),
                ComputedAtMilliseconds = ToriiExplorerJson.ReadRequiredUInt64(
                    payload,
                    "computed_at_ms",
                    "explorer asset definition econometrics.computed_at_ms"),
                VelocityWindows = ToriiExplorerJson.ReadRequiredItems<ToriiExplorerEconometricsVelocityWindow>(
                    payload,
                    "velocity_windows",
                    "explorer asset definition econometrics.velocity_windows",
                    "explorer velocity window"),
                IssuanceWindows = ToriiExplorerJson.ReadRequiredItems<ToriiExplorerEconometricsIssuanceWindow>(
                    payload,
                    "issuance_windows",
                    "explorer asset definition econometrics.issuance_windows",
                    "explorer issuance window"),
                IssuanceSeries = ToriiExplorerJson.ReadRequiredItems<ToriiExplorerEconometricsIssuanceSeriesPoint>(
                    payload,
                    "issuance_series",
                    "explorer asset definition econometrics.issuance_series",
                    "explorer issuance series point"),
            };
            ToriiExplorerJson.ValidateExplorerAssetDefinitionEconometrics(
                response,
                "explorer asset definition econometrics");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(
                error,
                "explorer asset definition econometrics");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerAssetDefinitionEconometrics value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerAssetDefinitionEconometrics(
            value,
            "explorer asset definition econometrics");

        writer.WriteStartObject();
        writer.WriteString("definition_id", value.DefinitionId);
        writer.WriteNumber("computed_at_ms", value.ComputedAtMilliseconds);
        ToriiExplorerJson.WriteItems(writer, "velocity_windows", value.VelocityWindows, options);
        ToriiExplorerJson.WriteItems(writer, "issuance_windows", value.IssuanceWindows, options);
        ToriiExplorerJson.WriteItems(writer, "issuance_series", value.IssuanceSeries, options);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerEconometricsLorenzPointJsonConverter :
    JsonConverter<ToriiExplorerEconometricsLorenzPoint>
{
    public override bool HandleNull => true;

    public override ToriiExplorerEconometricsLorenzPoint Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer Lorenz point");
        try
        {
            var response = new ToriiExplorerEconometricsLorenzPoint
            {
                Population = ToriiExplorerJson.ReadRequiredDouble(
                    payload,
                    "population",
                    "explorer Lorenz point.population"),
                Share = ToriiExplorerJson.ReadRequiredDouble(payload, "share", "explorer Lorenz point.share"),
            };
            ToriiExplorerJson.ValidateExplorerLorenzPoint(response, "explorer Lorenz point");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, "explorer Lorenz point");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerEconometricsLorenzPoint value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerLorenzPoint(value, "explorer Lorenz point");

        writer.WriteStartObject();
        writer.WriteNumber("population", value.Population);
        writer.WriteNumber("share", value.Share);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerEconometricsDistributionSnapshotJsonConverter :
    JsonConverter<ToriiExplorerEconometricsDistributionSnapshot>
{
    public override bool HandleNull => true;

    public override ToriiExplorerEconometricsDistributionSnapshot Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer distribution snapshot");
        try
        {
            var response = new ToriiExplorerEconometricsDistributionSnapshot
            {
                Gini = ToriiExplorerJson.ReadRequiredDouble(payload, "gini", "explorer distribution snapshot.gini"),
                Hhi = ToriiExplorerJson.ReadRequiredDouble(payload, "hhi", "explorer distribution snapshot.hhi"),
                Theil = ToriiExplorerJson.ReadRequiredDouble(payload, "theil", "explorer distribution snapshot.theil"),
                Entropy = ToriiExplorerJson.ReadRequiredDouble(
                    payload,
                    "entropy",
                    "explorer distribution snapshot.entropy"),
                EntropyNormalized = ToriiExplorerJson.ReadRequiredDouble(
                    payload,
                    "entropy_normalized",
                    "explorer distribution snapshot.entropy_normalized"),
                Nakamoto33 = ToriiExplorerJson.ReadRequiredUInt64(
                    payload,
                    "nakamoto_33",
                    "explorer distribution snapshot.nakamoto_33"),
                Nakamoto51 = ToriiExplorerJson.ReadRequiredUInt64(
                    payload,
                    "nakamoto_51",
                    "explorer distribution snapshot.nakamoto_51"),
                Nakamoto67 = ToriiExplorerJson.ReadRequiredUInt64(
                    payload,
                    "nakamoto_67",
                    "explorer distribution snapshot.nakamoto_67"),
                Top1 = ToriiExplorerJson.ReadRequiredDouble(payload, "top1", "explorer distribution snapshot.top1"),
                Top5 = ToriiExplorerJson.ReadRequiredDouble(payload, "top5", "explorer distribution snapshot.top5"),
                Top10 = ToriiExplorerJson.ReadRequiredDouble(payload, "top10", "explorer distribution snapshot.top10"),
                Median = ToriiExplorerJson.ReadOptionalString(payload, "median", "explorer distribution snapshot.median"),
                P90 = ToriiExplorerJson.ReadOptionalString(payload, "p90", "explorer distribution snapshot.p90"),
                P99 = ToriiExplorerJson.ReadOptionalString(payload, "p99", "explorer distribution snapshot.p99"),
                Lorenz = ToriiExplorerJson.ReadRequiredItems<ToriiExplorerEconometricsLorenzPoint>(
                    payload,
                    "lorenz",
                    "explorer distribution snapshot.lorenz",
                    "explorer Lorenz point"),
            };
            ToriiExplorerJson.ValidateExplorerDistributionSnapshot(response, "explorer distribution snapshot");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, "explorer distribution snapshot");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerEconometricsDistributionSnapshot value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerDistributionSnapshot(value, "explorer distribution snapshot");

        writer.WriteStartObject();
        writer.WriteNumber("gini", value.Gini);
        writer.WriteNumber("hhi", value.Hhi);
        writer.WriteNumber("theil", value.Theil);
        writer.WriteNumber("entropy", value.Entropy);
        writer.WriteNumber("entropy_normalized", value.EntropyNormalized);
        writer.WriteNumber("nakamoto_33", value.Nakamoto33);
        writer.WriteNumber("nakamoto_51", value.Nakamoto51);
        writer.WriteNumber("nakamoto_67", value.Nakamoto67);
        writer.WriteNumber("top1", value.Top1);
        writer.WriteNumber("top5", value.Top5);
        writer.WriteNumber("top10", value.Top10);
        ToriiExplorerJson.WriteNullableString(writer, "median", value.Median);
        ToriiExplorerJson.WriteNullableString(writer, "p90", value.P90);
        ToriiExplorerJson.WriteNullableString(writer, "p99", value.P99);
        ToriiExplorerJson.WriteItems(writer, "lorenz", value.Lorenz, options);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerEconometricsTopHolderJsonConverter :
    JsonConverter<ToriiExplorerEconometricsTopHolder>
{
    public override bool HandleNull => true;

    public override ToriiExplorerEconometricsTopHolder Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer top holder");
        try
        {
            var response = new ToriiExplorerEconometricsTopHolder
            {
                AccountId = ToriiExplorerJson.ReadRequiredString(payload, "account_id", "explorer top holder.account_id"),
                Balance = ToriiExplorerJson.ReadRequiredString(payload, "balance", "explorer top holder.balance"),
            };
            ToriiExplorerJson.ValidateExplorerTopHolder(response, "explorer top holder");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, "explorer top holder");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerEconometricsTopHolder value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerTopHolder(value, "explorer top holder");

        writer.WriteStartObject();
        writer.WriteString("account_id", value.AccountId);
        writer.WriteString("balance", value.Balance);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerAssetDefinitionSnapshotJsonConverter :
    JsonConverter<ToriiExplorerAssetDefinitionSnapshot>
{
    public override bool HandleNull => true;

    public override ToriiExplorerAssetDefinitionSnapshot Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer asset definition snapshot");
        var distribution = ToriiExplorerJson.ReadOptionalObject<ToriiExplorerEconometricsDistributionSnapshot>(
            payload,
            "distribution",
            "explorer asset definition snapshot.distribution",
            "explorer distribution snapshot");
        if (distribution is null)
        {
            throw new JsonException("explorer asset definition snapshot.distribution must not be null.");
        }

        try
        {
            var response = new ToriiExplorerAssetDefinitionSnapshot
            {
                DefinitionId = ToriiExplorerJson.ReadRequiredString(
                    payload,
                    "definition_id",
                    "explorer asset definition snapshot.definition_id"),
                ComputedAtMilliseconds = ToriiExplorerJson.ReadRequiredUInt64(
                    payload,
                    "computed_at_ms",
                    "explorer asset definition snapshot.computed_at_ms"),
                HoldersTotal = ToriiExplorerJson.ReadRequiredUInt64(
                    payload,
                    "holders_total",
                    "explorer asset definition snapshot.holders_total"),
                TotalSupply = ToriiExplorerJson.ReadRequiredString(
                    payload,
                    "total_supply",
                    "explorer asset definition snapshot.total_supply"),
                TopHolders = ToriiExplorerJson.ReadRequiredItems<ToriiExplorerEconometricsTopHolder>(
                    payload,
                    "top_holders",
                    "explorer asset definition snapshot.top_holders",
                    "explorer top holder"),
                Distribution = distribution,
            };
            ToriiExplorerJson.ValidateExplorerAssetDefinitionSnapshot(response, "explorer asset definition snapshot");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, "explorer asset definition snapshot");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerAssetDefinitionSnapshot value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerAssetDefinitionSnapshot(value, "explorer asset definition snapshot");

        writer.WriteStartObject();
        writer.WriteString("definition_id", value.DefinitionId);
        writer.WriteNumber("computed_at_ms", value.ComputedAtMilliseconds);
        writer.WriteNumber("holders_total", value.HoldersTotal);
        writer.WriteString("total_supply", value.TotalSupply);
        ToriiExplorerJson.WriteItems(writer, "top_holders", value.TopHolders, options);
        writer.WritePropertyName("distribution");
        JsonSerializer.Serialize(writer, value.Distribution, options);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerAssetJsonConverter : JsonConverter<ToriiExplorerAsset>
{
    public override bool HandleNull => true;

    public override ToriiExplorerAsset Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer asset");
        try
        {
            var response = new ToriiExplorerAsset
            {
                Id = ToriiExplorerJson.ReadRequiredString(payload, "id", "explorer asset.id"),
                DefinitionId = ToriiExplorerJson.ReadRequiredString(
                    payload,
                    "definition_id",
                    "explorer asset.definition_id"),
                AccountId = ToriiExplorerJson.ReadRequiredString(payload, "account_id", "explorer asset.account_id"),
                Value = ToriiExplorerJson.ReadRequiredString(payload, "value", "explorer asset.value"),
            };
            ToriiExplorerJson.ValidateExplorerAsset(response, "explorer asset");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, "explorer asset");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerAsset value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerAsset(value, "explorer asset");

        writer.WriteStartObject();
        writer.WriteString("id", value.Id);
        writer.WriteString("definition_id", value.DefinitionId);
        writer.WriteString("account_id", value.AccountId);
        writer.WriteString("value", value.Value);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerAssetsPageJsonConverter : JsonConverter<ToriiExplorerAssetsPage>
{
    public override bool HandleNull => true;

    public override ToriiExplorerAssetsPage Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer assets page");
        ToriiExplorerJson.RequireExactProperties(payload, "explorer assets page", "pagination", "items");
        var response = new ToriiExplorerAssetsPage
        {
            Pagination = ToriiExplorerJson.ReadRequiredCursorPagination(
                payload,
                "pagination",
                "explorer assets page.pagination"),
            Items = ToriiExplorerJson.ReadRequiredItems<ToriiExplorerAsset>(
                payload,
                "items",
                "explorer assets page.items",
                "explorer asset"),
        };
        ToriiExplorerJson.ValidateExplorerAssetsPage(response, "explorer assets page");
        return response;
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerAssetsPage value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerAssetsPage(value, "explorer assets page");

        writer.WriteStartObject();
        writer.WritePropertyName("pagination");
        JsonSerializer.Serialize(writer, value.Pagination, options);
        ToriiExplorerJson.WriteItems(writer, "items", value.Items, options);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerNftJsonConverter : JsonConverter<ToriiExplorerNft>
{
    public override bool HandleNull => true;

    public override ToriiExplorerNft Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer NFT");
        try
        {
            var response = new ToriiExplorerNft
            {
                Id = ToriiExplorerJson.ReadRequiredString(payload, "id", "explorer NFT.id"),
                OwnedBy = ToriiExplorerJson.ReadRequiredString(payload, "owned_by", "explorer NFT.owned_by"),
                Metadata = ToriiExplorerJson.ReadOptionalNode(payload, "metadata"),
            };
            ToriiExplorerJson.ValidateExplorerNft(response, "explorer NFT");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, "explorer NFT");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerNft value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerNft(value, "explorer NFT");

        writer.WriteStartObject();
        writer.WriteString("id", value.Id);
        writer.WriteString("owned_by", value.OwnedBy);
        writer.WritePropertyName("metadata");
        JsonSerializer.Serialize(writer, value.Metadata, options);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerNftsPageJsonConverter : JsonConverter<ToriiExplorerNftsPage>
{
    public override bool HandleNull => true;

    public override ToriiExplorerNftsPage Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer NFTs page");
        ToriiExplorerJson.RequireExactProperties(payload, "explorer NFTs page", "pagination", "items");
        var response = new ToriiExplorerNftsPage
        {
            Pagination = ToriiExplorerJson.ReadRequiredCursorPagination(
                payload,
                "pagination",
                "explorer NFTs page.pagination"),
            Items = ToriiExplorerJson.ReadRequiredItems<ToriiExplorerNft>(
                payload,
                "items",
                "explorer NFTs page.items",
                "explorer NFT"),
        };
        ToriiExplorerJson.ValidateExplorerNftsPage(response, "explorer NFTs page");
        return response;
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerNftsPage value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerNftsPage(value, "explorer NFTs page");

        writer.WriteStartObject();
        writer.WritePropertyName("pagination");
        JsonSerializer.Serialize(writer, value.Pagination, options);
        ToriiExplorerJson.WriteItems(writer, "items", value.Items, options);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerRwaParentJsonConverter : JsonConverter<ToriiExplorerRwaParent>
{
    public override bool HandleNull => true;

    public override ToriiExplorerRwaParent Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer RWA parent");
        try
        {
            var response = new ToriiExplorerRwaParent
            {
                Rwa = ToriiExplorerJson.ReadRequiredString(payload, "rwa", "explorer RWA parent.rwa"),
                Quantity = ToriiExplorerJson.ReadRequiredString(payload, "quantity", "explorer RWA parent.quantity"),
            };
            ToriiExplorerJson.ValidateExplorerRwaParent(response, "explorer RWA parent");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, "explorer RWA parent");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerRwaParent value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerRwaParent(value, "explorer RWA parent");

        writer.WriteStartObject();
        writer.WriteString("rwa", value.Rwa);
        writer.WriteString("quantity", value.Quantity);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerRwaJsonConverter : JsonConverter<ToriiExplorerRwa>
{
    public override bool HandleNull => true;

    public override ToriiExplorerRwa Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer RWA");
        try
        {
            var response = new ToriiExplorerRwa
            {
                Id = ToriiExplorerJson.ReadRequiredString(payload, "id", "explorer RWA.id"),
                OwnedBy = ToriiExplorerJson.ReadRequiredString(payload, "owned_by", "explorer RWA.owned_by"),
                Quantity = ToriiExplorerJson.ReadRequiredString(payload, "quantity", "explorer RWA.quantity"),
                HeldQuantity = ToriiExplorerJson.ReadRequiredString(payload, "held_quantity", "explorer RWA.held_quantity"),
                PrimaryReference = ToriiExplorerJson.ReadRequiredString(
                    payload,
                    "primary_reference",
                    "explorer RWA.primary_reference"),
                Status = ToriiExplorerJson.ReadOptionalString(payload, "status", "explorer RWA.status"),
                IsFrozen = ToriiExplorerJson.ReadRequiredBool(payload, "is_frozen", "explorer RWA.is_frozen"),
                Metadata = ToriiExplorerJson.ReadOptionalNode(payload, "metadata"),
                Parents = ToriiExplorerJson.ReadRequiredItems<ToriiExplorerRwaParent>(
                    payload,
                    "parents",
                    "explorer RWA.parents",
                    "explorer RWA parent"),
            };
            ToriiExplorerJson.ValidateExplorerRwa(response, "explorer RWA");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, "explorer RWA");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerRwa value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerRwa(value, "explorer RWA");

        writer.WriteStartObject();
        writer.WriteString("id", value.Id);
        writer.WriteString("owned_by", value.OwnedBy);
        writer.WriteString("quantity", value.Quantity);
        writer.WriteString("held_quantity", value.HeldQuantity);
        writer.WriteString("primary_reference", value.PrimaryReference);
        ToriiExplorerJson.WriteNullableString(writer, "status", value.Status);
        writer.WriteBoolean("is_frozen", value.IsFrozen);
        writer.WritePropertyName("metadata");
        JsonSerializer.Serialize(writer, value.Metadata, options);
        ToriiExplorerJson.WriteItems(writer, "parents", value.Parents, options);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerRwasPageJsonConverter : JsonConverter<ToriiExplorerRwasPage>
{
    public override bool HandleNull => true;

    public override ToriiExplorerRwasPage Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer RWAs page");
        ToriiExplorerJson.RequireExactProperties(payload, "explorer RWAs page", "pagination", "items");
        var response = new ToriiExplorerRwasPage
        {
            Pagination = ToriiExplorerJson.ReadRequiredCursorPagination(
                payload,
                "pagination",
                "explorer RWAs page.pagination"),
            Items = ToriiExplorerJson.ReadRequiredItems<ToriiExplorerRwa>(
                payload,
                "items",
                "explorer RWAs page.items",
                "explorer RWA"),
        };
        ToriiExplorerJson.ValidateExplorerRwasPage(response, "explorer RWAs page");
        return response;
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerRwasPage value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerRwasPage(value, "explorer RWAs page");

        writer.WriteStartObject();
        writer.WritePropertyName("pagination");
        JsonSerializer.Serialize(writer, value.Pagination, options);
        ToriiExplorerJson.WriteItems(writer, "items", value.Items, options);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerBlockJsonConverter : JsonConverter<ToriiExplorerBlock>
{
    public override bool HandleNull => true;

    public override ToriiExplorerBlock Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer block");
        try
        {
            var response = new ToriiExplorerBlock
            {
                Hash = ToriiExplorerJson.ReadRequiredString(payload, "hash", "explorer block.hash"),
                Height = ToriiExplorerJson.ReadRequiredUInt64(payload, "height", "explorer block.height"),
                CreatedAt = ToriiExplorerJson.ReadRequiredString(payload, "created_at", "explorer block.created_at"),
                PreviousBlockHash = ToriiExplorerJson.ReadOptionalString(payload, "prev_block_hash", "explorer block.prev_block_hash"),
                TransactionsHash = ToriiExplorerJson.ReadOptionalString(payload, "transactions_hash", "explorer block.transactions_hash"),
                TransactionsRejected = ToriiExplorerJson.ReadRequiredUInt32(payload, "transactions_rejected", "explorer block.transactions_rejected"),
                TransactionsTotal = ToriiExplorerJson.ReadRequiredUInt32(payload, "transactions_total", "explorer block.transactions_total"),
            };
            ToriiExplorerJson.ValidateExplorerBlock(response, "explorer block");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, "explorer block");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerBlock value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerBlock(value, "explorer block");

        writer.WriteStartObject();
        writer.WriteString("hash", value.Hash);
        writer.WriteNumber("height", value.Height);
        writer.WriteString("created_at", value.CreatedAt);
        ToriiExplorerJson.WriteNullableString(writer, "prev_block_hash", value.PreviousBlockHash);
        ToriiExplorerJson.WriteNullableString(writer, "transactions_hash", value.TransactionsHash);
        writer.WriteNumber("transactions_rejected", value.TransactionsRejected);
        writer.WriteNumber("transactions_total", value.TransactionsTotal);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerBlocksPageJsonConverter : JsonConverter<ToriiExplorerBlocksPage>
{
    public override bool HandleNull => true;

    public override ToriiExplorerBlocksPage Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer blocks page");
        var response = new ToriiExplorerBlocksPage
        {
            Pagination = ToriiExplorerJson.ReadRequiredPagination(
                payload,
                "pagination",
                "explorer blocks page.pagination"),
            Items = ToriiExplorerJson.ReadRequiredItems<ToriiExplorerBlock>(
                payload,
                "items",
                "explorer blocks page.items",
                "explorer block"),
        };
        ToriiExplorerJson.ValidateExplorerBlocksPage(response, "explorer blocks page");
        return response;
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerBlocksPage value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerBlocksPage(value, "explorer blocks page");

        writer.WriteStartObject();
        writer.WritePropertyName("pagination");
        JsonSerializer.Serialize(writer, value.Pagination, options);
        ToriiExplorerJson.WriteItems(writer, "items", value.Items, options);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerTransactionJsonConverter : JsonConverter<ToriiExplorerTransaction>
{
    public override bool HandleNull => true;

    public override ToriiExplorerTransaction Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer transaction");
        try
        {
            var response = new ToriiExplorerTransaction
            {
                Authority = ToriiExplorerJson.ReadRequiredString(payload, "authority", "explorer transaction.authority"),
                Hash = ToriiExplorerJson.ReadRequiredString(payload, "hash", "explorer transaction.hash"),
                Block = ToriiExplorerJson.ReadRequiredUInt64(payload, "block", "explorer transaction.block"),
                CreatedAt = ToriiExplorerJson.ReadRequiredString(payload, "created_at", "explorer transaction.created_at"),
                Executable = ToriiExplorerJson.ReadRequiredString(payload, "executable", "explorer transaction.executable"),
                Status = ToriiExplorerJson.ReadRequiredString(payload, "status", "explorer transaction.status"),
            };
            ToriiExplorerJson.ValidateExplorerTransaction(response, "explorer transaction");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, "explorer transaction");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerTransaction value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerTransaction(value, "explorer transaction");

        writer.WriteStartObject();
        writer.WriteString("authority", value.Authority);
        writer.WriteString("hash", value.Hash);
        writer.WriteNumber("block", value.Block);
        writer.WriteString("created_at", value.CreatedAt);
        writer.WriteString("executable", value.Executable);
        writer.WriteString("status", value.Status);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerTransactionRejectionJsonConverter :
    JsonConverter<ToriiExplorerTransactionRejection>
{
    public override bool HandleNull => true;

    public override ToriiExplorerTransactionRejection Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer transaction rejection");
        try
        {
            var response = new ToriiExplorerTransactionRejection
            {
                Encoded = ToriiExplorerJson.ReadRequiredString(
                    payload,
                    "encoded",
                    "explorer transaction rejection.encoded"),
                Json = ToriiExplorerJson.ReadOptionalNode(payload, "json"),
                Message = ToriiExplorerJson.ReadRequiredString(
                    payload,
                    "message",
                    "explorer transaction rejection.message"),
            };
            ToriiExplorerJson.ValidateExplorerTransactionRejection(response, "explorer transaction rejection");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, "explorer transaction rejection");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerTransactionRejection value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerTransactionRejection(value, "explorer transaction rejection");

        writer.WriteStartObject();
        writer.WriteString("encoded", value.Encoded);
        writer.WritePropertyName("json");
        JsonSerializer.Serialize(writer, value.Json, options);
        writer.WriteString("message", value.Message);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerTransactionDetailJsonConverter :
    JsonConverter<ToriiExplorerTransactionDetail>
{
    public override bool HandleNull => true;

    public override ToriiExplorerTransactionDetail Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer transaction detail");
        try
        {
            var response = new ToriiExplorerTransactionDetail
            {
                Authority = ToriiExplorerJson.ReadRequiredString(
                    payload,
                    "authority",
                    "explorer transaction detail.authority"),
                Hash = ToriiExplorerJson.ReadRequiredString(payload, "hash", "explorer transaction detail.hash"),
                Block = ToriiExplorerJson.ReadRequiredUInt64(payload, "block", "explorer transaction detail.block"),
                CreatedAt = ToriiExplorerJson.ReadRequiredString(
                    payload,
                    "created_at",
                    "explorer transaction detail.created_at"),
                Executable = ToriiExplorerJson.ReadRequiredString(
                    payload,
                    "executable",
                    "explorer transaction detail.executable"),
                Status = ToriiExplorerJson.ReadRequiredString(payload, "status", "explorer transaction detail.status"),
                RejectionReason = ToriiExplorerJson.ReadOptionalObject<ToriiExplorerTransactionRejection>(
                    payload,
                    "rejection_reason",
                    "explorer transaction detail.rejection_reason",
                    "explorer transaction rejection"),
                Metadata = ToriiExplorerJson.ReadOptionalNode(payload, "metadata"),
                Nonce = ToriiExplorerJson.ReadOptionalUInt64(payload, "nonce", "explorer transaction detail.nonce"),
                Signature = ToriiExplorerJson.ReadRequiredString(
                    payload,
                    "signature",
                    "explorer transaction detail.signature"),
                TimeToLive = ToriiExplorerJson.ReadOptionalObject<ToriiExplorerDuration>(
                    payload,
                    "time_to_live",
                    "explorer transaction detail.time_to_live",
                    "explorer duration"),
            };
            ToriiExplorerJson.ValidateExplorerTransactionDetail(response, "explorer transaction detail");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, "explorer transaction detail");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerTransactionDetail value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerTransactionDetail(value, "explorer transaction detail");

        writer.WriteStartObject();
        writer.WriteString("authority", value.Authority);
        writer.WriteString("hash", value.Hash);
        writer.WriteNumber("block", value.Block);
        writer.WriteString("created_at", value.CreatedAt);
        writer.WriteString("executable", value.Executable);
        writer.WriteString("status", value.Status);
        writer.WritePropertyName("rejection_reason");
        JsonSerializer.Serialize(writer, value.RejectionReason, options);
        writer.WritePropertyName("metadata");
        JsonSerializer.Serialize(writer, value.Metadata, options);
        if (value.Nonce is ulong nonce)
        {
            writer.WriteNumber("nonce", nonce);
        }
        else
        {
            writer.WriteNull("nonce");
        }

        writer.WriteString("signature", value.Signature);
        writer.WritePropertyName("time_to_live");
        JsonSerializer.Serialize(writer, value.TimeToLive, options);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerTransactionsPageJsonConverter : JsonConverter<ToriiExplorerTransactionsPage>
{
    public override bool HandleNull => true;

    public override ToriiExplorerTransactionsPage Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer transactions page");
        var response = new ToriiExplorerTransactionsPage
        {
            Pagination = ToriiExplorerJson.ReadRequiredPagination(
                payload,
                "pagination",
                "explorer transactions page.pagination"),
            Items = ToriiExplorerJson.ReadRequiredItems<ToriiExplorerTransaction>(
                payload,
                "items",
                "explorer transactions page.items",
                "explorer transaction"),
        };
        ToriiExplorerJson.ValidateExplorerTransactionsPage(response, "explorer transactions page");
        return response;
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerTransactionsPage value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerTransactionsPage(value, "explorer transactions page");

        writer.WriteStartObject();
        writer.WritePropertyName("pagination");
        JsonSerializer.Serialize(writer, value.Pagination, options);
        ToriiExplorerJson.WriteItems(writer, "items", value.Items, options);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerLatestTransactionsResponseJsonConverter
    : JsonConverter<ToriiExplorerLatestTransactionsResponse>
{
    public override bool HandleNull => true;

    public override ToriiExplorerLatestTransactionsResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer latest transactions response");
        try
        {
            var response = new ToriiExplorerLatestTransactionsResponse
            {
                SampledAt = ToriiExplorerJson.ReadRequiredString(
                    payload,
                    "sampled_at",
                    "explorer latest transactions response.sampled_at"),
                Items = ToriiExplorerJson.ReadRequiredItems<ToriiExplorerTransaction>(
                    payload,
                    "items",
                    "explorer latest transactions response.items",
                    "explorer transaction"),
            };
            ToriiExplorerJson.ValidateExplorerLatestTransactionsResponse(
                response,
                "explorer latest transactions response");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(
                error,
                "explorer latest transactions response");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerLatestTransactionsResponse value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerLatestTransactionsResponse(
            value,
            "explorer latest transactions response");

        writer.WriteStartObject();
        writer.WriteString("sampled_at", value.SampledAt);
        ToriiExplorerJson.WriteItems(writer, "items", value.Items, options);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerInstructionJsonJsonConverter : JsonConverter<ToriiExplorerInstructionJson>
{
    public override bool HandleNull => true;

    public override ToriiExplorerInstructionJson Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer instruction json");
        try
        {
            var response = new ToriiExplorerInstructionJson
            {
                Kind = ToriiExplorerJson.ReadRequiredString(payload, "kind", "explorer instruction json.kind"),
                Payload = ToriiExplorerJson.ReadOptionalNode(payload, "payload"),
                WireId = ToriiExplorerJson.ReadRequiredString(payload, "wire_id", "explorer instruction json.wire_id"),
                Encoded = ToriiExplorerJson.ReadRequiredString(payload, "encoded", "explorer instruction json.encoded"),
            };
            ToriiExplorerJson.ValidateExplorerInstructionJson(response, "explorer instruction json");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, "explorer instruction json");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerInstructionJson value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerInstructionJson(value, "explorer instruction json");

        writer.WriteStartObject();
        writer.WriteString("kind", value.Kind);
        writer.WritePropertyName("payload");
        if (value.Payload is null)
        {
            writer.WriteNullValue();
        }
        else
        {
            value.Payload.WriteTo(writer, options);
        }

        writer.WriteString("wire_id", value.WireId);
        writer.WriteString("encoded", value.Encoded);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerInstructionBoxJsonConverter : JsonConverter<ToriiExplorerInstructionBox>
{
    public override bool HandleNull => true;

    public override ToriiExplorerInstructionBox Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer instruction box");
        try
        {
            var response = new ToriiExplorerInstructionBox
            {
                Encoded = ToriiExplorerJson.ReadRequiredString(payload, "encoded", "explorer instruction box.encoded"),
                Json = ToriiExplorerJson.ReadOptionalObject<ToriiExplorerInstructionJson>(
                    payload,
                    "json",
                    "explorer instruction box.json",
                    "explorer instruction json"),
            };
            ToriiExplorerJson.ValidateExplorerInstructionBox(response, "explorer instruction box");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, "explorer instruction box");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerInstructionBox value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerInstructionBox(value, "explorer instruction box");

        writer.WriteStartObject();
        writer.WriteString("encoded", value.Encoded);
        writer.WritePropertyName("json");
        JsonSerializer.Serialize(writer, value.Json, options);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerInstructionJsonConverter : JsonConverter<ToriiExplorerInstruction>
{
    public override bool HandleNull => true;

    public override ToriiExplorerInstruction Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer instruction");
        try
        {
            var response = new ToriiExplorerInstruction
            {
                Authority = ToriiExplorerJson.ReadRequiredString(payload, "authority", "explorer instruction.authority"),
                CreatedAt = ToriiExplorerJson.ReadRequiredString(payload, "created_at", "explorer instruction.created_at"),
                Kind = ToriiExplorerJson.ReadRequiredString(payload, "kind", "explorer instruction.kind"),
                InstructionBox = ToriiExplorerJson.ReadRequiredObject<ToriiExplorerInstructionBox>(
                    payload,
                    "box",
                    "explorer instruction.box",
                    "explorer instruction box"),
                TransactionHash = ToriiExplorerJson.ReadRequiredString(payload, "transaction_hash", "explorer instruction.transaction_hash"),
                TransactionStatus = ToriiExplorerJson.ReadRequiredString(payload, "transaction_status", "explorer instruction.transaction_status"),
                Block = ToriiExplorerJson.ReadRequiredUInt64(payload, "block", "explorer instruction.block"),
                Index = ToriiExplorerJson.ReadRequiredUInt32(payload, "index", "explorer instruction.index"),
            };
            ToriiExplorerJson.ValidateExplorerInstruction(response, "explorer instruction");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, "explorer instruction");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerInstruction value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerInstruction(value, "explorer instruction");

        writer.WriteStartObject();
        writer.WriteString("authority", value.Authority);
        writer.WriteString("created_at", value.CreatedAt);
        writer.WriteString("kind", value.Kind);
        writer.WritePropertyName("box");
        JsonSerializer.Serialize(writer, value.InstructionBox, options);
        writer.WriteString("transaction_hash", value.TransactionHash);
        writer.WriteString("transaction_status", value.TransactionStatus);
        writer.WriteNumber("block", value.Block);
        writer.WriteNumber("index", value.Index);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerInstructionsPageJsonConverter : JsonConverter<ToriiExplorerInstructionsPage>
{
    public override bool HandleNull => true;

    public override ToriiExplorerInstructionsPage Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer instructions page");
        var response = new ToriiExplorerInstructionsPage
        {
            Pagination = ToriiExplorerJson.ReadRequiredPagination(
                payload,
                "pagination",
                "explorer instructions page.pagination"),
            Items = ToriiExplorerJson.ReadRequiredItems<ToriiExplorerInstruction>(
                payload,
                "items",
                "explorer instructions page.items",
                "explorer instruction"),
        };
        ToriiExplorerJson.ValidateExplorerInstructionsPage(response, "explorer instructions page");
        return response;
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerInstructionsPage value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerInstructionsPage(value, "explorer instructions page");

        writer.WriteStartObject();
        writer.WritePropertyName("pagination");
        JsonSerializer.Serialize(writer, value.Pagination, options);
        ToriiExplorerJson.WriteItems(writer, "items", value.Items, options);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiExplorerLatestInstructionsResponseJsonConverter
    : JsonConverter<ToriiExplorerLatestInstructionsResponse>
{
    public override bool HandleNull => true;

    public override ToriiExplorerLatestInstructionsResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, "explorer latest instructions response");
        try
        {
            var response = new ToriiExplorerLatestInstructionsResponse
            {
                SampledAt = ToriiExplorerJson.ReadRequiredString(
                    payload,
                    "sampled_at",
                    "explorer latest instructions response.sampled_at"),
                Items = ToriiExplorerJson.ReadRequiredItems<ToriiExplorerInstruction>(
                    payload,
                    "items",
                    "explorer latest instructions response.items",
                    "explorer instruction"),
            };
            ToriiExplorerJson.ValidateExplorerLatestInstructionsResponse(
                response,
                "explorer latest instructions response");
            return response;
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiExplorerJson.DirectMetadataErrorToJsonException(
                error,
                "explorer latest instructions response");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerLatestInstructionsResponse value,
        JsonSerializerOptions options)
    {
        ToriiExplorerJson.ValidateExplorerLatestInstructionsResponse(
            value,
            "explorer latest instructions response");

        writer.WriteStartObject();
        writer.WriteString("sampled_at", value.SampledAt);
        ToriiExplorerJson.WriteItems(writer, "items", value.Items, options);
        writer.WriteEndObject();
    }
}
