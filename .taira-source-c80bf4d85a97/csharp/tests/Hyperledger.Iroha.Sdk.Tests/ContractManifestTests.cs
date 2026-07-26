using System.Text.Json;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class ContractManifestTests
{
    private const string FilterBase64 =
        "TlJUMAAAl9+YQQ4oJZjALRf6FAto0QAKAAAAAAAAANzCjydU9+jNAgIAAAAFBAAAAAA=";

    [Fact]
    public void FullManifestPreservesExactKotodamaV1Interface()
    {
        var record = JsonSerializer.Deserialize<ToriiContractCodeRecord>(FullResponse())!;
        var manifest = record.Manifest;

        Assert.Equal("Ledger", manifest.SeiyakuName);
        Assert.Equal(new string('b', 64), manifest.CodeHash);
        Assert.Equal(new string('d', 64), manifest.AbiHash);
        Assert.Equal((uint)64, manifest.AccessSetHints!.DynamicReads.Single().MaxKeys);
        var entrypoint = manifest.Entrypoints!.Single();
        Assert.Equal(ToriiContractEntrypointKind.Kotoage, entrypoint.Kind);
        Assert.Equal(2, entrypoint.ArgumentSchema!.Fields[0].ValueType.WordCount);
        Assert.Equal("struct Transfer", entrypoint.ArgumentSchema.Fields[0].ValueType.CanonicalTypeName);
        Assert.Equal("List<Name, 64>", entrypoint.ArgumentSchema.Fields[1].ValueType.CanonicalTypeName);
        Assert.Equal(2, entrypoint.ArgumentSchema.Fields[1].ValueType.Nodes.Count);
        Assert.Equal(
            (byte)64,
            entrypoint.ArgumentSchema.Fields[1].ValueType.Nodes[0].ListValue!.Capacity);
        Assert.Equal(1, entrypoint.ReturnSchema!.WordCount);
        Assert.Equal("Result<(bool, int), string>", entrypoint.ReturnSchema.CanonicalTypeName);
        Assert.Equal(ToriiContractTriggerRepeatsKind.Indefinitely, entrypoint.Triggers.Single().Repeats.Kind);
        Assert.Equal("transfer", entrypoint.Triggers.Single().Callback.Entrypoint);
        Assert.Equal("daily-settlement", entrypoint.Triggers.Single().Metadata["purpose"]!.GetValue<string>());
        Assert.Equal("StateMap<AccountId, quantity>", manifest.States!.Single().TypeName);
        Assert.Equal((uint)1001, manifest.ErrorCodes!.Single().Code);
        Assert.Equal("ja", manifest.Kotoba!.Single().Translations.Last().Language);
        Assert.Equal("ed25519:fixture", manifest.Provenance!.Signer);

        var encoded = JsonSerializer.Serialize(record);
        Assert.Contains("\"seiyaku_name\":\"Ledger\"", encoded, StringComparison.Ordinal);
        Assert.Contains("hash:BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB#ABA2", encoded, StringComparison.Ordinal);
        Assert.DoesNotContain("contract_name", encoded, StringComparison.Ordinal);
        Assert.DoesNotContain("\"element\"", encoded, StringComparison.Ordinal);
    }

    [Fact]
    public void EntrypointSchemasUseOneFlatPreorderTapeAndExactReservedNames()
    {
        var views = QueryViews();
        foreach (var (name, fields, children) in views)
        {
            var viewNodes = new[] { StructNode(name, fields) }.Concat(children).ToArray();
            var (_, view) = ParseReturnSchema(viewNodes, name);
            Assert.Equal(name, view.CanonicalTypeName);

            var optionalNodes = new[] { NullNode("Option") }.Concat(viewNodes).ToArray();
            var (_, optional) = ParseReturnSchema(optionalNodes, $"Option<{name}>");
            Assert.Equal($"Option<{name}>", optional.CanonicalTypeName);

            var pageNodes = new[]
                {
                    StructNode("QueryPage", new[] { "items", "next_offset" }),
                    ListNode(64),
                }
                .Concat(viewNodes)
                .Concat(new[] { NullNode("Option"), Leaf("Int") })
                .ToArray();
            var (manifest, page) = ParseReturnSchema(pageNodes, $"QueryPage<{name}>");
            Assert.Equal($"QueryPage<{name}>", page.CanonicalTypeName);
            Assert.Equal(2, page.WordCount);

            var encoded = JsonSerializer.Serialize(manifest);
            Assert.DoesNotContain("\"element\"", encoded, StringComparison.Ordinal);
            var decodedAgain = JsonSerializer.Deserialize<ToriiContractManifest>(encoded)!;
            Assert.Equal(
                $"QueryPage<{name}>",
                decodedAgain.Entrypoints!.Single().ReturnSchema!.CanonicalTypeName);
        }

        var (_, pair) = ParseReturnSchema(
            new[]
            {
                StructNode("Pair", new[] { "left", "right" }),
                Leaf("Int"),
                Leaf("Bool"),
            },
            "struct Pair");
        Assert.Equal("struct Pair", pair.CanonicalTypeName);
    }

    [Fact]
    public void EntrypointSchemasExposeOnlyTheFirstReleaseNumericKinds()
    {
        foreach (var (wireKind, typeName) in new[]
        {
            ("Int", "int"),
            ("Decimal", "decimal"),
            ("Quantity", "quantity"),
        })
        {
            var (_, schema) = ParseReturnSchema(new[] { Leaf(wireKind) }, typeName);
            Assert.Equal(typeName, schema.CanonicalTypeName);
        }

        foreach (var retired in new[] { "U128", "Amount" })
        {
            AssertSchemaRejected(new[] { Leaf(retired) }, retired, "unsupported");
        }
    }

    [Fact]
    public void EntrypointSchemasRejectLegacyTruncatedDeepAndForgedTapes()
    {
        foreach (var (name, fields, children) in QueryViews())
        {
            var wrongFields = fields.ToArray();
            wrongFields[^1] = "forged";
            AssertForgedSchemaRejected(
                new[] { StructNode(name, wrongFields) }.Concat(children),
                name);

            var wrongLeaf = children.ToArray();
            wrongLeaf[^1] = Leaf("Blob");
            AssertForgedSchemaRejected(
                new[] { StructNode(name, fields) }.Concat(wrongLeaf),
                name);

            var viewNodes = new[] { StructNode(name, fields) }.Concat(children);
            AssertForgedSchemaRejected(
                new[]
                    {
                        StructNode("QueryPage", new[] { "items", "next_offset" }),
                        ListNode(64),
                    }
                    .Concat(viewNodes)
                    .Concat(new[] { NullNode("Option"), Leaf("String") }),
                $"QueryPage<{name}>");
            AssertForgedSchemaRejected(
                new[]
                    {
                        StructNode("QueryPage", new[] { "items", "next_offset" }),
                        ListNode(32),
                    }
                    .Concat(viewNodes)
                    .Concat(new[] { NullNode("Option"), Leaf("Int") }),
                $"QueryPage<{name}>");
        }

        AssertSchemaRejected(
            new[]
            {
                """
                {"kind":"List","value":{"capacity":64,"element":{"nodes":[{"kind":"Leaf","value":{"kind":"Name","value":null}}]}}}
                """,
            },
            "List<Name, 64>",
            "unknown field");
        AssertSchemaRejected(new[] { ListNode(64) }, "List<int, 64>", "ends before");
        AssertSchemaRejected(
            new[] { Leaf("Bool"), Leaf("Bool") },
            "bool",
            "complete canonical prefix type tree");
        AssertSchemaRejected(
            new[] { ListNode(0), Leaf("Int") },
            "List<int, 0>",
            "capacity");
        AssertSchemaRejected(
            new[] { ListNode(65), Leaf("Int") },
            "List<int, 65>",
            "capacity");

        var forgedForEncoding = new ToriiEntrypointValueTypeV1
        {
            Nodes = new[]
            {
                new ToriiEntrypointValueTypeNodeV1
                {
                    Kind = ToriiEntrypointValueTypeNodeKindV1.Struct,
                    StructValue = new ToriiEntrypointStructTypeNodeV1
                    {
                        Name = "AccountView",
                        Fields = new[] { "id", "metadata" },
                    },
                },
                new ToriiEntrypointValueTypeNodeV1
                {
                    Kind = ToriiEntrypointValueTypeNodeKindV1.Leaf,
                    LeafKind = ToriiEntrypointValueKindV1.AccountId,
                },
                new ToriiEntrypointValueTypeNodeV1
                {
                    Kind = ToriiEntrypointValueTypeNodeKindV1.Leaf,
                    LeafKind = ToriiEntrypointValueKindV1.Blob,
                },
            },
            WordCount = 2,
            CanonicalTypeName = "AccountView",
        };
        var forgedManifest = new ToriiContractManifest
        {
            Entrypoints = new[]
            {
                new ToriiContractEntrypointDescriptor
                {
                    Name = "inspect",
                    Kind = ToriiContractEntrypointKind.View,
                    ReturnType = "AccountView",
                    ReturnSchema = forgedForEncoding,
                    AccessHintsComplete = true,
                },
            },
        };
        var encodingError = Assert.Throws<JsonException>(
            () => JsonSerializer.Serialize(forgedManifest));
        Assert.Contains("forged", encodingError.Message, StringComparison.OrdinalIgnoreCase);

        var atLimitNodes = Enumerable.Repeat(ListNode(1), 255)
            .Append(Leaf("Int"))
            .ToArray();
        var atLimitName = "int";
        for (var depth = 0; depth < 255; depth++)
        {
            atLimitName = $"List<{atLimitName}, 1>";
        }
        var (_, atLimit) = ParseReturnSchema(atLimitNodes, atLimitName);
        Assert.Equal(1, atLimit.WordCount);

        AssertSchemaRejected(
            Enumerable.Repeat(ListNode(1), 256).Append(Leaf("Int")),
            atLimitName,
            "1..256");
    }

    [Fact]
    public void ManifestRejectsUnknownEnglishAndNoncanonicalShapes()
    {
        var response = FullResponse();
        var invalid = new[]
        {
            ReplaceFirst(response, "\"seiyaku_name\"", "\"contract_name\""),
            ReplaceFirst(response, "\"Kotoage\"", "\"Public\""),
            ReplaceFirst(response, "\"Kotoage\"", "\"View\""),
            ReplaceFirst(response, "\"capacity\":64", "\"capacity\":65"),
            ReplaceFirst(response, "\"name\":\"request\",\"ty\"", "\"name\":\"different\",\"ty\""),
            ReplaceFirst(response, "#ABA2", "#0000"),
            ReplaceFirst(response, "\"seiyaku_name\":\"Ledger\"", "\"seiyaku_name\":\"match\""),
            ReplaceFirst(response, "\"seiyaku_name\":\"Ledger\"", "\"seiyaku_name\":\"Option\""),
            ReplaceFirst(response, "\"seiyaku_name\":\"Ledger\"", "\"seiyaku_name\":\"Amount\""),
            ReplaceFirst(response, "\"seiyaku_name\":\"Ledger\"", "\"seiyaku_name\":\"amount\""),
            ReplaceFirst(response, "\"seiyaku_name\":\"Ledger\"", "\"seiyaku_name\":\"__kotodama_link_private\""),
            ReplaceFirst(response, "\"seiyaku_name\":\"Ledger\"", "\"seiyaku_name\":\"state_map_get\""),
            ReplaceFirst(response, "\"namespace\":\"TransferError\"", "\"namespace\":\"Option\""),
            ReplaceFirst(response, "\"features_bitmap\":0", "\"features_bitmap\":4"),
            ReplaceFirst(response, "\"dynamic_writes\":[]", "\"dynamic_writes\":[],\"unknown\":true"),
            ReplaceFirst(
                response,
                "\"repeats\":{\"Indefinitely\":null}",
                "\"repeats\":{\"kind\":\"Indefinitely\",\"value\":null}"),
            ReplaceFirst(
                response,
                $"\"code_hash\":\"{new string('b', 64)}\"",
                $"\"code_hash\":\"{new string('f', 64)}\""),
        };

        foreach (var payload in invalid)
        {
            Assert.Throws<JsonException>(() => JsonSerializer.Deserialize<ToriiContractCodeRecord>(payload));
        }
    }

    private static string ReplaceFirst(string source, string target, string replacement)
    {
        var index = source.IndexOf(target, StringComparison.Ordinal);
        Assert.True(index >= 0, $"missing test replacement target {target}");
        return source[..index] + replacement + source[(index + target.Length)..];
    }

    private static (ToriiContractManifest Manifest, ToriiEntrypointValueTypeV1 Schema)
        ParseReturnSchema(IEnumerable<string> nodes, string typeName)
    {
        var manifest = JsonSerializer.Deserialize<ToriiContractManifest>(
            ManifestWithReturnSchema(nodes, typeName))!;
        return (manifest, manifest.Entrypoints!.Single().ReturnSchema!);
    }

    private static void AssertForgedSchemaRejected(
        IEnumerable<string> nodes,
        string advertisedType)
    {
        AssertSchemaRejected(nodes, advertisedType, "forged");
    }

    private static void AssertSchemaRejected(
        IEnumerable<string> nodes,
        string advertisedType,
        string expectedMessage)
    {
        var exception = Assert.Throws<JsonException>(
            () => JsonSerializer.Deserialize<ToriiContractManifest>(
                ManifestWithReturnSchema(nodes, advertisedType)));
        Assert.Contains(expectedMessage, exception.Message, StringComparison.OrdinalIgnoreCase);
    }

    private static string ManifestWithReturnSchema(
        IEnumerable<string> nodes,
        string typeName)
    {
        return $$$"""
        {
          "entrypoints":[{
            "name":"inspect",
            "kind":{"kind":"View","value":null},
            "params":[],
            "argument_schema":null,
            "return_type":{{{JsonSerializer.Serialize(typeName)}}},
            "return_schema":{"nodes":[{{{string.Join(",", nodes)}}}]},
            "permission":null,
            "read_keys":[],
            "write_keys":[],
            "access_hints_complete":true,
            "access_hints_skipped":[],
            "triggers":[]
          }]
        }
        """;
    }

    private static IReadOnlyList<(string Name, string[] Fields, string[] Children)> QueryViews()
    {
        return new (string, string[], string[])[]
        {
            (
                "AccountView",
                new[] { "id", "metadata" },
                new[] { Leaf("AccountId"), Leaf("Json") }),
            (
                "AssetView",
                new[] { "id", "amount" },
                new[] { Leaf("AssetId"), Leaf("Quantity") }),
            (
                "AssetDefinitionView",
                new[] { "id", "name", "description", "owned_by", "total_quantity", "metadata" },
                new[]
                {
                    Leaf("AssetDefinitionId"),
                    Leaf("String"),
                    NullNode("Option"),
                    Leaf("String"),
                    Leaf("AccountId"),
                    Leaf("Quantity"),
                    Leaf("Json"),
                }),
            (
                "DomainView",
                new[] { "id", "owned_by", "metadata" },
                new[] { Leaf("DomainId"), Leaf("AccountId"), Leaf("Json") }),
            (
                "NftView",
                new[] { "id", "owned_by", "content" },
                new[] { Leaf("NftId"), Leaf("AccountId"), Leaf("Json") }),
        };
    }

    private static string StructNode(string name, IEnumerable<string> fields)
    {
        return $$$"""
        {"kind":"Struct","value":{"name":{{{JsonSerializer.Serialize(name)}}},"fields":[{{{string.Join(",", fields.Select(field => JsonSerializer.Serialize(field)))}}}]}}
        """;
    }

    private static string ListNode(byte capacity)
    {
        return $$$"""{"kind":"List","value":{"capacity":{{{capacity}}}}}""";
    }

    private static string NullNode(string kind)
    {
        return $$$"""{"kind":{{{JsonSerializer.Serialize(kind)}}},"value":null}""";
    }

    private static string Leaf(string kind)
    {
        return $$$"""{"kind":"Leaf","value":{"kind":{{{JsonSerializer.Serialize(kind)}}},"value":null}}""";
    }

    private static string FullResponse()
    {
        return $$$"""
        {
          "manifest":{
            "seiyaku_name":"Ledger",
            "code_hash":"hash:BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB#ABA2",
            "abi_hash":"hash:DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD#F071",
            "compiler_fingerprint":"kotodama_lang",
            "features_bitmap":0,
            "access_set_hints":{
              "read_keys":["state:Balances"],
              "write_keys":["state:Balances"],
              "dynamic_reads":[{
                "base_key":"state:Balances",
                "key_type":"AccountId",
                "bound_kind":"take",
                "max_keys":64
              }],
              "dynamic_writes":[]
            },
            "entrypoints":[{
              "name":"transfer",
              "kind":{"kind":"Kotoage","value":null},
              "params":[
                {"name":"request","type_name":"struct Transfer"},
                {"name":"tags","type_name":"List<Name, 64>"}
              ],
              "argument_schema":{"fields":[
                {"name":"request","ty":{"nodes":[
                  {"kind":"Struct","value":{"name":"Transfer","fields":["amount","memo"]}},
                  {"kind":"Leaf","value":{"kind":"Quantity","value":null}},
                  {"kind":"Option","value":null},
                  {"kind":"Leaf","value":{"kind":"String","value":null}}
                ]}},
                {"name":"tags","ty":{"nodes":[
                  {"kind":"List","value":{"capacity":64}},
                  {"kind":"Leaf","value":{"kind":"Name","value":null}}
                ]}}
              ]},
              "return_type":"Result<(bool, int), string>",
              "return_schema":{"nodes":[
                {"kind":"Result","value":null},
                {"kind":"Tuple","value":2},
                {"kind":"Leaf","value":{"kind":"Bool","value":null}},
                {"kind":"Leaf","value":{"kind":"Int","value":null}},
                {"kind":"Leaf","value":{"kind":"String","value":null}}
              ]},
              "permission":"TransferAsset",
              "read_keys":["state:Balances"],
              "write_keys":["state:Balances"],
              "access_hints_complete":true,
              "access_hints_skipped":[],
              "triggers":[{
                "id":"settle",
                "repeats":{"Indefinitely":null},
                "filter":"{{{FilterBase64}}}",
                "authority":null,
                "metadata":{"purpose":"daily-settlement","round":7},
                "callback":{"namespace":null,"entrypoint":"transfer"}
              }]
            }],
            "states":[{"name":"Balances","type_name":"StateMap<AccountId, quantity>"}],
            "error_codes":[{"namespace":"TransferError","name":"InsufficientFunds","code":1001}],
            "kotoba":[{
              "msg_id":"transfer.denied",
              "translations":[
                {"lang":"en","text":"Transfer denied"},
                {"lang":"ja","text":"送金は拒否されました"}
              ]
            }],
            "provenance":{"signer":"ed25519:fixture","signature":"fixture-signature"}
          },
          "code_hash":"{{{new string('b', 64)}}}",
          "abi_hash":"{{{new string('d', 64)}}}"
        }
        """;
    }
}
