using System.Globalization;
using System.Net;
using System.Numerics;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class FeeQuoteResponseValidationTests
{
    private const string AuthorityAccountId =
        "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";
    private const string AssetDefinitionIdA = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
    private const string AssetDefinitionIdB = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
    private const string AssetDefinitionIdC = "7ZepsJTHCVLKsrFFNZGSRGZgvBhv";
    private const string QuoteNetworkIdLiteral =
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";
    private static readonly byte[] AuthorityPrivateKeySeed = Convert.FromHexString(
        "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032");
    private static readonly NetworkId QuoteNetworkId = NetworkId.Parse(QuoteNetworkIdLiteral);

    private static readonly FeeSponsorProgramId SponsorProgram =
        new(AuthorityAccountId, "wallet_fx");

    [Fact]
    public void AuthorityQuoteBindsDraftAuthorityAndReturnedComponents()
    {
        var requested = FeePaymentIntent.Authority(
            [new FeeChargeLimit(FeeChargeKind.Nexus, AssetDefinitionIdA, "9")],
            500_000);
        var quote = AuthorityQuote("4", 500_000);

        Validate(quote, requested);

        AssertRejected(
            quote with { Observation = quote.Observation with { NextBlockHeight = 0 } },
            requested);
        AssertRejected(
            quote with
            {
                Decision = AuthorityDecision("different-authority"),
            },
            requested);
        AssertRejected(
            quote with { Components = requested.ChargeLimits },
            requested);
        AssertRejected(
            quote with
            {
                Capacities = [Capacity(AssetDefinitionIdA, "9", "0", "9", "9", "9")],
            },
            requested);
        AssertRejected(
            quote,
            FeePaymentIntent.Authority(requested.ChargeLimits, 500_001));
    }

    [Fact]
    public async Task QuoteAndSignFreezesTheDraftBeforeAwaitingTheQuote()
    {
        var requestedIntent = FeePaymentIntent.Authority(
            [new FeeChargeLimit(FeeChargeKind.Nexus, AssetDefinitionIdA, "9")]);
        var quote = AuthorityQuote("4", gasLimit: null);
        var requestObserved = new TaskCompletionSource(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var releaseResponse = new TaskCompletionSource(
            TaskCreationOptions.RunContinuationsAsynchronously);
        using var handler = new AsyncStubHandler(async (request, cancellationToken) =>
        {
            Assert.Equal("/v1/fees/quote", request.RequestUri!.AbsolutePath);
            requestObserved.SetResult();
            await releaseResponse.Task.WaitAsync(cancellationToken);
            return JsonResponse(QuoteBody(quote));
        });
        using var client = new IrohaClient(
            new Uri("https://torii.example"),
            new HttpClient(handler),
            new ToriiClientOptions
            {
                NetworkId = QuoteNetworkId,
                CanonicalRequestCredentials = new CanonicalRequestCredentials(
                    AuthorityAccountId,
                    AuthorityPrivateKeySeed),
            },
            TransactionSubmissionTransportAssurance.OneShotWithoutRedirectsOrRetries);
        var transaction = new TransactionBuilder(
                QuoteNetworkId,
                AuthorityAccountId,
                requestedIntent)
            .TransferDomain("wonderland", AuthorityAccountId)
            .SetCreationTimeMilliseconds(1_735_000_000_123)
            .SetTimeToLiveMilliseconds(60_000);

        var pending = client.Ledger.QuoteAndSignAsync(
            transaction,
            AuthorityPrivateKeySeed,
            TestContext.Current.CancellationToken);
        await requestObserved.Task.WaitAsync(TestContext.Current.CancellationToken);

        transaction
            .TransferDomain("mutated", AuthorityAccountId)
            .SetNonce(42)
            .SetMetadata("changed", JsonValue.Create(true));
        releaseResponse.SetResult();

        var actual = await pending;
        var expected = new TransactionBuilder(
                QuoteNetworkId,
                AuthorityAccountId,
                quote.Intent)
            .TransferDomain("wonderland", AuthorityAccountId)
            .SetCreationTimeMilliseconds(1_735_000_000_123)
            .SetTimeToLiveMilliseconds(60_000)
            .BuildSigned(AuthorityPrivateKeySeed);
        Assert.Equal(expected.VersionedNoritoBytes, actual.Transaction.VersionedNoritoBytes);
    }

    [Fact]
    public void SponsorQuoteAggregatesSharedAssetCapacityAndRejectsShortfalls()
    {
        var intent = FeePaymentIntent.Sponsor(
            SponsorProgram,
            7,
            [
                new FeeChargeLimit(FeeChargeKind.Nexus, AssetDefinitionIdA, "3"),
                new FeeChargeLimit(FeeChargeKind.PipelineGas, AssetDefinitionIdA, "4"),
            ],
            500_000);
        var quote = SponsorQuote(
            intent,
            [Capacity(AssetDefinitionIdA, "10", "3", "7", "7", "7")]);

        Validate(quote, intent);

        var capacity = quote.Capacities[0];
        foreach (var mutation in new ToriiFeeQuoteResponse[]
        {
            quote with { Capacities = [] },
            quote with { Capacities = [capacity with { VaultBalance = "9" }] },
            quote with { Capacities = [capacity with { BlockRemaining = "6" }] },
            quote with { Capacities = [capacity with { ProgramEpochRemaining = "6" }] },
            quote with { Capacities = [capacity with { BeneficiaryEpochRemaining = "6" }] },
            quote with { Decision = SponsorDecision(SponsorProgram, 8) },
        })
        {
            AssertRejected(mutation, intent);
        }
    }

    [Fact]
    public void SponsorQuoteRejectsAggregateQuantityOverflow()
    {
        var maximum = ((BigInteger.One << 511) - BigInteger.One)
            .ToString(CultureInfo.InvariantCulture);
        var intent = FeePaymentIntent.Sponsor(
            SponsorProgram,
            7,
            [
                new FeeChargeLimit(FeeChargeKind.Nexus, AssetDefinitionIdA, maximum),
                new FeeChargeLimit(FeeChargeKind.PipelineGas, AssetDefinitionIdA, "1"),
            ]);
        var quote = SponsorQuote(
            intent,
            [Capacity(AssetDefinitionIdA, maximum, "0", maximum, maximum, maximum)]);

        AssertRejected(quote, intent);
    }

    [Fact]
    public void SponsorCapacitiesAreUniqueRelatedAndCanonicallyOrdered()
    {
        Assert.True(string.CompareOrdinal(AssetDefinitionIdA, AssetDefinitionIdB) < 0);
        var intent = FeePaymentIntent.Sponsor(
            SponsorProgram,
            7,
            [
                new FeeChargeLimit(FeeChargeKind.Nexus, AssetDefinitionIdA, "2"),
                new FeeChargeLimit(FeeChargeKind.PipelineGas, AssetDefinitionIdB, "3"),
            ]);
        var first = Capacity(AssetDefinitionIdA, "2", "0", "2", "2", "2");
        var second = Capacity(AssetDefinitionIdB, "3", "0", "3", "3", "3");
        var quote = SponsorQuote(intent, [first, second]);

        Validate(quote, intent);

        foreach (var mutation in new ToriiFeeQuoteResponse[]
        {
            quote with { Capacities = [second, first] },
            quote with { Capacities = [first, second with { AssetDefinitionId = AssetDefinitionIdA }] },
            quote with { Capacities = [first, second with { AssetDefinitionId = AssetDefinitionIdC }] },
        })
        {
            AssertRejected(mutation, intent);
        }
    }

    [Fact]
    public void FeeFreeSponsorRequiresAndAcceptsEmptyCapacities()
    {
        var intent = FeePaymentIntent.Sponsor(SponsorProgram, 7, []);
        var quote = SponsorQuote(intent, []);

        Validate(quote, intent);
        AssertRejected(
            quote with
            {
                Capacities = [Capacity(AssetDefinitionIdA, "0", "0", "0", "0", "0")],
            },
            intent);
    }

    [Fact]
    public void FeeQuoteJsonRequiresNullableFieldsAndRejectsUnknownFields()
    {
        var quote = AuthorityQuote("4", null);
        var root = JsonNode.Parse(JsonSerializer.Serialize(quote))!.AsObject();
        var decisionValue = root["decision"]!["value"]!.AsObject();
        decisionValue.Remove("program_revision");

        Assert.Throws<JsonException>(() =>
            JsonSerializer.Deserialize<ToriiFeeQuoteResponse>(root.ToJsonString()));

        root = JsonNode.Parse(JsonSerializer.Serialize(quote))!.AsObject();
        root["observation"]!["legacy_height"] = 7;
        Assert.Throws<JsonException>(() =>
            JsonSerializer.Deserialize<ToriiFeeQuoteResponse>(root.ToJsonString()));
    }

    [Fact]
    public void FeeIntentJsonRequiresExplicitNullableGasLimit()
    {
        var root = JsonNode.Parse(JsonSerializer.Serialize(FeePaymentIntent.Authority([])))!
            .AsObject();
        root["value"]!.AsObject().Remove("gas_limit");

        Assert.Throws<JsonException>(() =>
            JsonSerializer.Deserialize<FeePaymentIntent>(root.ToJsonString()));
    }

    [Fact]
    public void SponsorProgramEqualityUsesControllerIdentityAndProgramName()
    {
        var alternateSponsor = AccountAddress.Parse(AuthorityAccountId)
            .ToI105(AccountAddress.TestChainDiscriminant);
        var canonical = new FeeSponsorProgramId(AuthorityAccountId, "wallet_fx");
        var alternate = new FeeSponsorProgramId(alternateSponsor, "wallet_fx");

        Assert.Equal(canonical, alternate);
        Assert.True(canonical == alternate);
        Assert.Equal(canonical.GetHashCode(), alternate.GetHashCode());
        Assert.Contains(alternate, new HashSet<FeeSponsorProgramId> { canonical });
        Assert.NotEqual(canonical, new FeeSponsorProgramId(alternateSponsor, "other"));

        var otherSponsor = Ed25519KeyPair.FromSeed(Enumerable.Repeat((byte)0x42, 32).ToArray())
            .ToAccountAddress()
            .ToI105();
        Assert.NotEqual(canonical, new FeeSponsorProgramId(otherSponsor, "wallet_fx"));
        _ = new FeeSponsorProgramId(AuthorityAccountId, "wallet😀");
        _ = new FeeSponsorProgramId(
            AuthorityAccountId,
            "wallet\u200b\u2060\ufeff\u00ad");

        foreach (var invalidName in new[]
        {
            new string('x', 256),
            "wallet\u0091",
            "wallet\u202e",
            "wallet\ud800",
        })
        {
            Assert.Throws<ArgumentException>(() =>
                new FeeSponsorProgramId(AuthorityAccountId, invalidName));
        }

        var requested = FeePaymentIntent.Sponsor(canonical, 7, []);
        var quoted = FeePaymentIntent.Sponsor(alternate, 7, []);
        Validate(SponsorQuote(quoted, []), requested);
    }

    [Fact]
    public async Task QuoteFeesAcceptsEquivalentI105DiscriminantsForAuthAndAuthorityDebit()
    {
        var alternateAuthority = AccountAddress.Parse(AuthorityAccountId)
            .ToI105(AccountAddress.TestChainDiscriminant);
        var quote = AuthorityQuote("4", null);
        using var handler = new StubHandler(request =>
        {
            Assert.Equal("/v1/fees/quote", request.RequestUri!.AbsolutePath);
            Assert.Equal(
                AccountAddress.Parse(AuthorityAccountId).CanonicalHex,
                Assert.Single(request.Headers.GetValues("X-Iroha-Account")));
            return JsonResponse(QuoteBody(quote));
        });
        using var client = CreateQuoteClient(handler, AuthorityAccountId);

        var response = await client.QuoteFeesAsync(
            Payload(quote.Intent, alternateAuthority),
            TestContext.Current.CancellationToken);

        Assert.Equal(quote.Intent, response.Intent);
    }

    [Fact]
    public async Task QuoteFeesRejectsADifferentAuthorityControllerBeforeDispatch()
    {
        var differentAuthority = Ed25519KeyPair.FromSeed(
                Enumerable.Repeat((byte)0x24, 32).ToArray())
            .ToAccountAddress()
            .ToI105();
        var quote = AuthorityQuote("4", null);
        using var handler = new StubHandler(_ =>
            throw new InvalidOperationException("mismatched authority reached HTTP dispatch"));
        using var client = CreateQuoteClient(handler, AuthorityAccountId);

        var error = await Assert.ThrowsAsync<InvalidOperationException>(() => client.QuoteFeesAsync(
            Payload(quote.Intent, differentAuthority),
            TestContext.Current.CancellationToken));

        Assert.Contains("must identify", error.Message, StringComparison.Ordinal);
    }

    [Fact]
    public async Task QuoteFeesDefersValidatedAliasAuthorityBindingToTorii()
    {
        const string alias = "wallet@universal";
        var quote = AuthorityQuote("4", null);
        using var handler = new StubHandler(request =>
        {
            Assert.Equal(alias, Assert.Single(request.Headers.GetValues("X-Iroha-Account")));
            return JsonResponse(QuoteBody(quote));
        });
        using var client = CreateQuoteClient(handler, alias);

        var response = await client.QuoteFeesAsync(
            Payload(quote.Intent),
            TestContext.Current.CancellationToken);

        Assert.Equal(quote.Intent, response.Intent);
    }
    [Fact]
    public async Task QuoteFeesAcceptsAnExact65536ByteActualBody()
    {
        const int maximumBytes = 64 * 1024;
        var quote = AuthorityQuote("4", null);
        var body = QuoteBody(quote, maximumBytes);
        using var handler = new StubHandler(_ => JsonResponse(body, declaredLength: 1));
        using var client = CreateQuoteClient(handler, AuthorityAccountId);

        var response = await client.QuoteFeesAsync(
            Payload(quote.Intent),
            TestContext.Current.CancellationToken);

        Assert.Equal(maximumBytes, body.Length);
        Assert.Equal(quote.Intent, response.Intent);
    }

    [Theory]
    [InlineData(HttpStatusCode.OK)]
    [InlineData(HttpStatusCode.BadRequest)]
    public async Task QuoteFeesRejectsA65537ByteActualBodyWithoutTrustingContentLength(
        HttpStatusCode statusCode)
    {
        const int excessiveBytes = 64 * 1024 + 1;
        var quote = AuthorityQuote("4", null);
        var body = QuoteBody(quote, excessiveBytes);
        using var handler = new StubHandler(_ => JsonResponse(body, statusCode, declaredLength: 1));
        using var client = CreateQuoteClient(handler, AuthorityAccountId);

        var error = await Assert.ThrowsAsync<InvalidDataException>(() => client.QuoteFeesAsync(
            Payload(quote.Intent),
            TestContext.Current.CancellationToken));

        Assert.Equal(excessiveBytes, body.Length);
        Assert.Contains("65536-byte limit", error.Message, StringComparison.Ordinal);
    }

    [Fact]
    public async Task QuoteFeesRejectsANonJsonSuccessResponseMediaType()
    {
        var quote = AuthorityQuote("4", null);
        using var handler = new StubHandler(_ =>
        {
            var response = JsonResponse(QuoteBody(quote));
            response.Content.Headers.Remove("Content-Type");
            response.Content.Headers.TryAddWithoutValidation("Content-Type", "text/plain");
            return response;
        });
        using var client = CreateQuoteClient(handler, AuthorityAccountId);

        var error = await Assert.ThrowsAsync<InvalidDataException>(() => client.QuoteFeesAsync(
            Payload(quote.Intent),
            TestContext.Current.CancellationToken));

        Assert.Contains("Content-Type must be application/json", error.Message);
    }

    [Fact]
    public async Task QuoteFeesRejectsDuplicateContentTypeValues()
    {
        var quote = AuthorityQuote("4", null);
        using var handler = new StubHandler(_ =>
        {
            var response = JsonResponse(QuoteBody(quote));
            response.Content.Headers.TryAddWithoutValidation("Content-Type", "text/plain");
            return response;
        });
        using var client = CreateQuoteClient(handler, AuthorityAccountId);

        var error = await Assert.ThrowsAsync<InvalidDataException>(() => client.QuoteFeesAsync(
            Payload(quote.Intent),
            TestContext.Current.CancellationToken));

        Assert.Contains("Content-Type must be application/json", error.Message);
    }

    [Fact]
    public async Task QuoteFeesRejectsNestedDuplicateJsonKeysBeforeTypedDeserialization()
    {
        var quote = AuthorityQuote("4", null);
        var json = Encoding.UTF8.GetString(QuoteBody(quote));
        var duplicate = json.Replace(
            "\"program_revision\":null",
            "\"program_revision\":null,\"program_revision\":null",
            StringComparison.Ordinal);
        Assert.NotEqual(json, duplicate);
        using var handler = new StubHandler(_ => JsonResponse(Encoding.UTF8.GetBytes(duplicate)));
        using var client = CreateQuoteClient(handler, AuthorityAccountId);

        var error = await Assert.ThrowsAsync<JsonException>(() => client.QuoteFeesAsync(
            Payload(quote.Intent),
            TestContext.Current.CancellationToken));

        Assert.Contains("program_revision must not appear more than once", error.Message);
    }

    [Fact]
    public async Task FeeSponsorProgramLookupAcceptsAnExact65536ByteJsonResponse()
    {
        const int maximumBytes = 64 * 1024;
        var program = SponsorProgramResponse(SponsorProgram);
        var body = SponsorProgramBody(program, maximumBytes);
        using var handler = new StubHandler(request =>
        {
            Assert.Equal(
                "/v1/fee-sponsor-programs/by-id",
                request.RequestUri!.AbsolutePath);
            var response = JsonResponse(body, declaredLength: 1);
            response.Content.Headers.Remove("Content-Type");
            response.Content.Headers.TryAddWithoutValidation(
                "Content-Type",
                "Application/JSON; charset=utf-8; note=\"\u00e9\"");
            return response;
        });
        using var client = CreateQuoteClient(handler, AuthorityAccountId);

        var response = await client.GetFeeSponsorProgramAsync(
            SponsorProgram,
            TestContext.Current.CancellationToken);

        Assert.Equal(maximumBytes, body.Length);
        Assert.Equal(SponsorProgram, response.Id);
    }

    [Theory]
    [InlineData(HttpStatusCode.OK)]
    [InlineData(HttpStatusCode.BadRequest)]
    public async Task FeeSponsorProgramLookupRejectsA65537ByteActualBodyBeforeStatus(
        HttpStatusCode statusCode)
    {
        const int excessiveBytes = 64 * 1024 + 1;
        var body = SponsorProgramBody(SponsorProgramResponse(SponsorProgram), excessiveBytes);
        using var handler = new StubHandler(_ =>
            JsonResponse(body, statusCode, declaredLength: 1));
        using var client = CreateQuoteClient(handler, AuthorityAccountId);

        var error = await Assert.ThrowsAsync<InvalidDataException>(() =>
            client.GetFeeSponsorProgramAsync(
                SponsorProgram,
                TestContext.Current.CancellationToken));

        Assert.Contains("65536-byte limit", error.Message, StringComparison.Ordinal);
    }

    [Theory]
    [InlineData("missing")]
    [InlineData("text")]
    [InlineData("duplicate")]
    [InlineData("folded")]
    [InlineData("quoted-comma")]
    [InlineData("long-s")]
    [InlineData("dotless-i")]
    [InlineData("dotted-capital-i")]
    public async Task FeeSponsorProgramLookupRequiresOneJsonContentType(string mediaCase)
    {
        var body = SponsorProgramBody(SponsorProgramResponse(SponsorProgram));
        using var handler = new StubHandler(_ =>
        {
            var response = JsonResponse(body);
            if (mediaCase is "missing" or "text")
            {
                response.Content.Headers.Remove("Content-Type");
            }
            if (mediaCase == "text")
            {
                response.Content.Headers.TryAddWithoutValidation("Content-Type", "text/plain");
            }
            else if (mediaCase == "duplicate")
            {
                response.Content.Headers.TryAddWithoutValidation("Content-Type", "text/plain");
            }
            else if (mediaCase is "folded" or "quoted-comma")
            {
                response.Content.Headers.Remove("Content-Type");
                response.Content.Headers.TryAddWithoutValidation(
                    "Content-Type",
                    mediaCase == "folded"
                        ? "application/json, application/json"
                        : "application/json; profile=\"a,b\"");
            }
            else if (mediaCase is "long-s" or "dotless-i" or "dotted-capital-i")
            {
                response.Content.Headers.Remove("Content-Type");
                response.Content.Headers.TryAddWithoutValidation(
                    "Content-Type",
                    mediaCase switch
                    {
                        "long-s" => "application/jſon",
                        "dotless-i" => "applıcation/json",
                        _ => "applİcation/json",
                    });
            }
            return response;
        });
        using var client = CreateQuoteClient(handler, AuthorityAccountId);

        var error = await Assert.ThrowsAsync<InvalidDataException>(() =>
            client.GetFeeSponsorProgramAsync(
                SponsorProgram,
                TestContext.Current.CancellationToken));

        Assert.Contains("Content-Type must be application/json", error.Message);
    }

    [Fact]
    public async Task FeeSponsorProgramLookupRejectsUnknownFieldsAndSubstitutedIds()
    {
        var substituted = new FeeSponsorProgramId(AuthorityAccountId, "other");
        var body = SponsorProgramBody(SponsorProgramResponse(substituted));
        using (var handler = new StubHandler(_ => JsonResponse(body)))
        using (var client = CreateQuoteClient(handler, AuthorityAccountId))
        {
            var error = await Assert.ThrowsAsync<JsonException>(() =>
                client.GetFeeSponsorProgramAsync(
                    SponsorProgram,
                    TestContext.Current.CancellationToken));
            Assert.Contains("different program id", error.Message, StringComparison.Ordinal);
        }

        var json = Encoding.UTF8.GetString(
            SponsorProgramBody(SponsorProgramResponse(SponsorProgram)));
        Assert.StartsWith("{", json, StringComparison.Ordinal);
        var unknown = "{\"legacy\":true," + json[1..];
        using var unknownHandler = new StubHandler(_ =>
            JsonResponse(Encoding.UTF8.GetBytes(unknown)));
        using var unknownClient = CreateQuoteClient(unknownHandler, AuthorityAccountId);
        await Assert.ThrowsAsync<JsonException>(() =>
            unknownClient.GetFeeSponsorProgramAsync(
                SponsorProgram,
                TestContext.Current.CancellationToken));

        var nestedUnknown = JsonNode.Parse(json)!.AsObject();
        nestedUnknown["id"]!.AsObject()["legacy"] = true;
        using var nestedUnknownHandler = new StubHandler(_ =>
            JsonResponse(Encoding.UTF8.GetBytes(nestedUnknown.ToJsonString())));
        using var nestedUnknownClient = CreateQuoteClient(
            nestedUnknownHandler,
            AuthorityAccountId);
        await Assert.ThrowsAsync<JsonException>(() =>
            nestedUnknownClient.GetFeeSponsorProgramAsync(
                SponsorProgram,
                TestContext.Current.CancellationToken));
    }

    [Fact]
    public async Task FeeSponsorProgramLookupRejectsNonCanonicalJson()
    {
        var canonical = Encoding.UTF8.GetString(
            SponsorProgramBody(SponsorProgramResponse(SponsorProgram)));
        var id = JsonNode.Parse(canonical)!["id"]!.ToJsonString();
        var active = Encoding.UTF8.GetString(SponsorProgramBody(
            SponsorProgramResponse(SponsorProgram) with { ActiveRevision = 1 }));
        var explicitNull = JsonNode.Parse(canonical)!.AsObject();
        explicitNull["active_revision"] = null;
        var invalidBodies = new[]
        {
            canonical.Replace("\"id\":", "\"ID\":", StringComparison.Ordinal),
            canonical.Replace(
                "\"id\":",
                $"\"ID\":{id},\"id\":",
                StringComparison.Ordinal),
            active.Replace(
                "\"active_revision\":1",
                "\"active_revision\":\"1\"",
                StringComparison.Ordinal),
            explicitNull.ToJsonString(),
        };
        Assert.All(invalidBodies, body => Assert.NotEqual(canonical, body));

        foreach (var invalidBody in invalidBodies)
        {
            using var handler = new StubHandler(_ =>
                JsonResponse(Encoding.UTF8.GetBytes(invalidBody)));
            using var client = CreateQuoteClient(handler, AuthorityAccountId);
            await Assert.ThrowsAsync<JsonException>(() =>
                client.GetFeeSponsorProgramAsync(
                    SponsorProgram,
                    TestContext.Current.CancellationToken));
        }
    }

    [Fact]
    public async Task FeeSponsorProgramLookupRejectsInvalidLifecycleAndZeroRevisions()
    {
        var invalidPrograms = new[]
        {
            SponsorProgramResponse(SponsorProgram) with
            {
                Lifecycle = new ToriiFeeSponsorProgramLifecycle
                {
                    State = "legacy",
                    Value = null,
                },
            },
            SponsorProgramResponse(SponsorProgram) with
            {
                Lifecycle = new ToriiFeeSponsorProgramLifecycle
                {
                    State = "active",
                    Value = JsonValue.Create(true),
                },
            },
            SponsorProgramResponse(SponsorProgram) with { ActiveRevision = 0 },
            SponsorProgramResponse(SponsorProgram) with { StagedRevision = 0 },
            SponsorProgramResponse(SponsorProgram) with
            {
                ScheduledActivation = new ToriiFeeSponsorProgramActivation
                {
                    Revision = 0,
                    ActivateAtHeight = 1,
                },
            },
            SponsorProgramResponse(SponsorProgram) with
            {
                ScheduledActivation = new ToriiFeeSponsorProgramActivation
                {
                    Revision = 1,
                    ActivateAtHeight = 0,
                },
            },
        };

        foreach (var invalidProgram in invalidPrograms)
        {
            using var handler = new StubHandler(_ =>
                JsonResponse(SponsorProgramBody(invalidProgram)));
            using var client = CreateQuoteClient(handler, AuthorityAccountId);
            await Assert.ThrowsAsync<JsonException>(() =>
                client.GetFeeSponsorProgramAsync(
                    SponsorProgram,
                    TestContext.Current.CancellationToken));
        }

        var canonicalJson = JsonNode.Parse(
            SponsorProgramBody(SponsorProgramResponse(SponsorProgram)))!.AsObject();
        foreach (var field in new[]
        {
            "active_revision",
            "staged_revision",
            "scheduled_activation",
        })
        {
            var explicitNull = canonicalJson.DeepClone().AsObject();
            explicitNull[field] = null;
            using var handler = new StubHandler(_ =>
                JsonResponse(Encoding.UTF8.GetBytes(explicitNull.ToJsonString())));
            using var client = CreateQuoteClient(handler, AuthorityAccountId);
            await Assert.ThrowsAsync<JsonException>(() =>
                client.GetFeeSponsorProgramAsync(
                    SponsorProgram,
                    TestContext.Current.CancellationToken));
        }
    }

    private static ToriiFeeQuoteResponse AuthorityQuote(string maximum, ulong? gasLimit)
    {
        var intent = FeePaymentIntent.Authority(
            [new FeeChargeLimit(FeeChargeKind.Nexus, AssetDefinitionIdA, maximum)],
            gasLimit);
        return new ToriiFeeQuoteResponse
        {
            Intent = intent,
            Observation = Observation(),
            Components = intent.ChargeLimits,
            Capacities = [],
            Decision = AuthorityDecision(AuthorityAccountId),
        };
    }

    private static ToriiFeeQuoteResponse SponsorQuote(
        FeePaymentIntent intent,
        IReadOnlyList<ToriiFeeQuoteCapacity> capacities) => new()
        {
            Intent = intent,
            Observation = Observation(),
            Components = intent.ChargeLimits,
            Capacities = capacities,
            Decision = SponsorDecision(SponsorProgram, 7),
        };

    private static ToriiFeeQuoteObservation Observation() => new()
    {
        LedgerTimeMilliseconds = 42,
        NextBlockHeight = 7,
        RouteDataspaceId = 0,
    };

    private static ToriiFeeSponsorProgram SponsorProgramResponse(
        FeeSponsorProgramId programId) => new()
        {
            Id = programId,
            PayoutAccount = AuthorityAccountId,
            Lifecycle = new ToriiFeeSponsorProgramLifecycle
            {
                State = "staged",
                Value = null,
            },
        };

    private static ToriiFeeQuoteDecision AuthorityDecision(string authority) => new()
    {
        Status = "accepted",
        Value = new ToriiFeeQuoteDecisionValue
        {
            DebitSource = new ToriiFeeDebitSource
            {
                Kind = "account",
                Value = JsonSerializer.SerializeToElement(authority),
            },
            ProgramRevision = null,
        },
    };

    private static ToriiFeeQuoteDecision SponsorDecision(
        FeeSponsorProgramId program,
        ulong revision) => new()
        {
            Status = "accepted",
            Value = new ToriiFeeQuoteDecisionValue
            {
                DebitSource = new ToriiFeeDebitSource
                {
                    Kind = "sponsor_program",
                    Value = JsonSerializer.SerializeToElement(new
                    {
                        sponsor = program.Sponsor,
                        name = program.Name,
                    }),
                },
                ProgramRevision = revision,
            },
        };

    private static ToriiFeeQuoteCapacity Capacity(
        string assetDefinitionId,
        string vaultBalance,
        string reserveFloor,
        string blockRemaining,
        string programEpochRemaining,
        string beneficiaryEpochRemaining) => new()
        {
            AssetDefinitionId = assetDefinitionId,
            VaultBalance = vaultBalance,
            ReserveFloor = reserveFloor,
            BlockRemaining = blockRemaining,
            ProgramEpochRemaining = programEpochRemaining,
            BeneficiaryEpochRemaining = beneficiaryEpochRemaining,
        };

    private static UnsignedTransactionPayload Payload(
        FeePaymentIntent intent,
        string authority = AuthorityAccountId) => new(
            QuoteNetworkId,
            authority,
            1_735_000_000_123,
            new JsonObject { ["Instructions"] = new JsonArray() },
            60_000,
            null,
            intent,
            TransactionAdmissionIntent.Ordinary,
            new Dictionary<string, JsonNode?>());

    private static ToriiClient CreateQuoteClient(
        HttpMessageHandler handler,
        string accountId) =>
        new(
            new Uri("https://torii.example"),
            new HttpClient(handler),
            new ToriiClientOptions
            {
                NetworkId = QuoteNetworkId,
                CanonicalRequestCredentials = new CanonicalRequestCredentials(
                    accountId,
                    AuthorityPrivateKeySeed),
            },
            TransactionSubmissionTransportAssurance.OneShotWithoutRedirectsOrRetries);

    private static byte[] QuoteBody(ToriiFeeQuoteResponse quote, int? exactLength = null)
    {
        var encoded = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(quote));
        if (exactLength is null)
        {
            return encoded;
        }
        Assert.True(encoded.Length <= exactLength.Value);
        var padded = new byte[exactLength.Value];
        encoded.CopyTo(padded, 0);
        padded.AsSpan(encoded.Length).Fill((byte)' ');
        return padded;
    }

    private static byte[] SponsorProgramBody(
        ToriiFeeSponsorProgram program,
        int? exactLength = null)
    {
        var encoded = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(program));
        if (exactLength is null)
        {
            return encoded;
        }
        Assert.True(encoded.Length <= exactLength.Value);
        var padded = new byte[exactLength.Value];
        encoded.CopyTo(padded, 0);
        padded.AsSpan(encoded.Length).Fill((byte)' ');
        return padded;
    }

    private static HttpResponseMessage JsonResponse(
        byte[] body,
        HttpStatusCode statusCode = HttpStatusCode.OK,
        long? declaredLength = null)
    {
        var content = new StreamContent(new MemoryStream(body, writable: false));
        content.Headers.TryAddWithoutValidation("Content-Type", "application/json");
        if (declaredLength is not null)
        {
            content.Headers.ContentLength = declaredLength;
        }
        return new HttpResponseMessage(statusCode) { Content = content };
    }

    private static void Validate(ToriiFeeQuoteResponse quote, FeePaymentIntent requested) =>
        ToriiClient.ValidateFeeQuoteResponse(
            quote,
            requested,
            AuthorityAccountId,
            "fee quote response");

    private static void AssertRejected(
        ToriiFeeQuoteResponse quote,
        FeePaymentIntent requested) =>
        Assert.Throws<JsonException>(() => Validate(quote, requested));

    private sealed class StubHandler(Func<HttpRequestMessage, HttpResponseMessage> handler)
        : HttpMessageHandler
    {
        protected override Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken) => Task.FromResult(handler(request));
    }

    private sealed class AsyncStubHandler(
        Func<HttpRequestMessage, CancellationToken, Task<HttpResponseMessage>> handler)
        : HttpMessageHandler
    {
        protected override Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken) => handler(request, cancellationToken);
    }
}
