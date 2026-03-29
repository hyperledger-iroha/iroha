using System.Globalization;
using System.Net;
using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization.Metadata;
using Hyperledger.Iroha.Http;
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
        using var response = await SendAsync(HttpMethod.Get, path, query, content: null, cancellationToken);
        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken);
        return await JsonDocument.ParseAsync(stream, cancellationToken: cancellationToken);
    }

    public async Task<TResponse> GetAsync<TResponse>(string path, string? query = null, CancellationToken cancellationToken = default)
    {
        using var response = await SendAsync(HttpMethod.Get, path, query, content: null, cancellationToken);
        return await DeserializeAsync<TResponse>(response, cancellationToken);
    }

    public async Task<TResponse> PostAsync<TRequest, TResponse>(
        string path,
        TRequest request,
        string? query = null,
        CancellationToken cancellationToken = default)
    {
        using var content = CreateJsonContent(request);
        using var response = await SendAsync(HttpMethod.Post, path, query, content, cancellationToken);
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
        using var response = await SendAsync(HttpMethod.Post, path, query, content, cancellationToken);
        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken);
        return await JsonDocument.ParseAsync(stream, cancellationToken: cancellationToken);
    }

    public async Task<HttpResponseMessage> SendAsync(
        HttpMethod method,
        string path,
        string? query = null,
        HttpContent? content = null,
        CancellationToken cancellationToken = default)
    {
        var request = await CreateRequestAsync(method, path, query, content, cancellationToken);
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

    private async Task<HttpResponseMessage> SendAllowingStatusAsync(
        HttpMethod method,
        string path,
        string? query,
        HttpContent? content,
        HttpStatusCode allowedStatusCode,
        CancellationToken cancellationToken)
    {
        var request = await CreateRequestAsync(method, path, query, content, cancellationToken);
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

        return request;
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
