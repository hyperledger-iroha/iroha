using System.Text.Json;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiContractCodeBytesJson
{
    internal static void ValidateContractCodeBytesResponse(ToriiContractCodeBytesResponse response)
    {
        ArgumentNullException.ThrowIfNull(response);
        if (response.NetworkId is null || response.ArtifactId is null)
        {
            throw new JsonException("contract code-byte response requires network_id and artifact_id.");
        }
        _ = response.DecodeBytes();
    }

    internal static ToriiContractCodeBytesResponse ReadContractCodeBytesResponse(
        ref Utf8JsonReader reader,
        string context)
    {
        var payload = ToriiExplorerJson.ReadObject(ref reader, context);
        if (payload["code_b64"] is null)
            throw new JsonException($"{context}.code_b64 must not be null.");
        if (payload["code_b64"] is not System.Text.Json.Nodes.JsonValue codeNode
            || !codeNode.TryGetValue<string>(out var codeBase64))
            throw new JsonException($"{context}.code_b64 must be a string.");
        if (payload.Count != 3)
        {
            throw new JsonException($"{context} requires exactly network_id, artifact_id, and code_b64.");
        }
        var response = new ToriiContractCodeBytesResponse
        {
            NetworkId = ToriiContractArtifactJson.ReadNetworkId(payload, context),
            ArtifactId = ToriiContractArtifactJson.ReadArtifactId(payload, context),
            CodeBase64 = codeBase64,
        };
        ValidateContractCodeBytesResponse(response);
        return response;
    }

    internal static void WriteContractCodeBytesResponse(
        Utf8JsonWriter writer,
        ToriiContractCodeBytesResponse response,
        string context)
    {
        ValidateContractCodeBytesResponse(response);

        writer.WriteStartObject();
        ToriiContractArtifactJson.WriteFields(writer, response.NetworkId, response.ArtifactId);
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
