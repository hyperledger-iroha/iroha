using System.Globalization;
using System.Net;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Locks runtime-governance reads to exact one-shot canonical authentication.</summary>
public sealed class ToriiRuntimeGovernanceAuthenticationTests
{
    private const string AccountId =
        "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";
    private const string ExactNetworkIdLiteral =
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";
    private const string ForeignNetworkIdLiteral =
        "hash:82531CE8EAE8BFF6BEECA4698BFD13A3BC8BEC5F0EE0D23D428C97FC17AB0F3B#3E94";
    private static readonly byte[] PrivateKeySeed = Convert.FromHexString(
        "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032");

    public static IEnumerable<object[]> AuthenticatedPaths()
    {
        yield return ["/v1/node/capabilities"];
        yield return ["/v1/runtime/abi/active"];
        yield return ["/v1/runtime/metrics"];
    }

    [Theory]
    [MemberData(nameof(AuthenticatedPaths))]
    public async Task AuthenticatedReadsRejectMissingCredentialsBeforeDispatch(string path)
    {
        using var handler = new RecordingHandler(ResponseForPath);
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler),
            new ToriiClientOptions
            {
                LocalSigningContext = new ToriiLocalSigningContext(
                    NetworkId.Parse(ExactNetworkIdLiteral)),
            });

        var error = await Assert.ThrowsAsync<InvalidOperationException>(() =>
            InvokeAsync(client, path));

        Assert.Contains(nameof(ToriiClientOptions.CanonicalRequestCredentials), error.Message);
        Assert.Empty(handler.Requests);
    }

    [Theory]
    [MemberData(nameof(AuthenticatedPaths))]
    public async Task AuthenticatedReadsRejectMissingNetworkContextBeforeDispatch(string path)
    {
        using var handler = new RecordingHandler(ResponseForPath);
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler),
            new ToriiClientOptions
            {
                CanonicalRequestCredentials = new CanonicalRequestCredentials(
                    AccountId,
                    PrivateKeySeed),
            });

        var error = await Assert.ThrowsAsync<InvalidOperationException>(() =>
            InvokeAsync(client, path));

        Assert.Contains(nameof(ToriiClientOptions.LocalSigningContext), error.Message);
        Assert.Empty(handler.Requests);
    }

    [Fact]
    public async Task AuthenticatedReadsSignExactNetworkMethodPathQueryAndBody()
    {
        using var handler = new RecordingHandler(ResponseForPath);
        using var client = CreateAuthenticatedClient(handler);

        foreach (var path in AuthenticatedPaths().Select(item => (string)item[0]))
        {
            await InvokeAsync(client, path);
        }

        Assert.Equal(3, handler.Requests.Count);
        foreach (var request in handler.Requests)
        {
            Assert.Equal("GET", request.Method);
            Assert.Equal(string.Empty, request.Query);
            Assert.Null(request.Body);
            Assert.Equal(
                AccountAddress.Parse(AccountId, AccountAddress.DefaultChainDiscriminant).CanonicalHex,
                Header(request, "X-Iroha-Account"));

            var timestamp = long.Parse(
                Header(request, "X-Iroha-Timestamp-Ms"),
                NumberStyles.None,
                CultureInfo.InvariantCulture);
            var nonce = Header(request, "X-Iroha-Nonce");
            var signature = Convert.FromBase64String(Header(request, "X-Iroha-Signature"));
            var publicKey = Ed25519Signer.GetPublicKey(PrivateKeySeed);
            var exactMessage = CanonicalRequest.BuildSignatureMessage(
                NetworkId.Parse(ExactNetworkIdLiteral),
                request.Method,
                request.Path,
                request.Query,
                ReadOnlySpan<byte>.Empty,
                timestamp,
                nonce);
            Assert.True(Ed25519Signer.Verify(exactMessage, signature, publicKey));

            var foreignNetworkMessage = CanonicalRequest.BuildSignatureMessage(
                NetworkId.Parse(ForeignNetworkIdLiteral),
                request.Method,
                request.Path,
                request.Query,
                ReadOnlySpan<byte>.Empty,
                timestamp,
                nonce);
            Assert.False(Ed25519Signer.Verify(foreignNetworkMessage, signature, publicKey));

            var foreignPathMessage = CanonicalRequest.BuildSignatureMessage(
                NetworkId.Parse(ExactNetworkIdLiteral),
                request.Method,
                request.Path + "/other",
                request.Query,
                ReadOnlySpan<byte>.Empty,
                timestamp,
                nonce);
            Assert.False(Ed25519Signer.Verify(foreignPathMessage, signature, publicKey));

            var foreignQueryMessage = CanonicalRequest.BuildSignatureMessage(
                NetworkId.Parse(ExactNetworkIdLiteral),
                request.Method,
                request.Path,
                "probe=1",
                ReadOnlySpan<byte>.Empty,
                timestamp,
                nonce);
            Assert.False(Ed25519Signer.Verify(foreignQueryMessage, signature, publicKey));
        }
    }

    [Fact]
    public async Task TransactionCompatibilityProbeRequiresAuthenticationBeforeDispatch()
    {
        using var handler = new RecordingHandler(ResponseForPath);
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        var error = await Assert.ThrowsAsync<InvalidOperationException>(() =>
            client.SubmitTransactionAsync(
                new byte[] { 1 },
                TestContext.Current.CancellationToken));

        Assert.Contains(nameof(ToriiClientOptions.CanonicalRequestCredentials), error.Message);
        Assert.Empty(handler.Requests);
    }

    [Fact]
    public async Task WrongNetworkContextCannotAuthenticateAgainstPinnedNetwork()
    {
        using var handler = new RecordingHandler(request =>
        {
            var timestamp = long.Parse(
                Assert.Single(request.Headers.GetValues("X-Iroha-Timestamp-Ms")),
                NumberStyles.None,
                CultureInfo.InvariantCulture);
            var nonce = Assert.Single(request.Headers.GetValues("X-Iroha-Nonce"));
            var signature = Convert.FromBase64String(
                Assert.Single(request.Headers.GetValues("X-Iroha-Signature")));
            var expectedMessage = CanonicalRequest.BuildSignatureMessage(
                NetworkId.Parse(ExactNetworkIdLiteral),
                request.Method.Method,
                request.RequestUri!.AbsolutePath,
                request.RequestUri.Query,
                ReadOnlySpan<byte>.Empty,
                timestamp,
                nonce);
            Assert.False(Ed25519Signer.Verify(
                expectedMessage,
                signature,
                Ed25519Signer.GetPublicKey(PrivateKeySeed)));
            return new HttpResponseMessage(HttpStatusCode.Unauthorized)
            {
                Content = new StringContent("wrong network"),
            };
        });
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler),
            new ToriiClientOptions
            {
                LocalSigningContext = new ToriiLocalSigningContext(
                    NetworkId.Parse(ForeignNetworkIdLiteral)),
                CanonicalRequestCredentials = new CanonicalRequestCredentials(
                    AccountId,
                    PrivateKeySeed),
            });

        var error = await Assert.ThrowsAsync<ToriiApiException>(() =>
            client.GetNodeCapabilitiesAsync(TestContext.Current.CancellationToken));

        Assert.Equal(HttpStatusCode.Unauthorized, error.StatusCode);
        Assert.Single(handler.Requests);
    }

    [Theory]
    [MemberData(nameof(AuthenticatedPaths))]
    public async Task AuthenticatedReadRedirectIsSurfacedWithoutReplay(string path)
    {
        using var handler = new RecordingHandler(request => new HttpResponseMessage(
            HttpStatusCode.TemporaryRedirect)
        {
            Headers = { Location = new Uri("https://redirect.example/replayed") },
            Content = new ByteArrayContent([]),
        });
        using var client = CreateAuthenticatedClient(handler);

        var error = await Assert.ThrowsAsync<ToriiApiException>(() =>
            InvokeAsync(client, path));

        Assert.Equal(HttpStatusCode.TemporaryRedirect, error.StatusCode);
        Assert.Single(handler.Requests);
        Assert.Equal(path, handler.Requests[0].Path);
    }

    [Fact]
    public async Task AuthenticatedReadRejectsSuccessfulResponseFromChangedTarget()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            RequestMessage = new HttpRequestMessage(
                HttpMethod.Get,
                new Uri("https://torii.example/replayed")),
            Content = new StringContent("{\"abi_version\":1}"),
        });
        using var client = CreateAuthenticatedClient(handler);

        var error = await Assert.ThrowsAsync<HttpRequestException>(() =>
            client.GetRuntimeAbiActiveAsync(TestContext.Current.CancellationToken));

        Assert.Contains("changed the exact request target", error.Message);
        Assert.Single(handler.Requests);
    }

    [Fact]
    public async Task AuthenticatedReadTransportFailureIsNotRetried()
    {
        using var handler = new RecordingHandler(_ =>
            throw new HttpRequestException("ambiguous authenticated read failure"));
        using var client = CreateAuthenticatedClient(handler);

        await Assert.ThrowsAsync<HttpRequestException>(() =>
            client.GetRuntimeMetricsAsync(TestContext.Current.CancellationToken));

        Assert.Single(handler.Requests);
        Assert.Equal("/v1/runtime/metrics", handler.Requests[0].Path);
    }

    [Fact]
    public async Task RuntimeAbiHashRemainsPublic()
    {
        using var handler = new RecordingHandler(ResponseForPath);
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        var response = await client.GetRuntimeAbiHashAsync(
            TestContext.Current.CancellationToken);

        Assert.Equal("V1", response.Policy);
        var request = Assert.Single(handler.Requests);
        Assert.Equal("/v1/runtime/abi/hash", request.Path);
        Assert.False(request.Headers.ContainsKey("X-Iroha-Signature"));
    }

    private static ToriiClient CreateAuthenticatedClient(HttpMessageHandler handler) =>
        new(
            new Uri("https://torii.example"),
            new HttpClient(handler),
            new ToriiClientOptions
            {
                LocalSigningContext = new ToriiLocalSigningContext(
                    NetworkId.Parse(ExactNetworkIdLiteral)),
                CanonicalRequestCredentials = new CanonicalRequestCredentials(
                    AccountId,
                    PrivateKeySeed),
            });

    private static async Task InvokeAsync(ToriiClient client, string path)
    {
        switch (path)
        {
            case "/v1/node/capabilities":
                _ = await client.GetNodeCapabilitiesAsync(TestContext.Current.CancellationToken);
                break;
            case "/v1/runtime/abi/active":
                _ = await client.GetRuntimeAbiActiveAsync(TestContext.Current.CancellationToken);
                break;
            case "/v1/runtime/metrics":
                _ = await client.GetRuntimeMetricsAsync(TestContext.Current.CancellationToken);
                break;
            default:
                throw new ArgumentOutOfRangeException(nameof(path), path, "Unknown authenticated route.");
        }
    }

    private static HttpResponseMessage ResponseForPath(HttpRequestMessage request)
    {
        var body = request.RequestUri!.AbsolutePath switch
        {
            "/v1/node/capabilities" =>
                ToriiClientTests.TransactionSubmissionCapabilitiesJson(),
            "/v1/runtime/abi/active" => "{\"abi_version\":1}",
            "/v1/runtime/metrics" =>
                "{\"abi_version\":1,\"upgrade_events_total\":{\"proposed\":0,\"activated\":0,\"canceled\":0}}",
            "/v1/runtime/abi/hash" =>
                "{\"policy\":\"V1\",\"abi_hash_hex\":\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"}",
            _ => throw new InvalidOperationException("Unexpected route."),
        };
        return new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent(body),
        };
    }

    private static string Header(CapturedRequest request, string name) =>
        Assert.Single(request.Headers[name]);

    private sealed record CapturedRequest(
        string Method,
        string Path,
        string Query,
        byte[]? Body,
        IReadOnlyDictionary<string, string[]> Headers);

    private sealed class RecordingHandler(
        Func<HttpRequestMessage, HttpResponseMessage> responder) : HttpMessageHandler
    {
        public List<CapturedRequest> Requests { get; } = [];

        protected override async Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken)
        {
            Requests.Add(new CapturedRequest(
                request.Method.Method,
                request.RequestUri!.AbsolutePath,
                request.RequestUri.Query,
                request.Content is null
                    ? null
                    : await request.Content.ReadAsByteArrayAsync(cancellationToken),
                request.Headers.ToDictionary(
                    header => header.Key,
                    header => header.Value.ToArray(),
                    StringComparer.OrdinalIgnoreCase)));
            var response = responder(request);
            response.RequestMessage ??= request;
            return response;
        }
    }
}
