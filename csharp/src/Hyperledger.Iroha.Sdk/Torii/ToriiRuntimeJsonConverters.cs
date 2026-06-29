using System.Text.Json;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiRuntimeJson
{
    internal static void ValidateRuntimeAbiActive(ToriiRuntimeAbiActive response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ValidateAbiVersionV1(response.AbiVersion, $"{context}.abi_version");
    }

    internal static void ValidateRuntimeAbiHash(ToriiRuntimeAbiHash response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ToriiSseEventJson.RequireExactTokenText(response.Policy, $"{context}.policy");
        if (!string.Equals(response.Policy, "V1", StringComparison.Ordinal))
        {
            throw new JsonException($"{context}.policy must be V1.");
        }

        ToriiSseEventJson.RequireExactSizedHex(response.AbiHashHex, $"{context}.abi_hash_hex", 32);
    }

    internal static void ValidateRuntimeMetrics(ToriiRuntimeMetrics response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ValidateAbiVersionV1(response.AbiVersion, $"{context}.abi_version");
        if (response.UpgradeEventsTotal is null)
        {
            throw new JsonException($"{context}.upgrade_events_total must not be null.");
        }

        ValidateRuntimeUpgradeCounters(response.UpgradeEventsTotal, $"{context}.upgrade_events_total");
    }

    internal static void ValidateRuntimeUpgradeCounters(ToriiRuntimeUpgradeCounters response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ValidateNonNegativeInt64(response.Proposed, $"{context}.proposed");
        ValidateNonNegativeInt64(response.Activated, $"{context}.activated");
        ValidateNonNegativeInt64(response.Canceled, $"{context}.canceled");
        if (response.Activated > response.Proposed ||
            response.Canceled > response.Proposed - response.Activated)
        {
            throw new JsonException($"{context} activated plus canceled must be less than or equal to proposed.");
        }
    }

    internal static Dictionary<string, JsonElement> ReadObject(ref Utf8JsonReader reader, string context)
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
        var properties = new Dictionary<string, JsonElement>(StringComparer.Ordinal);
        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                return properties;
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

            using var document = JsonDocument.ParseValue(ref reader);
            ToriiIdentifierJson.RejectDuplicateProperties(document.RootElement, $"{context}.{propertyName}");
            properties[propertyName] = document.RootElement.Clone();
        }

        throw new JsonException($"{context} is truncated.");
    }

    internal static int ReadRequiredInt32(
        IReadOnlyDictionary<string, JsonElement> properties,
        string propertyName,
        string field)
    {
        if (!properties.TryGetValue(propertyName, out var value) || value.ValueKind == JsonValueKind.Null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        if (value.ValueKind != JsonValueKind.Number || !value.TryGetInt32(out var number))
        {
            throw new JsonException($"{field} must be an integer.");
        }

        return number;
    }

    internal static long ReadRequiredInt64(
        IReadOnlyDictionary<string, JsonElement> properties,
        string propertyName,
        string field)
    {
        if (!properties.TryGetValue(propertyName, out var value) || value.ValueKind == JsonValueKind.Null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        if (value.ValueKind != JsonValueKind.Number || !value.TryGetInt64(out var number))
        {
            throw new JsonException($"{field} must be an integer.");
        }

        return number;
    }

    internal static string ReadRequiredString(
        IReadOnlyDictionary<string, JsonElement> properties,
        string propertyName,
        string field)
    {
        if (!properties.TryGetValue(propertyName, out var value) || value.ValueKind == JsonValueKind.Null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        if (value.ValueKind != JsonValueKind.String)
        {
            throw new JsonException($"{field} must be a string.");
        }

        return value.GetString() ?? throw new JsonException($"{field} must not be null.");
    }

    internal static T ReadRequiredObject<T>(
        IReadOnlyDictionary<string, JsonElement> properties,
        string propertyName,
        string field,
        string nestedContext)
    {
        if (!properties.TryGetValue(propertyName, out var value) || value.ValueKind == JsonValueKind.Null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        if (value.ValueKind != JsonValueKind.Object)
        {
            throw new JsonException($"{field} must be an object.");
        }

        try
        {
            return value.Deserialize<T>() ?? throw new JsonException($"{field} must not be null.");
        }
        catch (JsonException exception)
        {
            throw RewriteContext(exception, nestedContext, field);
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

    private static void ValidateNonNegativeInt64(long value, string field)
    {
        if (value < 0)
        {
            throw new JsonException($"{field} must be non-negative.");
        }
    }

    private static JsonException RewriteContext(JsonException exception, string from, string to)
    {
        var message = exception.Message;
        if (message.StartsWith(from, StringComparison.Ordinal))
        {
            message = to + message[from.Length..];
        }

        return new JsonException(message, exception);
    }

    internal static JsonException DirectMetadataErrorToJsonException(ArgumentException error, string context)
    {
        var field = (error.ParamName ?? "metadata") switch
        {
            nameof(ToriiRuntimeUpgradeCounters.Proposed) => "proposed",
            nameof(ToriiRuntimeUpgradeCounters.Activated) => "activated",
            nameof(ToriiRuntimeUpgradeCounters.Canceled) => "canceled",
            nameof(ToriiRuntimeMetrics.AbiVersion) => "abi_version",
            nameof(ToriiRuntimeMetrics.UpgradeEventsTotal) => "upgrade_events_total",
            nameof(ToriiRuntimeAbiHash.Policy) => "policy",
            nameof(ToriiRuntimeAbiHash.AbiHashHex) => "abi_hash_hex",
            var paramName => paramName,
        };

        return new JsonException($"{context}.{field}: {error.Message}", error);
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
}

internal sealed class ToriiRuntimeUpgradeCountersJsonConverter : JsonConverter<ToriiRuntimeUpgradeCounters>
{
    public override bool HandleNull => true;

    public override ToriiRuntimeUpgradeCounters Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var properties = ToriiRuntimeJson.ReadObject(ref reader, "runtime upgrade counters");
        return ToriiRuntimeJson.CreateWithDirectMetadataContext(() =>
        {
            var response = new ToriiRuntimeUpgradeCounters
            {
                Proposed = ToriiRuntimeJson.ReadRequiredInt64(
                    properties,
                    "proposed",
                    "runtime upgrade counters.proposed"),
                Activated = ToriiRuntimeJson.ReadRequiredInt64(
                    properties,
                    "activated",
                    "runtime upgrade counters.activated"),
                Canceled = ToriiRuntimeJson.ReadRequiredInt64(
                    properties,
                    "canceled",
                    "runtime upgrade counters.canceled"),
            };
            ToriiRuntimeJson.ValidateRuntimeUpgradeCounters(response, "runtime upgrade counters");
            return response;
        }, "runtime upgrade counters");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiRuntimeUpgradeCounters value,
        JsonSerializerOptions options)
    {
        ToriiRuntimeJson.ValidateRuntimeUpgradeCounters(value, "runtime upgrade counters");

        writer.WriteStartObject();
        writer.WriteNumber("proposed", value.Proposed);
        writer.WriteNumber("activated", value.Activated);
        writer.WriteNumber("canceled", value.Canceled);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiRuntimeMetricsJsonConverter : JsonConverter<ToriiRuntimeMetrics>
{
    public override bool HandleNull => true;

    public override ToriiRuntimeMetrics Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var properties = ToriiRuntimeJson.ReadObject(ref reader, "runtime metrics");
        return ToriiRuntimeJson.CreateWithDirectMetadataContext(() =>
        {
            var response = new ToriiRuntimeMetrics
            {
                AbiVersion = ToriiRuntimeJson.ReadRequiredInt32(
                    properties,
                    "abi_version",
                    "runtime metrics.abi_version"),
                UpgradeEventsTotal = ToriiRuntimeJson.ReadRequiredObject<ToriiRuntimeUpgradeCounters>(
                    properties,
                    "upgrade_events_total",
                    "runtime metrics.upgrade_events_total",
                    "runtime upgrade counters"),
            };
            ToriiRuntimeJson.ValidateRuntimeMetrics(response, "runtime metrics");
            return response;
        }, "runtime metrics");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiRuntimeMetrics value,
        JsonSerializerOptions options)
    {
        ToriiRuntimeJson.ValidateRuntimeMetrics(value, "runtime metrics");

        writer.WriteStartObject();
        writer.WriteNumber("abi_version", value.AbiVersion);
        writer.WritePropertyName("upgrade_events_total");
        JsonSerializer.Serialize(writer, value.UpgradeEventsTotal, options);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiRuntimeAbiActiveJsonConverter : JsonConverter<ToriiRuntimeAbiActive>
{
    public override bool HandleNull => true;

    public override ToriiRuntimeAbiActive Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var properties = ToriiRuntimeJson.ReadObject(ref reader, "runtime ABI active");
        return ToriiRuntimeJson.CreateWithDirectMetadataContext(() =>
        {
            var response = new ToriiRuntimeAbiActive
            {
                AbiVersion = ToriiRuntimeJson.ReadRequiredInt32(
                    properties,
                    "abi_version",
                    "runtime ABI active.abi_version"),
            };
            ToriiRuntimeJson.ValidateRuntimeAbiActive(response, "runtime ABI active");
            return response;
        }, "runtime ABI active");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiRuntimeAbiActive value,
        JsonSerializerOptions options)
    {
        ToriiRuntimeJson.ValidateRuntimeAbiActive(value, "runtime ABI active");

        writer.WriteStartObject();
        writer.WriteNumber("abi_version", value.AbiVersion);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiRuntimeAbiHashJsonConverter : JsonConverter<ToriiRuntimeAbiHash>
{
    public override bool HandleNull => true;

    public override ToriiRuntimeAbiHash Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var properties = ToriiRuntimeJson.ReadObject(ref reader, "runtime ABI hash");
        return ToriiRuntimeJson.CreateWithDirectMetadataContext(() =>
        {
            var response = new ToriiRuntimeAbiHash
            {
                Policy = ToriiRuntimeJson.ReadRequiredString(
                    properties,
                    "policy",
                    "runtime ABI hash.policy"),
                AbiHashHex = ToriiRuntimeJson.ReadRequiredString(
                    properties,
                    "abi_hash_hex",
                    "runtime ABI hash.abi_hash_hex"),
            };
            ToriiRuntimeJson.ValidateRuntimeAbiHash(response, "runtime ABI hash");
            return response;
        }, "runtime ABI hash");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiRuntimeAbiHash value,
        JsonSerializerOptions options)
    {
        ToriiRuntimeJson.ValidateRuntimeAbiHash(value, "runtime ABI hash");

        writer.WriteStartObject();
        writer.WriteString("policy", value.Policy);
        writer.WriteString("abi_hash_hex", value.AbiHashHex);
        writer.WriteEndObject();
    }
}
