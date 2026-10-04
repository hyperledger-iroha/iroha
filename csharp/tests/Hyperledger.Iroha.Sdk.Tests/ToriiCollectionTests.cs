using System.Net;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.Query;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Collection reads over a mocked Torii transport.</summary>
public sealed class ToriiCollectionTests
{
    private const string AccountId = "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";
    private const string NetworkIdLiteral =
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";
    private static readonly byte[] PrivateKeySeed = Convert.FromHexString(
        "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032");

    public static IEnumerable<object[]> CollectionPaths()
    {
        yield return ["domains", "/v1/domains/query"];
        yield return ["accounts", "/v1/accounts/query"];
        yield return ["asset-definitions", "/v1/assets/definitions/query"];
        yield return ["nfts", "/v1/nfts/query"];
        yield return ["rwas", "/v1/rwas/query"];
        yield return ["repo-agreements", "/v1/repo/agreements/query"];
        yield return ["account-assets", "/v1/accounts/acc%231/assets/query"];
        yield return ["asset-holders", "/v1/assets/7ZepsJTHCVLKsrFFNZGSRGZgvBhv/holders/query"];
        yield return ["transactions", "/v1/transactions/query"];
        yield return ["account-transactions", "/v1/accounts/acc%231/transactions/query"];
    }

    [Theory]
    [MemberData(nameof(CollectionPaths))]
    public async Task EveryCollectionPostsTheCanonicalBodyToItsQueryRoute(string collection, string path)
    {
        using var handler = new RecordingHandler(_ => Page("""{"items":[],"next_cursor":null}"""));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var query = new ListQuery { Filter = Filter.Field("owned_by").Eq("alice"), Limit = 3 };

        var page = collection switch
        {
            "domains" => (await client.Domains.Rows.GetPageAsync(query, TestContext.Current.CancellationToken)).Items.Length,
            "accounts" => (await client.Accounts.GetPageAsync(query, TestContext.Current.CancellationToken)).Items.Length,
            "asset-definitions" => (await client.AssetDefinitions.GetPageAsync(query, TestContext.Current.CancellationToken)).Items.Length,
            "nfts" => (await client.Nfts.GetPageAsync(query, TestContext.Current.CancellationToken)).Items.Length,
            "rwas" => (await client.Rwas.GetPageAsync(query, TestContext.Current.CancellationToken)).Items.Length,
            "repo-agreements" => (await client.RepoAgreements.GetPageAsync(query, TestContext.Current.CancellationToken)).Items.Length,
            "account-assets" => (await client.AccountAssets("acc#1").GetPageAsync(query, TestContext.Current.CancellationToken)).Items.Length,
            "asset-holders" => (await client.AssetHolders("7ZepsJTHCVLKsrFFNZGSRGZgvBhv").GetPageAsync(query, TestContext.Current.CancellationToken)).Items.Length,
            "transactions" => (await client.Transactions.GetPageAsync(query, TestContext.Current.CancellationToken)).Items.Length,
            _ => (await client.AccountTransactions("acc#1").GetPageAsync(query, TestContext.Current.CancellationToken)).Items.Length,
        };

        Assert.Equal(0, page);
        var request = Assert.Single(handler.Requests);
        Assert.Equal(HttpMethod.Post, request.Method);
        Assert.Equal(path, request.Uri.AbsolutePath);
        Assert.Equal("""{"filter":{"op":"eq","args":["owned_by","alice"]},"limit":3}""", request.Body);
        Assert.Equal("application/json", request.ContentType);
        Assert.Contains("application/json", request.Accept);
        Assert.Null(request.Account);
    }

    [Fact]
    public async Task TypedRowsReadKnownFieldsExactlyAndIgnoreUnknownFields()
    {
        using var handler = new RecordingHandler(_ => Page($$"""
            {"items":[
              {"asset":"7ZepsJTHCVLKsrFFNZGSRGZgvBhv","asset_name":"ds","asset_alias":null,"scope":"global",
               "account_id":"{{AccountId}}","quantity":"340282366920938463463374607431768211455.25","future":{"x":1} }
            ],"next_cursor":"c_2","total":7}
            """));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var page = await client.AccountAssets(AccountId)
            .GetPageAsync(new ListQuery { IncludeTotal = true }, TestContext.Current.CancellationToken);

        var row = Assert.Single(page.Items);
        Assert.Equal("7ZepsJTHCVLKsrFFNZGSRGZgvBhv", row.Asset);
        Assert.Equal("ds", row.AssetName);
        Assert.Null(row.AssetAlias);
        Assert.Equal("global", row.Scope);
        Assert.Equal(AccountId, row.AccountId);
        Assert.Equal("340282366920938463463374607431768211455.25", row.Quantity.ToString());
        Assert.Equal("c_2", page.NextCursor);
        Assert.True(page.HasMore);
        Assert.Equal(7UL, page.Total);
        Assert.Equal("""{"include_total":true}""", Assert.Single(handler.Requests).Body);
    }

    [Fact]
    public async Task BalanceRowsRequireTheirIdentityFields()
    {
        foreach (var (row, field) in new[]
        {
            ("""{"asset":"a","scope":"global","quantity":"1"}""", "account_id"),
            ("""{"asset":"a","account_id":"b","quantity":"1"}""", "scope"),
            ("""{"asset":"a","scope":"global","account_id":"b","quantity":null}""", "quantity"),
        })
        {
            using var handler = new RecordingHandler(_ => Page($$"""{"items":[{{row}}],"next_cursor":null}"""));
            using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

            var assets = await Assert.ThrowsAsync<JsonException>(() =>
                client.AccountAssets(AccountId).GetPageAsync(cancellationToken: TestContext.Current.CancellationToken));
            var holders = await Assert.ThrowsAsync<JsonException>(() =>
                client.AssetHolders("a").GetPageAsync(cancellationToken: TestContext.Current.CancellationToken));

            Assert.Contains($"].{field} must be", assets.Message, StringComparison.Ordinal);
            Assert.Contains($"].{field} must be", holders.Message, StringComparison.Ordinal);
        }
    }

    [Fact]
    public async Task TransactionRowsCarryBlockCoordinatesAndAssetLists()
    {
        using var handler = new RecordingHandler(_ => Page("""
            {"items":[
              {"entrypoint_hash":"ab01","block_height":1500,"block_index":3,"block_hash":"cd02",
               "authority":"sorau1","timestamp_ms":1700000000000,"entrypoint_kind":"External","result_ok":true,
               "asset_ids":["x##sorau1","y##sorau1"],"asset_definition_ids":["x","y"],"metadata":{"memo":"hi"}},
              {"entrypoint_hash":"ef03","block_height":1499,"block_index":0,"authority":null,"timestamp_ms":null,
               "asset_ids":[],"asset_definition_ids":null}
            ],"next_cursor":"h1"}
            """));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var page = await client.Transactions.GetPageAsync(
            new ListQuery { Filter = Filter.Field("asset_definition_ids").Eq("x") & Filter.Field("block_height").Gte(1200) },
            TestContext.Current.CancellationToken);

        Assert.Equal(2, page.Items.Length);
        var first = page.Items[0];
        Assert.Equal("ab01", first.EntrypointHash);
        Assert.Equal(1500UL, first.BlockHeight);
        Assert.Equal(3UL, first.BlockIndex);
        Assert.Equal("cd02", first.BlockHash);
        Assert.Equal("sorau1", first.Authority);
        Assert.Equal(1700000000000UL, first.TimestampMs);
        Assert.Equal("External", first.EntrypointKind);
        Assert.True(first.ResultOk);
        Assert.Equal(["x##sorau1", "y##sorau1"], first.AssetIds);
        Assert.Equal(["x", "y"], first.AssetDefinitionIds);
        Assert.Equal("hi", first.Metadata!["memo"]!.GetValue<string>());
        var second = page.Items[1];
        Assert.Null(second.BlockHash);
        Assert.Null(second.Authority);
        Assert.Null(second.TimestampMs);
        Assert.Null(second.ResultOk);
        Assert.Empty(second.AssetIds);
        Assert.Empty(second.AssetDefinitionIds);
        Assert.Null(second.Metadata);
        Assert.Equal(
            """{"filter":{"op":"and","args":[{"op":"eq","args":["asset_definition_ids","x"]},{"op":"gte","args":["block_height",1200]}]}}""",
            Assert.Single(handler.Requests).Body);
    }

    [Fact]
    public async Task TransactionRowsRequireTheirIdentityFields()
    {
        foreach (var (row, field) in new[]
        {
            ("""{"block_height":1,"block_index":0}""", "entrypoint_hash"),
            ("""{"entrypoint_hash":"ab","block_index":0}""", "block_height"),
            ("""{"entrypoint_hash":"ab","block_height":1,"block_index":-1}""", "block_index"),
            ("""{"entrypoint_hash":"ab","block_height":1,"block_index":0,"asset_ids":"x"}""", "asset_ids"),
            ("""{"entrypoint_hash":"ab","block_height":1,"block_index":0,"asset_ids":[1]}""", "asset_ids[0]"),
        })
        {
            using var handler = new RecordingHandler(_ => Page($$"""{"items":[{{row}}],"next_cursor":null}"""));
            using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

            var error = await Assert.ThrowsAsync<JsonException>(() =>
                client.AccountTransactions(AccountId).GetPageAsync(cancellationToken: TestContext.Current.CancellationToken));

            Assert.Contains($"].{field} must be", error.Message, StringComparison.Ordinal);
        }
    }

    [Fact]
    public async Task HistoryCollectionsRejectFullHistoryScansBeforeDispatch()
    {
        using var handler = new RecordingHandler(_ => Page("""{"items":[],"next_cursor":null}"""));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var aggregate = new AggregateSpec { GroupBy = ["authority"], Metrics = [AggregateMetric.Count("n")] };

        var sort = await Assert.ThrowsAsync<ListQueryException>(() =>
            client.Transactions.GetPageAsync(new ListQuery { Sort = ["-block_height"] }, TestContext.Current.CancellationToken));
        var total = await Assert.ThrowsAsync<ListQueryException>(() =>
            client.AccountTransactions(AccountId).GetPageAsync(new ListQuery { IncludeTotal = true }, TestContext.Current.CancellationToken));
        var grouped = await Assert.ThrowsAsync<ListQueryException>(() =>
            client.Transactions.Rows.GetPageAsync(new ListQuery { Aggregate = aggregate }, TestContext.Current.CancellationToken));
        var rawSort = await Assert.ThrowsAsync<ListQueryException>(async () =>
        {
            await foreach (var _ in client.AccountTransactions(AccountId).Rows.EnumerateAsync(
                new ListQuery { Sort = ["timestamp_ms"] },
                TestContext.Current.CancellationToken))
            {
            }
        });

        Assert.Equal("invalid_sort", sort.Code);
        Assert.Equal("sort", sort.Parameter);
        Assert.Equal(
            "invalid `sort`: `transactions` rows are returned newest first and cannot be re-sorted; omit `sort` and filter on `block_height` or `timestamp_ms` to select a range",
            sort.Message);
        Assert.Equal("invalid_include_total", total.Code);
        Assert.Equal(
            "invalid `include_total`: totals are not available for `account_transactions`: counting would scan the whole history",
            total.Message);
        Assert.Equal("invalid_aggregate", grouped.Code);
        Assert.Equal(
            "invalid `aggregate`: aggregates are not available for `transactions`: they would scan the whole history",
            grouped.Message);
        Assert.Equal("invalid_sort", rawSort.Code);
        Assert.Contains("`account_transactions` rows", rawSort.Message, StringComparison.Ordinal);
        Assert.Empty(handler.Requests);

        await client.Transactions.Rows.GetPageAsync(
            new ListQuery { Select = ["entrypoint_hash", "block_height"] },
            TestContext.Current.CancellationToken);
        Assert.Equal("""{"select":["entrypoint_hash","block_height"]}""", Assert.Single(handler.Requests).Body);
    }

    [Fact]
    public async Task HistoryIterationFollowsCursorsAcrossShortAndEmptyPages()
    {
        var pages = new Queue<string>(
        [
            """{"items":[{"entrypoint_hash":"a","block_height":9,"block_index":1}],"next_cursor":"h1"}""",
            """{"items":[],"next_cursor":"h2"}""",
            """{"items":[],"next_cursor":"h3"}""",
            """{"items":[{"entrypoint_hash":"b","block_height":4,"block_index":0}],"next_cursor":null}""",
        ]);
        using var handler = new RecordingHandler(_ => Page(pages.Dequeue()));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var hashes = new List<string>();
        await foreach (var transaction in client.AccountTransactions(AccountId).EnumerateAsync(
            new ListQuery { FilterText = "result_ok = false", Limit = 50 },
            TestContext.Current.CancellationToken))
        {
            hashes.Add(transaction.EntrypointHash);
        }

        Assert.Equal(["a", "b"], hashes);
        Assert.Equal(
            [
                """{"filter":"result_ok = false","limit":50}""",
                """{"filter":"result_ok = false","limit":50,"cursor":"h1"}""",
                """{"filter":"result_ok = false","limit":50,"cursor":"h2"}""",
                """{"filter":"result_ok = false","limit":50,"cursor":"h3"}""",
            ],
            handler.Requests.Select(static request => request.Body));
    }

    [Fact]
    public async Task AssetDefinitionRowsCarryTheDefinitionRecordAndAliasBinding()
    {
        using var handler = new RecordingHandler(_ => Page("""
            {"items":[{"id":"7ZepsJTHCVLKsrFFNZGSRGZgvBhv","name":"Digital Shekel","alias":"ds#boi.is",
              "owned_by":"o","owning_domain":"boi.is","mintable":"Infinitely","description":null,
              "spec":{"scale":2},"balance_scope_policy":"Global",
              "alias_binding":{"alias":"ds#boi.is","status":"active","bound_at_ms":12},
              "metadata":{"tier":3}}],"next_cursor":null}
            """));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var definition = Assert.Single(
            (await client.AssetDefinitions.GetPageAsync(cancellationToken: TestContext.Current.CancellationToken)).Items);

        Assert.Equal("Digital Shekel", definition.Name);
        Assert.Equal("boi.is", definition.OwningDomain);
        Assert.Equal(2, definition.Spec!["scale"]!.GetValue<int>());
        Assert.Equal("Global", definition.BalanceScopePolicy!.GetValue<string>());
        Assert.Equal(12UL, definition.AliasBinding!.BoundAtMs);
        Assert.Null(definition.AliasBinding.LeaseExpiryMs);
        Assert.Equal(3, definition.Metadata!["tier"]!.GetValue<int>());
        Assert.Equal("""{}""", Assert.Single(handler.Requests).Body);
    }

    [Fact]
    public async Task EnumerateFollowsCursorsUntilTheLastPage()
    {
        var pages = new Queue<string>(
        [
            """{"items":[{"id":"a"},{"id":"b"}],"next_cursor":"c1"}""",
            """{"items":[{"id":"c"}],"next_cursor":"c2"}""",
            """{"items":[],"next_cursor":null}""",
        ]);
        using var handler = new RecordingHandler(_ => Page(pages.Dequeue()));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var query = new ListQuery { Sort = ["-id"], Limit = 2, IncludeTotal = true };

        var ids = new List<string>();
        await foreach (var domain in client.Domains.EnumerateAsync(query, TestContext.Current.CancellationToken))
        {
            ids.Add(domain.Id);
        }

        Assert.Equal(["a", "b", "c"], ids);
        Assert.Equal(
            [
                """{"sort":["-id"],"limit":2,"include_total":true}""",
                """{"sort":["-id"],"limit":2,"cursor":"c1","include_total":true}""",
                """{"sort":["-id"],"limit":2,"cursor":"c2","include_total":true}""",
            ],
            handler.Requests.Select(static request => request.Body));
    }

    [Fact]
    public async Task EnumeratePagesStopsWhenToriiRepeatsTheRequestCursor()
    {
        using var handler = new RecordingHandler(_ => Page("""{"items":[{"id":"a"}],"next_cursor":"same"}"""));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var error = await Assert.ThrowsAsync<IrohaException>(async () =>
        {
            await foreach (var _ in client.Domains.EnumeratePagesAsync(
                new ListQuery { Cursor = "same" },
                TestContext.Current.CancellationToken))
            {
            }
        });

        Assert.Equal("cursor_not_advancing", error.Code);
        Assert.Single(handler.Requests);
    }

    [Fact]
    public async Task EnumerationHonorsCancellation()
    {
        using var cancellation = new CancellationTokenSource();
        using var handler = new RecordingHandler(_ => Page("""{"items":[{"id":"a"},{"id":"b"}],"next_cursor":"c1"}"""));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        await Assert.ThrowsAnyAsync<OperationCanceledException>(async () =>
        {
            await foreach (var _ in client.Domains.EnumerateAsync(cancellationToken: cancellation.Token))
            {
                cancellation.Cancel();
            }
        });

        Assert.Single(handler.Requests);
    }

    [Fact]
    public async Task ProjectionsAndAggregatesUseRawRows()
    {
        using var handler = new RecordingHandler(_ => Page("""{"items":[{"asset":"x","holders":12}],"next_cursor":null}"""));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var aggregate = new ListQuery
        {
            Filter = Filter.Field("quantity").Gt(0),
            Aggregate = new AggregateSpec
            {
                GroupBy = ["asset"],
                Metrics = [AggregateMetric.Count("holders"), AggregateMetric.Sum("supply", "quantity")],
                Having = Filter.Parse("holders >= 10"),
            },
            Sort = ["-holders"],
            Limit = 20,
        };

        await Assert.ThrowsAsync<ArgumentException>(() =>
            client.AssetHolders("7ZepsJTHCVLKsrFFNZGSRGZgvBhv").GetPageAsync(aggregate, TestContext.Current.CancellationToken));
        await Assert.ThrowsAsync<ArgumentException>(() =>
            client.Domains.GetPageAsync(new ListQuery { Select = ["id"] }, TestContext.Current.CancellationToken));
        Assert.Empty(handler.Requests);

        var page = await client.AssetHolders("7ZepsJTHCVLKsrFFNZGSRGZgvBhv").Rows
            .GetPageAsync(aggregate, TestContext.Current.CancellationToken);

        Assert.Equal(12, Assert.Single(page.Items)["holders"]!.GetValue<int>());
        Assert.Equal(
            """{"filter":{"op":"gt","args":["quantity",0]},"sort":["-holders"],"aggregate":{"group_by":["asset"],"metrics":[{"alias":"holders","fn":"count"},{"alias":"supply","fn":"sum","field":"quantity"}],"having":{"op":"gte","args":["holders",10]}},"limit":20}""",
            Assert.Single(handler.Requests).Body);
    }

    [Fact]
    public async Task RawFilterTextIsSentUnchanged()
    {
        using var handler = new RecordingHandler(_ => Page("""{"items":[],"next_cursor":null}"""));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        await client.Accounts.GetPageAsync(
            new ListQuery { FilterText = "label = 'ops'   AND exists(metadata.tier)" },
            TestContext.Current.CancellationToken);

        Assert.Equal(
            """{"filter":"label = 'ops'   AND exists(metadata.tier)"}""",
            Assert.Single(handler.Requests).Body);
    }

    [Fact]
    public async Task InvalidQueriesAreRejectedBeforeDispatchWithTheToriiCode()
    {
        using var handler = new RecordingHandler(_ => Page("""{"items":[],"next_cursor":null}"""));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var limit = await Assert.ThrowsAsync<ListQueryException>(() =>
            client.Domains.GetPageAsync(new ListQuery { Limit = 0 }, TestContext.Current.CancellationToken));
        var cursor = await Assert.ThrowsAsync<ListQueryException>(() =>
            client.Domains.GetPageAsync(new ListQuery { Cursor = "has space" }, TestContext.Current.CancellationToken));
        var sort = await Assert.ThrowsAsync<ListQueryException>(() =>
            client.Domains.GetPageAsync(new ListQuery { Sort = ["id", "-id"] }, TestContext.Current.CancellationToken));
        var both = await Assert.ThrowsAsync<ListQueryException>(() =>
            client.Domains.GetPageAsync(
                new ListQuery { Filter = Filter.Field("id").Eq("x"), FilterText = "id = \"x\"" },
                TestContext.Current.CancellationToken));

        Assert.Equal("invalid_limit", limit.Code);
        Assert.Equal("invalid_cursor", cursor.Code);
        Assert.Equal("invalid_sort", sort.Code);
        Assert.Equal("invalid_query", both.Code);
        Assert.Empty(handler.Requests);
    }

    [Fact]
    public async Task ToriiErrorEnvelopesBecomeTypedExceptions()
    {
        using var handler = new RecordingHandler(_ =>
        {
            var response = new HttpResponseMessage(HttpStatusCode.BadRequest)
            {
                Content = new StringContent(
                    """{"code":"invalid_filter","message":"invalid `filter`: use the keyword `and` instead of `&` or `&&` (column 17)","details":{"field":"filter","hint":"write `and`","expected":"and","extra":[1]}}""",
                    Encoding.UTF8,
                    "application/json"),
            };
            response.Headers.TryAddWithoutValidation("x-iroha-reject-code", "invalid_filter");
            return response;
        });
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var error = await Assert.ThrowsAsync<ToriiApiException>(() =>
            client.Domains.GetPageAsync(new ListQuery { FilterText = "a = 1 && b = 2" }, TestContext.Current.CancellationToken));

        Assert.IsAssignableFrom<IrohaException>(error);
        Assert.Equal("invalid_filter", error.Code);
        Assert.Equal(HttpStatusCode.BadRequest, error.StatusCode);
        Assert.Equal("invalid `filter`: use the keyword `and` instead of `&` or `&&` (column 17)", error.ServerMessage);
        Assert.Equal("filter", error.Details!.Field);
        Assert.Equal("write `and`", error.Details.Hint);
        Assert.Equal("and", error.Details.Expected);
        Assert.Null(error.Details.Actual);
        Assert.Equal(1, error.Details.ToJsonObject()["extra"]![0]!.GetValue<int>());
        Assert.Equal("invalid_filter", error.RejectCode);
        Assert.Contains("(invalid_filter)", error.Message, StringComparison.Ordinal);
    }

    [Fact]
    public void NonEnvelopeErrorsFallBackToTheHttpStatusCode()
    {
        var html = new ToriiApiException(HttpStatusCode.BadGateway, null, "<html>bad gateway</html>", "Bad Gateway");
        var empty = new ToriiApiException(HttpStatusCode.NotFound, null, null, null);
        var withDetailsCode = new ToriiApiException(
            HttpStatusCode.Forbidden,
            null,
            """{"code":"permission_denied","message":"no","details":{"reject_code":"AXT_DENIED"}}""",
            null);

        Assert.Equal("http_502", html.Code);
        Assert.Null(html.ServerMessage);
        Assert.Null(html.Details);
        Assert.Contains("<html>bad gateway</html>", html.Message, StringComparison.Ordinal);
        Assert.Equal("http_404", empty.Code);
        Assert.Equal("permission_denied", withDetailsCode.Code);
        Assert.Equal("AXT_DENIED", withDetailsCode.RejectCode);
    }

    [Fact]
    public async Task SignedClientsSignCollectionReadsAndAnonymousClientsDoNot()
    {
        using var credentials = new CanonicalRequestCredentials(AccountId, PrivateKeySeed);
        using var handler = new RecordingHandler(_ => Page("""{"items":[],"next_cursor":null}"""));
        using var signed = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler),
            new ToriiClientOptions
            {
                NetworkId = NetworkId.Parse(NetworkIdLiteral),
                CanonicalRequestCredentials = credentials,
            },
            TransactionSubmissionTransportAssurance.OneShotWithoutRedirectsOrRetries);

        await signed.Nfts.GetPageAsync(cancellationToken: TestContext.Current.CancellationToken);

        var request = Assert.Single(handler.Requests);
        Assert.NotNull(request.Account);
    }

    [Fact]
    public async Task MalformedPagesAreProtocolErrors()
    {
        foreach (var body in new[]
        {
            """{"next_cursor":null}""",
            """{"items":{},"next_cursor":null}""",
            """{"items":[],"next_cursor":5}""",
            """{"items":[],"next_cursor":""}""",
            """{"items":[],"next_cursor":null,"total":-1}""",
            """{"items":[{"id":1}],"next_cursor":null}""",
            """{"items":[1],"next_cursor":null}""",
            """{"items":[],"items":[],"next_cursor":null}""",
        })
        {
            using var handler = new RecordingHandler(_ => Page(body));
            using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
            await Assert.ThrowsAsync<JsonException>(() =>
                client.Domains.GetPageAsync(cancellationToken: TestContext.Current.CancellationToken));
        }
    }

    [Fact]
    public void CollectionPathIdentifiersRejectWhitespaceAndEmptyValues()
    {
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(new RecordingHandler(_ => Page("{}"))));

        Assert.Throws<ArgumentException>(() => client.AccountAssets(""));
        Assert.Throws<ArgumentException>(() => client.AssetHolders(" x"));
        Assert.Throws<ArgumentException>(() => client.AccountTransactions("a\tb"));
    }

    private static HttpResponseMessage Page(string json) => new(HttpStatusCode.OK)
    {
        Content = new StringContent(json, Encoding.UTF8, "application/json"),
    };

    private sealed record RecordedRequest(
        HttpMethod Method,
        Uri Uri,
        string? Body,
        string? ContentType,
        string Accept,
        string? Account);

    private sealed class RecordingHandler(Func<HttpRequestMessage, HttpResponseMessage> responder) : HttpMessageHandler
    {
        public List<RecordedRequest> Requests { get; } = [];

        protected override async Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            var body = request.Content is null ? null : await request.Content.ReadAsStringAsync(cancellationToken);
            Requests.Add(new RecordedRequest(
                request.Method,
                request.RequestUri!,
                body,
                request.Content?.Headers.ContentType?.MediaType,
                string.Join(",", request.Headers.Accept.Select(static value => value.MediaType)),
                request.Headers.TryGetValues("X-Iroha-Account", out var account) ? account.Single() : null));
            var response = responder(request);
            response.RequestMessage ??= request;
            return response;
        }
    }
}
