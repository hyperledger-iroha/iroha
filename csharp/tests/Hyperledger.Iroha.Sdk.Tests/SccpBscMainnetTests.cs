using System.Buffers.Binary;
using Hyperledger.Iroha.Sccp;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class SccpBscMainnetTests
{
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
        string expectedTransactionHash,
        byte[]? proofBytes = null) : IBscMainnetInboundProver
    {
        public ValueTask<byte[]> ProveAsync(
            BscMainnetInboundEvidence evidence,
            CancellationToken cancellationToken = default)
        {
            Assert.Equal(BscMainnetSccp.DomainBsc, evidence.SourceDomain);
            Assert.Equal(BscMainnetSccp.DomainSora, evidence.TargetDomain);
            Assert.Equal(expectedTransactionHash, evidence.TransactionHash);
            Assert.NotNull(evidence.ParliaFinality);
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

    private static BscMainnetTransparentPublicInputs SamplePublicInputs()
        => new(
            Version: 1,
            MessageId: "0x" + new string('1', 64),
            PayloadHash: "0x" + new string('2', 64),
            TargetDomain: BscMainnetSccp.DomainBsc,
            CommitmentRoot: "0x" + new string('3', 64),
            FinalityHeight: 42,
            FinalityBlockHash: "0x" + new string('4', 64));

    private static BscMainnetOutboundProofRequestInput SampleOutboundInput(
        BscMainnetSccpDestinationBinding? binding = null,
        BscMainnetTransparentPublicInputs? publicInputs = null)
    {
        var selectedBinding = binding ?? SampleDestinationBinding();
        return new BscMainnetOutboundProofRequestInput
        {
            PublicInputs = publicInputs ?? SamplePublicInputs(),
            BundleBytes = "bsc-mainnet-bundle"u8.ToArray(),
            SourceProofBytes = "bsc-source-proof"u8.ToArray(),
            StatementHash = "0x" + new string('5', 64),
            DestinationBinding = selectedBinding,
            DestinationBindingHash = selectedBinding.BindingHash,
            SourceDomain = BscMainnetSccp.DomainSora,
        };
    }

    private static byte[] Groth16ProofBytes()
        => Concat(
            AbiWord(1),
            RepeatByte(0x11, 32),
            AbiWord((ulong)BscMainnetSccp.DomainSora),
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
                "0X" + new string('b', 64).ToUpperInvariant(),
                "0X" + new string('c', 64).ToUpperInvariant(),
                expectedBindingHash: binding.BindingHash,
                expectedKey: binding.Key).BindingHash);

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
    }

    [Fact]
    public void LocalAdmissionSubmissionWrapsNativeBscOutput()
    {
        var input = new BscMainnetLocalAdmissionSubmissionInput(
            ProofBytes: [1, 2, 3],
            PublicInputsBytes: [4, 5, 6],
            BundleBytes: [7, 8, 9],
            EnvelopeBytes: [10, 11, 12],
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

        input.ProofBytes[0] = 99;
        Assert.Equal([1, 2, 3], submission.ProofBytes);

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
        var receipt = new Dictionary<string, object?>
        {
            ["transactionHash"] = txHash,
            ["blockHash"] = blockHash,
            ["blockNumber"] = "0x1234",
            ["status"] = "0x1",
        };
        var block = new Dictionary<string, object?>
        {
            ["hash"] = blockHash,
            ["number"] = "0x1234",
            ["receiptsRoot"] = receiptsRoot,
        };
        var parliaFinalityEvidence = new BscMainnetParliaFinalityEvidence(
            "0x1234",
            blockHash,
            receiptsRoot);
        var parliaFinality = parliaFinalityEvidence.ToDictionary(
            new Dictionary<string, object?>
            {
                ["validatorSetHash"] = "0x" + new string('d', 64),
                ["commitSealCount"] = 15,
            });
        var provider = new ExecutionProviderStub("0x38", receipt, block);

        var evidence = await BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
            new BscMainnetInboundEvidence
            {
                TransactionHash = txHash,
            }.WithParliaFinalityEvidence(
                parliaFinalityEvidence,
                new Dictionary<string, object?>
                {
                    ["validatorSetHash"] = "0x" + new string('d', 64),
                    ["commitSealCount"] = 15,
                }),
            provider);
        Assert.Equal(BscMainnetSccp.DomainBsc, evidence.SourceDomain);
        Assert.Equal(BscMainnetSccp.DomainSora, evidence.TargetDomain);
        Assert.Equal(txHash, evidence.TransactionHash);
        Assert.Equal("0x1", evidence.Receipt?["status"]);
        Assert.Equal(receiptsRoot, evidence.Block?["receiptsRoot"]);
        Assert.Equal("4660", evidence.ParliaFinality?["executionBlockNumber"]);
        Assert.Equal(blockHash, evidence.ParliaFinality?["executionBlockHash"]);
        Assert.Equal(receiptsRoot, evidence.ParliaFinality?["executionReceiptsRoot"]);
        Assert.Equal(
            ["eth_chainId", "eth_getTransactionReceipt", "eth_getBlockByHash"],
            provider.Calls);
        var providerFinalityEvidence = await BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
            new BscMainnetInboundEvidence { TransactionHash = txHash },
            provider,
            new ConsensusProviderStub(parliaFinality));
        Assert.Equal(blockHash, providerFinalityEvidence.ParliaFinality?["executionBlockHash"]);

        var proofBytes = await BscMainnetSccp.ProveInboundToSoraAsync(
            evidence,
            new InboundProverStub(txHash));
        Assert.Equal(new byte[] { 1, 2, 3 }, proofBytes);
        var mutableProverOutput = new byte[] { 4, 5, 6 };
        var copiedProofBytes = await BscMainnetSccp.ProveInboundToSoraAsync(
            evidence,
            new InboundProverStub(txHash, mutableProverOutput));
        mutableProverOutput[0] = 9;
        Assert.Equal(new byte[] { 4, 5, 6 }, copiedProofBytes);
        Assert.Equal(
            "submitted",
            await BscMainnetSccp.SubmitInboundToIrohaAsync(
                proofBytes,
                new InboundSubmitterStub()));
        var mutableSubmitInput = new byte[] { 1, 2, 3 };
        var recordingSubmitter = new RecordingInboundSubmitterStub();
        Assert.Equal(
            "submitted",
            await BscMainnetSccp.SubmitInboundToIrohaAsync(
                mutableSubmitInput,
                recordingSubmitter));
        mutableSubmitInput[0] = 9;
        Assert.Equal(new byte[] { 1, 2, 3 }, recordingSubmitter.SubmittedProofBytes);

        await Assert.ThrowsAsync<ArgumentOutOfRangeException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence { Receipt = receipt },
                new ExecutionProviderStub("0x1", receipt, block)).AsTask());
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.ValidateExecutionProviderMainnetAsync(
                new ExecutionProviderStub("0x038", receipt, block)).AsTask());
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence
                {
                    SourceDomain = EthereumMainnetSccp.DomainEthereum,
                    Receipt = receipt,
                    Block = block,
                }).AsTask());
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence { TransactionHash = txHash }).AsTask());

        var proofHashOnly = await BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
            new BscMainnetInboundEvidence
            {
                ReceiptProofHash = "0x" + new string('e', 64),
            });
        Assert.Equal("0x" + new string('e', 64), proofHashOnly.ReceiptProofHash);
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence
                {
                    ReceiptProofHash = "0x" + new string('0', 64),
                }).AsTask());

        var failedReceipt = new Dictionary<string, object?>(receipt)
        {
            ["status"] = "0x0",
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence { Receipt = failedReceipt, Block = block }).AsTask());

        var missingReceiptBlockNumber = new Dictionary<string, object?>(receipt);
        missingReceiptBlockNumber.Remove("blockNumber");
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence
                {
                    Receipt = missingReceiptBlockNumber,
                    Block = block,
                }).AsTask());

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
                }).AsTask());

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
                }).AsTask());

        var driftedBlock = new Dictionary<string, object?>(block)
        {
            ["hash"] = "0x" + new string('e', 64),
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence { Receipt = receipt, Block = driftedBlock }).AsTask());

        var missingBlockNumber = new Dictionary<string, object?>(block);
        missingBlockNumber.Remove("number");
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence { Receipt = receipt, Block = missingBlockNumber }).AsTask());

        var zeroBlockNumber = new Dictionary<string, object?>(block)
        {
            ["number"] = "0x0",
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence { Receipt = receipt, Block = zeroBlockNumber }).AsTask());

        var wrongNumberBlock = new Dictionary<string, object?>(block)
        {
            ["number"] = "0x1235",
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence { Receipt = receipt, Block = wrongNumberBlock }).AsTask());

        var uppercaseReceipt = new Dictionary<string, object?>(receipt)
        {
            ["transactionHash"] = txHash.ToUpperInvariant(),
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence { Receipt = uppercaseReceipt, Block = block }).AsTask());

        var missingReceiptRootBlock = new Dictionary<string, object?>(block);
        missingReceiptRootBlock.Remove("receiptsRoot");
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new BscMainnetInboundEvidence
                {
                    Receipt = receipt,
                    Block = missingReceiptRootBlock,
                }).AsTask());

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
                }).AsTask());

        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.ProveInboundToSoraAsync(
                new BscMainnetInboundEvidence
                {
                    ReceiptProofHash = "0x" + new string('e', 64),
                },
                new InboundProverStub(txHash)).AsTask());

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
                }).AsTask());

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
                }).AsTask());

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
                }).AsTask());

        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.ProveInboundToSoraAsync(
                evidence,
                new InboundProverStub(txHash, [0, 0])).AsTask());
        await Assert.ThrowsAsync<ArgumentException>(
            () => BscMainnetSccp.SubmitInboundToIrohaAsync(
                [0, 0],
                new InboundSubmitterStub()).AsTask());
    }

    [Fact]
    public async Task OutboundProofRequestCalldataAndSubmitUseBscMainnetBinding()
    {
        var binding = SampleDestinationBinding();
        var publicInputs = SamplePublicInputs();
        var input = SampleOutboundInput(binding, publicInputs);

        var request = BscMainnetSccp.BuildOutboundProofRequest(input);
        Assert.Equal(1, request.Version);
        Assert.Equal(BscMainnetSccp.EvmGroth16Bn254ProofBackend, request.Backend);
        Assert.Equal(BscMainnetSccp.DomainSora, request.SourceDomain);
        Assert.Equal(BscMainnetSccp.DomainBsc, request.TargetDomain);
        Assert.Equal(binding.BindingHash, request.DestinationBindingHash);
        Assert.Equal(binding, request.DestinationBinding);
        Assert.Equal(publicInputs, request.PublicInputs);
        Assert.Equal(9, request.PublicSignalWords.Length);
        Assert.NotSame(input.BundleBytes, request.BundleBytes);
        Assert.NotSame(input.SourceProofBytes, request.SourceProofBytes);

        var mutableProof = Groth16ProofBytes();
        var prover = new OutboundProverStub(mutableProof);
        var proofResult = await BscMainnetSccp.ProveOutboundToBscAsync(input, prover);
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

        var submitter = new OutboundSubmitterStub();
        Assert.Equal(
            "bsc-submitted",
            await BscMainnetSccp.SubmitOutboundToBscAsync(
                new BscMainnetSccpSubmissionInput(proofResult),
                submitter));
        Assert.NotNull(submitter.Submission);
        Assert.Equal(submission.CallDataHex, submitter.Submission.CallDataHex);
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
