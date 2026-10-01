using System.Globalization;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha;

/// <summary>An immutable complete contract-artifact hash in one explicit dataspace.</summary>
[JsonConverter(typeof(ContractArtifactIdJsonConverter))]
public sealed record ContractArtifactId
{
    /// <summary>Bind a canonical lowercase, marked 64-hex artifact hash to its exact dataspace.</summary>
    public ContractArtifactId(ulong dataspaceId, string codeHash)
    {
        if (string.IsNullOrWhiteSpace(codeHash))
            throw new ArgumentException("codeHash must not be null or whitespace.", nameof(codeHash));
        if (codeHash.Any(char.IsControl))
            throw new ArgumentException("codeHash must not contain control characters.", nameof(codeHash));
        if (codeHash.Any(char.IsWhiteSpace))
            throw new ArgumentException("codeHash must not contain whitespace.", nameof(codeHash));
        if (codeHash.Length != 64 || !codeHash.All(Uri.IsHexDigit))
            throw new ArgumentException("codeHash must be a 32-byte hex string.", nameof(codeHash));
        try
        {
            CanonicalHashLiteral.ValidateHex(codeHash, nameof(codeHash));
        }
        catch (JsonException error)
        {
            throw new ArgumentException(error.Message, nameof(codeHash), error);
        }
        DataspaceId = dataspaceId;
        CodeHashHex = codeHash;
    }

    public ulong DataspaceId { get; }
    public string CodeHashHex { get; }

    internal string Route => $"/v1/contracts/artifacts/{DataspaceId.ToString(CultureInfo.InvariantCulture)}/{CodeHashHex}";

    internal JsonObject ToJsonNode() => new()
    {
        ["dataspace_id"] = DataspaceId,
        ["code_hash"] = CanonicalHashLiteral.Format(CodeHashHex, "artifact_id.code_hash"),
    };

    internal static ContractArtifactId FromJsonNode(JsonNode? node, string context)
    {
        if (node is not JsonObject value || value.Count != 2
            || value["dataspace_id"] is not JsonValue dataspaceNode
            || !dataspaceNode.TryGetValue<ulong>(out var dataspace)
            || value["code_hash"] is not JsonValue hashNode
            || !hashNode.TryGetValue<string>(out var hash))
        {
            throw new JsonException($"{context} requires exactly an unsigned dataspace_id and a canonical code_hash.");
        }
        return new ContractArtifactId(dataspace, CanonicalHashLiteral.Parse(hash, $"{context}.code_hash"));
    }
}

internal sealed class ContractArtifactIdJsonConverter : JsonConverter<ContractArtifactId>
{
    public override bool HandleNull => true;

    public override ContractArtifactId Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
        ContractArtifactId.FromJsonNode(Torii.ToriiExplorerJson.ReadObject(ref reader, "artifact_id"), "artifact_id");

    public override void Write(Utf8JsonWriter writer, ContractArtifactId value, JsonSerializerOptions options)
    {
        ArgumentNullException.ThrowIfNull(value);
        value.ToJsonNode().WriteTo(writer);
    }
}
