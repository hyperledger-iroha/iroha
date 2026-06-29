using System.Text.Json;
using System.Text.Json.Serialization;
using Hyperledger.Iroha.Address;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiExplorerSnapshotJson
{
    internal static void ValidateExplorerDuration(ToriiExplorerDuration? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must not be null.");
        }
    }

    internal static void ValidateExplorerAccountQrSnapshot(
        ToriiExplorerAccountQrSnapshot response,
        string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        RequireCanonicalAccountId(response.CanonicalId, $"{context}.canonical_id");
        ToriiSseEventJson.RequireExactTokenText(response.Literal, $"{context}.literal");
        ValidateNonNegativeInt32(response.NetworkPrefix, $"{context}.network_prefix");
        ToriiSseEventJson.RequireExactTokenText(response.ErrorCorrection, $"{context}.error_correction");
        ValidatePositiveInt32(response.Modules, $"{context}.modules");
        ValidatePositiveInt32(response.QrVersion, $"{context}.qr_version");
        RequireExactNonEmptyText(response.Svg, $"{context}.svg");
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
            return AccountAddress.Parse(text, AccountAddress.DefaultChainDiscriminant)
                .ToI105(AccountAddress.DefaultChainDiscriminant);
        }
        catch (AccountAddressException exception)
        {
            throw new JsonException($"{field} must be a canonical I105 account id.", exception);
        }
    }

    internal static void ValidateExplorerHealthSnapshot(ToriiExplorerHealthSnapshot response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ToriiSseEventJson.RequireOptionalExactNonEmptyText(response.HeadCreatedAt, $"{context}.head_created_at");
        RequireExactNonEmptyText(response.SampledAt, $"{context}.sampled_at");
    }

    internal static void ValidateExplorerMetricsSnapshot(ToriiExplorerMetricsSnapshot response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ToriiSseEventJson.RequireOptionalExactNonEmptyText(response.BlockCreatedAt, $"{context}.block_created_at");
        if (response.AverageCommitTime is not null)
        {
            ValidateExplorerDuration(response.AverageCommitTime, $"{context}.avg_commit_time");
        }
        if (response.AverageBlockTime is not null)
        {
            ValidateExplorerDuration(response.AverageBlockTime, $"{context}.avg_block_time");
        }
    }

    internal static ToriiExplorerDuration ReadExplorerDuration(ref Utf8JsonReader reader, string context)
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
        ulong? milliseconds = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = new ToriiExplorerDuration
                {
                    Milliseconds = RequireUInt64(milliseconds, context, "ms"),
                };
                ValidateExplorerDuration(response, context);
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
                case "ms":
                    milliseconds = ReadUInt64(ref reader, $"{context}.ms");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiExplorerAccountQrSnapshot ReadExplorerAccountQrSnapshot(
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
        string? canonicalId = null;
        string? literal = null;
        int? networkPrefix = null;
        string? errorCorrection = null;
        int? modules = null;
        int? qrVersion = null;
        string? svg = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiExplorerAccountQrSnapshot
                    {
                        CanonicalId = RequireString(canonicalId, context, "canonical_id"),
                        Literal = RequireString(literal, context, "literal"),
                        NetworkPrefix = RequireInt32(networkPrefix, context, "network_prefix"),
                        ErrorCorrection = RequireString(errorCorrection, context, "error_correction"),
                        Modules = RequireInt32(modules, context, "modules"),
                        QrVersion = RequireInt32(qrVersion, context, "qr_version"),
                        Svg = RequireString(svg, context, "svg"),
                    };
                    ValidateExplorerAccountQrSnapshot(response, context);
                    return response;
                }
                catch (ArgumentException error) when (error.ParamName is not null)
                {
                    throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, context);
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
                case "canonical_id":
                    canonicalId = ReadOptionalString(ref reader, $"{context}.canonical_id");
                    break;
                case "literal":
                    literal = ReadOptionalString(ref reader, $"{context}.literal");
                    break;
                case "network_prefix":
                    networkPrefix = ReadInt32(ref reader, $"{context}.network_prefix");
                    break;
                case "error_correction":
                    errorCorrection = ReadOptionalString(ref reader, $"{context}.error_correction");
                    break;
                case "modules":
                    modules = ReadInt32(ref reader, $"{context}.modules");
                    break;
                case "qr_version":
                    qrVersion = ReadInt32(ref reader, $"{context}.qr_version");
                    break;
                case "svg":
                    svg = ReadOptionalString(ref reader, $"{context}.svg");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiExplorerHealthSnapshot ReadExplorerHealthSnapshot(
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
        ulong? headHeight = null;
        string? headCreatedAt = null;
        string? sampledAt = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiExplorerHealthSnapshot
                    {
                        HeadHeight = RequireUInt64(headHeight, context, "head_height"),
                        HeadCreatedAt = headCreatedAt,
                        SampledAt = RequireString(sampledAt, context, "sampled_at"),
                    };
                    ValidateExplorerHealthSnapshot(response, context);
                    return response;
                }
                catch (ArgumentException error) when (error.ParamName is not null)
                {
                    throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, context);
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
                case "head_height":
                    headHeight = ReadUInt64(ref reader, $"{context}.head_height");
                    break;
                case "head_created_at":
                    headCreatedAt = ReadOptionalString(ref reader, $"{context}.head_created_at");
                    break;
                case "sampled_at":
                    sampledAt = ReadOptionalString(ref reader, $"{context}.sampled_at");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiExplorerMetricsSnapshot ReadExplorerMetricsSnapshot(
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
        ulong? peers = null;
        ulong? domains = null;
        ulong? accounts = null;
        ulong? assets = null;
        ulong? transactionsAccepted = null;
        ulong? transactionsRejected = null;
        ulong? block = null;
        string? blockCreatedAt = null;
        ulong? finalizedBlock = null;
        ToriiExplorerDuration? averageCommitTime = null;
        ToriiExplorerDuration? averageBlockTime = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiExplorerMetricsSnapshot
                    {
                        Peers = RequireUInt64(peers, context, "peers"),
                        Domains = RequireUInt64(domains, context, "domains"),
                        Accounts = RequireUInt64(accounts, context, "accounts"),
                        Assets = RequireUInt64(assets, context, "assets"),
                        TransactionsAccepted = RequireUInt64(transactionsAccepted, context, "transactions_accepted"),
                        TransactionsRejected = RequireUInt64(transactionsRejected, context, "transactions_rejected"),
                        Block = RequireUInt64(block, context, "block"),
                        BlockCreatedAt = blockCreatedAt,
                        FinalizedBlock = RequireUInt64(finalizedBlock, context, "finalized_block"),
                        AverageCommitTime = averageCommitTime,
                        AverageBlockTime = averageBlockTime,
                    };
                    ValidateExplorerMetricsSnapshot(response, context);
                    return response;
                }
                catch (ArgumentException error) when (error.ParamName is not null)
                {
                    throw ToriiExplorerJson.DirectMetadataErrorToJsonException(error, context);
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
                case "peers":
                    peers = ReadUInt64(ref reader, $"{context}.peers");
                    break;
                case "domains":
                    domains = ReadUInt64(ref reader, $"{context}.domains");
                    break;
                case "accounts":
                    accounts = ReadUInt64(ref reader, $"{context}.accounts");
                    break;
                case "assets":
                    assets = ReadUInt64(ref reader, $"{context}.assets");
                    break;
                case "transactions_accepted":
                    transactionsAccepted = ReadUInt64(ref reader, $"{context}.transactions_accepted");
                    break;
                case "transactions_rejected":
                    transactionsRejected = ReadUInt64(ref reader, $"{context}.transactions_rejected");
                    break;
                case "block":
                    block = ReadUInt64(ref reader, $"{context}.block");
                    break;
                case "block_created_at":
                    blockCreatedAt = ReadOptionalString(ref reader, $"{context}.block_created_at");
                    break;
                case "finalized_block":
                    finalizedBlock = ReadUInt64(ref reader, $"{context}.finalized_block");
                    break;
                case "avg_commit_time":
                    averageCommitTime = ReadNullableExplorerDuration(ref reader, $"{context}.avg_commit_time");
                    break;
                case "avg_block_time":
                    averageBlockTime = ReadNullableExplorerDuration(ref reader, $"{context}.avg_block_time");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static void WriteExplorerDuration(
        Utf8JsonWriter writer,
        ToriiExplorerDuration response,
        string context)
    {
        ValidateExplorerDuration(response, context);

        writer.WriteStartObject();
        writer.WriteNumber("ms", response.Milliseconds);
        writer.WriteEndObject();
    }

    internal static void WriteExplorerAccountQrSnapshot(
        Utf8JsonWriter writer,
        ToriiExplorerAccountQrSnapshot response,
        string context)
    {
        ValidateExplorerAccountQrSnapshot(response, context);

        writer.WriteStartObject();
        writer.WriteString("canonical_id", response.CanonicalId);
        writer.WriteString("literal", response.Literal);
        writer.WriteNumber("network_prefix", response.NetworkPrefix);
        writer.WriteString("error_correction", response.ErrorCorrection);
        writer.WriteNumber("modules", response.Modules);
        writer.WriteNumber("qr_version", response.QrVersion);
        writer.WriteString("svg", response.Svg);
        writer.WriteEndObject();
    }

    internal static void WriteExplorerHealthSnapshot(
        Utf8JsonWriter writer,
        ToriiExplorerHealthSnapshot response,
        string context)
    {
        ValidateExplorerHealthSnapshot(response, context);

        writer.WriteStartObject();
        writer.WriteNumber("head_height", response.HeadHeight);
        ToriiVpnJson.WriteNullableString(writer, "head_created_at", response.HeadCreatedAt);
        writer.WriteString("sampled_at", response.SampledAt);
        writer.WriteEndObject();
    }

    internal static void WriteExplorerMetricsSnapshot(
        Utf8JsonWriter writer,
        ToriiExplorerMetricsSnapshot response,
        string context)
    {
        ValidateExplorerMetricsSnapshot(response, context);

        writer.WriteStartObject();
        writer.WriteNumber("peers", response.Peers);
        writer.WriteNumber("domains", response.Domains);
        writer.WriteNumber("accounts", response.Accounts);
        writer.WriteNumber("assets", response.Assets);
        writer.WriteNumber("transactions_accepted", response.TransactionsAccepted);
        writer.WriteNumber("transactions_rejected", response.TransactionsRejected);
        writer.WriteNumber("block", response.Block);
        ToriiVpnJson.WriteNullableString(writer, "block_created_at", response.BlockCreatedAt);
        writer.WriteNumber("finalized_block", response.FinalizedBlock);
        WriteNullableDuration(writer, "avg_commit_time", response.AverageCommitTime, $"{context}.avg_commit_time");
        WriteNullableDuration(writer, "avg_block_time", response.AverageBlockTime, $"{context}.avg_block_time");
        writer.WriteEndObject();
    }

    private static ToriiExplorerDuration? ReadNullableExplorerDuration(
        ref Utf8JsonReader reader,
        string context)
    {
        return reader.TokenType == JsonTokenType.Null
            ? null
            : ReadExplorerDuration(ref reader, context);
    }

    private static string? ReadOptionalString(ref Utf8JsonReader reader, string field)
    {
        return ToriiAccountFaucetJson.ReadOptionalString(ref reader, field);
    }

    private static int ReadInt32(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType != JsonTokenType.Number || !reader.TryGetInt32(out var value))
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

    private static int RequireInt32(int? value, string context, string propertyName)
    {
        if (!value.HasValue)
        {
            throw new JsonException($"{context}.{propertyName} must not be null.");
        }

        return value.Value;
    }

    private static ulong RequireUInt64(ulong? value, string context, string propertyName)
    {
        if (!value.HasValue)
        {
            throw new JsonException($"{context}.{propertyName} must not be null.");
        }

        return value.Value;
    }

    private static string RequireString(string? value, string context, string propertyName)
    {
        if (value is null)
        {
            throw new JsonException($"{context}.{propertyName} must not be null.");
        }

        return value;
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

    private static void ValidateNonNegativeInt32(int value, string field)
    {
        if (value < 0)
        {
            throw new JsonException($"{field} must be non-negative.");
        }
    }

    private static void ValidatePositiveInt32(int value, string field)
    {
        if (value <= 0)
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

    private static void WriteNullableDuration(
        Utf8JsonWriter writer,
        string propertyName,
        ToriiExplorerDuration? duration,
        string context)
    {
        writer.WritePropertyName(propertyName);
        if (duration is null)
        {
            writer.WriteNullValue();
        }
        else
        {
            WriteExplorerDuration(writer, duration, context);
        }
    }
}

internal sealed class ToriiExplorerDurationJsonConverter : JsonConverter<ToriiExplorerDuration>
{
    public override bool HandleNull => true;

    public override ToriiExplorerDuration Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiExplorerSnapshotJson.ReadExplorerDuration(ref reader, "explorer duration");
    }

    public override void Write(Utf8JsonWriter writer, ToriiExplorerDuration value, JsonSerializerOptions options)
    {
        ToriiExplorerSnapshotJson.WriteExplorerDuration(writer, value, "explorer duration");
    }
}

internal sealed class ToriiExplorerAccountQrSnapshotJsonConverter :
    JsonConverter<ToriiExplorerAccountQrSnapshot>
{
    public override bool HandleNull => true;

    public override ToriiExplorerAccountQrSnapshot Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiExplorerSnapshotJson.ReadExplorerAccountQrSnapshot(ref reader, "explorer account QR response");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerAccountQrSnapshot value,
        JsonSerializerOptions options)
    {
        ToriiExplorerSnapshotJson.WriteExplorerAccountQrSnapshot(writer, value, "explorer account QR response");
    }
}

internal sealed class ToriiExplorerHealthSnapshotJsonConverter : JsonConverter<ToriiExplorerHealthSnapshot>
{
    public override bool HandleNull => true;

    public override ToriiExplorerHealthSnapshot Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiExplorerSnapshotJson.ReadExplorerHealthSnapshot(ref reader, "explorer health response");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerHealthSnapshot value,
        JsonSerializerOptions options)
    {
        ToriiExplorerSnapshotJson.WriteExplorerHealthSnapshot(writer, value, "explorer health response");
    }
}

internal sealed class ToriiExplorerMetricsSnapshotJsonConverter : JsonConverter<ToriiExplorerMetricsSnapshot>
{
    public override bool HandleNull => true;

    public override ToriiExplorerMetricsSnapshot Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiExplorerSnapshotJson.ReadExplorerMetricsSnapshot(ref reader, "explorer metrics response");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiExplorerMetricsSnapshot value,
        JsonSerializerOptions options)
    {
        ToriiExplorerSnapshotJson.WriteExplorerMetricsSnapshot(writer, value, "explorer metrics response");
    }
}
