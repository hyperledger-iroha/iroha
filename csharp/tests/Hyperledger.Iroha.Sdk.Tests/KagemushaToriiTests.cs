using System.Buffers.Binary;
using System.Net;
using System.Text;
using System.Text.Json;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class KagemushaToriiTests
{
    private const string ExpectedNetworkIdLiteral =
        "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149";
    private static readonly string OperationId = new('1', 64);
    private static readonly string TransactionHash = new('3', 64);
    private static readonly string UnmarkedTransactionHash = new('2', 64);
    private static NetworkId ExpectedNetworkId => NetworkId.Parse(ExpectedNetworkIdLiteral);

    [Fact]
    public async Task OfflineCapabilityUsesTheAssetNeutralStableRouteAndRequiresBridgeAbi22()
    {
        using var handler = new KagemushaHandler(request =>
        {
            Assert.Equal(HttpMethod.Get, request.Method);
            Assert.Equal("/v1/offline/readiness", request.RequestUri!.AbsolutePath);
            Assert.Equal(string.Empty, request.RequestUri.Query);
            return JsonResponse(OfflineCapabilityJson());
        });
        using var client = NewClient(handler);

        var capability = await client.GetOfflineCapabilityAsync(
            TestContext.Current.CancellationToken);

        Assert.False(capability.Mandatory);
        Assert.Equal("cash_handoff_v1", capability.CashHandoffCapability);
        Assert.Equal(22U, capability.RequiredBridgeAbiVersion);
        Assert.Equal(8U, capability.MaxHops);
        Assert.False(capability.Ready);
        Assert.Empty(capability.Assets);
        Assert.Equal(3, capability.Blockers.Length);
        Assert.Null(typeof(ToriiClient).GetMethod("GetKagemushaReadinessV4Async"));
        Assert.Null(typeof(ToriiClient).Assembly.GetType(
            "Hyperledger.Iroha.Torii.ToriiKagemushaReadinessV4"));
    }

    [Fact]
    public async Task OfflineCapabilityRejectsAbi20InsteadOfUpgradingIt()
    {
        using var handler = new KagemushaHandler(_ => JsonResponse(
            OfflineCapabilityJson().Replace(
                "\"required_bridge_abi_version\": 22",
                "\"required_bridge_abi_version\": 20",
                StringComparison.Ordinal)));
        using var client = NewClient(handler);

        var error = await Assert.ThrowsAsync<JsonException>(() =>
            client.GetOfflineCapabilityAsync(TestContext.Current.CancellationToken));

        Assert.Contains("required_bridge_abi_version must be 22", error.Message);
    }

    [Theory]
    [MemberData(nameof(InvalidOfflineCapabilities))]
    public async Task OfflineCapabilityRejectsNonCanonicalFailClosedResponses(
        string payload,
        string expectedMessage)
    {
        using var handler = new KagemushaHandler(_ => JsonResponse(payload));
        using var client = NewClient(handler);

        var error = await Assert.ThrowsAsync<JsonException>(() =>
            client.GetOfflineCapabilityAsync(TestContext.Current.CancellationToken));

        Assert.Contains(expectedMessage, error.Message);
    }

    [Fact]
    public async Task TopUpTransportsAnExternalV4NoritoArchiveWithoutAProverClaim()
    {
        var archive = NoritoArchive();
        var expectedArchive = archive.ToArray();
        using var handler = new KagemushaHandler(request =>
        {
            Assert.Equal(HttpMethod.Post, request.Method);
            Assert.Equal("/v1/offline/top-up", request.RequestUri!.AbsolutePath);
            Assert.Equal("application/x-norito", request.Content!.Headers.ContentType!.MediaType);
            Assert.Equal(OperationId, Assert.Single(request.Headers.GetValues("Idempotency-Key")));
            Assert.Equal(expectedArchive, request.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult());
            return AcceptedOperationResponse(OperationReferenceJson("top_up"));
        });
        using var client = NewClient(handler);
        var request = new ToriiKagemushaTopUpRequestV4(archive);

        archive[4] = 0xff;
        var reference = await client.SubmitKagemushaTopUpV4Async(
            request,
            TestContext.Current.CancellationToken);

        Assert.Equal(4, request.Version);
        Assert.Equal(1_234UL, request.IssuedAtMilliseconds);
        Assert.Equal(ToriiKagemushaOperationKind.TopUp, reference.Kind);
        Assert.Equal(ToriiKagemushaOperationState.Pending, reference.State);
        Assert.Equal(OperationId, reference.OperationId);
        Assert.DoesNotContain(
            typeof(ToriiClient).Assembly.GetTypes(),
            type => type.Name.Contains("Kagemusha", StringComparison.Ordinal)
                && type.Name.Contains("Prover", StringComparison.Ordinal));
    }

    [Fact]
    public void RequestArchivesDeriveAndBindTheSignedOperationIdentityAndTime()
    {
        var topUp = new ToriiKagemushaTopUpRequestV4(NoritoArchive());
        var redeem = new ToriiKagemushaRedeemRequestV4(NoritoArchive(redeem: true));

        Assert.Equal(OperationId, topUp.OperationId);
        Assert.Equal(1_234UL, topUp.IssuedAtMilliseconds);
        Assert.Equal(ExpectedNetworkId, topUp.NetworkId);
        Assert.Equal(OperationId, redeem.OperationId);
        Assert.Equal(1_234UL, redeem.IssuedAtMilliseconds);
        Assert.Equal(ExpectedNetworkId, redeem.NetworkId);
        Assert.Throws<ArgumentException>(() =>
            new ToriiKagemushaTopUpRequestV4(new string('5', 64), NoritoArchive()));
        Assert.Throws<ArgumentException>(() =>
            new ToriiKagemushaTopUpRequestV4(NoritoArchive(issuedAtMilliseconds: 0)));
        Assert.Throws<ArgumentException>(() =>
            new ToriiKagemushaTopUpRequestV4(NoritoArchive(redeem: true)));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public async Task SubmissionRejectsASignedRequestForAnotherNetwork(bool redeem)
    {
        using var handler = new KagemushaHandler(_ =>
            throw new InvalidOperationException("foreign request must not be dispatched"));
        using var client = NewClient(handler);
        var archive = NoritoArchive(redeem, networkId: new string('5', 64));

        if (redeem)
        {
            var error = await Assert.ThrowsAsync<ArgumentException>(() =>
                client.SubmitKagemushaRedeemV4Async(
                    new ToriiKagemushaRedeemRequestV4(archive),
                    TestContext.Current.CancellationToken));
            Assert.Contains("does not match the local signing context", error.Message);
        }
        else
        {
            var error = await Assert.ThrowsAsync<ArgumentException>(() =>
                client.SubmitKagemushaTopUpV4Async(
                    new ToriiKagemushaTopUpRequestV4(archive),
                    TestContext.Current.CancellationToken));
            Assert.Contains("does not match the local signing context", error.Message);
        }
    }

    [Fact]
    public async Task AcceptedReferenceMustBindTheSignedRequestTime()
    {
        using var handler = new KagemushaHandler(_ => AcceptedOperationResponse(
            OperationReferenceJson(
                "top_up",
                TransactionHash,
                submittedAtMilliseconds: 1_235)));
        using var client = NewClient(handler);

        var error = await Assert.ThrowsAsync<JsonException>(() =>
            client.SubmitKagemushaTopUpV4Async(
                new ToriiKagemushaTopUpRequestV4(NoritoArchive()),
                TestContext.Current.CancellationToken));

        Assert.Contains("does not match the submitted V4 command", error.Message);
    }

    [Fact]
    public async Task AcceptedReferenceRequiresPositiveRetryAfter()
    {
        using var handler = new KagemushaHandler(_ =>
        {
            var response = JsonResponse(
                OperationReferenceJson("top_up"),
                HttpStatusCode.Accepted);
            response.Headers.Location = new Uri(
                $"/v1/offline/operations/{OperationId}",
                UriKind.Relative);
            return response;
        });
        using var client = NewClient(handler);

        var error = await Assert.ThrowsAsync<InvalidDataException>(() =>
            client.SubmitKagemushaTopUpV4Async(
                new ToriiKagemushaTopUpRequestV4(NoritoArchive()),
                TestContext.Current.CancellationToken));

        Assert.Contains("positive Retry-After", error.Message);
    }

    [Fact]
    public async Task AcceptedReferenceRejectsNonCanonicalRetryAfter()
    {
        using var handler = new KagemushaHandler(_ =>
        {
            var response = AcceptedOperationResponse(OperationReferenceJson("top_up"));
            response.Headers.Remove("Retry-After");
            response.Headers.TryAddWithoutValidation("Retry-After", "01");
            return response;
        });
        using var client = NewClient(handler);

        var error = await Assert.ThrowsAsync<InvalidDataException>(() =>
            client.SubmitKagemushaTopUpV4Async(
                new ToriiKagemushaTopUpRequestV4(NoritoArchive()),
                TestContext.Current.CancellationToken));

        Assert.Contains("positive Retry-After", error.Message);
    }

    [Fact]
    public async Task RedeemAndOperationStatusPreserveTheStableRoutes()
    {
        var requests = new Queue<Func<HttpRequestMessage, HttpResponseMessage>>();
        requests.Enqueue(request =>
        {
            Assert.Equal("/v1/offline/redeem", request.RequestUri!.AbsolutePath);
            return AcceptedOperationResponse(OperationReferenceJson("redeem"));
        });
        requests.Enqueue(request =>
        {
            Assert.Equal($"/v1/offline/operations/{OperationId}", request.RequestUri!.AbsolutePath);
            return JsonResponse($$"""
                {
                  "state": "applied",
                  "value": {
                    "operation_id": "{{OperationId}}",
                    "result": {
                      "kind": "redeem",
                      "result": {
                        "transaction_hash": "{{TransactionHash}}",
                        "finalized_block_height": 42,
                        "server_time_ms": 1234
                      }
                    }
                  }
                }
                """);
        });
        using var handler = new KagemushaHandler(request => requests.Dequeue()(request));
        using var client = NewClient(handler);

        var reference = await client.SubmitKagemushaRedeemV4Async(
            new ToriiKagemushaRedeemRequestV4(NoritoArchive(redeem: true)),
            TestContext.Current.CancellationToken);
        var status = await client.GetKagemushaOperationStatusAsync(
            OperationId,
            TestContext.Current.CancellationToken);

        Assert.Equal(ToriiKagemushaOperationKind.Redeem, reference.Kind);
        Assert.Equal(ToriiKagemushaOperationState.Applied, status.State);
        Assert.Equal(42UL, status.RedeemResult!.FinalizedBlockHeight);
        Assert.Empty(requests);
    }

    [Fact]
    public async Task AppliedTopUpRejectsV3AnchorBytesInsteadOfUpgradingThem()
    {
        using var handler = new KagemushaHandler(_ => JsonResponse($$"""
            {
              "state": "applied",
              "value": {
                "operation_id": "{{OperationId}}",
                "result": {
                  "kind": "top_up",
                  "result": {
                    "transaction_hash": "{{TransactionHash}}",
                    "finalized_block_height": 42,
                    "server_time_ms": 1234,
                    "anchor": {
                      "version": 3,
                      "artifact_binding": {
                        "version": 4
                      }
                    },
                    "finality_proof": {}
                  }
                }
              }
            }
            """));
        using var client = NewClient(handler);

        var error = await Assert.ThrowsAsync<JsonException>(() =>
            client.GetKagemushaOperationStatusAsync(
                OperationId,
                TestContext.Current.CancellationToken));

        Assert.Contains("top-up anchor must use V4", error.Message);
    }

    [Fact]
    public async Task AppliedTopUpPreservesTheExactKnownAnchorAndFinalityBindings()
    {
        var status = await ParseOperationStatusAsync(AppliedTopUpStatusJson());

        Assert.Equal(ToriiKagemushaOperationKind.TopUp, status.Kind);
        Assert.Equal(TransactionHash, status.TopUpResult!.TransactionHash);
        Assert.Equal(42UL, status.TopUpResult.FinalizedBlockHeight);
        Assert.Equal(1_234UL, status.TopUpResult.ServerTimeMilliseconds);
    }

    [Theory]
    [InlineData("operation")]
    [InlineData("transaction")]
    [InlineData("height")]
    public async Task AppliedTopUpRejectsMismatchedAnchorBindings(string mismatch)
    {
        var json = mismatch switch
        {
            "operation" => AppliedTopUpStatusJson(anchorOperationId: new string('5', 64)),
            "transaction" => AppliedTopUpStatusJson(anchorTransactionHash: new string('5', 64)),
            "height" => AppliedTopUpStatusJson(anchorFinalizedBlockHeight: 43),
            _ => throw new InvalidOperationException(),
        };

        var error = await AssertOperationStatusRejectedAsync(json);

        Assert.Contains("does not match the operation, transaction, or finalized height", error.Message);
    }

    [Fact]
    public async Task AppliedTopUpRejectsMismatchedFinalityProofBindings()
    {
        var error = await AssertOperationStatusRejectedAsync(
            AppliedTopUpStatusJson(finalityProofBlockHeight: 43));

        Assert.Contains("finality proof does not match", error.Message);
    }

    [Fact]
    public async Task AppliedTopUpRejectsASelfConsistentForeignNetwork()
    {
        var error = await AssertOperationStatusRejectedAsync(
            AppliedTopUpStatusJson(networkId: new string('5', 64)));

        Assert.Contains("expected network", error.Message);
    }

    [Fact]
    public async Task AppliedTopUpRejectsAForeignCurrentNoteNetwork()
    {
        var error = await AssertOperationStatusRejectedAsync(
            AppliedTopUpStatusJson(currentNoteNetworkId: new string('5', 64)));

        Assert.Contains("expected network", error.Message);
    }

    [Fact]
    public async Task AppliedTopUpRejectsPublicNetworkTextOnTheTypedJsonWire()
    {
        var error = await AssertOperationStatusRejectedAsync(
            AppliedTopUpStatusJson(useRawNetworkId: true));

        Assert.Contains("canonical checksummed Norito NetworkId", error.Message);
    }

    [Fact]
    public async Task AppliedTopUpRejectsAllZeroAnchorDigests()
    {
        var error = await AssertOperationStatusRejectedAsync(
            AppliedTopUpStatusJson(anchorDigestByte: 0));

        Assert.Contains("anchor_digest must not be all zeroes", error.Message);
    }

    [Fact]
    public async Task OperationStatusRequiresAnExactPinnedNetwork()
    {
        using var handler = new KagemushaHandler(_ => JsonResponse(PendingStatusJson(TransactionHash)));
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));

        var error = await Assert.ThrowsAsync<InvalidOperationException>(() =>
            client.GetKagemushaOperationStatusAsync(
                OperationId,
                TestContext.Current.CancellationToken));

        Assert.Contains("LocalSigningContext with the exact NetworkId", error.Message);
    }

    [Fact]
    public async Task PendingStatusRequiresAPositiveSubmissionTime()
    {
        var error = await AssertOperationStatusRejectedAsync(
            PendingStatusJson(TransactionHash, submittedAtMilliseconds: 0));

        Assert.Contains("submitted_at_ms must be at least 1", error.Message);
    }

    [Theory]
    [InlineData(0UL, 1UL, "finalized_block_height")]
    [InlineData(1UL, 0UL, "server_time_ms")]
    public async Task AppliedResultsRequirePositiveHeightAndServerTime(
        ulong finalizedBlockHeight,
        ulong serverTimeMilliseconds,
        string expectedField)
    {
        var error = await AssertOperationStatusRejectedAsync(
            AppliedRedeemStatusJson(
                TransactionHash,
                finalizedBlockHeight,
                serverTimeMilliseconds));

        Assert.Contains($"{expectedField} must be at least 1", error.Message);
    }

    [Fact]
    public async Task AcceptedReferenceRejectsAnUnmarkedTransactionHash()
    {
        using var handler = new KagemushaHandler(_ =>
        {
            return AcceptedOperationResponse(
                OperationReferenceJson("top_up", UnmarkedTransactionHash));
        });
        using var client = NewClient(handler);

        var error = await Assert.ThrowsAsync<JsonException>(() =>
            client.SubmitKagemushaTopUpV4Async(
                new ToriiKagemushaTopUpRequestV4(NoritoArchive()),
                TestContext.Current.CancellationToken));

        Assert.Contains("Iroha hash marker bit", error.Message);
    }

    [Fact]
    public async Task PendingStatusRejectsAnUnmarkedTransactionHash()
    {
        var error = await AssertOperationStatusRejectedAsync(PendingStatusJson(UnmarkedTransactionHash));

        Assert.Contains("Iroha hash marker bit", error.Message);
    }

    [Fact]
    public async Task AppliedStatusRejectsAnUnmarkedTransactionHash()
    {
        var error = await AssertOperationStatusRejectedAsync(AppliedRedeemStatusJson(UnmarkedTransactionHash));

        Assert.Contains("Iroha hash marker bit", error.Message);
    }

    [Fact]
    public async Task RejectedStatusRejectsAnUnmarkedTransactionHash()
    {
        var error = await AssertOperationStatusRejectedAsync(RejectedStatusJson(UnmarkedTransactionHash));

        Assert.Contains("Iroha hash marker bit", error.Message);
    }

    [Fact]
    public async Task RejectedStatusRequiresTheExactCodeAndMessageEnvelope()
    {
        var absent = await ParseOperationStatusAsync(RejectedStatusJson(TransactionHash));
        var explicitNullError = await AssertOperationStatusRejectedAsync(
            RejectedStatusJson(TransactionHash, includeDetails: true));

        Assert.Equal("offline_operation_rejected", absent.Error!.Code);
        Assert.Contains("missing or unknown fields", explicitNullError.Message);
        Assert.DoesNotContain(
            typeof(ToriiKagemushaOperationError).GetProperties(),
            property => property.Name == "Details");
    }

    [Fact]
    public async Task RejectedStatusRequiresTheStableErrorCode()
    {
        var error = await AssertOperationStatusRejectedAsync(
            RejectedStatusJson(TransactionHash, code: "another_rejection"));

        Assert.Contains("offline_operation_rejected", error.Message);
    }

    [Fact]
    public async Task RejectedStatusRejectsNonNullDetailsAsAnUnknownField()
    {
        var error = await AssertOperationStatusRejectedAsync(
            RejectedStatusJson(
                TransactionHash,
                includeDetails: true,
                details: new { layer = "torii" }));

        Assert.Contains("missing or unknown fields", error.Message);
    }

    [Fact]
    public async Task RejectedStatusRequiresCanonicalMessageText()
    {
        var invalidMessages = new (string Message, string ExpectedError)[]
        {
            (string.Empty, "non-empty string"),
            (" rejected", "surrounding whitespace"),
            ("rejected ", "surrounding whitespace"),
            ("\u00a0rejected", "surrounding whitespace"),
            ("bad\nmessage", "control characters"),
            ("bad\u0085message", "control characters"),
        };

        foreach (var (message, expectedError) in invalidMessages)
        {
            var error = await AssertOperationStatusRejectedAsync(
                RejectedStatusJson(TransactionHash, message: message));

            Assert.Contains(expectedError, error.Message);
        }
    }

    [Fact]
    public async Task RejectedStatusCountsAstralMessagesByUnicodeScalar()
    {
        var maximumMessage = string.Concat(Enumerable.Repeat("\U0001f600", 1_024));
        var accepted = await ParseOperationStatusAsync(
            RejectedStatusJson(TransactionHash, message: maximumMessage));

        Assert.Equal(maximumMessage, accepted.Error!.Message);

        var oversizedMessage = maximumMessage + "\U0001f600";
        var error = await AssertOperationStatusRejectedAsync(
            RejectedStatusJson(TransactionHash, message: oversizedMessage));

        Assert.Contains("at most 1024 Unicode scalar values", error.Message);
    }

    [Fact]
    public async Task RejectedStatusRejectsUnknownErrorEnvelopeMembers()
    {
        var error = await AssertOperationStatusRejectedAsync(
            RejectedStatusJson(TransactionHash, includeUnknownErrorMember: true));

        Assert.Contains("missing or unknown fields", error.Message);
    }

    [Fact]
    public async Task RejectedStatusRejectsMalformedUnicodeScalarText()
    {
        var json = RejectedStatusJson(TransactionHash, message: "placeholder")
            .Replace("\"placeholder\"", "\"\\uD800\"", StringComparison.Ordinal);

        await AssertOperationStatusRejectedAsync(json);
    }

    private static byte[] NoritoArchive(
        bool redeem = false,
        ulong issuedAtMilliseconds = 1_234,
        string? operationId = null,
        string? networkId = null)
    {
        var exactOperationId = operationId ?? OperationId;
        var operationIdBytes = Convert.FromHexString(exactOperationId);
        var networkIdBytes = NetworkId.Parse(networkId ?? ExpectedNetworkIdLiteral).ToBytes();
        var issuedAt = new byte[sizeof(ulong)];
        BinaryPrimitives.WriteUInt64LittleEndian(issuedAt, issuedAtMilliseconds);
        var authorization = new CanonicalNoritoWriter();
        for (var index = 0; index < 10; index++)
        {
            if (index == 3)
            {
                authorization.WriteField(operationIdBytes);
            }
            else if (index == 4)
            {
                authorization.WriteField(issuedAt);
            }
            else
            {
                authorization.WriteField([0]);
            }
        }

        var fieldCount = redeem ? 10 : 8;
        var operationIdFieldIndex = redeem ? 8 : 6;
        var currentNote = new CanonicalNoritoWriter();
        for (var index = 0; index < 5; index++)
        {
            currentNote.WriteField(index == 0 ? networkIdBytes : [0]);
        }
        var statement = new CanonicalNoritoWriter();
        for (var index = 0; index < 13; index++)
        {
            statement.WriteField(index == 0 ? networkIdBytes : [0]);
        }
        var bundle = new CanonicalNoritoWriter();
        for (var index = 0; index < 3; index++)
        {
            bundle.WriteField(index == 0 ? statement.ToArray() : [0]);
        }
        var version = new byte[sizeof(ushort)];
        BinaryPrimitives.WriteUInt16LittleEndian(version, 4);
        var payload = new CanonicalNoritoWriter();
        for (var index = 0; index < fieldCount; index++)
        {
            if (index == 0)
            {
                payload.WriteField(version);
            }
            else if (index == operationIdFieldIndex)
            {
                payload.WriteField(operationIdBytes);
            }
            else if (!redeem && index == 3)
            {
                payload.WriteField(currentNote.ToArray());
            }
            else if (redeem && index == 1)
            {
                payload.WriteField(bundle.ToArray());
            }
            else if (index == fieldCount - 1)
            {
                payload.WriteField(authorization.ToArray());
            }
            else
            {
                payload.WriteField([0]);
            }
        }

        var schemaName = redeem
            ? ToriiKagemushaTransport.RedeemRequestSchemaName
            : ToriiKagemushaTransport.TopUpRequestSchemaName;
        var unpadded = NoritoCodec.Encode(
            schemaName,
            payload.ToArray(),
            NoritoCodec.CanonicalLayoutFlags);
        var archive = new byte[unpadded.Length + 8];
        unpadded.AsSpan(0, NoritoHeader.EncodedLength).CopyTo(archive);
        unpadded.AsSpan(NoritoHeader.EncodedLength)
            .CopyTo(archive.AsSpan(NoritoHeader.EncodedLength + 8));
        return archive;
    }

    private static string OperationReferenceJson(string kind) =>
        OperationReferenceJson(kind, TransactionHash);

    private static string OperationReferenceJson(
        string kind,
        string transactionHash,
        ulong submittedAtMilliseconds = 1_234) => $$"""
        {
          "operation_id": "{{OperationId}}",
          "kind": {"kind": "{{kind}}", "value": null},
          "state": {"state": "pending", "value": null},
          "transaction_hash": "{{transactionHash}}",
          "status_uri": "/v1/offline/operations/{{OperationId}}",
          "submitted_at_ms": {{submittedAtMilliseconds}}
        }
        """;

    private static string PendingStatusJson(
        string transactionHash,
        ulong submittedAtMilliseconds = 1_234) =>
        JsonSerializer.Serialize(new
        {
            state = "pending",
            value = new
            {
                operation_id = OperationId,
                kind = new { kind = "top_up", value = (object?)null },
                transaction_hash = transactionHash,
                submitted_at_ms = submittedAtMilliseconds,
            },
        });

    private static string AppliedRedeemStatusJson(
        string transactionHash,
        ulong finalizedBlockHeight = 42,
        ulong serverTimeMilliseconds = 1_234) =>
        JsonSerializer.Serialize(new
        {
            state = "applied",
            value = new
            {
                operation_id = OperationId,
                result = new
                {
                    kind = "redeem",
                    result = new
                    {
                        transaction_hash = transactionHash,
                        finalized_block_height = finalizedBlockHeight,
                        server_time_ms = serverTimeMilliseconds,
                    },
                },
            },
        });

    private static string AppliedTopUpStatusJson(
        string? anchorOperationId = null,
        string? anchorTransactionHash = null,
        ulong anchorFinalizedBlockHeight = 42,
        ulong finalityProofBlockHeight = 42,
        string? networkId = null,
        string? currentNoteNetworkId = null,
        int anchorDigestByte = 0x77,
        bool useRawNetworkId = false)
    {
        var publicNetworkId = networkId ?? ExpectedNetworkIdLiteral;
        var exactNetworkId = useRawNetworkId
            ? publicNetworkId
            : TypedNetworkIdLiteral(publicNetworkId);
        var exactCurrentNoteNetworkId = TypedNetworkIdLiteral(
            currentNoteNetworkId ?? publicNetworkId);
        var exactAnchorOperationId = Convert.FromHexString(anchorOperationId ?? OperationId)
            .Select(static value => (int)value)
            .ToArray();
        var exactAnchorTransactionHash = Convert.FromHexString(
                anchorTransactionHash ?? TransactionHash)
            .Select(static value => (int)value)
            .ToArray();
        var anchorDigest = Enumerable.Repeat(anchorDigestByte, 32).ToArray();
        return JsonSerializer.Serialize(new
        {
            state = "applied",
            value = new
            {
                operation_id = OperationId,
                result = new
                {
                    kind = "top_up",
                    result = new
                    {
                        transaction_hash = TransactionHash,
                        finalized_block_height = 42,
                        server_time_ms = 1_234,
                        anchor = new
                        {
                            version = 4,
                            network_id = exactNetworkId,
                            current_note = new
                            {
                                network_id = exactCurrentNoteNetworkId,
                            },
                            topup_operation_id = exactAnchorOperationId,
                            finalized_tx_hash = exactAnchorTransactionHash,
                            finalized_height = anchorFinalizedBlockHeight,
                            anchor_digest = anchorDigest,
                            artifact_binding = new { version = 4 },
                        },
                        finality_proof = new
                        {
                            version = 1,
                            anchor = new
                            {
                                topup_operation_id = exactAnchorOperationId,
                                anchor_digest = anchorDigest,
                            },
                            commit_qc = new
                            {
                                height_context = new
                                {
                                    height = finalityProofBlockHeight,
                                    network_id = exactNetworkId,
                                },
                            },
                            anchor_path = new { },
                        },
                    },
                },
            },
        });
    }

    private static string TypedNetworkIdLiteral(string publicLiteral) =>
        JsonSerializer.Deserialize<string>(
            JsonSerializer.Serialize(NetworkId.Parse(publicLiteral)))!;

    private static string RejectedStatusJson(
        string transactionHash,
        string code = "offline_operation_rejected",
        string message = "rejected",
        bool includeDetails = false,
        object? details = null,
        bool includeUnknownErrorMember = false)
    {
        var error = new Dictionary<string, object?>
        {
            ["code"] = code,
            ["message"] = message,
        };
        if (includeDetails)
        {
            error["details"] = details;
        }
        if (includeUnknownErrorMember)
        {
            error["retryable"] = false;
        }

        return JsonSerializer.Serialize(new
        {
            state = "rejected",
            value = new
            {
                operation_id = OperationId,
                kind = new { kind = "redeem", value = (object?)null },
                transaction_hash = transactionHash,
                error,
            },
        });
    }

    private static async Task<ToriiKagemushaOperationStatus> ParseOperationStatusAsync(string json)
    {
        using var handler = new KagemushaHandler(_ => JsonResponse(json));
        using var client = NewClient(handler);
        return await client.GetKagemushaOperationStatusAsync(
            OperationId,
            TestContext.Current.CancellationToken);
    }

    private static async Task<JsonException> AssertOperationStatusRejectedAsync(string json) =>
        await Assert.ThrowsAsync<JsonException>(() => ParseOperationStatusAsync(json));

    private static ToriiClient NewClient(KagemushaHandler handler) => new(
        new Uri("https://torii.example"),
        new HttpClient(handler),
        new ToriiClientOptions
        {
            LocalSigningContext = new ToriiLocalSigningContext(ExpectedNetworkId),
        });

    public static IEnumerable<object[]> InvalidOfflineCapabilities()
    {
        var canonical = OfflineCapabilityJson();
        yield return
        [
            canonical.Replace("\"mandatory\": false", "\"mandatory\": true", StringComparison.Ordinal),
            "mandatory must be false",
        ];
        yield return
        [
            canonical.Replace("cash_handoff_v1", "cash_handoff_v2", StringComparison.Ordinal),
            "cash_handoff_capability must be cash_handoff_v1",
        ];
        yield return
        [
            canonical.Replace("\"max_hops\": 8", "\"max_hops\": 9", StringComparison.Ordinal),
            "max_hops must be 8",
        ];
        yield return
        [
            canonical.Replace("\"ready\": false", "\"ready\": true", StringComparison.Ordinal),
            "ready must be false",
        ];
        yield return
        [
            canonical.Replace("\"assets\": []", "\"assets\": [{}]", StringComparison.Ordinal),
            "assets must be an empty array",
        ];
        yield return
        [
            canonical.Replace(
                "\"offline_cash_authenticated_release_unavailable\"",
                "\"unexpected\"",
                StringComparison.Ordinal),
            "blockers[0] is not the canonical activation blocker",
        ];
        yield return
        [
            canonical.Replace("\"blockers\":", "\"future\": true, \"blockers\":", StringComparison.Ordinal),
            "contains missing or unknown fields",
        ];
    }

    private static string OfflineCapabilityJson() => """
        {
          "mandatory": false,
          "cash_handoff_capability": "cash_handoff_v1",
          "required_bridge_abi_version": 22,
          "max_hops": 8,
          "ready": false,
          "assets": [],
          "blockers": [
            {
              "code": "offline_cash_authenticated_release_unavailable",
              "message": "No authenticated Offline Cash V1 release is selected by this asset-neutral response."
            },
            {
              "code": "offline_cash_eligible_asset_unavailable",
              "message": "No eligible Offline Cash V1 asset is selected by this asset-neutral response."
            },
            {
              "code": "offline_cash_proof_backend_unavailable",
              "message": "No reviewed production Offline Cash V1 proof and secure-device backend is authenticated by this response."
            }
          ]
        }
        """;

    private static HttpResponseMessage JsonResponse(
        string json,
        HttpStatusCode status = HttpStatusCode.OK) =>
        new(status)
        {
            Content = new StringContent(json, Encoding.UTF8, "application/json"),
        };

    private static HttpResponseMessage AcceptedOperationResponse(string json)
    {
        var response = JsonResponse(json, HttpStatusCode.Accepted);
        response.Headers.Location = new Uri(
            $"/v1/offline/operations/{OperationId}",
            UriKind.Relative);
        response.Headers.TryAddWithoutValidation("Retry-After", "1");
        return response;
    }

    private sealed class KagemushaHandler(
        Func<HttpRequestMessage, HttpResponseMessage> responder) : HttpMessageHandler
    {
        protected override Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken) =>
            Task.FromResult(responder(request));
    }
}
