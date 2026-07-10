using System.Buffers.Binary;
using System.Net;
using System.Text;
using System.Text.Json;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Sccp;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class SccpExactTests
{
    [Fact]
    public void SharedNativeTransferEventVectorsMatchExactly()
    {
        using var document = JsonDocument.Parse(File.ReadAllBytes(
            Path.Combine(AppContext.BaseDirectory, "Fixtures", "sccp", "native_transfer_event_v1.json")));
        Assert.Equal(1, document.RootElement.GetProperty("version").GetInt32());
        foreach (var vector in document.RootElement.GetProperty("vectors").EnumerateArray())
        {
            var lane = new SccpLaneIdV1(
                SccpNetworkV1Extensions.ParseProfileKey(vector.GetProperty("source_profile").GetString()!),
                SccpNetworkV1Extensions.ParseProfileKey(vector.GetProperty("target_profile").GetString()!));
            var payload = SccpV1.DecodeLowerHex(vector.GetProperty("canonical_payload_hex").GetString()!);
            var payloadHash = SccpV1.PayloadHash(payload);
            var messageId = SccpV1.MessageId(lane, payload);
            Assert.Equal(vector.GetProperty("canonical_lane_hex").GetString(), SccpV1.LowerHex(SccpV1.CanonicalLaneBytes(lane)));
            Assert.Equal(vector.GetProperty("lane_hash_hex").GetString(), SccpV1.LowerHex(SccpV1.LaneHash(lane)));
            Assert.Equal(vector.GetProperty("payload_hash_hex").GetString(), SccpV1.LowerHex(payloadHash));
            Assert.Equal(vector.GetProperty("message_id_hex").GetString(), SccpV1.LowerHex(messageId));
            Assert.Equal(
                vector.GetProperty("source_event_digest_hex").GetString(),
                SccpV1.LowerHex(SccpV1.SourceEventDigest(lane, messageId, payloadHash)));
        }
    }

    [Fact]
    public void SourceEventDigestIsCrossLaneSeparatedAndRoleCollisionFails()
    {
        var payload = new byte[] { 1, 2, 3 };
        var mainnet = new SccpLaneIdV1(SccpNetworkV1.BscMainnet, SccpNetworkV1.SoraTaira);
        var testnet = new SccpLaneIdV1(SccpNetworkV1.BscTestnet, SccpNetworkV1.SoraTaira);
        var payloadHash = SccpV1.PayloadHash(payload);
        var mainnetMessage = SccpV1.MessageId(mainnet, payload);
        var testnetMessage = SccpV1.MessageId(testnet, payload);
        Assert.NotEqual(mainnetMessage, testnetMessage);
        Assert.NotEqual(
            SccpV1.SourceEventDigest(mainnet, mainnetMessage, payloadHash),
            SccpV1.SourceEventDigest(testnet, testnetMessage, payloadHash));
        Assert.Throws<ArgumentException>(() => SccpV1.SourceEventDigest(
            mainnet,
            SccpV1.LaneHash(mainnet),
            payloadHash));
    }

    [Fact]
    public void CanonicalCodecsRejectTextAliasesAndMalformedBinaryIdentities()
    {
        Assert.Equal("merchant@taira"u8.ToArray(), SccpCodecV1.CanonicalText.Validate("merchant@taira"u8));
        Assert.Throws<ArgumentException>(() => SccpCodecV1.CanonicalText.Validate([]));
        Assert.Throws<ArgumentException>(() => SccpCodecV1.CanonicalText.Validate("contains space"u8));
        Assert.Throws<ArgumentException>(() => SccpCodecV1.CanonicalText.Validate(new byte[257]));
        Assert.Throws<ArgumentException>(() => SccpCodecV1.EvmAddress20.Validate(new byte[20]));
        Assert.Equal(20, SccpCodecV1.EvmAddress20.Validate(Enumerable.Repeat((byte)1, 20).ToArray()).Length);

        var ton = new byte[36];
        BinaryPrimitives.WriteInt32LittleEndian(ton, -1);
        ton[4] = 1;
        Assert.Equal(36, SccpCodecV1.TonAccount36.Validate(ton).Length);
        BinaryPrimitives.WriteInt32LittleEndian(ton, 1);
        Assert.Throws<ArgumentException>(() => SccpCodecV1.TonAccount36.Validate(ton));

        var tron = Enumerable.Repeat((byte)1, 21).ToArray();
        tron[0] = 0x41;
        Assert.Equal(21, SccpCodecV1.TronAddress21.Validate(tron).Length);
        tron[0] = 0x42;
        Assert.Throws<ArgumentException>(() => SccpCodecV1.TronAddress21.Validate(tron));
    }

    [Fact]
    public void SourceEmittersRequireRouteConfigurationAndDistinctRoles()
    {
        Assert.Throws<ArgumentException>(() => new SccpSourceEmitterV1.Evm(
            Enumerable.Repeat((byte)1, 20).ToArray(),
            Enumerable.Repeat((byte)2, 32).ToArray(),
            Enumerable.Repeat((byte)2, 32).ToArray()));
        var emitter = new SccpSourceEmitterV1.Tron(
            Enumerable.Repeat((byte)1, 20).ToArray(),
            Enumerable.Repeat((byte)2, 32).ToArray(),
            Enumerable.Repeat((byte)3, 32).ToArray());
        Assert.Equal(32, emitter.RouteConfigHash.Length);
        Assert.DoesNotContain("Owner", typeof(SccpSourceEmitterV1.Tron).GetProperties().Select(static property => property.Name));
    }

    [Fact]
    public void SubmitRequestsRequireCanonicalNoritoAndAuthorityBoundSignerPair()
    {
        var pair = Ed25519KeyPair.FromSeed(Enumerable.Repeat((byte)7, 32).ToArray());
        var authority = pair.ToAccountAddress().ToI105();
        var artifact = Convert.ToBase64String(NoritoCodec.Encode("iroha_sccp::NativeProofV1", [1]));
        var signature = Convert.ToBase64String(Ed25519Signer.Sign(new byte[32], pair.PrivateKeySeed));
        var request = new SccpBridgeMessageSubmitRequest(
            authority,
            artifact,
            Convert.ToHexString(pair.PublicKey).ToLowerInvariant(),
            signature,
            creationTimeMs: 1);
        var json = JsonSerializer.Serialize(request);
        Assert.Contains("\"native_proof_b64\"", json, StringComparison.Ordinal);
        Assert.DoesNotContain("private_key", json, StringComparison.Ordinal);
        Assert.DoesNotContain("message_bundle", json, StringComparison.Ordinal);

        Assert.Throws<ArgumentException>(() => new SccpBridgeMessageSubmitRequest(authority, "AQ=="));
        Assert.Throws<ArgumentException>(() => new SccpBridgeMessageSubmitRequest(
            authority,
            artifact,
            Convert.ToHexString(pair.PublicKey).ToLowerInvariant()));
        Assert.Throws<ArgumentException>(() => new SccpBridgeMessageSubmitRequest(
            authority,
            artifact,
            new string('0', 64),
            signature));
        Assert.Throws<ArgumentOutOfRangeException>(() => new SccpBridgeMessageSubmitRequest(
            authority,
            artifact,
            creationTimeMs: 0));
    }

    [Fact]
    public void UnifiedResponseAcceptsOnlyExactSubmittedAndPreparedStates()
    {
        var submitted = SccpBridgeSubmitResponse.Parse(ResponseJson(
            submitted: true,
            txHash: new string('3', 64),
            transactionPayload: null,
            signingMessage: null));
        Assert.True(submitted.Submitted);

        var payload = new byte[] { 1, 2, 3, 4 };
        var prepared = SccpBridgeSubmitResponse.Parse(
            ResponseJson(
                submitted: false,
                txHash: null,
                transactionPayload: Convert.ToBase64String(payload),
                signingMessage: Convert.ToBase64String(IrohaHash.Hash(payload))),
            new SccpBridgeResponseExpectation(
                SccpPayloadKindV1.Transfer,
                new string('1', 64),
                2,
                SccpNetworkV1.BscMainnet,
                7));
        Assert.False(prepared.Submitted);
    }

    [Theory]
    [InlineData("ok")]
    [InlineData("proof_kind")]
    [InlineData("message_kind")]
    [InlineData("transaction_scaffold_b64")]
    [InlineData("signed_transaction_b64")]
    public void UnifiedResponseRejectsLegacyFields(string legacy)
    {
        var valid = Encoding.UTF8.GetString(ResponseJson(
            true,
            new string('3', 64),
            null,
            null));
        var mutated = valid[..^1] + $",\"{legacy}\":null}}";
        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.Parse(Encoding.UTF8.GetBytes(mutated)));
    }

    [Fact]
    public void UnifiedResponseRejectsDuplicateProfileRangeAndSigningAttacks()
    {
        var valid = Encoding.UTF8.GetString(ResponseJson(true, new string('3', 64), null, null));
        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.Parse(Encoding.UTF8.GetBytes(
            valid.Replace("\"submitted\":true", "\"submitted\":true,\"submitted\":false", StringComparison.Ordinal))));
        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.Parse(Encoding.UTF8.GetBytes(
            valid.Replace("\"counterparty_chain\":\"bsc-mainnet\"", "\"counterparty_chain\":\"ethereum-mainnet\"", StringComparison.Ordinal))));
        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.Parse(Encoding.UTF8.GetBytes(
            valid.Replace("\"range_end_height\":9", "\"range_end_height\":1", StringComparison.Ordinal))));

        var payload = Convert.ToBase64String([1, 2, 3]);
        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.Parse(ResponseJson(
            false,
            null,
            payload,
            Convert.ToBase64String(new byte[32]))));
    }

    [Fact]
    public void DiscoveryRejectsRetiredCodecAndOwnerEmitterIdentity()
    {
        var exact = CapabilitiesJson("evm_address20", inboundLane: null);
        Assert.Equal(Enum.GetValues<SccpCodecV1>(), SccpCapabilities.Parse(exact).Codecs.Select(static item => item.Codec));
        Assert.Throws<ArgumentException>(() => SccpCapabilities.Parse(CapabilitiesJson("evm_hex", inboundLane: null)));

        var lane = """
        {"source_profile":"bsc-mainnet","target_profile":"sora-taira","source_domain":2,"target_domain":0,"source_identity_hash":"0xREVISION","source_identity":{"lane":{"source":{"network":"bsc_mainnet","profile":null},"target":{"network":"sora_taira","profile":null}},"emitter":{"emitter":"evm","identity":{"address":"ADDRESS","runtime_code_hash":"RUNTIME","owner":"OWNER"}}},"admission_enabled":false,"native_admission":null,"native_proof_builder":null}
        """
            .Replace("REVISION", new string('7', 64), StringComparison.Ordinal)
            .Replace("ADDRESS", new string('A', 40), StringComparison.Ordinal)
            .Replace("RUNTIME", new string('B', 64), StringComparison.Ordinal)
            .Replace("OWNER", new string('C', 64), StringComparison.Ordinal);
        Assert.Throws<ArgumentException>(() => SccpCapabilities.Parse(CapabilitiesJson("evm_address20", lane)));
    }

    [Fact]
    public async Task ToriiClientPostsExactNativeRequestAndParsesTypedResponse()
    {
        var pair = Ed25519KeyPair.FromSeed(Enumerable.Repeat((byte)8, 32).ToArray());
        var authority = pair.ToAccountAddress().ToI105();
        var artifact = Convert.ToBase64String(NoritoCodec.Encode("iroha_sccp::NativeProofV1", [1]));
        var handler = new StubHandler(request =>
        {
            Assert.Equal(HttpMethod.Post, request.Method);
            Assert.Equal("/v1/bridge/messages", request.RequestUri!.AbsolutePath);
            Assert.Equal("application/json", request.Content!.Headers.ContentType!.MediaType);
            var body = request.Content.ReadAsStringAsync().GetAwaiter().GetResult();
            Assert.Contains("\"native_proof_b64\"", body, StringComparison.Ordinal);
            Assert.DoesNotContain("private_key", body, StringComparison.Ordinal);
            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new ByteArrayContent(ResponseJson(true, new string('3', 64), null, null)),
            }.WithJsonContentType();
        });
        using var client = new ToriiClient(new Uri("https://example.test"), new HttpClient(handler));
        var response = await client.SubmitSccpBridgeMessageAsync(
            new SccpBridgeMessageSubmitRequest(authority, artifact),
            cancellationToken: TestContext.Current.CancellationToken);
        Assert.True(response.Submitted);
    }

    private static byte[] ResponseJson(
        bool submitted,
        string? txHash,
        string? transactionPayload,
        string? signingMessage)
    {
        var tx = txHash is null ? "null" : $"\"{txHash}\"";
        var payload = transactionPayload is null ? "null" : $"\"{transactionPayload}\"";
        var signing = signingMessage is null ? "null" : $"\"{signingMessage}\"";
        return Encoding.UTF8.GetBytes(
            $$"""{"submitted":{{submitted.ToString().ToLowerInvariant()}},"payload_kind":"transfer","message_id_hex":"{{new string('1', 64)}}","backend":"bridge/sccp/native/bsc-parlia-v1","counterparty_domain":2,"counterparty_chain":"bsc-mainnet","manifest_hash_hex":"{{new string('2', 64)}}","range_start_height":4,"range_end_height":9,"creation_time_ms":7,"tx_hash_hex":{{tx}},"transaction_payload_b64":{{payload}},"signing_message_b64":{{signing}}}""");
    }

    private static byte[] CapabilitiesJson(string evmCodecKey, string? inboundLane)
    {
        var lanes = inboundLane is null ? string.Empty : inboundLane;
        return Encoding.UTF8.GetBytes(
            $$"""{"version":1,"registry_revision":"0x{{new string('1', 64)}}","native_message_submit_path":"/v1/bridge/messages","outbound":{"message_bundle_path":"/v1/sccp/proofs/message/{message_id}","proof_artifact_path":"/v1/sccp/artifacts/message/{message_id}","proof_job_path":"/v1/sccp/jobs/message/{message_id}","recent_messages_path":"/v1/sccp/messages/recent","manifest_path":"/v1/sccp/manifests"},"message_payload_kinds":["asset_register","route_activate","transfer","token_add","token_pause","token_resume"],"codecs":[{"id":1,"key":"canonical_text","description":"Canonical text."},{"id":2,"key":"{{evmCodecKey}}","description":"EVM address."},{"id":3,"key":"solana_pubkey32","description":"Solana key."},{"id":4,"key":"ton_account36","description":"TON account."},{"id":5,"key":"tron_address21","description":"TRON address."},{"id":6,"key":"sora_asset_id","description":"SORA asset id."}],"inbound_lanes":[{{lanes}}]}""");
    }

    private sealed class StubHandler(Func<HttpRequestMessage, HttpResponseMessage> handler) : HttpMessageHandler
    {
        protected override Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken) => Task.FromResult(handler(request));
    }
}

internal static class SccpTestHttpExtensions
{
    internal static HttpResponseMessage WithJsonContentType(this HttpResponseMessage response)
    {
        response.Content.Headers.ContentType = new System.Net.Http.Headers.MediaTypeHeaderValue("application/json");
        return response;
    }
}
