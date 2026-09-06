using System.Net;
using Hyperledger.Iroha;
using Hyperledger.Iroha.Queries;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed partial class ToriiClientTests
{
    [Fact]
    public async Task SubmitSignedQueryAsyncAcceptsManagedEnvelope()
    {
        SignedQueryEnvelope? seenEnvelope = null;
        using var handler = new RecordingHandler(request =>
        {
            Assert.Equal(HttpMethod.Post, request.Method);
            Assert.Equal("/v1/query", request.RequestUri!.AbsolutePath);
            Assert.Equal("limit=1", request.RequestUri.Query.TrimStart('?'));
            Assert.Equal("application/x-norito", request.Content!.Headers.ContentType!.MediaType);

            using var stream = request.Content.ReadAsStream();
            using var buffer = new MemoryStream();
            stream.CopyTo(buffer);
            Assert.Equal(seenEnvelope!.VersionedNoritoBytes, buffer.ToArray());

            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                    {
                      "kind": "Singular",
                      "value": {
                        "kind": "FindParameters"
                      }
                    }
                    """),
            };
        });

        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler),
            options: null,
            TransactionSubmissionTransportAssurance.OneShotWithoutRedirectsOrRetries);
        seenEnvelope = new SignedQueryBuilder(
            CanonicalAccountId,
            NetworkId.Parse(CanonicalNetworkId))
            .FindParameters()
            .BuildSigned(Convert.FromHexString("616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032"));

        using var response = await client.SubmitSignedQueryAsync(seenEnvelope, query: "limit=1", cancellationToken: TestContext.Current.CancellationToken);

        Assert.Equal("Singular", response.RootElement.GetProperty("kind").GetString());
        Assert.Equal("FindParameters", response.RootElement.GetProperty("value").GetProperty("kind").GetString());
    }
}
