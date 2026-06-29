using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;
using Hyperledger.Iroha.Address;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiUaidJson
{
    internal static void ValidateUaidPortfolioTotals(ToriiUaidPortfolioTotals? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        ValidateNonNegativeInt64(response.Accounts, $"{context}.accounts");
        ValidateNonNegativeInt64(response.Positions, $"{context}.positions");
    }

    internal static void ValidateUaidPortfolioAsset(ToriiUaidPortfolioAsset? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireExactNonEmptyText(response.AssetId, $"{context}.asset_id");
        RequireExactNonEmptyText(response.AssetDefinitionId, $"{context}.asset_definition_id");
        ValidateCanonicalNonNegativeNumericText(response.Quantity, $"{context}.quantity");
    }

    internal static void ValidateUaidPortfolioAccount(ToriiUaidPortfolioAccount? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireCanonicalAccountId(response.AccountId, $"{context}.account_id");
        RequireOptionalExactNonEmptyText(response.Label, $"{context}.label");
        ValidateItems(response.Assets, $"{context}.assets", ValidateUaidPortfolioAsset);
    }

    internal static void ValidateUaidPortfolioDataspace(ToriiUaidPortfolioDataspace? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        ValidateNonNegativeInt64(response.DataspaceId, $"{context}.dataspace_id");
        RequireOptionalExactNonEmptyText(response.DataspaceAlias, $"{context}.dataspace_alias");
        ValidateItems(response.Accounts, $"{context}.accounts", ValidateUaidPortfolioAccount);
    }

    internal static void ValidateUaidPortfolioResponse(ToriiUaidPortfolioResponse response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ValidateCanonicalUaidLiteral(response.Uaid, $"{context}.uaid");
        ValidateUaidPortfolioTotals(response.Totals, $"{context}.totals");
        ValidateItems(response.Dataspaces, $"{context}.dataspaces", ValidateUaidPortfolioDataspace);
    }

    internal static void ValidateUaidBindingsDataspace(ToriiUaidBindingsDataspace? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        ValidateNonNegativeInt64(response.DataspaceId, $"{context}.dataspace_id");
        RequireOptionalExactNonEmptyText(response.DataspaceAlias, $"{context}.dataspace_alias");
        ValidateCanonicalAccountIdList(response.Accounts, $"{context}.accounts");
    }

    internal static void ValidateUaidBindingsResponse(ToriiUaidBindingsResponse response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ValidateCanonicalUaidLiteral(response.Uaid, $"{context}.uaid");
        ValidateItems(response.Dataspaces, $"{context}.dataspaces", ValidateUaidBindingsDataspace);
    }

    internal static void ValidateUaidManifestRevocation(ToriiUaidManifestRevocation? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        ValidateNonNegativeInt64(response.Epoch, $"{context}.epoch");
        RequireOptionalExactNonEmptyText(response.Reason, $"{context}.reason");
    }

    internal static void ValidateUaidManifestLifecycle(ToriiUaidManifestLifecycle? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        ValidateOptionalNonNegativeInt64(response.ActivatedEpoch, $"{context}.activated_epoch");
        ValidateOptionalNonNegativeInt64(response.ExpiredEpoch, $"{context}.expired_epoch");
        if (response.Revocation is not null)
        {
            ValidateUaidManifestRevocation(response.Revocation, $"{context}.revocation");
        }
    }

    internal static void ValidateUaidManifestRecord(ToriiUaidManifestRecord? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        ValidateNonNegativeInt64(response.DataspaceId, $"{context}.dataspace_id");
        RequireOptionalExactNonEmptyText(response.DataspaceAlias, $"{context}.dataspace_alias");
        ToriiSseEventJson.RequireExactSizedHex(response.ManifestHash, $"{context}.manifest_hash", 32);
        RequireExactNonEmptyText(response.Status, $"{context}.status");
        ValidateUaidManifestLifecycle(response.Lifecycle, $"{context}.lifecycle");
        ValidateCanonicalAccountIdList(response.Accounts, $"{context}.accounts");
    }

    internal static void ValidateUaidManifestsResponse(ToriiUaidManifestsResponse response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ValidateCanonicalUaidLiteral(response.Uaid, $"{context}.uaid");
        ValidateNonNegativeInt64(response.Total, $"{context}.total");
        ValidateItems(response.Manifests, $"{context}.manifests", ValidateUaidManifestRecord);
    }

    internal static ToriiUaidPortfolioTotals ReadUaidPortfolioTotals(ref Utf8JsonReader reader, string context)
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
        long? accounts = null;
        long? positions = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = CreateWithDirectMetadataContext(
                    () => new ToriiUaidPortfolioTotals
                    {
                        Accounts = RequireCounter(accounts, context, "accounts"),
                        Positions = RequireCounter(positions, context, "positions"),
                    },
                    context);
                ValidateUaidPortfolioTotals(response, context);
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
                case "accounts":
                    accounts = ReadInt64(ref reader, $"{context}.accounts");
                    break;
                case "positions":
                    positions = ReadInt64(ref reader, $"{context}.positions");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiUaidPortfolioAsset ReadUaidPortfolioAsset(ref Utf8JsonReader reader, string context)
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
        string? assetId = null;
        string? assetDefinitionId = null;
        string? quantity = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = CreateWithDirectMetadataContext(
                    () => new ToriiUaidPortfolioAsset
                    {
                        AssetId = RequireString(assetId, $"{context}.asset_id"),
                        AssetDefinitionId = RequireString(assetDefinitionId, $"{context}.asset_definition_id"),
                        Quantity = RequireString(quantity, $"{context}.quantity"),
                    },
                    context);
                ValidateUaidPortfolioAsset(response, context);
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
                case "asset_id":
                    assetId = ReadOptionalString(ref reader, $"{context}.asset_id");
                    break;
                case "asset_definition_id":
                    assetDefinitionId = ReadOptionalString(ref reader, $"{context}.asset_definition_id");
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

    internal static ToriiUaidPortfolioAccount ReadUaidPortfolioAccount(ref Utf8JsonReader reader, string context)
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
        string? label = null;
        List<ToriiUaidPortfolioAsset>? assets = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = CreateWithDirectMetadataContext(
                    () => new ToriiUaidPortfolioAccount
                    {
                        AccountId = RequireString(accountId, $"{context}.account_id"),
                        Label = label,
                        Assets = RequireList(assets, context, "assets"),
                    },
                    context);
                ValidateUaidPortfolioAccount(response, context);
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
                case "account_id":
                    accountId = ReadOptionalString(ref reader, $"{context}.account_id");
                    break;
                case "label":
                    label = ReadOptionalString(ref reader, $"{context}.label");
                    break;
                case "assets":
                    assets = ReadItems(ref reader, $"{context}.assets", ReadUaidPortfolioAsset);
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiUaidPortfolioDataspace ReadUaidPortfolioDataspace(ref Utf8JsonReader reader, string context)
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
        long? dataspaceId = null;
        string? dataspaceAlias = null;
        List<ToriiUaidPortfolioAccount>? accounts = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = CreateWithDirectMetadataContext(
                    () => new ToriiUaidPortfolioDataspace
                    {
                        DataspaceId = RequireCounter(dataspaceId, context, "dataspace_id"),
                        DataspaceAlias = dataspaceAlias,
                        Accounts = RequireList(accounts, context, "accounts"),
                    },
                    context);
                ValidateUaidPortfolioDataspace(response, context);
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
                case "dataspace_id":
                    dataspaceId = ReadInt64(ref reader, $"{context}.dataspace_id");
                    break;
                case "dataspace_alias":
                    dataspaceAlias = ReadOptionalString(ref reader, $"{context}.dataspace_alias");
                    break;
                case "accounts":
                    accounts = ReadItems(ref reader, $"{context}.accounts", ReadUaidPortfolioAccount);
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiUaidPortfolioResponse ReadUaidPortfolioResponse(ref Utf8JsonReader reader, string context)
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
        string? uaid = null;
        ToriiUaidPortfolioTotals? totals = null;
        List<ToriiUaidPortfolioDataspace>? dataspaces = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = CreateWithDirectMetadataContext(
                    () => new ToriiUaidPortfolioResponse
                    {
                        Uaid = RequireString(uaid, $"{context}.uaid"),
                        Totals = totals!,
                        Dataspaces = RequireList(dataspaces, context, "dataspaces"),
                    },
                    context);
                ValidateUaidPortfolioResponse(response, context);
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
                case "uaid":
                    uaid = ReadOptionalString(ref reader, $"{context}.uaid");
                    break;
                case "totals":
                    totals = ReadNullableItem(ref reader, $"{context}.totals", ReadUaidPortfolioTotals);
                    break;
                case "dataspaces":
                    dataspaces = ReadItems(ref reader, $"{context}.dataspaces", ReadUaidPortfolioDataspace);
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiUaidBindingsDataspace ReadUaidBindingsDataspace(ref Utf8JsonReader reader, string context)
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
        long? dataspaceId = null;
        string? dataspaceAlias = null;
        List<string>? accounts = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = CreateWithDirectMetadataContext(
                    () => new ToriiUaidBindingsDataspace
                    {
                        DataspaceId = RequireCounter(dataspaceId, context, "dataspace_id"),
                        DataspaceAlias = dataspaceAlias,
                        Accounts = RequireList(accounts, context, "accounts"),
                    },
                    context);
                ValidateUaidBindingsDataspace(response, context);
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
                case "dataspace_id":
                    dataspaceId = ReadInt64(ref reader, $"{context}.dataspace_id");
                    break;
                case "dataspace_alias":
                    dataspaceAlias = ReadOptionalString(ref reader, $"{context}.dataspace_alias");
                    break;
                case "accounts":
                    accounts = ReadStringList(ref reader, $"{context}.accounts");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiUaidBindingsResponse ReadUaidBindingsResponse(ref Utf8JsonReader reader, string context)
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
        string? uaid = null;
        List<ToriiUaidBindingsDataspace>? dataspaces = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = CreateWithDirectMetadataContext(
                    () => new ToriiUaidBindingsResponse
                    {
                        Uaid = RequireString(uaid, $"{context}.uaid"),
                        Dataspaces = RequireList(dataspaces, context, "dataspaces"),
                    },
                    context);
                ValidateUaidBindingsResponse(response, context);
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
                case "uaid":
                    uaid = ReadOptionalString(ref reader, $"{context}.uaid");
                    break;
                case "dataspaces":
                    dataspaces = ReadItems(ref reader, $"{context}.dataspaces", ReadUaidBindingsDataspace);
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiUaidManifestRevocation ReadUaidManifestRevocation(ref Utf8JsonReader reader, string context)
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
        long? epoch = null;
        string? reason = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = CreateWithDirectMetadataContext(
                    () => new ToriiUaidManifestRevocation
                    {
                        Epoch = RequireCounter(epoch, context, "epoch"),
                        Reason = reason,
                    },
                    context);
                ValidateUaidManifestRevocation(response, context);
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
                case "epoch":
                    epoch = ReadInt64(ref reader, $"{context}.epoch");
                    break;
                case "reason":
                    reason = ReadOptionalString(ref reader, $"{context}.reason");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiUaidManifestLifecycle ReadUaidManifestLifecycle(ref Utf8JsonReader reader, string context)
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
        long? activatedEpoch = null;
        long? expiredEpoch = null;
        ToriiUaidManifestRevocation? revocation = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = CreateWithDirectMetadataContext(
                    () => new ToriiUaidManifestLifecycle
                    {
                        ActivatedEpoch = activatedEpoch,
                        ExpiredEpoch = expiredEpoch,
                        Revocation = revocation,
                    },
                    context);
                ValidateUaidManifestLifecycle(response, context);
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
                case "activated_epoch":
                    activatedEpoch = ReadNullableInt64(ref reader, $"{context}.activated_epoch");
                    break;
                case "expired_epoch":
                    expiredEpoch = ReadNullableInt64(ref reader, $"{context}.expired_epoch");
                    break;
                case "revocation":
                    revocation = ReadNullableItem(ref reader, $"{context}.revocation", ReadUaidManifestRevocation);
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiUaidManifestRecord ReadUaidManifestRecord(ref Utf8JsonReader reader, string context)
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
        long? dataspaceId = null;
        string? dataspaceAlias = null;
        string? manifestHash = null;
        string? status = null;
        ToriiUaidManifestLifecycle? lifecycle = null;
        List<string>? accounts = null;
        JsonNode? manifest = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = CreateWithDirectMetadataContext(
                    () => new ToriiUaidManifestRecord
                    {
                        DataspaceId = RequireCounter(dataspaceId, context, "dataspace_id"),
                        DataspaceAlias = dataspaceAlias,
                        ManifestHash = RequireString(manifestHash, $"{context}.manifest_hash"),
                        Status = RequireString(status, $"{context}.status"),
                        Lifecycle = lifecycle!,
                        Accounts = RequireList(accounts, context, "accounts"),
                        Manifest = manifest,
                    },
                    context);
                ValidateUaidManifestRecord(response, context);
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
                case "dataspace_id":
                    dataspaceId = ReadInt64(ref reader, $"{context}.dataspace_id");
                    break;
                case "dataspace_alias":
                    dataspaceAlias = ReadOptionalString(ref reader, $"{context}.dataspace_alias");
                    break;
                case "manifest_hash":
                    manifestHash = ReadOptionalString(ref reader, $"{context}.manifest_hash");
                    break;
                case "status":
                    status = ReadOptionalString(ref reader, $"{context}.status");
                    break;
                case "lifecycle":
                    lifecycle = ReadNullableItem(ref reader, $"{context}.lifecycle", ReadUaidManifestLifecycle);
                    break;
                case "accounts":
                    accounts = ReadStringList(ref reader, $"{context}.accounts");
                    break;
                case "manifest":
                    manifest = ToriiIdentifierJson.ReadOptionalNode(ref reader, $"{context}.manifest");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiUaidManifestsResponse ReadUaidManifestsResponse(ref Utf8JsonReader reader, string context)
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
        string? uaid = null;
        long? total = null;
        List<ToriiUaidManifestRecord>? manifests = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = CreateWithDirectMetadataContext(
                    () => new ToriiUaidManifestsResponse
                    {
                        Uaid = RequireString(uaid, $"{context}.uaid"),
                        Total = RequireTotal(total, context),
                        Manifests = RequireList(manifests, context, "manifests"),
                    },
                    context);
                ValidateUaidManifestsResponse(response, context);
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
                case "uaid":
                    uaid = ReadOptionalString(ref reader, $"{context}.uaid");
                    break;
                case "total":
                    total = ReadInt64(ref reader, $"{context}.total");
                    break;
                case "manifests":
                    manifests = ReadItems(ref reader, $"{context}.manifests", ReadUaidManifestRecord);
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static void WriteUaidPortfolioTotals(
        Utf8JsonWriter writer,
        ToriiUaidPortfolioTotals response,
        string context)
    {
        ValidateUaidPortfolioTotals(response, context);

        writer.WriteStartObject();
        writer.WriteNumber("accounts", response.Accounts);
        writer.WriteNumber("positions", response.Positions);
        writer.WriteEndObject();
    }

    internal static void WriteUaidPortfolioAsset(
        Utf8JsonWriter writer,
        ToriiUaidPortfolioAsset response,
        string context)
    {
        ValidateUaidPortfolioAsset(response, context);

        writer.WriteStartObject();
        writer.WriteString("asset_id", response.AssetId);
        writer.WriteString("asset_definition_id", response.AssetDefinitionId);
        writer.WriteString("quantity", response.Quantity);
        writer.WriteEndObject();
    }

    internal static void WriteUaidPortfolioAccount(
        Utf8JsonWriter writer,
        ToriiUaidPortfolioAccount response,
        string context)
    {
        ValidateUaidPortfolioAccount(response, context);

        writer.WriteStartObject();
        writer.WriteString("account_id", response.AccountId);
        ToriiVpnJson.WriteNullableString(writer, "label", response.Label);
        writer.WritePropertyName("assets");
        writer.WriteStartArray();
        for (var index = 0; index < response.Assets.Count; index++)
        {
            WriteUaidPortfolioAsset(writer, response.Assets[index], $"{context}.assets[{index}]");
        }
        writer.WriteEndArray();
        writer.WriteEndObject();
    }

    internal static void WriteUaidPortfolioDataspace(
        Utf8JsonWriter writer,
        ToriiUaidPortfolioDataspace response,
        string context)
    {
        ValidateUaidPortfolioDataspace(response, context);

        writer.WriteStartObject();
        writer.WriteNumber("dataspace_id", response.DataspaceId);
        ToriiVpnJson.WriteNullableString(writer, "dataspace_alias", response.DataspaceAlias);
        writer.WritePropertyName("accounts");
        writer.WriteStartArray();
        for (var index = 0; index < response.Accounts.Count; index++)
        {
            WriteUaidPortfolioAccount(writer, response.Accounts[index], $"{context}.accounts[{index}]");
        }
        writer.WriteEndArray();
        writer.WriteEndObject();
    }

    internal static void WriteUaidPortfolioResponse(
        Utf8JsonWriter writer,
        ToriiUaidPortfolioResponse response,
        string context)
    {
        ValidateUaidPortfolioResponse(response, context);

        writer.WriteStartObject();
        writer.WriteString("uaid", response.Uaid);
        writer.WritePropertyName("totals");
        WriteUaidPortfolioTotals(writer, response.Totals, $"{context}.totals");
        writer.WritePropertyName("dataspaces");
        writer.WriteStartArray();
        for (var index = 0; index < response.Dataspaces.Count; index++)
        {
            WriteUaidPortfolioDataspace(writer, response.Dataspaces[index], $"{context}.dataspaces[{index}]");
        }
        writer.WriteEndArray();
        writer.WriteEndObject();
    }

    internal static void WriteUaidBindingsDataspace(
        Utf8JsonWriter writer,
        ToriiUaidBindingsDataspace response,
        string context)
    {
        ValidateUaidBindingsDataspace(response, context);

        writer.WriteStartObject();
        writer.WriteNumber("dataspace_id", response.DataspaceId);
        ToriiVpnJson.WriteNullableString(writer, "dataspace_alias", response.DataspaceAlias);
        WriteStringList(writer, "accounts", response.Accounts);
        writer.WriteEndObject();
    }

    internal static void WriteUaidBindingsResponse(
        Utf8JsonWriter writer,
        ToriiUaidBindingsResponse response,
        string context)
    {
        ValidateUaidBindingsResponse(response, context);

        writer.WriteStartObject();
        writer.WriteString("uaid", response.Uaid);
        writer.WritePropertyName("dataspaces");
        writer.WriteStartArray();
        for (var index = 0; index < response.Dataspaces.Count; index++)
        {
            WriteUaidBindingsDataspace(writer, response.Dataspaces[index], $"{context}.dataspaces[{index}]");
        }
        writer.WriteEndArray();
        writer.WriteEndObject();
    }

    internal static void WriteUaidManifestRevocation(
        Utf8JsonWriter writer,
        ToriiUaidManifestRevocation response,
        string context)
    {
        ValidateUaidManifestRevocation(response, context);

        writer.WriteStartObject();
        writer.WriteNumber("epoch", response.Epoch);
        ToriiVpnJson.WriteNullableString(writer, "reason", response.Reason);
        writer.WriteEndObject();
    }

    internal static void WriteUaidManifestLifecycle(
        Utf8JsonWriter writer,
        ToriiUaidManifestLifecycle response,
        string context)
    {
        ValidateUaidManifestLifecycle(response, context);

        writer.WriteStartObject();
        WriteNullableNumber(writer, "activated_epoch", response.ActivatedEpoch);
        WriteNullableNumber(writer, "expired_epoch", response.ExpiredEpoch);
        writer.WritePropertyName("revocation");
        if (response.Revocation is null)
        {
            writer.WriteNullValue();
        }
        else
        {
            WriteUaidManifestRevocation(writer, response.Revocation, $"{context}.revocation");
        }
        writer.WriteEndObject();
    }

    internal static void WriteUaidManifestRecord(
        Utf8JsonWriter writer,
        ToriiUaidManifestRecord response,
        string context)
    {
        ValidateUaidManifestRecord(response, context);

        writer.WriteStartObject();
        writer.WriteNumber("dataspace_id", response.DataspaceId);
        ToriiVpnJson.WriteNullableString(writer, "dataspace_alias", response.DataspaceAlias);
        writer.WriteString("manifest_hash", response.ManifestHash);
        writer.WriteString("status", response.Status);
        writer.WritePropertyName("lifecycle");
        WriteUaidManifestLifecycle(writer, response.Lifecycle, $"{context}.lifecycle");
        WriteStringList(writer, "accounts", response.Accounts);
        writer.WritePropertyName("manifest");
        if (response.Manifest is null)
        {
            writer.WriteNullValue();
        }
        else
        {
            response.Manifest.WriteTo(writer);
        }
        writer.WriteEndObject();
    }

    internal static void WriteUaidManifestsResponse(
        Utf8JsonWriter writer,
        ToriiUaidManifestsResponse response,
        string context)
    {
        ValidateUaidManifestsResponse(response, context);

        writer.WriteStartObject();
        writer.WriteString("uaid", response.Uaid);
        writer.WriteNumber("total", response.Total);
        writer.WritePropertyName("manifests");
        writer.WriteStartArray();
        for (var index = 0; index < response.Manifests.Count; index++)
        {
            WriteUaidManifestRecord(writer, response.Manifests[index], $"{context}.manifests[{index}]");
        }
        writer.WriteEndArray();
        writer.WriteEndObject();
    }

    internal static JsonException DirectMetadataErrorToJsonException(ArgumentException error, string context)
    {
        return new JsonException($"{context}.{MapDirectMetadataField(error.ParamName ?? "metadata")}: {error.Message}", error);
    }

    internal static T CreateWithDirectMetadataContext<T>(Func<T> factory, string context)
    {
        try
        {
            return factory();
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw DirectMetadataErrorToJsonException(error, context);
        }
    }

    private static string MapDirectMetadataField(string paramName)
    {
        return paramName switch
        {
            _ when TryMapCollectionField(paramName, nameof(ToriiUaidPortfolioAccount.Assets), "assets", out var mapped) => mapped,
            _ when TryMapCollectionField(paramName, nameof(ToriiUaidPortfolioDataspace.Accounts), "accounts", out var mapped) => mapped,
            _ when TryMapCollectionField(paramName, nameof(ToriiUaidPortfolioResponse.Dataspaces), "dataspaces", out var mapped) => mapped,
            _ when TryMapCollectionField(paramName, nameof(ToriiUaidBindingsResponse.Dataspaces), "dataspaces", out var mapped) => mapped,
            _ when TryMapCollectionField(paramName, nameof(ToriiUaidManifestRecord.Accounts), "accounts", out var mapped) => mapped,
            _ when TryMapCollectionField(paramName, nameof(ToriiUaidManifestsResponse.Manifests), "manifests", out var mapped) => mapped,
            _ when TryMapNestedField(paramName, nameof(ToriiUaidPortfolioResponse.Totals), "totals", out var mapped) => mapped,
            _ when TryMapNestedField(paramName, nameof(ToriiUaidManifestLifecycle.Revocation), "revocation", out var mapped) => mapped,
            _ when TryMapNestedField(paramName, nameof(ToriiUaidManifestRecord.Lifecycle), "lifecycle", out var mapped) => mapped,
            nameof(ToriiUaidPortfolioTotals.Accounts) => "accounts",
            nameof(ToriiUaidPortfolioTotals.Positions) => "positions",
            nameof(ToriiUaidPortfolioAsset.AssetId) => "asset_id",
            nameof(ToriiUaidPortfolioAsset.AssetDefinitionId) => "asset_definition_id",
            nameof(ToriiUaidPortfolioAsset.Quantity) => "quantity",
            nameof(ToriiUaidPortfolioAccount.AccountId) => "account_id",
            nameof(ToriiUaidPortfolioAccount.Label) => "label",
            nameof(ToriiUaidPortfolioAccount.Assets) => "assets",
            nameof(ToriiUaidPortfolioDataspace.DataspaceId) => "dataspace_id",
            nameof(ToriiUaidPortfolioDataspace.DataspaceAlias) => "dataspace_alias",
            nameof(ToriiUaidPortfolioResponse.Uaid) => "uaid",
            nameof(ToriiUaidPortfolioResponse.Totals) => "totals",
            nameof(ToriiUaidPortfolioResponse.Dataspaces) => "dataspaces",
            nameof(ToriiUaidManifestRevocation.Epoch) => "epoch",
            nameof(ToriiUaidManifestRevocation.Reason) => "reason",
            nameof(ToriiUaidManifestLifecycle.ActivatedEpoch) => "activated_epoch",
            nameof(ToriiUaidManifestLifecycle.ExpiredEpoch) => "expired_epoch",
            nameof(ToriiUaidManifestLifecycle.Revocation) => "revocation",
            nameof(ToriiUaidManifestRecord.ManifestHash) => "manifest_hash",
            nameof(ToriiUaidManifestRecord.Status) => "status",
            nameof(ToriiUaidManifestRecord.Lifecycle) => "lifecycle",
            nameof(ToriiUaidManifestRecord.Manifest) => "manifest",
            nameof(ToriiUaidManifestsResponse.Total) => "total",
            nameof(ToriiUaidManifestsResponse.Manifests) => "manifests",
            _ => paramName,
        };
    }

    private static bool TryMapCollectionField(
        string paramName,
        string propertyName,
        string jsonName,
        out string mapped)
    {
        var prefix = propertyName + "[";
        if (paramName.StartsWith(prefix, StringComparison.Ordinal))
        {
            mapped = jsonName + MapIndexedSuffix(paramName[propertyName.Length..]);
            return true;
        }

        mapped = string.Empty;
        return false;
    }

    private static bool TryMapNestedField(
        string paramName,
        string propertyName,
        string jsonName,
        out string mapped)
    {
        var prefix = propertyName + ".";
        if (paramName.StartsWith(prefix, StringComparison.Ordinal))
        {
            mapped = jsonName + "." + MapDirectMetadataField(paramName[prefix.Length..]);
            return true;
        }

        mapped = string.Empty;
        return false;
    }

    private static string MapIndexedSuffix(string suffix)
    {
        var dot = suffix.IndexOf('.', StringComparison.Ordinal);
        if (dot < 0)
        {
            return suffix;
        }

        return suffix[..(dot + 1)] + MapDirectMetadataField(suffix[(dot + 1)..]);
    }

    private delegate T ReadItem<T>(ref Utf8JsonReader reader, string context);

    private static T? ReadNullableItem<T>(
        ref Utf8JsonReader reader,
        string context,
        ReadItem<T> readItem)
        where T : class
    {
        return reader.TokenType == JsonTokenType.Null ? null : readItem(ref reader, context);
    }

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

            var value = ReadOptionalString(ref reader, $"{context}[{index}]");
            values.Add(RequireString(value, $"{context}[{index}]"));
            index++;
        }

        throw new JsonException($"{context} array is incomplete.");
    }

    private static void ValidateTextList(IReadOnlyList<string>? values, string context)
    {
        if (values is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        for (var index = 0; index < values.Count; index++)
        {
            RequireExactNonEmptyText(values[index], $"{context}[{index}]");
        }
    }

    private static void ValidateCanonicalAccountIdList(IReadOnlyList<string>? values, string context)
    {
        if (values is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        for (var index = 0; index < values.Count; index++)
        {
            RequireCanonicalAccountId(values[index], $"{context}[{index}]");
        }
    }

    private static string? ReadOptionalString(ref Utf8JsonReader reader, string field)
    {
        return ToriiAccountFaucetJson.ReadOptionalString(ref reader, field);
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

    private static long RequireTotal(long? value, string context)
    {
        if (!value.HasValue)
        {
            throw new JsonException($"{context}.total must not be null.");
        }

        return value.Value;
    }

    private static long RequireCounter(long? value, string context, string propertyName)
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

    private static IReadOnlyList<T> RequireList<T>(IReadOnlyList<T>? values, string context, string propertyName)
    {
        if (values is null)
        {
            throw new JsonException($"{context}.{propertyName} must not be null.");
        }

        return values;
    }

    private static void ValidateCanonicalUaidLiteral(string? value, string field)
    {
        var text = RequireExactNonEmptyText(value, field);
        if (!text.StartsWith("uaid:", StringComparison.Ordinal)
            || text.Length != 69
            || !IsLowercaseHex(text.AsSpan(5))
            || (HexNibble(text[^1]) & 1) == 0)
        {
            throw new JsonException($"{field} must be a canonical `uaid:<64 lowercase hex chars>` literal.");
        }
    }

    private static string RequireExactNonEmptyText(string? value, string field)
    {
        return ToriiSseEventJson.RequireOptionalExactNonEmptyText(value, field)
            ?? throw new JsonException($"{field} must be a non-empty string.");
    }

    private static void RequireOptionalExactNonEmptyText(string? value, string field)
    {
        ToriiSseEventJson.RequireOptionalExactNonEmptyText(value, field);
    }

    private static string RequireCanonicalAccountId(string? value, string field)
    {
        var text = RequireExactNonEmptyText(value, field);
        try
        {
            return AccountAddress.Parse(text, AccountAddress.DefaultChainDiscriminant)
                .ToI105(AccountAddress.DefaultChainDiscriminant);
        }
        catch (AccountAddressException exception)
        {
            throw new JsonException($"{field} must be a canonical I105 account id.", exception);
        }
    }

    private static void ValidateCanonicalNonNegativeNumericText(string? value, string field)
    {
        var text = RequireExactNonEmptyText(value, field);
        if (text[0] == '+' || text[0] == '-')
        {
            throw new JsonException($"{field} must be a canonical non-negative numeric string.");
        }

        var separator = text.IndexOf('.', StringComparison.Ordinal);
        var integerPart = separator < 0 ? text : text[..separator];
        var fractionalPart = separator < 0 ? string.Empty : text[(separator + 1)..];
        if (integerPart.Length == 0 || integerPart.Any(static character => character is < '0' or > '9'))
        {
            throw new JsonException($"{field} must be a canonical non-negative numeric string.");
        }

        if (integerPart.Length > 1 && integerPart[0] == '0')
        {
            throw new JsonException($"{field} must be a canonical non-negative numeric string.");
        }

        if (separator >= 0
            && (fractionalPart.Length == 0
                || fractionalPart.Length > 28
                || fractionalPart.Any(static character => character is < '0' or > '9')
                || fractionalPart[^1] == '0'))
        {
            throw new JsonException($"{field} must be a canonical non-negative numeric string.");
        }
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

    private static bool IsLowercaseHex(ReadOnlySpan<char> value)
    {
        foreach (var character in value)
        {
            if (character is not (>= '0' and <= '9') and not (>= 'a' and <= 'f'))
            {
                return false;
            }
        }

        return true;
    }

    private static int HexNibble(char character)
    {
        if (character >= '0' && character <= '9')
        {
            return character - '0';
        }

        if (character >= 'a' && character <= 'f')
        {
            return character - 'a' + 10;
        }

        return 0;
    }

    private static void WriteStringList(Utf8JsonWriter writer, string propertyName, IReadOnlyList<string> values)
    {
        writer.WritePropertyName(propertyName);
        writer.WriteStartArray();
        foreach (var value in values)
        {
            writer.WriteStringValue(value);
        }
        writer.WriteEndArray();
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

internal sealed class ToriiUaidPortfolioTotalsJsonConverter : JsonConverter<ToriiUaidPortfolioTotals>
{
    public override bool HandleNull => true;

    public override ToriiUaidPortfolioTotals Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiUaidJson.ReadUaidPortfolioTotals(ref reader, "UAID portfolio totals");
    }

    public override void Write(Utf8JsonWriter writer, ToriiUaidPortfolioTotals value, JsonSerializerOptions options)
    {
        ToriiUaidJson.WriteUaidPortfolioTotals(writer, value, "UAID portfolio totals");
    }
}

internal sealed class ToriiUaidPortfolioAssetJsonConverter : JsonConverter<ToriiUaidPortfolioAsset>
{
    public override bool HandleNull => true;

    public override ToriiUaidPortfolioAsset Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiUaidJson.ReadUaidPortfolioAsset(ref reader, "UAID portfolio asset");
    }

    public override void Write(Utf8JsonWriter writer, ToriiUaidPortfolioAsset value, JsonSerializerOptions options)
    {
        ToriiUaidJson.WriteUaidPortfolioAsset(writer, value, "UAID portfolio asset");
    }
}

internal sealed class ToriiUaidPortfolioAccountJsonConverter : JsonConverter<ToriiUaidPortfolioAccount>
{
    public override bool HandleNull => true;

    public override ToriiUaidPortfolioAccount Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiUaidJson.ReadUaidPortfolioAccount(ref reader, "UAID portfolio account");
    }

    public override void Write(Utf8JsonWriter writer, ToriiUaidPortfolioAccount value, JsonSerializerOptions options)
    {
        ToriiUaidJson.WriteUaidPortfolioAccount(writer, value, "UAID portfolio account");
    }
}

internal sealed class ToriiUaidPortfolioDataspaceJsonConverter : JsonConverter<ToriiUaidPortfolioDataspace>
{
    public override bool HandleNull => true;

    public override ToriiUaidPortfolioDataspace Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiUaidJson.ReadUaidPortfolioDataspace(ref reader, "UAID portfolio dataspace");
    }

    public override void Write(Utf8JsonWriter writer, ToriiUaidPortfolioDataspace value, JsonSerializerOptions options)
    {
        ToriiUaidJson.WriteUaidPortfolioDataspace(writer, value, "UAID portfolio dataspace");
    }
}

internal sealed class ToriiUaidPortfolioResponseJsonConverter : JsonConverter<ToriiUaidPortfolioResponse>
{
    public override bool HandleNull => true;

    public override ToriiUaidPortfolioResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiUaidJson.ReadUaidPortfolioResponse(ref reader, "UAID portfolio response");
    }

    public override void Write(Utf8JsonWriter writer, ToriiUaidPortfolioResponse value, JsonSerializerOptions options)
    {
        ToriiUaidJson.WriteUaidPortfolioResponse(writer, value, "UAID portfolio response");
    }
}

internal sealed class ToriiUaidBindingsDataspaceJsonConverter : JsonConverter<ToriiUaidBindingsDataspace>
{
    public override bool HandleNull => true;

    public override ToriiUaidBindingsDataspace Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiUaidJson.ReadUaidBindingsDataspace(ref reader, "UAID bindings dataspace");
    }

    public override void Write(Utf8JsonWriter writer, ToriiUaidBindingsDataspace value, JsonSerializerOptions options)
    {
        ToriiUaidJson.WriteUaidBindingsDataspace(writer, value, "UAID bindings dataspace");
    }
}

internal sealed class ToriiUaidBindingsResponseJsonConverter : JsonConverter<ToriiUaidBindingsResponse>
{
    public override bool HandleNull => true;

    public override ToriiUaidBindingsResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiUaidJson.ReadUaidBindingsResponse(ref reader, "UAID bindings response");
    }

    public override void Write(Utf8JsonWriter writer, ToriiUaidBindingsResponse value, JsonSerializerOptions options)
    {
        ToriiUaidJson.WriteUaidBindingsResponse(writer, value, "UAID bindings response");
    }
}

internal sealed class ToriiUaidManifestRevocationJsonConverter : JsonConverter<ToriiUaidManifestRevocation>
{
    public override bool HandleNull => true;

    public override ToriiUaidManifestRevocation Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiUaidJson.ReadUaidManifestRevocation(ref reader, "UAID manifest revocation");
    }

    public override void Write(Utf8JsonWriter writer, ToriiUaidManifestRevocation value, JsonSerializerOptions options)
    {
        ToriiUaidJson.WriteUaidManifestRevocation(writer, value, "UAID manifest revocation");
    }
}

internal sealed class ToriiUaidManifestLifecycleJsonConverter : JsonConverter<ToriiUaidManifestLifecycle>
{
    public override bool HandleNull => true;

    public override ToriiUaidManifestLifecycle Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiUaidJson.ReadUaidManifestLifecycle(ref reader, "UAID manifest lifecycle");
    }

    public override void Write(Utf8JsonWriter writer, ToriiUaidManifestLifecycle value, JsonSerializerOptions options)
    {
        ToriiUaidJson.WriteUaidManifestLifecycle(writer, value, "UAID manifest lifecycle");
    }
}

internal sealed class ToriiUaidManifestRecordJsonConverter : JsonConverter<ToriiUaidManifestRecord>
{
    public override bool HandleNull => true;

    public override ToriiUaidManifestRecord Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiUaidJson.ReadUaidManifestRecord(ref reader, "UAID manifest record");
    }

    public override void Write(Utf8JsonWriter writer, ToriiUaidManifestRecord value, JsonSerializerOptions options)
    {
        ToriiUaidJson.WriteUaidManifestRecord(writer, value, "UAID manifest record");
    }
}

internal sealed class ToriiUaidManifestsResponseJsonConverter : JsonConverter<ToriiUaidManifestsResponse>
{
    public override bool HandleNull => true;

    public override ToriiUaidManifestsResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiUaidJson.ReadUaidManifestsResponse(ref reader, "UAID manifests response");
    }

    public override void Write(Utf8JsonWriter writer, ToriiUaidManifestsResponse value, JsonSerializerOptions options)
    {
        ToriiUaidJson.WriteUaidManifestsResponse(writer, value, "UAID manifests response");
    }
}
