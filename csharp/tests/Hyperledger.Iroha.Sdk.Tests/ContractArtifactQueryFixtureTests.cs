using System.Security.Cryptography;
using System.Text.Json;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Queries;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class ContractArtifactQueryFixtureTests
{
    [Fact]
    public void SignedArtifactQueriesMatchTheNativeCanonicalFixture()
    {
        using var fixture = JsonDocument.Parse(File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Fixtures", "contract_artifact_query_v1.json")));
        var root = fixture.RootElement;
        Assert.Equal(1, root.GetProperty("fixture_version").GetInt32());
        Assert.Equal(1, root.GetProperty("norito_layout_version").GetInt32());
        // Fixture v1 declares compact per-value lengths (0x02), never inferred packed bits.
        Assert.Equal(0x02, root.GetProperty("norito_layout_flags").GetInt32());
        Assert.Equal("iroha_data_model/examples/contract_artifact_query_fixture.rs", root.GetProperty("generator").GetString());
        var seed = Convert.FromHexString(root.GetProperty("test_seed_hex").GetString()!);
        try
        {
            var cases = root.GetProperty("cases").EnumerateArray().ToArray();
            Assert.Equal(2, cases.Length);
            var scopes = new List<ulong>();
            foreach (var item in cases)
            {
                var artifact = JsonSerializer.Deserialize<ContractArtifactId>(item.GetProperty("artifact_id").GetRawText())!;
                scopes.Add(artifact.DataspaceId);
                var envelope = new SignedQueryBuilder(root.GetProperty("authority").GetString()!, NetworkId.Parse(root.GetProperty("network_id").GetString()!))
                    .FindContractManifestByArtifactId(artifact)
                    .BuildSigned(seed, root.GetProperty("creation_time_ms").GetUInt64(), root.GetProperty("time_to_live_ms").GetUInt64(),
                        Convert.FromHexString(root.GetProperty("nonce_hex").GetString()!));
                Assert.Equal(Convert.FromHexString(item.GetProperty("payload_hex").GetString()!), envelope.PayloadBytes);
                Assert.Equal(Convert.FromHexString(item.GetProperty("signed_query_versioned_hex").GetString()!), envelope.VersionedNoritoBytes);
                Assert.True(Ed25519Signer.Verify(IrohaHash.Hash(envelope.PayloadBytes), envelope.SignatureBytes, Ed25519Signer.GetPublicKey(seed)));
            }
            Assert.Equal(new[] { 0UL, ulong.MaxValue }, scopes);
        }
        finally { CryptographicOperations.ZeroMemory(seed); }
    }
    [Theory]
    [InlineData("FindExecutorDataModel")]
    [InlineData("FindParameters")]
    [InlineData("FindAliasesByAccountId")]
    [InlineData("FindProofRecordById")]
    [InlineData("FindContractManifestByArtifactId")]
    [InlineData("FindAbiVersion")]
    [InlineData("FindAssetById")]
    [InlineData("FindAssetDefinitionById")]
    [InlineData("FindTwitterBindingByHash")]
    [InlineData("FindDomainEndorsements")]
    [InlineData("FindDomainEndorsementPolicy")]
    [InlineData("FindDomainCommittee")]
    [InlineData("FindDaPinIntentByTicket")]
    [InlineData("FindDaPinIntentByManifest")]
    [InlineData("FindDaPinIntentByAlias")]
    [InlineData("FindDaPinIntentByLaneEpochSequence")]
    [InlineData("FindSorafsProviderOwner")]
    [InlineData("FindDataspaceNameOwnerById")]
    public void EveryManagedSingularQueryMatchesNativeRequestPayloadAndSignature(string name)
    {
        using var fixture = JsonDocument.Parse(File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Fixtures", "contract_artifact_query_v1.json")));
        var root = fixture.RootElement;
        Assert.Equal(1, root.GetProperty("fixture_version").GetInt32());
        Assert.Equal(1, root.GetProperty("norito_layout_version").GetInt32());
        Assert.Equal(0x02, root.GetProperty("norito_layout_flags").GetInt32());
        var cases = root.GetProperty("singular_cases").EnumerateArray().ToArray();
        Assert.Equal(18, cases.Length);
        Assert.Equal(18, cases.Select(item => item.GetProperty("name").GetString()).Distinct().Count());
        var item = Assert.Single(cases, candidate => candidate.GetProperty("name").GetString() == name);
        var inputs = item.GetProperty("inputs");
        string Text(string key) => inputs.GetProperty(key).GetString()!;
        var builder = new SignedQueryBuilder(root.GetProperty("authority").GetString()!, NetworkId.Parse(root.GetProperty("network_id").GetString()!));
        builder = name switch
        {
            "FindExecutorDataModel" => builder.FindExecutorDataModel(),
            "FindParameters" => builder.FindParameters(),
            "FindAliasesByAccountId" => builder.FindAliasesByAccountId(Text("account_id"), Text("dataspace"), Text("domain")),
            "FindProofRecordById" => builder.FindProofRecordById(Text("backend"), Text("proof_hash")),
            "FindContractManifestByArtifactId" => builder.FindContractManifestByArtifactId(JsonSerializer.Deserialize<ContractArtifactId>(inputs.GetProperty("artifact_id").GetRawText())!),
            "FindAbiVersion" => builder.FindAbiVersion(),
            "FindAssetById" => builder.FindAssetById(Text("asset_definition_id"), Text("account_id"), inputs.GetProperty("dataspace_id").GetUInt64()),
            "FindAssetDefinitionById" => builder.FindAssetDefinitionById(Text("asset_definition_id")),
            "FindTwitterBindingByHash" => builder.FindTwitterBindingByHash(Text("pepper_id"), Text("digest_hex")),
            "FindDomainEndorsements" => builder.FindDomainEndorsements(Text("domain_id")),
            "FindDomainEndorsementPolicy" => builder.FindDomainEndorsementPolicy(Text("domain_id")),
            "FindDomainCommittee" => builder.FindDomainCommittee(Text("committee_id")),
            "FindDaPinIntentByTicket" => builder.FindDaPinIntentByTicket(Text("storage_ticket")),
            "FindDaPinIntentByManifest" => builder.FindDaPinIntentByManifest(Text("manifest_digest")),
            "FindDaPinIntentByAlias" => builder.FindDaPinIntentByAlias(Text("alias")),
            "FindDaPinIntentByLaneEpochSequence" => builder.FindDaPinIntentByLaneEpochSequence(inputs.GetProperty("lane_id").GetUInt32(), inputs.GetProperty("epoch").GetUInt64(), inputs.GetProperty("sequence").GetUInt64()),
            "FindSorafsProviderOwner" => builder.FindSorafsProviderOwner(Text("provider_id")),
            "FindDataspaceNameOwnerById" => builder.FindDataspaceNameOwnerById(inputs.GetProperty("dataspace_id").GetUInt64()),
            _ => throw new InvalidOperationException("Uncovered native query fixture."),
        };
        var seed = Convert.FromHexString(root.GetProperty("test_seed_hex").GetString()!);
        try
        {
            var envelope = builder.BuildSigned(seed, root.GetProperty("creation_time_ms").GetUInt64(), root.GetProperty("time_to_live_ms").GetUInt64(), Convert.FromHexString(root.GetProperty("nonce_hex").GetString()!));
            Assert.Equal(Convert.FromHexString(item.GetProperty("query_request_hex").GetString()!), ExtractRequest(envelope.PayloadBytes));
            Assert.Equal(Convert.FromHexString(item.GetProperty("payload_hex").GetString()!), envelope.PayloadBytes);
            Assert.Equal(Convert.FromHexString(item.GetProperty("signed_query_versioned_hex").GetString()!), envelope.VersionedNoritoBytes);
            Assert.True(Ed25519Signer.Verify(IrohaHash.Hash(envelope.PayloadBytes), envelope.SignatureBytes, Ed25519Signer.GetPublicKey(seed)));
        }
        finally { CryptographicOperations.ZeroMemory(seed); }
    }

    private static byte[] ExtractRequest(ReadOnlySpan<byte> payload)
    {
        // Fixture v1 explicitly fixes compact-length fields: network, authority,
        // creation time, TTL, nonce, then QueryRequest. Require exact framing.
        var offset = 0;
        byte[]? request = null;
        for (var field = 0; field < 6; field++)
        {
            ulong length = 0;
            var shift = 0;
            byte next;
            do
            {
                Assert.True(offset < payload.Length && shift < 64);
                next = payload[offset++];
                length |= (ulong)(next & 0x7f) << shift;
                shift += 7;
            }
            while ((next & 0x80) != 0);
            var count = checked((int)length);
            Assert.True(count <= payload.Length - offset);
            if (field == 5) request = payload.Slice(offset, count).ToArray();
            offset += count;
        }
        Assert.Equal(payload.Length, offset);
        return Assert.IsType<byte[]>(request);
    }
}
