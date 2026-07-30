using System.Globalization;
using System.Net;
using System.Net.Http.Headers;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.SoraFs;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class SoraFsReputationClientTests
{
    private const string AccountId =
        "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";
    private const string ProviderId = "provider:alpha";
    private const string SnapshotId = "11111111111111111111111111111111";
    private const string SecondSnapshotId = "22222222222222222222222222222222";
    private const string ThirdSnapshotId = "33333333333333333333333333333333";
    private static readonly string MerkleRootHex = new('a', 64);
    private static readonly string RawMetricsHashHex = new('b', 64);
    private static readonly byte[] PrivateKeySeed =
        Convert.FromHexString("616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032");

    [Fact]
    public async Task AuthenticatedReadsUseExactOneShotEmptyGetsAndFreshCanonicalSignatures()
    {
        using var handler = new RecordingHandler(request =>
        {
            var path = request.RequestUri!.AbsolutePath;
            var query = request.RequestUri.Query;
            return path switch
            {
                "/v1/sorafs/reputation/latest" =>
                    JsonResponse(SnapshotJson(limit: 7, generatedAt: ulong.MaxValue)),
                $"/v1/sorafs/reputation/providers/{ProviderId}" =>
                    JsonResponse(ProviderResponseJson()),
                $"/v1/sorafs/reputation/snapshots/{SnapshotId}" =>
                    JsonResponse(SnapshotJson(limit: 3)),
                "/v1/sorafs/reputation/weights" =>
                    JsonResponse(WeightsResponseJson()),
                "/v1/sorafs/reputation/events" when query == "?since=8&limit=2" =>
                    JsonResponse(EventPageJson("8", 2, EventJson(9, SnapshotId, null, 90))),
                _ => new HttpResponseMessage(HttpStatusCode.NotFound),
            };
        });
        using var client = CreateClient(handler);

        var latest = await client.GetSoraFsReputationLatestAsync(
            7,
            TestContext.Current.CancellationToken);
        var provider = await client.GetSoraFsReputationProviderAsync(
            ProviderId,
            TestContext.Current.CancellationToken);
        var snapshot = await client.GetSoraFsReputationSnapshotAsync(
            SnapshotId,
            3,
            TestContext.Current.CancellationToken);
        var weights = await client.GetSoraFsReputationWeightsAsync(
            TestContext.Current.CancellationToken);
        var events = await client.GetSoraFsReputationEventsAsync(
            8,
            2,
            TestContext.Current.CancellationToken);

        Assert.Equal(ulong.MaxValue, latest!.GeneratedAtUnix);
        Assert.Equal((ulong)7, latest.Limit);
        Assert.Equal(ProviderId, provider!.Provider.ProviderId);
        Assert.Equal((uint)1, provider.Proof.LeafCount);
        Assert.Equal(SnapshotId, snapshot!.SnapshotIdHex);
        Assert.Equal((ulong)3, snapshot.Limit);
        Assert.Equal((ushort)2_200, weights.Weights.PorSuccessBps);
        Assert.Equal((ulong)8, events.Since);
        Assert.Equal((ulong)9, events.NextSince);
        Assert.Equal(5, handler.Requests.Count);
        Assert.Equal("?limit=7", handler.Requests[0].Query);
        Assert.Equal(string.Empty, handler.Requests[1].Query);
        Assert.Equal("?limit=3", handler.Requests[2].Query);
        Assert.Equal(string.Empty, handler.Requests[3].Query);
        Assert.Equal("?since=8&limit=2", handler.Requests[4].Query);
        Assert.Contains(
            handler.Requests,
            request => request.AbsolutePath == $"/v1/sorafs/reputation/providers/{ProviderId}");
        Assert.All(handler.Requests, AssertExactSignedEmptyGet);
        Assert.Equal(
            handler.Requests.Count,
            handler.Requests.Select(RequestNonce).Distinct(StringComparer.Ordinal).Count());
    }

    [Theory]
    [InlineData(HttpStatusCode.Found)]
    [InlineData(HttpStatusCode.TemporaryRedirect)]
    [InlineData(HttpStatusCode.ServiceUnavailable)]
    public async Task ReputationReadsNeverFollowRedirectOrRetry(HttpStatusCode status)
    {
        using var handler = new RecordingHandler(_ =>
        {
            var response = new HttpResponseMessage(status);
            if ((int)status is >= 300 and < 400)
            {
                response.Headers.Location = new Uri("https://redirect.example/v1/sorafs/reputation/latest");
            }
            return response;
        });
        using var client = CreateClient(handler);

        var error = await Assert.ThrowsAsync<ToriiApiException>(() =>
            client.GetSoraFsReputationLatestAsync(
                cancellationToken: TestContext.Current.CancellationToken));

        Assert.Equal((HttpStatusCode?)status, error.StatusCode);
        Assert.Single(handler.Requests);
    }

    [Fact]
    public async Task ReputationReadFailsClosedForUnverifiedInjectedHttpClient()
    {
        using var handler = new RecordingHandler(_ =>
            throw new InvalidOperationException("unverified transport reached dispatch"));
        using var httpClient = new HttpClient(handler);
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            httpClient,
            new ToriiClientOptions
            {
                CanonicalRequestCredentials = new CanonicalRequestCredentials(
                    AccountId,
                    PrivateKeySeed),
            });

        var error = await Assert.ThrowsAsync<InvalidOperationException>(() =>
            client.GetSoraFsReputationLatestAsync(
                cancellationToken: TestContext.Current.CancellationToken));

        Assert.Contains("internally managed one-shot", error.Message, StringComparison.Ordinal);
        Assert.Empty(handler.Requests);
    }

    [Fact]
    public async Task ReputationReadRejectsResponseBeyondFourMiBBeforeParsing()
    {
        var bytes = new byte[ToriiClient.SoraFsReputationMaximumResponseBytes + 1];
        using var handler = new RecordingHandler(_ =>
        {
            var content = new StreamContent(new ChunkedReadStream(bytes, 8 * 1024));
            content.Headers.ContentType = MediaTypeHeaderValue.Parse(
                "application/json; charset=utf-8");
            return new HttpResponseMessage(HttpStatusCode.OK) { Content = content };
        });
        using var client = CreateClient(handler);

        var error = await Assert.ThrowsAsync<InvalidDataException>(() =>
            client.GetSoraFsReputationLatestAsync(
                cancellationToken: TestContext.Current.CancellationToken));

        Assert.Contains("4194304-byte limit", error.Message, StringComparison.Ordinal);
        Assert.Single(handler.Requests);
    }

    [Fact]
    public async Task ReputationReadRejectsInvalidUtf8BeforeJsonProjection()
    {
        using var handler = new RecordingHandler(_ =>
        {
            var content = new ByteArrayContent([0xff, 0xfe, 0xfd]);
            content.Headers.ContentType = MediaTypeHeaderValue.Parse("application/json");
            return new HttpResponseMessage(HttpStatusCode.OK) { Content = content };
        });
        using var client = CreateClient(handler);

        var error = await Assert.ThrowsAsync<JsonException>(() =>
            client.GetSoraFsReputationLatestAsync(
                cancellationToken: TestContext.Current.CancellationToken));

        Assert.Contains("strict UTF-8", error.Message, StringComparison.Ordinal);
    }

    [Fact]
    public async Task ReputationJsonRejectsDuplicateAndUnknownFields()
    {
        var valid = SnapshotJson();
        var duplicate = valid.Insert(
            1,
            $"\"snapshot_id_hex\":\"{SnapshotId}\",");
        var unknown = $"{valid[..^1]},\"unknown\":true}}";
        var responses = new Queue<string>([duplicate, unknown]);
        using var handler = new RecordingHandler(_ => JsonResponse(responses.Dequeue()));
        using var client = CreateClient(handler);

        var duplicateError = await Assert.ThrowsAsync<JsonException>(() =>
            client.GetSoraFsReputationLatestAsync(
                cancellationToken: TestContext.Current.CancellationToken));
        var unknownError = await Assert.ThrowsAsync<JsonException>(() =>
            client.GetSoraFsReputationLatestAsync(
                cancellationToken: TestContext.Current.CancellationToken));

        Assert.Contains("appears more than once", duplicateError.Message, StringComparison.Ordinal);
        Assert.Contains("unknown=[unknown]", unknownError.Message, StringComparison.Ordinal);
        Assert.Equal(2, handler.Requests.Count);
    }

    [Fact]
    public async Task ReputationProjectionPreservesUnsigned64BitMaximum()
    {
        var eventJson = EventJson(
            ulong.MaxValue,
            SnapshotId,
            previousSnapshotId: null,
            generatedAt: ulong.MaxValue);
        using var handler = new RecordingHandler(request =>
            request.RequestUri!.AbsolutePath.EndsWith("/latest", StringComparison.Ordinal)
                ? JsonResponse(SnapshotJson(generatedAt: ulong.MaxValue))
                : JsonResponse(EventPageJson(
                    (ulong.MaxValue - 1).ToString(CultureInfo.InvariantCulture),
                    1,
                    eventJson)));
        using var client = CreateClient(handler);

        var latest = await client.GetSoraFsReputationLatestAsync(
            cancellationToken: TestContext.Current.CancellationToken);
        var events = await client.GetSoraFsReputationEventsAsync(
            ulong.MaxValue - 1,
            1,
            TestContext.Current.CancellationToken);

        Assert.Equal(ulong.MaxValue, latest!.GeneratedAtUnix);
        Assert.Equal(ulong.MaxValue, events.Events[0].Sequence);
        Assert.Equal(ulong.MaxValue, events.Events[0].GeneratedAtUnix);
        Assert.Equal(ulong.MaxValue, events.NextSince);
    }

    [Fact]
    public async Task ReputationSnapshotBindsRequestedSnapshotAndLimit()
    {
        var responses = new Queue<string>(
        [
            SnapshotJson(limit: 4, snapshotId: SecondSnapshotId),
            SnapshotJson(limit: 5, snapshotId: SnapshotId),
        ]);
        using var handler = new RecordingHandler(_ => JsonResponse(responses.Dequeue()));
        using var client = CreateClient(handler);

        var snapshotError = await Assert.ThrowsAsync<JsonException>(() =>
            client.GetSoraFsReputationSnapshotAsync(
                SnapshotId,
                4,
                TestContext.Current.CancellationToken));
        var limitError = await Assert.ThrowsAsync<JsonException>(() =>
            client.GetSoraFsReputationSnapshotAsync(
                SnapshotId,
                4,
                TestContext.Current.CancellationToken));

        Assert.Contains("requested snapshot", snapshotError.Message, StringComparison.Ordinal);
        Assert.Contains("requested limit", limitError.Message, StringComparison.Ordinal);
    }

    [Fact]
    public async Task ReputationProviderBindsRequestAndRequiresExactProofShape()
    {
        var responses = new Queue<string>(
        [
            ProviderResponseJson(providerId: "provider:other"),
            ProviderResponseJson(leafCount: 2, siblings: "[]"),
        ]);
        using var handler = new RecordingHandler(_ => JsonResponse(responses.Dequeue()));
        using var client = CreateClient(handler);

        var providerError = await Assert.ThrowsAsync<JsonException>(() =>
            client.GetSoraFsReputationProviderAsync(
                ProviderId,
                TestContext.Current.CancellationToken));
        var proofError = await Assert.ThrowsAsync<JsonException>(() =>
            client.GetSoraFsReputationProviderAsync(
                ProviderId,
                TestContext.Current.CancellationToken));

        Assert.Contains("requested provider", providerError.Message, StringComparison.Ordinal);
        Assert.Contains("exact Merkle depth", proofError.Message, StringComparison.Ordinal);
        Assert.All(handler.Requests, request =>
            Assert.Equal($"/v1/sorafs/reputation/providers/{ProviderId}", request.AbsolutePath));
    }

    [Fact]
    public async Task ReputationProviderRejectsTooManyOrNoncanonicalDegradationFlags()
    {
        const string sixFlags =
            """[{"flag":"reserve_warning","value":null},{"flag":"reserve_grace","value":null},{"flag":"reserve_delinquent","value":null},{"flag":"reserve_default","value":null},{"flag":"proof_success_below90","value":null},{"flag":"proof_success_below80","value":null}]""";
        const string wrongOrder =
            """[{"flag":"active_dispute","value":null},{"flag":"reserve_warning","value":null}]""";
        var responses = new Queue<string>(
        [
            ProviderResponseJson(flags: sixFlags),
            ProviderResponseJson(flags: wrongOrder),
        ]);
        using var handler = new RecordingHandler(_ => JsonResponse(responses.Dequeue()));
        using var client = CreateClient(handler);

        var countError = await Assert.ThrowsAsync<JsonException>(() =>
            client.GetSoraFsReputationProviderAsync(
                ProviderId,
                TestContext.Current.CancellationToken));
        var orderError = await Assert.ThrowsAsync<JsonException>(() =>
            client.GetSoraFsReputationProviderAsync(
                ProviderId,
                TestContext.Current.CancellationToken));

        Assert.Contains("at most five", countError.Message, StringComparison.Ordinal);
        Assert.Contains("canonical order", orderError.Message, StringComparison.Ordinal);
    }

    [Fact]
    public async Task ReputationWeightsRejectNoncanonicalBasisPointBudget()
    {
        using var handler = new RecordingHandler(_ =>
            JsonResponse(WeightsResponseJson(repairBreachBps: 999)));
        using var client = CreateClient(handler);

        var error = await Assert.ThrowsAsync<JsonException>(() =>
            client.GetSoraFsReputationWeightsAsync(
                TestContext.Current.CancellationToken));

        Assert.Contains("sum to exactly 10000", error.Message, StringComparison.Ordinal);
        Assert.Single(handler.Requests);
    }

    [Fact]
    public async Task ReputationEventsBindSinceAndEnforceContinuity()
    {
        var wrongSince = EventPageJson(
            "6",
            2,
            EventJson(6, SnapshotId, null, 60));
        var discontinuousEvents = string.Join(
            ",",
            EventJson(6, SnapshotId, null, 60),
            EventJson(8, SecondSnapshotId, SnapshotId, 80));
        var responses = new Queue<string>(
        [
            wrongSince,
            EventPageJson("5", 2, discontinuousEvents),
        ]);
        using var handler = new RecordingHandler(_ => JsonResponse(responses.Dequeue()));
        using var client = CreateClient(handler);

        var sinceError = await Assert.ThrowsAsync<JsonException>(() =>
            client.GetSoraFsReputationEventsAsync(
                5,
                2,
                TestContext.Current.CancellationToken));
        var continuityError = await Assert.ThrowsAsync<JsonException>(() =>
            client.GetSoraFsReputationEventsAsync(
                5,
                2,
                TestContext.Current.CancellationToken));

        Assert.Contains("requested cursor", sinceError.Message, StringComparison.Ordinal);
        Assert.Contains("contiguous", continuityError.Message, StringComparison.Ordinal);
    }

    [Fact]
    public async Task ReputationEventsEnforceAdjacentSnapshotAndTimestampContinuity()
    {
        var wrongLink = string.Join(
            ",",
            EventJson(6, SnapshotId, null, 60),
            EventJson(7, SecondSnapshotId, ThirdSnapshotId, 70));
        var nonIncreasingTime = string.Join(
            ",",
            EventJson(6, SnapshotId, null, 60),
            EventJson(7, SecondSnapshotId, SnapshotId, 60));
        var responses = new Queue<string>(
        [
            EventPageJson("5", 2, wrongLink),
            EventPageJson("5", 2, nonIncreasingTime),
        ]);
        using var handler = new RecordingHandler(_ => JsonResponse(responses.Dequeue()));
        using var client = CreateClient(handler);

        var linkError = await Assert.ThrowsAsync<JsonException>(() =>
            client.GetSoraFsReputationEventsAsync(
                5,
                2,
                TestContext.Current.CancellationToken));
        var timeError = await Assert.ThrowsAsync<JsonException>(() =>
            client.GetSoraFsReputationEventsAsync(
                5,
                2,
                TestContext.Current.CancellationToken));

        Assert.Contains("snapshot ids", linkError.Message, StringComparison.Ordinal);
        Assert.Contains("timestamps", timeError.Message, StringComparison.Ordinal);
    }

    [Fact]
    public async Task ReputationSseIsOneShotStreamingAndValidatesLaggedContinuity()
    {
        var first = EventJson(8, SnapshotId, null, 80);
        var second = EventJson(9, SecondSnapshotId, SnapshotId, 90);
        var sse =
            $"event: lagged\ndata: 2\n\n"
            + $"id: 8\nevent: reputation_snapshot\ndata: {first}\n\n"
            + $"event: reputation_snapshot\nid: 9\ndata: {second}\n\n";
        using var handler = new RecordingHandler(_ => SseResponse(sse, chunkSize: 3));
        using var client = CreateClient(handler);
        var frames = new List<SoraFsReputationSseFrameV1>();

        await foreach (var frame in client.StreamSoraFsReputationEventsAsync(
                           5,
                           2,
                           TestContext.Current.CancellationToken))
        {
            frames.Add(frame);
        }

        var lagged = Assert.IsType<SoraFsReputationLaggedSseFrameV1>(frames[0]);
        var snapshot = Assert.IsType<SoraFsReputationSnapshotSseFrameV1>(frames[1]);
        var next = Assert.IsType<SoraFsReputationSnapshotSseFrameV1>(frames[2]);
        Assert.Equal((ulong)2, lagged.Skipped);
        Assert.Equal((ulong)8, snapshot.Id);
        Assert.Equal((ulong)9, next.Event.Sequence);
        Assert.Single(handler.Requests);
        Assert.Equal(
            "/v1/sorafs/reputation/events/stream",
            handler.Requests[0].AbsolutePath);
        Assert.Equal("?since=5&limit=2", handler.Requests[0].Query);
        Assert.False(handler.Requests[0].Headers.ContainsKey("Last-Event-ID"));
        AssertExactSignedEmptyGet(handler.Requests[0]);
    }

    [Theory]
    [MemberData(nameof(InvalidSseStreams))]
    public async Task ReputationSseRejectsReplayGapsResumeAndUnsupportedFields(
        ulong since,
        string sse,
        string expectedMessage)
    {
        using var handler = new RecordingHandler(_ => SseResponse(sse, chunkSize: 5));
        using var client = CreateClient(handler);

        var error = await Assert.ThrowsAsync<InvalidDataException>(async () =>
        {
            await foreach (var _ in client.StreamSoraFsReputationEventsAsync(
                               since,
                               5,
                               TestContext.Current.CancellationToken))
            {
            }
        });

        Assert.Contains(expectedMessage, error.Message, StringComparison.Ordinal);
        Assert.Single(handler.Requests);
    }

    [Fact]
    public async Task ReputationSseRejectsFramesBeyondSixtyFourKiB()
    {
        var oversized = new string('1', ToriiClient.SoraFsReputationMaximumSseFrameBytes);
        using var handler = new RecordingHandler(_ =>
            SseResponse($"event: lagged\ndata: {oversized}\n\n", chunkSize: 8 * 1024));
        using var client = CreateClient(handler);

        var error = await Assert.ThrowsAsync<InvalidDataException>(async () =>
        {
            await foreach (var _ in client.StreamSoraFsReputationEventsAsync(
                               cancellationToken: TestContext.Current.CancellationToken))
            {
            }
        });

        Assert.Contains("65536 bytes", error.Message, StringComparison.Ordinal);
        Assert.Single(handler.Requests);
    }

    [Theory]
    [InlineData("Last-Event-ID")]
    [InlineData("X-Iroha-Witness")]
    [InlineData("X-Iroha-Signature")]
    public async Task ReputationStreamRejectsInjectedResumeOrAlternativeAuthHeaders(
        string headerName)
    {
        using var handler = new RecordingHandler(_ =>
            throw new InvalidOperationException("invalid default header reached transport"));
        using var httpClient = new HttpClient(handler);
        httpClient.DefaultRequestHeaders.TryAddWithoutValidation(headerName, "forbidden");
        using var client = CreateClient(httpClient);

        var error = await Assert.ThrowsAsync<InvalidOperationException>(async () =>
        {
            await foreach (var _ in client.StreamSoraFsReputationEventsAsync(
                               cancellationToken: TestContext.Current.CancellationToken))
            {
            }
        });

        Assert.Contains(headerName, error.Message, StringComparison.OrdinalIgnoreCase);
        Assert.Empty(handler.Requests);
    }

    [Theory]
    [InlineData("")]
    [InlineData("provider/alpha")]
    [InlineData("provider%3Aalpha")]
    [InlineData("provider@alpha")]
    [InlineData(" provider")]
    [InlineData("提供者")]
    [InlineData(".")]
    [InlineData("..")]
    public async Task ReputationProviderRejectsNoncanonicalIdentifiersBeforeSigning(
        string providerId)
    {
        using var handler = new RecordingHandler(_ =>
            throw new InvalidOperationException("invalid provider reached transport"));
        using var client = CreateClient(handler);

        await Assert.ThrowsAsync<ArgumentException>(() =>
            client.GetSoraFsReputationProviderAsync(
                providerId,
                TestContext.Current.CancellationToken));
        Assert.Empty(handler.Requests);
    }

    [Fact]
    public async Task ReputationProviderPreservesEveryAllowedPathCharacter()
    {
        const string providerId = "provider:alpha";
        using var handler = new RecordingHandler(_ =>
            JsonResponse(ProviderResponseJson(providerId: providerId)));
        using var client = CreateClient(handler);

        var response = await client.GetSoraFsReputationProviderAsync(
            providerId,
            TestContext.Current.CancellationToken);

        Assert.Equal(providerId, response!.Provider.ProviderId);
        Assert.Equal(
            $"/v1/sorafs/reputation/providers/{providerId}",
            Assert.Single(handler.Requests).AbsolutePath);
    }

    public static IEnumerable<object[]> InvalidSseStreams()
    {
        var replay = EventJson(5, SnapshotId, null, 50);
        yield return
        [
            (ulong)5,
            $"event: reputation_snapshot\nid: 5\ndata: {replay}\n\n",
            "do not bind",
        ];

        var gap = EventJson(7, SnapshotId, null, 70);
        yield return
        [
            (ulong)5,
            $"event: reputation_snapshot\nid: 7\ndata: {gap}\n\n",
            "do not bind",
        ];

        var wrongLag = EventJson(9, SnapshotId, null, 90);
        yield return
        [
            (ulong)5,
            $"event: lagged\ndata: 2\n\nevent: reputation_snapshot\nid: 9\ndata: {wrongLag}\n\n",
            "do not bind",
        ];

        yield return
        [
            (ulong)0,
            "retry: 1000\nevent: lagged\ndata: 1\n\n",
            "unsupported field",
        ];

        var first = EventJson(6, SnapshotId, null, 60);
        var wrongLink = EventJson(7, SecondSnapshotId, ThirdSnapshotId, 70);
        yield return
        [
            (ulong)5,
            $"event: reputation_snapshot\nid: 6\ndata: {first}\n\n"
            + $"event: reputation_snapshot\nid: 7\ndata: {wrongLink}\n\n",
            "continuous snapshot chain",
        ];
    }

    private static ToriiClient CreateClient(RecordingHandler handler) =>
        CreateClient(new HttpClient(handler));

    private static ToriiClient CreateClient(HttpClient httpClient) =>
        new(
            new Uri("https://torii.example"),
            httpClient,
            new ToriiClientOptions
            {
                CanonicalRequestCredentials = new CanonicalRequestCredentials(
                    AccountId,
                    PrivateKeySeed),
            },
            SoraFsReputationTransportAssurance.OneShotWithoutRedirectsRetriesOrDecompression);

    private static HttpResponseMessage JsonResponse(string json)
    {
        var content = new ByteArrayContent(Encoding.UTF8.GetBytes(json));
        content.Headers.ContentType = MediaTypeHeaderValue.Parse(
            "application/json; charset=utf-8");
        return new HttpResponseMessage(HttpStatusCode.OK) { Content = content };
    }

    private static HttpResponseMessage SseResponse(string sse, int chunkSize)
    {
        var content = new StreamContent(
            new ChunkedReadStream(Encoding.UTF8.GetBytes(sse), chunkSize));
        content.Headers.ContentType = MediaTypeHeaderValue.Parse(
            "text/event-stream; charset=utf-8");
        return new HttpResponseMessage(HttpStatusCode.OK) { Content = content };
    }

    private static string WeightsJson(int repairBreachBps = 1_000) =>
        $$"""{"version":1,"por_success_bps":2200,"pdp_success_bps":2000,"potr_success_bps":1800,"latency_bps":1500,"dispute_bps":1000,"token_violation_bps":500,"repair_breach_bps":{{repairBreachBps}}}""";

    private static string MetricsJson() =>
        """{"version":1,"por_success_bps":9000,"pdp_success_bps":9100,"potr_success_bps":9200,"latency_health_bps":9300,"dispute_rate_bps":100,"token_violation_rate_bps":200,"repair_breach_rate_bps":300}""";

    private static string ProviderJson(
        string providerId = ProviderId,
        string flags = "[]") =>
        $$"""{"provider_id":"{{providerId}}","score_bps":8500,"degradation_flags":{{flags}},"raw_metrics":{{MetricsJson()}},"raw_metrics_hash_hex":"{{RawMetricsHashHex}}"}""";

    private static string SnapshotJson(
        int limit = ToriiClient.SoraFsReputationDefaultPageLimit,
        ulong generatedAt = 42,
        string snapshotId = SnapshotId) =>
        $$"""{"snapshot_id_hex":"{{snapshotId}}","generated_at_unix":{{generatedAt}},"previous_snapshot_id_hex":null,"merkle_root_hex":"{{MerkleRootHex}}","provider_count":1,"returned_provider_count":1,"limit":{{limit}},"truncated_providers":false,"alpha_bps":8500,"current_score_weight_bps":7000,"weights":{{WeightsJson()}},"providers":[{{ProviderJson()}}]}""";

    private static string ProviderResponseJson(
        string providerId = ProviderId,
        uint leafCount = 1,
        string siblings = "[]",
        string flags = "[]") =>
        $$$"""{"snapshot_id_hex":"{{{SnapshotId}}}","generated_at_unix":42,"merkle_root_hex":"{{{MerkleRootHex}}}","provider":{{{ProviderJson(providerId, flags)}}},"proof":{"provider_id":"{{{providerId}}}","leaf_index":0,"leaf_count":{{{leafCount}}},"siblings_hex":{{{siblings}}}}}""";

    private static string WeightsResponseJson(int repairBreachBps = 1_000) =>
        $$"""{"snapshot_id_hex":"{{SnapshotId}}","generated_at_unix":42,"alpha_bps":8500,"current_score_weight_bps":7000,"weights":{{WeightsJson(repairBreachBps)}}}""";

    private static string EventJson(
        ulong sequence,
        string snapshotId,
        string? previousSnapshotId,
        ulong generatedAt)
    {
        var previous = previousSnapshotId is null ? "null" : $"\"{previousSnapshotId}\"";
        return $$"""{"version":1,"sequence":{{sequence}},"snapshot_id_hex":"{{snapshotId}}","generated_at_unix":{{generatedAt}},"merkle_root_hex":"{{MerkleRootHex}}","provider_count":1,"previous_snapshot_id_hex":{{previous}}}""";
    }

    private static string EventPageJson(string since, int limit, string events)
    {
        var count = events.Length == 0 ? 0 : events.Count(character => character == '{');
        var lastSequence = count == 0
            ? "null"
            : ExtractLastSequence(events);
        return $$"""{"since":{{since}},"limit":{{limit}},"count":{{count}},"next_since":{{lastSequence}},"events":[{{events}}]}""";
    }

    private static string ExtractLastSequence(string events)
    {
        var marker = "\"sequence\":";
        var index = events.LastIndexOf(marker, StringComparison.Ordinal) + marker.Length;
        var end = index;
        while (end < events.Length && char.IsAsciiDigit(events[end]))
        {
            end++;
        }
        return events[index..end];
    }

    private static void AssertExactSignedEmptyGet(CapturedRequest request)
    {
        Assert.Equal("GET", request.Method);
        Assert.False(request.HasContent);
        Assert.Equal("identity", Assert.Single(request.Headers["Accept-Encoding"]));
        Assert.Equal("no-store", Assert.Single(request.Headers["Cache-Control"]));
        Assert.False(request.Headers.ContainsKey("Last-Event-ID"));
        Assert.False(request.Headers.ContainsKey("X-Iroha-Witness"));
        Assert.Equal(AccountId, Assert.Single(request.Headers["X-Iroha-Account"]));

        var nonce = RequestNonce(request);
        Assert.Equal(32, nonce.Length);
        Assert.All(nonce, character =>
            Assert.True(character is >= '0' and <= '9' or >= 'a' and <= 'f'));
        var timestamp = long.Parse(
            Assert.Single(request.Headers["X-Iroha-Timestamp-Ms"]),
            NumberStyles.None,
            CultureInfo.InvariantCulture);
        var bodyHash = Convert.ToHexString(SHA256.HashData([])).ToLowerInvariant();
        var canonicalQuery = CanonicalRequest.BuildCanonicalQueryString(request.Query);
        var message = Encoding.UTF8.GetBytes(
            $"GET\n{request.AbsolutePath}\n{canonicalQuery}\n{bodyHash}\n{timestamp}\n{nonce}");
        var signature = Convert.FromBase64String(
            Assert.Single(request.Headers["X-Iroha-Signature"]));
        Assert.True(Ed25519Signer.Verify(
            message,
            signature,
            Ed25519Signer.GetPublicKey(PrivateKeySeed)));
    }

    private static string RequestNonce(CapturedRequest request) =>
        Assert.Single(request.Headers["X-Iroha-Nonce"]);

    private sealed record CapturedRequest(
        string Method,
        string AbsoluteUri,
        string AbsolutePath,
        string Query,
        bool HasContent,
        IReadOnlyDictionary<string, string[]> Headers);

    private sealed class RecordingHandler(
        Func<HttpRequestMessage, HttpResponseMessage> responder) : HttpMessageHandler
    {
        public List<CapturedRequest> Requests { get; } = [];

        protected override Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken)
        {
            var headers = request.Headers.ToDictionary(
                header => header.Key,
                header => header.Value.ToArray(),
                StringComparer.OrdinalIgnoreCase);
            Requests.Add(new CapturedRequest(
                request.Method.Method,
                request.RequestUri!.AbsoluteUri,
                request.RequestUri.AbsolutePath,
                request.RequestUri.Query,
                request.Content is not null,
                headers));
            var response = responder(request);
            response.RequestMessage ??= request;
            return Task.FromResult(response);
        }
    }

    private sealed class ChunkedReadStream(byte[] bytes, int chunkSize) : MemoryStream(bytes)
    {
        public override bool CanSeek => false;

        public override ValueTask<int> ReadAsync(
            Memory<byte> buffer,
            CancellationToken cancellationToken = default)
        {
            var length = Math.Min(buffer.Length, chunkSize);
            return base.ReadAsync(buffer[..length], cancellationToken);
        }
    }
}
