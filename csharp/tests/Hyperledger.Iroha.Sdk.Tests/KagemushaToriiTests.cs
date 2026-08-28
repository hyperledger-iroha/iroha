using System.Net;
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
                client.GetKagemushaOperationStatusAsync(
                    OperationId,
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
        var request = new ToriiKagemushaTopUpRequestV4(OperationId, archive);

        archive[4] = 0xff;
        var reference = await client.SubmitKagemushaTopUpV4Async(
            request,
            TestContext.Current.CancellationToken);

        Assert.Equal(4, request.Version);
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
                        "finalized_block_height": 42,
                        "server_time_ms": 1234
                      }
                    }
                  }
                }
                """);
        });
        using var handler = new KagemushaHandler(request => requests.Dequeue()(request));
        using var client = AssuredClient(handler);

        var reference = await client.SubmitKagemushaRedeemV4Async(
            new ToriiKagemushaRedeemRequestV4(
                OperationId,
                NoritoArchive(RedeemRequestSchemaName)),
            TestContext.Current.CancellationToken);
        var status = await client.GetKagemushaOperationStatusAsync(
            OperationId,
            TestContext.Current.CancellationToken);

        Assert.Equal(ToriiKagemushaOperationKind.Redeem, reference.Kind);
        Assert.Equal(ToriiKagemushaOperationState.Applied, status.State);
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
                    "server_time_ms": 1234,
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
            client.GetKagemushaOperationStatusAsync(
                OperationId,
                TestContext.Current.CancellationToken));

        Assert.Contains("top-up anchor must use V4", error.Message);
    }

    [Fact]
    public void RequestConstructorsRequireExactSchemaBoundNoritoFrames()
    {
        var topUp = NoritoArchive(TopUpRequestSchemaName);
        var redeem = NoritoArchive(RedeemRequestSchemaName);

        _ = new ToriiKagemushaTopUpRequestV4(OperationId, topUp);
        _ = new ToriiKagemushaRedeemRequestV4(OperationId, redeem);

        Assert.Throws<ArgumentException>(() =>
            new ToriiKagemushaTopUpRequestV4(OperationId, redeem));
        Assert.Throws<ArgumentException>(() =>
            new ToriiKagemushaRedeemRequestV4(OperationId, topUp));

        var invalidTopUpFrames = new[]
        {
            Mutate(topUp, frame => frame[4] = 1),
            Mutate(topUp, frame => frame[22] = 1),
            NoritoArchive(TopUpRequestSchemaName, flags: 0),
            NoritoCodec.Encode(TopUpRequestSchemaName, [0x01], 0x02),
            NoritoArchive(TopUpRequestSchemaName, paddingLength: 9),
            NoritoArchive(TopUpRequestSchemaName, Array.Empty<byte>()),
            Mutate(topUp, frame => frame[NoritoHeader.EncodedLength] = 1),
            Mutate(topUp, frame => frame[^1] ^= 1),
        };

        foreach (var frame in invalidTopUpFrames)
        {
            Assert.Throws<ArgumentException>(() =>
                new ToriiKagemushaTopUpRequestV4(OperationId, frame));
        }
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
                    new ToriiKagemushaTopUpRequestV4(
                        OperationId,
                        NoritoArchive(TopUpRequestSchemaName)),
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
                    new ToriiKagemushaTopUpRequestV4(
                        OperationId,
                        NoritoArchive(TopUpRequestSchemaName)),
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
                    new ToriiKagemushaTopUpRequestV4(
                        OperationId,
                        NoritoArchive(TopUpRequestSchemaName)),
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
                new ToriiKagemushaTopUpRequestV4(
                    OperationId,
                    NoritoArchive(TopUpRequestSchemaName)),
                TestContext.Current.CancellationToken));

        Assert.Contains("one-shot, no-redirect transport", error.Message);
        Assert.Equal(0, dispatches);
    }

    [Fact]
    public async Task AppliedTopUpBindsBothNestedOperationIds()
    {
        using var handler = new KagemushaHandler(_ => JsonResponse(TopUpStatusJson()));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var status = await client.GetKagemushaOperationStatusAsync(
            OperationId,
            TestContext.Current.CancellationToken);

        Assert.Equal(ToriiKagemushaOperationState.Applied, status.State);
        Assert.Equal(ToriiKagemushaOperationKind.TopUp, status.Kind);
        Assert.Equal(4, status.TopUpResult!.Anchor.GetProperty("version").GetInt32());
        Assert.Equal(1, status.TopUpResult.FinalityProof.GetProperty("version").GetInt32());
    }

    [Fact]
    public async Task StatusRejectsCrossOperationAndImpossibleTerminalValues()
    {
        var invalidStatuses = new[]
        {
            TopUpStatusJson(anchorOperationByte: 0x12),
            TopUpStatusJson(finalityOperationByte: 0x12),
            TopUpStatusJson(finalityProofVersion: 2),
            TopUpStatusJson(finalizedBlockHeight: 0),
            TopUpStatusJson(serverTimeMilliseconds: 0),
            TopUpStatusJson(transactionHash: new string('2', 64)),
            PendingStatusJson(submittedAtMilliseconds: 0),
            RedeemStatusJson(finalizedBlockHeight: 0),
            RedeemStatusJson(serverTimeMilliseconds: 0),
        };

        foreach (var payload in invalidStatuses)
        {
            using var handler = new KagemushaHandler(_ => JsonResponse(payload));
            using var client = new ToriiClient(
                new Uri("https://torii.example"),
                new HttpClient(handler));

            await Assert.ThrowsAsync<JsonException>(() =>
                client.GetKagemushaOperationStatusAsync(
                    OperationId,
                    TestContext.Current.CancellationToken));
        }
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
            var status = await client.GetKagemushaOperationStatusAsync(
                OperationId,
                TestContext.Current.CancellationToken);

            Assert.Equal("busy", status.Error!.Code);
            Assert.Equal(maximumMessage, status.Error.Message);
            Assert.Equal(JsonValueKind.Object, status.Error.Details!.Value.ValueKind);
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
                client.GetKagemushaOperationStatusAsync(
                    OperationId,
                    TestContext.Current.CancellationToken));
        }
    }

    [Fact]
    public async Task KagemushaJsonResponsesAreBoundedBeforeParsingOpaqueDetails()
    {
        var oversizedDetails = RejectedStatusJson(
            detailsJson: $$"""{"padding":"{{new string('a', 256 * 1024)}}"}""");
        using var handler = new KagemushaHandler(_ => JsonResponse(oversizedDetails));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var error = await Assert.ThrowsAsync<InvalidDataException>(() =>
            client.GetKagemushaOperationStatusAsync(
                OperationId,
                TestContext.Current.CancellationToken));

        Assert.Contains("262144-byte limit", error.Message);
    }

    private static byte[] NoritoArchive(
        string schemaName,
        byte[]? payload = null,
        byte flags = 0x02,
        int paddingLength = 8)
    {
        var encoded = NoritoCodec.Encode(schemaName, payload ?? [0x01], flags);
        var archive = new byte[encoded.Length + paddingLength];
        encoded.AsSpan(0, NoritoHeader.EncodedLength).CopyTo(archive);
        encoded.AsSpan(NoritoHeader.EncodedLength).CopyTo(
            archive.AsSpan(NoritoHeader.EncodedLength + paddingLength));
        return archive;
    }

    private static ToriiClient AssuredClient(HttpMessageHandler handler) =>
        new(
            new Uri("https://torii.example"),
            new HttpClient(handler),
            options: null,
            TransactionSubmissionTransportAssurance.OneShotWithoutRedirectsOrRetries);

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
        ulong serverTimeMilliseconds = 1234,
        string? transactionHash = null) => $$"""
        {
          "state": "applied",
          "value": {
            "operation_id": "{{OperationId}}",
            "result": {
              "kind": "top_up",
              "result": {
                "transaction_hash": "{{transactionHash ?? TransactionHash}}",
                "finalized_block_height": {{finalizedBlockHeight}},
                "server_time_ms": {{serverTimeMilliseconds}},
                "anchor": {
                  "version": 4,
                  "topup_operation_id": [{{FixedBytesJson(anchorOperationByte)}}],
                  "artifact_binding": {"version": 4}
                },
                "finality_proof": {
                  "version": {{finalityProofVersion}},
                  "anchor": {
                    "topup_operation_id": [{{FixedBytesJson(finalityOperationByte)}}]
                  }
                }
              }
            }
          }
        }
        """;

    private static string PendingStatusJson(ulong submittedAtMilliseconds) => $$"""
        {
          "state": "pending",
          "value": {
            "operation_id": "{{OperationId}}",
            "kind": {"kind": "top_up", "value": null},
            "transaction_hash": "{{TransactionHash}}",
            "submitted_at_ms": {{submittedAtMilliseconds}}
          }
        }
        """;

    private static string RedeemStatusJson(
        ulong finalizedBlockHeight = 42,
        ulong serverTimeMilliseconds = 1234) => $$"""
        {
          "state": "applied",
          "value": {
            "operation_id": "{{OperationId}}",
            "result": {
              "kind": "redeem",
              "result": {
                "transaction_hash": "{{TransactionHash}}",
                "finalized_block_height": {{finalizedBlockHeight}},
                "server_time_ms": {{serverTimeMilliseconds}}
              }
            }
          }
        }
        """;

    private static string RejectedStatusJson(
        string code = "busy",
        string message = "retry later",
        string? detailsJson = null,
        string? extraErrorField = null)
    {
        var details = detailsJson is null ? string.Empty : $", \"details\": {detailsJson}";
        var extra = extraErrorField is null ? string.Empty : $", {extraErrorField}";
        return $$"""
            {
              "state": "rejected",
              "value": {
                "operation_id": "{{OperationId}}",
                "kind": {"kind": "top_up", "value": null},
                "transaction_hash": "{{TransactionHash}}",
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
}
