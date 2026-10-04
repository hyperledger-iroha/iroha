using System.Text;

namespace Hyperledger.Iroha.Query;

/// <summary>A dot-separated field path such as <c>owned_by</c>, <c>quantity</c> or <c>metadata.tier</c>.</summary>
/// <remarks>
/// <see cref="Value"/> is the dotted spelling used in JSON (<c>metadata.display-name</c>);
/// <see cref="ToString"/> is the canonical text spelling, which wraps segments that are not
/// identifiers, and a first segment that is a keyword, in backticks (<c>metadata.`display-name`</c>).
/// Whether a collection exposes the field is decided by Torii.
/// </remarks>
public sealed class FieldPath : IEquatable<FieldPath>
{
    /// <summary>Maximum UTF-8 length of one field path.</summary>
    public const int MaxUtf8Length = 256;

    /// <summary>Creates a field path from its dotted spelling.</summary>
    /// <exception cref="ArgumentException">The path is empty, too long, contains whitespace,
    /// control characters or backticks, or has an empty segment.</exception>
    public FieldPath(string path)
    {
        var reason = ValidationError(path);
        if (reason is not null)
        {
            throw new ArgumentException($"invalid field `{path}`: {reason}", nameof(path));
        }

        Value = path;
    }

    /// <summary>The dotted spelling, without backticks.</summary>
    public string Value { get; }

    /// <summary>Converts a dotted spelling such as <c>metadata.tier</c> into a field path.</summary>
    public static implicit operator FieldPath(string path) => new(path);

    /// <summary>The canonical text spelling, with backticks where the grammar needs them.</summary>
    public override string ToString()
    {
        var builder = new StringBuilder(Value.Length + 4);
        CanonicalText.AppendFieldPath(builder, Value);
        return builder.ToString();
    }

    /// <inheritdoc />
    public bool Equals(FieldPath? other) => other is not null && string.Equals(Value, other.Value, StringComparison.Ordinal);

    /// <inheritdoc />
    public override bool Equals(object? obj) => obj is FieldPath other && Equals(other);

    /// <inheritdoc />
    public override int GetHashCode() => StringComparer.Ordinal.GetHashCode(Value);

    /// <summary>Returns why <paramref name="path"/> is not a valid field path, or <see langword="null"/>.</summary>
    internal static string? ValidationError(string? path)
    {
        if (string.IsNullOrEmpty(path))
        {
            return "field paths must not be empty";
        }

        if (Encoding.UTF8.GetByteCount(path) > MaxUtf8Length)
        {
            return "field paths must not exceed 256 bytes";
        }

        foreach (var character in path)
        {
            if (char.IsWhiteSpace(character) || char.IsControl(character))
            {
                return "field paths must not contain whitespace or control characters";
            }
        }

        if (path[0] == '.' || path[^1] == '.' || path.Contains("..", StringComparison.Ordinal))
        {
            return "field path segments must not be empty";
        }

        // The text form quotes segments with backticks and has no escape.
        if (path.Contains('`', StringComparison.Ordinal))
        {
            return "field paths must not contain backticks";
        }

        return null;
    }
}

/// <summary>Canonical text spelling shared by filters, sort keys and literals.</summary>
internal static class CanonicalText
{
    private static readonly string[] Keywords = ["and", "or", "not", "in", "is", "null", "true", "false", "exists"];
    private const string HexDigits = "0123456789abcdef";

    /// <summary>The lower-case keyword equal to <paramref name="word"/>, ignoring case.</summary>
    internal static string? Keyword(ReadOnlySpan<char> word)
    {
        foreach (var keyword in Keywords)
        {
            if (word.Equals(keyword, StringComparison.OrdinalIgnoreCase))
            {
                return keyword;
            }
        }

        return null;
    }

    internal static void AppendFieldPath(StringBuilder builder, string path)
    {
        var start = 0;
        var first = true;
        while (true)
        {
            var dot = path.IndexOf('.', start);
            var segment = dot < 0 ? path.AsSpan(start) : path.AsSpan(start, dot - start);
            if (!first)
            {
                builder.Append('.');
            }

            if (IsBareSegment(segment, first))
            {
                builder.Append(segment);
            }
            else
            {
                builder.Append('`').Append(segment).Append('`');
            }

            if (dot < 0)
            {
                return;
            }

            start = dot + 1;
            first = false;
        }
    }

    /// <summary>Appends a JSON string literal using the canonical Norito escaping rules.</summary>
    internal static void AppendJsonString(StringBuilder builder, string value)
    {
        builder.Append('"');
        foreach (var character in value)
        {
            switch (character)
            {
                case '"':
                    builder.Append("\\\"");
                    break;
                case '\\':
                    builder.Append("\\\\");
                    break;
                case '\n':
                    builder.Append("\\n");
                    break;
                case '\r':
                    builder.Append("\\r");
                    break;
                case '\t':
                    builder.Append("\\t");
                    break;
                case '\b':
                    builder.Append("\\b");
                    break;
                case '\f':
                    builder.Append("\\f");
                    break;
                default:
                    if (character < 0x20)
                    {
                        builder.Append("\\u00")
                            .Append(HexDigits[character >> 4])
                            .Append(HexDigits[character & 0xF]);
                    }
                    else
                    {
                        builder.Append(character);
                    }

                    break;
            }
        }

        builder.Append('"');
    }

    private static bool IsBareSegment(ReadOnlySpan<char> segment, bool first)
    {
        if (segment.IsEmpty || !(char.IsAsciiLetter(segment[0]) || segment[0] == '_'))
        {
            return false;
        }

        foreach (var character in segment[1..])
        {
            if (!(char.IsAsciiLetterOrDigit(character) || character == '_'))
            {
                return false;
            }
        }

        return !(first && Keyword(segment) is not null);
    }
}
