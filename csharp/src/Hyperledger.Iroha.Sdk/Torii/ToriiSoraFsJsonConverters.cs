using System.Text.Json;
using System.Text.Json.Serialization;
using Hyperledger.Iroha.Address;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiSoraFsJson
{
    internal static void ValidateCidLookupResponse(ToriiSoraFsCidLookupResponse response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ValidateContentCid(response.ContentCid, $"{context}.content_cid");
        ToriiSseEventJson.RequireExactSizedHex(response.ManifestDigestHex, $"{context}.manifest_digest_hex", 32);
        ToriiSseEventJson.RequireExactSizedHex(response.ManifestIdHex, $"{context}.manifest_id_hex", 32);
        RequireOptionalExactNonEmptyText(response.IndexDocument, $"{context}.index_document");
        ValidateItems(response.Files, $"{context}.files", ValidateFileEntry);
    }

    internal static void ValidateFileEntry(ToriiSoraFsFileEntry? file, string context)
    {
        if (file is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        if (file.Path is null)
        {
            throw new JsonException($"{context}.path must not be null.");
        }
        if (file.Path.Count == 0)
        {
            throw new JsonException($"{context}.path must not be empty.");
        }

        for (var index = 0; index < file.Path.Count; index++)
        {
            var field = $"{context}.path[{index}]";
            RequireExactNonEmptyText(file.Path[index], field);
            if (file.Path[index] == "." || file.Path[index] == ".." || file.Path[index].Contains('/', StringComparison.Ordinal))
            {
                throw new JsonException($"{field} must be a relative path component.");
            }
        }

        ValidateNonNegativeInt64(file.Offset, $"{context}.offset");
        ValidateNonNegativeInt64(file.Size, $"{context}.size");
        ValidateNonNegativeInt64(file.FirstChunk, $"{context}.first_chunk");
        ValidateNonNegativeInt64(file.ChunkCount, $"{context}.chunk_count");
    }

    internal static void ValidateDenylistCatalogResponse(
        ToriiSoraFsDenylistCatalogResponse response,
        string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ValidatePositiveInt64(response.Version, $"{context}.version");
        RequireOptionalExactNonEmptyText(response.Jurisdiction, $"{context}.jurisdiction");
        ValidateTextList(response.OptOutPacks, $"{context}.opt_out_packs");
        ValidateTextList(response.ExtraPacks, $"{context}.extra_packs");
        ValidateItems(response.Packs, $"{context}.packs", ValidateDenylistPackSummary);
    }

    internal static void ValidateDenylistPackSummary(ToriiSoraFsDenylistPackSummary? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        ValidateDenylistPackFields(
            response.PackId,
            response.Version,
            response.PolicyTier,
            response.ManifestCid,
            response.MerkleRoot,
            response.IssuedByProposalId,
            response.ReviewReference,
            response.Jurisdiction,
            response.IssuedAt,
            response.ExpiresAt,
            response.EntryCount,
            context);
    }

    internal static void ValidateDenylistPackResponse(ToriiSoraFsDenylistPackResponse response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ValidateDenylistPackFields(
            response.PackId,
            response.Version,
            response.PolicyTier,
            response.ManifestCid,
            response.MerkleRoot,
            response.IssuedByProposalId,
            response.ReviewReference,
            response.Jurisdiction,
            response.IssuedAt,
            response.ExpiresAt,
            response.EntryCount,
            context);
        RequireExactNonEmptyText(response.SourcePath, $"{context}.source_path");
    }

    internal static void ValidatePinAlias(ToriiSoraFsPinAlias? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireExactTokenText(response.Namespace, $"{context}.namespace");
        RequireExactTokenText(response.Name, $"{context}.name");
        ValidateCanonicalBase64(response.ProofBase64, $"{context}.proof_base64");
    }

    internal static void ValidatePinRegisterResponse(ToriiSoraFsPinRegisterResponse response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ToriiSseEventJson.RequireExactSizedHex(response.ManifestDigestHex, $"{context}.manifest_digest_hex", 32);
        RequireExactTokenText(response.ChunkerHandle, $"{context}.chunker_handle");
        RequireRequiredUInt64(response.SubmittedEpoch, $"{context}.submitted_epoch");
        RequireRequiredUInt64(response.ContentLength, $"{context}.content_length");
        RequireRequiredUInt64(response.PinFeeNano, $"{context}.pin_fee_nano");
        RequireExactTokenText(response.PinFeeAssetId, $"{context}.pin_fee_asset_id");
        RequireCanonicalAccountId(response.PinFeeTreasuryAccountId, $"{context}.pin_fee_treasury_account_id");
        if (response.Alias is not null)
        {
            ValidatePinAlias(response.Alias, $"{context}.alias");
        }
        ToriiSseEventJson.RequireOptionalExactSizedHex(response.SuccessorOfHex, $"{context}.successor_of_hex", 32);
    }

    internal static void ValidateChunkerHandle(ToriiSoraFsChunkerHandle? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireRequiredNonZeroUInt32(response.ProfileId, $"{context}.profile_id");
        RequireExactTokenText(response.Namespace, $"{context}.namespace");
        RequireExactTokenText(response.Name, $"{context}.name");
        RequireExactTokenText(response.Semver, $"{context}.semver");
    }

    internal static void ValidateStorageClass(ToriiSoraFsStorageClass? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        NormalizeStorageClassType(response.Type, $"{context}.type");
    }

    internal static void ValidatePinPolicy(ToriiSoraFsPinPolicy? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireRequiredNonZeroUInt32(response.MinReplicas, $"{context}.min_replicas");
        ValidateStorageClass(response.StorageClass, $"{context}.storage_class");
    }

    internal static ToriiSoraFsFileEntry ReadFileEntry(ref Utf8JsonReader reader, string context)
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
        List<string>? path = null;
        long? offset = null;
        long? size = null;
        long? firstChunk = null;
        long? chunkCount = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = CreateWithDirectMetadataContext(
                    () => new ToriiSoraFsFileEntry
                    {
                        Path = RequireList(path, context, "path"),
                        Offset = RequireInt64(offset, context, "offset"),
                        Size = RequireInt64(size, context, "size"),
                        FirstChunk = RequireInt64(firstChunk, context, "first_chunk"),
                        ChunkCount = RequireInt64(chunkCount, context, "chunk_count"),
                    },
                    context);
                ValidateFileEntry(response, context);
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
                case "path":
                    path = ReadStringList(ref reader, $"{context}.path");
                    break;
                case "offset":
                    offset = ReadInt64(ref reader, $"{context}.offset");
                    break;
                case "size":
                    size = ReadInt64(ref reader, $"{context}.size");
                    break;
                case "first_chunk":
                    firstChunk = ReadInt64(ref reader, $"{context}.first_chunk");
                    break;
                case "chunk_count":
                    chunkCount = ReadInt64(ref reader, $"{context}.chunk_count");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiSoraFsCidLookupResponse ReadCidLookupResponse(
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
        string? contentCid = null;
        string? manifestDigestHex = null;
        string? manifestIdHex = null;
        string? indexDocument = null;
        List<ToriiSoraFsFileEntry>? files = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = CreateWithDirectMetadataContext(
                    () => new ToriiSoraFsCidLookupResponse
                    {
                        ContentCid = RequireString(contentCid, context, "content_cid"),
                        ManifestDigestHex = RequireString(manifestDigestHex, context, "manifest_digest_hex"),
                        ManifestIdHex = RequireString(manifestIdHex, context, "manifest_id_hex"),
                        IndexDocument = indexDocument,
                        Files = RequireList(files, context, "files"),
                    },
                    context);
                ValidateCidLookupResponse(response, context);
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
                case "content_cid":
                    contentCid = ReadOptionalString(ref reader, $"{context}.content_cid");
                    break;
                case "manifest_digest_hex":
                    manifestDigestHex = ReadOptionalString(ref reader, $"{context}.manifest_digest_hex");
                    break;
                case "manifest_id_hex":
                    manifestIdHex = ReadOptionalString(ref reader, $"{context}.manifest_id_hex");
                    break;
                case "index_document":
                    indexDocument = ReadOptionalString(ref reader, $"{context}.index_document");
                    break;
                case "files":
                    files = ReadItems(ref reader, $"{context}.files", ReadFileEntry);
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiSoraFsDenylistPackSummary ReadDenylistPackSummary(
        ref Utf8JsonReader reader,
        string context)
    {
        var fields = ReadDenylistPackFields(ref reader, context);
        var response = CreateWithDirectMetadataContext(
            () => new ToriiSoraFsDenylistPackSummary
            {
                PackId = RequireString(fields.PackId, context, "pack_id"),
                Version = fields.Version,
                DefaultEnabled = RequireBool(fields.DefaultEnabled, context, "default_enabled"),
                Active = RequireBool(fields.Active, context, "active"),
                PolicyTier = fields.PolicyTier,
                ManifestCid = fields.ManifestCid,
                MerkleRoot = fields.MerkleRoot,
                IssuedByProposalId = fields.IssuedByProposalId,
                ReviewReference = fields.ReviewReference,
                Jurisdiction = fields.Jurisdiction,
                IssuedAt = fields.IssuedAt,
                ExpiresAt = fields.ExpiresAt,
                EntryCount = RequireInt64(fields.EntryCount, context, "entry_count"),
            },
            context);
        ValidateDenylistPackSummary(response, context);
        return response;
    }

    internal static ToriiSoraFsDenylistCatalogResponse ReadDenylistCatalogResponse(
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
        long? version = null;
        string? jurisdiction = null;
        List<string>? optOutPacks = null;
        List<string>? extraPacks = null;
        List<ToriiSoraFsDenylistPackSummary>? packs = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = CreateWithDirectMetadataContext(
                    () => new ToriiSoraFsDenylistCatalogResponse
                    {
                        Version = RequireInt64(version, context, "version"),
                        Jurisdiction = jurisdiction,
                        OptOutPacks = RequireList(optOutPacks, context, "opt_out_packs"),
                        ExtraPacks = RequireList(extraPacks, context, "extra_packs"),
                        Packs = RequireList(packs, context, "packs"),
                    },
                    context);
                ValidateDenylistCatalogResponse(response, context);
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
                case "version":
                    version = ReadInt64(ref reader, $"{context}.version");
                    break;
                case "jurisdiction":
                    jurisdiction = ReadOptionalString(ref reader, $"{context}.jurisdiction");
                    break;
                case "opt_out_packs":
                    optOutPacks = ReadStringList(ref reader, $"{context}.opt_out_packs");
                    break;
                case "extra_packs":
                    extraPacks = ReadStringList(ref reader, $"{context}.extra_packs");
                    break;
                case "packs":
                    packs = ReadItems(ref reader, $"{context}.packs", ReadDenylistPackSummary);
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiSoraFsDenylistPackResponse ReadDenylistPackResponse(
        ref Utf8JsonReader reader,
        string context)
    {
        var fields = ReadDenylistPackFields(ref reader, context);
        var response = CreateWithDirectMetadataContext(
            () => new ToriiSoraFsDenylistPackResponse
            {
                PackId = RequireString(fields.PackId, context, "pack_id"),
                Version = fields.Version,
                DefaultEnabled = RequireBool(fields.DefaultEnabled, context, "default_enabled"),
                Active = RequireBool(fields.Active, context, "active"),
                PolicyTier = fields.PolicyTier,
                ManifestCid = fields.ManifestCid,
                MerkleRoot = fields.MerkleRoot,
                IssuedByProposalId = fields.IssuedByProposalId,
                ReviewReference = fields.ReviewReference,
                Jurisdiction = fields.Jurisdiction,
                IssuedAt = fields.IssuedAt,
                ExpiresAt = fields.ExpiresAt,
                EntryCount = RequireInt64(fields.EntryCount, context, "entry_count"),
                SourcePath = RequireString(fields.SourcePath, context, "source_path"),
            },
            context);
        ValidateDenylistPackResponse(response, context);
        return response;
    }

    internal static ToriiSoraFsChunkerHandle ReadChunkerHandle(ref Utf8JsonReader reader, string context)
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
        uint? profileId = null;
        string? namespaceValue = null;
        string? name = null;
        string? semver = null;
        uint? multihashCode = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = new ToriiSoraFsChunkerHandle
                {
                    ProfileId = profileId,
                    Namespace = namespaceValue,
                    Name = name,
                    Semver = semver,
                    MultihashCode = multihashCode ?? 0,
                };
                ValidateChunkerHandle(response, context);
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
                case "profile_id":
                    profileId = ReadNullableUInt32(ref reader, $"{context}.profile_id");
                    break;
                case "namespace":
                    namespaceValue = ReadOptionalString(ref reader, $"{context}.namespace");
                    break;
                case "name":
                    name = ReadOptionalString(ref reader, $"{context}.name");
                    break;
                case "semver":
                    semver = ReadOptionalString(ref reader, $"{context}.semver");
                    break;
                case "multihash_code":
                    multihashCode = ReadNullableUInt32(ref reader, $"{context}.multihash_code");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiSoraFsStorageClass ReadStorageClass(ref Utf8JsonReader reader, string context)
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
        string? type = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                return new ToriiSoraFsStorageClass
                {
                    Type = NormalizeStorageClassType(type, $"{context}.type"),
                };
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
                case "type":
                    type = ReadOptionalString(ref reader, $"{context}.type");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiSoraFsPinPolicy ReadPinPolicy(ref Utf8JsonReader reader, string context)
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
        uint? minReplicas = null;
        ToriiSoraFsStorageClass? storageClass = null;
        ulong? retentionEpoch = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = new ToriiSoraFsPinPolicy
                {
                    MinReplicas = minReplicas,
                    StorageClass = storageClass,
                    RetentionEpoch = retentionEpoch ?? 0,
                };
                ValidatePinPolicy(response, context);
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
                case "min_replicas":
                    minReplicas = ReadNullableUInt32(ref reader, $"{context}.min_replicas");
                    break;
                case "storage_class":
                    storageClass = reader.TokenType == JsonTokenType.Null
                        ? null
                        : ReadStorageClass(ref reader, $"{context}.storage_class");
                    break;
                case "retention_epoch":
                    retentionEpoch = ReadNullableUInt64(ref reader, $"{context}.retention_epoch");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiSoraFsPinAlias ReadPinAlias(ref Utf8JsonReader reader, string context)
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
        string? namespaceValue = null;
        string? name = null;
        string? proofBase64 = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = new ToriiSoraFsPinAlias
                {
                    Namespace = namespaceValue,
                    Name = name,
                    ProofBase64 = proofBase64,
                };
                ValidatePinAlias(response, context);
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
                case "namespace":
                    namespaceValue = ReadOptionalString(ref reader, $"{context}.namespace");
                    break;
                case "name":
                    name = ReadOptionalString(ref reader, $"{context}.name");
                    break;
                case "proof_base64":
                    proofBase64 = ReadOptionalString(ref reader, $"{context}.proof_base64");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiSoraFsPinRegisterResponse ReadPinRegisterResponse(
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
        string? manifestDigestHex = null;
        string? chunkerHandle = null;
        ulong? submittedEpoch = null;
        ulong? contentLength = null;
        ulong? pinFeeNano = null;
        string? pinFeeAssetId = null;
        string? pinFeeTreasuryAccountId = null;
        ToriiSoraFsPinAlias? alias = null;
        string? successorOfHex = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = CreateWithDirectMetadataContext(
                    () => new ToriiSoraFsPinRegisterResponse
                    {
                        ManifestDigestHex = manifestDigestHex,
                        ChunkerHandle = chunkerHandle,
                        SubmittedEpoch = submittedEpoch,
                        ContentLength = contentLength,
                        PinFeeNano = pinFeeNano,
                        PinFeeAssetId = pinFeeAssetId,
                        PinFeeTreasuryAccountId = pinFeeTreasuryAccountId,
                        Alias = alias,
                        SuccessorOfHex = successorOfHex,
                    },
                    context);
                ValidatePinRegisterResponse(response, context);
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
                case "manifest_digest_hex":
                    manifestDigestHex = ReadOptionalString(ref reader, $"{context}.manifest_digest_hex");
                    break;
                case "chunker_handle":
                    chunkerHandle = ReadOptionalString(ref reader, $"{context}.chunker_handle");
                    break;
                case "submitted_epoch":
                    submittedEpoch = ReadNullableUInt64(ref reader, $"{context}.submitted_epoch");
                    break;
                case "content_length":
                    contentLength = ReadNullableUInt64(ref reader, $"{context}.content_length");
                    break;
                case "pin_fee_nano":
                    pinFeeNano = ReadNullableUInt64(ref reader, $"{context}.pin_fee_nano");
                    break;
                case "pin_fee_asset_id":
                    pinFeeAssetId = ReadOptionalString(ref reader, $"{context}.pin_fee_asset_id");
                    break;
                case "pin_fee_treasury_account_id":
                    pinFeeTreasuryAccountId = ReadOptionalString(ref reader, $"{context}.pin_fee_treasury_account_id");
                    break;
                case "alias":
                    alias = reader.TokenType == JsonTokenType.Null ? null : ReadPinAlias(ref reader, $"{context}.alias");
                    break;
                case "successor_of_hex":
                    successorOfHex = ReadOptionalString(ref reader, $"{context}.successor_of_hex");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static void WriteFileEntry(Utf8JsonWriter writer, ToriiSoraFsFileEntry response, string context)
    {
        ValidateFileEntry(response, context);

        writer.WriteStartObject();
        WriteStringList(writer, "path", response.Path);
        writer.WriteNumber("offset", response.Offset);
        writer.WriteNumber("size", response.Size);
        writer.WriteNumber("first_chunk", response.FirstChunk);
        writer.WriteNumber("chunk_count", response.ChunkCount);
        writer.WriteEndObject();
    }

    internal static void WriteCidLookupResponse(
        Utf8JsonWriter writer,
        ToriiSoraFsCidLookupResponse response,
        string context)
    {
        ValidateCidLookupResponse(response, context);

        writer.WriteStartObject();
        writer.WriteString("content_cid", response.ContentCid);
        writer.WriteString("manifest_digest_hex", response.ManifestDigestHex);
        writer.WriteString("manifest_id_hex", response.ManifestIdHex);
        ToriiVpnJson.WriteNullableString(writer, "index_document", response.IndexDocument);
        writer.WritePropertyName("files");
        writer.WriteStartArray();
        for (var index = 0; index < response.Files.Count; index++)
        {
            WriteFileEntry(writer, response.Files[index], $"{context}.files[{index}]");
        }
        writer.WriteEndArray();
        writer.WriteEndObject();
    }

    internal static void WriteDenylistPackSummary(
        Utf8JsonWriter writer,
        ToriiSoraFsDenylistPackSummary response,
        string context)
    {
        ValidateDenylistPackSummary(response, context);
        WriteDenylistPackFields(writer, new DenylistPackFields(
            response.PackId,
            response.Version,
            response.DefaultEnabled,
            response.Active,
            response.PolicyTier,
            response.ManifestCid,
            response.MerkleRoot,
            response.IssuedByProposalId,
            response.ReviewReference,
            response.Jurisdiction,
            response.IssuedAt,
            response.ExpiresAt,
            response.EntryCount,
            sourcePath: null));
    }

    internal static void WriteDenylistCatalogResponse(
        Utf8JsonWriter writer,
        ToriiSoraFsDenylistCatalogResponse response,
        string context)
    {
        ValidateDenylistCatalogResponse(response, context);

        writer.WriteStartObject();
        writer.WriteNumber("version", response.Version);
        ToriiVpnJson.WriteNullableString(writer, "jurisdiction", response.Jurisdiction);
        WriteStringList(writer, "opt_out_packs", response.OptOutPacks);
        WriteStringList(writer, "extra_packs", response.ExtraPacks);
        writer.WritePropertyName("packs");
        writer.WriteStartArray();
        for (var index = 0; index < response.Packs.Count; index++)
        {
            WriteDenylistPackSummary(writer, response.Packs[index], $"{context}.packs[{index}]");
        }
        writer.WriteEndArray();
        writer.WriteEndObject();
    }

    internal static void WriteDenylistPackResponse(
        Utf8JsonWriter writer,
        ToriiSoraFsDenylistPackResponse response,
        string context)
    {
        ValidateDenylistPackResponse(response, context);
        WriteDenylistPackFields(writer, new DenylistPackFields(
            response.PackId,
            response.Version,
            response.DefaultEnabled,
            response.Active,
            response.PolicyTier,
            response.ManifestCid,
            response.MerkleRoot,
            response.IssuedByProposalId,
            response.ReviewReference,
            response.Jurisdiction,
            response.IssuedAt,
            response.ExpiresAt,
            response.EntryCount,
            response.SourcePath));
    }

    internal static void WriteChunkerHandle(Utf8JsonWriter writer, ToriiSoraFsChunkerHandle response, string context)
    {
        ValidateChunkerHandle(response, context);

        writer.WriteStartObject();
        writer.WriteNumber("profile_id", RequireValue(response.ProfileId, $"{context}.profile_id"));
        writer.WriteString("namespace", response.Namespace);
        writer.WriteString("name", response.Name);
        writer.WriteString("semver", response.Semver);
        writer.WriteNumber("multihash_code", response.MultihashCode ?? 0);
        writer.WriteEndObject();
    }

    internal static void WriteStorageClass(Utf8JsonWriter writer, ToriiSoraFsStorageClass response, string context)
    {
        var type = NormalizeStorageClassType(response.Type, $"{context}.type");

        writer.WriteStartObject();
        writer.WriteString("type", type);
        writer.WriteEndObject();
    }

    internal static void WritePinPolicy(Utf8JsonWriter writer, ToriiSoraFsPinPolicy response, string context)
    {
        ValidatePinPolicy(response, context);

        writer.WriteStartObject();
        writer.WriteNumber("min_replicas", RequireValue(response.MinReplicas, $"{context}.min_replicas"));
        writer.WritePropertyName("storage_class");
        WriteStorageClass(
            writer,
            RequireObject(response.StorageClass, $"{context}.storage_class"),
            $"{context}.storage_class");
        writer.WriteNumber("retention_epoch", response.RetentionEpoch ?? 0);
        writer.WriteEndObject();
    }

    internal static void WritePinAlias(Utf8JsonWriter writer, ToriiSoraFsPinAlias response, string context)
    {
        ValidatePinAlias(response, context);

        writer.WriteStartObject();
        writer.WriteString("namespace", response.Namespace);
        writer.WriteString("name", response.Name);
        writer.WriteString("proof_base64", response.ProofBase64);
        writer.WriteEndObject();
    }

    internal static void WritePinRegisterResponse(
        Utf8JsonWriter writer,
        ToriiSoraFsPinRegisterResponse response,
        string context)
    {
        ValidatePinRegisterResponse(response, context);

        writer.WriteStartObject();
        writer.WriteString("manifest_digest_hex", response.ManifestDigestHex);
        writer.WriteString("chunker_handle", response.ChunkerHandle);
        writer.WriteNumber("submitted_epoch", RequireValue(response.SubmittedEpoch, $"{context}.submitted_epoch"));
        writer.WriteNumber("content_length", RequireValue(response.ContentLength, $"{context}.content_length"));
        writer.WriteNumber("pin_fee_nano", RequireValue(response.PinFeeNano, $"{context}.pin_fee_nano"));
        writer.WriteString("pin_fee_asset_id", response.PinFeeAssetId);
        writer.WriteString("pin_fee_treasury_account_id", response.PinFeeTreasuryAccountId);
        writer.WritePropertyName("alias");
        if (response.Alias is null)
        {
            writer.WriteNullValue();
        }
        else
        {
            WritePinAlias(writer, response.Alias, $"{context}.alias");
        }
        ToriiVpnJson.WriteNullableString(writer, "successor_of_hex", response.SuccessorOfHex);
        writer.WriteEndObject();
    }

    private static T RequireValue<T>(T? value, string field)
        where T : struct
    {
        return value ?? throw new JsonException($"{field} must not be null.");
    }

    private static T RequireObject<T>(T? value, string field)
        where T : class
    {
        return value ?? throw new JsonException($"{field} must not be null.");
    }

    private delegate T ReadItem<T>(ref Utf8JsonReader reader, string context);

    private static void ValidateDenylistPackFields(
        string? packId,
        string? version,
        string? policyTier,
        string? manifestCid,
        string? merkleRoot,
        string? issuedByProposalId,
        string? reviewReference,
        string? jurisdiction,
        string? issuedAt,
        string? expiresAt,
        long entryCount,
        string context)
    {
        RequireExactNonEmptyText(packId, $"{context}.pack_id");
        RequireOptionalExactNonEmptyText(version, $"{context}.version");
        RequireOptionalExactNonEmptyText(policyTier, $"{context}.policy_tier");
        if (manifestCid is not null)
        {
            ValidateContentCid(manifestCid, $"{context}.manifest_cid");
        }
        RequireOptionalExactNonEmptyText(merkleRoot, $"{context}.merkle_root");
        RequireOptionalExactNonEmptyText(issuedByProposalId, $"{context}.issued_by_proposal_id");
        RequireOptionalExactNonEmptyText(reviewReference, $"{context}.review_reference");
        RequireOptionalExactNonEmptyText(jurisdiction, $"{context}.jurisdiction");
        RequireOptionalExactNonEmptyText(issuedAt, $"{context}.issued_at");
        RequireOptionalExactNonEmptyText(expiresAt, $"{context}.expires_at");
        ValidateNonNegativeInt64(entryCount, $"{context}.entry_count");
    }

    private static DenylistPackFields ReadDenylistPackFields(ref Utf8JsonReader reader, string context)
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
        var fields = new DenylistPackFields();

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                return fields;
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
                case "pack_id":
                    fields.PackId = ReadOptionalString(ref reader, $"{context}.pack_id");
                    break;
                case "version":
                    fields.Version = ReadOptionalString(ref reader, $"{context}.version");
                    break;
                case "default_enabled":
                    fields.DefaultEnabled = ReadBool(ref reader, $"{context}.default_enabled");
                    break;
                case "active":
                    fields.Active = ReadBool(ref reader, $"{context}.active");
                    break;
                case "policy_tier":
                    fields.PolicyTier = ReadOptionalString(ref reader, $"{context}.policy_tier");
                    break;
                case "manifest_cid":
                    fields.ManifestCid = ReadOptionalString(ref reader, $"{context}.manifest_cid");
                    break;
                case "merkle_root":
                    fields.MerkleRoot = ReadOptionalString(ref reader, $"{context}.merkle_root");
                    break;
                case "issued_by_proposal_id":
                    fields.IssuedByProposalId = ReadOptionalString(ref reader, $"{context}.issued_by_proposal_id");
                    break;
                case "review_reference":
                    fields.ReviewReference = ReadOptionalString(ref reader, $"{context}.review_reference");
                    break;
                case "jurisdiction":
                    fields.Jurisdiction = ReadOptionalString(ref reader, $"{context}.jurisdiction");
                    break;
                case "issued_at":
                    fields.IssuedAt = ReadOptionalString(ref reader, $"{context}.issued_at");
                    break;
                case "expires_at":
                    fields.ExpiresAt = ReadOptionalString(ref reader, $"{context}.expires_at");
                    break;
                case "entry_count":
                    fields.EntryCount = ReadInt64(ref reader, $"{context}.entry_count");
                    break;
                case "source_path":
                    fields.SourcePath = ReadOptionalString(ref reader, $"{context}.source_path");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    private static List<T>? ReadItems<T>(ref Utf8JsonReader reader, string context, ReadItem<T> readItem)
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

    private static void ValidateItems<T>(IReadOnlyList<T>? items, string context, Action<T?, string> validateItem)
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

            values.Add(ReadRequiredString(ref reader, $"{context}[{index}]"));
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

    private static string? ReadOptionalString(ref Utf8JsonReader reader, string field)
    {
        return ToriiAccountFaucetJson.ReadOptionalString(ref reader, field);
    }

    private static string ReadRequiredString(ref Utf8JsonReader reader, string field)
    {
        return ReadOptionalString(ref reader, field) ?? throw new JsonException($"{field} must be a non-empty string.");
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

    private static long ReadInt64(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType != JsonTokenType.Number || !reader.TryGetInt64(out var value))
        {
            throw new JsonException($"{field} must be an integer.");
        }

        return value;
    }

    private static long RequireInt64(long? value, string context, string propertyName)
    {
        if (!value.HasValue)
        {
            throw new JsonException($"{context}.{propertyName} must not be null.");
        }

        return value.Value;
    }

    private static bool RequireBool(bool? value, string context, string propertyName)
    {
        if (!value.HasValue)
        {
            throw new JsonException($"{context}.{propertyName} must not be null.");
        }

        return value.Value;
    }

    private static string RequireString(string? value, string context, string propertyName)
    {
        return value ?? throw new JsonException($"{context}.{propertyName} must not be null.");
    }

    private static IReadOnlyList<T> RequireList<T>(IReadOnlyList<T>? values, string context, string propertyName)
    {
        return values ?? throw new JsonException($"{context}.{propertyName} must not be null.");
    }

    private static uint? ReadNullableUInt32(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            return null;
        }

        if (reader.TokenType != JsonTokenType.Number || !reader.TryGetUInt32(out var value))
        {
            throw new JsonException($"{field} must be an unsigned integer.");
        }

        return value;
    }

    private static ulong? ReadNullableUInt64(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            return null;
        }

        if (reader.TokenType != JsonTokenType.Number || !reader.TryGetUInt64(out var value))
        {
            throw new JsonException($"{field} must be an unsigned integer.");
        }

        return value;
    }

    private static void ValidateContentCid(string? value, string field)
    {
        var text = RequireExactNonEmptyText(value, field);
        if (text.Any(char.IsWhiteSpace))
        {
            throw new JsonException($"{field} must not contain whitespace.");
        }

        if (text[0] != 'b' || text.Length == 1)
        {
            throw new JsonException($"{field} must be lowercase multibase base32 CID text.");
        }

        for (var index = 1; index < text.Length; index++)
        {
            var character = text[index];
            if (character is not (>= 'a' and <= 'z') and not (>= '2' and <= '7'))
            {
                throw new JsonException($"{field} must be lowercase multibase base32 CID text.");
            }
        }
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

    private static string RequireExactTokenText(string? value, string field)
    {
        var text = RequireExactNonEmptyText(value, field);
        if (text.Any(char.IsWhiteSpace))
        {
            throw new JsonException($"{field} must not contain whitespace.");
        }

        return text;
    }

    private static string RequireCanonicalAccountId(string? value, string field)
    {
        if (value is null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        try
        {
            return AccountAddress.Parse(value, AccountAddress.DefaultChainDiscriminant)
                .ToI105(AccountAddress.DefaultChainDiscriminant);
        }
        catch (AccountAddressException exception)
        {
            throw new JsonException($"{field} must be a canonical I105 account id.", exception);
        }
    }

    private static void RequireOptionalExactNonEmptyText(string? value, string field)
    {
        if (value is not null)
        {
            RequireExactNonEmptyText(value, field);
        }
    }

    private static ulong RequireRequiredUInt64(ulong? value, string field)
    {
        return value ?? throw new JsonException($"{field} must not be null.");
    }

    private static uint RequireRequiredNonZeroUInt32(uint? value, string field)
    {
        var number = value ?? throw new JsonException($"{field} must not be null.");
        if (number == 0)
        {
            throw new JsonException($"{field} must be greater than zero.");
        }

        return number;
    }

    private static string NormalizeStorageClassType(string? value, string field)
    {
        var text = RequireExactTokenText(value, field);
        return text.ToLowerInvariant() switch
        {
            "hot" => "Hot",
            "warm" => "Warm",
            "cold" => "Cold",
            _ => throw new JsonException($"{field} must be Hot, Warm, or Cold."),
        };
    }

    private static void ValidateCanonicalBase64(string? value, string field)
    {
        var text = RequireExactTokenText(value, field);
        byte[] bytes;
        try
        {
            bytes = Convert.FromBase64String(text);
        }
        catch (FormatException error)
        {
            throw new JsonException($"{field} must be base64 encoded.", error);
        }

        if (bytes.Length == 0)
        {
            throw new JsonException($"{field} must be a non-empty base64 payload.");
        }

        if (!string.Equals(Convert.ToBase64String(bytes), text, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must be canonical base64 text.");
        }
    }

    private static void ValidateNonNegativeInt64(long value, string field)
    {
        if (value < 0)
        {
            throw new JsonException($"{field} must be non-negative.");
        }
    }

    private static void ValidatePositiveInt64(long value, string field)
    {
        ValidateNonNegativeInt64(value, field);
        if (value == 0)
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
            _ when TryMapCollectionField(paramName, nameof(ToriiSoraFsFileEntry.Path), "path", out var mapped) => mapped,
            _ when TryMapCollectionField(paramName, nameof(ToriiSoraFsCidLookupResponse.Files), "files", out var mapped) => mapped,
            _ when TryMapCollectionField(paramName, nameof(ToriiSoraFsDenylistCatalogResponse.OptOutPacks), "opt_out_packs", out var mapped) => mapped,
            _ when TryMapCollectionField(paramName, nameof(ToriiSoraFsDenylistCatalogResponse.ExtraPacks), "extra_packs", out var mapped) => mapped,
            _ when TryMapCollectionField(paramName, nameof(ToriiSoraFsDenylistCatalogResponse.Packs), "packs", out var mapped) => mapped,
            _ when TryMapNestedField(paramName, nameof(ToriiSoraFsPinRegisterResponse.Alias), "alias", out var mapped) => mapped,
            nameof(ToriiSoraFsFileEntry.Path) => "path",
            nameof(ToriiSoraFsFileEntry.Offset) => "offset",
            nameof(ToriiSoraFsFileEntry.Size) => "size",
            nameof(ToriiSoraFsFileEntry.FirstChunk) => "first_chunk",
            nameof(ToriiSoraFsFileEntry.ChunkCount) => "chunk_count",
            nameof(ToriiSoraFsCidLookupResponse.ContentCid) => "content_cid",
            nameof(ToriiSoraFsCidLookupResponse.ManifestDigestHex) => "manifest_digest_hex",
            nameof(ToriiSoraFsCidLookupResponse.ManifestIdHex) => "manifest_id_hex",
            nameof(ToriiSoraFsCidLookupResponse.IndexDocument) => "index_document",
            nameof(ToriiSoraFsDenylistPackSummary.PackId) => "pack_id",
            nameof(ToriiSoraFsDenylistPackSummary.Version) => "version",
            nameof(ToriiSoraFsDenylistPackSummary.PolicyTier) => "policy_tier",
            nameof(ToriiSoraFsDenylistPackSummary.ManifestCid) => "manifest_cid",
            nameof(ToriiSoraFsDenylistPackSummary.MerkleRoot) => "merkle_root",
            nameof(ToriiSoraFsDenylistPackSummary.IssuedByProposalId) => "issued_by_proposal_id",
            nameof(ToriiSoraFsDenylistPackSummary.ReviewReference) => "review_reference",
            nameof(ToriiSoraFsDenylistPackSummary.Jurisdiction) => "jurisdiction",
            nameof(ToriiSoraFsDenylistPackSummary.IssuedAt) => "issued_at",
            nameof(ToriiSoraFsDenylistPackSummary.ExpiresAt) => "expires_at",
            nameof(ToriiSoraFsDenylistPackSummary.EntryCount) => "entry_count",
            nameof(ToriiSoraFsDenylistPackResponse.SourcePath) => "source_path",
            nameof(ToriiSoraFsPinRegisterResponse.ChunkerHandle) => "chunker_handle",
            nameof(ToriiSoraFsPinRegisterResponse.SubmittedEpoch) => "submitted_epoch",
            nameof(ToriiSoraFsPinRegisterResponse.ContentLength) => "content_length",
            nameof(ToriiSoraFsPinRegisterResponse.PinFeeNano) => "pin_fee_nano",
            nameof(ToriiSoraFsPinRegisterResponse.PinFeeAssetId) => "pin_fee_asset_id",
            nameof(ToriiSoraFsPinRegisterResponse.PinFeeTreasuryAccountId) => "pin_fee_treasury_account_id",
            nameof(ToriiSoraFsPinRegisterResponse.SuccessorOfHex) => "successor_of_hex",
            nameof(ToriiSoraFsPinAlias.Namespace) => "namespace",
            nameof(ToriiSoraFsPinAlias.Name) => "name",
            nameof(ToriiSoraFsPinAlias.ProofBase64) => "proof_base64",
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

    private static void WriteDenylistPackFields(Utf8JsonWriter writer, DenylistPackFields fields)
    {
        writer.WriteStartObject();
        writer.WriteString("pack_id", fields.PackId);
        ToriiVpnJson.WriteNullableString(writer, "version", fields.Version);
        writer.WriteBoolean("default_enabled", fields.DefaultEnabled.GetValueOrDefault());
        writer.WriteBoolean("active", fields.Active.GetValueOrDefault());
        ToriiVpnJson.WriteNullableString(writer, "policy_tier", fields.PolicyTier);
        ToriiVpnJson.WriteNullableString(writer, "manifest_cid", fields.ManifestCid);
        ToriiVpnJson.WriteNullableString(writer, "merkle_root", fields.MerkleRoot);
        ToriiVpnJson.WriteNullableString(writer, "issued_by_proposal_id", fields.IssuedByProposalId);
        ToriiVpnJson.WriteNullableString(writer, "review_reference", fields.ReviewReference);
        ToriiVpnJson.WriteNullableString(writer, "jurisdiction", fields.Jurisdiction);
        ToriiVpnJson.WriteNullableString(writer, "issued_at", fields.IssuedAt);
        ToriiVpnJson.WriteNullableString(writer, "expires_at", fields.ExpiresAt);
        writer.WriteNumber("entry_count", fields.EntryCount.GetValueOrDefault());
        if (fields.SourcePath is not null)
        {
            writer.WriteString("source_path", fields.SourcePath);
        }
        writer.WriteEndObject();
    }

    private sealed class DenylistPackFields
    {
        public DenylistPackFields()
        {
        }

        public DenylistPackFields(
            string? packId,
            string? version,
            bool defaultEnabled,
            bool active,
            string? policyTier,
            string? manifestCid,
            string? merkleRoot,
            string? issuedByProposalId,
            string? reviewReference,
            string? jurisdiction,
            string? issuedAt,
            string? expiresAt,
            long entryCount,
            string? sourcePath)
        {
            PackId = packId;
            Version = version;
            DefaultEnabled = defaultEnabled;
            Active = active;
            PolicyTier = policyTier;
            ManifestCid = manifestCid;
            MerkleRoot = merkleRoot;
            IssuedByProposalId = issuedByProposalId;
            ReviewReference = reviewReference;
            Jurisdiction = jurisdiction;
            IssuedAt = issuedAt;
            ExpiresAt = expiresAt;
            EntryCount = entryCount;
            SourcePath = sourcePath;
        }

        public string? PackId { get; set; }

        public string? Version { get; set; }

        public bool? DefaultEnabled { get; set; }

        public bool? Active { get; set; }

        public string? PolicyTier { get; set; }

        public string? ManifestCid { get; set; }

        public string? MerkleRoot { get; set; }

        public string? IssuedByProposalId { get; set; }

        public string? ReviewReference { get; set; }

        public string? Jurisdiction { get; set; }

        public string? IssuedAt { get; set; }

        public string? ExpiresAt { get; set; }

        public long? EntryCount { get; set; }

        public string? SourcePath { get; set; }
    }
}

internal sealed class ToriiSoraFsFileEntryJsonConverter : JsonConverter<ToriiSoraFsFileEntry>
{
    public override bool HandleNull => true;

    public override ToriiSoraFsFileEntry Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiSoraFsJson.ReadFileEntry(ref reader, "SoraFS file entry");
    }

    public override void Write(Utf8JsonWriter writer, ToriiSoraFsFileEntry value, JsonSerializerOptions options)
    {
        ToriiSoraFsJson.WriteFileEntry(writer, value, "SoraFS file entry");
    }
}

internal sealed class ToriiSoraFsCidLookupResponseJsonConverter : JsonConverter<ToriiSoraFsCidLookupResponse>
{
    public override bool HandleNull => true;

    public override ToriiSoraFsCidLookupResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiSoraFsJson.ReadCidLookupResponse(ref reader, "SoraFS CID lookup response");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiSoraFsCidLookupResponse value,
        JsonSerializerOptions options)
    {
        ToriiSoraFsJson.WriteCidLookupResponse(writer, value, "SoraFS CID lookup response");
    }
}

internal sealed class ToriiSoraFsChunkerHandleJsonConverter : JsonConverter<ToriiSoraFsChunkerHandle>
{
    public override bool HandleNull => true;

    public override ToriiSoraFsChunkerHandle Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiSoraFsJson.ReadChunkerHandle(ref reader, "SoraFS chunker handle");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiSoraFsChunkerHandle value,
        JsonSerializerOptions options)
    {
        ToriiSoraFsJson.WriteChunkerHandle(writer, value, "SoraFS chunker handle");
    }
}

internal sealed class ToriiSoraFsStorageClassJsonConverter : JsonConverter<ToriiSoraFsStorageClass>
{
    public override bool HandleNull => true;

    public override ToriiSoraFsStorageClass Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiSoraFsJson.ReadStorageClass(ref reader, "SoraFS storage class");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiSoraFsStorageClass value,
        JsonSerializerOptions options)
    {
        ToriiSoraFsJson.WriteStorageClass(writer, value, "SoraFS storage class");
    }
}

internal sealed class ToriiSoraFsPinPolicyJsonConverter : JsonConverter<ToriiSoraFsPinPolicy>
{
    public override bool HandleNull => true;

    public override ToriiSoraFsPinPolicy Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiSoraFsJson.ReadPinPolicy(ref reader, "SoraFS pin policy");
    }

    public override void Write(Utf8JsonWriter writer, ToriiSoraFsPinPolicy value, JsonSerializerOptions options)
    {
        ToriiSoraFsJson.WritePinPolicy(writer, value, "SoraFS pin policy");
    }
}

internal sealed class ToriiSoraFsPinAliasJsonConverter : JsonConverter<ToriiSoraFsPinAlias>
{
    public override bool HandleNull => true;

    public override ToriiSoraFsPinAlias Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiSoraFsJson.ReadPinAlias(ref reader, "SoraFS pin alias");
    }

    public override void Write(Utf8JsonWriter writer, ToriiSoraFsPinAlias value, JsonSerializerOptions options)
    {
        ToriiSoraFsJson.WritePinAlias(writer, value, "SoraFS pin alias");
    }
}

internal sealed class ToriiSoraFsPinRegisterResponseJsonConverter :
    JsonConverter<ToriiSoraFsPinRegisterResponse>
{
    public override bool HandleNull => true;

    public override ToriiSoraFsPinRegisterResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiSoraFsJson.ReadPinRegisterResponse(ref reader, "SoraFS pin register response");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiSoraFsPinRegisterResponse value,
        JsonSerializerOptions options)
    {
        ToriiSoraFsJson.WritePinRegisterResponse(writer, value, "SoraFS pin register response");
    }
}

internal sealed class ToriiSoraFsDenylistPackSummaryJsonConverter :
    JsonConverter<ToriiSoraFsDenylistPackSummary>
{
    public override bool HandleNull => true;

    public override ToriiSoraFsDenylistPackSummary Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiSoraFsJson.ReadDenylistPackSummary(ref reader, "SoraFS denylist pack summary");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiSoraFsDenylistPackSummary value,
        JsonSerializerOptions options)
    {
        ToriiSoraFsJson.WriteDenylistPackSummary(writer, value, "SoraFS denylist pack summary");
    }
}

internal sealed class ToriiSoraFsDenylistCatalogResponseJsonConverter :
    JsonConverter<ToriiSoraFsDenylistCatalogResponse>
{
    public override bool HandleNull => true;

    public override ToriiSoraFsDenylistCatalogResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiSoraFsJson.ReadDenylistCatalogResponse(ref reader, "SoraFS denylist catalog response");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiSoraFsDenylistCatalogResponse value,
        JsonSerializerOptions options)
    {
        ToriiSoraFsJson.WriteDenylistCatalogResponse(writer, value, "SoraFS denylist catalog response");
    }
}

internal sealed class ToriiSoraFsDenylistPackResponseJsonConverter :
    JsonConverter<ToriiSoraFsDenylistPackResponse>
{
    public override bool HandleNull => true;

    public override ToriiSoraFsDenylistPackResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiSoraFsJson.ReadDenylistPackResponse(ref reader, "SoraFS denylist pack response");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiSoraFsDenylistPackResponse value,
        JsonSerializerOptions options)
    {
        ToriiSoraFsJson.WriteDenylistPackResponse(writer, value, "SoraFS denylist pack response");
    }
}
