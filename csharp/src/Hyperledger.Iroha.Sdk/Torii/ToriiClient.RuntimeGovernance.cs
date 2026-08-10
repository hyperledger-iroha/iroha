namespace Hyperledger.Iroha.Torii;

public sealed partial class ToriiClient
{
    /// <summary>Reads the active runtime ABI through exact network-bound account authentication.</summary>
    public async Task<ToriiRuntimeAbiActive> GetRuntimeAbiActiveAsync(
        CancellationToken cancellationToken = default)
    {
        var response = await GetAuthenticatedRuntimeGovernanceAsync<ToriiRuntimeAbiActive>(
            "/v1/runtime/abi/active",
            cancellationToken);
        ValidateRuntimeAbiActive(response, "runtime ABI active response");
        return response;
    }

    /// <summary>Reads bounded runtime metrics through exact network-bound account authentication.</summary>
    public async Task<ToriiRuntimeMetrics> GetRuntimeMetricsAsync(
        CancellationToken cancellationToken = default)
    {
        var response = await GetAuthenticatedRuntimeGovernanceAsync<ToriiRuntimeMetrics>(
            "/v1/runtime/metrics",
            cancellationToken);
        ValidateRuntimeMetrics(response, "runtime metrics response");
        return response;
    }

    private async Task<TResponse> GetAuthenticatedRuntimeGovernanceAsync<TResponse>(
        string path,
        CancellationToken cancellationToken)
    {
        RequireCanonicalRequestCredentials(path);
        if (Options.LocalSigningContext is null)
        {
            throw new InvalidOperationException(
                $"Route `{path}` requires ToriiClientOptions.LocalSigningContext with the exact NetworkId.");
        }

        var expectedUri = BuildRequestUri(path, query: null);
        using var response = await SendAsync(
            HttpMethod.Get,
            path,
            query: null,
            content: null,
            cancellationToken: cancellationToken);
        if (response.RequestMessage?.RequestUri is not Uri responseUri
            || !string.Equals(
                responseUri.AbsoluteUri,
                expectedUri.AbsoluteUri,
                StringComparison.Ordinal))
        {
            throw new HttpRequestException(
                $"Authenticated Torii route `{path}` changed the exact request target.");
        }

        return await DeserializeAsync<TResponse>(response, cancellationToken);
    }
}
