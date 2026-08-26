using System.Net;
using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>
/// Verifies that transaction submission is gated by fresh canonical Torii capabilities.
/// </summary>
public sealed class ToriiTransactionSubmissionCompatibilityTests
{
    private const string CanonicalNetworkId = "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149";
    private const string CanonicalAccountId =
        "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";
    private static readonly byte[] CanonicalPrivateKeySeed =
        Convert.FromHexString(
            "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032");

    [Fact]
    public void SubmitTransactionCompatibilityConstantsMatchCanonicalV1()
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
            Assert.Equal("/transaction", request.RequestUri!.AbsolutePath);
            Assert.Equal(
                "application/x-norito",
                request.Content!.Headers.ContentType!.MediaType);
            using var stream = request.Content.ReadAsStream();
            using var buffer = new MemoryStream();
            stream.CopyTo(buffer);
            Assert.Equal(transaction.NoritoBytes, buffer.ToArray());

            return new HttpResponseMessage(HttpStatusCode.Accepted)
            {
                Content = new ByteArrayContent(Array.Empty<byte>()),
            };
        });

        using var client = CreateAuthenticatedClient(handler);
        await client.SubmitTransactionAsync(
            transaction,
            cancellationToken: TestContext.Current.CancellationToken);

        Assert.Equal(["/v1/node/capabilities", "/transaction"], requestPaths);
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

            Assert.Equal("/transaction", path);
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
            ["/v1/node/capabilities", "/transaction", "/v1/node/capabilities"],
            requestPaths);
    }

    private static SignedTransactionEnvelope ValidSignedTransactionEnvelope()
    {
        return new TransactionBuilder(
            NetworkId.Parse(CanonicalNetworkId),
            global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant,
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
                    NetworkId.Parse(CanonicalNetworkId),
                    global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant),
                CanonicalRequestCredentials = new CanonicalRequestCredentials(
                    CanonicalAccountId,
                    CanonicalPrivateKeySeed),
            });

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
}
