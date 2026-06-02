using System.Buffers.Binary;
using Hyperledger.Iroha.Sccp;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class SccpEthereumMainnetTests
{
    private const string ExpectedBindingHash =
        "0xc86f9d904df50c4522d01da3773916ebecce816f3fdfa664e2dff7cfbe697c45";
    private const string ExpectedRequestHash =
        "0x5f8c834251ab586c5beb632f058758647eb7f83c3aa22108d389db33e546c411";
    private const string ExpectedEnvelopeHash =
        "0x1bcaf6039957b7d66feae15cea46d75b026df8e28bfdd6b36926d40514400159";
    private const string ExpectedPublicInputsBytes =
        "011111111111111111111111111111111111111111111111111111111111111111"
        + "2222222222222222222222222222222222222222222222222222222222222222"
        + "01000000"
        + "3333333333333333333333333333333333333333333333333333333333333333"
        + "2a00000000000000"
        + "4444444444444444444444444444444444444444444444444444444444444444";
    private const string ExpectedCallDataHex =
        "0xbd57826c0000000000000000000000000000000000000000000000000000000000000100"
        + "1111111111111111111111111111111111111111111111111111111111111111"
        + "2222222222222222222222222222222222222222222222222222222222222222"
        + "0000000000000000000000000000000000000000000000000000000000000001"
        + "3333333333333333333333333333333333333333333333333333333333333333"
        + "000000000000000000000000000000000000000000000000000000000000002a"
        + "4444444444444444444444444444444444444444444444444444444444444444"
        + "5555555555555555555555555555555555555555555555555555555555555555"
        + "0000000000000000000000000000000000000000000000000000000000000180"
        + "0000000000000000000000000000000000000000000000000000000000000001"
        + "1111111111111111111111111111111111111111111111111111111111111111"
        + "0000000000000000000000000000000000000000000000000000000000000000"
        + "3333333333333333333333333333333333333333333333333333333333333333"
        + "0000000000000000000000000000000000000000000000000000000000000001"
        + "0000000000000000000000000000000000000000000000000000000000000002"
        + "1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed"
        + "198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2"
        + "12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa"
        + "090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b"
        + "0000000000000000000000000000000000000000000000000000000000000001"
        + "0000000000000000000000000000000000000000000000000000000000000002";
    private const string ExpectedReceiptProofBytes =
        "0101000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
        + "20000000000000003412000000000000"
        + "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        + "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
        + "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
        + "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        + "030000000000000002000000010000000102000000020301000000"
        + "1111111111111111111111111111111111111111111111111111111111111111";
    private const string ExpectedReceiptProofHash =
        "0x39f014e3f5f8d38b44d59f1afdf72ceb71d10d6d937f268c404b046f092b38f0";

    private static readonly string[] ExpectedPublicSignalWords =
    [
        "0x0ffdbc782e79d1dc508e08af01e87f16d93b6e58e4861a0b8155455e3ee7a683",
        "0x0c5398ea95021a790e276e3ece1592b32b85751dc77e50293c867a5f2e0131bb",
        "0x2eb6b5dbab56255a979f433862429637ba1e8251106271606f0a279f593d7a39",
        "0x01c73f2f9156a52493a9beabeec73e62deed32fcef2e3e6fac86a79f0764f0bc",
        "0x220a98afe36b6d6828e7e852988c8595f0ad6d128e845e74e0161cb0fa2f642f",
        "0x2b153d0fe1bc6e2a6d44e851523edb1511dac55443ca80c22cbe9cb7423886dc",
        "0x2697e4e42f34b673b4aa254c6a92de09304e84c1a667c7d266777775a231efb4",
        "0x16fbe0c1d659f142b3e7815b24df66da3cfd89cc42d051b04bc31aae6925c396",
        "0x02dcef873274cccbb6bde309daaabeec707adc38755c2d118518ecd716151da3",
    ];

    private static readonly string[] ExpectedPublicInputWords =
    [
        "0x1111111111111111111111111111111111111111111111111111111111111111",
        "0x2222222222222222222222222222222222222222222222222222222222222222",
        "0x0000000000000000000000000000000000000000000000000000000000000001",
        "0x3333333333333333333333333333333333333333333333333333333333333333",
        "0x000000000000000000000000000000000000000000000000000000000000002a",
        "0x4444444444444444444444444444444444444444444444444444444444444444",
    ];

    private sealed class ExecutionProviderStub(
        object chainId,
        IReadOnlyDictionary<string, object?> receipt,
        IReadOnlyDictionary<string, object?> block) : IEthereumMainnetExecutionProvider
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
        string expectedTransactionHash,
        string? expectedReceiptProofHash = null,
        string? expectedSourceEventDigest = null)
        : IEthereumMainnetInboundProver
    {
        public ValueTask<byte[]> ProveAsync(
            EthereumMainnetInboundEvidence evidence,
            CancellationToken cancellationToken = default)
        {
            Assert.Equal(EthereumMainnetSccp.DomainEthereum, evidence.SourceDomain);
            Assert.Equal(EthereumMainnetSccp.DomainSora, evidence.TargetDomain);
            Assert.Equal(expectedTransactionHash, evidence.TransactionHash);
            if (expectedReceiptProofHash is not null)
            {
                Assert.Equal(expectedReceiptProofHash, evidence.ReceiptProofHash);
                Assert.NotNull(evidence.ReceiptProof);
            }

            if (expectedSourceEventDigest is not null)
            {
                Assert.Equal(expectedSourceEventDigest, evidence.ReceiptProof?.SourceEventDigest);
            }

            return ValueTask.FromResult(new byte[] { 1, 2, 3 });
        }
    }

    private sealed class ConsensusProviderStub(
        IReadOnlyDictionary<string, object?> expectedReceipt,
        IReadOnlyDictionary<string, object?> expectedBlock,
        string expectedTransactionHash,
        IReadOnlyDictionary<string, object?> finality) : IEthereumMainnetConsensusProvider
    {
        public int Calls { get; private set; }

        public ValueTask<IReadOnlyDictionary<string, object?>?> CollectFinalityEvidenceAsync(
            IReadOnlyDictionary<string, object?>? receipt,
            IReadOnlyDictionary<string, object?>? block,
            string? transactionHash,
            CancellationToken cancellationToken = default)
        {
            Calls++;
            Assert.Same(expectedReceipt, receipt);
            Assert.Same(expectedBlock, block);
            Assert.Equal(expectedTransactionHash, transactionHash);
            return ValueTask.FromResult<IReadOnlyDictionary<string, object?>?>(finality);
        }
    }

    private sealed class InboundSubmitterStub : IEthereumMainnetInboundSubmitter
    {
        public ValueTask<object?> SubmitAsync(
            byte[] proofBytes,
            CancellationToken cancellationToken = default)
        {
            Assert.Equal(new byte[] { 1, 2, 3 }, proofBytes);
            return ValueTask.FromResult<object?>("submitted");
        }
    }

    private sealed class CountingInboundProver : IEthereumMainnetInboundProver
    {
        public int Calls { get; private set; }

        public ValueTask<byte[]> ProveAsync(
            EthereumMainnetInboundEvidence evidence,
            CancellationToken cancellationToken = default)
        {
            Calls++;
            return ValueTask.FromResult(new byte[] { 1, 2, 3 });
        }
    }

    private sealed class OutboundProverStub(byte[] proofBytes) : IEthereumMainnetOutboundProver
    {
        public EthereumMainnetOutboundProofRequest? Request { get; private set; }

        public ValueTask<byte[]> ProveAsync(
            EthereumMainnetOutboundProofRequest request,
            CancellationToken cancellationToken = default)
        {
            Request = request;
            Assert.Equal(ExpectedRequestHash, request.RequestHash);
            Assert.Equal(ExpectedBindingHash, request.DestinationBindingHash);
            Assert.Equal(ExpectedPublicSignalWords, request.PublicSignalWords);
            return ValueTask.FromResult(proofBytes);
        }
    }

    private sealed class OutboundSubmitterStub : IEthereumMainnetOutboundSubmitter
    {
        public EthereumMainnetSccpSubmission? Submission { get; private set; }

        public ValueTask<object?> SubmitAsync(
            EthereumMainnetSccpSubmission submission,
            CancellationToken cancellationToken = default)
        {
            Submission = submission;
            return ValueTask.FromResult<object?>("eth-submitted");
        }
    }

    private static EthereumMainnetSccpDestinationBinding SampleDestinationBinding()
        => EthereumMainnetSccp.DestinationBinding(
            "0x" + new string('1', 40),
            "0x" + new string('2', 40),
            "0x" + new string('b', 64),
            "0x" + new string('c', 64));

    private static EthereumMainnetTransparentPublicInputs SamplePublicInputs()
        => new(
            Version: 1,
            MessageId: "0x" + new string('1', 64),
            PayloadHash: "0x" + new string('2', 64),
            TargetDomain: EthereumMainnetSccp.DomainEthereum,
            CommitmentRoot: "0x" + new string('3', 64),
            FinalityHeight: 42,
            FinalityBlockHash: "0x" + new string('4', 64));

    private static EthereumMainnetOutboundProofRequestInput SampleOutboundInput(
        EthereumMainnetSccpDestinationBinding? binding = null,
        EthereumMainnetTransparentPublicInputs? publicInputs = null)
        => new()
        {
            PublicInputs = publicInputs ?? SamplePublicInputs(),
            BundleBytes = "eth-mainnet-bundle"u8.ToArray(),
            SourceProofBytes = "eth-source-proof"u8.ToArray(),
            StatementHash = "0x" + new string('5', 64),
            DestinationBinding = binding ?? SampleDestinationBinding(),
            DestinationBindingHash = (binding ?? SampleDestinationBinding()).BindingHash,
            SourceDomain = EthereumMainnetSccp.DomainSora,
        };

    private static byte[] Groth16ProofBytes()
        => Concat(
            AbiWord(1),
            RepeatByte(0x11, 32),
            AbiWord((ulong)EthereumMainnetSccp.DomainSora),
            RepeatByte(0x33, 32),
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
    public void MainnetGuardsAcceptEthereumAndRejectOtherRoutes()
    {
        EthereumMainnetSccp.RequireMainnetChainId(1);
        EthereumMainnetSccp.RequireMainnetNetworkId(EthereumMainnetSccp.MainnetNetworkId);
        EthereumMainnetSccp.RequireInboundRoute(
            EthereumMainnetSccp.DomainEthereum,
            EthereumMainnetSccp.DomainSora);
        Assert.Equal(
            "0x577b41c65ffbce226de59f224b464797257063747891b88ebec1bcd57af82727",
            EthereumMainnetSccp.SourceEventTopic);
        EthereumMainnetSccp.RequireOutboundRoute(
            EthereumMainnetSccp.DomainSora,
            EthereumMainnetSccp.DomainEthereum);

        var binding = EthereumMainnetSccp.DestinationBinding(
            "0x" + new string('1', 40),
            "0x" + new string('2', 40),
            "0x" + new string('b', 64),
            "0x" + new string('c', 64));
        Assert.Equal(1, binding.Version);
        Assert.Equal(EthereumMainnetSccp.DomainSora, binding.SourceDomain);
        Assert.Equal(EthereumMainnetSccp.DomainEthereum, binding.TargetDomain);
        Assert.Equal(EthereumMainnetSccp.MainnetNetworkId, binding.NetworkId);
        Assert.Equal(EthereumMainnetSccp.EvmGroth16Bn254ProofBackend, binding.VerifierBackend);
        Assert.Equal(EthereumMainnetSccp.StarkFriProofFamily, binding.ProofFamily);
        Assert.Equal(
            "evm:0:1:0000000000000000000000000000000000000000000000000000000000000001:"
                + "0x1111111111111111111111111111111111111111:"
                + "0x2222222222222222222222222222222222222222:"
                + "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:"
                + "0xcccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            binding.Key);
        Assert.Equal(
            "0xc86f9d904df50c4522d01da3773916ebecce816f3fdfa664e2dff7cfbe697c45",
            binding.BindingHash);
        Assert.Equal(
            binding.BindingHash,
            EthereumMainnetSccp.DestinationBindingHash(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64)));
        Assert.Equal(
            binding.BindingHash,
            EthereumMainnetSccp.DestinationBinding(
                "0X" + new string('1', 40).ToUpperInvariant(),
                "0X" + new string('2', 40).ToUpperInvariant(),
                "0X" + new string('b', 64).ToUpperInvariant(),
                "0X" + new string('c', 64).ToUpperInvariant(),
                expectedBindingHash: binding.BindingHash,
                expectedKey: binding.Key).BindingHash);

        Assert.Throws<ArgumentOutOfRangeException>(
            () => EthereumMainnetSccp.RequireMainnetChainId(56));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.RequireMainnetNetworkId("0x38"));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.RequireMainnetNetworkId(
                "0X0000000000000000000000000000000000000000000000000000000000000001"));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.RequireMainnetNetworkId("0x01"));
        Assert.Throws<ArgumentException>(() => EthereumMainnetSccp.RequireInboundRoute(2, 0));
        Assert.Throws<ArgumentException>(() => EthereumMainnetSccp.RequireOutboundRoute(0, 2));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64),
                networkId: "0x0000000000000000000000000000000000000000000000000000000000000038"));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('1', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64)));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.DestinationBinding(
                "0x" + new string('0', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64)));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64),
                targetDomain: 2));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64),
                expectedBindingHash: "0x" + new string('9', 64)));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64),
                expectedKey: binding.Key + "-wrong"));
    }

    [Fact]
    public void ReceiptProofTranscriptMatchesSharedEthereumVector()
    {
        var sourceEventDigest = "0x" + new string('e', 64);
        var proofNodes = new[] { new byte[] { 0x01 }, new byte[] { 0x02, 0x03 } };
        var branch = new[] { RepeatByte(0x11, 32) };

        byte[] BuildBytes(
            string? digest = null,
            IReadOnlyList<byte[]>? nodes = null,
            IReadOnlyList<byte[]>? inclusionBranch = null,
            int sourceDomain = EthereumMainnetSccp.DomainEthereum)
            => EthereumMainnetSccp.CanonicalEvmSccpReceiptProofBytes(
                digest ?? sourceEventDigest,
                beaconSlot: 32,
                executionBlockNumber: 0x1234,
                executionBlockHash: "0x" + new string('b', 64),
                executionReceiptsRoot: "0x" + new string('c', 64),
                beaconFinalizedRoot: "0x" + new string('d', 64),
                syncCommitteeRoot: "0x" + new string('a', 64),
                receiptRootIndex: 3,
                receiptTrieProofNodes: nodes ?? proofNodes,
                inclusionBranch: inclusionBranch ?? branch,
                sourceDomain: sourceDomain);

        var bytes = BuildBytes();
        Assert.Equal(240, bytes.Length);
        Assert.Equal(ExpectedReceiptProofBytes, Convert.ToHexString(bytes).ToLowerInvariant());
        Assert.Equal(
            ExpectedReceiptProofHash,
            EthereumMainnetSccp.EvmSccpReceiptProofHash(
                sourceEventDigest,
                beaconSlot: 32,
                executionBlockNumber: 0x1234,
                executionBlockHash: "0x" + new string('b', 64),
                executionReceiptsRoot: "0x" + new string('c', 64),
                beaconFinalizedRoot: "0x" + new string('d', 64),
                syncCommitteeRoot: "0x" + new string('a', 64),
                receiptRootIndex: 3,
                receiptTrieProofNodes: proofNodes,
                inclusionBranch: branch));

        Assert.Throws<ArgumentException>(() => BuildBytes(sourceDomain: 2));
        Assert.Throws<ArgumentException>(() => BuildBytes(digest: "0x" + new string('0', 64)));
        Assert.Throws<ArgumentException>(() => BuildBytes(nodes: Array.Empty<byte[]>()));
        Assert.Throws<ArgumentException>(() => BuildBytes(nodes: [new byte[0]]));
        Assert.Throws<ArgumentException>(() => BuildBytes(inclusionBranch: Array.Empty<byte[]>()));
        Assert.Throws<ArgumentException>(() => BuildBytes(inclusionBranch: [new byte[31]]));
    }

    [Fact]
    public async Task InboundEvidenceUsesMainnetRpcAndRejectsDrift()
    {
        var txHash = "0x" + new string('a', 64);
        var blockHash = "0x" + new string('b', 64);
        var sourceEventDigest = "0x" + new string('e', 64);
        var sourceBridgeEmitterAddress = "0x" + string.Concat(Enumerable.Repeat("12", 20));
        var unrelatedLog = new Dictionary<string, object?>
        {
            ["address"] = "0x" + new string('0', 40),
            ["topics"] = new object?[] { "0x" + new string('0', 64) },
            ["data"] = "0x1234",
        };
        var sourceEventLog = new Dictionary<string, object?>
        {
            ["address"] = sourceBridgeEmitterAddress,
            ["topics"] = new object?[] { EthereumMainnetSccp.SourceEventTopic, sourceEventDigest },
            ["data"] = "0x",
        };
        var receipt = new Dictionary<string, object?>
        {
            ["transactionHash"] = txHash,
            ["blockHash"] = blockHash,
            ["blockNumber"] = "0x1234",
            ["status"] = "0x1",
        };
        var receiptWithSourceEvent = new Dictionary<string, object?>(receipt)
        {
            ["logs"] = new object?[] { unrelatedLog, sourceEventLog },
        };
        var block = new Dictionary<string, object?>
        {
            ["hash"] = blockHash,
            ["number"] = "0x1234",
            ["receiptsRoot"] = "0x" + new string('c', 64),
        };
        var beaconFinalityEvidence = new EthereumMainnetBeaconFinalityEvidence(
            "0x1234",
            blockHash,
            "0x" + new string('c', 64));
        var beaconFinality = beaconFinalityEvidence.ToDictionary(
            [new KeyValuePair<string, object?>("finalizedHeaderRoot", "0x" + new string('d', 64))]);
        var mutableReceiptProofNode = new byte[] { 0x01 };
        var mutableReceiptProofBranch = RepeatByte(0x11, 32);
        var receiptProof = new EthereumMainnetReceiptProof
        {
            SourceEventDigest = sourceEventDigest,
            BeaconSlot = 32,
            ExecutionBlockNumber = 0x1234,
            ExecutionBlockHash = blockHash,
            ExecutionReceiptsRoot = "0x" + new string('c', 64),
            BeaconFinalizedRoot = "0x" + new string('d', 64),
            SyncCommitteeRoot = "0x" + new string('a', 64),
            ReceiptRootIndex = 3,
            ReceiptTrieProofNodes = [mutableReceiptProofNode, new byte[] { 0x02, 0x03 }],
            InclusionBranch = [mutableReceiptProofBranch],
        };
        var provider = new ExecutionProviderStub("0x1", receipt, block);
        var consensusProvider = new ConsensusProviderStub(
            receipt,
            block,
            txHash,
            beaconFinality);

        var evidence = await EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
            new EthereumMainnetInboundEvidence { TransactionHash = txHash },
            provider,
            consensusProvider);
        Assert.Equal(txHash, evidence.TransactionHash);
        Assert.Equal("0x1", evidence.Receipt?["status"]);
        Assert.Equal("0x" + new string('c', 64), evidence.Block?["receiptsRoot"]);
        Assert.Equal("4660", evidence.BeaconFinality?["executionBlockNumber"]);
        Assert.Equal(blockHash, evidence.BeaconFinality?["executionBlockHash"]);
        Assert.Equal(1, consensusProvider.Calls);
        Assert.Equal(
            ["eth_chainId", "eth_getTransactionReceipt", "eth_getBlockByHash"],
            provider.Calls);

        var proofBytes = await EthereumMainnetSccp.ProveInboundToSoraAsync(
            evidence with
            {
                ReceiptProof = receiptProof,
                ReceiptProofHash = ExpectedReceiptProofHash,
            },
            new InboundProverStub(txHash, ExpectedReceiptProofHash, sourceEventDigest));
        Assert.Equal(new byte[] { 1, 2, 3 }, proofBytes);
        Assert.Equal(
            "submitted",
            await EthereumMainnetSccp.SubmitInboundToIrohaAsync(
                proofBytes,
                new InboundSubmitterStub()));

        var sourceEventEvidence = await EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
            new EthereumMainnetInboundEvidence
            {
                Receipt = receiptWithSourceEvent,
                Block = block,
                BeaconFinality = beaconFinality,
                SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
            });
        Assert.Equal(sourceEventDigest, sourceEventEvidence.SourceEventDigest);
        Assert.Equal(sourceBridgeEmitterAddress, sourceEventEvidence.SourceBridgeEmitterAddress);
        var explicitSourceEventEvidence = await EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
            new EthereumMainnetInboundEvidence
            {
                Receipt = receiptWithSourceEvent,
                Block = block,
                BeaconFinality = beaconFinality,
                SourceEventDigest = sourceEventDigest,
                SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
            });
        Assert.Equal(sourceEventDigest, explicitSourceEventEvidence.SourceEventDigest);

        var receiptProofEvidence = await EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
            new EthereumMainnetInboundEvidence
            {
                ReceiptProof = receiptProof,
                ReceiptProofHash = ExpectedReceiptProofHash,
            });
        Assert.Equal(ExpectedReceiptProofHash, receiptProofEvidence.ReceiptProofHash);
        Assert.NotSame(receiptProof, receiptProofEvidence.ReceiptProof);
        mutableReceiptProofNode[0] = 0x7f;
        mutableReceiptProofBranch[0] = 0x7f;
        Assert.Equal(new byte[] { 0x01 }, receiptProofEvidence.ReceiptProof!.ReceiptTrieProofNodes[0]);
        Assert.Equal(RepeatByte(0x11, 32), receiptProofEvidence.ReceiptProof.InclusionBranch[0]);
        mutableReceiptProofNode[0] = 0x01;
        mutableReceiptProofBranch[0] = 0x11;
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    ReceiptProof = receiptProof,
                    ReceiptProofHash = "0x" + new string('9', 64),
                }).AsTask());
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    ReceiptProof = receiptProof with { SourceDomain = 2 },
                }).AsTask());

        var typedFinalityProof = await EthereumMainnetSccp.ProveInboundToSoraAsync(
            new EthereumMainnetInboundEvidence
            {
                Receipt = receipt,
                Block = block,
                ReceiptProof = receiptProof,
                ReceiptProofHash = ExpectedReceiptProofHash,
            }.WithBeaconFinalityEvidence(
                beaconFinalityEvidence,
                [new KeyValuePair<string, object?>("finalizedHeaderRoot", "0x" + new string('d', 64))]),
            new InboundProverStub(txHash, ExpectedReceiptProofHash, sourceEventDigest));
        Assert.Equal(new byte[] { 1, 2, 3 }, typedFinalityProof);

        var missingFinalityProver = new CountingInboundProver();
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ProveInboundToSoraAsync(
                new EthereumMainnetInboundEvidence { Receipt = receipt, Block = block },
                missingFinalityProver).AsTask());
        Assert.Equal(0, missingFinalityProver.Calls);

        var missingReceiptProofProver = new CountingInboundProver();
        var missingReceiptProof = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ProveInboundToSoraAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receipt,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    ReceiptProofHash = ExpectedReceiptProofHash,
                },
                missingReceiptProofProver).AsTask());
        Assert.Contains("receiptProof", missingReceiptProof.Message);
        Assert.Equal(0, missingReceiptProofProver.Calls);

        var driftedReceiptProofProver = new CountingInboundProver();
        var driftedReceiptProof = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ProveInboundToSoraAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receipt,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    ReceiptProof = receiptProof with
                    {
                        ExecutionReceiptsRoot = "0x" + new string('9', 64),
                    },
                },
                driftedReceiptProofProver).AsTask());
        Assert.Contains("receiptProof.executionReceiptsRoot", driftedReceiptProof.Message);
        Assert.Equal(0, driftedReceiptProofProver.Calls);

        await Assert.ThrowsAsync<ArgumentOutOfRangeException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence { Receipt = receipt },
                new ExecutionProviderStub("0x38", receipt, block)).AsTask());
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ValidateExecutionProviderMainnetAsync(
                new ExecutionProviderStub("0x01", receipt, block)).AsTask());
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ValidateExecutionProviderMainnetAsync(
                new ExecutionProviderStub("1", receipt, block)).AsTask());
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ValidateExecutionProviderMainnetAsync(
                new ExecutionProviderStub(1, receipt, block)).AsTask());

        var failedReceipt = new Dictionary<string, object?>(receipt)
        {
            ["status"] = "0x0",
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence { Receipt = failedReceipt, Block = block }).AsTask());

        var missingReceiptBlockNumber = new Dictionary<string, object?>(receipt);
        missingReceiptBlockNumber.Remove("blockNumber");
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = missingReceiptBlockNumber,
                    Block = block,
                }).AsTask());

        var zeroReceiptBlockNumber = new Dictionary<string, object?>(receipt)
        {
            ["blockNumber"] = "0x0",
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = zeroReceiptBlockNumber,
                    Block = block,
                }).AsTask());

        var missingProvider = await Assert.ThrowsAsync<InvalidOperationException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence { TransactionHash = txHash }).AsTask());
        Assert.Contains("execution provider", missingProvider.Message);

        var driftedReceipt = new Dictionary<string, object?>(receipt)
        {
            ["transactionHash"] = "0x" + new string('d', 64),
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    TransactionHash = txHash,
                    Receipt = driftedReceipt,
                    Block = block,
                }).AsTask());

        var driftedBlock = new Dictionary<string, object?>(block)
        {
            ["hash"] = "0x" + new string('e', 64),
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence { Receipt = receipt, Block = driftedBlock }).AsTask());

        var missingBlockNumber = new Dictionary<string, object?>(block);
        missingBlockNumber.Remove("number");
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence { Receipt = receipt, Block = missingBlockNumber }).AsTask());

        var zeroBlockNumber = new Dictionary<string, object?>(block)
        {
            ["number"] = "0x0",
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence { Receipt = receipt, Block = zeroBlockNumber }).AsTask());

        var uppercaseReceipt = new Dictionary<string, object?>(receipt)
        {
            ["transactionHash"] = txHash.ToUpperInvariant(),
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence { Receipt = uppercaseReceipt, Block = block }).AsTask());

        var driftedFinalityHash = new Dictionary<string, object?>(beaconFinality)
        {
            ["executionBlockHash"] = "0x" + new string('e', 64),
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receipt,
                    Block = block,
                    BeaconFinality = driftedFinalityHash,
                }).AsTask());

        var driftedFinalityNumber = new Dictionary<string, object?>(beaconFinality)
        {
            ["executionBlockNumber"] = "0x1235",
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receipt,
                    Block = block,
                    BeaconFinality = driftedFinalityNumber,
                }).AsTask());

        var driftedFinalityReceiptsRoot = new Dictionary<string, object?>(beaconFinality)
        {
            ["executionReceiptsRoot"] = "0x" + new string('e', 64),
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receipt,
                    Block = block,
                    BeaconFinality = driftedFinalityReceiptsRoot,
                }).AsTask());

        var missingReceiptRootBlock = new Dictionary<string, object?>(block);
        missingReceiptRootBlock.Remove("receiptsRoot");
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receipt,
                    Block = missingReceiptRootBlock,
                }).AsTask());

        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receiptWithSourceEvent,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    SourceEventDigest = sourceEventDigest,
                }).AsTask());

        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receipt,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());

        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receiptWithSourceEvent,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    SourceBridgeEmitterAddress = "0x" + string.Concat(Enumerable.Repeat("13", 20)),
                }).AsTask());

        var wrongTopicLog = new Dictionary<string, object?>(sourceEventLog)
        {
            ["topics"] = new object?[] { "0x" + new string('a', 64), sourceEventDigest },
        };
        var wrongTopicReceipt = new Dictionary<string, object?>(receipt)
        {
            ["logs"] = new object?[] { wrongTopicLog },
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = wrongTopicReceipt,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());

        var duplicateReceipt = new Dictionary<string, object?>(receipt)
        {
            ["logs"] = new object?[] { sourceEventLog, sourceEventLog },
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = duplicateReceipt,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());

        var removedLog = new Dictionary<string, object?>(sourceEventLog)
        {
            ["removed"] = true,
        };
        var removedReceipt = new Dictionary<string, object?>(receipt)
        {
            ["logs"] = new object?[] { removedLog },
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = removedReceipt,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());

        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.SubmitInboundToIrohaAsync(
                [0, 0],
                new InboundSubmitterStub()).AsTask());
    }

    [Fact]
    public async Task OutboundProofRequestCalldataAndSubmitUseEthereumMainnetBinding()
    {
        var binding = SampleDestinationBinding();
        var publicInputs = SamplePublicInputs();
        var input = SampleOutboundInput(binding, publicInputs);

        var request = EthereumMainnetSccp.BuildOutboundProofRequest(input);
        Assert.Equal(1, request.Version);
        Assert.Equal(EthereumMainnetSccp.EvmGroth16Bn254ProofBackend, request.Backend);
        Assert.Equal(EthereumMainnetSccp.DomainSora, request.SourceDomain);
        Assert.Equal(EthereumMainnetSccp.DomainEthereum, request.TargetDomain);
        Assert.Equal(ExpectedBindingHash, request.DestinationBindingHash);
        Assert.Equal(ExpectedRequestHash, request.RequestHash);
        Assert.Equal(ExpectedPublicInputsBytes, Convert.ToHexString(request.PublicInputsBytes).ToLowerInvariant());
        Assert.Equal(ExpectedPublicSignalWords, request.PublicSignalWords);
        Assert.Equal("eth-mainnet-bundle"u8.ToArray(), request.BundleBytes);
        Assert.Equal("eth-source-proof"u8.ToArray(), request.SourceProofBytes);
        Assert.NotSame(input.BundleBytes, request.BundleBytes);
        Assert.NotSame(input.SourceProofBytes, request.SourceProofBytes);

        var mutableProof = Groth16ProofBytes();
        var prover = new OutboundProverStub(mutableProof);
        var proofResult = await EthereumMainnetSccp.ProveOutboundToEthereumAsync(input, prover);
        mutableProof[31] = 9;
        Assert.NotNull(prover.Request);
        Assert.Equal(1, proofResult.ProofBytes[31]);
        Assert.Equal(ExpectedRequestHash, proofResult.RequestHash);
        Assert.Equal(ExpectedEnvelopeHash, proofResult.EnvelopeHash);
        Assert.Equal(ExpectedPublicSignalWords, proofResult.PublicSignalWords);
        Assert.Equal(publicInputs, proofResult.PublicInputs);
        Assert.Equal(binding, proofResult.DestinationBinding);

        var submission = EthereumMainnetSccp.BuildEthereumCalldata(
            new EthereumMainnetSccpSubmissionInput(proofResult));
        Assert.Equal(1, submission.Version);
        Assert.Equal(EthereumMainnetSccp.StarkFriProofFamily, submission.ProofFamily);
        Assert.Equal(EthereumMainnetSccp.EvmGroth16Bn254ProofBackend, submission.VerifierBackend);
        Assert.Equal(EthereumMainnetSccp.ContractCallAbiTuple, submission.EnvelopeEncoding);
        Assert.Equal(EthereumMainnetSccp.SubmitMessageProofAbi, submission.ContractMethod);
        Assert.Equal(EthereumMainnetSccp.SubmitMessageProofSelector, submission.FunctionSelector);
        Assert.Equal(EthereumMainnetSccp.DomainSora, submission.SourceDomain);
        Assert.Equal(EthereumMainnetSccp.DomainEthereum, submission.TargetDomain);
        Assert.Equal(ExpectedPublicInputWords, submission.PublicInputWords);
        Assert.Equal(ExpectedPublicSignalWords, submission.PublicSignalWords);
        Assert.Equal(ExpectedCallDataHex, submission.CallDataHex);
        Assert.Equal(676, submission.CallData.Length);
        Assert.Equal(submission.CallData, submission.EnvelopeBytes);
        Assert.Equal(submission.CallDataHex, submission.EnvelopeHex);

        var submitter = new OutboundSubmitterStub();
        Assert.Equal(
            "eth-submitted",
            await EthereumMainnetSccp.SubmitOutboundToEthereumAsync(
                new EthereumMainnetSccpSubmissionInput(proofResult),
                submitter));
        Assert.NotNull(submitter.Submission);
        Assert.Equal(ExpectedCallDataHex, submitter.Submission.CallDataHex);
    }

    [Fact]
    public void LocalAdmissionSubmissionWrapsNativeEthereumOutput()
    {
        var input = new EthereumMainnetLocalAdmissionSubmissionInput(
            ProofBytes: [1, 2, 3],
            PublicInputsBytes: [4, 5, 6],
            BundleBytes: [7, 8, 9],
            EnvelopeBytes: [10, 11, 12],
            StatementHash: "0x" + new string('6', 64),
            SourceVerifierMaterialHash: "0x" + new string('7', 64),
            SourceAdapterEngineDeploymentHash: "0x" + new string('8', 64));
        var submission = EthereumMainnetSccp.BuildLocalAdmissionSubmission(input);

        Assert.Equal(EthereumMainnetSccp.LocalAdmissionSubmissionKind, submission.PlatformPayload);
        Assert.Equal(EthereumMainnetSccp.LocalAdmissionEnvelopeEncoding, submission.EnvelopeEncoding);
        Assert.Equal(EthereumMainnetSccp.LocalAdmissionEntrypoint, submission.VerifierEntrypoint);
        Assert.Equal(EthereumMainnetSccp.DomainEthereum, submission.SourceDomain);
        Assert.Equal(EthereumMainnetSccp.DomainSora, submission.TargetDomain);
        Assert.Empty(submission.Arguments);
        Assert.Equal([1, 2, 3], submission.ProofBytes);
        Assert.Equal([4, 5, 6], submission.PublicInputsBytes);
        Assert.Equal([7, 8, 9], submission.BundleBytes);
        Assert.Equal([10, 11, 12], submission.EnvelopeBytes);
        Assert.Equal([1, 2, 3], submission.LocalAdmission.ProofBytes);
        Assert.Equal("0x0a0b0c", submission.EnvelopeHex);

        input.ProofBytes[0] = 99;
        Assert.Equal([1, 2, 3], submission.ProofBytes);

        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                SourceDomain = BscMainnetSccp.DomainBsc,
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                ProofBytes = [0, 0],
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                PublicInputsBytes = [0, 0],
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                BundleBytes = [0, 0],
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                EnvelopeBytes = [],
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                EnvelopeBytes = [0, 0],
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                StatementHash = "0x" + new string('0', 64),
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                SourceVerifierMaterialHash = "0x" + new string('0', 64),
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                SourceAdapterEngineDeploymentHash = "0x" + new string('0', 64),
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                EnvelopeEncoding = "abi_tuple_v1",
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                ProofFamily = "debug-proof-family",
            }));
    }

    [Fact]
    public void OutboundProofPathRejectsCrossLaneAndMalformedProofs()
    {
        var binding = SampleDestinationBinding();
        var publicInputs = SamplePublicInputs();
        var input = SampleOutboundInput(binding, publicInputs);
        var request = EthereumMainnetSccp.BuildOutboundProofRequest(input);
        var proofResult = EthereumMainnetSccp.WrapOutboundProofResult(Groth16ProofBytes(), request);

        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildOutboundProofRequest(
                SampleOutboundInput(
                    binding,
                    publicInputs with { TargetDomain = 2 })));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildOutboundProofRequest(
                input with
                {
                    DestinationBindingHash = "0x" + new string('9', 64),
                }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildEthereumCalldata(
                new EthereumMainnetSccpSubmissionInput(
                    proofResult with
                    {
                        DestinationBinding = proofResult.DestinationBinding with
                        {
                            NetworkId =
                                "0x0000000000000000000000000000000000000000000000000000000000000038",
                        },
                    })));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildEthereumCalldata(
                new EthereumMainnetSccpSubmissionInput(
                    proofResult with
                    {
                        DestinationBindingHash = "0x" + new string('9', 64),
                    })));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildEthereumCalldata(
                new EthereumMainnetSccpSubmissionInput(
                    proofResult with
                    {
                        PublicInputs = publicInputs with
                        {
                            PayloadHash = "0x" + new string('9', 64),
                        },
                    })));

        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.WrapOutboundProofResult([1, 2, 3], request));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.WrapOutboundProofResult(new byte[384], request));

        var wrongMessageId = Groth16ProofBytes();
        wrongMessageId[63] = 0x12;
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.WrapOutboundProofResult(wrongMessageId, request));

        var wrongSourceDomain = Groth16ProofBytes();
        wrongSourceDomain[(2 * 32) + 31] = 0x02;
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.WrapOutboundProofResult(wrongSourceDomain, request));

        var badG1Point = Groth16ProofBytes();
        badG1Point[(5 * 32) + 31] = 0x03;
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.WrapOutboundProofResult(badG1Point, request));
    }
}
