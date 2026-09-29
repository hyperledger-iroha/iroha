using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Preserves required program and independent opening-authority bindings in policy metadata.</summary>
public sealed class ToriiIdentifierPolicyBindingTests
{
    [Fact]
    public void PolicyRoundTripsExplicitBindingsWithSourceGeneratedCodec()
    {
        var source = ValidPolicy();
        var policy = JsonSerializer.Deserialize(source.ToJsonString(),
            ToriiJsonSerializerContext.Default.ToriiIdentifierPolicySummary)!;
        Assert.Equal("identifier_lookup_retail", policy.ProgramId);
        Assert.Equal(source["output_opening_public_key"]!.GetValue<string>(), policy.OutputOpeningPublicKey);
        Assert.NotEqual(policy.ResolverPublicKey, policy.OutputOpeningPublicKey);
        var encoded = JsonSerializer.Serialize(policy, ToriiJsonSerializerContext.Default.ToriiIdentifierPolicySummary);
        Assert.Equal(policy, JsonSerializer.Deserialize(encoded, ToriiJsonSerializerContext.Default.ToriiIdentifierPolicySummary));
    }

    [Theory]
    [InlineData("program_id")]
    [InlineData("output_opening_public_key")]
    public void PolicyRejectsMissingNullAndWrongTypeBindings(string field)
    {
        var source = ValidPolicy();
        source.Remove(field);
        Assert.Throws<JsonException>(() => Decode(source));
        source[field] = null;
        Assert.Throws<JsonException>(() => Decode(source));
        source[field] = 7;
        Assert.Throws<JsonException>(() => Decode(source));
        source[field] = new JsonObject();
        Assert.Throws<JsonException>(() => Decode(source));
    }

    [Theory]
    [InlineData("")]
    [InlineData(" ")]
    [InlineData(" padded")]
    [InlineData("padded ")]
    [InlineData("embedded space")]
    [InlineData("embedded\u00a0space")]
    [InlineData("control\u0001")]
    public void PolicyRejectsNonExactBindingsOnReadAndWrite(string invalid)
    {
        var valid = Decode(ValidPolicy())!;
        foreach (var field in new[] { "program_id", "output_opening_public_key" })
        {
            var source = ValidPolicy();
            source[field] = invalid;
            var error = Assert.Throws<JsonException>(() => Decode(source));
            Assert.Contains(field, error.Message);
            var malformed = field == "program_id"
                ? valid with { ProgramId = invalid }
                : valid with { OutputOpeningPublicKey = invalid };
            Assert.Throws<JsonException>(() => JsonSerializer.Serialize(malformed));
        }
    }

    [Theory]
    [InlineData("program_id")]
    [InlineData("output_opening_public_key")]
    public void PolicyRejectsDuplicateBindings(string field)
    {
        var source = ValidPolicy();
        var value = source[field]!.ToJsonString();
        var json = source.ToJsonString();
        var original = $"\"{field}\":{value}";
        var duplicate = json.Replace(original, original + "," + original, StringComparison.Ordinal);
        Assert.Throws<JsonException>(() => JsonSerializer.Deserialize<ToriiIdentifierPolicySummary>(duplicate));
    }

    [Fact]
    public void PolicyRejectsNullBindingsOnWrite()
    {
        var valid = Decode(ValidPolicy())!;
        Assert.Throws<JsonException>(() => JsonSerializer.Serialize(valid with { ProgramId = null! }));
        Assert.Throws<JsonException>(() => JsonSerializer.Serialize(valid with { OutputOpeningPublicKey = null! }));
    }

    private static ToriiIdentifierPolicySummary? Decode(JsonObject source) =>
        JsonSerializer.Deserialize<ToriiIdentifierPolicySummary>(source.ToJsonString());

    private static JsonObject ValidPolicy() => new()
    {
        ["policy_id"] = "email#retail",
        ["program_id"] = "identifier_lookup_retail",
        ["owner"] = "sorauﾛ1NｱｻｸYSafﾇｷヰc5ﾇﾄVxﾏ9jLZヱﾋzsKqurﾊﾘ9ｸ3eｴAｶD54TDT",
        ["active"] = true,
        ["normalization"] = "email_address",
        ["resolver_public_key"] = "ed012043046BFE4092B3E94994EADA15DCC20D8AAA07B658FD3954EB8E0EFB8BDCA5DE",
        ["output_opening_public_key"] = "ed01208FC2E4882B20ABCCBFADB4E44268206E187AEB235A51252F159B3B24D5BB6661",
        ["backend"] = "bfv-programmed-v1",
    };
}
