using System.Net;
using System.Text;
using System.Text.Json;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class KagemushaToriiTests
{
    private static readonly string OperationId = new('1', 64);
    private static readonly string TransactionHash = new('2', 64);

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
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var capability = await client.GetOfflineCapabilityAsync(
            TestContext.Current.CancellationToken);

        Assert.Equal("cash_handoff_v1", capability.CashHandoffCapability);
        Assert.Equal(22U, capability.RequiredBridgeAbiVersion);
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
    public async Task TopUpTransportsAnExternalV4NoritoArchiveWithoutAProverClaim()
    {
        var archive = NoritoArchive();
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
            return response;
        });
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
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
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var reference = await client.SubmitKagemushaRedeemV4Async(
            new ToriiKagemushaRedeemRequestV4(OperationId, NoritoArchive()),
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

    private static byte[] NoritoArchive()
    {
        var archive = new byte[40];
        Encoding.ASCII.GetBytes("NRT0").CopyTo(archive, 0);
        return archive;
    }

    private static string OperationReferenceJson(string kind) => $$"""
        {
          "operation_id": "{{OperationId}}",
          "kind": {"kind": "{{kind}}", "value": null},
          "state": {"state": "pending", "value": null},
          "transaction_hash": "{{TransactionHash}}",
          "status_uri": "/v1/offline/operations/{{OperationId}}",
          "submitted_at_ms": 1234
        }
        """;

    private static string OfflineCapabilityJson(
        string cashHandoffCapability = "cash_handoff_v1",
        int abiVersion = 22,
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
