using System.Buffers.Binary;
using System.Net;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class KagemushaToriiTests
{
    private static readonly string OperationId = new('1', 64);
    private static readonly string TransactionHash = new('3', 64);

    private const string TopUpRequestSchemaName = "iroha.torii.v1.offline.top_up.request";
    private const string RedeemRequestSchemaName = "iroha.torii.v1.offline.redeem.request";

    [Fact]
    public async Task OfflineCapabilityUsesTheAssetNeutralRouteAndExactSchema()
    {
        using var handler = new KagemushaHandler(request =>
        {
            Assert.Equal(HttpMethod.Get, request.Method);
            Assert.Equal("/v1/offline/readiness", request.RequestUri!.AbsolutePath);
            Assert.Empty(request.RequestUri.Query);
            return JsonResponse(OfflineCapabilityJson());
        });
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        var capability = await client.GetOfflineCapabilityAsync(
            TestContext.Current.CancellationToken);

        Assert.Equal("cash_handoff_v1", capability.CashHandoffCapability);
        Assert.Equal(23U, capability.RequiredBridgeAbiVersion);
        Assert.Equal(8U, capability.MaxHops);
        Assert.True(capability.Ready);
        Assert.Null(typeof(ToriiClient).GetMethod("GetKagemushaReadinessV4Async"));
        Assert.DoesNotContain(
            typeof(ToriiClient).Assembly.GetTypes(),
            type => type.Name.StartsWith("ToriiKagemushaReadiness", StringComparison.Ordinal));
    }

    [Fact]
    public async Task OfflineCapabilityRejectsRetiredAndNoncanonicalClaims()
    {
        var invalidPayloads = new[]
        {
            OfflineCapabilityJson(cashHandoffCapability: "cash_handoff_v2"),
            OfflineCapabilityJson(abiVersion: 21),
            OfflineCapabilityJson(maxHops: 9),
            OfflineCapabilityJson(ready: false),
            OfflineCapabilityJson(extra: "\"assets\":[]"),
            OfflineCapabilityJson(extra: "\"blockers\":[]"),
            OfflineCapabilityJson(extra: "\"mandatory\":true"),
            OfflineCapabilityJson(extra: "\"asset_definition_id\":\"coin#wonderland\""),
        };

        foreach (var payload in invalidPayloads)
        {
            using var handler = new KagemushaHandler(_ => JsonResponse(payload));
            using var client = new ToriiClient(
                new Uri("https://torii.example"),
                new HttpClient(handler));

            await Assert.ThrowsAsync<JsonException>(() =>
                client.GetOfflineCapabilityAsync(TestContext.Current.CancellationToken));
        }
    }

    [Fact]
    public async Task KagemushaReadRoutesRequireExactHttpOk()
    {
        using (var handler = new KagemushaHandler(_ =>
               JsonResponse(OfflineCapabilityJson(), HttpStatusCode.Created)))
        using (var client = new ToriiClient(
               new Uri("https://torii.example"),
               new HttpClient(handler)))
        {
            var error = await Assert.ThrowsAsync<InvalidDataException>(() =>
                client.GetOfflineCapabilityAsync(TestContext.Current.CancellationToken));
            Assert.Contains("expected HTTP 200, got 201", error.Message);
        }

        using (var handler = new KagemushaHandler(_ =>
               JsonResponse(TopUpStatusJson(), HttpStatusCode.Accepted)))
        using (var client = new ToriiClient(
               new Uri("https://torii.example"),
               new HttpClient(handler)))
        {
            var error = await Assert.ThrowsAsync<InvalidDataException>(() =>
                client.GetKagemushaOperationStatusForTestAsync(
                    OperationReference(ToriiKagemushaOperationKind.TopUp),
                    TestContext.Current.CancellationToken));
            Assert.Contains("expected HTTP 200, got 202", error.Message);
        }
    }

    [Fact]
    public async Task TopUpTransportsAnExternalV4NoritoArchiveWithoutAProverClaim()
    {
        var archive = NoritoArchive(TopUpRequestSchemaName);
        var expectedArchive = archive.ToArray();
        using var handler = new KagemushaHandler(request =>
        {
            Assert.Equal(HttpMethod.Post, request.Method);
            Assert.Equal("/v1/offline/top-up", request.RequestUri!.AbsolutePath);
            Assert.Equal("application/x-norito", request.Content!.Headers.ContentType!.MediaType);
            Assert.Equal(OperationId, Assert.Single(request.Headers.GetValues("Idempotency-Key")));
            Assert.Equal(expectedArchive, request.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult());
            var response = JsonResponse(OperationReferenceJson("top_up"), HttpStatusCode.Accepted);
            response.Headers.Location = new Uri($"/v1/offline/operations/{OperationId}", UriKind.Relative);
            response.Headers.TryAddWithoutValidation("Retry-After", "1");
            return response;
        });
        using var client = AssuredClient(handler);
        var request = new ToriiKagemushaTopUpRequestV4(archive);

        archive[4] = 0xff;
        var reference = await client.SubmitKagemushaTopUpV4Async(
            request,
            TestContext.Current.CancellationToken);

        Assert.Equal(4, request.Version);
        Assert.Equal(1234UL, request.IssuedAtMilliseconds);
        Assert.Equal(ToriiKagemushaOperationKind.TopUp, reference.Kind);
        Assert.Equal(ToriiKagemushaOperationState.Pending, reference.State);
        Assert.Equal(OperationId, reference.OperationId);
        Assert.DoesNotContain(
            typeof(ToriiClient).Assembly.GetTypes(),
            type => type.Name.Contains("Kagemusha", StringComparison.Ordinal)
                && type.Name.Contains("Prover", StringComparison.Ordinal));
    }

    [Fact]
    public async Task RedeemAndOperationStatusPreserveTheStableRoutes()
    {
        var requests = new Queue<Func<HttpRequestMessage, HttpResponseMessage>>();
        requests.Enqueue(request =>
        {
            Assert.Equal("/v1/offline/redeem", request.RequestUri!.AbsolutePath);
            var response = JsonResponse(OperationReferenceJson("redeem"), HttpStatusCode.Accepted);
            response.Headers.Location = new Uri($"/v1/offline/operations/{OperationId}", UriKind.Relative);
            response.Headers.TryAddWithoutValidation("Retry-After", "1");
            return response;
        });
        requests.Enqueue(request =>
        {
            Assert.Equal($"/v1/offline/operations/{OperationId}", request.RequestUri!.AbsolutePath);
            return JsonResponse($$"""
                {
                  "state": "applied",
                  "value": {
                    "operation_id": "{{OperationId}}",
                    "result": {
                      "kind": "redeem",
                      "result": {
                        "transaction_hash": "{{TransactionHash}}",
                        "finalized_block_height": 42
                      }
                    }
                  }
                }
                """);
        });
        using var handler = new KagemushaHandler(request => requests.Dequeue()(request));
        using var client = AssuredClient(handler);

        var reference = await client.SubmitKagemushaRedeemV4Async(
            new ToriiKagemushaRedeemRequestV4(NoritoArchive(RedeemRequestSchemaName)),
            TestContext.Current.CancellationToken);
        var status = await client.GetKagemushaOperationStatusForTestAsync(
            reference,
            TestContext.Current.CancellationToken);

        Assert.Equal(ToriiKagemushaOperationKind.Redeem, reference.Kind);
        Assert.Equal(ToriiKagemushaOperationState.Applied, status.State);
        Assert.Equal(reference.Kind, status.Kind);
        Assert.Equal(reference.TransactionHash, status.TransactionHash);
        Assert.Null(status.SubmittedAtMilliseconds);
        Assert.Equal(42UL, status.RedeemResult!.FinalizedBlockHeight);
        Assert.Empty(requests);
    }

    [Fact]
    public async Task AppliedTopUpRejectsV3AnchorBytesInsteadOfUpgradingThem()
    {
        using var handler = new KagemushaHandler(_ => JsonResponse($$"""
            {
              "state": "applied",
              "value": {
                "operation_id": "{{OperationId}}",
                "result": {
                  "kind": "top_up",
                  "result": {
                    "transaction_hash": "{{TransactionHash}}",
                    "finalized_block_height": 42,
                    "anchor": {
                      "version": 3,
                      "artifact_binding": {
                        "version": 4
                      }
                    },
                    "finality_proof": {}
                  }
                }
              }
            }
            """));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var error = await Assert.ThrowsAsync<JsonException>(() =>
            client.GetKagemushaOperationStatusForTestAsync(
                OperationReference(ToriiKagemushaOperationKind.TopUp),
                TestContext.Current.CancellationToken));

        Assert.Contains("top-up anchor must use V4", error.Message);
    }

    [Fact]
    public void RequestConstructorsRequireExactSchemaBoundNoritoFrames()
    {
        var topUp = NoritoArchive(TopUpRequestSchemaName);
        var redeem = NoritoArchive(RedeemRequestSchemaName);

        var topUpRequest = new ToriiKagemushaTopUpRequestV4(topUp);
        var redeemRequest = new ToriiKagemushaRedeemRequestV4(redeem);
        Assert.Equal(OperationId, topUpRequest.OperationId);
        Assert.Equal(OperationId, redeemRequest.OperationId);
        Assert.Equal(1234UL, topUpRequest.IssuedAtMilliseconds);
        Assert.Equal(1234UL, redeemRequest.IssuedAtMilliseconds);

        Assert.Throws<ArgumentException>(() =>
            new ToriiKagemushaTopUpRequestV4(redeem));
        Assert.Throws<ArgumentException>(() =>
            new ToriiKagemushaRedeemRequestV4(topUp));

        var invalidTopUpFrames = new[]
        {
            Mutate(topUp, frame => frame[4] = 1),
            Mutate(topUp, frame => frame[22] = 1),
            NoritoArchive(TopUpRequestSchemaName, flags: 0),
            NoritoCodec.Encode(TopUpRequestSchemaName, [0x01], 0x02),
            NoritoArchive(TopUpRequestSchemaName, [0x01]),
            NoritoArchive(
                TopUpRequestSchemaName,
                KagemushaRequestPayload(TopUpRequestSchemaName, new byte[32])),
            NoritoArchive(
                TopUpRequestSchemaName,
                KagemushaRequestPayload(TopUpRequestSchemaName, wireVersion: 3)),
            NoritoArchive(
                TopUpRequestSchemaName,
                KagemushaRequestPayload(
                    TopUpRequestSchemaName,
                    authorization: KagemushaRequestAuthorizationPayload(
                        Convert.FromHexString(OperationId),
                        fieldCount: 9))),
            NoritoArchive(
                TopUpRequestSchemaName,
                KagemushaRequestPayload(
                    TopUpRequestSchemaName,
                    authorization: KagemushaRequestAuthorizationPayload(
                        Convert.FromHexString(OperationId),
                        fieldCount: 11))),
            NoritoArchive(
                TopUpRequestSchemaName,
                KagemushaRequestPayload(
                    TopUpRequestSchemaName,
                    authorization: KagemushaRequestAuthorizationPayload(
                        Enumerable.Repeat((byte)0x22, 32).ToArray()))),
            NoritoArchive(
                TopUpRequestSchemaName,
                KagemushaRequestPayload(
                    TopUpRequestSchemaName,
                    authorization: KagemushaRequestAuthorizationPayload(
                        Convert.FromHexString(OperationId),
                        issuedAtField: [0x01]))),
            NoritoArchive(
                TopUpRequestSchemaName,
                KagemushaRequestPayload(
                    TopUpRequestSchemaName,
                    authorization: KagemushaRequestAuthorizationPayload(
                        Convert.FromHexString(OperationId),
                        issuedAtMilliseconds: 0))),
            NoritoArchive(TopUpRequestSchemaName, paddingLength: 9),
            NoritoArchive(TopUpRequestSchemaName, Array.Empty<byte>()),
            Mutate(topUp, frame => frame[NoritoHeader.EncodedLength] = 1),
            Mutate(topUp, frame => frame[^1] ^= 1),
        };

        foreach (var frame in invalidTopUpFrames)
        {
            Assert.Throws<ArgumentException>(() =>
                new ToriiKagemushaTopUpRequestV4(frame));
        }
    }

    [Theory]
    [InlineData(TopUpRequestSchemaName, "top_up")]
    [InlineData(RedeemRequestSchemaName, "redeem")]
    public async Task SubmissionRejectsReferenceTimestampNotSignedByRequest(
        string schemaName,
        string kind)
    {
        using var handler = new KagemushaHandler(_ =>
        {
            var response = AcceptedOperationReference(
                OperationReferenceJson(kind, submittedAtMilliseconds: 1235));
            response.Headers.TryAddWithoutValidation("Retry-After", "1");
            return response;
        });
        using var client = AssuredClient(handler);

        var error = await Assert.ThrowsAsync<JsonException>(async () =>
        {
            if (kind == "top_up")
            {
                await client.SubmitKagemushaTopUpV4Async(
                    new ToriiKagemushaTopUpRequestV4(NoritoArchive(schemaName)),
                    TestContext.Current.CancellationToken);
            }
            else
            {
                await client.SubmitKagemushaRedeemV4Async(
                    new ToriiKagemushaRedeemRequestV4(NoritoArchive(schemaName)),
                    TestContext.Current.CancellationToken);
            }
        });

        Assert.Contains("does not match the submitted V4 command", error.Message);
    }

    [Fact]
    public async Task SubmissionRequiresRetryAfterAndPositiveSubmittedTime()
    {
        string?[] invalidRetryAfter =
        [
            null,
            string.Empty,
            "0",
            "-1",
            "1.0",
            "18446744073709551616",
            "111111111111111111111",
            "1, 2",
        ];

        foreach (var retryAfter in invalidRetryAfter)
        {
            using var handler = new KagemushaHandler(_ =>
            {
                var response = AcceptedOperationReference(OperationReferenceJson("top_up"));
                if (retryAfter is not null)
                {
                    response.Headers.TryAddWithoutValidation("Retry-After", retryAfter);
                }
                return response;
            });
            using var client = AssuredClient(handler);

            await Assert.ThrowsAsync<InvalidDataException>(() =>
                client.SubmitKagemushaTopUpV4Async(
                    new ToriiKagemushaTopUpRequestV4(NoritoArchive(TopUpRequestSchemaName)),
                    TestContext.Current.CancellationToken));
        }

        using (var handler = new KagemushaHandler(_ =>
               {
                   var response = AcceptedOperationReference(OperationReferenceJson("top_up"));
                   response.Headers.TryAddWithoutValidation("Retry-After", ["1", "2"]);
                   return response;
               }))
        using (var client = AssuredClient(handler))
        {
            await Assert.ThrowsAsync<InvalidDataException>(() =>
                client.SubmitKagemushaTopUpV4Async(
                    new ToriiKagemushaTopUpRequestV4(NoritoArchive(TopUpRequestSchemaName)),
                    TestContext.Current.CancellationToken));
        }

        using (var handler = new KagemushaHandler(_ =>
               {
                   var response = AcceptedOperationReference(
                       OperationReferenceJson("top_up", submittedAtMilliseconds: 0));
                   response.Headers.TryAddWithoutValidation("Retry-After", "01");
                   return response;
               }))
        using (var client = AssuredClient(handler))
        {
            await Assert.ThrowsAsync<JsonException>(() =>
                client.SubmitKagemushaTopUpV4Async(
                    new ToriiKagemushaTopUpRequestV4(NoritoArchive(TopUpRequestSchemaName)),
                    TestContext.Current.CancellationToken));
        }
    }

    [Fact]
    public async Task KagemushaSubmissionRejectsUnassuredInjectedTransportBeforeDispatch()
    {
        var dispatches = 0;
        using var handler = new KagemushaHandler(_ =>
        {
            dispatches += 1;
            return AcceptedOperationReference(OperationReferenceJson("top_up"));
        });
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        var error = await Assert.ThrowsAsync<InvalidOperationException>(() =>
            client.SubmitKagemushaTopUpV4Async(
                new ToriiKagemushaTopUpRequestV4(NoritoArchive(TopUpRequestSchemaName)),
                TestContext.Current.CancellationToken));

        Assert.Contains("one-shot, no-redirect transport", error.Message);
        Assert.Equal(0, dispatches);
    }

    [Fact]
    public async Task AppliedTopUpBindsBothNestedOperationIds()
    {
        using var handler = new KagemushaHandler(_ => JsonResponse(TopUpStatusJson()));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var status = await client.GetKagemushaOperationStatusForTestAsync(
            OperationReference(ToriiKagemushaOperationKind.TopUp),
            TestContext.Current.CancellationToken);

        Assert.Equal(ToriiKagemushaOperationState.Applied, status.State);
        Assert.Equal(ToriiKagemushaOperationKind.TopUp, status.Kind);
        Assert.Equal(4, status.TopUpResult!.Anchor.GetProperty("version").GetInt32());
        Assert.Equal(1, status.TopUpResult.FinalityProof.GetProperty("version").GetInt32());
    }

    [Fact]
    public async Task OperationStatusValidatesTheExactResponseBytesBeforeProjection()
    {
        var payload = PendingStatusJson(submittedAtMilliseconds: 1234);
        var validator = new RecordingStatusValidator();
        using var handler = new KagemushaHandler(_ => JsonResponse(payload));
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        var status = await client.GetKagemushaOperationStatusAsync(
            OperationReference(ToriiKagemushaOperationKind.TopUp),
            validator,
            TestContext.Current.CancellationToken);

        Assert.Equal(ToriiKagemushaOperationState.Pending, status.State);
        Assert.Equal(Encoding.UTF8.GetBytes(payload), Assert.Single(validator.Payloads));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public async Task OperationStatusFailsClosedWhenNativeValidationCannotSucceed(
        bool nativeUnavailable)
    {
        Exception expected = nativeUnavailable
            ? new InvalidOperationException(
                "ABI-23 Kagemusha operation-status JSON validator is unavailable.")
            : new InvalidDataException(
                "native Kagemusha operation-status JSON validator failed closed (status -311).");
        var validator = new RecordingStatusValidator(expected);
        using var handler = new KagemushaHandler(_ => JsonResponse("{not valid JSON"));
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        Exception error = nativeUnavailable
            ? await Assert.ThrowsAsync<InvalidOperationException>(() =>
                client.GetKagemushaOperationStatusAsync(
                    OperationReference(ToriiKagemushaOperationKind.TopUp),
                    validator,
                    TestContext.Current.CancellationToken))
            : await Assert.ThrowsAsync<InvalidDataException>(() =>
                client.GetKagemushaOperationStatusAsync(
                    OperationReference(ToriiKagemushaOperationKind.TopUp),
                    validator,
                    TestContext.Current.CancellationToken));

        Assert.Equal(expected.Message, error.Message);
        Assert.Single(validator.Payloads);
    }

    [Fact]
    public void OperationStatusNativeSurfacePinsAbiAndJsonValidatorSymbol()
    {
        Assert.Equal(23U, KagemushaOperationStatusNative.RequiredBridgeAbiVersion);
        var flags = System.Reflection.BindingFlags.NonPublic
            | System.Reflection.BindingFlags.Static;
        var nativeMethods = typeof(KagemushaOperationStatusNative)
            .GetMethods(flags)
            .Select(method => method.GetCustomAttributes(typeof(DllImportAttribute), false)
                .Cast<DllImportAttribute>()
                .SingleOrDefault())
            .Where(attribute => attribute is not null)
            .Select(attribute => attribute!.EntryPoint)
            .ToArray();

        Assert.Contains("connect_norito_bridge_abi_version", nativeMethods);
        Assert.Equal(
            2,
            nativeMethods.Count(symbol =>
                string.Equals(
                    symbol,
                    "connect_norito_kagemusha_offline_operation_status_json_validate_v1",
                    StringComparison.Ordinal)));

        var unix = typeof(KagemushaOperationStatusNative)
            .GetMethod("NativeValidateJsonV1Unix", flags)!;
        var windows = typeof(KagemushaOperationStatusNative)
            .GetMethod("NativeValidateJsonV1Windows", flags)!;
        Assert.Equal(typeof(UIntPtr), unix.GetParameters()[1].ParameterType);
        Assert.Equal(typeof(uint), windows.GetParameters()[1].ParameterType);
    }

    [Fact]
    public async Task AppliedTopUpAuthenticatesMerkleAndPostStateRoots()
    {
        var sibling = Enumerable.Repeat((byte)0x55, 32).ToArray();
        var validProof = TopUpStatusJson(
            leafCount: 2,
            siblings: [sibling]);
        using (var handler = new KagemushaHandler(_ => JsonResponse(validProof)))
        using (var client = new ToriiClient(
               new Uri("https://torii.example"),
               new HttpClient(handler)))
        {
            var status = await client.GetKagemushaOperationStatusForTestAsync(
                OperationReference(ToriiKagemushaOperationKind.TopUp),
                TestContext.Current.CancellationToken);
            Assert.Equal(ToriiKagemushaOperationState.Applied, status.State);
        }

        var operationId = Enumerable.Repeat((byte)0x11, 32).ToArray();
        var anchorDigest = Enumerable.Repeat((byte)0x71, 32).ToArray();
        var committedRoot = ComputeTestTopUpRoot(operationId, anchorDigest, 0, [sibling]);
        Assert.Equal(
            "hash:B04A15CED3BA147B1FB13345E3D2ACBA0EDD825C3EBB4CB30FAE797BA1D5135D#5CFE",
            FormatTestHash(committedRoot));

        var forgedSibling = sibling.ToArray();
        forgedSibling[0] ^= 1;
        var noncanonicalSibling = sibling.ToArray();
        noncanonicalSibling[^1] &= 0xfe;
        var forgedTopUpRoot = IrohaHash.Hash("forged top-up root"u8);
        var forgedPostStateRoot = IrohaHash.Hash("forged post-state root"u8);
        var invalidProofs = new[]
        {
            (
                TopUpStatusJson(
                    leafCount: 2,
                    siblings: [forgedSibling],
                    topUpAnchorRoot: committedRoot),
                "does not authenticate the anchor"),
            (
                TopUpStatusJson(
                    leafCount: 2,
                    siblings: [noncanonicalSibling],
                    topUpAnchorRoot: committedRoot),
                "hash marker bit"),
            (
                TopUpStatusJson(topUpAnchorRoot: forgedTopUpRoot),
                "does not authenticate the anchor"),
            (
                TopUpStatusJson(postStateRoot: forgedPostStateRoot),
                "post-state root"),
            (
                TopUpStatusJson(finalityAnchorDigestByte: 0x73),
                "does not match the finalized anchor"),
            (
                TopUpStatusJson(
                    anchorFinalizedTransactionHash:
                        Enumerable.Repeat((byte)0x35, 32).ToArray()),
                "transaction or height"),
            (
                TopUpStatusJson(anchorFinalizedHeight: 43),
                "transaction or height"),
            (
                TopUpStatusJson(proofHeight: 43),
                "height context"),
            (
                TopUpStatusJson(
                    proofNetworkId: IrohaHash.Hash("other test network"u8)),
                "height context"),
        };

        foreach (var (payload, expectedMessage) in invalidProofs)
        {
            using var handler = new KagemushaHandler(_ => JsonResponse(payload));
            using var client = new ToriiClient(
                new Uri("https://torii.example"),
                new HttpClient(handler));

            var error = await Assert.ThrowsAsync<JsonException>(() =>
                client.GetKagemushaOperationStatusForTestAsync(
                    OperationReference(ToriiKagemushaOperationKind.TopUp),
                    TestContext.Current.CancellationToken));
            Assert.Contains(expectedMessage, error.Message);
        }
    }

    [Fact]
    public async Task StatusRejectsCrossOperationAndImpossibleTerminalValues()
    {
        var invalidStatuses = new[]
        {
            (TopUpStatusJson(anchorOperationByte: 0x12), ToriiKagemushaOperationKind.TopUp),
            (TopUpStatusJson(finalityOperationByte: 0x12), ToriiKagemushaOperationKind.TopUp),
            (TopUpStatusJson(finalityProofVersion: 2), ToriiKagemushaOperationKind.TopUp),
            (TopUpStatusJson(finalizedBlockHeight: 0), ToriiKagemushaOperationKind.TopUp),
            (TopUpStatusJson(transactionHash: new string('2', 64)), ToriiKagemushaOperationKind.TopUp),
            (PendingStatusJson(submittedAtMilliseconds: 0), ToriiKagemushaOperationKind.TopUp),
            (RedeemStatusJson(finalizedBlockHeight: 0), ToriiKagemushaOperationKind.Redeem),
        };

        foreach (var (payload, kind) in invalidStatuses)
        {
            using var handler = new KagemushaHandler(_ => JsonResponse(payload));
            using var client = new ToriiClient(
                new Uri("https://torii.example"),
                new HttpClient(handler));

            await Assert.ThrowsAsync<JsonException>(() =>
                client.GetKagemushaOperationStatusForTestAsync(
                    OperationReference(kind),
                    TestContext.Current.CancellationToken));
        }
    }

    [Fact]
    public async Task StatusKeepsKindButAllowsANewerCarrierTransactionHash()
    {
        var replacementHash = new string('5', 64);
        var replacementOperationId = new string('3', 64);
        using var wrongIdHandler = new KagemushaHandler(_ => JsonResponse(
            PendingStatusJson(1234, operationId: replacementOperationId)));
        using var wrongIdClient = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(wrongIdHandler));
        var wrongId = await Assert.ThrowsAsync<JsonException>(() =>
            wrongIdClient.GetKagemushaOperationStatusForTestAsync(
                OperationReference(ToriiKagemushaOperationKind.TopUp),
                TestContext.Current.CancellationToken));
        Assert.Contains("requested operation id", wrongId.Message);

        using var wrongKindHandler = new KagemushaHandler(_ => JsonResponse(RedeemStatusJson()));
        using var wrongKindClient = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(wrongKindHandler));
        var wrongKind = await Assert.ThrowsAsync<JsonException>(() =>
            wrongKindClient.GetKagemushaOperationStatusForTestAsync(
                OperationReference(ToriiKagemushaOperationKind.TopUp),
                TestContext.Current.CancellationToken));
        Assert.Contains("accepted operation reference", wrongKind.Message);

        using var appliedHandler = new KagemushaHandler(_ => JsonResponse(
            TopUpStatusJson(transactionHash: replacementHash)));
        using var appliedClient = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(appliedHandler));
        var applied = await appliedClient.GetKagemushaOperationStatusForTestAsync(
            OperationReference(ToriiKagemushaOperationKind.TopUp),
            TestContext.Current.CancellationToken);
        Assert.Equal(ToriiKagemushaOperationState.Applied, applied.State);
        Assert.Equal(replacementHash, applied.TransactionHash);
        Assert.Null(applied.SubmittedAtMilliseconds);

        using var rejectedHandler = new KagemushaHandler(_ => JsonResponse(
            RejectedStatusJson(transactionHash: replacementHash)));
        using var rejectedClient = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(rejectedHandler));
        var rejected = await rejectedClient.GetKagemushaOperationStatusForTestAsync(
            OperationReference(ToriiKagemushaOperationKind.TopUp),
            TestContext.Current.CancellationToken);
        Assert.Equal(ToriiKagemushaOperationState.Rejected, rejected.State);
        Assert.Equal(replacementHash, rejected.TransactionHash);
        Assert.Null(rejected.SubmittedAtMilliseconds);
    }

    [Fact]
    public async Task PendingStatusMustRepeatTheAcceptedSubmissionIdentity()
    {
        using var handler = new KagemushaHandler(_ => JsonResponse(
            PendingStatusJson(submittedAtMilliseconds: 1235)));
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        var error = await Assert.ThrowsAsync<JsonException>(() =>
            client.GetKagemushaOperationStatusForTestAsync(
                OperationReference(ToriiKagemushaOperationKind.TopUp),
                TestContext.Current.CancellationToken));

        Assert.Contains("accepted operation reference", error.Message);

        var replacementHash = new string('5', 64);
        using var retryHandler = new KagemushaHandler(_ => JsonResponse(
            PendingStatusJson(1234, replacementHash)));
        using var retryClient = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(retryHandler));
        var retry = await retryClient.GetKagemushaOperationStatusForTestAsync(
            OperationReference(ToriiKagemushaOperationKind.TopUp),
            TestContext.Current.CancellationToken);
        Assert.Equal(replacementHash, retry.TransactionHash);
        Assert.Equal(1234UL, retry.SubmittedAtMilliseconds);

        using var rewrittenTimestampHandler = new KagemushaHandler(_ => JsonResponse(
            PendingStatusJson(1235, replacementHash)));
        using var rewrittenTimestampClient = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(rewrittenTimestampHandler));
        var rewrittenTimestamp = await Assert.ThrowsAsync<JsonException>(() =>
            rewrittenTimestampClient.GetKagemushaOperationStatusForTestAsync(
                OperationReference(ToriiKagemushaOperationKind.TopUp),
                TestContext.Current.CancellationToken));
        Assert.Contains("accepted operation reference", rewrittenTimestamp.Message);
    }

    [Fact]
    public async Task PollingRequiresAnExactAcceptedReferenceBeforeDispatch()
    {
        Assert.DoesNotContain(
            typeof(ToriiClient).GetMethods(),
            method => method.Name == nameof(ToriiClient.GetKagemushaOperationStatusAsync)
                && method.GetParameters()[0].ParameterType == typeof(string));

        var dispatches = 0;
        using var handler = new KagemushaHandler(_ =>
        {
            dispatches += 1;
            return JsonResponse(PendingStatusJson(submittedAtMilliseconds: 1234));
        });
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));
        var invalidReferences = new[]
        {
            OperationReference(ToriiKagemushaOperationKind.TopUp) with
            {
                State = ToriiKagemushaOperationState.Applied,
            },
            OperationReference(ToriiKagemushaOperationKind.TopUp) with
            {
                StatusUri = "/v1/offline/operations/other",
            },
        };

        foreach (var invalidReference in invalidReferences)
        {
            await Assert.ThrowsAsync<ArgumentException>(() =>
                client.GetKagemushaOperationStatusForTestAsync(
                    invalidReference,
                    TestContext.Current.CancellationToken));
        }

        Assert.Equal(0, dispatches);
    }

    [Fact]
    public async Task FinalityProofDiagnosticNamesTheV2TypeAndNumericWireVersion()
    {
        using var handler = new KagemushaHandler(_ => JsonResponse(
            TopUpStatusJson(finalityProofVersion: 2)));
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        var error = await Assert.ThrowsAsync<JsonException>(() =>
            client.GetKagemushaOperationStatusForTestAsync(
                OperationReference(ToriiKagemushaOperationKind.TopUp),
                TestContext.Current.CancellationToken));

        Assert.Contains("KagemushaTopUpFinalityProofV2", error.Message);
        Assert.Contains("numeric wire version 1", error.Message);
    }

    [Fact]
    public async Task RejectedStatusRequiresExactBoundedPublicErrorMetadata()
    {
        var maximumMessage = string.Concat(Enumerable.Repeat("😀", 1024));
        using (var handler = new KagemushaHandler(_ => JsonResponse(
                   RejectedStatusJson(message: maximumMessage, detailsJson: "{}"))))
        using (var client = new ToriiClient(
               new Uri("https://torii.example"),
               new HttpClient(handler)))
        {
            var status = await client.GetKagemushaOperationStatusForTestAsync(
                OperationReference(ToriiKagemushaOperationKind.TopUp),
                TestContext.Current.CancellationToken);

            Assert.Equal("busy", status.Error!.Code);
            Assert.Equal(maximumMessage, status.Error.Message);
            Assert.Equal(JsonValueKind.Object, status.Error.Details!.Value.ValueKind);
            Assert.Null(status.SubmittedAtMilliseconds);
        }

        var invalidErrors = new[]
        {
            RejectedStatusJson(code: "Busy"),
            RejectedStatusJson(code: "_busy"),
            RejectedStatusJson(code: new string('a', 65)),
            RejectedStatusJson(message: " padded "),
            RejectedStatusJson(message: "\ufeffpadded"),
            RejectedStatusJson(message: "line\nbreak"),
            RejectedStatusJson(message: new string('a', 1025)),
            RejectedStatusJson(detailsJson: "null"),
            RejectedStatusJson(detailsJson: "[]"),
            RejectedStatusJson(extraErrorField: "\"unexpected\": true"),
        };

        foreach (var payload in invalidErrors)
        {
            using var handler = new KagemushaHandler(_ => JsonResponse(payload));
            using var client = new ToriiClient(
                new Uri("https://torii.example"),
                new HttpClient(handler));

            await Assert.ThrowsAsync<JsonException>(() =>
                client.GetKagemushaOperationStatusForTestAsync(
                    OperationReference(ToriiKagemushaOperationKind.TopUp),
                    TestContext.Current.CancellationToken));
        }
    }

    [Fact]
    public async Task StatusJsonAllowsAValidResponseAboveTheRetiredGenericLimit()
    {
        var payload = TopUpStatusJson() + new string(' ', 300 * 1024);
        Assert.True(Encoding.UTF8.GetByteCount(payload) > 256 * 1024);
        using var handler = new KagemushaHandler(_ => JsonResponse(payload));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var status = await client.GetKagemushaOperationStatusForTestAsync(
            OperationReference(ToriiKagemushaOperationKind.TopUp),
            TestContext.Current.CancellationToken);

        Assert.Equal(ToriiKagemushaOperationState.Applied, status.State);
        Assert.Null(status.SubmittedAtMilliseconds);
    }

    [Fact]
    public async Task KagemushaEndpointsUseTheirProtocolSpecificResponseLimits()
    {
        using (var handler = new KagemushaHandler(_ => JsonResponse(
                   OfflineCapabilityJson() + new string(' ', 4 * 1024))))
        using (var client = new ToriiClient(
               new Uri("https://torii.example"),
               new HttpClient(handler)))
        {
            var error = await Assert.ThrowsAsync<InvalidDataException>(() =>
                client.GetOfflineCapabilityAsync(TestContext.Current.CancellationToken));
            Assert.Contains("4096-byte limit", error.Message);
        }

        using (var handler = new KagemushaHandler(_ =>
               {
                   var response = AcceptedOperationReference(
                       OperationReferenceJson("top_up") + new string(' ', 4 * 1024));
                   response.Headers.TryAddWithoutValidation("Retry-After", "1");
                   return response;
               }))
        using (var client = AssuredClient(handler))
        {
            var error = await Assert.ThrowsAsync<InvalidDataException>(() =>
                client.SubmitKagemushaTopUpV4Async(
                    new ToriiKagemushaTopUpRequestV4(NoritoArchive(TopUpRequestSchemaName)),
                    TestContext.Current.CancellationToken));
            Assert.Contains("4096-byte limit", error.Message);
        }

        var oversizedStatus = RejectedStatusJson(
            detailsJson: $$"""{"padding":"{{new string('a', 16 * 1024 * 1024)}}"}""");
        using (var handler = new KagemushaHandler(_ => JsonResponse(oversizedStatus)))
        using (var client = new ToriiClient(
               new Uri("https://torii.example"),
               new HttpClient(handler)))
        {
            var error = await Assert.ThrowsAsync<InvalidDataException>(() =>
                client.GetKagemushaOperationStatusForTestAsync(
                    OperationReference(ToriiKagemushaOperationKind.TopUp),
                    TestContext.Current.CancellationToken));
            Assert.Contains("16777216-byte limit", error.Message);
        }
    }

    private static byte[] NoritoArchive(
        string schemaName,
        byte[]? payload = null,
        byte flags = 0x02,
        int paddingLength = 8)
    {
        var encoded = NoritoCodec.Encode(
            schemaName,
            payload ?? KagemushaRequestPayload(schemaName),
            flags);
        var archive = new byte[encoded.Length + paddingLength];
        encoded.AsSpan(0, NoritoHeader.EncodedLength).CopyTo(archive);
        encoded.AsSpan(NoritoHeader.EncodedLength).CopyTo(
            archive.AsSpan(NoritoHeader.EncodedLength + paddingLength));
        return archive;
    }

    private static byte[] KagemushaRequestPayload(
        string schemaName,
        byte[]? operationId = null,
        ushort wireVersion = 4,
        byte[]? authorization = null,
        ulong issuedAtMilliseconds = 1234)
    {
        var (fieldCount, operationIdFieldIndex) = schemaName switch
        {
            TopUpRequestSchemaName => (8, 6),
            RedeemRequestSchemaName => (10, 8),
            _ => throw new ArgumentException("Unknown Kagemusha request schema.", nameof(schemaName)),
        };
        var writer = new CanonicalNoritoWriter();
        Span<byte> version = stackalloc byte[sizeof(ushort)];
        BinaryPrimitives.WriteUInt16LittleEndian(version, wireVersion);
        writer.WriteField(version);
        var selectedOperationId = operationId ?? Convert.FromHexString(OperationId);
        var selectedAuthorization = authorization
            ?? KagemushaRequestAuthorizationPayload(
                selectedOperationId,
                issuedAtMilliseconds);
        for (var index = 1; index < fieldCount; index++)
        {
            writer.WriteField(index switch
            {
                _ when index == operationIdFieldIndex => selectedOperationId,
                _ when index == fieldCount - 1 => selectedAuthorization,
                _ => [0x01],
            });
        }
        return writer.ToArray();
    }

    private static byte[] KagemushaRequestAuthorizationPayload(
        byte[] operationId,
        ulong issuedAtMilliseconds = 1234,
        int fieldCount = 10,
        byte[]? issuedAtField = null)
    {
        var writer = new CanonicalNoritoWriter();
        Span<byte> encodedIssuedAt = stackalloc byte[sizeof(ulong)];
        BinaryPrimitives.WriteUInt64LittleEndian(encodedIssuedAt, issuedAtMilliseconds);
        for (var index = 0; index < fieldCount; index++)
        {
            writer.WriteField(index switch
            {
                3 => operationId,
                4 => issuedAtField ?? encodedIssuedAt.ToArray(),
                _ => [0x01],
            });
        }
        return writer.ToArray();
    }

    private static ToriiClient AssuredClient(HttpMessageHandler handler) =>
        new(
            new Uri("https://torii.example"),
            new HttpClient(handler),
            options: null,
            TransactionSubmissionTransportAssurance.OneShotWithoutRedirectsOrRetries);

    private static ToriiKagemushaOperationReference OperationReference(
        ToriiKagemushaOperationKind kind,
        string? transactionHash = null,
        ulong submittedAtMilliseconds = 1234) => new()
        {
            OperationId = OperationId,
            Kind = kind,
            State = ToriiKagemushaOperationState.Pending,
            TransactionHash = transactionHash ?? TransactionHash,
            StatusUri = $"/v1/offline/operations/{OperationId}",
            SubmittedAtMilliseconds = submittedAtMilliseconds,
        };

    private static byte[] Mutate(byte[] source, Action<byte[]> mutation)
    {
        var copy = source.ToArray();
        mutation(copy);
        return copy;
    }

    private static HttpResponseMessage AcceptedOperationReference(string json)
    {
        var response = JsonResponse(json, HttpStatusCode.Accepted);
        response.Headers.Location = new Uri(
            $"/v1/offline/operations/{OperationId}",
            UriKind.Relative);
        return response;
    }

    private static string OperationReferenceJson(
        string kind,
        ulong submittedAtMilliseconds = 1234) => $$"""
        {
          "operation_id": "{{OperationId}}",
          "kind": {"kind": "{{kind}}", "value": null},
          "state": {"state": "pending", "value": null},
          "transaction_hash": "{{TransactionHash}}",
          "status_uri": "/v1/offline/operations/{{OperationId}}",
          "submitted_at_ms": {{submittedAtMilliseconds}}
        }
        """;

    private static string TopUpStatusJson(
        byte anchorOperationByte = 0x11,
        byte finalityOperationByte = 0x11,
        int finalityProofVersion = 1,
        ulong finalizedBlockHeight = 42,
        string? transactionHash = null,
        byte anchorDigestByte = 0x71,
        byte finalityAnchorDigestByte = 0x71,
        uint leafIndex = 0,
        uint leafCount = 1,
        IReadOnlyList<byte[]>? siblings = null,
        byte[]? topUpAnchorRoot = null,
        uint? topUpAnchorCount = null,
        byte[]? ordinaryWritesRoot = null,
        byte[]? postStateRoot = null,
        byte[]? anchorFinalizedTransactionHash = null,
        ulong? anchorFinalizedHeight = null,
        ulong? proofHeight = null,
        byte[]? proofNetworkId = null)
    {
        var pathSiblings = siblings ?? [];
        var terminalTransactionHash = transactionHash ?? TransactionHash;
        var finalityOperationId = Enumerable.Repeat(finalityOperationByte, 32).ToArray();
        var finalityAnchorDigest = Enumerable.Repeat(finalityAnchorDigestByte, 32).ToArray();
        var committedTopUpRoot = topUpAnchorRoot
            ?? ComputeTestTopUpRoot(
                finalityOperationId,
                finalityAnchorDigest,
                leafIndex,
                pathSiblings);
        var committedCount = topUpAnchorCount ?? leafCount;
        var ordinaryRoot = ordinaryWritesRoot
            ?? IrohaHash.Hash("csharp-kagemusha-test-ordinary-root"u8);
        var committedPostStateRoot = postStateRoot
            ?? ComputeTestPostStateRoot(committedCount, ordinaryRoot, committedTopUpRoot);
        var networkId = IrohaHash.Hash("csharp-kagemusha-test-network"u8);
        var contextNetworkId = proofNetworkId ?? networkId;
        var finalizedTransactionHash = anchorFinalizedTransactionHash
            ?? Convert.FromHexString(terminalTransactionHash);
        var siblingsJson = string.Join(
            ", ",
            pathSiblings.Select(static sibling => $"[{FixedBytesJson(sibling)}]"));

        return $$"""
            {
              "state": "applied",
              "value": {
                "operation_id": "{{OperationId}}",
                "result": {
                  "kind": "top_up",
                  "result": {
                    "transaction_hash": "{{terminalTransactionHash}}",
                    "finalized_block_height": {{finalizedBlockHeight}},
                    "anchor": {
                      "version": 4,
                      "network_id": "{{FormatTestHash(networkId)}}",
                      "topup_operation_id": [{{FixedBytesJson(anchorOperationByte)}}],
                      "finalized_height": {{anchorFinalizedHeight ?? finalizedBlockHeight}},
                      "finalized_tx_hash": [{{FixedBytesJson(finalizedTransactionHash)}}],
                      "anchor_digest": [{{FixedBytesJson(Enumerable.Repeat(anchorDigestByte, 32).ToArray())}}],
                      "artifact_binding": {"version": 4}
                    },
                    "finality_proof": {
                      "version": {{finalityProofVersion}},
                      "anchor": {
                        "topup_operation_id": [{{FixedBytesJson(finalityOperationByte)}}],
                        "anchor_digest": [{{FixedBytesJson(finalityAnchorDigest)}}]
                      },
                      "commit_qc": {
                        "height_context": {
                          "network_id": "{{FormatTestHash(contextNetworkId)}}",
                          "height": {{proofHeight ?? finalizedBlockHeight}}
                        },
                        "certificate": {
                          "execution_commitment": {
                            "post_state_root": "{{FormatTestHash(committedPostStateRoot)}}",
                            "ordinary_writes_root": "{{FormatTestHash(ordinaryRoot)}}",
                            "topup_anchor_root": "{{FormatTestHash(committedTopUpRoot)}}",
                            "topup_anchor_count": {{committedCount}}
                          }
                        }
                      },
                      "anchor_path": {
                        "leaf_index": {{leafIndex}},
                        "leaf_count": {{leafCount}},
                        "siblings": [{{siblingsJson}}]
                      }
                    }
                  }
                }
              }
            }
            """;
    }

    private static string PendingStatusJson(
        ulong submittedAtMilliseconds,
        string? transactionHash = null,
        string? operationId = null)
    {
        var activeTransactionHash = transactionHash ?? TransactionHash;
        var activeOperationId = operationId ?? OperationId;
        return $$"""
        {
          "state": "pending",
          "value": {
            "operation_id": "{{activeOperationId}}",
            "kind": {"kind": "top_up", "value": null},
            "transaction_hash": "{{activeTransactionHash}}",
            "submitted_at_ms": {{submittedAtMilliseconds}}
          }
        }
        """;
    }

    private static string RedeemStatusJson(
        ulong finalizedBlockHeight = 42) => $$"""
        {
          "state": "applied",
          "value": {
            "operation_id": "{{OperationId}}",
            "result": {
              "kind": "redeem",
              "result": {
                "transaction_hash": "{{TransactionHash}}",
                "finalized_block_height": {{finalizedBlockHeight}}
              }
            }
          }
        }
        """;

    private static string RejectedStatusJson(
        string code = "busy",
        string message = "retry later",
        string? detailsJson = null,
        string? extraErrorField = null,
        string? transactionHash = null)
    {
        var details = detailsJson is null ? string.Empty : $", \"details\": {detailsJson}";
        var extra = extraErrorField is null ? string.Empty : $", {extraErrorField}";
        var activeTransactionHash = transactionHash ?? TransactionHash;
        return $$"""
            {
              "state": "rejected",
              "value": {
                "operation_id": "{{OperationId}}",
                "kind": {"kind": "top_up", "value": null},
                "transaction_hash": "{{activeTransactionHash}}",
                "error": {
                  "code": {{JsonSerializer.Serialize(code)}},
                  "message": {{JsonSerializer.Serialize(message)}}{{details}}{{extra}}
                }
              }
            }
            """;
    }

    private static string FixedBytesJson(byte value) =>
        string.Join(", ", Enumerable.Repeat(value, 32));

    private static string FixedBytesJson(byte[] value) => string.Join(", ", value);

    private static byte[] ComputeTestTopUpRoot(
        ReadOnlySpan<byte> operationId,
        ReadOnlySpan<byte> anchorDigest,
        uint leafIndex,
        IReadOnlyList<byte[]> siblings)
    {
        var key = new byte[33];
        key[0] = 0xd2;
        operationId.CopyTo(key.AsSpan(1));
        var keyHash = IrohaHash.Hash(key);
        var valueHash = IrohaHash.Hash(anchorDigest);
        var leafPreimage = new byte[65];
        keyHash.CopyTo(leafPreimage.AsSpan(1));
        valueHash.CopyTo(leafPreimage.AsSpan(33));
        var current = IrohaHash.Hash(leafPreimage);
        var index = leafIndex;
        for (var level = 0; level < siblings.Count; level++)
        {
            ReadOnlySpan<byte> domain = "iroha:kagemusha:v2:topup-node"u8;
            var preimage = new byte[domain.Length + 1 + sizeof(ushort) + 64];
            domain.CopyTo(preimage);
            BinaryPrimitives.WriteUInt16LittleEndian(
                preimage.AsSpan(domain.Length + 1, sizeof(ushort)),
                (ushort)level);
            var left = (index & 1) == 0 ? current : siblings[level];
            var right = (index & 1) == 0 ? siblings[level] : current;
            left.CopyTo(preimage.AsSpan(domain.Length + 1 + sizeof(ushort)));
            right.CopyTo(preimage.AsSpan(domain.Length + 1 + sizeof(ushort) + 32));
            current = IrohaHash.Hash(preimage);
            index >>= 1;
        }
        return current;
    }

    private static byte[] ComputeTestPostStateRoot(
        uint count,
        ReadOnlySpan<byte> ordinaryWritesRoot,
        ReadOnlySpan<byte> topUpRoot)
    {
        ReadOnlySpan<byte> domain = "iroha:kagemusha:v2:post-state-root"u8;
        var preimage = new byte[domain.Length + 1 + sizeof(uint) + 64];
        domain.CopyTo(preimage);
        BinaryPrimitives.WriteUInt32LittleEndian(
            preimage.AsSpan(domain.Length + 1, sizeof(uint)),
            count);
        ordinaryWritesRoot.CopyTo(preimage.AsSpan(domain.Length + 1 + sizeof(uint)));
        topUpRoot.CopyTo(preimage.AsSpan(domain.Length + 1 + sizeof(uint) + 32));
        return IrohaHash.Hash(preimage);
    }

    private static string FormatTestHash(ReadOnlySpan<byte> hash)
    {
        var body = Convert.ToHexString(hash);
        var checksum = TestCrc16(Encoding.ASCII.GetBytes($"hash:{body}"));
        return $"hash:{body}#{checksum:X4}";
    }

    private static ushort TestCrc16(ReadOnlySpan<byte> bytes)
    {
        var crc = 0xffff;
        foreach (var item in bytes)
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

    private static string OfflineCapabilityJson(
        string cashHandoffCapability = "cash_handoff_v1",
        int abiVersion = 23,
        int maxHops = 8,
        bool ready = true,
        string? extra = null)
    {
        var extraField = extra is null ? string.Empty : $",\n  {extra}";
        return $$"""
            {
              "cash_handoff_capability": "{{cashHandoffCapability}}",
              "required_bridge_abi_version": {{abiVersion}},
              "max_hops": {{maxHops}},
              "ready": {{ready.ToString().ToLowerInvariant()}}{{extraField}}
            }
            """;
    }

    private static HttpResponseMessage JsonResponse(
        string json,
        HttpStatusCode status = HttpStatusCode.OK) =>
        new(status)
        {
            Content = new StringContent(json, Encoding.UTF8, "application/json"),
        };

    private sealed class KagemushaHandler(
        Func<HttpRequestMessage, HttpResponseMessage> responder) : HttpMessageHandler
    {
        protected override Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken) =>
            Task.FromResult(responder(request));
    }

    private sealed class RecordingStatusValidator(Exception? failure = null)
        : IKagemushaOperationStatusValidator
    {
        internal List<byte[]> Payloads { get; } = [];

        public void Validate(byte[] statusJson)
        {
            Payloads.Add(statusJson.ToArray());
            if (failure is not null)
            {
                throw failure;
            }
        }
    }
}

internal static class KagemushaToriiTestExtensions
{
    internal static Task<ToriiKagemushaOperationStatus>
        GetKagemushaOperationStatusForTestAsync(
            this ToriiClient client,
            ToriiKagemushaOperationReference reference,
            CancellationToken cancellationToken) =>
        client.GetKagemushaOperationStatusAsync(
            reference,
            AcceptingKagemushaOperationStatusValidator.Instance,
            cancellationToken);
}

internal sealed class AcceptingKagemushaOperationStatusValidator
    : IKagemushaOperationStatusValidator
{
    internal static readonly AcceptingKagemushaOperationStatusValidator Instance = new();

    private AcceptingKagemushaOperationStatusValidator()
    {
    }

    public void Validate(byte[] statusJson)
    {
    }
}
