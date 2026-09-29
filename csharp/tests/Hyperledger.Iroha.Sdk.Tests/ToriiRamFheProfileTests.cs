using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Typed, immutable and strictly decoded public RAM-FHE profiles.</summary>
public sealed class ToriiRamFheProfileTests
{
    private const string InitializerHash = "abababababababababababababababababababababababababababababababab";

    [Fact]
    public async Task ExistingIdentifierPoliciesEndpointReturnsTypedProfileAndRejectsMalformedProfile()
    {
        foreach (var malformed in new[] { false, true })
        {
            var profile = Profile();
            if (malformed)
            {
                profile["initializer_descriptor_hash"] = new string('a', 64);
            }
            var payload = new JsonObject
            {
                ["total"] = 1,
                ["items"] = new JsonArray(PolicyJson(profile)),
            };
            using var handler = new PolicyHandler(payload.ToJsonString());
            using var http = new HttpClient(handler);
            using var client = new ToriiClient(new Uri("https://torii.example"), http);
            if (malformed)
            {
                await Assert.ThrowsAsync<JsonException>(() => client.GetIdentifierPoliciesAsync(TestContext.Current.CancellationToken));
            }
            else
            {
                var response = await client.GetIdentifierPoliciesAsync(TestContext.Current.CancellationToken);
                AssertProfile(Assert.Single(response.Items).RamFheProfile);
            }
            Assert.Equal(1, handler.Requests);
        }
    }

    private sealed class PolicyHandler(string payload) : HttpMessageHandler
    {
        public int Requests { get; private set; }

        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        {
            Assert.Equal(HttpMethod.Get, request.Method);
            Assert.Equal("/v1/identifier-policies", request.RequestUri!.AbsolutePath);
            Requests++;
            return Task.FromResult(new HttpResponseMessage(System.Net.HttpStatusCode.OK)
            {
                Content = new StringContent(payload, System.Text.Encoding.UTF8, "application/json"),
            });
        }
    }

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
    public void TypedProfileIsImmutableAcrossPolicyRecordCopies()
    {
        var source = TypedProfile();
        var policy = new ToriiIdentifierPolicySummary { RamFheProfile = source };
        var copy = policy with { Note = "copy" };
        Assert.Same(source, copy.RamFheProfile);
        var replacement = new ToriiRamFheProfile(1, 4, 32, 16,
            ToriiRamFheEncryptedInputMode.EncryptedEnvelopeV1, 257, new string('3', 64));
        var changed = copy with { RamFheProfile = replacement };
        Assert.Equal(new string('3', 64), changed.RamFheProfile!.InitializerDescriptorHash);
        AssertProfile(policy.RamFheProfile);
        AssertProfile(copy.RamFheProfile);
    }

    [Fact]
    public void PublicConstructorRejectsInvalidDimensionsModeAndHash()
    {
        Assert.Throws<ArgumentOutOfRangeException>(() => new ToriiRamFheProfile(0, 4, 32, 16, ToriiRamFheEncryptedInputMode.EncryptedEnvelopeV1, 257, InitializerHash));
        Assert.Throws<ArgumentOutOfRangeException>(() => new ToriiRamFheProfile(1, 0, 32, 16, ToriiRamFheEncryptedInputMode.EncryptedEnvelopeV1, 257, InitializerHash));
        Assert.Throws<ArgumentOutOfRangeException>(() => new ToriiRamFheProfile(1, 4, 0, 16, ToriiRamFheEncryptedInputMode.EncryptedEnvelopeV1, 257, InitializerHash));
        Assert.Throws<ArgumentOutOfRangeException>(() => new ToriiRamFheProfile(1, 4, 32, 0, ToriiRamFheEncryptedInputMode.EncryptedEnvelopeV1, 257, InitializerHash));
        Assert.Throws<ArgumentOutOfRangeException>(() => new ToriiRamFheProfile(1, 4, 32, 16, (ToriiRamFheEncryptedInputMode)1, 257, InitializerHash));
        Assert.Throws<ArgumentOutOfRangeException>(() => new ToriiRamFheProfile(1, 4, 32, 16, ToriiRamFheEncryptedInputMode.EncryptedEnvelopeV1, 0, InitializerHash));
        foreach (var hash in new[] { InitializerHash.ToUpperInvariant(), "0x" + InitializerHash, InitializerHash[..63], new string('a', 64), " " + InitializerHash, null })
        {
            Assert.Throws<ArgumentException>(() => new ToriiRamFheProfile(1, 4, 32, 16, ToriiRamFheEncryptedInputMode.EncryptedEnvelopeV1, 257, hash!));
        }
    }

    [Fact]
    public void NumericBoundariesRetainTheirUnsignedTypes()
    {
        var profile = new ToriiRamFheProfile(byte.MaxValue, ushort.MaxValue, ushort.MaxValue, byte.MaxValue,
            ToriiRamFheEncryptedInputMode.EncryptedEnvelopeV1, ulong.MaxValue, InitializerHash);
        var decoded = JsonSerializer.Deserialize<ToriiRamFheProfile>(JsonSerializer.Serialize(profile));
        Assert.Equal(profile, decoded);
        Assert.Equal(ulong.MaxValue, decoded!.MinCiphertextModulus);
    }

    [Fact]
    public void StrictProfileRejectsMissingUnknownMalformedAndDuplicateFields()
    {
        foreach (var field in Profile().Select(property => property.Key))
        {
            var missing = Profile();
            missing.Remove(field);
            AssertRejected(missing, field);
            var explicitNull = Profile();
            explicitNull[field] = null;
            AssertRejected(explicitNull, field);
            var wrongType = Profile();
            wrongType[field] = new JsonArray();
            AssertRejected(wrongType, field);
            var json = Profile().ToJsonString();
            var duplicate = json[..^1] + $",\"{field}\":null}}";
            var error = Assert.Throws<JsonException>(() => JsonSerializer.Deserialize<ToriiRamFheProfile>(duplicate));
            Assert.Contains("must not appear more than once", error.Message);
        }
        var unknown = Profile();
        unknown["legacy_initializer"] = true;
        AssertRejected(unknown, "legacy_initializer");
        foreach (var raw in new[] { "null", "[]", "true", "1", "\"profile\"" })
        {
            Assert.Throws<JsonException>(() => JsonSerializer.Deserialize<ToriiRamFheProfile>(raw));
        }
    }

    [Fact]
    public void NumericFieldsRejectFractionalQuotedNegativeZeroAndOverflowValues()
    {
        foreach (var (name, overflow) in new[] {
            ("profile_version", "256"), ("ciphertext_mul_per_step", "256"),
            ("register_count", "65536"), ("memory_lane_count", "65536"),
            ("min_ciphertext_modulus", "18446744073709551616") })
        {
            foreach (var raw in new[] { "0", "-1", "1.5", "1.0", "1e0", "\"1\"", "true", overflow })
            {
                var profile = Profile();
                profile[name] = JsonNode.Parse(raw);
                AssertRejected(profile, name);
            }
        }
    }

    [Fact]
    public void ProfileRejectsUnknownModeAndNoncanonicalInitializerHash()
    {
        foreach (var mode in new[] { "unknown", "EncryptedEnvelopeV1", "encrypted_envelope_v1 " })
        {
            var profile = Profile();
            profile["encrypted_input_mode"] = mode;
            AssertRejected(profile, "encrypted_input_mode");
        }
        foreach (var hash in new[] { InitializerHash.ToUpperInvariant(), "0x" + InitializerHash, InitializerHash[..63], new string('a', 64), " " + InitializerHash })
        {
            var profile = Profile();
            profile["initializer_descriptor_hash"] = hash;
            AssertRejected(profile, "initializer_descriptor_hash");
        }
    }

    [Fact]
    public void OptionalProfileMayBeAbsentFromPolicy()
    {
        var policy = PolicyJson(null);
        policy.Remove("ram_fhe_profile");
        Assert.Null(JsonSerializer.Deserialize<ToriiIdentifierPolicySummary>(policy.ToJsonString())!.RamFheProfile);
    }

    private static void AssertRejected(JsonObject profile, string field)
    {
        var error = Assert.Throws<JsonException>(() => JsonSerializer.Deserialize<ToriiIdentifierPolicySummary>(PolicyJson(profile).ToJsonString()));
        Assert.Contains(field, error.Message);
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

    private static void AssertProfile(ToriiRamFheProfile? profile)
    {
        Assert.NotNull(profile);
        Assert.Equal(TypedProfile(), profile);
        Assert.Equal(ulong.MaxValue, profile.MinCiphertextModulus);
    }

    private static ToriiRamFheProfile TypedProfile() => new(1, 4, 32, 16,
        ToriiRamFheEncryptedInputMode.EncryptedEnvelopeV1, ulong.MaxValue, InitializerHash);

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
        ["backend"] = "bfv-programmed-v1",
        ["ram_fhe_profile"] = profile,
    };
}
