using System.Collections.Immutable;
using System.Text;
using System.Text.Encodings.Web;
using System.Text.Json;

namespace Hyperledger.Iroha.Query;

/// <summary>A filter operator, named as in the JSON form.</summary>
public enum FilterOperator
{
    /// <summary><c>and</c>: every operand matches.</summary>
    And,

    /// <summary><c>or</c>: at least one operand matches.</summary>
    Or,

    /// <summary><c>not</c>: the operand does not match.</summary>
    Not,

    /// <summary><c>eq</c> (<c>field = value</c>).</summary>
    Eq,

    /// <summary><c>ne</c> (<c>field != value</c>); also matches rows where the field is absent.</summary>
    Ne,

    /// <summary><c>lt</c> (<c>field &lt; value</c>).</summary>
    Lt,

    /// <summary><c>lte</c> (<c>field &lt;= value</c>).</summary>
    Lte,

    /// <summary><c>gt</c> (<c>field &gt; value</c>).</summary>
    Gt,

    /// <summary><c>gte</c> (<c>field &gt;= value</c>).</summary>
    Gte,

    /// <summary><c>in</c> (<c>field in [a, b]</c>).</summary>
    In,

    /// <summary><c>nin</c> (<c>field not in [a, b]</c>); also matches rows where the field is absent.</summary>
    NotIn,

    /// <summary><c>exists</c> (<c>exists(field)</c>).</summary>
    Exists,

    /// <summary><c>is_null</c> (<c>field is null</c>): the field is absent or null.</summary>
    IsNull,
}

/// <summary>
/// A filter over the rows of a Torii collection or the events of a stream.
/// </summary>
/// <remarks>
/// <para>Build filters with <see cref="Field(string)"/> and combine them with <c>&amp;</c>,
/// <c>|</c> and <c>!</c>, parse the text form with <see cref="Parse(string)"/>, or decode the JSON
/// form with <see cref="FromJson(string)"/>. All three produce the same immutable tree.</para>
/// <para><see cref="ToString"/> renders the canonical text form (what Torii's own rendering
/// produces, e.g. <c>owned_by = "alice" and quantity >= "10.5"</c>) and <see cref="ToJson"/> the
/// canonical JSON form (<c>{"op":"eq","args":["owned_by","alice"]}</c>).</para>
/// <para>Object and array literals (<see cref="FilterLiteral.Json"/>, for <c>metadata.*</c> values)
/// exist only in the JSON form. Collection reads send filters as JSON, so they work there; a
/// <c>GET</c> query string (<see cref="ListQuery.ToQueryPairs"/>) or an event-stream filter rejects
/// them.</para>
/// </remarks>
public abstract class Filter : IEquatable<Filter>
{
    /// <summary>Maximum nesting depth of a filter (a single predicate has depth 0).</summary>
    public const int MaxDepth = 10;

    /// <summary>Maximum operator nodes in one filter.</summary>
    public const int MaxNodes = 1_024;

    /// <summary>Maximum literals in one <c>in</c> / <c>not in</c> list.</summary>
    public const int MaxListValues = 1_024;

    /// <summary>Maximum list literals across one filter.</summary>
    public const int MaxTotalListValues = 4_096;

    /// <summary>Maximum UTF-8 length of the text form.</summary>
    public const int MaxTextUtf8Length = 32 * 1024;

    internal static readonly JsonWriterOptions WriterOptions = new()
    {
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
        SkipValidation = false,
    };

    private protected Filter()
    {
    }

    /// <summary>The operator of the root node.</summary>
    public abstract FilterOperator Operator { get; }

    /// <summary>Starts a predicate or sort key on a dotted field path such as <c>metadata.tier</c>.</summary>
    public static FilterField Field(string path) => new(new FieldPath(path));

    /// <summary>Starts a predicate or sort key on a field path.</summary>
    public static FilterField Field(FieldPath path) => new(path);

    /// <summary>Parses the text form, e.g. <c>owned_by = "alice" and quantity >= 10</c>.</summary>
    /// <exception cref="FilterSyntaxException">The text is malformed; the error names the column.</exception>
    public static Filter Parse(string text) => FilterTextParser.ParseFilter(text, "filter");

    /// <summary>Decodes the JSON form, e.g. <c>{"op":"eq","args":["owned_by","alice"]}</c>.</summary>
    /// <exception cref="ListQueryException">The JSON is not a valid filter tree.</exception>
    public static Filter FromJson(string json)
    {
        ArgumentNullException.ThrowIfNull(json);
        JsonDocument document;
        try
        {
            document = JsonDocument.Parse(json, new JsonDocumentOptions { MaxDepth = 256 });
        }
        catch (JsonException exception)
        {
            throw new ListQueryException("filter", $"the JSON form is not valid JSON: {exception.Message}");
        }

        using (document)
        {
            return FromJson(document.RootElement);
        }
    }

    /// <summary>Decodes the JSON form from an element.</summary>
    /// <exception cref="ListQueryException">The JSON is not a valid filter tree.</exception>
    public static Filter FromJson(JsonElement element) =>
        FilterJsonReader.Read(element, "filter");

    /// <summary>The conjunction of <paramref name="filters"/>, or <see langword="null"/> when empty.</summary>
    public static Filter? All(IEnumerable<Filter> filters)
    {
        ArgumentNullException.ThrowIfNull(filters);
        Filter? result = null;
        foreach (var filter in filters)
        {
            result = result is null ? filter : result.And(filter);
        }

        return result;
    }

    /// <summary>The disjunction of <paramref name="filters"/>, or <see langword="null"/> when empty.</summary>
    public static Filter? Any(IEnumerable<Filter> filters)
    {
        ArgumentNullException.ThrowIfNull(filters);
        Filter? result = null;
        foreach (var filter in filters)
        {
            result = result is null ? filter : result.Or(filter);
        }

        return result;
    }

    /// <summary><c>this and other</c>, flattening chains of <c>and</c>.</summary>
    public Filter And(Filter other)
    {
        ArgumentNullException.ThrowIfNull(other);
        return new AndFilter(Flatten<AndFilter>(this, other, static node => node.Operands), validated: true);
    }

    /// <summary><c>this or other</c>, flattening chains of <c>or</c>.</summary>
    public Filter Or(Filter other)
    {
        ArgumentNullException.ThrowIfNull(other);
        return new OrFilter(Flatten<OrFilter>(this, other, static node => node.Operands), validated: true);
    }

    /// <summary><c>not this</c>.</summary>
    public Filter Negate() => new NotFilter(this);

    /// <summary><c>left and right</c>.</summary>
    public static Filter operator &(Filter left, Filter right)
    {
        ArgumentNullException.ThrowIfNull(left);
        return left.And(right);
    }

    /// <summary><c>left or right</c>.</summary>
    public static Filter operator |(Filter left, Filter right)
    {
        ArgumentNullException.ThrowIfNull(left);
        return left.Or(right);
    }

    /// <summary><c>not operand</c>.</summary>
    public static Filter operator !(Filter operand)
    {
        ArgumentNullException.ThrowIfNull(operand);
        return operand.Negate();
    }

    /// <summary>The canonical text form; <see cref="Parse(string)"/> reads it back to the same tree.</summary>
    /// <remarks>
    /// Object and array literals have no text spelling: they render as compact JSON for display, and
    /// that text does not parse. Send such filters in the JSON form.
    /// </remarks>
    public override string ToString()
    {
        var builder = new StringBuilder();
        AppendText(builder, TextParent.Root);
        return builder.ToString();
    }

    /// <summary>The canonical JSON form.</summary>
    public string ToJson()
    {
        var buffer = new System.Buffers.ArrayBufferWriter<byte>();
        using (var writer = new Utf8JsonWriter(buffer, WriterOptions))
        {
            WriteTo(writer);
        }

        return Encoding.UTF8.GetString(buffer.WrittenSpan);
    }

    /// <summary>Writes the canonical JSON form.</summary>
    public void WriteTo(Utf8JsonWriter writer)
    {
        ArgumentNullException.ThrowIfNull(writer);
        writer.WriteStartObject();
        writer.WriteString("op", OperatorName(Operator));
        writer.WriteStartArray("args");
        WriteArguments(writer);
        writer.WriteEndArray();
        writer.WriteEndObject();
    }

    /// <summary>
    /// The canonical text form for a transport that carries filters as text, rejecting object and
    /// array literals, which exist only in the JSON form.
    /// </summary>
    /// <param name="remedy">Completes the error message, e.g. <c>; send this filter in a POST body</c>.</param>
    internal string ToTransportText(string remedy)
    {
        if (HasStructuredLiteral(this))
        {
            throw new ListQueryException("filter", "object and array literals exist only in the JSON form" + remedy);
        }

        return ToString();
    }

    private static bool HasStructuredLiteral(Filter node) => node switch
    {
        AndFilter andNode => andNode.Operands.Any(HasStructuredLiteral),
        OrFilter orNode => orNode.Operands.Any(HasStructuredLiteral),
        NotFilter notNode => HasStructuredLiteral(notNode.Operand),
        ComparisonFilter comparison => comparison.Value.IsStructured,
        MembershipFilter membership => membership.Values.Any(static value => value.IsStructured),
        _ => false,
    };

    /// <summary>Checks the structural limits Torii enforces: depth, node count and list sizes.</summary>
    /// <exception cref="ListQueryException">The filter exceeds a limit or has an invalid operand.</exception>
    public void Validate()
    {
        var error = ValidationError();
        if (error is not null)
        {
            throw new ListQueryException("filter", error);
        }
    }

    /// <inheritdoc />
    public abstract bool Equals(Filter? other);

    /// <inheritdoc />
    public override bool Equals(object? obj) => obj is Filter other && Equals(other);

    /// <inheritdoc />
    public abstract override int GetHashCode();

    /// <summary>The JSON spelling of an operator (<c>nin</c>, <c>is_null</c>, ...).</summary>
    public static string OperatorName(FilterOperator op) => op switch
    {
        FilterOperator.And => "and",
        FilterOperator.Or => "or",
        FilterOperator.Not => "not",
        FilterOperator.Eq => "eq",
        FilterOperator.Ne => "ne",
        FilterOperator.Lt => "lt",
        FilterOperator.Lte => "lte",
        FilterOperator.Gt => "gt",
        FilterOperator.Gte => "gte",
        FilterOperator.In => "in",
        FilterOperator.NotIn => "nin",
        FilterOperator.Exists => "exists",
        FilterOperator.IsNull => "is_null",
        _ => throw new ArgumentOutOfRangeException(nameof(op), op, "Unknown filter operator."),
    };

    internal abstract void AppendText(StringBuilder builder, TextParent parent);

    internal abstract void WriteArguments(Utf8JsonWriter writer);

    /// <summary>Returns the first structural error of the whole tree, or <see langword="null"/>.</summary>
    internal string? ValidationError()
    {
        var budget = new Budget();
        return ValidateNode(this, 0, ref budget);
    }

    private static string? ValidateNode(Filter node, int depth, ref Budget budget)
    {
        if (depth > MaxDepth)
        {
            return LimitExceeded("nesting depth", MaxDepth);
        }

        budget.Nodes++;
        if (budget.Nodes > MaxNodes)
        {
            return LimitExceeded("node count", MaxNodes);
        }

        switch (node)
        {
            case AndFilter andNode:
                return ValidateOperands(andNode.Operands, "and", depth, ref budget);
            case OrFilter orNode:
                return ValidateOperands(orNode.Operands, "or", depth, ref budget);
            case NotFilter notNode:
                return ValidateNode(notNode.Operand, depth + 1, ref budget);
            case ComparisonFilter comparison:
                return ComparisonFilter.OperandError(comparison.Path, comparison.Operator, comparison.Value);
            case MembershipFilter membership:
                budget.ListValues += membership.Values.Length;
                return MembershipFilter.ValuesError(membership.Path, membership.Values, budget.ListValues);
            default:
                return null;
        }
    }

    private static string? ValidateOperands(ImmutableArray<Filter> operands, string op, int depth, ref Budget budget)
    {
        if (operands.IsDefaultOrEmpty)
        {
            return $"`{op}` needs at least one operand";
        }

        foreach (var operand in operands)
        {
            var error = ValidateNode(operand, depth + 1, ref budget);
            if (error is not null)
            {
                return error;
            }
        }

        return null;
    }

    internal static string LimitExceeded(string limit, int max) => $"filter exceeds the {limit} limit of {max}";

    internal static string InvalidOperand(FieldPath field, string reason) => $"invalid operand for `{field.Value}`: {reason}";

    private static ImmutableArray<Filter> Flatten<TNode>(
        Filter left,
        Filter right,
        Func<TNode, ImmutableArray<Filter>> operands)
        where TNode : Filter
    {
        var builder = ImmutableArray.CreateBuilder<Filter>();
        if (left is TNode leftNode)
        {
            builder.AddRange(operands(leftNode));
        }
        else
        {
            builder.Add(left);
        }

        if (right is TNode rightNode)
        {
            builder.AddRange(operands(rightNode));
        }
        else
        {
            builder.Add(right);
        }

        return builder.ToImmutable();
    }

    private struct Budget
    {
        internal int Nodes;
        internal int ListValues;
    }

    internal static ImmutableArray<Filter> RequireOperands(IEnumerable<Filter> operands, string op, string paramName)
    {
        ArgumentNullException.ThrowIfNull(operands, paramName);
        var array = operands.ToImmutableArray();
        if (array.IsEmpty)
        {
            throw new ListQueryException("filter", $"`{op}` needs at least one operand");
        }

        foreach (var operand in array)
        {
            if (operand is null)
            {
                throw new ArgumentException("Operands must not be null.", paramName);
            }
        }

        return array;
    }

    internal static void AppendLogical(StringBuilder builder, ImmutableArray<Filter> operands, string keyword, TextParent me, bool parenthesize)
    {
        if (parenthesize)
        {
            builder.Append('(');
        }

        for (var index = 0; index < operands.Length; index++)
        {
            if (index > 0)
            {
                builder.Append(' ').Append(keyword).Append(' ');
            }

            operands[index].AppendText(builder, me);
        }

        if (parenthesize)
        {
            builder.Append(')');
        }
    }

    internal static int SequenceHash<T>(ImmutableArray<T> items)
    {
        var hash = new HashCode();
        foreach (var item in items)
        {
            hash.Add(item);
        }

        return hash.ToHashCode();
    }
}

/// <summary>Where a node is rendered, deciding whether it needs parentheses.</summary>
internal enum TextParent
{
    Root,
    Or,
    And,
    Not,
}

/// <summary><c>a and b and ...</c>.</summary>
public sealed class AndFilter : Filter
{
    /// <summary>Creates a conjunction of at least one operand.</summary>
    public AndFilter(IEnumerable<Filter> operands)
    {
        Operands = RequireOperands(operands, "and", nameof(operands));
    }

    internal AndFilter(ImmutableArray<Filter> operands, bool validated)
    {
        _ = validated;
        Operands = operands;
    }

    /// <summary>The operands, in evaluation order.</summary>
    public ImmutableArray<Filter> Operands { get; }

    /// <inheritdoc />
    public override FilterOperator Operator => FilterOperator.And;

    /// <inheritdoc />
    public override bool Equals(Filter? other) =>
        other is AndFilter andNode && Operands.SequenceEqual(andNode.Operands);

    /// <inheritdoc />
    public override int GetHashCode() => HashCode.Combine(FilterOperator.And, SequenceHash(Operands));

    internal override void AppendText(StringBuilder builder, TextParent parent) =>
        AppendLogical(builder, Operands, "and", TextParent.And, parent is TextParent.And or TextParent.Not);

    internal override void WriteArguments(Utf8JsonWriter writer)
    {
        foreach (var operand in Operands)
        {
            operand.WriteTo(writer);
        }
    }
}

/// <summary><c>a or b or ...</c>.</summary>
public sealed class OrFilter : Filter
{
    /// <summary>Creates a disjunction of at least one operand.</summary>
    public OrFilter(IEnumerable<Filter> operands)
    {
        Operands = RequireOperands(operands, "or", nameof(operands));
    }

    internal OrFilter(ImmutableArray<Filter> operands, bool validated)
    {
        _ = validated;
        Operands = operands;
    }

    /// <summary>The operands, in evaluation order.</summary>
    public ImmutableArray<Filter> Operands { get; }

    /// <inheritdoc />
    public override FilterOperator Operator => FilterOperator.Or;

    /// <inheritdoc />
    public override bool Equals(Filter? other) =>
        other is OrFilter orNode && Operands.SequenceEqual(orNode.Operands);

    /// <inheritdoc />
    public override int GetHashCode() => HashCode.Combine(FilterOperator.Or, SequenceHash(Operands));

    internal override void AppendText(StringBuilder builder, TextParent parent) =>
        AppendLogical(builder, Operands, "or", TextParent.Or, parent != TextParent.Root);

    internal override void WriteArguments(Utf8JsonWriter writer)
    {
        foreach (var operand in Operands)
        {
            operand.WriteTo(writer);
        }
    }
}

/// <summary><c>not a</c>.</summary>
public sealed class NotFilter : Filter
{
    /// <summary>Negates <paramref name="operand"/>.</summary>
    public NotFilter(Filter operand)
    {
        Operand = operand ?? throw new ArgumentNullException(nameof(operand));
    }

    /// <summary>The negated filter.</summary>
    public Filter Operand { get; }

    /// <inheritdoc />
    public override FilterOperator Operator => FilterOperator.Not;

    /// <inheritdoc />
    public override bool Equals(Filter? other) => other is NotFilter notNode && Operand.Equals(notNode.Operand);

    /// <inheritdoc />
    public override int GetHashCode() => HashCode.Combine(FilterOperator.Not, Operand);

    internal override void AppendText(StringBuilder builder, TextParent parent)
    {
        if (Operand is IsNullFilter isNull)
        {
            CanonicalText.AppendFieldPath(builder, isNull.Path.Value);
            builder.Append(" is not null");
            return;
        }

        builder.Append("not ");
        Operand.AppendText(builder, TextParent.Not);
    }

    internal override void WriteArguments(Utf8JsonWriter writer) => Operand.WriteTo(writer);
}

/// <summary><c>field = value</c>, <c>!=</c>, <c>&lt;</c>, <c>&lt;=</c>, <c>&gt;</c> or <c>&gt;=</c>.</summary>
public sealed class ComparisonFilter : Filter
{
    private readonly FilterOperator op;

    /// <summary>Creates a comparison; <paramref name="op"/> must be a comparison operator.</summary>
    /// <exception cref="ListQueryException">The literal cannot be compared with this operator.</exception>
    public ComparisonFilter(FieldPath field, FilterOperator op, FilterLiteral value)
        : this(field, op, value, validated: false)
    {
        var error = OperandError(Path, op, value);
        if (error is not null)
        {
            throw new ListQueryException("filter", error);
        }
    }

    internal ComparisonFilter(FieldPath field, FilterOperator op, FilterLiteral value, bool validated)
    {
        _ = validated;
        Path = field ?? throw new ArgumentNullException(nameof(field));
        if (op is not (FilterOperator.Eq or FilterOperator.Ne or FilterOperator.Lt
            or FilterOperator.Lte or FilterOperator.Gt or FilterOperator.Gte))
        {
            throw new ArgumentOutOfRangeException(nameof(op), op, "Comparisons use eq, ne, lt, lte, gt or gte.");
        }

        this.op = op;
        Value = value;
    }

    /// <summary>The compared field.</summary>
    public FieldPath Path { get; }

    /// <summary>The literal on the right-hand side.</summary>
    public FilterLiteral Value { get; }

    /// <inheritdoc />
    public override FilterOperator Operator => op;

    /// <inheritdoc />
    public override bool Equals(Filter? other) =>
        other is ComparisonFilter comparison
        && op == comparison.op
        && Path.Equals(comparison.Path)
        && Value.Equals(comparison.Value);

    /// <inheritdoc />
    public override int GetHashCode() => HashCode.Combine(op, Path, Value);

    internal static string? OperandError(FieldPath field, FilterOperator op, FilterLiteral value)
    {
        if (value.InexactReason is { } inexact)
        {
            return InvalidOperand(field, inexact);
        }

        if (op is FilterOperator.Eq or FilterOperator.Ne)
        {
            return value.IsStructured && !field.Value.StartsWith("metadata.", StringComparison.Ordinal)
                ? InvalidOperand(field, "comparison literals must be strings, numbers, booleans or null")
                : null;
        }

        return value.IsNumeric || value.Kind == FilterLiteralKind.String
            ? null
            : InvalidOperand(field, "range comparisons need a number, decimal or string literal");
    }

    internal override void AppendText(StringBuilder builder, TextParent parent)
    {
        CanonicalText.AppendFieldPath(builder, Path.Value);
        builder.Append(op switch
        {
            FilterOperator.Eq => " = ",
            FilterOperator.Ne => " != ",
            FilterOperator.Lt => " < ",
            FilterOperator.Lte => " <= ",
            FilterOperator.Gt => " > ",
            _ => " >= ",
        });
        Value.AppendCanonical(builder);
    }

    internal override void WriteArguments(Utf8JsonWriter writer)
    {
        writer.WriteStringValue(Path.Value);
        Value.WriteTo(writer);
    }
}

/// <summary><c>field in [a, b]</c> or <c>field not in [a, b]</c>.</summary>
public sealed class MembershipFilter : Filter
{
    /// <summary>Creates a membership test against a non-empty list of unique literals.</summary>
    /// <exception cref="ListQueryException">The list is empty, too long, repeats a value or mixes types.</exception>
    public MembershipFilter(FieldPath field, IEnumerable<FilterLiteral> values, bool negated = false)
        : this(field, (values ?? throw new ArgumentNullException(nameof(values))).ToImmutableArray(), negated, validated: false)
    {
        var error = ValuesError(Path, Values, Values.Length);
        if (error is not null)
        {
            throw new ListQueryException("filter", error);
        }
    }

    internal MembershipFilter(FieldPath field, ImmutableArray<FilterLiteral> values, bool negated, bool validated)
    {
        _ = validated;
        Path = field ?? throw new ArgumentNullException(nameof(field));
        Values = values;
        IsNegated = negated;
    }

    /// <summary>The tested field.</summary>
    public FieldPath Path { get; }

    /// <summary>The candidate literals.</summary>
    public ImmutableArray<FilterLiteral> Values { get; }

    /// <summary>Whether this is <c>not in</c>.</summary>
    public bool IsNegated { get; }

    /// <inheritdoc />
    public override FilterOperator Operator => IsNegated ? FilterOperator.NotIn : FilterOperator.In;

    /// <inheritdoc />
    public override bool Equals(Filter? other) =>
        other is MembershipFilter membership
        && IsNegated == membership.IsNegated
        && Path.Equals(membership.Path)
        && Values.SequenceEqual(membership.Values);

    /// <inheritdoc />
    public override int GetHashCode() => HashCode.Combine(Operator, Path, SequenceHash(Values));

    internal static string? ValuesError(FieldPath field, ImmutableArray<FilterLiteral> values, int totalValues)
    {
        if (!values.IsDefault)
        {
            foreach (var value in values)
            {
                if (value.InexactReason is { } inexact)
                {
                    return InvalidOperand(field, inexact);
                }
            }
        }

        if (values.IsDefaultOrEmpty)
        {
            return InvalidOperand(field, "membership lists must not be empty");
        }

        if (values.Length > MaxListValues)
        {
            return LimitExceeded("membership list size", MaxListValues);
        }

        if (totalValues > MaxTotalListValues)
        {
            return LimitExceeded("total membership values", MaxTotalListValues);
        }

        var seen = new HashSet<FilterLiteral>();
        foreach (var value in values)
        {
            if (!seen.Add(value))
            {
                return InvalidOperand(field, "membership list values must be unique");
            }
        }

        var homogeneous = values.All(static value => value.Kind == FilterLiteralKind.String)
            || values.All(static value => value.IsNumeric)
            || values.All(static value => value.Kind == FilterLiteralKind.Boolean);
        return homogeneous || field.Value.StartsWith("metadata.", StringComparison.Ordinal)
            ? null
            : InvalidOperand(field, "membership list values must all be strings, numbers or booleans");
    }

    internal override void AppendText(StringBuilder builder, TextParent parent)
    {
        CanonicalText.AppendFieldPath(builder, Path.Value);
        builder.Append(IsNegated ? " not in [" : " in [");
        for (var index = 0; index < Values.Length; index++)
        {
            if (index > 0)
            {
                builder.Append(", ");
            }

            Values[index].AppendCanonical(builder);
        }

        builder.Append(']');
    }

    internal override void WriteArguments(Utf8JsonWriter writer)
    {
        writer.WriteStringValue(Path.Value);
        writer.WriteStartArray();
        foreach (var value in Values)
        {
            value.WriteTo(writer);
        }

        writer.WriteEndArray();
    }
}

/// <summary><c>exists(field)</c>: the field is present.</summary>
public sealed class ExistsFilter : Filter
{
    /// <summary>Tests whether <paramref name="field"/> is present.</summary>
    public ExistsFilter(FieldPath field)
    {
        Path = field ?? throw new ArgumentNullException(nameof(field));
    }

    /// <summary>The tested field.</summary>
    public FieldPath Path { get; }

    /// <inheritdoc />
    public override FilterOperator Operator => FilterOperator.Exists;

    /// <inheritdoc />
    public override bool Equals(Filter? other) => other is ExistsFilter exists && Path.Equals(exists.Path);

    /// <inheritdoc />
    public override int GetHashCode() => HashCode.Combine(FilterOperator.Exists, Path);

    internal override void AppendText(StringBuilder builder, TextParent parent)
    {
        builder.Append("exists(");
        CanonicalText.AppendFieldPath(builder, Path.Value);
        builder.Append(')');
    }

    internal override void WriteArguments(Utf8JsonWriter writer) => writer.WriteStringValue(Path.Value);
}

/// <summary><c>field is null</c>: the field is absent or null.</summary>
public sealed class IsNullFilter : Filter
{
    /// <summary>Tests whether <paramref name="field"/> is absent or null.</summary>
    public IsNullFilter(FieldPath field)
    {
        Path = field ?? throw new ArgumentNullException(nameof(field));
    }

    /// <summary>The tested field.</summary>
    public FieldPath Path { get; }

    /// <inheritdoc />
    public override FilterOperator Operator => FilterOperator.IsNull;

    /// <inheritdoc />
    public override bool Equals(Filter? other) => other is IsNullFilter isNull && Path.Equals(isNull.Path);

    /// <inheritdoc />
    public override int GetHashCode() => HashCode.Combine(FilterOperator.IsNull, Path);

    internal override void AppendText(StringBuilder builder, TextParent parent)
    {
        CanonicalText.AppendFieldPath(builder, Path.Value);
        builder.Append(" is null");
    }

    internal override void WriteArguments(Utf8JsonWriter writer) => writer.WriteStringValue(Path.Value);
}

/// <summary>A field awaiting an operator; see <see cref="Filter.Field(string)"/>. Reusable.</summary>
public sealed class FilterField
{
    /// <summary>Starts predicates on <paramref name="path"/>.</summary>
    public FilterField(FieldPath path)
    {
        Path = path ?? throw new ArgumentNullException(nameof(path));
    }

    /// <summary>The field path.</summary>
    public FieldPath Path { get; }

    /// <summary><c>field = value</c>.</summary>
    public ComparisonFilter Eq(FilterLiteral value) => new(Path, FilterOperator.Eq, value);

    /// <summary><c>field != value</c>; also matches rows where the field is absent.</summary>
    public ComparisonFilter Ne(FilterLiteral value) => new(Path, FilterOperator.Ne, value);

    /// <summary><c>field &lt; value</c>.</summary>
    public ComparisonFilter Lt(FilterLiteral value) => new(Path, FilterOperator.Lt, value);

    /// <summary><c>field &lt;= value</c>.</summary>
    public ComparisonFilter Lte(FilterLiteral value) => new(Path, FilterOperator.Lte, value);

    /// <summary><c>field &gt; value</c>.</summary>
    public ComparisonFilter Gt(FilterLiteral value) => new(Path, FilterOperator.Gt, value);

    /// <summary><c>field &gt;= value</c>.</summary>
    public ComparisonFilter Gte(FilterLiteral value) => new(Path, FilterOperator.Gte, value);

    /// <summary><c>field in [values...]</c>.</summary>
    public MembershipFilter In(params FilterLiteral[] values) => new(Path, values);

    /// <summary><c>field in [values...]</c>.</summary>
    public MembershipFilter In(IEnumerable<FilterLiteral> values) => new(Path, values);

    /// <summary><c>field not in [values...]</c>; also matches rows where the field is absent.</summary>
    public MembershipFilter NotIn(params FilterLiteral[] values) => new(Path, values, negated: true);

    /// <summary><c>field not in [values...]</c>; also matches rows where the field is absent.</summary>
    public MembershipFilter NotIn(IEnumerable<FilterLiteral> values) => new(Path, values, negated: true);

    /// <summary><c>exists(field)</c>.</summary>
    public ExistsFilter Exists() => new(Path);

    /// <summary><c>field is null</c> (absent or null).</summary>
    public IsNullFilter IsNull() => new(Path);

    /// <summary><c>field is not null</c> (present and not null).</summary>
    public NotFilter IsNotNull() => new(new IsNullFilter(Path));

    /// <summary>An ascending sort key on this field.</summary>
    public SortKey Ascending() => new(Path, SortOrder.Ascending);

    /// <summary>A descending sort key on this field.</summary>
    public SortKey Descending() => new(Path, SortOrder.Descending);

    /// <inheritdoc />
    public override string ToString() => Path.ToString();
}
