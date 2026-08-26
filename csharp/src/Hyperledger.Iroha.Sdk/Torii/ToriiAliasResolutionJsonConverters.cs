using System.Text.Json;
using System.Text.Json.Serialization;
using Hyperledger.Iroha.Address;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiAliasResolutionJson
{
    internal static void ValidateAssetAliasBinding(ToriiAssetAliasBinding? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        ToriiSseEventJson.RequireExactTokenText(response.Alias, $"{context}.alias");
        ToriiSseEventJson.RequireExactTokenText(response.Status, $"{context}.status");
        ValidatePositiveInt64(response.BoundAtMilliseconds, $"{context}.bound_at_ms");
        ValidateOptionalPositiveInt64(response.LeaseExpiryMilliseconds, $"{context}.lease_expiry_ms");
        ValidateOptionalPositiveInt64(response.GraceUntilMilliseconds, $"{context}.grace_until_ms");
    }

    internal static void ValidateAssetAliasResolution(ToriiAssetAliasResolution response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ToriiSseEventJson.RequireExactTokenText(response.Alias, $"{context}.alias");
        ToriiSseEventJson.RequireExactTokenText(response.AssetDefinitionId, $"{context}.asset_definition_id");
        ToriiSseEventJson.RequireExactTokenText(response.AssetName, $"{context}.asset_name");
        ToriiSseEventJson.RequireOptionalExactNonEmptyText(response.Description, $"{context}.description");
        ToriiSseEventJson.RequireOptionalExactTokenText(response.Logo, $"{context}.logo");
        ToriiSseEventJson.RequireOptionalExactTokenText(response.Source, $"{context}.source");
        if (response.AliasBinding is not null)
        {
            ValidateAssetAliasBinding(response.AliasBinding, $"{context}.alias_binding");
        }
    }

    internal static void ValidateAccountAliasResolution(ToriiAccountAliasResolution response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ToriiSseEventJson.RequireExactTokenText(response.Alias, $"{context}.alias");
        RequireCanonicalAccountId(response.AccountId, $"{context}.account_id");
        ToriiSseEventJson.RequireOptionalExactTokenText(response.Source, $"{context}.source");
        ValidateOptionalNonNegativeInt64(response.Index, $"{context}.index");
    }

    internal static void ValidateAccountAliasIndexResolution(
        ToriiAccountAliasIndexResolution response,
        string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ToriiSseEventJson.RequireExactTokenText(response.Alias, $"{context}.alias");
        RequireCanonicalAccountId(response.AccountId, $"{context}.account_id");
        ToriiSseEventJson.RequireOptionalExactTokenText(response.Source, $"{context}.source");
    }

    internal static void ValidateContractAliasBinding(ToriiContractAliasBinding? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        ToriiSseEventJson.RequireExactTokenText(response.Alias, $"{context}.alias");
        ToriiSseEventJson.RequireExactTokenText(response.Status, $"{context}.status");
        ValidatePositiveInt64(response.BoundAtMilliseconds, $"{context}.bound_at_ms");
        ValidateOptionalPositiveInt64(response.LeaseExpiryMilliseconds, $"{context}.lease_expiry_ms");
        ValidateOptionalPositiveInt64(response.GraceUntilMilliseconds, $"{context}.grace_until_ms");
    }

    internal static void ValidateContractAliasResolution(ToriiContractAliasResolution response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ToriiSseEventJson.RequireExactTokenText(response.ContractAlias, $"{context}.contract_alias");
        ToriiSseEventJson.RequireExactTokenText(response.ContractAddress, $"{context}.contract_address");
        ToriiSseEventJson.RequireExactTokenText(response.Dataspace, $"{context}.dataspace");
        ToriiSseEventJson.RequireOptionalExactTokenText(response.Source, $"{context}.source");
        if (response.ContractAliasBinding is not null)
        {
            ValidateContractAliasBinding(response.ContractAliasBinding, $"{context}.contract_alias_binding");
        }
    }

    internal static ToriiAssetAliasBinding ReadAssetAliasBinding(ref Utf8JsonReader reader, string context)
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
        string? status = null;
        long? boundAtMilliseconds = null;
        long? leaseExpiryMilliseconds = null;
        long? graceUntilMilliseconds = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiAssetAliasBinding
                    {
                        Alias = RequireString(alias, $"{context}.alias"),
                        Status = RequireString(status, $"{context}.status"),
                        BoundAtMilliseconds = RequireInt64(boundAtMilliseconds, context, "bound_at_ms"),
                        LeaseExpiryMilliseconds = leaseExpiryMilliseconds,
                        GraceUntilMilliseconds = graceUntilMilliseconds,
                    };
                    ValidateAssetAliasBinding(response, context);
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
                    alias = ReadOptionalString(ref reader, $"{context}.alias");
                    break;
                case "status":
                    status = ReadOptionalString(ref reader, $"{context}.status");
                    break;
                case "bound_at_ms":
                    boundAtMilliseconds = ReadInt64(ref reader, $"{context}.bound_at_ms");
                    break;
                case "lease_expiry_ms":
                    leaseExpiryMilliseconds = ReadNullableInt64(ref reader, $"{context}.lease_expiry_ms");
                    break;
                case "grace_until_ms":
                    graceUntilMilliseconds = ReadNullableInt64(ref reader, $"{context}.grace_until_ms");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiAssetAliasResolution ReadAssetAliasResolution(ref Utf8JsonReader reader, string context)
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
        string? assetDefinitionId = null;
        string? assetName = null;
        ToriiAssetAliasBinding? aliasBinding = null;
        string? description = null;
        string? logo = null;
        string? source = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiAssetAliasResolution
                    {
                        Alias = RequireString(alias, $"{context}.alias"),
                        AssetDefinitionId = RequireString(assetDefinitionId, $"{context}.asset_definition_id"),
                        AssetName = RequireString(assetName, $"{context}.asset_name"),
                        AliasBinding = aliasBinding,
                        Description = description,
                        Logo = logo,
                        Source = source,
                    };
                    ValidateAssetAliasResolution(response, context);
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
                    alias = ReadOptionalString(ref reader, $"{context}.alias");
                    break;
                case "asset_definition_id":
                    assetDefinitionId = ReadOptionalString(ref reader, $"{context}.asset_definition_id");
                    break;
                case "asset_name":
                    assetName = ReadOptionalString(ref reader, $"{context}.asset_name");
                    break;
                case "alias_binding":
                    aliasBinding = ReadNullableAssetAliasBinding(ref reader, $"{context}.alias_binding");
                    break;
                case "description":
                    description = ReadOptionalString(ref reader, $"{context}.description");
                    break;
                case "logo":
                    logo = ReadOptionalString(ref reader, $"{context}.logo");
                    break;
                case "source":
                    source = ReadOptionalString(ref reader, $"{context}.source");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiAccountAliasResolution ReadAccountAliasResolution(
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
        string? accountId = null;
        long? index = null;
        string? source = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiAccountAliasResolution
                    {
                        Alias = RequireString(alias, $"{context}.alias"),
                        AccountId = RequireString(accountId, $"{context}.account_id"),
                        Index = index,
                        Source = source,
                    };
                    ValidateAccountAliasResolution(response, context);
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
                    alias = ReadOptionalString(ref reader, $"{context}.alias");
                    break;
                case "account_id":
                    accountId = ReadOptionalString(ref reader, $"{context}.account_id");
                    break;
                case "index":
                    index = ReadNullableInt64(ref reader, $"{context}.index");
                    break;
                case "source":
                    source = ReadOptionalString(ref reader, $"{context}.source");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiAccountAliasIndexResolution ReadAccountAliasIndexResolution(
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
        ulong? index = null;
        string? alias = null;
        string? accountId = null;
        string? source = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiAccountAliasIndexResolution
                    {
                        Index = RequireUInt64(index, context, "index"),
                        Alias = RequireString(alias, $"{context}.alias"),
                        AccountId = RequireString(accountId, $"{context}.account_id"),
                        Source = source,
                    };
                    ValidateAccountAliasIndexResolution(response, context);
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
                case "index":
                    index = ReadUInt64(ref reader, $"{context}.index");
                    break;
                case "alias":
                    alias = ReadOptionalString(ref reader, $"{context}.alias");
                    break;
                case "account_id":
                    accountId = ReadOptionalString(ref reader, $"{context}.account_id");
                    break;
                case "source":
                    source = ReadOptionalString(ref reader, $"{context}.source");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiContractAliasBinding ReadContractAliasBinding(ref Utf8JsonReader reader, string context)
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
        string? status = null;
        long? boundAtMilliseconds = null;
        long? leaseExpiryMilliseconds = null;
        long? graceUntilMilliseconds = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiContractAliasBinding
                    {
                        Alias = RequireString(alias, $"{context}.alias"),
                        Status = RequireString(status, $"{context}.status"),
                        BoundAtMilliseconds = RequireInt64(boundAtMilliseconds, context, "bound_at_ms"),
                        LeaseExpiryMilliseconds = leaseExpiryMilliseconds,
                        GraceUntilMilliseconds = graceUntilMilliseconds,
                    };
                    ValidateContractAliasBinding(response, context);
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
                    alias = ReadOptionalString(ref reader, $"{context}.alias");
                    break;
                case "status":
                    status = ReadOptionalString(ref reader, $"{context}.status");
                    break;
                case "bound_at_ms":
                    boundAtMilliseconds = ReadInt64(ref reader, $"{context}.bound_at_ms");
                    break;
                case "lease_expiry_ms":
                    leaseExpiryMilliseconds = ReadNullableInt64(ref reader, $"{context}.lease_expiry_ms");
                    break;
                case "grace_until_ms":
                    graceUntilMilliseconds = ReadNullableInt64(ref reader, $"{context}.grace_until_ms");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiContractAliasResolution ReadContractAliasResolution(
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
        string? contractAlias = null;
        string? contractAddress = null;
        string? dataspace = null;
        ToriiContractAliasBinding? contractAliasBinding = null;
        string? source = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiContractAliasResolution
                    {
                        ContractAlias = RequireString(contractAlias, $"{context}.contract_alias"),
                        ContractAddress = RequireString(contractAddress, $"{context}.contract_address"),
                        Dataspace = RequireString(dataspace, $"{context}.dataspace"),
                        ContractAliasBinding = contractAliasBinding,
                        Source = source,
                    };
                    ValidateContractAliasResolution(response, context);
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
                case "contract_alias":
                    contractAlias = ReadOptionalString(ref reader, $"{context}.contract_alias");
                    break;
                case "contract_address":
                    contractAddress = ReadOptionalString(ref reader, $"{context}.contract_address");
                    break;
                case "dataspace":
                    dataspace = ReadOptionalString(ref reader, $"{context}.dataspace");
                    break;
                case "contract_alias_binding":
                    contractAliasBinding = ReadNullableContractAliasBinding(ref reader, $"{context}.contract_alias_binding");
                    break;
                case "source":
                    source = ReadOptionalString(ref reader, $"{context}.source");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static void WriteAssetAliasBinding(
        Utf8JsonWriter writer,
        ToriiAssetAliasBinding response,
        string context)
    {
        ValidateAssetAliasBinding(response, context);

        writer.WriteStartObject();
        writer.WriteString("alias", response.Alias);
        writer.WriteString("status", response.Status);
        writer.WriteNumber("bound_at_ms", response.BoundAtMilliseconds);
        WriteNullableNumber(writer, "lease_expiry_ms", response.LeaseExpiryMilliseconds);
        WriteNullableNumber(writer, "grace_until_ms", response.GraceUntilMilliseconds);
        writer.WriteEndObject();
    }

    internal static void WriteAssetAliasResolution(
        Utf8JsonWriter writer,
        ToriiAssetAliasResolution response,
        string context)
    {
        ValidateAssetAliasResolution(response, context);

        writer.WriteStartObject();
        writer.WriteString("alias", response.Alias);
        writer.WriteString("asset_definition_id", response.AssetDefinitionId);
        writer.WriteString("asset_name", response.AssetName);
        writer.WritePropertyName("alias_binding");
        if (response.AliasBinding is null)
        {
            writer.WriteNullValue();
        }
        else
        {
            WriteAssetAliasBinding(writer, response.AliasBinding, $"{context}.alias_binding");
        }
        ToriiVpnJson.WriteNullableString(writer, "description", response.Description);
        ToriiVpnJson.WriteNullableString(writer, "logo", response.Logo);
        ToriiVpnJson.WriteNullableString(writer, "source", response.Source);
        writer.WriteEndObject();
    }

    internal static void WriteAccountAliasResolution(
        Utf8JsonWriter writer,
        ToriiAccountAliasResolution response,
        string context)
    {
        ValidateAccountAliasResolution(response, context);

        writer.WriteStartObject();
        writer.WriteString("alias", response.Alias);
        writer.WriteString("account_id", response.AccountId);
        WriteNullableNumber(writer, "index", response.Index);
        ToriiVpnJson.WriteNullableString(writer, "source", response.Source);
        writer.WriteEndObject();
    }

    internal static void WriteAccountAliasIndexResolution(
        Utf8JsonWriter writer,
        ToriiAccountAliasIndexResolution response,
        string context)
    {
        ValidateAccountAliasIndexResolution(response, context);

        writer.WriteStartObject();
        writer.WriteNumber("index", response.Index);
        writer.WriteString("alias", response.Alias);
        writer.WriteString("account_id", response.AccountId);
        ToriiVpnJson.WriteNullableString(writer, "source", response.Source);
        writer.WriteEndObject();
    }

    internal static void WriteContractAliasBinding(
        Utf8JsonWriter writer,
        ToriiContractAliasBinding response,
        string context)
    {
        ValidateContractAliasBinding(response, context);

        writer.WriteStartObject();
        writer.WriteString("alias", response.Alias);
        writer.WriteString("status", response.Status);
        writer.WriteNumber("bound_at_ms", response.BoundAtMilliseconds);
        WriteNullableNumber(writer, "lease_expiry_ms", response.LeaseExpiryMilliseconds);
        WriteNullableNumber(writer, "grace_until_ms", response.GraceUntilMilliseconds);
        writer.WriteEndObject();
    }

    internal static void WriteContractAliasResolution(
        Utf8JsonWriter writer,
        ToriiContractAliasResolution response,
        string context)
    {
        ValidateContractAliasResolution(response, context);

        writer.WriteStartObject();
        writer.WriteString("contract_alias", response.ContractAlias);
        writer.WriteString("contract_address", response.ContractAddress);
        writer.WriteString("dataspace", response.Dataspace);
        writer.WritePropertyName("contract_alias_binding");
        if (response.ContractAliasBinding is null)
        {
            writer.WriteNullValue();
        }
        else
        {
            WriteContractAliasBinding(writer, response.ContractAliasBinding, $"{context}.contract_alias_binding");
        }
        ToriiVpnJson.WriteNullableString(writer, "source", response.Source);
        writer.WriteEndObject();
    }

    internal static JsonException DirectMetadataErrorToJsonException(ArgumentException error, string context)
    {
        var field = error.ParamName switch
        {
            "Alias" => "alias",
            "Status" => "status",
            "LeaseExpiryMilliseconds" => "lease_expiry_ms",
            "GraceUntilMilliseconds" => "grace_until_ms",
            "BoundAtMilliseconds" => "bound_at_ms",
            "AssetDefinitionId" => "asset_definition_id",
            "AssetName" => "asset_name",
            "Description" => "description",
            "Logo" => "logo",
            "Source" => "source",
            "AccountId" => "account_id",
            "Index" => "index",
            "ContractAlias" => "contract_alias",
            "ContractAddress" => "contract_address",
            "Dataspace" => "dataspace",
            _ => error.ParamName,
        };
        return new JsonException($"{context}.{field}: {error.Message}", error);
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

    private static ToriiAssetAliasBinding? ReadNullableAssetAliasBinding(
        ref Utf8JsonReader reader,
        string context)
    {
        return reader.TokenType == JsonTokenType.Null
            ? null
            : ReadAssetAliasBinding(ref reader, context);
    }

    private static ToriiContractAliasBinding? ReadNullableContractAliasBinding(
        ref Utf8JsonReader reader,
        string context)
    {
        return reader.TokenType == JsonTokenType.Null
            ? null
            : ReadContractAliasBinding(ref reader, context);
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

    private static long RequireInt64(long? value, string context, string propertyName)
    {
        if (!value.HasValue)
        {
            throw new JsonException($"{context}.{propertyName} must not be null.");
        }

        return value.Value;
    }

    private static ulong ReadUInt64(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType != JsonTokenType.Number || !reader.TryGetUInt64(out var value))
        {
            throw new JsonException($"{field} must be an unsigned integer.");
        }

        return value;
    }

    private static ulong RequireUInt64(ulong? value, string context, string propertyName)
    {
        if (!value.HasValue)
        {
            throw new JsonException($"{context}.{propertyName} must not be null.");
        }

        return value.Value;
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

internal sealed class ToriiAssetAliasBindingJsonConverter : JsonConverter<ToriiAssetAliasBinding>
{
    public override bool HandleNull => true;

    public override ToriiAssetAliasBinding Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiAliasResolutionJson.ReadAssetAliasBinding(ref reader, "asset alias binding");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiAssetAliasBinding value,
        JsonSerializerOptions options)
    {
        ToriiAliasResolutionJson.WriteAssetAliasBinding(writer, value, "asset alias binding");
    }
}

internal sealed class ToriiAssetAliasResolutionJsonConverter : JsonConverter<ToriiAssetAliasResolution>
{
    public override bool HandleNull => true;

    public override ToriiAssetAliasResolution Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiAliasResolutionJson.ReadAssetAliasResolution(ref reader, "asset alias resolution");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiAssetAliasResolution value,
        JsonSerializerOptions options)
    {
        ToriiAliasResolutionJson.WriteAssetAliasResolution(writer, value, "asset alias resolution");
    }
}

internal sealed class ToriiAccountAliasResolutionJsonConverter : JsonConverter<ToriiAccountAliasResolution>
{
    public override bool HandleNull => true;

    public override ToriiAccountAliasResolution Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiAliasResolutionJson.ReadAccountAliasResolution(ref reader, "account alias resolution");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiAccountAliasResolution value,
        JsonSerializerOptions options)
    {
        ToriiAliasResolutionJson.WriteAccountAliasResolution(writer, value, "account alias resolution");
    }
}

internal sealed class ToriiAccountAliasIndexResolutionJsonConverter :
    JsonConverter<ToriiAccountAliasIndexResolution>
{
    public override bool HandleNull => true;

    public override ToriiAccountAliasIndexResolution Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiAliasResolutionJson.ReadAccountAliasIndexResolution(ref reader, "account alias index resolution");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiAccountAliasIndexResolution value,
        JsonSerializerOptions options)
    {
        ToriiAliasResolutionJson.WriteAccountAliasIndexResolution(writer, value, "account alias index resolution");
    }
}

internal sealed class ToriiContractAliasBindingJsonConverter : JsonConverter<ToriiContractAliasBinding>
{
    public override bool HandleNull => true;

    public override ToriiContractAliasBinding Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiAliasResolutionJson.ReadContractAliasBinding(ref reader, "contract alias binding");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiContractAliasBinding value,
        JsonSerializerOptions options)
    {
        ToriiAliasResolutionJson.WriteContractAliasBinding(writer, value, "contract alias binding");
    }
}

internal sealed class ToriiContractAliasResolutionJsonConverter : JsonConverter<ToriiContractAliasResolution>
{
    public override bool HandleNull => true;

    public override ToriiContractAliasResolution Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiAliasResolutionJson.ReadContractAliasResolution(ref reader, "contract alias resolution");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiContractAliasResolution value,
        JsonSerializerOptions options)
    {
        ToriiAliasResolutionJson.WriteContractAliasResolution(writer, value, "contract alias resolution");
    }
}
