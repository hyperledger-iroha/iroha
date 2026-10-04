using System.Collections.Immutable;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace Hyperledger.Iroha.Query;

/// <summary>One page of a collection read: <c>{"items": [...], "next_cursor": "...", "total": N}</c>.</summary>
/// <typeparam name="T">The item type.</typeparam>
public sealed class Page<T>
{
    /// <summary>Creates a page.</summary>
    public Page(ImmutableArray<T> items, string? nextCursor, ulong? total = null)
    {
        Items = items.IsDefault ? ImmutableArray<T>.Empty : items;
        NextCursor = nextCursor;
        Total = total;
    }

    /// <summary>The items, in the requested order.</summary>
    public ImmutableArray<T> Items { get; }

    /// <summary>Pass as <see cref="ListQuery.Cursor"/> to fetch the next page; <see langword="null"/> on the last page.</summary>
    public string? NextCursor { get; }

    /// <summary>The exact number of matching rows; present only when <see cref="ListQuery.IncludeTotal"/> was set.</summary>
    public ulong? Total { get; }

    /// <summary>Whether another page follows.</summary>
    public bool HasMore => NextCursor is not null;
}

/// <summary>Reads one collection row from its JSON object.</summary>
internal delegate T CollectionRowReader<out T>(JsonElement row, string context);

/// <summary>Decodes the page envelope shared by every collection.</summary>
internal static class PageReader
{
    /// <summary>Raw rows as detached JSON objects.</summary>
    internal static readonly CollectionRowReader<JsonObject> JsonRows = static (row, context) =>
        row.ValueKind == JsonValueKind.Object
            ? JsonObject.Create(row.Clone())!
            : throw new JsonException($"{context} must be a JSON object.");

    internal static Page<T> Read<T>(JsonElement root, CollectionRowReader<T> readRow, string context)
    {
        if (root.ValueKind != JsonValueKind.Object)
        {
            throw new JsonException($"{context} must be a JSON object.");
        }

        ImmutableArray<T>? items = null;
        string? nextCursor = null;
        ulong? total = null;
        foreach (var member in root.EnumerateObject())
        {
            switch (member.Name)
            {
                case "items":
                    if (member.Value.ValueKind != JsonValueKind.Array)
                    {
                        throw new JsonException($"{context}.items must be an array.");
                    }

                    var builder = ImmutableArray.CreateBuilder<T>(member.Value.GetArrayLength());
                    var index = 0;
                    foreach (var row in member.Value.EnumerateArray())
                    {
                        builder.Add(readRow(row, $"{context}.items[{index++}]"));
                    }

                    items = builder.MoveToImmutable();
                    break;
                case "next_cursor":
                    nextCursor = member.Value.ValueKind switch
                    {
                        JsonValueKind.Null => null,
                        JsonValueKind.String when member.Value.GetString() is { Length: > 0 } cursor => cursor,
                        _ => throw new JsonException($"{context}.next_cursor must be a non-empty string or null."),
                    };
                    break;
                case "total":
                    total = member.Value.ValueKind switch
                    {
                        JsonValueKind.Null => null,
                        JsonValueKind.Number when member.Value.TryGetUInt64(out var count) => count,
                        _ => throw new JsonException($"{context}.total must be a non-negative integer."),
                    };
                    break;
            }
        }

        return items is { } rows
            ? new Page<T>(rows, nextCursor, total)
            : throw new JsonException($"{context} must contain an `items` array.");
    }
}
