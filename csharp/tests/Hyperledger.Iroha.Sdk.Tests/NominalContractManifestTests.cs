using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class NominalContractManifestTests
{
    [Fact]
    public void ExportedStructIdentitySurvivesPublicAndDurableSchemas()
    {
        const string identity = "std/math@1.0.0::Math::Receipt";
        var directory = Path.Combine(AppContext.BaseDirectory, "Fixtures", "kotodama");
        var payload = File.ReadAllText(Path.Combine(directory, "exported_structs_v1.json"));
        var vectors = JsonNode.Parse(File.ReadAllText(Path.Combine(directory, "exported_struct_names_v1.json")))!;
        foreach (var item in vectors["valid"]!.AsArray())
        {
            var name = item!.GetValue<string>();
            var manifest = JsonNode.Parse(payload.Replace(identity, name, StringComparison.Ordinal))!["manifest"]!
                .Deserialize<ToriiContractManifest>()!;
            Assert.Equal("struct " + name, manifest.Entrypoints![0].ReturnSchema!.CanonicalTypeName);
            Assert.Equal("struct " + name, manifest.Entrypoints[0].ArgumentSchema!.Fields[0].ValueType.CanonicalTypeName);
            Assert.Contains(name + "{", manifest.States![0].TypeName, StringComparison.Ordinal);
            Assert.NotNull(JsonSerializer.Deserialize<ToriiContractManifest>(JsonSerializer.Serialize(manifest)));
        }
        foreach (var item in vectors["invalid"]!.AsArray())
        {
            var name = item!.GetValue<string>();
            var root = JsonNode.Parse(payload.Replace(identity, name, StringComparison.Ordinal))!["manifest"]!.AsObject();
            var publicOnly = root.DeepClone().AsObject();
            publicOnly.Remove("states");
            Assert.Throws<JsonException>(() => publicOnly.Deserialize<ToriiContractManifest>());
            var stateOnly = root.DeepClone().AsObject();
            stateOnly.Remove("entrypoints");
            Assert.Throws<JsonException>(() => stateOnly.Deserialize<ToriiContractManifest>());
        }
    }

    private static JsonObject ManifestNode()
    {
        var fixture = File.ReadAllText(Path.Combine(
            AppContext.BaseDirectory, "Fixtures", "kotodama", "nominal_errors_v1.json"));
        return JsonNode.Parse(fixture)!["manifest"]!.AsObject();
    }

    [Fact]
    public void SharedFixturePreservesNominalErrorsUnitAndStatePages()
    {
        var manifest = ManifestNode().Deserialize<ToriiContractManifest>()!;
        Assert.Equal(2, manifest.ErrorTypes!.Count);
        Assert.Equal("example/vault@1.0.0::金庫::拒否", manifest.ErrorTypes[0].Identity);
        Assert.Equal("不足", manifest.ErrorTypes[0].Variants[0].Name);
        Assert.Equal(1U, manifest.ErrorTypes[0].Variants[0].Code);
        Assert.Equal(1U, manifest.ErrorTypes[1].Variants[0].Code);
        Assert.Equal("Result<(), example/vault@1.0.0::金庫::拒否>",
            manifest.Entrypoints![0].ReturnSchema!.CanonicalTypeName);
        Assert.Equal("Option<StateCursor<int>>", manifest.Entrypoints[1].ReturnSchema!.CanonicalTypeName);
        Assert.Equal("StatePage<int, bool, 8>", manifest.Entrypoints[2].ReturnSchema!.CanonicalTypeName);
        Assert.Equal(2, manifest.Entrypoints[2].ReturnSchema!.WordCount);
        var encoded = JsonSerializer.Serialize(manifest);
        var roundTrip = JsonSerializer.Deserialize<ToriiContractManifest>(encoded)!;
        Assert.Equal(manifest.ErrorTypes[0].Variants, roundTrip.ErrorTypes![0].Variants);
        Assert.DoesNotContain("error_codes", encoded, StringComparison.Ordinal);
    }

    [Fact]
    public void ErrorSchemasMustExactlyMatchTheirNominalCatalog()
    {
        foreach (var change in new Action<JsonObject>[]
        {
            root => root.Remove("error_types"),
            root => root["error_types"]![0]!["variants"]![0]!["name"] = "Different",
            root => root["error_types"]![0]!["identity"] = "other/vault@1.0.0::金庫::拒否",
            root => root["error_types"]!.AsArray().Add(root["error_types"]![0]!.DeepClone()),
            root => root["error_types"]![0]!["variants"]![0]!["code"] = 0,
            root => root["error_types"]![0]!["variants"]![1]!["code"] = 1,
            root => root["error_types"]![0]!["variants"]![1]!["name"] = "不足",
            root => root["error_types"]![0]!["variants"]![1]!["code"] = 4294967296UL,
        })
        {
            var root = ManifestNode();
            change(root);
            Assert.Throws<JsonException>(() => root.Deserialize<ToriiContractManifest>());
        }
        var manifest = ManifestNode().Deserialize<ToriiContractManifest>()!;
        Assert.Throws<JsonException>(() => JsonSerializer.Serialize(manifest with { ErrorTypes = [] }));
        Assert.Throws<JsonException>(() => JsonSerializer.Serialize(manifest with
        {
            Entrypoints = [],
            ErrorTypes = [],
        }));
        Assert.Throws<JsonException>(() => JsonSerializer.Serialize(manifest with
        {
            ErrorTypes = [manifest.ErrorTypes![0] with { Identity = new string('金', 342) }],
        }));
        Assert.Throws<JsonException>(() => JsonSerializer.Serialize(manifest with
        {
            ErrorTypes = [manifest.ErrorTypes![0] with
            {
                Variants = [new ToriiContractErrorVariantDescriptor { Name = "Other", Code = 1 }],
            }],
        }));
    }

    [Fact]
    public void ReservedCursorAndPageSchemasRejectForgedShapes()
    {
        foreach (var change in new Action<JsonObject>[]
        {
            root => root["entrypoints"]![0]!["return_schema"]!["nodes"]![1]!["value"] = 0,
            root => root["entrypoints"]![1]!["return_schema"]!["nodes"]![1]!["value"]!["kind"] = "Json",
            root => root["entrypoints"]![2]!["return_schema"]!["nodes"]![6]!["value"]!["kind"] = "Bool",
            root => root["entrypoints"]![2]!["return_schema"]!["nodes"]![0]!["value"]!["fields"]![1] = "next_offset",
            root => root["entrypoints"]![2]!["return_schema"]!["nodes"]![1]!["value"]!["capacity"] = 0,
            root => root["states"]![1]!["type_name"] = "Option<StateCursor<Json>>",
            root => root["states"]![2]!["type_name"] = "StatePage{items: List<(int, bool), 8>, next: Option<StateCursor<bool>>}",
        })
        {
            var root = ManifestNode();
            change(root);
            Assert.Throws<JsonException>(() => root.Deserialize<ToriiContractManifest>());
        }
    }

    [Fact]
    public void UnitHasOneZeroScalarWordAndItsSchemaPayloadIsNull()
    {
        var manifest = ManifestNode();
        manifest["entrypoints"] = JsonNode.Parse("""
            [{"name":"done","kind":{"kind":"View","value":null},"params":[],
              "return_type":"()","return_schema":{"nodes":[{"kind":"Unit","value":null}]}}]
            """);
        var decoded = manifest.Deserialize<ToriiContractManifest>()!;
        Assert.Equal(1, decoded.Entrypoints!.Single().ReturnSchema!.WordCount);
        Assert.Equal("()", decoded.Entrypoints!.Single().ReturnType);
        Assert.NotNull(JsonSerializer.Deserialize<ToriiContractManifest>(JsonSerializer.Serialize(decoded)));
        manifest["error_codes"] = new JsonArray();
        Assert.Throws<JsonException>(() => manifest.Deserialize<ToriiContractManifest>());
    }

    [Fact]
    public void EveryPublicEntrypointRequiresAnExplicitReturnSchema()
    {
        foreach (var change in new Action<JsonObject>[]
        {
            entrypoint => { entrypoint.Remove("return_type"); entrypoint.Remove("return_schema"); },
            entrypoint => { entrypoint["return_type"] = null; entrypoint["return_schema"] = null; },
            entrypoint => entrypoint.Remove("return_type"),
            entrypoint => entrypoint.Remove("return_schema"),
        })
        {
            var root = ManifestNode();
            change(root["entrypoints"]![0]!.AsObject());
            Assert.Throws<JsonException>(() => root.Deserialize<ToriiContractManifest>());
        }
        var manifest = ManifestNode().Deserialize<ToriiContractManifest>()!;
        var descriptor = manifest.Entrypoints![0];
        foreach (var invalid in new[]
        {
            descriptor with { ReturnType = null, ReturnSchema = null },
            descriptor with { ReturnType = null },
            descriptor with { ReturnSchema = null },
        })
        {
            Assert.Throws<JsonException>(() => JsonSerializer.Serialize(manifest with { Entrypoints = [invalid] }));
        }
    }

    [Fact]
    public void UnitFieldsContributeWordsInsidePublicProducts()
    {
        var manifest = ManifestNode();
        manifest["entrypoints"] = JsonNode.Parse("""
            [{"name":"done","kind":{"kind":"View","value":null},"params":[],
              "return_type":"((), int, ())","return_schema":{"nodes":[
                {"kind":"Tuple","value":3},{"kind":"Unit","value":null},
                {"kind":"Leaf","value":{"kind":"Int","value":null}},
                {"kind":"Unit","value":null}]}}]
            """);
        var decoded = manifest.Deserialize<ToriiContractManifest>()!;
        Assert.Equal(3, decoded.Entrypoints!.Single().ReturnSchema!.WordCount);
        Assert.NotNull(JsonSerializer.Deserialize<ToriiContractManifest>(JsonSerializer.Serialize(decoded)));
    }
}
