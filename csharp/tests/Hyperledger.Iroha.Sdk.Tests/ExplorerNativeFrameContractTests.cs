using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class ExplorerNativeFrameContractTests
{
    // Shape and integrity fixture only; this is not an authenticated native frame.
    private const string Instruction = "AQIDBA==";
    private const string Digest = "9f64a747e1b97f131fabb6b447296c9b6f0201e79fb3c5356e6c77e89b6a806a";

    private static JsonObject Box() => new()
    {
        ["wire_id"] = "iroha.set_key_value",
        ["framed_sha256"] = Digest,
        ["instruction"] = Instruction,
    };

    [Fact]
    public void InstructionEnvelopeRoundTripsOnlyItsThreeNativeFields()
    {
        var box = JsonSerializer.Deserialize<ToriiExplorerInstructionBox>(Box().ToJsonString())!;
        Assert.Equal(Instruction, box.Instruction);
        Assert.Equal(Digest, box.FramedSha256);
        Assert.Equal("iroha.set_key_value", box.WireId);
        using var serialized = JsonDocument.Parse(JsonSerializer.Serialize(box));
        Assert.Equal(new[] { "wire_id", "framed_sha256", "instruction" },
            serialized.RootElement.EnumerateObject().Select(property => property.Name));
    }

    [Theory]
    [InlineData("instruction", "AQ")]
    [InlineData("instruction", "AR==")]
    [InlineData("instruction", "AQID BA==")]
    [InlineData("instruction", "")]
    [InlineData("framed_sha256", "0000000000000000000000000000000000000000000000000000000000000000")]
    [InlineData("framed_sha256", "9F64A747E1B97F131FABB6B447296C9B6F0201E79FB3C5356E6C77E89B6A806A")]
    [InlineData("wire_id", " iroha.set_key_value")]
    public void InstructionEnvelopeRejectsMalformedOrSubstitutedFrame(string field, string value)
    {
        var box = Box();
        box[field] = value;
        var error = Assert.Throws<JsonException>(() =>
            JsonSerializer.Deserialize<ToriiExplorerInstructionBox>(box.ToJsonString()));
        Assert.Contains(field, error.Message);
    }

    [Theory]
    [InlineData("encoded")]
    [InlineData("json")]
    public void InstructionEnvelopeRejectsRetiredDuplicateTrees(string field)
    {
        var box = Box();
        box[field] = "retired";
        Assert.Throws<JsonException>(() =>
            JsonSerializer.Deserialize<ToriiExplorerInstructionBox>(box.ToJsonString()));
    }

    [Fact]
    public void RejectionEnvelopeHasOneNativeReasonAndPublicMessage()
    {
        const string json = "{\"reason\":\"AQ==\",\"message\":\"validation failed\"}";
        var reason = JsonSerializer.Deserialize<ToriiExplorerTransactionRejection>(json)!;
        Assert.Equal("AQ==", reason.Reason);
        Assert.Equal("validation failed", reason.Message);
        Assert.Equal(json, JsonSerializer.Serialize(reason));
        Assert.Throws<JsonException>(() => JsonSerializer.Deserialize<ToriiExplorerTransactionRejection>(
            "{\"encoded\":\"0x01\",\"json\":{},\"message\":\"validation failed\"}"));
    }
}
