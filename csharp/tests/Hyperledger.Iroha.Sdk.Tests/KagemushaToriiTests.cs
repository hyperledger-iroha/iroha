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
    public async Task ReadinessUsesTheStableRouteAndRequiresAbi20V4()
    {
        using var handler = new KagemushaHandler(request =>
        {
            Assert.Equal(HttpMethod.Get, request.Method);
            Assert.Equal("/v1/offline/readiness", request.RequestUri!.AbsolutePath);
            Assert.Equal("coin#wonderland", ParseQuery(request.RequestUri.Query)["asset_definition_id"]);
            return JsonResponse(UnavailableReadinessJson(20));
        });
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var readiness = await client.GetKagemushaReadinessV4Async(
            "coin#wonderland",
            TestContext.Current.CancellationToken);

        Assert.Equal(20U, readiness.RequiredBridgeAbiVersion);
        Assert.Equal(8U, readiness.MaxHops);
        Assert.False(readiness.Ready);
        Assert.False(readiness.ProofBackendAvailable);
        Assert.False(readiness.RecursiveLineageSupported);
        Assert.Null(readiness.ArtifactSet);
    }

    [Fact]
    public async Task ReadinessRejectsAbi19InsteadOfUpgradingIt()
    {
        using var handler = new KagemushaHandler(_ => JsonResponse(UnavailableReadinessJson(19)));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var error = await Assert.ThrowsAsync<JsonException>(() =>
            client.GetKagemushaReadinessV4Async(
                "coin#wonderland",
                TestContext.Current.CancellationToken));

        Assert.Contains("required_bridge_abi_version must be 20", error.Message);
    }

    [Fact]
    public async Task TopUpTransportsAnExternalV4NoritoArchiveWithoutAProverClaim()
    {
        var archive = NoritoArchive();
        using var handler = new KagemushaHandler(request =>
        {
            Assert.Equal(HttpMethod.Post, request.Method);
            Assert.Equal("/v1/offline/top-up", request.RequestUri!.AbsolutePath);
            Assert.Equal("application/x-norito", request.Content!.Headers.ContentType!.MediaType);
            Assert.Equal(OperationId, Assert.Single(request.Headers.GetValues("Idempotency-Key")));
            Assert.Equal(archive, request.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult());
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
                    "anchor": {"version": 3, "artifact_binding": {"version": 4}},
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

    private static string UnavailableReadinessJson(int abiVersion) => $$"""
        {
          "required_bridge_abi_version": {{abiVersion}},
          "max_hops": 8,
          "asset_definition_id": "coin#wonderland",
          "asset_scale": null,
          "evaluated_block_height": 7,
          "evaluated_block_hash": "{{new string('a', 64)}}",
          "active_transfer_verifier": null,
          "active_topup_shield_verifier": null,
          "active_unshield_verifier": null,
          "active_recursive_step_eq_verifier": null,
          "active_recursive_step_ep_verifier": null,
          "artifact_set": null,
          "proof_backend_available": false,
          "recursive_lineage_supported": false,
          "ready": false,
          "blockers": [{"code": "recursive_v4_registry_unavailable", "message": "not provisioned"}]
        }
        """;

    private static HttpResponseMessage JsonResponse(
        string json,
        HttpStatusCode status = HttpStatusCode.OK) =>
        new(status)
        {
            Content = new StringContent(json, Encoding.UTF8, "application/json"),
        };

    private static Dictionary<string, string> ParseQuery(string query) =>
        query.TrimStart('?')
            .Split('&', StringSplitOptions.RemoveEmptyEntries)
            .Select(part => part.Split('=', 2))
            .ToDictionary(
                pair => Uri.UnescapeDataString(pair[0]),
                pair => Uri.UnescapeDataString(pair[1]),
                StringComparer.Ordinal);

    private sealed class KagemushaHandler(
        Func<HttpRequestMessage, HttpResponseMessage> responder) : HttpMessageHandler
    {
        protected override Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken) =>
            Task.FromResult(responder(request));
    }
}
