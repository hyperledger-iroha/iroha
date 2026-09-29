using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

internal static class IdentifierRequestFixtures
{
    internal const string Ciphertext = "01020304";
    internal static readonly string OpenedHash = new('f', 64);

    // DTO fixtures only: these values are not an authenticated plaintext opening.
    internal static ToriiRamLfeOutputOpening Opening() => new()
    {
        Payload = new()
        {
            ProgramId = "identifier_lookup_retail",
            InputCiphertextHash = new string('1', 64),
            OutputCiphertextHash = new string('3', 64),
            ParameterDigest = new string('5', 64),
            EvaluationKeyDigest = new string('7', 64),
            OpenedOutputHash = OpenedHash,
            OpenedAtMilliseconds = 17,
            ExpiresAtMilliseconds = 29,
        },
        Signature = new string('a', 128),
    };

    internal static ToriiIdentifierResolveRequest Request() => new()
    {
        PolicyId = "phone#retail",
        EncryptedInput = Ciphertext,
        OutputOpening = Opening(),
    };
}

/// <summary>Current encrypted request wire shape, without native account or encryption dependencies.</summary>
public sealed class ToriiIdentifierRequestTests
{
    [Fact]
    public void RequestRoundTripsExactCiphertextAndIndependentOpeningWithSourceGeneratedCodec()
    {
        var original = IdentifierRequestFixtures.Request();
        var json = JsonSerializer.Serialize(original, ToriiJsonSerializerContext.Default.ToriiIdentifierResolveRequest);
        var decoded = JsonSerializer.Deserialize(json, ToriiJsonSerializerContext.Default.ToriiIdentifierResolveRequest);
        Assert.Equal(original, decoded);
        using var document = JsonDocument.Parse(json);
        Assert.Equal(new[] { "policy_id", "encrypted_input", "output_opening" },
            document.RootElement.EnumerateObject().Select(p => p.Name));
        Assert.NotEqual(original.OutputOpening.Payload.OutputCiphertextHash,
            original.OutputOpening.Payload.OpenedOutputHash);
        Assert.DoesNotContain("+1555", json);
        Assert.False(document.RootElement.TryGetProperty("input", out _));
    }

    [Theory]
    [InlineData("policy_id")]
    [InlineData("encrypted_input")]
    [InlineData("output_opening")]
    public void RequestRejectsMissingAndNullRequiredFields(string field)
    {
        var value = RequestJson();
        value.Remove(field);
        Assert.Throws<JsonException>(() => Decode(value));
        value[field] = null;
        Assert.Throws<JsonException>(() => Decode(value));
    }

    [Theory]
    [InlineData("input")]
    [InlineData("seed")]
    [InlineData("plaintext")]
    public void RequestRejectsRetiredAndUnknownFields(string field)
    {
        var value = RequestJson();
        value[field] = "+15551234567";
        Assert.Throws<JsonException>(() => Decode(value));
    }

    [Theory]
    [InlineData("", "encrypted_input")]
    [InlineData(" abc0", "encrypted_input")]
    [InlineData("abc0 ", "encrypted_input")]
    [InlineData("ab c0", "encrypted_input")]
    [InlineData("abcd\u0001", "encrypted_input")]
    [InlineData("ABC0", "encrypted_input")]
    [InlineData("0xabc0", "encrypted_input")]
    [InlineData("abc", "encrypted_input")]
    [InlineData("ciphertext", "encrypted_input")]
    [InlineData(" phone#retail", "policy_id")]
    [InlineData("phone", "policy_id")]
    public void RequestRejectsMalformedTextOnReadAndWrite(string malformed, string field)
    {
        var value = RequestJson();
        value[field] = malformed;
        Assert.Throws<JsonException>(() => Decode(value));
        var request = IdentifierRequestFixtures.Request();
        request = field == "policy_id" ? request with { PolicyId = malformed } : request with { EncryptedInput = malformed };
        Assert.Throws<JsonException>(() => JsonSerializer.Serialize(request));
    }

    [Theory]
    [InlineData("program_id")]
    [InlineData("input_ciphertext_hash")]
    [InlineData("output_ciphertext_hash")]
    [InlineData("parameter_digest")]
    [InlineData("evaluation_key_digest")]
    [InlineData("opened_output_hash")]
    [InlineData("opened_at_ms")]
    public void OpeningRequiresAllBindingFields(string field)
    {
        var value = RequestJson();
        var payload = value["output_opening"]!["payload"]!.AsObject();
        payload.Remove(field);
        Assert.Throws<JsonException>(() => Decode(value));
        payload[field] = null;
        Assert.Throws<JsonException>(() => Decode(value));
    }

    [Theory]
    [InlineData("input_ciphertext_hash")]
    [InlineData("output_ciphertext_hash")]
    [InlineData("parameter_digest")]
    [InlineData("evaluation_key_digest")]
    [InlineData("opened_output_hash")]
    public void OpeningRejectsNonExactHashes(string field)
    {
        foreach (var malformed in new[] { new string('A', 64), new string('a', 62), "0x" + new string('a', 64), " " + new string('a', 64) })
        {
            var value = RequestJson();
            value["output_opening"]!["payload"]![field] = malformed;
            Assert.Throws<JsonException>(() => Decode(value));
        }
    }

    [Theory]
    [InlineData("\"1\"")]
    [InlineData("-1")]
    [InlineData("1.0")]
    [InlineData("1e1")]
    [InlineData("18446744073709551616")]
    public void OpeningRejectsNonU64Timestamps(string number)
    {
        var json = JsonSerializer.Serialize(IdentifierRequestFixtures.Request());
        Assert.Throws<JsonException>(() => JsonSerializer.Deserialize<ToriiIdentifierResolveRequest>(
            json.Replace("\"opened_at_ms\":17", "\"opened_at_ms\":" + number, StringComparison.Ordinal)));
    }

    [Fact]
    public void OpeningPreservesFullUnsignedRangeAndOmittedExpiry()
    {
        var request = IdentifierRequestFixtures.Request();
        request = request with
        {
            OutputOpening = request.OutputOpening with
            {
                Payload = request.OutputOpening.Payload with
                { OpenedAtMilliseconds = ulong.MaxValue, ExpiresAtMilliseconds = null },
            },
        };
        var json = JsonSerializer.Serialize(request);
        Assert.DoesNotContain("expires_at_ms", json);
        Assert.Equal(ulong.MaxValue, JsonSerializer.Deserialize<ToriiIdentifierResolveRequest>(json)!.OutputOpening.Payload.OpenedAtMilliseconds);
    }

    [Fact]
    public void RequestRejectsDuplicatesAtEveryBindingLevel()
    {
        var json = JsonSerializer.Serialize(IdentifierRequestFixtures.Request());
        foreach (var (field, literal) in new[]
        {
            ("policy_id", "\"phone#retail\""),
            ("signature", "\"" + new string('a', 128) + "\""),
            ("opened_at_ms", "17"),
        })
        {
            var original = $"\"{field}\":{literal}";
            Assert.Throws<JsonException>(() => JsonSerializer.Deserialize<ToriiIdentifierResolveRequest>(
                json.Replace(original, original + "," + original, StringComparison.Ordinal)));
        }
    }

    [Fact]
    public void RequestRejectsNullOpeningAndInvalidSignatureOnWrite()
    {
        var request = IdentifierRequestFixtures.Request();
        Assert.Throws<JsonException>(() => JsonSerializer.Serialize(request with { OutputOpening = null! }));
        Assert.Throws<JsonException>(() => JsonSerializer.Serialize(request with
            { OutputOpening = request.OutputOpening with { Signature = "ABCD" } }));
        Assert.Throws<JsonException>(() => JsonSerializer.Serialize(request with
            { OutputOpening = request.OutputOpening with { Payload = null! } }));
    }

    [Fact]
    public void OpeningRejectsUnknownFields()
    {
        var value = RequestJson();
        value["output_opening"]!["plaintext"] = "private";
        Assert.Throws<JsonException>(() => Decode(value));
        value = RequestJson();
        value["output_opening"]!["payload"]!["output_hash"] = IdentifierRequestFixtures.OpenedHash;
        Assert.Throws<JsonException>(() => Decode(value));
    }

    private static JsonObject RequestJson() => JsonSerializer.SerializeToNode(IdentifierRequestFixtures.Request())!.AsObject();
    private static ToriiIdentifierResolveRequest? Decode(JsonObject value) =>
        JsonSerializer.Deserialize<ToriiIdentifierResolveRequest>(value.ToJsonString());
}
