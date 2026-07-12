using System.Buffers.Binary;
using System.Net;
using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Offline;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class OfflineToriiApiTests
{
    private const byte CompactLengthFlag = 0x02;
    private const string TopUpSchema = "iroha.torii.v1.offline.top_up.request";
    private const string RedeemSchema = "iroha.torii.v1.offline.redeem.request";
    private const string ReferenceSchema = "iroha_torii_shared::offline_api::OfflineOperationReference";
    private const string StatusSchema = "iroha_torii_shared::offline_api::OfflineOperationStatus";
    private static readonly byte[] OperationIdBytes = Enumerable.Repeat((byte)0x11, 32).ToArray();
    private static readonly string OperationId = new('1', 64);
    private static readonly string TransactionHash = new('2', 64);
    private static readonly string EvaluatedBlockHash = new('a', 64);
    private const string CanonicalAssetDefinitionId = "7EAD8EFYUx1aVKZPUU1fyKvr8dF1";

    private static string ActiveTransferVerifierJson(
        string version = "7",
        string commitment = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        string publicInputsSchemaHash = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        string maxProofBytes = "4096",
        string activationHeight = "1",
        string withdrawalHeight = "null",
        string extra = "") =>
        $$"""{"id":{"backend":"halo2-ipa-pasta","name":"transfer-v2"},"version":{{version}},"circuit_id":"confidential-transfer-v2","commitment":"{{commitment}}","public_inputs_schema_hash":"{{publicInputsSchemaHash}}","max_proof_bytes":{{maxProofBytes}},"activation_height":{{activationHeight}},"withdrawal_height":{{withdrawalHeight}}{{extra}}}""";

    private static string ActiveTopUpShieldVerifierJson(
        string version = "3",
        string commitment = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        string publicInputsSchemaHash = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        string maxProofBytes = "196608",
        string activationHeight = "2",
        string withdrawalHeight = "null",
        string extra = "") =>
        $$"""{"id":{"backend":"halo2-ipa-pasta","name":"kagemusha-topup-shield-v2"},"version":{{version}},"circuit_id":"kagemusha-topup-shield-v2","commitment":"{{commitment}}","public_inputs_schema_hash":"{{publicInputsSchemaHash}}","max_proof_bytes":{{maxProofBytes}},"activation_height":{{activationHeight}},"withdrawal_height":{{withdrawalHeight}}{{extra}}}""";

    private static string ReadinessJson(
        string assetScale = "28",
        string? activeTransferVerifier = null,
        string? activeTopUpShieldVerifier = null,
        string ready = "true",
        string blockers = "[]",
        string extra = "",
        string assetDefinitionId = CanonicalAssetDefinitionId) =>
        $$"""{"asset_definition_id":"{{assetDefinitionId}}","asset_scale":{{assetScale}},"evaluated_block_height":42,"evaluated_block_hash":"{{EvaluatedBlockHash}}","active_transfer_verifier":{{activeTransferVerifier ?? ActiveTransferVerifierJson()}},"active_topup_shield_verifier":{{activeTopUpShieldVerifier ?? ActiveTopUpShieldVerifierJson()}},"ready":{{ready}},"blockers":{{blockers}}{{extra}}}""";

    [Fact]
    public void RequestArchivesDeriveOperationIdAndDefensivelyCopyBytes()
    {
        var topUpArchive = RequestArchive(TopUpSchema, 8, 6, OperationIdBytes);
        var redeemArchive = RequestArchive(RedeemSchema, 11, 9, OperationIdBytes);
        var expectedTopUp = (byte[])topUpArchive.Clone();
        var expectedRedeem = (byte[])redeemArchive.Clone();

        var topUp = new OfflineTopUpRequest(topUpArchive);
        var redeem = new OfflineRedeemRequest(redeemArchive);
        topUpArchive[^1] ^= 0xff;
        redeemArchive[^1] ^= 0xff;

        Assert.Equal(OperationId, topUp.OperationId);
        Assert.Equal(OperationId, redeem.OperationId);
        Assert.Equal(expectedTopUp, topUp.NoritoArchive());
        Assert.Equal(expectedRedeem, redeem.NoritoArchive());

        var exposed = topUp.NoritoArchive();
        exposed[0] ^= 0xff;
        Assert.Equal(expectedTopUp, topUp.NoritoArchive());
    }

    [Theory]
    [InlineData("wrong_schema")]
    [InlineData("checksum")]
    [InlineData("non_compact")]
    [InlineData("packed_flag")]
    [InlineData("compression")]
    [InlineData("header_padding")]
    [InlineData("nonzero_padding")]
    [InlineData("missing_field")]
    [InlineData("extra_field")]
    [InlineData("zero_operation_id")]
    [InlineData("short_operation_id")]
    [InlineData("overlong_field_length")]
    [InlineData("trailing_byte")]
    [InlineData("declared_length_overflow")]
    public void TopUpRequestRejectsNonCanonicalOrAmbiguousArchives(string mutation)
    {
        var archive = mutation switch
        {
            "wrong_schema" => RequestArchive(RedeemSchema, 8, 6, OperationIdBytes),
            "checksum" => MutatePayload(RequestArchive(TopUpSchema, 8, 6, OperationIdBytes)),
            "non_compact" => RequestArchive(TopUpSchema, 8, 6, OperationIdBytes, flags: 0),
            "packed_flag" => RequestArchive(TopUpSchema, 8, 6, OperationIdBytes, flags: 0x03),
            "compression" => MutateClone(
                RequestArchive(TopUpSchema, 8, 6, OperationIdBytes),
                clone => clone[22] = 1),
            "header_padding" => AddHeaderPadding(
                RequestArchive(TopUpSchema, 8, 6, OperationIdBytes),
                [0]),
            "nonzero_padding" => AddHeaderPadding(
                RequestArchive(TopUpSchema, 8, 6, OperationIdBytes),
                [0x7f]),
            "missing_field" => RequestArchive(TopUpSchema, 7, 6, OperationIdBytes),
            "extra_field" => RequestArchive(TopUpSchema, 9, 6, OperationIdBytes),
            "zero_operation_id" => RequestArchive(TopUpSchema, 8, 6, new byte[32]),
            "short_operation_id" => RequestArchive(TopUpSchema, 8, 6, new byte[31]),
            "overlong_field_length" => OverlongFirstFieldArchive(TopUpSchema, 8, 6, OperationIdBytes),
            "trailing_byte" => [.. RequestArchive(TopUpSchema, 8, 6, OperationIdBytes), 0],
            "declared_length_overflow" => MutateClone(
                RequestArchive(TopUpSchema, 8, 6, OperationIdBytes),
                clone => BinaryPrimitives.WriteUInt64LittleEndian(clone.AsSpan(23, 8), ulong.MaxValue)),
            _ => throw new InvalidOperationException(),
        };

        Assert.ThrowsAny<ArgumentException>(() => new OfflineTopUpRequest(archive));
    }

    [Theory]
    [InlineData("wrong_schema")]
    [InlineData("missing_field")]
    [InlineData("extra_field")]
    [InlineData("zero_operation_id")]
    [InlineData("short_operation_id")]
    public void RedeemRequestRejectsWrongSchemaFieldCountAndOperationId(string mutation)
    {
        var archive = mutation switch
        {
            "wrong_schema" => RequestArchive(TopUpSchema, 11, 9, OperationIdBytes),
            "missing_field" => RequestArchive(RedeemSchema, 10, 9, OperationIdBytes),
            "extra_field" => RequestArchive(RedeemSchema, 12, 9, OperationIdBytes),
            "zero_operation_id" => RequestArchive(RedeemSchema, 11, 9, new byte[32]),
            "short_operation_id" => RequestArchive(RedeemSchema, 11, 9, new byte[33]),
            _ => throw new InvalidOperationException(),
        };

        Assert.ThrowsAny<ArgumentException>(() => new OfflineRedeemRequest(archive));
    }

    [Fact]
    public void OperationReferenceDecodesRustGoldenVector()
    {
        const string golden =
            "4e5254300000e8e2244e45e4be2a975e34957141128b00f0000000000000001f5b5402d6dc2092024140313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131310400000000040000000041403232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323258572f76312f6f66666c696e652f6f7065726174696f6e732f3131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313108ffffffffffffffff";

        var reference = OfflineOperationCodec.DecodeReference(Convert.FromHexString(golden));

        Assert.Equal(OperationId, reference.OperationId);
        Assert.Equal(OfflineOperationKind.TopUp, reference.Kind);
        Assert.Equal(OfflineOperationState.Pending, reference.State);
        Assert.Equal(TransactionHash, reference.TransactionHash);
        Assert.Equal($"/v1/offline/operations/{OperationId}", reference.StatusUri);
        Assert.Equal(ulong.MaxValue, reference.SubmittedAtMs);
    }

    [Fact]
    public void OperationStatusDecodesAllRustGoldenVariants()
    {
        const string pendingGolden =
            "4e5254300000fb04214104df1bdcd39249bddd4db23a009600000000000000bdfee2508f80055702000000000000000000000000414031313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131040000000041403232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323208ffffffffffffffff";
        const string rejectedGolden =
            "4e5254300000fb04214104df1bdcd39249bddd4db23a00b6000000000000009322104cda8e602a020000000000000000020000004140313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131310401000000414032323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232281b1a6f66666c696e655f6f7065726174696f6e5f72656a6563746564090872656a65637465640100";
        const string appliedRedeemGolden =
            "4e5254300000fb04214104df1bdcd39249bddd4db23a00a00000000000000092cd6b32b062b3d30200000000000000000100000041403131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313159010000005441403232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323208ffffffffffffffff082a00000000000000";

        var pending = Assert.IsType<OfflineOperationStatus.Pending>(
            OfflineOperationCodec.DecodeStatus(Convert.FromHexString(pendingGolden)));
        Assert.Equal(OperationId, pending.OperationId);
        Assert.Equal(OfflineOperationKind.TopUp, pending.Kind);
        Assert.Equal(TransactionHash, pending.TransactionHash);
        Assert.Equal(ulong.MaxValue, pending.SubmittedAtMs);

        var rejected = Assert.IsType<OfflineOperationStatus.Rejected>(
            OfflineOperationCodec.DecodeStatus(Convert.FromHexString(rejectedGolden)));
        Assert.Equal(OfflineOperationKind.Redeem, rejected.Kind);
        Assert.Equal("offline_operation_rejected", rejected.Error.Code);
        Assert.Equal("rejected", rejected.Error.Message);
        Assert.Null(rejected.Error.Details);

        var applied = Assert.IsType<OfflineOperationStatus.Applied>(
            OfflineOperationCodec.DecodeStatus(Convert.FromHexString(appliedRedeemGolden)));
        var redeem = Assert.IsType<OfflineOperationResult.Redeem>(applied.Result);
        Assert.Equal(TransactionHash, redeem.Value.TransactionHash);
        Assert.Equal(ulong.MaxValue, redeem.Value.FinalizedBlockHeight);
        Assert.Equal(42UL, redeem.Value.ServerTimeMs);
    }

    [Fact]
    public void OperationStatusDecodesTypedTopUpAnchor()
    {
        var archive = AppliedTopUpStatusArchive(OperationId, TransactionHash);

        var applied = Assert.IsType<OfflineOperationStatus.Applied>(
            OfflineOperationCodec.DecodeStatus(archive));
        var topUp = Assert.IsType<OfflineOperationResult.TopUp>(applied.Result);

        Assert.Equal(TransactionHash, topUp.Value.TransactionHash);
        Assert.Equal(42UL, topUp.Value.FinalizedBlockHeight);
        Assert.Equal(84UL, topUp.Value.ServerTimeMs);
        var anchor = topUp.Value.Anchor.NoritoArchive();
        Assert.Equal("NRT0"u8.ToArray(), anchor[..4]);
        Assert.Equal(CompactLengthFlag, anchor[39]);
        anchor[0] ^= 0xff;
        Assert.Equal((byte)'N', topUp.Value.Anchor.NoritoArchive()[0]);
        var finalityProof = topUp.Value.FinalityProof.NoritoArchive();
        Assert.Equal("NRT0"u8.ToArray(), finalityProof[..4]);
        Assert.Equal(CompactLengthFlag, finalityProof[39]);
        finalityProof[0] ^= 0xff;
        Assert.Equal((byte)'N', topUp.Value.FinalityProof.NoritoArchive()[0]);
    }

    [Fact]
    public void AppliedResultsRejectZeroFinalityFields()
    {
        var applied = Assert.IsType<OfflineOperationStatus.Applied>(
            OfflineOperationCodec.DecodeStatus(
                AppliedTopUpStatusArchive(OperationId, TransactionHash)));
        var anchor = Assert.IsType<OfflineOperationResult.TopUp>(applied.Result).Value.Anchor;
        var finalityProof = Assert.IsType<OfflineOperationResult.TopUp>(applied.Result)
            .Value.FinalityProof;

        foreach (var (finalizedBlockHeight, serverTimeMs) in new[]
                 {
                     (0UL, 1UL),
                     (1UL, 0UL),
                 })
        {
            Assert.Throws<ArgumentOutOfRangeException>(() => new OfflineTopUpResult(
                TransactionHash,
                finalizedBlockHeight,
                serverTimeMs,
                anchor,
                finalityProof));
            Assert.Throws<ArgumentOutOfRangeException>(() => new OfflineRedeemResult(
                TransactionHash,
                finalizedBlockHeight,
                serverTimeMs));
        }
    }

    [Fact]
    public void OperationStatusDecodesClosedTypedRejectionDetails()
    {
        var rejected = Assert.IsType<OfflineOperationStatus.Rejected>(
            OfflineOperationCodec.DecodeStatus(RejectedStatusWithDetailsArchive()));

        Assert.Equal("offline_operation_rejected", rejected.Error.Code);
        var details = Assert.IsType<OfflineOperationErrorDetails>(rejected.Error.Details);
        Assert.Equal("torii", details.Layer);
        Assert.Equal("queue_full", details.RejectCode);
        Assert.Equal(3UL, details.RetryAfterSeconds);
        Assert.Equal((ushort)369, details.ChainDiscriminant);
        Assert.Equal(TransactionHash, details.TransactionHash);
        Assert.Equal("saturated", details.Queue!.State);
        Assert.Equal(7UL, details.Queue.Queued);
        Assert.Equal(8UL, details.Queue.Capacity);
        Assert.True(details.Queue.Saturated);
        Assert.Equal("axt_stale", details.Axt!.Code);
        Assert.Equal(12UL, details.Axt.SnapshotVersion);
        Assert.Equal(13UL, details.Axt.Dataspace);
        Assert.Equal(14U, details.Axt.Lane);
        Assert.Equal(15UL, details.Axt.NextMinHandleEra);
        Assert.Equal(16UL, details.Axt.NextMinSubNonce);
    }

    [Theory]
    [InlineData("unknown_tag")]
    [InlineData("missing_padding")]
    [InlineData("nonzero_padding")]
    [InlineData("checksum")]
    [InlineData("trailing")]
    [InlineData("malformed_anchor")]
    [InlineData("missing_finality_proof")]
    public void OperationStatusRejectsAdversarialFramingAndVariants(string mutation)
    {
        var canonical = AppliedTopUpStatusArchive(OperationId, TransactionHash);
        var archive = mutation switch
        {
            "unknown_tag" => MutateStatusPayload(canonical, payload =>
            {
                BinaryPrimitives.WriteUInt32LittleEndian(payload, 99);
            }),
            "missing_padding" => RemoveStatusPadding(canonical),
            "nonzero_padding" => MutateClone(canonical, clone => clone[NoritoHeader.EncodedLength] = 1),
            "checksum" => MutateClone(canonical, clone => clone[^1] ^= 0xff),
            "trailing" => [.. canonical, 0],
            "malformed_anchor" => AppliedTopUpStatusArchive(
                OperationId,
                TransactionHash,
                zeroAnchorDigest: true),
            "missing_finality_proof" => AppliedTopUpStatusArchive(
                OperationId,
                TransactionHash,
                omitFinalityProof: true),
            _ => throw new InvalidOperationException(),
        };

        Assert.ThrowsAny<ArgumentException>(() => OfflineOperationCodec.DecodeStatus(archive));
    }

    [Fact]
    public async Task ToriiClientUsesExactReadinessRouteQueryAndJsonNegotiation()
    {
        using var handler = new CaptureHandler(_ => JsonResponse(ReadinessJson(
            ready: "false",
            blockers: "[{\"code\":\"proof_backend_unavailable\",\"message\":\"Proof backend unavailable.\"}]")));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var readiness = await client.GetOfflineReadinessAsync(
            "xor#wonderland",
            TestContext.Current.CancellationToken);

        Assert.False(readiness.Ready);
        Assert.Equal(28U, readiness.AssetScale);
        Assert.Equal(42UL, readiness.EvaluatedBlockHeight);
        Assert.Equal(EvaluatedBlockHash, readiness.EvaluatedBlockHash);
        Assert.NotNull(readiness.ActiveTransferVerifier);
        Assert.Equal("halo2-ipa-pasta", readiness.ActiveTransferVerifier.Id.Backend);
        Assert.Equal(4096U, readiness.ActiveTransferVerifier.MaxProofBytes);
        Assert.NotNull(readiness.ActiveTopUpShieldVerifier);
        Assert.Equal("kagemusha-topup-shield-v2", readiness.ActiveTopUpShieldVerifier.Id.Name);
        Assert.Equal(196608U, readiness.ActiveTopUpShieldVerifier.MaxProofBytes);
        Assert.Equal("proof_backend_unavailable", Assert.Single(readiness.Blockers).Code);
        Assert.Equal(HttpMethod.Get, handler.Last!.Method);
        Assert.Equal(
            "/v1/offline/readiness?asset_definition_id=xor%23wonderland",
            handler.Last.PathAndQuery);
        Assert.Equal("application/json", handler.Last.Accept);
        Assert.Empty(handler.Last.Body);
    }

    [Theory]
    [InlineData("")]
    [InlineData("different-asset")]
    [InlineData("XOR#wonderland")]
    [InlineData(" xor#wonderland")]
    public async Task ToriiClientRejectsMalformedReadinessSelectorsBeforeTransport(string selector)
    {
        using var handler = new CaptureHandler(_ => JsonResponse(ReadinessJson()));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        await Assert.ThrowsAnyAsync<ArgumentException>(() =>
            client.GetOfflineReadinessAsync(selector, TestContext.Current.CancellationToken));
        Assert.Null(handler.Last);
    }

    [Fact]
    public async Task ToriiClientBindsCanonicalReadinessSelectorButAllowsAliasResolution()
    {
        using var handler = new CaptureHandler(_ => JsonResponse(ReadinessJson()));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var alias = await client.GetOfflineReadinessAsync(
            "xor#wonderland",
            TestContext.Current.CancellationToken);
        Assert.Equal(CanonicalAssetDefinitionId, alias.AssetDefinitionId);

        var error = await Assert.ThrowsAsync<InvalidDataException>(() =>
            client.GetOfflineReadinessAsync(
                "61CtjvNd9T3THAR65GsMVHr82Bjc",
                TestContext.Current.CancellationToken));
        Assert.Contains("does not match the requested asset definition", error.Message);
    }

    [Theory]
    [InlineData("{}")]
    [InlineData("null")]
    [InlineData("[]")]
    [InlineData("{\"asset_definition_id\":\"7EAD8EFYUx1aVKZPUU1fyKvr8dF1\",\"asset_definition_id\":\"7EAD8EFYUx1aVKZPUU1fyKvr8dF1\",\"evaluated_block_height\":1,\"evaluated_block_hash\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"ready\":true,\"blockers\":[]}")]
    [InlineData("{\"asset_definition_id\":\"7EAD8EFYUx1aVKZPUU1fyKvr8dF1\",\"evaluated_block_height\":-1,\"evaluated_block_hash\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"ready\":true,\"blockers\":[]}")]
    [InlineData("{\"asset_definition_id\":\"7EAD8EFYUx1aVKZPUU1fyKvr8dF1\",\"evaluated_block_height\":1,\"evaluated_block_hash\":\"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\",\"ready\":true,\"blockers\":[]}")]
    [InlineData("{\"asset_definition_id\":\"7EAD8EFYUx1aVKZPUU1fyKvr8dF1\",\"evaluated_block_height\":1,\"evaluated_block_hash\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"ready\":true,\"blockers\":[]}")]
    [InlineData("{\"asset_definition_id\":\"7EAD8EFYUx1aVKZPUU1fyKvr8dF1\",\"evaluated_block_height\":1,\"evaluated_block_hash\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"ready\":\"true\",\"blockers\":[]}")]
    [InlineData("{\"asset_definition_id\":\"7EAD8EFYUx1aVKZPUU1fyKvr8dF1\",\"evaluated_block_height\":1,\"evaluated_block_hash\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"ready\":true,\"blockers\":[{\"code\":\"blocked\",\"message\":\"blocked\"}]}")]
    [InlineData("{\"asset_definition_id\":\"7EAD8EFYUx1aVKZPUU1fyKvr8dF1\",\"evaluated_block_height\":1,\"evaluated_block_hash\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"ready\":false,\"blockers\":[]}")]
    [InlineData("{\"asset_definition_id\":\"7EAD8EFYUx1aVKZPUU1fyKvr8dF1\",\"evaluated_block_height\":1,\"evaluated_block_hash\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"ready\":false,\"blockers\":[{\"code\":\"BAD\",\"message\":\"blocked\"}]}")]
    public void ReadinessJsonRejectsMissingDuplicateAndTypeConfusedFields(string json)
    {
        Assert.Throws<JsonException>(() => JsonSerializer.Deserialize<OfflineReadiness>(json));
    }

    [Fact]
    public void ReadinessJsonIgnoresUnknownObjectMembers()
    {
        var verifier = ActiveTransferVerifierJson(
            extra: ",\"future_verifier_detail\":{\"ignored\":true}");
        var topUpShieldVerifier = ActiveTopUpShieldVerifierJson(
            extra: ",\"future_shield_detail\":{\"ignored\":true}");
        var json = ReadinessJson(
            activeTransferVerifier: verifier,
            activeTopUpShieldVerifier: topUpShieldVerifier,
            ready: "false",
            blockers: "[{\"code\":\"2fa_required\",\"message\":\"blocked\",\"future_detail\":7}]",
            extra: ",\"future_top_level\":{\"ignored\":true}");

        var readiness = JsonSerializer.Deserialize<OfflineReadiness>(json);
        Assert.NotNull(readiness);
        Assert.Equal("2fa_required", Assert.Single(readiness.Blockers).Code);
        Assert.Equal("transfer-v2", readiness.ActiveTransferVerifier!.Id.Name);
        Assert.Equal(
            "kagemusha-topup-shield-v2",
            readiness.ActiveTopUpShieldVerifier!.Id.Name);
    }

    [Fact]
    public async Task ToriiClientAppliesTheSharedReadinessJsonDepthLimit()
    {
        var acceptedNested = new string('[', 65) + "0" + new string(']', 65);
        using (var handler = new CaptureHandler(_ => JsonResponse(
                   ReadinessJson(extra: $",\"future_nested\":{acceptedNested}"))))
        using (var client = new ToriiClient(
                   new Uri("https://torii.example"),
                   new HttpClient(handler)))
        {
            var readiness = await client.GetOfflineReadinessAsync(
                "xor#wonderland",
                TestContext.Current.CancellationToken);
            Assert.Equal(CanonicalAssetDefinitionId, readiness.AssetDefinitionId);
        }

        var rejectedNested = new string('[', 129) + "0" + new string(']', 129);
        using var rejectedHandler = new CaptureHandler(_ => JsonResponse(
            ReadinessJson(extra: $",\"future_nested\":{rejectedNested}")));
        using var rejectedClient = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(rejectedHandler));
        await Assert.ThrowsAnyAsync<JsonException>(() =>
            rejectedClient.GetOfflineReadinessAsync(
                "xor#wonderland",
                TestContext.Current.CancellationToken));
    }

    [Fact]
    public void ReadinessJsonPreservesExpectedUnavailableScaleAndNullableCapabilities()
    {
        var unsupportedScale = JsonSerializer.Deserialize<OfflineReadiness>(ReadinessJson(
            assetScale: "29",
            ready: "false",
            blockers: "[{\"code\":\"asset_scale_unsupported\",\"message\":\"unsupported\"}]"));
        Assert.NotNull(unsupportedScale);
        Assert.Equal(29U, unsupportedScale.AssetScale);
        Assert.Equal("asset_scale_unsupported", Assert.Single(unsupportedScale.Blockers).Code);

        var unavailable = JsonSerializer.Deserialize<OfflineReadiness>(ReadinessJson(
            assetScale: "null",
            activeTransferVerifier: "null",
            activeTopUpShieldVerifier: "null",
            ready: "false",
            blockers: "[{\"code\":\"asset_scale_unavailable\",\"message\":\"no scale\"},{\"code\":\"transfer_verifier_unavailable\",\"message\":\"no verifier\"},{\"code\":\"topup_shield_verifier_unavailable\",\"message\":\"no shield verifier\"}]"));
        Assert.NotNull(unavailable);
        Assert.Null(unavailable.AssetScale);
        Assert.Null(unavailable.ActiveTransferVerifier);
        Assert.Null(unavailable.ActiveTopUpShieldVerifier);

        var shieldUnavailable = JsonSerializer.Deserialize<OfflineReadiness>(ReadinessJson(
            activeTopUpShieldVerifier: "null",
            ready: "false",
            blockers: "[{\"code\":\"topup_shield_verifier_unavailable\",\"message\":\"no shield verifier\"}]"));
        Assert.NotNull(shieldUnavailable);
        Assert.NotNull(shieldUnavailable.ActiveTransferVerifier);
        Assert.Null(shieldUnavailable.ActiveTopUpShieldVerifier);

        var encoded = JsonSerializer.Serialize(shieldUnavailable);
        using var encodedDocument = JsonDocument.Parse(encoded);
        Assert.Equal(
            JsonValueKind.Null,
            encodedDocument.RootElement.GetProperty("active_topup_shield_verifier").ValueKind);
        var roundTripped = JsonSerializer.Deserialize<OfflineReadiness>(encoded);
        Assert.NotNull(roundTripped);
        Assert.Null(roundTripped.ActiveTopUpShieldVerifier);
        Assert.Equal(
            "topup_shield_verifier_unavailable",
            Assert.Single(roundTripped.Blockers).Code);
    }

    [Fact]
    public void ReadinessJsonRejectsAdversarialSnapshotFields()
    {
        var missingVerifierField = ActiveTransferVerifierJson().Replace(
            "\"max_proof_bytes\":4096,",
            string.Empty,
            StringComparison.Ordinal);
        var missingTopUpShieldVerifierField = ActiveTopUpShieldVerifierJson().Replace(
            "\"max_proof_bytes\":196608,",
            string.Empty,
            StringComparison.Ordinal);
        var missingTopUpShieldSnapshotField = ReadinessJson().Replace(
            $",\"active_topup_shield_verifier\":{ActiveTopUpShieldVerifierJson()}",
            string.Empty,
            StringComparison.Ordinal);
        string[] invalid =
        [
            ReadinessJson(assetScale: "-1"),
            ReadinessJson(assetScale: "4294967296"),
            ReadinessJson(assetScale: "\"28\""),
            ReadinessJson(assetScale: "29"),
            ReadinessJson(assetScale: "null", ready: "false", blockers: "[{\"code\":\"proof_backend_unavailable\",\"message\":\"blocked\"}]"),
            ReadinessJson(activeTransferVerifier: "null", ready: "false", blockers: "[{\"code\":\"proof_backend_unavailable\",\"message\":\"blocked\"}]"),
            ReadinessJson(activeTopUpShieldVerifier: "null", ready: "false", blockers: "[{\"code\":\"proof_backend_unavailable\",\"message\":\"blocked\"}]"),
            ReadinessJson(ready: "false", blockers: "[{\"code\":\"topup_shield_verifier_unavailable\",\"message\":\"blocked\"}]"),
            ReadinessJson(activeTransferVerifier: ActiveTransferVerifierJson(maxProofBytes: "0")),
            ReadinessJson(activeTransferVerifier: ActiveTransferVerifierJson(activationHeight: "43")),
            ReadinessJson(activeTransferVerifier: ActiveTransferVerifierJson(withdrawalHeight: "42")),
            ReadinessJson(activeTransferVerifier: ActiveTransferVerifierJson(version: "-1")),
            ReadinessJson(activeTransferVerifier: ActiveTransferVerifierJson(version: "4294967296")),
            ReadinessJson(activeTransferVerifier: ActiveTransferVerifierJson(commitment: "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB")),
            ReadinessJson(activeTransferVerifier: ActiveTransferVerifierJson(extra: ",\"version\":8")),
            ReadinessJson(activeTransferVerifier: missingVerifierField),
            ReadinessJson(activeTopUpShieldVerifier: ActiveTopUpShieldVerifierJson(maxProofBytes: "0")),
            ReadinessJson(activeTopUpShieldVerifier: ActiveTopUpShieldVerifierJson(activationHeight: "43")),
            ReadinessJson(activeTopUpShieldVerifier: ActiveTopUpShieldVerifierJson(withdrawalHeight: "42")),
            ReadinessJson(activeTopUpShieldVerifier: ActiveTopUpShieldVerifierJson(version: "-1")),
            ReadinessJson(activeTopUpShieldVerifier: ActiveTopUpShieldVerifierJson(version: "4294967296")),
            ReadinessJson(activeTopUpShieldVerifier: ActiveTopUpShieldVerifierJson(extra: ",\"version\":4")),
            ReadinessJson(activeTopUpShieldVerifier: "[]"),
            ReadinessJson(activeTopUpShieldVerifier: missingTopUpShieldVerifierField),
            ReadinessJson(extra: $",\"active_topup_shield_verifier\":{ActiveTopUpShieldVerifierJson()}"),
            missingTopUpShieldSnapshotField,
            ReadinessJson(
                ready: "false",
                blockers: "[{\"code\":\"blocked\",\"message\":\"one\"},{\"code\":\"blocked\",\"message\":\"two\"}]"),
        ];

        foreach (var json in invalid)
        {
            Assert.Throws<JsonException>(() => JsonSerializer.Deserialize<OfflineReadiness>(json));
        }
    }

    [Fact]
    public void ReadinessTextUsesUnicodeScalarLimitsAndRejectsMalformedText()
    {
        var boundary = new string('x', 1023) + "😀";
        var blocker = new OfflineReadinessBlocker("blocked", boundary);
        Assert.Equal(1025, blocker.Message.Length);

        Assert.Throws<ArgumentException>(() =>
            new OfflineReadinessBlocker("blocked", new string('x', 1024) + "😀"));
        Assert.Throws<ArgumentException>(() =>
            new OfflineReadinessBlocker("blocked", "line\u0001break"));
        Assert.Throws<ArgumentException>(() =>
            new OfflineReadinessBlocker("blocked", new string('\uD800', 1)));
        Assert.Throws<ArgumentException>(() =>
            new OfflineVerifierId(new string('\uD800', 1), "transfer"));
    }

    [Fact]
    public void ReadinessJsonWritesEveryRequiredSnapshotField()
    {
        var readiness = JsonSerializer.Deserialize<OfflineReadiness>(ReadinessJson());
        Assert.NotNull(readiness);

        var encoded = JsonSerializer.Serialize(readiness);
        using var document = JsonDocument.Parse(encoded);
        var root = document.RootElement;
        foreach (var field in new[]
        {
            "asset_definition_id",
            "asset_scale",
            "evaluated_block_height",
            "evaluated_block_hash",
            "active_transfer_verifier",
            "active_topup_shield_verifier",
            "ready",
            "blockers",
        })
        {
            Assert.True(root.TryGetProperty(field, out _), $"missing serialized field {field}");
        }
        Assert.Equal(
            "confidential-transfer-v2",
            root.GetProperty("active_transfer_verifier").GetProperty("circuit_id").GetString());
        Assert.Equal(
            "kagemusha-topup-shield-v2",
            root.GetProperty("active_topup_shield_verifier").GetProperty("circuit_id").GetString());
    }

    [Fact]
    public void StableErrorCodesUseTheFiniteFirstReleaseGrammar()
    {
        Assert.Equal(
            "2fa_required",
            new OfflineReadinessBlocker("2fa_required", "blocked").Code);
        Assert.Equal(
            "9future_code",
            new OfflineOperationErrorEnvelope("9future_code", "unknown but valid").Code);

        Assert.Throws<ArgumentException>(() =>
            new OfflineOperationErrorEnvelope("_invalid", "invalid"));
        Assert.Throws<ArgumentException>(() =>
            new OfflineOperationErrorEnvelope(new string('a', 65), "invalid"));
    }

    [Fact]
    public async Task ToriiClientSubmitsDirectNoritoBodyAndDerivedIdempotencyKey()
    {
        var requestArchive = RequestArchive(TopUpSchema, 8, 6, OperationIdBytes);
        var responseArchive = ReferenceArchive(OperationId, OfflineOperationKind.TopUp, TransactionHash);
        using var handler = new CaptureHandler(_ => BinaryResponse(
            responseArchive,
            HttpStatusCode.Accepted,
            $"/v1/offline/operations/{OperationId}"));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var reference = await client.SubmitOfflineTopUpAsync(
            new OfflineTopUpRequest(requestArchive),
            TestContext.Current.CancellationToken);

        Assert.Equal(OperationId, reference.OperationId);
        Assert.Equal(HttpMethod.Post, handler.Last!.Method);
        Assert.Equal("/v1/offline/top-up", handler.Last.PathAndQuery);
        Assert.Equal("application/x-norito", handler.Last.ContentType);
        Assert.Equal("application/x-norito", handler.Last.Accept);
        Assert.Equal(OperationId, handler.Last.IdempotencyKey);
        Assert.Equal(requestArchive, handler.Last.Body);
        Assert.DoesNotContain("base64", Encoding.ASCII.GetString(handler.Last.Body), StringComparison.OrdinalIgnoreCase);
    }

    [Fact]
    public async Task ToriiClientRedeemUsesExactRouteAndEmbeddedOperationId()
    {
        var requestArchive = RequestArchive(RedeemSchema, 11, 9, OperationIdBytes);
        var responseArchive = ReferenceArchive(OperationId, OfflineOperationKind.Redeem, TransactionHash);
        using var handler = new CaptureHandler(_ => BinaryResponse(
            responseArchive,
            HttpStatusCode.Accepted,
            $"/v1/offline/operations/{OperationId}"));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var reference = await client.SubmitOfflineRedeemAsync(
            new OfflineRedeemRequest(requestArchive),
            TestContext.Current.CancellationToken);

        Assert.Equal(OfflineOperationKind.Redeem, reference.Kind);
        Assert.Equal("/v1/offline/redeem", handler.Last!.PathAndQuery);
        Assert.Equal(OperationId, handler.Last.IdempotencyKey);
        Assert.Equal(requestArchive, handler.Last.Body);
    }

    [Fact]
    public async Task ToriiClientFetchesExactOperationRouteAndChecksReturnedId()
    {
        var statusArchive = PendingStatusArchive(OperationId, TransactionHash);
        using var handler = new CaptureHandler(_ => BinaryResponse(statusArchive));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var status = Assert.IsType<OfflineOperationStatus.Pending>(
            await client.GetOfflineOperationStatusAsync(
                OperationId,
                TestContext.Current.CancellationToken));

        Assert.Equal(OperationId, status.OperationId);
        Assert.Equal(HttpMethod.Get, handler.Last!.Method);
        Assert.Equal($"/v1/offline/operations/{OperationId}", handler.Last.PathAndQuery);
        Assert.Equal("application/x-norito", handler.Last.Accept);
    }

    [Theory]
    [InlineData("wrong_status")]
    [InlineData("wrong_content_type")]
    [InlineData("missing_location")]
    [InlineData("wrong_location")]
    [InlineData("wrong_operation")]
    [InlineData("wrong_kind")]
    public async Task ToriiClientRejectsInconsistentAcceptedResponses(string mutation)
    {
        var otherOperationId = new string('3', 64);
        var responseOperation = mutation == "wrong_operation" ? otherOperationId : OperationId;
        var responseKind = mutation == "wrong_kind" ? OfflineOperationKind.Redeem : OfflineOperationKind.TopUp;
        var responseArchive = ReferenceArchive(responseOperation, responseKind, TransactionHash);
        var status = mutation == "wrong_status" ? HttpStatusCode.OK : HttpStatusCode.Accepted;
        var mediaType = mutation == "wrong_content_type" ? "application/json" : "application/x-norito";
        var location = mutation switch
        {
            "missing_location" => null,
            "wrong_location" => $"/v1/offline/operations/{otherOperationId}",
            _ => $"/v1/offline/operations/{responseOperation}",
        };
        using var handler = new CaptureHandler(_ => BinaryResponse(responseArchive, status, location, mediaType));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var request = new OfflineTopUpRequest(RequestArchive(TopUpSchema, 8, 6, OperationIdBytes));

        await Assert.ThrowsAnyAsync<Exception>(() => client.SubmitOfflineTopUpAsync(
            request,
            TestContext.Current.CancellationToken));
    }

    [Theory]
    [InlineData("")]
    [InlineData("00")]
    [InlineData("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")]
    [InlineData("0000000000000000000000000000000000000000000000000000000000000000")]
    [InlineData("111111111111111111111111111111111111111111111111111111111111111g")]
    public async Task OperationLookupRejectsNonCanonicalIdsBeforeDispatch(string operationId)
    {
        using var handler = new CaptureHandler(_ => throw new InvalidOperationException("must not dispatch"));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        await Assert.ThrowsAnyAsync<ArgumentException>(() => client.GetOfflineOperationStatusAsync(
            operationId,
            TestContext.Current.CancellationToken));
        Assert.Null(handler.Last);
    }

    [Fact]
    public void FirstReleaseRouteCatalogContainsNoNestedOrLegacyOfflineVersion()
    {
        var routes = new[]
        {
            OfflineApiRoutes.Readiness,
            OfflineApiRoutes.TopUp,
            OfflineApiRoutes.Redeem,
            OfflineApiRoutes.Operations,
        };

        Assert.All(routes, route => Assert.StartsWith("/v1/offline/", route, StringComparison.Ordinal));
        Assert.DoesNotContain(routes, route => route.Contains("/v2", StringComparison.Ordinal));
        Assert.DoesNotContain(routes, route => route.Contains("notes", StringComparison.Ordinal));
        Assert.Equal(4, routes.Distinct(StringComparer.Ordinal).Count());
    }

    private static byte[] RequestArchive(
        string schema,
        int fieldCount,
        int operationIdFieldIndex,
        byte[] operationId,
        byte flags = CompactLengthFlag)
    {
        using var payload = new MemoryStream();
        for (var index = 0; index < fieldCount; index++)
        {
            var field = index == operationIdFieldIndex ? operationId : new[] { checked((byte)(index + 1)) };
            WriteCompactLength(payload, (ulong)field.Length);
            payload.Write(field);
        }
        return NoritoCodec.Encode(schema, payload.ToArray(), flags);
    }

    private static byte[] OverlongFirstFieldArchive(
        string schema,
        int fieldCount,
        int operationIdFieldIndex,
        byte[] operationId)
    {
        using var payload = new MemoryStream();
        payload.Write([0x81, 0x00, 0x01]);
        for (var index = 1; index < fieldCount; index++)
        {
            var field = index == operationIdFieldIndex ? operationId : new[] { checked((byte)(index + 1)) };
            WriteCompactLength(payload, (ulong)field.Length);
            payload.Write(field);
        }
        return NoritoCodec.Encode(schema, payload.ToArray(), CompactLengthFlag);
    }

    private static byte[] ReferenceArchive(
        string operationId,
        OfflineOperationKind kind,
        string transactionHash)
    {
        using var payload = new MemoryStream();
        WriteField(payload, StringPayload(operationId));
        WriteField(payload, UInt32Payload(kind == OfflineOperationKind.TopUp ? 0U : 1U));
        WriteField(payload, UInt32Payload(0));
        WriteField(payload, StringPayload(transactionHash));
        WriteField(payload, StringPayload($"/v1/offline/operations/{operationId}"));
        WriteField(payload, UInt64Payload(1_725_000_000_123));
        return NoritoCodec.Encode(ReferenceSchema, payload.ToArray(), CompactLengthFlag);
    }

    private static byte[] PendingStatusArchive(string operationId, string transactionHash)
    {
        using var payload = new MemoryStream();
        payload.Write(UInt32Payload(0));
        WriteField(payload, StringPayload(operationId));
        WriteField(payload, UInt32Payload(0));
        WriteField(payload, StringPayload(transactionHash));
        WriteField(payload, UInt64Payload(100));
        return AddStatusPadding(NoritoCodec.Encode(StatusSchema, payload.ToArray(), CompactLengthFlag));
    }

    private static byte[] AppliedTopUpStatusArchive(
        string operationId,
        string transactionHash,
        bool zeroAnchorDigest = false,
        bool omitFinalityProof = false)
    {
        using var anchor = new MemoryStream();
        for (var index = 0; index < 17; index++)
        {
            byte[] field = index switch
            {
                0 => UInt16Payload(2),
                6 => Enumerable.Repeat((byte)0x61, 32).ToArray(),
                7 => Enumerable.Repeat((byte)0x62, 32).ToArray(),
                10 => OperationIdBytes,
                12 => Enumerable.Repeat((byte)0x63, 32).ToArray(),
                13 => StringPayload("artifact-generation"),
                14 => UInt64Payload(42),
                15 => Enumerable.Repeat((byte)0x64, 32).ToArray(),
                16 => zeroAnchorDigest
                    ? new byte[32]
                    : Enumerable.Repeat((byte)0x65, 32).ToArray(),
                _ => new[] { checked((byte)(index + 1)) },
            };
            WriteField(anchor, field);
        }

        using var resultValue = new MemoryStream();
        WriteField(resultValue, StringPayload(transactionHash));
        WriteField(resultValue, UInt64Payload(42));
        WriteField(resultValue, UInt64Payload(84));
        WriteField(resultValue, anchor.ToArray());
        if (!omitFinalityProof)
        {
            WriteField(resultValue, [2]);
        }

        using var result = new MemoryStream();
        result.Write(UInt32Payload(0));
        WriteCompactLength(result, (ulong)resultValue.Length);
        result.Write(resultValue.ToArray());

        using var status = new MemoryStream();
        status.Write(UInt32Payload(1));
        WriteField(status, StringPayload(operationId));
        WriteField(status, result.ToArray());
        return AddStatusPadding(NoritoCodec.Encode(StatusSchema, status.ToArray(), CompactLengthFlag));
    }

    private static byte[] RejectedStatusWithDetailsArchive()
    {
        using var queue = new MemoryStream();
        WriteField(queue, StringPayload("saturated"));
        WriteField(queue, UInt64Payload(7));
        WriteField(queue, UInt64Payload(8));
        WriteField(queue, [1]);

        using var axt = new MemoryStream();
        WriteField(axt, SomeOption(StringPayload("axt_stale")));
        WriteField(axt, SomeOption(StringPayload("stale snapshot")));
        WriteField(axt, SomeOption(UInt64Payload(12)));
        WriteField(axt, SomeOption(UInt64Payload(13)));
        WriteField(axt, SomeOption(UInt32Payload(14)));
        WriteField(axt, SomeOption(UInt64Payload(15)));
        WriteField(axt, SomeOption(UInt64Payload(16)));

        using var details = new MemoryStream();
        WriteField(details, SomeOption(StringPayload("torii")));
        WriteField(details, SomeOption(StringPayload("queue_full")));
        WriteField(details, SomeOption(queue.ToArray()));
        WriteField(details, SomeOption(UInt64Payload(3)));
        WriteField(details, SomeOption(StringPayload("/v1/offline/redeem")));
        WriteField(details, SomeOption(StringPayload("request")));
        WriteField(details, SomeOption(StringPayload("capacity")));
        WriteField(details, SomeOption(StringPayload("saturated")));
        WriteField(details, SomeOption(StringPayload("taira")));
        WriteField(details, SomeOption(UInt16Payload(369)));
        WriteField(details, SomeOption(StringPayload(TransactionHash)));
        WriteField(details, SomeOption(StringPayload("queued")));
        WriteField(details, SomeOption(StringPayload("retry later")));
        WriteField(details, SomeOption(axt.ToArray()));

        using var error = new MemoryStream();
        WriteField(error, StringPayload("offline_operation_rejected"));
        WriteField(error, StringPayload("Rejected by queue policy."));
        WriteField(error, SomeOption(details.ToArray()));

        using var status = new MemoryStream();
        status.Write(UInt32Payload(2));
        WriteField(status, StringPayload(OperationId));
        WriteField(status, UInt32Payload(1));
        WriteField(status, StringPayload(TransactionHash));
        WriteField(status, error.ToArray());
        return AddStatusPadding(NoritoCodec.Encode(StatusSchema, status.ToArray(), CompactLengthFlag));
    }

    private static byte[] SomeOption(byte[] value)
    {
        using var output = new MemoryStream();
        output.WriteByte(1);
        WriteCompactLength(output, (ulong)value.Length);
        output.Write(value);
        return output.ToArray();
    }

    private static byte[] AddStatusPadding(byte[] archive) => AddHeaderPadding(archive, new byte[8]);

    private static byte[] RemoveStatusPadding(byte[] archive)
    {
        var result = new byte[archive.Length - 8];
        archive.AsSpan(0, NoritoHeader.EncodedLength).CopyTo(result);
        archive.AsSpan(NoritoHeader.EncodedLength + 8).CopyTo(result.AsSpan(NoritoHeader.EncodedLength));
        return result;
    }

    private static byte[] AddHeaderPadding(byte[] archive, byte[] padding)
    {
        var result = new byte[archive.Length + padding.Length];
        archive.AsSpan(0, NoritoHeader.EncodedLength).CopyTo(result);
        padding.CopyTo(result, NoritoHeader.EncodedLength);
        archive.AsSpan(NoritoHeader.EncodedLength).CopyTo(
            result.AsSpan(NoritoHeader.EncodedLength + padding.Length));
        return result;
    }

    private static byte[] MutatePayload(byte[] archive) =>
        MutateClone(archive, clone => clone[^1] ^= 0xff);

    private static byte[] MutateStatusPayload(byte[] archive, SpanMutator mutate)
    {
        var clone = (byte[])archive.Clone();
        mutate(clone.AsSpan(NoritoHeader.EncodedLength + 8));
        RewriteChecksum(clone, padding: 8);
        return clone;
    }

    private static byte[] MutateClone(byte[] value, Action<byte[]> mutate)
    {
        var clone = (byte[])value.Clone();
        mutate(clone);
        return clone;
    }

    private delegate void SpanMutator(Span<byte> value);

    private static void RewriteChecksum(byte[] archive, int padding)
    {
        var payloadLength = checked((int)BinaryPrimitives.ReadUInt64LittleEndian(archive.AsSpan(23, 8)));
        var payload = archive.AsSpan(NoritoHeader.EncodedLength + padding, payloadLength);
        BinaryPrimitives.WriteUInt64LittleEndian(archive.AsSpan(31, 8), Crc64Ecma.Compute(payload));
    }

    private static void WriteField(Stream output, byte[] field)
    {
        WriteCompactLength(output, (ulong)field.Length);
        output.Write(field);
    }

    private static byte[] StringPayload(string value)
    {
        var bytes = Encoding.UTF8.GetBytes(value);
        using var output = new MemoryStream();
        WriteCompactLength(output, (ulong)bytes.Length);
        output.Write(bytes);
        return output.ToArray();
    }

    private static byte[] UInt16Payload(ushort value)
    {
        var output = new byte[sizeof(ushort)];
        BinaryPrimitives.WriteUInt16LittleEndian(output, value);
        return output;
    }

    private static byte[] UInt32Payload(uint value)
    {
        var output = new byte[sizeof(uint)];
        BinaryPrimitives.WriteUInt32LittleEndian(output, value);
        return output;
    }

    private static byte[] UInt64Payload(ulong value)
    {
        var output = new byte[sizeof(ulong)];
        BinaryPrimitives.WriteUInt64LittleEndian(output, value);
        return output;
    }

    private static void WriteCompactLength(Stream output, ulong value)
    {
        do
        {
            var current = (byte)(value & 0x7f);
            value >>= 7;
            if (value != 0)
            {
                current |= 0x80;
            }
            output.WriteByte(current);
        }
        while (value != 0);
    }

    private static HttpResponseMessage JsonResponse(string json)
    {
        var response = new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent(json, Encoding.UTF8, "application/json"),
        };
        return response;
    }

    private static HttpResponseMessage BinaryResponse(
        byte[] bytes,
        HttpStatusCode status = HttpStatusCode.OK,
        string? location = null,
        string mediaType = "application/x-norito")
    {
        var content = new ByteArrayContent(bytes);
        content.Headers.ContentType = new MediaTypeHeaderValue(mediaType);
        var response = new HttpResponseMessage(status) { Content = content };
        if (location is not null)
        {
            response.Headers.Location = new Uri(location, UriKind.Relative);
        }
        return response;
    }

    private sealed record RequestSnapshot(
        HttpMethod Method,
        string PathAndQuery,
        string? ContentType,
        string Accept,
        string? IdempotencyKey,
        byte[] Body);

    private sealed class CaptureHandler(Func<HttpRequestMessage, HttpResponseMessage> responder)
        : HttpMessageHandler
    {
        internal RequestSnapshot? Last { get; private set; }

        protected override async Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken)
        {
            var body = request.Content is null
                ? []
                : await request.Content.ReadAsByteArrayAsync(cancellationToken);
            Last = new RequestSnapshot(
                request.Method,
                request.RequestUri!.PathAndQuery,
                request.Content?.Headers.ContentType?.MediaType,
                string.Join(",", request.Headers.Accept.Select(static value => value.ToString())),
                request.Headers.TryGetValues("Idempotency-Key", out var values)
                    ? Assert.Single(values)
                    : null,
                body);
            var response = responder(request);
            response.RequestMessage = request;
            return response;
        }
    }
}
