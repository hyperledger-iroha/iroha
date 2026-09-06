using System.Net;
using System.Text;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Locks the generic KAGEMUSHA V1 routes and result-verification boundary.</summary>
public sealed class ToriiKagemushaClientV1Tests
{
    [Fact]
    public async Task ReadinessPreservesAValidUnavailableState()
    {
        using var handler = new CapturingHandler(_ => JsonResponse(
            """{"kagemusha_handoff_capability":"kagemusha_handoff_v1","wire_version":1,"device_lifecycle_version":1,"ready":false}"""));
        using var client = new ToriiClient(
            new Uri("https://torii.example/api/"),
            new HttpClient(handler));

        var readiness = await client.GetKagemushaReadinessAsync(
            TestContext.Current.CancellationToken);

        Assert.Equal("/api/v1/kagemusha/readiness", handler.Request!.RequestUri!.AbsolutePath);
        Assert.False(readiness.Ready);
    }

    [Fact]
    public async Task OperationPollingWithholdsAppliedResultUntilPinnedVerification()
    {
        var operationId = Enumerable.Repeat((byte)0xd1, 32).ToArray();
        var idArray = string.Join(',', operationId.Select(value => value.ToString()));
        var response = $$"""{"version":1,"operation_id":[{{idArray}}],"kind":{"kind":"redemption","value":null},"state":{"state":"applied","value":null},"result":{"untrusted_finality":"opaque"},"rejection":null}""";
        using var handler = new CapturingHandler(_ => JsonResponse(response));
        using var client = new ToriiClient(
            new Uri("https://torii.example/api/"),
            new HttpClient(handler));

        var status = await client.GetKagemushaOperationAsync(
            operationId,
            TestContext.Current.CancellationToken);

        Assert.Equal(
            "/api/v1/kagemusha/operations/" + Convert.ToHexString(operationId).ToLowerInvariant(),
            handler.Request!.RequestUri!.AbsolutePath);
        Assert.Equal("applied", status.State);
        Assert.DoesNotContain("opaque", status.ToString(), StringComparison.Ordinal);
        var released = status.VerifyAgainst("pinned-anchor", (json, anchor) =>
        {
            Assert.Equal("pinned-anchor", anchor);
            return json.GetProperty("result").GetProperty("untrusted_finality").GetString();
        });
        Assert.Equal("opaque", released);
    }

    [Fact]
    public async Task TopUpSubmissionPreservesSignedBytesAndValidatesPendingHeaders()
    {
        var operationId = Enumerable.Repeat((byte)0xd2, 32).ToArray();
        var operationIdHex = Convert.ToHexString(operationId).ToLowerInvariant();
        var idArray = string.Join(',', operationId.Select(value => value.ToString()));
        var body = $$"""{"version":1,"operation_id":[{{idArray}}],"kind":{"kind":"top_up","value":null},"state":{"state":"pending","value":null},"result":null,"rejection":null}""";
        var envelope = SignedEnvelopeFixture();
        using var handler = new CapturingHandler(request =>
        {
            Assert.Equal(envelope.VersionedNoritoBytes, request.Content!.ReadAsByteArrayAsync().Result);
            Assert.Equal(operationIdHex, request.Headers.GetValues("Idempotency-Key").Single());
            var response = JsonResponse(body, HttpStatusCode.Accepted);
            response.Headers.TryAddWithoutValidation(
                "Location",
                $"/v1/kagemusha/operations/{operationIdHex}");
            response.Headers.TryAddWithoutValidation("Retry-After", "1");
            return response;
        });
        using var client = new ToriiClient(
            new Uri("https://torii.example/api/"),
            new HttpClient(handler));

        var status = await client.SubmitKagemushaTopUpAsync(
            envelope,
            operationId,
            TestContext.Current.CancellationToken);

        Assert.Equal("pending", status.State);
        Assert.Equal("/api/v1/kagemusha/top-up", handler.Request!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task TopUpSubmissionRejectsPendingStatusReturnedAsTerminalReplay()
    {
        var operationId = Enumerable.Repeat((byte)0xd3, 32).ToArray();
        var operationIdHex = Convert.ToHexString(operationId).ToLowerInvariant();
        var idArray = string.Join(',', operationId.Select(value => value.ToString()));
        var body = $$"""{"version":1,"operation_id":[{{idArray}}],"kind":{"kind":"top_up","value":null},"state":{"state":"pending","value":null},"result":null,"rejection":null}""";
        using var handler = new CapturingHandler(_ =>
        {
            var response = JsonResponse(body);
            response.Headers.TryAddWithoutValidation(
                "Location",
                $"/v1/kagemusha/operations/{operationIdHex}");
            return response;
        });
        using var client = new ToriiClient(
            new Uri("https://torii.example/api/"),
            new HttpClient(handler));

        await Assert.ThrowsAsync<InvalidDataException>(() =>
            client.SubmitKagemushaTopUpAsync(
                SignedEnvelopeFixture(),
                operationId,
                TestContext.Current.CancellationToken));
    }

    [Fact]
    public async Task TopUpSubmissionAcceptsTerminalReplayWithoutRetryAfter()
    {
        var operationId = Enumerable.Repeat((byte)0xd4, 32).ToArray();
        var operationIdHex = Convert.ToHexString(operationId).ToLowerInvariant();
        using var handler = new CapturingHandler(_ =>
        {
            var response = JsonResponse(RejectedBody(operationId));
            response.Headers.TryAddWithoutValidation(
                "Location",
                $"/v1/kagemusha/operations/{operationIdHex}");
            return response;
        });
        using var client = new ToriiClient(
            new Uri("https://torii.example/api/"),
            new HttpClient(handler));

        var status = await client.SubmitKagemushaTopUpAsync(
            SignedEnvelopeFixture(),
            operationId,
            TestContext.Current.CancellationToken);

        Assert.Equal("rejected", status.State);
    }

    [Fact]
    public async Task TopUpSubmissionRejectsRetryAfterOnTerminalReplay()
    {
        var operationId = Enumerable.Repeat((byte)0xd5, 32).ToArray();
        var operationIdHex = Convert.ToHexString(operationId).ToLowerInvariant();
        using var handler = new CapturingHandler(_ =>
        {
            var response = JsonResponse(RejectedBody(operationId));
            response.Headers.TryAddWithoutValidation(
                "Location",
                $"/v1/kagemusha/operations/{operationIdHex}");
            response.Headers.TryAddWithoutValidation("Retry-After", "1");
            return response;
        });
        using var client = new ToriiClient(
            new Uri("https://torii.example/api/"),
            new HttpClient(handler));

        await Assert.ThrowsAsync<InvalidDataException>(() =>
            client.SubmitKagemushaTopUpAsync(
                SignedEnvelopeFixture(),
                operationId,
                TestContext.Current.CancellationToken));
    }

    private static HttpResponseMessage JsonResponse(
        string json,
        HttpStatusCode status = HttpStatusCode.OK) => new(status)
        {
            Content = new StringContent(json, Encoding.UTF8, "application/json"),
        };

    private static string RejectedBody(byte[] operationId)
    {
        var idArray = string.Join(',', operationId.Select(value => value.ToString()));
        var digestArray = string.Join(',', Enumerable.Repeat("49", 32));
        return $$$"""{"version":1,"operation_id":[{{{idArray}}}],"kind":{"kind":"top_up","value":null},"state":{"state":"rejected","value":null},"result":null,"rejection":{"code":{"code":"invalid_request","value":null},"detail_digest":[{{{digestArray}}}]}}""";
    }

    private static SignedTransactionEnvelope SignedEnvelopeFixture()
    {
        var payload = new byte[] { 0x07 };
        var signature = new CanonicalNoritoWriter();
        signature.WriteSequenceLength(64);
        for (var index = 0; index < 64; index++)
            signature.WriteField([(byte)(index + 1)]);
        var transactionSignature = new CanonicalNoritoWriter();
        transactionSignature.WriteField(signature.ToArray());
        var signed = new CanonicalNoritoWriter();
        signed.WriteField(transactionSignature.ToArray());
        signed.WriteField(payload);
        signed.WriteField([0]);
        var signedBytes = signed.ToArray();
        var versioned = new byte[signedBytes.Length + 1];
        versioned[0] = 1;
        signedBytes.CopyTo(versioned.AsSpan(1));
        var entrypoint = new CanonicalNoritoWriter();
        entrypoint.WriteUInt32LittleEndian(0);
        entrypoint.WriteField(payload);
        return new SignedTransactionEnvelope(
            versioned,
            signedBytes,
            payload,
            IrohaHash.Hash(entrypoint.ToArray()));
    }

    private sealed class CapturingHandler(Func<HttpRequestMessage, HttpResponseMessage> responder)
        : HttpMessageHandler
    {
        public HttpRequestMessage? Request { get; private set; }

        protected override Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken)
        {
            Request = request;
            var response = responder(request);
            response.RequestMessage = request;
            return Task.FromResult(response);
        }
    }
}
