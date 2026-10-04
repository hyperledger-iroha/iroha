using System.Numerics;
using System.Text;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Numeric;
using Hyperledger.Iroha.Query;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Filter construction, rendering and local validation.</summary>
public sealed class FilterBuilderTests
{
    [Fact]
    public void BuilderMatchesTheParser()
    {
        var built = Filter.Field("owned_by").Eq("alice")
            & Filter.Field("quantity").Gte(10.5m)
            & (Filter.Field("status").In("A", "B") | Filter.Field("tier").Lt(-1))
            & !Filter.Field("metadata.frozen").Exists()
            & Filter.Field("note").IsNotNull();
        var parsed = Filter.Parse(
            """
            owned_by = "alice" and quantity >= 10.5 and (status in ["A", "B"] or tier < -1)
               and not exists(metadata.frozen) and note is not null
            """);

        Assert.Equal(parsed, built);
        Assert.Equal(
            """owned_by = "alice" and quantity >= "10.5" and (status in ["A", "B"] or tier < -1) and not exists(metadata.frozen) and note is not null""",
            built.ToString());
        built.Validate();
    }

    [Fact]
    public void LiteralsFollowTheTextRules()
    {
        Assert.Equal("a = 7", Filter.Field("a").Eq(7).ToString());
        Assert.Equal("a = -7", Filter.Field("a").Eq(-7L).ToString());
        Assert.Equal("a = 18446744073709551615", Filter.Field("a").Eq(ulong.MaxValue).ToString());
        Assert.Equal(
            "a = \"340282366920938463463374607431768211455\"",
            Filter.Field("a").Eq(UInt128.MaxValue).ToString());
        Assert.Equal("a = \"-9223372036854775809\"", Filter.Field("a").Eq(BigInteger.Parse("-9223372036854775809")).ToString());
        Assert.Equal("a = 25", Filter.Field("a").Eq(25m).ToString());
        Assert.Equal("a = \"10.50\"", Filter.Field("a").Eq(10.50m).ToString());
        Assert.Equal("a = \"10.5\"", Filter.Field("a").Eq(NumericV1.QuantityValue.Parse("10.50")).ToString());
        Assert.Equal("a = 3", Filter.Field("a").Eq(NumericV1.DecimalValue.Parse("3.000")).ToString());
        Assert.Equal("a = \"x\"", Filter.Field("a").Eq("x").ToString());
        Assert.Equal("a = null", Filter.Field("a").Eq(FilterLiteral.Null).ToString());
        Assert.Equal("a = true", Filter.Field("a").Eq(true).ToString());
        Assert.Equal("a = \"0.5\"", Filter.Field("a").Eq(FilterLiteral.Decimal("0.5")).ToString());
        Assert.Throws<ArgumentException>(() => FilterLiteral.Decimal("1e5"));
        Assert.Throws<ArgumentException>(() => FilterLiteral.Decimal("01"));

        Assert.Equal(FilterLiteralKind.Integer, ((FilterLiteral)7).Kind);
        Assert.Equal(FilterLiteralKind.String, ((FilterLiteral)10.5m).Kind);
        Assert.True(((FilterLiteral)10.5m).IsNumeric);
        Assert.False(((FilterLiteral)"abc").IsNumeric);
    }

    [Fact]
    public void JsonFormIsCanonical()
    {
        var filter = Filter.Field("owned_by").Eq("alice") & Filter.Field("tier").NotIn(1, 2) & Filter.Field("note").IsNull();

        Assert.Equal(
            """{"op":"and","args":[{"op":"eq","args":["owned_by","alice"]},{"op":"nin","args":["tier",[1,2]]},{"op":"is_null","args":["note"]}]}""",
            filter.ToJson());
        Assert.Equal(filter, Filter.FromJson(filter.ToJson()));
        Assert.Equal("""{"op":"eq","args":["text","é\n"]}""", Filter.Field("text").Eq("é\n").ToJson());
    }

    [Fact]
    public void TextRenderingQuotesPathsAndStrings()
    {
        Assert.Equal("metadata.`display-name` = \"x\"", Filter.Field("metadata.display-name").Eq("x").ToString());
        Assert.Equal("`and` = 1", Filter.Field("and").Eq(1).ToString());
        Assert.Equal("metadata.null = 1", Filter.Field("metadata.null").Eq(1).ToString());
        Assert.Equal("`9lives`.x = 1", Filter.Field("9lives.x").Eq(1).ToString());
        Assert.Equal(
            "text = \"quote \\\" backslash \\\\ tab \\t bell \\u0007 unicode é\"",
            Filter.Field("text").Eq("quote \" backslash \\ tab \t bell \u0007 unicode é").ToString());
    }

    [Fact]
    public void CompositionFlattensAndParenthesizesLikeTorii()
    {
        var a = Filter.Field("a").Eq(1);
        var b = Filter.Field("b").Eq(2);
        var c = Filter.Field("c").Eq(3);

        var and = Assert.IsType<AndFilter>((a & b) & c);
        Assert.Equal(3, and.Operands.Length);
        var or = Assert.IsType<OrFilter>(a | (b | c));
        Assert.Equal(3, or.Operands.Length);
        Assert.Equal("a = 1 or b = 2 and c = 3", (a | (b & c)).ToString());
        Assert.Equal("(a = 1 or b = 2) and c = 3", ((a | b) & c).ToString());
        Assert.Equal("not (a = 1 and b = 2)", (!(a & b)).ToString());
        Assert.Equal("not (a = 1 or b = 2)", (!(a | b)).ToString());
        Assert.Equal("not not a = 1", (!!a).ToString());
        Assert.Equal("a is not null", Filter.Field("a").IsNotNull().ToString());
        Assert.Equal("(a = 1 and b = 2) and c = 3", new AndFilter([new AndFilter([a, b]), c]).ToString());
        Assert.Null(Filter.All([]));
        Assert.Equal(and, Filter.All([a, b, c]));
        Assert.Equal(or, Filter.Any([a, b, c]));
    }

    [Fact]
    public void FieldsAreReusableAndProduceSortKeys()
    {
        var tier = Filter.Field("tier");

        Assert.Equal("tier > 1 and tier < 5", (tier.Gt(1) & tier.Lt(5)).ToString());
        Assert.Equal("-tier", tier.Descending().ToString());
        Assert.Equal("tier", tier.Ascending().ToString());
        Assert.Equal(SortKey.Parse("-metadata.`ui-order`"), SortKey.Descending("metadata.ui-order"));
        Assert.Equal("-quantity,id", SortKey.Format(["-quantity", "id"]));
    }

    [Fact]
    public void LocalValidationMatchesToriiRules()
    {
        AssertInvalid(() => Filter.Field("a").In(), "membership lists must not be empty");
        AssertInvalid(() => Filter.Field("a").In(1, 1), "membership list values must be unique");
        AssertInvalid(() => Filter.Field("a").In("x", 1), "must all be strings, numbers or booleans");
        AssertInvalid(() => Filter.Field("a").Lt(true), "range comparisons need a number, decimal or string literal");
        AssertInvalid(() => Filter.Field("a").Gte(FilterLiteral.Null), "range comparisons");
        AssertInvalid(
            () => Filter.Field("a").Eq(FilterLiteral.Json(JsonNode.Parse("[1]"))),
            "comparison literals must be strings, numbers, booleans or null");
        AssertInvalid(() => new AndFilter([]), "`and` needs at least one operand");

        Filter.Field("metadata.tags").Eq(FilterLiteral.Json(JsonNode.Parse("""{"b":1,"a":[2]}"""))).Validate();
        Assert.Equal(
            """metadata.tags = {"a":[2],"b":1}""",
            Filter.Field("metadata.tags").Eq(FilterLiteral.Json(JsonNode.Parse("""{"b":1,"a":[2]}"""))).ToString());
        Filter.Field("metadata.mixed").In("x", 1).Validate();
        Filter.Field("a").In("1.5", 2).Validate();

        Assert.Throws<ArgumentException>(() => Filter.Field("a b"));
        Assert.Throws<ArgumentException>(() => Filter.Field(""));
        Assert.Throws<ArgumentException>(() => Filter.Field("a..b"));
        Assert.Throws<ArgumentException>(() => Filter.Field(new string('x', 257)));
    }

    [Fact]
    public void FieldPathsMustNotContainBackticks()
    {
        const string Message = "invalid field `metadata.a`b`: field paths must not contain backticks";
        Assert.StartsWith(Message, Assert.Throws<ArgumentException>(() => new FieldPath("metadata.a`b")).Message, StringComparison.Ordinal);
        Assert.Throws<ArgumentException>(() => Filter.Field("metadata.a`b"));
        Assert.Throws<ArgumentException>(() => SortKey.Ascending("a`b"));
        foreach (var json in new[]
        {
            """{"op":"eq","args":["metadata.a`b",1]}""",
            """{"op":"in","args":["metadata.a`b",[1]]}""",
            """{"op":"exists","args":["metadata.a`b"]}""",
        })
        {
            var error = Assert.Throws<ListQueryException>(() => Filter.FromJson(json));
            Assert.Equal(Message, error.Reason);
            Assert.Equal("invalid_filter", error.Code);
        }

        var select = Assert.Throws<ListQueryException>(() => ListQuery.FromJson("""{"select":["a`b"]}"""));
        Assert.Equal("invalid_select", select.Code);
        // A backtick in the text form always opens or closes a quoted segment.
        Assert.Equal(Filter.Field("a-b.c").Eq(1), Filter.Parse("`a-b`.c = 1"));
    }

    [Fact]
    public void StringLiteralsKeepDelAndC1ButRejectC0Controls()
    {
        const string Raw = "x\u007fy\u0080\u0085\u009fz";
        var parsed = Filter.Parse($"a = \"{Raw}\"");
        Assert.Equal(Filter.Field("a").Eq(Raw), parsed);
        Assert.Equal($"a = \"{Raw}\"", parsed.ToString());
        Assert.Equal(parsed, Filter.Parse($"a = '{Raw}'"));
        foreach (var control in new[] { "\u0000", "\u0001", "\t", "\n", "\u001f" })
        {
            var error = Assert.Throws<FilterSyntaxException>(() => Filter.Parse($"a = \"x{control}y\""));
            Assert.Equal("control characters must be escaped inside string literals", error.SyntaxMessage);
            Assert.Equal(7, error.Column);
        }

        // Only `"`, `\` and U+0000..U+001F are escaped, with JSON's short forms.
        const string Value = "q\"b\\s\b\f\n\r\t\u0000\u001f\u007f\u0085/'";
        var rendered = Filter.Field("a").Eq(Value).ToString();
        Assert.Equal("a = \"q\\\"b\\\\s\\b\\f\\n\\r\\t\\u0000\\u001f\u007f\u0085/'\"", rendered);
        Assert.Equal(Filter.Field("a").Eq(Value), Filter.Parse(rendered));
    }

    [Fact]
    public void SingleOperandConnectivesDecodeToTheirOperand()
    {
        foreach (var op in new[] { "and", "or" })
        {
            var decoded = Filter.FromJson($$"""{"op":"{{op}}","args":[{"op":"eq","args":["a",1]}]}""");
            Assert.Equal(Filter.Field("a").Eq(1), decoded);
            Assert.Equal("a = 1", decoded.ToString());
        }

        var nested = Filter.FromJson(
            """{"op":"and","args":[{"op":"or","args":[{"op":"and","args":[{"op":"eq","args":["a",1]},{"op":"is_null","args":["b"]}]}]},{"op":"or","args":[{"op":"eq","args":["a",1]}]}]}""");
        Assert.Equal("(a = 1 and b is null) and a = 1", nested.ToString());
        var and = Assert.IsType<AndFilter>(nested);
        Assert.Equal(2, and.Operands.Length);
        Assert.IsType<AndFilter>(and.Operands[0]);

        // The collapsed connective still counts toward the depth limit.
        var deep = """{"op":"eq","args":["a",1]}""";
        for (var level = 0; level <= Filter.MaxDepth; level++)
        {
            deep = $$"""{"op":"or","args":[{{deep}}]}""";
        }

        var tooDeep = Assert.Throws<ListQueryException>(() => Filter.FromJson(deep));
        Assert.Equal("filter exceeds the nesting depth limit of 10", tooDeep.Reason);
    }

    [Fact]
    public void StructuralLimitsAreEnforced()
    {
        Filter deep = Filter.Field("a").Eq(1);
        for (var level = 0; level < Filter.MaxDepth; level++)
        {
            deep = !deep;
        }

        deep.Validate();
        var tooDeep = Assert.Throws<ListQueryException>(() => (!deep).Validate());
        Assert.Contains("nesting depth limit of 10", tooDeep.Message, StringComparison.Ordinal);
        Assert.Equal("invalid_filter", tooDeep.Code);

        var wide = new AndFilter(Enumerable.Range(0, Filter.MaxNodes).Select(index => (Filter)Filter.Field("a").Eq(index)));
        Assert.Contains("node count", Assert.Throws<ListQueryException>(wide.Validate).Message, StringComparison.Ordinal);

        var longList = Enumerable.Range(0, Filter.MaxListValues + 1).Select(static value => (FilterLiteral)value).ToArray();
        Assert.Contains(
            "membership list size",
            Assert.Throws<ListQueryException>(() => Filter.Field("a").In(longList)).Message,
            StringComparison.Ordinal);

        var many = Filter.All(Enumerable.Range(0, 5).Select(field =>
            (Filter)Filter.Field($"f{field}").In(Enumerable.Range(0, 1000).Select(static value => (FilterLiteral)value).ToArray())))!;
        Assert.Contains("total membership values", Assert.Throws<ListQueryException>(many.Validate).Message, StringComparison.Ordinal);

        var tooLong = Assert.Throws<FilterSyntaxException>(() => Filter.Parse("a = \"" + new string('é', 16_400) + "\""));
        Assert.Contains("must not exceed 32768 bytes", tooLong.SyntaxMessage, StringComparison.Ordinal);
    }

    [Fact]
    public void JsonDecodingNamesTheOffendingNode()
    {
        var unknown = Assert.Throws<ListQueryException>(() => Filter.FromJson(
            """{"op":"and","args":[{"op":"eq","args":["a",1]},{"op":"between","args":["b",1,2]}]}"""));
        Assert.Contains("unknown operator `between`", unknown.Reason, StringComparison.Ordinal);
        Assert.Contains("(at `args[1]`)", unknown.Reason, StringComparison.Ordinal);

        foreach (var (json, needle) in new[]
        {
            ("""{"op":"eq","args":["a",true],"extra":1}""", "unknown member `extra`"),
            ("""{"op":"and","args":[]}""", "at least one operand"),
            ("""{"op":"not","args":[]}""", "exactly one filter node"),
            ("""{"op":"in","args":["a",[]]}""", "must not be empty"),
            ("""{"op":"nin","args":["a",[true,true]]}""", "must be unique"),
            ("""{"op":"exists","args":"a"}""", "takes [\"field\"]"),
            ("""{"args":[]}""", "needs an `op`"),
            ("""["eq"]""", "must be an object"),
            ("""{"op":"eq","args":["a b",1]}""", "whitespace"),
            ("""{"op":"lt","args":["a",true]}""", "range comparisons"),
            ("""{"op":"eq","args":["a",[1]]}""", "comparison literals"),
            ("""{"op":"eq"}""", "takes [\"field\", value]"),
        })
        {
            var error = Assert.Throws<ListQueryException>(() => Filter.FromJson(json));
            Assert.Contains(needle, error.Reason, StringComparison.Ordinal);
            Assert.Equal("invalid_filter", error.Code);
        }

        Assert.Equal(Filter.Parse("a = 1"), Filter.FromJson("\"a = 1\""));
        Assert.Equal(Filter.Field("metadata.tags").Eq(FilterLiteral.Json(JsonNode.Parse("[\"a\"]"))),
            Filter.FromJson("""{"op":"eq","args":["metadata.tags",["a"]]}"""));
    }

    [Fact]
    public void FractionalJsonNumbersAreRejectedAtEveryDepth()
    {
        const string Fractional =
            "invalid operand for `{0}`: fractional JSON numbers are not exact; write decimals as strings such as \"1.5\"";
        foreach (var (json, field) in new (string Json, string? Field)[]
        {
            ("""{"op":"eq","args":["quantity",1.5]}""", "quantity"),
            ("""{"op":"gte","args":["quantity",1.0]}""", "quantity"),
            ("""{"op":"lt","args":["quantity",1e3]}""", "quantity"),
            ("""{"op":"ne","args":["a",-0]}""", "a"),
            ("""{"op":"in","args":["quantity",[1,2.5]]}""", "quantity"),
            ("""{"op":"eq","args":["metadata.limits",{"max":{"ratio":0.25}}]}""", "metadata.limits"),
            ("""{"op":"nin","args":["metadata.tags",[[1,[2.5]]]]}""", "metadata.tags"),
            ("""{"op":"eq","args":["a",[0.5]]}""", "a"),
            ("""{"op":"in","args":["a",[]]}""", null),
        })
        {
            var error = Assert.Throws<ListQueryException>(() => Filter.FromJson(json));
            Assert.Equal("invalid_filter", error.Code);
            if (field is null)
            {
                Assert.Contains("must not be empty", error.Reason, StringComparison.Ordinal);
                continue;
            }

            Assert.Equal(string.Format(System.Globalization.CultureInfo.InvariantCulture, Fractional, field), error.Reason);
        }

        var wide = Assert.Throws<ListQueryException>(() => Filter.FromJson("""{"op":"eq","args":["a",-9223372036854775809]}"""));
        Assert.Contains("JSON integers must fit u128 or i64", wide.Reason, StringComparison.Ordinal);
        Assert.Contains(
            "must fit u128",
            Assert.Throws<ListQueryException>(() => Filter.FromJson("""{"op":"eq","args":["a",340282366920938463463374607431768211456]}""")).Reason,
            StringComparison.Ordinal);

        var exact = Filter.FromJson(
            """{"op":"and","args":[{"op":"gte","args":["quantity",340282366920938463463374607431768211455]},{"op":"in","args":["tier",[-9223372036854775808,18446744073709551615]]},{"op":"eq","args":["metadata.limits",{"max":10,"min":"0.5"}]}]}""");
        exact.Validate();
        Assert.Equal(
            """{"op":"and","args":[{"op":"gte","args":["quantity",340282366920938463463374607431768211455]},{"op":"in","args":["tier",[-9223372036854775808,18446744073709551615]]},{"op":"eq","args":["metadata.limits",{"max":10,"min":"0.5"}]}]}""",
            exact.ToJson());

        AssertInvalid(
            () => Filter.Field("metadata.limits").Eq(FilterLiteral.Json(JsonNode.Parse("""{"ratio":0.25}"""))),
            "fractional JSON numbers are not exact");
        AssertInvalid(
            () => Filter.Field("quantity").In(FilterLiteral.Json(JsonValue.Create(1.5m)), 2),
            "fractional JSON numbers are not exact");
        Assert.Equal(Filter.Field("quantity").Eq("1.5"), Filter.Parse("quantity = 1.5"));
    }

    [Fact]
    public void StructuredLiteralsExistOnlyInTheJsonForm()
    {
        var filter = Filter.Field("metadata.tags").In(FilterLiteral.Json(JsonNode.Parse("""["a","b"]""")), "c")
            & Filter.Field("owned_by").Eq("alice");
        var query = new ListQuery { Filter = filter, Limit = 5 };

        Assert.Equal(
            """{"filter":{"op":"and","args":[{"op":"in","args":["metadata.tags",[["a","b"],"c"]]},{"op":"eq","args":["owned_by","alice"]}]},"limit":5}""",
            query.ToJson());
        Assert.Equal(query, ListQuery.FromJson(query.ToJson()));
        Assert.Equal("metadata.tags in [[\"a\",\"b\"], \"c\"] and owned_by = \"alice\"", filter.ToString());
        Assert.Throws<FilterSyntaxException>(() => Filter.Parse(filter.ToString()));

        var pairs = Assert.Throws<ListQueryException>(query.ToQueryPairs);
        Assert.Equal("invalid_filter", pairs.Code);
        Assert.Equal(
            "invalid `filter`: object and array literals exist only in the JSON form; send this filter in a POST …/query body instead of a GET query string",
            pairs.Message);
        Assert.Throws<ListQueryException>(query.ToQueryString);
        Assert.Equal(
            "filter=owned_by%20%3D%20%22alice%22&limit=5",
            (query with { Filter = Filter.Field("owned_by").Eq("alice") }).ToQueryString());
    }

    [Fact]
    public void TextParsingAcceptsTheDocumentedSpellings()
    {
        Assert.Equal(Filter.Field("a").Eq(1), Filter.Parse("a == 1"));
        Assert.Equal(Filter.Field("a").Ne(true), Filter.Parse("a <> true"));
        Assert.Equal(Filter.Field("a").Gt("x"), Filter.Parse("a > 'x'"));
        Assert.Equal(Filter.Field("a").Eq("it's"), Filter.Parse(@"a = 'it\'s'"));
        Assert.Equal(Filter.Field("a").Eq("q\"\\\né😀"), Filter.Parse("a = \"q\\\"\\\\\\n\\u00e9\\ud83d\\ude00\""));
        Assert.Equal(Filter.Field("status").In("a", "b"), Filter.Parse("status IN (\"a\", \"b\",)"));
        Assert.Equal(Filter.Field("x").Exists(), Filter.Parse("EXISTS(x)"));

        var surrogate = Assert.Throws<FilterSyntaxException>(() => Filter.Parse("a = \"\\ud83d\""));
        Assert.Contains("unpaired UTF-16 surrogate", surrogate.SyntaxMessage, StringComparison.Ordinal);
        var nesting = Assert.Throws<FilterSyntaxException>(() => Filter.Parse(new string('(', 100) + "a = 1" + new string(')', 100)));
        Assert.Contains("nests too deeply", nesting.SyntaxMessage, StringComparison.Ordinal);
        var multiline = Assert.Throws<FilterSyntaxException>(() => Filter.Parse("a = 1\nand b ~ 2"));
        Assert.EndsWith("(line 2, column 7)", multiline.Message, StringComparison.Ordinal);
        Assert.Equal(12, multiline.Position);
        var astral = Assert.Throws<FilterSyntaxException>(() => Filter.Parse("t = \"😀\" ~"));
        Assert.Equal(9, astral.Column);
    }

    [Fact]
    public void EqualityIsStructural()
    {
        var left = Filter.Parse("a = 1 and b in [\"x\", \"y\"]");
        var right = Filter.Field("a").Eq(1) & Filter.Field("b").In("x", "y");

        Assert.Equal(left, right);
        Assert.Equal(left.GetHashCode(), right.GetHashCode());
        Assert.NotEqual(left, Filter.Parse("a = 1 and b in [\"y\", \"x\"]"));
        Assert.NotEqual(Filter.Field("a").Eq(1), Filter.Field("a").Eq("1"));
        Assert.Contains(right, new HashSet<Filter> { left });
    }

    private static void AssertInvalid(Func<object> build, string needle)
    {
        var error = Assert.Throws<ListQueryException>(build);
        Assert.Contains(needle, error.Message, StringComparison.Ordinal);
        Assert.Equal("invalid_filter", error.Code);
    }
}
