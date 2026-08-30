using System.Net;
using System.Text;
using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed partial class ToriiClientTests
{
    [Fact]
    public void PublicHeaderBuilderEnforcesPreparedWireQueryByteLimit()
    {
        var rawQuery = "x=" + new string('é', 32_767);
        Assert.Equal(CanonicalRequest.MaxRawQueryBytesV1, Encoding.UTF8.GetByteCount(rawQuery));
        _ = CanonicalRequest.BuildCanonicalQueryString(rawQuery);

        Assert.Throws<ArgumentException>(() => CanonicalRequest.BuildHeaders(
            OnboardingFixtureNetworkId,
            CanonicalAccountId,
            CanonicalPrivateKeySeed,
            "GET",
            "/v1/test",
            query: rawQuery,
            timestampMs: 1,
            nonce: "prepared-query-cap"));
    }

    [Fact]
    public void PublicHeaderBuilderSignsTheHttpRequestMessageWirePath()
    {
        const string callerPath = "/v1/%2e%2Fasset/%252e";
        using var request = new HttpRequestMessage(
            HttpMethod.Get,
            new Uri(new Uri("https://torii.example"), callerPath));
        var wirePath = request.RequestUri!.AbsolutePath;

        Assert.Equal("/v1/.%2Fasset/%252e", wirePath);
        Assert.NotEqual(
            CanonicalRequest.BuildMessage("GET", wirePath),
            CanonicalRequest.BuildMessage("GET", callerPath));

        var callerHeaders = CanonicalRequest.BuildHeaders(
            OnboardingFixtureNetworkId,
            CanonicalAccountId,
            CanonicalPrivateKeySeed,
            "GET",
            callerPath,
            timestampMs: 1,
            nonce: "caller-path");
        var wireHeaders = CanonicalRequest.BuildHeaders(
            OnboardingFixtureNetworkId,
            CanonicalAccountId,
            CanonicalPrivateKeySeed,
            "GET",
            wirePath,
            timestampMs: 1,
            nonce: "caller-path");

        Assert.Equal(wireHeaders.SignatureBase64, callerHeaders.SignatureBase64);
    }

    [Theory]
    [InlineData("/v1/a b")]
    [InlineData("/v1/a<b")]
    [InlineData("/v1/a>b")]
    [InlineData("/v1/a[b")]
    [InlineData("/v1/a]b")]
    [InlineData("/v1/a^b")]
    [InlineData("/v1/a`b")]
    [InlineData("/v1/a{b")]
    [InlineData("/v1/a|b")]
    [InlineData("/v1/a}b")]
    public void BuildMessageRejectsPathsHttpClientWouldPercentEncode(string path)
    {
        var error = Assert.Throws<ArgumentException>(() =>
            CanonicalRequest.BuildMessage(HttpMethod.Get.Method, path));

        Assert.Equal("path", error.ParamName);
    }

    [Theory]
    [InlineData("/v1/a b")]
    [InlineData("/v1/a<b")]
    [InlineData("/v1/a>b")]
    [InlineData("/v1/a[b")]
    [InlineData("/v1/a]b")]
    [InlineData("/v1/a^b")]
    [InlineData("/v1/a`b")]
    [InlineData("/v1/a{b")]
    [InlineData("/v1/a|b")]
    [InlineData("/v1/a}b")]
    public async Task SendAsyncRejectsPathsHttpClientWouldPercentEncodeBeforeDispatch(string path)
    {
        using var handler = new RecordingHandler(_ =>
            throw new InvalidOperationException("inexact Torii path reached HTTP dispatch"));
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler),
            new ToriiClientOptions
            {
                NetworkId = OnboardingFixtureNetworkId,
                CanonicalRequestCredentials = new CanonicalRequestCredentials(
                    CanonicalAccountId,
                    CanonicalPrivateKeySeed),
            },
            TransactionSubmissionTransportAssurance.OneShotWithoutRedirectsOrRetries);

        var error = await Assert.ThrowsAsync<ArgumentException>(() => client.SendAsync(
            HttpMethod.Get,
            path,
            cancellationToken: TestContext.Current.CancellationToken));

        Assert.Equal("path", error.ParamName);
        Assert.Null(handler.LastRequest);
    }

    [Theory]
    [InlineData("/v1/query:http")]
    [InlineData("/v1/%FF")]
    [InlineData("/v1/%00")]
    [InlineData("/v1/!$&'()*+,-.;=@A_Z~")]
    public async Task SendAsyncPreservesExactAsciiWirePaths(string path)
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("{\"ok\":true}"),
        });
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler),
            new ToriiClientOptions
            {
                NetworkId = OnboardingFixtureNetworkId,
                CanonicalRequestCredentials = new CanonicalRequestCredentials(
                    CanonicalAccountId,
                    CanonicalPrivateKeySeed),
            },
            TransactionSubmissionTransportAssurance.OneShotWithoutRedirectsOrRetries);

        using var response = await client.SendAsync(
            HttpMethod.Get,
            path,
            cancellationToken: TestContext.Current.CancellationToken);

        Assert.Equal(path, handler.LastRequest!.RequestUri!.AbsolutePath);
        Assert.True(handler.LastRequest.Headers.Contains("X-Iroha-Signature"));
    }
}
