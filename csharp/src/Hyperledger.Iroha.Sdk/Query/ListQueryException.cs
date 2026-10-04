namespace Hyperledger.Iroha.Query;

/// <summary>
/// A list-query control that Torii would reject, detected before any request is sent.
/// </summary>
/// <remarks>
/// <see cref="IrohaException.Code"/> is the code Torii returns for the same problem:
/// <c>invalid_filter</c>, <c>invalid_sort</c>, <c>invalid_select</c>, <c>invalid_aggregate</c>,
/// <c>invalid_limit</c>, <c>invalid_cursor</c>, <c>invalid_include_total</c> or
/// <c>invalid_query</c>.
/// </remarks>
public class ListQueryException : IrohaException
{
    /// <summary>Creates an error for one request control.</summary>
    /// <param name="parameter">
    /// The control at fault (<c>filter</c>, <c>sort</c>, <c>select</c>, <c>aggregate</c>,
    /// <c>limit</c>, <c>cursor</c> or <c>include_total</c>), or <c>query</c> for the request as a whole.
    /// </param>
    /// <param name="reason">What is wrong, including a fix where one is obvious.</param>
    public ListQueryException(string parameter, string reason)
        : base(CodeFor(parameter), $"invalid `{parameter}`: {reason}")
    {
        Parameter = parameter;
        Reason = reason;
    }

    /// <summary>The control at fault, or <c>query</c> for the request as a whole.</summary>
    public string Parameter { get; }

    /// <summary>The description without the <c>invalid `control`:</c> prefix.</summary>
    public string Reason { get; }

    /// <summary>The Torii error code for a rejected control.</summary>
    public static string CodeFor(string parameter) => parameter switch
    {
        "filter" => "invalid_filter",
        "sort" => "invalid_sort",
        "select" => "invalid_select",
        "aggregate" => "invalid_aggregate",
        "limit" => "invalid_limit",
        "cursor" => "invalid_cursor",
        "include_total" => "invalid_include_total",
        _ => "invalid_query",
    };
}

/// <summary>A syntax or structure error in the text form of a filter or sort specification.</summary>
public sealed class FilterSyntaxException : ListQueryException
{
    internal FilterSyntaxException(string parameter, string syntaxMessage, int line, int column, int position, bool multiline)
        : base(parameter, multiline
            ? $"{syntaxMessage} (line {line}, column {column})"
            : $"{syntaxMessage} (column {column})")
    {
        SyntaxMessage = syntaxMessage;
        Line = line;
        Column = column;
        Position = position;
    }

    /// <summary>The description without position information.</summary>
    public string SyntaxMessage { get; }

    /// <summary>The 1-based line of the offending token.</summary>
    public int Line { get; }

    /// <summary>The 1-based column (in Unicode scalar values) of the offending token.</summary>
    public int Column { get; }

    /// <summary>The 0-based UTF-16 index of the offending token in the input.</summary>
    public int Position { get; }
}
