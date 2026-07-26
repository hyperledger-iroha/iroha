using System.Text.Json;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiSseEventJson
{
    internal static void ValidatePipelineEvent(ToriiPipelineEvent response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        if (!string.Equals(response.Category, "Pipeline", StringComparison.Ordinal))
        {
            throw new JsonException($"{context}.category must be Pipeline.");
        }

        RequireExactTokenText(response.Event, $"{context}.event");
        RequireOptionalExactTokenText(response.Status, $"{context}.status");
        RequireOptionalExactSizedHex(response.Hash, $"{context}.hash", 32);
        RequireOptionalExactTokenText(response.Kind, $"{context}.kind");
        RequireOptionalExactNonEmptyText(response.Details, $"{context}.details");
        RequireOptionalExactSizedHex(response.GlobalStateRoot, $"{context}.global_state_root", 32);
        RequireOptionalExactSizedHex(response.BlockHash, $"{context}.block_hash", 32);
        ValidateSseProjectionMetadata(response.LastEventId, response.SseEventName, context);
    }

    internal static void ValidateProofEvent(ToriiProofEvent response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        if (!string.Equals(response.Category, "Data", StringComparison.Ordinal))
        {
            throw new JsonException($"{context}.category must be Data.");
        }

        RequireExactTokenText(response.Event, $"{context}.event");
        if (!response.Event.StartsWith("Proof", StringComparison.Ordinal))
        {
            throw new JsonException($"{context}.event must be a proof event.");
        }

        RequireExactTokenText(response.Backend, $"{context}.backend");
        RequireOptionalExactSizedHex(response.ProofHash, $"{context}.proof_hash", 32);
        RequireOptionalExactSizedHex(response.CallHash, $"{context}.call_hash", 32);
        RequireOptionalExactSizedHex(response.EnvelopeHash, $"{context}.envelope_hash", 32);
        RequireOptionalExactTokenText(response.VerificationKeyReference, $"{context}.vk_ref");
        RequireOptionalExactSizedHex(response.VerificationKeyCommitment, $"{context}.vk_commitment", 32);
        RequireOptionalExactTokenText(response.PrunedBy, $"{context}.pruned_by");
        RequireOptionalExactTokenText(response.Origin, $"{context}.origin");

        if (response.Cap.HasValue && response.Remaining.HasValue && response.Remaining.Value > response.Cap.Value)
        {
            throw new JsonException($"{context}.remaining must be less than or equal to cap.");
        }

        if (response.RemovedCount.HasValue && response.Removed is null)
        {
            throw new JsonException($"{context}.removed must not be null when removed_count is present.");
        }

        if (response.Removed is not null)
        {
            if (response.RemovedCount.HasValue && response.RemovedCount.Value != (ulong)response.Removed.Count)
            {
                throw new JsonException($"{context}.removed_count must match removed item count.");
            }

            for (var index = 0; index < response.Removed.Count; index++)
            {
                ValidateProofRemovedRecord(response.Removed[index], $"{context}.removed[{index}]");
            }
        }

        ValidateSseProjectionMetadata(response.LastEventId, response.SseEventName, context);
    }

    internal static void ValidateProofRemovedRecord(ToriiProofRemovedRecord? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        RequireExactTokenText(response.Backend, $"{context}.backend");
        RequireExactSizedHex(response.ProofHash, $"{context}.proof_hash", 32);
    }

    internal static string RequireExactTokenText(string? value, string field)
    {
        var exact = RequireExactNonEmptyText(value, field);
        if (ContainsWhitespace(exact))
        {
            throw new JsonException($"{field} must not contain whitespace.");
        }

        return exact;
    }

    internal static string? RequireOptionalExactTokenText(string? value, string field)
    {
        return value is null ? null : RequireExactTokenText(value, field);
    }

    internal static string? RequireOptionalExactNonEmptyText(string? value, string field)
    {
        return value is null ? null : RequireExactNonEmptyText(value, field);
    }

    internal static string RequireExactSizedHex(string? value, string field, int expectedBytes)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new JsonException($"{field} must be a non-empty {expectedBytes}-byte hex string.");
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must not contain surrounding whitespace.");
        }

        if (ContainsWhitespace(value))
        {
            throw new JsonException($"{field} must not contain whitespace.");
        }

        if (ContainsControlCharacter(value))
        {
            throw new JsonException($"{field} must not contain control characters.");
        }

        if (value.Length != expectedBytes * 2 || !IsLowercaseHex(value))
        {
            throw new JsonException($"{field} must be an exact lowercase {expectedBytes}-byte hex string.");
        }

        return value;
    }

    internal static string? RequireOptionalExactSizedHex(string? value, string field, int expectedBytes)
    {
        return value is null ? null : RequireExactSizedHex(value, field, expectedBytes);
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

    private static void ValidateSseProjectionMetadata(string? lastEventId, string? sseEventName, string context)
    {
        RequireOptionalExactNonEmptyText(lastEventId, $"{context}.last_event_id");
        RequireOptionalExactNonEmptyText(sseEventName, $"{context}.sse_event_name");
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
}

internal sealed class ToriiPipelineEventJsonConverter : JsonConverter<ToriiPipelineEvent>
{
    public override bool HandleNull => true;

    public override ToriiPipelineEvent Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            throw new JsonException("pipeline event must not be null.");
        }

        if (reader.TokenType != JsonTokenType.StartObject)
        {
            throw new JsonException("pipeline event must be an object.");
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);
        string? category = null;
        string? eventName = null;
        string? status = null;
        string? hash = null;
        ulong? laneId = null;
        ulong? dataspaceId = null;
        ulong? blockHeight = null;
        string? kind = null;
        string? details = null;
        ulong? height = null;
        ulong? epochId = null;
        string? globalStateRoot = null;
        string? blockHash = null;
        ulong? view = null;
        ulong? epoch = null;
        ulong? readCount = null;
        ulong? writeCount = null;
        Dictionary<string, JsonElement>? additionalProperties = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiPipelineEvent
                    {
                        Category = RequireRequiredString(category, "pipeline event.category"),
                        Event = RequireRequiredString(eventName, "pipeline event.event"),
                        Status = status,
                        Hash = hash,
                        LaneId = laneId,
                        DataspaceId = dataspaceId,
                        BlockHeight = blockHeight,
                        Kind = kind,
                        Details = details,
                        Height = height,
                        EpochId = epochId,
                        GlobalStateRoot = globalStateRoot,
                        BlockHash = blockHash,
                        View = view,
                        Epoch = epoch,
                        ReadCount = readCount,
                        WriteCount = writeCount,
                        AdditionalProperties = additionalProperties,
                    };
                    ToriiSseEventJson.ValidatePipelineEvent(response, "pipeline event");
                    return response;
                }
                catch (ArgumentException error) when (error.ParamName is not null)
                {
                    throw DirectMetadataErrorToJsonException(error, "pipeline event");
                }
            }

            if (reader.TokenType != JsonTokenType.PropertyName)
            {
                throw new JsonException("pipeline event property name expected.");
            }

            var propertyName = reader.GetString() ?? throw new JsonException("pipeline event property name must be a string.");
            ToriiIdentifierJson.RequireUniqueProperty(seen, propertyName, "pipeline event");
            if (!reader.Read())
            {
                throw new JsonException($"pipeline event.{propertyName} is truncated.");
            }

            switch (propertyName)
            {
                case "category":
                    category = ReadOptionalString(ref reader, "pipeline event.category");
                    break;
                case "event":
                    eventName = ReadOptionalString(ref reader, "pipeline event.event");
                    break;
                case "status":
                    status = ReadOptionalString(ref reader, "pipeline event.status");
                    break;
                case "hash":
                    hash = ReadOptionalString(ref reader, "pipeline event.hash");
                    break;
                case "lane_id":
                    laneId = ReadOptionalUInt64(ref reader, "pipeline event.lane_id");
                    break;
                case "dataspace_id":
                    dataspaceId = ReadOptionalUInt64(ref reader, "pipeline event.dataspace_id");
                    break;
                case "block_height":
                    blockHeight = ReadOptionalUInt64(ref reader, "pipeline event.block_height");
                    break;
                case "kind":
                    kind = ReadOptionalString(ref reader, "pipeline event.kind");
                    break;
                case "details":
                    details = ReadOptionalString(ref reader, "pipeline event.details");
                    break;
                case "height":
                    height = ReadOptionalUInt64(ref reader, "pipeline event.height");
                    break;
                case "epoch_id":
                    epochId = ReadOptionalUInt64(ref reader, "pipeline event.epoch_id");
                    break;
                case "global_state_root":
                    globalStateRoot = ReadOptionalString(ref reader, "pipeline event.global_state_root");
                    break;
                case "block_hash":
                    blockHash = ReadOptionalString(ref reader, "pipeline event.block_hash");
                    break;
                case "view":
                    view = ReadOptionalUInt64(ref reader, "pipeline event.view");
                    break;
                case "epoch":
                    epoch = ReadOptionalUInt64(ref reader, "pipeline event.epoch");
                    break;
                case "read_count":
                    readCount = ReadOptionalUInt64(ref reader, "pipeline event.read_count");
                    break;
                case "write_count":
                    writeCount = ReadOptionalUInt64(ref reader, "pipeline event.write_count");
                    break;
                default:
                    additionalProperties ??= new Dictionary<string, JsonElement>(StringComparer.Ordinal);
                    additionalProperties[propertyName] = ReadExtensionProperty(
                        ref reader,
                        $"pipeline event.{propertyName}");
                    break;
            }
        }

        throw new JsonException("pipeline event is truncated.");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiPipelineEvent value,
        JsonSerializerOptions options)
    {
        ToriiSseEventJson.ValidatePipelineEvent(value, "pipeline event");

        writer.WriteStartObject();
        writer.WriteString("category", value.Category);
        writer.WriteString("event", value.Event);
        WriteNullableString(writer, "status", value.Status);
        WriteNullableString(writer, "hash", value.Hash);
        WriteNullableUInt64(writer, "lane_id", value.LaneId);
        WriteNullableUInt64(writer, "dataspace_id", value.DataspaceId);
        WriteNullableUInt64(writer, "block_height", value.BlockHeight);
        WriteNullableString(writer, "kind", value.Kind);
        WriteNullableString(writer, "details", value.Details);
        WriteNullableUInt64(writer, "height", value.Height);
        WriteNullableUInt64(writer, "epoch_id", value.EpochId);
        WriteNullableString(writer, "global_state_root", value.GlobalStateRoot);
        WriteNullableString(writer, "block_hash", value.BlockHash);
        WriteNullableUInt64(writer, "view", value.View);
        WriteNullableUInt64(writer, "epoch", value.Epoch);
        WriteNullableUInt64(writer, "read_count", value.ReadCount);
        WriteNullableUInt64(writer, "write_count", value.WriteCount);
        WriteExtensionProperties(writer, value.AdditionalProperties);
        writer.WriteEndObject();
    }

    internal static string? ReadOptionalString(ref Utf8JsonReader reader, string field)
    {
        return reader.TokenType switch
        {
            JsonTokenType.Null => null,
            JsonTokenType.String => reader.GetString(),
            _ => throw new JsonException($"{field} must be a string."),
        };
    }

    internal static string RequireRequiredString(string? value, string field)
    {
        if (value is null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        return value;
    }

    internal static ulong? ReadOptionalUInt64(ref Utf8JsonReader reader, string field)
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

    internal static JsonElement ReadExtensionProperty(ref Utf8JsonReader reader, string field)
    {
        using var document = JsonDocument.ParseValue(ref reader);
        ToriiIdentifierJson.RejectDuplicateProperties(document.RootElement, field);
        return document.RootElement.Clone();
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

    internal static void WriteNullableUInt64(Utf8JsonWriter writer, string propertyName, ulong? value)
    {
        if (value is null)
        {
            writer.WriteNull(propertyName);
            return;
        }

        writer.WriteNumber(propertyName, value.Value);
    }

    internal static void WriteExtensionProperties(
        Utf8JsonWriter writer,
        IReadOnlyDictionary<string, JsonElement>? additionalProperties)
    {
        if (additionalProperties is null)
        {
            return;
        }

        foreach (var (propertyName, value) in additionalProperties)
        {
            writer.WritePropertyName(propertyName);
            value.WriteTo(writer);
        }
    }

    internal static JsonException DirectMetadataErrorToJsonException(ArgumentException error, string context)
    {
        var field = error.ParamName switch
        {
            "Category" => "category",
            "Event" => "event",
            "Status" => "status",
            "Hash" => "hash",
            "Kind" => "kind",
            "Details" => "details",
            "GlobalStateRoot" => "global_state_root",
            "BlockHash" => "block_hash",
            "Backend" => "backend",
            "ProofHash" => "proof_hash",
            "CallHash" => "call_hash",
            "EnvelopeHash" => "envelope_hash",
            "VerificationKeyReference" => "vk_ref",
            "VerificationKeyCommitment" => "vk_commitment",
            "PrunedBy" => "pruned_by",
            "Origin" => "origin",
            "LastEventId" => "last_event_id",
            "SseEventName" => "sse_event_name",
            "RetryMilliseconds" => "retry",
            _ => error.ParamName,
        };
        return new JsonException($"{context}.{field}: {error.Message}", error);
    }
}

internal sealed class ToriiProofEventJsonConverter : JsonConverter<ToriiProofEvent>
{
    public override bool HandleNull => true;

    public override ToriiProofEvent Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            throw new JsonException("proof event must not be null.");
        }

        if (reader.TokenType != JsonTokenType.StartObject)
        {
            throw new JsonException("proof event must be an object.");
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);
        string? category = null;
        string? eventName = null;
        string? backend = null;
        string? proofHash = null;
        string? callHash = null;
        string? envelopeHash = null;
        string? verificationKeyReference = null;
        string? verificationKeyCommitment = null;
        ulong? removedCount = null;
        ulong? remaining = null;
        ulong? cap = null;
        ulong? graceBlocks = null;
        ulong? pruneBatch = null;
        ulong? prunedAtHeight = null;
        string? prunedBy = null;
        string? origin = null;
        List<ToriiProofRemovedRecord>? removed = null;
        Dictionary<string, JsonElement>? additionalProperties = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiProofEvent
                    {
                        Category = ToriiPipelineEventJsonConverter.RequireRequiredString(category, "proof event.category"),
                        Event = ToriiPipelineEventJsonConverter.RequireRequiredString(eventName, "proof event.event"),
                        Backend = ToriiPipelineEventJsonConverter.RequireRequiredString(backend, "proof event.backend"),
                        ProofHash = proofHash,
                        CallHash = callHash,
                        EnvelopeHash = envelopeHash,
                        VerificationKeyReference = verificationKeyReference,
                        VerificationKeyCommitment = verificationKeyCommitment,
                        RemovedCount = removedCount,
                        Remaining = remaining,
                        Cap = cap,
                        GraceBlocks = graceBlocks,
                        PruneBatch = pruneBatch,
                        PrunedAtHeight = prunedAtHeight,
                        PrunedBy = prunedBy,
                        Origin = origin,
                        Removed = removed,
                        AdditionalProperties = additionalProperties,
                    };
                    ToriiSseEventJson.ValidateProofEvent(response, "proof event");
                    return response;
                }
                catch (ArgumentException error) when (error.ParamName is not null)
                {
                    throw ToriiPipelineEventJsonConverter.DirectMetadataErrorToJsonException(error, "proof event");
                }
            }

            if (reader.TokenType != JsonTokenType.PropertyName)
            {
                throw new JsonException("proof event property name expected.");
            }

            var propertyName = reader.GetString() ?? throw new JsonException("proof event property name must be a string.");
            ToriiIdentifierJson.RequireUniqueProperty(seen, propertyName, "proof event");
            if (!reader.Read())
            {
                throw new JsonException($"proof event.{propertyName} is truncated.");
            }

            switch (propertyName)
            {
                case "category":
                    category = ToriiPipelineEventJsonConverter.ReadOptionalString(ref reader, "proof event.category");
                    break;
                case "event":
                    eventName = ToriiPipelineEventJsonConverter.ReadOptionalString(ref reader, "proof event.event");
                    break;
                case "backend":
                    backend = ToriiPipelineEventJsonConverter.ReadOptionalString(ref reader, "proof event.backend");
                    break;
                case "proof_hash":
                    proofHash = ToriiPipelineEventJsonConverter.ReadOptionalString(ref reader, "proof event.proof_hash");
                    break;
                case "call_hash":
                    callHash = ToriiPipelineEventJsonConverter.ReadOptionalString(ref reader, "proof event.call_hash");
                    break;
                case "envelope_hash":
                    envelopeHash = ToriiPipelineEventJsonConverter.ReadOptionalString(ref reader, "proof event.envelope_hash");
                    break;
                case "vk_ref":
                    verificationKeyReference = ToriiPipelineEventJsonConverter.ReadOptionalString(ref reader, "proof event.vk_ref");
                    break;
                case "vk_commitment":
                    verificationKeyCommitment = ToriiPipelineEventJsonConverter.ReadOptionalString(ref reader, "proof event.vk_commitment");
                    break;
                case "removed_count":
                    removedCount = ToriiPipelineEventJsonConverter.ReadOptionalUInt64(ref reader, "proof event.removed_count");
                    break;
                case "remaining":
                    remaining = ToriiPipelineEventJsonConverter.ReadOptionalUInt64(ref reader, "proof event.remaining");
                    break;
                case "cap":
                    cap = ToriiPipelineEventJsonConverter.ReadOptionalUInt64(ref reader, "proof event.cap");
                    break;
                case "grace_blocks":
                    graceBlocks = ToriiPipelineEventJsonConverter.ReadOptionalUInt64(ref reader, "proof event.grace_blocks");
                    break;
                case "prune_batch":
                    pruneBatch = ToriiPipelineEventJsonConverter.ReadOptionalUInt64(ref reader, "proof event.prune_batch");
                    break;
                case "pruned_at_height":
                    prunedAtHeight = ToriiPipelineEventJsonConverter.ReadOptionalUInt64(ref reader, "proof event.pruned_at_height");
                    break;
                case "pruned_by":
                    prunedBy = ToriiPipelineEventJsonConverter.ReadOptionalString(ref reader, "proof event.pruned_by");
                    break;
                case "origin":
                    origin = ToriiPipelineEventJsonConverter.ReadOptionalString(ref reader, "proof event.origin");
                    break;
                case "removed":
                    removed = ReadRemovedRecords(ref reader, "proof event.removed");
                    break;
                default:
                    additionalProperties ??= new Dictionary<string, JsonElement>(StringComparer.Ordinal);
                    additionalProperties[propertyName] = ToriiPipelineEventJsonConverter.ReadExtensionProperty(
                        ref reader,
                        $"proof event.{propertyName}");
                    break;
            }
        }

        throw new JsonException("proof event is truncated.");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiProofEvent value,
        JsonSerializerOptions options)
    {
        ToriiSseEventJson.ValidateProofEvent(value, "proof event");

        writer.WriteStartObject();
        writer.WriteString("category", value.Category);
        writer.WriteString("event", value.Event);
        ToriiPipelineEventJsonConverter.WriteNullableString(writer, "backend", value.Backend);
        ToriiPipelineEventJsonConverter.WriteNullableString(writer, "proof_hash", value.ProofHash);
        ToriiPipelineEventJsonConverter.WriteNullableString(writer, "call_hash", value.CallHash);
        ToriiPipelineEventJsonConverter.WriteNullableString(writer, "envelope_hash", value.EnvelopeHash);
        ToriiPipelineEventJsonConverter.WriteNullableString(writer, "vk_ref", value.VerificationKeyReference);
        ToriiPipelineEventJsonConverter.WriteNullableString(writer, "vk_commitment", value.VerificationKeyCommitment);
        ToriiPipelineEventJsonConverter.WriteNullableUInt64(writer, "removed_count", value.RemovedCount);
        ToriiPipelineEventJsonConverter.WriteNullableUInt64(writer, "remaining", value.Remaining);
        ToriiPipelineEventJsonConverter.WriteNullableUInt64(writer, "cap", value.Cap);
        ToriiPipelineEventJsonConverter.WriteNullableUInt64(writer, "grace_blocks", value.GraceBlocks);
        ToriiPipelineEventJsonConverter.WriteNullableUInt64(writer, "prune_batch", value.PruneBatch);
        ToriiPipelineEventJsonConverter.WriteNullableUInt64(writer, "pruned_at_height", value.PrunedAtHeight);
        ToriiPipelineEventJsonConverter.WriteNullableString(writer, "pruned_by", value.PrunedBy);
        ToriiPipelineEventJsonConverter.WriteNullableString(writer, "origin", value.Origin);
        WriteRemovedRecords(writer, value.Removed, options);
        ToriiPipelineEventJsonConverter.WriteExtensionProperties(writer, value.AdditionalProperties);
        writer.WriteEndObject();
    }

    internal static List<ToriiProofRemovedRecord>? ReadRemovedRecords(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            return null;
        }

        if (reader.TokenType != JsonTokenType.StartArray)
        {
            throw new JsonException($"{field} must be an array.");
        }

        var records = new List<ToriiProofRemovedRecord>();
        var index = 0;
        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndArray)
            {
                return records;
            }

            records.Add(ToriiProofRemovedRecordJsonConverter.ReadRecord(ref reader, $"{field}[{index}]"));
            index++;
        }

        throw new JsonException($"{field} is truncated.");
    }

    private static void WriteRemovedRecords(
        Utf8JsonWriter writer,
        IReadOnlyList<ToriiProofRemovedRecord>? records,
        JsonSerializerOptions options)
    {
        writer.WritePropertyName("removed");
        if (records is null)
        {
            writer.WriteNullValue();
            return;
        }

        writer.WriteStartArray();
        foreach (var record in records)
        {
            JsonSerializer.Serialize(writer, record, options);
        }

        writer.WriteEndArray();
    }
}

internal sealed class ToriiProofRemovedRecordJsonConverter : JsonConverter<ToriiProofRemovedRecord>
{
    public override bool HandleNull => true;

    public override ToriiProofRemovedRecord Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ReadRecord(ref reader, "proof removed record");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiProofRemovedRecord value,
        JsonSerializerOptions options)
    {
        ToriiSseEventJson.ValidateProofRemovedRecord(value, "proof removed record");

        writer.WriteStartObject();
        writer.WriteString("backend", value.Backend);
        writer.WriteString("proof_hash", value.ProofHash);
        writer.WriteEndObject();
    }

    internal static ToriiProofRemovedRecord ReadRecord(ref Utf8JsonReader reader, string context)
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
        string? backend = null;
        string? proofHash = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var record = new ToriiProofRemovedRecord
                    {
                        Backend = ToriiPipelineEventJsonConverter.RequireRequiredString(backend, $"{context}.backend"),
                        ProofHash = ToriiPipelineEventJsonConverter.RequireRequiredString(proofHash, $"{context}.proof_hash"),
                    };
                    ToriiSseEventJson.ValidateProofRemovedRecord(record, context);
                    return record;
                }
                catch (ArgumentException error) when (error.ParamName is not null)
                {
                    throw ToriiPipelineEventJsonConverter.DirectMetadataErrorToJsonException(error, context);
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
                case "backend":
                    backend = ToriiPipelineEventJsonConverter.ReadOptionalString(ref reader, $"{context}.backend");
                    break;
                case "proof_hash":
                    proofHash = ToriiPipelineEventJsonConverter.ReadOptionalString(ref reader, $"{context}.proof_hash");
                    break;
                default:
                    ToriiPipelineEventJsonConverter.ReadExtensionProperty(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} is truncated.");
    }
}
