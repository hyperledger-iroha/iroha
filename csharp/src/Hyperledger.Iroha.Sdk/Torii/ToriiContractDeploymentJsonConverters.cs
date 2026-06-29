using System.Text.Json;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiContractDeploymentJson
{
    internal static void ValidateDeployContractResponse(ToriiDeployContractResponse response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        if (!response.Ok)
        {
            throw new JsonException($"{context}.ok must be true.");
        }

        ToriiSseEventJson.RequireExactTokenText(response.ContractAddress, $"{context}.contract_address");
        ToriiSseEventJson.RequireExactTokenText(response.Dataspace, $"{context}.dataspace");
        ToriiSseEventJson.RequireExactSizedHex(response.CodeHashHex, $"{context}.code_hash_hex", 32);
        ToriiSseEventJson.RequireExactSizedHex(response.AbiHashHex, $"{context}.abi_hash_hex", 32);
    }

    internal static void ValidateDeployAndActivateContractInstanceResponse(
        ToriiDeployAndActivateContractInstanceResponse response,
        string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        if (!response.Ok)
        {
            throw new JsonException($"{context}.ok must be true.");
        }

        ToriiSseEventJson.RequireExactTokenText(response.Namespace, $"{context}.namespace");
        ToriiSseEventJson.RequireExactTokenText(response.ContractId, $"{context}.contract_id");
        ToriiSseEventJson.RequireExactSizedHex(response.CodeHashHex, $"{context}.code_hash_hex", 32);
        ToriiSseEventJson.RequireExactSizedHex(response.AbiHashHex, $"{context}.abi_hash_hex", 32);
    }

    internal static void ValidateActivateContractInstanceResponse(
        ToriiActivateContractInstanceResponse response,
        string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        if (!response.Ok)
        {
            throw new JsonException($"{context}.ok must be true.");
        }
    }

    internal static void WriteDeployContractResponse(
        Utf8JsonWriter writer,
        ToriiDeployContractResponse response,
        string context)
    {
        ValidateDeployContractResponse(response, context);

        writer.WriteStartObject();
        writer.WriteBoolean("ok", response.Ok);
        writer.WriteString("contract_address", response.ContractAddress);
        writer.WriteString("dataspace", response.Dataspace);
        writer.WriteNumber("deploy_nonce", response.DeployNonce);
        writer.WriteString("code_hash_hex", response.CodeHashHex);
        writer.WriteString("abi_hash_hex", response.AbiHashHex);
        writer.WriteEndObject();
    }

    internal static void WriteDeployAndActivateContractInstanceResponse(
        Utf8JsonWriter writer,
        ToriiDeployAndActivateContractInstanceResponse response,
        string context)
    {
        ValidateDeployAndActivateContractInstanceResponse(response, context);

        writer.WriteStartObject();
        writer.WriteBoolean("ok", response.Ok);
        writer.WriteString("namespace", response.Namespace);
        writer.WriteString("contract_id", response.ContractId);
        writer.WriteString("code_hash_hex", response.CodeHashHex);
        writer.WriteString("abi_hash_hex", response.AbiHashHex);
        writer.WriteEndObject();
    }

    internal static void WriteActivateContractInstanceResponse(
        Utf8JsonWriter writer,
        ToriiActivateContractInstanceResponse response,
        string context)
    {
        ValidateActivateContractInstanceResponse(response, context);

        writer.WriteStartObject();
        writer.WriteBoolean("ok", response.Ok);
        writer.WriteEndObject();
    }

    internal static ToriiDeployContractResponse ReadDeployContractResponse(
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
        bool? ok = null;
        string? contractAddress = null;
        string? dataspace = null;
        ulong? deployNonce = null;
        string? codeHashHex = null;
        string? abiHashHex = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiDeployContractResponse
                    {
                        Ok = RequireBool(ok, context, "ok"),
                        ContractAddress = RequireString(contractAddress, context, "contract_address"),
                        Dataspace = RequireString(dataspace, context, "dataspace"),
                        DeployNonce = RequireUInt64(deployNonce, context, "deploy_nonce"),
                        CodeHashHex = RequireString(codeHashHex, context, "code_hash_hex"),
                        AbiHashHex = RequireString(abiHashHex, context, "abi_hash_hex"),
                    };
                    ValidateDeployContractResponse(response, context);
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
                case "ok":
                    ok = ReadBool(ref reader, $"{context}.ok");
                    break;
                case "contract_address":
                    contractAddress = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.contract_address");
                    break;
                case "dataspace":
                    dataspace = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.dataspace");
                    break;
                case "deploy_nonce":
                    deployNonce = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.deploy_nonce");
                    break;
                case "code_hash_hex":
                    codeHashHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.code_hash_hex");
                    break;
                case "abi_hash_hex":
                    abiHashHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.abi_hash_hex");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiDeployAndActivateContractInstanceResponse ReadDeployAndActivateContractInstanceResponse(
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
        bool? ok = null;
        string? @namespace = null;
        string? contractId = null;
        string? codeHashHex = null;
        string? abiHashHex = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiDeployAndActivateContractInstanceResponse
                    {
                        Ok = RequireBool(ok, context, "ok"),
                        Namespace = RequireString(@namespace, context, "namespace"),
                        ContractId = RequireString(contractId, context, "contract_id"),
                        CodeHashHex = RequireString(codeHashHex, context, "code_hash_hex"),
                        AbiHashHex = RequireString(abiHashHex, context, "abi_hash_hex"),
                    };
                    ValidateDeployAndActivateContractInstanceResponse(response, context);
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
                case "ok":
                    ok = ReadBool(ref reader, $"{context}.ok");
                    break;
                case "namespace":
                    @namespace = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.namespace");
                    break;
                case "contract_id":
                    contractId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.contract_id");
                    break;
                case "code_hash_hex":
                    codeHashHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.code_hash_hex");
                    break;
                case "abi_hash_hex":
                    abiHashHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.abi_hash_hex");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiActivateContractInstanceResponse ReadActivateContractInstanceResponse(
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
        bool? ok = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiActivateContractInstanceResponse { Ok = RequireBool(ok, context, "ok") };
                    ValidateActivateContractInstanceResponse(response, context);
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

            if (propertyName == "ok")
            {
                ok = ReadBool(ref reader, $"{context}.ok");
            }
            else
            {
                ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
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

    private static bool RequireBool(bool? value, string context, string propertyName)
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
        return value ?? throw new JsonException($"{context}.{propertyName} must not be null.");
    }

    internal static JsonException DirectMetadataErrorToJsonException(ArgumentException error, string context)
    {
        if (string.Equals(error.ParamName, nameof(ToriiDeployContractResponse.Ok), StringComparison.Ordinal))
        {
            return new JsonException($"{context}.ok must be true.", error);
        }

        var field = error.ParamName switch
        {
            nameof(ToriiDeployContractResponse.Ok) => "ok",
            nameof(ToriiDeployContractResponse.ContractAddress) => "contract_address",
            nameof(ToriiDeployContractResponse.Dataspace) => "dataspace",
            nameof(ToriiDeployContractResponse.CodeHashHex) => "code_hash_hex",
            nameof(ToriiDeployContractResponse.AbiHashHex) => "abi_hash_hex",
            nameof(ToriiDeployAndActivateContractInstanceResponse.Namespace) => "namespace",
            nameof(ToriiDeployAndActivateContractInstanceResponse.ContractId) => "contract_id",
            _ => error.ParamName,
        };

        return new JsonException($"{context}.{field}: {error.Message}", error);
    }
}

internal sealed class ToriiDeployContractResponseJsonConverter : JsonConverter<ToriiDeployContractResponse>
{
    public override bool HandleNull => true;

    public override ToriiDeployContractResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiContractDeploymentJson.ReadDeployContractResponse(ref reader, "contract deploy response");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiDeployContractResponse value,
        JsonSerializerOptions options)
    {
        ToriiContractDeploymentJson.WriteDeployContractResponse(writer, value, "contract deploy response");
    }
}

internal sealed class ToriiDeployAndActivateContractInstanceResponseJsonConverter :
    JsonConverter<ToriiDeployAndActivateContractInstanceResponse>
{
    public override bool HandleNull => true;

    public override ToriiDeployAndActivateContractInstanceResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiContractDeploymentJson.ReadDeployAndActivateContractInstanceResponse(
            ref reader,
            "contract instance deploy response");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiDeployAndActivateContractInstanceResponse value,
        JsonSerializerOptions options)
    {
        ToriiContractDeploymentJson.WriteDeployAndActivateContractInstanceResponse(
            writer,
            value,
            "contract instance deploy response");
    }
}

internal sealed class ToriiActivateContractInstanceResponseJsonConverter :
    JsonConverter<ToriiActivateContractInstanceResponse>
{
    public override bool HandleNull => true;

    public override ToriiActivateContractInstanceResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiContractDeploymentJson.ReadActivateContractInstanceResponse(
            ref reader,
            "contract instance activation response");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiActivateContractInstanceResponse value,
        JsonSerializerOptions options)
    {
        ToriiContractDeploymentJson.WriteActivateContractInstanceResponse(
            writer,
            value,
            "contract instance activation response");
    }
}
