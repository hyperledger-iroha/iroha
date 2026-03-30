using System.Globalization;
using System.Net;
using System.Net.Http.Headers;
using System.Runtime.CompilerServices;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization.Metadata;
using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.Queries;
using Hyperledger.Iroha.Transactions;

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

    public Task<ToriiIdentifierPoliciesResponse> GetIdentifierPoliciesAsync(CancellationToken cancellationToken = default)
    {
        return GetAsync<ToriiIdentifierPoliciesResponse>("/v1/identifier-policies", cancellationToken: cancellationToken);
    }

    public Task<ToriiIdentifierResolveResponse> ResolveIdentifierAsync(
        ToriiIdentifierResolveRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);

        return PostAsync<ToriiIdentifierResolveRequest, ToriiIdentifierResolveResponse>(
            "/v1/identifiers/resolve",
            request,
            cancellationToken: cancellationToken);
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

    public Task<ToriiVpnSession> CreateVpnSessionAsync(CancellationToken cancellationToken = default)
    {
        return CreateVpnSessionAsync(new ToriiVpnSessionCreateRequest(), cancellationToken);
    }

    public Task<ToriiVpnSessionDeleteResponse> DeleteVpnSessionAsync(
        string sessionId,
        CancellationToken cancellationToken = default)
    {
        var normalizedSessionId = NormalizeRequiredValue(sessionId, nameof(sessionId));
        return SendAllowingStatusAndDeserializeAsync<ToriiVpnSessionDeleteResponse>(
            HttpMethod.Delete,
            $"/v1/vpn/sessions/{EncodePathSegment(normalizedSessionId)}",
            query: null,
            content: null,
            HttpStatusCode.NotFound,
            cancellationToken);
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

    public Task<ToriiSoraFsCidLookupResponse> GetSoraFsCidLookupAsync(
        string cid,
        CancellationToken cancellationToken = default)
    {
        var normalizedCid = NormalizeRequiredValue(cid, nameof(cid));
        return GetAsync<ToriiSoraFsCidLookupResponse>(
            $"/v1/sorafs/cid/{EncodePathSegment(normalizedCid)}",
            cancellationToken: cancellationToken);
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
        return OpenSseAsync("/v1/events/sse", query, lastEventId, cancellationToken);
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

    private static string NormalizeRequiredValue(string? value, string paramName)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException("Value cannot be null or whitespace.", paramName);
        }

        return value.Trim();
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
