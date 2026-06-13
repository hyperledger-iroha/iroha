using System.Globalization;
using System.Net;
using System.Net.Http.Headers;
using System.Runtime.CompilerServices;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization.Metadata;
using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.Queries;
using Hyperledger.Iroha.Transactions;
using Hyperledger.Iroha.Zk;

namespace Hyperledger.Iroha.Torii;

public sealed class ToriiClient : IDisposable
{
    private readonly bool ownsHttpClient;
    private readonly JsonSerializerOptions serializerOptions;

    public ToriiClient(Uri baseUri, HttpClient? httpClient = null, ToriiClientOptions? options = null)
    {
        ArgumentNullException.ThrowIfNull(baseUri);

        BaseUri = EnsureTrailingSlash(baseUri);
        HttpClient = httpClient ?? new HttpClient();
        ownsHttpClient = httpClient is null;
        Options = options ?? new ToriiClientOptions();
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
        return await JsonDocument.ParseAsync(stream, cancellationToken: cancellationToken);
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

    public async Task<string> GetHealthAsync(CancellationToken cancellationToken = default)
    {
        using var response = await SendAsync(HttpMethod.Get, "/v1/health", cancellationToken: cancellationToken);
        return await response.Content.ReadAsStringAsync(cancellationToken);
    }

    public Task<ToriiAccountsPage> GetAccountsAsync(
        int? limit = null,
        long offset = 0,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiAccountsPage>(
            "/v1/accounts",
            BuildPaginationQuery(limit, offset),
            cancellationToken);
    }

    public Task<ToriiExplorerAccountQrSnapshot> GetExplorerAccountQrAsync(
        string accountId,
        CancellationToken cancellationToken = default)
    {
        var encodedAccountId = EncodePathSegment(accountId);
        return GetAsync<ToriiExplorerAccountQrSnapshot>(
            $"/v1/explorer/accounts/{encodedAccountId}/qr",
            cancellationToken: cancellationToken);
    }

    public Task<ToriiExplorerAccountsPage> GetExplorerAccountsAsync(
        ToriiExplorerAccountsQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerAccountsPage>(
            "/v1/explorer/accounts",
            BuildExplorerAccountsQuery(query),
            cancellationToken);
    }

    public Task<ToriiExplorerAccount> GetExplorerAccountAsync(
        string accountId,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerAccount>(
            $"/v1/explorer/accounts/{EncodePathSegment(accountId)}",
            cancellationToken: cancellationToken);
    }

    public Task<ToriiExplorerDomainsPage> GetExplorerDomainsAsync(
        ToriiExplorerDomainsQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerDomainsPage>(
            "/v1/explorer/domains",
            BuildExplorerDomainsQuery(query),
            cancellationToken);
    }

    public Task<ToriiExplorerDomain> GetExplorerDomainAsync(
        string domainId,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerDomain>(
            $"/v1/explorer/domains/{EncodePathSegment(domainId)}",
            cancellationToken: cancellationToken);
    }

    public Task<ToriiExplorerAssetDefinitionsPage> GetExplorerAssetDefinitionsAsync(
        ToriiExplorerAssetDefinitionsQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerAssetDefinitionsPage>(
            "/v1/explorer/asset-definitions",
            BuildExplorerAssetDefinitionsQuery(query),
            cancellationToken);
    }

    public Task<ToriiExplorerAssetDefinition> GetExplorerAssetDefinitionAsync(
        string definitionId,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerAssetDefinition>(
            $"/v1/explorer/asset-definitions/{EncodePathSegment(definitionId)}",
            cancellationToken: cancellationToken);
    }

    public Task<ToriiExplorerAssetDefinitionEconometrics> GetExplorerAssetDefinitionEconometricsAsync(
        string definitionId,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerAssetDefinitionEconometrics>(
            $"/v1/explorer/asset-definitions/{EncodePathSegment(definitionId)}/econometrics",
            cancellationToken: cancellationToken);
    }

    public Task<ToriiExplorerAssetDefinitionSnapshot> GetExplorerAssetDefinitionSnapshotAsync(
        string definitionId,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerAssetDefinitionSnapshot>(
            $"/v1/explorer/asset-definitions/{EncodePathSegment(definitionId)}/snapshot",
            cancellationToken: cancellationToken);
    }

    public Task<ToriiExplorerAssetsPage> GetExplorerAssetsAsync(
        ToriiExplorerAssetsQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerAssetsPage>(
            "/v1/explorer/assets",
            BuildExplorerAssetsQuery(query),
            cancellationToken);
    }

    public Task<ToriiExplorerAsset> GetExplorerAssetAsync(
        string assetId,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerAsset>(
            $"/v1/explorer/assets/{EncodePathSegment(assetId)}",
            cancellationToken: cancellationToken);
    }

    public Task<ToriiExplorerNftsPage> GetExplorerNftsAsync(
        ToriiExplorerNftsQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerNftsPage>(
            "/v1/explorer/nfts",
            BuildExplorerNftsQuery(query),
            cancellationToken);
    }

    public Task<ToriiExplorerNft> GetExplorerNftAsync(
        string nftId,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerNft>(
            $"/v1/explorer/nfts/{EncodePathSegment(nftId)}",
            cancellationToken: cancellationToken);
    }

    public Task<ToriiExplorerRwasPage> GetExplorerRwasAsync(
        ToriiExplorerRwasQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerRwasPage>(
            "/v1/explorer/rwas",
            BuildExplorerRwasQuery(query),
            cancellationToken);
    }

    public Task<ToriiExplorerRwa> GetExplorerRwaAsync(
        string rwaId,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerRwa>(
            $"/v1/explorer/rwas/{EncodePathSegment(rwaId)}",
            cancellationToken: cancellationToken);
    }

    public Task<ToriiExplorerBlocksPage> GetExplorerBlocksAsync(
        ToriiExplorerPaginationQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerBlocksPage>(
            "/v1/explorer/blocks",
            BuildExplorerPaginationQuery(query),
            cancellationToken);
    }

    public Task<ToriiExplorerBlock> GetExplorerBlockAsync(
        string identifier,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerBlock>(
            $"/v1/explorer/blocks/{EncodePathSegment(identifier)}",
            cancellationToken: cancellationToken);
    }

    public Task<ToriiExplorerTransactionsPage> GetExplorerTransactionsAsync(
        ToriiExplorerTransactionsQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerTransactionsPage>(
            "/v1/explorer/transactions",
            BuildExplorerTransactionsQuery(query),
            cancellationToken);
    }

    public Task<ToriiExplorerLatestTransactionsResponse> GetExplorerLatestTransactionsAsync(
        ToriiExplorerTransactionsQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerLatestTransactionsResponse>(
            "/v1/explorer/transactions/latest",
            BuildExplorerTransactionsQuery(query),
            cancellationToken);
    }

    public Task<ToriiExplorerTransactionDetail> GetExplorerTransactionAsync(
        string transactionHash,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerTransactionDetail>(
            $"/v1/explorer/transactions/{EncodePathSegment(transactionHash)}",
            cancellationToken: cancellationToken);
    }

    public Task<ToriiExplorerInstructionsPage> GetExplorerInstructionsAsync(
        ToriiExplorerInstructionsQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerInstructionsPage>(
            "/v1/explorer/instructions",
            BuildExplorerInstructionsQuery(query),
            cancellationToken);
    }

    public Task<ToriiExplorerLatestInstructionsResponse> GetExplorerLatestInstructionsAsync(
        ToriiExplorerInstructionsQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerLatestInstructionsResponse>(
            "/v1/explorer/instructions/latest",
            BuildExplorerInstructionsQuery(query),
            cancellationToken);
    }

    public Task<ToriiExplorerInstruction> GetExplorerInstructionAsync(
        string transactionHash,
        ulong index,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerInstruction>(
            $"/v1/explorer/instructions/{EncodePathSegment(transactionHash)}/{index.ToString(CultureInfo.InvariantCulture)}",
            cancellationToken: cancellationToken);
    }

    public Task<ToriiContractCodeView> GetExplorerInstructionContractViewAsync(
        string transactionHash,
        ulong index,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiContractCodeView>(
            $"/v1/explorer/instructions/{EncodePathSegment(transactionHash)}/{index.ToString(CultureInfo.InvariantCulture)}/contract-view",
            cancellationToken: cancellationToken);
    }

    public Task<ToriiExplorerHealthSnapshot> GetExplorerHealthAsync(CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerHealthSnapshot>("/v1/explorer/health", cancellationToken: cancellationToken);
    }

    public Task<ToriiExplorerMetricsSnapshot> GetExplorerMetricsAsync(CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiExplorerMetricsSnapshot>("/v1/explorer/metrics", cancellationToken: cancellationToken);
    }

    public Task<ToriiAssetBalancesPage> GetAccountAssetsAsync(
        string accountId,
        int? limit = null,
        long offset = 0,
        string? asset = null,
        string? scope = null,
        CancellationToken cancellationToken = default)
    {
        var encodedAccountId = EncodePathSegment(accountId);
        return GetAsync<ToriiAssetBalancesPage>(
            $"/v1/accounts/{encodedAccountId}/assets",
            BuildPaginationQuery(
                limit,
                offset,
                new KeyValuePair<string, string?>("asset", NormalizeOptionalValue(asset)),
                new KeyValuePair<string, string?>("scope", NormalizeOptionalValue(scope))),
            cancellationToken);
    }

    public Task<ToriiTransactionsPage> GetAccountTransactionsAsync(
        string accountId,
        int? limit = null,
        long offset = 0,
        string? assetId = null,
        CancellationToken cancellationToken = default)
    {
        var encodedAccountId = EncodePathSegment(accountId);
        return GetAsync<ToriiTransactionsPage>(
            $"/v1/accounts/{encodedAccountId}/transactions",
            BuildPaginationQuery(
                limit,
                offset,
                new KeyValuePair<string, string?>("asset_id", NormalizeOptionalValue(assetId))),
            cancellationToken);
    }

    public Task<ToriiAccountPermissionsPage> GetAccountPermissionsAsync(
        string accountId,
        int? limit = null,
        long offset = 0,
        CancellationToken cancellationToken = default)
    {
        var encodedAccountId = EncodePathSegment(accountId);
        return GetAsync<ToriiAccountPermissionsPage>(
            $"/v1/accounts/{encodedAccountId}/permissions",
            BuildPaginationQuery(limit, offset),
            cancellationToken);
    }

    public Task<ToriiAccountAliasLookupResponse?> LookupAliasesByAccountAsync(
        string accountId,
        string? dataspace = null,
        string? domain = null,
        CancellationToken cancellationToken = default)
    {
        return PostOptionalAsync<ToriiAccountAliasLookupRequest, ToriiAccountAliasLookupResponse>(
            "/v1/aliases/by_account",
            new ToriiAccountAliasLookupRequest
            {
                AccountId = NormalizeRequiredValue(accountId, nameof(accountId)),
                Dataspace = NormalizeOptionalValue(dataspace),
                Domain = NormalizeOptionalValue(domain),
            },
            cancellationToken);
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

    public Task<ToriiAccountAliasIndexResolution?> ResolveAccountAliasIndexAsync(
        ulong index,
        CancellationToken cancellationToken = default)
    {
        return PostOptionalAsync<ToriiAliasResolveIndexRequest, ToriiAccountAliasIndexResolution>(
            "/v1/aliases/resolve_index",
            new ToriiAliasResolveIndexRequest
            {
                Index = index,
            },
            cancellationToken);
    }

    public Task<ToriiUaidPortfolioResponse> GetUaidPortfolioAsync(
        string uaid,
        ToriiUaidPortfolioQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        var normalizedUaid = NormalizeUaidLiteral(uaid);
        return GetAsync<ToriiUaidPortfolioResponse>(
            $"/v1/accounts/{EncodePathSegment(normalizedUaid)}/portfolio",
            BuildQueryString([
                new KeyValuePair<string, string?>("asset_id", NormalizeOptionalValue(query?.AssetId)),
            ]),
            cancellationToken);
    }

    public Task<ToriiUaidBindingsResponse> GetUaidBindingsAsync(
        string uaid,
        CancellationToken cancellationToken = default)
    {
        var normalizedUaid = NormalizeUaidLiteral(uaid);
        return GetAsync<ToriiUaidBindingsResponse>(
            $"/v1/space-directory/uaids/{EncodePathSegment(normalizedUaid)}",
            cancellationToken: cancellationToken);
    }

    public Task<ToriiUaidManifestsResponse> GetUaidManifestsAsync(
        string uaid,
        ToriiUaidManifestQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        var normalizedUaid = NormalizeUaidLiteral(uaid);
        return GetAsync<ToriiUaidManifestsResponse>(
            $"/v1/space-directory/uaids/{EncodePathSegment(normalizedUaid)}/manifests",
            BuildUaidManifestQuery(query),
            cancellationToken);
    }

    public Task<ToriiAccountOnboardingResponse> RegisterAccountAsync(
        ToriiAccountOnboardingRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);

        return PostAsync<ToriiAccountOnboardingRequest, ToriiAccountOnboardingResponse>(
            "/v1/accounts/onboard",
            request,
            cancellationToken: cancellationToken);
    }

    public Task<ToriiAccountFaucetPuzzle> GetAccountFaucetPuzzleAsync(CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiAccountFaucetPuzzle>("/v1/accounts/faucet/puzzle", cancellationToken: cancellationToken);
    }

    public async Task<ToriiAccountFaucetSolution> SolveAccountFaucetAsync(
        string accountId,
        ToriiAccountFaucetSolveOptions? solveOptions = null,
        CancellationToken cancellationToken = default)
    {
        var puzzle = await GetAccountFaucetPuzzleAsync(cancellationToken);
        return ToriiAccountFaucetPow.Solve(accountId, puzzle, solveOptions, cancellationToken);
    }

    public Task<ToriiAccountFaucetResponse> ClaimAccountFaucetAsync(
        ToriiAccountFaucetRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);

        return PostAsync<ToriiAccountFaucetRequest, ToriiAccountFaucetResponse>(
            "/v1/accounts/faucet",
            request,
            cancellationToken: cancellationToken);
    }

    public async Task<ToriiAccountFaucetResponse> ClaimAccountFaucetAsync(
        string accountId,
        ToriiAccountFaucetSolveOptions? solveOptions = null,
        CancellationToken cancellationToken = default)
    {
        var prepared = await SolveAccountFaucetAsync(accountId, solveOptions, cancellationToken);
        return await ClaimAccountFaucetAsync(prepared.ToRequest(), cancellationToken);
    }

    public Task<ToriiVpnProfile> GetVpnProfileAsync(CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiVpnProfile>("/v1/vpn/profile", cancellationToken: cancellationToken);
    }

    public Task<ToriiVpnQuote> CreateVpnQuoteAsync(
        ToriiVpnQuoteCreateRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);

        return PostAsync<ToriiVpnQuoteCreateRequest, ToriiVpnQuote>(
            "/v1/vpn/quotes",
            request,
            cancellationToken: cancellationToken);
    }

    public Task<ToriiVpnSession> CreateVpnSessionAsync(
        ToriiVpnSessionCreateRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);

        return PostAsync<ToriiVpnSessionCreateRequest, ToriiVpnSession>(
            "/v1/vpn/sessions",
            request,
            cancellationToken: cancellationToken);
    }

    public async Task<ToriiVpnSession?> GetVpnSessionAsync(
        string sessionId,
        CancellationToken cancellationToken = default)
    {
        var normalizedSessionId = NormalizeRequiredValue(sessionId, nameof(sessionId));
        using var response = await SendAllowingStatusAsync(
            HttpMethod.Get,
            $"/v1/vpn/sessions/{EncodePathSegment(normalizedSessionId)}",
            query: null,
            content: null,
            HttpStatusCode.NotFound,
            cancellationToken);

        if (response.StatusCode == HttpStatusCode.NotFound)
        {
            return null;
        }

        return await DeserializeAsync<ToriiVpnSession>(response, cancellationToken);
    }

    public async Task<ToriiVpnReceipt?> DeleteVpnSessionAsync(
        string sessionId,
        CancellationToken cancellationToken = default)
    {
        var normalizedSessionId = NormalizeRequiredValue(sessionId, nameof(sessionId));
        using var response = await SendAllowingStatusAsync(
            HttpMethod.Delete,
            $"/v1/vpn/sessions/{EncodePathSegment(normalizedSessionId)}",
            query: null,
            content: null,
            HttpStatusCode.NotFound,
            cancellationToken);

        if (response.StatusCode == HttpStatusCode.NotFound)
        {
            return null;
        }

        return await DeserializeAsync<ToriiVpnReceipt>(response, cancellationToken);
    }

    public Task<ToriiVpnReceipt> SubmitVpnReceiptAsync(
        ToriiVpnReceiptSubmitRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);

        return PostAsync<ToriiVpnReceiptSubmitRequest, ToriiVpnReceipt>(
            "/v1/vpn/receipts",
            request,
            cancellationToken: cancellationToken);
    }

    public Task<ToriiVpnReceiptListResponse> ListVpnReceiptsAsync(CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiVpnReceiptListResponse>("/v1/vpn/receipts", cancellationToken: cancellationToken);
    }

    public Task<ToriiMultisigAccountOnboardingResponse> RegisterMultisigAccountAsync(
        ToriiMultisigAccountOnboardingRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);

        return PostAsync<ToriiMultisigAccountOnboardingRequest, ToriiMultisigAccountOnboardingResponse>(
            "/v1/accounts/onboard/multisig",
            request,
            cancellationToken: cancellationToken);
    }

    public Task<ToriiAssetAliasResolution?> ResolveAssetAliasAsync(string alias, CancellationToken cancellationToken = default)
    {
        return PostOptionalAsync<ToriiAliasResolutionRequest, ToriiAssetAliasResolution>(
            "/v1/assets/aliases/resolve",
            new ToriiAliasResolutionRequest
            {
                Alias = NormalizeRequiredValue(alias, nameof(alias)),
            },
            cancellationToken);
    }

    public Task<ToriiAccountAliasResolution?> ResolveAccountAliasAsync(string alias, CancellationToken cancellationToken = default)
    {
        return PostOptionalAsync<ToriiAliasResolutionRequest, ToriiAccountAliasResolution>(
            "/v1/aliases/resolve",
            new ToriiAliasResolutionRequest
            {
                Alias = NormalizeRequiredValue(alias, nameof(alias)),
            },
            cancellationToken);
    }

    public Task<ToriiContractAliasResolution?> ResolveContractAliasAsync(
        string contractAlias,
        CancellationToken cancellationToken = default)
    {
        return PostOptionalAsync<ToriiContractAliasResolutionRequest, ToriiContractAliasResolution>(
            "/v1/contracts/aliases/resolve",
            new ToriiContractAliasResolutionRequest
            {
                ContractAlias = NormalizeRequiredValue(contractAlias, nameof(contractAlias)),
            },
            cancellationToken);
    }

    public Task<ToriiDeployContractResponse> DeployContractAsync(
        ToriiDeployContractRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);

        return PostAsync<ToriiDeployContractRequest, ToriiDeployContractResponse>(
            "/v1/contracts/deploy",
            request,
            cancellationToken: cancellationToken);
    }

    public Task<ToriiContractCodeRecord> GetContractCodeAsync(
        string codeHash,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiContractCodeRecord>(
            $"/v1/contracts/code/{EncodePathSegment(codeHash)}",
            cancellationToken: cancellationToken);
    }

    public Task<ToriiContractCodeBytesResponse> GetContractCodeBytesResponseAsync(
        string codeHash,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiContractCodeBytesResponse>(
            $"/v1/contracts/code-bytes/{EncodePathSegment(codeHash)}",
            cancellationToken: cancellationToken);
    }

    public async Task<byte[]> GetContractCodeBytesAsync(
        string codeHash,
        CancellationToken cancellationToken = default)
    {
        var response = await GetContractCodeBytesResponseAsync(codeHash, cancellationToken);
        return response.DecodeBytes();
    }

    public Task<ToriiContractInstancesResponse> GetContractInstancesAsync(
        string namespaceId,
        ToriiContractInstancesQuery? query = null,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiContractInstancesResponse>(
            $"/v1/contracts/instances/{EncodePathSegment(namespaceId)}",
            BuildContractInstancesQuery(query),
            cancellationToken);
    }

    public Task<ToriiDeployAndActivateContractInstanceResponse> DeployAndActivateContractInstanceAsync(
        ToriiDeployAndActivateContractInstanceRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);

        return PostAsync<ToriiDeployAndActivateContractInstanceRequest, ToriiDeployAndActivateContractInstanceResponse>(
            "/v1/contracts/instance",
            request,
            cancellationToken: cancellationToken);
    }

    public Task<ToriiActivateContractInstanceResponse> ActivateContractInstanceAsync(
        ToriiActivateContractInstanceRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);

        return PostAsync<ToriiActivateContractInstanceRequest, ToriiActivateContractInstanceResponse>(
            "/v1/contracts/instance/activate",
            request,
            cancellationToken: cancellationToken);
    }

    public Task<ToriiContractStateResponse> GetContractStateAsync(
        ToriiContractStateQuery query,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(query);

        return GetAsync<ToriiContractStateResponse>(
            "/v1/contracts/state",
            BuildContractStateQuery(query),
            cancellationToken);
    }

    public Task<ToriiContractCallResponse> CallContractAsync(
        ToriiContractCallRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);

        return PostAsync<ToriiContractCallRequest, ToriiContractCallResponse>(
            "/v1/contracts/call",
            request,
            cancellationToken: cancellationToken);
    }

    public Task<ToriiContractCodeView> GetContractCodeViewAsync(
        string codeHash,
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiContractCodeView>(
            $"/v1/contracts/code/{EncodePathSegment(codeHash)}/contract-view",
            cancellationToken: cancellationToken);
    }

    public async Task<ToriiContractViewExecutionResult> ExecuteContractViewAsync(
        ToriiContractViewRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);

        using var content = CreateJsonContent(request);
        using var response = await SendAllowingStatusAsync(
            HttpMethod.Post,
            "/v1/contracts/view",
            query: null,
            content,
            HttpStatusCode.UnprocessableEntity,
            cancellationToken);

        if (response.StatusCode == HttpStatusCode.UnprocessableEntity)
        {
            return new ToriiContractViewExecutionResult
            {
                Error = await DeserializeAsync<ToriiContractViewErrorResponse>(response, cancellationToken),
            };
        }

        return new ToriiContractViewExecutionResult
        {
            Success = await DeserializeAsync<ToriiContractViewResponse>(response, cancellationToken),
        };
    }

    public Task<ToriiMultisigContractCallResponse> ProposeMultisigContractCallAsync(
        ToriiMultisigContractCallProposeRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);

        return PostAsync<ToriiMultisigContractCallProposeRequest, ToriiMultisigContractCallResponse>(
            "/v1/contracts/call/multisig/propose",
            request,
            cancellationToken: cancellationToken);
    }

    public async Task<ToriiMultisigResponse> ProposeMultisigAsync(
        ToriiMultisigProposeRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);

        var response = await PostAsync<ToriiMultisigProposeRequest, ToriiMultisigResponse>(
            "/v1/multisig/propose",
            request,
            cancellationToken: cancellationToken);
        ValidateMultisigResponse(response, "multisig response");
        return response;
    }

    public Task<ToriiMultisigContractCallResponse> ApproveMultisigContractCallAsync(
        ToriiMultisigContractCallApproveRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);

        return PostAsync<ToriiMultisigContractCallApproveRequest, ToriiMultisigContractCallResponse>(
            "/v1/contracts/call/multisig/approve",
            request,
            cancellationToken: cancellationToken);
    }

    public Task<ToriiContractVerifiedSourceJob> SubmitContractVerifiedSourceJobAsync(
        string codeHash,
        ToriiContractVerifiedSourceSubmission request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);

        return PostAsync<ToriiContractVerifiedSourceSubmission, ToriiContractVerifiedSourceJob>(
            $"/v1/contracts/code/{EncodePathSegment(codeHash)}/verified-source/jobs",
            request,
            cancellationToken: cancellationToken);
    }

    public async Task<ToriiContractVerifiedSourceJob?> GetContractVerifiedSourceJobAsync(
        string codeHash,
        string jobId,
        CancellationToken cancellationToken = default)
    {
        using var response = await SendAllowingStatusAsync(
            HttpMethod.Get,
            $"/v1/contracts/code/{EncodePathSegment(codeHash)}/verified-source-jobs/{EncodePathSegment(jobId)}",
            query: null,
            content: null,
            HttpStatusCode.NotFound,
            cancellationToken);

        if (response.StatusCode == HttpStatusCode.NotFound)
        {
            return null;
        }

        return await DeserializeAsync<ToriiContractVerifiedSourceJob>(response, cancellationToken);
    }

    public async Task<string> GetMetricsAsync(CancellationToken cancellationToken = default)
    {
        using var response = await SendAsync(HttpMethod.Get, "/v1/metrics", cancellationToken: cancellationToken);
        return await response.Content.ReadAsStringAsync(cancellationToken);
    }

    public Task<ToriiNodeCapabilities> GetNodeCapabilitiesAsync(CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiNodeCapabilities>("/v1/node/capabilities", cancellationToken: cancellationToken);
    }

    public Task<ToriiRuntimeAbiActive> GetRuntimeAbiActiveAsync(CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiRuntimeAbiActive>("/v1/runtime/abi/active", cancellationToken: cancellationToken);
    }

    public Task<ToriiRuntimeAbiHash> GetRuntimeAbiHashAsync(CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiRuntimeAbiHash>("/v1/runtime/abi/hash", cancellationToken: cancellationToken);
    }

    public Task<ToriiRuntimeMetrics> GetRuntimeMetricsAsync(CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiRuntimeMetrics>("/v1/runtime/metrics", cancellationToken: cancellationToken);
    }

    public Task<JsonDocument> GetVerifyingKeyAsync(
        string backend,
        string name,
        CancellationToken cancellationToken = default)
    {
        var normalizedBackend = VerifyingKeyBackendTags.RequireProductionVerifyBackendLabel(
            backend,
            nameof(backend));
        var normalizedName = NormalizeVerifyingKeyName(name, nameof(name));
        return GetJsonDocumentAsync(
            $"/v1/zk/vk/{EncodePathSegment(normalizedBackend)}/{EncodePathSegment(normalizedName)}",
            cancellationToken: cancellationToken);
    }

    public Task<JsonDocument> RegisterVerifyingKeyAsync(
        ToriiVerifyingKeyRegisterRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);

        var normalizedRequest = NormalizeVerifyingKeyRegisterRequest(request);
        return PostJsonDocumentAsync(
            "/v1/zk/vk/register",
            normalizedRequest,
            cancellationToken: cancellationToken);
    }

    public Task<JsonDocument> UpdateVerifyingKeyAsync(
        ToriiVerifyingKeyUpdateRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);

        var normalizedRequest = NormalizeVerifyingKeyUpdateRequest(request);
        return PostJsonDocumentAsync(
            "/v1/zk/vk/update",
            normalizedRequest,
            cancellationToken: cancellationToken);
    }

    public Task<ToriiSoraFsCidLookupResponse> GetSoraFsCidLookupAsync(
        string cid,
        CancellationToken cancellationToken = default)
    {
        var normalizedCid = NormalizeRequiredValue(cid, nameof(cid));
        return GetAsync<ToriiSoraFsCidLookupResponse>(
            $"/v1/sorafs/cid/{EncodePathSegment(normalizedCid)}",
            cancellationToken: cancellationToken);
    }

    public async Task<ToriiSoraFsPinRegisterResponse> RegisterSoraFsPinManifestAsync(
        ToriiSoraFsPinRegisterRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);

        var normalizedRequest = NormalizeSoraFsPinRegisterRequest(request);
        var response = await PostAsync<ToriiSoraFsPinRegisterWireRequest, ToriiSoraFsPinRegisterResponse>(
            "/v1/sorafs/pin/register",
            normalizedRequest,
            cancellationToken: cancellationToken);
        return NormalizeSoraFsPinRegisterResponse(response);
    }

    public Task<ToriiSoraFsDenylistCatalogResponse> GetSoraFsDenylistCatalogAsync(
        CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiSoraFsDenylistCatalogResponse>(
            "/v1/sorafs/denylist/catalog",
            cancellationToken: cancellationToken);
    }

    public Task<ToriiSoraFsDenylistPackResponse> GetSoraFsDenylistPackAsync(
        string packId,
        CancellationToken cancellationToken = default)
    {
        var normalizedPackId = NormalizeRequiredValue(packId, nameof(packId));
        return GetAsync<ToriiSoraFsDenylistPackResponse>(
            $"/v1/sorafs/denylist/packs/{EncodePathSegment(normalizedPackId)}",
            cancellationToken: cancellationToken);
    }

    public Task<HttpResponseMessage> OpenSoraFsCidContentAsync(
        string cid,
        string? relativePath = null,
        CancellationToken cancellationToken = default)
    {
        var normalizedCid = NormalizeRequiredValue(cid, nameof(cid));
        var gatewayPath = BuildSoraFsCidGatewayPath(normalizedCid, relativePath);
        return SendAsync(HttpMethod.Get, gatewayPath, cancellationToken: cancellationToken);
    }

    public async Task<ToriiSoraFsContentResponse> GetSoraFsCidContentAsync(
        string cid,
        string? relativePath = null,
        CancellationToken cancellationToken = default)
    {
        using var response = await OpenSoraFsCidContentAsync(cid, relativePath, cancellationToken);
        var bytes = await response.Content.ReadAsByteArrayAsync(cancellationToken);
        return new ToriiSoraFsContentResponse
        {
            Bytes = bytes,
            ContentType = response.Content.Headers.ContentType?.ToString(),
            ContentLength = response.Content.Headers.ContentLength,
            ContentCid = response.Headers.TryGetValues("sora-content-cid", out var values)
                ? values.FirstOrDefault()
                : null,
        };
    }

    public async Task<JsonDocument> SubmitSignedQueryAsync(
        ReadOnlyMemory<byte> noritoVersionedBytes,
        string? query = null,
        CancellationToken cancellationToken = default)
    {
        using var content = CreateBinaryContent(noritoVersionedBytes, "application/x-norito");
        using var response = await SendAsync(HttpMethod.Post, "/query", query, content, cancellationToken: cancellationToken);
        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken);
        return await JsonDocument.ParseAsync(stream, cancellationToken: cancellationToken);
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
        string? lastEventId = null,
        CancellationToken cancellationToken = default)
    {
        return OpenSseAsync(
            "/v1/events/sse",
            NormalizeEventSseQuery(query),
            lastEventId,
            cancellationToken);
    }

    public async IAsyncEnumerable<ToriiServerSentEvent> StreamEventsAsync(
        string? query = null,
        string? lastEventId = null,
        [EnumeratorCancellation] CancellationToken cancellationToken = default)
    {
        using var response = await OpenEventSseAsync(query, lastEventId, cancellationToken);
        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken);

        await foreach (var sseEvent in ReadServerSentEventsAsync(stream, cancellationToken))
        {
            yield return sseEvent;
        }
    }

    public async IAsyncEnumerable<ToriiPipelineEvent> StreamPipelineEventsAsync(
        string? query = null,
        string? lastEventId = null,
        [EnumeratorCancellation] CancellationToken cancellationToken = default)
    {
        await foreach (var sseEvent in StreamEventsAsync(query, lastEventId, cancellationToken))
        {
            if (sseEvent.IsComment || sseEvent.JsonData is null)
            {
                continue;
            }

            var pipelineEvent = JsonSerializer.Deserialize(
                sseEvent.JsonData,
                ToriiJsonSerializerContext.Default.ToriiPipelineEvent);
            if (pipelineEvent is null || !string.Equals(pipelineEvent.Category, "Pipeline", StringComparison.Ordinal))
            {
                continue;
            }

            pipelineEvent.LastEventId = sseEvent.Id;
            pipelineEvent.SseEventName = sseEvent.Event;
            pipelineEvent.RetryMilliseconds = sseEvent.RetryMilliseconds;
            yield return pipelineEvent;
        }
    }

    public async IAsyncEnumerable<ToriiProofEvent> StreamProofEventsAsync(
        string? query = null,
        string? lastEventId = null,
        [EnumeratorCancellation] CancellationToken cancellationToken = default)
    {
        await foreach (var sseEvent in StreamEventsAsync(query, lastEventId, cancellationToken))
        {
            if (sseEvent.IsComment || sseEvent.JsonData is null)
            {
                continue;
            }

            var proofEvent = JsonSerializer.Deserialize(
                sseEvent.JsonData,
                ToriiJsonSerializerContext.Default.ToriiProofEvent);
            if (proofEvent is null
                || !string.Equals(proofEvent.Category, "Data", StringComparison.Ordinal)
                || !proofEvent.Event.StartsWith("Proof", StringComparison.Ordinal))
            {
                continue;
            }

            proofEvent.LastEventId = sseEvent.Id;
            proofEvent.SseEventName = sseEvent.Event;
            proofEvent.RetryMilliseconds = sseEvent.RetryMilliseconds;
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
        using var content = CreateBinaryContent(noritoBytes, "application/x-norito");
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
        using var document = await JsonDocument.ParseAsync(stream, cancellationToken: cancellationToken);
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
        return await JsonDocument.ParseAsync(stream, cancellationToken: cancellationToken);
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
        ArgumentException.ThrowIfNullOrWhiteSpace(path);

        var requestUri = BuildRequestUri(path, query);
        var request = new HttpRequestMessage(method, requestUri)
        {
            Content = content,
        };

        if (!string.IsNullOrWhiteSpace(Options.BearerToken))
        {
            request.Headers.Authorization = new AuthenticationHeaderValue("Bearer", Options.BearerToken);
        }

        if (!string.IsNullOrWhiteSpace(accept))
        {
            request.Headers.Accept.Add(new MediaTypeWithQualityHeaderValue(accept));
        }

        if (Options.CanonicalRequestCredentials is not null)
        {
            var bodyBytes = content is null ? Array.Empty<byte>() : await content.ReadAsByteArrayAsync(cancellationToken);
            var headers = CanonicalRequest.BuildHeaders(
                Options.CanonicalRequestCredentials.AccountId,
                Options.CanonicalRequestCredentials.PrivateKeySeed,
                method.Method,
                requestUri.AbsolutePath,
                requestUri.Query,
                bodyBytes);

            foreach (var header in headers.ToDictionary())
            {
                request.Headers.TryAddWithoutValidation(header.Key, header.Value);
            }
        }

        configureRequest?.Invoke(request);
        return request;
    }

    private static async IAsyncEnumerable<ToriiServerSentEvent> ReadServerSentEventsAsync(
        Stream stream,
        [EnumeratorCancellation] CancellationToken cancellationToken)
    {
        using var reader = new StreamReader(stream, Encoding.UTF8, detectEncodingFromByteOrderMarks: true, leaveOpen: false);
        var dataBuilder = new StringBuilder();
        var commentBuilder = new StringBuilder();
        var hasData = false;
        var hasComment = false;
        string? eventName = null;
        string? eventId = null;
        int? retryMilliseconds = null;

        while (true)
        {
            var line = await reader.ReadLineAsync(cancellationToken);
            if (line is null)
            {
                break;
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
                case "retry" when int.TryParse(value, NumberStyles.None, CultureInfo.InvariantCulture, out var parsedRetry) && parsedRetry >= 0:
                    retryMilliseconds = parsedRetry;
                    break;
            }
        }

        var finalEvent = BuildServerSentEvent(eventName, eventId, retryMilliseconds, hasData, dataBuilder, hasComment, commentBuilder);
        if (finalEvent is not null)
        {
            yield return finalEvent;
        }
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
            try
            {
                jsonData = JsonNode.Parse(rawData);
            }
            catch (JsonException)
            {
                jsonData = null;
            }
        }

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

    private async Task<TResponse> DeserializeAsync<TResponse>(
        HttpResponseMessage response,
        CancellationToken cancellationToken)
    {
        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken);
        var value = await JsonSerializer.DeserializeAsync<TResponse>(stream, serializerOptions, cancellationToken);
        return value ?? throw new JsonException($"Torii response for `{response.RequestMessage?.RequestUri}` deserialized to null.");
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
        if (!response.Ok)
        {
            throw new JsonException($"{context}.ok must be true.");
        }

        ValidateOptionalBase64(response.SigningMessageBase64, $"{context}.signing_message_b64");
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

        if (IsNestedIdentifierResolveResponse(response))
        {
            if (!string.IsNullOrEmpty(response.Signature))
            {
                ValidateExactHex(response.Signature, "identifier resolve response.attestation.signature");
            }

            ValidateIdentifierReceiptSignaturePayload(
                response.SignaturePayload,
                "identifier resolve response");
            return;
        }

        ValidateExactHex(response.Signature, "identifier resolve response.signature");
        ValidateExactHex(response.SignaturePayloadHex, "identifier resolve response.signature_payload_hex");
        ValidateIdentifierReceiptSignaturePayload(
            response.SignaturePayload,
            "identifier resolve response.signature_payload");
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
            ValidateOptionalUnsignedInteger(execution, "executed_at_ms", $"{context}.execution.executed_at_ms");
            ValidateOptionalUnsignedInteger(execution, "expires_at_ms", $"{context}.execution.expires_at_ms");
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
            ValidateOptionalUnsignedInteger(
                openingPayload,
                "opened_at_ms",
                $"{context}.opening.payload.opened_at_ms");
            ValidateOptionalUnsignedInteger(
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
            if (text.Length == 0
                || !ulong.TryParse(text, NumberStyles.None, CultureInfo.InvariantCulture, out _))
            {
                throw new JsonException($"{context} must be an unsigned integer.");
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

        try
        {
            if (Convert.FromBase64String(value).Length == 0)
            {
                throw new JsonException($"{field} must not decode to empty bytes.");
            }
        }
        catch (FormatException ex)
        {
            throw new JsonException($"{field} must be valid base64.", ex);
        }
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

        var body = value.StartsWith("0x", StringComparison.OrdinalIgnoreCase)
            ? value[2..]
            : value;
        if (body.Length == 0 || body.Length % 2 != 0 || !IsHex(body))
        {
            throw new JsonException($"{field} must be an exact hex string.");
        }
    }

    private static void ValidateOptionalBase64(string? value, string field)
    {
        if (value is null)
        {
            return;
        }

        var trimmed = value.Trim();
        if (trimmed.Length == 0)
        {
            throw new JsonException($"{field} must be a non-empty base64 string.");
        }

        try
        {
            if (Convert.FromBase64String(trimmed).Length == 0)
            {
                throw new JsonException($"{field} must not decode to empty bytes.");
            }
        }
        catch (FormatException ex)
        {
            throw new JsonException($"{field} must be valid base64.", ex);
        }
    }

    private async Task<ToriiApiException> CreateApiExceptionAsync(
        HttpResponseMessage response,
        CancellationToken cancellationToken)
    {
        var responseBody = response.Content is null
            ? null
            : await response.Content.ReadAsStringAsync(cancellationToken);

        return new ToriiApiException(
            response.StatusCode,
            response.RequestMessage?.RequestUri,
            responseBody,
            response.ReasonPhrase);
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
                if (!string.IsNullOrWhiteSpace(lastEventId))
                {
                    request.Headers.TryAddWithoutValidation("Last-Event-ID", lastEventId.Trim());
                }
            },
            cancellationToken: cancellationToken);
    }

    private async IAsyncEnumerable<TResponse> StreamSsePayloadsAsync<TResponse>(
        string path,
        JsonTypeInfo<TResponse> jsonTypeInfo,
        string? query = null,
        string? lastEventId = null,
        [EnumeratorCancellation] CancellationToken cancellationToken = default)
    {
        using var response = await OpenSseAsync(path, query, lastEventId, cancellationToken);
        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken);

        await foreach (var sseEvent in ReadServerSentEventsAsync(stream, cancellationToken))
        {
            if (sseEvent.IsComment || sseEvent.JsonData is null)
            {
                continue;
            }

            var payload = JsonSerializer.Deserialize(sseEvent.JsonData, jsonTypeInfo);
            if (payload is not null)
            {
                yield return payload;
            }
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
        return Uri.UnescapeDataString(value.Replace("+", " ", StringComparison.Ordinal));
    }

    private static string NormalizeEventFilterPayload(string filter, string context)
    {
        var trimmed = filter.Trim();
        if (trimmed.Length == 0 || (trimmed[0] != '{' && trimmed[0] != '['))
        {
            return filter;
        }

        JsonNode? node;
        try
        {
            node = JsonNode.Parse(trimmed);
        }
        catch (JsonException)
        {
            return filter;
        }

        if (node is not JsonObject obj)
        {
            return filter;
        }

        var changed = NormalizeProductionEventFilterObject(obj, context);
        return changed ? obj.ToJsonString() : filter;
    }

    private static bool NormalizeProductionEventFilterObject(JsonObject filter, string context)
    {
        var changed = false;
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
                matcher["backend"] = normalizedBackend;
                changed = true;
            }

            if (string.Equals(eventKind, "Proof", StringComparison.Ordinal))
            {
                changed |= NormalizeProofHashMatcher(
                    matcher,
                    "hash_hex",
                    $"{context}.{eventKind}.id_matcher.hash_hex");
                changed |= NormalizeProofHashMatcher(
                    matcher,
                    "proof_hash_hex",
                    $"{context}.{eventKind}.id_matcher.proof_hash_hex");
            }
            else
            {
                changed |= NormalizeVerifyingKeyNameMatcher(
                    matcher,
                    $"{context}.{eventKind}.id_matcher.name");
            }
        }

        return changed;
    }

    private static bool NormalizeVerifyingKeyNameMatcher(JsonObject matcher, string context)
    {
        if (!matcher.TryGetPropertyValue("name", out var node))
        {
            return false;
        }

        var raw = RequireJsonString(node, context);
        var normalized = raw.Trim();
        if (normalized.Length == 0)
        {
            throw new ArgumentException($"{context} must be a non-empty string.", context);
        }

        if (normalized.Contains(':', StringComparison.Ordinal))
        {
            throw new ArgumentException($"{context} must not contain ':' characters.", context);
        }

        if (string.Equals(raw, normalized, StringComparison.Ordinal))
        {
            return false;
        }

        matcher["name"] = normalized;
        return true;
    }

    private static bool NormalizeProofHashMatcher(JsonObject matcher, string propertyName, string context)
    {
        if (!matcher.TryGetPropertyValue(propertyName, out var node))
        {
            return false;
        }

        var raw = RequireJsonString(node, context);
        var normalized = NormalizeHex32String(raw, context);
        if (string.Equals(raw, normalized, StringComparison.Ordinal))
        {
            return false;
        }

        matcher[propertyName] = normalized;
        return true;
    }

    private static string RequireJsonString(JsonNode? node, string context)
    {
        if (node is JsonValue value && value.TryGetValue<string>(out var text))
        {
            return text;
        }

        throw new ArgumentException($"{context} must be a string.", context);
    }

    private static string NormalizeHex32String(string raw, string context)
    {
        var normalized = raw.Trim().ToLowerInvariant();
        if (normalized.StartsWith("0x", StringComparison.Ordinal))
        {
            normalized = normalized[2..];
        }

        if (normalized.Length != 64 || !IsLowerHex(normalized))
        {
            throw new ArgumentException($"{context} must be a 32-byte hex string.", context);
        }

        return normalized;
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
            new KeyValuePair<string, string?>("contains", NormalizeOptionalValue(query.Contains)),
            new KeyValuePair<string, string?>("hash_prefix", NormalizeOptionalValue(query.HashPrefix)),
            new KeyValuePair<string, string?>("offset", query.Offset?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>("limit", query.Limit?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>("order", NormalizeOptionalValue(query.Order)),
        ]);
    }

    private static string BuildContractStateQuery(ToriiContractStateQuery query)
    {
        ArgumentNullException.ThrowIfNull(query);

        var hasPath = !string.IsNullOrWhiteSpace(query.Path);
        var normalizedPaths = query.Paths?
            .Where(static value => !string.IsNullOrWhiteSpace(value))
            .Select(static value => value.Trim())
            .ToArray();
        var hasPaths = normalizedPaths is { Length: > 0 };
        var hasPrefix = !string.IsNullOrWhiteSpace(query.Prefix);
        var modeCount = (hasPath ? 1 : 0) + (hasPaths ? 1 : 0) + (hasPrefix ? 1 : 0);
        if (modeCount != 1)
        {
            throw new ArgumentException("Exactly one of Path, Paths, or Prefix must be provided.", nameof(query));
        }

        return BuildQueryString(
        [
            new KeyValuePair<string, string?>("path", hasPath ? NormalizeRequiredValue(query.Path, nameof(query.Path)) : null),
            new KeyValuePair<string, string?>("paths", hasPaths ? string.Join(',', normalizedPaths!) : null),
            new KeyValuePair<string, string?>("prefix", hasPrefix ? NormalizeRequiredValue(query.Prefix, nameof(query.Prefix)) : null),
            new KeyValuePair<string, string?>(
                "include_value",
                query.IncludeValue.HasValue ? query.IncludeValue.Value.ToString().ToLowerInvariant() : null),
            new KeyValuePair<string, string?>("offset", query.Offset?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>("limit", query.Limit?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>("decode", NormalizeOptionalValue(query.Decode)),
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

        ValidateExplorerPagination(query, nameof(query));

        return BuildQueryString(
        [
            new KeyValuePair<string, string?>("page", query.Page?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>("per_page", query.PerPage?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>("domain", NormalizeOptionalValue(query.Domain)),
            new KeyValuePair<string, string?>("with_asset", NormalizeOptionalValue(query.WithAsset)),
        ]);
    }

    private static string? BuildExplorerDomainsQuery(ToriiExplorerDomainsQuery? query)
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
            new KeyValuePair<string, string?>("owned_by", NormalizeOptionalValue(query.OwnedBy)),
        ]);
    }

    private static string? BuildExplorerAssetDefinitionsQuery(ToriiExplorerAssetDefinitionsQuery? query)
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
            new KeyValuePair<string, string?>("domain", NormalizeOptionalValue(query.Domain)),
            new KeyValuePair<string, string?>("owned_by", NormalizeOptionalValue(query.OwnedBy)),
        ]);
    }

    private static string? BuildExplorerAssetsQuery(ToriiExplorerAssetsQuery? query)
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
            new KeyValuePair<string, string?>("owned_by", NormalizeOptionalValue(query.OwnedBy)),
            new KeyValuePair<string, string?>("definition", NormalizeOptionalValue(query.Definition)),
            new KeyValuePair<string, string?>("asset_id", NormalizeOptionalValue(query.AssetId)),
        ]);
    }

    private static string? BuildExplorerNftsQuery(ToriiExplorerNftsQuery? query)
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
            new KeyValuePair<string, string?>("owned_by", NormalizeOptionalValue(query.OwnedBy)),
            new KeyValuePair<string, string?>("domain", NormalizeOptionalValue(query.Domain)),
        ]);
    }

    private static string? BuildExplorerRwasQuery(ToriiExplorerRwasQuery? query)
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
            new KeyValuePair<string, string?>("owned_by", NormalizeOptionalValue(query.OwnedBy)),
            new KeyValuePair<string, string?>("domain", NormalizeOptionalValue(query.Domain)),
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
            new KeyValuePair<string, string?>("authority", NormalizeOptionalValue(query.Authority)),
            new KeyValuePair<string, string?>("block", query.Block?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>(
                "status",
                query.Status is null ? null : FormatExplorerTransactionStatusFilter(query.Status.Value)),
            new KeyValuePair<string, string?>("asset_id", NormalizeOptionalValue(query.AssetId)),
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
            new KeyValuePair<string, string?>("authority", NormalizeOptionalValue(query.Authority)),
            new KeyValuePair<string, string?>("account", NormalizeOptionalValue(query.Account)),
            new KeyValuePair<string, string?>("transaction_hash", NormalizeOptionalValue(query.TransactionHash)),
            new KeyValuePair<string, string?>(
                "transaction_status",
                query.TransactionStatus is null ? null : FormatExplorerTransactionStatusFilter(query.TransactionStatus.Value)),
            new KeyValuePair<string, string?>("block", query.Block?.ToString(CultureInfo.InvariantCulture)),
            new KeyValuePair<string, string?>("kind", NormalizeOptionalValue(query.Kind)),
            new KeyValuePair<string, string?>("asset_id", NormalizeOptionalValue(query.AssetId)),
        ]);
    }

    private static JsonSerializerOptions CreateSerializerOptions(JsonSerializerOptions baseOptions)
    {
        ArgumentNullException.ThrowIfNull(baseOptions);

        var options = new JsonSerializerOptions(baseOptions);
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

    private static ByteArrayContent CreateBinaryContent(ReadOnlyMemory<byte> bytes, string mediaType)
    {
        var content = new ByteArrayContent(bytes.ToArray());
        content.Headers.ContentType = new MediaTypeHeaderValue(mediaType);
        return content;
    }

    private static string EncodePathSegment(string value)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(value);
        return Uri.EscapeDataString(value.Trim());
    }

    private static string BuildSoraFsCidGatewayPath(string cid, string? relativePath)
    {
        var builder = new StringBuilder("/sorafs/cid/");
        builder.Append(EncodePathSegment(cid));
        builder.Append('/');

        if (string.IsNullOrWhiteSpace(relativePath))
        {
            return builder.ToString();
        }

        var normalizedPath = relativePath.Trim().Trim('/');
        if (normalizedPath.Length == 0)
        {
            return builder.ToString();
        }

        var segments = normalizedPath
            .Split('/', StringSplitOptions.RemoveEmptyEntries)
            .Select(Uri.EscapeDataString);
        builder.Append(string.Join('/', segments));
        return builder.ToString();
    }

    private static ToriiVerifyingKeyRegisterRequest NormalizeVerifyingKeyRegisterRequest(
        ToriiVerifyingKeyRegisterRequest request)
    {
        var normalizedBackend = VerifyingKeyBackendTags.RequireProductionVerifyBackendLabel(
            request.Backend,
            nameof(request.Backend));
        var vkBytes = NormalizeOptionalVerifierBytes(
            request.VerifyingKeyBytes,
            request.VerifyingKeyLength,
            out var vkLength,
            nameof(request.VerifyingKeyBytes));
        var commitmentHex = NormalizeOptionalVerifyingKeyHex(
            request.CommitmentHex,
            nameof(request.CommitmentHex));
        ValidateVerifyingKeyMaterial(vkBytes, vkLength, commitmentHex);
        ValidateInlineVerifyingKeyCommitment(normalizedBackend, vkBytes, commitmentHex);
        ValidateVerifyingKeyHeightRange(
            request.ActivationHeight,
            request.WithdrawHeight,
            nameof(request.WithdrawHeight));

        return request with
        {
            Authority = NormalizeRequiredValue(request.Authority, nameof(request.Authority)),
            PrivateKey = NormalizeRequiredValue(request.PrivateKey, nameof(request.PrivateKey)),
            Backend = normalizedBackend,
            Name = NormalizeVerifyingKeyName(request.Name, nameof(request.Name)),
            Version = RequirePositiveUInt32(request.Version, nameof(request.Version)),
            CircuitId = NormalizeRequiredValue(request.CircuitId, nameof(request.CircuitId)),
            PublicInputsSchemaHashHex = NormalizeVerifyingKeyHex(
                request.PublicInputsSchemaHashHex,
                nameof(request.PublicInputsSchemaHashHex)),
            Curve = NormalizeOptionalValue(request.Curve),
            GasScheduleId = NormalizeRequiredValue(request.GasScheduleId, nameof(request.GasScheduleId)),
            VerifyingKeyLength = vkLength,
            MaxProofBytes = request.MaxProofBytes,
            MetadataUriCid = NormalizeOptionalValue(request.MetadataUriCid),
            VerifyingKeyBytesCid = NormalizeOptionalValue(request.VerifyingKeyBytesCid),
            ActivationHeight = request.ActivationHeight,
            WithdrawHeight = request.WithdrawHeight,
            CommitmentHex = commitmentHex,
            VerifyingKeyBytes = vkBytes,
            Status = NormalizeOptionalVerifyingKeyStatus(request.Status, nameof(request.Status)),
        };
    }

    private static ToriiVerifyingKeyUpdateRequest NormalizeVerifyingKeyUpdateRequest(
        ToriiVerifyingKeyUpdateRequest request)
    {
        var normalizedBackend = VerifyingKeyBackendTags.RequireProductionVerifyBackendLabel(
            request.Backend,
            nameof(request.Backend));
        var vkBytes = NormalizeOptionalVerifierBytes(
            request.VerifyingKeyBytes,
            request.VerifyingKeyLength,
            out var vkLength,
            nameof(request.VerifyingKeyBytes));
        var commitmentHex = NormalizeOptionalVerifyingKeyHex(
            request.CommitmentHex,
            nameof(request.CommitmentHex));
        ValidateVerifyingKeyMaterial(vkBytes, vkLength, commitmentHex);
        ValidateInlineVerifyingKeyCommitment(normalizedBackend, vkBytes, commitmentHex);
        ValidateVerifyingKeyHeightRange(
            request.ActivationHeight,
            request.WithdrawHeight,
            nameof(request.WithdrawHeight));

        return request with
        {
            Authority = NormalizeRequiredValue(request.Authority, nameof(request.Authority)),
            PrivateKey = NormalizeRequiredValue(request.PrivateKey, nameof(request.PrivateKey)),
            Backend = normalizedBackend,
            Name = NormalizeVerifyingKeyName(request.Name, nameof(request.Name)),
            Version = RequirePositiveUInt32(request.Version, nameof(request.Version)),
            CircuitId = NormalizeRequiredValue(request.CircuitId, nameof(request.CircuitId)),
            PublicInputsSchemaHashHex = NormalizeVerifyingKeyHex(
                request.PublicInputsSchemaHashHex,
                nameof(request.PublicInputsSchemaHashHex)),
            Curve = NormalizeOptionalValue(request.Curve),
            GasScheduleId = NormalizeOptionalValue(request.GasScheduleId),
            CommitmentHex = commitmentHex,
            VerifyingKeyLength = vkLength,
            MaxProofBytes = request.MaxProofBytes,
            MetadataUriCid = NormalizeOptionalValue(request.MetadataUriCid),
            VerifyingKeyBytesCid = NormalizeOptionalValue(request.VerifyingKeyBytesCid),
            ActivationHeight = request.ActivationHeight,
            WithdrawHeight = request.WithdrawHeight,
            VerifyingKeyBytes = vkBytes,
            Status = NormalizeOptionalVerifyingKeyStatus(request.Status, nameof(request.Status)),
        };
    }

    private static string NormalizeVerifyingKeyName(string? value, string paramName)
    {
        var normalized = NormalizeRequiredValue(value, paramName);
        if (normalized.Contains(':', StringComparison.Ordinal))
        {
            throw new ArgumentException("Value must not contain ':' characters.", paramName);
        }

        return normalized;
    }

    private static string NormalizeVerifyingKeyHex(string? value, string paramName)
    {
        var normalized = NormalizeRequiredValue(value, paramName);
        if (normalized.StartsWith("0x", StringComparison.OrdinalIgnoreCase))
        {
            normalized = normalized[2..].Trim();
        }

        if (normalized.Length != 64 || !IsHex(normalized))
        {
            throw new ArgumentException("Value must be a 32-byte hex string.", paramName);
        }

        return normalized.ToLowerInvariant();
    }

    private static string? NormalizeOptionalVerifyingKeyHex(string? value, string paramName)
    {
        return string.IsNullOrWhiteSpace(value) ? null : NormalizeVerifyingKeyHex(value, paramName);
    }

    private static byte[]? NormalizeOptionalVerifierBytes(
        byte[]? bytes,
        uint? explicitLength,
        out uint? normalizedLength,
        string paramName)
    {
        if (bytes is null)
        {
            normalizedLength = explicitLength;
            if (normalizedLength == 0)
            {
                throw new ArgumentOutOfRangeException(paramName, "vk_len must be positive when provided.");
            }

            return null;
        }

        if (bytes.Length == 0)
        {
            throw new ArgumentException("Value must be a non-empty verifying key byte payload.", paramName);
        }

        var actualLength = (uint)bytes.Length;
        if (explicitLength.HasValue && explicitLength.Value != actualLength)
        {
            throw new ArgumentException("vk_len must match vk_bytes length.", paramName);
        }

        normalizedLength = actualLength;
        return bytes.ToArray();
    }

    private static string? NormalizeOptionalVerifyingKeyStatus(string? value, string paramName)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            return null;
        }

        var normalized = value.Trim().ToLowerInvariant();
        return normalized switch
        {
            "proposed" => "Proposed",
            "active" => "Active",
            "withdrawn" => "Withdrawn",
            _ => throw new ArgumentException(
                "Value must be one of Proposed, Active, or Withdrawn.",
                paramName),
        };
    }

    private static void ValidateVerifyingKeyHeightRange(
        ulong? activationHeight,
        ulong? withdrawHeight,
        string paramName)
    {
        if (activationHeight.HasValue
            && withdrawHeight.HasValue
            && withdrawHeight.Value < activationHeight.Value)
        {
            throw new ArgumentOutOfRangeException(
                paramName,
                "withdraw_height must be greater than or equal to activation_height.");
        }
    }

    private static void ValidateVerifyingKeyMaterial(
        byte[]? bytes,
        uint? vkLength,
        string? commitmentHex)
    {
        if (bytes is not null)
        {
            return;
        }

        if (commitmentHex is null)
        {
            throw new ArgumentException(
                "commitment_hex is required when vk_bytes is omitted.",
                nameof(commitmentHex));
        }

        if (!vkLength.HasValue)
        {
            throw new ArgumentException(
                "vk_len is required when vk_bytes is omitted.",
                nameof(vkLength));
        }
    }

    private static uint RequirePositiveUInt32(uint? value, string paramName)
    {
        if (value is null)
        {
            throw new ArgumentException("Value must be provided.", paramName);
        }

        if (value.Value == 0)
        {
            throw new ArgumentOutOfRangeException(paramName, "Value must be positive.");
        }

        return value.Value;
    }

    private static void ValidateInlineVerifyingKeyCommitment(
        string backend,
        byte[]? bytes,
        string? commitmentHex)
    {
        if (bytes is null || commitmentHex is null)
        {
            return;
        }

        var expected = ComputeVerifyingKeyCommitmentHex(backend, bytes);
        if (!string.Equals(expected, commitmentHex, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "commitment_hex must match domain-separated SHA-256 of backend and vk_bytes.",
                nameof(commitmentHex));
        }
    }

    private static string ComputeVerifyingKeyCommitmentHex(string backend, byte[] bytes)
    {
        var domainBytes = Encoding.UTF8.GetBytes("iroha:zk:v1:vk");
        var backendBytes = Encoding.UTF8.GetBytes(backend);
        var preimage = new byte[domainBytes.Length + 8 + backendBytes.Length + 8 + bytes.Length];
        var offset = 0;
        Buffer.BlockCopy(domainBytes, 0, preimage, offset, domainBytes.Length);
        offset += domainBytes.Length;
        WriteUInt64BigEndian(preimage, offset, (ulong)backendBytes.Length);
        offset += 8;
        Buffer.BlockCopy(backendBytes, 0, preimage, offset, backendBytes.Length);
        offset += backendBytes.Length;
        WriteUInt64BigEndian(preimage, offset, (ulong)bytes.Length);
        offset += 8;
        Buffer.BlockCopy(bytes, 0, preimage, offset, bytes.Length);
        return Convert.ToHexString(SHA256.HashData(preimage)).ToLowerInvariant();
    }

    private static void WriteUInt64BigEndian(byte[] target, int offset, ulong value)
    {
        for (var index = 7; index >= 0; index--)
        {
            target[offset + index] = (byte)(value & 0xff);
            value >>= 8;
        }
    }

    private static ToriiSoraFsPinRegisterWireRequest NormalizeSoraFsPinRegisterRequest(
        ToriiSoraFsPinRegisterRequest request)
    {
        var chunker = request.Chunker ?? throw new ArgumentNullException(nameof(request.Chunker));
        var pinPolicy = request.PinPolicy ?? throw new ArgumentNullException(nameof(request.PinPolicy));
        var normalizedChunker = NormalizeSoraFsChunker(chunker);

        return new ToriiSoraFsPinRegisterWireRequest
        {
            Authority = NormalizeRequiredValue(request.Authority, nameof(request.Authority)),
            PrivateKey = NormalizeRequiredValue(request.PrivateKey, nameof(request.PrivateKey)),
            ChunkerProfileId = normalizedChunker.ProfileId,
            ChunkerNamespace = normalizedChunker.Namespace,
            ChunkerName = normalizedChunker.Name,
            ChunkerSemver = normalizedChunker.Semver,
            ChunkerMultihashCode = normalizedChunker.MultihashCode,
            PinPolicy = NormalizeSoraFsPinPolicy(pinPolicy),
            ManifestDigestHex = NormalizeSoraFsDigestHex(
                request.ManifestDigestHex,
                nameof(request.ManifestDigestHex)),
            ChunkDigestSha3_256Hex = NormalizeSoraFsDigestHex(
                request.ChunkDigestSha3_256Hex,
                nameof(request.ChunkDigestSha3_256Hex)),
            ContentLength = RequireSoraFsUnsigned(
                request.ContentLength,
                nameof(request.ContentLength),
                allowZero: true),
            SubmittedEpoch = RequireSoraFsUnsigned(
                request.SubmittedEpoch,
                nameof(request.SubmittedEpoch),
                allowZero: true),
            Alias = request.Alias is null
                ? null
                : NormalizeSoraFsPinAlias(request.Alias, nameof(request.Alias)),
            SuccessorOfHex = string.IsNullOrWhiteSpace(request.SuccessorOfHex)
                ? null
                : NormalizeSoraFsDigestHex(request.SuccessorOfHex, nameof(request.SuccessorOfHex)),
        };
    }

    private static ToriiSoraFsPinRegisterResponse NormalizeSoraFsPinRegisterResponse(
        ToriiSoraFsPinRegisterResponse response)
    {
        ArgumentNullException.ThrowIfNull(response);

        return new ToriiSoraFsPinRegisterResponse
        {
            ManifestDigestHex = NormalizeSoraFsDigestHex(
                response.ManifestDigestHex,
                nameof(response.ManifestDigestHex)),
            ChunkerHandle = NormalizeRequiredValue(
                response.ChunkerHandle,
                nameof(response.ChunkerHandle)),
            SubmittedEpoch = RequireSoraFsUnsigned(
                response.SubmittedEpoch,
                nameof(response.SubmittedEpoch),
                allowZero: true),
            ContentLength = RequireSoraFsUnsigned(
                response.ContentLength,
                nameof(response.ContentLength),
                allowZero: true),
            PinFeeNano = RequireSoraFsUnsigned(
                response.PinFeeNano,
                nameof(response.PinFeeNano),
                allowZero: true),
            PinFeeAssetId = NormalizeRequiredValue(
                response.PinFeeAssetId,
                nameof(response.PinFeeAssetId)),
            PinFeeTreasuryAccountId = NormalizeRequiredValue(
                response.PinFeeTreasuryAccountId,
                nameof(response.PinFeeTreasuryAccountId)),
            Alias = response.Alias is null
                ? null
                : NormalizeSoraFsPinAlias(response.Alias, nameof(response.Alias)),
            SuccessorOfHex = string.IsNullOrWhiteSpace(response.SuccessorOfHex)
                ? null
                : NormalizeSoraFsDigestHex(response.SuccessorOfHex, nameof(response.SuccessorOfHex)),
        };
    }

    private static ToriiSoraFsChunkerHandle NormalizeSoraFsChunker(ToriiSoraFsChunkerHandle chunker)
    {
        return new ToriiSoraFsChunkerHandle
        {
            ProfileId = RequireSoraFsUnsigned(
                chunker.ProfileId,
                nameof(chunker.ProfileId),
                allowZero: false),
            Namespace = NormalizeRequiredValue(chunker.Namespace, nameof(chunker.Namespace)),
            Name = NormalizeRequiredValue(chunker.Name, nameof(chunker.Name)),
            Semver = NormalizeRequiredValue(chunker.Semver, nameof(chunker.Semver)),
            MultihashCode = chunker.MultihashCode ?? 0,
        };
    }

    private static ToriiSoraFsPinPolicy NormalizeSoraFsPinPolicy(ToriiSoraFsPinPolicy pinPolicy)
    {
        return new ToriiSoraFsPinPolicy
        {
            MinReplicas = RequireSoraFsUnsigned(
                pinPolicy.MinReplicas,
                nameof(pinPolicy.MinReplicas),
                allowZero: false),
            StorageClass = new ToriiSoraFsStorageClass
            {
                Type = NormalizeSoraFsStorageClass(pinPolicy.StorageClass),
            },
            RetentionEpoch = pinPolicy.RetentionEpoch ?? 0,
        };
    }

    private static ToriiSoraFsPinAlias NormalizeSoraFsPinAlias(
        ToriiSoraFsPinAlias alias,
        string paramName)
    {
        return new ToriiSoraFsPinAlias
        {
            Namespace = NormalizeRequiredValue(alias.Namespace, $"{paramName}.{nameof(alias.Namespace)}"),
            Name = NormalizeRequiredValue(alias.Name, $"{paramName}.{nameof(alias.Name)}"),
            ProofBase64 = NormalizeRequiredBase64(
                alias.ProofBase64,
                $"{paramName}.{nameof(alias.ProofBase64)}"),
        };
    }

    private static string NormalizeSoraFsStorageClass(ToriiSoraFsStorageClass? storageClass)
    {
        if (storageClass is null)
        {
            throw new ArgumentNullException(nameof(storageClass));
        }

        var normalized = NormalizeRequiredValue(storageClass.Type, nameof(storageClass.Type)).ToLowerInvariant();
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
        var normalized = NormalizeRequiredValue(value, paramName);
        if (normalized.StartsWith("0x", StringComparison.OrdinalIgnoreCase))
        {
            normalized = normalized[2..].Trim();
        }

        if (normalized.Length != 64 || !IsHex(normalized))
        {
            throw new ArgumentException("Value must be a 32-byte hex string.", paramName);
        }

        return normalized.ToLowerInvariant();
    }

    private static string NormalizeRequiredBase64(string? value, string paramName)
    {
        var normalized = NormalizeRequiredValue(value, paramName);
        if (normalized.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Value must be base64 encoded without whitespace.", paramName);
        }

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

        return Convert.ToBase64String(bytes);
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

    private static string NormalizeRequiredValue(string? value, string paramName)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException("Value cannot be null or whitespace.", paramName);
        }

        return value.Trim();
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

        return value;
    }

    private static string? NormalizeOptionalValue(string? value)
    {
        return string.IsNullOrWhiteSpace(value) ? null : value.Trim();
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
        var content = root.TryGetProperty("content", out var contentElement)
            ? contentElement
            : root;

        var statusElement = content.TryGetProperty("status", out var explicitStatus)
            ? explicitStatus
            : root.GetProperty("status");

        var rawKind = statusElement.ValueKind switch
        {
            JsonValueKind.String => statusElement.GetString() ?? string.Empty,
            JsonValueKind.Object when statusElement.TryGetProperty("kind", out var kindElement) => kindElement.GetString() ?? string.Empty,
            _ => string.Empty,
        };

        return new PipelineTransactionStatus
        {
            HashHex = content.TryGetProperty("hash", out var hashElement)
                ? NormalizeTransactionHashHex(hashElement.GetString() ?? transactionHashHex)
                : transactionHashHex,
            RawKind = rawKind,
            State = ParsePipelineTransactionState(rawKind),
            BlockHeight = ReadOptionalUInt64(statusElement, "block_height") ?? ReadOptionalUInt64(content, "block_height"),
            Scope = content.TryGetProperty("scope", out var scopeElement)
                ? scopeElement.GetString() ?? string.Empty
                : string.Empty,
            ResolvedFrom = content.TryGetProperty("resolved_from", out var resolvedElement)
                ? resolvedElement.GetString() ?? string.Empty
                : string.Empty,
            RejectionContentBase64 = statusElement.ValueKind == JsonValueKind.Object
                && statusElement.TryGetProperty("content", out var rejectionElement)
                && rejectionElement.ValueKind == JsonValueKind.String
                    ? rejectionElement.GetString()
                    : null,
        };
    }

    private static ulong? ReadOptionalUInt64(JsonElement element, string propertyName)
    {
        if (element.ValueKind != JsonValueKind.Object)
        {
            return null;
        }

        return element.TryGetProperty(propertyName, out var property)
            && property.ValueKind == JsonValueKind.Number
            && property.TryGetUInt64(out var value)
                ? value
                : null;
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
        if (string.IsNullOrWhiteSpace(transactionHashHex))
        {
            throw new ArgumentException("Transaction hash cannot be null or whitespace.", nameof(transactionHashHex));
        }

        var trimmed = transactionHashHex.Trim();
        var normalized = trimmed.StartsWith("0x", StringComparison.OrdinalIgnoreCase)
            ? trimmed[2..]
            : trimmed;

        if (normalized.Length != 64 || !normalized.All(static character => Uri.IsHexDigit(character)))
        {
            throw new ArgumentException("Transaction hash must be a 32-byte hex string.", nameof(transactionHashHex));
        }

        return normalized.ToLowerInvariant();
    }

    private static string NormalizePipelineScope(string scope)
    {
        if (string.IsNullOrWhiteSpace(scope))
        {
            return "auto";
        }

        var normalized = scope.Trim().ToLowerInvariant();
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

    private static string NormalizeUaidLiteral(string raw)
    {
        if (string.IsNullOrWhiteSpace(raw))
        {
            throw new ArgumentException("UAID literal cannot be null or whitespace.", nameof(raw));
        }

        var trimmed = raw.Trim();
        var hexPortion = trimmed.StartsWith("uaid:", StringComparison.OrdinalIgnoreCase)
            ? trimmed[5..].Trim()
            : trimmed;

        if (hexPortion.Length != 64 || !hexPortion.All(static character => Uri.IsHexDigit(character)))
        {
            throw new ArgumentException("UAID literal must be `uaid:<64 hex chars>` or a bare 64-character hex string.", nameof(raw));
        }

        return $"uaid:{hexPortion.ToLowerInvariant()}";
    }

    private static Uri EnsureTrailingSlash(Uri baseUri)
    {
        return baseUri.AbsoluteUri.Length > 0 && baseUri.AbsoluteUri[^1] == '/'
            ? baseUri
            : new Uri($"{baseUri.AbsoluteUri}/", UriKind.Absolute);
    }
}
