using System.Net;
using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class ToriiIdentifierReceiptTests
{
    [Fact]
    public async Task ResolveIdentifierAsyncRejectsPaddedPolicyIdBeforePost()
    {
        var called = false;
        using var handler = new RecordingHandler(_ =>
        {
            called = true;
            return new HttpResponseMessage(HttpStatusCode.OK);
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var error = await Assert.ThrowsAsync<ArgumentException>(() => client.ResolveIdentifierAsync(
            new ToriiIdentifierResolveRequest
            {
                PolicyId = " phone#retail ",
                Input = "+15551234567",
            }));

        Assert.Contains("PolicyId", error.Message);
        Assert.Contains("surrounding whitespace", error.Message);
        Assert.False(called);
    }

    [Theory]
    [InlineData("policy_id", " phone#retail ")]
    [InlineData("opaque_id", " opaque-1 ")]
    [InlineData("receipt_hash", " receipt-1 ")]
    [InlineData("uaid", " uaid:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef ")]
    [InlineData("account_id", " sorauﾛ1Nmerchant ")]
    [InlineData("backend", " bfv-programmed-sha3-256-v1 ")]
    [InlineData("signature", " ABCD ")]
    [InlineData("signature_payload_hex", " DEADBEEF ")]
    public void IdentifierResolveResponseRejectsPaddedReceiptFields(string field, string value)
    {
        var receipt = ValidIdentifierResolveResponse();
        receipt[field] = value;

        var error = Assert.Throws<JsonException>(
            () => JsonSerializer.Deserialize<ToriiIdentifierResolveResponse>(receipt.ToJsonString()));

        Assert.Contains(field, error.Message);
        Assert.Contains("surrounding whitespace", error.Message);
    }

    [Theory]
    [InlineData("resolved_at_ms")]
    [InlineData("expires_at_ms")]
    public void IdentifierResolveResponseRejectsNegativeReceiptTimes(string field)
    {
        var receipt = ValidIdentifierResolveResponse();
        receipt[field] = -1;

        var error = Assert.Throws<JsonException>(
            () => JsonSerializer.Deserialize<ToriiIdentifierResolveResponse>(receipt.ToJsonString()));

        Assert.Contains(field, error.Message);
        Assert.Contains("non-negative", error.Message);
    }

    [Fact]
    public void IdentifierResolveResponseDeserializesNestedReceiptEnvelope()
    {
        var receipt = JsonSerializer.Deserialize<ToriiIdentifierResolveResponse>(
            NestedIdentifierResolveResponse("""{"kind":"signed","signature":"ABCD"}"""));

        Assert.NotNull(receipt);
        Assert.Equal("phone#retail", receipt.PolicyId);
        Assert.Equal("opaque-1", receipt.OpaqueId);
        Assert.Equal("receipt-1", receipt.ReceiptHash);
        Assert.Equal("sorauﾛ1Nmerchant", receipt.AccountId);
        Assert.Equal(1710000000000L, receipt.ResolvedAtMilliseconds);
        Assert.Equal(1710003600000L, receipt.ExpiresAtMilliseconds);
        Assert.Equal("bfv-programmed-sha3-256-v1", receipt.Backend);
        Assert.Equal("ABCD", receipt.Signature);
        Assert.Equal(string.Empty, receipt.SignaturePayloadHex);
        Assert.Equal("signed", receipt.SignaturePayload!["attestation"]!["kind"]!.GetValue<string>());
    }

    [Theory]
    [InlineData("policy_id", " phone#retail ")]
    [InlineData("owner", " sorauﾛ1Nissuer ")]
    [InlineData("normalization", " phone_e164 ")]
    [InlineData("resolver_public_key", " ed25519:0123456789abcdef ")]
    [InlineData("backend", " bfv-programmed-sha3-256-v1 ")]
    [InlineData("input_encryption", " bfv-v1 ")]
    [InlineData("input_encryption_public_parameters", " params-b64 ")]
    public void IdentifierPolicySummaryRejectsPaddedProofMetadata(string field, string value)
    {
        var policy = ValidIdentifierPolicySummary();
        policy[field] = value;

        var error = Assert.Throws<JsonException>(
            () => JsonSerializer.Deserialize<ToriiIdentifierPolicySummary>(policy.ToJsonString()));

        Assert.Contains(field, error.Message);
        Assert.Contains("surrounding whitespace", error.Message);
    }

    private static JsonObject ValidIdentifierResolveResponse()
    {
        return new JsonObject
        {
            ["policy_id"] = "phone#retail",
            ["opaque_id"] = "opaque-1",
            ["receipt_hash"] = "receipt-1",
            ["uaid"] = "uaid:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            ["account_id"] = "sorauﾛ1Nmerchant",
            ["resolved_at_ms"] = 1710000000000L,
            ["expires_at_ms"] = 1710003600000L,
            ["backend"] = "bfv-programmed-sha3-256-v1",
            ["signature"] = "ABCD",
            ["signature_payload_hex"] = "DEADBEEF",
            ["signature_payload"] = new JsonObject
            {
                ["policy_id"] = "phone#retail",
                ["account_id"] = "sorauﾛ1Nmerchant",
            },
        };
    }

    private static JsonObject ValidIdentifierPolicySummary()
    {
        return new JsonObject
        {
            ["policy_id"] = "phone#retail",
            ["owner"] = "sorauﾛ1Nissuer",
            ["active"] = true,
            ["normalization"] = "phone_e164",
            ["resolver_public_key"] = "ed25519:0123456789abcdef",
            ["backend"] = "bfv-programmed-sha3-256-v1",
            ["input_encryption"] = "bfv-v1",
            ["input_encryption_public_parameters"] = "params-b64",
            ["input_encryption_public_parameters_decoded"] = null,
            ["ram_fhe_profile"] = null,
            ["note"] = "retail policy",
        };
    }

    private static string NestedIdentifierResolveResponse(string attestationJson)
    {
        return $$"""
            {
              "payload": {
                "policy_id": "phone#retail",
                "account_id": "sorauﾛ1Nmerchant",
                "opaque_id": "opaque-1",
                "receipt_hash": "receipt-1",
                "uaid": "uaid:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "execution": {
                  "program_id": "identifier_lookup_retail",
                  "program_digest": "program-digest",
                  "backend": "bfv-programmed-sha3-256-v1",
                  "verification_mode": "signed",
                  "input_ciphertext_hash": "input-hash",
                  "output_ciphertext_hash": "output-hash",
                  "parameter_digest": "parameter-digest",
                  "evaluation_key_digest": "evaluation-key-digest",
                  "output_hash": "output-open-hash",
                  "associated_data_hash": "associated-data-hash",
                  "executed_at_ms": 1710000000000,
                  "expires_at_ms": 1710003600000
                },
                "opening": {
                  "payload": {
                    "program_id": "identifier_lookup_retail",
                    "input_ciphertext_hash": "input-hash",
                    "output_ciphertext_hash": "output-hash",
                    "parameter_digest": "parameter-digest",
                    "evaluation_key_digest": "evaluation-key-digest",
                    "opened_output_hash": "opened-output-hash",
                    "opened_at_ms": 1710000000000,
                    "expires_at_ms": 1710003600000
                  },
                  "signature": "ABCD"
                }
              },
              "attestation": {{attestationJson}}
            }
            """;
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
