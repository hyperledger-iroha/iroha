using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Query;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Checks the query core against the shared golden vectors (fixtures/torii/list_query).</summary>
public sealed class ListQueryVectorTests
{
    private static readonly Lazy<JsonObject> Vectors = new(() =>
        JsonNode.Parse(File.ReadAllText(Path.Combine(
            AppContext.BaseDirectory, "Fixtures", "torii", "list_query", "vectors.json")))!.AsObject());

    public static IEnumerable<object[]> FilterCases() => Cases("filters");

    public static IEnumerable<object[]> FilterErrorCases() => Cases("filter_errors");

    public static IEnumerable<object[]> JsonFilterCases() => Cases("json_filters");

    public static IEnumerable<object[]> SortCases() => Cases("sorts");

    public static IEnumerable<object[]> SortErrorCases() => Cases("sort_errors");

    public static IEnumerable<object[]> PageCases() => Cases("pages");

    public static IEnumerable<object[]> QueryBodyErrorCases() => Cases("query_body_errors");

    public static IEnumerable<object[]> QueryPairErrorCases() => Cases("query_pair_errors");

    [Fact]
    public void VectorsUseTheSupportedVersion()
    {
        Assert.Equal(1, Vectors.Value["version"]!.GetValue<int>());
        Assert.NotEmpty(Cases("filters"));
    }

    [Theory]
    [MemberData(nameof(FilterCases))]
    public void FiltersRenderCanonicalTextAndJsonFromTextAndJson(int index)
    {
        var vector = Case("filters", index);
        var text = vector["text"]!.GetValue<string>();
        var canonical = vector["canonical"]!.GetValue<string>();
        var json = vector["json"]!;

        var parsed = Filter.Parse(text);
        Assert.Equal(canonical, parsed.ToString());
        AssertJsonEqual(json, JsonNode.Parse(parsed.ToJson()));

        var decoded = Filter.FromJson(json.ToJsonString());
        Assert.Equal(parsed, decoded);
        Assert.Equal(parsed.GetHashCode(), decoded.GetHashCode());
        Assert.Equal(canonical, decoded.ToString());

        Assert.Equal(parsed, Filter.Parse(canonical));
    }

    [Theory]
    [MemberData(nameof(JsonFilterCases))]
    public void JsonFiltersDecodeToTheNormalizedTree(int index)
    {
        var vector = Case("json_filters", index);
        var json = vector["json"]!;
        var canonical = vector["canonical"]!.GetValue<string>();
        var normalized = vector["normalized"]!;

        var decoded = Filter.FromJson(json.ToJsonString());
        Assert.Equal(canonical, decoded.ToString());
        AssertJsonEqual(normalized, JsonNode.Parse(decoded.ToJson()));
        Assert.Equal(decoded, Filter.Parse(canonical));
        Assert.Equal(decoded, Filter.FromJson(normalized.ToJsonString()));

        var body = new JsonObject { ["filter"] = json.DeepClone() };
        AssertJsonEqual(
            new JsonObject { ["filter"] = normalized.DeepClone() },
            JsonNode.Parse(ListQuery.FromJson(body.ToJsonString()).ToJson()));
    }

    [Theory]
    [MemberData(nameof(FilterErrorCases))]
    public void FilterErrorsMatchTheReferenceMessageAndPosition(int index)
    {
        var vector = Case("filter_errors", index);
        var error = Assert.Throws<FilterSyntaxException>(() => Filter.Parse(vector["text"]!.GetValue<string>()));

        Assert.Equal(vector["message"]!.GetValue<string>(), error.SyntaxMessage);
        Assert.Equal(vector["line"]!.GetValue<int>(), error.Line);
        Assert.Equal(vector["column"]!.GetValue<int>(), error.Column);
        Assert.Equal("invalid_filter", error.Code);
        Assert.Equal("filter", error.Parameter);
    }

    [Theory]
    [MemberData(nameof(SortCases))]
    public void SortsRenderCanonicalTextAndJson(int index)
    {
        var vector = Case("sorts", index);
        var keys = SortKey.ParseList(vector["text"]!.GetValue<string>());

        Assert.Equal(vector["canonical"]!.GetValue<string>(), SortKey.Format(keys));
        Assert.Equal(
            vector["json"]!.AsArray().Select(static node => node!.GetValue<string>()),
            keys.Select(static key => key.ToString()));
        Assert.Equal(keys, vector["json"]!.AsArray().Select(static node => SortKey.Parse(node!.GetValue<string>())));
    }

    [Theory]
    [MemberData(nameof(SortErrorCases))]
    public void SortErrorsMatchTheReferenceMessageAndPosition(int index)
    {
        var vector = Case("sort_errors", index);
        var error = Assert.Throws<FilterSyntaxException>(() => SortKey.ParseList(vector["text"]!.GetValue<string>()));

        Assert.Equal(vector["message"]!.GetValue<string>(), error.SyntaxMessage);
        Assert.Equal(vector["line"]!.GetValue<int>(), error.Line);
        Assert.Equal(vector["column"]!.GetValue<int>(), error.Column);
        Assert.Equal("invalid_sort", error.Code);
    }

    [Fact]
    public void QueriesRenderTheReferenceBodiesAndQueryPairs()
    {
        ListQuery[] queries =
        [
            ListQuery.Empty,
            new ListQuery
            {
                Filter = Filter.Field("owned_by").Eq("alice") & Filter.Field("quantity").Gt(1),
                Sort = [SortKey.Descending("quantity"), SortKey.Ascending("id")],
                Select = ["id", "quantity"],
                Limit = 25,
                IncludeTotal = true,
            },
            new ListQuery { Limit = 10, Cursor = "q1_abc-DEF" },
        ];
        var vectors = Vectors.Value["queries"]!.AsArray();
        Assert.Equal(vectors.Count, queries.Length);

        for (var index = 0; index < queries.Length; index++)
        {
            var vector = vectors[index]!;
            var query = queries[index];
            AssertJsonEqual(vector["body"], JsonNode.Parse(query.ToJson()));
            var expectedPairs = vector["query_pairs"]!.AsArray()
                .Select(static pair => new KeyValuePair<string, string>(
                    pair![0]!.GetValue<string>(),
                    pair[1]!.GetValue<string>()))
                .ToArray();
            Assert.Equal(expectedPairs, query.ToQueryPairs());

            Assert.Equal(query, ListQuery.FromJson(vector["body"]!.ToJsonString()));
            Assert.Equal(query, ListQuery.FromQueryPairs(expectedPairs));
        }
    }

    [Fact]
    public void QueryBodyMembersUseTheCanonicalOrder()
    {
        var query = new ListQuery
        {
            IncludeTotal = true,
            Cursor = "c1",
            Limit = 5,
            Select = ["id"],
            Sort = ["-quantity"],
            Filter = Filter.Field("a").Eq(1),
        };

        Assert.Equal(
            """{"filter":{"op":"eq","args":["a",1]},"sort":["-quantity"],"select":["id"],"limit":5,"cursor":"c1","include_total":true}""",
            query.ToJson());
        Assert.Equal(
            "filter=a%20%3D%201&sort=-quantity&select=id&limit=5&cursor=c1&include_total=true",
            query.ToQueryString());
    }

    [Fact]
    public void AggregateBodiesRoundTripAndHaveNoUrlForm()
    {
        var query = new ListQuery
        {
            Filter = Filter.Parse("quantity > 0"),
            Aggregate = new AggregateSpec
            {
                GroupBy = ["asset"],
                Metrics = [AggregateMetric.Count("holders"), AggregateMetric.Sum("supply", "quantity")],
                Having = Filter.Parse("holders >= 10"),
            },
            Sort = ["-supply"],
            Limit = 20,
        };

        Assert.Equal(query, ListQuery.FromJson(query.ToJson()));
        var textHaving = ListQuery.FromJson(
            """{"aggregate":{"metrics":[{"alias":"n","fn":"count"}],"having":"n >= 2"}}""");
        Assert.Equal(Filter.Parse("n >= 2"), textHaving.Aggregate!.Having);
        Assert.Equal(
            "invalid_aggregate",
            Assert.Throws<ListQueryException>(() => ListQuery.FromJson(
                """{"aggregate":{"metrics":[{"alias":"n","fn":"median"}]}}""")).Code);
        Assert.Equal(
            "invalid_aggregate",
            Assert.Throws<ListQueryException>(() => ListQuery.FromJson("""{"aggregate":{"metrics":[]}}""")).Code);
        Assert.Equal("invalid_aggregate", Assert.Throws<ListQueryException>(() => query.ToQueryPairs()).Code);
    }

    [Fact]
    public void AggregatesAreBoundedAndTheirPathsValidated()
    {
        static AggregateSpec Spec(int groups, int metrics) => new()
        {
            GroupBy = [.. Enumerable.Range(0, groups).Select(static index => new FieldPath($"metadata.k{index}"))],
            Metrics = [.. Enumerable.Range(0, metrics).Select(static index => AggregateMetric.Count($"m{index}"))],
        };

        var widest = new ListQuery { Aggregate = Spec(AggregateSpec.MaxGroupBy, AggregateSpec.MaxMetrics) };
        widest.Validate();
        Assert.Equal(widest, ListQuery.FromJson(widest.ToJson()));

        var groups = Assert.Throws<ListQueryException>(
            () => new ListQuery { Aggregate = Spec(AggregateSpec.MaxGroupBy + 1, 1) }.Validate());
        Assert.Equal("`group_by` lists at most 8 fields", groups.Reason);
        Assert.Equal("invalid_aggregate", groups.Code);
        var metrics = Assert.Throws<ListQueryException>(
            () => new ListQuery { Aggregate = Spec(0, AggregateSpec.MaxMetrics + 1) }.Validate());
        Assert.Equal("`metrics` lists at most 16 metrics", metrics.Reason);
        Assert.Equal("aggregate", metrics.Parameter);

        var manyMetrics = "{\"aggregate\":{\"metrics\":["
            + string.Join(",", Enumerable.Range(0, 17).Select(static index => $$"""{"alias":"m{{index}}","fn":"count"}"""))
            + "]}}";
        foreach (var (body, needle) in new[]
        {
            ("""{"aggregate":{"groupby":["a"],"metrics":[{"alias":"n","fn":"count"}]}}""", "unknown member `groupby`"),
            ("""{"aggregate":{"metrics":[{"alias":"n","fn":"count","feild":"a"}]}}""", "invalid metric member `feild`"),
            ("""{"aggregate":{"group_by":["a","b","c","d","e","f","g","h","i"],"metrics":[{"alias":"n","fn":"count"}]}}""", "at most 8 fields"),
            (manyMetrics, "at most 16 metrics"),
            ("""{"aggregate":{"group_by":["a..b"],"metrics":[{"alias":"n","fn":"count"}]}}""", "segments must not be empty"),
            ("""{"aggregate":{"group_by":["a`b"],"metrics":[{"alias":"n","fn":"count"}]}}""", "must not contain backticks"),
            ("""{"aggregate":{"metrics":[{"alias":"s","fn":"sum","field":"a b"}]}}""", "whitespace or control characters"),
        })
        {
            var error = Assert.Throws<ListQueryException>(() => ListQuery.FromJson(body));
            Assert.Equal("invalid_aggregate", error.Code);
            Assert.Equal("aggregate", error.Parameter);
            Assert.Contains(needle, error.Reason, StringComparison.Ordinal);
        }
    }

    [Theory]
    [MemberData(nameof(PageCases))]
    public void PagesDecodeTheEnvelope(int index)
    {
        var vector = Case("pages", index);
        using var document = JsonDocument.Parse(vector["json"]!.ToJsonString());
        var page = PageReader.Read(document.RootElement, PageReader.JsonRows, "page");
        var json = vector["json"]!.AsObject();

        Assert.Equal(vector["has_more"]!.GetValue<bool>(), page.HasMore);
        Assert.Equal(json["items"]!.AsArray().Count, page.Items.Length);
        Assert.Equal(json["next_cursor"]?.GetValue<string>(), page.NextCursor);
        Assert.Equal(json["total"]?.GetValue<ulong>(), page.Total);
        for (var item = 0; item < page.Items.Length; item++)
        {
            AssertJsonEqual(json["items"]![item], page.Items[item]);
        }
    }

    [Theory]
    [MemberData(nameof(QueryBodyErrorCases))]
    public void QueryBodyErrorsExposeTheToriiCode(int index)
    {
        var vector = Case("query_body_errors", index);
        var error = Assert.ThrowsAny<ListQueryException>(() => ListQuery.FromJson(vector["body"]!.ToJsonString()));

        Assert.Equal(vector["code"]!.GetValue<string>(), error.Code);
        Assert.Equal(vector["parameter"]!.GetValue<string>(), error.Parameter);
    }

    [Theory]
    [MemberData(nameof(QueryPairErrorCases))]
    public void QueryPairErrorsExposeTheToriiCode(int index)
    {
        var vector = Case("query_pair_errors", index);
        var pairs = vector["query_pairs"]!.AsArray()
            .Select(static pair => new KeyValuePair<string, string>(
                pair![0]!.GetValue<string>(),
                pair[1]!.GetValue<string>()));
        var error = Assert.ThrowsAny<ListQueryException>(() => ListQuery.FromQueryPairs(pairs));

        Assert.Equal(vector["code"]!.GetValue<string>(), error.Code);
        Assert.Equal(vector["parameter"]!.GetValue<string>(), error.Parameter);
    }

    private static IEnumerable<object[]> Cases(string name) =>
        Enumerable.Range(0, Vectors.Value[name]!.AsArray().Count).Select(static index => new object[] { index });

    private static JsonObject Case(string name, int index) => Vectors.Value[name]!.AsArray()[index]!.AsObject();

    private static void AssertJsonEqual(JsonNode? expected, JsonNode? actual) =>
        Assert.True(
            JsonNode.DeepEquals(expected, actual),
            $"expected {expected?.ToJsonString()} but got {actual?.ToJsonString()}");
}
