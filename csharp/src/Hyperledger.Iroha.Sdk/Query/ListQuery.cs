using System.Buffers;
using System.Collections.Immutable;
using System.Globalization;
using System.Text;
using System.Text.Json;

namespace Hyperledger.Iroha.Query;

/// <summary>
/// Filter, order, projection and paging controls for one read of a Torii collection.
/// </summary>
/// <remarks>
/// <para>Every Torii collection accepts the same controls, on <c>GET /v1/&lt;collection&gt;</c> and
/// <c>POST /v1/&lt;collection&gt;/query</c>:</para>
/// <code>
/// var query = new ListQuery
/// {
///     Filter = Filter.Field("owned_by").Eq(alice) &amp; Filter.Field("quantity").Gte(10.5m),
///     Sort = ["-quantity", "id"],
///     Limit = 50,
/// };
/// </code>
/// <para>Continue with <c>query with { Cursor = page.NextCursor }</c>; the collection iterators do this
/// for you. Values are immutable and validated by <see cref="Validate"/> before they are sent.</para>
/// </remarks>
public sealed record ListQuery
{
    /// <summary>Maximum number of fields in one projection.</summary>
    public const int MaxSelectFields = 64;

    /// <summary>Maximum length of a pagination cursor.</summary>
    public const int MaxCursorLength = 4096;

    private static readonly string[] Members = ["filter", "sort", "select", "aggregate", "limit", "cursor", "include_total"];
    private static readonly string[] Parameters = ["filter", "sort", "select", "limit", "cursor", "include_total"];

    private readonly ImmutableArray<SortKey> sort;
    private readonly ImmutableArray<FieldPath> select;

    /// <summary>A query with the collection's default order, all fields and the server page size.</summary>
    public static ListQuery Empty { get; } = new();

    /// <summary>Rows to keep, as a filter tree. Mutually exclusive with <see cref="FilterText"/>.</summary>
    public Filter? Filter { get; init; }

    /// <summary>
    /// Rows to keep, as text sent to Torii unchanged (for example from a search box). Mutually
    /// exclusive with <see cref="Filter"/>; use <see cref="Query.Filter.Parse(string)"/> to check text locally.
    /// </summary>
    public string? FilterText { get; init; }

    /// <summary>Ordering; empty means the collection's default order. At most 8 keys.</summary>
    /// <remarks>
    /// Keys travel in their text spelling in both <c>GET</c> and <c>POST</c>, so a non-identifier
    /// segment is backtick-quoted: <c>"-metadata.`ui-order`"</c>. Transaction history collections
    /// cannot be re-sorted.
    /// </remarks>
    public ImmutableArray<SortKey> Sort
    {
        get => sort.IsDefault ? ImmutableArray<SortKey>.Empty : sort;
        init => sort = value;
    }

    /// <summary>Fields returned per item; empty means every field. At most 64 fields.</summary>
    /// <remarks>Fields travel as raw dotted paths (<c>metadata.ui-order</c>), without backticks.</remarks>
    public ImmutableArray<FieldPath> Select
    {
        get => select.IsDefault ? ImmutableArray<FieldPath>.Empty : select;
        init => select = value;
    }

    /// <summary>Grouped metrics instead of items; <c>POST …/query</c> only. Excludes <see cref="Select"/>.</summary>
    /// <remarks>
    /// Aggregates are computed where the rows live: Torii rejects a read whose visible rows span
    /// several dataspace routes with <c>invalid_aggregate</c> (page through the rows instead), and
    /// transaction history collections reject aggregates altogether.
    /// </remarks>
    public AggregateSpec? Aggregate { get; init; }

    /// <summary>Rows per page; the server default (usually 100) applies when unset.</summary>
    public int? Limit { get; init; }

    /// <summary>The previous page's <c>next_cursor</c>.</summary>
    public string? Cursor { get; init; }

    /// <summary>Whether to compute the exact number of matching rows (costs a full scan).</summary>
    /// <remarks>Transaction history collections reject totals.</remarks>
    public bool IncludeTotal { get; init; }

    /// <summary>Checks every control the way Torii does, without contacting a server.</summary>
    /// <exception cref="ListQueryException">A control is invalid; <see cref="IrohaException.Code"/> names it.</exception>
    public void Validate()
    {
        if (Filter is not null && FilterText is not null)
        {
            throw new ListQueryException("query", "set either `Filter` or `FilterText`, not both");
        }

        Filter?.Validate();
        if (FilterText is not null)
        {
            if (string.IsNullOrWhiteSpace(FilterText))
            {
                throw new ListQueryException("filter", "expected a filter expression");
            }

            if (Encoding.UTF8.GetByteCount(FilterText) > Query.Filter.MaxTextUtf8Length)
            {
                throw new ListQueryException("filter", $"filters must not exceed {Query.Filter.MaxTextUtf8Length} bytes");
            }
        }

        ValidateSort(Sort);
        ValidateSelect(Select);
        if (!Select.IsEmpty && Aggregate is not null)
        {
            throw new ListQueryException(
                "select",
                "`select` and `aggregate` cannot be combined; aggregates define their own columns");
        }

        var aggregate = Aggregate?.ValidationError();
        if (aggregate is not null)
        {
            throw new ListQueryException("aggregate", aggregate);
        }

        if (Limit is <= 0)
        {
            throw new ListQueryException("limit", "`limit` must be at least 1");
        }

        if (Cursor is not null && !IsCursor(Cursor))
        {
            throw new ListQueryException("cursor", "`cursor` must be a `next_cursor` value returned by a previous page");
        }
    }

    /// <summary>Writes the canonical <c>POST …/query</c> body.</summary>
    /// <remarks>Members appear in the order filter, sort, select, aggregate, limit, cursor, include_total; absent controls are omitted.</remarks>
    public void WriteTo(Utf8JsonWriter writer)
    {
        ArgumentNullException.ThrowIfNull(writer);
        writer.WriteStartObject();
        if (Filter is not null)
        {
            writer.WritePropertyName("filter");
            Filter.WriteTo(writer);
        }
        else if (FilterText is not null)
        {
            writer.WriteString("filter", FilterText);
        }

        if (!Sort.IsEmpty)
        {
            writer.WriteStartArray("sort");
            foreach (var key in Sort)
            {
                writer.WriteStringValue(key.ToString());
            }

            writer.WriteEndArray();
        }

        if (!Select.IsEmpty)
        {
            writer.WriteStartArray("select");
            foreach (var field in Select)
            {
                writer.WriteStringValue(field.Value);
            }

            writer.WriteEndArray();
        }

        if (Aggregate is not null)
        {
            writer.WritePropertyName("aggregate");
            Aggregate.WriteTo(writer);
        }

        if (Limit is { } limit)
        {
            writer.WriteNumber("limit", limit);
        }

        if (Cursor is not null)
        {
            writer.WriteString("cursor", Cursor);
        }

        if (IncludeTotal)
        {
            writer.WriteBoolean("include_total", true);
        }

        writer.WriteEndObject();
    }

    /// <summary>The canonical <c>POST …/query</c> body as UTF-8 bytes.</summary>
    public byte[] ToJsonUtf8Bytes()
    {
        var buffer = new ArrayBufferWriter<byte>(256);
        using (var writer = new Utf8JsonWriter(buffer, Query.Filter.WriterOptions))
        {
            WriteTo(writer);
        }

        return buffer.WrittenSpan.ToArray();
    }

    /// <summary>The canonical <c>POST …/query</c> body.</summary>
    public string ToJson() => Encoding.UTF8.GetString(ToJsonUtf8Bytes());

    /// <summary>
    /// The <c>GET</c> parameters in canonical order (not percent-encoded). Aggregates and filters with
    /// object or array literals have no URL form.
    /// </summary>
    /// <exception cref="ListQueryException">
    /// <see cref="Aggregate"/> is set, or <see cref="Filter"/> has an object or array literal.
    /// </exception>
    public IReadOnlyList<KeyValuePair<string, string>> ToQueryPairs()
    {
        if (Aggregate is not null)
        {
            throw new ListQueryException("aggregate", "aggregates are only available through POST /query");
        }

        var pairs = new List<KeyValuePair<string, string>>(6);
        if (Filter is not null)
        {
            pairs.Add(new("filter", Filter.ToTransportText("; send this filter in a POST …/query body instead of a GET query string")));
        }
        else if (FilterText is not null)
        {
            pairs.Add(new("filter", FilterText));
        }

        if (!Sort.IsEmpty)
        {
            pairs.Add(new("sort", SortKey.Format(Sort)));
        }

        if (!Select.IsEmpty)
        {
            pairs.Add(new("select", string.Join(',', Select.Select(static field => field.Value))));
        }

        if (Limit is { } limit)
        {
            pairs.Add(new("limit", limit.ToString(CultureInfo.InvariantCulture)));
        }

        if (Cursor is not null)
        {
            pairs.Add(new("cursor", Cursor));
        }

        if (IncludeTotal)
        {
            pairs.Add(new("include_total", "true"));
        }

        return pairs;
    }

    /// <summary>The percent-encoded <c>GET</c> query string, without a leading <c>?</c>.</summary>
    /// <exception cref="ListQueryException">The query has no URL form; see <see cref="ToQueryPairs"/>.</exception>
    public string ToQueryString() =>
        string.Join('&', ToQueryPairs().Select(static pair => $"{Uri.EscapeDataString(pair.Key)}={Uri.EscapeDataString(pair.Value)}"));

    /// <summary>Decodes a <c>POST …/query</c> body with Torii's checks.</summary>
    /// <exception cref="ListQueryException">The body has unknown members or invalid controls.</exception>
    public static ListQuery FromJson(string json)
    {
        ArgumentNullException.ThrowIfNull(json);
        JsonDocument document;
        try
        {
            document = JsonDocument.Parse(json, new JsonDocumentOptions { MaxDepth = 256 });
        }
        catch (JsonException exception)
        {
            throw new ListQueryException("query", $"the request body is not valid JSON: {exception.Message}");
        }

        using (document)
        {
            return FromJson(document.RootElement);
        }
    }

    /// <summary>Decodes a <c>POST …/query</c> body with Torii's checks.</summary>
    /// <exception cref="ListQueryException">The body has unknown members or invalid controls.</exception>
    public static ListQuery FromJson(JsonElement body)
    {
        if (body.ValueKind != JsonValueKind.Object)
        {
            throw new ListQueryException(
                "query",
                "the request body must be a JSON object such as {\"filter\": \"...\", \"limit\": 50}");
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);
        var query = Empty;
        foreach (var member in body.EnumerateObject())
        {
            var value = member.Value;
            if (Array.IndexOf(Members, member.Name) < 0)
            {
                throw new ListQueryException(
                    "query",
                    $"unknown member `{member.Name}`; expected one of: {string.Join(", ", Members)}");
            }

            if (!seen.Add(member.Name))
            {
                throw new ListQueryException(member.Name, $"`{member.Name}` must appear at most once");
            }

            if (value.ValueKind == JsonValueKind.Null)
            {
                continue;
            }

            query = member.Name switch
            {
                "filter" => query with { Filter = FilterJsonReader.Read(value, "filter") },
                "sort" => query with { Sort = SortFromJson(value) },
                "select" => query with { Select = SelectFromJson(value) },
                "aggregate" => query with { Aggregate = AggregateSpec.FromJson(value) },
                "limit" => query with { Limit = LimitFromJson(value) },
                "cursor" => query with
                {
                    Cursor = value.ValueKind == JsonValueKind.String
                        ? value.GetString()
                        : throw new ListQueryException("cursor", "`cursor` must be the string returned as `next_cursor`"),
                },
                _ => query with
                {
                    IncludeTotal = value.ValueKind switch
                    {
                        JsonValueKind.True => true,
                        JsonValueKind.False => false,
                        _ => throw new ListQueryException("include_total", "`include_total` must be true or false"),
                    },
                },
            };
        }

        query.Validate();
        return query;
    }

    /// <summary>Decodes percent-decoded <c>GET</c> parameters with Torii's checks.</summary>
    /// <exception cref="ListQueryException">A parameter is unknown, repeated or invalid.</exception>
    public static ListQuery FromQueryPairs(IEnumerable<KeyValuePair<string, string>> pairs)
    {
        ArgumentNullException.ThrowIfNull(pairs);
        var seen = new HashSet<string>(StringComparer.Ordinal);
        var query = Empty;
        foreach (var (key, value) in pairs)
        {
            if (Array.IndexOf(Parameters, key) < 0)
            {
                var hint = key == "aggregate" ? "; aggregates are only available through POST /query" : string.Empty;
                throw new ListQueryException(
                    "query",
                    $"unknown parameter `{key}`; expected one of: {string.Join(", ", Parameters)}{hint}");
            }

            if (!seen.Add(key))
            {
                throw new ListQueryException(key, $"`{key}` must appear at most once");
            }

            query = key switch
            {
                "filter" => query with { Filter = Query.Filter.Parse(value) },
                "sort" => query with { Sort = SortKey.ParseList(value) },
                "select" => query with
                {
                    Select = value.Split(',').Select(static field => SelectField(field.Trim())).ToImmutableArray(),
                },
                "limit" => query with
                {
                    Limit = ulong.TryParse(value, NumberStyles.None, CultureInfo.InvariantCulture, out var limit)
                        ? CheckedLimit(limit)
                        : throw new ListQueryException("limit", $"`limit` must be a positive integer, got `{value}`"),
                },
                "cursor" => query with { Cursor = value },
                _ => query with
                {
                    IncludeTotal = value switch
                    {
                        "true" => true,
                        "false" => false,
                        _ => throw new ListQueryException(
                            "include_total",
                            $"`include_total` must be `true` or `false`, got `{value}`"),
                    },
                },
            };
        }

        query.Validate();
        return query;
    }

    /// <inheritdoc />
    public bool Equals(ListQuery? other) =>
        other is not null
        && Equals(Filter, other.Filter)
        && string.Equals(FilterText, other.FilterText, StringComparison.Ordinal)
        && Sort.SequenceEqual(other.Sort)
        && Select.SequenceEqual(other.Select)
        && Equals(Aggregate, other.Aggregate)
        && Limit == other.Limit
        && string.Equals(Cursor, other.Cursor, StringComparison.Ordinal)
        && IncludeTotal == other.IncludeTotal;

    /// <inheritdoc />
    public override int GetHashCode()
    {
        var hash = new HashCode();
        hash.Add(Filter);
        hash.Add(FilterText, StringComparer.Ordinal);
        hash.Add(Query.Filter.SequenceHash(Sort));
        hash.Add(Query.Filter.SequenceHash(Select));
        hash.Add(Aggregate);
        hash.Add(Limit);
        hash.Add(Cursor, StringComparer.Ordinal);
        hash.Add(IncludeTotal);
        return hash.ToHashCode();
    }

    /// <summary>The canonical <c>POST …/query</c> body.</summary>
    public override string ToString() => ToJson();

    private static void ValidateSort(ImmutableArray<SortKey> keys)
    {
        if (keys.Length > SortKey.MaxKeys)
        {
            throw new ListQueryException("sort", $"at most {SortKey.MaxKeys} sort keys are allowed");
        }

        for (var index = 0; index < keys.Length; index++)
        {
            if (keys[index] is null)
            {
                throw new ListQueryException("sort", "sort keys must not be null");
            }

            for (var earlier = 0; earlier < index; earlier++)
            {
                if (keys[earlier].Field.Equals(keys[index].Field))
                {
                    throw new ListQueryException("sort", $"sort key `{keys[index].Field}` appears more than once");
                }
            }
        }
    }

    private static void ValidateSelect(ImmutableArray<FieldPath> fields)
    {
        if (fields.Length > MaxSelectFields)
        {
            throw new ListQueryException("select", $"at most {MaxSelectFields} fields can be selected");
        }

        for (var index = 0; index < fields.Length; index++)
        {
            if (fields[index] is null)
            {
                throw new ListQueryException("select", "selected fields must not be null");
            }

            for (var earlier = 0; earlier < index; earlier++)
            {
                if (fields[earlier].Equals(fields[index]))
                {
                    throw new ListQueryException("select", $"field `{fields[index]}` is selected more than once");
                }
            }
        }
    }

    private static bool IsCursor(string cursor)
    {
        if (cursor.Length == 0 || cursor.Length > MaxCursorLength)
        {
            return false;
        }

        foreach (var character in cursor)
        {
            if (!(char.IsAsciiLetterOrDigit(character) || character is '-' or '_'))
            {
                return false;
            }
        }

        return true;
    }

    private static ImmutableArray<SortKey> SortFromJson(JsonElement value)
    {
        if (value.ValueKind != JsonValueKind.Array)
        {
            throw new ListQueryException("sort", "`sort` must be an array of keys such as [\"-quantity\", \"id\"]");
        }

        var keys = ImmutableArray.CreateBuilder<SortKey>();
        foreach (var item in value.EnumerateArray())
        {
            if (item.ValueKind != JsonValueKind.String)
            {
                throw new ListQueryException("sort", "sort keys are strings such as \"-quantity\" or \"id\"");
            }

            keys.Add(SortKey.Parse(item.GetString()!));
        }

        return keys.ToImmutable();
    }

    private static ImmutableArray<FieldPath> SelectFromJson(JsonElement value)
    {
        if (value.ValueKind != JsonValueKind.Array)
        {
            throw new ListQueryException(
                "select",
                "`select` must be an array of field names such as [\"id\", \"quantity\"]");
        }

        if (value.GetArrayLength() == 0)
        {
            throw new ListQueryException("select", "`select` must list at least one field");
        }

        var fields = ImmutableArray.CreateBuilder<FieldPath>();
        foreach (var item in value.EnumerateArray())
        {
            if (item.ValueKind != JsonValueKind.String)
            {
                throw new ListQueryException("select", "`select` must be an array of field names");
            }

            fields.Add(SelectField(item.GetString()!));
        }

        return fields.ToImmutable();
    }

    private static FieldPath SelectField(string field)
    {
        var reason = FieldPath.ValidationError(field);
        return reason is null
            ? new FieldPath(field)
            : throw new ListQueryException("select", $"invalid field `{field}`: {reason}");
    }

    private static int LimitFromJson(JsonElement value) =>
        value.ValueKind == JsonValueKind.Number && value.TryGetUInt64(out var limit)
            ? CheckedLimit(limit)
            : throw new ListQueryException("limit", "`limit` must be a positive integer");

    private static int CheckedLimit(ulong limit) => limit switch
    {
        0 => throw new ListQueryException("limit", "`limit` must be at least 1"),
        > int.MaxValue => throw new ListQueryException("limit", "`limit` is too large"),
        _ => (int)limit,
    };
}
