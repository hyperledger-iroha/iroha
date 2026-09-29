using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Preservation and ownership of the public RAM-FHE profile JSON.</summary>
public sealed class ToriiRamFheProfileTests
{
    private const string InitializerHash = "abababababababababababababababababababababababababababababababab";

    [Fact]
    public void PolicyParseAndReencodePreserveInitializerAndUnsignedModulus()
    {
        var expected = Profile();
        var policy = Assert.IsType<ToriiIdentifierPolicySummary>(
            JsonSerializer.Deserialize<ToriiIdentifierPolicySummary>(PolicyJson(expected).ToJsonString()));

        AssertProfile(policy.RamFheProfile);
        var encoded = Assert.IsType<JsonObject>(JsonNode.Parse(JsonSerializer.Serialize(policy)));
        Assert.True(JsonNode.DeepEquals(expected, encoded["ram_fhe_profile"]));
        var decodedAgain = Assert.IsType<ToriiIdentifierPolicySummary>(
            JsonSerializer.Deserialize<ToriiIdentifierPolicySummary>(encoded.ToJsonString()));
        AssertProfile(decodedAgain.RamFheProfile);
    }

    [Fact]
    public void PolicyListRoundtripRetainsProfileAlongsideOptionalAbsence()
    {
        var input = new JsonObject
        {
            ["total"] = 2,
            ["items"] = new JsonArray(PolicyJson(Profile()), PolicyJson(null)),
        };
        var policies = Assert.IsType<ToriiIdentifierPoliciesResponse>(
            JsonSerializer.Deserialize<ToriiIdentifierPoliciesResponse>(input.ToJsonString()));
        Assert.Equal(2, policies.Total);
        Assert.Equal(2, policies.Items.Count);
        AssertProfile(policies.Items[0].RamFheProfile);
        Assert.Null(policies.Items[1].RamFheProfile);

        var roundtrip = Assert.IsType<ToriiIdentifierPoliciesResponse>(
            JsonSerializer.Deserialize<ToriiIdentifierPoliciesResponse>(JsonSerializer.Serialize(policies)));
        AssertProfile(roundtrip.Items[0].RamFheProfile);
        Assert.Null(roundtrip.Items[1].RamFheProfile);
    }

    [Fact]
    public void ProfileSnapshotsProtectInitializerFromInputAccessAndRecordCopyMutations()
    {
        var source = Profile();
        var policy = new ToriiIdentifierPolicySummary { RamFheProfile = source };
        source["initializer_descriptor_hash"] = new string('1', 64);
        source["min_ciphertext_modulus"] = 1;
        AssertProfile(policy.RamFheProfile);

        var access = Assert.IsType<JsonObject>(policy.RamFheProfile);
        access.Remove("initializer_descriptor_hash");
        access["min_ciphertext_modulus"] = 2;
        AssertProfile(policy.RamFheProfile);

        var recordCopy = policy with { Note = "copy" };
        var copyAccess = Assert.IsType<JsonObject>(recordCopy.RamFheProfile);
        copyAccess.Clear();
        AssertProfile(policy.RamFheProfile);
        AssertProfile(recordCopy.RamFheProfile);

        var replacement = Profile();
        replacement["initializer_descriptor_hash"] = new string('3', 64);
        var replacedCopy = policy with { RamFheProfile = replacement };
        replacement["initializer_descriptor_hash"] = new string('5', 64);
        Assert.Equal(new string('3', 64), replacedCopy.RamFheProfile!["initializer_descriptor_hash"]!.GetValue<string>());
        AssertProfile(policy.RamFheProfile);
    }

    [Fact]
    public void DuplicateInitializerIdentityIsRejectedBeforePolicyPublication()
    {
        var json = PolicyJson(Profile()).ToJsonString();
        var property = $"\"initializer_descriptor_hash\":\"{InitializerHash}\"";
        Assert.Contains(property, json);
        var malformed = json.Replace(property, $"{property},{property}", StringComparison.Ordinal);
        var error = Assert.Throws<JsonException>(() => JsonSerializer.Deserialize<ToriiIdentifierPolicySummary>(malformed));
        Assert.Contains("initializer_descriptor_hash", error.Message);
        Assert.Contains("must not appear more than once", error.Message);
    }

    private static void AssertProfile(JsonNode? value)
    {
        var profile = Assert.IsType<JsonObject>(value);
        Assert.Equal(InitializerHash, profile["initializer_descriptor_hash"]!.GetValue<string>());
        Assert.Equal(ulong.MaxValue, profile["min_ciphertext_modulus"]!.GetValue<ulong>());
        Assert.Equal("encrypted_envelope_v1", profile["encrypted_input_mode"]!.GetValue<string>());
        Assert.Equal(1, profile["profile_version"]!.GetValue<int>());
        Assert.Equal(4, profile["register_count"]!.GetValue<int>());
        Assert.Equal(32, profile["memory_lane_count"]!.GetValue<int>());
        Assert.Equal(16, profile["ciphertext_mul_per_step"]!.GetValue<int>());
        Assert.Equal(7, profile.Count);
    }

    private static JsonObject Profile() => new()
    {
        ["profile_version"] = 1,
        ["register_count"] = 4,
        ["memory_lane_count"] = 32,
        ["ciphertext_mul_per_step"] = 16,
        ["encrypted_input_mode"] = "encrypted_envelope_v1",
        ["min_ciphertext_modulus"] = ulong.MaxValue,
        ["initializer_descriptor_hash"] = InitializerHash,
    };

    private static JsonObject PolicyJson(JsonNode? profile) => new()
    {
        ["policy_id"] = "phone#retail",
        ["owner"] = "sorauﾛ1NｱｻｸYSafﾇｷヰc5ﾇﾄVxﾏ9jLZヱﾋzsKqurﾊﾘ9ｸ3eｴAｶD54TDT",
        ["active"] = true,
        ["normalization"] = "phone_e164",
        ["resolver_public_key"] = "ed25519:ed01203B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29",
        ["backend"] = "bfv-programmed-sha3-256-v1",
        ["ram_fhe_profile"] = profile,
    };
}
