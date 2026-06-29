using System.Text.Json;
using System.Text.Json.Serialization;
using Hyperledger.Iroha.Address;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiMultisigJson
{
    internal static void ValidateMultisigResponse(ToriiMultisigResponse response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ValidateCommonResponse(
            response.Ok,
            response.ResolvedMultisigAccountId,
            response.ProposalId,
            response.InstructionsHash,
            response.TransactionHashHex,
            response.ExecutedTransactionHashHex,
            response.CreationTimeMilliseconds,
            response.SigningMessageBase64,
            context);
    }

    internal static void ValidateMultisigContractCallResponse(
        ToriiMultisigContractCallResponse response,
        string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ValidateCommonResponse(
            response.Ok,
            response.ResolvedMultisigAccountId,
            response.ProposalId,
            response.InstructionsHash,
            response.TransactionHashHex,
            response.ExecutedTransactionHashHex,
            response.CreationTimeMilliseconds,
            response.SigningMessageBase64,
            context);
    }

    internal static MultisigResponseFields ReadCommonResponseFields(ref Utf8JsonReader reader, string context)
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
        var fields = new MultisigResponseFields();

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var resolvedMultisigAccountId = RequireString(
                    fields.ResolvedMultisigAccountId,
                    context,
                    "resolved_multisig_account_id");
                ValidateCommonResponse(
                    fields.Ok,
                    resolvedMultisigAccountId,
                    fields.ProposalId,
                    fields.InstructionsHash,
                    fields.TransactionHashHex,
                    fields.ExecutedTransactionHashHex,
                    fields.CreationTimeMilliseconds,
                    fields.SigningMessageBase64,
                    context);
                fields.ResolvedMultisigAccountId = resolvedMultisigAccountId;
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
                case "ok":
                    fields.Ok = ReadBool(ref reader, $"{context}.ok");
                    break;
                case "resolved_multisig_account_id":
                    fields.ResolvedMultisigAccountId = ToriiAccountFaucetJson.ReadOptionalString(
                        ref reader,
                        $"{context}.resolved_multisig_account_id");
                    break;
                case "submitted":
                    fields.Submitted = ReadOptionalBool(ref reader, $"{context}.submitted");
                    break;
                case "proposal_id":
                    fields.ProposalId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.proposal_id");
                    break;
                case "instructions_hash":
                    fields.InstructionsHash = ToriiAccountFaucetJson.ReadOptionalString(
                        ref reader,
                        $"{context}.instructions_hash");
                    break;
                case "tx_hash_hex":
                    fields.TransactionHashHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.tx_hash_hex");
                    break;
                case "executed_tx_hash_hex":
                    fields.ExecutedTransactionHashHex = ToriiAccountFaucetJson.ReadOptionalString(
                        ref reader,
                        $"{context}.executed_tx_hash_hex");
                    break;
                case "creation_time_ms":
                    fields.CreationTimeMilliseconds = ReadOptionalUInt64(ref reader, $"{context}.creation_time_ms");
                    break;
                case "signing_message_b64":
                    fields.SigningMessageBase64 = ToriiAccountFaucetJson.ReadOptionalString(
                        ref reader,
                        $"{context}.signing_message_b64");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static void WriteCommonResponseFields(
        Utf8JsonWriter writer,
        MultisigResponseFields fields,
        string context)
    {
        var resolvedMultisigAccountId = RequireString(
            fields.ResolvedMultisigAccountId,
            context,
            "resolved_multisig_account_id");
        ValidateCommonResponse(
            fields.Ok,
            resolvedMultisigAccountId,
            fields.ProposalId,
            fields.InstructionsHash,
            fields.TransactionHashHex,
            fields.ExecutedTransactionHashHex,
            fields.CreationTimeMilliseconds,
            fields.SigningMessageBase64,
            context);

        writer.WriteStartObject();
        writer.WriteBoolean("ok", fields.Ok);
        writer.WriteString("resolved_multisig_account_id", resolvedMultisigAccountId);
        WriteNullableBoolean(writer, "submitted", fields.Submitted);
        ToriiVpnJson.WriteNullableString(writer, "proposal_id", fields.ProposalId);
        ToriiVpnJson.WriteNullableString(writer, "instructions_hash", fields.InstructionsHash);
        ToriiVpnJson.WriteNullableString(writer, "tx_hash_hex", fields.TransactionHashHex);
        ToriiVpnJson.WriteNullableString(writer, "executed_tx_hash_hex", fields.ExecutedTransactionHashHex);
        if (fields.CreationTimeMilliseconds.HasValue)
        {
            writer.WriteNumber("creation_time_ms", fields.CreationTimeMilliseconds.Value);
        }
        else
        {
            writer.WriteNull("creation_time_ms");
        }

        ToriiVpnJson.WriteNullableString(writer, "signing_message_b64", fields.SigningMessageBase64);
        writer.WriteEndObject();
    }

    private static void ValidateCommonResponse(
        bool ok,
        string resolvedMultisigAccountId,
        string? proposalId,
        string? instructionsHash,
        string? transactionHashHex,
        string? executedTransactionHashHex,
        ulong? creationTimeMilliseconds,
        string? signingMessageBase64,
        string context)
    {
        if (!ok)
        {
            throw new JsonException($"{context}.ok must be true.");
        }

        RequireCanonicalAccountId(
            resolvedMultisigAccountId,
            $"{context}.resolved_multisig_account_id");
        ToriiSseEventJson.RequireOptionalExactSizedHex(proposalId, $"{context}.proposal_id", 32);
        ToriiSseEventJson.RequireOptionalExactSizedHex(instructionsHash, $"{context}.instructions_hash", 32);
        ValidateOptionalTransactionHashHex(transactionHashHex, $"{context}.tx_hash_hex");
        ValidateOptionalTransactionHashHex(executedTransactionHashHex, $"{context}.executed_tx_hash_hex");
        if (creationTimeMilliseconds == 0)
        {
            throw new JsonException($"{context}.creation_time_ms must be positive when provided.");
        }

        ValidateOptionalBase64(signingMessageBase64, $"{context}.signing_message_b64");
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

    private static bool? ReadOptionalBool(ref Utf8JsonReader reader, string field)
    {
        return reader.TokenType == JsonTokenType.Null ? null : ReadBool(ref reader, field);
    }

    private static ulong? ReadOptionalUInt64(ref Utf8JsonReader reader, string field)
    {
        return reader.TokenType == JsonTokenType.Null
            ? null
            : ToriiAccountFaucetJson.ReadUInt64(ref reader, field);
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

    private static string RequireString(string? value, string context, string propertyName)
    {
        return value ?? throw new JsonException($"{context}.{propertyName} must not be null.");
    }

    private static string RequireCanonicalAccountId(string value, string field)
    {
        ToriiSseEventJson.RequireExactTokenText(value, field);
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

    internal static JsonException DirectMetadataErrorToJsonException(ArgumentException error, string context)
    {
        var field = error.ParamName switch
        {
            nameof(ToriiMultisigResponse.Ok) => "ok",
            nameof(ToriiMultisigResponse.ResolvedMultisigAccountId) => "resolved_multisig_account_id",
            nameof(ToriiMultisigResponse.ProposalId) => "proposal_id",
            nameof(ToriiMultisigResponse.InstructionsHash) => "instructions_hash",
            nameof(ToriiMultisigResponse.TransactionHashHex) => "tx_hash_hex",
            nameof(ToriiMultisigResponse.ExecutedTransactionHashHex) => "executed_tx_hash_hex",
            nameof(ToriiMultisigResponse.CreationTimeMilliseconds) => "creation_time_ms",
            nameof(ToriiMultisigResponse.SigningMessageBase64) => "signing_message_b64",
            _ => error.ParamName,
        };

        return new JsonException($"{context}.{field}: {error.Message}", error);
    }
}

internal struct MultisigResponseFields
{
    internal bool Ok { get; set; }

    internal string? ResolvedMultisigAccountId { get; set; }

    internal bool? Submitted { get; set; }

    internal string? ProposalId { get; set; }

    internal string? InstructionsHash { get; set; }

    internal string? TransactionHashHex { get; set; }

    internal string? ExecutedTransactionHashHex { get; set; }

    internal ulong? CreationTimeMilliseconds { get; set; }

    internal string? SigningMessageBase64 { get; set; }
}

internal sealed class ToriiMultisigResponseJsonConverter : JsonConverter<ToriiMultisigResponse>
{
    public override bool HandleNull => true;

    public override ToriiMultisigResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var fields = ToriiMultisigJson.ReadCommonResponseFields(ref reader, "multisig response");
        try
        {
            return new ToriiMultisigResponse
            {
                Ok = fields.Ok,
                ResolvedMultisigAccountId = fields.ResolvedMultisigAccountId
                    ?? throw new JsonException("multisig response.resolved_multisig_account_id must not be null."),
                Submitted = fields.Submitted,
                ProposalId = fields.ProposalId,
                InstructionsHash = fields.InstructionsHash,
                TransactionHashHex = fields.TransactionHashHex,
                ExecutedTransactionHashHex = fields.ExecutedTransactionHashHex,
                CreationTimeMilliseconds = fields.CreationTimeMilliseconds,
                SigningMessageBase64 = fields.SigningMessageBase64,
            };
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiMultisigJson.DirectMetadataErrorToJsonException(error, "multisig response");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiMultisigResponse value,
        JsonSerializerOptions options)
    {
        ArgumentNullException.ThrowIfNull(value);

        ToriiMultisigJson.WriteCommonResponseFields(
            writer,
            new MultisigResponseFields
            {
                Ok = value.Ok,
                ResolvedMultisigAccountId = value.ResolvedMultisigAccountId,
                Submitted = value.Submitted,
                ProposalId = value.ProposalId,
                InstructionsHash = value.InstructionsHash,
                TransactionHashHex = value.TransactionHashHex,
                ExecutedTransactionHashHex = value.ExecutedTransactionHashHex,
                CreationTimeMilliseconds = value.CreationTimeMilliseconds,
                SigningMessageBase64 = value.SigningMessageBase64,
            },
            "multisig response");
    }
}

internal sealed class ToriiMultisigContractCallResponseJsonConverter
    : JsonConverter<ToriiMultisigContractCallResponse>
{
    public override bool HandleNull => true;

    public override ToriiMultisigContractCallResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        var fields = ToriiMultisigJson.ReadCommonResponseFields(ref reader, "multisig contract-call response");
        try
        {
            return new ToriiMultisigContractCallResponse
            {
                Ok = fields.Ok,
                ResolvedMultisigAccountId = fields.ResolvedMultisigAccountId
                    ?? throw new JsonException("multisig contract-call response.resolved_multisig_account_id must not be null."),
                Submitted = fields.Submitted,
                ProposalId = fields.ProposalId,
                InstructionsHash = fields.InstructionsHash,
                TransactionHashHex = fields.TransactionHashHex,
                ExecutedTransactionHashHex = fields.ExecutedTransactionHashHex,
                CreationTimeMilliseconds = fields.CreationTimeMilliseconds,
                SigningMessageBase64 = fields.SigningMessageBase64,
            };
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw ToriiMultisigJson.DirectMetadataErrorToJsonException(error, "multisig contract-call response");
        }
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiMultisigContractCallResponse value,
        JsonSerializerOptions options)
    {
        ArgumentNullException.ThrowIfNull(value);

        ToriiMultisigJson.WriteCommonResponseFields(
            writer,
            new MultisigResponseFields
            {
                Ok = value.Ok,
                ResolvedMultisigAccountId = value.ResolvedMultisigAccountId,
                Submitted = value.Submitted,
                ProposalId = value.ProposalId,
                InstructionsHash = value.InstructionsHash,
                TransactionHashHex = value.TransactionHashHex,
                ExecutedTransactionHashHex = value.ExecutedTransactionHashHex,
                CreationTimeMilliseconds = value.CreationTimeMilliseconds,
                SigningMessageBase64 = value.SigningMessageBase64,
            },
            "multisig contract-call response");
    }
}
