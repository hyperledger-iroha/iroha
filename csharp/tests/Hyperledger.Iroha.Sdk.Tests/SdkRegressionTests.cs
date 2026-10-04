using System.Net;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Query;
using Hyperledger.Iroha.Queries;
using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Regression tests for transport, builder and address behavior fixed with the collection API.</summary>
public sealed class SdkRegressionTests
{
    private const string FixtureSeedHex = "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032";
    private const string FixtureNetworkId = "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";
    private const string FixtureAccountId = "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";
    private const string FixtureAssetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";

    [Fact]
    public void BuildingATransactionNeverFreezesTheBuilderCreationTime()
    {
        var builder = new TransactionBuilder(
                NetworkId.Parse(FixtureNetworkId),
                FixtureAccountId,
                FeePaymentIntent.Authority(Array.Empty<FeeChargeLimit>()))
            .TransferAsset(FixtureAssetDefinitionId, "1", FixtureAccountId);

        _ = builder.BuildUnsignedPayload();
        var first = builder.BuildSigned(Convert.FromHexString(FixtureSeedHex));
        Assert.Null(builder.CreationTimeMilliseconds);

        Thread.Sleep(5);
        var second = builder.BuildSigned(Convert.FromHexString(FixtureSeedHex));
        Assert.NotEqual(first.TransactionHashHex, second.TransactionHashHex);

        builder.SetCreationTimeMilliseconds(1_736_000_000_000);
        Assert.Equal(
            builder.BuildSigned(Convert.FromHexString(FixtureSeedHex)).TransactionHashHex,
            builder.BuildSigned(Convert.FromHexString(FixtureSeedHex)).TransactionHashHex);
        Assert.Equal(1_736_000_000_000UL, builder.BuildUnsignedPayload().CreationTimeMilliseconds);
    }

    [Fact]
    public void AccountLiteralsOfAnyNetworkPrefixAreAcceptedWhenCanonical()
    {
        var taira = AccountAddress.Parse(FixtureAccountId).ToI105(TairaTestnetProfile.I105Discriminant);
        Assert.StartsWith("test", taira, StringComparison.Ordinal);

        var query = new SignedIterableQueryBuilder(taira, NetworkId.Parse(FixtureNetworkId)).FindAccounts();
        Assert.Equal(taira, query.AuthorityAccountId);
        var singular = new SignedQueryBuilder(taira, NetworkId.Parse(FixtureNetworkId));
        Assert.Equal(taira, singular.AuthorityAccountId);
        var envelope = query.BuildSigned(Convert.FromHexString(FixtureSeedHex));
        Assert.NotEmpty(envelope.VersionedNoritoBytes);
    }

    [Fact]
    public async Task EventStreamsSendTheFilterTextAndRejectInvalidFiltersBeforeDispatch()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent(": heartbeat\n\n", Encoding.UTF8, "text/event-stream"),
        });
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var hash = new string('c', 64);
        var filter = Filter.Field("tx_hash").Eq(hash) & Filter.Field("tx_status").In("Approved", "Rejected");

        await foreach (var _ in client.StreamEventsAsync(filter, TestContext.Current.CancellationToken))
        {
        }

        Assert.Equal(
            "filter=" + Uri.EscapeDataString($"tx_hash = \"{hash}\" and tx_status in [\"Approved\", \"Rejected\"]"),
            handler.LastRequest!.RequestUri!.Query.TrimStart('?'));

        Filter tooDeep = Filter.Field("tx_status").Eq("Approved");
        for (var level = 0; level <= Filter.MaxDepth; level++)
        {
            tooDeep = !tooDeep;
        }

        handler.LastRequest = null;
        var error = await Assert.ThrowsAsync<ListQueryException>(async () =>
        {
            await foreach (var _ in client.StreamPipelineEventsAsync(tooDeep, TestContext.Current.CancellationToken))
            {
            }
        });
        Assert.Equal("invalid_filter", error.Code);
        Assert.Null(handler.LastRequest);

        var structured = await Assert.ThrowsAsync<ListQueryException>(async () =>
        {
            await foreach (var _ in client.StreamServerSentEventsAsync(
                Filter.Field("metadata.tags").Eq(FilterLiteral.Json(JsonNode.Parse("[1]"))),
                TestContext.Current.CancellationToken))
            {
            }
        });
        Assert.Equal(
            "invalid `filter`: object and array literals exist only in the JSON form, and event-stream filters are text",
            structured.Message);
        Assert.Null(handler.LastRequest);
    }

    [Theory]
    [InlineData("a\nb\r\nc\rd", new[] { "a", "b", "c", "d" })]
    [InlineData("a\r\n\r\nb\n", new[] { "a", "", "b" })]
    [InlineData("\r\n", new[] { "" })]
    [InlineData("", new string[0])]
    [InlineData("é😀\n", new[] { "é😀" })]
    public async Task SseLinesSplitOnEveryTerminator(string input, string[] expected)
    {
        foreach (var chunk in new[] { 1, 2, 3, 1024 })
        {
            using var stream = new ChunkedStream(Encoding.UTF8.GetBytes(input), chunk);
            using var reader = new SseLineReader(stream, 64);
            var lines = new List<string>();
            while (await reader.ReadLineAsync(TestContext.Current.CancellationToken) is { } line)
            {
                lines.Add(line);
            }

            Assert.Equal(expected, lines);
        }
    }

    [Fact]
    public async Task SseLinesAreBoundedAndStrictUtf8()
    {
        using (var stream = new MemoryStream(Encoding.UTF8.GetBytes(new string('x', 65) + "\n")))
        using (var reader = new SseLineReader(stream, 64))
        {
            await Assert.ThrowsAsync<InvalidDataException>(() => reader.ReadLineAsync(CancellationToken.None).AsTask());
        }

        using (var stream = new ChunkedStream(Encoding.UTF8.GetBytes(new string('x', 200)), 7))
        using (var reader = new SseLineReader(stream, 64))
        {
            await Assert.ThrowsAsync<InvalidDataException>(() => reader.ReadLineAsync(CancellationToken.None).AsTask());
        }

        using (var stream = new MemoryStream([0x61, 0xFF, 0x0A]))
        using (var reader = new SseLineReader(stream, 64))
        {
            await Assert.ThrowsAsync<DecoderFallbackException>(() => reader.ReadLineAsync(CancellationToken.None).AsTask());
        }
    }

    [Fact]
    public async Task OversizedServerSentEventLinesFailTheStream()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StreamContent(new ChunkedStream(Encoding.UTF8.GetBytes("data: " + new string('x', 9 * 1024 * 1024)), 64 * 1024)),
        });
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        await Assert.ThrowsAsync<InvalidDataException>(async () =>
        {
            await foreach (var _ in client.StreamServerSentEventsAsync(cancellationToken: TestContext.Current.CancellationToken))
            {
            }
        });
    }

    [Fact]
    public void StreamErrorsShareTheIrohaExceptionBase()
    {
        Assert.True(typeof(IrohaException).IsAssignableFrom(typeof(ToriiStreamException)));
        Assert.True(typeof(IrohaException).IsAssignableFrom(typeof(ToriiApiException)));
        Assert.True(typeof(IrohaException).IsAssignableFrom(typeof(ListQueryException)));
        Assert.True(typeof(IrohaException).IsAssignableFrom(typeof(FilterSyntaxException)));
    }

    private sealed class RecordingHandler(Func<HttpRequestMessage, HttpResponseMessage> responder) : HttpMessageHandler
    {
        public HttpRequestMessage? LastRequest { get; set; }

        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        {
            LastRequest = request;
            var response = responder(request);
            response.RequestMessage ??= request;
            return Task.FromResult(response);
        }
    }

    /// <summary>Returns at most <c>chunk</c> bytes per read to exercise buffer boundaries.</summary>
    private sealed class ChunkedStream(byte[] data, int chunk) : MemoryStream(data, writable: false)
    {
        public override int Read(byte[] buffer, int offset, int count) => base.Read(buffer, offset, Math.Min(count, chunk));

        public override int Read(Span<byte> buffer) => base.Read(buffer[..Math.Min(buffer.Length, chunk)]);

        public override ValueTask<int> ReadAsync(Memory<byte> buffer, CancellationToken cancellationToken = default) =>
            base.ReadAsync(buffer[..Math.Min(buffer.Length, chunk)], cancellationToken);

        public override Task<int> ReadAsync(byte[] buffer, int offset, int count, CancellationToken cancellationToken) =>
            base.ReadAsync(buffer, offset, Math.Min(count, chunk), cancellationToken);
    }
}
