using System.Text.Json;
using System.Text.Json.Serialization;
using Hyperledger.Iroha.Address;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiOnboardingJson
{
    internal static void ValidateAccountOnboardingResponse(ToriiAccountOnboardingResponse response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        RequireCanonicalAccountId(response.AccountId, $"{context}.account_id");
        ToriiSseEventJson.RequireExactTokenText(response.Uaid, $"{context}.uaid");
        ValidateOptionalTransactionHashHex(response.TransactionHashHex, $"{context}.tx_hash_hex");
        ToriiSseEventJson.RequireExactTokenText(response.Status, $"{context}.status");
    }

    internal static void ValidateAccountFaucetResponse(ToriiAccountFaucetResponse response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        RequireCanonicalAccountId(response.AccountId, $"{context}.account_id");
        ToriiSseEventJson.RequireExactTokenText(response.AssetDefinitionId, $"{context}.asset_definition_id");
        ToriiSseEventJson.RequireExactTokenText(response.AssetId, $"{context}.asset_id");
        ToriiSseEventJson.RequireExactTokenText(response.Amount, $"{context}.amount");
        ValidateOptionalTransactionHashHex(response.TransactionHashHex, $"{context}.tx_hash_hex");
        ToriiSseEventJson.RequireExactTokenText(response.Status, $"{context}.status");
    }

    internal static void ValidateMultisigAccountOnboardingResponse(
        ToriiMultisigAccountOnboardingResponse response,
        string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        RequireCanonicalAccountId(response.AccountId, $"{context}.account_id");
        ValidateOptionalTransactionHashHex(response.TransactionHashHex, $"{context}.tx_hash_hex");
        ToriiSseEventJson.RequireExactTokenText(response.Status, $"{context}.status");
    }

    internal static ToriiAccountOnboardingResponse ReadAccountOnboardingResponse(
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
        string? accountId = null;
        string? uaid = null;
        string? transactionHashHex = null;
        string? status = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiAccountOnboardingResponse
                    {
                        AccountId = RequireString(accountId, context, "account_id"),
                        Uaid = RequireString(uaid, context, "uaid"),
                        TransactionHashHex = transactionHashHex is null ? string.Empty : transactionHashHex,
                        Status = RequireString(status, context, "status"),
                    };
                    ValidateAccountOnboardingResponse(response, context);
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
                case "account_id":
                    accountId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.account_id");
                    break;
                case "uaid":
                    uaid = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.uaid");
                    break;
                case "tx_hash_hex":
                    transactionHashHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.tx_hash_hex");
                    break;
                case "status":
                    status = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.status");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiAccountFaucetResponse ReadAccountFaucetResponse(
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
        string? accountId = null;
        string? assetDefinitionId = null;
        string? assetId = null;
        string? amount = null;
        string? transactionHashHex = null;
        string? status = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiAccountFaucetResponse
                    {
                        AccountId = RequireString(accountId, context, "account_id"),
                        AssetDefinitionId = RequireString(assetDefinitionId, context, "asset_definition_id"),
                        AssetId = RequireString(assetId, context, "asset_id"),
                        Amount = RequireString(amount, context, "amount"),
                        TransactionHashHex = transactionHashHex is null ? string.Empty : transactionHashHex,
                        Status = RequireString(status, context, "status"),
                    };
                    ValidateAccountFaucetResponse(response, context);
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
                case "account_id":
                    accountId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.account_id");
                    break;
                case "asset_definition_id":
                    assetDefinitionId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.asset_definition_id");
                    break;
                case "asset_id":
                    assetId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.asset_id");
                    break;
                case "amount":
                    amount = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.amount");
                    break;
                case "tx_hash_hex":
                    transactionHashHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.tx_hash_hex");
                    break;
                case "status":
                    status = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.status");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiMultisigAccountOnboardingResponse ReadMultisigAccountOnboardingResponse(
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
        string? accountId = null;
        string? transactionHashHex = null;
        string? status = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiMultisigAccountOnboardingResponse
                    {
                        AccountId = RequireString(accountId, context, "account_id"),
                        TransactionHashHex = transactionHashHex is null ? string.Empty : transactionHashHex,
                        Status = RequireString(status, context, "status"),
                    };
                    ValidateMultisigAccountOnboardingResponse(response, context);
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
                case "account_id":
                    accountId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.account_id");
                    break;
                case "tx_hash_hex":
                    transactionHashHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.tx_hash_hex");
                    break;
                case "status":
                    status = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.status");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
                    break;
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static void WriteAccountOnboardingResponse(
        Utf8JsonWriter writer,
        ToriiAccountOnboardingResponse response,
        string context)
    {
        ValidateAccountOnboardingResponse(response, context);

        writer.WriteStartObject();
        writer.WriteString("account_id", response.AccountId);
        writer.WriteString("uaid", response.Uaid);
        writer.WriteString("tx_hash_hex", response.TransactionHashHex);
        writer.WriteString("status", response.Status);
        writer.WriteEndObject();
    }

    internal static void WriteAccountFaucetResponse(
        Utf8JsonWriter writer,
        ToriiAccountFaucetResponse response,
        string context)
    {
        ValidateAccountFaucetResponse(response, context);

        writer.WriteStartObject();
        writer.WriteString("account_id", response.AccountId);
        writer.WriteString("asset_definition_id", response.AssetDefinitionId);
        writer.WriteString("asset_id", response.AssetId);
        writer.WriteString("amount", response.Amount);
        writer.WriteString("tx_hash_hex", response.TransactionHashHex);
        writer.WriteString("status", response.Status);
        writer.WriteEndObject();
    }

    internal static void WriteMultisigAccountOnboardingResponse(
        Utf8JsonWriter writer,
        ToriiMultisigAccountOnboardingResponse response,
        string context)
    {
        ValidateMultisigAccountOnboardingResponse(response, context);

        writer.WriteStartObject();
        writer.WriteString("account_id", response.AccountId);
        writer.WriteString("tx_hash_hex", response.TransactionHashHex);
        writer.WriteString("status", response.Status);
        writer.WriteEndObject();
    }

    internal static JsonException DirectMetadataErrorToJsonException(ArgumentException error, string context)
    {
        var field = error.ParamName switch
        {
            "AccountId" => "account_id",
            "Uaid" => "uaid",
            "TransactionHashHex" => "tx_hash_hex",
            "Status" => "status",
            "AssetDefinitionId" => "asset_definition_id",
            "AssetId" => "asset_id",
            "Amount" => "amount",
            _ => error.ParamName,
        };
        return new JsonException($"{context}.{field}: {error.Message}", error);
    }

    private static void ValidateOptionalTransactionHashHex(string? value, string field)
    {
        if (value is null || value.Length == 0)
        {
            return;
        }

        ToriiSseEventJson.RequireExactSizedHex(value, field, 32);
    }

    private static string RequireString(string? value, string context, string propertyName)
    {
        return value ?? throw new JsonException($"{context}.{propertyName} must not be null.");
    }

    private static string RequireCanonicalAccountId(string? value, string field)
    {
        var text = ToriiSseEventJson.RequireExactTokenText(value, field);
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
}

internal sealed class ToriiAccountOnboardingResponseJsonConverter : JsonConverter<ToriiAccountOnboardingResponse>
{
    public override bool HandleNull => true;

    public override ToriiAccountOnboardingResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiOnboardingJson.ReadAccountOnboardingResponse(ref reader, "account onboarding response");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiAccountOnboardingResponse value,
        JsonSerializerOptions options)
    {
        ToriiOnboardingJson.WriteAccountOnboardingResponse(writer, value, "account onboarding response");
    }
}

internal sealed class ToriiAccountFaucetResponseJsonConverter : JsonConverter<ToriiAccountFaucetResponse>
{
    public override bool HandleNull => true;

    public override ToriiAccountFaucetResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiOnboardingJson.ReadAccountFaucetResponse(ref reader, "account faucet response");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiAccountFaucetResponse value,
        JsonSerializerOptions options)
    {
        ToriiOnboardingJson.WriteAccountFaucetResponse(writer, value, "account faucet response");
    }
}

internal sealed class ToriiMultisigAccountOnboardingResponseJsonConverter :
    JsonConverter<ToriiMultisigAccountOnboardingResponse>
{
    public override bool HandleNull => true;

    public override ToriiMultisigAccountOnboardingResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiOnboardingJson.ReadMultisigAccountOnboardingResponse(
            ref reader,
            "multisig account onboarding response");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiMultisigAccountOnboardingResponse value,
        JsonSerializerOptions options)
    {
        ToriiOnboardingJson.WriteMultisigAccountOnboardingResponse(
            writer,
            value,
            "multisig account onboarding response");
    }
}
