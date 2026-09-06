using System.Text.Json;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiNodeCapabilitiesJson
{
    private const int QueryProjectionArchiveVersion = 1;
    private const int QueryProjectionSchemaVersion = 1;
    private const int QueryProjectionBlobClassCustomId = 1001;
    private const string QueryProjectionCodec = "application/x-iroha-query-shard+norito+zstd";
    private const string QueryProjectionRowsetCodec = "application/x-iroha-query-shard-rowset+norito";
    private const string QueryProjectionCompression = "zstd";
    private const int QueryProjectionDefaultPartitionCount = 4096;

    private static readonly string[] QueryRowEnrichmentFields =
    [
        "primary_alias",
        "primary_alias_name",
        "primary_alias_dataspace",
        "primary_alias_domain",
        "has_primary_alias",
    ];

    private static readonly string[] QueryProjectionMetadataKeys =
    [
        "query_projection.locator",
        "query_projection.resource",
        "query_projection.partition_id",
        "query_projection.asset_definition_id",
        "query_projection.indexed_height",
        "query_projection.indexed_block_hash_hex",
        "query_projection.row_count",
        "query_projection.rowset_codec",
        "query_projection.rowset_hash_hex",
        "query_projection.emitted_at_unix",
    ];

    internal static void ValidateNodeCapabilities(ToriiNodeCapabilities? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        ValidateAbiVersionV1(response.AbiVersion, $"{context}.abi_version");
        ValidateNonNegativeInt32(response.DataModelVersion, $"{context}.data_model_version");
        ValidateExactLowercaseHexChars(
            response.SignedTransactionSchemaHashHex,
            $"{context}.signed_transaction_schema_hash_hex",
            32);
        ValidateNodeCryptoCapabilities(response.Crypto, $"{context}.crypto");
        ValidateNodeQueryCapabilities(response.Query, $"{context}.query");
    }

    internal static void ValidateNodeCryptoCapabilities(ToriiNodeCryptoCapabilities? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        ValidateNodeSmCapabilities(response.Sm, $"{context}.sm");
        ValidateNodeCurveCapabilities(response.Curves, $"{context}.curves");
    }

    internal static void ValidateNodeSmCapabilities(ToriiNodeSmCapabilities? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        ValidateExactTokenText(response.DefaultHash, $"{context}.default_hash");
        ValidateExactNonEmptyText(response.Sm2DistidDefault, $"{context}.sm2_distid_default");
        if (response.AllowedSigning is null)
        {
            throw new JsonException($"{context}.allowed_signing must not be null.");
        }

        var hasSm2Signing = ValidateAllowedSigningLabels(response.AllowedSigning, $"{context}.allowed_signing");
        ValidateSmDefaultHashConsistency(response.DefaultHash, hasSm2Signing, context);
        ValidateNodeSmAcceleration(response.Acceleration, $"{context}.acceleration");
    }

    internal static void ValidateNodeSmAcceleration(ToriiNodeSmAcceleration? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        if (!response.Scalar)
        {
            throw new JsonException($"{context}.scalar must be true.");
        }

        ValidateExactTokenText(response.Policy, $"{context}.policy");
        if (response.Policy is not ("auto" or "force-enable" or "force-disable" or "scalar-only"))
        {
            throw new JsonException($"{context}.policy must be one of auto, force-enable, force-disable, or scalar-only.");
        }

        if (response.NeonSm3 != response.NeonSm4)
        {
            throw new JsonException($"{context}.neon_sm3 and {context}.neon_sm4 must match.");
        }

        if ((response.Policy is "scalar-only" or "force-disable") && response.NeonSm3)
        {
            throw new JsonException($"{context}.policy must not advertise NEON acceleration when {context}.policy is {response.Policy}.");
        }
    }

    internal static void ValidateNodeCurveCapabilities(ToriiNodeCurveCapabilities? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        ValidateNonNegativeInt32(response.RegistryVersion, $"{context}.registry_version");
        if (response.AllowedCurveIds is null)
        {
            throw new JsonException($"{context}.allowed_curve_ids must not be null.");
        }
        if (response.AllowedCurveBitmap is null)
        {
            throw new JsonException($"{context}.allowed_curve_bitmap must not be null.");
        }

        for (var index = 0; index < response.AllowedCurveIds.Count; index++)
        {
            ValidateNonNegativeInt32(response.AllowedCurveIds[index], $"{context}.allowed_curve_ids[{index}]");
        }

        ValidateAllowedCurveBitmap(response.AllowedCurveIds, response.AllowedCurveBitmap, context);
    }

    internal static void ValidateNodeQueryCapabilities(ToriiNodeQueryCapabilities? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        ValidateNodeAggregateQueryCapabilities(response.Aggregate, $"{context}.aggregate");
        if (!response.IndexedSnapshotMarker)
        {
            throw new JsonException($"{context}.indexed_snapshot_marker must be true.");
        }

        ValidateExactTokenSequence(response.RowEnrichmentFields, QueryRowEnrichmentFields, $"{context}.row_enrichment_fields");
        ValidateNodeProjectionCapabilities(response.Projection, $"{context}.projection");
    }

    internal static void ValidateNodeAggregateQueryCapabilities(
        ToriiNodeAggregateQueryCapabilities? response,
        string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        if (!response.V1)
        {
            throw new JsonException($"{context}.v1 must be true.");
        }

        if (!response.ExactResults)
        {
            throw new JsonException($"{context}.exact_results must be true.");
        }

        ValidateExactUniqueTokenList(response.SupportedResources, $"{context}.supported_resources");
    }

    internal static void ValidateNodeProjectionCapabilities(ToriiNodeProjectionCapabilities? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        if (!response.CheckpointContractV1)
        {
            throw new JsonException($"{context}.checkpoint_contract_v1 must be true.");
        }

        if (response.DaV1Enabled)
        {
            throw new JsonException($"{context}.da_v1_enabled must be false.");
        }

        ValidateExactInt32(response.ArchiveVersion, QueryProjectionArchiveVersion, $"{context}.archive_version");
        ValidateExactInt32(response.SchemaVersion, QueryProjectionSchemaVersion, $"{context}.schema_version");
        ValidateExactInt32(response.BlobClassCustomId, QueryProjectionBlobClassCustomId, $"{context}.blob_class_custom_id");
        ValidateExactTokenText(response.Codec, $"{context}.codec");
        ValidateExactTokenValue(response.Codec, QueryProjectionCodec, $"{context}.codec");
        ValidateExactTokenText(response.RowsetCodec, $"{context}.rowset_codec");
        ValidateExactTokenValue(response.RowsetCodec, QueryProjectionRowsetCodec, $"{context}.rowset_codec");
        ValidateExactTokenText(response.Compression, $"{context}.compression");
        ValidateExactTokenValue(response.Compression, QueryProjectionCompression, $"{context}.compression");
        ValidateExactInt32(
            response.DefaultPartitionCount,
            QueryProjectionDefaultPartitionCount,
            $"{context}.default_partition_count");
        ValidateExactTokenSequence(response.MetadataKeys, QueryProjectionMetadataKeys, $"{context}.metadata_keys");
        ValidateExactUniqueTokenList(response.ExportSupportedResources, $"{context}.export_supported_resources");
        if (!response.ArchiveExportV1 && response.ExportSupportedResources.Count != 0)
        {
            throw new JsonException($"{context}.export_supported_resources must be empty when archive_export_v1 is false.");
        }

        if (response.LatestCheckpointIndexedHeight is < 0)
        {
            throw new JsonException($"{context}.latest_checkpoint_indexed_height must be non-negative.");
        }

        ToriiSseEventJson.RequireOptionalExactSizedHex(
            response.LatestCheckpointBlockHashHex,
            $"{context}.latest_checkpoint_block_hash_hex",
            32);
        if (response.LatestCheckpointBlockHashHex is not null && response.LatestCheckpointIndexedHeight is null)
        {
            throw new JsonException(
                $"{context}.latest_checkpoint_block_hash_hex requires latest_checkpoint_indexed_height.");
        }
    }

    internal static JsonException DirectMetadataErrorToJsonException(ArgumentException error, string context)
    {
        var paramName = error.ParamName ?? "metadata";
        var field = paramName switch
        {
            _ when TryMapCollectionField(paramName, nameof(ToriiNodeSmCapabilities.AllowedSigning), "allowed_signing", out var mapped) => mapped,
            _ when TryMapCollectionField(paramName, nameof(ToriiNodeCurveCapabilities.AllowedCurveIds), "allowed_curve_ids", out var mapped) => mapped,
            _ when TryMapCollectionField(paramName, nameof(ToriiNodeQueryCapabilities.RowEnrichmentFields), "row_enrichment_fields", out var mapped) => mapped,
            _ when TryMapCollectionField(paramName, nameof(ToriiNodeAggregateQueryCapabilities.SupportedResources), "supported_resources", out var mapped) => mapped,
            _ when TryMapCollectionField(paramName, nameof(ToriiNodeProjectionCapabilities.MetadataKeys), "metadata_keys", out var mapped) => mapped,
            _ when TryMapCollectionField(paramName, nameof(ToriiNodeProjectionCapabilities.ExportSupportedResources), "export_supported_resources", out var mapped) => mapped,
            nameof(ToriiNodeCapabilities.AbiVersion) => "abi_version",
            nameof(ToriiNodeCapabilities.DataModelVersion) => "data_model_version",
            nameof(ToriiNodeCapabilities.SignedTransactionSchemaHashHex) => "signed_transaction_schema_hash_hex",
            nameof(ToriiNodeCapabilities.Crypto) => "crypto",
            nameof(ToriiNodeCapabilities.Query) => "query",
            nameof(ToriiNodeCryptoCapabilities.Sm) => "sm",
            nameof(ToriiNodeCryptoCapabilities.Curves) => "curves",
            nameof(ToriiNodeSmCapabilities.DefaultHash) => "default_hash",
            nameof(ToriiNodeSmCapabilities.Sm2DistidDefault) => "sm2_distid_default",
            nameof(ToriiNodeSmCapabilities.Acceleration) => "acceleration",
            nameof(ToriiNodeSmAcceleration.Scalar) => "scalar",
            nameof(ToriiNodeSmAcceleration.Policy) => "policy",
            nameof(ToriiNodeCurveCapabilities.RegistryVersion) => "registry_version",
            nameof(ToriiNodeQueryCapabilities.Aggregate) => "aggregate",
            nameof(ToriiNodeQueryCapabilities.Projection) => "projection",
            nameof(ToriiNodeAggregateQueryCapabilities.V1) => "v1",
            nameof(ToriiNodeAggregateQueryCapabilities.ExactResults) => "exact_results",
            nameof(ToriiNodeProjectionCapabilities.CheckpointContractV1) => "checkpoint_contract_v1",
            nameof(ToriiNodeProjectionCapabilities.DaV1Enabled) => "da_v1_enabled",
            nameof(ToriiNodeProjectionCapabilities.ArchiveVersion) => "archive_version",
            nameof(ToriiNodeProjectionCapabilities.SchemaVersion) => "schema_version",
            nameof(ToriiNodeProjectionCapabilities.BlobClassCustomId) => "blob_class_custom_id",
            nameof(ToriiNodeProjectionCapabilities.Codec) => "codec",
            nameof(ToriiNodeProjectionCapabilities.RowsetCodec) => "rowset_codec",
            nameof(ToriiNodeProjectionCapabilities.Compression) => "compression",
            nameof(ToriiNodeProjectionCapabilities.DefaultPartitionCount) => "default_partition_count",
            nameof(ToriiNodeProjectionCapabilities.LatestCheckpointIndexedHeight) => "latest_checkpoint_indexed_height",
            nameof(ToriiNodeProjectionCapabilities.LatestCheckpointBlockHashHex) => "latest_checkpoint_block_hash_hex",
            _ => paramName,
        };

        return new JsonException($"{context}.{field}: {error.Message}", error);
    }

    private static T CreateWithDirectMetadataContext<T>(Func<T> factory, string context)
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

    private static bool TryMapCollectionField(
        string paramName,
        string propertyName,
        string jsonName,
        out string mapped)
    {
        var prefix = propertyName + "[";
        if (paramName.StartsWith(prefix, StringComparison.Ordinal))
        {
            mapped = jsonName + paramName[propertyName.Length..];
            return true;
        }

        mapped = string.Empty;
        return false;
    }

    internal static ToriiNodeCapabilities ReadNodeCapabilities(ref Utf8JsonReader reader, string context)
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
        int? abiVersion = null;
        int? dataModelVersion = null;
        string? signedTransactionSchemaHashHex = null;
        ToriiNodeCryptoCapabilities? crypto = null;
        ToriiNodeQueryCapabilities? query = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                return CreateWithDirectMetadataContext(() =>
                {
                    var response = new ToriiNodeCapabilities
                    {
                        AbiVersion = RequireInt32(abiVersion, context, "abi_version"),
                        DataModelVersion = RequireInt32(dataModelVersion, context, "data_model_version"),
                        SignedTransactionSchemaHashHex = RequireString(
                            signedTransactionSchemaHashHex,
                            $"{context}.signed_transaction_schema_hash_hex"),
                        Crypto = crypto!,
                        Query = query!,
                    };
                    ValidateNodeCapabilities(response, context);
                    return response;
                }, context);
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
                case "abi_version":
                    abiVersion = ReadInt32(ref reader, $"{context}.abi_version");
                    break;
                case "data_model_version":
                    dataModelVersion = ReadInt32(ref reader, $"{context}.data_model_version");
                    break;
                case "signed_transaction_schema_hash_hex":
                    signedTransactionSchemaHashHex = ReadOptionalString(ref reader, $"{context}.signed_transaction_schema_hash_hex");
                    break;
                case "crypto":
                    crypto = ReadNullableItem(ref reader, $"{context}.crypto", ReadNodeCryptoCapabilities);
                    break;
                case "query":
                    query = ReadNullableItem(ref reader, $"{context}.query", ReadNodeQueryCapabilities);
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiNodeCryptoCapabilities ReadNodeCryptoCapabilities(ref Utf8JsonReader reader, string context)
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
        ToriiNodeSmCapabilities? sm = null;
        ToriiNodeCurveCapabilities? curves = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                return CreateWithDirectMetadataContext(() =>
                {
                    var response = new ToriiNodeCryptoCapabilities
                    {
                        Sm = sm!,
                        Curves = curves!,
                    };
                    ValidateNodeCryptoCapabilities(response, context);
                    return response;
                }, context);
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
                case "sm":
                    sm = ReadNullableItem(ref reader, $"{context}.sm", ReadNodeSmCapabilities);
                    break;
                case "curves":
                    curves = ReadNullableItem(ref reader, $"{context}.curves", ReadNodeCurveCapabilities);
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiNodeSmCapabilities ReadNodeSmCapabilities(ref Utf8JsonReader reader, string context)
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
        bool? enabled = null;
        string? defaultHash = null;
        List<string>? allowedSigning = null;
        string? sm2DistidDefault = null;
        bool? opensslPreview = null;
        ToriiNodeSmAcceleration? acceleration = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                return CreateWithDirectMetadataContext(() =>
                {
                    var response = new ToriiNodeSmCapabilities
                    {
                        Enabled = RequireBool(enabled, context, "enabled"),
                        DefaultHash = RequireString(defaultHash, $"{context}.default_hash"),
                        AllowedSigning = RequireList(allowedSigning, context, "allowed_signing"),
                        Sm2DistidDefault = RequireString(sm2DistidDefault, $"{context}.sm2_distid_default"),
                        OpensslPreview = RequireBool(opensslPreview, context, "openssl_preview"),
                        Acceleration = acceleration!,
                    };
                    ValidateNodeSmCapabilities(response, context);
                    return response;
                }, context);
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
                case "enabled":
                    enabled = ReadBool(ref reader, $"{context}.enabled");
                    break;
                case "default_hash":
                    defaultHash = ReadOptionalString(ref reader, $"{context}.default_hash");
                    break;
                case "allowed_signing":
                    allowedSigning = ReadStringList(ref reader, $"{context}.allowed_signing");
                    break;
                case "sm2_distid_default":
                    sm2DistidDefault = ReadOptionalString(ref reader, $"{context}.sm2_distid_default");
                    break;
                case "openssl_preview":
                    opensslPreview = ReadBool(ref reader, $"{context}.openssl_preview");
                    break;
                case "acceleration":
                    acceleration = ReadNullableItem(ref reader, $"{context}.acceleration", ReadNodeSmAcceleration);
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiNodeSmAcceleration ReadNodeSmAcceleration(ref Utf8JsonReader reader, string context)
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
        bool? scalar = null;
        bool? neonSm3 = null;
        bool? neonSm4 = null;
        string? policy = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                return CreateWithDirectMetadataContext(() =>
                {
                    var response = new ToriiNodeSmAcceleration
                    {
                        Scalar = RequireBool(scalar, context, "scalar"),
                        NeonSm3 = RequireBool(neonSm3, context, "neon_sm3"),
                        NeonSm4 = RequireBool(neonSm4, context, "neon_sm4"),
                        Policy = RequireString(policy, $"{context}.policy"),
                    };
                    ValidateNodeSmAcceleration(response, context);
                    return response;
                }, context);
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
                case "scalar":
                    scalar = ReadBool(ref reader, $"{context}.scalar");
                    break;
                case "neon_sm3":
                    neonSm3 = ReadBool(ref reader, $"{context}.neon_sm3");
                    break;
                case "neon_sm4":
                    neonSm4 = ReadBool(ref reader, $"{context}.neon_sm4");
                    break;
                case "policy":
                    policy = ReadOptionalString(ref reader, $"{context}.policy");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiNodeCurveCapabilities ReadNodeCurveCapabilities(ref Utf8JsonReader reader, string context)
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
        int? registryVersion = null;
        List<int>? allowedCurveIds = null;
        List<ulong>? allowedCurveBitmap = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                return CreateWithDirectMetadataContext(() =>
                {
                    var response = new ToriiNodeCurveCapabilities
                    {
                        RegistryVersion = RequireInt32(registryVersion, context, "registry_version"),
                        AllowedCurveIds = RequireList(allowedCurveIds, context, "allowed_curve_ids"),
                        AllowedCurveBitmap = RequireList(allowedCurveBitmap, context, "allowed_curve_bitmap"),
                    };
                    ValidateNodeCurveCapabilities(response, context);
                    return response;
                }, context);
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
                case "registry_version":
                    registryVersion = ReadInt32(ref reader, $"{context}.registry_version");
                    break;
                case "allowed_curve_ids":
                    allowedCurveIds = ReadInt32List(ref reader, $"{context}.allowed_curve_ids");
                    break;
                case "allowed_curve_bitmap":
                    allowedCurveBitmap = ReadUInt64List(ref reader, $"{context}.allowed_curve_bitmap");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiNodeQueryCapabilities ReadNodeQueryCapabilities(ref Utf8JsonReader reader, string context)
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
        ToriiNodeAggregateQueryCapabilities? aggregate = null;
        bool? indexedSnapshotMarker = null;
        List<string>? rowEnrichmentFields = null;
        ToriiNodeProjectionCapabilities? projection = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                return CreateWithDirectMetadataContext(() =>
                {
                    var response = new ToriiNodeQueryCapabilities
                    {
                        Aggregate = aggregate!,
                        IndexedSnapshotMarker = RequireBool(indexedSnapshotMarker, context, "indexed_snapshot_marker"),
                        RowEnrichmentFields = RequireList(rowEnrichmentFields, context, "row_enrichment_fields"),
                        Projection = projection!,
                    };
                    ValidateNodeQueryCapabilities(response, context);
                    return response;
                }, context);
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
                case "aggregate":
                    aggregate = ReadNullableItem(ref reader, $"{context}.aggregate", ReadNodeAggregateQueryCapabilities);
                    break;
                case "indexed_snapshot_marker":
                    indexedSnapshotMarker = ReadBool(ref reader, $"{context}.indexed_snapshot_marker");
                    break;
                case "row_enrichment_fields":
                    rowEnrichmentFields = ReadStringList(ref reader, $"{context}.row_enrichment_fields");
                    break;
                case "projection":
                    projection = ReadNullableItem(ref reader, $"{context}.projection", ReadNodeProjectionCapabilities);
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiNodeAggregateQueryCapabilities ReadNodeAggregateQueryCapabilities(
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
        bool? v1 = null;
        bool? exactResults = null;
        List<string>? supportedResources = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                return CreateWithDirectMetadataContext(() =>
                {
                    var response = new ToriiNodeAggregateQueryCapabilities
                    {
                        V1 = RequireBool(v1, context, "v1"),
                        ExactResults = RequireBool(exactResults, context, "exact_results"),
                        SupportedResources = RequireList(supportedResources, context, "supported_resources"),
                    };
                    ValidateNodeAggregateQueryCapabilities(response, context);
                    return response;
                }, context);
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
                case "v1":
                    v1 = ReadBool(ref reader, $"{context}.v1");
                    break;
                case "exact_results":
                    exactResults = ReadBool(ref reader, $"{context}.exact_results");
                    break;
                case "supported_resources":
                    supportedResources = ReadStringList(ref reader, $"{context}.supported_resources");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiNodeProjectionCapabilities ReadNodeProjectionCapabilities(
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
        var response = new ToriiNodeProjectionCapabilities();
        var fields = new ProjectionFields();

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                return CreateWithDirectMetadataContext(() =>
                {
                    response = new ToriiNodeProjectionCapabilities
                    {
                        CheckpointContractV1 = RequireBool(
                            fields.CheckpointContractV1,
                            context,
                            "checkpoint_contract_v1"),
                        DaV1Enabled = RequireBool(fields.DaV1Enabled, context, "da_v1_enabled"),
                        ShardCatalogV1 = RequireBool(fields.ShardCatalogV1, context, "shard_catalog_v1"),
                        ArchiveExportV1 = RequireBool(fields.ArchiveExportV1, context, "archive_export_v1"),
                        ArchiveVersion = RequireInt32(fields.ArchiveVersion, context, "archive_version"),
                        SchemaVersion = RequireInt32(fields.SchemaVersion, context, "schema_version"),
                        BlobClassCustomId = RequireInt32(fields.BlobClassCustomId, context, "blob_class_custom_id"),
                        Codec = RequireString(fields.Codec, $"{context}.codec"),
                        RowsetCodec = RequireString(fields.RowsetCodec, $"{context}.rowset_codec"),
                        Compression = RequireString(fields.Compression, $"{context}.compression"),
                        DefaultPartitionCount = RequireInt32(
                            fields.DefaultPartitionCount,
                            context,
                            "default_partition_count"),
                        MetadataKeys = RequireList(fields.MetadataKeys, context, "metadata_keys"),
                        ExportSupportedResources = RequireList(
                            fields.ExportSupportedResources,
                            context,
                            "export_supported_resources"),
                        LatestCheckpointIndexedHeight = fields.LatestCheckpointIndexedHeight,
                        LatestCheckpointBlockHashHex = fields.LatestCheckpointBlockHashHex,
                    };
                    ValidateNodeProjectionCapabilities(response, context);
                    return response;
                }, context);
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

            ReadProjectionProperty(ref reader, context, propertyName, fields);
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static void WriteNodeCapabilities(Utf8JsonWriter writer, ToriiNodeCapabilities response, string context)
    {
        ValidateNodeCapabilities(response, context);

        writer.WriteStartObject();
        writer.WriteNumber("abi_version", response.AbiVersion);
        writer.WriteNumber("data_model_version", response.DataModelVersion);
        writer.WriteString("signed_transaction_schema_hash_hex", response.SignedTransactionSchemaHashHex);
        writer.WritePropertyName("crypto");
        WriteNodeCryptoCapabilities(writer, response.Crypto, $"{context}.crypto");
        writer.WritePropertyName("query");
        WriteNodeQueryCapabilities(writer, response.Query, $"{context}.query");
        writer.WriteEndObject();
    }

    internal static void WriteNodeCryptoCapabilities(
        Utf8JsonWriter writer,
        ToriiNodeCryptoCapabilities response,
        string context)
    {
        ValidateNodeCryptoCapabilities(response, context);

        writer.WriteStartObject();
        writer.WritePropertyName("sm");
        WriteNodeSmCapabilities(writer, response.Sm, $"{context}.sm");
        writer.WritePropertyName("curves");
        WriteNodeCurveCapabilities(writer, response.Curves, $"{context}.curves");
        writer.WriteEndObject();
    }

    internal static void WriteNodeSmCapabilities(Utf8JsonWriter writer, ToriiNodeSmCapabilities response, string context)
    {
        ValidateNodeSmCapabilities(response, context);

        writer.WriteStartObject();
        writer.WriteBoolean("enabled", response.Enabled);
        writer.WriteString("default_hash", response.DefaultHash);
        WriteStringList(writer, "allowed_signing", response.AllowedSigning);
        writer.WriteString("sm2_distid_default", response.Sm2DistidDefault);
        writer.WriteBoolean("openssl_preview", response.OpensslPreview);
        writer.WritePropertyName("acceleration");
        WriteNodeSmAcceleration(writer, response.Acceleration, $"{context}.acceleration");
        writer.WriteEndObject();
    }

    internal static void WriteNodeSmAcceleration(
        Utf8JsonWriter writer,
        ToriiNodeSmAcceleration response,
        string context)
    {
        ValidateNodeSmAcceleration(response, context);

        writer.WriteStartObject();
        writer.WriteBoolean("scalar", response.Scalar);
        writer.WriteBoolean("neon_sm3", response.NeonSm3);
        writer.WriteBoolean("neon_sm4", response.NeonSm4);
        writer.WriteString("policy", response.Policy);
        writer.WriteEndObject();
    }

    internal static void WriteNodeCurveCapabilities(
        Utf8JsonWriter writer,
        ToriiNodeCurveCapabilities response,
        string context)
    {
        ValidateNodeCurveCapabilities(response, context);

        writer.WriteStartObject();
        writer.WriteNumber("registry_version", response.RegistryVersion);
        WriteNumberList(writer, "allowed_curve_ids", response.AllowedCurveIds);
        WriteNumberList(writer, "allowed_curve_bitmap", response.AllowedCurveBitmap);
        writer.WriteEndObject();
    }

    internal static void WriteNodeQueryCapabilities(
        Utf8JsonWriter writer,
        ToriiNodeQueryCapabilities response,
        string context)
    {
        ValidateNodeQueryCapabilities(response, context);

        writer.WriteStartObject();
        writer.WritePropertyName("aggregate");
        WriteNodeAggregateQueryCapabilities(writer, response.Aggregate, $"{context}.aggregate");
        writer.WriteBoolean("indexed_snapshot_marker", response.IndexedSnapshotMarker);
        WriteStringList(writer, "row_enrichment_fields", response.RowEnrichmentFields);
        writer.WritePropertyName("projection");
        WriteNodeProjectionCapabilities(writer, response.Projection, $"{context}.projection");
        writer.WriteEndObject();
    }

    internal static void WriteNodeAggregateQueryCapabilities(
        Utf8JsonWriter writer,
        ToriiNodeAggregateQueryCapabilities response,
        string context)
    {
        ValidateNodeAggregateQueryCapabilities(response, context);

        writer.WriteStartObject();
        writer.WriteBoolean("v1", response.V1);
        writer.WriteBoolean("exact_results", response.ExactResults);
        WriteStringList(writer, "supported_resources", response.SupportedResources);
        writer.WriteEndObject();
    }

    internal static void WriteNodeProjectionCapabilities(
        Utf8JsonWriter writer,
        ToriiNodeProjectionCapabilities response,
        string context)
    {
        ValidateNodeProjectionCapabilities(response, context);

        writer.WriteStartObject();
        writer.WriteBoolean("checkpoint_contract_v1", response.CheckpointContractV1);
        writer.WriteBoolean("da_v1_enabled", response.DaV1Enabled);
        writer.WriteBoolean("shard_catalog_v1", response.ShardCatalogV1);
        writer.WriteBoolean("archive_export_v1", response.ArchiveExportV1);
        writer.WriteNumber("archive_version", response.ArchiveVersion);
        writer.WriteNumber("schema_version", response.SchemaVersion);
        writer.WriteNumber("blob_class_custom_id", response.BlobClassCustomId);
        writer.WriteString("codec", response.Codec);
        writer.WriteString("rowset_codec", response.RowsetCodec);
        writer.WriteString("compression", response.Compression);
        writer.WriteNumber("default_partition_count", response.DefaultPartitionCount);
        WriteStringList(writer, "metadata_keys", response.MetadataKeys);
        WriteStringList(writer, "export_supported_resources", response.ExportSupportedResources);
        WriteNullableNumber(writer, "latest_checkpoint_indexed_height", response.LatestCheckpointIndexedHeight);
        ToriiVpnJson.WriteNullableString(
            writer,
            "latest_checkpoint_block_hash_hex",
            response.LatestCheckpointBlockHashHex);
        writer.WriteEndObject();
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

    private static void ReadProjectionProperty(
        ref Utf8JsonReader reader,
        string context,
        string propertyName,
        ProjectionFields fields)
    {
        switch (propertyName)
        {
            case "checkpoint_contract_v1":
                fields.CheckpointContractV1 = ReadBool(ref reader, $"{context}.checkpoint_contract_v1");
                break;
            case "da_v1_enabled":
                fields.DaV1Enabled = ReadBool(ref reader, $"{context}.da_v1_enabled");
                break;
            case "shard_catalog_v1":
                fields.ShardCatalogV1 = ReadBool(ref reader, $"{context}.shard_catalog_v1");
                break;
            case "archive_export_v1":
                fields.ArchiveExportV1 = ReadBool(ref reader, $"{context}.archive_export_v1");
                break;
            case "archive_version":
                fields.ArchiveVersion = ReadInt32(ref reader, $"{context}.archive_version");
                break;
            case "schema_version":
                fields.SchemaVersion = ReadInt32(ref reader, $"{context}.schema_version");
                break;
            case "blob_class_custom_id":
                fields.BlobClassCustomId = ReadInt32(ref reader, $"{context}.blob_class_custom_id");
                break;
            case "codec":
                fields.Codec = ReadOptionalString(ref reader, $"{context}.codec");
                break;
            case "rowset_codec":
                fields.RowsetCodec = ReadOptionalString(ref reader, $"{context}.rowset_codec");
                break;
            case "compression":
                fields.Compression = ReadOptionalString(ref reader, $"{context}.compression");
                break;
            case "default_partition_count":
                fields.DefaultPartitionCount = ReadInt32(ref reader, $"{context}.default_partition_count");
                break;
            case "metadata_keys":
                fields.MetadataKeys = ReadStringList(ref reader, $"{context}.metadata_keys");
                break;
            case "export_supported_resources":
                fields.ExportSupportedResources = ReadStringList(ref reader, $"{context}.export_supported_resources");
                break;
            case "latest_checkpoint_indexed_height":
                fields.LatestCheckpointIndexedHeight = ReadNullableInt64(ref reader, $"{context}.latest_checkpoint_indexed_height");
                break;
            case "latest_checkpoint_block_hash_hex":
                fields.LatestCheckpointBlockHashHex = ReadOptionalString(ref reader, $"{context}.latest_checkpoint_block_hash_hex");
                break;
            default:
                ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                break;
        }
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

    private static int ReadInt32(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType != JsonTokenType.Number || !reader.TryGetInt32(out var value))
        {
            throw new JsonException($"{field} must be an integer.");
        }

        return value;
    }

    private static bool RequireBool(bool? value, string context, string propertyName)
    {
        if (value is null)
        {
            throw new JsonException($"{context}.{propertyName} must not be null.");
        }

        return value.Value;
    }

    private static int RequireInt32(int? value, string context, string propertyName)
    {
        if (value is null)
        {
            throw new JsonException($"{context}.{propertyName} must not be null.");
        }

        return value.Value;
    }

    private static long? ReadNullableInt64(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            return null;
        }

        if (reader.TokenType != JsonTokenType.Number || !reader.TryGetInt64(out var value))
        {
            throw new JsonException($"{field} must be an integer.");
        }

        return value;
    }

    private static ulong ReadUInt64(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType != JsonTokenType.Number || !reader.TryGetUInt64(out var value))
        {
            throw new JsonException($"{field} must be an unsigned integer.");
        }

        return value;
    }

    private static List<int>? ReadInt32List(ref Utf8JsonReader reader, string context)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            return null;
        }

        if (reader.TokenType != JsonTokenType.StartArray)
        {
            throw new JsonException($"{context} must be an array.");
        }

        var values = new List<int>();
        var index = 0;
        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndArray)
            {
                return values;
            }

            values.Add(ReadInt32(ref reader, $"{context}[{index}]"));
            index++;
        }

        throw new JsonException($"{context} array is incomplete.");
    }

    private static List<ulong>? ReadUInt64List(ref Utf8JsonReader reader, string context)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            return null;
        }

        if (reader.TokenType != JsonTokenType.StartArray)
        {
            throw new JsonException($"{context} must be an array.");
        }

        var values = new List<ulong>();
        var index = 0;
        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndArray)
            {
                return values;
            }

            values.Add(ReadUInt64(ref reader, $"{context}[{index}]"));
            index++;
        }

        throw new JsonException($"{context} array is incomplete.");
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

            values.Add(RequireString(ReadOptionalString(ref reader, $"{context}[{index}]"), $"{context}[{index}]"));
            index++;
        }

        throw new JsonException($"{context} array is incomplete.");
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

    private static IReadOnlyList<T> RequireList<T>(IReadOnlyList<T>? values, string context, string propertyName)
    {
        if (values is null)
        {
            throw new JsonException($"{context}.{propertyName} must not be null.");
        }

        return values;
    }

    private static bool ValidateAllowedSigningLabels(IReadOnlyList<string> allowedSigning, string context)
    {
        var seen = new HashSet<string>(StringComparer.Ordinal);
        var hasEd25519 = false;
        var hasSm2 = false;
        for (var index = 0; index < allowedSigning.Count; index++)
        {
            var label = allowedSigning[index];
            ValidateExactTokenText(label, $"{context}[{index}]");
            if (!seen.Add(label))
            {
                throw new JsonException($"{context}[{index}] must not contain duplicate signing labels.");
            }

            if (string.Equals(label, "ed25519", StringComparison.Ordinal))
            {
                hasEd25519 = true;
            }

            if (string.Equals(label, "sm2", StringComparison.Ordinal))
            {
                hasSm2 = true;
            }
        }

        if (!hasEd25519)
        {
            throw new JsonException($"{context} must include ed25519 for control-plane operations.");
        }

        return hasSm2;
    }

    private static void ValidateSmDefaultHashConsistency(string defaultHash, bool hasSm2Signing, string context)
    {
        var usesSm3Hash = string.Equals(defaultHash, "sm3-256", StringComparison.Ordinal);
        if (hasSm2Signing && !usesSm3Hash)
        {
            throw new JsonException($"{context}.default_hash must be sm3-256 when allowed_signing includes sm2.");
        }

        if (!hasSm2Signing && usesSm3Hash)
        {
            throw new JsonException($"{context}.allowed_signing must include sm2 when default_hash is sm3-256.");
        }
    }

    private static void ValidateAllowedCurveBitmap(
        IReadOnlyList<int> allowedCurveIds,
        IReadOnlyList<ulong> allowedCurveBitmap,
        string context)
    {
        var seen = new HashSet<int>();
        for (var index = 0; index < allowedCurveIds.Count; index++)
        {
            var curveId = allowedCurveIds[index];
            if (!seen.Add(curveId))
            {
                throw new JsonException($"{context}.allowed_curve_ids[{index}] must not contain duplicate curve ids.");
            }

            var wordIndex = curveId / 64;
            var bitMask = 1UL << (curveId & 63);
            if (wordIndex >= allowedCurveBitmap.Count || (allowedCurveBitmap[wordIndex] & bitMask) == 0)
            {
                throw new JsonException(
                    $"{context}.allowed_curve_ids[{index}] must have a matching allowed_curve_bitmap bit.");
            }
        }

        for (var wordIndex = 0; wordIndex < allowedCurveBitmap.Count; wordIndex++)
        {
            var word = allowedCurveBitmap[wordIndex];
            if (word == 0)
            {
                continue;
            }

            for (var bit = 0; bit < 64; bit++)
            {
                var bitMask = 1UL << bit;
                if ((word & bitMask) == 0)
                {
                    continue;
                }

                var curveId = ((long)wordIndex * 64L) + bit;
                if (curveId > int.MaxValue || !seen.Contains((int)curveId))
                {
                    throw new JsonException(
                        $"{context}.allowed_curve_bitmap[{wordIndex}] must not advertise curve ids missing from allowed_curve_ids.");
                }
            }
        }
    }

    private static void ValidateExactUniqueTokenList(IReadOnlyList<string>? values, string field)
    {
        if (values is null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);
        for (var index = 0; index < values.Count; index++)
        {
            var itemField = $"{field}[{index}]";
            var value = values[index];
            ValidateExactTokenText(value, itemField);
            if (!seen.Add(value))
            {
                throw new JsonException($"{itemField} must not contain duplicate capability labels.");
            }
        }
    }

    private static void ValidateExactTokenSequence(
        IReadOnlyList<string>? values,
        IReadOnlyList<string> expected,
        string field)
    {
        if (values is null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        if (values.Count != expected.Count)
        {
            throw new JsonException($"{field} must match the expected projection metadata key list.");
        }

        for (var index = 0; index < values.Count; index++)
        {
            var itemField = $"{field}[{index}]";
            ValidateExactTokenText(values[index], itemField);
            if (!string.Equals(values[index], expected[index], StringComparison.Ordinal))
            {
                throw new JsonException($"{itemField} must match the expected projection metadata key.");
            }
        }
    }

    private static void ValidateExactTokenValue(string value, string expected, string field)
    {
        if (!string.Equals(value, expected, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must be {expected}.");
        }
    }

    private static void ValidateExactTokenText(string? value, string field)
    {
        ValidateExactNonEmptyText(value, field);
        var text = value ?? throw new JsonException($"{field} must not be null.");
        if (text.Any(char.IsWhiteSpace))
        {
            throw new JsonException($"{field} must not contain whitespace.");
        }
    }

    private static void ValidateExactNonEmptyText(string? value, string field)
    {
        _ = ToriiSseEventJson.RequireOptionalExactNonEmptyText(value, field)
            ?? throw new JsonException($"{field} must be a non-empty string.");
    }

    private static void ValidateExactLowercaseHexChars(string? value, string field, int expectedChars)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new JsonException($"{field} must be a non-empty {expectedChars}-character lowercase hex string.");
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must not contain surrounding whitespace.");
        }

        if (value.Any(char.IsWhiteSpace))
        {
            throw new JsonException($"{field} must not contain whitespace.");
        }

        if (value.Any(char.IsControl))
        {
            throw new JsonException($"{field} must not contain control characters.");
        }

        if (value.Length != expectedChars || !IsLowercaseHex(value))
        {
            throw new JsonException($"{field} must be a {expectedChars}-character lowercase hex string.");
        }
    }

    private static void ValidateAbiVersionV1(int value, string field)
    {
        ValidateNonNegativeInt32(value, field);
        if (value != 1)
        {
            throw new JsonException($"{field} must be 1.");
        }
    }

    private static void ValidateNonNegativeInt32(int value, string field)
    {
        if (value < 0)
        {
            throw new JsonException($"{field} must be non-negative.");
        }
    }

    private static void ValidateExactInt32(int value, int expected, string field)
    {
        if (value != expected)
        {
            throw new JsonException($"{field} must be {expected}.");
        }
    }

    private static bool IsLowercaseHex(string value)
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

    private static void WriteNumberList(Utf8JsonWriter writer, string propertyName, IReadOnlyList<int> values)
    {
        writer.WritePropertyName(propertyName);
        writer.WriteStartArray();
        foreach (var value in values)
        {
            writer.WriteNumberValue(value);
        }
        writer.WriteEndArray();
    }

    private static void WriteNumberList(Utf8JsonWriter writer, string propertyName, IReadOnlyList<ulong> values)
    {
        writer.WritePropertyName(propertyName);
        writer.WriteStartArray();
        foreach (var value in values)
        {
            writer.WriteNumberValue(value);
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

    private sealed class ProjectionFields
    {
        public bool? CheckpointContractV1 { get; set; }

        public bool? DaV1Enabled { get; set; }

        public bool? ShardCatalogV1 { get; set; }

        public bool? ArchiveExportV1 { get; set; }

        public int? ArchiveVersion { get; set; }

        public int? SchemaVersion { get; set; }

        public int? BlobClassCustomId { get; set; }

        public string? Codec { get; set; }

        public string? RowsetCodec { get; set; }

        public string? Compression { get; set; }

        public int? DefaultPartitionCount { get; set; }

        public List<string>? MetadataKeys { get; set; }

        public List<string>? ExportSupportedResources { get; set; }

        public long? LatestCheckpointIndexedHeight { get; set; }

        public string? LatestCheckpointBlockHashHex { get; set; }
    }
}

internal sealed class ToriiNodeCapabilitiesJsonConverter : JsonConverter<ToriiNodeCapabilities>
{
    public override bool HandleNull => true;

    public override ToriiNodeCapabilities Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiNodeCapabilitiesJson.ReadNodeCapabilities(ref reader, "node capabilities response");
    }

    public override void Write(Utf8JsonWriter writer, ToriiNodeCapabilities value, JsonSerializerOptions options)
    {
        ToriiNodeCapabilitiesJson.WriteNodeCapabilities(writer, value, "node capabilities response");
    }
}

internal sealed class ToriiNodeCryptoCapabilitiesJsonConverter : JsonConverter<ToriiNodeCryptoCapabilities>
{
    public override bool HandleNull => true;

    public override ToriiNodeCryptoCapabilities Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiNodeCapabilitiesJson.ReadNodeCryptoCapabilities(ref reader, "node crypto capabilities");
    }

    public override void Write(Utf8JsonWriter writer, ToriiNodeCryptoCapabilities value, JsonSerializerOptions options)
    {
        ToriiNodeCapabilitiesJson.WriteNodeCryptoCapabilities(writer, value, "node crypto capabilities");
    }
}

internal sealed class ToriiNodeSmCapabilitiesJsonConverter : JsonConverter<ToriiNodeSmCapabilities>
{
    public override bool HandleNull => true;

    public override ToriiNodeSmCapabilities Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiNodeCapabilitiesJson.ReadNodeSmCapabilities(ref reader, "node SM capabilities");
    }

    public override void Write(Utf8JsonWriter writer, ToriiNodeSmCapabilities value, JsonSerializerOptions options)
    {
        ToriiNodeCapabilitiesJson.WriteNodeSmCapabilities(writer, value, "node SM capabilities");
    }
}

internal sealed class ToriiNodeSmAccelerationJsonConverter : JsonConverter<ToriiNodeSmAcceleration>
{
    public override bool HandleNull => true;

    public override ToriiNodeSmAcceleration Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiNodeCapabilitiesJson.ReadNodeSmAcceleration(ref reader, "node SM acceleration");
    }

    public override void Write(Utf8JsonWriter writer, ToriiNodeSmAcceleration value, JsonSerializerOptions options)
    {
        ToriiNodeCapabilitiesJson.WriteNodeSmAcceleration(writer, value, "node SM acceleration");
    }
}

internal sealed class ToriiNodeCurveCapabilitiesJsonConverter : JsonConverter<ToriiNodeCurveCapabilities>
{
    public override bool HandleNull => true;

    public override ToriiNodeCurveCapabilities Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiNodeCapabilitiesJson.ReadNodeCurveCapabilities(ref reader, "node curve capabilities");
    }

    public override void Write(Utf8JsonWriter writer, ToriiNodeCurveCapabilities value, JsonSerializerOptions options)
    {
        ToriiNodeCapabilitiesJson.WriteNodeCurveCapabilities(writer, value, "node curve capabilities");
    }
}

internal sealed class ToriiNodeQueryCapabilitiesJsonConverter : JsonConverter<ToriiNodeQueryCapabilities>
{
    public override bool HandleNull => true;

    public override ToriiNodeQueryCapabilities Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiNodeCapabilitiesJson.ReadNodeQueryCapabilities(ref reader, "node query capabilities");
    }

    public override void Write(Utf8JsonWriter writer, ToriiNodeQueryCapabilities value, JsonSerializerOptions options)
    {
        ToriiNodeCapabilitiesJson.WriteNodeQueryCapabilities(writer, value, "node query capabilities");
    }
}

internal sealed class ToriiNodeAggregateQueryCapabilitiesJsonConverter :
    JsonConverter<ToriiNodeAggregateQueryCapabilities>
{
    public override bool HandleNull => true;

    public override ToriiNodeAggregateQueryCapabilities Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiNodeCapabilitiesJson.ReadNodeAggregateQueryCapabilities(ref reader, "node aggregate query capabilities");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiNodeAggregateQueryCapabilities value,
        JsonSerializerOptions options)
    {
        ToriiNodeCapabilitiesJson.WriteNodeAggregateQueryCapabilities(
            writer,
            value,
            "node aggregate query capabilities");
    }
}

internal sealed class ToriiNodeProjectionCapabilitiesJsonConverter :
    JsonConverter<ToriiNodeProjectionCapabilities>
{
    public override bool HandleNull => true;

    public override ToriiNodeProjectionCapabilities Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiNodeCapabilitiesJson.ReadNodeProjectionCapabilities(ref reader, "node projection capabilities");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiNodeProjectionCapabilities value,
        JsonSerializerOptions options)
    {
        ToriiNodeCapabilitiesJson.WriteNodeProjectionCapabilities(writer, value, "node projection capabilities");
    }
}
