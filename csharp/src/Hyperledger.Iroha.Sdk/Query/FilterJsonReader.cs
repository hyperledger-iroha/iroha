using System.Collections.Immutable;
using System.Text;
using System.Text.Json;

namespace Hyperledger.Iroha.Query;

/// <summary>Decodes the JSON form of a filter with Torii's checks and error messages.</summary>
internal static class FilterJsonReader
{
    private const string Operators = "and, or, not, eq, ne, lt, lte, gt, gte, in, nin, exists, is_null";

    /// <summary>Decodes a JSON filter node, or the text form when the element is a string.</summary>
    internal static Filter Read(JsonElement element, string parameter)
    {
        if (element.ValueKind == JsonValueKind.String)
        {
            return FilterTextParser.ParseFilter(element.GetString()!, parameter);
        }

        return new Reader(parameter).ReadNode(element, 0);
    }

    private sealed class Reader
    {
        private readonly string parameter;
        private readonly List<object> location = [];
        private int nodes;
        private int listValues;

        internal Reader(string parameter)
        {
            this.parameter = parameter;
        }

        internal Filter ReadNode(JsonElement value, int depth)
        {
            if (depth > Filter.MaxDepth)
            {
                throw Error(Filter.LimitExceeded("nesting depth", Filter.MaxDepth));
            }

            nodes++;
            if (nodes > Filter.MaxNodes)
            {
                throw Error(Filter.LimitExceeded("node count", Filter.MaxNodes));
            }

            if (value.ValueKind != JsonValueKind.Object)
            {
                throw Malformed(
                    "a filter node must be an object such as {\"op\": \"eq\", \"args\": [\"field\", value]}");
            }

            JsonElement? op = null;
            JsonElement? args = null;
            string? unknown = null;
            foreach (var member in value.EnumerateObject())
            {
                switch (member.Name)
                {
                    case "op" when op is null:
                        op = member.Value;
                        break;
                    case "args" when args is null:
                        args = member.Value;
                        break;
                    case "op" or "args":
                        throw Malformed($"duplicate member `{member.Name}`");
                    default:
                        if (unknown is null || string.CompareOrdinal(member.Name, unknown) < 0)
                        {
                            unknown = member.Name;
                        }

                        break;
                }
            }

            if (op is null)
            {
                throw Malformed("a filter node needs an `op` member");
            }

            if (op.Value.ValueKind != JsonValueKind.String)
            {
                throw Malformed("`op` must be a string");
            }

            if (unknown is not null)
            {
                throw Malformed($"unknown member `{unknown}`; a filter node has only `op` and `args`");
            }

            var name = op.Value.GetString()!;
            var arguments = args ?? default;
            location.Add("args");
            Filter parsed;
            switch (name)
            {
                case "and" or "or":
                    parsed = ReadLogical(name, arguments, depth);
                    break;
                case "not":
                    if (arguments.ValueKind != JsonValueKind.Array || arguments.GetArrayLength() != 1)
                    {
                        throw Malformed("`not` takes an array with exactly one filter node");
                    }

                    location.Add(0);
                    var inner = ReadNode(arguments[0], depth + 1);
                    location.RemoveAt(location.Count - 1);
                    parsed = new NotFilter(inner);
                    break;
                case "eq" or "ne" or "lt" or "lte" or "gt" or "gte":
                {
                    var (field, operand) = BinaryArguments(arguments, name);
                    var comparison = name switch
                    {
                        "eq" => FilterOperator.Eq,
                        "ne" => FilterOperator.Ne,
                        "lt" => FilterOperator.Lt,
                        "lte" => FilterOperator.Lte,
                        "gt" => FilterOperator.Gt,
                        _ => FilterOperator.Gte,
                    };
                    var path = RequireField(field);
                    var literal = FilterLiteral.FromJsonElement(operand);
                    var error = ComparisonFilter.OperandError(path, comparison, literal);
                    if (error is not null)
                    {
                        throw Error(error);
                    }

                    parsed = new ComparisonFilter(path, comparison, literal, validated: true);
                    break;
                }

                case "in" or "nin":
                {
                    var (field, operand) = BinaryArguments(arguments, name);
                    if (operand.ValueKind != JsonValueKind.Array)
                    {
                        throw Malformed($"`{name}` takes [\"field\", [value, ...]]");
                    }

                    var path = RequireField(field);
                    var values = operand.EnumerateArray().Select(FilterLiteral.FromJsonElement).ToImmutableArray();
                    listValues += values.Length;
                    var error = MembershipFilter.ValuesError(path, values, listValues);
                    if (error is not null)
                    {
                        throw Error(error);
                    }

                    parsed = new MembershipFilter(path, values, negated: name == "nin", validated: true);
                    break;
                }

                case "exists" or "is_null":
                {
                    if (arguments.ValueKind != JsonValueKind.Array || arguments.GetArrayLength() != 1)
                    {
                        throw Malformed($"`{name}` takes [\"field\"]");
                    }

                    if (arguments[0].ValueKind != JsonValueKind.String)
                    {
                        throw Malformed("the field must be a string");
                    }

                    var path = RequireField(arguments[0].GetString()!);
                    parsed = name == "exists" ? new ExistsFilter(path) : new IsNullFilter(path);
                    break;
                }

                default:
                    location.RemoveAt(location.Count - 1);
                    throw Malformed($"unknown operator `{name}`; expected one of: {Operators}");
            }

            location.RemoveAt(location.Count - 1);
            return parsed;
        }

        private Filter ReadLogical(string name, JsonElement arguments, int depth)
        {
            if (arguments.ValueKind != JsonValueKind.Array)
            {
                throw Malformed($"`{name}` takes an array of filter nodes");
            }

            var count = arguments.GetArrayLength();
            if (count == 0)
            {
                throw Malformed($"`{name}` needs at least one operand");
            }

            if (count > Filter.MaxNodes - nodes)
            {
                throw Error(Filter.LimitExceeded("node count", Filter.MaxNodes));
            }

            var operands = ImmutableArray.CreateBuilder<Filter>(count);
            var index = 0;
            foreach (var nested in arguments.EnumerateArray())
            {
                location.Add(index++);
                operands.Add(ReadNode(nested, depth + 1));
                location.RemoveAt(location.Count - 1);
            }

            return name == "and"
                ? new AndFilter(operands.MoveToImmutable(), validated: true)
                : new OrFilter(operands.MoveToImmutable(), validated: true);
        }

        private (string Field, JsonElement Operand) BinaryArguments(JsonElement arguments, string name)
        {
            if (arguments.ValueKind != JsonValueKind.Array || arguments.GetArrayLength() != 2)
            {
                throw Malformed($"`{name}` takes [\"field\", value]");
            }

            if (arguments[0].ValueKind != JsonValueKind.String)
            {
                throw Malformed("the first argument must be the field name");
            }

            return (arguments[0].GetString()!, arguments[1]);
        }

        private FieldPath RequireField(string field)
        {
            var reason = FieldPath.ValidationError(field);
            return reason is null ? new FieldPath(field) : throw Error($"invalid field `{field}`: {reason}");
        }

        private ListQueryException Error(string message) => new(parameter, message);

        private ListQueryException Malformed(string reason)
        {
            var rendered = new StringBuilder();
            foreach (var segment in location)
            {
                if (segment is int index)
                {
                    rendered.Append('[').Append(index).Append(']');
                }
                else
                {
                    if (rendered.Length > 0)
                    {
                        rendered.Append('.');
                    }

                    rendered.Append((string)segment);
                }
            }

            return Error(rendered.Length == 0 ? reason : $"{reason} (at `{rendered}`)");
        }
    }
}
