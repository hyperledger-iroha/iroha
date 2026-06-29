using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiContractCallJson
{
    internal static void ValidateContractCallResponse(ToriiContractCallResponse response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        if (!response.Ok)
        {
            throw new JsonException($"{context}.ok must be true.");
        }

        ToriiSseEventJson.RequireExactTokenText(response.Dataspace, $"{context}.dataspace");
        ToriiSseEventJson.RequireExactTokenText(response.ContractId, $"{context}.contract_id");
        ToriiSseEventJson.RequireOptionalExactTokenText(response.ContractAddress, $"{context}.contract_address");
        ToriiSseEventJson.RequireExactSizedHex(response.CodeHashHex, $"{context}.code_hash_hex", 32);
        ToriiSseEventJson.RequireExactSizedHex(response.AbiHashHex, $"{context}.abi_hash_hex", 32);
        if (response.CreationTimeMilliseconds == 0)
        {
            throw new JsonException($"{context}.creation_time_ms must be positive.");
        }

        ValidateOptionalTransactionHashHex(response.TransactionHashHex, $"{context}.tx_hash_hex");
        ValidateOptionalBase64(response.TransactionScaffoldBase64, $"{context}.transaction_scaffold_b64");
        ValidateOptionalBase64(response.SignedTransactionBase64, $"{context}.signed_transaction_b64");
        ValidateOptionalBase64(response.SigningMessageBase64, $"{context}.signing_message_b64");
        ToriiSseEventJson.RequireOptionalExactTokenText(response.Entrypoint, $"{context}.entrypoint");
    }

    internal static void WriteContractCallResponse(
        Utf8JsonWriter writer,
        ToriiContractCallResponse response,
        string context)
    {
        ValidateContractCallResponse(response, context);

        writer.WriteStartObject();
        writer.WriteBoolean("ok", response.Ok);
        writer.WriteBoolean("submitted", response.Submitted);
        writer.WriteString("dataspace", response.Dataspace);
        writer.WriteString("contract_id", response.ContractId);
        ToriiVpnJson.WriteNullableString(writer, "contract_address", response.ContractAddress);
        writer.WriteString("code_hash_hex", response.CodeHashHex);
        writer.WriteString("abi_hash_hex", response.AbiHashHex);
        writer.WriteNumber("creation_time_ms", response.CreationTimeMilliseconds);
        ToriiVpnJson.WriteNullableString(writer, "tx_hash_hex", response.TransactionHashHex);
        ToriiVpnJson.WriteNullableString(writer, "transaction_scaffold_b64", response.TransactionScaffoldBase64);
        ToriiVpnJson.WriteNullableString(writer, "signed_transaction_b64", response.SignedTransactionBase64);
        ToriiVpnJson.WriteNullableString(writer, "signing_message_b64", response.SigningMessageBase64);
        ToriiVpnJson.WriteNullableString(writer, "entrypoint", response.Entrypoint);
        writer.WriteEndObject();
    }

    internal static void ValidateContractViewResponse(ToriiContractViewResponse response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        if (!response.Ok)
        {
            throw new JsonException($"{context}.ok must be true.");
        }

        ToriiSseEventJson.RequireExactTokenText(response.Dataspace, $"{context}.dataspace");
        ToriiSseEventJson.RequireExactTokenText(response.ContractId, $"{context}.contract_id");
        ToriiSseEventJson.RequireOptionalExactTokenText(response.ContractAddress, $"{context}.contract_address");
        ToriiSseEventJson.RequireExactSizedHex(response.CodeHashHex, $"{context}.code_hash_hex", 32);
        ToriiSseEventJson.RequireExactSizedHex(response.AbiHashHex, $"{context}.abi_hash_hex", 32);
        ToriiSseEventJson.RequireExactTokenText(response.Entrypoint, $"{context}.entrypoint");
    }

    internal static void WriteContractViewResponse(
        Utf8JsonWriter writer,
        ToriiContractViewResponse response,
        string context)
    {
        ValidateContractViewResponse(response, context);

        writer.WriteStartObject();
        writer.WriteBoolean("ok", response.Ok);
        writer.WriteString("dataspace", response.Dataspace);
        writer.WriteString("contract_id", response.ContractId);
        ToriiVpnJson.WriteNullableString(writer, "contract_address", response.ContractAddress);
        writer.WriteString("code_hash_hex", response.CodeHashHex);
        writer.WriteString("abi_hash_hex", response.AbiHashHex);
        writer.WriteString("entrypoint", response.Entrypoint);
        writer.WritePropertyName("result");
        if (response.Result is null)
        {
            writer.WriteNullValue();
        }
        else
        {
            response.Result.WriteTo(writer);
        }

        writer.WriteEndObject();
    }

    internal static void ValidateContractViewErrorResponse(ToriiContractViewErrorResponse response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        if (response.Ok)
        {
            throw new JsonException($"{context}.ok must be false.");
        }

        ToriiSseEventJson.RequireExactTokenText(response.Dataspace, $"{context}.dataspace");
        ToriiSseEventJson.RequireExactTokenText(response.ContractId, $"{context}.contract_id");
        ToriiSseEventJson.RequireOptionalExactTokenText(response.ContractAddress, $"{context}.contract_address");
        ToriiSseEventJson.RequireExactSizedHex(response.CodeHashHex, $"{context}.code_hash_hex", 32);
        ToriiSseEventJson.RequireExactSizedHex(response.AbiHashHex, $"{context}.abi_hash_hex", 32);
        ToriiSseEventJson.RequireExactTokenText(response.Entrypoint, $"{context}.entrypoint");
        RequireExactNonEmptyText(response.Error, $"{context}.error");
        ValidateOptionalContractViewVmDiagnostic(response.VmDiagnostic, $"{context}.vm_diagnostic");
    }

    internal static void ValidateOptionalContractViewVmDiagnostic(
        ToriiContractViewVmDiagnostic? response,
        string context)
    {
        if (response is null)
        {
            return;
        }

        ToriiSseEventJson.RequireExactTokenText(response.TrapKind, $"{context}.trap_kind");
        RequireExactNonEmptyText(response.Message, $"{context}.message");
        RequireOptionalExactNonEmptyText(response.Function, $"{context}.function");
        RequireOptionalExactNonEmptyText(response.SourcePath, $"{context}.source_path");
        RequireOptionalExactNonEmptyText(response.CurrentFunction, $"{context}.current_function");
        RequirePositiveUInt64(response.GasLimit, $"{context}.gas_limit");
        RequirePositiveUInt64(response.MaxCycles, $"{context}.max_cycles");
        RequirePositiveUInt64(response.StackLimitBytes, $"{context}.stack_limit_bytes");

        if (response.GasRemaining > response.GasLimit)
        {
            throw new JsonException($"{context}.gas_remaining must be less than or equal to gas_limit.");
        }

        if (response.GasUsed > response.GasLimit)
        {
            throw new JsonException($"{context}.gas_used must be less than or equal to gas_limit.");
        }

        if (response.Cycles > response.MaxCycles)
        {
            throw new JsonException($"{context}.cycles must be less than or equal to max_cycles.");
        }

        if (response.StackBytesUsed > response.StackLimitBytes)
        {
            throw new JsonException($"{context}.stack_bytes_used must be less than or equal to stack_limit_bytes.");
        }
    }

    private static void RequirePositiveUInt64(ulong value, string field)
    {
        if (value == 0)
        {
            throw new JsonException($"{field} must be positive.");
        }
    }

    private static ulong RequireUInt64(ulong? value, string context, string propertyName)
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
        if (value is null)
        {
            throw new JsonException($"{context}.{propertyName} must not be null.");
        }

        return value;
    }

    internal static void WriteContractViewErrorResponse(
        Utf8JsonWriter writer,
        ToriiContractViewErrorResponse response,
        string context)
    {
        ValidateContractViewErrorResponse(response, context);

        writer.WriteStartObject();
        writer.WriteBoolean("ok", response.Ok);
        writer.WriteString("dataspace", response.Dataspace);
        writer.WriteString("contract_id", response.ContractId);
        ToriiVpnJson.WriteNullableString(writer, "contract_address", response.ContractAddress);
        writer.WriteString("code_hash_hex", response.CodeHashHex);
        writer.WriteString("abi_hash_hex", response.AbiHashHex);
        writer.WriteString("entrypoint", response.Entrypoint);
        writer.WriteString("error", response.Error);
        if (response.VmDiagnostic is null)
        {
            writer.WriteNull("vm_diagnostic");
        }
        else
        {
            writer.WritePropertyName("vm_diagnostic");
            WriteContractViewVmDiagnostic(writer, response.VmDiagnostic, $"{context}.vm_diagnostic");
        }

        writer.WriteEndObject();
    }

    internal static void WriteContractViewVmDiagnostic(
        Utf8JsonWriter writer,
        ToriiContractViewVmDiagnostic response,
        string context)
    {
        ValidateOptionalContractViewVmDiagnostic(response, context);

        writer.WriteStartObject();
        writer.WriteString("trap_kind", response.TrapKind);
        writer.WriteString("message", response.Message);
        writer.WriteNumber("pc", response.ProgramCounter);
        ToriiVpnJson.WriteNullableString(writer, "function", response.Function);
        ToriiVpnJson.WriteNullableString(writer, "source_path", response.SourcePath);
        WriteNullableUInt32(writer, "line", response.Line);
        WriteNullableUInt32(writer, "column", response.Column);
        writer.WriteNumber("gas_limit", response.GasLimit);
        writer.WriteNumber("gas_remaining", response.GasRemaining);
        writer.WriteNumber("gas_used", response.GasUsed);
        writer.WriteNumber("cycles", response.Cycles);
        writer.WriteNumber("max_cycles", response.MaxCycles);
        writer.WriteNumber("stack_limit_bytes", response.StackLimitBytes);
        writer.WriteNumber("stack_bytes_used", response.StackBytesUsed);
        WriteNullableUInt64(writer, "entrypoint_pc", response.EntrypointProgramCounter);
        ToriiVpnJson.WriteNullableString(writer, "current_function", response.CurrentFunction);
        WriteNullableUInt16(writer, "opcode", response.Opcode);
        WriteNullableUInt32(writer, "syscall", response.Syscall);
        writer.WriteBoolean("predecoded_loaded", response.PredecodedLoaded);
        WriteNullableBoolean(writer, "predecoded_hit", response.PredecodedHit);
        writer.WriteEndObject();
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

    private static void ValidateOptionalTransactionHashHex(string? value, string field)
    {
        if (value is null || value.Length == 0)
        {
            return;
        }

        ToriiSseEventJson.RequireExactSizedHex(value, field, 32);
    }

    private static void ValidateOptionalBase64(string? value, string field)
    {
        if (value is null)
        {
            return;
        }

        if (string.IsNullOrWhiteSpace(value))
        {
            throw new JsonException($"{field} must be a non-empty base64 string.");
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must not contain surrounding whitespace.");
        }

        foreach (var character in value)
        {
            if (char.IsWhiteSpace(character))
            {
                throw new JsonException($"{field} must not contain whitespace.");
            }

            if (char.IsControl(character))
            {
                throw new JsonException($"{field} must not contain control characters.");
            }
        }

        byte[] bytes;
        try
        {
            bytes = Convert.FromBase64String(value);
        }
        catch (FormatException error)
        {
            throw new JsonException($"{field} must be valid base64.", error);
        }

        if (bytes.Length == 0)
        {
            throw new JsonException($"{field} must not decode to empty bytes.");
        }

        if (!string.Equals(Convert.ToBase64String(bytes), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must be canonical base64 text.");
        }
    }

    private static bool? ReadOptionalBool(ref Utf8JsonReader reader, string field)
    {
        return reader.TokenType == JsonTokenType.Null ? null : ReadBool(ref reader, field);
    }

    private static ushort? ReadOptionalUInt16(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            return null;
        }

        if (reader.TokenType != JsonTokenType.Number || !reader.TryGetUInt16(out var value))
        {
            throw new JsonException($"{field} must be an unsigned 16-bit integer.");
        }

        return value;
    }

    private static uint? ReadOptionalUInt32(ref Utf8JsonReader reader, string field)
    {
        return reader.TokenType == JsonTokenType.Null ? null : ToriiAccountFaucetJson.ReadUInt32(ref reader, field);
    }

    private static ulong? ReadOptionalUInt64(ref Utf8JsonReader reader, string field)
    {
        return reader.TokenType == JsonTokenType.Null ? null : ToriiAccountFaucetJson.ReadUInt64(ref reader, field);
    }

    private static void WriteNullableBoolean(Utf8JsonWriter writer, string propertyName, bool? value)
    {
        if (value.HasValue)
        {
            writer.WriteBoolean(propertyName, value.Value);
            return;
        }

        writer.WriteNull(propertyName);
    }

    private static void WriteNullableUInt16(Utf8JsonWriter writer, string propertyName, ushort? value)
    {
        if (value.HasValue)
        {
            writer.WriteNumber(propertyName, value.Value);
            return;
        }

        writer.WriteNull(propertyName);
    }

    private static void WriteNullableUInt32(Utf8JsonWriter writer, string propertyName, uint? value)
    {
        if (value.HasValue)
        {
            writer.WriteNumber(propertyName, value.Value);
            return;
        }

        writer.WriteNull(propertyName);
    }

    private static void WriteNullableUInt64(Utf8JsonWriter writer, string propertyName, ulong? value)
    {
        if (value.HasValue)
        {
            writer.WriteNumber(propertyName, value.Value);
            return;
        }

        writer.WriteNull(propertyName);
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

        foreach (var character in value)
        {
            if (char.IsControl(character))
            {
                throw new JsonException($"{field} must not contain control characters.");
            }
        }

        return value;
    }

    private static string? RequireOptionalExactNonEmptyText(string? value, string field)
    {
        return value is null ? null : RequireExactNonEmptyText(value, field);
    }

    internal static JsonException DirectMetadataErrorToJsonException(ArgumentException error, string context)
    {
        if (error.ParamName == nameof(ToriiContractCallResponse.Ok))
        {
            return context.Contains("error", StringComparison.Ordinal)
                ? new JsonException($"{context}.ok must be false.", error)
                : new JsonException($"{context}.ok must be true.", error);
        }

        var field = error.ParamName switch
        {
            nameof(ToriiContractCallResponse.Submitted) => "submitted",
            nameof(ToriiContractCallResponse.Dataspace) => "dataspace",
            nameof(ToriiContractCallResponse.ContractId) => "contract_id",
            nameof(ToriiContractCallResponse.ContractAddress) => "contract_address",
            nameof(ToriiContractCallResponse.CodeHashHex) => "code_hash_hex",
            nameof(ToriiContractCallResponse.AbiHashHex) => "abi_hash_hex",
            nameof(ToriiContractCallResponse.CreationTimeMilliseconds) => "creation_time_ms",
            nameof(ToriiContractCallResponse.TransactionHashHex) => "tx_hash_hex",
            nameof(ToriiContractCallResponse.TransactionScaffoldBase64) => "transaction_scaffold_b64",
            nameof(ToriiContractCallResponse.SignedTransactionBase64) => "signed_transaction_b64",
            nameof(ToriiContractCallResponse.SigningMessageBase64) => "signing_message_b64",
            nameof(ToriiContractCallResponse.Entrypoint) => "entrypoint",
            nameof(ToriiContractViewResponse.Result) => "result",
            nameof(ToriiContractViewErrorResponse.Error) => "error",
            nameof(ToriiContractViewErrorResponse.VmDiagnostic) => "vm_diagnostic",
            nameof(ToriiContractViewVmDiagnostic.TrapKind) => "trap_kind",
            nameof(ToriiContractViewVmDiagnostic.Message) => "message",
            nameof(ToriiContractViewVmDiagnostic.ProgramCounter) => "pc",
            nameof(ToriiContractViewVmDiagnostic.Function) => "function",
            nameof(ToriiContractViewVmDiagnostic.SourcePath) => "source_path",
            nameof(ToriiContractViewVmDiagnostic.Line) => "line",
            nameof(ToriiContractViewVmDiagnostic.Column) => "column",
            nameof(ToriiContractViewVmDiagnostic.GasLimit) => "gas_limit",
            nameof(ToriiContractViewVmDiagnostic.GasRemaining) => "gas_remaining",
            nameof(ToriiContractViewVmDiagnostic.GasUsed) => "gas_used",
            nameof(ToriiContractViewVmDiagnostic.Cycles) => "cycles",
            nameof(ToriiContractViewVmDiagnostic.MaxCycles) => "max_cycles",
            nameof(ToriiContractViewVmDiagnostic.StackLimitBytes) => "stack_limit_bytes",
            nameof(ToriiContractViewVmDiagnostic.StackBytesUsed) => "stack_bytes_used",
            nameof(ToriiContractViewVmDiagnostic.EntrypointProgramCounter) => "entrypoint_pc",
            nameof(ToriiContractViewVmDiagnostic.CurrentFunction) => "current_function",
            nameof(ToriiContractViewVmDiagnostic.Opcode) => "opcode",
            nameof(ToriiContractViewVmDiagnostic.Syscall) => "syscall",
            nameof(ToriiContractViewVmDiagnostic.PredecodedLoaded) => "predecoded_loaded",
            nameof(ToriiContractViewVmDiagnostic.PredecodedHit) => "predecoded_hit",
            _ => error.ParamName,
        };

        return new JsonException($"{context}.{field}: {error.Message}", error);
    }

    internal static ToriiContractCallResponse ReadContractCallResponse(
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
        bool? submitted = null;
        string? dataspace = null;
        string? contractId = null;
        string? contractAddress = null;
        string? codeHashHex = null;
        string? abiHashHex = null;
        ulong? creationTimeMilliseconds = null;
        string? transactionHashHex = null;
        string? transactionScaffoldBase64 = null;
        string? signedTransactionBase64 = null;
        string? signingMessageBase64 = null;
        string? entrypoint = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiContractCallResponse
                    {
                        Ok = RequireBool(ok, context, "ok"),
                        Submitted = RequireBool(submitted, context, "submitted"),
                        Dataspace = RequireString(dataspace, context, "dataspace"),
                        ContractId = RequireString(contractId, context, "contract_id"),
                        ContractAddress = contractAddress,
                        CodeHashHex = RequireString(codeHashHex, context, "code_hash_hex"),
                        AbiHashHex = RequireString(abiHashHex, context, "abi_hash_hex"),
                        CreationTimeMilliseconds = RequireUInt64(
                            creationTimeMilliseconds,
                            context,
                            "creation_time_ms"),
                        TransactionHashHex = transactionHashHex,
                        TransactionScaffoldBase64 = transactionScaffoldBase64,
                        SignedTransactionBase64 = signedTransactionBase64,
                        SigningMessageBase64 = signingMessageBase64,
                        Entrypoint = entrypoint,
                    };
                    ValidateContractCallResponse(response, context);
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
                case "submitted":
                    submitted = ReadBool(ref reader, $"{context}.submitted");
                    break;
                case "dataspace":
                    dataspace = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.dataspace");
                    break;
                case "contract_id":
                    contractId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.contract_id");
                    break;
                case "contract_address":
                    contractAddress = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.contract_address");
                    break;
                case "code_hash_hex":
                    codeHashHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.code_hash_hex");
                    break;
                case "abi_hash_hex":
                    abiHashHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.abi_hash_hex");
                    break;
                case "creation_time_ms":
                    creationTimeMilliseconds = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.creation_time_ms");
                    break;
                case "tx_hash_hex":
                    transactionHashHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.tx_hash_hex");
                    break;
                case "transaction_scaffold_b64":
                    transactionScaffoldBase64 = ToriiAccountFaucetJson.ReadOptionalString(
                        ref reader,
                        $"{context}.transaction_scaffold_b64");
                    break;
                case "signed_transaction_b64":
                    signedTransactionBase64 = ToriiAccountFaucetJson.ReadOptionalString(
                        ref reader,
                        $"{context}.signed_transaction_b64");
                    break;
                case "signing_message_b64":
                    signingMessageBase64 = ToriiAccountFaucetJson.ReadOptionalString(
                        ref reader,
                        $"{context}.signing_message_b64");
                    break;
                case "entrypoint":
                    entrypoint = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.entrypoint");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiContractViewResponse ReadContractViewResponse(
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
        string? dataspace = null;
        string? contractId = null;
        string? contractAddress = null;
        string? codeHashHex = null;
        string? abiHashHex = null;
        string? entrypoint = null;
        JsonNode? result = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiContractViewResponse
                    {
                        Ok = RequireBool(ok, context, "ok"),
                        Dataspace = RequireString(dataspace, context, "dataspace"),
                        ContractId = RequireString(contractId, context, "contract_id"),
                        ContractAddress = contractAddress,
                        CodeHashHex = RequireString(codeHashHex, context, "code_hash_hex"),
                        AbiHashHex = RequireString(abiHashHex, context, "abi_hash_hex"),
                        Entrypoint = RequireString(entrypoint, context, "entrypoint"),
                        Result = result,
                    };
                    ValidateContractViewResponse(response, context);
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
                case "dataspace":
                    dataspace = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.dataspace");
                    break;
                case "contract_id":
                    contractId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.contract_id");
                    break;
                case "contract_address":
                    contractAddress = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.contract_address");
                    break;
                case "code_hash_hex":
                    codeHashHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.code_hash_hex");
                    break;
                case "abi_hash_hex":
                    abiHashHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.abi_hash_hex");
                    break;
                case "entrypoint":
                    entrypoint = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.entrypoint");
                    break;
                case "result":
                    result = ToriiIdentifierJson.ReadOptionalNode(ref reader, $"{context}.result");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiContractViewErrorResponse ReadContractViewErrorResponse(
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
        string? dataspace = null;
        string? contractId = null;
        string? contractAddress = null;
        string? codeHashHex = null;
        string? abiHashHex = null;
        string? entrypoint = null;
        string? error = null;
        ToriiContractViewVmDiagnostic? vmDiagnostic = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiContractViewErrorResponse
                    {
                        Ok = RequireBool(ok, context, "ok"),
                        Dataspace = RequireString(dataspace, context, "dataspace"),
                        ContractId = RequireString(contractId, context, "contract_id"),
                        ContractAddress = contractAddress,
                        CodeHashHex = RequireString(codeHashHex, context, "code_hash_hex"),
                        AbiHashHex = RequireString(abiHashHex, context, "abi_hash_hex"),
                        Entrypoint = RequireString(entrypoint, context, "entrypoint"),
                        Error = RequireString(error, context, "error"),
                        VmDiagnostic = vmDiagnostic,
                    };
                    ValidateContractViewErrorResponse(response, context);
                    return response;
                }
                catch (ArgumentException exception) when (exception.ParamName is not null)
                {
                    throw DirectMetadataErrorToJsonException(exception, context);
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
                case "dataspace":
                    dataspace = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.dataspace");
                    break;
                case "contract_id":
                    contractId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.contract_id");
                    break;
                case "contract_address":
                    contractAddress = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.contract_address");
                    break;
                case "code_hash_hex":
                    codeHashHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.code_hash_hex");
                    break;
                case "abi_hash_hex":
                    abiHashHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.abi_hash_hex");
                    break;
                case "entrypoint":
                    entrypoint = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.entrypoint");
                    break;
                case "error":
                    error = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.error");
                    break;
                case "vm_diagnostic":
                    vmDiagnostic = reader.TokenType == JsonTokenType.Null
                        ? null
                        : ReadContractViewVmDiagnostic(ref reader, $"{context}.vm_diagnostic");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiContractViewVmDiagnostic ReadContractViewVmDiagnostic(
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
        string? trapKind = null;
        string? message = null;
        ulong? programCounter = null;
        string? function = null;
        string? sourcePath = null;
        uint? line = null;
        uint? column = null;
        ulong? gasLimit = null;
        ulong? gasRemaining = null;
        ulong? gasUsed = null;
        ulong? cycles = null;
        ulong? maxCycles = null;
        ulong? stackLimitBytes = null;
        ulong? stackBytesUsed = null;
        ulong? entrypointProgramCounter = null;
        string? currentFunction = null;
        ushort? opcode = null;
        uint? syscall = null;
        bool? predecodedLoaded = null;
        bool? predecodedHit = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiContractViewVmDiagnostic
                    {
                        TrapKind = RequireString(trapKind, context, "trap_kind"),
                        Message = RequireString(message, context, "message"),
                        ProgramCounter = RequireUInt64(programCounter, context, "pc"),
                        Function = function,
                        SourcePath = sourcePath,
                        Line = line,
                        Column = column,
                        GasLimit = RequireUInt64(gasLimit, context, "gas_limit"),
                        GasRemaining = RequireUInt64(gasRemaining, context, "gas_remaining"),
                        GasUsed = RequireUInt64(gasUsed, context, "gas_used"),
                        Cycles = RequireUInt64(cycles, context, "cycles"),
                        MaxCycles = RequireUInt64(maxCycles, context, "max_cycles"),
                        StackLimitBytes = RequireUInt64(stackLimitBytes, context, "stack_limit_bytes"),
                        StackBytesUsed = RequireUInt64(stackBytesUsed, context, "stack_bytes_used"),
                        EntrypointProgramCounter = entrypointProgramCounter,
                        CurrentFunction = currentFunction,
                        Opcode = opcode,
                        Syscall = syscall,
                        PredecodedLoaded = RequireBool(predecodedLoaded, context, "predecoded_loaded"),
                        PredecodedHit = predecodedHit,
                    };
                    ValidateOptionalContractViewVmDiagnostic(response, context);
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
                case "trap_kind":
                    trapKind = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.trap_kind");
                    break;
                case "message":
                    message = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.message");
                    break;
                case "pc":
                    programCounter = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.pc");
                    break;
                case "function":
                    function = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.function");
                    break;
                case "source_path":
                    sourcePath = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.source_path");
                    break;
                case "line":
                    line = ReadOptionalUInt32(ref reader, $"{context}.line");
                    break;
                case "column":
                    column = ReadOptionalUInt32(ref reader, $"{context}.column");
                    break;
                case "gas_limit":
                    gasLimit = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.gas_limit");
                    break;
                case "gas_remaining":
                    gasRemaining = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.gas_remaining");
                    break;
                case "gas_used":
                    gasUsed = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.gas_used");
                    break;
                case "cycles":
                    cycles = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.cycles");
                    break;
                case "max_cycles":
                    maxCycles = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.max_cycles");
                    break;
                case "stack_limit_bytes":
                    stackLimitBytes = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.stack_limit_bytes");
                    break;
                case "stack_bytes_used":
                    stackBytesUsed = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.stack_bytes_used");
                    break;
                case "entrypoint_pc":
                    entrypointProgramCounter = ReadOptionalUInt64(ref reader, $"{context}.entrypoint_pc");
                    break;
                case "current_function":
                    currentFunction = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.current_function");
                    break;
                case "opcode":
                    opcode = ReadOptionalUInt16(ref reader, $"{context}.opcode");
                    break;
                case "syscall":
                    syscall = ReadOptionalUInt32(ref reader, $"{context}.syscall");
                    break;
                case "predecoded_loaded":
                    predecodedLoaded = ReadBool(ref reader, $"{context}.predecoded_loaded");
                    break;
                case "predecoded_hit":
                    predecodedHit = ReadOptionalBool(ref reader, $"{context}.predecoded_hit");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }
}

internal sealed class ToriiContractCallResponseJsonConverter : JsonConverter<ToriiContractCallResponse>
{
    public override bool HandleNull => true;

    public override ToriiContractCallResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiContractCallJson.ReadContractCallResponse(ref reader, "contract call response");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiContractCallResponse value,
        JsonSerializerOptions options)
    {
        ArgumentNullException.ThrowIfNull(value);
        ToriiContractCallJson.WriteContractCallResponse(writer, value, "contract call response");
    }
}

internal sealed class ToriiContractViewResponseJsonConverter : JsonConverter<ToriiContractViewResponse>
{
    public override bool HandleNull => true;

    public override ToriiContractViewResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiContractCallJson.ReadContractViewResponse(ref reader, "contract view response");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiContractViewResponse value,
        JsonSerializerOptions options)
    {
        ArgumentNullException.ThrowIfNull(value);
        ToriiContractCallJson.WriteContractViewResponse(writer, value, "contract view response");
    }
}

internal sealed class ToriiContractViewErrorResponseJsonConverter : JsonConverter<ToriiContractViewErrorResponse>
{
    public override bool HandleNull => true;

    public override ToriiContractViewErrorResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiContractCallJson.ReadContractViewErrorResponse(ref reader, "contract view error response");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiContractViewErrorResponse value,
        JsonSerializerOptions options)
    {
        ArgumentNullException.ThrowIfNull(value);
        ToriiContractCallJson.WriteContractViewErrorResponse(writer, value, "contract view error response");
    }
}

internal sealed class ToriiContractViewVmDiagnosticJsonConverter : JsonConverter<ToriiContractViewVmDiagnostic>
{
    public override bool HandleNull => true;

    public override ToriiContractViewVmDiagnostic Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiContractCallJson.ReadContractViewVmDiagnostic(ref reader, "contract view VM diagnostic");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiContractViewVmDiagnostic value,
        JsonSerializerOptions options)
    {
        ArgumentNullException.ThrowIfNull(value);
        ToriiContractCallJson.WriteContractViewVmDiagnostic(writer, value, "contract view VM diagnostic");
    }
}
