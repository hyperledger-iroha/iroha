using System.Net;
using System.Text.Json;
using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Locks identifier resolution to exact-network one-shot account authentication.</summary>
public sealed class ToriiIdentifierResolutionAuthenticationTests
{
    private const string AccountId =
        "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";
    private static readonly byte[] PrivateKeySeed = Convert.FromHexString(
        "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032");
    private static readonly NetworkId ExactNetwork = NetworkId.Parse(
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0");
    private static readonly NetworkId ForeignNetwork = NetworkId.Parse(
        "hash:0E5751C026E543B2E8AB2EB06099DAA1D1E5DF47778F7787FAAB45CDF12FE3A9#6A22");

    [Fact]
    public async Task ResolveRequiresCanonicalCredentialsBeforeDispatch()
    {
        using var handler = new CountingHandler();
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        await Assert.ThrowsAsync<InvalidOperationException>(() =>
            client.ResolveIdentifierAsync(Request(), TestContext.Current.CancellationToken));

        Assert.Equal(0, handler.Count);
    }

    [Fact]
    public async Task ResolveSignsExactPostOnceAndSurfacesRedirect()
    {
        using var handler = new CountingHandler(HttpStatusCode.TemporaryRedirect);
        using var client = AuthenticatedClient(handler);

        var error = await Assert.ThrowsAsync<ToriiApiException>(() =>
            client.ResolveIdentifierAsync(Request(), TestContext.Current.CancellationToken));

        Assert.Equal(HttpStatusCode.TemporaryRedirect, error.StatusCode);
        Assert.Equal(1, handler.Count);
        Assert.Equal("/v1/identifiers/resolve", handler.RequestPath);
        Assert.Equal("POST", handler.RequestMethod);
        Assert.True(handler.HasAccountHeader);
        using var body = JsonDocument.Parse(handler.RequestBody!);
        Assert.Equal("phone#retail", body.RootElement.GetProperty("policy_id").GetString());
        Assert.Equal("ciphertext", body.RootElement.GetProperty("encrypted_input").GetString());
    }

    [Fact]
    public async Task ResolveRejectsPrecomputedCanonicalHeadersBeforeDispatch()
    {
        using var handler = new CountingHandler();
        using var httpClient = new HttpClient(handler);
        httpClient.DefaultRequestHeaders.TryAddWithoutValidation("X-Iroha-Signature", "precomputed");
        using var client = AuthenticatedClient(handler, httpClient);

        await Assert.ThrowsAsync<InvalidOperationException>(() =>
            client.ResolveIdentifierAsync(Request(), TestContext.Current.CancellationToken));

        Assert.Equal(0, handler.Count);
    }

    [Fact]
    public async Task ResolveSignatureSeparatesSameAccountAcrossForeignGenesis()
    {
        using var localHandler = new CountingHandler(HttpStatusCode.NotFound);
        using var foreignHandler = new CountingHandler(HttpStatusCode.NotFound);
        using var localClient = AuthenticatedClient(localHandler);
        using var foreignClient = AuthenticatedClient(foreignHandler, networkId: ForeignNetwork);

        await Assert.ThrowsAsync<ToriiApiException>(() => localClient.ResolveIdentifierAsync(
            Request(), TestContext.Current.CancellationToken));
        await Assert.ThrowsAsync<ToriiApiException>(() => foreignClient.ResolveIdentifierAsync(
            Request(), TestContext.Current.CancellationToken));

        Assert.NotNull(localHandler.Signature);
        Assert.NotEqual(localHandler.Signature, foreignHandler.Signature);
    }

    private static ToriiIdentifierResolveRequest Request() => new()
    {
        PolicyId = "phone#retail",
        EncryptedInput = "ciphertext",
    };

    internal static ToriiClient AuthenticatedClient(
        HttpMessageHandler handler,
        HttpClient? httpClient = null,
        NetworkId? networkId = null) =>
        new(
            new Uri("https://torii.example"),
            httpClient ?? new HttpClient(handler),
            new ToriiClientOptions
            {
                NetworkId = networkId ?? ExactNetwork,
                CanonicalRequestCredentials = new CanonicalRequestCredentials(AccountId, PrivateKeySeed),
            },
            TransactionSubmissionTransportAssurance.OneShotWithoutRedirectsOrRetries);

    private sealed class CountingHandler(HttpStatusCode status = HttpStatusCode.OK) : HttpMessageHandler
    {
        public int Count { get; private set; }
        public string? RequestPath { get; private set; }
        public string? RequestMethod { get; private set; }
        public bool HasAccountHeader { get; private set; }
        public string? Signature { get; private set; }
        public string? RequestBody { get; private set; }

        protected override async Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken)
        {
            Count++;
            RequestPath = request.RequestUri!.AbsolutePath;
            RequestMethod = request.Method.Method;
            HasAccountHeader = request.Headers.Contains("X-Iroha-Account");
            Signature = request.Headers.GetValues("X-Iroha-Signature").Single();
            RequestBody = request.Content is null
                ? string.Empty
                : await request.Content.ReadAsStringAsync(cancellationToken);
            return new HttpResponseMessage(status)
            {
                RequestMessage = request,
                Headers = { Location = new Uri("https://redirect.example/replayed") },
                Content = new StringContent("{}"),
            };
        }
    }
}
