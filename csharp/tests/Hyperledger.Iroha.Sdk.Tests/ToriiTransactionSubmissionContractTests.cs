using System.Net;
using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>
/// Verifies the single canonical V1 transaction-submission contract.
/// </summary>
public sealed class ToriiTransactionSubmissionContractTests
{
    private const string CanonicalNetworkId = "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";
    private const string CanonicalAccountId =
        "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";
    private static readonly byte[] CanonicalPrivateKeySeed =
        Convert.FromHexString(
            "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032");

    [Fact]
    public void SubmitTransactionContractConstantsMatchCanonicalV1()
    {
        Assert.Equal(4, ToriiNodeCapabilities.ExpectedDataModelVersion);
        Assert.Equal(
            "7ab5ff9c572efb316deac478f19209c5",
            ToriiNodeCapabilities.ExpectedSignedTransactionSchemaHashHex);
    }

    [Fact]
    public async Task SubmitTransactionAsyncPostsNoritoPayload()
    {
        var transaction = ValidSignedTransactionEnvelope();
        var requestPaths = new List<string>();

        using var handler = new RecordingHandler(request =>
        {
            requestPaths.Add(request.RequestUri!.AbsolutePath);
            if (request.RequestUri.AbsolutePath == "/v1/node/capabilities")
            {
                Assert.Equal(HttpMethod.Get, request.Method);
                return JsonResponse(ToriiClientTests.TransactionSubmissionCapabilitiesJson());
            }

            Assert.Equal(HttpMethod.Post, request.Method);
            Assert.Equal("/v1/pipeline/transactions", request.RequestUri!.AbsolutePath);
            Assert.Equal(
                "application/x-norito",
                request.Content!.Headers.ContentType!.MediaType);
            using var stream = request.Content.ReadAsStream();
            using var buffer = new MemoryStream();
            stream.CopyTo(buffer);
            Assert.Equal(1, transaction.VersionedNoritoBytes[0]);
            Assert.Equal(transaction.SignedTransactionBytes, transaction.VersionedNoritoBytes[1..]);
            Assert.Equal(transaction.VersionedNoritoBytes, buffer.ToArray());

            return new HttpResponseMessage(HttpStatusCode.Accepted)
            {
                Content = new ByteArrayContent(Array.Empty<byte>()),
            };
        });

        using var client = CreateAuthenticatedClient(handler);
        await client.SubmitTransactionAsync(
            transaction,
            cancellationToken: TestContext.Current.CancellationToken);

        Assert.Equal(["/v1/node/capabilities", "/v1/pipeline/transactions"], requestPaths);
    }

    [Theory]
    [InlineData(HttpStatusCode.OK)]
    [InlineData(HttpStatusCode.Created)]
    [InlineData(HttpStatusCode.NoContent)]
    public async Task SubmitTransactionAsyncRejectsNonCanonicalAdmissionStatus(
        HttpStatusCode statusCode)
    {
        var requestPaths = new List<string>();
        using var handler = new RecordingHandler(request =>
        {
            requestPaths.Add(request.RequestUri!.AbsolutePath);
            if (request.RequestUri.AbsolutePath == "/v1/node/capabilities")
            {
                return JsonResponse(ToriiClientTests.TransactionSubmissionCapabilitiesJson());
            }

            Assert.Equal("/v1/pipeline/transactions", request.RequestUri.AbsolutePath);
            return new HttpResponseMessage(statusCode)
            {
                Content = new ByteArrayContent(Array.Empty<byte>()),
            };
        });
        using var client = CreateAuthenticatedClient(handler);

        var error = await Assert.ThrowsAsync<ToriiApiException>(() =>
            client.SubmitTransactionAsync(
                ValidSignedTransactionEnvelope(),
                cancellationToken: TestContext.Current.CancellationToken));

        Assert.Equal(statusCode, error.StatusCode);
        Assert.Equal(["/v1/node/capabilities", "/v1/pipeline/transactions"], requestPaths);
    }

    [Theory]
    [InlineData(3)]
    [InlineData(5)]
    public async Task SubmitTransactionAsyncRejectsMismatchedDataModelVersionBeforePost(
        int advertisedVersion)
    {
        var requestPaths = new List<string>();
        using var handler = new RecordingHandler(request =>
        {
            requestPaths.Add(request.RequestUri!.AbsolutePath);
            if (request.RequestUri.AbsolutePath == "/v1/node/capabilities")
            {
                return JsonResponse(
                    ToriiClientTests.TransactionSubmissionCapabilitiesJson(
                        dataModelVersion: advertisedVersion));
            }

            throw new InvalidOperationException(
                "transaction posted after data-model mismatch");
        });
        using var client = CreateAuthenticatedClient(handler);

        var error = await Assert.ThrowsAsync<ToriiDataModelMismatchException>(() =>
            client.SubmitTransactionAsync(
                ValidSignedTransactionEnvelope(),
                cancellationToken: TestContext.Current.CancellationToken));

        Assert.Equal(ToriiNodeCapabilities.ExpectedDataModelVersion, error.Expected);
        Assert.Equal(advertisedVersion, error.Actual);
        Assert.Equal(["/v1/node/capabilities"], requestPaths);
    }

    [Fact]
    public async Task SubmitTransactionAsyncRejectsMismatchedSchemaBeforePost()
    {
        var advertisedSchemaHash = new string('0', 32);
        var requestPaths = new List<string>();
        using var handler = new RecordingHandler(request =>
        {
            requestPaths.Add(request.RequestUri!.AbsolutePath);
            if (request.RequestUri.AbsolutePath == "/v1/node/capabilities")
            {
                return JsonResponse(
                    ToriiClientTests.TransactionSubmissionCapabilitiesJson(
                        signedTransactionSchemaHashHex: advertisedSchemaHash));
            }

            throw new InvalidOperationException(
                "transaction posted after schema mismatch");
        });
        using var client = CreateAuthenticatedClient(handler);

        var error = await Assert.ThrowsAsync<ToriiTransactionSchemaMismatchException>(() =>
            client.SubmitTransactionAsync(
                ValidSignedTransactionEnvelope(),
                cancellationToken: TestContext.Current.CancellationToken));

        Assert.Equal(
            ToriiNodeCapabilities.ExpectedSignedTransactionSchemaHashHex,
            error.Expected);
        Assert.Equal(advertisedSchemaHash, error.Actual);
        Assert.Equal(["/v1/node/capabilities"], requestPaths);
    }

    [Fact]
    public async Task SubmitTransactionAsyncFetchesFreshCapabilitiesBeforeEveryPost()
    {
        var requestPaths = new List<string>();
        var capabilitiesRequests = 0;
        using var handler = new RecordingHandler(request =>
        {
            var path = request.RequestUri!.AbsolutePath;
            requestPaths.Add(path);
            if (path == "/v1/node/capabilities")
            {
                capabilitiesRequests++;
                return JsonResponse(
                    ToriiClientTests.TransactionSubmissionCapabilitiesJson(
                        dataModelVersion: capabilitiesRequests == 1
                            ? ToriiNodeCapabilities.ExpectedDataModelVersion
                            : ToriiNodeCapabilities.ExpectedDataModelVersion + 1));
            }

            Assert.Equal("/v1/pipeline/transactions", path);
            return new HttpResponseMessage(HttpStatusCode.Accepted)
            {
                Content = new ByteArrayContent(Array.Empty<byte>()),
            };
        });
        using var client = CreateAuthenticatedClient(handler);
        var transaction = ValidSignedTransactionEnvelope();

        await client.SubmitTransactionAsync(
            transaction,
            cancellationToken: TestContext.Current.CancellationToken);
        await Assert.ThrowsAsync<ToriiDataModelMismatchException>(() =>
            client.SubmitTransactionAsync(
                transaction,
                cancellationToken: TestContext.Current.CancellationToken));

        Assert.Equal(
            ["/v1/node/capabilities", "/v1/pipeline/transactions", "/v1/node/capabilities"],
            requestPaths);
    }

    [Fact]
    public async Task SubmitTransactionAsyncRejectsReplayCapableInjectedTransportBeforeDispatch()
    {
        var terminal = new ReplayProbeTerminalHandler();
        using var httpClient = new HttpClient(
            new ReplayTransactionPostHandler(terminal));

        using (var probe = await httpClient.PostAsync(
                   "https://torii.example/v1/pipeline/transactions",
                   new ByteArrayContent([1]),
                   TestContext.Current.CancellationToken))
        {
            Assert.Equal(HttpStatusCode.Accepted, probe.StatusCode);
        }
        Assert.Equal(2, terminal.TransactionPosts);

        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            httpClient,
            new ToriiClientOptions
            {
                LocalSigningContext = new ToriiLocalSigningContext(
                    NetworkId.Parse(CanonicalNetworkId)),
                CanonicalRequestCredentials = new CanonicalRequestCredentials(
                    CanonicalAccountId,
                    CanonicalPrivateKeySeed),
            });

        var error = await Assert.ThrowsAsync<InvalidOperationException>(() =>
            client.SubmitTransactionAsync(
                ValidSignedTransactionEnvelope(),
                TestContext.Current.CancellationToken));

        Assert.Contains("internally managed one-shot", error.Message);
        Assert.Equal(0, terminal.CapabilitiesGets);
        Assert.Equal(2, terminal.TransactionPosts);
    }

    private static SignedTransactionEnvelope ValidSignedTransactionEnvelope()
    {
        return new TransactionBuilder(
            NetworkId.Parse(CanonicalNetworkId),
            CanonicalAccountId,
            FeePaymentIntent.Authority(Array.Empty<FeeChargeLimit>()))
            .TransferAsset(
                "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                "15.75",
                CanonicalAccountId)
            .SetCreationTimeMilliseconds(1736000000000)
            .SetTimeToLiveMilliseconds(3500)
            .SetNonce(17)
            .BuildSigned(CanonicalPrivateKeySeed);
    }

    private static ToriiClient CreateAuthenticatedClient(HttpMessageHandler handler) =>
        new(
            new Uri("https://torii.example"),
            new HttpClient(handler),
            new ToriiClientOptions
            {
                LocalSigningContext = new ToriiLocalSigningContext(
                    NetworkId.Parse(CanonicalNetworkId)),
                CanonicalRequestCredentials = new CanonicalRequestCredentials(
                    CanonicalAccountId,
                    CanonicalPrivateKeySeed),
            },
            TransactionSubmissionTransportAssurance.OneShotWithoutRedirectsOrRetries);

    private static HttpResponseMessage JsonResponse(string json)
    {
        return new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent(
                json,
                System.Text.Encoding.UTF8,
                "application/json"),
        };
    }

    private sealed class RecordingHandler : HttpMessageHandler
    {
        private readonly Func<HttpRequestMessage, HttpResponseMessage> responder;

        public RecordingHandler(Func<HttpRequestMessage, HttpResponseMessage> responder)
        {
            this.responder = responder;
        }

        protected override Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken)
        {
            var response = responder(request);
            response.RequestMessage ??= request;
            return Task.FromResult(response);
        }
    }

    private sealed class ReplayTransactionPostHandler(HttpMessageHandler innerHandler)
        : DelegatingHandler(innerHandler)
    {
        protected override async Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken)
        {
            if (request.RequestUri?.AbsolutePath != "/v1/pipeline/transactions")
            {
                return await base.SendAsync(request, cancellationToken);
            }

            using var replay = await CloneAsync(request, cancellationToken);
            using var first = await base.SendAsync(replay, cancellationToken);
            return await base.SendAsync(request, cancellationToken);
        }

        private static async Task<HttpRequestMessage> CloneAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken)
        {
            var clone = new HttpRequestMessage(request.Method, request.RequestUri);
            foreach (var header in request.Headers)
            {
                clone.Headers.TryAddWithoutValidation(header.Key, header.Value);
            }
            if (request.Content is not null)
            {
                clone.Content = new ByteArrayContent(
                    await request.Content.ReadAsByteArrayAsync(cancellationToken));
                foreach (var header in request.Content.Headers)
                {
                    clone.Content.Headers.TryAddWithoutValidation(header.Key, header.Value);
                }
            }
            return clone;
        }
    }

    private sealed class ReplayProbeTerminalHandler : HttpMessageHandler
    {
        private int capabilitiesGets;
        private int transactionPosts;

        public int CapabilitiesGets => Volatile.Read(ref capabilitiesGets);

        public int TransactionPosts => Volatile.Read(ref transactionPosts);

        protected override Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken)
        {
            HttpResponseMessage response;
            if (request.RequestUri?.AbsolutePath == "/v1/node/capabilities")
            {
                Interlocked.Increment(ref capabilitiesGets);
                response = JsonResponse(ToriiClientTests.TransactionSubmissionCapabilitiesJson());
            }
            else
            {
                Assert.Equal("/v1/pipeline/transactions", request.RequestUri?.AbsolutePath);
                Interlocked.Increment(ref transactionPosts);
                response = new HttpResponseMessage(HttpStatusCode.Accepted)
                {
                    Content = new ByteArrayContent([]),
                };
            }
            response.RequestMessage = request;
            return Task.FromResult(response);
        }
    }
}
