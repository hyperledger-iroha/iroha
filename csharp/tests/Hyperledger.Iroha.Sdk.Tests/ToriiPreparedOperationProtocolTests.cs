using System.Buffers.Binary;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Checks canonical signed operation bindings, exact metadata, and durable versus active verification.</summary>
public sealed partial class ToriiClientTests
{
    [Fact]
    public void PreparedProtocolReceiptBodyMatchesSharedCanonicalHashAndSignature()
    {
        var receipt = PreparedOnboardingReceipt();
        var encoded = PreparedOnboardingBodyEncoder(receipt.Body);
        var hash = IrohaHash.Hash([.. Encoding.UTF8.GetBytes("iroha:account-onboarding-plan-receipt:v1\0"), .. encoded]);
        Assert.Equal(receipt.PlanHash.Substring(5, 64).ToLowerInvariant(), LowerHex(hash));
        Assert.True(Ed25519Signer.Verify(hash, Convert.FromHexString(receipt.Signature),
            PreparedFixturePublicKey(PreparedTransactionSignatureVector("onboarding_prepared"))));
        var substituted = receipt.Body with
        {
            Request = receipt.Body.Request with { Permissions = [.. receipt.Body.Request.Permissions, "CanManagePeers"] },
        };
        var substitutedBytes = PreparedOnboardingBodyEncoder(substituted);
        Assert.NotEqual(encoded, substitutedBytes);
        Assert.NotEqual(hash, IrohaHash.Hash([.. Encoding.UTF8.GetBytes("iroha:account-onboarding-plan-receipt:v1\0"), .. substitutedBytes]));
    }

    [Theory]
    [InlineData("authorization_sha256")]
    [InlineData("authorization_nonce")]
    [InlineData("phase")]
    [InlineData("idempotency_key")]
    public void PreparedOperationBindingRejectsEveryRetiredResetField(string field)
    {
        var node = PreparedTransactionSignatureVector("faucet_prepared")
            .GetProperty("response").GetProperty("binding");
        var binding = JsonNode.Parse(node.GetRawText())!.AsObject();
        binding[field] = "obsolete";
        Assert.Throws<JsonException>(() => binding.Deserialize<ToriiPreparedOperationBindingV1>());
    }

    [Theory]
    [InlineData("schema")]
    [InlineData("semantic_hash_hex")]
    [InlineData("kind")]
    [InlineData("request_id")]
    [InlineData("execution_expires_at_unix_ms")]
    public void PreparedOperationBindingRequiresEveryCanonicalField(string field)
    {
        var node = PreparedTransactionSignatureVector("faucet_prepared")
            .GetProperty("response").GetProperty("binding");
        var binding = JsonNode.Parse(node.GetRawText())!.AsObject();
        Assert.True(binding.Remove(field));
        Assert.Throws<JsonException>(() => binding.Deserialize<ToriiPreparedOperationBindingV1>());
    }

    [Theory]
    [InlineData("onboarding_prepared")]
    [InlineData("onboarding_proof_required")]
    [InlineData("faucet_prepared")]
    public void PreparedProtocolSignatureUsesExactSharedPublicKeyAndTranscript(string name)
    {
        var vector = PreparedTransactionSignatureVector(name);
        var response = vector.GetProperty("response");
        var transcript = PreparedFixtureTranscript(name, JsonNode.Parse(response.GetRawText())!.AsObject());
        Assert.Equal(vector.GetProperty("transcript_hex").GetString(), LowerHex(transcript));
        Assert.Equal(vector.GetProperty("digest_hex").GetString(), LowerHex(IrohaHash.Hash(transcript)));
        var publicKey = PreparedFixturePublicKey(vector);
        var seed = PreparedFixtureSeed(name);
        Assert.Equal(publicKey, Ed25519Signer.GetPublicKey(seed));
        Assert.Equal(vector.GetProperty("server_signature_hex").GetString(),
            LowerHex(Ed25519Signer.Sign(IrohaHash.Hash(transcript), seed)));
        ToriiPreparedTransactionSignatureV1.Verify(transcript,
            response.GetProperty("server_signature").GetString()!, publicKey, name);
    }

    [Theory]
    [InlineData("onboarding_prepared")]
    [InlineData("faucet_prepared")]
    public void PreparedProtocolResigningPreservesExactSharedWireAndMetadata(string name)
    {
        var original = PreparedTransactionSignatureVector(name).GetProperty("response");
        var rebuilt = ResignPreparedFixture(name);
        foreach (var field in new[] { "signed_transaction_wire_hex", "signed_transaction_wire_sha256", "transaction_hash_hex", "server_signature" })
        {
            Assert.Equal(original.GetProperty(field).GetString(), rebuilt[field]!.GetValue<string>());
        }
    }

    [Theory]
    [InlineData("onboarding_prepared")]
    [InlineData("faucet_prepared")]
    public void PreparedProtocolMetadataMatchesSharedCanonicalBinding(string name)
    {
        var original = PreparedTransactionSignatureVector(name).GetProperty("response");
        var fields = PreparedFixturePayloadFields(original);
        var metadata = ReadPreparedFixtureMetadata(fields[8]);
        Assert.Equal(fields[8], EncodePreparedFixtureMetadata(metadata));
        Assert.Equal(name == "faucet_prepared"
                ? new[] { "prepared_operation", "prepared_operation_binding", "prepared_semantic_hash", "taira_faucet_claim_marker_version" }
                : new[] { "prepared_operation", "prepared_operation_binding", "prepared_semantic_hash" },
            metadata.Keys.Order(StringComparer.Ordinal));
        Assert.True(JsonNode.DeepEquals(JsonNode.Parse(original.GetProperty("binding").GetRawText()),
            metadata["prepared_operation_binding"]));
        if (name == "faucet_prepared")
        {
            Assert.Equal(1UL, metadata["taira_faucet_claim_marker_version"]!.GetValue<ulong>());
            var claim = original.GetProperty("claim");
            Assert.Equal(original.GetProperty("semantic_hash_hex").GetString(), PreparedFaucetClaimHash(
                claim.GetProperty("account_id").GetString()!, claim.GetProperty("pow_anchor_height").GetUInt64(),
                claim.GetProperty("pow_nonce_hex").GetString()!));
        }
    }

    [Theory]
    [InlineData("missing")]
    [InlineData("wrong_number")]
    [InlineData("string")]
    [InlineData("boolean")]
    [InlineData("extra")]
    [InlineData("retired")]
    public async Task PreparedProtocolFaucetRejectsAuthenticatedMetadataMutationBeforeDispatch(string mutation)
    {
        var vector = PreparedTransactionSignatureVector("faucet_prepared");
        var original = DeserializePreparedFixture<ToriiAccountFaucetPreparedTransactionV1>(vector.GetProperty("response"));
        var node = ResignPreparedFixture("faucet_prepared", mutateMetadata: metadata =>
        {
            switch (mutation)
            {
                case "missing": Assert.True(metadata.Remove("taira_faucet_claim_marker_version")); break;
                case "wrong_number": metadata["taira_faucet_claim_marker_version"] = JsonValue.Create(2UL); break;
                case "string": metadata["taira_faucet_claim_marker_version"] = JsonValue.Create("1"); break;
                case "boolean": metadata["taira_faucet_claim_marker_version"] = JsonValue.Create(true); break;
                case "extra": metadata["unapproved"] = JsonValue.Create(1); break;
                case "retired":
                    foreach (var key in new[] { "prepared_operation_binding", "prepared_operation", "prepared_semantic_hash" })
                    {
                        Assert.True(metadata.Remove(key, out var value));
                        metadata[key == "prepared_operation_binding" ? "taira_public_reset_binding" : $"taira_{key}"] = value;
                    }
                    break;
                default: throw new ArgumentOutOfRangeException(nameof(mutation));
            }
        });
        var prepared = node.Deserialize<ToriiAccountFaucetPreparedTransactionV1>()!;
        AssertPreparedFixtureEnvelopeSignature("faucet_prepared", node);
        var policy = FaucetPolicy(vector, original);
        var network = PreparedTransactionSignatureNetworkId(vector);
        var error = Assert.Throws<JsonException>(() => ToriiClient.VerifyAccountFaucetPreparedTransactionV1(
            prepared, original.Claim, original.Binding, original.FeePayment, policy, network));
        Assert.Contains("metadata differs", error.Message, StringComparison.Ordinal);
        using var handler = new RecordingHandler(_ => throw new InvalidOperationException("invalid metadata reached HTTP"));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        await Assert.ThrowsAsync<JsonException>(() => client.SubmitPreparedAccountFaucetAsync(
            prepared, original.FeePayment, policy, network, TestContext.Current.CancellationToken));
        Assert.Null(handler.LastRequest);
    }

    [Theory]
    [InlineData("onboarding_prepared")]
    [InlineData("faucet_prepared")]
    public async Task PreparedProtocolExpiredSignedEnvelopeVerifiesDurablyAndNeverDispatches(string name)
    {
        var vector = PreparedTransactionSignatureVector(name);
        var binding = DeserializePreparedFixture<ToriiPreparedOperationBindingV1>(
            vector.GetProperty("response").GetProperty("binding")) with { ExecutionExpiresAtUnixMilliseconds = 1 };
        var node = ResignPreparedFixture(name, binding, creation: 0, ttl: PreparedOptionalU64(1));
        AssertPreparedFixtureEnvelopeSignature(name, node);
        using var handler = new RecordingHandler(_ => throw new InvalidOperationException("expired envelope reached HTTP"));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var network = PreparedTransactionSignatureNetworkId(vector);
        if (name == "onboarding_prepared")
        {
            var prepared = node.Deserialize<ToriiAccountOnboardingPreparedTransactionV1>()!;
            var receipt = PreparedOnboardingReceipt();
            var authority = vector.GetProperty("signer_account_id").GetString()!;
            ToriiClient.VerifyAccountOnboardingPreparedTransactionV1(prepared, receipt.Body.Request,
                receipt, binding, PreparedAccountFeePayment, authority, network, PreparedOnboardingBodyEncoder);
            var error = await Assert.ThrowsAsync<ArgumentException>(() => client.SubmitPreparedAccountOnboardingAsync(
                receipt.Body.Request, prepared, PreparedAccountFeePayment, AccountOnboardingToken,
                authority, network, PreparedOnboardingBodyEncoder, TestContext.Current.CancellationToken));
            Assert.Contains("expired", error.Message, StringComparison.Ordinal);
        }
        else
        {
            var original = DeserializePreparedFixture<ToriiAccountFaucetPreparedTransactionV1>(vector.GetProperty("response"));
            var prepared = node.Deserialize<ToriiAccountFaucetPreparedTransactionV1>()!;
            var policy = FaucetPolicy(vector, original);
            ToriiClient.VerifyAccountFaucetPreparedTransactionV1(prepared, original.Claim, binding,
                original.FeePayment, policy, network);
            var error = await Assert.ThrowsAsync<ArgumentException>(() => client.SubmitPreparedAccountFaucetAsync(
                prepared, original.FeePayment, policy, network, TestContext.Current.CancellationToken));
            Assert.Contains("expired", error.Message, StringComparison.Ordinal);
        }
        Assert.Null(handler.LastRequest);
    }

    [Theory]
    [InlineData("missing")]
    [InlineData("zero")]
    [InlineData("outlives")]
    [InlineData("overflow")]
    public void PreparedProtocolRejectsAuthenticatedUnboundedLifetime(string mutation)
    {
        var vector = PreparedTransactionSignatureVector("faucet_prepared");
        var original = DeserializePreparedFixture<ToriiAccountFaucetPreparedTransactionV1>(vector.GetProperty("response"));
        var ttl = mutation == "missing" ? new byte[] { 0 } : PreparedOptionalU64(mutation == "zero" ? 0UL : 2UL);
        var creation = mutation == "overflow" ? ulong.MaxValue : original.Binding.ExecutionExpiresAtUnixMilliseconds;
        var node = ResignPreparedFixture("faucet_prepared", creation: creation, ttl: ttl);
        AssertPreparedFixtureEnvelopeSignature("faucet_prepared", node);
        var prepared = node.Deserialize<ToriiAccountFaucetPreparedTransactionV1>()!;
        var error = Assert.Throws<JsonException>(() => ToriiClient.VerifyAccountFaucetPreparedTransactionV1(
            prepared, original.Claim, original.Binding, original.FeePayment,
            FaucetPolicy(vector, original), PreparedTransactionSignatureNetworkId(vector)));
        Assert.Contains(mutation is "missing" or "zero" ? "positive signed transaction TTL" : "outlives", error.Message, StringComparison.Ordinal);
    }

    [Theory]
    [InlineData(1UL, false)]
    [InlineData(ulong.MaxValue, true)]
    public async Task PreparedProtocolDispatchChecksDeadlineAfterRequestPreparation(ulong deadline, bool active)
    {
        var preparations = 0;
        var dispatches = 0;
        using var handler = new RecordingHandler(_ =>
        {
            dispatches++;
            return new HttpResponseMessage(System.Net.HttpStatusCode.OK);
        });
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        async Task Dispatch()
        {
            using var response = await client.SendAsync(HttpMethod.Post, "/v1/accounts/faucet",
                configureRequest: _ => preparations++,
                cancellationToken: TestContext.Current.CancellationToken,
                preparedExecutionDeadlineUnixMilliseconds: deadline);
        }
        if (active) await Dispatch();
        else
        {
            var error = await Assert.ThrowsAsync<ArgumentException>(Dispatch);
            Assert.Contains("expired", error.Message, StringComparison.Ordinal);
        }
        Assert.Equal(1, preparations);
        Assert.Equal(active ? 1 : 0, dispatches);
    }

    [Fact]
    public async Task PreparedProtocolUncertainSubmissionIsNotReplayed()
    {
        var dispatches = 0;
        using var handler = new RecordingHandler(_ =>
        {
            dispatches++;
            throw new HttpRequestException("submission outcome unknown");
        });
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        await Assert.ThrowsAsync<HttpRequestException>(() => client.SendAsync(HttpMethod.Post,
            "/v1/accounts/faucet", cancellationToken: TestContext.Current.CancellationToken,
            preparedExecutionDeadlineUnixMilliseconds: ulong.MaxValue));
        Assert.Equal(1, dispatches);
    }

    private static JsonObject ResignPreparedFixture(string name,
        ToriiPreparedOperationBindingV1? binding = null,
        Action<Dictionary<string, JsonNode?>>? mutateMetadata = null,
        ulong? creation = null, byte[]? ttl = null)
    {
        var vector = PreparedTransactionSignatureVector(name);
        var original = vector.GetProperty("response");
        var result = JsonNode.Parse(original.GetRawText())!.AsObject();
        var fields = PreparedFixturePayloadFields(original);
        var metadata = ReadPreparedFixtureMetadata(fields[8]);
        if (binding is not null)
        {
            result["binding"] = JsonSerializer.SerializeToNode(binding);
            metadata["prepared_operation_binding"] = JsonSerializer.SerializeToNode(binding);
        }
        mutateMetadata?.Invoke(metadata);
        fields[8] = EncodePreparedFixtureMetadata(metadata);
        if (creation is { } time) fields[2] = PreparedU64(time);
        if (ttl is not null) fields[4] = ttl;
        var payloadWriter = new CanonicalNoritoWriter();
        foreach (var field in fields) payloadWriter.WriteField(field);
        var payload = payloadWriter.ToArray();
        var seed = PreparedFixtureSeed(name);
        Assert.Equal(PreparedFixturePublicKey(vector), Ed25519Signer.GetPublicKey(seed));
        var signatureBytes = Ed25519Signer.Sign(IrohaHash.Hash(payload), seed);
        Assert.True(Ed25519Signer.Verify(IrohaHash.Hash(payload), signatureBytes, PreparedFixturePublicKey(vector)));
        var signatureVector = new CanonicalNoritoWriter();
        signatureVector.WriteSequenceLength((ulong)signatureBytes.Length);
        signatureVector.WriteByteElements(signatureBytes);
        var signature = new CanonicalNoritoWriter();
        signature.WriteField(signatureVector.ToArray());
        var wire = new CanonicalNoritoWriter();
        wire.WriteByte(1);
        wire.WriteField(signature.ToArray());
        wire.WriteField(payload);
        wire.WriteField([0]);
        var wireBytes = wire.ToArray();
        var entrypoint = new CanonicalNoritoWriter();
        entrypoint.WriteUInt32LittleEndian(0);
        entrypoint.WriteField(payload);
        result["signed_transaction_wire_hex"] = LowerHex(wireBytes);
        result["signed_transaction_wire_sha256"] = LowerHex(SHA256.HashData(wireBytes));
        result["transaction_hash_hex"] = LowerHex(IrohaHash.Hash(entrypoint.ToArray()));
        result["server_signature"] = Convert.ToHexString(Ed25519Signer.Sign(
            IrohaHash.Hash(PreparedFixtureTranscript(name, result)), seed));
        return result;
    }

    private static byte[][] PreparedFixturePayloadFields(JsonElement response)
    {
        var wire = Convert.FromHexString(response.GetProperty("signed_transaction_wire_hex").GetString()!);
        Assert.Equal(1, wire[0]);
        var reader = new CanonicalNoritoReader(wire.AsSpan(1), "fixture transaction", nameof(response));
        _ = reader.ReadField("signature");
        var payload = reader.ReadField("payload");
        Assert.Equal(new byte[] { 0 }, reader.ReadField("multisig").ToArray());
        reader.RequireEnd();
        var payloadReader = new CanonicalNoritoReader(payload, "fixture payload", nameof(response));
        var fields = new byte[10][];
        for (var index = 0; index < fields.Length; index++) fields[index] = payloadReader.ReadField($"field[{index}]").ToArray();
        payloadReader.RequireEnd();
        return fields;
    }

    private static Dictionary<string, JsonNode?> ReadPreparedFixtureMetadata(byte[] bytes)
    {
        var reader = new CanonicalNoritoReader(bytes, "fixture metadata", nameof(bytes));
        var count = reader.ReadSequenceLength("count");
        Assert.InRange(count, 3UL, 4UL);
        var values = new Dictionary<string, JsonNode?>(StringComparer.Ordinal);
        for (ulong index = 0; index < count; index++)
        {
            var entry = new CanonicalNoritoReader(reader.ReadField("entry"), "metadata entry", nameof(bytes));
            var key = ReadPreparedFixtureString(entry.ReadField("key"));
            var json = new CanonicalNoritoReader(entry.ReadField("value"), "metadata JSON", nameof(bytes));
            var value = ReadPreparedFixtureString(json.ReadField("json"));
            json.RequireEnd();
            entry.RequireEnd();
            Assert.True(values.TryAdd(key, JsonNode.Parse(value)));
        }
        reader.RequireEnd();
        return values;
    }

    private static byte[] EncodePreparedFixtureMetadata(Dictionary<string, JsonNode?> values)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteSequenceLength((ulong)values.Count);
        foreach (var (key, value) in values.OrderBy(entry => entry.Key, StringComparer.Ordinal))
        {
            var entry = new CanonicalNoritoWriter();
            entry.WriteField(PreparedString(key));
            var json = new CanonicalNoritoWriter();
            json.WriteField(PreparedString(TransactionEncodingContext.CanonicalJson(value)));
            entry.WriteField(json.ToArray());
            writer.WriteField(entry.ToArray());
        }
        return writer.ToArray();
    }

    private static string ReadPreparedFixtureString(ReadOnlySpan<byte> bytes)
    {
        var reader = new CanonicalNoritoReader(bytes, "fixture string", nameof(bytes));
        var length = reader.ReadCompactLength("length");
        var value = Encoding.UTF8.GetString(reader.ReadExact(checked((int)length), "value"));
        reader.RequireEnd();
        return value;
    }

    private static byte[] PreparedString(string value)
    {
        var bytes = Encoding.UTF8.GetBytes(value);
        var writer = new CanonicalNoritoWriter();
        writer.WriteCompactLength((ulong)bytes.Length);
        writer.WriteBytes(bytes);
        return writer.ToArray();
    }

    private static byte[] PreparedU64(ulong value)
    {
        var bytes = new byte[sizeof(ulong)];
        BinaryPrimitives.WriteUInt64LittleEndian(bytes, value);
        return bytes;
    }

    private static byte[] PreparedOptionalU64(ulong value)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteByte(1);
        writer.WriteField(PreparedU64(value));
        return writer.ToArray();
    }

    private static string PreparedFaucetClaimHash(string accountId, ulong powAnchorHeight, string powNonceHex)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteField(PreparedString(accountId));
        writer.WriteField(PreparedU64(powAnchorHeight));
        writer.WriteField(PreparedString(powNonceHex));
        return LowerHex(IrohaHash.Hash([.. Encoding.UTF8.GetBytes("iroha:accounts:faucet:claim:v1\0"), .. writer.ToArray()]));
    }

    private static byte[] PreparedFixtureSeed(string name) => Enumerable.Repeat(
        name == "faucet_prepared" ? (byte)0x61 : (byte)0x51, Ed25519Signer.PrivateKeySeedLength).ToArray();

    private static byte[] PreparedFixturePublicKey(JsonElement vector)
    {
        var multihash = vector.GetProperty("signer_public_key").GetString()!;
        Assert.StartsWith("ed0120", multihash, StringComparison.OrdinalIgnoreCase);
        return Convert.FromHexString(multihash[6..]);
    }

    private static string LowerHex(byte[] bytes) => Convert.ToHexString(bytes).ToLowerInvariant();

    private static void AssertPreparedFixtureEnvelopeSignature(string name, JsonObject response) =>
        ToriiPreparedTransactionSignatureV1.Verify(PreparedFixtureTranscript(name, response),
            response["server_signature"]!.GetValue<string>(),
            PreparedFixturePublicKey(PreparedTransactionSignatureVector(name)), name);

    private static byte[] PreparedFixtureTranscript(string name, JsonObject response) => name switch
    {
        "onboarding_prepared" => ToriiPreparedTransactionSignatureV1.OnboardingPreparedTranscript(
            response.Deserialize<ToriiAccountOnboardingPreparedTransactionV1>()!,
            Convert.FromHexString(response["signed_transaction_wire_hex"]!.GetValue<string>())),
        "onboarding_proof_required" => ToriiPreparedTransactionSignatureV1.OnboardingProofRequiredTranscript(
            response.Deserialize<ToriiAccountOnboardingProofRequiredPrepareResponseV1>()!),
        "faucet_prepared" => ToriiPreparedTransactionSignatureV1.FaucetPreparedTranscript(
            response.Deserialize<ToriiAccountFaucetPreparedTransactionV1>()!,
            Convert.FromHexString(response["signed_transaction_wire_hex"]!.GetValue<string>())),
        _ => throw new ArgumentOutOfRangeException(nameof(name)),
    };
}
