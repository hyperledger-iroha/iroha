// Torii collections: one query language and one page envelope for every list route.

using System.Net.Http.Headers;
using System.Text.Json;
using Hyperledger.Iroha.Query;

namespace Hyperledger.Iroha.Torii;

public sealed partial class ToriiClient
{
    /// <summary>Domains (<c>/v1/domains</c>); default order <c>id</c>.</summary>
    public ToriiCollection<DomainRow> Domains => new(this, "/v1/domains", DomainRow.Read, typedRows: true);

    /// <summary>Accounts (<c>/v1/accounts</c>); default order <c>id</c>.</summary>
    public ToriiCollection<AccountRow> Accounts => new(this, "/v1/accounts", AccountRow.Read, typedRows: true);

    /// <summary>Asset definitions (<c>/v1/assets/definitions</c>); default order <c>id</c>.</summary>
    public ToriiCollection<AssetDefinitionRow> AssetDefinitions =>
        new(this, "/v1/assets/definitions", AssetDefinitionRow.Read, typedRows: true);

    /// <summary>NFTs (<c>/v1/nfts</c>); default order <c>id</c>.</summary>
    public ToriiCollection<NftRow> Nfts => new(this, "/v1/nfts", NftRow.Read, typedRows: true);

    /// <summary>RWA lots (<c>/v1/rwas</c>); default order <c>id</c>.</summary>
    public ToriiCollection<RwaRow> Rwas => new(this, "/v1/rwas", RwaRow.Read, typedRows: true);

    /// <summary>Repo agreements (<c>/v1/repo/agreements</c>); default order <c>id</c>.</summary>
    public ToriiCollection<RepoAgreementRow> RepoAgreements =>
        new(this, "/v1/repo/agreements", RepoAgreementRow.Read, typedRows: true);

    /// <summary>The assets held by one account; default order <c>asset</c>, <c>scope</c>.</summary>
    /// <param name="accountId">The canonical I105 account id.</param>
    public ToriiCollection<AccountAssetRow> AccountAssets(string accountId) =>
        new(this, $"/v1/accounts/{EncodeIdentifierPathSegment(accountId, nameof(accountId))}/assets", AccountAssetRow.Read, typedRows: true);

    /// <summary>The holders of one asset definition; default order <c>account_id</c>, <c>scope</c>.</summary>
    /// <param name="assetDefinitionId">The Base58 asset definition id.</param>
    public ToriiCollection<AssetHolderRow> AssetHolders(string assetDefinitionId) =>
        new(this, $"/v1/assets/{EncodeIdentifierPathSegment(assetDefinitionId, nameof(assetDefinitionId))}/holders", AssetHolderRow.Read, typedRows: true);

    /// <summary>
    /// Every committed transaction (<c>POST /v1/transactions/query</c>), newest first by
    /// <c>block_height</c> and <c>block_index</c>.
    /// </summary>
    /// <remarks>
    /// A history collection: no <see cref="ListQuery.Sort"/>, <see cref="ListQuery.IncludeTotal"/> or
    /// <see cref="ListQuery.Aggregate"/>, and pages may be short or empty while a cursor remains.
    /// Bounds on <c>block_height</c> in the filter's top-level <c>and</c> also bound the scan, so
    /// <c>block_height &gt;= 1200 and result_ok = true</c> reads only blocks from height 1200 up.
    /// </remarks>
    public ToriiCollection<TransactionRow> Transactions =>
        new(this, "/v1/transactions", TransactionRow.Read, typedRows: true, historyId: "transactions");

    /// <summary>
    /// The committed transactions one account signed or that reference it, newest first; a history
    /// collection like <see cref="Transactions"/>.
    /// </summary>
    /// <param name="accountId">The canonical I105 account id.</param>
    public ToriiCollection<TransactionRow> AccountTransactions(string accountId) =>
        new(
            this,
            $"/v1/accounts/{EncodeIdentifierPathSegment(accountId, nameof(accountId))}/transactions",
            TransactionRow.Read,
            typedRows: true,
            historyId: "account_transactions");

    /// <summary>Sends one already-validated query to <c>POST {path}/query</c>.</summary>
    internal async Task<Page<T>> QueryCollectionPageAsync<T>(
        string path,
        ListQuery query,
        CollectionRowReader<T> readRow,
        CancellationToken cancellationToken)
    {
        using var content = new ByteArrayContent(query.ToJsonUtf8Bytes());
        content.Headers.ContentType = new MediaTypeHeaderValue("application/json", "utf-8");
        using var response = await SendAsync(
                HttpMethod.Post,
                path + "/query",
                query: null,
                content,
                accept: "application/json",
                cancellationToken: cancellationToken)
            .ConfigureAwait(false);
        var context = $"Torii page for `{path}`";
        var body = await ReadBoundedResponseBodyAsync(
                response.Content,
                DefaultJsonResponseMaxBytes,
                context,
                cancellationToken)
            .ConfigureAwait(false);
        using var document = JsonDocument.Parse(body, new JsonDocumentOptions { MaxDepth = 128 });
        ToriiIdentifierJson.RejectDuplicateProperties(document.RootElement, context);
        return PageReader.Read(document.RootElement, readRow, context);
    }
}
