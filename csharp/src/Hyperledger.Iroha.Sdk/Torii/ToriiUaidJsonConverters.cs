using System.Text;
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

    }

    internal static void ValidateUaidPortfolioAsset(ToriiUaidPortfolioAsset? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        ToriiUaidDirectMetadata.CanonicalAssetIdParts asset;
        string definition;
        try
        {
            asset = ToriiUaidDirectMetadata.RequireCanonicalAssetId(response.AssetId, $"{context}.asset_id");
            definition = ToriiUaidDirectMetadata.RequireCanonicalAssetDefinitionId(
                response.AssetDefinitionId,
                $"{context}.asset_definition_id");
        }
        catch (ArgumentException error)
        {
            throw new JsonException(error.Message, error);
        }
        if (!string.Equals(asset.AssetDefinitionId, definition, StringComparison.Ordinal))
        {
            throw new JsonException($"{context}.asset_id must match {context}.asset_definition_id.");
        }
        ValidateCanonicalQuantityText(response.Quantity, $"{context}.quantity");
    }

    internal static void ValidateUaidPortfolioAccount(ToriiUaidPortfolioAccount? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireCanonicalAccountId(response.AccountId, $"{context}.account_id");
        ValidateItems(response.Assets, $"{context}.assets", ValidateUaidPortfolioAsset);
    }

    internal static void ValidateUaidPortfolioDataspace(ToriiUaidPortfolioDataspace? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        ValidateItems(response.Accounts, $"{context}.accounts", ValidateUaidPortfolioAccount);
        for (var accountIndex = 0; accountIndex < response.Accounts.Count; accountIndex++)
        {
            var account = response.Accounts[accountIndex];
            for (var assetIndex = 0; assetIndex < account.Assets.Count; assetIndex++)
            {
                var asset = account.Assets[assetIndex];
                ToriiUaidDirectMetadata.CanonicalAssetIdParts parts;
                try
                {
                    parts = ToriiUaidDirectMetadata.RequireCanonicalAssetId(
                        asset.AssetId,
                        $"{context}.accounts[{accountIndex}].assets[{assetIndex}].asset_id");
                }
                catch (ArgumentException error)
                {
                    throw new JsonException(error.Message, error);
                }
                if (!string.Equals(parts.AccountId, account.AccountId, StringComparison.Ordinal)
                    || (parts.DataspaceId.HasValue && parts.DataspaceId.Value != response.DataspaceId))
                {
                    throw new JsonException(
                        $"{context}.accounts[{accountIndex}].assets[{assetIndex}].asset_id must match its account and dataspace.");
                }
            }
        }
    }

    internal static void ValidateUaidPortfolioResponse(ToriiUaidPortfolioResponse response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ValidateCanonicalUaidLiteral(response.Uaid, $"{context}.uaid");
        ValidateUaidPortfolioTotals(response.Totals, $"{context}.totals");
        ValidateItems(response.Dataspaces, $"{context}.dataspaces", ValidateUaidPortfolioDataspace);
        var accounts = new HashSet<string>(StringComparer.Ordinal);
        ulong positions = 0;
        foreach (var dataspace in response.Dataspaces)
        {
            foreach (var account in dataspace.Accounts)
            {
                accounts.Add(account.AccountId);
                try
                {
                    positions = checked(positions + (ulong)account.Assets.Count);
                }
                catch (OverflowException exception)
                {
                    throw new JsonException($"{context} position count overflows UInt64.", exception);
                }
            }
        }
        if (accounts.Count > 1)
        {
            throw new JsonException($"{context} must contain at most one universal account.");
        }
        if (response.Totals.Accounts != (ulong)accounts.Count || response.Totals.Positions != positions)
        {
            throw new JsonException($"{context}.totals must match the portfolio tree.");
        }
    }

    internal static void ValidateUaidBindingsDataspace(ToriiUaidBindingsDataspace? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        ValidateCanonicalAccountIdList(response.Accounts, $"{context}.accounts");
        if (response.Accounts.Count > 1)
        {
            throw new JsonException($"{context}.accounts must contain at most one universal account.");
        }
    }

    internal static void ValidateUaidBindingsResponse(ToriiUaidBindingsResponse response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ValidateCanonicalUaidLiteral(response.Uaid, $"{context}.uaid");
        ValidateItems(response.Dataspaces, $"{context}.dataspaces", ValidateUaidBindingsDataspace);
        var accounts = response.Dataspaces
            .SelectMany(static dataspace => dataspace.Accounts)
            .ToHashSet(StringComparer.Ordinal);
        if (accounts.Count > 1)
        {
            throw new JsonException($"{context} must contain at most one universal account.");
        }
    }

    internal static void ValidateUaidManifestRevocation(ToriiUaidManifestRevocation? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

    }

    internal static void ValidateUaidManifestLifecycle(ToriiUaidManifestLifecycle? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

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

        ToriiSseEventJson.RequireExactSizedHex(response.ManifestHash, $"{context}.manifest_hash", 32);
        ValidateManifestStatus(response.Status, $"{context}.status");
        ValidateUaidManifestLifecycle(response.Lifecycle, $"{context}.lifecycle");
        var derivedStatus = response.Lifecycle.Revocation is not null
            ? ToriiUaidManifestStatus.Revoked
            : response.Lifecycle.ExpiredEpoch.HasValue
                ? ToriiUaidManifestStatus.Expired
                : response.Lifecycle.ActivatedEpoch.HasValue
                    ? ToriiUaidManifestStatus.Active
                    : ToriiUaidManifestStatus.Pending;
        if (response.Status != derivedStatus)
        {
            throw new JsonException($"{context}.status must match {context}.lifecycle.");
        }
        ValidateCanonicalAccountIdList(response.Accounts, $"{context}.accounts");
        if (response.Accounts.Count > 1)
        {
            throw new JsonException($"{context}.accounts must contain at most one universal account.");
        }
        ValidateAssetPermissionManifest(response.Manifest, $"{context}.manifest");
        var manifest = response.Manifest.AsObject();
        if (RequireJsonUInt64(manifest, "dataspace", $"{context}.manifest") != response.DataspaceId)
        {
            throw new JsonException($"{context}.manifest.dataspace must match {context}.dataspace_id.");
        }
    }

    internal static void ValidateUaidManifestsResponse(ToriiUaidManifestsResponse response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ValidateCanonicalUaidLiteral(response.Uaid, $"{context}.uaid");
        ValidateManifestCountMode(response.CountMode, $"{context}.count_mode");
        ValidateItems(response.Manifests, $"{context}.manifests", ValidateUaidManifestRecord);
        if (response.Total < (ulong)response.Manifests.Count)
        {
            throw new JsonException($"{context}.total cannot be smaller than the page.");
        }
        for (var index = 0; index < response.Manifests.Count; index++)
        {
            var manifestUaid = response.Manifests[index].Manifest["uaid"]!.GetValue<string>();
            if (!string.Equals(manifestUaid, response.Uaid, StringComparison.Ordinal))
            {
                throw new JsonException($"{context}.manifests[{index}].manifest.uaid must match {context}.uaid.");
            }
        }
    }

    internal static void ValidateAssetPermissionManifest(JsonNode? value, string context)
    {
        var manifest = RequireJsonObject(value, context);
        RequireJsonFields(
            manifest,
            context,
            ["version", "uaid", "dataspace", "issued_ms", "activation_epoch", "entries"],
            ["expiry_epoch"]);

        if (RequireJsonUInt64(manifest, "version", context) != 1)
        {
            throw new JsonException($"{context}.version must be numeric V1 value 1.");
        }
        ValidateCanonicalUaidLiteral(RequireJsonString(manifest, "uaid", context), $"{context}.uaid");
        _ = RequireJsonUInt64(manifest, "dataspace", context);
        _ = RequireJsonUInt64(manifest, "issued_ms", context);
        _ = RequireJsonUInt64(manifest, "activation_epoch", context);
        if (manifest.TryGetPropertyValue("expiry_epoch", out var expiry))
        {
            if (expiry is null)
            {
                throw new JsonException($"{context}.expiry_epoch must be omitted instead of null.");
            }
            _ = RequireJsonUInt64(expiry, $"{context}.expiry_epoch");
        }

        var entries = RequireJsonArray(manifest["entries"], $"{context}.entries");
        for (var index = 0; index < entries.Count; index++)
        {
            ValidateManifestEntry(entries[index], $"{context}.entries[{index}]");
        }
    }

    private static void ValidateManifestEntry(JsonNode? value, string context)
    {
        var entry = RequireJsonObject(value, context);
        RequireJsonFields(entry, context, ["scope", "effect"], ["notes"]);
        ValidateManifestScope(entry["scope"], $"{context}.scope");
        ValidateManifestEffect(entry["effect"], $"{context}.effect");
        ValidateOmittedOptionalString(entry, "notes", context, validateCanonicalName: false);
    }

    private static void ValidateManifestScope(JsonNode? value, string context)
    {
        var scope = RequireJsonObject(value, context);
        RequireJsonFields(scope, context, [], ["dataspace", "program", "method", "asset", "role"]);
        if (scope.TryGetPropertyValue("dataspace", out var dataspace))
        {
            RejectNullOptional(dataspace, $"{context}.dataspace");
            _ = RequireJsonUInt64(dataspace!, $"{context}.dataspace");
        }
        ValidateOmittedOptionalString(scope, "program", context, validateCanonicalName: true);
        ValidateOmittedOptionalString(scope, "method", context, validateCanonicalName: true);
        if (scope.TryGetPropertyValue("asset", out var asset))
        {
            RejectNullOptional(asset, $"{context}.asset");
            var literal = RequireJsonString(asset!, $"{context}.asset");
            try
            {
                _ = ToriiUaidDirectMetadata.RequireCanonicalAssetDefinitionId(
                    literal,
                    $"{context}.asset");
            }
            catch (ArgumentException error)
            {
                throw new JsonException(error.Message, error);
            }
        }
        if (scope.TryGetPropertyValue("role", out var role))
        {
            RejectNullOptional(role, $"{context}.role");
            var text = RequireJsonString(role!, $"{context}.role");
            if (text is not ("Initiator" or "Participant"))
            {
                throw new JsonException($"{context}.role must be Initiator or Participant.");
            }
        }
    }

    private static void ValidateManifestEffect(JsonNode? value, string context)
    {
        var effect = RequireJsonObject(value, context);
        if (effect.Count != 1)
        {
            throw new JsonException($"{context} must contain exactly one of Allow or Deny.");
        }
        var decision = effect.First();
        switch (decision.Key)
        {
            case "Allow":
            {
                var allowance = RequireJsonObject(decision.Value, $"{context}.Allow");
                RequireJsonFields(allowance, $"{context}.Allow", ["window"], ["max_amount"]);
                var window = RequireJsonString(allowance, "window", $"{context}.Allow");
                if (window is not ("PerSlot" or "PerMinute" or "PerDay"))
                {
                    throw new JsonException($"{context}.Allow.window contains an unsupported allowance window.");
                }
                if (allowance.TryGetPropertyValue("max_amount", out var maxAmount))
                {
                    RejectNullOptional(maxAmount, $"{context}.Allow.max_amount");
                    ValidateCanonicalQuantityText(
                        RequireJsonString(maxAmount!, $"{context}.Allow.max_amount"),
                        $"{context}.Allow.max_amount");
                }
                break;
            }
            case "Deny":
            {
                var deny = RequireJsonObject(decision.Value, $"{context}.Deny");
                RequireJsonFields(deny, $"{context}.Deny", [], ["reason"]);
                ValidateOmittedOptionalString(deny, "reason", $"{context}.Deny", validateCanonicalName: false);
                break;
            }
            default:
                throw new JsonException($"{context} contains unknown decision `{decision.Key}`.");
        }
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
        ulong? accounts = null;
        ulong? positions = null;

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
                RequireExactFields(seen, context, "accounts", "positions");
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
                    accounts = ReadUInt64(ref reader, $"{context}.accounts");
                    break;
                case "positions":
                    positions = ReadUInt64(ref reader, $"{context}.positions");
                    break;
                default:
                    throw new JsonException($"{context} contains unknown field `{propertyName}`.");
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
                RequireExactFields(seen, context, "asset_id", "asset_definition_id", "quantity");
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
                    throw new JsonException($"{context} contains unknown field `{propertyName}`.");
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
                RequireExactFields(seen, context, "account_id", "label", "assets");
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
                    throw new JsonException($"{context} contains unknown field `{propertyName}`.");
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
        ulong? dataspaceId = null;
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
                RequireExactFields(seen, context, "dataspace_id", "dataspace_alias", "accounts");
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
                    dataspaceId = ReadUInt64(ref reader, $"{context}.dataspace_id");
                    break;
                case "dataspace_alias":
                    dataspaceAlias = ReadOptionalString(ref reader, $"{context}.dataspace_alias");
                    break;
                case "accounts":
                    accounts = ReadItems(ref reader, $"{context}.accounts", ReadUaidPortfolioAccount);
                    break;
                default:
                    throw new JsonException($"{context} contains unknown field `{propertyName}`.");
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
                RequireExactFields(seen, context, "uaid", "totals", "dataspaces");
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
                    throw new JsonException($"{context} contains unknown field `{propertyName}`.");
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
        ulong? dataspaceId = null;
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
                RequireExactFields(seen, context, "dataspace_id", "dataspace_alias", "accounts");
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
                    dataspaceId = ReadUInt64(ref reader, $"{context}.dataspace_id");
                    break;
                case "dataspace_alias":
                    dataspaceAlias = ReadOptionalString(ref reader, $"{context}.dataspace_alias");
                    break;
                case "accounts":
                    accounts = ReadStringList(ref reader, $"{context}.accounts");
                    break;
                default:
                    throw new JsonException($"{context} contains unknown field `{propertyName}`.");
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
                RequireExactFields(seen, context, "uaid", "dataspaces");
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
                    throw new JsonException($"{context} contains unknown field `{propertyName}`.");
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
        ulong? epoch = null;
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
                RequireExactFields(seen, context, "epoch", "reason");
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
                    epoch = ReadUInt64(ref reader, $"{context}.epoch");
                    break;
                case "reason":
                    reason = ReadOptionalString(ref reader, $"{context}.reason");
                    break;
                default:
                    throw new JsonException($"{context} contains unknown field `{propertyName}`.");
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
        ulong? activatedEpoch = null;
        ulong? expiredEpoch = null;
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
                RequireExactFields(seen, context, "activated_epoch", "expired_epoch", "revocation");
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
                    activatedEpoch = ReadNullableUInt64(ref reader, $"{context}.activated_epoch");
                    break;
                case "expired_epoch":
                    expiredEpoch = ReadNullableUInt64(ref reader, $"{context}.expired_epoch");
                    break;
                case "revocation":
                    revocation = ReadNullableItem(ref reader, $"{context}.revocation", ReadUaidManifestRevocation);
                    break;
                default:
                    throw new JsonException($"{context} contains unknown field `{propertyName}`.");
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
        ulong? dataspaceId = null;
        string? dataspaceAlias = null;
        string? manifestHash = null;
        ToriiUaidManifestStatus? status = null;
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
                        Status = RequireManifestStatus(status, $"{context}.status"),
                        Lifecycle = lifecycle!,
                        Accounts = RequireList(accounts, context, "accounts"),
                        Manifest = RequireNode(manifest, $"{context}.manifest"),
                    },
                    context);
                ValidateUaidManifestRecord(response, context);
                RequireExactFields(
                    seen,
                    context,
                    "dataspace_id",
                    "dataspace_alias",
                    "manifest_hash",
                    "status",
                    "lifecycle",
                    "accounts",
                    "manifest");
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
                    dataspaceId = ReadUInt64(ref reader, $"{context}.dataspace_id");
                    break;
                case "dataspace_alias":
                    dataspaceAlias = ReadOptionalString(ref reader, $"{context}.dataspace_alias");
                    break;
                case "manifest_hash":
                    manifestHash = ReadOptionalString(ref reader, $"{context}.manifest_hash");
                    break;
                case "status":
                    status = ReadManifestStatus(ref reader, $"{context}.status");
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
                    throw new JsonException($"{context} contains unknown field `{propertyName}`.");
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
        ulong? total = null;
        bool? hasMore = null;
        ToriiUaidManifestCountMode? countMode = null;
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
                        HasMore = RequireBoolean(hasMore, $"{context}.has_more"),
                        CountMode = RequireManifestCountMode(countMode, $"{context}.count_mode"),
                        Manifests = RequireList(manifests, context, "manifests"),
                    },
                    context);
                ValidateUaidManifestsResponse(response, context);
                RequireExactFields(seen, context, "uaid", "total", "has_more", "count_mode", "manifests");
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
                    total = ReadUInt64(ref reader, $"{context}.total");
                    break;
                case "has_more":
                    hasMore = ReadBoolean(ref reader, $"{context}.has_more");
                    break;
                case "count_mode":
                    countMode = ReadManifestCountMode(ref reader, $"{context}.count_mode");
                    break;
                case "manifests":
                    manifests = ReadItems(ref reader, $"{context}.manifests", ReadUaidManifestRecord);
                    break;
                default:
                    throw new JsonException($"{context} contains unknown field `{propertyName}`.");
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
        writer.WriteString("status", FormatManifestStatus(response.Status));
        writer.WritePropertyName("lifecycle");
        WriteUaidManifestLifecycle(writer, response.Lifecycle, $"{context}.lifecycle");
        WriteStringList(writer, "accounts", response.Accounts);
        writer.WritePropertyName("manifest");
        response.Manifest.WriteTo(writer);
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
        writer.WriteBoolean("has_more", response.HasMore);
        writer.WriteString("count_mode", FormatManifestCountMode(response.CountMode));
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
            nameof(ToriiUaidManifestsResponse.HasMore) => "has_more",
            nameof(ToriiUaidManifestsResponse.CountMode) => "count_mode",
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

    private static void RequireExactFields(HashSet<string> seen, string context, params string[] fields)
    {
        foreach (var field in fields)
        {
            if (!seen.Contains(field))
            {
                throw new JsonException($"{context}.{field} must be present.");
            }
        }
    }

    private static JsonObject RequireJsonObject(JsonNode? value, string context)
    {
        return value as JsonObject
            ?? throw new JsonException($"{context} must be a non-null object.");
    }

    private static JsonArray RequireJsonArray(JsonNode? value, string context)
    {
        return value as JsonArray
            ?? throw new JsonException($"{context} must be a non-null array.");
    }

    private static void RequireJsonFields(
        JsonObject value,
        string context,
        IReadOnlyCollection<string> required,
        IReadOnlyCollection<string> optional)
    {
        foreach (var property in value)
        {
            if (!required.Contains(property.Key) && !optional.Contains(property.Key))
            {
                throw new JsonException($"{context} contains unknown field `{property.Key}`.");
            }
        }
        foreach (var field in required)
        {
            if (!value.ContainsKey(field))
            {
                throw new JsonException($"{context}.{field} must be present.");
            }
            if (value[field] is null)
            {
                throw new JsonException($"{context}.{field} must not be null.");
            }
        }
    }

    private static ulong RequireJsonUInt64(JsonObject value, string propertyName, string context)
    {
        return RequireJsonUInt64(value[propertyName], $"{context}.{propertyName}");
    }

    internal static ulong RequireJsonUInt64(JsonNode? value, string context)
    {
        if (value is not JsonValue scalar)
        {
            throw new JsonException($"{context} must be an unsigned integer.");
        }

        if (scalar.TryGetValue<ulong>(out var unsigned64))
        {
            return unsigned64;
        }
        if (scalar.TryGetValue<uint>(out var unsigned32))
        {
            return unsigned32;
        }
        if (scalar.TryGetValue<ushort>(out var unsigned16))
        {
            return unsigned16;
        }
        if (scalar.TryGetValue<byte>(out var unsigned8))
        {
            return unsigned8;
        }
        if (scalar.TryGetValue<long>(out var signed64) && signed64 >= 0)
        {
            return (ulong)signed64;
        }
        if (scalar.TryGetValue<int>(out var signed32) && signed32 >= 0)
        {
            return (uint)signed32;
        }
        if (scalar.TryGetValue<short>(out var signed16) && signed16 >= 0)
        {
            return (ushort)signed16;
        }
        if (scalar.TryGetValue<sbyte>(out var signed8) && signed8 >= 0)
        {
            return (byte)signed8;
        }

        throw new JsonException($"{context} must be an unsigned integer.");
    }

    private static string RequireJsonString(JsonObject value, string propertyName, string context)
    {
        return RequireJsonString(value[propertyName], $"{context}.{propertyName}");
    }

    private static string RequireJsonString(JsonNode? value, string context)
    {
        if (value is not JsonValue scalar || !scalar.TryGetValue<string>(out var text))
        {
            throw new JsonException($"{context} must be a string.");
        }
        return text;
    }

    private static void RejectNullOptional(JsonNode? value, string context)
    {
        if (value is null)
        {
            throw new JsonException($"{context} must be omitted instead of null.");
        }
    }

    private static void ValidateOmittedOptionalString(
        JsonObject value,
        string propertyName,
        string context,
        bool validateCanonicalName)
    {
        if (!value.TryGetPropertyValue(propertyName, out var node))
        {
            return;
        }
        RejectNullOptional(node, $"{context}.{propertyName}");
        var text = RequireJsonString(node!, $"{context}.{propertyName}");
        if (validateCanonicalName
            && (text.Length == 0
                || !string.Equals(text.Trim(), text, StringComparison.Ordinal)
                || text.Any(char.IsControl)
                || !string.Equals(text.Normalize(NormalizationForm.FormC), text, StringComparison.Ordinal)))
        {
            throw new JsonException($"{context}.{propertyName} must use exact canonical NFC spelling.");
        }
    }

    private static void ValidateManifestStatus(ToriiUaidManifestStatus value, string context)
    {
        if (value is not (ToriiUaidManifestStatus.Active
            or ToriiUaidManifestStatus.Pending
            or ToriiUaidManifestStatus.Expired
            or ToriiUaidManifestStatus.Revoked))
        {
            throw new JsonException($"{context} contains an unknown manifest status.");
        }
    }

    private static void ValidateManifestCountMode(ToriiUaidManifestCountMode value, string context)
    {
        if (value is not (ToriiUaidManifestCountMode.Exact or ToriiUaidManifestCountMode.Bounded))
        {
            throw new JsonException($"{context} must be exact or bounded.");
        }
    }

    private static string FormatManifestStatus(ToriiUaidManifestStatus value)
    {
        return value switch
        {
            ToriiUaidManifestStatus.Active => "Active",
            ToriiUaidManifestStatus.Pending => "Pending",
            ToriiUaidManifestStatus.Expired => "Expired",
            ToriiUaidManifestStatus.Revoked => "Revoked",
            _ => throw new JsonException("Unknown UAID manifest status."),
        };
    }

    private static string FormatManifestCountMode(ToriiUaidManifestCountMode value)
    {
        return value switch
        {
            ToriiUaidManifestCountMode.Exact => "exact",
            ToriiUaidManifestCountMode.Bounded => "bounded",
            _ => throw new JsonException("Unknown UAID manifest count mode."),
        };
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

    private static bool ReadBoolean(ref Utf8JsonReader reader, string field)
    {
        return reader.TokenType switch
        {
            JsonTokenType.True => true,
            JsonTokenType.False => false,
            _ => throw new JsonException($"{field} must be a boolean."),
        };
    }

    private static ToriiUaidManifestStatus ReadManifestStatus(ref Utf8JsonReader reader, string field)
    {
        var value = ReadOptionalString(ref reader, field)
            ?? throw new JsonException($"{field} must not be null.");
        return value switch
        {
            "Active" => ToriiUaidManifestStatus.Active,
            "Pending" => ToriiUaidManifestStatus.Pending,
            "Expired" => ToriiUaidManifestStatus.Expired,
            "Revoked" => ToriiUaidManifestStatus.Revoked,
            _ => throw new JsonException($"{field} contains an unknown manifest status."),
        };
    }

    private static ToriiUaidManifestCountMode ReadManifestCountMode(ref Utf8JsonReader reader, string field)
    {
        var value = ReadOptionalString(ref reader, field)
            ?? throw new JsonException($"{field} must not be null.");
        return value switch
        {
            "exact" => ToriiUaidManifestCountMode.Exact,
            "bounded" => ToriiUaidManifestCountMode.Bounded,
            _ => throw new JsonException($"{field} must be exact or bounded."),
        };
    }

    private static ulong ReadUInt64(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType != JsonTokenType.Number || !reader.TryGetUInt64(out var value))
        {
            throw new JsonException($"{field} must be an unsigned integer.");
        }

        return value;
    }

    private static ulong? ReadNullableUInt64(ref Utf8JsonReader reader, string field)
    {
        return reader.TokenType == JsonTokenType.Null ? null : ReadUInt64(ref reader, field);
    }

    private static ulong RequireTotal(ulong? value, string context)
    {
        if (!value.HasValue)
        {
            throw new JsonException($"{context}.total must not be null.");
        }

        return value.Value;
    }

    private static ulong RequireCounter(ulong? value, string context, string propertyName)
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

    private static JsonNode RequireNode(JsonNode? value, string field)
    {
        return value ?? throw new JsonException($"{field} must not be null.");
    }

    private static bool RequireBoolean(bool? value, string field)
    {
        return value ?? throw new JsonException($"{field} must not be null.");
    }

    private static ToriiUaidManifestStatus RequireManifestStatus(
        ToriiUaidManifestStatus? value,
        string field)
    {
        return value ?? throw new JsonException($"{field} must not be null.");
    }

    private static ToriiUaidManifestCountMode RequireManifestCountMode(
        ToriiUaidManifestCountMode? value,
        string field)
    {
        return value ?? throw new JsonException($"{field} must not be null.");
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

    private static void ValidateCanonicalQuantityText(string? value, string field)
    {
        _ = RequireExactNonEmptyText(value, field);
        _ = ToriiQuantityJson.RequireCanonicalQuantity(value, field);
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

    private static void WriteNullableNumber(Utf8JsonWriter writer, string propertyName, ulong? value)
    {
        if (value is ulong integer)
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
