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

    private static string? ReadOptionalString(ref Utf8JsonReader reader, string field)
    {
        return ToriiAccountFaucetJson.ReadOptionalString(ref reader, field);
    }

    private static string ReadRequiredString(ref Utf8JsonReader reader, string field)
    {
        return ReadOptionalString(ref reader, field) ?? throw new JsonException($"{field} must be a non-empty string.");
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
