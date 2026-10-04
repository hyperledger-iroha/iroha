using System.Collections.Immutable;
using System.Text;

namespace Hyperledger.Iroha.Query;

/// <summary>Sort direction.</summary>
public enum SortOrder
{
    /// <summary>Smallest values first.</summary>
    Ascending,

    /// <summary>Largest values first.</summary>
    Descending,
}

/// <summary>One sort key: <c>field</c> sorts ascending and <c>-field</c> sorts descending.</summary>
public sealed class SortKey : IEquatable<SortKey>
{
    /// <summary>Maximum number of keys in one sort specification.</summary>
    public const int MaxKeys = 8;

    /// <summary>Creates a sort key.</summary>
    public SortKey(FieldPath field, SortOrder order = SortOrder.Ascending)
    {
        Field = field ?? throw new ArgumentNullException(nameof(field));
        if (!Enum.IsDefined(order))
        {
            throw new ArgumentOutOfRangeException(nameof(order), order, "Unknown sort order.");
        }

        Order = order;
    }

    /// <summary>The sorted field.</summary>
    public FieldPath Field { get; }

    /// <summary>The direction.</summary>
    public SortOrder Order { get; }

    /// <summary>An ascending key.</summary>
    public static SortKey Ascending(string field) => new(new FieldPath(field), SortOrder.Ascending);

    /// <summary>A descending key.</summary>
    public static SortKey Descending(string field) => new(new FieldPath(field), SortOrder.Descending);

    /// <summary>Parses one key such as <c>-quantity</c> or <c>metadata.`ui-order`</c>.</summary>
    /// <exception cref="FilterSyntaxException">The text is not exactly one valid key.</exception>
    public static SortKey Parse(string text)
    {
        var keys = FilterTextParser.ParseSort(text);
        if (keys.Length != 1)
        {
            throw FilterTextParser.SyntaxError(
                "sort",
                text,
                0,
                "expected exactly one sort key; pass each key as its own array element");
        }

        return keys[0];
    }

    /// <summary>Parses a comma-separated specification such as <c>-quantity,id</c>.</summary>
    /// <exception cref="FilterSyntaxException">The specification is malformed, empty or repeats a key.</exception>
    public static ImmutableArray<SortKey> ParseList(string text) => FilterTextParser.ParseSort(text);

    /// <summary>Renders keys as a comma-separated specification such as <c>-quantity,id</c>.</summary>
    public static string Format(IEnumerable<SortKey> keys)
    {
        ArgumentNullException.ThrowIfNull(keys);
        var builder = new StringBuilder();
        foreach (var key in keys)
        {
            if (builder.Length > 0)
            {
                builder.Append(',');
            }

            key.AppendTo(builder);
        }

        return builder.ToString();
    }

    /// <summary>Parses one key such as <c>-quantity</c>.</summary>
    public static implicit operator SortKey(string text) => Parse(text);

    /// <summary>The canonical spelling: <c>-</c> for descending, backticks where needed.</summary>
    public override string ToString()
    {
        var builder = new StringBuilder();
        AppendTo(builder);
        return builder.ToString();
    }

    /// <inheritdoc />
    public bool Equals(SortKey? other) => other is not null && Order == other.Order && Field.Equals(other.Field);

    /// <inheritdoc />
    public override bool Equals(object? obj) => obj is SortKey other && Equals(other);

    /// <inheritdoc />
    public override int GetHashCode() => HashCode.Combine(Field, Order);

    private void AppendTo(StringBuilder builder)
    {
        if (Order == SortOrder.Descending)
        {
            builder.Append('-');
        }

        CanonicalText.AppendFieldPath(builder, Field.Value);
    }
}
