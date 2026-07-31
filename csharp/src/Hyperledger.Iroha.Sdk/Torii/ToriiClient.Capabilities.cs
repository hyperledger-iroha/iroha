// Node-capability probing, transaction compatibility, and response validation.

using System.Text.Json;

namespace Hyperledger.Iroha.Torii;

public sealed partial class ToriiClient
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

    public async Task<ToriiNodeCapabilities> GetNodeCapabilitiesAsync(CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiNodeCapabilities>("/v1/node/capabilities", cancellationToken: cancellationToken);
        ValidateNodeCapabilities(response, "node capabilities response");
        return response;
    }

    private static void ValidateNodeCapabilities(ToriiNodeCapabilities response, string context)
    {
        ToriiNodeCapabilitiesJson.ValidateNodeCapabilities(response, context);
    }

    private async Task EnsureTransactionSubmissionCompatibilityAsync(
        CancellationToken cancellationToken)
    {
        var capabilities = await GetNodeCapabilitiesAsync(cancellationToken);
        if (capabilities.DataModelVersion != ToriiNodeCapabilities.ExpectedDataModelVersion)
        {
            throw new ToriiDataModelMismatchException(
                ToriiNodeCapabilities.ExpectedDataModelVersion,
                capabilities.DataModelVersion);
        }

        if (!string.Equals(
                capabilities.SignedTransactionSchemaHashHex,
                ToriiNodeCapabilities.ExpectedSignedTransactionSchemaHashHex,
                StringComparison.Ordinal))
        {
            throw new ToriiTransactionSchemaMismatchException(
                ToriiNodeCapabilities.ExpectedSignedTransactionSchemaHashHex,
                capabilities.SignedTransactionSchemaHashHex);
        }
    }

    private static void ValidateNodeCryptoCapabilities(ToriiNodeCryptoCapabilities response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        if (response.Sm is null)
        {
            throw new JsonException($"{context}.sm must not be null.");
        }
        ValidateNodeSmCapabilities(response.Sm, $"{context}.sm");

        if (response.Curves is null)
        {
            throw new JsonException($"{context}.curves must not be null.");
        }
        ValidateNodeCurveCapabilities(response.Curves, $"{context}.curves");
    }

    private static void ValidateNodeSmCapabilities(ToriiNodeSmCapabilities response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ValidateExactTokenText(response.DefaultHash, $"{context}.default_hash");
        ValidateExactNonEmptyText(
            response.Sm2DistidDefault,
            $"{context}.sm2_distid_default",
            message => new JsonException(message));

        if (response.AllowedSigning is null)
        {
            throw new JsonException($"{context}.allowed_signing must not be null.");
        }

        var hasSm2Signing = ValidateAllowedSigningLabels(response.AllowedSigning, $"{context}.allowed_signing");
        ValidateSmDefaultHashConsistency(response.DefaultHash, hasSm2Signing, context);

        if (response.Acceleration is null)
        {
            throw new JsonException($"{context}.acceleration must not be null.");
        }
        ValidateNodeSmAcceleration(response.Acceleration, $"{context}.acceleration");
    }

    private static void ValidateNodeSmAcceleration(ToriiNodeSmAcceleration response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

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

    private static void ValidateNodeCurveCapabilities(ToriiNodeCurveCapabilities response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ValidateNonNegativeInt32(response.RegistryVersion, $"{context}.registry_version");

        if (response.AllowedCurveIds is null)
        {
            throw new JsonException($"{context}.allowed_curve_ids must not be null.");
        }

        for (var index = 0; index < response.AllowedCurveIds.Count; index++)
        {
            ValidateNonNegativeInt32(response.AllowedCurveIds[index], $"{context}.allowed_curve_ids[{index}]");
        }

        if (response.AllowedCurveBitmap is null)
        {
            throw new JsonException($"{context}.allowed_curve_bitmap must not be null.");
        }

        ValidateAllowedCurveBitmap(response.AllowedCurveIds, response.AllowedCurveBitmap, context);
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

    private static void ValidateNodeQueryCapabilities(ToriiNodeQueryCapabilities response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        if (response.Aggregate is null)
        {
            throw new JsonException($"{context}.aggregate must not be null.");
        }
        ValidateNodeAggregateQueryCapabilities(response.Aggregate, $"{context}.aggregate");

        if (!response.IndexedSnapshotMarker)
        {
            throw new JsonException($"{context}.indexed_snapshot_marker must be true.");
        }

        ValidateExactTokenSequence(response.RowEnrichmentFields, QueryRowEnrichmentFields, $"{context}.row_enrichment_fields");

        if (response.Projection is null)
        {
            throw new JsonException($"{context}.projection must not be null.");
        }
        ValidateNodeProjectionCapabilities(response.Projection, $"{context}.projection");
    }

    private static void ValidateNodeAggregateQueryCapabilities(
        ToriiNodeAggregateQueryCapabilities response,
        string context)
    {
        ArgumentNullException.ThrowIfNull(response);

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

    private static void ValidateNodeProjectionCapabilities(ToriiNodeProjectionCapabilities response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        if (!response.CheckpointContractV1)
        {
            throw new JsonException($"{context}.checkpoint_contract_v1 must be true.");
        }

        if (response.DaV1Enabled)
        {
            throw new JsonException($"{context}.da_v1_enabled must be false.");
        }

        ValidateProjectionFeatureFlags(response, context);
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

        ValidateOptionalExactSizedHex(response.LatestCheckpointBlockHashHex, $"{context}.latest_checkpoint_block_hash_hex", 32);
        if (response.LatestCheckpointBlockHashHex is not null && response.LatestCheckpointIndexedHeight is null)
        {
            throw new JsonException(
                $"{context}.latest_checkpoint_block_hash_hex requires latest_checkpoint_indexed_height.");
        }
    }

    private static void ValidateProjectionFeatureFlags(ToriiNodeProjectionCapabilities response, string context)
    {
        var expected = response.CheckpointPlanV1;
        if (response.CheckpointPublishV1 != expected ||
            response.ShardCatalogV1 != expected ||
            response.ArchiveExportV1 != expected)
        {
            throw new JsonException(
                $"{context}.checkpoint_plan_v1, {context}.checkpoint_publish_v1, {context}.shard_catalog_v1, and {context}.archive_export_v1 must match.");
        }
    }
}
