using System.Collections.Immutable;
using System.Globalization;
using System.Text;

namespace Hyperledger.Iroha.Query;

/// <summary>
/// The text grammar shared with Torii:
/// <code>
/// filter     := or
/// or         := and ("or" and)*
/// and        := unary ("and" unary)*
/// unary      := "not" unary | primary
/// primary    := "(" filter ")" | "exists" "(" path ")" | path predicate
/// predicate  := compare literal | ["not"] "in" list | "is" ["not"] "null"
/// compare    := "=" | "==" | "!=" | "&lt;&gt;" | "&lt;" | "&lt;=" | "&gt;" | "&gt;="
/// list       := "[" literal ("," literal)* [","] "]" | "(" literal ("," literal)* [","] ")"
/// literal    := string | number | "true" | "false" | "null"
/// path       := segment ("." segment)*
/// segment    := [A-Za-z_][A-Za-z0-9_]* | "`" any character except "`" "`"
/// sort       := key ("," key)*
/// key        := ["-"] path
/// </code>
/// Error messages and positions match Torii's parser exactly.
/// </summary>
internal static class FilterTextParser
{
    private const int MaxParseNesting = 64;

    internal static Filter ParseFilter(string text, string parameter)
    {
        ArgumentNullException.ThrowIfNull(text);
        if (Encoding.UTF8.GetByteCount(text) > Filter.MaxTextUtf8Length)
        {
            throw SyntaxError(
                parameter,
                text,
                CharIndexOfUtf8Offset(text, Filter.MaxTextUtf8Length),
                $"filters must not exceed {Filter.MaxTextUtf8Length} bytes");
        }

        if (string.IsNullOrWhiteSpace(text))
        {
            throw SyntaxError(parameter, text, 0, "expected a filter expression");
        }

        var parser = new Parser(text, Lexer.Tokenize(text, allowMinus: false, parameter), parameter);
        var filter = parser.ParseOr();
        parser.Finish();
        var structural = filter.ValidationError();
        if (structural is not null)
        {
            throw new FilterSyntaxException(parameter, structural, 1, 1, 0, multiline: false);
        }

        return filter;
    }

    internal static ImmutableArray<SortKey> ParseSort(string text)
    {
        ArgumentNullException.ThrowIfNull(text);
        if (string.IsNullOrWhiteSpace(text))
        {
            throw SyntaxError("sort", text, 0, "expected at least one sort key");
        }

        var parser = new Parser(text, Lexer.Tokenize(text, allowMinus: true, "sort"), "sort");
        var keys = ImmutableArray.CreateBuilder<SortKey>();
        while (true)
        {
            var token = parser.Peek();
            var order = SortOrder.Ascending;
            if (token.Kind == TokenKind.Minus)
            {
                parser.Advance();
                order = SortOrder.Descending;
            }

            var key = new SortKey(parser.ParsePath(), order);
            if (keys.Any(existing => existing.Field.Equals(key.Field)))
            {
                throw parser.ErrorAt(token, $"sort key `{key.Field}` appears more than once");
            }

            keys.Add(key);
            if (keys.Count > SortKey.MaxKeys)
            {
                throw parser.ErrorAt(token, $"sort specifications accept at most {SortKey.MaxKeys} keys");
            }

            var next = parser.Advance();
            switch (next.Kind)
            {
                case TokenKind.End:
                    return keys.ToImmutable();
                case TokenKind.Comma:
                    continue;
                case TokenKind.Word when string.Equals(next.Text, "asc", StringComparison.OrdinalIgnoreCase)
                    || string.Equals(next.Text, "desc", StringComparison.OrdinalIgnoreCase):
                    throw parser.ErrorAt(next, "write `field` for ascending and `-field` for descending order");
                default:
                    throw parser.ErrorAt(next, $"expected `,` between sort keys, found {Describe(next)}");
            }
        }
    }

    internal static FilterSyntaxException SyntaxError(string parameter, string input, int position, string message)
    {
        position = Math.Clamp(position, 0, input.Length);
        var lineStart = position;
        while (lineStart > 0 && input[lineStart - 1] != '\n')
        {
            lineStart--;
        }

        var line = 1;
        for (var index = 0; index < position; index++)
        {
            if (input[index] == '\n')
            {
                line++;
            }
        }

        var column = 1;
        for (var index = lineStart; index < position; index++)
        {
            if (!(char.IsLowSurrogate(input[index]) && index > lineStart && char.IsHighSurrogate(input[index - 1])))
            {
                column++;
            }
        }

        return new FilterSyntaxException(parameter, message, line, column, position, input.Contains('\n'));
    }

    private static int CharIndexOfUtf8Offset(string text, int utf8Offset)
    {
        var bytes = 0;
        for (var index = 0; index < text.Length; index++)
        {
            if (bytes >= utf8Offset)
            {
                return index;
            }

            var character = text[index];
            if (char.IsHighSurrogate(character) && index + 1 < text.Length && char.IsLowSurrogate(text[index + 1]))
            {
                bytes += 4;
                index++;
            }
            else
            {
                bytes += character < 0x80 ? 1 : character < 0x800 ? 2 : 3;
            }
        }

        return text.Length;
    }

    private static string Describe(Token token) => token.Kind switch
    {
        TokenKind.Word or TokenKind.Quoted => $"`{token.Text}`",
        TokenKind.Str => "a string literal",
        TokenKind.Number => $"the number `{token.Text}`",
        TokenKind.Compare => "a comparison operator",
        TokenKind.Minus => "`-`",
        TokenKind.LParen => "`(`",
        TokenKind.RParen => "`)`",
        TokenKind.LBracket => "`[`",
        TokenKind.RBracket => "`]`",
        TokenKind.Comma => "`,`",
        TokenKind.Dot => "`.`",
        _ => "the end of the input",
    };

    private static bool IsKeyword(Token token, string keyword) =>
        token.Kind == TokenKind.Word && string.Equals(token.Text, keyword, StringComparison.OrdinalIgnoreCase);

    private enum TokenKind
    {
        Word,
        Quoted,
        Str,
        Number,
        Compare,
        Minus,
        LParen,
        RParen,
        LBracket,
        RBracket,
        Comma,
        Dot,
        End,
    }

    private readonly record struct Token(TokenKind Kind, int Start, string? Text = null, FilterOperator Compare = FilterOperator.Eq);

    private sealed class Lexer
    {
        private readonly string input;
        private readonly bool allowMinus;
        private readonly string parameter;
        private int position;

        private Lexer(string input, bool allowMinus, string parameter)
        {
            this.input = input;
            this.allowMinus = allowMinus;
            this.parameter = parameter;
        }

        internal static List<Token> Tokenize(string input, bool allowMinus, string parameter)
        {
            var lexer = new Lexer(input, allowMinus, parameter);
            var tokens = new List<Token>();
            while (true)
            {
                var token = lexer.Next();
                tokens.Add(token);
                if (token.Kind == TokenKind.End)
                {
                    return tokens;
                }
            }
        }

        private char? PeekChar(int ahead) =>
            position + ahead < input.Length ? input[position + ahead] : null;

        private FilterSyntaxException Error(int at, string message) => SyntaxError(parameter, input, at, message);

        private Token Next()
        {
            while (PeekChar(0) is ' ' or '\t' or '\r' or '\n')
            {
                position++;
            }

            var start = position;
            if (PeekChar(0) is not { } character)
            {
                return new Token(TokenKind.End, start);
            }

            switch (character)
            {
                case '(':
                    return Single(TokenKind.LParen, start);
                case ')':
                    return Single(TokenKind.RParen, start);
                case '[':
                    return Single(TokenKind.LBracket, start);
                case ']':
                    return Single(TokenKind.RBracket, start);
                case ',':
                    return Single(TokenKind.Comma, start);
                case '.':
                    if (PeekChar(1) is { } afterDot && char.IsAsciiDigit(afterDot))
                    {
                        throw Error(start, "decimal literals need a leading digit, e.g. `0.5`");
                    }

                    return Single(TokenKind.Dot, start);
                case '=':
                    position += PeekChar(1) == '=' ? 2 : 1;
                    return new Token(TokenKind.Compare, start, Compare: FilterOperator.Eq);
                case '!':
                    if (PeekChar(1) == '=')
                    {
                        position += 2;
                        return new Token(TokenKind.Compare, start, Compare: FilterOperator.Ne);
                    }

                    throw Error(start, "use the keyword `not` instead of `!`");
                case '<':
                    var (lessOp, lessWidth) = PeekChar(1) switch
                    {
                        '=' => (FilterOperator.Lte, 2),
                        '>' => (FilterOperator.Ne, 2),
                        _ => (FilterOperator.Lt, 1),
                    };
                    position += lessWidth;
                    return new Token(TokenKind.Compare, start, Compare: lessOp);
                case '>':
                    var greaterOp = PeekChar(1) == '=' ? FilterOperator.Gte : FilterOperator.Gt;
                    position += greaterOp == FilterOperator.Gte ? 2 : 1;
                    return new Token(TokenKind.Compare, start, Compare: greaterOp);
                case '&':
                    throw Error(start, "use the keyword `and` instead of `&` or `&&`");
                case '|':
                    throw Error(start, "use the keyword `or` instead of `|` or `||`");
                case '"' or '\'':
                    return LexString(character);
                case '`':
                    return LexQuotedSegment();
                case '-':
                    if (PeekChar(1) is { } afterMinus && char.IsAsciiDigit(afterMinus))
                    {
                        return LexNumber();
                    }

                    if (allowMinus)
                    {
                        return Single(TokenKind.Minus, start);
                    }

                    throw Error(
                        start,
                        "unexpected `-`; quote field names that contain `-` with backticks, e.g. `display-name`");
                case >= '0' and <= '9':
                    return LexNumber();
                case (>= 'A' and <= 'Z') or (>= 'a' and <= 'z') or '_':
                    return LexWord(start);
                case ':' when allowMinus:
                    throw Error(
                        start,
                        "unexpected `:`; write `field` for ascending and `-field` for descending order");
                case ':':
                    throw Error(start, "unexpected `:`; compare values with `=`, e.g. `status = \"active\"`");
                default:
                    var length = char.IsHighSurrogate(character) && PeekChar(1) is { } low && char.IsLowSurrogate(low) ? 2 : 1;
                    throw Error(start, $"unexpected character `{input.Substring(start, length)}`");
            }
        }

        private Token Single(TokenKind kind, int start)
        {
            position++;
            return new Token(kind, start);
        }

        private Token LexWord(int start)
        {
            var end = start + 1;
            while (end < input.Length && (char.IsAsciiLetterOrDigit(input[end]) || input[end] == '_'))
            {
                end++;
            }

            position = end;
            if (PeekChar(0) == '-' && PeekChar(1) is { } afterDash && char.IsAsciiLetter(afterDash))
            {
                var wordEnd = end;
                while (wordEnd < input.Length
                    && (char.IsAsciiLetterOrDigit(input[wordEnd]) || input[wordEnd] is '_' or '-'))
                {
                    wordEnd++;
                }

                throw Error(
                    start,
                    $"wrap field names containing `-` in backticks, e.g. `{input[start..wordEnd]}`");
            }

            return new Token(TokenKind.Word, start, input[start..end]);
        }

        private Token LexNumber()
        {
            var start = position;
            var end = start;
            if (end < input.Length && input[end] == '-')
            {
                end++;
            }

            var integerStart = end;
            while (end < input.Length && char.IsAsciiDigit(input[end]))
            {
                end++;
            }

            if (end - integerStart > 1 && input[integerStart] == '0')
            {
                throw Error(start, "numbers must not have leading zeros");
            }

            if (end < input.Length && input[end] == '.')
            {
                end++;
                var fractionStart = end;
                while (end < input.Length && char.IsAsciiDigit(input[end]))
                {
                    end++;
                }

                if (end == fractionStart)
                {
                    throw Error(start, "decimal literals need digits after `.`");
                }
            }

            if (end < input.Length)
            {
                var next = input[end];
                if (next is 'e' or 'E')
                {
                    throw Error(start, "exponent notation is not supported; write the full decimal value");
                }

                if (char.IsAsciiLetter(next) || next == '_')
                {
                    throw Error(start, "a number cannot be followed directly by letters; quote text values");
                }
            }

            position = end;
            return new Token(TokenKind.Number, start, input[start..end]);
        }

        private Token LexString(char quote)
        {
            var start = position;
            var builder = new StringBuilder();
            var index = start + 1;
            while (true)
            {
                if (index >= input.Length)
                {
                    throw Error(start, "unterminated string literal");
                }

                var character = input[index];
                if (character == quote)
                {
                    position = index + 1;
                    return new Token(TokenKind.Str, start, builder.ToString());
                }

                if (character == '\\')
                {
                    if (index + 1 >= input.Length)
                    {
                        throw Error(index, "unterminated escape sequence");
                    }

                    var escaped = input[index + 1];
                    switch (escaped)
                    {
                        case '"' or '\'' or '\\' or '/':
                            builder.Append(escaped);
                            index += 2;
                            continue;
                        case 'b':
                            builder.Append('\b');
                            index += 2;
                            continue;
                        case 'f':
                            builder.Append('\f');
                            index += 2;
                            continue;
                        case 'n':
                            builder.Append('\n');
                            index += 2;
                            continue;
                        case 'r':
                            builder.Append('\r');
                            index += 2;
                            continue;
                        case 't':
                            builder.Append('\t');
                            index += 2;
                            continue;
                        case 'u':
                            index = UnicodeEscape(builder, index);
                            continue;
                        default:
                            var length = char.IsHighSurrogate(escaped) && index + 2 < input.Length && char.IsLowSurrogate(input[index + 2]) ? 2 : 1;
                            throw Error(index, $"unknown escape sequence `\\{input.Substring(index + 1, length)}`");
                    }
                }

                if (char.IsControl(character))
                {
                    throw Error(index, "control characters must be escaped inside string literals");
                }

                builder.Append(character);
                index++;
            }
        }

        /// <summary>Decodes <c>\uXXXX</c> (and a surrogate pair) starting at the backslash.</summary>
        private int UnicodeEscape(StringBuilder builder, int at)
        {
            var cursor = at + 2;
            int? ReadUnit()
            {
                if (cursor + 4 > input.Length)
                {
                    cursor = input.Length;
                    return null;
                }

                if (!int.TryParse(input.AsSpan(cursor, 4), NumberStyles.AllowHexSpecifier, CultureInfo.InvariantCulture, out var value)
                    || !IsHex(input.AsSpan(cursor, 4)))
                {
                    return null;
                }

                cursor += 4;
                return value;
            }

            const string invalid = "invalid `\\u` escape; expected four hexadecimal digits";
            const string unpaired = "unpaired UTF-16 surrogate in `\\u` escape";
            var first = ReadUnit() ?? throw Error(at, invalid);
            if (first is >= 0xD800 and < 0xDC00)
            {
                if (cursor + 1 >= input.Length || input[cursor] != '\\' || input[cursor + 1] != 'u')
                {
                    throw Error(at, unpaired);
                }

                cursor += 2;
                var second = ReadUnit() ?? throw Error(at, invalid);
                if (second is not (>= 0xDC00 and < 0xE000))
                {
                    throw Error(at, unpaired);
                }

                builder.Append((char)first).Append((char)second);
                return cursor;
            }

            if (first is >= 0xDC00 and < 0xE000)
            {
                throw Error(at, unpaired);
            }

            builder.Append((char)first);
            return cursor;
        }

        private static bool IsHex(ReadOnlySpan<char> digits)
        {
            foreach (var digit in digits)
            {
                if (!char.IsAsciiHexDigit(digit))
                {
                    return false;
                }
            }

            return true;
        }

        private Token LexQuotedSegment()
        {
            var start = position;
            var close = input.IndexOf('`', start + 1);
            if (close < 0)
            {
                throw Error(start, "unterminated backtick-quoted field name");
            }

            var segment = input[(start + 1)..close];
            if (segment.Length == 0)
            {
                throw Error(start, "backtick-quoted field names must not be empty");
            }

            if (segment.Contains('.', StringComparison.Ordinal))
            {
                throw Error(start, "a backtick-quoted segment must not contain `.`; quote each segment separately");
            }

            position = close + 1;
            return new Token(TokenKind.Quoted, start, segment);
        }
    }

    private sealed class Parser
    {
        private readonly string input;
        private readonly List<Token> tokens;
        private readonly string parameter;
        private int position;
        private int nesting;

        internal Parser(string input, List<Token> tokens, string parameter)
        {
            this.input = input;
            this.tokens = tokens;
            this.parameter = parameter;
        }

        internal Token Peek() => tokens[Math.Min(position, tokens.Count - 1)];

        private Token PeekAt(int ahead) => tokens[Math.Min(position + ahead, tokens.Count - 1)];

        internal Token Advance()
        {
            var token = Peek();
            if (token.Kind != TokenKind.End)
            {
                position++;
            }

            return token;
        }

        internal FilterSyntaxException ErrorAt(Token token, string message) =>
            SyntaxError(parameter, input, token.Start, message);

        private void Enter(Token token)
        {
            nesting++;
            if (nesting > MaxParseNesting)
            {
                throw ErrorAt(token, "filter nests too deeply");
            }
        }

        internal Filter ParseOr()
        {
            var first = ParseAnd();
            if (!IsKeyword(Peek(), "or"))
            {
                return first;
            }

            var operands = ImmutableArray.CreateBuilder<Filter>();
            operands.Add(first);
            while (IsKeyword(Peek(), "or"))
            {
                Advance();
                operands.Add(ParseAnd());
            }

            return new OrFilter(operands.ToImmutable(), validated: false);
        }

        private Filter ParseAnd()
        {
            var first = ParseUnary();
            if (!IsKeyword(Peek(), "and"))
            {
                return first;
            }

            var operands = ImmutableArray.CreateBuilder<Filter>();
            operands.Add(first);
            while (IsKeyword(Peek(), "and"))
            {
                Advance();
                operands.Add(ParseUnary());
            }

            return new AndFilter(operands.ToImmutable(), validated: false);
        }

        private Filter ParseUnary()
        {
            if (IsKeyword(Peek(), "not"))
            {
                var token = Advance();
                Enter(token);
                var inner = ParseUnary();
                nesting--;
                return new NotFilter(inner);
            }

            return ParsePrimary();
        }

        private Filter ParsePrimary()
        {
            var token = Peek();
            switch (token.Kind)
            {
                case TokenKind.LParen:
                    Advance();
                    Enter(token);
                    var inner = ParseOr();
                    nesting--;
                    ExpectClose(TokenKind.RParen, token);
                    return inner;
                case TokenKind.Word when IsKeyword(token, "exists") && PeekAt(1).Kind == TokenKind.LParen:
                    Advance();
                    var open = Advance();
                    var field = ParsePath();
                    ExpectClose(TokenKind.RParen, open);
                    return new ExistsFilter(field);
                case TokenKind.Word or TokenKind.Quoted:
                    return ParsePredicate(ParsePath());
                case TokenKind.Str or TokenKind.Number:
                    throw ErrorAt(token, "expected a field name on the left-hand side, e.g. `quantity > 5`");
                case TokenKind.End:
                    throw ErrorAt(token, "expected a filter expression");
                default:
                    throw ErrorAt(token, $"expected a field name, found {Describe(token)}");
            }
        }

        private void ExpectClose(TokenKind close, Token open)
        {
            var token = Advance();
            if (token.Kind == close)
            {
                return;
            }

            var (symbol, opened) = close == TokenKind.RParen ? ("`)`", "`(`") : ("`]`", "`[`");
            var column = SyntaxError(parameter, input, open.Start, string.Empty).Column;
            throw ErrorAt(
                token,
                $"expected {symbol} to close the {opened} at column {column}, found {Describe(token)}");
        }

        internal FieldPath ParsePath()
        {
            var first = Advance();
            string path;
            switch (first.Kind)
            {
                case TokenKind.Word:
                    if (CanonicalText.Keyword(first.Text) is { } keyword)
                    {
                        throw ErrorAt(
                            first,
                            $"expected a field name, found the keyword `{keyword}`; quote a field with this name as `{first.Text}` in backticks");
                    }

                    path = first.Text!;
                    break;
                case TokenKind.Quoted:
                    path = first.Text!;
                    break;
                default:
                    throw ErrorAt(first, $"expected a field name, found {Describe(first)}");
            }

            var builder = new StringBuilder(path);
            while (Peek().Kind == TokenKind.Dot)
            {
                Advance();
                var segment = Advance();
                if (segment.Kind is TokenKind.Word or TokenKind.Quoted)
                {
                    builder.Append('.').Append(segment.Text);
                }
                else
                {
                    throw ErrorAt(segment, $"expected a field name after `.`, found {Describe(segment)}");
                }
            }

            path = builder.ToString();
            var reason = FieldPath.ValidationError(path);
            if (reason is not null)
            {
                throw ErrorAt(first, $"invalid field `{path}`: {reason}");
            }

            return new FieldPath(path);
        }

        private Filter ParsePredicate(FieldPath field)
        {
            var token = Advance();
            if (token.Kind == TokenKind.Compare)
            {
                return new ComparisonFilter(field, token.Compare, ParseLiteral(), validated: false);
            }

            if (IsKeyword(token, "in"))
            {
                return new MembershipFilter(field, ParseList(), negated: false, validated: false);
            }

            if (IsKeyword(token, "not"))
            {
                var next = Advance();
                if (IsKeyword(next, "in"))
                {
                    return new MembershipFilter(field, ParseList(), negated: true, validated: false);
                }

                throw ErrorAt(next, "expected `in` after `not` (as in `field not in [...]`)");
            }

            if (IsKeyword(token, "is"))
            {
                var next = Advance();
                if (IsKeyword(next, "null"))
                {
                    return new IsNullFilter(field);
                }

                if (IsKeyword(next, "not"))
                {
                    var nullToken = Advance();
                    if (IsKeyword(nullToken, "null"))
                    {
                        return new NotFilter(new IsNullFilter(field));
                    }

                    throw ErrorAt(nullToken, "expected `null` after `is not`");
                }

                throw ErrorAt(next, "expected `null` or `not null` after `is`; compare values with `=`");
            }

            const string operators = "(=, !=, <, <=, >, >=, in, not in, is null)";
            throw token.Kind == TokenKind.End
                ? ErrorAt(token, $"expected an operator after `{field}` {operators}")
                : ErrorAt(token, $"expected an operator after `{field}` {operators}, found {Describe(token)}");
        }

        private ImmutableArray<FilterLiteral> ParseList()
        {
            var open = Advance();
            var close = open.Kind switch
            {
                TokenKind.LBracket => TokenKind.RBracket,
                TokenKind.LParen => TokenKind.RParen,
                _ => throw ErrorAt(open, $"expected `[` to start a value list, found {Describe(open)}"),
            };
            var values = ImmutableArray.CreateBuilder<FilterLiteral>();
            while (true)
            {
                if (Peek().Kind == close)
                {
                    if (values.Count == 0)
                    {
                        throw ErrorAt(Peek(), "value lists must not be empty");
                    }

                    Advance();
                    return values.ToImmutable();
                }

                values.Add(ParseLiteral());
                var separator = Peek();
                if (separator.Kind == TokenKind.Comma)
                {
                    Advance();
                }
                else if (separator.Kind != close)
                {
                    ExpectClose(close, open);
                    return values.ToImmutable();
                }
            }
        }

        private FilterLiteral ParseLiteral()
        {
            var token = Advance();
            switch (token.Kind)
            {
                case TokenKind.Str:
                    return FilterLiteral.String(token.Text!);
                case TokenKind.Number:
                    return FilterLiteral.FromNumberText(token.Text!);
                case TokenKind.Word when IsKeyword(token, "true"):
                    return FilterLiteral.Boolean(true);
                case TokenKind.Word when IsKeyword(token, "false"):
                    return FilterLiteral.Boolean(false);
                case TokenKind.Word when IsKeyword(token, "null"):
                    return FilterLiteral.Null;
                case TokenKind.Word:
                    throw ErrorAt(
                        token,
                        $"expected a literal value, found `{token.Text}`; quote text values, e.g. \"{token.Text}\"");
                default:
                    throw ErrorAt(token, $"expected a literal value, found {Describe(token)}");
            }
        }

        internal void Finish()
        {
            var token = Peek();
            if (token.Kind == TokenKind.End)
            {
                return;
            }

            var hint = token.Kind is TokenKind.Word or TokenKind.Quoted
                ? "; combine conditions with `and` or `or`"
                : string.Empty;
            throw ErrorAt(token, $"unexpected {Describe(token)} after a complete filter{hint}");
        }
    }
}
