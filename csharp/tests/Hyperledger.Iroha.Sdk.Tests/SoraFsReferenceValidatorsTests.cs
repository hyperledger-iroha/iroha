using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using Hyperledger.Iroha.SoraFs;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class SoraFsReferenceValidatorsTests
{
    [Fact]
    public void GovernanceReferenceConstantsMatchNativeAbi()
    {
        Assert.Equal(21u, SoraFsReferenceValidators.RequiredBridgeAbiVersion);
        Assert.Equal(-114, SoraFsReferenceValidators.BridgeReferenceError);
        Assert.Equal(67_108_864, SoraFsReferenceValidators.MaxInputBytesV1);
        Assert.Equal(1_024, SoraFsReferenceValidators.MaxLabelBytesV1);
        Assert.Equal(64, SoraFsReferenceValidators.GovernanceDagMaxBlocksV1);
        Assert.Equal(32, SoraFsReferenceValidators.GovernanceDagCidBytesV1);
        Assert.Equal(1, SoraFsReferenceValidators.ValidationOutcomeVersionV1);
    }

    [Fact]
    public void AvailabilityRequiresAbiAndBothGovernanceSymbols()
    {
        Assert.True(SoraFsReferenceValidators.IsAvailable(new FakeNativeBoundary()));
        Assert.False(SoraFsReferenceValidators.IsAvailable(
            new FakeNativeBoundary { Abi = 20 }));
        Assert.False(SoraFsReferenceValidators.IsAvailable(
            new FakeNativeBoundary { SymbolsAvailable = false }));
        Assert.False(SoraFsReferenceValidators.IsAvailable(
            new FakeNativeBoundary
            {
                AbiError = new DllNotFoundException("bridge missing"),
            }));
    }

    [Fact]
    public void OrderbookPdpAvailabilityRequiresAbiAndEverySymbol()
    {
        Assert.True(SoraFsReferenceValidators.IsOrderbookPdpAvailable(
            new FakeNativeBoundary()));
        Assert.False(SoraFsReferenceValidators.IsOrderbookPdpAvailable(
            new FakeNativeBoundary { Abi = 20 }));
        Assert.False(SoraFsReferenceValidators.IsOrderbookPdpAvailable(
            new FakeNativeBoundary { OrderbookPdpSymbolsAvailable = false }));
        Assert.False(SoraFsReferenceValidators.IsOrderbookPdpAvailable(
            new FakeNativeBoundary
            {
                AbiError = new DllNotFoundException("bridge missing"),
            }));
    }

    [Fact]
    public void OrderbookAndPdpValidationCopyInputsAndFreeOutputs()
    {
        var native = new FakeNativeBoundary();
        var order = new byte[] { 1, 2, 3 };
        var orderJson = SoraFsReferenceValidators.ValidateOrderbookPayloadJson(
            SoraFsOrderbookPayloadKind.OrderRequest,
            order,
            "order.to",
            123,
            native);

        Assert.Equal(1, native.OrderbookCalls);
        Assert.Equal((uint)SoraFsOrderbookPayloadKind.OrderRequest, native.LastOrderbookKind);
        Assert.NotSame(order, native.LastOrderbookBytes);
        Assert.Equal(order, native.LastOrderbookBytes!);
        Assert.Equal("order.to", Encoding.UTF8.GetString(native.LastOrderbookLabel!));
        Assert.Contains("\"generated_at\":123", orderJson, StringComparison.Ordinal);
        Assert.Equal(1, native.FreeCalls);

        var commitment = new byte[] { 4 };
        var challenge = new byte[] { 5 };
        var proof = new byte[] { 6 };
        var bundleJson = SoraFsReferenceValidators.ValidatePdpBundleJson(
            commitment,
            challenge,
            proof,
            "commitment.to",
            "challenge.to",
            "proof.to",
            456,
            native);

        Assert.Equal(1, native.PdpBundleCalls);
        Assert.NotSame(commitment, native.LastPdpCommitment);
        Assert.NotSame(challenge, native.LastPdpChallenge);
        Assert.NotSame(proof, native.LastPdpProof);
        Assert.Equal(commitment, native.LastPdpCommitment!);
        Assert.Equal(challenge, native.LastPdpChallenge!);
        Assert.Equal(proof, native.LastPdpProof!);
        Assert.Equal(
            "commitment.to",
            Encoding.UTF8.GetString(native.LastPdpCommitmentLabel!));
        Assert.Equal(
            "challenge.to",
            Encoding.UTF8.GetString(native.LastPdpChallengeLabel!));
        Assert.Equal("proof.to", Encoding.UTF8.GetString(native.LastPdpProofLabel!));
        Assert.Contains("\"generated_at\":456", bundleJson, StringComparison.Ordinal);
        Assert.Equal(2, native.FreeCalls);

        native.LastOrderbookBytes![0] = 0x7f;
        native.LastPdpCommitment![0] = 0x7f;
        native.LastPdpChallenge![0] = 0x7f;
        native.LastPdpProof![0] = 0x7f;
        Assert.Equal(1, order[0]);
        Assert.Equal(4, commitment[0]);
        Assert.Equal(5, challenge[0]);
        Assert.Equal(6, proof[0]);
    }

    [Fact]
    public void GovernanceBlockValidationCopiesInputsAndFreesOutput()
    {
        var native = new FakeNativeBoundary();
        var payload = new byte[] { 1, 2, 3 };
        var expectedCid = Enumerable.Repeat((byte)4, 32).ToArray();

        var json = SoraFsReferenceValidators.ValidateGovernanceDagBlockJson(
            payload,
            "治理-block.to",
            expectedCid,
            123,
            native);

        Assert.Equal(1, native.BlockCalls);
        Assert.NotSame(payload, native.LastBlockBytes);
        Assert.NotSame(expectedCid, native.LastExpectedCid);
        Assert.Equal(payload, native.LastBlockBytes!);
        Assert.Equal(expectedCid, native.LastExpectedCid!);
        Assert.Equal("治理-block.to", Encoding.UTF8.GetString(native.LastBlockLabel!));
        Assert.Equal(123ul, native.LastGeneratedAt);
        Assert.Equal(1, native.FreeCalls);
        Assert.Contains("\"generated_at\":123", json, StringComparison.Ordinal);

        native.LastBlockBytes![0] = 0x7f;
        native.LastExpectedCid![0] = 0x7f;
        Assert.Equal(1, payload[0]);
        Assert.Equal(4, expectedCid[0]);
    }

    [Fact]
    public void GovernanceHeadChainCopiesOrderedBlocksAndFreesOutput()
    {
        var native = new FakeNativeBoundary();
        var head = new byte[] { 9, 8 };
        var firstBytes = new byte[] { 1 };
        var secondBytes = new byte[] { 2 };
        var first = new SoraFsGovernanceDagBlockInput(firstBytes, "first.to");
        var second = new SoraFsGovernanceDagBlockInput(secondBytes, "second.to");

        _ = SoraFsReferenceValidators.ValidateGovernanceDagHeadChainJson(
            head,
            new[] { first, second },
            "head.to",
            456,
            native);

        Assert.Equal(1, native.HeadCalls);
        Assert.NotSame(head, native.LastHeadBytes);
        Assert.Equal(head, native.LastHeadBytes!);
        Assert.Equal("head.to", Encoding.UTF8.GetString(native.LastHeadLabel!));
        Assert.Collection(
            native.LastBlocks!,
            block =>
            {
                Assert.Equal(new byte[] { 1 }, block.Bytes);
                Assert.Equal("first.to", Encoding.UTF8.GetString(block.LabelBytes));
            },
            block =>
            {
                Assert.Equal(new byte[] { 2 }, block.Bytes);
                Assert.Equal("second.to", Encoding.UTF8.GetString(block.LabelBytes));
            });
        Assert.Equal(456ul, native.LastGeneratedAt);
        Assert.Equal(1, native.FreeCalls);

        native.LastHeadBytes![0] = 0x7f;
        native.LastBlocks![0].Bytes[0] = 0x7f;
        Assert.Equal(9, head[0]);
        Assert.Equal(1, firstBytes[0]);
    }

    [Fact]
    public void GovernanceHeadChainUsesIndexedDefaultBlockLabels()
    {
        var native = new FakeNativeBoundary();
        var blocks = new[]
        {
            new SoraFsGovernanceDagBlockInput(new byte[] { 1 }),
            new SoraFsGovernanceDagBlockInput(new byte[] { 2 }),
        };

        _ = SoraFsReferenceValidators.ValidateGovernanceDagHeadChainJson(
            new byte[] { 9 },
            blocks,
            null,
            456,
            native);

        Assert.Collection(
            native.LastBlocks!,
            block => Assert.Equal(
                "governance-dag-block-0.to",
                Encoding.UTF8.GetString(block.LabelBytes)),
            block => Assert.Equal(
                "governance-dag-block-1.to",
                Encoding.UTF8.GetString(block.LabelBytes)));
    }

    [Fact]
    public void GovernanceBlockInputSnapshotsConstructionAndAccess()
    {
        var source = new byte[] { 1, 2, 3 };
        var input = new SoraFsGovernanceDagBlockInput(source, "block.to");
        source[0] = 0x7f;

        var firstRead = input.NoritoBytes;
        Assert.Equal(new byte[] { 1, 2, 3 }, firstRead);
        firstRead[1] = 0x7f;
        Assert.Equal(new byte[] { 1, 2, 3 }, input.NoritoBytes);
    }

    [Fact]
    public void GeneratedAtAndAbiAreValidatedBeforeNativeDispatch()
    {
        var native = new FakeNativeBoundary();
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            SoraFsReferenceValidators.ValidateGovernanceDagBlockJson(
                new byte[] { 1 },
                "block.to",
                null,
                -1,
                native));
        Assert.Equal(0, native.BlockCalls);

        native.Abi = 20;
        var abiError = Assert.Throws<InvalidOperationException>(() =>
            SoraFsReferenceValidators.ValidateGovernanceDagBlockJson(
                new byte[] { 1 },
                "block.to",
                null,
                0,
                native));
        Assert.Contains("ABI 21", abiError.Message, StringComparison.Ordinal);
        Assert.Equal(0, native.BlockCalls);

        native.Abi = 21;
        native.SymbolsAvailable = false;
        var symbolError = Assert.Throws<InvalidOperationException>(() =>
            SoraFsReferenceValidators.ValidateGovernanceDagBlockJson(
                new byte[] { 1 },
                "block.to",
                null,
                0,
                native));
        Assert.Contains("Governance DAG reference symbols", symbolError.Message);
        Assert.Equal(0, native.BlockCalls);
    }

    [Fact]
    public void LabelsAreStrictUtf8AndBoundedBeforeNativeDispatch()
    {
        var native = new FakeNativeBoundary();
        foreach (var label in new[] { "", " ", " padded", "padded ", "bad\u0001label" })
        {
            Assert.Throws<ArgumentException>(() =>
                SoraFsReferenceValidators.ValidateGovernanceDagBlockJson(
                    new byte[] { 1 },
                    label,
                    null,
                    1,
                    native));
        }
        Assert.Throws<ArgumentException>(() =>
            SoraFsReferenceValidators.ValidateGovernanceDagBlockJson(
                new byte[] { 1 },
                "\ud800",
                null,
                1,
                native));
        Assert.Throws<ArgumentException>(() =>
            SoraFsReferenceValidators.ValidateGovernanceDagBlockJson(
                new byte[] { 1 },
                new string('a', SoraFsReferenceValidators.MaxLabelBytesV1 + 1),
                null,
                1,
                native));
        Assert.Equal(0, native.BlockCalls);
    }

    [Fact]
    public void HeadChainRejectsEmptyExcessAndNullBlocksBeforeNativeDispatch()
    {
        var native = new FakeNativeBoundary();
        Assert.Throws<ArgumentException>(() =>
            SoraFsReferenceValidators.ValidateGovernanceDagHeadChainJson(
                new byte[] { 1 },
                Array.Empty<SoraFsGovernanceDagBlockInput>(),
                "head.to",
                1,
                native));

        var excess = Enumerable.Range(
                0,
                SoraFsReferenceValidators.GovernanceDagMaxBlocksV1 + 1)
            .Select(index => new SoraFsGovernanceDagBlockInput(
                new byte[] { (byte)index },
                $"block-{index}.to"))
            .ToArray();
        Assert.Throws<ArgumentException>(() =>
            SoraFsReferenceValidators.ValidateGovernanceDagHeadChainJson(
                new byte[] { 1 },
                excess,
                "head.to",
                1,
                native));

        SoraFsGovernanceDagBlockInput[] withNull =
        [
            new SoraFsGovernanceDagBlockInput(new byte[] { 1 }),
            null!,
        ];
        Assert.Throws<ArgumentException>(() =>
            SoraFsReferenceValidators.ValidateGovernanceDagHeadChainJson(
                new byte[] { 1 },
                withNull,
                "head.to",
                1,
                native));
        Assert.Equal(0, native.HeadCalls);
    }

    [Fact]
    public void GovernanceBlockRejectsNonCanonicalExpectedCidLengths()
    {
        var native = new FakeNativeBoundary();
        foreach (var length in new[] { 0, 31, 33 })
        {
            var error = Assert.Throws<ArgumentException>(() =>
                SoraFsReferenceValidators.ValidateGovernanceDagBlockJson(
                    new byte[] { 1 },
                    "block.to",
                    new byte[length],
                    1,
                    native));
            Assert.Contains("exactly 32 bytes", error.Message);
        }
        Assert.Equal(0, native.BlockCalls);
    }

    [Fact]
    public void OrderbookAndPdpFixturesMatchExactNativeReferenceOutcomesWhenAvailable()
    {
        if (!SoraFsReferenceValidators.IsOrderbookPdpAvailable())
        {
            Assert.False(
                string.Equals(
                    Environment.GetEnvironmentVariable(
                        "IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION"),
                    "1",
                    StringComparison.Ordinal),
                "ABI-21 connect_norito_bridge with orderbook/PDP symbols is required.");
            return;
        }

        var orderbookRoot = Path.Combine(
            AppContext.BaseDirectory,
            "Fixtures",
            "sorafs_manifest",
            "orderbook");
        var orderOutcome = SoraFsReferenceValidators.ValidateOrderbookPayloadJson(
            SoraFsOrderbookPayloadKind.OrderRequest,
            File.ReadAllBytes(Path.Combine(orderbookRoot, "order_request_v1.to")),
            "order_request_v1.to",
            123);
        Assert.Equal(
            File.ReadAllText(
                Path.Combine(
                    orderbookRoot,
                    "order_request_validation_outcome_v1.json"),
                Encoding.UTF8),
            orderOutcome);

        foreach (var name in new[]
        {
            "order_request_bad_signature",
            "order_request_trailing_bytes",
        })
        {
            var outcome = SoraFsReferenceValidators.ValidateOrderbookPayloadJson(
                SoraFsOrderbookPayloadKind.OrderRequest,
                File.ReadAllBytes(
                    Path.Combine(orderbookRoot, "negative", $"{name}_v1.to")),
                $"{name}_v1.to",
                123);
            Assert.Equal(
                File.ReadAllText(
                    Path.Combine(
                        orderbookRoot,
                        "negative",
                        $"{name}_validation_outcome_v1.json"),
                    Encoding.UTF8),
                outcome);
        }

        var pdpRoot = Path.Combine(
            AppContext.BaseDirectory,
            "Fixtures",
            "sorafs_manifest",
            "pdp");
        var commitment = File.ReadAllBytes(Path.Combine(pdpRoot, "commitment_v1.to"));
        var challenge = File.ReadAllBytes(Path.Combine(pdpRoot, "challenge_v1.to"));
        var proof = File.ReadAllBytes(Path.Combine(pdpRoot, "proof_v1.to"));
        var commitmentChallenge =
            SoraFsReferenceValidators.ValidatePdpCommitmentChallengeJson(
                commitment,
                challenge,
                "commitment_v1.to",
                "challenge_v1.to",
                123);
        using (var outcome = JsonDocument.Parse(commitmentChallenge))
        {
            Assert.Equal("Ok", outcome.RootElement.GetProperty("status").GetString());
            Assert.Equal(
                "SFS-PDP-DIAG-000",
                outcome.RootElement.GetProperty("code").GetString());
        }
        var bundle = SoraFsReferenceValidators.ValidatePdpBundleJson(
            commitment,
            challenge,
            proof,
            "commitment_v1.to",
            "challenge_v1.to",
            "proof_v1.to",
            123);
        Assert.Equal(
            File.ReadAllText(
                Path.Combine(pdpRoot, "bundle_validation_outcome_v1.json"),
                Encoding.UTF8),
            bundle);

        foreach (var (name, kind) in new[]
        {
            ("duplicate_hot_leaf_challenge", SoraFsPdpPayloadKind.Challenge),
            ("missing_signature_proof", SoraFsPdpPayloadKind.Proof),
        })
        {
            var outcome = SoraFsReferenceValidators.ValidatePdpPayloadJson(
                kind,
                ReadPdpNegative(pdpRoot, name),
                $"{name}_v1.to",
                123);
            AssertPdpOutcome(pdpRoot, name, outcome);
        }

        foreach (var name in new[]
        {
            "late_proof",
            "wrong_manifest_proof",
            "wrong_provider_proof",
        })
        {
            var outcome = SoraFsReferenceValidators.ValidatePdpChallengeProofJson(
                challenge,
                ReadPdpNegative(pdpRoot, name),
                "challenge_v1.to",
                $"{name}_v1.to",
                123);
            AssertPdpOutcome(pdpRoot, name, outcome);
        }

        foreach (var name in new[]
        {
            "missing_hot_leaf_path_proof",
            "missing_segment_path_proof",
            "wrong_path_proof",
        })
        {
            var outcome = SoraFsReferenceValidators.ValidatePdpBundleJson(
                commitment,
                challenge,
                ReadPdpNegative(pdpRoot, name),
                "commitment_v1.to",
                "challenge_v1.to",
                $"{name}_v1.to",
                123);
            AssertPdpOutcome(pdpRoot, name, outcome);
        }
    }

    [Fact]
    public void GovernanceFixturesAndNegativeVectorsMatchNativeReferenceWhenAvailable()
    {
        if (!SoraFsReferenceValidators.IsAvailable())
        {
            Assert.False(
                string.Equals(
                    Environment.GetEnvironmentVariable(
                        "IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION"),
                    "1",
                    StringComparison.Ordinal),
                "ABI-21 connect_norito_bridge with Governance DAG symbols is required.");
            return;
        }

        var fixtureRoot = Path.Combine(
            AppContext.BaseDirectory,
            "Fixtures",
            "sorafs_manifest",
            "governance");
        var first = File.ReadAllBytes(Path.Combine(fixtureRoot, "dag_block_0_v1.to"));
        var second = File.ReadAllBytes(Path.Combine(fixtureRoot, "dag_block_1_v1.to"));
        var head = File.ReadAllBytes(Path.Combine(fixtureRoot, "dag_head_v1.to"));
        var blocks = new[]
        {
            new SoraFsGovernanceDagBlockInput(
                first,
                "dag_block_0_v1.to"),
            new SoraFsGovernanceDagBlockInput(
                second,
                "dag_block_1_v1.to"),
        };
        var expected = File.ReadAllText(
            Path.Combine(fixtureRoot, "dag_head_validation_outcome_v1.json"),
            Encoding.UTF8);

        var blockOutcome = SoraFsReferenceValidators.ValidateGovernanceDagBlockJson(
            first,
            "dag_block_0_v1.to",
            null,
            123);
        Assert.Equal(
            File.ReadAllText(
                Path.Combine(
                    fixtureRoot,
                    "dag_block_validation_outcome_v1.json"),
                Encoding.UTF8),
            blockOutcome);

        var cidMismatch = SoraFsReferenceValidators.ValidateGovernanceDagBlockJson(
            first,
            null,
            Enumerable.Repeat((byte)0x7f, 32).ToArray(),
            123);
        Assert.Equal(
            File.ReadAllText(
                Path.Combine(
                    fixtureRoot,
                    "dag_block_cid_mismatch_validation_outcome_v1.json"),
                Encoding.UTF8),
            cidMismatch);

        var headOutcome = SoraFsReferenceValidators.ValidateGovernanceDagHeadChainJson(
            head,
            blocks,
            "dag_head_v1.to",
            123);
        Assert.Equal(expected, headOutcome);

        var reordered = SoraFsReferenceValidators.ValidateGovernanceDagHeadChainJson(
            head,
            new[]
            {
                new SoraFsGovernanceDagBlockInput(second),
                new SoraFsGovernanceDagBlockInput(first),
            },
            null,
            123);
        Assert.Equal(
            File.ReadAllText(
                Path.Combine(
                    fixtureRoot,
                    "dag_head_reordered_validation_outcome_v1.json"),
                Encoding.UTF8),
            reordered);

        var blockSignatureOutcome =
            SoraFsReferenceValidators.ValidateGovernanceDagBlockJson(
                File.ReadAllBytes(
                    Path.Combine(
                        fixtureRoot,
                        "dag_block_bad_signature_v1.to")),
                "dag_block_bad_signature_v1.to",
                null,
                123);
        Assert.Equal(
            File.ReadAllText(
                Path.Combine(
                    fixtureRoot,
                    "dag_block_bad_signature_validation_outcome_v1.json"),
                Encoding.UTF8),
            blockSignatureOutcome);

        var trailingBytesOutcome =
            SoraFsReferenceValidators.ValidateGovernanceDagBlockJson(
                File.ReadAllBytes(
                    Path.Combine(
                        fixtureRoot,
                        "dag_block_trailing_bytes_v1.to")),
                "dag_block_trailing_bytes_v1.to",
                null,
                123);
        Assert.Equal(
            File.ReadAllText(
                Path.Combine(
                    fixtureRoot,
                    "dag_block_trailing_bytes_validation_outcome_v1.json"),
                Encoding.UTF8),
            trailingBytesOutcome);

        var headSignatureOutcome =
            SoraFsReferenceValidators.ValidateGovernanceDagHeadChainJson(
                File.ReadAllBytes(
                    Path.Combine(
                        fixtureRoot,
                        "dag_head_bad_signature_v1.to")),
                blocks,
                "dag_head_bad_signature_v1.to",
                123);
        Assert.Equal(
            File.ReadAllText(
                Path.Combine(
                    fixtureRoot,
                    "dag_head_bad_signature_validation_outcome_v1.json"),
                Encoding.UTF8),
            headSignatureOutcome);

        var predecessorOutcome =
            SoraFsReferenceValidators.ValidateGovernanceDagHeadChainJson(
                File.ReadAllBytes(
                    Path.Combine(
                        fixtureRoot,
                        "dag_head_bad_predecessor_v1.to")),
                new[]
                {
                    new SoraFsGovernanceDagBlockInput(
                        first,
                        "dag_block_0_v1.to"),
                    new SoraFsGovernanceDagBlockInput(
                        File.ReadAllBytes(
                            Path.Combine(
                                fixtureRoot,
                                "dag_block_1_bad_predecessor_v1.to")),
                        "dag_block_1_bad_predecessor_v1.to"),
                },
                "dag_head_bad_predecessor_v1.to",
                123);
        Assert.Equal(
            File.ReadAllText(
                Path.Combine(
                    fixtureRoot,
                    "dag_head_bad_predecessor_validation_outcome_v1.json"),
                Encoding.UTF8),
            predecessorOutcome);
    }

    [Fact]
    public void InvalidOutcomeJsonFailsClosedAndStillFreesOutput()
    {
        var native = new FakeNativeBoundary
        {
            OutputFactory = generatedAt => Encoding.UTF8.GetBytes(
                ValidOutcomeJson(generatedAt).Replace(
                    "\"generated_at\":" + generatedAt,
                    "\"generated_at\":" + generatedAt
                        + ",\"generated_at\":" + generatedAt,
                    StringComparison.Ordinal)),
        };

        var error = Assert.Throws<InvalidOperationException>(() =>
            SoraFsReferenceValidators.ValidateGovernanceDagBlockJson(
                new byte[] { 1 },
                "block.to",
                null,
                789,
                native));

        Assert.Contains("must not appear more than once", error.Message, StringComparison.Ordinal);
        Assert.Equal(1, native.FreeCalls);
    }

    [Fact]
    public void TimestampSubstitutionAndInvalidUtf8FailClosed()
    {
        var timestampNative = new FakeNativeBoundary
        {
            OutputFactory = _ => Encoding.UTF8.GetBytes(ValidOutcomeJson(999)),
        };
        var timestampError = Assert.Throws<InvalidOperationException>(() =>
            SoraFsReferenceValidators.ValidateGovernanceDagBlockJson(
                new byte[] { 1 },
                "block.to",
                null,
                1,
                timestampNative));
        Assert.Contains("generated_at", timestampError.Message, StringComparison.Ordinal);
        Assert.Equal(1, timestampNative.FreeCalls);

        var utf8Native = new FakeNativeBoundary
        {
            OutputFactory = _ => new byte[] { 0xff },
        };
        var utf8Error = Assert.Throws<InvalidOperationException>(() =>
            SoraFsReferenceValidators.ValidateGovernanceDagBlockJson(
                new byte[] { 1 },
                "block.to",
                null,
                1,
                utf8Native));
        Assert.Contains("invalid UTF-8", utf8Error.Message, StringComparison.Ordinal);
        Assert.Equal(1, utf8Native.FreeCalls);
    }

    [Fact]
    public void BridgeErrorsFreeAnyReturnedAllocation()
    {
        var native = new FakeNativeBoundary { ReturnCode = -114 };

        var error = Assert.Throws<InvalidOperationException>(() =>
            SoraFsReferenceValidators.ValidateGovernanceDagBlockJson(
                new byte[] { 1 },
                "block.to",
                null,
                1,
                native));

        Assert.Contains("-114", error.Message, StringComparison.Ordinal);
        Assert.Equal(1, native.FreeCalls);
    }

    private static byte[] ReadPdpNegative(string pdpRoot, string name)
    {
        return File.ReadAllBytes(
            Path.Combine(pdpRoot, "negative", $"{name}_v1.to"));
    }

    private static void AssertPdpOutcome(
        string pdpRoot,
        string name,
        string actual)
    {
        Assert.Equal(
            File.ReadAllText(
                Path.Combine(
                    pdpRoot,
                    "negative",
                    $"{name}_validation_outcome_v1.json"),
                Encoding.UTF8),
            actual);
    }

    private static string ValidOutcomeJson(ulong generatedAt)
    {
        var outcome = new Dictionary<string, object?>
        {
            ["status"] = "Ok",
            ["code"] = "SFS-OK-000",
            ["category"] = "validation",
            ["message"] = "governance DAG payload accepted",
            ["action"] = null,
            ["docs_url"] = "docs/portal/docs/sorafs/reference-sdk/errors.md",
            ["telemetry_tags"] = new[] { "sorafs.reference.governance_dag" },
            ["context"] = Array.Empty<object>(),
            ["inputs"] = new object[]
            {
                new Dictionary<string, object?>
                {
                    ["kind"] = "governance_dag_block",
                    ["path"] = "block.to",
                },
            },
            ["version"] = 1,
            ["generated_at"] = generatedAt,
        };
        return JsonSerializer.Serialize(outcome);
    }

    private sealed class FakeNativeBoundary : ISoraFsReferenceNativeBoundary
    {
        private readonly HashSet<IntPtr> allocations = new();

        internal uint Abi { get; set; } = 21;

        internal Exception? AbiError { get; set; }

        internal bool SymbolsAvailable { get; set; } = true;

        internal bool OrderbookPdpSymbolsAvailable { get; set; } = true;

        internal int ReturnCode { get; set; }

        internal Func<ulong, byte[]> OutputFactory { get; set; } =
            generatedAt => Encoding.UTF8.GetBytes(ValidOutcomeJson(generatedAt));

        internal int BlockCalls { get; private set; }

        internal int HeadCalls { get; private set; }

        internal int OrderbookCalls { get; private set; }

        internal int PdpBundleCalls { get; private set; }

        internal int FreeCalls { get; private set; }

        internal uint LastOrderbookKind { get; private set; }

        internal byte[]? LastOrderbookBytes { get; private set; }

        internal byte[]? LastOrderbookLabel { get; private set; }

        internal byte[]? LastPdpCommitment { get; private set; }

        internal byte[]? LastPdpCommitmentLabel { get; private set; }

        internal byte[]? LastPdpChallenge { get; private set; }

        internal byte[]? LastPdpChallengeLabel { get; private set; }

        internal byte[]? LastPdpProof { get; private set; }

        internal byte[]? LastPdpProofLabel { get; private set; }

        internal byte[]? LastBlockBytes { get; private set; }

        internal byte[]? LastBlockLabel { get; private set; }

        internal byte[]? LastExpectedCid { get; private set; }

        internal byte[]? LastHeadBytes { get; private set; }

        internal byte[]? LastHeadLabel { get; private set; }

        internal NativeGovernanceInput[]? LastBlocks { get; private set; }

        internal ulong LastGeneratedAt { get; private set; }

        public uint AbiVersion()
        {
            if (AbiError is not null)
            {
                throw AbiError;
            }
            return Abi;
        }

        public bool HasGovernanceDagSymbols()
        {
            return SymbolsAvailable;
        }

        public bool HasOrderbookPdpSymbols()
        {
            return OrderbookPdpSymbolsAvailable;
        }

        public NativeValidationResult ValidateOrderbookPayload(
            uint kind,
            byte[] bytes,
            byte[] label,
            ulong generatedAt)
        {
            OrderbookCalls++;
            LastOrderbookKind = kind;
            LastOrderbookBytes = bytes;
            LastOrderbookLabel = label;
            LastGeneratedAt = generatedAt;
            return AllocateResult(generatedAt);
        }

        public NativeValidationResult ValidatePdpPayload(
            uint kind,
            byte[] bytes,
            byte[] label,
            ulong generatedAt)
        {
            LastGeneratedAt = generatedAt;
            return AllocateResult(generatedAt);
        }

        public NativeValidationResult ValidatePdpCommitmentChallenge(
            byte[] commitment,
            byte[] commitmentLabel,
            byte[] challenge,
            byte[] challengeLabel,
            ulong generatedAt)
        {
            LastGeneratedAt = generatedAt;
            return AllocateResult(generatedAt);
        }

        public NativeValidationResult ValidatePdpChallengeProof(
            byte[] challenge,
            byte[] challengeLabel,
            byte[] proof,
            byte[] proofLabel,
            ulong generatedAt)
        {
            LastGeneratedAt = generatedAt;
            return AllocateResult(generatedAt);
        }

        public NativeValidationResult ValidatePdpBundle(
            byte[] commitment,
            byte[] commitmentLabel,
            byte[] challenge,
            byte[] challengeLabel,
            byte[] proof,
            byte[] proofLabel,
            ulong generatedAt)
        {
            PdpBundleCalls++;
            LastPdpCommitment = commitment;
            LastPdpCommitmentLabel = commitmentLabel;
            LastPdpChallenge = challenge;
            LastPdpChallengeLabel = challengeLabel;
            LastPdpProof = proof;
            LastPdpProofLabel = proofLabel;
            LastGeneratedAt = generatedAt;
            return AllocateResult(generatedAt);
        }

        public NativeValidationResult ValidateGovernanceDagBlock(
            byte[] bytes,
            byte[] label,
            byte[] expectedBlockCid,
            ulong generatedAt)
        {
            BlockCalls++;
            LastBlockBytes = bytes;
            LastBlockLabel = label;
            LastExpectedCid = expectedBlockCid;
            LastGeneratedAt = generatedAt;
            return AllocateResult(generatedAt);
        }

        public NativeValidationResult ValidateGovernanceDagHeadChain(
            byte[] head,
            byte[] headLabel,
            NativeGovernanceInput[] blocks,
            ulong generatedAt)
        {
            HeadCalls++;
            LastHeadBytes = head;
            LastHeadLabel = headLabel;
            LastBlocks = blocks;
            LastGeneratedAt = generatedAt;
            return AllocateResult(generatedAt);
        }

        public void Free(IntPtr pointer)
        {
            Assert.True(allocations.Remove(pointer), "output must be freed exactly once");
            Marshal.FreeHGlobal(pointer);
            FreeCalls++;
        }

        private NativeValidationResult AllocateResult(ulong generatedAt)
        {
            var output = OutputFactory(generatedAt);
            var pointer = Marshal.AllocHGlobal(output.Length);
            Marshal.Copy(output, 0, pointer, output.Length);
            allocations.Add(pointer);
            return new NativeValidationResult(
                ReturnCode,
                pointer,
                (UIntPtr)output.Length);
        }
    }
}
