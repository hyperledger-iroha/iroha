using System.Runtime.CompilerServices;
using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Query;

namespace Hyperledger.Iroha.Torii;

/// <summary>
/// One Torii collection, read with <see cref="ListQuery"/>.
/// </summary>
/// <remarks>
/// <para>Every method sends <c>POST {Path}/query</c>. Reads are public; when the client has
/// canonical request credentials the request is signed, which only widens visibility into
/// restricted dataspaces.</para>
/// <code>
/// await foreach (var definition in client.AssetDefinitions.EnumerateAsync(
///     new ListQuery { Filter = Filter.Field("owned_by").Eq(alice), Sort = ["name"] },
///     cancellationToken))
/// {
///     Console.WriteLine(definition.Id);
/// }
/// </code>
/// <para>Typed items are full rows. Projections (<see cref="ListQuery.Select"/>) and aggregates
/// (<see cref="ListQuery.Aggregate"/>) return partial or computed rows; read them through
/// <see cref="Rows"/>.</para>
/// <para>History and Explorer collections use fixed server order, and
/// <see cref="ListQuery.Sort"/>, <see cref="ListQuery.IncludeTotal"/> and
/// <see cref="ListQuery.Aggregate"/> are rejected (<c>invalid_sort</c>, <c>invalid_include_total</c>,
/// <c>invalid_aggregate</c>) because each would exceed the scan budget. Each bounded page has a
/// bounded scan budget, so a page may hold fewer than <see cref="ListQuery.Limit"/> items, even none,
/// while <see cref="Page{T}.NextCursor"/> is set; the iterators keep following the cursor until it is
/// <see langword="null"/>.</para>
/// </remarks>
/// <typeparam name="T">The row type.</typeparam>
public sealed class ToriiCollection<T>
{
    private readonly ToriiClient client;
    private readonly CollectionRowReader<T> readRow;
    private readonly bool typedRows;
    private readonly string? historyId;

    /// <param name="client">The client that sends the requests.</param>
    /// <param name="path">The collection path without <c>/query</c>.</param>
    /// <param name="readRow">Decodes one item.</param>
    /// <param name="typedRows">Whether items are full typed rows, which excludes projections and aggregates.</param>
    /// <param name="historyId">
    /// The Torii collection id of a history or Explorer feed, which rejects re-sorting, totals and aggregates; <see langword="null"/> otherwise.
    /// </param>
    internal ToriiCollection(
        ToriiClient client,
        string path,
        CollectionRowReader<T> readRow,
        bool typedRows,
        string? historyId = null)
    {
        this.client = client;
        Path = path;
        this.readRow = readRow;
        this.typedRows = typedRows;
        this.historyId = historyId;
    }

    /// <summary>The collection path, e.g. <c>/v1/assets/definitions</c>.</summary>
    public string Path { get; }

    /// <summary>
    /// The same collection with each item as a raw JSON object, for projections, aggregates and
    /// fields this SDK does not model yet.
    /// </summary>
    public ToriiCollection<JsonObject> Rows => new(client, Path, PageReader.JsonRows, typedRows: false, historyId);

    /// <summary>Reads one page.</summary>
    /// <param name="query">The query; <see langword="null"/> reads the first page in default order.</param>
    /// <param name="cancellationToken">Cancels the request.</param>
    /// <exception cref="ListQueryException">A control is invalid; nothing was sent.</exception>
    /// <exception cref="ToriiApiException">Torii rejected the query; <see cref="IrohaException.Code"/> names the control.</exception>
    public async Task<Page<T>> GetPageAsync(ListQuery? query = null, CancellationToken cancellationToken = default)
    {
        query ??= ListQuery.Empty;
        Prepare(query);
        var page = await client.QueryCollectionPageAsync(Path, query, readRow, cancellationToken).ConfigureAwait(false);
        ValidatePage(page, query);
        return page;
    }

    /// <summary>Reads every page, following <see cref="Page{T}.NextCursor"/> until the last page.</summary>
    /// <remarks>
    /// Pages with fewer items than requested, or none, do not end the iteration; only a
    /// <see langword="null"/> <see cref="Page{T}.NextCursor"/> does.
    /// </remarks>
    /// <param name="query">The query for the first page; its <see cref="ListQuery.Cursor"/> is the starting point.</param>
    /// <param name="cancellationToken">Cancels the iteration.</param>
    /// <exception cref="IrohaException">
    /// <c>cursor_not_advancing</c>: Torii answered a page with the cursor that was just sent.
    /// </exception>
    public async IAsyncEnumerable<Page<T>> EnumeratePagesAsync(
        ListQuery? query = null,
        [EnumeratorCancellation] CancellationToken cancellationToken = default)
    {
        var current = query ?? ListQuery.Empty;
        Prepare(current);
        while (true)
        {
            var page = await client.QueryCollectionPageAsync(Path, current, readRow, cancellationToken)
                .ConfigureAwait(false);
            ValidatePage(page, current);
            yield return page;
            if (page.NextCursor is null)
            {
                yield break;
            }

            if (string.Equals(page.NextCursor, current.Cursor, StringComparison.Ordinal))
            {
                throw new IrohaException(
                    "cursor_not_advancing",
                    $"Torii returned the request cursor again for `{Path}`; stopping to avoid an endless loop.");
            }

            current = current with { Cursor = page.NextCursor };
        }
    }

    /// <summary>Reads every item of every page, following cursors until the last page.</summary>
    /// <param name="query">The query for the first page.</param>
    /// <param name="cancellationToken">Cancels the iteration.</param>
    public async IAsyncEnumerable<T> EnumerateAsync(
        ListQuery? query = null,
        [EnumeratorCancellation] CancellationToken cancellationToken = default)
    {
        await foreach (var page in EnumeratePagesAsync(query, cancellationToken).ConfigureAwait(false))
        {
            foreach (var item in page.Items)
            {
                yield return item;
            }
        }
    }

    private void ValidatePage(Page<T> page, ListQuery query)
    {
        if (query.Limit is { } limit && page.Items.Length > limit)
            throw new JsonException($"Torii page for `{Path}` contains more items than the requested limit.");
        if (historyId is not null && page.Total is not null)
            throw new JsonException($"Torii page for `{Path}` must omit total for a bounded collection.");
    }

    private void Prepare(ListQuery query)
    {
        query.Validate();
        if (historyId is not null)
        {
            RejectHistoryScans(query, historyId);
        }

        if (typedRows && (!query.Select.IsEmpty || query.Aggregate is not null))
        {
            throw new ArgumentException(
                $"`Select` and `Aggregate` return partial rows; read them through `{nameof(Rows)}` (for example `client.Domains.Rows`).",
                nameof(query));
        }
    }

    /// <summary>Rejects the controls a history collection cannot serve, with Torii's codes and reasons.</summary>
    private static void RejectHistoryScans(ListQuery query, string id)
    {
        if (!query.Sort.IsEmpty)
        {
            throw new ListQueryException(
                "sort",
                $"`{id}` rows use fixed server order and cannot be re-sorted; omit `sort` and use `filter` to select rows");
        }

        if (query.IncludeTotal)
        {
            throw new ListQueryException(
                "include_total",
                $"totals are not available for `{id}`: counting would exceed the bounded scan");
        }

        if (query.Aggregate is not null)
        {
            throw new ListQueryException(
                "aggregate",
                $"aggregates are not available for `{id}`: they would exceed the bounded scan");
        }
    }
}
