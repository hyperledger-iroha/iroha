using System.Collections.Immutable;
using System.Text.Json;

namespace Hyperledger.Iroha.Query;

/// <summary>An aggregate function computed per group.</summary>
public enum AggregateFunction
{
    /// <summary><c>count</c>: rows in the group (takes no field).</summary>
    Count,

    /// <summary><c>sum</c> of a numeric field.</summary>
    Sum,

    /// <summary><c>min</c> of a numeric field.</summary>
    Min,

    /// <summary><c>max</c> of a numeric field.</summary>
    Max,

    /// <summary><c>avg</c> of a numeric field.</summary>
    Avg,

    /// <summary><c>distinct_count</c> of a scalar field.</summary>
    DistinctCount,
}

/// <summary>One metric of an aggregate query, e.g. <c>{"alias": "supply", "fn": "sum", "field": "quantity"}</c>.</summary>
public sealed class AggregateMetric : IEquatable<AggregateMetric>
{
    /// <summary>Creates a metric; <paramref name="field"/> is omitted for <see cref="AggregateFunction.Count"/>.</summary>
    public AggregateMetric(string alias, AggregateFunction function, FieldPath? field = null)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(alias);
        if (!Enum.IsDefined(function))
        {
            throw new ArgumentOutOfRangeException(nameof(function), function, "Unknown aggregate function.");
        }

        Alias = alias;
        Function = function;
        Field = field;
    }

    /// <summary>The output column, also usable in <c>having</c> and <c>sort</c>.</summary>
    public string Alias { get; }

    /// <summary>The aggregate function.</summary>
    public AggregateFunction Function { get; }

    /// <summary>The consumed field, absent for <c>count</c>.</summary>
    public FieldPath? Field { get; }

    /// <summary><c>count</c> of the rows in each group.</summary>
    public static AggregateMetric Count(string alias) => new(alias, AggregateFunction.Count);

    /// <summary><c>sum</c> of <paramref name="field"/>.</summary>
    public static AggregateMetric Sum(string alias, FieldPath field) => new(alias, AggregateFunction.Sum, field);

    /// <summary><c>min</c> of <paramref name="field"/>.</summary>
    public static AggregateMetric Min(string alias, FieldPath field) => new(alias, AggregateFunction.Min, field);

    /// <summary><c>max</c> of <paramref name="field"/>.</summary>
    public static AggregateMetric Max(string alias, FieldPath field) => new(alias, AggregateFunction.Max, field);

    /// <summary><c>avg</c> of <paramref name="field"/>.</summary>
    public static AggregateMetric Avg(string alias, FieldPath field) => new(alias, AggregateFunction.Avg, field);

    /// <summary><c>distinct_count</c> of <paramref name="field"/>.</summary>
    public static AggregateMetric DistinctCount(string alias, FieldPath field) =>
        new(alias, AggregateFunction.DistinctCount, field);

    /// <summary>The JSON spelling of an aggregate function.</summary>
    public static string FunctionName(AggregateFunction function) => function switch
    {
        AggregateFunction.Count => "count",
        AggregateFunction.Sum => "sum",
        AggregateFunction.Min => "min",
        AggregateFunction.Max => "max",
        AggregateFunction.Avg => "avg",
        AggregateFunction.DistinctCount => "distinct_count",
        _ => throw new ArgumentOutOfRangeException(nameof(function), function, "Unknown aggregate function."),
    };

    /// <inheritdoc />
    public bool Equals(AggregateMetric? other) =>
        other is not null
        && string.Equals(Alias, other.Alias, StringComparison.Ordinal)
        && Function == other.Function
        && Equals(Field, other.Field);

    /// <inheritdoc />
    public override bool Equals(object? obj) => obj is AggregateMetric other && Equals(other);

    /// <inheritdoc />
    public override int GetHashCode() => HashCode.Combine(Alias, Function, Field);

    internal void WriteTo(Utf8JsonWriter writer)
    {
        writer.WriteStartObject();
        writer.WriteString("alias", Alias);
        writer.WriteString("fn", FunctionName(Function));
        if (Field is not null)
        {
            writer.WriteString("field", Field.Value);
        }

        writer.WriteEndObject();
    }
}

/// <summary>
/// Grouped metrics evaluated after filtering and before paging (<c>POST …/query</c> only).
/// </summary>
/// <remarks><c>having</c> filters grouped rows and may reference group fields and metric aliases.</remarks>
public sealed class AggregateSpec : IEquatable<AggregateSpec>
{
    /// <summary>Maximum number of <c>group_by</c> fields in one aggregate.</summary>
    public const int MaxGroupBy = 8;

    /// <summary>Maximum number of metrics in one aggregate.</summary>
    public const int MaxMetrics = 16;

    private readonly ImmutableArray<FieldPath> groupBy;
    private readonly ImmutableArray<AggregateMetric> metrics;

    /// <summary>Grouping dimensions; at most <see cref="MaxGroupBy"/>.</summary>
    public ImmutableArray<FieldPath> GroupBy
    {
        get => groupBy.IsDefault ? ImmutableArray<FieldPath>.Empty : groupBy;
        init => groupBy = value;
    }

    /// <summary>Metrics computed per group; at least one and at most <see cref="MaxMetrics"/>.</summary>
    public ImmutableArray<AggregateMetric> Metrics
    {
        get => metrics.IsDefault ? ImmutableArray<AggregateMetric>.Empty : metrics;
        init => metrics = value;
    }

    /// <summary>A filter over the aggregated rows.</summary>
    public Filter? Having { get; init; }

    /// <inheritdoc />
    public bool Equals(AggregateSpec? other) =>
        other is not null
        && GroupBy.SequenceEqual(other.GroupBy)
        && Metrics.SequenceEqual(other.Metrics)
        && Equals(Having, other.Having);

    /// <inheritdoc />
    public override bool Equals(object? obj) => obj is AggregateSpec other && Equals(other);

    /// <inheritdoc />
    public override int GetHashCode() =>
        HashCode.Combine(Filter.SequenceHash(GroupBy), Filter.SequenceHash(Metrics), Having);

    internal string? ValidationError()
    {
        if (Metrics.IsEmpty)
        {
            return "`metrics` must list at least one metric";
        }

        if (GroupBy.Length > MaxGroupBy)
        {
            return $"`group_by` lists at most {MaxGroupBy} fields";
        }

        if (Metrics.Length > MaxMetrics)
        {
            return $"`metrics` lists at most {MaxMetrics} metrics";
        }

        foreach (var metric in Metrics)
        {
            if (metric is null)
            {
                return "metrics must not be null";
            }
        }

        foreach (var field in GroupBy)
        {
            if (field is null)
            {
                return "`group_by` fields must not be null";
            }
        }

        var having = Having?.ValidationError();
        return having is null ? null : $"having: {having}";
    }

    internal void WriteTo(Utf8JsonWriter writer)
    {
        writer.WriteStartObject();
        if (!GroupBy.IsEmpty)
        {
            writer.WriteStartArray("group_by");
            foreach (var field in GroupBy)
            {
                writer.WriteStringValue(field.Value);
            }

            writer.WriteEndArray();
        }

        writer.WriteStartArray("metrics");
        foreach (var metric in Metrics)
        {
            metric.WriteTo(writer);
        }

        writer.WriteEndArray();
        if (Having is not null)
        {
            writer.WritePropertyName("having");
            Having.WriteTo(writer);
        }

        writer.WriteEndObject();
    }

    internal static AggregateSpec FromJson(JsonElement element)
    {
        if (element.ValueKind != JsonValueKind.Object)
        {
            throw Invalid("`aggregate` must be an object such as {\"metrics\": [{\"alias\": \"n\", \"fn\": \"count\"}]}");
        }

        var groupBy = ImmutableArray<FieldPath>.Empty;
        var metrics = ImmutableArray<AggregateMetric>.Empty;
        Filter? having = null;
        foreach (var member in element.EnumerateObject())
        {
            switch (member.Name)
            {
                case "group_by":
                    groupBy = member.Value.ValueKind == JsonValueKind.Null
                        ? ImmutableArray<FieldPath>.Empty
                        : ReadFieldList(member.Value, "group_by");
                    break;
                case "metrics":
                    metrics = member.Value.ValueKind == JsonValueKind.Null
                        ? ImmutableArray<AggregateMetric>.Empty
                        : ReadMetrics(member.Value);
                    break;
                case "having":
                    if (member.Value.ValueKind != JsonValueKind.Null)
                    {
                        try
                        {
                            having = FilterJsonReader.Read(member.Value, "aggregate");
                        }
                        catch (ListQueryException exception)
                        {
                            throw Invalid($"having: {exception.Reason}");
                        }
                    }

                    break;
                default:
                    throw Invalid($"unknown member `{member.Name}`; expected one of: group_by, metrics, having");
            }
        }

        return new AggregateSpec { GroupBy = groupBy, Metrics = metrics, Having = having };
    }

    private static ImmutableArray<FieldPath> ReadFieldList(JsonElement value, string name)
    {
        if (value.ValueKind != JsonValueKind.Array)
        {
            throw Invalid($"`{name}` must be an array of field names");
        }

        var fields = ImmutableArray.CreateBuilder<FieldPath>();
        foreach (var item in value.EnumerateArray())
        {
            fields.Add(ReadField(item, name));
        }

        return fields.ToImmutable();
    }

    private static ImmutableArray<AggregateMetric> ReadMetrics(JsonElement value)
    {
        if (value.ValueKind != JsonValueKind.Array)
        {
            throw Invalid("`metrics` must be an array of {\"alias\", \"fn\", \"field\"} objects");
        }

        var metrics = ImmutableArray.CreateBuilder<AggregateMetric>();
        foreach (var item in value.EnumerateArray())
        {
            if (item.ValueKind != JsonValueKind.Object)
            {
                throw Invalid("each metric must be an object such as {\"alias\": \"n\", \"fn\": \"count\"}");
            }

            string? alias = null;
            AggregateFunction? function = null;
            FieldPath? field = null;
            foreach (var member in item.EnumerateObject())
            {
                switch (member.Name)
                {
                    case "alias" when member.Value.ValueKind == JsonValueKind.String:
                        alias = member.Value.GetString();
                        break;
                    case "fn" when member.Value.ValueKind == JsonValueKind.String:
                        function = member.Value.GetString() switch
                        {
                            "count" => AggregateFunction.Count,
                            "sum" => AggregateFunction.Sum,
                            "min" => AggregateFunction.Min,
                            "max" => AggregateFunction.Max,
                            "avg" => AggregateFunction.Avg,
                            "distinct_count" => AggregateFunction.DistinctCount,
                            var other => throw Invalid(
                                $"unknown aggregate function `{other}`; expected one of: count, sum, min, max, avg, distinct_count"),
                        };
                        break;
                    case "field" when member.Value.ValueKind == JsonValueKind.Null:
                        break;
                    case "field":
                        field = ReadField(member.Value, "field");
                        break;
                    default:
                        throw Invalid($"invalid metric member `{member.Name}`; a metric has `alias`, `fn` and `field`");
                }
            }

            if (string.IsNullOrWhiteSpace(alias) || function is null)
            {
                throw Invalid("each metric needs an `alias` and an `fn`");
            }

            metrics.Add(new AggregateMetric(alias, function.Value, field));
        }

        return metrics.ToImmutable();
    }

    private static FieldPath ReadField(JsonElement item, string name)
    {
        if (item.ValueKind != JsonValueKind.String)
        {
            throw Invalid($"`{name}` entries must be field names");
        }

        var text = item.GetString()!;
        var reason = FieldPath.ValidationError(text);
        return reason is null ? new FieldPath(text) : throw Invalid($"invalid field `{text}`: {reason}");
    }

    private static ListQueryException Invalid(string reason) => new("aggregate", reason);
}
