using System.Text.Json;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiContractInstancesJson
{
    internal static void ValidateContractInstance(ToriiContractInstance? response, string context)
    {
        if (response is null)
        {
            throw new JsonException($"{context} must be an object.");
        }

        ToriiSseEventJson.RequireExactTokenText(response.ContractId, $"{context}.contract_id");
        ToriiSseEventJson.RequireExactSizedHex(response.CodeHashHex, $"{context}.code_hash_hex", 32);
    }

    internal static void ValidateContractInstancesResponse(ToriiContractInstancesResponse response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ToriiSseEventJson.RequireExactTokenText(response.Namespace, $"{context}.namespace");

        if (response.Instances is null)
        {
            throw new JsonException($"{context}.instances is required.");
        }

        if ((ulong)response.Instances.Count > response.Limit)
        {
            throw new JsonException($"{context}.instances item count must be less than or equal to limit.");
        }

        if (response.Offset > response.Total)
        {
            throw new JsonException($"{context}.offset must be less than or equal to total.");
        }

        if ((ulong)response.Instances.Count > response.Total - response.Offset)
        {
            throw new JsonException($"{context}.offset plus item count must be less than or equal to total.");
        }

        for (var index = 0; index < response.Instances.Count; index++)
        {
            ValidateContractInstance(response.Instances[index], $"{context}.instances[{index}]");
        }
    }

    internal static void WriteContractInstance(
        Utf8JsonWriter writer,
        ToriiContractInstance response,
        string context)
    {
        ValidateContractInstance(response, context);

        writer.WriteStartObject();
        writer.WriteString("contract_id", response.ContractId);
        writer.WriteString("code_hash_hex", response.CodeHashHex);
        writer.WriteEndObject();
    }

    internal static void WriteContractInstancesResponse(
        Utf8JsonWriter writer,
        ToriiContractInstancesResponse response,
        string context)
    {
        ValidateContractInstancesResponse(response, context);

        writer.WriteStartObject();
        writer.WriteString("namespace", response.Namespace);
        writer.WritePropertyName("instances");
        writer.WriteStartArray();
        for (var index = 0; index < response.Instances.Count; index++)
        {
            WriteContractInstance(writer, response.Instances[index], $"{context}.instances[{index}]");
        }

        writer.WriteEndArray();
        writer.WriteNumber("total", response.Total);
        writer.WriteNumber("offset", response.Offset);
        writer.WriteNumber("limit", response.Limit);
        writer.WriteEndObject();
    }

    internal static ToriiContractInstance ReadContractInstance(
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
        string? contractId = null;
        string? codeHashHex = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiContractInstance
                    {
                        ContractId = RequireString(contractId, $"{context}.contract_id"),
                        CodeHashHex = RequireString(codeHashHex, $"{context}.code_hash_hex"),
                    };
                    ValidateContractInstance(response, context);
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
                case "contract_id":
                    contractId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.contract_id");
                    break;
                case "code_hash_hex":
                    codeHashHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.code_hash_hex");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiContractInstancesResponse ReadContractInstancesResponse(
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
        string? @namespace = null;
        List<ToriiContractInstance>? instances = null;
        ulong? total = null;
        ulong? offset = null;
        ulong? limit = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiContractInstancesResponse
                    {
                        Namespace = RequireString(@namespace, $"{context}.namespace"),
                        Instances = RequireInstances(instances, context),
                        Total = RequireCounter(total, context, "total"),
                        Offset = RequireCounter(offset, context, "offset"),
                        Limit = RequireCounter(limit, context, "limit"),
                    };
                    ValidateContractInstancesResponse(response, context);
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
                case "namespace":
                    @namespace = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.namespace");
                    break;
                case "instances":
                    instances = ReadContractInstanceList(ref reader, $"{context}.instances");
                    break;
                case "total":
                    total = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.total");
                    break;
                case "offset":
                    offset = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.offset");
                    break;
                case "limit":
                    limit = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.limit");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static JsonException DirectMetadataErrorToJsonException(ArgumentException error, string context)
    {
        var field = error.ParamName switch
        {
            nameof(ToriiContractInstance.ContractId) => "contract_id",
            nameof(ToriiContractInstance.CodeHashHex) => "code_hash_hex",
            nameof(ToriiContractInstancesResponse.Namespace) => "namespace",
            nameof(ToriiContractInstancesResponse.Instances) => "instances",
            nameof(ToriiContractInstancesResponse.Total) => "total",
            nameof(ToriiContractInstancesResponse.Offset) => "offset",
            nameof(ToriiContractInstancesResponse.Limit) => "limit",
            _ => error.ParamName,
        };

        return new JsonException($"{context}.{field}: {error.Message}", error);
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

    private static IReadOnlyList<ToriiContractInstance> RequireInstances(
        IReadOnlyList<ToriiContractInstance>? instances,
        string context)
    {
        if (instances is null)
        {
            throw new JsonException($"{context}.instances is required.");
        }

        return instances;
    }

    private static List<ToriiContractInstance>? ReadContractInstanceList(
        ref Utf8JsonReader reader,
        string context)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            return null;
        }

        if (reader.TokenType != JsonTokenType.StartArray)
        {
            throw new JsonException($"{context} must be an array.");
        }

        var instances = new List<ToriiContractInstance>();
        var index = 0;
        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndArray)
            {
                return instances;
            }

            if (reader.TokenType == JsonTokenType.Null)
            {
                throw new JsonException($"{context}[{index}] must be an object.");
            }

            instances.Add(ReadContractInstance(ref reader, $"{context}[{index}]"));
            index++;
        }

        throw new JsonException($"{context} array is incomplete.");
    }
}

internal sealed class ToriiContractInstanceJsonConverter : JsonConverter<ToriiContractInstance>
{
    public override bool HandleNull => true;

    public override ToriiContractInstance Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiContractInstancesJson.ReadContractInstance(ref reader, "contract instance");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiContractInstance value,
        JsonSerializerOptions options)
    {
        ToriiContractInstancesJson.WriteContractInstance(writer, value, "contract instance");
    }
}

internal sealed class ToriiContractInstancesResponseJsonConverter : JsonConverter<ToriiContractInstancesResponse>
{
    public override bool HandleNull => true;

    public override ToriiContractInstancesResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiContractInstancesJson.ReadContractInstancesResponse(ref reader, "contract instances response");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiContractInstancesResponse value,
        JsonSerializerOptions options)
    {
        ToriiContractInstancesJson.WriteContractInstancesResponse(writer, value, "contract instances response");
    }
}
