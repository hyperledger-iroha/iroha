// Strict DTOs and direct-metadata guards for Torii's node-capabilities endpoint.

using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

[JsonConverter(typeof(ToriiNodeCapabilitiesJsonConverter))]
public sealed record class ToriiNodeCapabilities
{
    /// <summary>Current first-release data-model version encoded by this SDK.</summary>
    public const int ExpectedDataModelVersion = 4;

    /// <summary>
    /// Current <c>SignedTransaction</c> Norito schema hash encoded by this SDK.
    /// </summary>
    public const string ExpectedSignedTransactionSchemaHashHex =
        "7ab5ff9c572efb316deac478f19209c5";

    private int abiVersion = 1;
    private int dataModelVersion;
    private string signedTransactionSchemaHashHex = string.Empty;
    private ToriiNodeCryptoCapabilities crypto = new();
    private ToriiNodeQueryCapabilities query = new();

    [JsonPropertyName("abi_version")]
    public int AbiVersion
    {
        get => abiVersion;
        init => abiVersion = ToriiNodeCapabilitiesDirectMetadata.RequireAbiVersionV1(value, nameof(AbiVersion));
    }

    [JsonPropertyName("data_model_version")]
    public int DataModelVersion
    {
        get => dataModelVersion;
        init => dataModelVersion = ToriiNodeCapabilitiesDirectMetadata.RequireNonNegativeInt32(
            value,
            nameof(DataModelVersion));
    }

    [JsonPropertyName("signed_transaction_schema_hash_hex")]
    public string SignedTransactionSchemaHashHex
    {
        get => signedTransactionSchemaHashHex;
        init => signedTransactionSchemaHashHex = ToriiNodeCapabilitiesDirectMetadata.RequireExactHexChars(
            value,
            32,
            nameof(SignedTransactionSchemaHashHex));
    }

    [JsonPropertyName("crypto")]
    public ToriiNodeCryptoCapabilities Crypto
    {
        get => crypto;
        init => crypto = ToriiNodeCapabilitiesDirectMetadata.RequireObject(value, nameof(Crypto));
    }

    [JsonPropertyName("query")]
    public ToriiNodeQueryCapabilities Query
    {
        get => query;
        init => query = ToriiNodeCapabilitiesDirectMetadata.RequireObject(value, nameof(Query));
    }
}

[JsonConverter(typeof(ToriiNodeCryptoCapabilitiesJsonConverter))]
public sealed record class ToriiNodeCryptoCapabilities
{
    private ToriiNodeSmCapabilities sm = new();
    private ToriiNodeCurveCapabilities curves = new();

    [JsonPropertyName("sm")]
    public ToriiNodeSmCapabilities Sm
    {
        get => sm;
        init => sm = ToriiNodeCapabilitiesDirectMetadata.RequireObject(value, nameof(Sm));
    }

    [JsonPropertyName("curves")]
    public ToriiNodeCurveCapabilities Curves
    {
        get => curves;
        init => curves = ToriiNodeCapabilitiesDirectMetadata.RequireObject(value, nameof(Curves));
    }
}

[JsonConverter(typeof(ToriiNodeSmCapabilitiesJsonConverter))]
public sealed record class ToriiNodeSmCapabilities
{
    private string defaultHash = string.Empty;
    private string[] allowedSigning = Array.Empty<string>();
    private string sm2DistidDefault = string.Empty;
    private ToriiNodeSmAcceleration acceleration = new();

    [JsonPropertyName("enabled")]
    public bool Enabled { get; init; }

    [JsonPropertyName("default_hash")]
    public string DefaultHash
    {
        get => defaultHash;
        init => defaultHash = ToriiNodeCapabilitiesDirectMetadata.RequireExactTokenText(value, nameof(DefaultHash));
    }

    [JsonPropertyName("allowed_signing")]
    public IReadOnlyList<string> AllowedSigning
    {
        get => ToriiListSnapshots.CopyRequired(allowedSigning);
        init => allowedSigning = ToriiNodeCapabilitiesDirectMetadata.CopyRequiredExactTokenTextList(
            value,
            nameof(AllowedSigning));
    }

    [JsonPropertyName("sm2_distid_default")]
    public string Sm2DistidDefault
    {
        get => sm2DistidDefault;
        init => sm2DistidDefault = ToriiNodeCapabilitiesDirectMetadata.RequireExactNonEmptyText(
            value,
            nameof(Sm2DistidDefault));
    }

    [JsonPropertyName("openssl_preview")]
    public bool OpensslPreview { get; init; }

    [JsonPropertyName("acceleration")]
    public ToriiNodeSmAcceleration Acceleration
    {
        get => acceleration;
        init => acceleration = ToriiNodeCapabilitiesDirectMetadata.RequireObject(value, nameof(Acceleration));
    }
}

[JsonConverter(typeof(ToriiNodeSmAccelerationJsonConverter))]
public sealed record class ToriiNodeSmAcceleration
{
    private bool scalar;
    private string policy = string.Empty;

    [JsonPropertyName("scalar")]
    public bool Scalar
    {
        get => scalar;
        init => scalar = ToriiNodeCapabilitiesDirectMetadata.RequireTrue(value, nameof(Scalar));
    }

    [JsonPropertyName("neon_sm3")]
    public bool NeonSm3 { get; init; }

    [JsonPropertyName("neon_sm4")]
    public bool NeonSm4 { get; init; }

    [JsonPropertyName("policy")]
    public string Policy
    {
        get => policy;
        init => policy = ToriiNodeCapabilitiesDirectMetadata.RequireExactTokenText(value, nameof(Policy));
    }
}

[JsonConverter(typeof(ToriiNodeCurveCapabilitiesJsonConverter))]
public sealed record class ToriiNodeCurveCapabilities
{
    private int registryVersion;
    private int[] allowedCurveIds = Array.Empty<int>();
    private ulong[] allowedCurveBitmap = Array.Empty<ulong>();

    [JsonPropertyName("registry_version")]
    public int RegistryVersion
    {
        get => registryVersion;
        init => registryVersion = ToriiNodeCapabilitiesDirectMetadata.RequireNonNegativeInt32(
            value,
            nameof(RegistryVersion));
    }

    [JsonPropertyName("allowed_curve_ids")]
    public IReadOnlyList<int> AllowedCurveIds
    {
        get => ToriiListSnapshots.CopyRequired(allowedCurveIds);
        init => allowedCurveIds = ToriiNodeCapabilitiesDirectMetadata.CopyRequiredNonNegativeInt32List(
            value,
            nameof(AllowedCurveIds));
    }

    [JsonPropertyName("allowed_curve_bitmap")]
    public IReadOnlyList<ulong> AllowedCurveBitmap
    {
        get => ToriiListSnapshots.CopyRequired(allowedCurveBitmap);
        init => allowedCurveBitmap = ToriiListSnapshots.CopyRequired(value);
    }
}

[JsonConverter(typeof(ToriiNodeQueryCapabilitiesJsonConverter))]
public sealed record class ToriiNodeQueryCapabilities
{
    private ToriiNodeAggregateQueryCapabilities aggregate = new();
    private string[] rowEnrichmentFields = Array.Empty<string>();
    private ToriiNodeProjectionCapabilities projection = new();

    [JsonPropertyName("aggregate")]
    public ToriiNodeAggregateQueryCapabilities Aggregate
    {
        get => aggregate;
        init => aggregate = ToriiNodeCapabilitiesDirectMetadata.RequireObject(value, nameof(Aggregate));
    }

    [JsonPropertyName("indexed_snapshot_marker")]
    public bool IndexedSnapshotMarker { get; init; }

    [JsonPropertyName("row_enrichment_fields")]
    public IReadOnlyList<string> RowEnrichmentFields
    {
        get => ToriiListSnapshots.CopyRequired(rowEnrichmentFields);
        init => rowEnrichmentFields = ToriiNodeCapabilitiesDirectMetadata.CopyRequiredExactTokenTextList(
            value,
            nameof(RowEnrichmentFields));
    }

    [JsonPropertyName("projection")]
    public ToriiNodeProjectionCapabilities Projection
    {
        get => projection;
        init => projection = ToriiNodeCapabilitiesDirectMetadata.RequireObject(value, nameof(Projection));
    }
}

[JsonConverter(typeof(ToriiNodeAggregateQueryCapabilitiesJsonConverter))]
public sealed record class ToriiNodeAggregateQueryCapabilities
{
    private bool v1;
    private bool exactResults;
    private string[] supportedResources = Array.Empty<string>();

    [JsonPropertyName("v1")]
    public bool V1
    {
        get => v1;
        init => v1 = ToriiNodeCapabilitiesDirectMetadata.RequireTrue(value, nameof(V1));
    }

    [JsonPropertyName("exact_results")]
    public bool ExactResults
    {
        get => exactResults;
        init => exactResults = ToriiNodeCapabilitiesDirectMetadata.RequireTrue(value, nameof(ExactResults));
    }

    [JsonPropertyName("supported_resources")]
    public IReadOnlyList<string> SupportedResources
    {
        get => ToriiListSnapshots.CopyRequired(supportedResources);
        init => supportedResources = ToriiNodeCapabilitiesDirectMetadata.CopyRequiredExactTokenTextList(
            value,
            nameof(SupportedResources));
    }
}

[JsonConverter(typeof(ToriiNodeProjectionCapabilitiesJsonConverter))]
public sealed record class ToriiNodeProjectionCapabilities
{
    private bool checkpointContractV1;
    private bool daV1Enabled;
    private int archiveVersion = 1;
    private int schemaVersion = 1;
    private int blobClassCustomId = 1001;
    private string codec = "application/x-iroha-query-shard+norito+zstd";
    private string rowsetCodec = "application/x-iroha-query-shard-rowset+norito";
    private string compression = "zstd";
    private int defaultPartitionCount = 4096;
    private string[] metadataKeys = Array.Empty<string>();
    private string[] exportSupportedResources = Array.Empty<string>();
    private long? latestCheckpointIndexedHeight;
    private string? latestCheckpointBlockHashHex;

    [JsonPropertyName("checkpoint_contract_v1")]
    public bool CheckpointContractV1
    {
        get => checkpointContractV1;
        init => checkpointContractV1 = ToriiNodeCapabilitiesDirectMetadata.RequireTrue(
            value,
            nameof(CheckpointContractV1));
    }

    [JsonPropertyName("da_v1_enabled")]
    public bool DaV1Enabled
    {
        get => daV1Enabled;
        init => daV1Enabled = ToriiNodeCapabilitiesDirectMetadata.RequireFalse(value, nameof(DaV1Enabled));
    }

    [JsonPropertyName("checkpoint_plan_v1")]
    public bool CheckpointPlanV1 { get; init; }

    [JsonPropertyName("checkpoint_publish_v1")]
    public bool CheckpointPublishV1 { get; init; }

    [JsonPropertyName("shard_catalog_v1")]
    public bool ShardCatalogV1 { get; init; }

    [JsonPropertyName("archive_export_v1")]
    public bool ArchiveExportV1 { get; init; }

    [JsonPropertyName("archive_version")]
    public int ArchiveVersion
    {
        get => archiveVersion;
        init => archiveVersion = ToriiNodeCapabilitiesDirectMetadata.RequireExactInt32(
            value,
            1,
            nameof(ArchiveVersion));
    }

    [JsonPropertyName("schema_version")]
    public int SchemaVersion
    {
        get => schemaVersion;
        init => schemaVersion = ToriiNodeCapabilitiesDirectMetadata.RequireExactInt32(value, 1, nameof(SchemaVersion));
    }

    [JsonPropertyName("blob_class_custom_id")]
    public int BlobClassCustomId
    {
        get => blobClassCustomId;
        init => blobClassCustomId = ToriiNodeCapabilitiesDirectMetadata.RequireExactInt32(
            value,
            1001,
            nameof(BlobClassCustomId));
    }

    [JsonPropertyName("codec")]
    public string Codec
    {
        get => codec;
        init => codec = ToriiNodeCapabilitiesDirectMetadata.RequireExactTokenValue(
            value,
            "application/x-iroha-query-shard+norito+zstd",
            nameof(Codec));
    }

    [JsonPropertyName("rowset_codec")]
    public string RowsetCodec
    {
        get => rowsetCodec;
        init => rowsetCodec = ToriiNodeCapabilitiesDirectMetadata.RequireExactTokenValue(
            value,
            "application/x-iroha-query-shard-rowset+norito",
            nameof(RowsetCodec));
    }

    [JsonPropertyName("compression")]
    public string Compression
    {
        get => compression;
        init => compression = ToriiNodeCapabilitiesDirectMetadata.RequireExactTokenValue(
            value,
            "zstd",
            nameof(Compression));
    }

    [JsonPropertyName("default_partition_count")]
    public int DefaultPartitionCount
    {
        get => defaultPartitionCount;
        init => defaultPartitionCount = ToriiNodeCapabilitiesDirectMetadata.RequireExactInt32(
            value,
            4096,
            nameof(DefaultPartitionCount));
    }

    [JsonPropertyName("metadata_keys")]
    public IReadOnlyList<string> MetadataKeys
    {
        get => ToriiListSnapshots.CopyRequired(metadataKeys);
        init => metadataKeys = ToriiNodeCapabilitiesDirectMetadata.CopyRequiredExactTokenTextList(
            value,
            nameof(MetadataKeys));
    }

    [JsonPropertyName("export_supported_resources")]
    public IReadOnlyList<string> ExportSupportedResources
    {
        get => ToriiListSnapshots.CopyRequired(exportSupportedResources);
        init => exportSupportedResources = ToriiNodeCapabilitiesDirectMetadata.CopyRequiredExactTokenTextList(
            value,
            nameof(ExportSupportedResources));
    }

    [JsonPropertyName("latest_checkpoint_indexed_height")]
    public long? LatestCheckpointIndexedHeight
    {
        get => latestCheckpointIndexedHeight;
        init => latestCheckpointIndexedHeight = ToriiNodeCapabilitiesDirectMetadata.RequireOptionalNonNegativeInt64(
            value,
            nameof(LatestCheckpointIndexedHeight));
    }

    [JsonPropertyName("latest_checkpoint_block_hash_hex")]
    public string? LatestCheckpointBlockHashHex
    {
        get => latestCheckpointBlockHashHex;
        init => latestCheckpointBlockHashHex = ToriiNodeCapabilitiesDirectMetadata.RequireOptionalExactSizedHex(
            value,
            nameof(LatestCheckpointBlockHashHex));
    }
}

internal static class ToriiNodeCapabilitiesDirectMetadata
{
    internal static T RequireObject<T>(T? value, string paramName)
        where T : class
    {
        return value ?? throw new ArgumentNullException(paramName, "Value must not be null.");
    }

    internal static int RequireAbiVersionV1(int value, string paramName)
    {
        RequireNonNegativeInt32(value, paramName);
        if (value != 1)
        {
            throw new ArgumentException("Value must be 1.", paramName);
        }

        return value;
    }

    internal static int RequireNonNegativeInt32(int value, string paramName)
    {
        if (value < 0)
        {
            throw new ArgumentOutOfRangeException(paramName, value, "Value must be non-negative.");
        }

        return value;
    }

    internal static long? RequireOptionalNonNegativeInt64(long? value, string paramName)
    {
        if (value is < 0)
        {
            throw new ArgumentOutOfRangeException(paramName, value, "Value must be non-negative.");
        }

        return value;
    }

    internal static int RequireExactInt32(int value, int expected, string paramName)
    {
        if (value != expected)
        {
            throw new ArgumentException($"Value must be {expected}.", paramName);
        }

        return value;
    }

    internal static bool RequireTrue(bool value, string paramName)
    {
        if (!value)
        {
            throw new ArgumentException("Value must be true.", paramName);
        }

        return value;
    }

    internal static bool RequireFalse(bool value, string paramName)
    {
        if (value)
        {
            throw new ArgumentException("Value must be false.", paramName);
        }

        return value;
    }

    internal static string RequireExactTokenText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireExactTokenText(value, paramName);
    }

    internal static string RequireExactNonEmptyText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireExactNonEmptyText(value, paramName);
    }

    internal static string RequireExactTokenValue(string? value, string expected, string paramName)
    {
        var exact = RequireExactTokenText(value, paramName);
        if (!string.Equals(exact, expected, StringComparison.Ordinal))
        {
            throw new ArgumentException($"Value must be {expected}.", paramName);
        }

        return exact;
    }

    internal static string RequireExactSizedHex(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireExactSizedHex(value, paramName, 32);
    }

    internal static string RequireExactHexChars(string? value, int expectedChars, string paramName)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException(
                $"Value must be a non-empty {expectedChars}-character lowercase hex string.",
                paramName);
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException("Value must not contain surrounding whitespace.", paramName);
        }

        foreach (var character in value)
        {
            if (char.IsWhiteSpace(character))
            {
                throw new ArgumentException("Value must not contain whitespace.", paramName);
            }

            if (char.IsControl(character))
            {
                throw new ArgumentException("Value must not contain control characters.", paramName);
            }
        }

        if (value.Length != expectedChars || !value.All(static character => character is (>= '0' and <= '9') or (>= 'a' and <= 'f')))
        {
            throw new ArgumentException(
                $"Value must be a {expectedChars}-character lowercase hex string.",
                paramName);
        }

        return value;
    }

    internal static string? RequireOptionalExactSizedHex(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireOptionalExactSizedHex(value, paramName, 32);
    }

    internal static string[] CopyRequiredExactTokenTextList(IReadOnlyList<string>? values, string paramName)
    {
        if (values is null)
        {
            return Array.Empty<string>();
        }

        var copy = new string[values.Count];
        for (var index = 0; index < values.Count; index++)
        {
            copy[index] = RequireExactTokenText(values[index], $"{paramName}[{index}]");
        }

        return copy;
    }

    internal static int[] CopyRequiredNonNegativeInt32List(IReadOnlyList<int>? values, string paramName)
    {
        if (values is null)
        {
            return Array.Empty<int>();
        }

        var copy = new int[values.Count];
        for (var index = 0; index < values.Count; index++)
        {
            copy[index] = RequireNonNegativeInt32(values[index], $"{paramName}[{index}]");
        }

        return copy;
    }
}
