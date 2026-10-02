using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Fixture and rejection gates for the closed release-governance JSON shapes.</summary>
public sealed class KagemushaReleaseGovernanceV1Tests
{
    private static byte[] Fixture(string name) => File.ReadAllBytes(
        Path.Combine(AppContext.BaseDirectory, "Fixtures", name));

    private static JsonObject MutableFixture(string name) =>
        JsonNode.Parse(Fixture(name))!.AsObject();

    private static void Reject(JsonNode proposal) =>
        Assert.Throws<JsonException>(() => KagemushaReleaseProposalV1.Parse(
            Encoding.UTF8.GetBytes(proposal.ToJsonString())));

    [Fact]
    public void ReleaseKindsOccupyTheExactFirstReleaseFixtureIndices()
    {
        using var fixture = JsonDocument.Parse(Fixture("parliament_api_v1.json"));
        var kinds = fixture.RootElement.GetProperty("proposal_kinds").EnumerateArray()
            .Select(static item => item.GetString()).ToArray();
        Assert.Equal("KagemushaVerifierPolicyInstall", kinds[10]);
        Assert.Equal("KagemushaVerifierReleaseInstall", kinds[11]);
        Assert.Equal("KagemushaVerifierReleaseActivate", kinds[12]);
        Assert.Equal("KagemushaVerifierReleaseRetire", kinds[13]);
        Assert.Equal(14, kinds.Length);
    }

    [Fact]
    public void EmbeddedReleaseSchemaClosureMatchesCanonicalOpenApi()
    {
        using var openApi = JsonDocument.Parse(Fixture("torii-openapi.json"));
        using var stream = typeof(KagemushaReleaseProposalV1).Assembly.GetManifestResourceStream(
            "Hyperledger.Iroha.Torii.KagemushaReleaseSchemasV1.json");
        Assert.NotNull(stream);
        using var embedded = JsonDocument.Parse(stream);
        var schemas = openApi.RootElement.GetProperty("components").GetProperty("schemas");
        var roots = new[]
        {
            "GovernanceKagemushaGovernedVerifierRegistryV1",
            "GovernanceKagemushaReleaseManifestV1",
            "GovernanceKagemushaInternalValidationReceiptV1",
            "GovernanceKagemushaReleaseAttestationV1",
        };
        var closure = new HashSet<string>(roots, StringComparer.Ordinal);
        var pending = new Queue<string>(roots);
        while (pending.TryDequeue(out var name))
        {
            VisitReferences(schemas.GetProperty(name), referenced =>
            {
                if (closure.Add(referenced))
                {
                    pending.Enqueue(referenced);
                }
            });
        }
        Assert.Equal(closure.Order(StringComparer.Ordinal),
            embedded.RootElement.EnumerateObject().Select(static item => item.Name)
                .Order(StringComparer.Ordinal));
        foreach (var name in closure)
        {
            Assert.Equal(CanonicalSchema(schemas.GetProperty(name), stripDocumentation: true),
                CanonicalSchema(embedded.RootElement.GetProperty(name), stripDocumentation: false));
        }
    }

    private static void VisitReferences(JsonElement value, Action<string> accept)
    {
        if (value.ValueKind == JsonValueKind.Array)
        {
            foreach (var item in value.EnumerateArray()) VisitReferences(item, accept);
        }
        else if (value.ValueKind == JsonValueKind.Object)
        {
            foreach (var field in value.EnumerateObject())
            {
                if (field.Name == "$ref" && field.Value.ValueKind == JsonValueKind.String)
                {
                    const string prefix = "#/components/schemas/";
                    var reference = field.Value.GetString()!;
                    if (reference.StartsWith(prefix, StringComparison.Ordinal))
                    {
                        accept(reference[prefix.Length..]);
                    }
                }
                VisitReferences(field.Value, accept);
            }
        }
    }

    private static string CanonicalSchema(JsonElement value, bool stripDocumentation)
    {
        if (value.ValueKind == JsonValueKind.Object)
        {
            return "{" + string.Join(",", value.EnumerateObject()
                .Where(item => !stripDocumentation || item.Name is not ("description" or "title" or "example"))
                .OrderBy(static item => item.Name, StringComparer.Ordinal)
                .Select(item => JsonSerializer.Serialize(item.Name) + ":"
                    + CanonicalSchema(item.Value, stripDocumentation))) + "}";
        }
        if (value.ValueKind == JsonValueKind.Array)
        {
            return "[" + string.Join(",", value.EnumerateArray()
                .Select(item => CanonicalSchema(item, stripDocumentation))) + "]";
        }
        return value.ValueKind == JsonValueKind.String
            ? JsonSerializer.Serialize(value.GetString())
            : value.GetRawText();
    }

    [Fact]
    public void RustGeneratedInstallFixtureHasClosedNestedSchemasAndDefensiveOwnership()
    {
        var proposal = Assert.IsType<KagemushaReleaseInstallProposalV1>(
            KagemushaReleaseProposalV1.Parse(Fixture("kagemusha_verifier_release_install_v1.json")));
        Assert.Equal(32, proposal.Manifest.GetProperty("release_id").GetArrayLength());
        Assert.Equal(1, proposal.ExpectedPredecessor.GetProperty("version").GetInt32());
        Assert.Equal(0, proposal.ExpectedPredecessor.GetProperty("releases").GetArrayLength());
        Assert.NotEmpty(proposal.ProposalOperator);
        Assert.StartsWith("hash:", proposal.NetworkId.ToString());
        // The owned JsonElements remain readable after the parser disposes its input document.
        Assert.Equal(1, proposal.Attestation.GetProperty("version").GetInt32());
    }

    [Fact]
    public void RustGeneratedActivationFixtureSelectsTheSoleStandby()
    {
        var proposal = Assert.IsType<KagemushaReleaseActivateProposalV1>(
            KagemushaReleaseProposalV1.Parse(Fixture("kagemusha_verifier_release_activate_v1.json")));
        var predecessor = proposal.ExpectedPredecessor;
        Assert.Equal(1, predecessor.GetProperty("releases").GetArrayLength());
        Assert.Equal(2, predecessor.GetProperty("releases")[0].GetProperty("status").GetInt32());
        Assert.Equal(32, proposal.SuccessorReleaseId.Length);
        var copy = proposal.SuccessorReleaseId;
        copy[0] ^= 0xff;
        Assert.NotEqual(copy[0], proposal.SuccessorReleaseId[0]);
    }

    [Fact]
    public void RustGeneratedRetirementFixtureOwnsOnlyTheUnusedStandby()
    {
        var proposal = Assert.IsType<KagemushaReleaseRetireProposalV1>(
            KagemushaReleaseProposalV1.Parse(Fixture("kagemusha_verifier_release_retire_v1.json")));
        var statuses = proposal.ExpectedPredecessor.GetProperty("releases").EnumerateArray()
            .Select(static row => row.GetProperty("status").GetInt32()).Order().ToArray();
        Assert.Equal(new[] { 1, 2, 3 }, statuses);
        var selected = proposal.StandbyReleaseId;
        selected[0] ^= 1;
        Assert.NotEqual(selected[0], proposal.StandbyReleaseId[0]);
    }

    [Fact]
    public void RetirementRejectsNonstandbyAndMalformedCompletePredecessors()
    {
        Action<JsonObject>[] mutations =
        [
            p => p.Remove("standby_release_id"),
            p => p["retired_alias"] = true,
            p => p["standby_release_id"] = new JsonArray(Enumerable.Repeat(1, 31).Select(static b => (JsonNode?)JsonValue.Create(b)).ToArray()),
            p => p["standby_release_id"] = new JsonArray(Enumerable.Repeat(0, 32).Select(static b => (JsonNode?)JsonValue.Create(b)).ToArray()),
            p => p["standby_release_id"] = p["expected_predecessor"]!["releases"]!.AsArray().Single(row => row!["status"]!.GetValue<int>() == 1)!["release_id"]!.DeepClone(),
            p => p["standby_release_id"] = p["expected_predecessor"]!["releases"]!.AsArray().Single(row => row!["status"]!.GetValue<int>() == 3)!["release_id"]!.DeepClone(),
            p => p["expected_predecessor"]!["authority_policy"] = null,
            p => p["expected_predecessor"]!["active_release_id"] = null,
            p => p["expected_predecessor"]!["releases"] = new JsonArray(p["expected_predecessor"]!["releases"]!.AsArray().Reverse().Select(static row => row!.DeepClone()).ToArray()),
            p => p["expected_predecessor"]!["releases"]!.AsArray().Add(p["expected_predecessor"]!["releases"]!.AsArray().Last()!.DeepClone()),
            p => p["expected_predecessor"]!["releases"]![0]!["profile_digest"] = new JsonArray(Enumerable.Repeat(0, 32).Select(static b => (JsonNode?)JsonValue.Create(b)).ToArray()),
            p => p["expected_predecessor"]!["releases"] = new JsonArray(p["expected_predecessor"]!["releases"]!.AsArray().Where(row => row!["status"]!.GetValue<int>() != 2).Select(static row => row!.DeepClone()).ToArray()),
        ];
        foreach (var mutate in mutations)
        {
            var proposal = MutableFixture("kagemusha_verifier_release_retire_v1.json");
            mutate(proposal["payload"]!.AsObject());
            Reject(proposal);
        }
    }

    [Fact]
    public void RetirementRejectsMalformedThresholdAndNoncanonicalSignerOrderOrKeys()
    {
        Action<JsonObject>[] mutations =
        [
            policy => policy["threshold"] = 32,
            policy => policy["authorized_signers"]![0] = "not-a-public-key",
            policy => policy["authorized_signers"]![0] = policy["authorized_signers"]![0]!.GetValue<string>().ToUpperInvariant(),
            policy => policy["authorized_signers"] = new JsonArray(policy["authorized_signers"]!.AsArray().Reverse().Select(static key => key!.DeepClone()).ToArray()),
            policy => policy["authorized_signers"]![1] = policy["authorized_signers"]![0]!.DeepClone(),
            policy => policy["authorized_signers"]![0] = "ed0120" + new string('0', 64),
            policy => policy["authorized_signers"]![0] = "ed810020" + policy["authorized_signers"]![0]!.GetValue<string>()[6..],
        ];
        foreach (var mutate in mutations)
        {
            var proposal = MutableFixture("kagemusha_verifier_release_retire_v1.json");
            mutate(proposal["payload"]!["expected_predecessor"]!["authority_policy"]!.AsObject());
            Reject(proposal);
        }
    }

    [Fact]
    public void InstallRejectsUnknownAndMissingNestedFieldsAndUnsafeNumbers()
    {
        var name = "kagemusha_verifier_release_install_v1.json";
        var noNetwork = MutableFixture(name);
        noNetwork["payload"]!["manifest"]!.AsObject().Remove("network_id");
        Reject(noNetwork);

        var noPurpose = MutableFixture(name);
        noPurpose["payload"]!["manifest"]!.AsObject().Remove("purpose");
        Reject(noPurpose);

        var invalidPurpose = MutableFixture(name);
        invalidPurpose["payload"]!["manifest"]!["purpose"]!["kind"] = "unknown";
        Reject(invalidPurpose);

        var extra = MutableFixture(name);
        extra["payload"]!["manifest"]!["enabled_profiles"]![0]!["hardware_profile"]!["unrecognized"] = true;
        Reject(extra);

        var missing = MutableFixture(name);
        missing["payload"]!["attestation"]!["subject"]!.AsObject().Remove("release_id");
        Reject(missing);

        var mismatch = MutableFixture(name);
        mismatch["payload"]!["attestation"]!["subject"]!["release_id"]![0] = 0;
        Reject(mismatch);

        var tooWide = MutableFixture(name);
        tooWide["payload"]!["receipt"]!["fuzz_cases"] = 9_007_199_254_740_992L;
        Reject(tooWide);

        var noPolicy = MutableFixture(name);
        noPolicy["payload"]!["expected_predecessor"]!["authority_policy"] = null;
        Reject(noPolicy);
    }

    [Fact]
    public void ActivationRejectsWrongOrAmbiguousStandbyAndMalformedIdentity()
    {
        var name = "kagemusha_verifier_release_activate_v1.json";
        var wrongRelease = MutableFixture(name);
        wrongRelease["payload"]!["successor_release_id"]![0] = 0;
        Reject(wrongRelease);

        var active = MutableFixture(name);
        active["payload"]!["expected_predecessor"]!["active_release_id"] =
            active["payload"]!["successor_release_id"]!.DeepClone();
        Reject(active);

        var second = MutableFixture(name);
        second["payload"]!["expected_predecessor"]!["releases"]!.AsArray().Add(
            second["payload"]!["expected_predecessor"]!["releases"]![0]!.DeepClone());
        Reject(second);

        var malformedNetwork = MutableFixture(name);
        malformedNetwork["payload"]!["network_id"] = "not-a-network";
        Reject(malformedNetwork);

        var malformedAccount = MutableFixture(name);
        malformedAccount["payload"]!["proposal_operator"] = "alice@wonderland";
        Reject(malformedAccount);
    }

    [Fact]
    public void ClosedEnvelopeRejectsUnknownKindsFieldsDuplicatesAndOversizeInput()
    {
        var proposal = MutableFixture("kagemusha_verifier_release_activate_v1.json");
        proposal["kind"] = "KagemushaVerifierReleaseActivation";
        Reject(proposal);
        proposal = MutableFixture("kagemusha_verifier_release_activate_v1.json");
        proposal["extra"] = 1;
        Reject(proposal);
        proposal = MutableFixture("kagemusha_verifier_release_activate_v1.json");
        proposal["payload"]!["unknown"] = 1;
        Reject(proposal);
        Assert.Throws<JsonException>(() => KagemushaReleaseProposalV1.Parse(
            Encoding.UTF8.GetBytes("{\"kind\":\"KagemushaVerifierReleaseActivate\",\"kind\":\"KagemushaVerifierReleaseActivate\",\"payload\":{}}")));
        Assert.Throws<JsonException>(() => KagemushaReleaseProposalV1.Parse(
            new byte[16 * 1024 * 1024 + 1]));
    }
}
