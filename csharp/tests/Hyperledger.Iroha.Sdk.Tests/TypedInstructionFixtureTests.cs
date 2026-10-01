using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class TypedInstructionFixtureTests
{
    [Theory]
    [InlineData("TransferAssetDefinition")]
    [InlineData("TransferNft")]
    [InlineData("SetAccountKeyValue")]
    [InlineData("RemoveAccountKeyValue")]
    [InlineData("SetAssetDefinitionKeyValue")]
    [InlineData("RemoveAssetDefinitionKeyValue")]
    [InlineData("SetNftKeyValue")]
    [InlineData("RemoveNftKeyValue")]
    [InlineData("SetTriggerKeyValue")]
    [InlineData("RemoveTriggerKeyValue")]
    [InlineData("MintTriggerRepetitions")]
    [InlineData("BurnTriggerRepetitions")]
    [InlineData("ExecuteTrigger")]
    [InlineData("SetAssetKeyValue")]
    [InlineData("CustomInstruction")]
    public void TypedInstructionsMatchNativeFrames(string operation)
    {
        using var fixture = LoadFixture();
        var root = fixture.RootElement;
        string Text(string name) => root.GetProperty(name).GetString()!;
        var cases = root.GetProperty("cases").EnumerateArray().ToArray();
        Assert.Equal(15, cases.Length);
        Assert.Equal(15, cases.Select(item => item.GetProperty("name").GetString()).Distinct().Count());
        var item = Assert.Single(cases, candidate => candidate.GetProperty("name").GetString() == operation);
        var authority = Text("authority");
        var destination = Text("destination");
        var definition = Text("asset_definition_id");
        var nft = Text("nft_id");
        var trigger = Text("trigger_id");
        var key = Text("metadata_key");
        var value = JsonValue.Create(Text("metadata_value"));
        TransactionInstruction instruction = operation switch
        {
            "TransferAssetDefinition" => TransactionInstruction.TransferAssetDefinition(definition, destination),
            "TransferNft" => TransactionInstruction.TransferNft(nft, destination),
            "SetAccountKeyValue" => TransactionInstruction.SetAccountKeyValue(authority, key, value),
            "RemoveAccountKeyValue" => TransactionInstruction.RemoveAccountKeyValue(authority, key),
            "SetAssetDefinitionKeyValue" => TransactionInstruction.SetAssetDefinitionKeyValue(definition, key, value),
            "RemoveAssetDefinitionKeyValue" => TransactionInstruction.RemoveAssetDefinitionKeyValue(definition, key),
            "SetNftKeyValue" => TransactionInstruction.SetNftKeyValue(nft, key, value),
            "RemoveNftKeyValue" => TransactionInstruction.RemoveNftKeyValue(nft, key),
            "SetTriggerKeyValue" => TransactionInstruction.SetTriggerKeyValue(trigger, key, value),
            "RemoveTriggerKeyValue" => TransactionInstruction.RemoveTriggerKeyValue(trigger, key),
            "MintTriggerRepetitions" => TransactionInstruction.MintTriggerRepetitions(7, trigger),
            "BurnTriggerRepetitions" => TransactionInstruction.BurnTriggerRepetitions(3, trigger),
            "ExecuteTrigger" => TransactionInstruction.ExecuteTrigger(trigger, new JsonObject { ["force"] = true }),
            "SetAssetKeyValue" => TransactionInstruction.SetAssetKeyValue(definition, authority, key, value),
            "CustomInstruction" => new FixtureCustomInstruction(new JsonObject { ["force"] = true }),
            _ => throw new InvalidOperationException("Unexpected native operation."),
        };
        var actual = instruction.EncodeInstructionBox(authority);
        Assert.Equal(Convert.FromHexString(item.GetProperty("instruction_box_frame_hex").GetString()!), actual);
        var (payload, flags) = NoritoCodec.DecodeWithSchemaHash(actual.AsSpan(6, 16), actual);
        Assert.Equal(0x02, flags);
        Assert.Equal(Convert.FromHexString(item.GetProperty("instruction_box_payload_hex").GetString()!), payload);
    }

    [Fact]
    public void SharedJsonAndMetadataMatchNativePayloads()
    {
        using var fixture = LoadFixture();
        var root = fixture.RootElement;
        var context = new TransactionEncodingContext(root.GetProperty("authority").GetString()!);
        var cases = root.GetProperty("json_values").EnumerateArray().ToArray();
        Assert.Equal(3, cases.Length);
        var metadata = new Dictionary<string, JsonNode?>();
        for (var index = 0; index < cases.Length; index++)
        {
            var item = cases[index];
            var value = JsonNode.Parse(item.GetProperty("value").GetRawText());
            Assert.Equal(Convert.FromHexString(item.GetProperty("payload_hex").GetString()!), context.EncodeJson(value));
            metadata.Add("key_" + index, value);
        }
        Assert.Equal(Convert.FromHexString(root.GetProperty("metadata_payload_hex").GetString()!), context.EncodeMetadata(metadata));
    }

    [Theory]
    [InlineData("dragon$universal")]
    [InlineData("dragon$banka.UNIVERSAL")]
    [InlineData("dragon$BANKA.universal")]
    [InlineData("dragon$xn--a.universal")]
    [InlineData("dragon$bücher.universal")]
    [InlineData("dragon$banka.universal.extra")]
    [InlineData("dragon$banka.universal$")]
    [InlineData("$banka.universal")]
    public void NftInstructionsRequireExactFullyQualifiedDomains(string nft)
    {
        using var fixture = LoadFixture();
        var authority = fixture.RootElement.GetProperty("authority").GetString()!;
        Assert.Throws<ArgumentException>(() => TransactionInstruction.TransferNft(nft, authority));
        Assert.Throws<ArgumentException>(() => TransactionInstruction.SetNftKeyValue(nft, "memo", JsonValue.Create("value")));
        Assert.Throws<ArgumentException>(() => TransactionInstruction.RemoveNftKeyValue(nft, "memo"));
        var valid = fixture.RootElement.GetProperty("nft_id").GetString()!;
        var transfer = TransactionInstruction.TransferNft(valid, authority);
        var set = TransactionInstruction.SetNftKeyValue(valid, "memo", JsonValue.Create("value"));
        var remove = TransactionInstruction.RemoveNftKeyValue(valid, "memo");
        Assert.Throws<ArgumentException>(() => transfer with { NftId = nft });
        Assert.Throws<ArgumentException>(() => set with { NftId = nft });
        Assert.Throws<ArgumentException>(() => remove with { NftId = nft });
    }

    private static JsonDocument LoadFixture()
    {
        var fixture = JsonDocument.Parse(File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Fixtures", "typed_transaction_v1.json")));
        var root = fixture.RootElement;
        Assert.Equal(1, root.GetProperty("fixture_version").GetInt32());
        Assert.Equal(1, root.GetProperty("norito_layout_version").GetInt32());
        Assert.Equal(0x02, root.GetProperty("norito_layout_flags").GetInt32());
        Assert.Equal("iroha_data_model/examples/typed_transaction_fixture.rs", root.GetProperty("generator").GetString());
        return fixture;
    }

    private sealed record class FixtureCustomInstruction(JsonNode Payload) : TransactionInstruction
    {
        internal override string WireId => "iroha.custom";
        internal override string TypeName => "iroha_data_model::isi::transparent::CustomInstruction";
        internal override byte[] EncodePayload(TransactionEncodingContext context)
        {
            var writer = new CanonicalNoritoWriter();
            writer.WriteField(context.EncodeJson(Payload));
            return writer.ToArray();
        }
    }
}
