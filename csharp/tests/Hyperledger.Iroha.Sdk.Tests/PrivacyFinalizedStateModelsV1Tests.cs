using System.Globalization;
using System.Text;
using Hyperledger.Iroha.Privacy;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class PrivacyFinalizedStateModelsV1Tests
{
    private static readonly NetworkId TestNetworkId = NetworkId.Parse(
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0");

    [Fact]
    public void RequestsExposeTheClosedNativeQueryUnionAndDefensiveBindings()
    {
        var pool = Fixed32(0x21);
        var proofManaged = new PrivacyProofManagedPoolStateRequestV1(
            PrivacyProtocolIdV1.IrohaIvmPrivateNoteStarkV1,
            pool);
        var bound = (IPrivacyFinalizedStateRequestV1)proofManaged;
        Assert.Equal(98U, bound.QueryId);
        Assert.Equal(1U, bound.ProtocolIndex);
        Assert.Equal(pool, bound.RequestBinding);

        pool[0] ^= 0xff;
        Assert.NotEqual(pool, proofManaged.PoolId);
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            new PrivacyProofManagedPoolStateRequestV1(
                PrivacyProtocolIdV1.OrchardHalo2ActionsV1,
                Fixed32(0x22)));
        Assert.Throws<ArgumentException>(() =>
            new PrivacyOrchardPoolStateRequestV1(new byte[32]));

        AssertRequest(
            new PrivacyZkAceReplayNullifierRequestV1(Fixed32(1), Fixed32(2)),
            queryId: 97,
            protocolIndex: 0,
            bindingLength: 64);
        AssertRequest(
            new PrivacyProofManagedPoolStateRequestV1(
                PrivacyProtocolIdV1.PqMaspStarkV0,
                Fixed32(3)),
            queryId: 98,
            protocolIndex: 2,
            bindingLength: 32);
        AssertRequest(
            new PrivacyOrchardPoolStateRequestV1(Fixed32(4)),
            queryId: 99,
            protocolIndex: 0,
            bindingLength: 32);
        AssertRequest(
            new PrivacyOrchardNullifierRequestV1(Fixed32(5), Fixed32(6)),
            queryId: 100,
            protocolIndex: 0,
            bindingLength: 64);
        AssertRequest(
            new PrivacyAnonymousPgcPoolStateRequestV1(Fixed32(7)),
            queryId: 101,
            protocolIndex: 0,
            bindingLength: 32);
        AssertRequest(
            new PrivacyZkAmsAdmissionRequestV1(
                Fixed32(8),
                Fixed32(9),
                Fixed32(10),
                Fixed32(11)),
            queryId: 102,
            protocolIndex: 0,
            bindingLength: 128);
        AssertRequest(
            new PrivacyZkAmsProvisionRequestV1(
                Fixed32(12),
                Fixed32(13),
                Fixed32(14),
                Fixed32(15)),
            queryId: 103,
            protocolIndex: 0,
            bindingLength: 128);
        var trustAnchor = Fixed32(16);
        var x509Policy = Fixed32(17);
        var certificateNullifier = Fixed32(18);
        var x509Request = new PrivacyZkX509CertificateNullifierRequestV1(
            trustAnchor,
            x509Policy,
            certificateNullifier);
        AssertRequest(
            x509Request,
            queryId: 104,
            protocolIndex: 0,
            bindingLength: 96);
        var x509Binding = ((IPrivacyFinalizedStateRequestV1)x509Request).RequestBinding;
        Assert.Equal(trustAnchor, x509Binding[..32]);
        Assert.Equal(x509Policy, x509Binding[32..64]);
        Assert.Equal(certificateNullifier, x509Binding[64..96]);
    }

    [Fact]
    public void ProofManagedProjectionBindsNetworkSelectorAndCanonicalWireForms()
    {
        var pool = Fixed32(0x21);
        var query = NativeQuery(98, 0, pool);
        var json = ProofManagedJson(pool);

        var parsed = Assert.IsType<PrivacyProofManagedPoolStateViewV1>(
            PrivacyFinalizedStateContractV1.ParseProjectionV1(
                Encoding.UTF8.GetBytes(json),
                query));
        Assert.Equal(TestNetworkId, parsed.NetworkId);
        Assert.Equal(PrivacyProtocolIdV1.MoneroFcmpPlusPlusV1, parsed.ProtocolId);
        Assert.Equal(PrivacyFinalizedRootRoleV1.OutputSet, parsed.RootRole);
        Assert.Equal(1UL, parsed.CurrentEpoch);
        Assert.Null(parsed.LatestTransition);
        Assert.Equal(Fixed32(0x31), parsed.FinalizedBlockHash);

        var returned = parsed.PoolId;
        returned[0] ^= 0xff;
        Assert.Equal(pool, parsed.PoolId);
    }

    [Theory]
    [InlineData("network")]
    [InlineData("binding")]
    [InlineData("unknown")]
    [InlineData("numeric")]
    [InlineData("protocol")]
    [InlineData("transition")]
    public void ProjectionRejectsHostileSchemaAndBindingMutations(string mutation)
    {
        var pool = Fixed32(0x21);
        var query = NativeQuery(98, 0, pool);
        var json = ProofManagedJson(pool);
        switch (mutation)
        {
            case "network":
                json = json.Replace(
                    TestNetworkId.ToString(),
                    new string('a', 64),
                    StringComparison.Ordinal);
                break;
            case "binding":
                json = json.Replace(ByteArrayJson(0x21), ByteArrayJson(0x22), StringComparison.Ordinal);
                break;
            case "unknown":
                json = json.Replace("{", "{\"unexpected\":null,", StringComparison.Ordinal);
                break;
            case "numeric":
                json = json.Replace("\"current_epoch\":\"1\"", "\"current_epoch\":\"01\"", StringComparison.Ordinal);
                break;
            case "protocol":
                json = json.Replace(
                    "monero-fcmp-plus-plus-v1",
                    "pq-masp-stark-v0",
                    StringComparison.Ordinal);
                break;
            case "transition":
                json = json.Replace("\"current_epoch\":\"1\"", "\"current_epoch\":\"2\"", StringComparison.Ordinal);
                break;
        }

        Assert.ThrowsAny<Exception>(() =>
            PrivacyFinalizedStateContractV1.ParseProjectionV1(
                Encoding.UTF8.GetBytes(json),
                query));
    }

    [Fact]
    public void FinalityHashesRequireCanonicalChecksummedHashLiterals()
    {
        var pool = Fixed32(0x21);
        var query = NativeQuery(98, 0, pool);
        var json = ProofManagedJson(pool).Replace(
            HashLiteral(0x31),
            new string('3', 64),
            StringComparison.Ordinal);

        Assert.Throws<InvalidDataException>(() =>
            PrivacyFinalizedStateContractV1.ParseProjectionV1(
                Encoding.UTF8.GetBytes(json),
                query));
    }

    private static PrivacyAuthenticatedStateQueryV1 NativeQuery(
        uint queryId,
        uint protocolIndex,
        byte[] binding) =>
        new(
            new byte[] { 1 },
            new byte[] { 2 },
            TestNetworkId,
            queryId,
            protocolIndex,
            binding);

    private static void AssertRequest(
        IPrivacyFinalizedStateRequestV1 request,
        uint queryId,
        uint protocolIndex,
        int bindingLength)
    {
        Assert.Equal(queryId, request.QueryId);
        Assert.Equal(protocolIndex, request.ProtocolIndex);
        Assert.Equal(bindingLength, request.RequestBinding.Length);
        Assert.Contains(request.RequestBinding, static value => value != 0);

        var first = request.RequestBinding;
        first[0] ^= 0xff;
        Assert.NotEqual(first, request.RequestBinding);
    }

    private static string ProofManagedJson(byte[] pool) => $$"""
        {
          "network_id":"{{TestNetworkId}}",
          "protocol_id":{"protocol":"monero-fcmp-plus-plus-v1","value":null},
          "pool_id":{{ByteArrayJson(pool)}},
          "asset_definition_id":"rose#wonderland",
          "root_role":{"role":"OutputSet","value":null},
          "bootstrap_digest":{{ByteArrayJson(0x23)}},
          "initial_root":{{ByteArrayJson(0x24)}},
          "current_epoch":"1",
          "current_root":{{ByteArrayJson(0x25)}},
          "output_count":"1",
          "bootstrap_admitted_at_height":"2",
          "latest_transition":null,
          "finalized_height":"3",
          "finalized_block_hash":"{{HashLiteral(0x31)}}"
        }
        """;

    private static byte[] Fixed32(byte value) => Enumerable.Repeat(value, 32).ToArray();

    private static string ByteArrayJson(byte value) => ByteArrayJson(Fixed32(value));

    private static string ByteArrayJson(byte[] value) =>
        $"[{string.Join(',', value.Select(item => item.ToString(CultureInfo.InvariantCulture)))}]";

    private static string HashLiteral(byte value)
    {
        var body = Convert.ToHexString(Fixed32(value));
        var checksum = Crc16(Encoding.ASCII.GetBytes($"hash:{body}"));
        return $"hash:{body}#{checksum:X4}";
    }

    private static ushort Crc16(ReadOnlySpan<byte> value)
    {
        var crc = 0xffff;
        foreach (var item in value)
        {
            crc ^= item << 8;
            for (var bit = 0; bit < 8; bit++)
            {
                crc = (crc & 0x8000) != 0
                    ? ((crc << 1) ^ 0x1021) & 0xffff
                    : (crc << 1) & 0xffff;
            }
        }
        return (ushort)crc;
    }
}
