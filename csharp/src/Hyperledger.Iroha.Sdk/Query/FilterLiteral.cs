using System.Globalization;
using System.Numerics;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Numeric;

namespace Hyperledger.Iroha.Query;

/// <summary>The kind of a <see cref="FilterLiteral"/>.</summary>
public enum FilterLiteralKind
{
    /// <summary>The JSON literal <c>null</c>.</summary>
    Null,

    /// <summary><c>true</c> or <c>false</c>.</summary>
    Boolean,

    /// <summary>An integer that fits <c>u64</c> or <c>i64</c>, carried as a JSON number.</summary>
    Integer,

    /// <summary>
    /// A JSON number from the JSON form that is not a 64-bit integer: an exact integer up to
    /// <c>u128</c>, or a fractional or out-of-range number, which filters reject because it is not exact.
    /// </summary>
    Number,

    /// <summary>A string; decimals and integers wider than 64 bits are exact decimal strings.</summary>
    String,

    /// <summary>
    /// A JSON array or object, accepted only for <c>metadata.*</c> fields and only in the JSON form
    /// of a filter (<c>POST …/query</c> bodies); the text grammar has no structured literals.
    /// </summary>
    Json,
}

/// <summary>
/// The right-hand side of a filter predicate: a JSON scalar, or a structured JSON value for
/// <c>metadata.*</c> fields.
/// </summary>
/// <remarks>
/// Integers that fit <c>u64</c>/<c>i64</c> are JSON numbers. Decimals and wider integers are exact
/// decimal strings, matching the text grammar (<c>10.5</c> and <c>"10.5"</c> are the same literal).
/// There is deliberately no conversion from <see cref="double"/>: binary floating point cannot
/// carry exact amounts, and Torii rejects fractional JSON numbers, including those nested in
/// structured literals. <see langword="default"/> is <see cref="Null"/>.
/// </remarks>
public readonly struct FilterLiteral : IEquatable<FilterLiteral>
{
    /// <summary>Torii's reason for rejecting a fractional JSON number.</summary>
    internal const string FractionalNumberReason =
        "fractional JSON numbers are not exact; write decimals as strings such as \"1.5\"";

    /// <summary>The reason for rejecting a JSON integer that Torii cannot represent exactly.</summary>
    internal const string WideIntegerReason =
        "JSON integers must fit u128 or i64; write wider integers as decimal strings such as \"-9223372036854775809\"";

    private readonly string? text;

    private FilterLiteral(FilterLiteralKind kind, string? text, string? inexactReason = null)
    {
        Kind = kind;
        this.text = text;
        InexactReason = inexactReason;
    }

    /// <summary>The literal kind.</summary>
    public FilterLiteralKind Kind { get; }

    /// <summary>The JSON literal <c>null</c>.</summary>
    public static FilterLiteral Null => default;

    /// <summary>
    /// The string value for <see cref="FilterLiteralKind.String"/>, the number spelling for
    /// numbers, the compact JSON for structured values, and <c>true</c>/<c>false</c>/<c>null</c>
    /// otherwise.
    /// </summary>
    public string Text => Kind == FilterLiteralKind.Null ? "null" : text!;

    /// <summary>Whether a range comparison can use this literal numerically.</summary>
    public bool IsNumeric => Kind is FilterLiteralKind.Integer or FilterLiteralKind.Number
        || (Kind == FilterLiteralKind.String && IsDecimalText(text!));

    /// <summary>A string literal.</summary>
    public static FilterLiteral String(string value)
    {
        ArgumentNullException.ThrowIfNull(value);
        return new FilterLiteral(FilterLiteralKind.String, value);
    }

    /// <summary>A boolean literal.</summary>
    public static FilterLiteral Boolean(bool value) =>
        new(FilterLiteralKind.Boolean, value ? "true" : "false");

    /// <summary>An exact integer: a JSON number when it fits <c>u64</c>/<c>i64</c>, else a decimal string.</summary>
    public static FilterLiteral Integer(BigInteger value) =>
        value >= long.MinValue && value <= ulong.MaxValue
            ? new FilterLiteral(FilterLiteralKind.Integer, value.ToString(CultureInfo.InvariantCulture))
            : new FilterLiteral(FilterLiteralKind.String, value.ToString(CultureInfo.InvariantCulture));

    /// <summary>
    /// An exact decimal written as <c>-?(0|[1-9][0-9]*)(\.[0-9]+)?</c>. Integers that fit
    /// <c>u64</c>/<c>i64</c> become JSON numbers; every other value stays an exact decimal string.
    /// </summary>
    /// <exception cref="ArgumentException">The text is not a canonical decimal.</exception>
    public static FilterLiteral Decimal(string decimalText)
    {
        ArgumentNullException.ThrowIfNull(decimalText);
        if (!IsDecimalText(decimalText))
        {
            throw new ArgumentException(
                $"`{decimalText}` is not a decimal literal; write digits with an optional `-` and `.fraction`.",
                nameof(decimalText));
        }

        return FromNumberText(decimalText);
    }

    /// <summary>A structured JSON value (array or object), for <c>metadata.*</c> fields.</summary>
    /// <remarks>
    /// Scalars are converted to the matching scalar literal. Filters with structured literals exist
    /// only in the JSON form: send them with <c>POST …/query</c>; <c>GET</c> query strings and
    /// event-stream filters reject them. Numbers must be exact integers (decimals go in strings);
    /// filters reject fractional numbers anywhere in the value.
    /// </remarks>
    public static FilterLiteral Json(JsonNode? value)
    {
        if (value is null)
        {
            return Null;
        }

        using var document = JsonDocument.Parse(value.ToJsonString());
        return FromJsonElement(document.RootElement);
    }

    /// <inheritdoc cref="String(string)" />
    public static implicit operator FilterLiteral(string? value) =>
        value is null ? Null : String(value);

    /// <inheritdoc cref="Boolean(bool)" />
    public static implicit operator FilterLiteral(bool value) => Boolean(value);

    /// <summary>An integer literal.</summary>
    public static implicit operator FilterLiteral(int value) => Integer(value);

    /// <summary>An integer literal.</summary>
    public static implicit operator FilterLiteral(uint value) => Integer(value);

    /// <summary>An integer literal.</summary>
    public static implicit operator FilterLiteral(long value) => Integer(value);

    /// <summary>An integer literal.</summary>
    public static implicit operator FilterLiteral(ulong value) => Integer(value);

    /// <summary>An exact integer literal; values wider than 64 bits become decimal strings.</summary>
    public static implicit operator FilterLiteral(Int128 value) => Integer(value);

    /// <summary>An exact integer literal; values wider than 64 bits become decimal strings.</summary>
    public static implicit operator FilterLiteral(UInt128 value) => Integer(value);

    /// <inheritdoc cref="Integer(BigInteger)" />
    public static implicit operator FilterLiteral(BigInteger value) => Integer(value);

    /// <summary>An exact decimal literal (scale preserved as written, e.g. <c>10.50</c>).</summary>
    public static implicit operator FilterLiteral(decimal value) =>
        FromNumberText(value.ToString(CultureInfo.InvariantCulture));

    /// <summary>An exact decimal literal.</summary>
    public static implicit operator FilterLiteral(NumericV1.DecimalValue value)
    {
        ArgumentNullException.ThrowIfNull(value);
        return FromNumberText(value.ToString());
    }

    /// <summary>An exact quantity literal.</summary>
    public static implicit operator FilterLiteral(NumericV1.QuantityValue value)
    {
        ArgumentNullException.ThrowIfNull(value);
        return FromNumberText(value.ToString());
    }

    /// <summary>An exact integer literal.</summary>
    public static implicit operator FilterLiteral(NumericV1.IntValue value)
    {
        ArgumentNullException.ThrowIfNull(value);
        return Integer(value.Value);
    }

    /// <inheritdoc />
    public bool Equals(FilterLiteral other) =>
        Kind == other.Kind && string.Equals(text, other.text, StringComparison.Ordinal);

    /// <inheritdoc />
    public override bool Equals(object? obj) => obj is FilterLiteral other && Equals(other);

    /// <inheritdoc />
    public override int GetHashCode() => HashCode.Combine(Kind, text is null ? 0 : StringComparer.Ordinal.GetHashCode(text));

    /// <summary>Whether two literals are equal.</summary>
    public static bool operator ==(FilterLiteral left, FilterLiteral right) => left.Equals(right);

    /// <summary>Whether two literals differ.</summary>
    public static bool operator !=(FilterLiteral left, FilterLiteral right) => !left.Equals(right);

    /// <summary>The canonical text spelling: strings in double quotes with JSON escapes.</summary>
    public override string ToString()
    {
        var builder = new StringBuilder();
        AppendCanonical(builder);
        return builder.ToString();
    }

    internal bool IsStructured => Kind == FilterLiteralKind.Json;

    /// <summary>
    /// Why a JSON number in this literal (at any depth) is not exact, or <see langword="null"/> when
    /// every number is exact. Only literals decoded from JSON can be inexact.
    /// </summary>
    internal string? InexactReason { get; }

    internal void AppendCanonical(StringBuilder builder)
    {
        switch (Kind)
        {
            case FilterLiteralKind.String:
                CanonicalText.AppendJsonString(builder, text!);
                break;
            default:
                builder.Append(Text);
                break;
        }
    }

    internal void WriteTo(Utf8JsonWriter writer)
    {
        switch (Kind)
        {
            case FilterLiteralKind.Null:
                writer.WriteNullValue();
                break;
            case FilterLiteralKind.Boolean:
                writer.WriteBooleanValue(text == "true");
                break;
            case FilterLiteralKind.String:
                writer.WriteStringValue(text);
                break;
            default:
                writer.WriteRawValue(text!, skipInputValidation: true);
                break;
        }
    }

    /// <summary>Applies the text-grammar number rule to a canonical number spelling.</summary>
    internal static FilterLiteral FromNumberText(string number)
    {
        if (!number.Contains('.', StringComparison.Ordinal)
            && (ulong.TryParse(number, NumberStyles.None, CultureInfo.InvariantCulture, out _)
                || long.TryParse(number, NumberStyles.AllowLeadingSign, CultureInfo.InvariantCulture, out _)))
        {
            return new FilterLiteral(FilterLiteralKind.Integer, Canonicalize(number));
        }

        return new FilterLiteral(FilterLiteralKind.String, number);
    }

    /// <summary>Reads a literal from the JSON form.</summary>
    internal static FilterLiteral FromJsonElement(JsonElement element)
    {
        switch (element.ValueKind)
        {
            case JsonValueKind.Null:
                return Null;
            case JsonValueKind.True:
                return Boolean(true);
            case JsonValueKind.False:
                return Boolean(false);
            case JsonValueKind.String:
                return new FilterLiteral(FilterLiteralKind.String, element.GetString()!);
            case JsonValueKind.Number:
                var inexact = NumberInexactReason(element);
                if (inexact is null && element.TryGetUInt64(out var unsigned))
                {
                    return new FilterLiteral(FilterLiteralKind.Integer, unsigned.ToString(CultureInfo.InvariantCulture));
                }

                if (inexact is null && element.TryGetInt64(out var signed))
                {
                    return new FilterLiteral(FilterLiteralKind.Integer, signed.ToString(CultureInfo.InvariantCulture));
                }

                return new FilterLiteral(FilterLiteralKind.Number, element.GetRawText(), inexact);
            default:
                var builder = new StringBuilder();
                string? nestedInexact = null;
                AppendCanonicalJson(builder, element, ref nestedInexact);
                return new FilterLiteral(FilterLiteralKind.Json, builder.ToString(), nestedInexact);
        }
    }

    /// <summary>
    /// Torii's exactness rule for a JSON number: integers that fit <c>u64</c>, <c>i64</c> or
    /// <c>u128</c> are exact; fractions, exponents and <c>-0</c> are not.
    /// </summary>
    private static string? NumberInexactReason(JsonElement number)
    {
        var raw = number.GetRawText();
        if (raw.AsSpan().IndexOfAny('.', 'e', 'E') >= 0 || raw == "-0")
        {
            return FractionalNumberReason;
        }

        if (number.TryGetUInt64(out _) || number.TryGetInt64(out _))
        {
            return null;
        }

        return raw[0] != '-' && UInt128.TryParse(raw, NumberStyles.None, CultureInfo.InvariantCulture, out _)
            ? null
            : WideIntegerReason;
    }

    /// <summary>Whether <paramref name="text"/> is <c>-?(0|[1-9][0-9]*)(\.[0-9]+)?</c>.</summary>
    internal static bool IsDecimalText(string text)
    {
        var span = text.AsSpan();
        if (span.StartsWith("-"))
        {
            span = span[1..];
        }

        var dot = span.IndexOf('.');
        var integer = dot < 0 ? span : span[..dot];
        var integerOk = integer.SequenceEqual("0")
            || (integer.Length > 0 && integer[0] is >= '1' and <= '9' && AllDigits(integer[1..]));
        if (!integerOk)
        {
            return false;
        }

        if (dot < 0)
        {
            return true;
        }

        var fraction = span[(dot + 1)..];
        return fraction.Length > 0 && AllDigits(fraction);
    }

    private static bool AllDigits(ReadOnlySpan<char> digits)
    {
        foreach (var digit in digits)
        {
            if (digit is < '0' or > '9')
            {
                return false;
            }
        }

        return true;
    }

    private static string Canonicalize(string integer) =>
        BigInteger.Parse(integer, NumberStyles.AllowLeadingSign, CultureInfo.InvariantCulture)
            .ToString(CultureInfo.InvariantCulture);

    /// <summary>Compact JSON with object members sorted by name, as Torii renders them.</summary>
    /// <param name="builder">Receives the JSON.</param>
    /// <param name="element">The value to render.</param>
    /// <param name="inexact">Set to the first reason a nested number is not exact.</param>
    private static void AppendCanonicalJson(StringBuilder builder, JsonElement element, ref string? inexact)
    {
        switch (element.ValueKind)
        {
            case JsonValueKind.Object:
                builder.Append('{');
                var members = element.EnumerateObject()
                    .OrderBy(static property => property.Name, StringComparer.Ordinal)
                    .ToArray();
                for (var index = 0; index < members.Length; index++)
                {
                    if (index > 0)
                    {
                        builder.Append(',');
                    }

                    CanonicalText.AppendJsonString(builder, members[index].Name);
                    builder.Append(':');
                    AppendCanonicalJson(builder, members[index].Value, ref inexact);
                }

                builder.Append('}');
                break;
            case JsonValueKind.Array:
                builder.Append('[');
                var first = true;
                foreach (var item in element.EnumerateArray())
                {
                    if (!first)
                    {
                        builder.Append(',');
                    }

                    AppendCanonicalJson(builder, item, ref inexact);
                    first = false;
                }

                builder.Append(']');
                break;
            case JsonValueKind.String:
                CanonicalText.AppendJsonString(builder, element.GetString()!);
                break;
            case JsonValueKind.Number:
                inexact ??= NumberInexactReason(element);
                builder.Append(element.GetRawText());
                break;
            default:
                builder.Append(element.GetRawText());
                break;
        }
    }
}
