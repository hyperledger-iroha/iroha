using System.Globalization;
using System.Net;
using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Numeric;
using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Exercises the first-release prepared onboarding and faucet protocol.</summary>
public sealed partial class ToriiClientTests
{
    private static FeePaymentIntent PreparedAccountFeePayment =>
        FeePaymentIntent.Authority([]);

    [Theory]
    [InlineData("onboarding_prepared")]
    [InlineData("onboarding_proof_required")]
    [InlineData("faucet_prepared")]
    public void PreparedTransactionSignatureV1MatchesSharedRustGolden(string name)
    {
        var vector = PreparedTransactionSignatureVector(name);
        var response = vector.GetProperty("response");
        byte[] transcript;
        string signature;
        switch (name)
        {
            case "onboarding_prepared":
            {
                var prepared = DeserializePreparedFixture<ToriiAccountOnboardingPreparedTransactionV1>(response);
                var wire = Convert.FromHexString(prepared.SignedTransactionWireHex);
                transcript = ToriiPreparedTransactionSignatureV1.OnboardingPreparedTranscript(
                    prepared,
                    wire);
                signature = prepared.ServerSignature;
                break;
            }
            case "onboarding_proof_required":
            {
                var proofRequired = DeserializePreparedFixture<ToriiAccountOnboardingProofRequiredPrepareResponseV1>(response);
                transcript = ToriiPreparedTransactionSignatureV1.OnboardingProofRequiredTranscript(
                    proofRequired);
                signature = proofRequired.ServerSignature;
                break;
            }
            case "faucet_prepared":
            {
                var prepared = DeserializePreparedFixture<ToriiAccountFaucetPreparedTransactionV1>(response);
                var wire = Convert.FromHexString(prepared.SignedTransactionWireHex);
                transcript = ToriiPreparedTransactionSignatureV1.FaucetPreparedTranscript(
                    prepared,
                    wire);
                signature = prepared.ServerSignature;
                break;
            }
            default:
                throw new ArgumentOutOfRangeException(nameof(name), name, "Unknown prepared fixture vector.");
        }

        Assert.Equal(
            vector.GetProperty("transcript_hex").GetString(),
            Convert.ToHexString(transcript).ToLowerInvariant());
        Assert.Equal(
            vector.GetProperty("digest_hex").GetString(),
            Convert.ToHexString(IrohaHash.Hash(transcript)).ToLowerInvariant());
        Assert.Equal(
            vector.GetProperty("server_signature_hex").GetString(),
            signature.ToLowerInvariant());

        var signer = AccountAddress.Parse(
            vector.GetProperty("signer_account_id").GetString()
            ?? throw new InvalidOperationException("Prepared fixture signer account is missing."));
        ToriiPreparedTransactionSignatureV1.Verify(
            transcript,
            signature,
            signer.PublicKey,
            $"prepared fixture {name}");
    }

    [Fact]
    public void PreparedTransactionSignatureV1RejectsEveryAuthenticatedFieldMutation()
    {
        var vector = PreparedTransactionSignatureVector("onboarding_proof_required");
        var proofRequired = DeserializePreparedFixture<ToriiAccountOnboardingProofRequiredPrepareResponseV1>(
            vector.GetProperty("response"));
        var signer = AccountAddress.Parse(
            vector.GetProperty("signer_account_id").GetString()
            ?? throw new InvalidOperationException("Prepared fixture signer account is missing."));
        var mutations = new ToriiAccountOnboardingProofRequiredPrepareResponseV1[]
        {
            proofRequired with { Schema = $"{proofRequired.Schema}.tampered" },
            proofRequired with { Operation = "faucet" },
            proofRequired with { Outcome = "Applied" },
            proofRequired with { ProofKind = "receipt_only" },
            proofRequired with { SemanticHashHex = new string('a', 64) },
            proofRequired with { AccountId = OnboardingFixtureAuthority },
            proofRequired with { Alias = $"x{proofRequired.Alias}" },
            proofRequired with
            {
                Disposition = new ToriiAccountOnboardingDisposition { Kind = "repair" },
            },
            proofRequired with
            {
                Binding = proofRequired.Binding with
                {
                    Schema = $"{proofRequired.Binding.Schema}.tampered",
                },
            },
            proofRequired with
            {
                Binding = proofRequired.Binding with
                {
                    AuthorizationSha256 = new string('a', 64),
                },
            },
            proofRequired with
            {
                Binding = proofRequired.Binding with
                {
                    AuthorizationNonce = new string('z', 32),
                },
            },
            proofRequired with
            {
                Binding = proofRequired.Binding with { Kind = "faucet" },
            },
            proofRequired with
            {
                Binding = proofRequired.Binding with { Phase = "tampered" },
            },
            proofRequired with
            {
                Binding = proofRequired.Binding with
                {
                    IdempotencyKey = new string('b', 64),
                },
            },
            proofRequired with
            {
                Binding = proofRequired.Binding with
                {
                    ExecutionExpiresAtUnixMilliseconds = checked(
                        proofRequired.Binding.ExecutionExpiresAtUnixMilliseconds + 1),
                },
            },
        };

        foreach (var mutation in mutations)
        {
            var transcript = ToriiPreparedTransactionSignatureV1.OnboardingProofRequiredTranscript(
                mutation);
            Assert.Throws<JsonException>(() => ToriiPreparedTransactionSignatureV1.Verify(
                transcript,
                proofRequired.ServerSignature,
                signer.PublicKey,
                "tampered proof-required fixture"));
        }

        var originalTranscript = ToriiPreparedTransactionSignatureV1.OnboardingProofRequiredTranscript(
            proofRequired);
        Assert.Throws<JsonException>(() => ToriiPreparedTransactionSignatureV1.Verify(
            originalTranscript,
            proofRequired.ServerSignature.ToLowerInvariant(),
            signer.PublicKey,
            "lowercase signature fixture"));
    }

    [Fact]
    public void ProofRequiredEnvelopeRejectsMissingProofKind()
    {
        var response = PreparedTransactionSignatureVector("onboarding_proof_required")
            .GetProperty("response");
        var node = JsonNode.Parse(response.GetRawText())?.AsObject()
            ?? throw new InvalidOperationException("Proof-required fixture must be an object.");
        Assert.True(node.Remove("proof_kind"));

        Assert.Throws<JsonException>(() =>
            node.Deserialize<ToriiAccountOnboardingProofRequiredPrepareResponseV1>());
    }

    [Fact]
    public async Task PrepareAccountOnboardingAsyncRejectsUnsignedDispositionPayload()
    {
        var vector = PreparedTransactionSignatureVector("onboarding_proof_required");
        var response = vector.GetProperty("response");
        var node = JsonNode.Parse(response.GetRawText())?.AsObject()
            ?? throw new InvalidOperationException("Proof-required fixture must be an object.");
        node["disposition"]!["value"] = "substituted";
        var proofRequired = DeserializePreparedFixture<ToriiAccountOnboardingProofRequiredPrepareResponseV1>(
            response);
        var receipt = SharedOnboardingReceipt();
        using var handler = new RecordingHandler(_ =>
            JsonResponse(node.ToJsonString(), HttpStatusCode.OK));
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        await Assert.ThrowsAsync<JsonException>(() => client.PrepareAccountOnboardingAsync(
            receipt.Body.Request,
            receipt,
            proofRequired.Binding,
            PreparedAccountFeePayment,
            AccountOnboardingToken,
            vector.GetProperty("signer_account_id").GetString()!,
            PreparedTransactionSignatureNetworkId(vector),
            SharedOnboardingBodyEncoder,
            TestContext.Current.CancellationToken));
    }

    [Fact]
    public void ReceiptTrustPinUsesAccountPayloadIdentityAcrossI105Displays()
    {
        var receipt = SharedOnboardingReceipt();
        var authority = AccountAddress.Parse(receipt.Body.Authority);
        var alternate = authority.ToI105(AccountAddress.DevChainDiscriminant);
        if (string.Equals(alternate, receipt.Body.Authority, StringComparison.Ordinal))
        {
            alternate = authority.ToI105(AccountAddress.DefaultChainDiscriminant);
        }
        Assert.NotEqual(receipt.Body.Authority, alternate);

        Assert.Same(receipt, ToriiAccountOnboardingReceiptVerifier.RequirePinned(
            receipt,
            alternate,
            SharedOnboardingReceiptNetworkId,
            SharedOnboardingBodyEncoder));
    }

    [Fact]
    public async Task PrepareAccountOnboardingAsyncAcceptsAuthenticatedPreparedGolden()
    {
        var vector = PreparedTransactionSignatureVector("onboarding_prepared");
        var response = vector.GetProperty("response");
        var expected = DeserializePreparedFixture<ToriiAccountOnboardingPreparedTransactionV1>(
            response);
        using var handler = new RecordingHandler(_ =>
            JsonResponse(response.GetRawText(), HttpStatusCode.OK));
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        var result = await client.PrepareAccountOnboardingAsync(
            expected.Receipt.Body.Request,
            expected.Receipt,
            expected.Binding,
            expected.FeePayment,
            AccountOnboardingToken,
            vector.GetProperty("signer_account_id").GetString()!,
            PreparedTransactionSignatureNetworkId(vector),
            SharedOnboardingBodyEncoder,
            TestContext.Current.CancellationToken);

        Assert.Null(result.ProofRequired);
        Assert.Equal(expected.TransactionHashHex, Assert.IsType<ToriiAccountOnboardingPreparedTransactionV1>(
            result.Prepared).TransactionHashHex);
    }

    [Fact]
    public async Task PrepareAccountOnboardingAsyncReturnsAuthenticatedNonterminalProofRequired()
    {
        var vector = PreparedTransactionSignatureVector("onboarding_proof_required");
        var response = vector.GetProperty("response");
        var expected = DeserializePreparedFixture<ToriiAccountOnboardingProofRequiredPrepareResponseV1>(
            response);
        var receipt = SharedOnboardingReceipt();
        using var handler = new RecordingHandler(_ =>
            JsonResponse(response.GetRawText(), HttpStatusCode.OK));
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        var result = await client.PrepareAccountOnboardingAsync(
            receipt.Body.Request,
            receipt,
            expected.Binding,
            PreparedAccountFeePayment,
            AccountOnboardingToken,
            vector.GetProperty("signer_account_id").GetString()!,
            PreparedTransactionSignatureNetworkId(vector),
            SharedOnboardingBodyEncoder,
            TestContext.Current.CancellationToken);

        Assert.Null(result.Prepared);
        var proofRequired = Assert.IsType<ToriiAccountOnboardingProofRequiredPrepareResponseV1>(
            result.ProofRequired);
        Assert.Equal(ToriiAccountOnboardingProofRequiredPrepareResponseV1.OutcomeV1,
            proofRequired.Outcome);
        Assert.Equal(ToriiAccountOnboardingProofRequiredPrepareResponseV1.ProofKindV1,
            proofRequired.ProofKind);
    }

    [Fact]
    public async Task ProveAccountOnboardingCurrentStateAsyncUsesOneExactAtomicPost()
    {
        var vector = PreparedTransactionSignatureVector("onboarding_proof_required");
        var proofRequired = DeserializePreparedFixture<ToriiAccountOnboardingProofRequiredPrepareResponseV1>(
            vector.GetProperty("response"));
        var expectedNetworkId = PreparedTransactionSignatureNetworkId(vector);
        var account = AccountAddress.Parse(proofRequired.AccountId);
        var alternateAccountId = account.ToI105(AccountAddress.DevChainDiscriminant);
        if (string.Equals(alternateAccountId, proofRequired.AccountId, StringComparison.Ordinal))
        {
            alternateAccountId = account.ToI105(AccountAddress.DefaultChainDiscriminant);
        }
        Assert.NotEqual(proofRequired.AccountId, alternateAccountId);
        var requests = new List<string>();
        using var handler = new RecordingHandler(request =>
        {
            requests.Add($"{request.Method} {request.RequestUri!.AbsolutePath}");
            Assert.Equal(HttpMethod.Post, request.Method);
            Assert.Equal(
                "/v1/accounts/onboarding/current-state",
                request.RequestUri.AbsolutePath);
            var requestBodyBytes = request.Content!.ReadAsByteArrayAsync()
                .GetAwaiter()
                .GetResult();
            using var requestBody = JsonDocument.Parse(requestBodyBytes);
            Assert.Equal(
                ["version", "account_id", "alias"],
                requestBody.RootElement
                    .EnumerateObject()
                    .Select(static property => property.Name));
            Assert.Equal(1, requestBody.RootElement.GetProperty("version").GetByte());
            Assert.Equal(
                proofRequired.AccountId,
                requestBody.RootElement.GetProperty("account_id").GetString());
            Assert.Equal(
                proofRequired.Alias,
                requestBody.RootElement.GetProperty("alias").GetString());

            Assert.Equal(
                AccountAddress.Parse(
                    CanonicalAccountId,
                    AccountAddress.DefaultChainDiscriminant).CanonicalHex,
                Assert.Single(request.Headers.GetValues("X-Iroha-Account")));
            var timestamp = long.Parse(
                Assert.Single(request.Headers.GetValues("X-Iroha-Timestamp-Ms")),
                NumberStyles.None,
                CultureInfo.InvariantCulture);
            var nonce = Assert.Single(request.Headers.GetValues("X-Iroha-Nonce"));
            var signature = Convert.FromBase64String(
                Assert.Single(request.Headers.GetValues("X-Iroha-Signature")));
            var publicKey = Ed25519Signer.GetPublicKey(CanonicalPrivateKeySeed);
            var signatureMessage = CanonicalRequest.BuildSignatureMessage(
                expectedNetworkId,
                request.Method.Method,
                request.RequestUri.AbsolutePath,
                request.RequestUri.Query,
                requestBodyBytes,
                timestamp,
                nonce);
            Assert.True(Ed25519Signer.Verify(signatureMessage, signature, publicKey));
            Assert.False(Ed25519Signer.Verify(
                CanonicalRequest.BuildSignatureMessage(
                    expectedNetworkId,
                    HttpMethod.Get.Method,
                    request.RequestUri.AbsolutePath,
                    request.RequestUri.Query,
                    requestBodyBytes,
                    timestamp,
                    nonce),
                signature,
                publicKey));
            Assert.False(Ed25519Signer.Verify(
                CanonicalRequest.BuildSignatureMessage(
                    expectedNetworkId,
                    request.Method.Method,
                    request.RequestUri.AbsolutePath + "/substitution",
                    request.RequestUri.Query,
                    requestBodyBytes,
                    timestamp,
                    nonce),
                signature,
                publicKey));
            var substitutedBody = (byte[])requestBodyBytes.Clone();
            substitutedBody[^1] ^= 1;
            Assert.False(Ed25519Signer.Verify(
                CanonicalRequest.BuildSignatureMessage(
                    expectedNetworkId,
                    request.Method.Method,
                    request.RequestUri.AbsolutePath,
                    request.RequestUri.Query,
                    substitutedBody,
                    timestamp,
                    nonce),
                signature,
                publicKey));
            return JsonResponse(
                AtomicOnboardingStateResponse(
                    proofRequired,
                    expectedNetworkId,
                    alternateAccountId).ToJsonString(),
                HttpStatusCode.OK);
        });
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler),
            new ToriiClientOptions
            {
                CanonicalRequestCredentials = new CanonicalRequestCredentials(
                    CanonicalAccountId,
                    CanonicalPrivateKeySeed),
                LocalSigningContext = new ToriiLocalSigningContext(expectedNetworkId),
            },
            TransactionSubmissionTransportAssurance.OneShotWithoutRedirectsOrRetries);

        var proof = await client.ProveAccountOnboardingCurrentStateAsync(
            SharedOnboardingReceipt().Body.Request,
            proofRequired,
            SharedOnboardingReceipt(),
            proofRequired.Binding,
            vector.GetProperty("signer_account_id").GetString()!,
            expectedNetworkId,
            SharedOnboardingBodyEncoder,
            TestContext.Current.CancellationToken);

        Assert.Equal(ToriiAccountOnboardingCurrentStateKindV1.Applied, proof.Kind);
        Assert.Equal(alternateAccountId, proof.Observation.AliasTargetAccountId);
        Assert.Equal((ulong)19, proof.Observation.ObservedBlockHeight);
        Assert.Equal(expectedNetworkId.ToString(), proof.Observation.ObservedBlockHash);
        Assert.Equal(
            ["POST /v1/accounts/onboarding/current-state"],
            requests);
    }

    [Fact]
    public async Task GetAccountReadAsyncAcceptsAlternateI105DisplayForSamePayload()
    {
        var account = AccountAddress.Parse(CanonicalAccountId);
        var alternateAccountId = account.ToI105(AccountAddress.DevChainDiscriminant);
        if (string.Equals(alternateAccountId, CanonicalAccountId, StringComparison.Ordinal))
        {
            alternateAccountId = account.ToI105(AccountAddress.DefaultChainDiscriminant);
        }
        Assert.NotEqual(CanonicalAccountId, alternateAccountId);
        string? requestedPath = null;
        using var handler = new RecordingHandler(request =>
        {
            requestedPath = request.RequestUri!.AbsolutePath;
            return JsonResponse(JsonSerializer.Serialize(new ToriiAccountReadResponse
            {
                AccountId = CanonicalAccountId,
                OpaqueIds = Array.Empty<JsonElement>(),
            }), HttpStatusCode.OK);
        });
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        var response = await client.GetAccountReadAsync(
            alternateAccountId,
            TestContext.Current.CancellationToken);

        Assert.Equal(CanonicalAccountId, response.AccountId);
        Assert.Equal(
            alternateAccountId,
            Uri.UnescapeDataString(requestedPath!["/v1/accounts/".Length..]));
    }

    [Fact]
    public async Task ProveAccountOnboardingCurrentStateAsyncClassifiesAbsentAndConflictingAliases()
    {
        var vector = PreparedTransactionSignatureVector("onboarding_proof_required");
        var proofRequired = DeserializePreparedFixture<ToriiAccountOnboardingProofRequiredPrepareResponseV1>(
            vector.GetProperty("response"));
        var expectedNetworkId = PreparedTransactionSignatureNetworkId(vector);
        Assert.False(
            AccountAddress.Parse(proofRequired.AccountId)
                .CanonicalBytes()
                .AsSpan()
                .SequenceEqual(AccountAddress.Parse(OnboardingFixtureAuthority).CanonicalBytes()));

        foreach (var (target, expectedKind) in new[]
        {
            ((string?)null, ToriiAccountOnboardingCurrentStateKindV1.AliasAbsent),
            (OnboardingFixtureAuthority, ToriiAccountOnboardingCurrentStateKindV1.AliasConflict),
        })
        {
            var calls = 0;
            using var handler = new RecordingHandler(request =>
            {
                calls += 1;
                Assert.Equal(HttpMethod.Post, request.Method);
                Assert.Equal(
                    "/v1/accounts/onboarding/current-state",
                    request.RequestUri!.AbsolutePath);
                return JsonResponse(
                    AtomicOnboardingStateResponse(
                        proofRequired,
                        expectedNetworkId,
                        target).ToJsonString(),
                    HttpStatusCode.OK);
            });
            using var client = new ToriiClient(
                new Uri("https://torii.example"),
                new HttpClient(handler));

            var proof = await client.ProveAccountOnboardingCurrentStateAsync(
                SharedOnboardingReceipt().Body.Request,
                proofRequired,
                SharedOnboardingReceipt(),
                proofRequired.Binding,
                vector.GetProperty("signer_account_id").GetString()!,
                expectedNetworkId,
                SharedOnboardingBodyEncoder,
                TestContext.Current.CancellationToken);

            Assert.Equal(expectedKind, proof.Kind);
            Assert.Equal(target, proof.Observation.AliasTargetAccountId);
            Assert.Equal(1, calls);
        }
    }

    [Fact]
    public async Task ProveAccountOnboardingCurrentStateAsyncRejectsOpenSubstitutedOrUnanchoredResponses()
    {
        var vector = PreparedTransactionSignatureVector("onboarding_proof_required");
        var proofRequired = DeserializePreparedFixture<ToriiAccountOnboardingProofRequiredPrepareResponseV1>(
            vector.GetProperty("response"));
        var expectedNetworkId = PreparedTransactionSignatureNetworkId(vector);
        var exact = AtomicOnboardingStateResponse(
            proofRequired,
            expectedNetworkId,
            proofRequired.AccountId);
        var canonicalBlockHash = expectedNetworkId.ToString();
        var badChecksumBlockHash = canonicalBlockHash[..^1]
            + (canonicalBlockHash[^1] == '0' ? "1" : "0");
        var candidates = new List<(JsonObject Body, HttpStatusCode Status)>
        {
            (MutateAtomicState(exact, "version", 2), HttpStatusCode.OK),
            (MutateAtomicState(
                exact,
                "network_id",
                "hash:82531CE8EAE8BFF6BEECA4698BFD13A3BC8BEC5F0EE0D23D428C97FC17AB0F3B#3E94"),
                HttpStatusCode.OK),
            (MutateAtomicState(exact, "account_id", OnboardingFixtureAuthority), HttpStatusCode.OK),
            (MutateAtomicState(exact, "alias", $"other.{proofRequired.Alias}"), HttpStatusCode.OK),
            (MutateAtomicState(exact, "account_exists", false), HttpStatusCode.OK),
            (MutateAtomicState(exact, "observed_block_height", 0), HttpStatusCode.OK),
            (MutateAtomicState(exact, "observed_block_hash", "not-a-typed-hash"), HttpStatusCode.OK),
            (MutateAtomicState(
                exact,
                "observed_block_hash",
                expectedNetworkId.ToString().ToLowerInvariant()), HttpStatusCode.OK),
            (MutateAtomicState(
                exact,
                "observed_block_hash",
                badChecksumBlockHash), HttpStatusCode.OK),
            (MutateAtomicState(
                exact,
                "alias_target_account_id",
                $" {proofRequired.AccountId}"), HttpStatusCode.OK),
            (MutateAtomicState(
                exact,
                "alias_target_account_id",
                proofRequired.Alias), HttpStatusCode.OK),
            (MutateAtomicState(exact, "legacy_account_state", "applied"), HttpStatusCode.OK),
            ((JsonObject)exact.DeepClone(), HttpStatusCode.Accepted),
        };

        foreach (var (body, status) in candidates)
        {
            var calls = 0;
            using var handler = new RecordingHandler(_ =>
            {
                calls += 1;
                return JsonResponse(body.ToJsonString(), status);
            });
            using var client = new ToriiClient(
                new Uri("https://torii.example"),
                new HttpClient(handler));

            await Assert.ThrowsAsync<JsonException>(() => client.ProveAccountOnboardingCurrentStateAsync(
                SharedOnboardingReceipt().Body.Request,
                proofRequired,
                SharedOnboardingReceipt(),
                proofRequired.Binding,
                vector.GetProperty("signer_account_id").GetString()!,
                PreparedTransactionSignatureNetworkId(vector),
                SharedOnboardingBodyEncoder,
                TestContext.Current.CancellationToken));
            Assert.Equal(1, calls);
        }
    }

    [Fact]
    public async Task ProveAccountOnboardingCurrentStateAsyncRejectsResponseOverFourKiBBeforeParsing()
    {
        var vector = PreparedTransactionSignatureVector("onboarding_proof_required");
        var proofRequired = DeserializePreparedFixture<ToriiAccountOnboardingProofRequiredPrepareResponseV1>(
            vector.GetProperty("response"));
        var oversized = AtomicOnboardingStateResponse(
            proofRequired,
            PreparedTransactionSignatureNetworkId(vector),
            proofRequired.AccountId);
        oversized["padding"] = new string('x', 4 * 1024);
        var calls = 0;
        using var handler = new RecordingHandler(_ =>
        {
            calls += 1;
            return JsonResponse(oversized.ToJsonString(), HttpStatusCode.OK);
        });
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        var error = await Assert.ThrowsAsync<JsonException>(() => client.ProveAccountOnboardingCurrentStateAsync(
            SharedOnboardingReceipt().Body.Request,
            proofRequired,
            SharedOnboardingReceipt(),
            proofRequired.Binding,
            vector.GetProperty("signer_account_id").GetString()!,
            PreparedTransactionSignatureNetworkId(vector),
            SharedOnboardingBodyEncoder,
            TestContext.Current.CancellationToken));

        Assert.Contains("4096-byte limit", error.Message, StringComparison.Ordinal);
        Assert.Equal(1, calls);
    }

    [Fact]
    public async Task ProveAccountOnboardingCurrentStateAsyncRejectsDuplicateFields()
    {
        var vector = PreparedTransactionSignatureVector("onboarding_proof_required");
        var proofRequired = DeserializePreparedFixture<ToriiAccountOnboardingProofRequiredPrepareResponseV1>(
            vector.GetProperty("response"));
        var exact = AtomicOnboardingStateResponse(
            proofRequired,
            PreparedTransactionSignatureNetworkId(vector),
            proofRequired.AccountId).ToJsonString();
        var duplicate = $"{exact[..^1]},\"account_exists\":true}}";
        var calls = 0;
        using var handler = new RecordingHandler(_ =>
        {
            calls += 1;
            return JsonResponse(duplicate, HttpStatusCode.OK);
        });
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        var error = await Assert.ThrowsAsync<JsonException>(() => client.ProveAccountOnboardingCurrentStateAsync(
            SharedOnboardingReceipt().Body.Request,
            proofRequired,
            SharedOnboardingReceipt(),
            proofRequired.Binding,
            vector.GetProperty("signer_account_id").GetString()!,
            PreparedTransactionSignatureNetworkId(vector),
            SharedOnboardingBodyEncoder,
            TestContext.Current.CancellationToken));

        Assert.Contains("account_exists must not appear more than once", error.Message, StringComparison.Ordinal);
        Assert.Equal(1, calls);
    }

    [Fact]
    public void AccountFaucetPolicyRequiresCanonicalPositiveTypedValues()
    {
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            new ToriiAccountFaucetPolicyV1(
                OnboardingFixtureAuthority,
                "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                NumericV1.QuantityValue.ParseCanonical("0")));
        Assert.Throws<ArgumentException>(() =>
            new ToriiAccountFaucetPolicyV1(
                OnboardingFixtureAuthority,
                "not-an-asset-definition",
                NumericV1.QuantityValue.ParseCanonical("1")));
    }

    [Fact]
    public async Task PrepareAccountFaucetAsyncAcceptsAuthenticatedPreparedGolden()
    {
        var vector = PreparedTransactionSignatureVector("faucet_prepared");
        var response = vector.GetProperty("response");
        var expected = DeserializePreparedFixture<ToriiAccountFaucetPreparedTransactionV1>(response);
        using var handler = new RecordingHandler(_ =>
            JsonResponse(response.GetRawText(), HttpStatusCode.OK));
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        var prepared = await client.PrepareAccountFaucetAsync(
            expected.Claim,
            expected.Binding,
            expected.FeePayment,
            FaucetPolicy(vector, expected),
            PreparedTransactionSignatureNetworkId(vector),
            TestContext.Current.CancellationToken);

        Assert.Equal(expected.TransactionHashHex, prepared.TransactionHashHex);
    }

    [Fact]
    public async Task PrepareAccountFaucetAsyncRejectsSamePayloadDifferentClaimDisplay()
    {
        var vector = PreparedTransactionSignatureVector("faucet_prepared");
        var expected = DeserializePreparedFixture<ToriiAccountFaucetPreparedTransactionV1>(
            vector.GetProperty("response"));
        var account = AccountAddress.Parse(expected.Claim.AccountId);
        var alternateAccountId = account.ToI105(AccountAddress.DevChainDiscriminant);
        if (string.Equals(alternateAccountId, expected.Claim.AccountId, StringComparison.Ordinal))
        {
            alternateAccountId = account.ToI105(AccountAddress.DefaultChainDiscriminant);
        }
        Assert.NotEqual(expected.Claim.AccountId, alternateAccountId);
        using var handler = new RecordingHandler(_ =>
            JsonResponse(vector.GetProperty("response").GetRawText(), HttpStatusCode.OK));
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        var error = await Assert.ThrowsAsync<JsonException>(() => client.PrepareAccountFaucetAsync(
            expected.Claim with { AccountId = alternateAccountId },
            expected.Binding,
            expected.FeePayment,
            FaucetPolicy(vector, expected),
            PreparedTransactionSignatureNetworkId(vector),
            TestContext.Current.CancellationToken));

        Assert.Contains("exact claim", error.Message, StringComparison.Ordinal);
    }

    [Fact]
    public async Task SubmitPreparedAccountOnboardingAsyncPostsOnlyAuthenticatedGolden()
    {
        var vector = PreparedTransactionSignatureVector("onboarding_prepared");
        var prepared = DeserializePreparedFixture<ToriiAccountOnboardingPreparedTransactionV1>(
            vector.GetProperty("response"));
        var response = JsonSerializer.Serialize(new ToriiPreparedTransactionSubmitResponseV1
        {
            Binding = prepared.Binding,
            Operation = prepared.Operation,
            TransactionHashHex = prepared.TransactionHashHex,
            Outcome = "Pending",
        });
        using var handler = new RecordingHandler(request =>
        {
            using var payload = ReadBodyAsJson(request);
            Assert.Equal("/v1/accounts/onboard", request.RequestUri!.AbsolutePath);
            Assert.Equal(
                prepared.TransactionHashHex,
                payload.RootElement.GetProperty("transaction_hash_hex").GetString());
            Assert.Equal(
                AccountOnboardingToken,
                Assert.Single(request.Headers.GetValues("X-Iroha-Onboarding-Token")));
            Assert.False(payload.RootElement.TryGetProperty("apply", out _));
            return JsonResponse(response, HttpStatusCode.Accepted);
        });
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        var outcome = await client.SubmitPreparedAccountOnboardingAsync(
            prepared.Receipt.Body.Request,
            prepared,
            prepared.FeePayment,
            AccountOnboardingToken,
            vector.GetProperty("signer_account_id").GetString()!,
            PreparedTransactionSignatureNetworkId(vector),
            SharedOnboardingBodyEncoder,
            TestContext.Current.CancellationToken);

        Assert.Equal("Pending", outcome.Outcome);
    }

    [Fact]
    public async Task PreparedSubmitsRejectCallerFeeAndFaucetPolicySubstitutionBeforeDispatch()
    {
        var onboardingVector = PreparedTransactionSignatureVector("onboarding_prepared");
        var onboarding = DeserializePreparedFixture<ToriiAccountOnboardingPreparedTransactionV1>(
            onboardingVector.GetProperty("response"));
        using var onboardingHandler = new RecordingHandler(_ =>
            throw new InvalidOperationException("fee-substituted onboarding reached HTTP dispatch"));
        using var onboardingClient = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(onboardingHandler));

        await Assert.ThrowsAsync<JsonException>(() =>
            onboardingClient.SubmitPreparedAccountOnboardingAsync(
                onboarding.Receipt.Body.Request,
                onboarding,
                FeePaymentIntent.Authority([], gasLimit: 1),
                AccountOnboardingToken,
                onboardingVector.GetProperty("signer_account_id").GetString()!,
                PreparedTransactionSignatureNetworkId(onboardingVector),
                SharedOnboardingBodyEncoder,
                TestContext.Current.CancellationToken));
        Assert.Null(onboardingHandler.LastRequest);

        var faucetVector = PreparedTransactionSignatureVector("faucet_prepared");
        var faucet = DeserializePreparedFixture<ToriiAccountFaucetPreparedTransactionV1>(
            faucetVector.GetProperty("response"));
        using var faucetHandler = new RecordingHandler(_ =>
            throw new InvalidOperationException("fee-substituted faucet reached HTTP dispatch"));
        using var faucetClient = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(faucetHandler));

        await Assert.ThrowsAsync<JsonException>(() =>
            faucetClient.SubmitPreparedAccountFaucetAsync(
                faucet,
                FeePaymentIntent.Authority([], gasLimit: 1),
                FaucetPolicy(faucetVector, faucet),
                PreparedTransactionSignatureNetworkId(faucetVector),
                TestContext.Current.CancellationToken));

        var trustedPolicy = FaucetPolicy(faucetVector, faucet);
        var substitutedPolicies = new ToriiAccountFaucetPolicyV1[]
        {
            new(
                onboardingVector.GetProperty("signer_account_id").GetString()!,
                trustedPolicy.AssetDefinitionId,
                trustedPolicy.Amount),
            new(
                trustedPolicy.FaucetAuthority,
                "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                trustedPolicy.Amount),
            new(
                trustedPolicy.FaucetAuthority,
                trustedPolicy.AssetDefinitionId,
                NumericV1.QuantityValue.ParseCanonical("6")),
        };
        foreach (var substitutedPolicy in substitutedPolicies)
        {
            await Assert.ThrowsAsync<JsonException>(() =>
                faucetClient.SubmitPreparedAccountFaucetAsync(
                    faucet,
                    faucet.FeePayment,
                    substitutedPolicy,
                    PreparedTransactionSignatureNetworkId(faucetVector),
                    TestContext.Current.CancellationToken));
        }
        Assert.Null(faucetHandler.LastRequest);
    }

    [Theory]
    [InlineData(200, "Applied", true)]
    [InlineData(200, "Pending", true)]
    [InlineData(200, "Rejected", true)]
    [InlineData(202, "Pending", true)]
    [InlineData(202, "Applied", false)]
    [InlineData(201, "Pending", false)]
    [InlineData(200, "Absent", false)]
    public async Task SubmitPreparedAccountFaucetAsyncAcceptsOnlyClosedStatusOutcomePairs(
        int statusCode,
        string outcome,
        bool succeeds)
    {
        var vector = PreparedTransactionSignatureVector("faucet_prepared");
        var prepared = DeserializePreparedFixture<ToriiAccountFaucetPreparedTransactionV1>(
            vector.GetProperty("response"));
        var response = JsonSerializer.Serialize(new ToriiPreparedTransactionSubmitResponseV1
        {
            Binding = prepared.Binding,
            Operation = prepared.Operation,
            TransactionHashHex = prepared.TransactionHashHex,
            Outcome = outcome,
        });
        using var handler = new RecordingHandler(request =>
        {
            Assert.Equal("/v1/accounts/faucet", request.RequestUri!.AbsolutePath);
            return JsonResponse(response, (HttpStatusCode)statusCode);
        });
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        var task = client.SubmitPreparedAccountFaucetAsync(
            prepared,
            prepared.FeePayment,
            FaucetPolicy(vector, prepared),
            PreparedTransactionSignatureNetworkId(vector),
            TestContext.Current.CancellationToken);
        if (succeeds)
        {
            Assert.Equal(outcome, (await task).Outcome);
        }
        else
        {
            await Assert.ThrowsAsync<JsonException>(async () => await task);
        }
    }

    [Fact]
    public void PreparedEnvelopeJsonRoundTripPreservesExactAuthenticatedBytes()
    {
        var vector = PreparedTransactionSignatureVector("faucet_prepared");
        var prepared = DeserializePreparedFixture<ToriiAccountFaucetPreparedTransactionV1>(
            vector.GetProperty("response"));
        var serialized = JsonSerializer.Serialize(prepared);
        var restored = JsonSerializer.Deserialize<ToriiAccountFaucetPreparedTransactionV1>(serialized)
            ?? throw new InvalidOperationException("Persisted prepared faucet envelope decoded to null.");

        Assert.Equal(prepared.SignedTransactionWireHex, restored.SignedTransactionWireHex);
        Assert.Equal(prepared.ServerSignature, restored.ServerSignature);
        Assert.Equal(
            vector.GetProperty("transcript_hex").GetString(),
            Convert.ToHexString(ToriiPreparedTransactionSignatureV1.FaucetPreparedTranscript(
                restored,
                Convert.FromHexString(restored.SignedTransactionWireHex))).ToLowerInvariant());
    }

    [Fact]
    public async Task SubmitPreparedAccountFaucetAsyncRejectsTamperingBeforeDispatch()
    {
        var vector = PreparedTransactionSignatureVector("faucet_prepared");
        var prepared = DeserializePreparedFixture<ToriiAccountFaucetPreparedTransactionV1>(
            vector.GetProperty("response"));
        var tamperedWire = $"{(prepared.SignedTransactionWireHex[0] == '0' ? '1' : '0')}" +
            prepared.SignedTransactionWireHex[1..];
        var mutations = new ToriiAccountFaucetPreparedTransactionV1[]
        {
            prepared with { Schema = $"{prepared.Schema}.tampered" },
            prepared with { Operation = "onboarding" },
            prepared with { SemanticHashHex = new string('a', 64) },
            prepared with { TransactionHashHex = new string('a', 64) },
            prepared with { TransactionHashHex = prepared.TransactionHashHex.ToUpperInvariant() },
            prepared with { SignedTransactionWireSha256 = new string('b', 64) },
            prepared with
            {
                SignedTransactionWireSha256 = prepared.SignedTransactionWireSha256.ToUpperInvariant(),
            },
            prepared with { SignedTransactionWireHex = tamperedWire },
            prepared with { SignedTransactionWireHex = prepared.SignedTransactionWireHex.ToUpperInvariant() },
            prepared with
            {
                FeePayment = FeePaymentIntent.Authority([], gasLimit: 1),
            },
            prepared with
            {
                ServerSignature = new string('A', Ed25519Signer.SignatureLength * 2),
            },
            prepared with
            {
                Binding = prepared.Binding with { Phase = "tampered" },
            },
        };

        foreach (var mutation in mutations)
        {
            using var handler = new RecordingHandler(_ =>
                throw new InvalidOperationException("tampered envelope reached HTTP dispatch"));
            using var client = new ToriiClient(
                new Uri("https://torii.example"),
                new HttpClient(handler));

            await Assert.ThrowsAnyAsync<Exception>(() => client.SubmitPreparedAccountFaucetAsync(
                mutation,
                prepared.FeePayment,
                FaucetPolicy(vector, prepared),
                PreparedTransactionSignatureNetworkId(vector),
                TestContext.Current.CancellationToken));
            Assert.Null(handler.LastRequest);
        }
    }

    [Fact]
    public async Task PrepareAccountOnboardingAsyncPostsOnlyClosedPrepareRequest()
    {
        var receipt = SharedOnboardingReceipt();
        var binding = ValidPreparedMutationBinding(
            ToriiAccountOnboardingPreparedTransactionV1.OperationV1);
        using var handler = new RecordingHandler(request =>
        {
            using var payload = ReadBodyAsJson(request);
            Assert.Equal(HttpMethod.Post, request.Method);
            Assert.Equal("/v1/accounts/onboard/prepare", request.RequestUri!.AbsolutePath);
            Assert.Equal(
                ["schema", "binding", "receipt", "fee_payment"],
                payload.RootElement.EnumerateObject().Select(static property => property.Name));
            Assert.Equal(
                ToriiAccountOnboardingPrepareRequestV1.SchemaV1,
                payload.RootElement.GetProperty("schema").GetString());
            Assert.Equal(
                binding.IdempotencyKey,
                payload.RootElement.GetProperty("binding").GetProperty("idempotency_key").GetString());
            Assert.Equal(
                receipt.PlanHash,
                payload.RootElement.GetProperty("receipt").GetProperty("plan_hash").GetString());
            Assert.Equal(
                "authority",
                payload.RootElement.GetProperty("fee_payment").GetProperty("payer").GetString());
            Assert.False(payload.RootElement.TryGetProperty("transaction", out _));
            Assert.False(payload.RootElement.TryGetProperty("apply", out _));
            return JsonResponse("{\"schema\":\"unsupported\"}", HttpStatusCode.OK);
        });
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        await Assert.ThrowsAsync<JsonException>(() => client.PrepareAccountOnboardingAsync(
            receipt.Body.Request,
            receipt,
            binding,
            PreparedAccountFeePayment,
            AccountOnboardingToken,
            OnboardingFixtureAuthority,
            SharedOnboardingReceiptNetworkId,
            SharedOnboardingBodyEncoder,
            cancellationToken: TestContext.Current.CancellationToken));
    }

    [Fact]
    public async Task PrepareAccountOnboardingAsyncRejectsRetiredTerminalNoOpSchema()
    {
        var response = PreparedTransactionSignatureVector("onboarding_proof_required")
            .GetProperty("response");
        var node = JsonNode.Parse(response.GetRawText())?.AsObject()
            ?? throw new InvalidOperationException("Proof-required fixture must be an object.");
        node["schema"] = "iroha.accounts.onboard.prepare-unchanged.v1";
        using var handler = new RecordingHandler(_ =>
            JsonResponse(node.ToJsonString(), HttpStatusCode.OK));
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        await Assert.ThrowsAsync<JsonException>(() => client.PrepareAccountOnboardingAsync(
            SharedOnboardingReceipt().Body.Request,
            SharedOnboardingReceipt(),
            DeserializePreparedFixture<ToriiAccountOnboardingProofRequiredPrepareResponseV1>(
                response).Binding,
            PreparedAccountFeePayment,
            AccountOnboardingToken,
            PreparedTransactionSignatureVector("onboarding_proof_required")
                .GetProperty("signer_account_id")
                .GetString()!,
            PreparedTransactionSignatureNetworkId(
                PreparedTransactionSignatureVector("onboarding_proof_required")),
            SharedOnboardingBodyEncoder,
            TestContext.Current.CancellationToken));
    }

    [Fact]
    public async Task PrepareAccountOnboardingAsyncRejectsPermissionSubstitutionBeforeDispatch()
    {
        var receipt = SharedOnboardingReceipt();
        var substituted = receipt with
        {
            Body = receipt.Body with
            {
                Request = receipt.Body.Request with
                {
                    Permissions = receipt.Body.Request.Permissions
                        .Append("CanManagePeers")
                        .OrderBy(static permission => permission, StringComparer.Ordinal)
                        .ToArray(),
                },
            },
        };
        using var handler = new RecordingHandler(_ =>
            throw new InvalidOperationException("substituted permissions reached HTTP dispatch"));
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        await Assert.ThrowsAsync<JsonException>(() => client.PrepareAccountOnboardingAsync(
            receipt.Body.Request,
            substituted,
            ValidPreparedMutationBinding(ToriiAccountOnboardingPreparedTransactionV1.OperationV1),
            PreparedAccountFeePayment,
            AccountOnboardingToken,
            OnboardingFixtureAuthority,
            SharedOnboardingReceiptNetworkId,
            SharedOnboardingBodyEncoder,
            TestContext.Current.CancellationToken));
        Assert.Null(handler.LastRequest);
    }

    [Fact]
    public async Task PrepareAccountOnboardingAsyncRejectsAccountDisplaySubstitutionBeforeDispatch()
    {
        var receipt = SharedOnboardingReceipt();
        var account = AccountAddress.Parse(receipt.Body.Request.AccountId);
        var alternateAccountId = account.ToI105(AccountAddress.DevChainDiscriminant);
        if (string.Equals(alternateAccountId, receipt.Body.Request.AccountId, StringComparison.Ordinal))
        {
            alternateAccountId = account.ToI105(AccountAddress.DefaultChainDiscriminant);
        }
        Assert.NotEqual(receipt.Body.Request.AccountId, alternateAccountId);
        using var handler = new RecordingHandler(_ =>
            throw new InvalidOperationException("substituted account display reached HTTP dispatch"));
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        await Assert.ThrowsAsync<JsonException>(() => client.PrepareAccountOnboardingAsync(
            receipt.Body.Request with { AccountId = alternateAccountId },
            receipt,
            ValidPreparedMutationBinding(ToriiAccountOnboardingPreparedTransactionV1.OperationV1),
            PreparedAccountFeePayment,
            AccountOnboardingToken,
            OnboardingFixtureAuthority,
            SharedOnboardingReceiptNetworkId,
            SharedOnboardingBodyEncoder,
            TestContext.Current.CancellationToken));

        Assert.Null(handler.LastRequest);
    }

    [Fact]
    public async Task PrepareAccountFaucetAsyncPostsOnlyClosedPrepareRequest()
    {
        var binding = ValidPreparedMutationBinding(
            ToriiAccountFaucetPreparedTransactionV1.OperationV1);
        using var handler = new RecordingHandler(request =>
        {
            using var payload = ReadBodyAsJson(request);
            Assert.Equal(HttpMethod.Post, request.Method);
            Assert.Equal("/v1/accounts/faucet/prepare", request.RequestUri!.AbsolutePath);
            Assert.Equal(
                ["schema", "binding", "claim", "fee_payment"],
                payload.RootElement.EnumerateObject().Select(static property => property.Name));
            Assert.Equal(
                ToriiAccountFaucetPrepareRequestV1.SchemaV1,
                payload.RootElement.GetProperty("schema").GetString());
            var claim = payload.RootElement.GetProperty("claim");
            Assert.Equal(CanonicalAccountId, claim.GetProperty("account_id").GetString());
            Assert.Equal<ulong>(68, claim.GetProperty("pow_anchor_height").GetUInt64());
            Assert.Equal("00", claim.GetProperty("pow_nonce_hex").GetString());
            Assert.Equal(
                "authority",
                payload.RootElement.GetProperty("fee_payment").GetProperty("payer").GetString());
            Assert.False(payload.RootElement.TryGetProperty("tx_hash_hex", out _));
            return JsonResponse("{}", HttpStatusCode.OK);
        });
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        await Assert.ThrowsAsync<JsonException>(() => client.PrepareAccountFaucetAsync(
            ValidFaucetClaim(),
            binding,
            PreparedAccountFeePayment,
            FaucetPolicy(),
            OnboardingFixtureNetworkId,
            cancellationToken: TestContext.Current.CancellationToken));
    }

    public static IEnumerable<object[]> InvalidPreparedMutationBindings()
    {
        var valid = ValidPreparedMutationBinding(
            ToriiAccountFaucetPreparedTransactionV1.OperationV1);
        yield return [valid with { Schema = "iroha.taira.public-reset.mutation-binding.v2" }];
        yield return [valid with { AuthorizationSha256 = new string('A', 64) }];
        yield return [valid with { AuthorizationSha256 = new string('a', 63) }];
        yield return [valid with { AuthorizationNonce = new string('N', 32) }];
        yield return [valid with { AuthorizationNonce = new string('n', 31) }];
        yield return [valid with { Kind = "onboarding" }];
        yield return [valid with { Phase = "pre edge" }];
        yield return [valid with { Phase = string.Empty }];
        yield return [valid with { IdempotencyKey = new string('B', 64) }];
        yield return [valid with { IdempotencyKey = new string('b', 65) }];
        yield return [valid with { ExecutionExpiresAtUnixMilliseconds = 0 }];
        yield return
        [
            valid with
            {
                ExecutionExpiresAtUnixMilliseconds = checked(
                    (ulong)DateTimeOffset.UtcNow.ToUnixTimeMilliseconds()),
            },
        ];
    }

    [Theory]
    [MemberData(nameof(InvalidPreparedMutationBindings))]
    public async Task PrepareAccountFaucetAsyncRejectsNoncanonicalBindingBeforeDispatch(
        ToriiTairaPublicResetMutationBindingV1 binding)
    {
        using var handler = new RecordingHandler(_ =>
            throw new InvalidOperationException("noncanonical binding reached HTTP dispatch"));
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        await Assert.ThrowsAnyAsync<ArgumentException>(() => client.PrepareAccountFaucetAsync(
            ValidFaucetClaim(),
            binding,
            PreparedAccountFeePayment,
            FaucetPolicy(),
            OnboardingFixtureNetworkId,
            cancellationToken: TestContext.Current.CancellationToken));

        Assert.Null(handler.LastRequest);
    }

    [Fact]
    public void PreparedRequestModelsRejectUnknownLegacyFields()
    {
        var bindingJson = JsonSerializer.Serialize(
            ValidPreparedMutationBinding(ToriiAccountFaucetPreparedTransactionV1.OperationV1));
        var claimJson = JsonSerializer.Serialize(ValidFaucetClaim());
        var json = $$"""
            {
              "schema": "{{ToriiAccountFaucetPrepareRequestV1.SchemaV1}}",
              "binding": {{bindingJson}},
              "claim": {{claimJson}},
              "direct_submit": true
            }
            """;

        Assert.Throws<JsonException>(() =>
            JsonSerializer.Deserialize<ToriiAccountFaucetPrepareRequestV1>(json));
    }

    [Fact]
    public void PreparedProtocolModelsRejectMissingDefaultableRequiredFields()
    {
        var onboardingPrepare = JsonSerializer.SerializeToNode(
            new ToriiAccountOnboardingPrepareRequestV1(
                ValidPreparedMutationBinding(
                    ToriiAccountOnboardingPreparedTransactionV1.OperationV1),
                SharedOnboardingReceipt(),
                PreparedAccountFeePayment))!.AsObject();
        Assert.True(onboardingPrepare.Remove("fee_payment"));
        Assert.Throws<JsonException>(() =>
            onboardingPrepare.Deserialize<ToriiAccountOnboardingPrepareRequestV1>());

        var faucetPrepare = JsonSerializer.SerializeToNode(
            new ToriiAccountFaucetPrepareRequestV1(
                ValidPreparedMutationBinding(
                    ToriiAccountFaucetPreparedTransactionV1.OperationV1),
                ValidFaucetClaim(),
                PreparedAccountFeePayment))!.AsObject();
        Assert.True(faucetPrepare.Remove("fee_payment"));
        Assert.Throws<JsonException>(() =>
            faucetPrepare.Deserialize<ToriiAccountFaucetPrepareRequestV1>());

        foreach (var field in new[] { "pow_anchor_height", "pow_nonce_hex" })
        {
            var missingClaimField = JsonSerializer.SerializeToNode(
                ValidFaucetClaim())!.AsObject();
            Assert.True(missingClaimField.Remove(field));
            Assert.ThrowsAny<Exception>(() =>
                missingClaimField.Deserialize<ToriiAccountFaucetClaimV1>());

            var nullClaimField = JsonSerializer.SerializeToNode(
                ValidFaucetClaim())!.AsObject();
            nullClaimField[field] = null;
            Assert.ThrowsAny<Exception>(() =>
                nullClaimField.Deserialize<ToriiAccountFaucetClaimV1>());
        }

        var onboarding = JsonNode.Parse(
            PreparedTransactionSignatureVector("onboarding_prepared")
                .GetProperty("response")
                .GetRawText())!.AsObject();
        Assert.True(onboarding.Remove("schema"));
        Assert.Throws<JsonException>(() =>
            onboarding.Deserialize<ToriiAccountOnboardingPreparedTransactionV1>());

        var proofRequired = JsonNode.Parse(
            PreparedTransactionSignatureVector("onboarding_proof_required")
                .GetProperty("response")
                .GetRawText())!.AsObject();
        Assert.True(proofRequired["binding"]!.AsObject().Remove("schema"));
        Assert.Throws<JsonException>(() =>
            proofRequired.Deserialize<ToriiAccountOnboardingProofRequiredPrepareResponseV1>());

        var faucet = JsonNode.Parse(
            PreparedTransactionSignatureVector("faucet_prepared")
                .GetProperty("response")
                .GetRawText())!.AsObject();
        Assert.True(faucet.Remove("operation"));
        Assert.Throws<JsonException>(() =>
            faucet.Deserialize<ToriiAccountFaucetPreparedTransactionV1>());

        Assert.Throws<JsonException>(() => JsonSerializer.Deserialize<
            ToriiPreparedTransactionSubmitResponseV1>(
                """{"binding":{},"operation":"faucet","transaction_hash_hex":"","outcome":"Pending"}"""));
    }

    private static ToriiAccountFaucetClaimV1 ValidFaucetClaim() =>
        new(CanonicalAccountId, 68, "00");

    private static ToriiAccountFaucetPolicyV1 FaucetPolicy(
        JsonElement vector,
        ToriiAccountFaucetPreparedTransactionV1 prepared) =>
        new(
            vector.GetProperty("signer_account_id").GetString()!,
            prepared.AssetDefinitionId,
            NumericV1.QuantityValue.ParseCanonical(prepared.Amount));

    private static ToriiAccountFaucetPolicyV1 FaucetPolicy() =>
        new(
            OnboardingFixtureAuthority,
            "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
            NumericV1.QuantityValue.ParseCanonical("1"));

    private static JsonElement PreparedTransactionSignatureVector(string name)
    {
        using var fixture = JsonDocument.Parse(File.ReadAllText(Path.Combine(
            AppContext.BaseDirectory,
            "Fixtures",
            "prepared_transaction_signature_v1.json")));
        Assert.Equal(
            "iroha.taira.prepared-transaction-signature-fixture.v1",
            fixture.RootElement.GetProperty("schema").GetString());
        Assert.Equal(
            "69726f68613a74616972613a70726570617265642d7472616e73616374696f6e3a763100",
            fixture.RootElement.GetProperty("signature_domain_hex").GetString());
        Assert.Equal(
            ToriiPreparedTransactionSignatureV1.TranscriptSchema,
            fixture.RootElement.GetProperty("transcript_schema").GetString());
        Assert.Equal("u64_be", fixture.RootElement.GetProperty("frame_length_encoding").GetString());
        Assert.Equal("iroha_blake2b_256", fixture.RootElement.GetProperty("digest_algorithm").GetString());
        var vector = fixture.RootElement
            .GetProperty("vectors")
            .EnumerateArray()
            .Single(vector => string.Equals(
                vector.GetProperty("name").GetString(),
                name,
                StringComparison.Ordinal))
            .Clone();
        _ = PreparedTransactionSignatureNetworkId(vector);
        return vector;
    }

    private static T DeserializePreparedFixture<T>(JsonElement value) where T : class =>
        JsonSerializer.Deserialize<T>(value.GetRawText())
        ?? throw new InvalidOperationException($"Prepared fixture `{typeof(T).Name}` decoded to null.");

    private static JsonObject AtomicOnboardingStateResponse(
        ToriiAccountOnboardingProofRequiredPrepareResponseV1 proofRequired,
        NetworkId networkId,
        string? aliasTargetAccountId) =>
        new()
        {
            ["version"] = 1,
            ["network_id"] = networkId.ToString(),
            ["account_id"] = proofRequired.AccountId,
            ["alias"] = proofRequired.Alias,
            ["account_exists"] = true,
            ["alias_target_account_id"] = aliasTargetAccountId,
            ["observed_block_height"] = 19,
            ["observed_block_hash"] = networkId.ToString(),
        };

    private static JsonObject MutateAtomicState(
        JsonObject exact,
        string field,
        JsonNode? value)
    {
        var mutated = (JsonObject)exact.DeepClone();
        mutated[field] = value;
        return mutated;
    }

    private static NetworkId PreparedTransactionSignatureNetworkId(JsonElement vector) =>
        NetworkId.Parse(
            vector.GetProperty("network_id").GetString()
            ?? throw new InvalidOperationException("Prepared fixture vector network_id is missing."));
}
