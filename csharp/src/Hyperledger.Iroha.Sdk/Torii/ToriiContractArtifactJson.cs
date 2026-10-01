using System.Text.Json;
using System.Text.Json.Nodes;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiContractArtifactJson
{
    internal static NetworkId ReadNetworkId(JsonObject value, string context)
    {
        if (value["network_id"] is not JsonValue node || !node.TryGetValue<string>(out var literal))
        {
            throw new JsonException($"{context}.network_id is required.");
        }
        try { return NetworkId.Parse(literal); }
        catch (FormatException error) { throw new JsonException($"{context}.network_id is invalid.", error); }
    }

    internal static ContractArtifactId ReadArtifactId(JsonObject value, string context) =>
        ContractArtifactId.FromJsonNode(value["artifact_id"], $"{context}.artifact_id");

    internal static void ValidateCodeHash(ContractArtifactId artifactId, string? codeHash, string context)
    {
        if (artifactId is null || artifactId.CodeHashHex != codeHash)
        {
            throw new JsonException($"{context}.artifact_id.code_hash must exactly match code_hash.");
        }
    }

    internal static void Validate(NetworkId networkId, ContractArtifactId artifactId, string? codeHash, string context)
    {
        if (networkId is null) { throw new JsonException($"{context}.network_id is required."); }
        ValidateCodeHash(artifactId, codeHash, context);
    }

    internal static void WriteFields(Utf8JsonWriter writer, NetworkId networkId, ContractArtifactId artifactId)
    {
        writer.WriteString("network_id", networkId.ToString());
        writer.WritePropertyName("artifact_id");
        artifactId.ToJsonNode().WriteTo(writer);
    }
}
