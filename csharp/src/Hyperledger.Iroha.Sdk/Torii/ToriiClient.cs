using System.Buffers.Binary;
using System.Globalization;
using System.Net;
using System.Net.Http.Headers;
using System.Runtime.CompilerServices;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization.Metadata;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Queries;
using Hyperledger.Iroha.Sccp;
using Hyperledger.Iroha.Transactions;
using Hyperledger.Iroha.Zk;

namespace Hyperledger.Iroha.Torii;

public sealed partial class ToriiClient : IDisposable
{
    public const string AccountOnboardingTokenHeaderName = "X-Iroha-Onboarding-Token";

    private const int SoraFsAliasTextMaxChars = 128;
    private const string InvalidUtf8ResponseBody = "<response body is not valid UTF-8>";
    private static readonly Encoding StrictUtf8 = new UTF8Encoding(false, true);

    private readonly bool ownsHttpClient;
    private readonly JsonSerializerOptions serializerOptions;

    public ToriiClient(Uri baseUri, HttpClient? httpClient = null, ToriiClientOptions? options = null)
    {
        ArgumentNullException.ThrowIfNull(baseUri);

        BaseUri = NormalizeBaseUri(baseUri);
        HttpClient = httpClient ?? new HttpClient(new HttpClientHandler
        {
            AllowAutoRedirect = false,
        });
        HttpClient.DefaultRequestHeaders.Remove(AccountOnboardingTokenHeaderName);
        ownsHttpClient = httpClient is null;
        Options = options?.Snapshot() ?? new ToriiClientOptions();
        serializerOptions = CreateSerializerOptions(Options.JsonSerializerOptions);
    }

    public Uri BaseUri { get; }

    public HttpClient HttpClient { get; }

    public ToriiClientOptions Options { get; }

    public void Dispose()
    {
        if (ownsHttpClient)
        {
            HttpClient.Dispose();
        }
    }

    public async Task<JsonDocument> GetJsonDocumentAsync(string path, string? query = null, CancellationToken cancellationToken = default)
    {
        using var response = await SendAsync(HttpMethod.Get, path, query, content: null, cancellationToken: cancellationToken);
        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken);
        return await ParseJsonDocumentRejectingDuplicatePropertiesAsync(
            stream,
            $"Torii JSON response for `{response.RequestMessage?.RequestUri}`",
            cancellationToken);
    }

    public async Task<TResponse> GetAsync<TResponse>(string path, string? query = null, CancellationToken cancellationToken = default)
    {
        using var response = await SendAsync(HttpMethod.Get, path, query, content: null, cancellationToken: cancellationToken);
        return await DeserializeAsync<TResponse>(response, cancellationToken);
    }

    public async Task<TResponse> PostAsync<TRequest, TResponse>(
        string path,
        TRequest request,
        string? query = null,
        CancellationToken cancellationToken = default)
    {
        using var content = CreateJsonContent(request);
        using var response = await SendAsync(HttpMethod.Post, path, query, content, cancellationToken: cancellationToken);
        return await DeserializeAsync<TResponse>(response, cancellationToken);
    }

    private async Task<TResponse> PostAccountOnboardingAsync<TRequest, TResponse>(
        string path,
        TRequest request,
        string exactOnboardingToken,
        CancellationToken cancellationToken)
    {
        using var content = CreateJsonContent(request);
        try
        {
            using var response = await SendAsync(
                HttpMethod.Post,
                path,
                query: null,
                content,
                accept: "application/json",
                configureRequest: httpRequest =>
                {
                    httpRequest.Headers.Remove(AccountOnboardingTokenHeaderName);
                    if (!httpRequest.Headers.TryAddWithoutValidation(
                            AccountOnboardingTokenHeaderName,
                            exactOnboardingToken))
                    {
                        throw new InvalidOperationException("Unable to set the account onboarding credential header.");
                    }
                },
                cancellationToken);
            return await DeserializeAccountOnboardingAsync<TResponse>(
                response,
                exactOnboardingToken,
                cancellationToken);
        }
        catch (ToriiApiException error)
        {
            throw new ToriiApiException(
                error.StatusCode.GetValueOrDefault(HttpStatusCode.InternalServerError),
                error.RequestUri,
                RedactAccountOnboardingCredential(error.ResponseBody, exactOnboardingToken),
                RedactAccountOnboardingCredential(error.ReasonPhrase, exactOnboardingToken));
        }
    }

    private static string? RedactAccountOnboardingCredential(string? value, string credential) =>
        value?.Replace(credential, "<redacted>", StringComparison.Ordinal);

    private async Task<TResponse> DeserializeAccountOnboardingAsync<TResponse>(
        HttpResponseMessage response,
        string exactOnboardingToken,
        CancellationToken cancellationToken)
    {
        var responseText = await ReadStrictUtf8TextContentAsync(
            response.Content,
            "Torii account onboarding response body",
            cancellationToken);
        var redactedText = RedactAccountOnboardingCredential(responseText, exactOnboardingToken)
            ?? string.Empty;
        JsonDocument parsed;
        try
        {
            parsed = JsonDocument.Parse(redactedText, new JsonDocumentOptions { MaxDepth = 128 });
        }
        catch (JsonException)
        {
            throw new JsonException("Torii account onboarding response body was not valid JSON.");
        }

        using (parsed)
        using (var redactedDocument = RedactAccountOnboardingJson(
                   parsed.RootElement,
                   exactOnboardingToken))
        {
            ToriiIdentifierJson.RejectDuplicateProperties(
                redactedDocument.RootElement,
                DuplicatePropertyContext<TResponse>(response));
            var value = redactedDocument.RootElement.Deserialize<TResponse>(serializerOptions);
            return value
                ?? throw new JsonException(
                    $"Torii response for `{response.RequestMessage?.RequestUri}` deserialized to null.");
        }
    }

    private static JsonDocument RedactAccountOnboardingJson(
        JsonElement root,
        string exactOnboardingToken)
    {
        using var buffer = new MemoryStream();
        using (var writer = new Utf8JsonWriter(buffer))
        {
            WriteRedactedAccountOnboardingJson(writer, root, exactOnboardingToken);
        }
        return JsonDocument.Parse(buffer.ToArray(), new JsonDocumentOptions { MaxDepth = 128 });
    }

    private static void WriteRedactedAccountOnboardingJson(
        Utf8JsonWriter writer,
        JsonElement element,
        string exactOnboardingToken)
    {
        switch (element.ValueKind)
        {
            case JsonValueKind.Object:
                writer.WriteStartObject();
                foreach (var property in element.EnumerateObject())
                {
                    writer.WritePropertyName(
                        RedactAccountOnboardingCredential(property.Name, exactOnboardingToken)!);
                    WriteRedactedAccountOnboardingJson(
                        writer,
                        property.Value,
                        exactOnboardingToken);
                }
                writer.WriteEndObject();
                break;
            case JsonValueKind.Array:
                writer.WriteStartArray();
                foreach (var item in element.EnumerateArray())
                {
                    WriteRedactedAccountOnboardingJson(writer, item, exactOnboardingToken);
                }
                writer.WriteEndArray();
                break;
            case JsonValueKind.String:
                writer.WriteStringValue(
                    RedactAccountOnboardingCredential(element.GetString(), exactOnboardingToken));
                break;
            default:
                element.WriteTo(writer);
                break;
        }
    }

    public async Task<string> GetHealthAsync(CancellationToken cancellationToken = default)
    {
        using var response = await SendAsync(HttpMethod.Get, "/v1/health", cancellationToken: cancellationToken);
        return await ReadStrictUtf8TextContentAsync(response.Content, "Torii health response body", cancellationToken);
    }

    public async Task<ToriiAccountsPage> GetAccountsAsync(
        int? limit = null,
        long offset = 0,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiAccountsPage>(
            "/v1/accounts",
            BuildPaginationQuery(limit, offset),
            cancellationToken);
        ValidateAccountsPage(response, "accounts response");
        return response;
    }

    public async Task<ToriiExplorerAccountQrSnapshot> GetExplorerAccountQrAsync(
        string accountId,
        CancellationToken cancellationToken = default)
    {
        var encodedAccountId = EncodeIdentifierPathSegment(accountId, nameof(accountId));
        var response = await GetAsync<ToriiExplorerAccountQrSnapshot>(
            $"/v1/explorer/accounts/{encodedAccountId}/qr",
            cancellationToken: cancellationToken);
        ValidateExplorerAccountQrSnapshot(response, "explorer account QR response");
        return response;
    }

    public async Task<ToriiExplorerAccountsPage> GetExplorerAccountsAsync(
        ToriiExplorerAccountsQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerAccountsPage>(
            "/v1/explorer/accounts",
            BuildExplorerAccountsQuery(query),
            cancellationToken);
        ValidateExplorerAccountsPage(response, "explorer accounts response");
        return response;
    }

    public async Task<ToriiExplorerAccount> GetExplorerAccountAsync(
        string accountId,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerAccount>(
            $"/v1/explorer/accounts/{EncodeIdentifierPathSegment(accountId, nameof(accountId))}",
            cancellationToken: cancellationToken);
        ValidateExplorerAccount(response, "explorer account response");
        return response;
    }

    public async Task<ToriiExplorerDomainsPage> GetExplorerDomainsAsync(
        ToriiExplorerDomainsQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerDomainsPage>(
            "/v1/explorer/domains",
            BuildExplorerDomainsQuery(query),
            cancellationToken);
        ValidateExplorerDomainsPage(response, "explorer domains response");
        return response;
    }

    public async Task<ToriiExplorerDomain> GetExplorerDomainAsync(
        string domainId,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerDomain>(
            $"/v1/explorer/domains/{EncodeIdentifierPathSegment(domainId, nameof(domainId))}",
            cancellationToken: cancellationToken);
        ValidateExplorerDomain(response, "explorer domain response");
        return response;
    }

    public async Task<ToriiExplorerAssetDefinitionsPage> GetExplorerAssetDefinitionsAsync(
        ToriiExplorerAssetDefinitionsQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerAssetDefinitionsPage>(
            "/v1/explorer/asset-definitions",
            BuildExplorerAssetDefinitionsQuery(query),
            cancellationToken);
        ValidateExplorerAssetDefinitionsPage(response, "explorer asset definitions response");
        return response;
    }

    public async Task<ToriiExplorerAssetDefinition> GetExplorerAssetDefinitionAsync(
        string definitionId,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerAssetDefinition>(
            $"/v1/explorer/asset-definitions/{EncodeIdentifierPathSegment(definitionId, nameof(definitionId))}",
            cancellationToken: cancellationToken);
        ValidateExplorerAssetDefinition(response, "explorer asset definition response");
        return response;
    }

    public async Task<ToriiExplorerAssetDefinitionEconometrics> GetExplorerAssetDefinitionEconometricsAsync(
        string definitionId,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerAssetDefinitionEconometrics>(
            $"/v1/explorer/asset-definitions/{EncodeIdentifierPathSegment(definitionId, nameof(definitionId))}/econometrics",
            cancellationToken: cancellationToken);
        ValidateExplorerAssetDefinitionEconometrics(response, "explorer asset definition econometrics response");
        return response;
    }

    public async Task<ToriiExplorerAssetDefinitionSnapshot> GetExplorerAssetDefinitionSnapshotAsync(
        string definitionId,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerAssetDefinitionSnapshot>(
            $"/v1/explorer/asset-definitions/{EncodeIdentifierPathSegment(definitionId, nameof(definitionId))}/snapshot",
            cancellationToken: cancellationToken);
        ValidateExplorerAssetDefinitionSnapshot(response, "explorer asset definition snapshot response");
        return response;
    }

    public async Task<ToriiExplorerAssetsPage> GetExplorerAssetsAsync(
        ToriiExplorerAssetsQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerAssetsPage>(
            "/v1/explorer/assets",
            BuildExplorerAssetsQuery(query),
            cancellationToken);
        ValidateExplorerAssetsPage(response, "explorer assets response");
        return response;
    }

    public async Task<ToriiExplorerAsset> GetExplorerAssetAsync(
        string assetId,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerAsset>(
            $"/v1/explorer/assets/{EncodeIdentifierPathSegment(assetId, nameof(assetId))}",
            cancellationToken: cancellationToken);
        ValidateExplorerAsset(response, "explorer asset response");
        return response;
    }

    public async Task<ToriiExplorerNftsPage> GetExplorerNftsAsync(
        ToriiExplorerNftsQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerNftsPage>(
            "/v1/explorer/nfts",
            BuildExplorerNftsQuery(query),
            cancellationToken);
        ValidateExplorerNftsPage(response, "explorer nfts response");
        return response;
    }

    public async Task<ToriiExplorerNft> GetExplorerNftAsync(
        string nftId,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerNft>(
            $"/v1/explorer/nfts/{EncodeIdentifierPathSegment(nftId, nameof(nftId))}",
            cancellationToken: cancellationToken);
        ValidateExplorerNft(response, "explorer nft response");
        return response;
    }

    public async Task<ToriiExplorerRwasPage> GetExplorerRwasAsync(
        ToriiExplorerRwasQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerRwasPage>(
            "/v1/explorer/rwas",
            BuildExplorerRwasQuery(query),
            cancellationToken);
        ValidateExplorerRwasPage(response, "explorer rwas response");
        return response;
    }

    public async Task<ToriiExplorerRwa> GetExplorerRwaAsync(
        string rwaId,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerRwa>(
            $"/v1/explorer/rwas/{EncodeIdentifierPathSegment(rwaId, nameof(rwaId))}",
            cancellationToken: cancellationToken);
        ValidateExplorerRwa(response, "explorer rwa response");
        return response;
    }

    public async Task<ToriiExplorerBlocksPage> GetExplorerBlocksAsync(
        ToriiExplorerPaginationQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerBlocksPage>(
            "/v1/explorer/blocks",
            BuildExplorerPaginationQuery(query),
            cancellationToken);
        ValidateExplorerBlocksPage(response, "explorer blocks response");
        return response;
    }

    public async Task<ToriiExplorerBlock> GetExplorerBlockAsync(
        string identifier,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerBlock>(
            $"/v1/explorer/blocks/{EncodeIdentifierPathSegment(identifier, nameof(identifier))}",
            cancellationToken: cancellationToken);
        ValidateExplorerBlock(response, "explorer block response");
        return response;
    }

    public async Task<ToriiExplorerTransactionsPage> GetExplorerTransactionsAsync(
        ToriiExplorerTransactionsQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerTransactionsPage>(
            "/v1/explorer/transactions",
            BuildExplorerTransactionsQuery(query),
            cancellationToken);
        ValidateExplorerTransactionsPage(response, "explorer transactions response");
        return response;
    }

    public async Task<ToriiExplorerLatestTransactionsResponse> GetExplorerLatestTransactionsAsync(
        ToriiExplorerTransactionsQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerLatestTransactionsResponse>(
            "/v1/explorer/transactions/latest",
            BuildExplorerTransactionsQuery(query),
            cancellationToken);
        ValidateExplorerLatestTransactionsResponse(response, "explorer latest transactions response");
        return response;
    }

    public async Task<ToriiExplorerTransactionDetail> GetExplorerTransactionAsync(
        string transactionHash,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerTransactionDetail>(
            $"/v1/explorer/transactions/{EncodeIdentifierPathSegment(transactionHash, nameof(transactionHash))}",
            cancellationToken: cancellationToken);
        ValidateExplorerTransactionDetail(response, "explorer transaction response");
        return response;
    }

    public async Task<ToriiExplorerInstructionsPage> GetExplorerInstructionsAsync(
        ToriiExplorerInstructionsQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerInstructionsPage>(
            "/v1/explorer/instructions",
            BuildExplorerInstructionsQuery(query),
            cancellationToken);
        ValidateExplorerInstructionsPage(response, "explorer instructions response");
        return response;
    }

    public async Task<ToriiExplorerLatestInstructionsResponse> GetExplorerLatestInstructionsAsync(
        ToriiExplorerInstructionsQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerLatestInstructionsResponse>(
            "/v1/explorer/instructions/latest",
            BuildExplorerInstructionsQuery(query),
            cancellationToken);
        ValidateExplorerLatestInstructionsResponse(response, "explorer latest instructions response");
        return response;
    }

    public async Task<ToriiExplorerInstruction> GetExplorerInstructionAsync(
        string transactionHash,
        ulong index,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerInstruction>(
            $"/v1/explorer/instructions/{EncodeIdentifierPathSegment(transactionHash, nameof(transactionHash))}/{index.ToString(CultureInfo.InvariantCulture)}",
            cancellationToken: cancellationToken);
        ValidateExplorerInstruction(response, "explorer instruction response");
        return response;
    }

    public async Task<ToriiContractCodeView> GetExplorerInstructionContractViewAsync(
        string transactionHash,
        ulong index,
        CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiContractCodeView>(
            $"/v1/explorer/instructions/{EncodeIdentifierPathSegment(transactionHash, nameof(transactionHash))}/{index.ToString(CultureInfo.InvariantCulture)}/contract-view",
            cancellationToken: cancellationToken);
        ValidateContractCodeView(response, "explorer instruction contract-view response");
        return response;
    }

    public async Task<ToriiExplorerHealthSnapshot> GetExplorerHealthAsync(CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerHealthSnapshot>(
            "/v1/explorer/health",
            cancellationToken: cancellationToken);
        ValidateExplorerHealthSnapshot(response, "explorer health response");
        return response;
    }

    public async Task<ToriiExplorerMetricsSnapshot> GetExplorerMetricsAsync(CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiExplorerMetricsSnapshot>(
            "/v1/explorer/metrics",
            cancellationToken: cancellationToken);
        ValidateExplorerMetricsSnapshot(response, "explorer metrics response");
        return response;
    }

    public async Task<ToriiAssetBalancesPage> GetAccountAssetsAsync(
        string accountId,
        int? limit = null,
        long offset = 0,
        string? asset = null,
        string? scope = null,
        CancellationToken cancellationToken = default)
    {
        var encodedAccountId = EncodeAccountIdPathSegment(accountId, nameof(accountId));
        var response = await GetAsync<ToriiAssetBalancesPage>(
            $"/v1/accounts/{encodedAccountId}/assets",
            BuildPaginationQuery(
                limit,
                offset,
                new KeyValuePair<string, string?>("asset", NormalizeOptionalExactValue(asset, nameof(asset))),
                new KeyValuePair<string, string?>("scope", NormalizeOptionalExactValue(scope, nameof(scope)))),
            cancellationToken);
        ValidateAccountAssetBalancesPage(response, "account assets response");
        return response;
    }

    public async Task<ToriiTransactionsPage> GetAccountTransactionsAsync(
        string accountId,
        int? limit = null,
        long offset = 0,
        string? assetId = null,
        CancellationToken cancellationToken = default)
    {
        var encodedAccountId = EncodeAccountIdPathSegment(accountId, nameof(accountId));
        var response = await GetAsync<ToriiTransactionsPage>(
            $"/v1/accounts/{encodedAccountId}/transactions",
            BuildPaginationQuery(
                limit,
                offset,
                new KeyValuePair<string, string?>("asset_id", NormalizeOptionalExactValue(assetId, nameof(assetId)))),
            cancellationToken);
        ValidateAccountTransactionsPage(response, "account transactions response");
        return response;
    }

    public async Task<ToriiAccountPermissionsPage> GetAccountPermissionsAsync(
        string accountId,
        int? limit = null,
        long offset = 0,
        CancellationToken cancellationToken = default)
    {
        var encodedAccountId = EncodeAccountIdPathSegment(accountId, nameof(accountId));
        var response = await GetAsync<ToriiAccountPermissionsPage>(
            $"/v1/accounts/{encodedAccountId}/permissions",
            BuildPaginationQuery(limit, offset),
            cancellationToken);
        ValidateAccountPermissionsPage(response, "account permissions response");
        return response;
    }

    public async Task<ToriiAccountAliasLookupResponse?> LookupAliasesByAccountAsync(
        string accountId,
        string? dataspace = null,
        string? domain = null,
        CancellationToken cancellationToken = default)
    {
        var response = await PostOptionalAsync<ToriiAccountAliasLookupRequest, ToriiAccountAliasLookupResponse>(
            "/v1/aliases/by-account",
            new ToriiAccountAliasLookupRequest
            {
                AccountId = ToriiAccountFaucetPow.RequireExactAccountId(accountId, nameof(accountId)),
                Dataspace = NormalizeOptionalExactValue(dataspace, nameof(dataspace)),
                Domain = NormalizeOptionalExactValue(domain, nameof(domain)),
            },
            cancellationToken);
        if (response is not null)
        {
            ValidateAccountAliasLookupResponse(response, "account alias lookup response");
        }

        return response;
    }

    public async Task<ToriiIdentifierPoliciesResponse> GetIdentifierPoliciesAsync(CancellationToken cancellationToken = default)
    {
        ToriiIdentifierPoliciesResponse response;
        try
        {
            response = await GetAsync<ToriiIdentifierPoliciesResponse>(
                "/v1/identifier-policies",
                cancellationToken: cancellationToken);
        }
        catch (JsonException exception)
        {
            throw RewriteIdentifierPoliciesJsonException(exception);
        }

        ValidateIdentifierPoliciesResponse(response);
        return response;
    }

    public async Task<ToriiIdentifierResolveResponse> ResolveIdentifierAsync(
        ToriiIdentifierResolveRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);
        ValidateIdentifierResolveRequest(request);
        var normalizedRequest = request with
        {
            PolicyId = NormalizeIdentifierPolicyId(request.PolicyId, nameof(request.PolicyId)),
            EncryptedInput = NormalizeOptionalIdentifierCiphertext(
                request.EncryptedInput,
                nameof(request.EncryptedInput)),
        };

        ToriiIdentifierResolveResponse response;
        try
        {
            response = await PostAsync<ToriiIdentifierResolveRequest, ToriiIdentifierResolveResponse>(
                "/v1/identifiers/resolve",
                normalizedRequest,
                cancellationToken: cancellationToken);
        }
        catch (JsonException exception)
        {
            throw RewriteIdentifierResolveJsonException(exception);
        }

        ValidateIdentifierResolveResponse(response);
        return response;
    }

    public async Task<ToriiAccountAliasIndexResolution?> ResolveAccountAliasIndexAsync(
        ulong index,
        CancellationToken cancellationToken = default)
    {
        var response = await PostOptionalAsync<ToriiAliasResolveIndexRequest, ToriiAccountAliasIndexResolution>(
            "/v1/aliases/resolve-index",
            new ToriiAliasResolveIndexRequest
            {
                Index = index,
            },
            cancellationToken);
        if (response is not null)
        {
            ValidateAccountAliasIndexResolution(response, "account alias index response");
        }

        return response;
    }

    public async Task<ToriiUaidPortfolioResponse> GetUaidPortfolioAsync(
        string uaid,
        ToriiUaidPortfolioQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        var normalizedUaid = NormalizeUaidLiteral(uaid);
        var response = await GetAsync<ToriiUaidPortfolioResponse>(
            $"/v1/accounts/{EncodePathSegment(normalizedUaid)}/portfolio",
            BuildQueryString([
                new KeyValuePair<string, string?>(
                    "asset_id",
                    NormalizeOptionalExactValue(
                        query?.AssetId,
                        nameof(ToriiUaidPortfolioQuery.AssetId))),
            ]),
            cancellationToken);
        ValidateUaidPortfolioResponse(response, "UAID portfolio response");
        return response;
    }

    public async Task<ToriiUaidBindingsResponse> GetUaidBindingsAsync(
        string uaid,
        CancellationToken cancellationToken = default)
    {
        var normalizedUaid = NormalizeUaidLiteral(uaid);
        var response = await GetAsync<ToriiUaidBindingsResponse>(
            $"/v1/space-directory/uaids/{EncodePathSegment(normalizedUaid)}",
            cancellationToken: cancellationToken);
        ValidateUaidBindingsResponse(response, "UAID bindings response");
        return response;
    }

    public async Task<ToriiUaidManifestsResponse> GetUaidManifestsAsync(
        string uaid,
        ToriiUaidManifestQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        var normalizedUaid = NormalizeUaidLiteral(uaid);
        var response = await GetAsync<ToriiUaidManifestsResponse>(
            $"/v1/space-directory/uaids/{EncodePathSegment(normalizedUaid)}/manifests",
            BuildUaidManifestQuery(query),
            cancellationToken);
        ValidateUaidManifestsResponse(response, "UAID manifests response");
        return response;
    }

    public async Task<ToriiAccountOnboardingPlanReceipt> PlanAccountOnboardingAsync(
        ToriiAccountOnboardingPlanRequest request,
        string onboardingToken,
        string expectedAuthority,
        string expectedChainId,
        ToriiAccountOnboardingPlanBodyEncoder canonicalBodyEncoder,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);
        var exactOnboardingToken = RequireAccountOnboardingToken(onboardingToken);
        var normalizedRequest = NormalizeAccountOnboardingPlanRequest(request);

        var receipt = await PostAccountOnboardingAsync<ToriiAccountOnboardingPlanRequest, ToriiAccountOnboardingPlanReceipt>(
            "/v1/accounts/onboard/plan",
            normalizedRequest,
            exactOnboardingToken,
            cancellationToken: cancellationToken);

        ToriiAccountOnboardingReceiptVerifier.RequirePinned(
            receipt,
            expectedAuthority,
            expectedChainId,
            canonicalBodyEncoder);
        RequireMatchingAccountOnboardingRequest(
            normalizedRequest,
            receipt.Body.Request,
            nameof(receipt));
        return receipt;
    }

    public async Task<ToriiAccountOnboardingResponse> ApplyAccountOnboardingAsync(
        ToriiAccountOnboardingPlanReceipt receipt,
        string onboardingToken,
        string expectedAuthority,
        string expectedChainId,
        ToriiAccountOnboardingPlanBodyEncoder canonicalBodyEncoder,
        CancellationToken cancellationToken = default)
    {
        ToriiAccountOnboardingReceiptVerifier.RequirePinned(
            receipt,
            expectedAuthority,
            expectedChainId,
            canonicalBodyEncoder);
        var exactOnboardingToken = RequireAccountOnboardingToken(onboardingToken);
        var response = await PostAccountOnboardingAsync<ToriiAccountOnboardingApplyRequest, ToriiAccountOnboardingResponse>(
            "/v1/accounts/onboard",
            new ToriiAccountOnboardingApplyRequest { Receipt = receipt },
            exactOnboardingToken,
            cancellationToken: cancellationToken);

        ValidateAccountOnboardingResponse(response, "account onboarding response");
        if (!string.Equals(response.AccountId, receipt.Body.Request.AccountId, StringComparison.Ordinal)
            || !string.Equals(response.Alias, receipt.Body.Request.Alias, StringComparison.Ordinal))
        {
            throw new JsonException("account onboarding response does not match the pinned receipt intent");
        }
        return response;
    }

    public async Task<ToriiAccountFaucetPuzzle> GetAccountFaucetPuzzleAsync(CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiAccountFaucetPuzzle>(
            "/v1/accounts/faucet/puzzle",
            cancellationToken: cancellationToken);
        ValidateAccountFaucetPuzzle(response, "account faucet puzzle response");
        return response;
    }

    public async Task<ToriiAccountFaucetSolution> SolveAccountFaucetAsync(
        string accountId,
        ToriiAccountFaucetSolveOptions? solveOptions = null,
        CancellationToken cancellationToken = default)
    {
        var puzzle = await GetAccountFaucetPuzzleAsync(cancellationToken);
        return ToriiAccountFaucetPow.Solve(accountId, puzzle, solveOptions, cancellationToken);
    }

    public async Task<ToriiAccountFaucetResponse> ClaimAccountFaucetAsync(
        ToriiAccountFaucetRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);
        var normalizedRequest = NormalizeAccountFaucetRequest(request);

        var response = await PostAsync<ToriiAccountFaucetRequest, ToriiAccountFaucetResponse>(
            "/v1/accounts/faucet",
            normalizedRequest,
            cancellationToken: cancellationToken);

        ValidateAccountFaucetResponse(response, "account faucet response");
        return response;
    }

    public async Task<ToriiAccountFaucetResponse> ClaimAccountFaucetAsync(
        string accountId,
        ToriiAccountFaucetSolveOptions? solveOptions = null,
        CancellationToken cancellationToken = default)
    {
        var prepared = await SolveAccountFaucetAsync(accountId, solveOptions, cancellationToken);
        return await ClaimAccountFaucetAsync(prepared.ToRequest(), cancellationToken);
    }

    public async Task<ToriiVpnProfile> GetVpnProfileAsync(CancellationToken cancellationToken = default)
    {
        RequireSecureVpnTransport();
        using var response = await SendExpectingStatusAsync(
            HttpMethod.Get,
            "/v1/vpn/profile",
            query: null,
            content: null,
            HttpStatusCode.OK,
            allowedStatusCode: null,
            cancellationToken);
        var profile = await DeserializeAsync<ToriiVpnProfile>(response, cancellationToken);
        ValidateVpnProfile(profile, "vpn profile response");
        return profile;
    }

    public async Task<ToriiVpnQuote> CreateVpnQuoteAsync(
        ToriiVpnQuoteCreateRequest request,
        CancellationToken cancellationToken = default)
    {
        RequireVpnCanonicalRequestCredentials("/v1/vpn/quotes");
        ArgumentNullException.ThrowIfNull(request);
        var normalizedRequest = NormalizeVpnQuoteCreateRequest(request);

        using var content = CreateJsonContent(normalizedRequest);
        using var httpResponse = await SendExpectingStatusAsync(
            HttpMethod.Post,
            "/v1/vpn/quotes",
            query: null,
            content,
            HttpStatusCode.Created,
            allowedStatusCode: null,
            cancellationToken);
        var response = await DeserializeAsync<ToriiVpnQuote>(httpResponse, cancellationToken);
        ValidateVpnQuote(response, "vpn quote response");
        return response;
    }

    public async Task<ToriiVpnSession> CreateVpnSessionAsync(
        ToriiVpnSessionCreateRequest request,
        CancellationToken cancellationToken = default)
    {
        RequireVpnCanonicalRequestCredentials("/v1/vpn/sessions");
        ArgumentNullException.ThrowIfNull(request);
        var normalizedRequest = NormalizeVpnSessionCreateRequest(request);

        using var content = CreateJsonContent(normalizedRequest);
        using var httpResponse = await SendExpectingStatusAsync(
            HttpMethod.Post,
            "/v1/vpn/sessions",
            query: null,
            content,
            HttpStatusCode.Created,
            allowedStatusCode: null,
            cancellationToken);
        var response = await DeserializeAsync<ToriiVpnSession>(httpResponse, cancellationToken);
        ValidateVpnSession(response, "vpn session response");
        return response;
    }

    public async Task<ToriiVpnSession?> GetVpnSessionAsync(
        string sessionId,
        CancellationToken cancellationToken = default)
    {
        RequireVpnCanonicalRequestCredentials("/v1/vpn/sessions/{session_id}");
        var encodedSessionId = EncodeVpnSessionPathSegment(sessionId, nameof(sessionId));
        using var response = await SendExpectingStatusAsync(
            HttpMethod.Get,
            $"/v1/vpn/sessions/{encodedSessionId}",
            query: null,
            content: null,
            HttpStatusCode.OK,
            HttpStatusCode.NotFound,
            cancellationToken);

        if (response.StatusCode == HttpStatusCode.NotFound)
        {
            return null;
        }

        var session = await DeserializeAsync<ToriiVpnSession>(response, cancellationToken);
        ValidateVpnSession(session, "vpn session response");
        return session;
    }

    public async Task<ToriiVpnReceipt?> DeleteVpnSessionAsync(
        string sessionId,
        CancellationToken cancellationToken = default)
    {
        RequireVpnCanonicalRequestCredentials("/v1/vpn/sessions/{session_id}");
        var encodedSessionId = EncodeVpnSessionPathSegment(sessionId, nameof(sessionId));
        using var response = await SendExpectingStatusAsync(
            HttpMethod.Delete,
            $"/v1/vpn/sessions/{encodedSessionId}",
            query: null,
            content: null,
            HttpStatusCode.OK,
            HttpStatusCode.NotFound,
            cancellationToken);

        if (response.StatusCode == HttpStatusCode.NotFound)
        {
            return null;
        }

        var receipt = await DeserializeAsync<ToriiVpnReceipt>(response, cancellationToken);
        ValidateVpnReceipt(receipt, "vpn receipt response");
        return receipt;
    }

    public async Task<ToriiVpnReceipt> SubmitVpnReceiptAsync(
        ToriiVpnReceiptSubmitRequest request,
        CancellationToken cancellationToken = default)
    {
        RequireVpnCanonicalRequestCredentials("/v1/vpn/receipts");
        ArgumentNullException.ThrowIfNull(request);
        var normalizedRequest = NormalizeVpnReceiptSubmitRequest(request);

        using var content = CreateJsonContent(normalizedRequest);
        using var httpResponse = await SendExpectingStatusAsync(
            HttpMethod.Post,
            "/v1/vpn/receipts",
            query: null,
            content,
            HttpStatusCode.Created,
            allowedStatusCode: null,
            cancellationToken);
        var response = await DeserializeAsync<ToriiVpnReceipt>(httpResponse, cancellationToken);
        ValidateVpnReceipt(response, "vpn receipt response");
        return response;
    }

    public async Task<ToriiVpnReceiptListResponse> ListVpnReceiptsAsync(CancellationToken cancellationToken = default)
    {
        RequireVpnCanonicalRequestCredentials("/v1/vpn/receipts");
        using var httpResponse = await SendExpectingStatusAsync(
            HttpMethod.Get,
            "/v1/vpn/receipts",
            query: null,
            content: null,
            HttpStatusCode.OK,
            allowedStatusCode: null,
            cancellationToken);
        var response = await DeserializeAsync<ToriiVpnReceiptListResponse>(httpResponse, cancellationToken);
        ValidateVpnReceiptListResponse(response, "vpn receipt list response");
        return response;
    }

    public async Task<ToriiAssetAliasResolution?> ResolveAssetAliasAsync(string alias, CancellationToken cancellationToken = default)
    {
        var response = await PostOptionalAsync<ToriiAliasResolutionRequest, ToriiAssetAliasResolution>(
            "/v1/assets/aliases/resolve",
            new ToriiAliasResolutionRequest
            {
                Alias = NormalizeExactValue(alias, nameof(alias)),
            },
            cancellationToken);
        if (response is not null)
        {
            ValidateAssetAliasResolution(response, "asset alias resolution response");
        }

        return response;
    }

    public async Task<ToriiAccountAliasResolution?> ResolveAccountAliasAsync(string alias, CancellationToken cancellationToken = default)
    {
        var response = await PostOptionalAsync<ToriiAliasResolutionRequest, ToriiAccountAliasResolution>(
            "/v1/aliases/resolve",
            new ToriiAliasResolutionRequest
            {
                Alias = NormalizeExactValue(alias, nameof(alias)),
            },
            cancellationToken);
        if (response is not null)
        {
            ValidateAccountAliasResolution(response, "account alias resolution response");
        }

        return response;
    }

    public async Task<ToriiContractAliasResolution?> ResolveContractAliasAsync(
        string contractAlias,
        CancellationToken cancellationToken = default)
    {
        var response = await PostOptionalAsync<ToriiContractAliasResolutionRequest, ToriiContractAliasResolution>(
            "/v1/contracts/aliases/resolve",
            new ToriiContractAliasResolutionRequest
            {
                ContractAlias = NormalizeExactValue(contractAlias, nameof(contractAlias)),
            },
            cancellationToken);
        if (response is not null)
        {
            ValidateContractAliasResolution(response, "contract alias resolution response");
        }

        return response;
    }

    public async Task<ToriiContractCodeRecord> GetContractCodeAsync(
        string codeHash,
        CancellationToken cancellationToken = default)
    {
        var normalizedCodeHash = NormalizeExactSizedHex(codeHash, nameof(codeHash), 32);

        var response = await GetAsync<ToriiContractCodeRecord>(
            $"/v1/contracts/code/{EncodePathSegment(normalizedCodeHash)}",
            cancellationToken: cancellationToken);
        ValidateContractCodeRecord(response, "contract code response");
        return response;
    }

    public async Task<ToriiContractCodeBytesResponse> GetContractCodeBytesResponseAsync(
        string codeHash,
        CancellationToken cancellationToken = default)
    {
        var normalizedCodeHash = NormalizeExactSizedHex(codeHash, nameof(codeHash), 32);

        var response = await GetAsync<ToriiContractCodeBytesResponse>(
            $"/v1/contracts/code-bytes/{EncodePathSegment(normalizedCodeHash)}",
            cancellationToken: cancellationToken);

        _ = response.DecodeBytes();
        return response;
    }

    public async Task<byte[]> GetContractCodeBytesAsync(
        string codeHash,
        CancellationToken cancellationToken = default)
    {
        var response = await GetContractCodeBytesResponseAsync(codeHash, cancellationToken);
        return response.DecodeBytes();
    }

    public async Task<ToriiContractInstancesResponse> GetContractInstancesAsync(
        string namespaceId,
        ToriiContractInstancesQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        var normalizedNamespaceId = NormalizeExactValue(namespaceId, nameof(namespaceId));

        var response = await GetAsync<ToriiContractInstancesResponse>(
            $"/v1/contracts/instances/{EncodePathSegment(normalizedNamespaceId)}",
            BuildContractInstancesQuery(query),
            cancellationToken);
        ValidateContractInstancesResponse(response, "contract instances response");
        return response;
    }

    public async Task<ToriiContractStateResponse> GetContractStateAsync(
        ToriiContractStateQuery query,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(query);

        var response = await GetAsync<ToriiContractStateResponse>(
            "/v1/contracts/state",
            BuildContractStateQuery(query),
            cancellationToken);

        ValidateContractStateResponse(response, "contract state response");
        return response;
    }

    public async Task<ToriiContractCallResponse> CallContractAsync(
        ToriiContractCallRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);
        var normalizedRequest = NormalizeContractCallRequest(request);

        var response = await PostAsync<ToriiContractCallRequest, ToriiContractCallResponse>(
            "/v1/contracts/call",
            normalizedRequest,
            cancellationToken: cancellationToken);
        ValidateContractCallResponse(response, "contract call response");
        return response;
    }

    /// <summary>Quotes the exact unsigned transaction payload using account-signed Torii auth.</summary>
    public async Task<ToriiFeeQuoteResponse> QuoteFeesAsync(
        UnsignedTransactionPayload payload,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(payload);
        RequireCanonicalRequestCredentials("/v1/fees/quote");
        if (!string.Equals(
                Options.CanonicalRequestCredentials!.AccountId,
                payload.Authority,
                StringComparison.Ordinal))
        {
            throw new InvalidOperationException(
                "Canonical request account must equal the unsigned transaction authority.");
        }

        var response = await PostAsync<ToriiFeeQuoteRequest, ToriiFeeQuoteResponse>(
            "/v1/fees/quote",
            new ToriiFeeQuoteRequest { Payload = payload },
            cancellationToken: cancellationToken);
        ValidateFeeQuoteResponse(response, payload.FeePayment, "fee quote response");
        return response;
    }

    /// <summary>Looks up one exact on-chain sponsor program using account-signed Torii auth.</summary>
    public async Task<ToriiFeeSponsorProgram> GetFeeSponsorProgramAsync(
        FeeSponsorProgramId programId,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(programId);
        RequireCanonicalRequestCredentials("/v1/fee-sponsor-programs/by-id");
        var response = await PostAsync<ToriiFeeSponsorProgramLookupRequest, ToriiFeeSponsorProgram>(
            "/v1/fee-sponsor-programs/by-id",
            new ToriiFeeSponsorProgramLookupRequest { ProgramId = programId.ToString() },
            cancellationToken: cancellationToken);
        if (response.Id != programId)
        {
            throw new JsonException("Fee sponsor program lookup returned a different program id.");
        }
        return response;
    }

    public async Task<ToriiContractCodeView> GetContractCodeViewAsync(
        string codeHash,
        CancellationToken cancellationToken = default)
    {
        var normalizedCodeHash = NormalizeExactSizedHex(codeHash, nameof(codeHash), 32);

        var response = await GetAsync<ToriiContractCodeView>(
            $"/v1/contracts/code/{EncodePathSegment(normalizedCodeHash)}/contract-view",
            cancellationToken: cancellationToken);
        ValidateContractCodeView(response, "contract code-view response");
        return response;
    }

    public async Task<ToriiContractViewExecutionResult> ExecuteContractViewAsync(
        ToriiContractViewRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);
        var normalizedRequest = NormalizeContractViewRequest(request);

        using var content = CreateJsonContent(normalizedRequest);
        using var response = await SendAllowingStatusAsync(
            HttpMethod.Post,
            "/v1/contracts/view",
            query: null,
            content,
            HttpStatusCode.UnprocessableEntity,
            cancellationToken);

        if (response.StatusCode == HttpStatusCode.UnprocessableEntity)
        {
            var error = await DeserializeAsync<ToriiContractViewErrorResponse>(response, cancellationToken);
            ValidateContractViewErrorResponse(error, "contract view error response");
            return new ToriiContractViewExecutionResult
            {
                Error = error,
            };
        }

        var success = await DeserializeAsync<ToriiContractViewResponse>(response, cancellationToken);
        ValidateContractViewResponse(success, "contract view response");
        return new ToriiContractViewExecutionResult
        {
            Success = success,
        };
    }

    public async Task<ToriiMultisigContractCallResponse> ProposeMultisigContractCallAsync(
        ToriiMultisigContractCallProposeRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);
        var normalizedRequest = NormalizeMultisigContractCallProposeRequest(request);

        var response = await PostAsync<ToriiMultisigContractCallProposeRequest, ToriiMultisigContractCallResponse>(
            "/v1/contracts/call/multisig/propose",
            normalizedRequest,
            cancellationToken: cancellationToken);
        ValidateMultisigContractCallResponse(response, "multisig contract-call response");
        return response;
    }

    public async Task<ToriiMultisigResponse> ProposeMultisigAsync(
        ToriiMultisigProposeRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);
        var normalizedRequest = NormalizeMultisigProposeRequest(request);

        var response = await PostAsync<ToriiMultisigProposeRequest, ToriiMultisigResponse>(
            "/v1/multisig/propose",
            normalizedRequest,
            cancellationToken: cancellationToken);
        ValidateMultisigResponse(response, "multisig response");
        return response;
    }

    public async Task<ToriiMultisigResponse> ApproveMultisigAsync(
        ToriiMultisigApproveRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);
        var normalizedRequest = NormalizeMultisigApproveRequest(request);
        var response = await PostAsync<ToriiMultisigApproveRequest, ToriiMultisigResponse>(
            "/v1/multisig/approve",
            normalizedRequest,
            cancellationToken: cancellationToken);
        ValidateMultisigResponse(response, "multisig approval response");
        return response;
    }

    public async Task<ToriiMultisigCancelResponse> CancelMultisigAsync(
        ToriiMultisigCancelRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);
        var normalizedRequest = NormalizeMultisigCancelRequest(request);
        return await PostAsync<ToriiMultisigCancelRequest, ToriiMultisigCancelResponse>(
            "/v1/multisig/cancel",
            normalizedRequest,
            cancellationToken: cancellationToken);
    }

    public async Task<ToriiMultisigContractCallResponse> ApproveMultisigContractCallAsync(
        ToriiMultisigContractCallApproveRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);
        var normalizedRequest = NormalizeMultisigContractCallApproveRequest(request);

        var response = await PostAsync<ToriiMultisigContractCallApproveRequest, ToriiMultisigContractCallResponse>(
            "/v1/contracts/call/multisig/approve",
            normalizedRequest,
            cancellationToken: cancellationToken);
        ValidateMultisigContractCallResponse(response, "multisig contract-call response");
        return response;
    }

    public async Task<ToriiContractVerifiedSourceJob> SubmitContractVerifiedSourceJobAsync(
        string codeHash,
        ToriiContractVerifiedSourceSubmission request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);
        var normalizedCodeHash = NormalizeExactSizedHex(codeHash, nameof(codeHash), 32);
        var normalizedRequest = NormalizeContractVerifiedSourceSubmission(request);

        var response = await PostAsync<ToriiContractVerifiedSourceSubmission, ToriiContractVerifiedSourceJob>(
            $"/v1/contracts/code/{EncodePathSegment(normalizedCodeHash)}/verified-source/jobs",
            normalizedRequest,
            cancellationToken: cancellationToken);
        ValidateContractVerifiedSourceJob(response, "contract verified-source job response");
        return response;
    }

    public async Task<ToriiContractVerifiedSourceJob?> GetContractVerifiedSourceJobAsync(
        string codeHash,
        string jobId,
        CancellationToken cancellationToken = default)
    {
        var normalizedCodeHash = NormalizeExactSizedHex(codeHash, nameof(codeHash), 32);
        var normalizedJobId = NormalizeExactValue(jobId, nameof(jobId));

        using var response = await SendAllowingStatusAsync(
            HttpMethod.Get,
            $"/v1/contracts/code/{EncodePathSegment(normalizedCodeHash)}/verified-source-jobs/{EncodePathSegment(normalizedJobId)}",
            query: null,
            content: null,
            HttpStatusCode.NotFound,
            cancellationToken);

        if (response.StatusCode == HttpStatusCode.NotFound)
        {
            return null;
        }

        var job = await DeserializeAsync<ToriiContractVerifiedSourceJob>(response, cancellationToken);
        ValidateContractVerifiedSourceJob(job, "contract verified-source job response");
        return job;
    }

    public async Task<string> GetMetricsAsync(CancellationToken cancellationToken = default)
    {
        using var response = await SendAsync(HttpMethod.Get, "/v1/metrics", cancellationToken: cancellationToken);
        return await ReadStrictUtf8TextContentAsync(response.Content, "Torii metrics response body", cancellationToken);
    }

    public async Task<ToriiRuntimeAbiActive> GetRuntimeAbiActiveAsync(CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiRuntimeAbiActive>("/v1/runtime/abi/active", cancellationToken: cancellationToken);
        ValidateRuntimeAbiActive(response, "runtime ABI active response");
        return response;
    }

    public async Task<ToriiRuntimeAbiHash> GetRuntimeAbiHashAsync(CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiRuntimeAbiHash>(
            "/v1/runtime/abi/hash",
            cancellationToken: cancellationToken);
        ValidateRuntimeAbiHash(response, "runtime ABI hash response");
        return response;
    }

    public async Task<ToriiRuntimeMetrics> GetRuntimeMetricsAsync(CancellationToken cancellationToken = default)
    {
        var response = await GetAsync<ToriiRuntimeMetrics>("/v1/runtime/metrics", cancellationToken: cancellationToken);
        ValidateRuntimeMetrics(response, "runtime metrics response");
        return response;
    }


    public async Task<ToriiSoraFsCidLookupResponse> GetSoraFsCidLookupAsync(
        string cid,
        CancellationToken cancellationToken = default)
    {
        var encodedCid = EncodeSoraFsCidPathSegment(cid, nameof(cid));
        var response = await GetAsync<ToriiSoraFsCidLookupResponse>(
            $"/v1/sorafs/cid/{encodedCid}",
            cancellationToken: cancellationToken);
        ValidateSoraFsCidLookupResponse(response, "SoraFS CID lookup response");
        return response;
    }

    public async Task<ToriiSoraFsPinRegisterResponse> RegisterSoraFsPinManifestAsync(
        SignedTransactionEnvelope transaction,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(transaction);
        using var content = CreateBinaryContent(
            NormalizeNonEmptyBinaryPayload(transaction.NoritoBytes, nameof(transaction)),
            "application/x-norito");
        using var response = await SendAsync(
            HttpMethod.Post,
            "/v1/sorafs/pin/register",
            content: content,
            accept: "application/json",
            cancellationToken: cancellationToken);
        if (response.StatusCode != HttpStatusCode.Accepted)
        {
            throw new HttpRequestException(
                $"SoraFS pin registration must return HTTP 202, got {(int)response.StatusCode}.");
        }

        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken);
        using var document = await ParseJsonDocumentRejectingDuplicatePropertiesAsync(
            stream,
            "SoraFS pin registration response",
            cancellationToken);
        var root = document.RootElement;
        if (root.ValueKind != JsonValueKind.Object)
        {
            throw new JsonException("SoraFS pin registration response must be an object.");
        }
        var fields = root.EnumerateObject().Select(property => property.Name).ToHashSet(
            StringComparer.Ordinal);
        if (!fields.SetEquals(["status", "tx_hash_hex", "manifest_digest_hex"]))
        {
            throw new JsonException(
                "SoraFS pin registration response must contain only status, tx_hash_hex, and manifest_digest_hex.");
        }

        var status = root.GetProperty("status").GetString();
        if (!string.Equals(status, "submitted", StringComparison.Ordinal))
        {
            throw new JsonException("SoraFS pin registration response status must be submitted.");
        }
        return new ToriiSoraFsPinRegisterResponse
        {
            Status = "submitted",
            TxHashHex = RequireCanonicalLowercaseSoraFsPinDigest(root, "tx_hash_hex"),
            ManifestDigestHex = RequireCanonicalLowercaseSoraFsPinDigest(
                root,
                "manifest_digest_hex"),
        };
    }

    private static string RequireCanonicalLowercaseSoraFsPinDigest(
        JsonElement root,
        string field)
    {
        var value = root.GetProperty(field).GetString();
        if (value is null
            || value.Length != 64
            || value.Any(character =>
                character is not (>= '0' and <= '9')
                and not (>= 'a' and <= 'f')))
        {
            throw new JsonException($"{field} must be exactly 64 lowercase hexadecimal characters.");
        }
        return value;
    }

    public Task<HttpResponseMessage> OpenSoraFsCidContentAsync(
        string cid,
        string? relativePath = null,
        CancellationToken cancellationToken = default)
    {
        var gatewayPath = BuildSoraFsCidGatewayPath(cid, relativePath);
        return SendAsync(HttpMethod.Get, gatewayPath, cancellationToken: cancellationToken);
    }

    public async Task<ToriiSoraFsContentResponse> GetSoraFsCidContentAsync(
        string cid,
        string? relativePath = null,
        CancellationToken cancellationToken = default)
    {
        using var response = await OpenSoraFsCidContentAsync(cid, relativePath, cancellationToken);
        var bytes = await response.Content.ReadAsByteArrayAsync(cancellationToken);
        var contentCid = ReadSoraFsContentCidHeader(response);
        return new ToriiSoraFsContentResponse
        {
            Bytes = bytes,
            ContentType = response.Content.Headers.ContentType?.ToString(),
            ContentLength = response.Content.Headers.ContentLength,
            ContentCid = contentCid,
        };
    }

    public async Task<JsonDocument> SubmitSignedQueryAsync(
        ReadOnlyMemory<byte> noritoVersionedBytes,
        string? query = null,
        CancellationToken cancellationToken = default)
    {
        var normalizedBytes = NormalizeNonEmptyBinaryPayload(noritoVersionedBytes, nameof(noritoVersionedBytes));
        using var content = CreateBinaryContent(normalizedBytes, "application/x-norito");
        using var response = await SendAsync(HttpMethod.Post, "/query", query, content, cancellationToken: cancellationToken);
        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken);
        return await ParseJsonDocumentRejectingDuplicatePropertiesAsync(
            stream,
            $"signed query response for `{response.RequestMessage?.RequestUri}`",
            cancellationToken);
    }

    public Task<JsonDocument> SubmitSignedQueryAsync(
        SignedQueryEnvelope signedQuery,
        string? query = null,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(signedQuery);
        return SubmitSignedQueryAsync(signedQuery.VersionedNoritoBytes, query, cancellationToken);
    }

    public Task<HttpResponseMessage> OpenEventSseAsync(
        string? query = null,
        CancellationToken cancellationToken = default)
    {
        return OpenLiveSseAsync(
            "/v1/events/sse",
            NormalizeEventSseQuery(query),
            cancellationToken: cancellationToken);
    }

    public async IAsyncEnumerable<ToriiServerSentEvent> StreamEventsAsync(
        string? query = null,
        [EnumeratorCancellation] CancellationToken cancellationToken = default)
    {
        using var response = await OpenEventSseAsync(query, cancellationToken);
        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken);

        await foreach (var sseEvent in ReadServerSentEventsAsync(stream, cancellationToken))
        {
            yield return sseEvent;
        }
    }

    public async IAsyncEnumerable<ToriiPipelineEvent> StreamPipelineEventsAsync(
        string? query = null,
        [EnumeratorCancellation] CancellationToken cancellationToken = default)
    {
        await foreach (var sseEvent in StreamEventsAsync(query, cancellationToken))
        {
            ThrowIfTerminalStreamError(sseEvent, "pipeline SSE payload");
            if (sseEvent.IsComment || sseEvent.RawData is null)
            {
                continue;
            }

            var payload = RequireSseJsonData(sseEvent, "pipeline SSE payload");
            if (!TryReadSseStringProperty(payload, "category", "pipeline SSE payload", out var category)
                || !string.Equals(category, "Pipeline", StringComparison.Ordinal))
            {
                continue;
            }

            var pipelineEvent = JsonSerializer.Deserialize(
                payload,
                ToriiJsonSerializerContext.Default.ToriiPipelineEvent);
            if (pipelineEvent is null)
            {
                throw new JsonException("pipeline SSE payload must not deserialize to null.");
            }

            pipelineEvent.LastEventId = sseEvent.Id;
            pipelineEvent.SseEventName = sseEvent.Event;
            pipelineEvent.RetryMilliseconds = sseEvent.RetryMilliseconds;
            ValidatePipelineEvent(pipelineEvent, "pipeline SSE payload");
            yield return pipelineEvent;
        }
    }

    public async IAsyncEnumerable<ToriiProofEvent> StreamProofEventsAsync(
        string? query = null,
        [EnumeratorCancellation] CancellationToken cancellationToken = default)
    {
        await foreach (var sseEvent in StreamEventsAsync(query, cancellationToken))
        {
            ThrowIfTerminalStreamError(sseEvent, "proof SSE payload");
            if (sseEvent.IsComment || sseEvent.RawData is null)
            {
                continue;
            }

            var payload = RequireSseJsonData(sseEvent, "proof SSE payload");
            if (!TryReadSseStringProperty(payload, "category", "proof SSE payload", out var category)
                || !string.Equals(category, "Data", StringComparison.Ordinal))
            {
                continue;
            }

            if (!TryReadSseStringProperty(payload, "event", "proof SSE payload", out var proofEventName))
            {
                ToriiPipelineEventJsonConverter.RequireRequiredString(proofEventName, "proof SSE payload.event");
            }

            ToriiSseEventJson.RequireExactTokenText(proofEventName, "proof SSE payload.event");
            var exactProofEventName = proofEventName
                ?? throw new JsonException("proof SSE payload.event must not be null.");
            if (!exactProofEventName.StartsWith("Proof", StringComparison.Ordinal))
            {
                continue;
            }

            var proofEvent = JsonSerializer.Deserialize(
                payload,
                ToriiJsonSerializerContext.Default.ToriiProofEvent);
            if (proofEvent is null)
            {
                throw new JsonException("proof SSE payload must not deserialize to null.");
            }

            proofEvent.LastEventId = sseEvent.Id;
            proofEvent.SseEventName = sseEvent.Event;
            proofEvent.RetryMilliseconds = sseEvent.RetryMilliseconds;
            ValidateProofEvent(proofEvent, "proof SSE payload");
            yield return proofEvent;
        }
    }

    public Task<HttpResponseMessage> OpenExplorerBlocksSseAsync(
        string? lastEventId = null,
        CancellationToken cancellationToken = default)
    {
        return OpenSseAsync(
            "/v1/explorer/blocks/stream",
            query: null,
            lastEventId: lastEventId,
            cancellationToken: cancellationToken);
    }

    public Task<HttpResponseMessage> OpenExplorerTransactionsSseAsync(
        string? lastEventId = null,
        CancellationToken cancellationToken = default)
    {
        return OpenSseAsync(
            "/v1/explorer/transactions/stream",
            query: null,
            lastEventId: lastEventId,
            cancellationToken: cancellationToken);
    }

    public Task<HttpResponseMessage> OpenExplorerInstructionsSseAsync(
        string? lastEventId = null,
        CancellationToken cancellationToken = default)
    {
        return OpenSseAsync(
            "/v1/explorer/instructions/stream",
            query: null,
            lastEventId: lastEventId,
            cancellationToken: cancellationToken);
    }

    public IAsyncEnumerable<ToriiExplorerBlock> StreamExplorerBlocksAsync(
        string? lastEventId = null,
        CancellationToken cancellationToken = default)
    {
        return StreamSsePayloadsAsync(
            "/v1/explorer/blocks/stream",
            ToriiJsonSerializerContext.Default.ToriiExplorerBlock,
            payload => ValidateExplorerBlock(payload, "explorer block SSE payload"),
            query: null,
            lastEventId: lastEventId,
            cancellationToken: cancellationToken);
    }

    public IAsyncEnumerable<ToriiExplorerTransaction> StreamExplorerTransactionsAsync(
        string? lastEventId = null,
        CancellationToken cancellationToken = default)
    {
        return StreamSsePayloadsAsync(
            "/v1/explorer/transactions/stream",
            ToriiJsonSerializerContext.Default.ToriiExplorerTransaction,
            payload => ValidateExplorerTransaction(payload, "explorer transaction SSE payload"),
            query: null,
            lastEventId: lastEventId,
            cancellationToken: cancellationToken);
    }

    public IAsyncEnumerable<ToriiExplorerInstruction> StreamExplorerInstructionsAsync(
        string? lastEventId = null,
        CancellationToken cancellationToken = default)
    {
        return StreamSsePayloadsAsync(
            "/v1/explorer/instructions/stream",
            ToriiJsonSerializerContext.Default.ToriiExplorerInstruction,
            payload => ValidateExplorerInstruction(payload, "explorer instruction SSE payload"),
            query: null,
            lastEventId: lastEventId,
            cancellationToken: cancellationToken);
    }

    public async Task SubmitTransactionAsync(
        SignedTransactionEnvelope transaction,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(transaction);
        await SubmitTransactionAsync(transaction.NoritoBytes, cancellationToken);
    }

    public async Task SubmitTransactionAsync(
        ReadOnlyMemory<byte> noritoBytes,
        CancellationToken cancellationToken = default)
    {
        var normalizedBytes = NormalizeNonEmptyBinaryPayload(noritoBytes, nameof(noritoBytes));
        await EnsureTransactionSubmissionCompatibilityAsync(cancellationToken);
        using var content = CreateBinaryContent(normalizedBytes, "application/x-norito");
        using var response = await SendAsync(HttpMethod.Post, "/transaction", content: content, cancellationToken: cancellationToken);
    }

    public async Task<PipelineTransactionStatus?> GetPipelineTransactionStatusAsync(
        string transactionHashHex,
        string scope = "auto",
        CancellationToken cancellationToken = default)
    {
        var normalizedHash = NormalizeTransactionHashHex(transactionHashHex);
        var normalizedScope = NormalizePipelineScope(scope);
        var query = BuildQueryString(
        [
            new KeyValuePair<string, string?>("hash", normalizedHash),
            new KeyValuePair<string, string?>("scope", normalizedScope),
        ]);

        using var response = await SendAllowingStatusAsync(
            HttpMethod.Get,
            "/v1/pipeline/transactions/status",
            query,
            content: null,
            HttpStatusCode.NotFound,
            cancellationToken);

        if (response.StatusCode == HttpStatusCode.NotFound)
        {
            return null;
        }

        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken);
        using var document = await ParseJsonDocumentRejectingDuplicatePropertiesAsync(
            stream,
            $"pipeline transaction status response for `{response.RequestMessage?.RequestUri}`",
            cancellationToken);
        return ParsePipelineTransactionStatus(document.RootElement, normalizedHash);
    }

    public async Task<JsonDocument> PostJsonDocumentAsync<TRequest>(
        string path,
        TRequest request,
        string? query = null,
        CancellationToken cancellationToken = default)
    {
        using var content = CreateJsonContent(request);
        using var response = await SendAsync(HttpMethod.Post, path, query, content, cancellationToken: cancellationToken);
        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken);
        return await ParseJsonDocumentRejectingDuplicatePropertiesAsync(
            stream,
            $"Torii JSON response for `{response.RequestMessage?.RequestUri}`",
            cancellationToken);
    }

    public async Task<HttpResponseMessage> SendAsync(
        HttpMethod method,
        string path,
        string? query = null,
        HttpContent? content = null,
        string? accept = null,
        Action<HttpRequestMessage>? configureRequest = null,
        CancellationToken cancellationToken = default)
    {
        var request = await CreateRequestAsync(method, path, query, content, accept, configureRequest, cancellationToken);
        var response = await HttpClient.SendAsync(request, HttpCompletionOption.ResponseHeadersRead, cancellationToken);
        if (response.IsSuccessStatusCode)
        {
            return response;
        }

        var exception = await CreateApiExceptionAsync(response, cancellationToken);
        response.Dispose();
        throw exception;
    }

    private async Task<TResponse?> PostOptionalAsync<TRequest, TResponse>(
        string path,
        TRequest request,
        CancellationToken cancellationToken)
    {
        using var content = CreateJsonContent(request);
        using var response = await SendAllowingStatusAsync(
            HttpMethod.Post,
            path,
            query: null,
            content,
            HttpStatusCode.NotFound,
            cancellationToken);

        if (response.StatusCode == HttpStatusCode.NotFound)
        {
            return default;
        }

        return await DeserializeAsync<TResponse>(response, cancellationToken);
    }

    private async Task<TResponse> SendAllowingStatusAndDeserializeAsync<TResponse>(
        HttpMethod method,
        string path,
        string? query,
        HttpContent? content,
        HttpStatusCode allowedStatusCode,
        CancellationToken cancellationToken)
    {
        using var response = await SendAllowingStatusAsync(
            method,
            path,
            query,
            content,
            allowedStatusCode,
            cancellationToken);

        return await DeserializeAsync<TResponse>(response, cancellationToken);
    }

    private async Task<HttpResponseMessage> SendAllowingStatusAsync(
        HttpMethod method,
        string path,
        string? query,
        HttpContent? content,
        HttpStatusCode allowedStatusCode,
        CancellationToken cancellationToken)
    {
        var request = await CreateRequestAsync(method, path, query, content, accept: null, configureRequest: null, cancellationToken);
        var response = await HttpClient.SendAsync(request, HttpCompletionOption.ResponseHeadersRead, cancellationToken);
        if (response.IsSuccessStatusCode || response.StatusCode == allowedStatusCode)
        {
            return response;
        }

        var exception = await CreateApiExceptionAsync(response, cancellationToken);
        response.Dispose();
        throw exception;
    }

    private async Task<HttpResponseMessage> SendExpectingStatusAsync(
        HttpMethod method,
        string path,
        string? query,
        HttpContent? content,
        HttpStatusCode expectedStatusCode,
        HttpStatusCode? allowedStatusCode,
        CancellationToken cancellationToken)
    {
        var request = await CreateRequestAsync(
            method,
            path,
            query,
            content,
            accept: null,
            configureRequest: null,
            cancellationToken);
        var expectedRequestUri = request.RequestUri;
        var response = await HttpClient.SendAsync(request, HttpCompletionOption.ResponseHeadersRead, cancellationToken);
        if (path.StartsWith("/v1/vpn/", StringComparison.Ordinal)
            && response.RequestMessage?.RequestUri is Uri responseUri
            && expectedRequestUri is not null
            && responseUri != expectedRequestUri)
        {
            response.Dispose();
            throw new HttpRequestException("Sora VPN requests must not follow redirects.");
        }

        if (response.StatusCode == expectedStatusCode
            || (allowedStatusCode.HasValue && response.StatusCode == allowedStatusCode.Value))
        {
            return response;
        }

        var exception = await CreateApiExceptionAsync(response, cancellationToken);
        response.Dispose();
        throw exception;
    }

    private void RequireVpnCanonicalRequestCredentials(string route)
    {
        RequireSecureVpnTransport();
        if (Options.CanonicalRequestCredentials is null)
        {
            throw new InvalidOperationException(
                $"VPN route `{route}` requires ToriiClientOptions.CanonicalRequestCredentials.");
        }
    }

    private void RequireSecureVpnTransport()
    {
        if (!string.Equals(BaseUri.Scheme, Uri.UriSchemeHttps, StringComparison.OrdinalIgnoreCase))
        {
            throw new InvalidOperationException("Sora VPN requests require an HTTPS Torii base URI.");
        }
    }

    private void RequireCanonicalRequestCredentials(string route)
    {
        if (Options.CanonicalRequestCredentials is null)
        {
            throw new InvalidOperationException(
                $"Route `{route}` requires ToriiClientOptions.CanonicalRequestCredentials.");
        }
    }

    private async Task<HttpRequestMessage> CreateRequestAsync(
        HttpMethod method,
        string path,
        string? query,
        HttpContent? content,
        string? accept,
        Action<HttpRequestMessage>? configureRequest,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(method);
        var exactMethod = RequireExactRequestMethod(method.Method, nameof(method));
        var exactPath = RequireExactRequestPath(path, nameof(path));
        var exactQuery = NormalizeOptionalExactQuery(query, nameof(query));
        var exactAccept = NormalizeOptionalAcceptHeaderValue(accept, nameof(accept));

        var requestUri = BuildRequestUri(exactPath, exactQuery);
        var request = new HttpRequestMessage(method, requestUri)
        {
            Content = content,
        };

        var bearerToken = NormalizeOptionalBearerToken(Options.BearerToken, nameof(ToriiClientOptions.BearerToken));
        if (bearerToken is not null)
        {
            request.Headers.Authorization = new AuthenticationHeaderValue("Bearer", bearerToken);
        }

        if (!string.IsNullOrEmpty(exactAccept))
        {
            request.Headers.Accept.Add(new MediaTypeWithQualityHeaderValue(exactAccept));
        }

        byte[]? signedBodyBytes = null;
        IReadOnlyDictionary<string, string>? signedHeaders = null;
        if (Options.CanonicalRequestCredentials is not null)
        {
            var bodyBytes = content is null ? Array.Empty<byte>() : await content.ReadAsByteArrayAsync(cancellationToken);
            var headers = CanonicalRequest.BuildHeaders(
                Options.CanonicalRequestCredentials.AccountId,
                Options.CanonicalRequestCredentials.PrivateKeySeed,
                exactMethod,
                requestUri.AbsolutePath,
                requestUri.Query,
                bodyBytes);

            signedBodyBytes = bodyBytes;
            signedHeaders = headers.ToDictionary();
            foreach (var header in signedHeaders)
            {
                request.Headers.TryAddWithoutValidation(header.Key, header.Value);
            }
        }

        var protectedAuthorization = request.Headers.Authorization?.ToString();
        var protectedAccept = FormatAcceptHeaders(request);
        configureRequest?.Invoke(request);
        await EnsureConfigureRequestDidNotMutateProtectedStateAsync(
            request,
            exactMethod,
            requestUri,
            content,
            protectedAuthorization,
            protectedAccept,
            signedBodyBytes,
            signedHeaders,
            cancellationToken);
        return request;
    }

    private static async Task EnsureConfigureRequestDidNotMutateProtectedStateAsync(
        HttpRequestMessage request,
        string expectedMethod,
        Uri expectedUri,
        HttpContent? expectedContent,
        string? expectedAuthorization,
        string expectedAccept,
        byte[]? signedBodyBytes,
        IReadOnlyDictionary<string, string>? signedHeaders,
        CancellationToken cancellationToken)
    {
        if (!string.Equals(request.Method.Method, expectedMethod, StringComparison.Ordinal))
        {
            throw new ArgumentException("configureRequest must not mutate the HTTP method.", "configureRequest");
        }

        if (request.RequestUri is null
            || !string.Equals(request.RequestUri.AbsoluteUri, expectedUri.AbsoluteUri, StringComparison.Ordinal))
        {
            throw new ArgumentException("configureRequest must not mutate the request URI.", "configureRequest");
        }

        if (!ReferenceEquals(request.Content, expectedContent))
        {
            throw new ArgumentException("configureRequest must not replace request content.", "configureRequest");
        }

        if (!string.Equals(request.Headers.Authorization?.ToString(), expectedAuthorization, StringComparison.Ordinal))
        {
            throw new ArgumentException("configureRequest must not mutate the Authorization header.", "configureRequest");
        }

        if (!string.Equals(FormatAcceptHeaders(request), expectedAccept, StringComparison.Ordinal))
        {
            throw new ArgumentException("configureRequest must not mutate the Accept header.", "configureRequest");
        }

        if (signedBodyBytes is not null)
        {
            var currentBodyBytes = request.Content is null
                ? Array.Empty<byte>()
                : await request.Content.ReadAsByteArrayAsync(cancellationToken);
            if (!currentBodyBytes.SequenceEqual(signedBodyBytes))
            {
                throw new ArgumentException("configureRequest must not mutate signed request content.", "configureRequest");
            }
        }

        if (signedHeaders is not null)
        {
            foreach (var header in signedHeaders)
            {
                if (!request.Headers.TryGetValues(header.Key, out var values)
                    || !values.SequenceEqual([header.Value]))
                {
                    throw new ArgumentException("configureRequest must not mutate canonical signing headers.", "configureRequest");
                }
            }
        }
    }

    private static string FormatAcceptHeaders(HttpRequestMessage request)
        => string.Join(",", request.Headers.Accept.Select(static header => header.ToString()));

    private static string RequireExactRequestPart(string? value, string paramName)
    {
        if (string.IsNullOrEmpty(value))
        {
            throw new ArgumentException($"{paramName} must not be empty.", paramName);
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException($"{paramName} must not contain surrounding whitespace.", paramName);
        }

        if (value.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException($"{paramName} must not contain whitespace.", paramName);
        }

        if (value.Any(char.IsControl))
        {
            throw new ArgumentException($"{paramName} must not contain control characters.", paramName);
        }

        return value;
    }

    private static string RequireAccountOnboardingToken(string? onboardingToken)
    {
        ArgumentNullException.ThrowIfNull(onboardingToken);
        var bytes = Encoding.UTF8.GetBytes(onboardingToken);
        if (bytes.Length is < 32 or > 256
            || bytes.Any(static value => value is < 0x21 or > 0x7e))
        {
            throw new ArgumentException(
                "Account onboarding token must contain 32...256 printable ASCII bytes without spaces or normalization.",
                nameof(onboardingToken));
        }

        return onboardingToken;
    }

    private static string RequireExactRequestMethod(string? value, string paramName)
    {
        var exact = RequireExactRequestPart(value, paramName);
        if (!exact.All(IsHttpTokenCharacter))
        {
            throw new ArgumentException($"{paramName} must be an HTTP token.", paramName);
        }

        return exact;
    }

    private static bool IsHttpTokenCharacter(char value)
        => value is >= 'A' and <= 'Z'
            or >= 'a' and <= 'z'
            or >= '0' and <= '9'
            or '!' or '#' or '$' or '%' or '&' or '\'' or '*' or '+'
            or '-' or '.' or '^' or '_' or '`' or '|' or '~';

    private static string RequireExactRequestPath(string? value, string paramName)
    {
        var exact = RequireExactRequestPart(value, paramName);
        if (exact[0] != '/')
        {
            throw new ArgumentException($"{paramName} must be a root-relative path.", paramName);
        }

        if (exact.Length > 1 && exact[1] == '/')
        {
            throw new ArgumentException($"{paramName} must not be a scheme-relative URI.", paramName);
        }

        if (exact.Contains(':', StringComparison.Ordinal))
        {
            throw new ArgumentException($"{paramName} must not contain raw ':' characters.", paramName);
        }

        ValidateUriStableRequestPath(exact, paramName);
        return exact;
    }

    private static void ValidateUriStableRequestPath(string value, string paramName)
    {
        if (value.Contains('\\', StringComparison.Ordinal))
        {
            throw new ArgumentException($"{paramName} must not contain raw backslash characters.", paramName);
        }

        ValidatePathPercentEscapes(value, paramName);
        foreach (var segment in value.Split('/', StringSplitOptions.None))
        {
            if (segment.Length == 0)
            {
                continue;
            }

            var decodedSegment = DecodePercentEncodedPathSegment(segment, paramName);
            if (decodedSegment.Any(char.IsControl))
            {
                throw new ArgumentException($"{paramName} path segments must not contain percent-decoded control characters.", paramName);
            }

            if (decodedSegment is "." or "..")
            {
                throw new ArgumentException($"{paramName} must not contain dot path segments.", paramName);
            }
        }
    }

    private static string? NormalizeOptionalExactQuery(string? value, string paramName)
    {
        if (value is null || value.Length == 0)
        {
            return value;
        }

        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException($"{paramName} must not be whitespace.", paramName);
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException($"{paramName} must not contain surrounding whitespace.", paramName);
        }

        if (value.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException($"{paramName} must not contain whitespace.", paramName);
        }

        if (value.Any(char.IsControl))
        {
            throw new ArgumentException($"{paramName} must not contain control characters.", paramName);
        }

        var decoded = DecodeQueryText(
            value,
            paramName,
            $"{paramName} must contain valid percent escapes.",
            $"{paramName} percent-encoded bytes must be valid UTF-8.");
        if (decoded.Any(char.IsControl))
        {
            throw new ArgumentException($"{paramName} must not contain percent-decoded control characters.", paramName);
        }

        ValidateExactQuerySegments(value, paramName);
        return value;
    }

    private static void ValidateExactQuerySegments(string value, string paramName)
    {
        var query = value[0] == '?' ? value[1..] : value;
        if (query.Length == 0)
        {
            throw new ArgumentException($"{paramName} must contain at least one query parameter.", paramName);
        }

        foreach (var segment in query.Split('&', StringSplitOptions.None))
        {
            if (segment.Length == 0)
            {
                throw new ArgumentException($"{paramName} must not contain empty query segments.", paramName);
            }

            var equalsIndex = segment.IndexOf('=');
            var rawName = equalsIndex >= 0 ? segment[..equalsIndex] : segment;
            var decodedName = DecodeQueryText(
                rawName,
                paramName,
                $"{paramName} must contain valid percent escapes.",
                $"{paramName} percent-encoded bytes must be valid UTF-8.");
            if (string.IsNullOrEmpty(decodedName) || decodedName.Any(char.IsWhiteSpace))
            {
                throw new ArgumentException($"{paramName} parameter names must not be empty or whitespace.", paramName);
            }

            if (decodedName.Any(char.IsControl))
            {
                throw new ArgumentException($"{paramName} parameter names must not contain control characters.", paramName);
            }
        }
    }

    private static string? NormalizeOptionalExactHeaderValue(string? value, string paramName)
    {
        if (value is null || value.Length == 0)
        {
            return value;
        }

        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException($"{paramName} must not be whitespace.", paramName);
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException($"{paramName} must not contain surrounding whitespace.", paramName);
        }

        if (value.Any(char.IsControl))
        {
            throw new ArgumentException($"{paramName} must not contain control characters.", paramName);
        }

        return value;
    }

    private static string? NormalizeOptionalAcceptHeaderValue(string? value, string paramName)
    {
        var exact = NormalizeOptionalExactHeaderValue(value, paramName);
        if (string.IsNullOrEmpty(exact))
        {
            return exact;
        }

        var slashIndex = exact.IndexOf('/');
        if (slashIndex <= 0 || slashIndex != exact.LastIndexOf('/') || slashIndex == exact.Length - 1)
        {
            throw new ArgumentException($"{paramName} must be a single HTTP media range.", paramName);
        }

        var type = exact[..slashIndex];
        var subtype = exact[(slashIndex + 1)..];
        if (!IsMediaRangePart(type) || !IsMediaRangePart(subtype))
        {
            throw new ArgumentException($"{paramName} must be a single HTTP media range.", paramName);
        }

        if (type == "*" && subtype != "*")
        {
            throw new ArgumentException($"{paramName} wildcard media ranges must use */*.", paramName);
        }

        return exact;
    }

    private static bool IsMediaRangePart(string value)
        => value == "*" || value.All(IsHttpTokenCharacter);

    private static string? NormalizeOptionalBearerToken(string? value, string paramName)
    {
        if (value is null)
        {
            return null;
        }

        if (value.Length == 0 || string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException($"{paramName} must not be empty or whitespace.", paramName);
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException($"{paramName} must not contain surrounding whitespace.", paramName);
        }

        if (value.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException($"{paramName} must not contain whitespace.", paramName);
        }

        if (value.Any(char.IsControl))
        {
            throw new ArgumentException($"{paramName} must not contain control characters.", paramName);
        }

        ValidateBearerToken(value, paramName);
        return value;
    }

    private static void ValidateBearerToken(string value, string paramName)
    {
        var firstPaddingIndex = value.IndexOf('=');
        var tokenLength = firstPaddingIndex < 0 ? value.Length : firstPaddingIndex;
        if (tokenLength == 0)
        {
            throw new ArgumentException($"{paramName} must be an HTTP bearer token.", paramName);
        }

        for (var index = 0; index < tokenLength; index++)
        {
            if (!IsBearerTokenCharacter(value[index]))
            {
                throw new ArgumentException($"{paramName} must be an HTTP bearer token.", paramName);
            }
        }

        if (firstPaddingIndex < 0)
        {
            return;
        }

        for (var index = firstPaddingIndex; index < value.Length; index++)
        {
            if (value[index] != '=')
            {
                throw new ArgumentException($"{paramName} must use trailing-only bearer token padding.", paramName);
            }
        }
    }

    private static bool IsBearerTokenCharacter(char value)
        => value is >= 'A' and <= 'Z'
            or >= 'a' and <= 'z'
            or >= '0' and <= '9'
            or '-' or '.' or '_' or '~' or '+' or '/';

    private static async IAsyncEnumerable<ToriiServerSentEvent> ReadServerSentEventsAsync(
        Stream stream,
        [EnumeratorCancellation] CancellationToken cancellationToken)
    {
        using var reader = new StreamReader(stream, StrictUtf8, detectEncodingFromByteOrderMarks: false, leaveOpen: false);
        var dataBuilder = new StringBuilder();
        var commentBuilder = new StringBuilder();
        var hasData = false;
        var hasComment = false;
        string? eventName = null;
        string? eventId = null;
        int? retryMilliseconds = null;
        var firstLine = true;

        while (true)
        {
            string? line;
            try
            {
                line = await reader.ReadLineAsync(cancellationToken);
            }
            catch (DecoderFallbackException exception)
            {
                throw new JsonException("SSE stream must be valid UTF-8.", exception);
            }

            if (line is null)
            {
                break;
            }

            if (firstLine)
            {
                firstLine = false;
                if (line.Length > 0 && line[0] == '\ufeff')
                {
                    line = line[1..];
                }
            }

            if (line.Length == 0)
            {
                var sseEvent = BuildServerSentEvent(eventName, eventId, retryMilliseconds, hasData, dataBuilder, hasComment, commentBuilder);
                if (sseEvent is not null)
                {
                    yield return sseEvent;
                }

                dataBuilder.Clear();
                commentBuilder.Clear();
                hasData = false;
                hasComment = false;
                eventName = null;
                eventId = null;
                retryMilliseconds = null;
                continue;
            }

            if (line[0] == ':')
            {
                if (hasComment)
                {
                    commentBuilder.Append('\n');
                }

                hasComment = true;
                commentBuilder.Append(line.Length > 1 && line[1] == ' ' ? line[2..] : line[1..]);
                continue;
            }

            var separatorIndex = line.IndexOf(':');
            var field = separatorIndex < 0 ? line : line[..separatorIndex];
            var value = separatorIndex < 0 ? string.Empty : line[(separatorIndex + 1)..];
            if (value.Length > 0 && value[0] == ' ')
            {
                value = value[1..];
            }

            switch (field)
            {
                case "event":
                    eventName = value.Length == 0 ? null : value;
                    break;
                case "data":
                    if (hasData)
                    {
                        dataBuilder.Append('\n');
                    }

                    hasData = true;
                    dataBuilder.Append(value);
                    break;
                case "id":
                    eventId = value;
                    break;
                case "retry":
                    retryMilliseconds = ParseSseRetryMilliseconds(value);
                    break;
            }
        }

        var finalEvent = BuildServerSentEvent(eventName, eventId, retryMilliseconds, hasData, dataBuilder, hasComment, commentBuilder);
        if (finalEvent is not null)
        {
            yield return finalEvent;
        }
    }

    private static int ParseSseRetryMilliseconds(string value)
    {
        if (!IsCanonicalNonNegativeInt32Text(value)
            || !int.TryParse(value, NumberStyles.None, CultureInfo.InvariantCulture, out var parsed))
        {
            throw new JsonException("SSE retry must be canonical non-negative Int32 milliseconds.");
        }

        return parsed;
    }

    private static bool IsCanonicalNonNegativeInt32Text(string value)
    {
        if (value.Length == 0 || (value.Length > 1 && value[0] == '0'))
        {
            return false;
        }

        return value.All(static character => character is >= '0' and <= '9');
    }

    private static ToriiServerSentEvent? BuildServerSentEvent(
        string? eventName,
        string? eventId,
        int? retryMilliseconds,
        bool hasData,
        StringBuilder dataBuilder,
        bool hasComment,
        StringBuilder commentBuilder)
    {
        if (!hasData && !hasComment && eventName is null && eventId is null && retryMilliseconds is null)
        {
            return null;
        }

        var rawData = hasData ? dataBuilder.ToString() : null;
        JsonNode? jsonData = null;
        if (!string.IsNullOrWhiteSpace(rawData))
        {
            jsonData = TryParseSseJsonData(rawData);
        }

        try
        {
            return new ToriiServerSentEvent
            {
                Event = hasData ? eventName ?? "message" : eventName,
                Id = eventId,
                RetryMilliseconds = retryMilliseconds,
                RawData = rawData,
                JsonData = jsonData,
                Comment = hasComment ? commentBuilder.ToString() : null,
            };
        }
        catch (ArgumentException error) when (error.ParamName is "Event" or "Id" or "RetryMilliseconds")
        {
            var field = error.ParamName switch
            {
                "Event" => "sse_event_name",
                "Id" => "last_event_id",
                "RetryMilliseconds" => "retry",
                _ => error.ParamName,
            };
            throw new JsonException($"SSE event.{field}: {error.Message}", error);
        }
    }

    private static JsonNode? TryParseSseJsonData(string rawData)
    {
        try
        {
            return ToriiIdentifierJson.ParseNodeRejectingDuplicateProperties(rawData, "SSE data");
        }
        catch (JsonException)
        {
            return null;
        }
    }

    private static JsonNode RequireSseJsonData(ToriiServerSentEvent sseEvent, string context)
    {
        if (string.IsNullOrWhiteSpace(sseEvent.RawData))
        {
            throw new JsonException($"{context}.data must be valid non-null JSON.");
        }

        try
        {
            return ToriiIdentifierJson.ParseNodeRejectingDuplicateProperties(
                    sseEvent.RawData,
                    $"{context}.data")
                ?? throw new JsonException($"{context}.data must be valid non-null JSON.");
        }
        catch (JsonException exception) when (!ToriiIdentifierJson.IsDuplicatePropertyError(exception))
        {
            throw new JsonException($"{context}.data must be valid non-null JSON.", exception);
        }
    }

    private static void ThrowIfTerminalStreamError(ToriiServerSentEvent sseEvent, string context)
    {
        if (!string.Equals(sseEvent.Event, "stream_error", StringComparison.Ordinal))
        {
            return;
        }

        var payload = RequireSseJsonData(sseEvent, $"{context} terminal stream error");
        if (payload is not JsonObject payloadObject)
        {
            throw new JsonException($"{context} terminal stream error.data must be a JSON object.");
        }

        string[] expectedProperties = ["code", "message", "dropped_messages", "replay_available"];
        var unexpectedProperty = payloadObject
            .Select(static property => property.Key)
            .FirstOrDefault(property => !expectedProperties.Contains(property, StringComparer.Ordinal));
        if (unexpectedProperty is not null)
        {
            throw new JsonException(
                $"{context} terminal stream error.{unexpectedProperty} is not part of the v1 stream error schema.");
        }

        if (payloadObject.Count != expectedProperties.Length)
        {
            var missingProperty = expectedProperties.First(property => !payloadObject.ContainsKey(property));
            throw new JsonException($"{context} terminal stream error.{missingProperty} is required.");
        }

        var code = RequireTerminalStreamErrorString(payloadObject, "code", context, token: true);
        var message = RequireTerminalStreamErrorString(payloadObject, "message", context, token: false);

        var droppedNode = payloadObject["dropped_messages"];
        ulong? droppedMessages = null;
        if (droppedNode is not null)
        {
            if (droppedNode is not JsonValue droppedValue || !droppedValue.TryGetValue<ulong>(out var parsedDropped))
            {
                throw new JsonException(
                    $"{context} terminal stream error.dropped_messages must be null or an unsigned integer.");
            }

            droppedMessages = parsedDropped;
        }

        if (payloadObject["replay_available"] is not JsonValue replayValue
            || !replayValue.TryGetValue<bool>(out var replayAvailable))
        {
            throw new JsonException(
                $"{context} terminal stream error.replay_available must be a boolean.");
        }

        throw new ToriiStreamException(code, message, droppedMessages, replayAvailable);
    }

    private static string RequireTerminalStreamErrorString(
        JsonObject payload,
        string propertyName,
        string context,
        bool token)
    {
        if (payload[propertyName] is not JsonValue value || !value.TryGetValue<string>(out var text))
        {
            throw new JsonException($"{context} terminal stream error.{propertyName} must be a string.");
        }

        if (string.IsNullOrEmpty(text)
            || !string.Equals(text.Trim(), text, StringComparison.Ordinal)
            || text.Any(char.IsControl)
            || (token && text.Any(char.IsWhiteSpace)))
        {
            var shape = token ? "a non-empty exact token" : "non-empty exact text";
            throw new JsonException($"{context} terminal stream error.{propertyName} must be {shape}.");
        }

        return text;
    }

    private static bool TryReadSseStringProperty(
        JsonNode payload,
        string propertyName,
        string context,
        out string? value)
    {
        if (payload is not JsonObject payloadObject)
        {
            throw new JsonException($"{context}.data must be a JSON object.");
        }

        if (!payloadObject.TryGetPropertyValue(propertyName, out var propertyValue) || propertyValue is null)
        {
            value = null;
            return false;
        }

        if (propertyValue is JsonValue jsonValue && jsonValue.TryGetValue<string>(out value))
        {
            return true;
        }

        throw new JsonException($"{context}.{propertyName} must be a string.");
    }

    private async Task<TResponse> DeserializeAsync<TResponse>(
        HttpResponseMessage response,
        CancellationToken cancellationToken)
    {
        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken);
        using var document = await ParseJsonDocumentRejectingDuplicatePropertiesAsync(
            stream,
            DuplicatePropertyContext<TResponse>(response),
            cancellationToken);
        if (typeof(TResponse) == typeof(ToriiContractCallResponse))
        {
            ToriiContractCallJson.ValidateContractCallResponseJsonShape(
                document.RootElement,
                "contract call response");
        }
        TResponse? value;
        try
        {
            value = document.RootElement.Deserialize<TResponse>(serializerOptions);
        }
        catch (ArgumentException error)
            when (typeof(TResponse) == typeof(ToriiContractCallResponse)
                && error.ParamName is not null)
        {
            throw ToriiContractCallJson.DirectMetadataErrorToJsonException(
                error,
                "contract call response");
        }
        return value ?? throw new JsonException($"Torii response for `{response.RequestMessage?.RequestUri}` deserialized to null.");
    }

    private static string DuplicatePropertyContext<TResponse>(HttpResponseMessage response)
    {
        if (typeof(TResponse) == typeof(ToriiIdentifierResolveResponse))
        {
            return "identifier resolve response";
        }

        return $"Torii response for `{response.RequestMessage?.RequestUri}`";
    }

    private static async Task<JsonDocument> ParseJsonDocumentRejectingDuplicatePropertiesAsync(
        Stream stream,
        string context,
        CancellationToken cancellationToken)
    {
        var document = await JsonDocument.ParseAsync(
            stream,
            new JsonDocumentOptions { MaxDepth = 128 },
            cancellationToken);
        try
        {
            ToriiIdentifierJson.RejectDuplicateProperties(document.RootElement, context);
            return document;
        }
        catch
        {
            document.Dispose();
            throw;
        }
    }

    private static JsonException RewriteIdentifierPoliciesJsonException(JsonException exception)
    {
        const string converterPrefix = "policy.";
        if (!exception.Message.StartsWith(converterPrefix, StringComparison.Ordinal))
        {
            return exception;
        }

        var itemContext = "identifier policies response.items[0]";
        var path = exception.Path;
        if (!string.IsNullOrEmpty(path))
        {
            const string itemMarker = ".items[";
            var markerStart = path.IndexOf(itemMarker, StringComparison.Ordinal);
            if (markerStart >= 0)
            {
                var indexStart = markerStart + itemMarker.Length;
                var indexEnd = path.IndexOf(']', indexStart);
                if (indexEnd > indexStart)
                {
                    itemContext = $"identifier policies response.items[{path[indexStart..indexEnd]}]";
                }
            }
        }

        var detail = exception.Message[converterPrefix.Length..];
        if (detail.EndsWith(" must not be empty.", StringComparison.Ordinal))
        {
            detail = $"{detail[..^" must not be empty.".Length]} must be a non-empty string.";
        }

        return new JsonException($"{itemContext}.{detail}", exception);
    }

    private static JsonException RewriteIdentifierResolveJsonException(JsonException exception)
    {
        const string converterPrefix = "identifier receipt.";
        return exception.Message.StartsWith(converterPrefix, StringComparison.Ordinal)
            ? new JsonException(
                $"identifier resolve response.{exception.Message[converterPrefix.Length..]}",
                exception)
            : exception;
    }

    private static void ValidateMultisigResponse(ToriiMultisigResponse response, string context)
    {
        ToriiMultisigJson.ValidateMultisigResponse(response, context);
    }

    private static void ValidateMultisigContractCallResponse(
        ToriiMultisigContractCallResponse response,
        string context)
    {
        ToriiMultisigJson.ValidateMultisigContractCallResponse(response, context);
    }

    private static void ValidateAccountOnboardingResponse(ToriiAccountOnboardingResponse response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        _ = ToriiAccountOnboardingReceiptVerifier.RequireCanonicalAccountId(
            response.AccountId,
            $"{context}.account_id");
        _ = ToriiAccountOnboardingReceiptVerifier.RequireAlias(response.Alias, $"{context}.alias");
        if (response.TransactionHashHex is not null)
        {
            _ = ToriiExplorerDirectMetadata.RequireExactSizedHex(
                response.TransactionHashHex,
                $"{context}.tx_hash_hex",
                32);
        }
        var expectedDisposition = response.Status switch
        {
            "Queued" => "create",
            "Repaired" => "repair",
            "Unchanged" => "no_op",
            _ => throw new JsonException($"{context}.status must be Queued, Repaired, or Unchanged."),
        };
        if (!string.Equals(response.Disposition?.Kind, expectedDisposition, StringComparison.Ordinal))
        {
            throw new JsonException($"{context}.disposition does not match status.");
        }
        if ((response.Status == "Unchanged") != (response.TransactionHashHex is null))
        {
            throw new JsonException($"{context}.tx_hash_hex presence does not match status.");
        }
    }

    private static void ValidateAccountFaucetResponse(ToriiAccountFaucetResponse response, string context)
    {
        ToriiOnboardingJson.ValidateAccountFaucetResponse(response, context);
    }

    private static void ValidateAccountFaucetPuzzle(ToriiAccountFaucetPuzzle response, string context)
    {
        ToriiAccountFaucetJson.ValidateAccountFaucetPuzzle(response, context);
    }

    private static void ValidateVpnProfile(ToriiVpnProfile response, string context)
    {
        ToriiVpnJson.ValidateVpnProfile(response, context);
    }

    private static void ValidateVpnQuote(ToriiVpnQuote response, string context)
    {
        ToriiVpnJson.ValidateVpnQuote(response, context);
    }

    private static void ValidateVpnSession(ToriiVpnSession response, string context)
    {
        ToriiVpnJson.ValidateVpnSession(response, context);
    }

    private static void ValidateVpnReceiptListResponse(ToriiVpnReceiptListResponse response, string context)
    {
        ToriiVpnJson.ValidateVpnReceiptListResponse(response, context);
    }

    private static void ValidateVpnReceipt(ToriiVpnReceipt response, string context)
    {
        ToriiVpnJson.ValidateVpnReceipt(response, context);
    }

    private static void ValidateContractCodeRecord(ToriiContractCodeRecord response, string context)
    {
        ToriiContractMetadataJson.ValidateContractCodeRecord(response, context);
    }

    private static void ValidateContractInstancesResponse(
        ToriiContractInstancesResponse response,
        string context)
    {
        ToriiContractInstancesJson.ValidateContractInstancesResponse(response, context);
    }

    private static void ValidateContractCodeView(ToriiContractCodeView response, string context)
    {
        ToriiContractMetadataJson.ValidateContractCodeView(response, context);
    }

    private static void ValidateContractCodeViewAccessHints(
        ToriiContractViewAccessHints response,
        string context)
    {
        ValidateContractCodeViewTokenList(response.ReadKeys, $"{context}.read_keys");
        ValidateContractCodeViewTokenList(response.WriteKeys, $"{context}.write_keys");
    }

    private static void ValidateContractCodeViewEntrypoints(
        IReadOnlyList<ToriiContractViewEntrypoint>? entrypoints,
        string context)
    {
        if (entrypoints is null)
        {
            throw new JsonException($"{context} is required.");
        }

        for (var index = 0; index < entrypoints.Count; index++)
        {
            var entrypoint = entrypoints[index];
            if (entrypoint is null)
            {
                throw new JsonException($"{context}[{index}] must not be null.");
            }

            ValidateContractCodeViewEntrypoint(entrypoint, $"{context}[{index}]");
        }
    }

    private static void ValidateContractCodeViewEntrypoint(
        ToriiContractViewEntrypoint response,
        string context)
    {
        ValidateExactTokenText(response.Name, $"{context}.name");
        ValidateExactTokenText(response.Kind, $"{context}.kind");
        ValidateContractCodeViewEntrypointParams(response.Parameters, $"{context}.params");
        ValidateOptionalExactNonEmptyText(response.ReturnType, $"{context}.return_type");
        ValidateOptionalExactTokenText(response.Permission, $"{context}.permission");
        ValidateContractCodeViewTokenList(response.ReadKeys, $"{context}.read_keys");
        ValidateContractCodeViewTokenList(response.WriteKeys, $"{context}.write_keys");
        ValidateContractCodeViewTokenList(response.AccessHintsSkipped, $"{context}.access_hints_skipped");
        ValidateContractCodeViewTokenList(response.Triggers, $"{context}.triggers");
    }

    private static void ValidateContractCodeViewEntrypointParams(
        IReadOnlyList<ToriiContractViewEntrypointParam>? parameters,
        string context)
    {
        if (parameters is null)
        {
            throw new JsonException($"{context} is required.");
        }

        for (var index = 0; index < parameters.Count; index++)
        {
            var parameter = parameters[index];
            if (parameter is null)
            {
                throw new JsonException($"{context}[{index}] must not be null.");
            }

            ValidateExactTokenText(parameter.Name, $"{context}[{index}].name");
            ValidateExactNonEmptyText(
                parameter.TypeName,
                $"{context}[{index}].type_name",
                message => new JsonException(message));
        }
    }

    private static void ValidateContractCodeViewAnalysis(
        ToriiContractViewAnalysis response,
        string context)
    {
        if (response.Memory is null)
        {
            throw new JsonException($"{context}.memory is required.");
        }

        if (response.Syscalls is null)
        {
            throw new JsonException($"{context}.syscalls is required.");
        }

        for (var index = 0; index < response.Syscalls.Count; index++)
        {
            var syscall = response.Syscalls[index];
            if (syscall is null)
            {
                throw new JsonException($"{context}.syscalls[{index}] must not be null.");
            }

            ValidateOptionalExactTokenText(syscall.Name, $"{context}.syscalls[{index}].name");
        }
    }

    private static void ValidateContractCodeViewTokenList(IReadOnlyList<string>? values, string context)
    {
        if (values is null)
        {
            throw new JsonException($"{context} is required.");
        }

        for (var index = 0; index < values.Count; index++)
        {
            ValidateExactTokenText(values[index], $"{context}[{index}]");
        }
    }

    private static void ValidateContractCodeViewWarnings(IReadOnlyList<string>? values, string context)
    {
        if (values is null)
        {
            throw new JsonException($"{context} is required.");
        }

        for (var index = 0; index < values.Count; index++)
        {
            ValidateExactNonEmptyText(values[index], $"{context}[{index}]", message => new JsonException(message));
        }
    }

    private static void ValidateRenderedSourceText(string? value, string field)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new JsonException($"{field} must be non-empty rendered source text.");
        }

        if (value.IndexOf('\0') >= 0)
        {
            throw new JsonException($"{field} must not contain NUL characters.");
        }
    }

    private static void ValidateContractStateResponse(ToriiContractStateResponse response, string context)
    {
        ToriiContractStateJson.ValidateContractStateResponse(response, context);
    }

    private static void ValidateContractCallResponse(ToriiContractCallResponse response, string context)
    {
        ToriiContractCallJson.ValidateContractCallResponse(response, context);
    }

    private static void ValidateFeeQuoteResponse(
        ToriiFeeQuoteResponse response,
        FeePaymentIntent requestedIntent,
        string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        if (response.Intent is null)
        {
            throw new JsonException($"{context}.intent must not be null.");
        }
        if (!requestedIntent.HasSamePayerAndGasBound(response.Intent))
        {
            throw new JsonException(
                $"{context}.intent changed the selected payer, sponsor revision, or gas bound.");
        }
        if (response.Observation is null || response.Observation.NextBlockHeight == 0)
        {
            throw new JsonException($"{context}.observation.next_block_height must be positive.");
        }
        if (response.Decision?.Value?.DebitSource is null
            || !string.Equals(response.Decision.Status, "accepted", StringComparison.Ordinal))
        {
            throw new JsonException($"{context}.decision must be accepted with a debit source.");
        }
        if (response.Components is null || response.Capacities is null)
        {
            throw new JsonException($"{context} component and capacity arrays must not be null.");
        }
        if (!response.Components.SequenceEqual(response.Intent.ChargeLimits))
        {
            throw new JsonException($"{context}.components must equal the returned intent charge limits.");
        }
    }

    private static void ValidateContractViewResponse(ToriiContractViewResponse response, string context)
    {
        ToriiContractCallJson.ValidateContractViewResponse(response, context);
    }

    private static void ValidateContractViewErrorResponse(ToriiContractViewErrorResponse response, string context)
    {
        ToriiContractCallJson.ValidateContractViewErrorResponse(response, context);
    }

    private static void ValidateOptionalContractViewVmDiagnostic(
        ToriiContractViewVmDiagnostic? response,
        string context)
    {
        ToriiContractCallJson.ValidateOptionalContractViewVmDiagnostic(response, context);
    }

    private static void ValidateContractVerifiedSourceJob(ToriiContractVerifiedSourceJob response, string context)
    {
        ToriiContractMetadataJson.ValidateVerifiedSourceJob(response, context);
    }

    private static void ValidateOptionalContractVerifiedSourceReference(
        ToriiContractVerifiedSourceReference? response,
        string context)
    {
        if (response is null)
        {
            return;
        }

        ValidateExactNonEmptyText(response.Language, $"{context}.language", message => new JsonException(message));
        if (response.SourceName is not null)
        {
            ValidateExactNonEmptyText(
                response.SourceName,
                $"{context}.source_name",
                message => new JsonException(message));
        }
        ValidateExactNonEmptyText(response.SubmittedAt, $"{context}.submitted_at", message => new JsonException(message));
        ValidateOptionalExactSizedHex(response.ManifestIdHex, $"{context}.manifest_id_hex", 32);
        ValidateOptionalExactSizedHex(response.PayloadDigestHex, $"{context}.payload_digest_hex", 32);
    }

    private static void ValidateRuntimeAbiHash(ToriiRuntimeAbiHash response, string context)
    {
        ToriiRuntimeJson.ValidateRuntimeAbiHash(response, context);
    }

    private static void ValidateRuntimeAbiActive(ToriiRuntimeAbiActive response, string context)
    {
        ToriiRuntimeJson.ValidateRuntimeAbiActive(response, context);
    }

    private static void ValidateRuntimeMetrics(ToriiRuntimeMetrics response, string context)
    {
        ToriiRuntimeJson.ValidateRuntimeMetrics(response, context);
    }

    private static void ValidatePipelineEvent(ToriiPipelineEvent response, string context)
    {
        ToriiSseEventJson.ValidatePipelineEvent(response, context);
    }

    private static void ValidateProofEvent(ToriiProofEvent response, string context)
    {
        ToriiSseEventJson.ValidateProofEvent(response, context);
    }

    private static void ValidateUaidManifestsResponse(ToriiUaidManifestsResponse response, string context)
    {
        ToriiUaidJson.ValidateUaidManifestsResponse(response, context);
    }

    private static void ValidateUaidManifestRecord(ToriiUaidManifestRecord response, string context)
    {
        ToriiUaidJson.ValidateUaidManifestRecord(response, context);
    }

    private static void ValidateUaidManifestLifecycle(ToriiUaidManifestLifecycle response, string context)
    {
        ToriiUaidJson.ValidateUaidManifestLifecycle(response, context);
    }

    private static void ValidateUaidPortfolioResponse(ToriiUaidPortfolioResponse response, string context)
    {
        ToriiUaidJson.ValidateUaidPortfolioResponse(response, context);
    }

    private static void ValidateUaidPortfolioDataspace(ToriiUaidPortfolioDataspace response, string context)
    {
        ToriiUaidJson.ValidateUaidPortfolioDataspace(response, context);
    }

    private static void ValidateUaidPortfolioAccount(ToriiUaidPortfolioAccount response, string context)
    {
        ToriiUaidJson.ValidateUaidPortfolioAccount(response, context);
    }

    private static void ValidateUaidBindingsResponse(ToriiUaidBindingsResponse response, string context)
    {
        ToriiUaidJson.ValidateUaidBindingsResponse(response, context);
    }

    private static void ValidateAccountsPage(ToriiAccountsPage response, string context)
    {
        ToriiAccountQueryJson.ValidateAccountsPage(response, context);
    }

    private static void ValidateAccountSummary(ToriiAccountSummary response, string context)
    {
        ToriiAccountQueryJson.ValidateAccountSummary(response, context);
    }

    private static void ValidateAccountAssetBalancesPage(ToriiAssetBalancesPage response, string context)
    {
        ToriiAccountQueryJson.ValidateAssetBalancesPage(response, context);
    }

    private static void ValidateAccountAssetBalance(ToriiAssetBalance response, string context)
    {
        ToriiAccountQueryJson.ValidateAssetBalance(response, context);
    }

    private static void ValidateAccountPermissionsPage(ToriiAccountPermissionsPage response, string context)
    {
        ToriiAccountQueryJson.ValidateAccountPermissionsPage(response, context);
    }

    private static void ValidateAccountAliasLookupResponse(ToriiAccountAliasLookupResponse response, string context)
    {
        ToriiAccountAliasLookupJson.ValidateAccountAliasLookupResponse(response, context);
    }

    private static void ValidateAccountAliasResolution(ToriiAccountAliasResolution response, string context)
    {
        ToriiAliasResolutionJson.ValidateAccountAliasResolution(response, context);
    }

    private static void ValidateAccountAliasIndexResolution(ToriiAccountAliasIndexResolution response, string context)
    {
        ToriiAliasResolutionJson.ValidateAccountAliasIndexResolution(response, context);
    }

    private static void ValidateAssetAliasResolution(ToriiAssetAliasResolution response, string context)
    {
        ToriiAliasResolutionJson.ValidateAssetAliasResolution(response, context);
    }

    private static void ValidateAssetAliasBinding(ToriiAssetAliasBinding response, string context)
    {
        ToriiAliasResolutionJson.ValidateAssetAliasBinding(response, context);
    }

    private static void ValidateContractAliasResolution(ToriiContractAliasResolution response, string context)
    {
        ToriiAliasResolutionJson.ValidateContractAliasResolution(response, context);
    }

    private static void ValidateContractAliasBinding(ToriiContractAliasBinding response, string context)
    {
        ToriiAliasResolutionJson.ValidateContractAliasBinding(response, context);
    }

    private static void ValidateAccountTransactionsPage(ToriiTransactionsPage response, string context)
    {
        ToriiAccountQueryJson.ValidateTransactionsPage(response, context);
    }

    private static void ValidateAccountTransactionSummary(ToriiTransactionSummary response, string context)
    {
        ToriiAccountQueryJson.ValidateTransactionSummary(response, context);
    }

    private static void ValidateExplorerBlocksPage(ToriiExplorerBlocksPage response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        ValidateExplorerItems(response.Items, $"{context}.items", ValidateExplorerBlock);
    }

    private static void ValidateExplorerAccountsPage(ToriiExplorerAccountsPage response, string context)
    {
        ToriiExplorerJson.ValidateExplorerAccountsPage(response, context);
    }

    private static void ValidateExplorerAccount(ToriiExplorerAccount response, string context)
    {
        ToriiExplorerJson.ValidateExplorerAccount(response, context);
    }

    private static void ValidateExplorerAccountQrSnapshot(ToriiExplorerAccountQrSnapshot response, string context)
    {
        ToriiExplorerSnapshotJson.ValidateExplorerAccountQrSnapshot(response, context);
    }

    private static void ValidateExplorerDomainsPage(ToriiExplorerDomainsPage response, string context)
    {
        ToriiExplorerJson.ValidateExplorerDomainsPage(response, context);
    }

    private static void ValidateExplorerDomain(ToriiExplorerDomain response, string context)
    {
        ToriiExplorerJson.ValidateExplorerDomain(response, context);
    }

    private static void ValidateExplorerAssetDefinitionsPage(
        ToriiExplorerAssetDefinitionsPage response,
        string context)
    {
        ToriiExplorerJson.ValidateExplorerAssetDefinitionsPage(response, context);
    }

    private static void ValidateExplorerAssetDefinition(ToriiExplorerAssetDefinition response, string context)
    {
        ToriiExplorerJson.ValidateExplorerAssetDefinition(response, context);
    }

    private static void ValidateExplorerAssetDefinitionEconometrics(
        ToriiExplorerAssetDefinitionEconometrics response,
        string context)
    {
        ToriiExplorerJson.ValidateExplorerAssetDefinitionEconometrics(response, context);
    }

    private static void ValidateExplorerVelocityWindow(
        ToriiExplorerEconometricsVelocityWindow response,
        string context)
    {
        ToriiExplorerJson.ValidateExplorerVelocityWindow(response, context);
    }

    private static void ValidateExplorerIssuanceWindow(
        ToriiExplorerEconometricsIssuanceWindow response,
        string context)
    {
        ToriiExplorerJson.ValidateExplorerIssuanceWindow(response, context);
    }

    private static void ValidateExplorerIssuanceSeriesPoint(
        ToriiExplorerEconometricsIssuanceSeriesPoint response,
        string context)
    {
        ToriiExplorerJson.ValidateExplorerIssuanceSeriesPoint(response, context);
    }

    private static void ValidateExplorerAssetDefinitionSnapshot(
        ToriiExplorerAssetDefinitionSnapshot response,
        string context)
    {
        ToriiExplorerJson.ValidateExplorerAssetDefinitionSnapshot(response, context);
    }

    private static void ValidateExplorerTopHolder(ToriiExplorerEconometricsTopHolder response, string context)
    {
        ToriiExplorerJson.ValidateExplorerTopHolder(response, context);
    }

    private static void ValidateExplorerDistributionSnapshot(
        ToriiExplorerEconometricsDistributionSnapshot response,
        string context)
    {
        ToriiExplorerJson.ValidateExplorerDistributionSnapshot(response, context);
    }

    private static void ValidateExplorerLorenzPoint(ToriiExplorerEconometricsLorenzPoint response, string context)
    {
        ToriiExplorerJson.ValidateExplorerLorenzPoint(response, context);
    }

    private static void ValidateExplorerAssetsPage(ToriiExplorerAssetsPage response, string context)
    {
        ToriiExplorerJson.ValidateExplorerAssetsPage(response, context);
    }

    private static void ValidateExplorerAsset(ToriiExplorerAsset response, string context)
    {
        ToriiExplorerJson.ValidateExplorerAsset(response, context);
    }

    private static void ValidateExplorerNftsPage(ToriiExplorerNftsPage response, string context)
    {
        ToriiExplorerJson.ValidateExplorerNftsPage(response, context);
    }

    private static void ValidateExplorerNft(ToriiExplorerNft response, string context)
    {
        ToriiExplorerJson.ValidateExplorerNft(response, context);
    }

    private static void ValidateExplorerRwasPage(ToriiExplorerRwasPage response, string context)
    {
        ToriiExplorerJson.ValidateExplorerRwasPage(response, context);
    }

    private static void ValidateExplorerRwa(ToriiExplorerRwa response, string context)
    {
        ToriiExplorerJson.ValidateExplorerRwa(response, context);
    }

    private static void ValidateExplorerRwaParent(ToriiExplorerRwaParent response, string context)
    {
        ToriiExplorerJson.ValidateExplorerRwaParent(response, context);
    }

    private static void ValidateExplorerHealthSnapshot(ToriiExplorerHealthSnapshot response, string context)
    {
        ToriiExplorerSnapshotJson.ValidateExplorerHealthSnapshot(response, context);
    }

    private static void ValidateExplorerMetricsSnapshot(ToriiExplorerMetricsSnapshot response, string context)
    {
        ToriiExplorerSnapshotJson.ValidateExplorerMetricsSnapshot(response, context);
    }

    private static void ValidateExplorerPagination(ToriiExplorerPaginationMeta? response, string context)
    {
        ToriiExplorerJson.ValidateExplorerPagination(response, context);
    }

    private static void ValidateExplorerBlock(ToriiExplorerBlock response, string context)
    {
        ToriiExplorerJson.ValidateExplorerBlock(response, context);
    }

    private static void ValidateExplorerTransactionsPage(ToriiExplorerTransactionsPage response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        ValidateExplorerItems(response.Items, $"{context}.items", ValidateExplorerTransaction);
    }

    private static void ValidateExplorerLatestTransactionsResponse(
        ToriiExplorerLatestTransactionsResponse response,
        string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        ValidateExplorerItems(response.Items, $"{context}.items", ValidateExplorerTransaction);
    }

    private static void ValidateExplorerTransaction(ToriiExplorerTransaction response, string context)
    {
        ToriiExplorerJson.ValidateExplorerTransaction(response, context);
    }

    private static void ValidateExplorerTransactionDetail(
        ToriiExplorerTransactionDetail response,
        string context)
    {
        ToriiExplorerJson.ValidateExplorerTransactionDetail(response, context);
    }

    private static void ValidateExplorerTransactionRejection(
        ToriiExplorerTransactionRejection response,
        string context)
    {
        ToriiExplorerJson.ValidateExplorerTransactionRejection(response, context);
    }

    private static void ValidateExplorerInstructionsPage(ToriiExplorerInstructionsPage response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        ValidateExplorerItems(response.Items, $"{context}.items", ValidateExplorerInstruction);
    }

    private static void ValidateExplorerLatestInstructionsResponse(
        ToriiExplorerLatestInstructionsResponse response,
        string context)
    {
        ArgumentNullException.ThrowIfNull(response);
        ValidateExplorerItems(response.Items, $"{context}.items", ValidateExplorerInstruction);
    }

    private static void ValidateExplorerInstruction(ToriiExplorerInstruction response, string context)
    {
        ToriiExplorerJson.ValidateExplorerInstruction(response, context);
    }

    private static void ValidateExplorerItems<TItem>(
        IReadOnlyList<TItem>? items,
        string context,
        Action<TItem, string> validate)
        where TItem : class
    {
        if (items is null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        for (var index = 0; index < items.Count; index++)
        {
            var item = items[index];
            if (item is null)
            {
                throw new JsonException($"{context}[{index}] must not be null.");
            }

            validate(item, $"{context}[{index}]");
        }
    }

    private static void ValidateSoraFsCidLookupResponse(ToriiSoraFsCidLookupResponse response, string context)
    {
        ToriiSoraFsJson.ValidateCidLookupResponse(response, context);
    }

    private static void ValidateSoraFsFileEntry(ToriiSoraFsFileEntry file, string context)
    {
        ToriiSoraFsJson.ValidateFileEntry(file, context);
    }

    private static void ValidateSoraFsContentCid(string? value, string field)
    {
        ValidateExactNonEmptyText(value, field, message => new JsonException(message));
        var text = value ?? throw new JsonException($"{field} must not be null.");

        if (text.Any(char.IsWhiteSpace))
        {
            throw new JsonException($"{field} must not contain whitespace.");
        }

        if (text[0] != 'b' || text.Length == 1)
        {
            throw new JsonException($"{field} must be lowercase multibase base32 CID text.");
        }

        for (var index = 1; index < text.Length; index++)
        {
            var character = text[index];
            if (character is not (>= 'a' and <= 'z') and not (>= '2' and <= '7'))
            {
                throw new JsonException($"{field} must be lowercase multibase base32 CID text.");
            }
        }
    }

    private static string? ReadSoraFsContentCidHeader(HttpResponseMessage response)
    {
        if (!response.Headers.TryGetValues("sora-content-cid", out var values))
        {
            return null;
        }

        var contentCids = values.ToArray();
        if (contentCids.Length != 1)
        {
            throw new JsonException("SoraFS content response.sora-content-cid must appear exactly once.");
        }

        ValidateSoraFsContentCid(contentCids[0], "SoraFS content response.sora-content-cid");
        return contentCids[0];
    }

    private static void ValidateSoraFsPathText(string? value, string field)
    {
        ValidateExactNonEmptyText(value, field, message => new JsonException(message));
    }

    private static void ValidateOptionalExactNonEmptyText(string? value, string field)
    {
        if (value is null)
        {
            return;
        }

        ValidateExactNonEmptyText(value, field, message => new JsonException(message));
    }

    private static void ValidateExactTokenText(string? value, string field)
    {
        ValidateExactNonEmptyText(value, field, message => new JsonException(message));
        var text = value ?? throw new JsonException($"{field} must not be null.");
        if (text.Any(char.IsWhiteSpace))
        {
            throw new JsonException($"{field} must not contain whitespace.");
        }
    }

    private static void ValidateExactUniqueTokenList(
        IReadOnlyList<string>? values,
        string field,
        bool requireNonEmpty = false)
    {
        if (values is null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        if (requireNonEmpty && values.Count == 0)
        {
            throw new JsonException($"{field} must not be empty.");
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);
        for (var index = 0; index < values.Count; index++)
        {
            var itemField = $"{field}[{index}]";
            var value = values[index];
            ValidateExactTokenText(value, itemField);
            if (!seen.Add(value))
            {
                throw new JsonException($"{itemField} must not contain duplicate capability labels.");
            }
        }
    }

    private static void ValidateExactTokenSequence(
        IReadOnlyList<string>? values,
        IReadOnlyList<string> expected,
        string field)
    {
        if (values is null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        if (values.Count != expected.Count)
        {
            throw new JsonException($"{field} must match the expected projection metadata key list.");
        }

        for (var index = 0; index < values.Count; index++)
        {
            var itemField = $"{field}[{index}]";
            ValidateExactTokenText(values[index], itemField);
            if (!string.Equals(values[index], expected[index], StringComparison.Ordinal))
            {
                throw new JsonException($"{itemField} must match the expected projection metadata key.");
            }
        }
    }

    private static void ValidateExactTokenValue(string value, string expected, string field)
    {
        if (!string.Equals(value, expected, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must be {expected}.");
        }
    }

    private static void ValidateOptionalExactTokenText(string? value, string field)
    {
        if (value is null)
        {
            return;
        }

        ValidateExactTokenText(value, field);
    }

    private static void ValidateOptionalCanonicalQuantityText(string? value, string field)
    {
        if (value is null)
        {
            return;
        }

        ValidateCanonicalQuantityText(value, field);
    }

    private static void ValidateCanonicalQuantityText(string? value, string field)
    {
        ValidateExactNonEmptyText(value, field, message => new JsonException(message));
        _ = ToriiQuantityJson.RequireCanonicalQuantity(value, field);
    }

    private static void ValidateNonNegativeInt32(int value, string field)
    {
        if (value < 0)
        {
            throw new JsonException($"{field} must be non-negative.");
        }
    }

    private static void ValidateAbiVersionV1(int value, string field)
    {
        ValidateNonNegativeInt32(value, field);
        if (value != 1)
        {
            throw new JsonException($"{field} must be 1.");
        }
    }

    private static void ValidatePositiveInt32(int value, string field)
    {
        if (value <= 0)
        {
            throw new JsonException($"{field} must be positive.");
        }
    }

    private static void ValidateExactInt32(int value, int expected, string field)
    {
        if (value != expected)
        {
            throw new JsonException($"{field} must be {expected}.");
        }
    }

    private static void ValidateNonNegativeInt64(long value, string field)
    {
        if (value < 0)
        {
            throw new JsonException($"{field} must be non-negative.");
        }
    }

    private static void ValidateFiniteNonNegativeDouble(double value, string field)
    {
        if (!double.IsFinite(value) || value < 0)
        {
            throw new JsonException($"{field} must be a finite non-negative number.");
        }
    }

    private static void ValidateFiniteUnitIntervalDouble(double value, string field)
    {
        if (!double.IsFinite(value) || value < 0 || value > 1)
        {
            throw new JsonException($"{field} must be a finite number from 0 to 1.");
        }
    }

    private static void ValidateIdentifierResolveRequest(ToriiIdentifierResolveRequest request)
    {
        ValidateIdentifierPolicyId(
            request.PolicyId,
            "identifier resolve request.policy_id",
            message => new ArgumentException(message, nameof(request.PolicyId)));
    }

    private static void ValidateIdentifierPoliciesResponse(ToriiIdentifierPoliciesResponse response)
    {
        if (response.Items is null)
        {
            throw new JsonException("identifier policies response.items is required.");
        }

        for (var index = 0; index < response.Items.Count; index++)
        {
            var item = response.Items[index];
            ValidateIdentifierPolicyId(
                item.PolicyId,
                $"identifier policies response.items[{index}].policy_id",
                static message => new JsonException(message));
            ValidateExactNonEmptyText(
                item.ResolverPublicKey,
                $"identifier policies response.items[{index}].resolver_public_key",
                static message => new JsonException(message));
        }
    }

    private static void ValidateIdentifierResolveResponse(ToriiIdentifierResolveResponse response)
    {
        ValidateIdentifierPolicyId(
            response.PolicyId,
            "identifier resolve response.policy_id",
            static message => new JsonException(message));

        if (!IsNestedIdentifierResolveResponse(response))
        {
            throw new JsonException("identifier resolve response must use the current payload/attestation envelope.");
        }

        if (!string.IsNullOrEmpty(response.Signature))
        {
            ValidateExactHex(response.Signature, "identifier resolve response.attestation.signature");
        }

        ValidateIdentifierReceiptSignaturePayload(
            response.SignaturePayload,
            "identifier resolve response");
    }

    private static bool IsNestedIdentifierResolveResponse(ToriiIdentifierResolveResponse response)
    {
        if (!string.IsNullOrEmpty(response.SignaturePayloadHex)
            || response.SignaturePayload is not JsonObject payloadObject)
        {
            return false;
        }

        return payloadObject.TryGetPropertyValue("payload", out var payload)
            && payload is JsonObject
            && payloadObject.TryGetPropertyValue("attestation", out var attestation)
            && attestation is JsonObject;
    }

    private static void ValidateIdentifierReceiptSignaturePayload(JsonNode? payload, string context)
    {
        if (payload is not JsonObject payloadObject)
        {
            return;
        }

        ValidateOpeningSignature(payloadObject, context);
        ValidateAttestationSignature(payloadObject, context);
        ValidateIdentifierReceiptAttestation(payloadObject, context);
        ValidateIdentifierReceiptPayloadPolicyId(payloadObject, context);
        ValidateIdentifierReceiptPayloadExactFields(payloadObject, context);

        if (payloadObject.TryGetPropertyValue("payload", out var nestedPayload)
            && nestedPayload is JsonObject nestedPayloadObject)
        {
            ValidateOpeningSignature(nestedPayloadObject, $"{context}.payload");
            ValidateIdentifierReceiptPayloadPolicyId(nestedPayloadObject, $"{context}.payload");
            ValidateIdentifierReceiptPayloadExactFields(nestedPayloadObject, $"{context}.payload");
        }

        if (payloadObject.TryGetPropertyValue("attestation", out var attestation)
            && attestation is JsonObject attestationObject)
        {
            ValidateAttestationSignature(attestationObject, $"{context}.attestation");
            ValidateIdentifierReceiptAttestation(attestationObject, $"{context}.attestation");
        }
    }

    private static void ValidateIdentifierReceiptPayloadExactFields(JsonObject payload, string context)
    {
        ValidateOptionalExactJsonStringProperty(payload, "account_id", $"{context}.account_id");
        ValidateOptionalExactJsonStringProperty(payload, "opaque_id", $"{context}.opaque_id");
        ValidateOptionalExactJsonStringProperty(payload, "receipt_hash", $"{context}.receipt_hash");
        ValidateOptionalExactJsonStringProperty(payload, "uaid", $"{context}.uaid");

        if (TryGetOptionalJsonObject(payload, "execution", $"{context}.execution", out var execution))
        {
            ValidateOptionalExactJsonStringProperty(execution, "program_id", $"{context}.execution.program_id");
            ValidateOptionalExactJsonStringProperty(execution, "program_digest", $"{context}.execution.program_digest");
            ValidateOptionalExactJsonStringProperty(execution, "backend", $"{context}.execution.backend");
            ValidateOptionalExactJsonStringProperty(execution, "verification_mode", $"{context}.execution.verification_mode");
            ValidateOptionalExactJsonStringProperty(
                execution,
                "input_ciphertext_hash",
                $"{context}.execution.input_ciphertext_hash");
            ValidateOptionalExactJsonStringProperty(
                execution,
                "output_ciphertext_hash",
                $"{context}.execution.output_ciphertext_hash");
            ValidateOptionalExactJsonStringProperty(
                execution,
                "parameter_digest",
                $"{context}.execution.parameter_digest");
            ValidateOptionalExactJsonStringProperty(
                execution,
                "evaluation_key_digest",
                $"{context}.execution.evaluation_key_digest");
            ValidateOptionalExactJsonStringProperty(execution, "output_hash", $"{context}.execution.output_hash");
            ValidateOptionalExactJsonStringProperty(
                execution,
                "associated_data_hash",
                $"{context}.execution.associated_data_hash");
            ValidateOptionalPositiveUnsignedInteger(execution, "executed_at_ms", $"{context}.execution.executed_at_ms");
            ValidateOptionalPositiveUnsignedInteger(execution, "expires_at_ms", $"{context}.execution.expires_at_ms");
        }

        if (TryGetOptionalJsonObject(payload, "opening", $"{context}.opening", out var opening)
            && TryGetOptionalJsonObject(opening, "payload", $"{context}.opening.payload", out var openingPayload))
        {
            ValidateOptionalExactJsonStringProperty(
                openingPayload,
                "program_id",
                $"{context}.opening.payload.program_id");
            ValidateOptionalExactJsonStringProperty(
                openingPayload,
                "input_ciphertext_hash",
                $"{context}.opening.payload.input_ciphertext_hash");
            ValidateOptionalExactJsonStringProperty(
                openingPayload,
                "output_ciphertext_hash",
                $"{context}.opening.payload.output_ciphertext_hash");
            ValidateOptionalExactJsonStringProperty(
                openingPayload,
                "parameter_digest",
                $"{context}.opening.payload.parameter_digest");
            ValidateOptionalExactJsonStringProperty(
                openingPayload,
                "evaluation_key_digest",
                $"{context}.opening.payload.evaluation_key_digest");
            ValidateOptionalExactJsonStringProperty(
                openingPayload,
                "opened_output_hash",
                $"{context}.opening.payload.opened_output_hash");
            ValidateOptionalPositiveUnsignedInteger(
                openingPayload,
                "opened_at_ms",
                $"{context}.opening.payload.opened_at_ms");
            ValidateOptionalPositiveUnsignedInteger(
                openingPayload,
                "expires_at_ms",
                $"{context}.opening.payload.expires_at_ms");
        }
    }

    private static bool TryGetOptionalJsonObject(
        JsonObject payload,
        string propertyName,
        string context,
        out JsonObject value)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var node) || node is null)
        {
            value = new JsonObject();
            return false;
        }

        if (node is JsonObject jsonObject)
        {
            value = jsonObject;
            return true;
        }

        throw new JsonException($"{context} must be an object.");
    }

    private static void ValidateOpeningSignature(JsonObject payload, string context)
    {
        if (!payload.TryGetPropertyValue("opening", out var opening)
            || opening is not JsonObject openingObject
            || !openingObject.TryGetPropertyValue("signature", out var signature))
        {
            return;
        }

        ValidateExactHex(RequireJsonString(signature, $"{context}.opening.signature"), $"{context}.opening.signature");
    }

    private static void ValidateAttestationSignature(JsonObject payload, string context)
    {
        if (!payload.TryGetPropertyValue("signature", out var signature))
        {
            return;
        }

        ValidateExactHex(RequireJsonString(signature, $"{context}.signature"), $"{context}.signature");
    }

    private static void ValidateIdentifierReceiptAttestation(JsonObject attestation, string context)
    {
        var hasKind = attestation.TryGetPropertyValue("kind", out var kindNode);
        var hasProofBackend = attestation.TryGetPropertyValue("proof_backend", out _);
        var hasProofB64 = attestation.TryGetPropertyValue("proof_b64", out _);
        if (!hasKind)
        {
            if (hasProofBackend || hasProofB64)
            {
                throw new JsonException($"{context}.kind is required when proof fields are present.");
            }
            return;
        }

        var kind = RequireExactJsonString(kindNode, $"{context}.kind");
        switch (kind)
        {
            case "signed":
                if (hasProofBackend || hasProofB64)
                {
                    throw new JsonException($"{context} signed attestations must not include proof fields.");
                }
                break;
            case "proof":
                _ = RequireExactJsonStringProperty(attestation, "proof_backend", $"{context}.proof_backend");
                var proofB64 = RequireExactJsonStringProperty(attestation, "proof_b64", $"{context}.proof_b64");
                ValidateExactBase64(proofB64, $"{context}.proof_b64");
                break;
            default:
                throw new JsonException($"{context}.kind must be signed or proof.");
        }
    }

    private static void ValidateIdentifierReceiptPayloadPolicyId(JsonObject payload, string context)
    {
        if (!payload.TryGetPropertyValue("policy_id", out var policyId))
        {
            return;
        }

        ValidateIdentifierPolicyId(
            RequireJsonStringForJson(policyId, $"{context}.policy_id"),
            $"{context}.policy_id",
            static message => new JsonException(message));
    }

    private static void ValidateIdentifierPolicyId(
        string? value,
        string field,
        Func<string, Exception> createException)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw createException($"{field} must be a non-empty policy id.");
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw createException($"{field} must not contain surrounding whitespace.");
        }

        if (ContainsControlCharacter(value))
        {
            throw createException($"{field} must not contain control characters.");
        }

        var parts = value.Split('#');
        if (parts.Length != 2)
        {
            throw createException($"{field} must use `kind#rule`.");
        }

        ValidateIdentifierPolicyIdComponent(parts[0], $"{field}.kind", createException);
        ValidateIdentifierPolicyIdComponent(parts[1], $"{field}.rule", createException);
    }

    private static void ValidateIdentifierPolicyIdComponent(
        string value,
        string field,
        Func<string, Exception> createException)
    {
        if (value.Length == 0)
        {
            throw createException($"{field} must not be empty.");
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw createException($"{field} must not contain surrounding whitespace.");
        }

        if (value.Any(char.IsWhiteSpace))
        {
            throw createException($"{field} must not contain whitespace.");
        }

        if (ContainsControlCharacter(value))
        {
            throw createException($"{field} must not contain control characters.");
        }
    }

    private static bool ContainsControlCharacter(string value)
    {
        foreach (var character in value)
        {
            if (char.IsControl(character))
            {
                return true;
            }
        }

        return false;
    }

    private static void ValidateExactNonEmptyText(
        string? value,
        string field,
        Func<string, Exception> createException)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw createException($"{field} must be a non-empty string.");
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw createException($"{field} must not contain surrounding whitespace.");
        }

        if (ContainsControlCharacter(value))
        {
            throw createException($"{field} must not contain control characters.");
        }
    }

    private static void ValidateOptionalExactJsonStringProperty(
        JsonObject payload,
        string propertyName,
        string context)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var value) || value is null)
        {
            return;
        }

        _ = RequireExactJsonString(value, context);
    }

    private static void ValidateOptionalUnsignedInteger(
        JsonObject payload,
        string propertyName,
        string context)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var value) || value is null)
        {
            return;
        }

        if (value is not JsonValue jsonValue)
        {
            throw new JsonException($"{context} must be an unsigned integer.");
        }

        if (jsonValue.TryGetValue<string>(out var text))
        {
            _ = RequireExactJsonString(value, context);
            if (text.StartsWith("-", StringComparison.Ordinal))
            {
                throw new JsonException($"{context} must be non-negative.");
            }
            if (!ToriiIdentifierJson.IsCanonicalUnsignedDecimalText(text)
                || !ulong.TryParse(text, NumberStyles.None, CultureInfo.InvariantCulture, out _))
            {
                throw new JsonException($"{context} must be canonical unsigned decimal text.");
            }
            return;
        }

        if (jsonValue.TryGetValue<long>(out var signedInteger))
        {
            if (signedInteger < 0)
            {
                throw new JsonException($"{context} must be non-negative.");
            }
            return;
        }

        if (jsonValue.TryGetValue<ulong>(out _))
        {
            return;
        }

        throw new JsonException($"{context} must be an unsigned integer.");
    }

    private static void ValidateOptionalPositiveUnsignedInteger(
        JsonObject payload,
        string propertyName,
        string context)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var value) || value is null)
        {
            return;
        }

        ValidateOptionalUnsignedInteger(payload, propertyName, context);

        if (value is not JsonValue jsonValue)
        {
            return;
        }

        if (jsonValue.TryGetValue<string>(out var text))
        {
            if (text == "0")
            {
                throw new JsonException($"{context} must be positive.");
            }

            return;
        }

        if (jsonValue.TryGetValue<long>(out var signedInteger))
        {
            if (signedInteger == 0)
            {
                throw new JsonException($"{context} must be positive.");
            }

            return;
        }

        if (jsonValue.TryGetValue<ulong>(out var unsignedInteger) && unsignedInteger == 0)
        {
            throw new JsonException($"{context} must be positive.");
        }
    }

    private static string RequireExactJsonStringProperty(JsonObject payload, string propertyName, string context)
    {
        if (!payload.TryGetPropertyValue(propertyName, out var value))
        {
            throw new JsonException($"{context} is required.");
        }

        return RequireExactJsonString(value, context);
    }

    private static string RequireExactJsonString(JsonNode? node, string context)
    {
        var value = RequireJsonStringForJson(node, context);
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new JsonException($"{context} must be a non-empty string.");
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{context} must not contain surrounding whitespace.");
        }

        if (value.Any(char.IsWhiteSpace))
        {
            throw new JsonException($"{context} must not contain whitespace.");
        }

        if (ContainsControlCharacter(value))
        {
            throw new JsonException($"{context} must not contain control characters.");
        }

        return value;
    }

    private static void ValidateExactBase64(string value, string field)
    {
        foreach (var character in value)
        {
            if (char.IsWhiteSpace(character))
            {
                throw new JsonException($"{field} must not contain whitespace.");
            }
        }

        byte[] bytes;
        try
        {
            bytes = Convert.FromBase64String(value);
        }
        catch (FormatException ex)
        {
            throw new JsonException($"{field} must be valid base64.", ex);
        }

        if (bytes.Length == 0)
        {
            throw new JsonException($"{field} must not decode to empty bytes.");
        }

        if (!string.Equals(Convert.ToBase64String(bytes), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must be canonical base64 text.");
        }
    }

    private static byte[] DecodeExactBase64AllowEmpty(string value, string field)
    {
        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must not contain surrounding whitespace.");
        }

        if (value.Any(char.IsWhiteSpace))
        {
            throw new JsonException($"{field} must not contain whitespace.");
        }

        if (ContainsControlCharacter(value))
        {
            throw new JsonException($"{field} must not contain control characters.");
        }

        byte[] bytes;
        try
        {
            bytes = Convert.FromBase64String(value);
        }
        catch (FormatException error)
        {
            throw new JsonException($"{field} must be valid base64.", error);
        }

        if (!string.Equals(Convert.ToBase64String(bytes), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must be canonical base64 text.");
        }

        return bytes;
    }

    private static string RequireJsonStringForJson(JsonNode? node, string context)
    {
        if (node is JsonValue value && value.TryGetValue<string>(out var text))
        {
            return text;
        }

        throw new JsonException($"{context} must be a string.");
    }

    private static void ValidateExactHex(string? value, string field)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new JsonException($"{field} must be a non-empty hex string.");
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must not contain surrounding whitespace.");
        }

        if (value.Any(char.IsWhiteSpace))
        {
            throw new JsonException($"{field} must not contain whitespace.");
        }

        if (ContainsControlCharacter(value))
        {
            throw new JsonException($"{field} must not contain control characters.");
        }

        var body = value.StartsWith("0x", StringComparison.Ordinal)
            ? value[2..]
            : value;
        if (body.Length == 0 || body.Length % 2 != 0 || !IsHex(body))
        {
            throw new JsonException($"{field} must be an exact hex string.");
        }
    }

    private static void ValidateExactSizedHex(string? value, string field, int expectedBytes)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new JsonException($"{field} must be a non-empty {expectedBytes}-byte hex string.");
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must not contain surrounding whitespace.");
        }

        if (value.Any(char.IsWhiteSpace))
        {
            throw new JsonException($"{field} must not contain whitespace.");
        }

        if (ContainsControlCharacter(value))
        {
            throw new JsonException($"{field} must not contain control characters.");
        }

        if (value.Length != expectedBytes * 2 || !IsLowercaseHex(value))
        {
            throw new JsonException($"{field} must be an exact lowercase {expectedBytes}-byte hex string.");
        }
    }

    private static void ValidateExactLowercaseHexChars(string? value, string field, int expectedChars)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new JsonException($"{field} must be a non-empty {expectedChars}-character lowercase hex string.");
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must not contain surrounding whitespace.");
        }

        if (value.Any(char.IsWhiteSpace))
        {
            throw new JsonException($"{field} must not contain whitespace.");
        }

        if (ContainsControlCharacter(value))
        {
            throw new JsonException($"{field} must not contain control characters.");
        }

        if (value.Length != expectedChars || !IsLowercaseHex(value))
        {
            throw new JsonException($"{field} must be a {expectedChars}-character lowercase hex string.");
        }
    }

    private static void ValidateOptionalExactSizedHex(string? value, string field, int expectedBytes)
    {
        if (value is null)
        {
            return;
        }

        ValidateExactSizedHex(value, field, expectedBytes);
    }

    private static void ValidateOptionalExactSizedHexOrEmpty(string? value, string field, int expectedBytes)
    {
        if (value is null || value.Length == 0)
        {
            return;
        }

        ValidateExactSizedHex(value, field, expectedBytes);
    }

    private static void ValidateExactEvenLengthHex(string? value, string field)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new JsonException($"{field} must be a non-empty even-length hex string.");
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must not contain surrounding whitespace.");
        }

        if (value.Any(char.IsWhiteSpace))
        {
            throw new JsonException($"{field} must not contain whitespace.");
        }

        if (ContainsControlCharacter(value))
        {
            throw new JsonException($"{field} must not contain control characters.");
        }

        if (value.Length % 2 != 0 || !IsLowercaseHex(value))
        {
            throw new JsonException($"{field} must be an exact lowercase even-length hex string.");
        }
    }

    private static void ValidateOptionalTransactionHashHex(string? value, string field)
    {
        if (value is null || value.Length == 0)
        {
            return;
        }

        ValidateExactSizedHex(value, field, 32);
    }

    private static void ValidateOptionalBase64(string? value, string field)
    {
        if (value is null)
        {
            return;
        }

        if (string.IsNullOrWhiteSpace(value))
        {
            throw new JsonException($"{field} must be a non-empty base64 string.");
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must not contain surrounding whitespace.");
        }

        if (value.Any(char.IsWhiteSpace))
        {
            throw new JsonException($"{field} must not contain whitespace.");
        }

        if (ContainsControlCharacter(value))
        {
            throw new JsonException($"{field} must not contain control characters.");
        }

        byte[] bytes;
        try
        {
            bytes = Convert.FromBase64String(value);
        }
        catch (FormatException ex)
        {
            throw new JsonException($"{field} must be valid base64.", ex);
        }

        if (bytes.Length == 0)
        {
            throw new JsonException($"{field} must not decode to empty bytes.");
        }

        if (!string.Equals(Convert.ToBase64String(bytes), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must be canonical base64 text.");
        }
    }

    private async Task<ToriiApiException> CreateApiExceptionAsync(
        HttpResponseMessage response,
        CancellationToken cancellationToken)
    {
        string? responseBody = null;
        if (response.Content is not null)
        {
            try
            {
                responseBody = await ReadStrictUtf8TextContentAsync(
                    response.Content,
                    "Torii error response body",
                    cancellationToken);
            }
            catch (InvalidDataException)
            {
                responseBody = InvalidUtf8ResponseBody;
            }
        }

        return new ToriiApiException(
            response.StatusCode,
            response.RequestMessage?.RequestUri,
            responseBody,
            response.ReasonPhrase);
    }

    private static async Task<string> ReadStrictUtf8TextContentAsync(
        HttpContent content,
        string context,
        CancellationToken cancellationToken)
    {
        var bytes = await content.ReadAsByteArrayAsync(cancellationToken);
        try
        {
            return StrictUtf8.GetString(bytes);
        }
        catch (DecoderFallbackException exception)
        {
            throw new InvalidDataException($"{context} must be valid UTF-8.", exception);
        }
    }

    private Task<HttpResponseMessage> OpenLiveSseAsync(
        string path,
        string? query = null,
        CancellationToken cancellationToken = default)
    {
        return SendAsync(
            HttpMethod.Get,
            path,
            query,
            content: null,
            accept: "text/event-stream",
            cancellationToken: cancellationToken);
    }

    private Task<HttpResponseMessage> OpenSseAsync(
        string path,
        string? query = null,
        string? lastEventId = null,
        CancellationToken cancellationToken = default)
    {
        return SendAsync(
            HttpMethod.Get,
            path,
            query,
            content: null,
            accept: "text/event-stream",
            configureRequest: request =>
            {
                var exactLastEventId = NormalizeOptionalExactHeaderValue(lastEventId, nameof(lastEventId));
                if (!string.IsNullOrEmpty(exactLastEventId))
                {
                    request.Headers.TryAddWithoutValidation("Last-Event-ID", exactLastEventId);
                }
            },
            cancellationToken: cancellationToken);
    }

    private async IAsyncEnumerable<TResponse> StreamSsePayloadsAsync<TResponse>(
        string path,
        JsonTypeInfo<TResponse> jsonTypeInfo,
        Action<TResponse>? validate = null,
        string? query = null,
        string? lastEventId = null,
        [EnumeratorCancellation] CancellationToken cancellationToken = default)
    {
        using var response = await OpenSseAsync(path, query, lastEventId, cancellationToken);
        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken);

        await foreach (var sseEvent in ReadServerSentEventsAsync(stream, cancellationToken))
        {
            if (sseEvent.IsComment || sseEvent.RawData is null)
            {
                continue;
            }

            var payloadContext = $"{path} SSE payload";
            var payload = JsonSerializer.Deserialize(RequireSseJsonData(sseEvent, payloadContext), jsonTypeInfo);
            if (payload is null)
            {
                throw new JsonException($"{payloadContext} must not deserialize to null.");
            }

            validate?.Invoke(payload);
            yield return payload;
        }
    }

    private Uri BuildRequestUri(string path, string? query)
    {
        var relativePath = path.Length > 0 && path[0] == '/' ? path[1..] : path;
        var uri = new Uri(BaseUri, relativePath);
        if (string.IsNullOrWhiteSpace(query))
        {
            return uri;
        }

        var builder = new UriBuilder(uri)
        {
            Query = query.Length > 0 && query[0] == '?' ? query[1..] : query,
        };
        return builder.Uri;
    }

    private static string? NormalizeEventSseQuery(string? query)
    {
        if (string.IsNullOrWhiteSpace(query))
        {
            return query;
        }

        var queryText = query[0] == '?' ? query[1..] : query;
        var segments = queryText.Split('&');
        var changed = false;
        for (var index = 0; index < segments.Length; index++)
        {
            var segment = segments[index];
            if (segment.Length == 0)
            {
                continue;
            }

            var equalsIndex = segment.IndexOf('=');
            var rawName = equalsIndex >= 0 ? segment[..equalsIndex] : segment;
            var rawValue = equalsIndex >= 0 ? segment[(equalsIndex + 1)..] : string.Empty;
            var name = DecodeQueryComponent(rawName);
            if (!string.Equals(name, "filter", StringComparison.Ordinal))
            {
                continue;
            }

            var value = DecodeQueryComponent(rawValue);
            var normalized = NormalizeEventFilterPayload(value, "eventFilter");
            if (!string.Equals(normalized, value, StringComparison.Ordinal))
            {
                segments[index] = $"{Uri.EscapeDataString(name)}={Uri.EscapeDataString(normalized)}";
                changed = true;
            }
        }

        return changed ? string.Join('&', segments) : query;
    }

    private static string DecodeQueryComponent(string value)
    {
        var decoded = DecodeQueryText(
            value,
            nameof(value),
            "event SSE query components must contain valid percent escapes.",
            "event SSE query components must contain valid UTF-8 percent-encoded bytes.");
        if (decoded.Any(char.IsControl))
        {
            throw new ArgumentException("event SSE query components must not contain control characters.", nameof(value));
        }

        return decoded;
    }

    private static string DecodeQueryText(
        string value,
        string paramName,
        string invalidPercentMessage,
        string invalidUtf8Message)
    {
        ValidateQueryPercentEscapes(value, paramName, invalidPercentMessage);
        var builder = new StringBuilder(value.Length);
        for (var index = 0; index < value.Length;)
        {
            if (value[index] == '+')
            {
                builder.Append(' ');
                index++;
                continue;
            }

            if (value[index] != '%')
            {
                builder.Append(value[index]);
                index++;
                continue;
            }

            var bytes = new List<byte>();
            while (index < value.Length && value[index] == '%')
            {
                bytes.Add((byte)((HexValue(value[index + 1]) << 4) | HexValue(value[index + 2])));
                index += 3;
            }

            try
            {
                builder.Append(StrictUtf8.GetString(bytes.ToArray()));
            }
            catch (DecoderFallbackException exception)
            {
                throw new ArgumentException(invalidUtf8Message, paramName, exception);
            }
        }

        return builder.ToString();
    }

    private static string DecodePercentEncodedPathSegment(string value, string paramName)
    {
        var builder = new StringBuilder(value.Length);
        for (var index = 0; index < value.Length;)
        {
            if (value[index] != '%')
            {
                builder.Append(value[index]);
                index++;
                continue;
            }

            var bytes = new List<byte>();
            while (index < value.Length && value[index] == '%')
            {
                bytes.Add((byte)((HexValue(value[index + 1]) << 4) | HexValue(value[index + 2])));
                index += 3;
            }

            try
            {
                builder.Append(StrictUtf8.GetString(bytes.ToArray()));
            }
            catch (DecoderFallbackException exception)
            {
                throw new ArgumentException(
                    $"{paramName} path segments must contain valid UTF-8 percent-encoded bytes.",
                    paramName,
                    exception);
            }
        }

        return builder.ToString();
    }

    private static void ValidateQueryPercentEscapes(string value, string paramName, string message)
    {
        for (var index = 0; index < value.Length; index++)
        {
            if (value[index] != '%')
            {
                continue;
            }

            if (index + 2 >= value.Length
                || !Uri.IsHexDigit(value[index + 1])
                || !Uri.IsHexDigit(value[index + 2]))
            {
                throw new ArgumentException(message, paramName);
            }

            index += 2;
        }
    }

    private static void ValidatePathPercentEscapes(string value, string paramName)
    {
        for (var index = 0; index < value.Length; index++)
        {
            if (value[index] != '%')
            {
                continue;
            }

            if (index + 2 >= value.Length
                || !Uri.IsHexDigit(value[index + 1])
                || !Uri.IsHexDigit(value[index + 2]))
            {
                throw new ArgumentException($"{paramName} must contain valid percent escapes.", paramName);
            }

            index += 2;
        }
    }

    private static int HexValue(char value)
        => value switch
        {
            >= '0' and <= '9' => value - '0',
            >= 'A' and <= 'F' => value - 'A' + 10,
            >= 'a' and <= 'f' => value - 'a' + 10,
            _ => throw new ArgumentException("query components must contain valid percent escapes."),
        };

    private static string NormalizeEventFilterPayload(string filter, string context)
    {
        var trimmed = filter.Trim();
        if (trimmed.Length == 0 || (trimmed[0] != '{' && trimmed[0] != '['))
        {
            return filter;
        }

        if (!string.Equals(trimmed, filter, StringComparison.Ordinal))
        {
            throw new ArgumentException($"{context} must not contain surrounding whitespace.", context);
        }

        JsonNode? node;
        try
        {
            node = ToriiIdentifierJson.ParseNodeRejectingDuplicateProperties(filter, context);
        }
        catch (JsonException exception) when (ToriiIdentifierJson.IsDuplicatePropertyError(exception))
        {
            throw new ArgumentException(exception.Message, context, exception);
        }
        catch (JsonException exception)
        {
            throw new ArgumentException($"{context} must be valid JSON.", context, exception);
        }

        if (node is not JsonObject obj)
        {
            return filter;
        }

        ValidateProductionEventFilterObject(obj, context);
        return filter;
    }

    private static void ValidateProductionEventFilterObject(JsonObject filter, string context)
    {
        foreach (var eventKind in new[] { "VerifyingKey", "Proof" })
        {
            if (filter[eventKind] is not JsonObject body
                || body["id_matcher"] is not JsonObject matcher
                || !matcher.TryGetPropertyValue("backend", out var backendNode))
            {
                continue;
            }

            var backend = RequireJsonString(backendNode, $"{context}.{eventKind}.id_matcher.backend");
            var normalizedBackend = VerifyingKeyBackendTags.RequireProductionVerifyBackendLabel(
                backend,
                $"{context}.{eventKind}.id_matcher.backend");
            if (!string.Equals(normalizedBackend, backend, StringComparison.Ordinal))
            {
                throw new ArgumentException(
                    $"{context}.{eventKind}.id_matcher.backend must use exact production verifier backend text.",
                    $"{context}.{eventKind}.id_matcher.backend");
            }

            if (string.Equals(eventKind, "Proof", StringComparison.Ordinal))
            {
                ValidateProofHashMatcher(
                    matcher,
                    "hash_hex",
                    $"{context}.{eventKind}.id_matcher.hash_hex");
                ValidateProofHashMatcher(
                    matcher,
                    "proof_hash_hex",
                    $"{context}.{eventKind}.id_matcher.proof_hash_hex");
            }
            else
            {
                ValidateVerifyingKeyNameMatcher(
                    matcher,
                    $"{context}.{eventKind}.id_matcher.name");
            }
        }
    }

    private static void ValidateVerifyingKeyNameMatcher(JsonObject matcher, string context)
    {
        if (!matcher.TryGetPropertyValue("name", out var node))
        {
            return;
        }

        var raw = RequireJsonString(node, context);
        if (string.IsNullOrWhiteSpace(raw))
        {
            throw new ArgumentException($"{context} must be a non-empty string.", context);
        }

        if (!string.Equals(raw.Trim(), raw, StringComparison.Ordinal))
        {
            throw new ArgumentException($"{context} must not contain surrounding whitespace.", context);
        }

        if (raw.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException($"{context} must not contain whitespace.", context);
        }

        if (ContainsControlCharacter(raw))
        {
            throw new ArgumentException($"{context} must not contain control characters.", context);
        }

        if (raw.Contains(':', StringComparison.Ordinal))
        {
            throw new ArgumentException($"{context} must not contain ':' characters.", context);
        }
    }

    private static void ValidateProofHashMatcher(JsonObject matcher, string propertyName, string context)
    {
        if (!matcher.TryGetPropertyValue(propertyName, out var node))
        {
            return;
        }

        var raw = RequireJsonString(node, context);
        RequireExactHex32String(raw, context);
    }

    private static string RequireJsonString(JsonNode? node, string context)
    {
        if (node is JsonValue value && value.TryGetValue<string>(out var text))
        {
            return text;
        }

        throw new ArgumentException($"{context} must be a string.", context);
    }

    private static void RequireExactHex32String(string raw, string context)
    {
        if (string.IsNullOrWhiteSpace(raw))
        {
            throw new ArgumentException($"{context} must be a non-empty 32-byte hex string.", context);
        }

        if (!string.Equals(raw.Trim(), raw, StringComparison.Ordinal))
        {
            throw new ArgumentException($"{context} must not contain surrounding whitespace.", context);
        }

        if (raw.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException($"{context} must not contain whitespace.", context);
        }

        if (ContainsControlCharacter(raw))
        {
            throw new ArgumentException($"{context} must not contain control characters.", context);
        }

        if (raw.Length != 64 || !IsLowerHex(raw))
        {
            throw new ArgumentException($"{context} must be a lowercase 32-byte hex string without 0x prefix.", context);
        }
    }

    private static bool IsLowerHex(string value)
    {
        foreach (var c in value)
        {
            if (!((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f')))
            {
                return false;
            }
        }

        return true;
    }

    private static string BuildPaginationQuery(
        int? limit,
        long offset,
        params KeyValuePair<string, string?>[] extraParameters)
    {
        if (limit.HasValue && limit.Value <= 0)
        {
            throw new ArgumentOutOfRangeException(nameof(limit), "Pagination limit must be positive.");
        }

        if (offset < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(offset), "Pagination offset cannot be negative.");
        }

        var parameters = new List<KeyValuePair<string, string?>>(extraParameters.Length + 2);
        if (limit.HasValue)
        {
            parameters.Add(new KeyValuePair<string, string?>("limit", limit.Value.ToString(CultureInfo.InvariantCulture)));
        }

        if (offset > 0)
        {
            parameters.Add(new KeyValuePair<string, string?>("offset", offset.ToString(CultureInfo.InvariantCulture)));
        }

        parameters.AddRange(extraParameters);
        return BuildQueryString(parameters);
    }

    private static string BuildQueryString(IEnumerable<KeyValuePair<string, string?>> parameters)
    {
        var builder = new StringBuilder();
        foreach (var (name, value) in parameters)
        {
            if (string.IsNullOrWhiteSpace(value))
            {
                continue;
            }

            if (builder.Length > 0)
            {
                builder.Append('&');
            }

            builder.Append(Uri.EscapeDataString(name));
            builder.Append('=');
            builder.Append(Uri.EscapeDataString(value));
        }

        return builder.ToString();
    }

    private static string? BuildUaidManifestQuery(ToriiUaidManifestQuery? query)
    {
        if (query is null)
        {
            return null;
        }

        if (query.DataspaceId.HasValue && query.DataspaceId.Value < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(query), "DataspaceId cannot be negative.");
        }

        return BuildPaginationQuery(
            query.Limit,
            query.Offset,
            new KeyValuePair<string, string?>(
                "dataspace",
                query.DataspaceId?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>(
                "status",
                query.Status is null ? null : FormatUaidManifestStatusFilter(query.Status.Value)));
    }

    private static string? BuildContractInstancesQuery(ToriiContractInstancesQuery? query)
    {
        if (query is null)
        {
            return null;
        }

        return BuildQueryString(
        [
            new KeyValuePair<string, string?>("contains", NormalizeOptionalExactValue(query.Contains, nameof(query.Contains))),
            new KeyValuePair<string, string?>("hash_prefix", NormalizeOptionalExactHexPrefix(query.HashPrefix, nameof(query.HashPrefix))),
            new KeyValuePair<string, string?>("offset", query.Offset?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>(
                "limit",
                NormalizeOptionalPositiveUInt64(query.Limit, nameof(query.Limit))?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>("order", NormalizeOptionalExactValue(query.Order, nameof(query.Order))),
        ]);
    }

    private static string BuildContractStateQuery(ToriiContractStateQuery query)
    {
        ArgumentNullException.ThrowIfNull(query);

        var normalizedPath = query.Path is null
            ? null
            : NormalizeExactValue(query.Path, nameof(query.Path));
        var normalizedPrefix = query.Prefix is null
            ? null
            : NormalizeExactValue(query.Prefix, nameof(query.Prefix));
        var normalizedPaths = NormalizeOptionalExactPathList(query.Paths, nameof(query.Paths));

        var hasPath = normalizedPath is not null;
        var hasPaths = normalizedPaths is { Length: > 0 };
        var hasPrefix = normalizedPrefix is not null;
        var modeCount = (hasPath ? 1 : 0) + (hasPaths ? 1 : 0) + (hasPrefix ? 1 : 0);
        if (modeCount != 1)
        {
            throw new ArgumentException("Exactly one of Path, Paths, or Prefix must be provided.", nameof(query));
        }

        var encodedPaths = normalizedPaths is { Length: > 0 } paths
            ? string.Join(',', paths)
            : null;

        return BuildQueryString(
        [
            new KeyValuePair<string, string?>("path", normalizedPath),
            new KeyValuePair<string, string?>("paths", encodedPaths),
            new KeyValuePair<string, string?>("prefix", normalizedPrefix),
            new KeyValuePair<string, string?>(
                "include_value",
                query.IncludeValue.HasValue ? query.IncludeValue.Value.ToString().ToLowerInvariant() : null),
            new KeyValuePair<string, string?>("offset", query.Offset?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>(
                "limit",
                NormalizeOptionalPositiveUInt64(query.Limit, nameof(query.Limit))?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>("decode", NormalizeOptionalExactValue(query.Decode, nameof(query.Decode))),
        ]);
    }

    private static string? BuildExplorerPaginationQuery(ToriiExplorerPaginationQuery? query)
    {
        if (query is null)
        {
            return null;
        }

        ValidateExplorerPagination(query, nameof(query));

        return BuildQueryString(
        [
            new KeyValuePair<string, string?>("page", query.Page?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>("per_page", query.PerPage?.ToString(CultureInfo.InvariantCulture)),
        ]);
    }

    private static string? BuildExplorerAccountsQuery(ToriiExplorerAccountsQuery? query)
    {
        if (query is null)
        {
            return null;
        }

        ValidateExplorerCursor(query, nameof(query));

        return BuildQueryString(
        [
            new KeyValuePair<string, string?>("cursor", query.Cursor),
            new KeyValuePair<string, string?>("limit", query.Limit?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>("domain", NormalizeOptionalExactValue(query.Domain, nameof(query.Domain))),
            new KeyValuePair<string, string?>("with_asset", NormalizeOptionalExactValue(query.WithAsset, nameof(query.WithAsset))),
        ]);
    }

    private static string? BuildExplorerDomainsQuery(ToriiExplorerDomainsQuery? query)
    {
        if (query is null)
        {
            return null;
        }

        ValidateExplorerCursor(query, nameof(query));

        return BuildQueryString(
        [
            new KeyValuePair<string, string?>("cursor", query.Cursor),
            new KeyValuePair<string, string?>("limit", query.Limit?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>("owned_by", NormalizeOptionalAccountId(query.OwnedBy, nameof(query.OwnedBy))),
        ]);
    }

    private static string? BuildExplorerAssetDefinitionsQuery(ToriiExplorerAssetDefinitionsQuery? query)
    {
        if (query is null)
        {
            return null;
        }

        ValidateExplorerCursor(query, nameof(query));

        return BuildQueryString(
        [
            new KeyValuePair<string, string?>("cursor", query.Cursor),
            new KeyValuePair<string, string?>("limit", query.Limit?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>("owning_domain", NormalizeOptionalExactValue(query.OwningDomain, nameof(query.OwningDomain))),
            new KeyValuePair<string, string?>("owned_by", NormalizeOptionalAccountId(query.OwnedBy, nameof(query.OwnedBy))),
        ]);
    }

    private static string? BuildExplorerAssetsQuery(ToriiExplorerAssetsQuery? query)
    {
        if (query is null)
        {
            return null;
        }

        ValidateExplorerCursor(query, nameof(query));

        return BuildQueryString(
        [
            new KeyValuePair<string, string?>("cursor", query.Cursor),
            new KeyValuePair<string, string?>("limit", query.Limit?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>("owned_by", NormalizeOptionalAccountId(query.OwnedBy, nameof(query.OwnedBy))),
            new KeyValuePair<string, string?>("definition", NormalizeOptionalExactValue(query.Definition, nameof(query.Definition))),
            new KeyValuePair<string, string?>("asset_id", NormalizeOptionalExactValue(query.AssetId, nameof(query.AssetId))),
        ]);
    }

    private static string? BuildExplorerNftsQuery(ToriiExplorerNftsQuery? query)
    {
        if (query is null)
        {
            return null;
        }

        ValidateExplorerCursor(query, nameof(query));

        return BuildQueryString(
        [
            new KeyValuePair<string, string?>("cursor", query.Cursor),
            new KeyValuePair<string, string?>("limit", query.Limit?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>("owned_by", NormalizeOptionalAccountId(query.OwnedBy, nameof(query.OwnedBy))),
            new KeyValuePair<string, string?>("domain", NormalizeOptionalExactValue(query.Domain, nameof(query.Domain))),
        ]);
    }

    private static string? BuildExplorerRwasQuery(ToriiExplorerRwasQuery? query)
    {
        if (query is null)
        {
            return null;
        }

        ValidateExplorerCursor(query, nameof(query));

        return BuildQueryString(
        [
            new KeyValuePair<string, string?>("cursor", query.Cursor),
            new KeyValuePair<string, string?>("limit", query.Limit?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>("owned_by", NormalizeOptionalAccountId(query.OwnedBy, nameof(query.OwnedBy))),
            new KeyValuePair<string, string?>("domain", NormalizeOptionalExactValue(query.Domain, nameof(query.Domain))),
        ]);
    }

    private static string? BuildExplorerTransactionsQuery(ToriiExplorerTransactionsQuery? query)
    {
        if (query is null)
        {
            return null;
        }

        ValidateExplorerPagination(query, nameof(query));

        return BuildQueryString(
        [
            new KeyValuePair<string, string?>("page", query.Page?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>("per_page", query.PerPage?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>("authority", NormalizeOptionalAccountId(query.Authority, nameof(query.Authority))),
            new KeyValuePair<string, string?>("block", query.Block?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>(
                "status",
                query.Status is null ? null : FormatExplorerTransactionStatusFilter(query.Status.Value)),
            new KeyValuePair<string, string?>("asset_id", NormalizeOptionalExactValue(query.AssetId, nameof(query.AssetId))),
        ]);
    }

    private static string? BuildExplorerInstructionsQuery(ToriiExplorerInstructionsQuery? query)
    {
        if (query is null)
        {
            return null;
        }

        ValidateExplorerPagination(query, nameof(query));

        return BuildQueryString(
        [
            new KeyValuePair<string, string?>("page", query.Page?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>("per_page", query.PerPage?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>("authority", NormalizeOptionalAccountId(query.Authority, nameof(query.Authority))),
            new KeyValuePair<string, string?>("account", NormalizeOptionalAccountId(query.Account, nameof(query.Account))),
            new KeyValuePair<string, string?>("transaction_hash", NormalizeOptionalExactValue(query.TransactionHash, nameof(query.TransactionHash))),
            new KeyValuePair<string, string?>(
                "transaction_status",
                query.TransactionStatus is null ? null : FormatExplorerTransactionStatusFilter(query.TransactionStatus.Value)),
            new KeyValuePair<string, string?>("block", query.Block?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>("kind", NormalizeOptionalExactValue(query.Kind, nameof(query.Kind))),
            new KeyValuePair<string, string?>("asset_id", NormalizeOptionalExactValue(query.AssetId, nameof(query.AssetId))),
        ]);
    }

    private static JsonSerializerOptions CreateSerializerOptions(JsonSerializerOptions baseOptions)
    {
        ArgumentNullException.ThrowIfNull(baseOptions);

        var options = new JsonSerializerOptions(baseOptions);
        if (options.MaxDepth == 0)
        {
            options.MaxDepth = 128;
        }
        IList<IJsonTypeInfoResolver> resolverChain = options.TypeInfoResolverChain;
        if (!resolverChain.Contains(ToriiJsonSerializerContext.Default))
        {
            resolverChain.Insert(0, ToriiJsonSerializerContext.Default);
        }

        return options;
    }

    private StringContent CreateJsonContent<TRequest>(TRequest request)
    {
        var json = JsonSerializer.Serialize(request, serializerOptions);
        return new StringContent(json, Encoding.UTF8, "application/json");
    }

    private static ToriiAccountOnboardingPlanRequest NormalizeAccountOnboardingPlanRequest(
        ToriiAccountOnboardingPlanRequest request)
    {
        if (request.Version != 1)
        {
            throw new ArgumentOutOfRangeException(nameof(request.Version), "Version must be 1.");
        }
        var permissions = ToriiAccountOnboardingReceiptVerifier.NormalizePermissions(
            request.Permissions,
            nameof(request.Permissions));

        return request with
        {
            Alias = ToriiAccountOnboardingReceiptVerifier.RequireAlias(request.Alias, nameof(request.Alias)),
            AccountId = ToriiAccountOnboardingReceiptVerifier.RequireCanonicalAccountId(
                request.AccountId,
                nameof(request.AccountId)),
            Permissions = permissions,
        };
    }

    private static void RequireMatchingAccountOnboardingRequest(
        ToriiAccountOnboardingPlanRequest expected,
        ToriiAccountOnboardingPlanRequest actual,
        string context)
    {
        if (actual is null
            || expected.Version != actual.Version
            || !string.Equals(expected.Alias, actual.Alias, StringComparison.Ordinal)
            || !string.Equals(expected.AccountId, actual.AccountId, StringComparison.Ordinal)
            || !expected.Permissions.SequenceEqual(actual.Permissions, StringComparer.Ordinal))
        {
            throw new JsonException($"{context} does not contain the exact requested onboarding intent.");
        }
    }

    private static ToriiAccountFaucetRequest NormalizeAccountFaucetRequest(
        ToriiAccountFaucetRequest request)
    {
        var accountId = ToriiAccountFaucetPow.RequireExactAccountId(
            request.AccountId,
            nameof(request.AccountId));

        string? nonceHex = null;
        if (request.PowNonceHex is not null)
        {
            nonceHex = ToriiAccountFaucetPow.RequireExactHex(
                request.PowNonceHex,
                nameof(request.PowNonceHex));
            if (nonceHex.Length > 64)
            {
                throw new ArgumentException(
                    "Faucet PoW nonce must not exceed 32 bytes.",
                    nameof(request.PowNonceHex));
            }
        }

        if (request.PowAnchorHeight is null)
        {
            if (nonceHex is not null)
            {
                throw new ArgumentException(
                    "Faucet PoW nonce requires an anchor height.",
                    nameof(request.PowNonceHex));
            }
        }
        else
        {
            if (request.PowAnchorHeight.Value == 0)
            {
                throw new ArgumentOutOfRangeException(
                    nameof(request.PowAnchorHeight),
                    "Faucet PoW anchor height must be positive.");
            }
            if (nonceHex is null)
            {
                throw new ArgumentException(
                    "Faucet PoW anchor height requires a nonce.",
                    nameof(request.PowAnchorHeight));
            }
        }

        return new ToriiAccountFaucetRequest
        {
            AccountId = accountId,
            PowAnchorHeight = request.PowAnchorHeight,
            PowNonceHex = nonceHex,
        };
    }

    private static ToriiVpnQuoteCreateRequest NormalizeVpnQuoteCreateRequest(
        ToriiVpnQuoteCreateRequest request)
    {
        return request with
        {
            ExitClass = NormalizeOptionalVpnExitClass(request.ExitClass, nameof(request.ExitClass)),
            MeteringPublicKeyHex = NormalizeVpnExactSizedHex(
                request.MeteringPublicKeyHex,
                nameof(request.MeteringPublicKeyHex),
                32),
        };
    }

    private static ToriiVpnSessionCreateRequest NormalizeVpnSessionCreateRequest(
        ToriiVpnSessionCreateRequest request)
    {
        return request with
        {
            ExitClass = NormalizeOptionalVpnExitClass(request.ExitClass, nameof(request.ExitClass)),
            QuoteId = NormalizeExactSizedHex(request.QuoteId, nameof(request.QuoteId), 32),
            PaymentTransactionHash = NormalizeVpnExactSizedHex(
                request.PaymentTransactionHash,
                nameof(request.PaymentTransactionHash),
                32),
            MeteringPublicKeyHex = NormalizeVpnExactSizedHex(
                request.MeteringPublicKeyHex,
                nameof(request.MeteringPublicKeyHex),
                32),
        };
    }

    private static ToriiVpnReceiptSubmitRequest NormalizeVpnReceiptSubmitRequest(
        ToriiVpnReceiptSubmitRequest request)
    {
        return request with
        {
            RelayReceiptHex = NormalizeVpnExactHex(request.RelayReceiptHex, nameof(request.RelayReceiptHex)),
            ClientVoucherHex = NormalizeVpnExactHex(request.ClientVoucherHex, nameof(request.ClientVoucherHex)),
            LeaseIdHex = NormalizeOptionalVpnExactSizedHexOrEmpty(
                request.LeaseIdHex,
                nameof(request.LeaseIdHex),
                32),
        };
    }

    private static ToriiContractCallRequest NormalizeContractCallRequest(ToriiContractCallRequest request)
    {
        var privateKey = NormalizeOptionalExactValue(request.PrivateKey, nameof(request.PrivateKey));
        var publicKeyHex = NormalizeOptionalExactSizedHex(request.PublicKeyHex, nameof(request.PublicKeyHex), 32);
        var signatureBase64 = NormalizeOptionalExactBase64(request.SignatureBase64, nameof(request.SignatureBase64));
        ValidateSigningMaterial(privateKey, publicKeyHex, signatureBase64);
        var (contractAddress, contractAlias) = NormalizeContractTarget(
            request.ContractAddress,
            request.ContractAlias,
            nameof(request.ContractAddress),
            nameof(request.ContractAlias),
            requireTarget: true);
        if (request.CreationTimeMilliseconds == 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(request.CreationTimeMilliseconds),
                "Creation time must be positive when provided.");
        }
        if (request.TransactionTimeToLiveMilliseconds == 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(request.TransactionTimeToLiveMilliseconds),
                "Transaction TTL must be positive when provided.");
        }

        return request with
        {
            Authority = ToriiAccountFaucetPow.RequireExactAccountId(request.Authority, nameof(request.Authority)),
            PrivateKey = privateKey,
            PublicKeyHex = publicKeyHex,
            SignatureBase64 = signatureBase64,
            ContractAddress = contractAddress,
            ContractAlias = contractAlias,
            Entrypoint = NormalizeOptionalExactValue(request.Entrypoint, nameof(request.Entrypoint)),
            FeePayment = NormalizeFeePaymentIntent(
                request.FeePayment,
                nameof(request.FeePayment),
                requireGasLimit: true),
        };
    }

    private static ToriiContractViewRequest NormalizeContractViewRequest(ToriiContractViewRequest request)
    {
        var (contractAddress, contractAlias) = NormalizeContractTarget(
            request.ContractAddress,
            request.ContractAlias,
            nameof(request.ContractAddress),
            nameof(request.ContractAlias),
            requireTarget: true);
        if (request.GasLimit == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(request.GasLimit), "Gas limit must be positive.");
        }

        return request with
        {
            Authority = ToriiAccountFaucetPow.RequireExactAccountId(request.Authority, nameof(request.Authority)),
            ContractAddress = contractAddress,
            ContractAlias = contractAlias,
            Entrypoint = NormalizeOptionalExactValue(request.Entrypoint, nameof(request.Entrypoint)),
        };
    }

    private static FeePaymentIntent NormalizeFeePaymentIntent(
        FeePaymentIntent? feePayment,
        string paramName,
        bool requireGasLimit)
    {
        if (feePayment is null)
        {
            throw new ArgumentNullException(paramName, "Fee payment is required.");
        }
        if (requireGasLimit && !feePayment.GasLimit.HasValue)
        {
            throw new ArgumentException(
                "Fee payment must include a positive gas limit for contract execution.",
                paramName);
        }
        return feePayment;
    }

    private static ToriiMultisigProposeRequest NormalizeMultisigProposeRequest(
        ToriiMultisigProposeRequest request)
    {
        var (multisigAccountId, multisigAccountAlias) = NormalizeMultisigSelector(
            request.MultisigAccountId,
            request.MultisigAccountAlias);
        var publicKeyHex = NormalizeOptionalExactSizedHex(request.PublicKeyHex, nameof(request.PublicKeyHex), 32);
        var signatureBase64 = NormalizeOptionalExactBase64(request.SignatureBase64, nameof(request.SignatureBase64));
        var privateKey = NormalizeOptionalExactValue(request.PrivateKey, nameof(request.PrivateKey));
        ValidateSigningMaterial(privateKey, publicKeyHex, signatureBase64);
        if (request.CreationTimeMilliseconds == 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(request.CreationTimeMilliseconds),
                "Creation time must be positive when provided.");
        }

        return request with
        {
            MultisigAccountId = multisigAccountId,
            MultisigAccountAlias = multisigAccountAlias,
            SignerAccountId = ToriiAccountFaucetPow.RequireExactAccountId(
                request.SignerAccountId,
                nameof(request.SignerAccountId)),
            PrivateKey = privateKey,
            PublicKeyHex = publicKeyHex,
            SignatureBase64 = signatureBase64,
            FeePayment = NormalizeFeePaymentIntent(
                request.FeePayment,
                nameof(request.FeePayment),
                requireGasLimit: false),
            Instructions = NormalizeExactBase64List(
                request.Instructions,
                nameof(request.Instructions),
                allowEmpty: false),
        };
    }

    private static ToriiMultisigContractCallProposeRequest NormalizeMultisigContractCallProposeRequest(
        ToriiMultisigContractCallProposeRequest request)
    {
        var (multisigAccountId, multisigAccountAlias) = NormalizeMultisigSelector(
            request.MultisigAccountId,
            request.MultisigAccountAlias);
        var privateKey = NormalizeOptionalExactValue(request.PrivateKey, nameof(request.PrivateKey));
        var publicKeyHex = NormalizeOptionalExactSizedHex(request.PublicKeyHex, nameof(request.PublicKeyHex), 32);
        var signatureBase64 = NormalizeOptionalExactBase64(request.SignatureBase64, nameof(request.SignatureBase64));
        ValidateSigningMaterial(privateKey, publicKeyHex, signatureBase64);
        var (contractAddress, contractAlias) = NormalizeContractTarget(
            request.ContractAddress,
            request.ContractAlias,
            nameof(request.ContractAddress),
            nameof(request.ContractAlias),
            requireTarget: true);
        if (request.CreationTimeMilliseconds == 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(request.CreationTimeMilliseconds),
                "Creation time must be positive when provided.");
        }

        return request with
        {
            MultisigAccountId = multisigAccountId,
            MultisigAccountAlias = multisigAccountAlias,
            SignerAccountId = ToriiAccountFaucetPow.RequireExactAccountId(
                request.SignerAccountId,
                nameof(request.SignerAccountId)),
            PrivateKey = privateKey,
            PublicKeyHex = publicKeyHex,
            SignatureBase64 = signatureBase64,
            ContractAddress = contractAddress,
            ContractAlias = contractAlias,
            Entrypoint = NormalizeExactValue(request.Entrypoint, nameof(request.Entrypoint)),
            FeePayment = NormalizeFeePaymentIntent(
                request.FeePayment,
                nameof(request.FeePayment),
                requireGasLimit: true),
        };
    }

    private static ToriiMultisigApproveRequest NormalizeMultisigApproveRequest(
        ToriiMultisigApproveRequest request)
    {
        var (multisigAccountId, multisigAccountAlias) = NormalizeMultisigSelector(
            request.MultisigAccountId,
            request.MultisigAccountAlias);
        var privateKey = NormalizeOptionalExactValue(request.PrivateKey, nameof(request.PrivateKey));
        var publicKeyHex = NormalizeOptionalExactSizedHex(request.PublicKeyHex, nameof(request.PublicKeyHex), 32);
        var signatureBase64 = NormalizeOptionalExactBase64(request.SignatureBase64, nameof(request.SignatureBase64));
        ValidateSigningMaterial(privateKey, publicKeyHex, signatureBase64);
        ValidateMultisigProposalSelector(request.ProposalId, request.InstructionsHash);
        if (request.CreationTimeMilliseconds == 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(request.CreationTimeMilliseconds),
                "Creation time must be positive when provided.");
        }

        return request with
        {
            MultisigAccountId = multisigAccountId,
            MultisigAccountAlias = multisigAccountAlias,
            SignerAccountId = ToriiAccountFaucetPow.RequireExactAccountId(
                request.SignerAccountId,
                nameof(request.SignerAccountId)),
            PrivateKey = privateKey,
            PublicKeyHex = publicKeyHex,
            SignatureBase64 = signatureBase64,
            FeePayment = NormalizeFeePaymentIntent(
                request.FeePayment,
                nameof(request.FeePayment),
                requireGasLimit: false),
            ProposalId = request.ProposalId is null
                ? null
                : NormalizeExactSizedHex(request.ProposalId, nameof(request.ProposalId), 32),
            InstructionsHash = request.InstructionsHash is null
                ? null
                : NormalizeExactSizedHex(request.InstructionsHash, nameof(request.InstructionsHash), 32),
        };
    }

    private static ToriiMultisigCancelRequest NormalizeMultisigCancelRequest(
        ToriiMultisigCancelRequest request)
    {
        var (multisigAccountId, multisigAccountAlias) = NormalizeMultisigSelector(
            request.MultisigAccountId,
            request.MultisigAccountAlias);
        var privateKey = NormalizeOptionalExactValue(request.PrivateKey, nameof(request.PrivateKey));
        var publicKeyHex = NormalizeOptionalExactSizedHex(request.PublicKeyHex, nameof(request.PublicKeyHex), 32);
        var signatureBase64 = NormalizeOptionalExactBase64(request.SignatureBase64, nameof(request.SignatureBase64));
        ValidateSigningMaterial(privateKey, publicKeyHex, signatureBase64);
        ValidateMultisigProposalSelector(request.ProposalId, request.InstructionsHash);
        if (request.CreationTimeMilliseconds == 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(request.CreationTimeMilliseconds),
                "Creation time must be positive when provided.");
        }

        return request with
        {
            MultisigAccountId = multisigAccountId,
            MultisigAccountAlias = multisigAccountAlias,
            SignerAccountId = ToriiAccountFaucetPow.RequireExactAccountId(
                request.SignerAccountId,
                nameof(request.SignerAccountId)),
            PrivateKey = privateKey,
            PublicKeyHex = publicKeyHex,
            SignatureBase64 = signatureBase64,
            FeePayment = NormalizeFeePaymentIntent(
                request.FeePayment,
                nameof(request.FeePayment),
                requireGasLimit: false),
            ProposalId = request.ProposalId is null
                ? null
                : NormalizeExactSizedHex(request.ProposalId, nameof(request.ProposalId), 32),
            InstructionsHash = request.InstructionsHash is null
                ? null
                : NormalizeExactSizedHex(request.InstructionsHash, nameof(request.InstructionsHash), 32),
        };
    }

    private static void ValidateMultisigProposalSelector(string? proposalId, string? instructionsHash)
    {
        if (proposalId is null && instructionsHash is null)
        {
            throw new ArgumentException(
                "Provide either proposal_id or instructions_hash.",
                nameof(proposalId));
        }
    }

    private static ToriiMultisigContractCallApproveRequest NormalizeMultisigContractCallApproveRequest(
        ToriiMultisigContractCallApproveRequest request)
    {
        var (multisigAccountId, multisigAccountAlias) = NormalizeMultisigSelector(
            request.MultisigAccountId,
            request.MultisigAccountAlias);
        var privateKey = NormalizeOptionalExactValue(request.PrivateKey, nameof(request.PrivateKey));
        var publicKeyHex = NormalizeOptionalExactSizedHex(request.PublicKeyHex, nameof(request.PublicKeyHex), 32);
        var signatureBase64 = NormalizeOptionalExactBase64(request.SignatureBase64, nameof(request.SignatureBase64));
        ValidateSigningMaterial(privateKey, publicKeyHex, signatureBase64);
        if (request.ProposalId is null && request.InstructionsHash is null)
        {
            throw new ArgumentException(
                "Provide either proposal_id or instructions_hash.",
                nameof(request.ProposalId));
        }
        if (request.CreationTimeMilliseconds == 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(request.CreationTimeMilliseconds),
                "Creation time must be positive when provided.");
        }

        return request with
        {
            MultisigAccountId = multisigAccountId,
            MultisigAccountAlias = multisigAccountAlias,
            SignerAccountId = ToriiAccountFaucetPow.RequireExactAccountId(
                request.SignerAccountId,
                nameof(request.SignerAccountId)),
            PrivateKey = privateKey,
            PublicKeyHex = publicKeyHex,
            SignatureBase64 = signatureBase64,
            FeePayment = NormalizeFeePaymentIntent(
                request.FeePayment,
                nameof(request.FeePayment),
                requireGasLimit: false),
            ProposalId = request.ProposalId is null
                ? null
                : NormalizeExactSizedHex(request.ProposalId, nameof(request.ProposalId), 32),
            InstructionsHash = request.InstructionsHash is null
                ? null
                : NormalizeExactSizedHex(request.InstructionsHash, nameof(request.InstructionsHash), 32),
        };
    }

    private static ToriiContractVerifiedSourceSubmission NormalizeContractVerifiedSourceSubmission(
        ToriiContractVerifiedSourceSubmission request)
    {
        var language = NormalizeExactValue(request.Language, nameof(request.Language));
        if (!string.Equals(language, "kotodama", StringComparison.Ordinal))
        {
            throw new ArgumentException("Verified source language must be kotodama.", nameof(request.Language));
        }

        return request with
        {
            Language = language,
            SourceName = NormalizeOptionalSourceName(request.SourceName, nameof(request.SourceName)),
            SourceText = NormalizeSourceText(request.SourceText, nameof(request.SourceText)),
        };
    }

    private static ByteArrayContent CreateBinaryContent(ReadOnlyMemory<byte> bytes, string mediaType)
    {
        var content = new ByteArrayContent(bytes.ToArray());
        content.Headers.ContentType = new MediaTypeHeaderValue(mediaType);
        return content;
    }

    private static string EncodePathSegment(string? value, string paramName)
    {
        return Uri.EscapeDataString(NormalizeExactPathSegment(value, paramName));
    }

    private static string EncodeIdentifierPathSegment(string? value, string paramName)
    {
        return Uri.EscapeDataString(NormalizeExactIdentifierPathSegment(value, paramName));
    }

    private static string EncodeAccountIdPathSegment(string? value, string paramName)
    {
        return Uri.EscapeDataString(ToriiAccountFaucetPow.RequireExactAccountId(value, paramName));
    }

    private static string EncodeVpnSessionPathSegment(string? value, string paramName)
    {
        return Uri.EscapeDataString(NormalizeExactSizedHex(value, paramName, 32));
    }

    private static string EncodeSoraFsCidPathSegment(string? value, string paramName)
    {
        return Uri.EscapeDataString(NormalizeSoraFsContentCidArgument(value, paramName));
    }

    private static string EncodePathSegment(string value)
    {
        return EncodePathSegment(value, nameof(value));
    }

    private static string NormalizeExactIdentifierPathSegment(string? value, string paramName)
    {
        var normalized = NormalizeExactPathSegment(value, paramName);
        if (normalized.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Value must not contain whitespace.", paramName);
        }

        return normalized;
    }

    private static string NormalizeExactPathSegment(string? value, string paramName)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException("Value cannot be null or whitespace.", paramName);
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException("Value must not contain surrounding whitespace.", paramName);
        }

        if (value.Any(char.IsControl))
        {
            throw new ArgumentException("Value must not contain control characters.", paramName);
        }

        return value;
    }

    private static string BuildSoraFsCidGatewayPath(string cid, string? relativePath)
    {
        var builder = new StringBuilder("/sorafs/cid/");
        builder.Append(EncodeSoraFsCidPathSegment(cid, nameof(cid)));

        if (relativePath is null || relativePath.Length == 0)
        {
            return builder.ToString();
        }

        if (string.IsNullOrWhiteSpace(relativePath))
        {
            throw new ArgumentException("Relative path must not be whitespace.", nameof(relativePath));
        }

        if (!string.Equals(relativePath.Trim(), relativePath, StringComparison.Ordinal))
        {
            throw new ArgumentException("Relative path must not contain surrounding whitespace.", nameof(relativePath));
        }

        if (relativePath.Any(char.IsControl))
        {
            throw new ArgumentException("Relative path must not contain control characters.", nameof(relativePath));
        }

        var segments = relativePath.Split('/');
        for (var index = 0; index < segments.Length; index++)
        {
            ValidateSoraFsRelativePathSegment(segments[index], index, nameof(relativePath));
        }

        var encodedSegments = segments.Select(Uri.EscapeDataString);
        builder.Append('/');
        builder.Append(string.Join('/', encodedSegments));
        return builder.ToString();
    }

    private static void ValidateSoraFsRelativePathSegment(string segment, int index, string paramName)
    {
        var field = $"{nameof(BuildSoraFsCidGatewayPath)}.{nameof(segment)}[{index}]";
        if (segment.Length == 0)
        {
            throw new ArgumentException("Relative path must not contain empty path segments.", paramName);
        }

        if (string.IsNullOrWhiteSpace(segment))
        {
            throw new ArgumentException($"{field} must not be blank.", paramName);
        }

        if (!string.Equals(segment.Trim(), segment, StringComparison.Ordinal))
        {
            throw new ArgumentException($"{field} must not contain surrounding whitespace.", paramName);
        }

        if (segment.Any(char.IsControl))
        {
            throw new ArgumentException($"{field} must not contain control characters.", paramName);
        }

        if (segment is "." or ".." || segment.Contains('\\', StringComparison.Ordinal))
        {
            throw new ArgumentException($"{field} must be a relative path component.", paramName);
        }
    }

    private static JsonElement RequireJsonObject(JsonElement element, string context)
    {
        if (element.ValueKind != JsonValueKind.Object)
        {
            throw new JsonException($"{context} must be an object.");
        }

        return element;
    }

    private static JsonElement RequireJsonObjectProperty(JsonElement element, string propertyName, string context)
    {
        if (!element.TryGetProperty(propertyName, out var property) || property.ValueKind == JsonValueKind.Null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        return RequireJsonObject(property, context);
    }

    private static bool TryReadOptionalJsonObjectProperty(
        JsonElement element,
        string propertyName,
        string context,
        out JsonElement value)
    {
        if (!element.TryGetProperty(propertyName, out var property) || property.ValueKind == JsonValueKind.Null)
        {
            value = default;
            return false;
        }

        value = RequireJsonObject(property, context);
        return true;
    }

    private static string RequireJsonStringProperty(JsonElement element, string propertyName, string context)
    {
        if (!element.TryGetProperty(propertyName, out var property) || property.ValueKind == JsonValueKind.Null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        if (property.ValueKind != JsonValueKind.String)
        {
            throw new JsonException($"{context} must be a string.");
        }

        return property.GetString() ?? throw new JsonException($"{context} must not be null.");
    }

    private static string? ReadOptionalJsonStringProperty(JsonElement element, string propertyName, string context)
    {
        if (!element.TryGetProperty(propertyName, out var property) || property.ValueKind == JsonValueKind.Null)
        {
            return null;
        }

        if (property.ValueKind != JsonValueKind.String)
        {
            throw new JsonException($"{context} must be a string.");
        }

        return property.GetString();
    }

    private static ulong ValidatePositiveJsonUInt64Property(
        JsonElement element,
        string propertyName,
        string context)
    {
        var value = RequireJsonUInt64Property(element, propertyName, context);
        if (value == 0)
        {
            throw new JsonException($"{context} must be positive.");
        }

        return value;
    }

    private static ulong RequireJsonUInt64Property(JsonElement element, string propertyName, string context)
    {
        if (!element.TryGetProperty(propertyName, out var property) || property.ValueKind == JsonValueKind.Null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        return ReadJsonUInt64(property, context);
    }

    private static ulong? ReadOptionalPositiveJsonUInt64Property(
        JsonElement element,
        string propertyName,
        string context)
    {
        var value = ReadOptionalJsonUInt64Property(element, propertyName, context);
        if (value == 0)
        {
            throw new JsonException($"{context} must be positive.");
        }

        return value;
    }

    private static ulong? ReadOptionalJsonUInt64Property(JsonElement element, string propertyName, string context)
    {
        if (!element.TryGetProperty(propertyName, out var property) || property.ValueKind == JsonValueKind.Null)
        {
            return null;
        }

        return ReadJsonUInt64(property, context);
    }

    private static ulong ReadJsonUInt64(JsonElement element, string context)
    {
        if (element.ValueKind != JsonValueKind.Number || !element.TryGetUInt64(out var value))
        {
            throw new JsonException($"{context} must be an unsigned integer.");
        }

        return value;
    }

    private static string NormalizeSoraFsStorageClass(ToriiSoraFsStorageClass? storageClass)
    {
        if (storageClass is null)
        {
            throw new ArgumentNullException(nameof(storageClass));
        }

        var normalized = NormalizeExactValue(storageClass.Type, nameof(storageClass.Type)).ToLowerInvariant();
        return normalized switch
        {
            "hot" => "Hot",
            "warm" => "Warm",
            "cold" => "Cold",
            _ => throw new ArgumentException("SoraFS storage class must be Hot, Warm, or Cold.", nameof(storageClass)),
        };
    }

    private static uint RequireSoraFsUnsigned(uint? value, string paramName, bool allowZero)
    {
        if (value is null)
        {
            throw new ArgumentException("Value must be provided.", paramName);
        }

        if (!allowZero && value.Value == 0)
        {
            throw new ArgumentOutOfRangeException(paramName, "Value must be positive.");
        }

        return value.Value;
    }

    private static ulong RequireSoraFsUnsigned(ulong? value, string paramName, bool allowZero)
    {
        if (value is null)
        {
            throw new ArgumentException("Value must be provided.", paramName);
        }

        if (!allowZero && value.Value == 0)
        {
            throw new ArgumentOutOfRangeException(paramName, "Value must be positive.");
        }

        return value.Value;
    }

    private static string NormalizeSoraFsDigestHex(string? value, string paramName)
    {
        var normalized = NormalizeExactValue(value, paramName);
        if (normalized.StartsWith("0x", StringComparison.OrdinalIgnoreCase))
        {
            normalized = normalized[2..];
        }

        if (normalized.Length != 64 || !IsHex(normalized))
        {
            throw new ArgumentException("Value must be a 32-byte hex string.", paramName);
        }

        return normalized.ToLowerInvariant();
    }

    private static string NormalizeNonzeroSoraFsDigestHex(string? value, string paramName)
    {
        var normalized = NormalizeSoraFsDigestHex(value, paramName);
        if (normalized.All(character => character == '0'))
        {
            throw new ArgumentException("Value must not be the all-zero digest.", paramName);
        }
        return normalized;
    }

    private static string NormalizeRequiredBase64(string? value, string paramName)
    {
        var normalized = NormalizeExactValue(value, paramName);

        byte[] bytes;
        try
        {
            bytes = Convert.FromBase64String(normalized);
        }
        catch (FormatException exception)
        {
            throw new ArgumentException("Value must be base64 encoded.", paramName, exception);
        }

        if (bytes.Length == 0)
        {
            throw new ArgumentException("Value must be a non-empty base64 payload.", paramName);
        }

        var canonical = Convert.ToBase64String(bytes);
        if (!string.Equals(canonical, normalized, StringComparison.Ordinal))
        {
            throw new ArgumentException("Value must be canonical base64 text.", paramName);
        }

        return canonical;
    }

    private static string NormalizeBoundedRequiredBase64(
        string? value,
        string paramName,
        int maximumBytes,
        int maximumBase64Chars)
    {
        if (value is not null && value.Length > maximumBase64Chars)
        {
            throw new ArgumentOutOfRangeException(
                paramName,
                $"Value must encode at most {maximumBytes} bytes.");
        }
        var canonical = NormalizeRequiredBase64(value, paramName);
        if (Convert.FromBase64String(canonical).Length > maximumBytes)
        {
            throw new ArgumentOutOfRangeException(
                paramName,
                $"Value must encode at most {maximumBytes} bytes.");
        }
        return canonical;
    }

    private static bool IsHex(string value)
    {
        foreach (var c in value)
        {
            var digit =
                c is >= '0' and <= '9'
                || c is >= 'a' and <= 'f'
                || c is >= 'A' and <= 'F';
            if (!digit)
            {
                return false;
            }
        }

        return true;
    }

    private static bool IsLowercaseHex(string value)
    {
        foreach (var c in value)
        {
            var digit = c is >= '0' and <= '9' || c is >= 'a' and <= 'f';
            if (!digit)
            {
                return false;
            }
        }

        return true;
    }

    private static string NormalizeExactValue(string? value, string paramName)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException("Value cannot be null or whitespace.", paramName);
        }

        if (value.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Value must not contain whitespace.", paramName);
        }

        if (value.Any(char.IsControl))
        {
            throw new ArgumentException("Value must not contain control characters.", paramName);
        }

        return value;
    }

    private static string? NormalizeOptionalExactValue(string? value, string paramName)
    {
        return value is null ? null : NormalizeExactValue(value, paramName);
    }

    private static string? NormalizeOptionalExactHexPrefix(string? value, string paramName)
    {
        if (value is null)
        {
            return null;
        }

        var exact = NormalizeExactValue(value, paramName);
        if (exact.Length > 64 || !exact.All(static character => Uri.IsHexDigit(character)))
        {
            throw new ArgumentException("Value must be a hexadecimal prefix up to 32 bytes.", paramName);
        }

        return exact.ToLowerInvariant();
    }

    private static string[]? NormalizeOptionalExactPathList(IReadOnlyList<string>? values, string paramName)
    {
        if (values is null)
        {
            return null;
        }

        if (values.Count == 0)
        {
            throw new ArgumentException("Value list cannot be empty.", paramName);
        }

        var normalized = new string[values.Count];
        for (var index = 0; index < values.Count; index++)
        {
            normalized[index] = NormalizeExactValue(values[index], $"{paramName}[{index}]");
        }

        return normalized;
    }

    private static IReadOnlyList<string> NormalizeExactStringList(
        IReadOnlyList<string>? values,
        string paramName,
        bool allowEmpty)
    {
        if (values is null)
        {
            throw new ArgumentException("Value list cannot be null.", paramName);
        }
        if (!allowEmpty && values.Count == 0)
        {
            throw new ArgumentException("Value list must not be empty.", paramName);
        }

        var normalized = new string[values.Count];
        for (var index = 0; index < values.Count; index++)
        {
            normalized[index] = NormalizeExactValue(values[index], $"{paramName}[{index}]");
        }

        return normalized;
    }

    private static IReadOnlyList<string> NormalizeAccountIdList(
        IReadOnlyList<string>? values,
        string paramName,
        bool allowEmpty)
    {
        if (values is null)
        {
            throw new ArgumentException("Value list cannot be null.", paramName);
        }
        if (!allowEmpty && values.Count == 0)
        {
            throw new ArgumentException("Value list must not be empty.", paramName);
        }

        var normalized = new string[values.Count];
        for (var index = 0; index < values.Count; index++)
        {
            normalized[index] = ToriiAccountFaucetPow.RequireExactAccountId(
                values[index],
                $"{paramName}[{index}]");
        }

        return normalized;
    }

    private static string? NormalizeOptionalExactSizedHex(
        string? value,
        string paramName,
        int expectedBytes)
    {
        if (value is null)
        {
            return null;
        }

        var exact = NormalizeExactValue(value, paramName);
        if (exact.Length != expectedBytes * 2 || !exact.All(Uri.IsHexDigit))
        {
            throw new ArgumentException(
                $"Value must be a {expectedBytes}-byte hex string.",
                paramName);
        }

        return exact.ToLowerInvariant();
    }

    private static string NormalizeExactSizedHex(
        string? value,
        string paramName,
        int expectedBytes)
    {
        return NormalizeOptionalExactSizedHex(value, paramName, expectedBytes)
            ?? throw new ArgumentException("Value cannot be null or whitespace.", paramName);
    }

    private static string NormalizeVpnExactSizedHex(
        string? value,
        string paramName,
        int expectedBytes)
    {
        var exact = NormalizeExactValue(value, paramName);
        var payload = RemoveOptionalHexPrefix(exact);
        if (payload.Length != expectedBytes * 2 || !payload.All(Uri.IsHexDigit))
        {
            throw new ArgumentException(
                $"Value must be a {expectedBytes}-byte hex string with an optional 0x prefix.",
                paramName);
        }

        return payload.ToLowerInvariant();
    }

    private static string NormalizeOptionalVpnExactSizedHexOrEmpty(
        string? value,
        string paramName,
        int expectedBytes)
    {
        if (value is null || value.Length == 0)
        {
            return string.Empty;
        }

        return NormalizeVpnExactSizedHex(value, paramName, expectedBytes);
    }

    private static string NormalizeVpnExactHex(string? value, string paramName)
    {
        var exact = NormalizeExactValue(value, paramName);
        var payload = RemoveOptionalHexPrefix(exact);
        if (payload.Length == 0 || payload.Length % 2 != 0 || !IsHex(payload))
        {
            throw new ArgumentException(
                "Value must contain a non-empty even number of hexadecimal characters with an optional 0x prefix.",
                paramName);
        }

        return payload.ToLowerInvariant();
    }

    private static string RemoveOptionalHexPrefix(string value)
    {
        return value.StartsWith("0x", StringComparison.OrdinalIgnoreCase) ? value[2..] : value;
    }

    private static string NormalizeOptionalVpnExitClass(string? value, string paramName)
    {
        if (value is null || value.Length == 0)
        {
            return string.Empty;
        }

        return NormalizeExactValue(value, paramName);
    }

    private static string NormalizeSoraFsContentCidArgument(string? value, string paramName)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException("Value cannot be null or whitespace.", paramName);
        }

        if (value.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Value must not contain whitespace.", paramName);
        }

        if (value.Any(char.IsControl))
        {
            throw new ArgumentException("Value must not contain control characters.", paramName);
        }

        if (value[0] != 'b' || value.Length == 1)
        {
            throw new ArgumentException("Value must be lowercase multibase base32 CID text.", paramName);
        }

        for (var index = 1; index < value.Length; index++)
        {
            var character = value[index];
            if (character is not (>= 'a' and <= 'z') and not (>= '2' and <= '7'))
            {
                throw new ArgumentException("Value must be lowercase multibase base32 CID text.", paramName);
            }
        }

        return value;
    }

    private static string NormalizeExactBase64(string? value, string paramName)
    {
        var exact = NormalizeExactValue(value, paramName);
        byte[] bytes;
        try
        {
            bytes = Convert.FromBase64String(exact);
        }
        catch (FormatException exception)
        {
            throw new ArgumentException("Value must be base64 encoded.", paramName, exception);
        }

        if (bytes.Length == 0)
        {
            throw new ArgumentException("Value must be a non-empty base64 payload.", paramName);
        }

        var canonical = Convert.ToBase64String(bytes);
        if (!string.Equals(canonical, exact, StringComparison.Ordinal))
        {
            throw new ArgumentException("Value must be canonical base64 text.", paramName);
        }

        return canonical;
    }

    private static string? NormalizeOptionalExactBase64(string? value, string paramName)
    {
        return value is null ? null : NormalizeExactBase64(value, paramName);
    }

    private static IReadOnlyList<string> NormalizeExactBase64List(
        IReadOnlyList<string>? values,
        string paramName,
        bool allowEmpty)
    {
        if (values is null)
        {
            throw new ArgumentException("Value list cannot be null.", paramName);
        }
        if (!allowEmpty && values.Count == 0)
        {
            throw new ArgumentException("Value list must not be empty.", paramName);
        }

        var normalized = new string[values.Count];
        for (var index = 0; index < values.Count; index++)
        {
            normalized[index] = NormalizeExactBase64(values[index], $"{paramName}[{index}]");
        }

        return normalized;
    }

    private static (string? ContractAddress, string? ContractAlias) NormalizeContractTarget(
        string? contractAddress,
        string? contractAlias,
        string contractAddressParamName,
        string contractAliasParamName,
        bool requireTarget)
    {
        var normalizedAddress = NormalizeOptionalExactValue(contractAddress, contractAddressParamName);
        var normalizedAlias = NormalizeOptionalExactValue(contractAlias, contractAliasParamName);
        if (normalizedAddress is not null && normalizedAlias is not null)
        {
            throw new ArgumentException(
                "Provide exactly one of contract_address or contract_alias.",
                contractAddressParamName);
        }
        if (requireTarget && normalizedAddress is null && normalizedAlias is null)
        {
            throw new ArgumentException(
                "Provide either contract_address or contract_alias.",
                contractAddressParamName);
        }

        return (normalizedAddress, normalizedAlias);
    }

    private static (string? MultisigAccountId, string? MultisigAccountAlias) NormalizeMultisigSelector(
        string? multisigAccountId,
        string? multisigAccountAlias)
    {
        var normalizedAccountId = NormalizeOptionalAccountId(
            multisigAccountId,
            nameof(ToriiMultisigProposeRequest.MultisigAccountId));
        var normalizedAlias = NormalizeOptionalExactValue(
            multisigAccountAlias,
            nameof(ToriiMultisigProposeRequest.MultisigAccountAlias));
        if (normalizedAccountId is not null && normalizedAlias is not null)
        {
            throw new ArgumentException(
                "Provide exactly one of multisig_account_id or multisig_account_alias.",
                nameof(ToriiMultisigProposeRequest.MultisigAccountId));
        }
        if (normalizedAccountId is null && normalizedAlias is null)
        {
            throw new ArgumentException(
                "Provide either multisig_account_id or multisig_account_alias.",
                nameof(ToriiMultisigProposeRequest.MultisigAccountId));
        }

        return (normalizedAccountId, normalizedAlias);
    }

    private static string? NormalizeOptionalAccountId(string? value, string paramName)
    {
        return value is null
            ? null
            : ToriiAccountFaucetPow.RequireExactAccountId(value, paramName);
    }

    private static void ValidateDetachedSigningPair(string? publicKeyHex, string? signatureBase64)
    {
        if ((publicKeyHex is null) != (signatureBase64 is null))
        {
            throw new ArgumentException(
                "Detached signing requires both public_key_hex and signature_b64.",
                publicKeyHex is null ? nameof(ToriiContractCallRequest.PublicKeyHex) : nameof(ToriiContractCallRequest.SignatureBase64));
        }
    }

    private static void ValidateSigningMaterial(
        string? privateKey,
        string? publicKeyHex,
        string? signatureBase64)
    {
        ValidateDetachedSigningPair(publicKeyHex, signatureBase64);
        if (privateKey is not null && publicKeyHex is not null)
        {
            throw new ArgumentException(
                "Provide either private_key or detached public_key_hex/signature_b64, not both.",
                nameof(ToriiContractCallRequest.PrivateKey));
        }
    }

    private static string NormalizeSourceText(string? value, string paramName)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException("Value cannot be null or whitespace.", paramName);
        }

        return value;
    }

    private static string? NormalizeOptionalSourceName(string? value, string paramName)
    {
        if (value is null)
        {
            return null;
        }
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException("Value cannot be null or whitespace.", paramName);
        }
        if (value.Any(char.IsControl))
        {
            throw new ArgumentException("Value must not contain control characters.", paramName);
        }

        return value;
    }

    private static ReadOnlyMemory<byte> NormalizeNonEmptyBinaryPayload(
        ReadOnlyMemory<byte> bytes,
        string paramName)
    {
        if (bytes.Length == 0)
        {
            throw new ArgumentException("Binary payload must not be empty.", paramName);
        }

        return bytes;
    }

    private static string NormalizeIdentifierPolicyId(string? value, string paramName)
    {
        var exact = NormalizeExactIdentifierValue(value, paramName);
        var separator = exact.IndexOf('#', StringComparison.Ordinal);
        if (separator <= 0 || separator != exact.LastIndexOf('#') || separator == exact.Length - 1)
        {
            throw new ArgumentException("Value must use `kind#rule`.", paramName);
        }

        NormalizeExactIdentifierValue(exact[..separator], $"{paramName}.kind");
        NormalizeExactIdentifierValue(exact[(separator + 1)..], $"{paramName}.rule");
        return exact;
    }

    private static string? NormalizeOptionalIdentifierCiphertext(string? value, string paramName)
    {
        return value is null ? null : NormalizeExactIdentifierValue(value, paramName);
    }

    private static string NormalizeExactIdentifierValue(string? value, string paramName)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException("Value cannot be null or whitespace.", paramName);
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException("Value must not contain surrounding whitespace.", paramName);
        }

        if (value.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Value must not contain whitespace.", paramName);
        }

        if (value.Any(char.IsControl))
        {
            throw new ArgumentException("Value must not contain control characters.", paramName);
        }

        return value;
    }

    private static void ValidateExplorerPagination(ToriiExplorerPaginationQuery query, string paramName)
    {
        if (query.Page == 0)
        {
            throw new ArgumentOutOfRangeException(paramName, "Explorer page numbers must be positive when provided.");
        }

        if (query.PerPage == 0)
        {
            throw new ArgumentOutOfRangeException(paramName, "Explorer per-page values must be positive when provided.");
        }
    }

    private static PipelineTransactionStatus ParsePipelineTransactionStatus(JsonElement root, string transactionHashHex)
    {
        const string context = "pipeline transaction status response";
        var rootObject = RequireJsonObject(root, context);
        JsonElement content;
        if (rootObject.TryGetProperty("content", out var contentElement))
        {
            if (contentElement.ValueKind == JsonValueKind.Null)
            {
                throw new JsonException($"{context}.content must not be null.");
            }

            content = RequireJsonObject(contentElement, $"{context}.content");
        }
        else
        {
            content = rootObject;
        }

        var statusElement = content.TryGetProperty("status", out var explicitStatus)
            ? explicitStatus
            : rootObject.TryGetProperty("status", out var rootStatus)
                ? rootStatus
                : throw new JsonException($"{context}.status must not be null.");

        var rawKind = ReadPipelineStatusKind(statusElement, $"{context}.status");

        var rejectionContentBase64 = ReadPipelineRejectionContent(statusElement, $"{context}.status.content");
        ValidateOptionalBase64(
            rejectionContentBase64,
            $"{context}.status.content");

        var hashHex = ReadOptionalJsonStringProperty(content, "hash", $"{context}.content.hash") is { } responseHash
            ? NormalizePipelineResponseTransactionHashHex(responseHash, $"{context}.content.hash")
            : transactionHashHex;
        var blockHeight = statusElement.ValueKind == JsonValueKind.Object
            ? ReadOptionalJsonUInt64Property(statusElement, "block_height", $"{context}.status.block_height")
            : null;
        blockHeight ??= ReadOptionalJsonUInt64Property(content, "block_height", $"{context}.content.block_height");
        var scope = ReadOptionalJsonStringProperty(content, "scope", $"{context}.content.scope");
        if (scope is not null)
        {
            ValidateExactTokenText(scope, $"{context}.content.scope");
        }

        var resolvedFrom = ReadOptionalJsonStringProperty(content, "resolved_from", $"{context}.content.resolved_from");
        if (resolvedFrom is not null)
        {
            ValidateExactTokenText(resolvedFrom, $"{context}.content.resolved_from");
        }

        return new PipelineTransactionStatus
        {
            HashHex = hashHex,
            RawKind = rawKind,
            State = ParsePipelineTransactionState(rawKind),
            BlockHeight = blockHeight,
            Scope = scope is null ? string.Empty : scope,
            ResolvedFrom = resolvedFrom is null ? string.Empty : resolvedFrom,
            RejectionContentBase64 = rejectionContentBase64,
        };
    }

    private static string ReadPipelineStatusKind(JsonElement statusElement, string context)
    {
        if (statusElement.ValueKind == JsonValueKind.Null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        string rawKind;
        if (statusElement.ValueKind == JsonValueKind.String)
        {
            rawKind = statusElement.GetString() ?? throw new JsonException($"{context} must not be null.");
        }
        else if (statusElement.ValueKind == JsonValueKind.Object)
        {
            rawKind = RequireJsonStringProperty(statusElement, "kind", $"{context}.kind");
        }
        else
        {
            throw new JsonException($"{context} must be a string or object.");
        }

        ValidateExactTokenText(rawKind, $"{context}.kind");
        return rawKind;
    }

    private static string? ReadPipelineRejectionContent(JsonElement statusElement, string context)
    {
        if (statusElement.ValueKind != JsonValueKind.Object
            || !statusElement.TryGetProperty("content", out var rejectionElement)
            || rejectionElement.ValueKind == JsonValueKind.Null)
        {
            return null;
        }

        if (rejectionElement.ValueKind != JsonValueKind.String)
        {
            throw new JsonException($"{context} must be a string.");
        }

        return rejectionElement.GetString();
    }

    private static string NormalizePipelineResponseTransactionHashHex(string value, string context)
    {
        try
        {
            return NormalizeTransactionHashHex(value);
        }
        catch (ArgumentException error)
        {
            throw new JsonException($"{context}: {error.Message}", error);
        }
    }

    private static PipelineTransactionState ParsePipelineTransactionState(string rawKind)
    {
        return rawKind switch
        {
            "Queued" => PipelineTransactionState.Queued,
            "Approved" => PipelineTransactionState.Approved,
            "Committed" => PipelineTransactionState.Committed,
            "Applied" => PipelineTransactionState.Applied,
            "Rejected" => PipelineTransactionState.Rejected,
            "Expired" => PipelineTransactionState.Expired,
            _ => PipelineTransactionState.Unknown,
        };
    }

    private static string NormalizeTransactionHashHex(string transactionHashHex)
    {
        var exact = NormalizeExactValue(transactionHashHex, nameof(transactionHashHex));
        var normalized = exact.StartsWith("0x", StringComparison.OrdinalIgnoreCase)
            ? exact[2..]
            : exact;

        if (normalized.Length != 64 || !normalized.All(static character => Uri.IsHexDigit(character)))
        {
            throw new ArgumentException("Transaction hash must be a 32-byte hex string.", nameof(transactionHashHex));
        }

        return normalized.ToLowerInvariant();
    }

    private static string NormalizePipelineScope(string scope)
    {
        if (scope is null || scope.Length == 0)
        {
            return "auto";
        }

        var normalized = NormalizeExactValue(scope, nameof(scope)).ToLowerInvariant();
        return normalized switch
        {
            "auto" or "local" or "global" => normalized,
            _ => throw new ArgumentException("Pipeline scope must be `auto`, `local`, or `global`.", nameof(scope)),
        };
    }

    private static string FormatUaidManifestStatusFilter(ToriiUaidManifestStatusFilter status)
    {
        return status switch
        {
            ToriiUaidManifestStatusFilter.Active => "active",
            ToriiUaidManifestStatusFilter.Inactive => "inactive",
            ToriiUaidManifestStatusFilter.All => "all",
            _ => throw new ArgumentOutOfRangeException(nameof(status), status, "Unknown UAID manifest status filter."),
        };
    }

    private static string FormatExplorerTransactionStatusFilter(ToriiExplorerTransactionStatusFilter status)
    {
        return status switch
        {
            ToriiExplorerTransactionStatusFilter.Committed => "committed",
            ToriiExplorerTransactionStatusFilter.Rejected => "rejected",
            _ => throw new ArgumentOutOfRangeException(nameof(status), status, "Unknown explorer transaction status filter."),
        };
    }

    private static string NormalizeUaidLiteral(string? raw, string paramName = "raw")
    {
        var exact = NormalizeExactValue(raw, paramName);
        var hexPortion = exact.StartsWith("uaid:", StringComparison.OrdinalIgnoreCase)
            ? exact[5..]
            : exact;

        if (hexPortion.Length != 64 || !hexPortion.All(static character => Uri.IsHexDigit(character)))
        {
            throw new ArgumentException(
                "UAID literal must be `uaid:<64 hex chars>` or a bare 64-character hex string.",
                paramName);
        }

        if ((HexNibble(hexPortion[^1]) & 1) == 0)
        {
            throw new ArgumentException(
                "UAID literal must have the canonical low bit set.",
                paramName);
        }

        return $"uaid:{hexPortion.ToLowerInvariant()}";
    }

    private static int HexNibble(char character)
    {
        if (character >= '0' && character <= '9')
        {
            return character - '0';
        }
        if (character >= 'a' && character <= 'f')
        {
            return character - 'a' + 10;
        }
        return character - 'A' + 10;
    }

    private static Uri NormalizeBaseUri(Uri baseUri)
    {
        if (!baseUri.IsAbsoluteUri)
        {
            throw new ArgumentException("baseUri must be absolute.", nameof(baseUri));
        }

        if (!string.Equals(baseUri.Scheme, Uri.UriSchemeHttp, StringComparison.OrdinalIgnoreCase)
            && !string.Equals(baseUri.Scheme, Uri.UriSchemeHttps, StringComparison.OrdinalIgnoreCase))
        {
            throw new ArgumentException("baseUri must use http or https.", nameof(baseUri));
        }

        if (!string.IsNullOrEmpty(baseUri.UserInfo))
        {
            throw new ArgumentException("baseUri must not include user information.", nameof(baseUri));
        }

        if (!string.IsNullOrEmpty(baseUri.Query))
        {
            throw new ArgumentException("baseUri must not include a query string.", nameof(baseUri));
        }

        if (!string.IsNullOrEmpty(baseUri.Fragment))
        {
            throw new ArgumentException("baseUri must not include a fragment.", nameof(baseUri));
        }

        return baseUri.AbsoluteUri.Length > 0 && baseUri.AbsoluteUri[^1] == '/'
            ? baseUri
            : new Uri($"{baseUri.AbsoluteUri}/", UriKind.Absolute);
    }
}
