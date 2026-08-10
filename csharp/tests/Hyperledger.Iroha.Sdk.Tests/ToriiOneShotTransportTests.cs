using System.Collections.Concurrent;
using System.Net;
using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.Queries;
using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Verifies one-shot transport semantics for canonical signed bodies.</summary>
public sealed class ToriiOneShotTransportTests
{
    private const string CanonicalAccountId =
        "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";
    private const string CanonicalNetworkId =
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";
    private static readonly byte[] CanonicalPrivateKeySeed =
        Convert.FromHexString(
            "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032");

    [Theory]
    [InlineData(HttpStatusCode.TemporaryRedirect)]
    [InlineData(HttpStatusCode.PermanentRedirect)]
    public async Task SignedQueryRedirectIsSurfacedWithoutReplay(
        HttpStatusCode redirectStatus)
    {
        using var handler = new CountingHandler((request, _) =>
            Task.FromResult(RedirectResponse(request, redirectStatus)));
        using var client = CreateClient(handler);

        var error = await Assert.ThrowsAsync<ToriiApiException>(() =>
            client.SubmitSignedQueryAsync(
                ValidSignedQueryEnvelope(),
                cancellationToken: TestContext.Current.CancellationToken));

        Assert.Equal(redirectStatus, error.StatusCode);
        Assert.Equal(1, handler.CountFor("/v1/query"));
        Assert.Equal(0, handler.CountFor("/replayed"));
    }

    [Theory]
    [InlineData(HttpStatusCode.TemporaryRedirect)]
    [InlineData(HttpStatusCode.PermanentRedirect)]
    public async Task SignedTransactionDetailsRedirectIsSurfacedWithoutReplay(
        HttpStatusCode redirectStatus)
    {
        const string transactionHash =
            "1111111111111111111111111111111111111111111111111111111111111111";
        using var handler = new CountingHandler((request, _) =>
            Task.FromResult(RedirectResponse(request, redirectStatus)));
        using var client = CreateClient(handler);

        var error = await Assert.ThrowsAsync<ToriiApiException>(() =>
            client.GetPipelineTransactionDetailsAsync(
                new SignedIterableQueryBuilder(CanonicalAccountId, CanonicalNetworkId)
                    .FindTransactionDetails(transactionHash)
                    .BuildSigned(CanonicalPrivateKeySeed),
                transactionHash,
                TestContext.Current.CancellationToken));

        Assert.Equal(redirectStatus, error.StatusCode);
        Assert.Equal(1, handler.CountFor("/v1/pipeline/transactions/details"));
        Assert.Equal(0, handler.CountFor("/replayed"));
    }

    [Theory]
    [InlineData(HttpStatusCode.TemporaryRedirect)]
    [InlineData(HttpStatusCode.PermanentRedirect)]
    public async Task SignedTransactionRedirectIsSurfacedWithoutReplay(
        HttpStatusCode redirectStatus)
    {
        using var handler = TransactionHandler((request, _) =>
            Task.FromResult(RedirectResponse(request, redirectStatus)));
        using var client = CreateClient(handler);

        var error = await Assert.ThrowsAsync<ToriiApiException>(() =>
            client.SubmitTransactionAsync(
                ValidSignedTransactionEnvelope(),
                cancellationToken: TestContext.Current.CancellationToken));

        Assert.Equal(redirectStatus, error.StatusCode);
        Assert.Equal(1, handler.CountFor("/v1/node/capabilities"));
        Assert.Equal(1, handler.CountFor("/transaction"));
        Assert.Equal(0, handler.CountFor("/replayed"));
    }

    [Fact]
    public async Task SignedQueryNetworkFailureIsNotRetried()
    {
        using var handler = new CountingHandler((_, _) =>
            Task.FromException<HttpResponseMessage>(
                new HttpRequestException("ambiguous signed-query transport failure")));
        using var client = CreateClient(handler);

        await Assert.ThrowsAsync<HttpRequestException>(() =>
            client.SubmitSignedQueryAsync(
                ValidSignedQueryEnvelope(),
                cancellationToken: TestContext.Current.CancellationToken));

        Assert.Equal(1, handler.CountFor("/v1/query"));
    }

    [Fact]
    public async Task SignedTransactionNetworkFailureIsNotRetried()
    {
        using var handler = TransactionHandler((_, _) =>
            Task.FromException<HttpResponseMessage>(
                new HttpRequestException("ambiguous signed-transaction transport failure")));
        using var client = CreateClient(handler);

        await Assert.ThrowsAsync<HttpRequestException>(() =>
            client.SubmitTransactionAsync(
                ValidSignedTransactionEnvelope(),
                cancellationToken: TestContext.Current.CancellationToken));

        Assert.Equal(1, handler.CountFor("/v1/node/capabilities"));
        Assert.Equal(1, handler.CountFor("/transaction"));
    }

    [Fact]
    public async Task SignedQueryStatusFailureIsNotRetried()
    {
        using var handler = new CountingHandler((request, _) =>
            Task.FromResult(StatusFailureResponse(request)));
        using var client = CreateClient(handler);

        var error = await Assert.ThrowsAsync<ToriiApiException>(() =>
            client.SubmitSignedQueryAsync(
                ValidSignedQueryEnvelope(),
                cancellationToken: TestContext.Current.CancellationToken));

        Assert.Equal(HttpStatusCode.ServiceUnavailable, error.StatusCode);
        Assert.Equal(1, handler.CountFor("/v1/query"));
    }

    [Fact]
    public async Task SignedTransactionStatusFailureIsNotRetried()
    {
        using var handler = TransactionHandler((request, _) =>
            Task.FromResult(StatusFailureResponse(request)));
        using var client = CreateClient(handler);

        var error = await Assert.ThrowsAsync<ToriiApiException>(() =>
            client.SubmitTransactionAsync(
                ValidSignedTransactionEnvelope(),
                cancellationToken: TestContext.Current.CancellationToken));

        Assert.Equal(HttpStatusCode.ServiceUnavailable, error.StatusCode);
        Assert.Equal(1, handler.CountFor("/v1/node/capabilities"));
        Assert.Equal(1, handler.CountFor("/transaction"));
    }

    private static ToriiClient CreateClient(HttpMessageHandler handler)
    {
        return new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler),
            new ToriiClientOptions
            {
                LocalSigningContext = new ToriiLocalSigningContext(
                    NetworkId.Parse(CanonicalNetworkId)),
                CanonicalRequestCredentials = new CanonicalRequestCredentials(
                    CanonicalAccountId,
                    CanonicalPrivateKeySeed),
            });
    }

    private static CountingHandler TransactionHandler(
        Func<HttpRequestMessage, CancellationToken, Task<HttpResponseMessage>>
            transactionResponse)
    {
        return new CountingHandler((request, cancellationToken) =>
        {
            if (request.RequestUri!.AbsolutePath == "/v1/node/capabilities")
            {
                return Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK)
                {
                    RequestMessage = request,
                    Content = new StringContent(
                        ToriiClientTests.TransactionSubmissionCapabilitiesJson(),
                        System.Text.Encoding.UTF8,
                        "application/json"),
                });
            }

            Assert.Equal("/transaction", request.RequestUri.AbsolutePath);
            return transactionResponse(request, cancellationToken);
        });
    }

    private static HttpResponseMessage RedirectResponse(
        HttpRequestMessage request,
        HttpStatusCode status)
    {
        return new HttpResponseMessage(status)
        {
            RequestMessage = request,
            Headers = { Location = new Uri("https://redirect.example/replayed") },
            Content = new ByteArrayContent([]),
        };
    }

    private static HttpResponseMessage StatusFailureResponse(
        HttpRequestMessage request)
    {
        return new HttpResponseMessage(HttpStatusCode.ServiceUnavailable)
        {
            RequestMessage = request,
            Content = new StringContent("temporarily unavailable"),
        };
    }

    private static SignedQueryEnvelope ValidSignedQueryEnvelope()
    {
        return new SignedQueryBuilder(CanonicalAccountId, CanonicalNetworkId)
            .FindParameters()
            .BuildSigned(CanonicalPrivateKeySeed);
    }

    private static SignedTransactionEnvelope ValidSignedTransactionEnvelope()
    {
        return new TransactionBuilder(
            NetworkId.Parse(CanonicalNetworkId),
            CanonicalAccountId,
            FeePaymentIntent.Authority([]))
            .TransferAsset(
                "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                "15.75",
                CanonicalAccountId)
            .SetCreationTimeMilliseconds(1736000000000)
            .SetTimeToLiveMilliseconds(3500)
            .SetNonce(17)
            .BuildSigned(CanonicalPrivateKeySeed);
    }

    private sealed class CountingHandler(
        Func<HttpRequestMessage, CancellationToken, Task<HttpResponseMessage>> responder)
        : HttpMessageHandler
    {
        private readonly ConcurrentDictionary<string, int> requestCounts =
            new(StringComparer.Ordinal);

        public int CountFor(string absolutePath)
        {
            return requestCounts.TryGetValue(absolutePath, out var count) ? count : 0;
        }

        protected override async Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken)
        {
            requestCounts.AddOrUpdate(
                request.RequestUri!.AbsolutePath,
                1,
                static (_, count) => count + 1);
            var response = await responder(request, cancellationToken);
            response.RequestMessage ??= request;
            return response;
        }
    }
}
