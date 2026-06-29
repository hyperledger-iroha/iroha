using System.Text.Json;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiContractCodeBytesJson
{
    internal static void ValidateContractCodeBytesResponse(ToriiContractCodeBytesResponse response)
    {
        ArgumentNullException.ThrowIfNull(response);
        _ = response.DecodeBytes();
    }

    internal static ToriiContractCodeBytesResponse ReadContractCodeBytesResponse(
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
        string? codeBase64 = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                if (codeBase64 is null)
                {
                    throw new JsonException($"{context}.code_b64 must not be null.");
                }

                var response = new ToriiContractCodeBytesResponse { CodeBase64 = codeBase64 };
                ValidateContractCodeBytesResponse(response);
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

            if (propertyName == "code_b64")
            {
                codeBase64 = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.code_b64");
            }
            else
            {
                ToriiIdentifierJson.SkipRejectingDuplicateProperties(ref reader, $"{context}.{propertyName}");
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static void WriteContractCodeBytesResponse(
        Utf8JsonWriter writer,
        ToriiContractCodeBytesResponse response,
        string context)
    {
        ValidateContractCodeBytesResponse(response);

        writer.WriteStartObject();
        writer.WriteString("code_b64", response.CodeBase64);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiContractCodeBytesResponseJsonConverter : JsonConverter<ToriiContractCodeBytesResponse>
{
    public override bool HandleNull => true;

    public override ToriiContractCodeBytesResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiContractCodeBytesJson.ReadContractCodeBytesResponse(ref reader, "contract code-byte response");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiContractCodeBytesResponse value,
        JsonSerializerOptions options)
    {
        ToriiContractCodeBytesJson.WriteContractCodeBytesResponse(writer, value, "contract code-byte response");
    }
}
