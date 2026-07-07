using System.Buffers.Binary;
using Hyperledger.Iroha.Sccp;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class SccpBscMainnetTests
{
    private const string SampleOutboundMessageId =
        "0x8f67c559bee1d1dcda7a179a4c13c0e72d53d1b47460c5fc9a54c44c9c5426bb";
    private const string SampleOutboundPayloadHash =
        "0x9e34310f639da096c9f23435d9c0293d8174f29ebe8b0fdcb274a9b5e7b60141";
    private const string SampleOutboundCommitmentRoot =
        "0xbb0d0ca7500aa193a4934410197ddf47e582ba81225b0b42eec2f6c93566fa65";
    private const string SampleOutboundFinalityBlockHash =
        "0x5555555555555555555555555555555555555555555555555555555555555555";
    private const string SampleOutboundBundleHex =
        "01bb0d0ca7500aa193a4934410197ddf47e582ba81225b0b42eec2f6c93566fa"
        + "65460000000106020000008f67c559bee1d1dcda7a179a4c13c0e72d53d1b474"
        + "60c5fc9a54c44c9c5426bb9e34310f639da096c9f23435d9c0293d8174f29ebe"
        + "8b0fdcb274a9b5e7b6014104000000000000007e000000020100000000020000"
        + "000100000000000000000000000103000000786f72e803000000000000000000"
        + "0000000000010a000000616c69636540736f7261022a00000030783131313131"
        + "3131313131313131313131313131313131313131313131313131313131313131"
        + "313131010d00000074616972615f6273635f786f7203000000010203";

    private const string ExpectedSourceAdapterVerifierVkHash =
        "0x12536f25748a6520f10ebd42a7bcccd6ec181b9d53129795c8e186dc6e8b18cc";
    private const string ExpectedSourceVerifierMaterialHash =
        "0x1630e4d75e2676cc443e07b0477303240ae4cff13bdf9fe61725b4a9a4ee959a";
    private const string ExpectedSourceAdapterEngineDeploymentHash =
        "0x7d47ade779a5bddb3a5f283600af677db8605b75a00516a4328f3823ff28fb2d";

    private sealed class ExecutionProviderStub(
        object chainId,
        IReadOnlyDictionary<string, object?> receipt,
        IReadOnlyDictionary<string, object?> block) : IBscMainnetExecutionProvider
    {
        public List<string> Calls { get; } = [];

        public ValueTask<object?> RequestAsync(
            string method,
            IReadOnlyList<object?> parameters,
            CancellationToken cancellationToken = default)
        {
            Calls.Add(method);
            return method switch
            {
                "eth_chainId" => ValueTask.FromResult<object?>(chainId),
                "eth_getTransactionReceipt" => ReceiptResult(parameters),
                "eth_getBlockByHash" => BlockResult(parameters),
                _ => throw new ArgumentException($"unexpected method {method}", nameof(method)),
            };
        }

        private ValueTask<object?> ReceiptResult(IReadOnlyList<object?> parameters)
        {
            Assert.Single(parameters);
            Assert.Equal(receipt["transactionHash"], parameters[0]);
            return ValueTask.FromResult<object?>(receipt);
        }

        private ValueTask<object?> BlockResult(IReadOnlyList<object?> parameters)
        {
            Assert.Equal(2, parameters.Count);
            Assert.Equal(block["hash"], parameters[0]);
            Assert.False((bool)parameters[1]!);
            return ValueTask.FromResult<object?>(block);
        }
    }

    private sealed class InboundProverStub(
        string? expectedTransactionHash,
        byte[]? proofBytes = null,
        string? expectedReceiptProofHash = null,
        string? expectedSourceEventDigest = null,
        string? expectedSourceBridgeEmitterAddress = null) : IBscMainnetInboundProver
    {
        public int Calls { get; private set; }

        public ValueTask<byte[]> ProveAsync(
            BscMainnetInboundEvidence evidence,
            CancellationToken cancellationToken = default)
        {
            Calls += 1;
            Assert.Equal(BscMainnetSccp.DomainBsc, evidence.SourceDomain);
            Assert.Equal(BscMainnetSccp.DomainSora, evidence.TargetDomain);
            if (expectedTransactionHash is not null)
            {
                Assert.Equal(expectedTransactionHash, evidence.TransactionHash);
            }
            Assert.NotNull(evidence.ParliaFinality);
            Assert.NotNull(evidence.ReceiptProof);
            if (expectedReceiptProofHash is not null)
            {
                Assert.Equal(expectedReceiptProofHash, evidence.ReceiptProofHash);
            }
            if (expectedSourceEventDigest is not null)
            {
                Assert.Equal(expectedSourceEventDigest, evidence.SourceEventDigest);
                Assert.Equal(expectedSourceEventDigest, evidence.ReceiptProof.SourceEventDigest);
            }
            if (expectedSourceBridgeEmitterAddress is not null)
            {
                Assert.Equal(expectedSourceBridgeEmitterAddress, evidence.SourceBridgeEmitterAddress);
            }
            return ValueTask.FromResult(proofBytes ?? new byte[] { 1, 2, 3 });
        }
    }

    private sealed class ConsensusProviderStub(
        IReadOnlyDictionary<string, object?> finality) : IBscMainnetConsensusProvider
    {
        public ValueTask<IReadOnlyDictionary<string, object?>> CollectFinalityEvidenceAsync(
            IReadOnlyDictionary<string, object?>? receipt,
            IReadOnlyDictionary<string, object?>? block,
            string? transactionHash,
            CancellationToken cancellationToken = default)
        {
            Assert.NotNull(receipt);
            Assert.NotNull(block);
            Assert.NotNull(transactionHash);
            return ValueTask.FromResult(finality);
        }
    }

    private sealed class MutatingConsensusProviderStub(
        IReadOnlyDictionary<string, object?> expectedReceipt,
        IReadOnlyDictionary<string, object?> expectedBlock,
        IReadOnlyDictionary<string, object?> finality) : IBscMainnetConsensusProvider
    {
        public ValueTask<IReadOnlyDictionary<string, object?>> CollectFinalityEvidenceAsync(
            IReadOnlyDictionary<string, object?>? receipt,
            IReadOnlyDictionary<string, object?>? block,
            string? transactionHash,
            CancellationToken cancellationToken = default)
        {
            Assert.NotSame(expectedReceipt, receipt);
            Assert.NotSame(expectedBlock, block);
            Assert.Equal(expectedReceipt["transactionHash"], receipt?["transactionHash"]);
            Assert.Equal(expectedBlock["hash"], block?["hash"]);
            Assert.True(receipt is IDictionary<string, object?>);
            Assert.True(block is IDictionary<string, object?>);
            ((IDictionary<string, object?>)receipt!)["status"] = "0x0";
            MutateNestedReceiptSnapshot(receipt);
            ((IDictionary<string, object?>)block!)["receiptsRoot"] = "0x" + new string('e', 64);
            return ValueTask.FromResult(finality);
        }
    }

    private sealed class MutatingInboundProverStub(
        IReadOnlyDictionary<string, object?> originalReceipt,
        IReadOnlyDictionary<string, object?> originalBlock,
        IReadOnlyDictionary<string, object?> originalFinality,
        string expectedTransactionHash) : IBscMainnetInboundProver
    {
        public ValueTask<byte[]> ProveAsync(
            BscMainnetInboundEvidence evidence,
            CancellationToken cancellationToken = default)
        {
            Assert.NotSame(originalReceipt, evidence.Receipt);
            Assert.NotSame(originalBlock, evidence.Block);
            Assert.NotSame(originalFinality, evidence.ParliaFinality);
            Assert.Equal(expectedTransactionHash, evidence.TransactionHash);
            Assert.True(evidence.Receipt is IDictionary<string, object?>);
            Assert.True(evidence.Block is IDictionary<string, object?>);
            Assert.True(evidence.ParliaFinality is IDictionary<string, object?>);
            ((IDictionary<string, object?>)evidence.Receipt!)["status"] = "0x0";
            MutateNestedReceiptSnapshot(evidence.Receipt);
            ((IDictionary<string, object?>)evidence.Block!)["receiptsRoot"] =
                "0x" + new string('e', 64);
            ((IDictionary<string, object?>)evidence.ParliaFinality!)["executionBlockHash"] =
                "0x" + new string('e', 64);
            return ValueTask.FromResult(new byte[] { 1, 2, 3 });
        }
    }

    private static void MutateNestedReceiptSnapshot(
        IReadOnlyDictionary<string, object?>? receipt)
    {
        var logs = Assert.IsType<object?[]>(receipt?["logs"]);
        var metadata = Assert.IsAssignableFrom<IDictionary<string, object?>>(logs[0]);
        var topics = Assert.IsType<object?[]>(metadata["topics"]);

        metadata["address"] = "0x" + new string('e', 40);
        topics[0] = "0x" + new string('e', 64);
    }

    [Fact]
    public void InboundEvidenceDictionariesSnapshotInitAndAccessValues()
    {
        var receipt = MutableInboundEvidenceReceipt();
        var block = new Dictionary<string, object?>
        {
            ["hash"] = "0x" + new string('a', 64),
            ["receiptsRoot"] = "0x" + new string('b', 64),
        };
        var finality = new Dictionary<string, object?>
        {
            ["executionBlockHash"] = "0x" + new string('c', 64),
            ["executionBlockNumber"] = "0x2a",
        };
        var evidence = new BscMainnetInboundEvidence
        {
            Receipt = receipt,
            Block = block,
            ParliaFinality = finality,
        };

        receipt["status"] = "0x0";
        MutateNestedReceiptSnapshot(receipt);
        block["hash"] = "0x" + new string('d', 64);
        finality["executionBlockHash"] = "0x" + new string('d', 64);

        AssertOriginalInboundEvidence(evidence);

        var receiptAccess = Assert.IsAssignableFrom<IDictionary<string, object?>>(evidence.Receipt);
        receiptAccess["status"] = "0x0";
        MutateNestedReceiptSnapshot(Assert.IsAssignableFrom<IReadOnlyDictionary<string, object?>>(receiptAccess));
        var blockAccess = Assert.IsAssignableFrom<IDictionary<string, object?>>(evidence.Block);
        blockAccess["hash"] = "0x" + new string('e', 64);
        var finalityAccess = Assert.IsAssignableFrom<IDictionary<string, object?>>(evidence.ParliaFinality);
        finalityAccess["executionBlockHash"] = "0x" + new string('e', 64);

        AssertOriginalInboundEvidence(evidence);

        static Dictionary<string, object?> MutableInboundEvidenceReceipt()
        {
            return new Dictionary<string, object?>
            {
                ["transactionHash"] = "0x" + new string('1', 64),
                ["status"] = "0x1",
                ["logs"] = new object?[]
                {
                    new Dictionary<string, object?>
                    {
                        ["address"] = "0x" + new string('2', 40),
                        ["topics"] = new object?[] { "0x" + new string('3', 64) },
                    },
                },
            };
        }

        static void AssertOriginalInboundEvidence(BscMainnetInboundEvidence evidence)
        {
            var receipt = Assert.IsAssignableFrom<IReadOnlyDictionary<string, object?>>(evidence.Receipt);
            Assert.Equal("0x1", receipt["status"]);
            var logs = Assert.IsType<object?[]>(receipt["logs"]);
            var metadata = Assert.IsAssignableFrom<IReadOnlyDictionary<string, object?>>(logs[0]);
            Assert.Equal("0x" + new string('2', 40), metadata["address"]);
            var topics = Assert.IsType<object?[]>(metadata["topics"]);
            Assert.Equal("0x" + new string('3', 64), topics[0]);

            var block = Assert.IsAssignableFrom<IReadOnlyDictionary<string, object?>>(evidence.Block);
            Assert.Equal("0x" + new string('a', 64), block["hash"]);
            var finality = Assert.IsAssignableFrom<IReadOnlyDictionary<string, object?>>(evidence.ParliaFinality);
            Assert.Equal("0x" + new string('c', 64), finality["executionBlockHash"]);
        }
    }

    private sealed class InboundSubmitterStub : IBscMainnetInboundSubmitter
    {
        public ValueTask<object?> SubmitAsync(
            byte[] proofBytes,
            CancellationToken cancellationToken = default)
        {
            Assert.Equal(new byte[] { 1, 2, 3 }, proofBytes);
            return ValueTask.FromResult<object?>("submitted");
        }
    }

    private sealed class RecordingInboundSubmitterStub : IBscMainnetInboundSubmitter
    {
        public byte[]? SubmittedProofBytes { get; private set; }

        public ValueTask<object?> SubmitAsync(
            byte[] proofBytes,
            CancellationToken cancellationToken = default)
        {
            SubmittedProofBytes = proofBytes;
            return ValueTask.FromResult<object?>("submitted");
        }
    }

    private sealed class OutboundProverStub(byte[] proofBytes) : IBscMainnetOutboundProver
    {
        public BscMainnetOutboundProofRequest? Request { get; private set; }

        public ValueTask<byte[]> ProveAsync(
            BscMainnetOutboundProofRequest request,
            CancellationToken cancellationToken = default)
        {
            Request = request;
            Assert.Equal(BscMainnetSccp.DomainBsc, request.TargetDomain);
            Assert.Equal(BscMainnetSccp.DomainBsc, request.PublicInputs.TargetDomain);
            Assert.Equal(request.DestinationBinding.BindingHash, request.DestinationBindingHash);
            Assert.Equal(9, request.PublicSignalWords.Length);
            return ValueTask.FromResult(proofBytes);
        }
    }

    private sealed class MutatingOutboundProverStub(byte[] proofBytes) : IBscMainnetOutboundProver
    {
        public BscMainnetOutboundProofRequest? Request { get; private set; }

        public ValueTask<byte[]> ProveAsync(
            BscMainnetOutboundProofRequest request,
            CancellationToken cancellationToken = default)
        {
            Request = request;
            request.BundleBytes[0] = (byte)(request.BundleBytes[0] ^ 0xff);
            if (request.SourceProofBytes.Length > 0)
            {
                request.SourceProofBytes[0] = (byte)(request.SourceProofBytes[0] ^ 0xff);
            }
            request.PublicInputsBytes[0] = (byte)(request.PublicInputsBytes[0] ^ 0xff);
            if (request.PublicSignalWords is string[] publicSignalWords)
            {
                publicSignalWords[0] = "0x" + new string('9', 64);
            }

            return ValueTask.FromResult(proofBytes);
        }
    }

    private sealed class OutboundSubmitterStub : IBscMainnetOutboundSubmitter
    {
        public BscMainnetSccpSubmission? Submission { get; private set; }

        public ValueTask<object?> SubmitAsync(
            BscMainnetSccpSubmission submission,
            CancellationToken cancellationToken = default)
        {
            Submission = submission;
            return ValueTask.FromResult<object?>("bsc-submitted");
        }
    }

    private static BscMainnetSccpDestinationBinding SampleDestinationBinding()
        => BscMainnetSccp.DestinationBinding(
            "0x" + new string('1', 40),
            "0x" + new string('2', 40),
            "0x" + new string('b', 64),
            "0x" + new string('c', 64));

    private static BscMainnetSourceVerifierMaterialInput SampleSourceMaterial()
        => new(
            SourceTrustAnchorHash: "0x" + new string('4', 64),
            ConsensusVerifierHash: "0x" + new string('5', 64),
            MessageInclusionVerifierHash: "0x" + new string('6', 64),
            FinalityPolicyHash: "0x" + new string('8', 64),
            BridgeAddress: "0x" + new string('1', 40),
            SourceBridgeEmitterCodeHash: "0x" + new string('7', 64));

    private static BscMainnetSourceAdapterDeploymentInput SampleSourceAdapterDeployment()
    {
        var material = SampleSourceMaterial();
        return new(
            SourceTrustAnchorHash: material.SourceTrustAnchorHash,
            ConsensusVerifierHash: material.ConsensusVerifierHash,
            MessageInclusionVerifierHash: material.MessageInclusionVerifierHash,
            FinalityPolicyHash: material.FinalityPolicyHash,
            BridgeAddress: material.BridgeAddress,
            SourceBridgeEmitterCodeHash: material.SourceBridgeEmitterCodeHash,
            DeploymentReceiptHash: "0x" + new string('a', 64));
    }

    private static BscMainnetTransparentPublicInputs SamplePublicInputs()
        => new(
            Version: 1,
            MessageId: SampleOutboundMessageId,
            PayloadHash: SampleOutboundPayloadHash,
            TargetDomain: BscMainnetSccp.DomainBsc,
            CommitmentRoot: SampleOutboundCommitmentRoot,
            FinalityHeight: 42,
            FinalityBlockHash: SampleOutboundFinalityBlockHash);

    private static string UpperFixedHex(string value)
        => "0X" + value[2..].ToUpperInvariant();

    private static BscMainnetOutboundProofRequestInput SampleOutboundInput(
        BscMainnetSccpDestinationBinding? binding = null,
        BscMainnetTransparentPublicInputs? publicInputs = null)
    {
        var selectedBinding = binding ?? SampleDestinationBinding();
        return new BscMainnetOutboundProofRequestInput
        {
            PublicInputs = publicInputs ?? SamplePublicInputs(),
            BundleBytes = SampleOutboundBundleBytes(),
            SourceProofBytes = [],
            StatementHash = "0x" + new string('5', 64),
            DestinationBinding = selectedBinding,
            DestinationBindingHash = selectedBinding.BindingHash,
            SourceDomain = BscMainnetSccp.DomainSora,
        };
    }

    private static byte[] Groth16ProofBytes()
        => Concat(
            AbiWord(1),
            HexWord(SampleOutboundMessageId[2..]),
            AbiWord((ulong)BscMainnetSccp.DomainSora),
            HexWord(SampleOutboundCommitmentRoot[2..]),
            AbiWord(1),
            AbiWord(2),
            HexWord("1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed"),
            HexWord("198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2"),
            HexWord("12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa"),
            HexWord("090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b"),
            AbiWord(1),
            AbiWord(2));

    private static byte[] AbiWord(ulong value)
    {
        var bytes = new byte[32];
        BinaryPrimitives.WriteUInt64BigEndian(bytes.AsSpan(24, 8), value);
        return bytes;
    }

    private static byte[] HexWord(string hex)
        => Convert.FromHexString(hex);

    private static byte[] SampleOutboundBundleBytes()
        => Convert.FromHexString(SampleOutboundBundleHex);

    private static byte[] RepeatByte(byte value, int count)
    {
        var bytes = new byte[count];
        Array.Fill(bytes, value);
        return bytes;
    }

    private static byte[] Concat(params byte[][] chunks)
    {
        var output = new byte[chunks.Sum(static chunk => chunk.Length)];
        var offset = 0;
        foreach (var chunk in chunks)
        {
            chunk.CopyTo(output.AsSpan(offset));
            offset += chunk.Length;
        }

        return output;
    }

    [Fact]
    public void MainnetGuardsAcceptBscAndRejectOtherRoutes()
    {
        BscMainnetSccp.RequireMainnetChainId(56);
        BscMainnetSccp.RequireMainnetNetworkId(BscMainnetSccp.MainnetNetworkId);
        BscMainnetSccp.RequireInboundRoute(
            BscMainnetSccp.DomainBsc,
            BscMainnetSccp.DomainSora);
        BscMainnetSccp.RequireOutboundRoute(
            BscMainnetSccp.DomainSora,
            BscMainnetSccp.DomainBsc);

        var binding = BscMainnetSccp.DestinationBinding(
            "0x" + new string('1', 40),
            "0x" + new string('2', 40),
            "0x" + new string('b', 64),
            "0x" + new string('c', 64));
        Assert.Equal(1, binding.Version);
        Assert.Equal(BscMainnetSccp.DomainSora, binding.SourceDomain);
        Assert.Equal(BscMainnetSccp.DomainBsc, binding.TargetDomain);
        Assert.Equal(BscMainnetSccp.MainnetNetworkId, binding.NetworkId);
        Assert.Equal(BscMainnetSccp.EvmGroth16Bn254ProofBackend, binding.VerifierBackend);
        Assert.Equal(BscMainnetSccp.StarkFriProofFamily, binding.ProofFamily);
        Assert.Equal(
            "evm:0:2:0000000000000000000000000000000000000000000000000000000000000038:"
                + "0x1111111111111111111111111111111111111111:"
                + "0x2222222222222222222222222222222222222222:"
                + "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:"
                + "0xcccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            binding.Key);
        Assert.Equal(
            "0x5e97d6da2b4ca7d64171ae717cfa31340a736c125485812a7cb9641570bc27d6",
            binding.BindingHash);
        Assert.Equal(
            binding.BindingHash,
            BscMainnetSccp.DestinationBindingHash(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64)));
        Assert.Equal(
            binding.BindingHash,
            BscMainnetSccp.DestinationBinding(
                "0X" + new string('1', 40).ToUpperInvariant(),
                "0X" + new string('2', 40).ToUpperInvariant(),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64),
                expectedBindingHash: binding.BindingHash,
                expectedKey: binding.Key).BindingHash);
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0X" + new string('b', 64).ToUpperInvariant(),
                "0x" + new string('c', 64)));
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0X" + new string('c', 64).ToUpperInvariant()));

        Assert.Throws<ArgumentOutOfRangeException>(
            () => BscMainnetSccp.RequireMainnetChainId(1));
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.RequireMainnetNetworkId("0x38"));
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.RequireMainnetNetworkId(
                "0X0000000000000000000000000000000000000000000000000000000000000038"));
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.RequireMainnetNetworkId("0x0000000000000000000000000000000000000000000000000000000000000039"));
        Assert.Throws<ArgumentException>(() => BscMainnetSccp.RequireInboundRoute(1, 0));
        Assert.Throws<ArgumentException>(() => BscMainnetSccp.RequireOutboundRoute(0, 1));
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64),
                networkId: "0x0000000000000000000000000000000000000000000000000000000000000001"));
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('1', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64)));
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.DestinationBinding(
                "0x" + new string('0', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64)));
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64),
                targetDomain: EthereumMainnetSccp.DomainEthereum));
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64),
                expectedBindingHash: "0x" + new string('9', 64)));
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64),
                expectedKey: binding.Key + "-wrong"));
        foreach (var paddedExpectedKey in new[]
        {
            " " + binding.Key,
            binding.Key + " ",
            binding.Key + "\n",
        })
        {
            Assert.Throws<ArgumentException>(
                () => BscMainnetSccp.DestinationBinding(
                    "0x" + new string('1', 40),
                    "0x" + new string('2', 40),
                    "0x" + new string('b', 64),
                    "0x" + new string('c', 64),
                    expectedKey: paddedExpectedKey));
        }
    }

    [Fact]
    public void SourceMaterialHashesMatchSharedBscVectors()
    {
        var material = SampleSourceMaterial();
        var deployment = SampleSourceAdapterDeployment();

        Assert.Equal(
            ExpectedSourceAdapterVerifierVkHash,
            BscMainnetSccp.SourceAdapterVerifierVkHash());
        Assert.NotEmpty(BscMainnetSccp.CanonicalSourceVerifierMaterialBytes(material));
        Assert.Equal(
            ExpectedSourceVerifierMaterialHash,
            BscMainnetSccp.SourceVerifierMaterialHash(material));
        Assert.NotEmpty(BscMainnetSccp.CanonicalSourceAdapterEngineDeploymentBytes(deployment));
        Assert.Equal(
            ExpectedSourceAdapterEngineDeploymentHash,
            BscMainnetSccp.SourceAdapterEngineDeploymentHash(deployment));

        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.SourceVerifierMaterialHash(material with
            {
                SourceDomain = EthereumMainnetSccp.DomainEthereum,
            }));
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.SourceAdapterVerifierVkHash(
                sourceDomain: EthereumMainnetSccp.DomainEthereum));
        var reusedSourceRole = Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.SourceVerifierMaterialHash(material with
            {
                ConsensusVerifierHash = material.SourceTrustAnchorHash,
            }));
        Assert.Contains("role-separated", reusedSourceRole.Message);
        var nonCanonicalAdapterVerifier = Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.SourceAdapterEngineDeploymentHash(deployment with
            {
                AdapterVerifierVkHash = "0x" + new string('9', 64),
            }));
        Assert.Contains("canonical BSC source-adapter verifier profile", nonCanonicalAdapterVerifier.Message);

        var replayedDeploymentReceipt = Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.SourceAdapterEngineDeploymentHash(deployment with
            {
                DeploymentReceiptHash = ExpectedSourceAdapterVerifierVkHash,
            }));
        Assert.Contains("role-separated", replayedDeploymentReceipt.Message);
        Assert.Contains(
            nameof(BscMainnetSourceAdapterDeploymentInput.DeploymentReceiptHash),
            replayedDeploymentReceipt.Message);
    }

    [Fact]
    public void ReceiptProofByteListsSnapshotConstructorAndGetterValues()
    {
        var node = new byte[] { 0x01, 0x02 };
        var branch = Enumerable.Repeat((byte)0x11, 32).ToArray();
        var proof = new BscMainnetReceiptProof
        {
            ReceiptTrieProofNodes = [node],
            InclusionBranch = [branch],
        };

        node[0] = 0x7f;
        branch[0] = 0x7e;
        var detachedNodes = proof.ReceiptTrieProofNodes;
        var detachedBranch = proof.InclusionBranch;
        detachedNodes[0][1] = 0x7d;
        detachedBranch[0][1] = 0x7c;

        Assert.Equal(new byte[] { 0x01, 0x02 }, proof.ReceiptTrieProofNodes[0]);
        Assert.Equal(Enumerable.Repeat((byte)0x11, 32).ToArray(), proof.InclusionBranch[0]);
        Assert.NotSame(node, proof.ReceiptTrieProofNodes[0]);
        Assert.NotSame(branch, proof.InclusionBranch[0]);
    }

    [Fact]
    public void BscValidatorSetPayloadHelpersRejectMalformedMaterial()
    {
        var validatorAddresses = new[]
        {
            "0x" + new string('1', 40),
            "0x" + new string('2', 40),
        };
        var validatorPowers = new ulong[] { 1, 2 };
        var expectedPayload = Convert.FromHexString(
            "0102000000"
            + new string('1', 40)
            + "0100000000000000"
            + new string('2', 40)
            + "0200000000000000");

        var payload = BscMainnetSccp.CanonicalBscValidatorSetPayloadBytes(
            validatorAddresses,
            validatorPowers);

        Assert.Equal(expectedPayload, payload);
        Assert.Equal(
            "0xdc6190956bc147c9a0a2fbf1384d40a1deb4b211a709f229275d1ea5ac3f8370",
            BscMainnetSccp.BscValidatorSetPayloadHash(payload));
        Assert.Equal(
            "0xdc6190956bc147c9a0a2fbf1384d40a1deb4b211a709f229275d1ea5ac3f8370",
            BscMainnetSccp.BscValidatorSetPayloadHash(
                validatorAddresses,
                validatorPowers));
        Assert.Equal(
            "0x3ef5ecfb6dc4f5fc9e970cc18cd72164495c827e96f77851813973a286f5c762",
            BscMainnetSccp.BscValidatorSetHashFromPayload(payload));
        Assert.Equal(
            "0x3ef5ecfb6dc4f5fc9e970cc18cd72164495c827e96f77851813973a286f5c762",
            BscMainnetSccp.BscValidatorSetHashFromPayload(
                validatorAddresses,
                validatorPowers));

        AssertArgumentContains(
            () => BscMainnetSccp.CanonicalBscValidatorSetPayloadBytes(
                validatorAddresses,
                new ulong[] { 1 }),
            "non-empty equal-length arrays");
        AssertArgumentContains(
            () => BscMainnetSccp.CanonicalBscValidatorSetPayloadBytes(
                Array.Empty<string>(),
                Array.Empty<ulong>()),
            "non-empty equal-length arrays");
        AssertArgumentContains(
            () => BscMainnetSccp.CanonicalBscValidatorSetPayloadBytes(
                Enumerable.Repeat("0x" + new string('1', 40), 256).ToArray(),
                Enumerable.Repeat(1UL, 256).ToArray()),
            "at most 255 entries");
        AssertArgumentContains(
            () => BscMainnetSccp.CanonicalBscValidatorSetPayloadBytes(
                new[] { "0X" + new string('1', 40) },
                new ulong[] { 1 }),
            "validatorAddresses[0]",
            "canonical lowercase 0x hex");
        AssertArgumentContains(
            () => BscMainnetSccp.CanonicalBscValidatorSetPayloadBytes(
                new[] { "0x" + new string('0', 40) },
                new ulong[] { 1 }),
            "validatorAddresses[0]",
            "must not be zero");
        AssertArgumentContains(
            () => BscMainnetSccp.CanonicalBscValidatorSetPayloadBytes(
                new[] { "0x" + new string('1', 40), "0x" + new string('1', 40) },
                new ulong[] { 1, 2 }),
            "validatorAddresses[1]",
            "unique");
        AssertArgumentContains(
            () => BscMainnetSccp.CanonicalBscValidatorSetPayloadBytes(
                new[] { "0x" + new string('1', 40) },
                new ulong[] { 0 }),
            "validatorPowers[0]",
            "must not be zero");

        var duplicatePayload = payload.ToArray();
        Array.Copy(duplicatePayload, 5, duplicatePayload, 5 + 28, 20);
        AssertArgumentContains(
            () => BscMainnetSccp.BscValidatorSetHashFromPayload(duplicatePayload),
            "validatorAddresses[1]",
            "unique");

        var zeroPowerPayload = payload.ToArray();
        Array.Clear(zeroPowerPayload, 5 + 20, 8);
        AssertArgumentContains(
            () => BscMainnetSccp.BscValidatorSetHashFromPayload(zeroPowerPayload),
            "validatorPowers[0]",
            "must not be zero");

        AssertArgumentContains(
            () => BscMainnetSccp.BscValidatorSetHashFromPayload(new byte[] { 0 }),
            "version 1");
        AssertArgumentContains(
            () => BscMainnetSccp.BscValidatorSetHashFromPayload(
                payload.Concat(new byte[] { 0 }).ToArray()),
            "invalid validator count");

        static void AssertArgumentContains(Action action, params string[] snippets)
        {
            var error = Assert.Throws<ArgumentException>(action);
            foreach (var snippet in snippets)
            {
                Assert.Contains(snippet, error.Message);
            }
        }
    }

    [Fact]
    public void BscCommitSealHelpersValidateSignersAndQuorum()
    {
        var seal = SampleCommitSealProof();

        Assert.Equal(297, BscMainnetSccp.CanonicalBscCommitSealBytes(seal).Length);
        Assert.Equal(
            "0x14659b4643d3a7961f7f86f46319992444617392c8e84967a3bb2a5ad7bc72fb",
            BscMainnetSccp.BscCommitSealHash(seal));

        foreach (var recoveryId in new byte[] { 0, 1, 29, 30 })
        {
            var badRecoveryId = MutatedSignature(0, 64, recoveryId);
            AssertArgumentContains(
                () => BscMainnetSccp.CanonicalBscCommitSealBytes(seal with
                {
                    Signatures =
                    [
                        badRecoveryId,
                        seal.Signatures[1],
                        seal.Signatures[2],
                    ],
                }),
                "canonical recoverable");
        }

        var wrongSigner = MutatedSignature(0, 0, 0x31);
        AssertArgumentContains(
            () => BscMainnetSccp.CanonicalBscCommitSealBytes(seal with
            {
                Signatures =
                [
                    wrongSigner,
                    seal.Signatures[1],
                    seal.Signatures[2],
                ],
            }),
            "recover the selected validator address");

        AssertArgumentContains(
            () => BscMainnetSccp.CanonicalBscCommitSealBytes(seal with
            {
                SignedPower = 2,
                SignersBitmap = HexBytes("03"),
                Signatures = seal.Signatures.Take(2).ToArray(),
            }),
            "two thirds");
        AssertArgumentContains(
            () => BscMainnetSccp.CanonicalBscCommitSealBytes(seal with
            {
                SignersBitmap = HexBytes("1f"),
            }),
            "padding bits");
        AssertArgumentContains(
            () => BscMainnetSccp.CanonicalBscCommitSealBytes(seal with
            {
                ValidatorSetHash = "0x" + new string('a', 64),
            }),
            "validatorSetHash");
        AssertArgumentContains(
            () => BscMainnetSccp.CanonicalBscCommitSealBytes(seal with
            {
                TotalPower = 5,
            }),
            "totalPower");
        AssertArgumentContains(
            () => BscMainnetSccp.CanonicalBscCommitSealBytes(seal with
            {
                SignedPower = 4,
            }),
            "signedPower");
        AssertArgumentContains(
            () => BscMainnetSccp.CanonicalBscCommitSealBytes(seal with
            {
                ValidatorPublicKeys =
                [
                    seal.ValidatorPublicKeys[0],
                    seal.ValidatorPublicKeys[0],
                    seal.ValidatorPublicKeys[2],
                    seal.ValidatorPublicKeys[3],
                ],
            }),
            "validatorPublicKeys[1]",
            "unique");
        AssertArgumentContains(
            () => BscMainnetSccp.CanonicalBscCommitSealBytes(seal with
            {
                ValidatorPowers = new ulong[] { 0, 1, 1, 1 },
            }),
            "validatorPowers[0]",
            "must not be zero");
        AssertArgumentContains(
            () => BscMainnetSccp.CanonicalBscCommitSealBytes(seal with
            {
                ValidatorPublicKeys =
                [
                    new byte[] { 0x05 },
                    seal.ValidatorPublicKeys[1],
                    seal.ValidatorPublicKeys[2],
                    seal.ValidatorPublicKeys[3],
                ],
            }),
            "compressed or uncompressed");

        static BscMainnetCommitSealProof SampleCommitSealProof()
        {
            var validatorPublicKeys = new[]
            {
                HexBytes("0x0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"),
                HexBytes("0x02c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5"),
                HexBytes("0x02f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"),
                HexBytes("0x02e493dbf1c10d80f3581e4904930b1404cc6c13900ee0758474fa94abe8c4cd13"),
            };
            var signatures = new[]
            {
                HexBytes(
                    "0x1b8802069b82c3d4cb6d7bec82323853f36d965c1e71647560084e7c7a0de9c"
                    + "17c85fcc3c6222f905cbbc4ba5b5f3f005f07d144304184181be67b3d02d1ba9f1b"),
                HexBytes(
                    "0x921d39c29fb793c496f96cf647128232d228024ed2f3e68cc6a52aa4cf64fac"
                    + "f6bbd9dfcf7d703165f7880e7e1310f34d1b0fb8ca6dd8f506bf289ba012387f01c"),
                HexBytes(
                    "0xcfa11aa1ec214278afdb4ef7f3c40af97a2784e0336afb5ebef345c0d2eaa9"
                    + "ef629ad2d25cf9709eb9b842fb2fb3f749ce365af97af6e7064771614312d361961b"),
            };
            return new BscMainnetCommitSealProof(
                Version: 1,
                TotalPower: 4,
                SignedPower: 3,
                CommitMessageHash:
                    "0x5832165d1a87ed49a323f2ecaecbef973489aed1a42e7eab369244e7abec43c7",
                ValidatorPublicKeys: validatorPublicKeys,
                ValidatorPowers: new ulong[] { 1, 1, 1, 1 },
                SignersBitmap: HexBytes("0x07"),
                Signatures: signatures,
                ValidatorSetHash:
                    "0xc5152802f6ca9ec72a4249646aca7476496f00b71ab5b1482c881a31fb42dd8c");
        }

        static byte[] MutatedSignature(int signatureIndex, int byteIndex, byte value)
        {
            var signature = SampleCommitSealProof().Signatures[signatureIndex].ToArray();
            signature[byteIndex] = value;
            return signature;
        }

        static byte[] HexBytes(string value)
        {
            var hex = value.StartsWith("0x", StringComparison.Ordinal) ? value[2..] : value;
            return Convert.FromHexString(hex);
        }

        static void AssertArgumentContains(Action action, params string[] snippets)
        {
            var error = Assert.Throws<ArgumentException>(action);
            foreach (var snippet in snippets)
            {
                Assert.Contains(snippet, error.Message);
            }
        }
    }

    [Fact]
    public void LocalAdmissionSubmissionWrapsNativeBscOutput()
    {
        var proofBytes = new byte[] { 1, 2, 3 };
        var publicInputsBytes = new byte[] { 4, 5, 6 };
        var bundleBytes = new byte[] { 7, 8, 9 };
        var envelopeBytes = new byte[] { 10, 11, 12 };
        var input = new BscMainnetLocalAdmissionSubmissionInput(
            ProofBytes: proofBytes,
            PublicInputsBytes: publicInputsBytes,
            BundleBytes: bundleBytes,
            EnvelopeBytes: envelopeBytes,
            StatementHash: "0x" + new string('6', 64),
            SourceVerifierMaterialHash: "0x" + new string('7', 64),
            SourceAdapterEngineDeploymentHash: "0x" + new string('8', 64));
        var submission = BscMainnetSccp.BuildLocalAdmissionSubmission(input);

        Assert.Equal(BscMainnetSccp.LocalAdmissionSubmissionKind, submission.PlatformPayload);
        Assert.Equal(BscMainnetSccp.LocalAdmissionEnvelopeEncoding, submission.EnvelopeEncoding);
        Assert.Equal(BscMainnetSccp.LocalAdmissionEntrypoint, submission.VerifierEntrypoint);
        Assert.Equal(BscMainnetSccp.DomainBsc, submission.SourceDomain);
        Assert.Equal(BscMainnetSccp.DomainSora, submission.TargetDomain);
        Assert.Empty(submission.Arguments);
        Assert.Equal([1, 2, 3], submission.ProofBytes);
        Assert.Equal([4, 5, 6], submission.PublicInputsBytes);
        Assert.Equal([7, 8, 9], submission.BundleBytes);
        Assert.Equal([10, 11, 12], submission.EnvelopeBytes);
        Assert.Equal([1, 2, 3], submission.LocalAdmission.ProofBytes);
        Assert.Equal("0x0a0b0c", submission.EnvelopeHex);

        proofBytes[0] = 99;
        publicInputsBytes[0] = 99;
        bundleBytes[0] = 99;
        envelopeBytes[0] = 99;
        input.ProofBytes[0] = 98;
        input.PublicInputsBytes[0] = 98;
        input.BundleBytes[0] = 98;
        input.EnvelopeBytes[0] = 98;
        submission.ProofBytes[0] = 97;
        submission.PublicInputsBytes[0] = 97;
        submission.BundleBytes[0] = 97;
        submission.EnvelopeBytes[0] = 97;
        submission.LocalAdmission.ProofBytes[0] = 96;
        submission.LocalAdmission.PublicInputsBytes[0] = 96;
        submission.LocalAdmission.BundleBytes[0] = 96;
        Assert.Equal([1, 2, 3], input.ProofBytes);
        Assert.Equal([4, 5, 6], input.PublicInputsBytes);
        Assert.Equal([7, 8, 9], input.BundleBytes);
        Assert.Equal([10, 11, 12], input.EnvelopeBytes);
        Assert.Equal([1, 2, 3], submission.ProofBytes);
        Assert.Equal([4, 5, 6], submission.PublicInputsBytes);
        Assert.Equal([7, 8, 9], submission.BundleBytes);
        Assert.Equal([10, 11, 12], submission.EnvelopeBytes);
        Assert.Equal([1, 2, 3], submission.LocalAdmission.ProofBytes);
        Assert.Equal([4, 5, 6], submission.LocalAdmission.PublicInputsBytes);
        Assert.Equal([7, 8, 9], submission.LocalAdmission.BundleBytes);

        var updatedPayload = submission.LocalAdmission with
        {
            ProofBytes = [0xaa],
            PublicInputsBytes = [0xbb, 0xcc],
            BundleBytes = [0xdd, 0xee, 0xff],
        };
        var updatedSubmission = submission with
        {
            ProofBytes = [0x11],
            PublicInputsBytes = [0x22, 0x33],
            BundleBytes = [0x44, 0x55, 0x66],
            EnvelopeBytes = [0x77, 0x88],
        };
        Assert.Equal("0xaa", updatedPayload.ProofBytesHex);
        Assert.Equal("0xbbcc", updatedPayload.PublicInputsBytesHex);
        Assert.Equal("0xddeeff", updatedPayload.BundleBytesHex);
        Assert.Equal("0x11", updatedSubmission.ProofBytesHex);
        Assert.Equal("0x2233", updatedSubmission.PublicInputsBytesHex);
        Assert.Equal("0x445566", updatedSubmission.BundleBytesHex);
        Assert.Equal("0x7788", updatedSubmission.EnvelopeHex);

        Assert.Throws<ArgumentNullException>(
            () => new BscMainnetLocalAdmissionSubmissionInput(
                ProofBytes: null!,
                PublicInputsBytes: publicInputsBytes,
                BundleBytes: bundleBytes,
                EnvelopeBytes: envelopeBytes,
                StatementHash: input.StatementHash,
                SourceVerifierMaterialHash: input.SourceVerifierMaterialHash,
                SourceAdapterEngineDeploymentHash: input.SourceAdapterEngineDeploymentHash));
        Assert.Throws<ArgumentNullException>(() => input with { EnvelopeBytes = null! });
        Assert.Throws<ArgumentNullException>(
            () => new BscMainnetLocalAdmissionPayload(
                ProofBytes: null!,
                PublicInputsBytes: publicInputsBytes,
                BundleBytes: bundleBytes,
                StatementHash: input.StatementHash,
                SourceVerifierMaterialHash: input.SourceVerifierMaterialHash,
                SourceAdapterEngineDeploymentHash: input.SourceAdapterEngineDeploymentHash));
        Assert.Throws<ArgumentNullException>(() => updatedPayload with { BundleBytes = null! });
        Assert.Throws<ArgumentNullException>(
            () => new BscMainnetLocalAdmissionSubmission(
                submission.ProofFamily,
                submission.VerifierBackend,
                submission.SourceDomain,
                submission.TargetDomain,
                submission.StatementHash,
                submission.SourceVerifierMaterialHash,
                submission.SourceAdapterEngineDeploymentHash,
                submission.LocalAdmission,
                ProofBytes: null!,
                PublicInputsBytes: publicInputsBytes,
                BundleBytes: bundleBytes,
                EnvelopeBytes: envelopeBytes));
        Assert.Throws<ArgumentNullException>(() => updatedSubmission with { EnvelopeBytes = null! });

        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                SourceDomain = EthereumMainnetSccp.DomainEthereum,
            }));
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                ProofBytes = [0, 0],
            }));
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                EnvelopeBytes = [],
            }));
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                EnvelopeEncoding = "abi_tuple_v1",
            }));
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                ProofFamily = "debug-proof-family",
            }));
    }

    [Fact]
    public async Task InboundEvidenceUsesMainnetRpcAndRejectsDrift()
    {
        var txHash = "0x" + new string('a', 64);
        var blockHash = "0x" + new string('b', 64);
        var receiptsRoot = "0x" + new string('c', 64);
        var sourceEventDigest = "0x" + new string('e', 64);
        var sourceBridgeEmitterAddress = "0x" + new string('4', 40);
        Dictionary<string, object?> SourceEventLog(params KeyValuePair<string, object?>[] overrides)
        {
            var log = new Dictionary<string, object?>
            {
                ["address"] = sourceBridgeEmitterAddress,
                ["transactionHash"] = txHash,
                ["blockHash"] = blockHash,
                ["blockNumber"] = "0x1234",
                ["topics"] = new object?[] { BscMainnetSccp.SourceEventTopic, sourceEventDigest },
                ["data"] = "0x",
            };
            foreach (var (key, value) in overrides)
            {
                log[key] = value;
            }

            return log;
        }
        var receipt = new Dictionary<string, object?>
        {
            ["transactionHash"] = txHash,
            ["blockHash"] = blockHash,
            ["blockNumber"] = "0x1234",
            ["status"] = "0x1",
            ["logs"] = new object?[] { SourceEventLog() },
        };
        var block = new Dictionary<string, object?>
        {
            ["hash"] = blockHash,
            ["number"] = "0x1234",
            ["receiptsRoot"] = receiptsRoot,
        };
        var receiptProof = new BscMainnetReceiptProof
        {
            SourceEventDigest = sourceEventDigest,
            ValidatorEpoch = 36,
            BlockNumber = 4660,
            BlockHash = blockHash,
            ReceiptsRoot = receiptsRoot,
            ValidatorSetHash = "0x" + new string('a', 64),
            CommitSealHash = "0x" + new string('d', 64),
            ReceiptRootIndex = 3,
            ReceiptTrieProofNodes = new[] { new byte[] { 0x01 }, new byte[] { 0x02, 0x03 } },
            InclusionBranch = new[] { Enumerable.Repeat((byte)0x11, 32).ToArray() },
        };
        var receiptProofHash = BscMainnetSccp.BscSccpReceiptProofHash(
            receiptProof.SourceEventDigest,
            receiptProof.ValidatorEpoch,
            receiptProof.BlockNumber,
            receiptProof.BlockHash,
            receiptProof.ReceiptsRoot,
            receiptProof.ValidatorSetHash,
            receiptProof.CommitSealHash,
            receiptProof.ReceiptRootIndex,
            receiptProof.ReceiptTrieProofNodes,
            receiptProof.InclusionBranch);
        var parliaFinalityEvidence = new BscMainnetParliaFinalityEvidence(
            "0x1234",
            blockHash,
            receiptsRoot);
        var parliaFinality = parliaFinalityEvidence.ToDictionary(
            new Dictionary<string, object?>
            {
                ["validatorEpoch"] = "0x24",
                ["validatorSetHash"] = "0x" + new string('a', 64),
                ["commitSealHash"] = "0x" + new string('d', 64),
            });
        var provider = new ExecutionProviderStub("0x38", receipt, block);

        var evidence = await BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
            new BscMainnetInboundEvidence
            {
                TransactionHash = txHash,
                ReceiptProof = receiptProof,
            }.WithParliaFinalityEvidence(
                parliaFinalityEvidence,
                new Dictionary<string, object?>
                {
                    ["validatorEpoch"] = "0x24",
                    ["validatorSetHash"] = "0x" + new string('a', 64),
                    ["commitSealHash"] = "0x" + new string('d', 64),
                }) with
            {
                SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
            },
            provider, cancellationToken: TestContext.Current.CancellationToken);
        Assert.Equal(BscMainnetSccp.DomainBsc, evidence.SourceDomain);
        Assert.Equal(BscMainnetSccp.DomainSora, evidence.TargetDomain);
        Assert.Equal(txHash, evidence.TransactionHash);
        Assert.Equal("0x1", evidence.Receipt?["status"]);
        Assert.Equal(receiptsRoot, evidence.Block?["receiptsRoot"]);
        Assert.Equal("4660", evidence.ParliaFinality?["executionBlockNumber"]);
        Assert.Equal(blockHash, evidence.ParliaFinality?["executionBlockHash"]);
        Assert.Equal(receiptsRoot, evidence.ParliaFinality?["executionReceiptsRoot"]);
        Assert.Equal(receiptProofHash, evidence.ReceiptProofHash);
        Assert.Equal(receiptsRoot, evidence.ReceiptProof?.ReceiptsRoot);
        Assert.Equal(sourceEventDigest, evidence.SourceEventDigest);
        Assert.Equal(sourceBridgeEmitterAddress, evidence.SourceBridgeEmitterAddress);
        Assert.NotSame(receiptProof.ReceiptTrieProofNodes[0], evidence.ReceiptProof?.ReceiptTrieProofNodes[0]);
        Assert.Equal(
            ["eth_chainId", "eth_getTransactionReceipt", "eth_getBlockByHash"],
            provider.Calls);
        var providerFinalityEvidence = await BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
            new BscMainnetInboundEvidence
            {
                TransactionHash = txHash,
                ReceiptProof = receiptProof,
                SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
            },
            provider,
            new ConsensusProviderStub(parliaFinality), cancellationToken: TestContext.Current.CancellationToken);
        Assert.Equal(blockHash, providerFinalityEvidence.ParliaFinality?["executionBlockHash"]);
        Assert.Equal(receiptProofHash, providerFinalityEvidence.ReceiptProofHash);
        Assert.Equal(sourceEventDigest, providerFinalityEvidence.SourceEventDigest);

        var proofBytes = await BscMainnetSccp.ProveInboundToSoraAsync(
            evidence,
            new InboundProverStub(
                txHash,
                expectedReceiptProofHash: receiptProofHash,
                expectedSourceEventDigest: sourceEventDigest,
                expectedSourceBridgeEmitterAddress: sourceBridgeEmitterAddress), cancellationToken: TestContext.Current.CancellationToken);
        Assert.Equal(new byte[] { 1, 2, 3 }, proofBytes);
        var mutableProverOutput = new byte[] { 4, 5, 6 };
        var copiedProofBytes = await BscMainnetSccp.ProveInboundToSoraAsync(
            evidence,
            new InboundProverStub(
                txHash,
                mutableProverOutput,
                receiptProofHash,
                sourceEventDigest,
                sourceBridgeEmitterAddress), cancellationToken: TestContext.Current.CancellationToken);
        mutableProverOutput[0] = 9;
        Assert.Equal(new byte[] { 4, 5, 6 }, copiedProofBytes);
        Assert.Equal(
            "submitted",
            await BscMainnetSccp.SubmitInboundToIrohaAsync(
                proofBytes,
                new InboundSubmitterStub(), cancellationToken: TestContext.Current.CancellationToken));
        var mutableSubmitInput = new byte[] { 1, 2, 3 };
        var recordingSubmitter = new RecordingInboundSubmitterStub();
        Assert.Equal(
            "submitted",
            await BscMainnetSccp.SubmitInboundToIrohaAsync(
                mutableSubmitInput,
                recordingSubmitter, cancellationToken: TestContext.Current.CancellationToken));
        mutableSubmitInput[0] = 9;
        Assert.Equal(new byte[] { 1, 2, 3 }, recordingSubmitter.SubmittedProofBytes);

        await Assert.ThrowsAsync<ArgumentOutOfRangeException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence { Receipt = receipt },
                new ExecutionProviderStub("0x1", receipt, block), cancellationToken: TestContext.Current.CancellationToken).AsTask());
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.ValidateExecutionProviderMainnetAsync(
                new ExecutionProviderStub("0x038", receipt, block), cancellationToken: TestContext.Current.CancellationToken).AsTask());
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.ValidateExecutionProviderMainnetAsync(
                new ExecutionProviderStub("0x10000000000000000", receipt, block), cancellationToken: TestContext.Current.CancellationToken).AsTask());
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.ValidateExecutionProviderMainnetAsync(
                new ExecutionProviderStub("56", receipt, block), cancellationToken: TestContext.Current.CancellationToken).AsTask());
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.ValidateExecutionProviderMainnetAsync(
                new ExecutionProviderStub(56, receipt, block), cancellationToken: TestContext.Current.CancellationToken).AsTask());
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence
                {
                    SourceDomain = EthereumMainnetSccp.DomainEthereum,
                    Receipt = receipt,
                    Block = block,
                }, cancellationToken: TestContext.Current.CancellationToken).AsTask());
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence { TransactionHash = txHash }, cancellationToken: TestContext.Current.CancellationToken).AsTask());

        var proofHashOnly = await BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
            new BscMainnetInboundEvidence
            {
                ReceiptProofHash = receiptProofHash,
            }, cancellationToken: TestContext.Current.CancellationToken);
        Assert.Equal(receiptProofHash, proofHashOnly.ReceiptProofHash);
        Assert.Null(proofHashOnly.ReceiptProof);
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence
                {
                    ReceiptProof = receiptProof,
                    ReceiptProofHash = "0x" + new string('9', 64),
                }, cancellationToken: TestContext.Current.CancellationToken).AsTask());
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence
                {
                    ReceiptProof = receiptProof with
                    {
                        SourceDomain = EthereumMainnetSccp.DomainEthereum,
                    },
                }, cancellationToken: TestContext.Current.CancellationToken).AsTask());
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence
                {
                    ReceiptProofHash = "0x" + new string('0', 64),
                }, cancellationToken: TestContext.Current.CancellationToken).AsTask());

        var failedReceipt = new Dictionary<string, object?>(receipt)
        {
            ["status"] = "0x0",
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence { Receipt = failedReceipt, Block = block }, cancellationToken: TestContext.Current.CancellationToken).AsTask());

        var missingReceiptBlockNumber = new Dictionary<string, object?>(receipt);
        missingReceiptBlockNumber.Remove("blockNumber");
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence
                {
                    Receipt = missingReceiptBlockNumber,
                    Block = block,
                }, cancellationToken: TestContext.Current.CancellationToken).AsTask());

        var zeroReceiptBlockNumber = new Dictionary<string, object?>(receipt)
        {
            ["blockNumber"] = "0x0",
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence
                {
                    Receipt = zeroReceiptBlockNumber,
                    Block = block,
                }, cancellationToken: TestContext.Current.CancellationToken).AsTask());

        var driftedReceipt = new Dictionary<string, object?>(receipt)
        {
            ["transactionHash"] = "0x" + new string('f', 64),
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence
                {
                    TransactionHash = txHash,
                    Receipt = driftedReceipt,
                    Block = block,
                }, cancellationToken: TestContext.Current.CancellationToken).AsTask());

        var driftedBlock = new Dictionary<string, object?>(block)
        {
            ["hash"] = "0x" + new string('e', 64),
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence { Receipt = receipt, Block = driftedBlock }, cancellationToken: TestContext.Current.CancellationToken).AsTask());

        var missingBlockNumber = new Dictionary<string, object?>(block);
        missingBlockNumber.Remove("number");
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence { Receipt = receipt, Block = missingBlockNumber }, cancellationToken: TestContext.Current.CancellationToken).AsTask());

        var zeroBlockNumber = new Dictionary<string, object?>(block)
        {
            ["number"] = "0x0",
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence { Receipt = receipt, Block = zeroBlockNumber }, cancellationToken: TestContext.Current.CancellationToken).AsTask());

        var wrongNumberBlock = new Dictionary<string, object?>(block)
        {
            ["number"] = "0x1235",
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence { Receipt = receipt, Block = wrongNumberBlock }, cancellationToken: TestContext.Current.CancellationToken).AsTask());

        var uppercaseReceipt = new Dictionary<string, object?>(receipt)
        {
            ["transactionHash"] = txHash.ToUpperInvariant(),
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence { Receipt = uppercaseReceipt, Block = block }, cancellationToken: TestContext.Current.CancellationToken).AsTask());

        var missingReceiptRootBlock = new Dictionary<string, object?>(block);
        missingReceiptRootBlock.Remove("receiptsRoot");
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence
                {
                    Receipt = receipt,
                    Block = missingReceiptRootBlock,
                }, cancellationToken: TestContext.Current.CancellationToken).AsTask());

        var zeroReceiptRootBlock = new Dictionary<string, object?>(block)
        {
            ["receiptsRoot"] = "0x" + new string('0', 64),
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence
                {
                    Receipt = receipt,
                    Block = zeroReceiptRootBlock,
                }, cancellationToken: TestContext.Current.CancellationToken).AsTask());

        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.ProveInboundToSoraAsync(
                new BscMainnetInboundEvidence
                {
                    ReceiptProofHash = receiptProofHash,
                },
                new InboundProverStub(null), cancellationToken: TestContext.Current.CancellationToken).AsTask());

        var hashOnlyProver = new InboundProverStub(null);
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.ProveInboundToSoraAsync(
                new BscMainnetInboundEvidence
                {
                    ParliaFinality = parliaFinality,
                    ReceiptProofHash = receiptProofHash,
                },
                hashOnlyProver, cancellationToken: TestContext.Current.CancellationToken).AsTask());
        Assert.Equal(0, hashOnlyProver.Calls);

        var noSourceEventProver = new InboundProverStub(null);
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.ProveInboundToSoraAsync(
                new BscMainnetInboundEvidence
                {
                    ParliaFinality = parliaFinality,
                    ReceiptProof = receiptProof,
                },
                noSourceEventProver, cancellationToken: TestContext.Current.CancellationToken).AsTask());
        Assert.Equal(0, noSourceEventProver.Calls);

        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.ProveInboundToSoraAsync(
                new BscMainnetInboundEvidence
                {
                    Receipt = receipt,
                    Block = block,
                    ParliaFinality = parliaFinality,
                    ReceiptProof = receiptProof with
                    {
                        ReceiptsRoot = "0x" + new string('9', 64),
                    },
                },
                new InboundProverStub(txHash), cancellationToken: TestContext.Current.CancellationToken).AsTask());

        var driftedSourceReceipt = new Dictionary<string, object?>(receipt)
        {
            ["logs"] = new object?[]
            {
                SourceEventLog(
                    new KeyValuePair<string, object?>(
                        "topics",
                        new object?[] { BscMainnetSccp.SourceEventTopic, "0x" + new string('9', 64) })),
            },
        };
        var driftedSourceProver = new InboundProverStub(null);
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.ProveInboundToSoraAsync(
                new BscMainnetInboundEvidence
                {
                    Receipt = driftedSourceReceipt,
                    Block = block,
                    ParliaFinality = parliaFinality,
                    ReceiptProof = receiptProof,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                },
                driftedSourceProver, cancellationToken: TestContext.Current.CancellationToken).AsTask());
        Assert.Equal(0, driftedSourceProver.Calls);

        var extraTopicBscSourceReceipt = new Dictionary<string, object?>(receipt)
        {
            ["logs"] = new object?[]
            {
                SourceEventLog(
                    new KeyValuePair<string, object?>(
                        "topics",
                        new object?[]
                        {
                            BscMainnetSccp.SourceEventTopic,
                            sourceEventDigest,
                            "0x" + new string('6', 64),
                        })),
            },
        };
        var extraTopicBscSourceError = await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence
                {
                    Receipt = extraTopicBscSourceReceipt,
                    Block = block,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }, cancellationToken: TestContext.Current.CancellationToken).AsTask());
        Assert.Contains("exactly 2 topics", extraTopicBscSourceError.Message);

        var nonEmptyDataBscSourceReceipt = new Dictionary<string, object?>(receipt)
        {
            ["logs"] = new object?[]
            {
                SourceEventLog(new KeyValuePair<string, object?>("data", "0x01")),
            },
        };
        var nonEmptyDataBscSourceError = await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence
                {
                    Receipt = nonEmptyDataBscSourceReceipt,
                    Block = block,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }, cancellationToken: TestContext.Current.CancellationToken).AsTask());
        Assert.Contains("data must be 0x", nonEmptyDataBscSourceError.Message);

        var zeroDigestBscSourceReceipt = new Dictionary<string, object?>(receipt)
        {
            ["logs"] = new object?[]
            {
                SourceEventLog(
                    new KeyValuePair<string, object?>(
                        "topics",
                        new object?[] { BscMainnetSccp.SourceEventTopic, "0x" + new string('0', 64) })),
            },
        };
        var zeroDigestBscSourceError = await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence
                {
                    Receipt = zeroDigestBscSourceReceipt,
                    Block = block,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }, cancellationToken: TestContext.Current.CancellationToken).AsTask());
        Assert.Contains("digest must not be zero", zeroDigestBscSourceError.Message);

        var duplicateBscSourceReceipt = new Dictionary<string, object?>(receipt)
        {
            ["logs"] = new object?[] { SourceEventLog(), SourceEventLog() },
        };
        var duplicateBscSourceError = await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence
                {
                    Receipt = duplicateBscSourceReceipt,
                    Block = block,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }, cancellationToken: TestContext.Current.CancellationToken).AsTask());
        Assert.Contains("exactly one matching", duplicateBscSourceError.Message);

        var removedBscSourceReceipt = new Dictionary<string, object?>(receipt)
        {
            ["logs"] = new object?[]
            {
                SourceEventLog(new KeyValuePair<string, object?>("removed", true)),
            },
        };
        var removedBscSourceError = await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence
                {
                    Receipt = removedBscSourceReceipt,
                    Block = block,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }, cancellationToken: TestContext.Current.CancellationToken).AsTask());
        Assert.Contains("removed logs", removedBscSourceError.Message);

        foreach (var removed in new object?[] { null, 1, "secret-token-removed" })
        {
            var malformedRemovedBscSourceReceipt = new Dictionary<string, object?>(receipt)
            {
                ["logs"] = new object?[]
                {
                    SourceEventLog(new KeyValuePair<string, object?>("removed", removed)),
                },
            };
            var malformedRemovedBscSourceError = await Assert.ThrowsAsync<ArgumentException>(
                () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                    new BscMainnetInboundEvidence
                    {
                        Receipt = malformedRemovedBscSourceReceipt,
                        Block = block,
                        SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                    }, cancellationToken: TestContext.Current.CancellationToken).AsTask());
            Assert.Contains(
                "receipt.logs[0].removed must be a boolean",
                malformedRemovedBscSourceError.Message);
            Assert.DoesNotContain("secret-token", malformedRemovedBscSourceError.Message);
        }

        var missingBscSourceContextLog = SourceEventLog();
        missingBscSourceContextLog.Remove("transactionHash");
        var missingBscSourceContextReceipt = new Dictionary<string, object?>(receipt)
        {
            ["logs"] = new object?[] { missingBscSourceContextLog },
        };
        var missingBscSourceContextError = await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence
                {
                    Receipt = missingBscSourceContextReceipt,
                    Block = block,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }, cancellationToken: TestContext.Current.CancellationToken).AsTask());
        Assert.Contains("receipt.logs[0].transactionHash", missingBscSourceContextError.Message);

        var driftedFinalityHash = new Dictionary<string, object?>(parliaFinality)
        {
            ["executionBlockHash"] = "0x" + new string('e', 64),
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence
                {
                    Receipt = receipt,
                    Block = block,
                    ParliaFinality = driftedFinalityHash,
                }, cancellationToken: TestContext.Current.CancellationToken).AsTask());

        var driftedFinalityNumber = new Dictionary<string, object?>(parliaFinality)
        {
            ["executionBlockNumber"] = "0x1235",
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence
                {
                    Receipt = receipt,
                    Block = block,
                    ParliaFinality = driftedFinalityNumber,
                }, cancellationToken: TestContext.Current.CancellationToken).AsTask());

        var driftedFinalityReceiptsRoot = new Dictionary<string, object?>(parliaFinality)
        {
            ["executionReceiptsRoot"] = "0x" + new string('e', 64),
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence
                {
                    Receipt = receipt,
                    Block = block,
                    ParliaFinality = driftedFinalityReceiptsRoot,
                }, cancellationToken: TestContext.Current.CancellationToken).AsTask());

        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.ProveInboundToSoraAsync(
                evidence,
                new InboundProverStub(txHash, [0, 0]), cancellationToken: TestContext.Current.CancellationToken).AsTask());
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.SubmitInboundToIrohaAsync(
                [0, 0],
                new InboundSubmitterStub(), cancellationToken: TestContext.Current.CancellationToken).AsTask());
    }

    [Fact]
    public async Task InboundCallbacksReceiveSnapshotEvidence()
    {
        var txHash = "0x" + new string('a', 64);
        var blockHash = "0x" + new string('b', 64);
        var receiptsRoot = "0x" + new string('c', 64);
        var sourceEventDigest = "0x" + new string('1', 64);
        var logAddress = "0x" + new string('f', 40);
        var validatorSetHash = "0x" + new string('d', 64);
        var commitSealHash = "0x" + new string('e', 64);
        var logTopics = new object?[] { BscMainnetSccp.SourceEventTopic, sourceEventDigest };
        var logMetadata = new Dictionary<string, object?>
        {
            ["address"] = logAddress,
            ["transactionHash"] = txHash,
            ["blockHash"] = blockHash,
            ["blockNumber"] = "0x1234",
            ["topics"] = logTopics,
            ["data"] = "0x",
        };
        var receipt = new Dictionary<string, object?>
        {
            ["status"] = "0x1",
            ["transactionHash"] = txHash,
            ["blockHash"] = blockHash,
            ["blockNumber"] = "0x1234",
            ["logs"] = new object?[] { logMetadata },
        };
        var block = new Dictionary<string, object?>
        {
            ["hash"] = blockHash,
            ["number"] = "0x1234",
            ["receiptsRoot"] = receiptsRoot,
        };
        var parliaFinality = new BscMainnetParliaFinalityEvidence(
            "0x1234",
            blockHash,
            receiptsRoot).ToDictionary(
                [
                    new KeyValuePair<string, object?>("validatorSetHash", validatorSetHash),
                    new KeyValuePair<string, object?>("commitSealHash", commitSealHash),
                ]);
        var receiptProof = new BscMainnetReceiptProof
        {
            SourceEventDigest = sourceEventDigest,
            ValidatorEpoch = 0,
            BlockNumber = 0x1234,
            BlockHash = blockHash,
            ReceiptsRoot = receiptsRoot,
            ValidatorSetHash = validatorSetHash,
            CommitSealHash = commitSealHash,
            ReceiptRootIndex = 0,
            ReceiptTrieProofNodes = [new byte[] { 0x01 }],
            InclusionBranch = [Enumerable.Repeat((byte)0x11, 32).ToArray()],
        };

        var collected = await BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
            new BscMainnetInboundEvidence { TransactionHash = txHash },
            new ExecutionProviderStub("0x38", receipt, block),
            new MutatingConsensusProviderStub(receipt, block, parliaFinality), cancellationToken: TestContext.Current.CancellationToken);
        Assert.Equal("0x1", collected.Receipt?["status"]);
        Assert.Equal(receiptsRoot, collected.Block?["receiptsRoot"]);
        Assert.Equal("0x1", receipt["status"]);
        Assert.Equal(receiptsRoot, block["receiptsRoot"]);
        Assert.Equal(logAddress, logMetadata["address"]);
        Assert.Equal(BscMainnetSccp.SourceEventTopic, Assert.IsType<string>(logTopics[0]));
        Assert.Equal(sourceEventDigest, Assert.IsType<string>(logTopics[1]));

        var proofBytes = await BscMainnetSccp.ProveInboundToSoraAsync(
            new BscMainnetInboundEvidence
            {
                Receipt = receipt,
                Block = block,
                ParliaFinality = parliaFinality,
                ReceiptProof = receiptProof,
                SourceBridgeEmitterAddress = logAddress,
            },
            new MutatingInboundProverStub(receipt, block, parliaFinality, txHash), cancellationToken: TestContext.Current.CancellationToken);
        Assert.Equal(new byte[] { 1, 2, 3 }, proofBytes);
        Assert.Equal("0x1", receipt["status"]);
        Assert.Equal(receiptsRoot, block["receiptsRoot"]);
        Assert.Equal(logAddress, logMetadata["address"]);
        Assert.Equal(BscMainnetSccp.SourceEventTopic, Assert.IsType<string>(logTopics[0]));
        Assert.Equal(sourceEventDigest, Assert.IsType<string>(logTopics[1]));
        Assert.Equal(blockHash, parliaFinality["executionBlockHash"]);
    }

    [Fact]
    public async Task OutboundProofRequestCalldataAndSubmitUseBscMainnetBinding()
    {
        var binding = SampleDestinationBinding();
        var publicInputs = SamplePublicInputs();
        var mutableBundleBytes = SampleOutboundBundleBytes();
        var expectedBundleBytes = mutableBundleBytes.ToArray();
        var input = SampleOutboundInput(binding, publicInputs) with
        {
            BundleBytes = mutableBundleBytes,
        };
        mutableBundleBytes[0] ^= 0xff;
        var detachedBundleBytes = input.BundleBytes;
        detachedBundleBytes[1] ^= 0xff;
        Assert.Equal(expectedBundleBytes, input.BundleBytes);

        var mutableSourceProofBytes = new byte[] { 1, 2, 3 };
        var sourceProofInput = input with
        {
            SourceProofBytes = mutableSourceProofBytes,
        };
        mutableSourceProofBytes[0] = 9;
        var detachedSourceProofBytes = Assert.IsType<byte[]>(sourceProofInput.SourceProofBytes);
        detachedSourceProofBytes[1] = 9;
        Assert.Equal(new byte[] { 1, 2, 3 }, Assert.IsType<byte[]>(sourceProofInput.SourceProofBytes));

        var request = BscMainnetSccp.BuildOutboundProofRequest(input);
        Assert.Equal(1, request.Version);
        Assert.Equal(BscMainnetSccp.EvmGroth16Bn254ProofBackend, request.Backend);
        Assert.Equal(BscMainnetSccp.DomainSora, request.SourceDomain);
        Assert.Equal(BscMainnetSccp.DomainBsc, request.TargetDomain);
        Assert.Equal(binding.BindingHash, request.DestinationBindingHash);
        Assert.Equal(binding, request.DestinationBinding);
        Assert.Equal(publicInputs, request.PublicInputs);
        Assert.Equal(9, request.PublicSignalWords.Length);
        Assert.Equal(expectedBundleBytes, request.BundleBytes);
        Assert.Empty(request.SourceProofBytes);
        Assert.NotSame(input.BundleBytes, request.BundleBytes);

        var mutableProof = Groth16ProofBytes();
        var prover = new OutboundProverStub(mutableProof);
        var proofResult = await BscMainnetSccp.ProveOutboundToBscAsync(input, prover, cancellationToken: TestContext.Current.CancellationToken);
        mutableProof[31] = 9;
        Assert.NotNull(prover.Request);
        Assert.Equal(1, proofResult.ProofBytes[31]);
        Assert.Equal(request.RequestHash, proofResult.RequestHash);
        Assert.Equal(request.PublicSignalWords, proofResult.PublicSignalWords);
        Assert.Equal(publicInputs, proofResult.PublicInputs);
        Assert.Equal(binding, proofResult.DestinationBinding);

        var submission = BscMainnetSccp.BuildBscCalldata(
            new BscMainnetSccpSubmissionInput(proofResult));
        Assert.Equal(1, submission.Version);
        Assert.Equal(BscMainnetSccp.StarkFriProofFamily, submission.ProofFamily);
        Assert.Equal(BscMainnetSccp.EvmGroth16Bn254ProofBackend, submission.VerifierBackend);
        Assert.Equal(BscMainnetSccp.ContractCallAbiTuple, submission.EnvelopeEncoding);
        Assert.Equal(BscMainnetSccp.SubmitMessageProofAbi, submission.ContractMethod);
        Assert.Equal(BscMainnetSccp.SubmitMessageProofSelector, submission.FunctionSelector);
        Assert.Equal(BscMainnetSccp.DomainSora, submission.SourceDomain);
        Assert.Equal(BscMainnetSccp.DomainBsc, submission.TargetDomain);
        Assert.Equal(proofResult.PublicSignalWords, submission.PublicSignalWords);
        Assert.Equal(676, submission.CallData.Length);
        Assert.StartsWith(BscMainnetSccp.SubmitMessageProofSelector, submission.CallDataHex, StringComparison.Ordinal);
        Assert.Equal(submission.CallData, submission.EnvelopeBytes);
        Assert.Equal(submission.CallDataHex, submission.EnvelopeHex);
        var expectedPublicInputWords = submission.PublicInputWords.ToArray();
        var expectedPublicSignalWords = submission.PublicSignalWords.ToArray();
        var expectedArguments = submission.Arguments.ToArray();
        var expectedCallData = submission.CallData.ToArray();
        var expectedEnvelopeBytes = submission.EnvelopeBytes.ToArray();
        var expectedProofBytes = submission.ProofBytes.ToArray();
        var expectedPublicInputWordsBytes = submission.PublicInputWordsBytes.ToArray();
        var detachedPublicInputWords = submission.PublicInputWords;
        detachedPublicInputWords[0] = "0x" + new string('0', 64);
        var detachedPublicSignalWords = submission.PublicSignalWords;
        detachedPublicSignalWords[0] = "0x" + new string('1', 64);
        var detachedArguments = submission.Arguments;
        detachedArguments[0] = detachedArguments[0] with { Bytes = "0x00" };
        var detachedCallData = submission.CallData;
        detachedCallData[0] ^= 0xff;
        var detachedEnvelopeBytes = submission.EnvelopeBytes;
        detachedEnvelopeBytes[0] ^= 0xff;
        var detachedProofBytes = submission.ProofBytes;
        detachedProofBytes[0] ^= 0xff;
        var detachedPublicInputWordsBytes = submission.PublicInputWordsBytes;
        detachedPublicInputWordsBytes[0] ^= 0xff;
        Assert.Equal(expectedPublicInputWords, submission.PublicInputWords);
        Assert.Equal(expectedPublicSignalWords, submission.PublicSignalWords);
        Assert.Equal(expectedArguments, submission.Arguments);
        Assert.Equal(expectedCallData, submission.CallData);
        Assert.Equal(expectedEnvelopeBytes, submission.EnvelopeBytes);
        Assert.Equal(expectedProofBytes, submission.ProofBytes);
        Assert.Equal(expectedPublicInputWordsBytes, submission.PublicInputWordsBytes);

        var rewrittenSubmission = submission with
        {
            CallData = [0x11, 0x22],
            EnvelopeBytes = [0x33, 0x44, 0x55],
        };
        Assert.Equal("0x1122", rewrittenSubmission.CallDataHex);
        Assert.Equal("0x334455", rewrittenSubmission.EnvelopeHex);
        Assert.Throws<ArgumentException>(() => rewrittenSubmission with { CallDataHex = "0x00" });
        Assert.Throws<ArgumentException>(() => rewrittenSubmission with { EnvelopeHex = "0x00" });
        Assert.Throws<ArgumentException>(
            () => new BscMainnetSccpSubmission(
                rewrittenSubmission.Version,
                rewrittenSubmission.ProofFamily,
                rewrittenSubmission.VerifierBackend,
                rewrittenSubmission.PlatformPayload,
                rewrittenSubmission.EnvelopeEncoding,
                rewrittenSubmission.SubmissionKind,
                rewrittenSubmission.VerifierEntrypoint,
                rewrittenSubmission.ContractMethod,
                rewrittenSubmission.FunctionSelector,
                rewrittenSubmission.SourceDomain,
                rewrittenSubmission.TargetDomain,
                rewrittenSubmission.PublicInputs,
                rewrittenSubmission.PublicInputWords,
                rewrittenSubmission.PublicSignalWords,
                rewrittenSubmission.StatementHash,
                rewrittenSubmission.DestinationBindingHash,
                rewrittenSubmission.Arguments,
                rewrittenSubmission.CallData,
                "0x00",
                rewrittenSubmission.EnvelopeBytes,
                rewrittenSubmission.EnvelopeHex,
                rewrittenSubmission.ProofBytes,
                rewrittenSubmission.PublicInputWordsBytes));
        Assert.Throws<ArgumentException>(
            () => new BscMainnetSccpSubmission(
                rewrittenSubmission.Version,
                rewrittenSubmission.ProofFamily,
                rewrittenSubmission.VerifierBackend,
                rewrittenSubmission.PlatformPayload,
                rewrittenSubmission.EnvelopeEncoding,
                rewrittenSubmission.SubmissionKind,
                rewrittenSubmission.VerifierEntrypoint,
                rewrittenSubmission.ContractMethod,
                rewrittenSubmission.FunctionSelector,
                rewrittenSubmission.SourceDomain,
                rewrittenSubmission.TargetDomain,
                rewrittenSubmission.PublicInputs,
                rewrittenSubmission.PublicInputWords,
                rewrittenSubmission.PublicSignalWords,
                rewrittenSubmission.StatementHash,
                rewrittenSubmission.DestinationBindingHash,
                rewrittenSubmission.Arguments,
                rewrittenSubmission.CallData,
                rewrittenSubmission.CallDataHex,
                rewrittenSubmission.EnvelopeBytes,
                "0x00",
                rewrittenSubmission.ProofBytes,
                rewrittenSubmission.PublicInputWordsBytes));

        var submitter = new OutboundSubmitterStub();
        Assert.Equal(
            "bsc-submitted",
            await BscMainnetSccp.SubmitOutboundToBscAsync(
                new BscMainnetSccpSubmissionInput(proofResult),
                submitter, cancellationToken: TestContext.Current.CancellationToken));
        Assert.NotNull(submitter.Submission);
        Assert.Equal(submission.CallDataHex, submitter.Submission.CallDataHex);
    }

    [Fact]
    public void OutboundProofRequestRejectsNonCanonicalFixedHexFields()
    {
        var binding = SampleDestinationBinding();
        var publicInputs = SamplePublicInputs();
        var input = SampleOutboundInput(binding, publicInputs);

        static void AssertCanonicalHex(Action action, string field)
        {
            var error = Assert.Throws<ArgumentException>(action);
            Assert.Contains(field, error.Message);
            Assert.Contains("canonical lowercase 0x-prefixed 32-byte hex", error.Message);
        }

        AssertCanonicalHex(
            () => BscMainnetSccp.BuildOutboundProofRequest(input with
            {
                PublicInputs = publicInputs with
                {
                    MessageId = UpperFixedHex(publicInputs.MessageId),
                },
            }),
            "MessageId");
        AssertCanonicalHex(
            () => BscMainnetSccp.BuildOutboundProofRequest(input with
            {
                PublicInputs = publicInputs with
                {
                    PayloadHash = UpperFixedHex(publicInputs.PayloadHash),
                },
            }),
            "PayloadHash");
        AssertCanonicalHex(
            () => BscMainnetSccp.BuildOutboundProofRequest(input with
            {
                PublicInputs = publicInputs with
                {
                    CommitmentRoot = UpperFixedHex(publicInputs.CommitmentRoot),
                },
            }),
            "CommitmentRoot");
        AssertCanonicalHex(
            () => BscMainnetSccp.BuildOutboundProofRequest(input with
            {
                PublicInputs = publicInputs with
                {
                    FinalityBlockHash = UpperFixedHex(publicInputs.FinalityBlockHash),
                },
            }),
            "FinalityBlockHash");
        AssertCanonicalHex(
            () => BscMainnetSccp.BuildOutboundProofRequest(input with
            {
                StatementHash = UpperFixedHex(input.StatementHash),
            }),
            "StatementHash");
        AssertCanonicalHex(
            () => BscMainnetSccp.BuildOutboundProofRequest(input with
            {
                DestinationBindingHash = UpperFixedHex(binding.BindingHash),
            }),
            "DestinationBindingHash");
    }

    [Fact]
    public async Task OutboundCallbackAndSubmissionSnapshotsRejectMutation()
    {
        var input = SampleOutboundInput();
        var expectedRequest = BscMainnetSccp.BuildOutboundProofRequest(input);
        var prover = new MutatingOutboundProverStub(Groth16ProofBytes());

        var proofResult = await BscMainnetSccp.ProveOutboundToBscAsync(input, prover, cancellationToken: TestContext.Current.CancellationToken);

        Assert.NotNull(prover.Request);
        Assert.Equal(expectedRequest.BundleBytes, prover.Request.BundleBytes);
        Assert.Equal(expectedRequest.SourceProofBytes, prover.Request.SourceProofBytes);
        Assert.Equal(expectedRequest.PublicInputsBytes, prover.Request.PublicInputsBytes);
        Assert.Equal(expectedRequest.PublicSignalWords, prover.Request.PublicSignalWords);
        Assert.Equal(expectedRequest.RequestHash, proofResult.RequestHash);
        Assert.Equal(expectedRequest.BundleBytes, proofResult.Request.BundleBytes);
        Assert.Equal(expectedRequest.SourceProofBytes, proofResult.Request.SourceProofBytes);
        Assert.Equal(expectedRequest.PublicInputsBytes, proofResult.Request.PublicInputsBytes);
        Assert.Equal(expectedRequest.PublicSignalWords, proofResult.Request.PublicSignalWords);

        var submission = BscMainnetSccp.BuildBscCalldata(
            new BscMainnetSccpSubmissionInput(proofResult));
        Assert.StartsWith(BscMainnetSccp.SubmitMessageProofSelector, submission.CallDataHex, StringComparison.Ordinal);

        var mutatedProofBytes = proofResult.ProofBytes.ToArray();
        mutatedProofBytes[31] = 9;
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.BuildBscCalldata(
                new BscMainnetSccpSubmissionInput(
                    proofResult with { ProofBytes = mutatedProofBytes })));
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.BuildBscCalldata(
                new BscMainnetSccpSubmissionInput(
                    proofResult with
                    {
                        ProofBase64 = Convert.ToBase64String(mutatedProofBytes),
                    })));
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.BuildBscCalldata(
                new BscMainnetSccpSubmissionInput(
                    proofResult with
                    {
                        Request = proofResult.Request with
                        {
                            BundleBytes = "swapped-bsc-bundle"u8.ToArray(),
                        },
                    })));
        var mutatedSignals = proofResult.PublicSignalWords.ToArray();
        mutatedSignals[0] = "0x" + new string('9', 64);
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.BuildBscCalldata(
                new BscMainnetSccpSubmissionInput(
                    proofResult with { PublicSignalWords = mutatedSignals })));
        Assert.Throws<ArgumentException>(() => proofResult.Request with
        {
            PublicSignalWords = new string[] { null! },
        });
        Assert.Throws<ArgumentException>(() => proofResult with
        {
            PublicSignalWords = new string[] { null! },
        });
        Assert.Throws<ArgumentNullException>(() => proofResult.Request with
        {
            PublicInputsBytes = null!,
        });
        Assert.Throws<ArgumentNullException>(() => proofResult.Request with
        {
            SourceProofBytes = null!,
        });
        Assert.Throws<ArgumentNullException>(() => proofResult with
        {
            ProofBytes = null!,
        });
        Assert.Throws<ArgumentNullException>(() => submission with
        {
            CallData = null!,
        });
        Assert.Throws<ArgumentNullException>(() => submission with
        {
            EnvelopeBytes = null!,
        });
        Assert.Throws<ArgumentNullException>(() => submission with
        {
            ProofBytes = null!,
        });
        Assert.Throws<ArgumentNullException>(() => submission with
        {
            PublicInputWordsBytes = null!,
        });
        Assert.Throws<ArgumentException>(() => submission with
        {
            PublicInputWords = new string[] { null! },
        });
        Assert.Throws<ArgumentException>(() => submission with
        {
            PublicSignalWords = new string[] { null! },
        });
        Assert.Throws<ArgumentException>(() => submission with
        {
            Arguments = new BscMainnetSccpSubmissionArgument[] { null! },
        });
    }

    [Fact]
    public void OutboundProofPathRejectsCrossLaneAndMalformedProofs()
    {
        var binding = SampleDestinationBinding();
        var publicInputs = SamplePublicInputs();
        var input = SampleOutboundInput(binding, publicInputs);
        var request = BscMainnetSccp.BuildOutboundProofRequest(input);
        var proofResult = BscMainnetSccp.WrapOutboundProofResult(Groth16ProofBytes(), request);

        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.BuildOutboundProofRequest(
                SampleOutboundInput(
                    binding,
                    publicInputs with { TargetDomain = EthereumMainnetSccp.DomainEthereum })));
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.BuildOutboundProofRequest(
                input with
                {
                    DestinationBindingHash = "0x" + new string('9', 64),
                }));
        var zeroBundleError = Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.BuildOutboundProofRequest(
                input with
                {
                    BundleBytes = [0, 0],
                }));
        Assert.Contains("BundleBytes must not be all zero", zeroBundleError.Message);
        var malformedBundleError = Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.BuildOutboundProofRequest(
                input with
                {
                    BundleBytes = [1, 2, 3],
                }));
        Assert.Contains("bundleBytes.commitment_root is too short", malformedBundleError.Message);
        var publicInputDriftError = Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.BuildOutboundProofRequest(
                input with
                {
                    PublicInputs = publicInputs with
                    {
                        MessageId = "0x" + new string('9', 64),
                    },
                }));
        Assert.Contains("bundleBytes must match publicInputs", publicInputDriftError.Message);
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.WrapOutboundProofResult(
                Groth16ProofBytes(),
                request with
                {
                    DestinationBindingHash = "0x" + new string('9', 64),
                }));
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.BuildBscCalldata(
                new BscMainnetSccpSubmissionInput(
                    proofResult with
                    {
                        DestinationBinding = proofResult.DestinationBinding with
                        {
                            NetworkId = EthereumMainnetSccp.MainnetNetworkId,
                        },
                    })));
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.BuildBscCalldata(
                new BscMainnetSccpSubmissionInput(
                    proofResult with
                    {
                        DestinationBindingHash = "0x" + new string('9', 64),
                    })));
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.BuildBscCalldata(
                new BscMainnetSccpSubmissionInput(
                    proofResult with
                    {
                        PublicInputs = publicInputs with
                        {
                            PayloadHash = "0x" + new string('9', 64),
                        },
                    })));

        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.WrapOutboundProofResult([1, 2, 3], request));
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.WrapOutboundProofResult(new byte[384], request));

        var wrongMessageId = Groth16ProofBytes();
        wrongMessageId[63] = 0x12;
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.WrapOutboundProofResult(wrongMessageId, request));

        var wrongSourceDomain = Groth16ProofBytes();
        wrongSourceDomain[(2 * 32) + 31] = 0x02;
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.WrapOutboundProofResult(wrongSourceDomain, request));

        var badG1Point = Groth16ProofBytes();
        badG1Point[(5 * 32) + 31] = 0x03;
        Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.WrapOutboundProofResult(badG1Point, request));
    }
}
