using System.Buffers.Binary;
using System.Numerics;
using System.Text;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Sccp;

/// <summary>
/// Ethereum mainnet SCCP constants, route validators, and destination bindings for native .NET callers.
/// </summary>
public static class EthereumMainnetSccp
{
    public const int DomainSora = 0;
    public const int DomainEthereum = 1;
    public const ulong MainnetChainId = 1;
    public const string EvmGroth16Bn254ProofBackend = "evm-groth16-bn254-v1";
    public const string StarkFriProofFamily = "stark-fri-v1";
    public const string ContractCallAbiTuple = "abi_tuple_v1";
    public const string LocalAdmissionEnvelopeEncoding = "norito:sccp-local-admission:v1";
    public const string LocalAdmissionSubmissionKind = "local_admission";
    public const string LocalAdmissionEntrypoint = "SubmitBridgeProof";
    public const int NativeRecursiveMaxProofBytes = 2 * 1024 * 1024;
    public const string SourceEventAbi = "SccpSourceEvent(bytes32)";
    public const string SourceEventTopic =
        "0x577b41c65ffbce226de59f224b464797257063747891b88ebec1bcd57af82727";
    public const string SubmitMessageProofAbi = "submitSccpMessageProof(bytes,bytes32[6],bytes32)";
    public const string SubmitMessageProofSelector = "0xbd57826c";
    public const string MainnetNetworkId =
        "0x0000000000000000000000000000000000000000000000000000000000000001";

    private const string EvmDestinationBindingLabel = "iroha:sccp:evm-destination-binding:v1";
    private const string EvmReceiptProofPrefix = "sccp:evm:receipt-proof:v1";
    private const string ProofRequestPrefix = "sccp:evm:groth16-proof-request:v1";
    private const string ProofEnvelopePrefix = "sccp:evm:groth16-proof-envelope:v1";
    private const int Groth16Bn254ProofAbiByteLength = 384;
    private const int Keccak256Rate = 136;
    private const int MaxSourceMerkleBranchNodes = 64;
    private const int MaxMptProofNodes = 64;
    private const int MaxMptNodeBytes = 16 * 1024;
    private static readonly byte[] SubmitMessageProofSelectorBytes = [0xbd, 0x57, 0x82, 0x6c];
    private static readonly BigInteger Bn254ScalarFieldModulus =
        new(
            Convert.FromHexString("30644e72e131a029b85045b68181585d2833e84879b9709143e1f593f0000001"),
            isUnsigned: true,
            isBigEndian: true);
    private static readonly BigInteger Bn254BaseFieldModulus =
        new(
            Convert.FromHexString("30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47"),
            isUnsigned: true,
            isBigEndian: true);
    private static readonly BigInteger Bn254G2BC0 =
        new(
            Convert.FromHexString("2b149d40ceb8aaae81be18991be06ac3b5b4c5e559dbefa33267e6dc24a138e5"),
            isUnsigned: true,
            isBigEndian: true);
    private static readonly BigInteger Bn254G2BC1 =
        new(
            Convert.FromHexString("009713b03af0fed4cd2cafadeed8fdf4a74fa084e52d1852e4a2bd0685c315d2"),
            isUnsigned: true,
            isBigEndian: true);
    private static readonly int[] Bn254ScalarFieldBits = ScalarBits(Bn254ScalarFieldModulus);
    private static readonly string[] Groth16Bn254SignalLabels =
    [
        "sccp:groth16-bn254:signal:message-id:v1",
        "sccp:groth16-bn254:signal:payload-hash:v1",
        "sccp:groth16-bn254:signal:target-domain:v1",
        "sccp:groth16-bn254:signal:commitment-root:v1",
        "sccp:groth16-bn254:signal:finality-height:v1",
        "sccp:groth16-bn254:signal:finality-block-hash:v1",
        "sccp:groth16-bn254:signal:source-domain:v1",
        "sccp:groth16-bn254:signal:statement-hash:v1",
        "sccp:groth16-bn254:signal:destination-binding-hash:v1",
    ];

    private static readonly int[] KeccakRhoOffsets =
    [
        0, 1, 62, 28, 27,
        36, 44, 6, 55, 20,
        3, 10, 43, 25, 39,
        41, 45, 15, 21, 8,
        18, 2, 61, 56, 14,
    ];

    private static readonly ulong[] KeccakRoundConstants =
    [
        0x0000000000000001UL,
        0x0000000000008082UL,
        0x800000000000808aUL,
        0x8000000080008000UL,
        0x000000000000808bUL,
        0x0000000080000001UL,
        0x8000000080008081UL,
        0x8000000000008009UL,
        0x000000000000008aUL,
        0x0000000000000088UL,
        0x0000000080008009UL,
        0x000000008000000aUL,
        0x000000008000808bUL,
        0x800000000000008bUL,
        0x8000000000008089UL,
        0x8000000000008003UL,
        0x8000000000008002UL,
        0x8000000000000080UL,
        0x000000000000800aUL,
        0x800000008000000aUL,
        0x8000000080008081UL,
        0x8000000000008080UL,
        0x0000000080000001UL,
        0x8000000080008008UL,
    ];

    private sealed record SourceEvent(string? SourceEventDigest, string? SourceBridgeEmitterAddress);

    private readonly record struct Bn254Fq2(BigInteger C0, BigInteger C1);

    private readonly record struct Bn254G2Projective(
        Bn254Fq2 X,
        Bn254Fq2 Y,
        Bn254Fq2 Z,
        bool Infinity);

    public static void RequireMainnetChainId(ulong chainId)
    {
        if (chainId != MainnetChainId)
        {
            throw new ArgumentOutOfRangeException(
                nameof(chainId),
                chainId,
                "Ethereum mainnet SCCP requires eth_chainId == 1.");
        }
    }

    public static async ValueTask<object?> ValidateExecutionProviderMainnetAsync(
        IEthereumMainnetExecutionProvider executionProvider,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(executionProvider);

        var chainId = await executionProvider.RequestAsync(
            "eth_chainId",
            Array.Empty<object?>(),
            cancellationToken).ConfigureAwait(false);
        RequireMainnetChainId(NormalizeMainnetChainId(chainId));
        return chainId;
    }

    public static async ValueTask<EthereumMainnetInboundEvidence> CollectInboundEvidenceFromReceiptAsync(
        EthereumMainnetInboundEvidence input,
        IEthereumMainnetExecutionProvider? executionProvider = null,
        IEthereumMainnetConsensusProvider? consensusProvider = null,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(input);
        RequireInboundRoute(input.SourceDomain, input.TargetDomain);

        if (executionProvider is not null)
        {
            _ = await ValidateExecutionProviderMainnetAsync(
                executionProvider,
                cancellationToken).ConfigureAwait(false);
        }

        var transactionHash = input.TransactionHash is null
            ? null
            : NormalizeRpcHex(input.TransactionHash, nameof(input.TransactionHash), 32);
        var receipt = input.Receipt;
        if (receipt is null && transactionHash is not null)
        {
            if (executionProvider is null)
            {
                throw new InvalidOperationException(
                    "Ethereum mainnet execution provider is not linked for transactionHash evidence collection.");
            }
            receipt = RequireDictionary(
                await executionProvider.RequestAsync(
                    "eth_getTransactionReceipt",
                    new object?[] { transactionHash },
                    cancellationToken).ConfigureAwait(false),
                "eth_getTransactionReceipt");
        }

        var receiptProof = SnapshotReceiptProof(input.ReceiptProof);
        if (receipt is null && receiptProof is null && input.ReceiptProofHash is null)
        {
            throw new ArgumentException(
                "Ethereum mainnet inbound evidence requires receipt, receiptProof, receiptProofHash, or transactionHash.",
                nameof(input));
        }

        string? blockHash = null;
        string? receiptBlockNumber = null;
        string? blockReceiptsRoot = null;
        if (receipt is not null)
        {
            if (!string.Equals(FirstPresent(receipt, "status") as string, "0x1", StringComparison.Ordinal))
            {
                throw new ArgumentException(
                    "Ethereum mainnet inbound receipt status must be 0x1.",
                    nameof(input));
            }

            var receiptTransactionHash = NormalizeRpcHex(
                FirstPresent(receipt, "transactionHash", "transaction_hash"),
                "receipt.transactionHash",
                32);
            if (transactionHash is not null
                && !string.Equals(transactionHash, receiptTransactionHash, StringComparison.Ordinal))
            {
                throw new ArgumentException(
                    "receipt.transactionHash must match transactionHash.",
                    nameof(input));
            }

            transactionHash = receiptTransactionHash;
            blockHash = NormalizeRpcHex(
                FirstPresent(receipt, "blockHash", "block_hash"),
                "receipt.blockHash",
                32);
            var receiptBlockNumberValue = FirstPresent(receipt, "blockNumber", "block_number");
            receiptBlockNumber = NormalizePositiveRpcQuantity(receiptBlockNumberValue, "receipt.blockNumber");
        }

        var block = input.Block;
        if (block is null && blockHash is not null && executionProvider is not null)
        {
            block = RequireDictionary(
                await executionProvider.RequestAsync(
                    "eth_getBlockByHash",
                    new object?[] { blockHash, false },
                    cancellationToken).ConfigureAwait(false),
                "eth_getBlockByHash");
        }

        if (block is not null)
        {
            var normalizedBlockHash = NormalizeRpcHex(
                FirstPresent(block, "hash"),
                "block.hash",
                32);
            if (blockHash is not null
                && !string.Equals(blockHash, normalizedBlockHash, StringComparison.Ordinal))
            {
                throw new ArgumentException(
                    "block.hash must match receipt.blockHash.",
                    nameof(input));
            }

            var blockNumberValue = FirstPresent(block, "number", "blockNumber", "block_number");
            var blockNumber = NormalizePositiveRpcQuantity(blockNumberValue, "block.number");
            if (receiptBlockNumber is not null
                && !string.Equals(receiptBlockNumber, blockNumber, StringComparison.Ordinal))
            {
                throw new ArgumentException(
                    "block.number must match receipt.blockNumber.",
                    nameof(input));
            }

            receiptBlockNumber = blockNumber;
            blockReceiptsRoot = NormalizeRpcHex(
                FirstPresent(block, "receiptsRoot", "receipts_root"),
                "block.receiptsRoot",
                32);
        }

        var beaconFinality = input.BeaconFinality;
        if (beaconFinality is null && consensusProvider is not null)
        {
            beaconFinality = await consensusProvider.CollectFinalityEvidenceAsync(
                receipt,
                block,
                transactionHash,
                cancellationToken).ConfigureAwait(false);
        }
        if (beaconFinality is not null)
        {
            beaconFinality = NormalizeBeaconFinality(
                beaconFinality,
                blockHash,
                receiptBlockNumber,
                blockReceiptsRoot);
        }

        var sourceEvent = NormalizeEthereumReceiptSourceEvent(
            receipt,
            input.SourceEventDigest,
            input.SourceBridgeEmitterAddress);
        RequireReceiptProofMatchesEvidence(
            receiptProof,
            blockHash,
            receiptBlockNumber,
            blockReceiptsRoot,
            beaconFinality,
            sourceEvent.SourceEventDigest);

        return input with
        {
            SourceDomain = DomainEthereum,
            TargetDomain = DomainSora,
            TransactionHash = transactionHash,
            Receipt = receipt,
            Block = block,
            BeaconFinality = beaconFinality,
            ReceiptProof = receiptProof,
            ReceiptProofHash = NormalizeReceiptProofHash(receiptProof, input.ReceiptProofHash),
            SourceEventDigest = sourceEvent.SourceEventDigest,
            SourceBridgeEmitterAddress = sourceEvent.SourceBridgeEmitterAddress,
        };
    }

    public static async ValueTask<byte[]> ProveInboundToSoraAsync(
        EthereumMainnetInboundEvidence input,
        IEthereumMainnetInboundProver inboundProver,
        IEthereumMainnetExecutionProvider? executionProvider = null,
        IEthereumMainnetConsensusProvider? consensusProvider = null,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(inboundProver);

        var evidence = await CollectInboundEvidenceFromReceiptAsync(
            input,
            executionProvider,
            consensusProvider,
            cancellationToken).ConfigureAwait(false);
        if (evidence.BeaconFinality is null)
        {
            throw new ArgumentException(
                "Ethereum mainnet SCCP inbound proof requires beaconFinality.",
                nameof(input));
        }
        if (evidence.ReceiptProof is null)
        {
            throw new ArgumentException(
                "Ethereum mainnet SCCP inbound proof requires receiptProof.",
                nameof(input));
        }

        var proofBytes = await inboundProver.ProveAsync(
            evidence,
            cancellationToken).ConfigureAwait(false);
        return RequireNonZeroProofBytes(proofBytes, nameof(proofBytes));
    }

    public static async ValueTask<object?> SubmitInboundToIrohaAsync(
        byte[] proofBytes,
        IEthereumMainnetInboundSubmitter inboundSubmitter,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(inboundSubmitter);

        var proofCopy = RequireNonZeroProofBytes(proofBytes, nameof(proofBytes));
        return await inboundSubmitter.SubmitAsync(proofCopy, cancellationToken).ConfigureAwait(false);
    }

    public static byte[] CanonicalEvmSccpReceiptProofBytes(
        string sourceEventDigest,
        ulong beaconSlot,
        ulong executionBlockNumber,
        string executionBlockHash,
        string executionReceiptsRoot,
        string beaconFinalizedRoot,
        string syncCommitteeRoot,
        ulong receiptRootIndex,
        IReadOnlyList<byte[]> receiptTrieProofNodes,
        IReadOnlyList<byte[]> inclusionBranch,
        int sourceDomain = DomainEthereum)
    {
        if (sourceDomain != DomainEthereum)
        {
            throw new ArgumentException("sourceDomain must be ETH.", nameof(sourceDomain));
        }

        var nodes = NormalizeReceiptTrieProofNodes(receiptTrieProofNodes);
        var branch = NormalizeReceiptInclusionBranch(inclusionBranch, requireNonEmpty: true);
        using var payload = new MemoryStream();
        payload.WriteByte(1);
        payload.Write(LeU32(sourceDomain));
        payload.Write(RpcHexToBytes(sourceEventDigest, nameof(sourceEventDigest), 32));
        payload.Write(LeU64(beaconSlot));
        payload.Write(LeU64(executionBlockNumber));
        payload.Write(RpcHexToBytes(executionBlockHash, nameof(executionBlockHash), 32, allowZero: true));
        payload.Write(RpcHexToBytes(executionReceiptsRoot, nameof(executionReceiptsRoot), 32, allowZero: true));
        payload.Write(RpcHexToBytes(beaconFinalizedRoot, nameof(beaconFinalizedRoot), 32, allowZero: true));
        payload.Write(RpcHexToBytes(syncCommitteeRoot, nameof(syncCommitteeRoot), 32, allowZero: true));
        payload.Write(LeU64(receiptRootIndex));
        payload.Write(LeU32(nodes.Count));
        foreach (var node in nodes)
        {
            payload.Write(WriteBytes(node));
        }

        payload.Write(LeU32(branch.Count));
        foreach (var sibling in branch)
        {
            payload.Write(sibling);
        }

        return payload.ToArray();
    }

    public static string EvmSccpReceiptProofHash(
        string sourceEventDigest,
        ulong beaconSlot,
        ulong executionBlockNumber,
        string executionBlockHash,
        string executionReceiptsRoot,
        string beaconFinalizedRoot,
        string syncCommitteeRoot,
        ulong receiptRootIndex,
        IReadOnlyList<byte[]> receiptTrieProofNodes,
        IReadOnlyList<byte[]> inclusionBranch,
        int sourceDomain = DomainEthereum)
        => PrefixedBlake2bHex(
            Encoding.UTF8.GetBytes(EvmReceiptProofPrefix),
            CanonicalEvmSccpReceiptProofBytes(
                sourceEventDigest,
                beaconSlot,
                executionBlockNumber,
                executionBlockHash,
                executionReceiptsRoot,
                beaconFinalizedRoot,
                syncCommitteeRoot,
                receiptRootIndex,
                receiptTrieProofNodes,
                inclusionBranch,
                sourceDomain));

    public static EthereumMainnetLocalAdmissionSubmission BuildLocalAdmissionSubmission(
        EthereumMainnetLocalAdmissionSubmissionInput input)
    {
        ArgumentNullException.ThrowIfNull(input);
        RequireInboundRoute(input.SourceDomain, input.TargetDomain);

        if (!string.Equals(input.EnvelopeEncoding, LocalAdmissionEnvelopeEncoding, StringComparison.Ordinal)
            || !string.Equals(input.SubmissionKind, LocalAdmissionSubmissionKind, StringComparison.Ordinal)
            || !string.Equals(input.VerifierEntrypoint, LocalAdmissionEntrypoint, StringComparison.Ordinal)
            || !string.Equals(input.ProofFamily, StarkFriProofFamily, StringComparison.Ordinal)
            || !string.Equals(input.VerifierBackend, EvmGroth16Bn254ProofBackend, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "Ethereum mainnet local-admission submission metadata is not canonical.",
                nameof(input));
        }

        var proofBytes = RequireNativeRecursiveBytes(input.ProofBytes, nameof(input.ProofBytes));
        var publicInputsBytes = RequireNativeRecursiveBytes(
            input.PublicInputsBytes,
            nameof(input.PublicInputsBytes));
        var bundleBytes = RequireNativeRecursiveBytes(input.BundleBytes, nameof(input.BundleBytes));
        var envelopeBytes = RequireNativeRecursiveBytes(input.EnvelopeBytes, nameof(input.EnvelopeBytes));
        var statementHash = NormalizeNonZeroHex(input.StatementHash, nameof(input.StatementHash), 32);
        var sourceVerifierMaterialHash = NormalizeNonZeroHex(
            input.SourceVerifierMaterialHash,
            nameof(input.SourceVerifierMaterialHash),
            32);
        var sourceAdapterEngineDeploymentHash = NormalizeNonZeroHex(
            input.SourceAdapterEngineDeploymentHash,
            nameof(input.SourceAdapterEngineDeploymentHash),
            32);
        var payload = new EthereumMainnetLocalAdmissionPayload(
            ProofBytes: proofBytes,
            PublicInputsBytes: publicInputsBytes,
            BundleBytes: bundleBytes,
            StatementHash: statementHash,
            SourceVerifierMaterialHash: sourceVerifierMaterialHash,
            SourceAdapterEngineDeploymentHash: sourceAdapterEngineDeploymentHash);

        return new EthereumMainnetLocalAdmissionSubmission(
            ProofFamily: input.ProofFamily,
            VerifierBackend: input.VerifierBackend,
            SourceDomain: DomainEthereum,
            TargetDomain: DomainSora,
            StatementHash: statementHash,
            SourceVerifierMaterialHash: sourceVerifierMaterialHash,
            SourceAdapterEngineDeploymentHash: sourceAdapterEngineDeploymentHash,
            LocalAdmission: payload,
            ProofBytes: proofBytes,
            PublicInputsBytes: publicInputsBytes,
            BundleBytes: bundleBytes,
            EnvelopeBytes: envelopeBytes);
    }

    public static EthereumMainnetOutboundProofRequest BuildOutboundProofRequest(
        EthereumMainnetOutboundProofRequestInput input)
    {
        ArgumentNullException.ThrowIfNull(input);
        ArgumentNullException.ThrowIfNull(input.PublicInputs);
        ArgumentNullException.ThrowIfNull(input.DestinationBinding);
        RequireOutboundRoute(input.SourceDomain, input.PublicInputs.TargetDomain);

        var publicInputs = NormalizePublicInputs(input.PublicInputs);
        var destinationBinding = RequireEthereumDestinationBinding(input.DestinationBinding);
        var destinationBindingHash = NormalizeNonZeroHex(
            input.DestinationBindingHash ?? destinationBinding.BindingHash,
            nameof(input.DestinationBindingHash),
            32);
        if (!string.Equals(destinationBindingHash, destinationBinding.BindingHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "destinationBindingHash must match Ethereum mainnet destinationBinding.",
                nameof(input));
        }

        var statementHash = NormalizeNonZeroHex(input.StatementHash, nameof(input.StatementHash), 32);
        var bundleBytes = RequireNonEmptyBytes(input.BundleBytes, nameof(input.BundleBytes));
        var sourceProofBytes = NormalizeOptionalNonZeroBytes(input.SourceProofBytes, nameof(input.SourceProofBytes));
        var publicInputsBytes = CanonicalPublicInputsBytes(publicInputs);
        var publicSignalWords = PublicSignalWords(
            publicInputs,
            input.SourceDomain,
            statementHash,
            destinationBindingHash);
        var requestHash = ComputeProofRequestHash(
            publicInputsBytes,
            bundleBytes,
            sourceProofBytes,
            statementHash,
            destinationBindingHash,
            publicSignalWords);

        return new EthereumMainnetOutboundProofRequest(
            Version: 1,
            Backend: EvmGroth16Bn254ProofBackend,
            SourceDomain: input.SourceDomain,
            TargetDomain: DomainEthereum,
            PublicInputs: publicInputs,
            PublicInputsBytes: publicInputsBytes,
            PublicSignalWords: publicSignalWords,
            BundleBytes: bundleBytes,
            SourceProofBytes: sourceProofBytes,
            ProofContext: new EthereumMainnetSccpProofContext(statementHash, destinationBindingHash),
            StatementHash: statementHash,
            DestinationBindingHash: destinationBindingHash,
            RequestHash: requestHash,
            DestinationBinding: destinationBinding);
    }

    public static async ValueTask<EthereumMainnetOutboundProofResult> ProveOutboundToEthereumAsync(
        EthereumMainnetOutboundProofRequestInput input,
        IEthereumMainnetOutboundProver outboundProver,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(outboundProver);

        var request = BuildOutboundProofRequest(input);
        var proofBytes = await outboundProver.ProveAsync(
            Snapshot(request),
            cancellationToken).ConfigureAwait(false);
        return WrapOutboundProofResult(proofBytes, request);
    }

    public static EthereumMainnetOutboundProofResult WrapOutboundProofResult(
        byte[] proofBytes,
        EthereumMainnetOutboundProofRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);
        RequireEthereumProofRequest(request);

        var proofCopy = RequireGroth16ProofBytesForContext(
            proofBytes,
            request.PublicInputs,
            request.SourceDomain,
            nameof(proofBytes));
        var requestHash = ComputeProofRequestHash(request);
        if (!string.Equals(requestHash, request.RequestHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "requestHash must match Ethereum mainnet proof request fields.",
                nameof(request));
        }

        var envelopeHash = PrefixedBlake2bHex(
            Encoding.UTF8.GetBytes(ProofEnvelopePrefix),
            Concat(HexToBytes(request.RequestHash, 32), proofCopy));
        return new EthereumMainnetOutboundProofResult(
            Version: 1,
            Backend: EvmGroth16Bn254ProofBackend,
            ProofBytes: proofCopy,
            ProofBase64: Convert.ToBase64String(proofCopy),
            Request: Snapshot(request),
            PublicInputs: request.PublicInputs,
            PublicSignalWords: request.PublicSignalWords.ToArray(),
            StatementHash: request.StatementHash,
            DestinationBindingHash: request.DestinationBindingHash,
            ProofContext: request.ProofContext,
            RequestHash: request.RequestHash,
            EnvelopeHash: envelopeHash,
            DestinationBinding: request.DestinationBinding);
    }

    public static EthereumMainnetSccpSubmission BuildEthereumCalldata(
        EthereumMainnetSccpSubmissionInput input)
    {
        ArgumentNullException.ThrowIfNull(input);
        ArgumentNullException.ThrowIfNull(input.ProofResult);
        var proofResult = input.ProofResult;
        RequireEthereumProofResult(proofResult);

        var publicInputWords = PublicInputAbiWords(proofResult.PublicInputs);
        var publicInputWordBytes = Concat(publicInputWords.Select(HexToBytes32).ToArray());
        var callData = SccpSubmitMessageProofCallData(
            proofResult.ProofBytes,
            publicInputWords,
            proofResult.StatementHash,
            proofResult.Request.SourceDomain);
        return new EthereumMainnetSccpSubmission(
            Version: 1,
            ProofFamily: StarkFriProofFamily,
            VerifierBackend: EvmGroth16Bn254ProofBackend,
            PlatformPayload: "evm_groth16_contract_call",
            EnvelopeEncoding: ContractCallAbiTuple,
            SubmissionKind: "contract_call",
            VerifierEntrypoint:
                "submitSccpMessageProof(bytes proof_bytes, bytes32[6] public_inputs, bytes32 statement_hash)",
            ContractMethod: SubmitMessageProofAbi,
            FunctionSelector: SubmitMessageProofSelector,
            SourceDomain: proofResult.Request.SourceDomain,
            TargetDomain: DomainEthereum,
            PublicInputs: proofResult.PublicInputs,
            PublicInputWords: publicInputWords,
            PublicSignalWords: proofResult.PublicSignalWords.ToArray(),
            StatementHash: proofResult.StatementHash,
            DestinationBindingHash: proofResult.DestinationBindingHash,
            Arguments:
            [
                new EthereumMainnetSccpSubmissionArgument(
                    "proof_bytes",
                    "raw_bytes",
                    ToHex(proofResult.ProofBytes)),
                new EthereumMainnetSccpSubmissionArgument(
                    "public_inputs",
                    "abi_bytes32x6",
                    ToHex(publicInputWordBytes)),
                new EthereumMainnetSccpSubmissionArgument(
                    "statement_hash",
                    "abi_bytes32",
                    proofResult.StatementHash),
            ],
            CallData: callData,
            CallDataHex: ToHex(callData),
            EnvelopeBytes: callData.ToArray(),
            EnvelopeHex: ToHex(callData),
            ProofBytes: proofResult.ProofBytes.ToArray(),
            PublicInputWordsBytes: publicInputWordBytes);
    }

    public static async ValueTask<object?> SubmitOutboundToEthereumAsync(
        EthereumMainnetSccpSubmissionInput input,
        IEthereumMainnetOutboundSubmitter outboundSubmitter,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(outboundSubmitter);

        var submission = BuildEthereumCalldata(input);
        return await outboundSubmitter.SubmitAsync(submission, cancellationToken).ConfigureAwait(false);
    }

    public static void RequireInboundRoute(int sourceDomain, int targetDomain)
    {
        if (sourceDomain != DomainEthereum || targetDomain != DomainSora)
        {
            throw new ArgumentException(
                "Ethereum mainnet inbound SCCP proofs must route ETH -> SORA.");
        }
    }

    public static void RequireOutboundRoute(int sourceDomain, int targetDomain)
    {
        if (sourceDomain != DomainSora || targetDomain != DomainEthereum)
        {
            throw new ArgumentException(
                "Ethereum mainnet outbound SCCP proofs must route SORA -> ETH.");
        }
    }

    public static void RequireMainnetNetworkId(string networkId)
    {
        if (!string.Equals(networkId, MainnetNetworkId, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "Ethereum mainnet SCCP destination bindings must use the canonical chain id 1 "
                    + "bytes32 network id.",
                nameof(networkId));
        }
    }

    public static EthereumMainnetSccpDestinationBinding DestinationBinding(
        string verifierAddress,
        string bridgeAddress,
        string verifierCodeHash,
        string verifierKeyHash,
        string? networkId = null,
        int sourceDomain = DomainSora,
        int targetDomain = DomainEthereum,
        string? expectedBindingHash = null,
        string? expectedKey = null)
    {
        RequireOutboundRoute(sourceDomain, targetDomain);

        var canonicalNetworkId = NormalizeNonZeroHex(
            networkId ?? MainnetNetworkId,
            nameof(networkId),
            32);
        if (!string.Equals(canonicalNetworkId, MainnetNetworkId, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "Ethereum mainnet destination bindings must use chain id 1.",
                nameof(networkId));
        }

        var canonicalVerifierAddress = NormalizeNonZeroHex(
            verifierAddress,
            nameof(verifierAddress),
            20);
        var canonicalBridgeAddress = NormalizeNonZeroHex(
            bridgeAddress,
            nameof(bridgeAddress),
            20);
        if (string.Equals(canonicalVerifierAddress, canonicalBridgeAddress, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "Ethereum mainnet verifierAddress must differ from bridgeAddress.",
                nameof(verifierAddress));
        }

        var canonicalVerifierCodeHash = NormalizeNonZeroHex(
            verifierCodeHash,
            nameof(verifierCodeHash),
            32);
        var canonicalVerifierKeyHash = NormalizeNonZeroHex(
            verifierKeyHash,
            nameof(verifierKeyHash),
            32);
        var key =
            $"evm:{sourceDomain}:{targetDomain}:{canonicalNetworkId[2..]}:" +
            $"{canonicalVerifierAddress}:{canonicalBridgeAddress}:" +
            $"{canonicalVerifierCodeHash}:{canonicalVerifierKeyHash}";
        var bindingHash = ComputeDestinationBindingHash(
            canonicalNetworkId,
            sourceDomain,
            targetDomain,
            canonicalVerifierAddress,
            canonicalBridgeAddress,
            canonicalVerifierCodeHash,
            canonicalVerifierKeyHash);

        if (expectedBindingHash is not null)
        {
            var canonicalExpectedBindingHash = NormalizeNonZeroHex(
                expectedBindingHash,
                nameof(expectedBindingHash),
                32);
            if (!string.Equals(canonicalExpectedBindingHash, bindingHash, StringComparison.Ordinal))
            {
                throw new ArgumentException(
                    "expectedBindingHash must match the Ethereum mainnet destination binding.",
                    nameof(expectedBindingHash));
            }
        }

        if (expectedKey is not null && !string.Equals(expectedKey.Trim(), key, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "expectedKey must match the Ethereum mainnet destination binding.",
                nameof(expectedKey));
        }

        return new EthereumMainnetSccpDestinationBinding(
            Version: 1,
            SourceDomain: sourceDomain,
            TargetDomain: targetDomain,
            NetworkId: canonicalNetworkId,
            VerifierAddress: canonicalVerifierAddress,
            BridgeAddress: canonicalBridgeAddress,
            VerifierCodeHash: canonicalVerifierCodeHash,
            VerifierKeyHash: canonicalVerifierKeyHash,
            VerifierBackend: EvmGroth16Bn254ProofBackend,
            ProofFamily: StarkFriProofFamily,
            Key: key,
            BindingHash: bindingHash);
    }

    public static string DestinationBindingHash(
        string verifierAddress,
        string bridgeAddress,
        string verifierCodeHash,
        string verifierKeyHash,
        string? networkId = null)
    {
        return DestinationBinding(
            verifierAddress,
            bridgeAddress,
            verifierCodeHash,
            verifierKeyHash,
            networkId).BindingHash;
    }

    private static EthereumMainnetTransparentPublicInputs NormalizePublicInputs(
        EthereumMainnetTransparentPublicInputs input)
    {
        if (input.Version != 1)
        {
            throw new ArgumentOutOfRangeException(
                nameof(input),
                input.Version,
                "Ethereum mainnet SCCP public inputs must use version 1.");
        }

        if (input.TargetDomain != DomainEthereum)
        {
            throw new ArgumentException(
                "Ethereum mainnet SCCP public inputs must target ETH.",
                nameof(input));
        }

        if (input.FinalityHeight == 0)
        {
            throw new ArgumentException(
                "Ethereum mainnet SCCP publicInputs.finalityHeight must not be zero.",
                nameof(input));
        }

        return input with
        {
            MessageId = NormalizeNonZeroHex(input.MessageId, nameof(input.MessageId), 32),
            PayloadHash = NormalizeNonZeroHex(input.PayloadHash, nameof(input.PayloadHash), 32),
            CommitmentRoot = NormalizeNonZeroHex(input.CommitmentRoot, nameof(input.CommitmentRoot), 32),
            FinalityBlockHash = NormalizeNonZeroHex(input.FinalityBlockHash, nameof(input.FinalityBlockHash), 32),
        };
    }

    private static byte[] CanonicalPublicInputsBytes(EthereumMainnetTransparentPublicInputs input)
    {
        using var payload = new MemoryStream();
        payload.WriteByte((byte)input.Version);
        payload.Write(HexToBytes(input.MessageId, 32));
        payload.Write(HexToBytes(input.PayloadHash, 32));
        payload.Write(LeU32(input.TargetDomain));
        payload.Write(HexToBytes(input.CommitmentRoot, 32));
        payload.Write(LeU64(input.FinalityHeight));
        payload.Write(HexToBytes(input.FinalityBlockHash, 32));
        return payload.ToArray();
    }

    private static string[] PublicInputAbiWords(EthereumMainnetTransparentPublicInputs input)
    {
        return
        [
            input.MessageId,
            input.PayloadHash,
            ToHex(AbiWordU32(input.TargetDomain)),
            input.CommitmentRoot,
            ToHex(AbiWordU64(input.FinalityHeight)),
            input.FinalityBlockHash,
        ];
    }

    private static string[] PublicSignalWords(
        EthereumMainnetTransparentPublicInputs input,
        int sourceDomain,
        string statementHash,
        string destinationBindingHash)
    {
        var values = new[]
        {
            HexToBytes(input.MessageId, 32),
            HexToBytes(input.PayloadHash, 32),
            AbiWordU32(input.TargetDomain),
            HexToBytes(input.CommitmentRoot, 32),
            AbiWordU64(input.FinalityHeight),
            HexToBytes(input.FinalityBlockHash, 32),
            AbiWordU32(sourceDomain),
            HexToBytes(statementHash, 32),
            HexToBytes(destinationBindingHash, 32),
        };
        var words = new string[Groth16Bn254SignalLabels.Length];
        for (var index = 0; index < Groth16Bn254SignalLabels.Length; index++)
        {
            words[index] = Groth16Bn254SignalWord(Groth16Bn254SignalLabels[index], values[index]);
        }

        return words;
    }

    private static string Groth16Bn254SignalWord(string label, byte[] value)
    {
        var labelHash = Keccak256(Encoding.UTF8.GetBytes(label));
        var digest = Keccak256(Concat(labelHash, value));
        var reduced = new BigInteger(digest, isUnsigned: true, isBigEndian: true)
            % Bn254ScalarFieldModulus;
        var bytes = reduced.ToByteArray(isUnsigned: true, isBigEndian: true);
        if (bytes.Length > 32)
        {
            throw new InvalidOperationException("BN254 signal word does not fit bytes32.");
        }

        var word = new byte[32];
        bytes.CopyTo(word.AsSpan(32 - bytes.Length));
        return ToHex(word);
    }

    private static string ComputeProofRequestHash(EthereumMainnetOutboundProofRequest request)
    {
        RequireEthereumProofRequestShape(request);
        return ComputeProofRequestHash(
            request.PublicInputsBytes,
            request.BundleBytes,
            request.SourceProofBytes,
            request.StatementHash,
            request.DestinationBindingHash,
            request.PublicSignalWords);
    }

    private static string ComputeProofRequestHash(
        byte[] publicInputsBytes,
        byte[] bundleBytes,
        byte[] sourceProofBytes,
        string statementHash,
        string destinationBindingHash,
        IReadOnlyList<string> publicSignalWords)
    {
        if (publicSignalWords.Count != 9)
        {
            throw new ArgumentException(
                "Ethereum mainnet SCCP publicSignalWords must contain 9 words.",
                nameof(publicSignalWords));
        }

        using var payload = new MemoryStream();
        payload.Write(publicInputsBytes);
        payload.Write(WriteBytes(bundleBytes));
        payload.Write(WriteBytes(sourceProofBytes));
        payload.Write(HexToBytes(statementHash, 32));
        payload.Write(HexToBytes(destinationBindingHash, 32));
        foreach (var word in publicSignalWords)
        {
            payload.Write(FixedHexToBytes(word, "publicSignalWords", 32));
        }

        return PrefixedBlake2bHex(Encoding.UTF8.GetBytes(ProofRequestPrefix), payload.ToArray());
    }

    private static string PrefixedBlake2bHex(byte[] prefix, byte[] payload)
        => ToHex(Blake2b.Hash256(Concat(prefix, payload)));

    private static byte[] SccpSubmitMessageProofCallData(
        byte[] proofBytes,
        IReadOnlyList<string> publicInputWords,
        string statementHash,
        int sourceDomain)
    {
        if (sourceDomain != DomainSora)
        {
            throw new ArgumentException("Ethereum mainnet verifier calldata must prove SORA-origin messages.");
        }

        if (publicInputWords.Count != 6)
        {
            throw new ArgumentException("publicInputWords must contain 6 ABI words.", nameof(publicInputWords));
        }

        var proofCopy = RequireGroth16ProofBytes(proofBytes, nameof(proofBytes));
        using var payload = new MemoryStream();
        payload.Write(SubmitMessageProofSelectorBytes);
        payload.Write(AbiWordU256(32UL * 8UL));
        foreach (var word in publicInputWords)
        {
            payload.Write(FixedHexToBytes(word, "publicInputWords", 32));
        }

        payload.Write(HexToBytes(statementHash, 32));
        payload.Write(AbiWordU256((ulong)proofCopy.Length));
        payload.Write(proofCopy);
        var padding = (32 - proofCopy.Length % 32) % 32;
        if (padding > 0)
        {
            payload.Write(new byte[padding]);
        }

        return payload.ToArray();
    }

    private static void RequireEthereumProofResult(EthereumMainnetOutboundProofResult proofResult)
    {
        if (proofResult.Version != 1)
        {
            throw new ArgumentException("proofResult.version must be 1.");
        }

        if (!string.Equals(proofResult.Backend, EvmGroth16Bn254ProofBackend, StringComparison.Ordinal))
        {
            throw new ArgumentException("proofResult.backend must be evm-groth16-bn254-v1.");
        }

        RequireEthereumProofRequest(proofResult.Request);
        var publicInputs = NormalizePublicInputs(proofResult.PublicInputs);
        if (!publicInputs.Equals(NormalizePublicInputs(proofResult.Request.PublicInputs)))
        {
            throw new ArgumentException("proofResult.publicInputs must match request.publicInputs.");
        }

        RequireGroth16ProofBytesForContext(
            proofResult.ProofBytes,
            publicInputs,
            proofResult.Request.SourceDomain,
            nameof(proofResult.ProofBytes));
        var expectedRequestHash = ComputeProofRequestHash(proofResult.Request);
        if (!string.Equals(expectedRequestHash, proofResult.RequestHash, StringComparison.Ordinal))
        {
            throw new ArgumentException("proofResult.requestHash must match request fields.");
        }

        if (!string.Equals(
                Convert.ToBase64String(proofResult.ProofBytes),
                proofResult.ProofBase64,
                StringComparison.Ordinal))
        {
            throw new ArgumentException("proofResult.proofBase64 must match proof bytes.");
        }

        if (!string.Equals(proofResult.StatementHash, proofResult.Request.StatementHash, StringComparison.Ordinal)
            || !string.Equals(
                proofResult.ProofContext.StatementHash,
                proofResult.Request.StatementHash,
                StringComparison.Ordinal))
        {
            throw new ArgumentException("proofResult statement hash must match request.");
        }

        if (!string.Equals(
                proofResult.DestinationBindingHash,
                proofResult.Request.DestinationBindingHash,
                StringComparison.Ordinal)
            || !string.Equals(
                proofResult.ProofContext.DestinationBindingHash,
                proofResult.Request.DestinationBindingHash,
                StringComparison.Ordinal))
        {
            throw new ArgumentException("proofResult destination binding hash must match request.");
        }

        if (!proofResult.PublicSignalWords.SequenceEqual(proofResult.Request.PublicSignalWords))
        {
            throw new ArgumentException("proofResult publicSignalWords must match request.");
        }

        var expectedEnvelopeHash = PrefixedBlake2bHex(
            Encoding.UTF8.GetBytes(ProofEnvelopePrefix),
            Concat(HexToBytes(proofResult.RequestHash, 32), proofResult.ProofBytes));
        if (!string.Equals(expectedEnvelopeHash, proofResult.EnvelopeHash, StringComparison.Ordinal))
        {
            throw new ArgumentException("proofResult.envelopeHash must match wrapped proof bytes.");
        }

        var destinationBinding = RequireEthereumDestinationBinding(proofResult.DestinationBinding);
        if (!string.Equals(destinationBinding.BindingHash, proofResult.DestinationBindingHash, StringComparison.Ordinal))
        {
            throw new ArgumentException("proofResult destinationBindingHash must match destinationBinding.");
        }
    }

    private static void RequireEthereumProofRequest(EthereumMainnetOutboundProofRequest request)
    {
        RequireEthereumProofRequestShape(request);
        var expectedRequestHash = ComputeProofRequestHash(
            request.PublicInputsBytes,
            request.BundleBytes,
            request.SourceProofBytes,
            request.StatementHash,
            request.DestinationBindingHash,
            request.PublicSignalWords);
        if (!string.Equals(expectedRequestHash, request.RequestHash, StringComparison.Ordinal))
        {
            throw new ArgumentException("requestHash must match Ethereum mainnet proof request fields.");
        }
    }

    private static void RequireEthereumProofRequestShape(EthereumMainnetOutboundProofRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);
        if (request.Version != 1
            || !string.Equals(request.Backend, EvmGroth16Bn254ProofBackend, StringComparison.Ordinal))
        {
            throw new ArgumentException("Ethereum mainnet proof requests must use EVM Groth16 v1.");
        }

        RequireOutboundRoute(request.SourceDomain, request.TargetDomain);
        if (request.PublicInputs.TargetDomain != DomainEthereum)
        {
            throw new ArgumentException("Ethereum mainnet proof request public inputs must target ETH.");
        }

        var publicInputs = NormalizePublicInputs(request.PublicInputs);
        if (!publicInputs.Equals(request.PublicInputs))
        {
            throw new ArgumentException("publicInputs must be canonical.");
        }

        if (!CanonicalPublicInputsBytes(publicInputs).SequenceEqual(request.PublicInputsBytes))
        {
            throw new ArgumentException("publicInputsBytes must match publicInputs.");
        }

        RequireNonEmptyBytes(request.BundleBytes, nameof(request.BundleBytes));
        NormalizeOptionalNonZeroBytes(request.SourceProofBytes, nameof(request.SourceProofBytes));
        var statementHash = NormalizeNonZeroHex(request.StatementHash, nameof(request.StatementHash), 32);
        var destinationBinding = RequireEthereumDestinationBinding(request.DestinationBinding);
        if (!string.Equals(destinationBinding.BindingHash, request.DestinationBindingHash, StringComparison.Ordinal))
        {
            throw new ArgumentException("destinationBindingHash must match Ethereum mainnet destinationBinding.");
        }

        var publicSignalWords = PublicSignalWords(
            publicInputs,
            request.SourceDomain,
            statementHash,
            request.DestinationBindingHash);
        if (!publicSignalWords.SequenceEqual(request.PublicSignalWords))
        {
            throw new ArgumentException("publicSignalWords must match public inputs and proof context.");
        }
    }

    private static EthereumMainnetSccpDestinationBinding RequireEthereumDestinationBinding(
        EthereumMainnetSccpDestinationBinding binding)
    {
        ArgumentNullException.ThrowIfNull(binding);
        var normalized = DestinationBinding(
            binding.VerifierAddress,
            binding.BridgeAddress,
            binding.VerifierCodeHash,
            binding.VerifierKeyHash,
            binding.NetworkId,
            binding.SourceDomain,
            binding.TargetDomain,
            binding.BindingHash,
            binding.Key);
        if (!string.Equals(binding.VerifierBackend, EvmGroth16Bn254ProofBackend, StringComparison.Ordinal)
            || !string.Equals(binding.ProofFamily, StarkFriProofFamily, StringComparison.Ordinal))
        {
            throw new ArgumentException("Ethereum mainnet destinationBinding verifier profile is invalid.");
        }

        return normalized;
    }

    private static string ComputeDestinationBindingHash(
        string networkId,
        int sourceDomain,
        int targetDomain,
        string verifierAddress,
        string bridgeAddress,
        string verifierCodeHash,
        string verifierKeyHash)
    {
        using var payload = new MemoryStream();
        payload.Write(Keccak256(Encoding.UTF8.GetBytes(EvmDestinationBindingLabel)));
        payload.Write(Keccak256(Encoding.UTF8.GetBytes(EvmGroth16Bn254ProofBackend)));
        payload.Write(Keccak256(Encoding.UTF8.GetBytes(StarkFriProofFamily)));
        payload.Write(HexToBytes(networkId, 32));
        payload.Write(AbiWordU32(sourceDomain));
        payload.Write(AbiWordU32(targetDomain));
        payload.Write(AbiWordAddress20(verifierAddress));
        payload.Write(AbiWordAddress20(bridgeAddress));
        payload.Write(HexToBytes(verifierCodeHash, 32));
        payload.Write(HexToBytes(verifierKeyHash, 32));
        return ToHex(Keccak256(payload.ToArray()));
    }

    private static ulong NormalizeMainnetChainId(object? value)
        => NormalizeUnsignedInteger(value, "eth_chainId");

    private static ulong NormalizeUnsignedInteger(object? value, string parameterName)
    {
        switch (value)
        {
            case byte byteValue:
                return byteValue;
            case ushort ushortValue:
                return ushortValue;
            case uint uintValue:
                return uintValue;
            case ulong ulongValue:
                return ulongValue;
            case sbyte sbyteValue when sbyteValue >= 0:
                return (ulong)sbyteValue;
            case short shortValue when shortValue >= 0:
                return (ulong)shortValue;
            case int intValue when intValue >= 0:
                return (ulong)intValue;
            case long longValue when longValue >= 0:
                return (ulong)longValue;
            case string text:
                return NormalizeUnsignedIntegerString(text, parameterName);
            default:
                throw new ArgumentException(
                    $"{parameterName} must be an integral JSON-RPC quantity.",
                    parameterName);
        }
    }

    private static ulong NormalizeUnsignedIntegerString(string value, string parameterName)
    {
        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException($"{parameterName} must be canonical.", parameterName);
        }

        if (value.StartsWith("0x", StringComparison.Ordinal))
        {
            var hex = value[2..];
            if (!IsCanonicalRpcQuantityHex(hex))
            {
                throw new ArgumentException(
                    $"{parameterName} must be a canonical JSON-RPC quantity.",
                    parameterName);
            }

            return Convert.ToUInt64(hex, 16);
        }

        if (value.Length == 0
            || (value != "0" && (value[0] == '0' || !value.All(IsDecimalDigit))))
        {
            throw new ArgumentException(
                $"{parameterName} must be a canonical decimal integer.",
                parameterName);
        }

        return ulong.Parse(value, System.Globalization.CultureInfo.InvariantCulture);
    }

    private static IReadOnlyDictionary<string, object?> RequireDictionary(object? value, string label)
    {
        if (value is IReadOnlyDictionary<string, object?> dictionary)
        {
            return dictionary;
        }

        throw new ArgumentException($"{label} must return an object.", nameof(value));
    }

    private static object? FirstPresent(IReadOnlyDictionary<string, object?> input, params string[] keys)
    {
        foreach (var key in keys)
        {
            if (input.TryGetValue(key, out var value))
            {
                return value;
            }
        }

        return null;
    }

    private static string NormalizeRpcHex(
        object? value,
        string parameterName,
        int byteLength,
        bool allowZero = false)
    {
        if (value is not string text
            || !string.Equals(text.Trim(), text, StringComparison.Ordinal)
            || !text.StartsWith("0x", StringComparison.Ordinal))
        {
            throw new ArgumentException(
                $"{parameterName} must be canonical lowercase 0x hex.",
                parameterName);
        }

        var hex = text[2..];
        if (hex.Length != byteLength * 2 || !hex.All(IsLowerHex))
        {
            throw new ArgumentException(
                $"{parameterName} must be {byteLength} bytes canonical lowercase 0x hex.",
                parameterName);
        }

        if (!allowZero && !hex.Any(static character => character != '0'))
        {
            throw new ArgumentException($"{parameterName} must not be zero.", parameterName);
        }

        return text;
    }

    private static SourceEvent NormalizeEthereumReceiptSourceEvent(
        IReadOnlyDictionary<string, object?>? receipt,
        string? sourceEventDigestInput,
        string? sourceBridgeEmitterAddressInput)
    {
        var sourceEventDigest = sourceEventDigestInput is null
            ? null
            : NormalizeRpcHex(sourceEventDigestInput, nameof(EthereumMainnetInboundEvidence.SourceEventDigest), 32);
        var sourceBridgeEmitterAddress = sourceBridgeEmitterAddressInput is null
            ? null
            : NormalizeRpcHex(
                sourceBridgeEmitterAddressInput,
                nameof(EthereumMainnetInboundEvidence.SourceBridgeEmitterAddress),
                20);
        if (sourceEventDigest is null && sourceBridgeEmitterAddress is null)
        {
            return new SourceEvent(null, null);
        }

        if (sourceBridgeEmitterAddress is null)
        {
            throw new ArgumentException(
                "sourceBridgeEmitterAddress is required when validating sourceEventDigest.",
                nameof(sourceBridgeEmitterAddressInput));
        }

        if (receipt is null)
        {
            throw new ArgumentException(
                "receipt.logs is required for SCCP source event validation.",
                nameof(receipt));
        }

        var logs = RequireList(FirstPresent(receipt, "logs"), "receipt.logs");
        string? matchedDigest = null;
        for (var index = 0; index < logs.Count; index++)
        {
            if (logs[index] is not IReadOnlyDictionary<string, object?> log)
            {
                throw new ArgumentException($"receipt.logs[{index}] must be an object.", nameof(receipt));
            }

            if (FirstPresent(log, "removed") is true)
            {
                throw new ArgumentException("receipt.logs must not contain removed logs.", nameof(receipt));
            }

            var logAddress = NormalizeRpcHex(
                FirstPresent(log, "address"),
                $"receipt.logs[{index}].address",
                20,
                allowZero: true);
            var topics = RequireList(FirstPresent(log, "topics"), $"receipt.logs[{index}].topics");
            if (topics.Count > 4)
            {
                throw new ArgumentException(
                    $"receipt.logs[{index}].topics must contain at most 4 entries.",
                    nameof(receipt));
            }

            var normalizedTopics = topics
                .Select((topic, topicIndex) => NormalizeRpcHex(
                    topic,
                    $"receipt.logs[{index}].topics[{topicIndex}]",
                    32,
                    allowZero: true))
                .ToArray();
            var data = FirstPresent(log, "data") ?? "0x";
            if (string.Equals(logAddress, sourceBridgeEmitterAddress, StringComparison.Ordinal)
                && normalizedTopics.Length == 2
                && string.Equals(normalizedTopics[0], SourceEventTopic, StringComparison.Ordinal))
            {
                var candidateDigest = normalizedTopics[1];
                if (IsZeroRpcHex(candidateDigest)
                    || (sourceEventDigest is not null
                        && !string.Equals(sourceEventDigest, candidateDigest, StringComparison.Ordinal))
                    || !string.Equals(data as string, "0x", StringComparison.Ordinal))
                {
                    continue;
                }

                if (matchedDigest is not null)
                {
                    throw new ArgumentException(
                        "receipt.logs must contain exactly one matching SCCP source event.",
                        nameof(receipt));
                }

                matchedDigest = candidateDigest;
            }
        }

        if (matchedDigest is null)
        {
            throw new ArgumentException(
                "receipt.logs must contain the expected SCCP source event.",
                nameof(receipt));
        }

        return new SourceEvent(matchedDigest, sourceBridgeEmitterAddress);
    }

    private static string? NormalizeReceiptProofHash(
        EthereumMainnetReceiptProof? receiptProof,
        string? suppliedHash)
    {
        var normalizedHash = suppliedHash is null
            ? null
            : NormalizeRpcHex(suppliedHash, nameof(EthereumMainnetInboundEvidence.ReceiptProofHash), 32);
        if (receiptProof is null)
        {
            return normalizedHash;
        }

        if (receiptProof.SourceDomain != DomainEthereum)
        {
            throw new ArgumentException(
                "receiptProof.sourceDomain must be ETH.",
                nameof(receiptProof));
        }

        var computedHash = EvmSccpReceiptProofHash(
            receiptProof.SourceEventDigest,
            receiptProof.BeaconSlot,
            receiptProof.ExecutionBlockNumber,
            receiptProof.ExecutionBlockHash,
            receiptProof.ExecutionReceiptsRoot,
            receiptProof.BeaconFinalizedRoot,
            receiptProof.SyncCommitteeRoot,
            receiptProof.ReceiptRootIndex,
            receiptProof.ReceiptTrieProofNodes,
            receiptProof.InclusionBranch,
            receiptProof.SourceDomain);
        if (normalizedHash is not null
            && !string.Equals(normalizedHash, computedHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "receiptProofHash must match receiptProof.",
                nameof(suppliedHash));
        }

        return computedHash;
    }

    private static void RequireReceiptProofMatchesEvidence(
        EthereumMainnetReceiptProof? receiptProof,
        string? blockHash,
        string? receiptBlockNumber,
        string? blockReceiptsRoot,
        IReadOnlyDictionary<string, object?>? beaconFinality,
        string? sourceEventDigest)
    {
        if (receiptProof is null)
        {
            return;
        }

        var proofBlockNumber = receiptProof.ExecutionBlockNumber;
        if (receiptBlockNumber is not null
            && proofBlockNumber != NormalizeUnsignedInteger(receiptBlockNumber, "block.number"))
        {
            throw new ArgumentException(
                "receiptProof.executionBlockNumber must match block.number.",
                nameof(receiptProof));
        }

        if (beaconFinality is not null
            && proofBlockNumber != NormalizeUnsignedInteger(
                beaconFinality["executionBlockNumber"],
                "beaconFinality.executionBlockNumber"))
        {
            throw new ArgumentException(
                "receiptProof.executionBlockNumber must match beaconFinality.executionBlockNumber.",
                nameof(receiptProof));
        }

        var proofBlockHash = NormalizeRpcHex(
            receiptProof.ExecutionBlockHash,
            "receiptProof.executionBlockHash",
            32);
        if (blockHash is not null
            && !string.Equals(proofBlockHash, blockHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "receiptProof.executionBlockHash must match block.hash.",
                nameof(receiptProof));
        }

        if (beaconFinality is not null
            && !string.Equals(
                proofBlockHash,
                beaconFinality["executionBlockHash"] as string,
                StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "receiptProof.executionBlockHash must match beaconFinality.executionBlockHash.",
                nameof(receiptProof));
        }

        var proofReceiptsRoot = NormalizeRpcHex(
            receiptProof.ExecutionReceiptsRoot,
            "receiptProof.executionReceiptsRoot",
            32);
        if (blockReceiptsRoot is not null
            && !string.Equals(proofReceiptsRoot, blockReceiptsRoot, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "receiptProof.executionReceiptsRoot must match block.receiptsRoot.",
                nameof(receiptProof));
        }

        if (beaconFinality is not null
            && !string.Equals(
                proofReceiptsRoot,
                beaconFinality["executionReceiptsRoot"] as string,
                StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "receiptProof.executionReceiptsRoot must match beaconFinality.executionReceiptsRoot.",
                nameof(receiptProof));
        }

        if (sourceEventDigest is not null)
        {
            var proofSourceEventDigest = NormalizeRpcHex(
                receiptProof.SourceEventDigest,
                "receiptProof.sourceEventDigest",
                32);
            if (!string.Equals(proofSourceEventDigest, sourceEventDigest, StringComparison.Ordinal))
            {
                throw new ArgumentException(
                    "receiptProof.sourceEventDigest must match receipt source event.",
                    nameof(receiptProof));
            }
        }
    }

    private static EthereumMainnetReceiptProof? SnapshotReceiptProof(EthereumMainnetReceiptProof? receiptProof)
    {
        if (receiptProof is null)
        {
            return null;
        }

        return receiptProof with
        {
            ReceiptTrieProofNodes = CopyByteArrays(receiptProof.ReceiptTrieProofNodes),
            InclusionBranch = CopyByteArrays(receiptProof.InclusionBranch),
        };
    }

    private static byte[][] CopyByteArrays(IReadOnlyList<byte[]> values)
    {
        var copy = new byte[values.Count][];
        for (var index = 0; index < values.Count; index++)
        {
            copy[index] = values[index].ToArray();
        }

        return copy;
    }

    private static IReadOnlyList<object?> RequireList(object? value, string parameterName)
    {
        if (value is IReadOnlyList<object?> list)
        {
            return list;
        }

        if (value is System.Collections.IEnumerable enumerable && value is not string)
        {
            return enumerable.Cast<object?>().ToArray();
        }

        throw new ArgumentException($"{parameterName} must be an array.", parameterName);
    }

    private static bool IsZeroRpcHex(string text)
    {
        for (var index = 2; index < text.Length; index++)
        {
            if (text[index] != '0')
            {
                return false;
            }
        }

        return true;
    }

    private static string NormalizeRpcQuantity(object? value, string parameterName)
    {
        if (value is not string text
            || !string.Equals(text.Trim(), text, StringComparison.Ordinal)
            || !text.StartsWith("0x", StringComparison.Ordinal))
        {
            throw new ArgumentException(
                $"{parameterName} must be a canonical JSON-RPC quantity.",
                parameterName);
        }

        var hex = text[2..];
        if (!IsCanonicalRpcQuantityHex(hex))
        {
            throw new ArgumentException(
                $"{parameterName} must be a canonical JSON-RPC quantity.",
                parameterName);
        }

        return "0x" + Convert.ToUInt64(hex, 16).ToString("x", System.Globalization.CultureInfo.InvariantCulture);
    }

    private static string NormalizePositiveRpcQuantity(object? value, string parameterName)
    {
        if (value is null)
        {
            throw new ArgumentException($"{parameterName} is required.", parameterName);
        }

        var quantity = NormalizeRpcQuantity(value, parameterName);
        if (string.Equals(quantity, "0x0", StringComparison.Ordinal))
        {
            throw new ArgumentException($"{parameterName} must be positive.", parameterName);
        }

        return quantity;
    }

    private static IReadOnlyDictionary<string, object?> NormalizeBeaconFinality(
        IReadOnlyDictionary<string, object?> finality,
        string? expectedBlockHash,
        string? expectedBlockNumber,
        string? expectedReceiptsRoot)
    {
        var executionBlockNumber = NormalizeUnsignedInteger(
            FirstPresent(
                finality,
                "executionBlockNumber",
                "execution_block_number",
                "finalityHeight",
                "finality_height"),
            "beaconFinality.executionBlockNumber");
        if (executionBlockNumber == 0)
        {
            throw new ArgumentException(
                "beaconFinality.executionBlockNumber must be positive.",
                nameof(finality));
        }

        if (expectedBlockNumber is not null
            && executionBlockNumber != NormalizeUnsignedInteger(expectedBlockNumber, "block.number"))
        {
            throw new ArgumentException(
                "beaconFinality.executionBlockNumber must match block.number.",
                nameof(finality));
        }

        var executionBlockHash = NormalizeRpcHex(
            FirstPresent(
                finality,
                "executionBlockHash",
                "execution_block_hash",
                "finalityBlockHash",
                "finality_block_hash"),
            "beaconFinality.executionBlockHash",
            32);
        if (expectedBlockHash is not null
            && !string.Equals(executionBlockHash, expectedBlockHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "beaconFinality.executionBlockHash must match block.hash.",
                nameof(finality));
        }

        var executionReceiptsRoot = NormalizeRpcHex(
            FirstPresent(
                finality,
                "executionReceiptsRoot",
                "execution_receipts_root",
                "receiptsRoot",
                "receipts_root"),
            "beaconFinality.executionReceiptsRoot",
            32);
        if (expectedReceiptsRoot is not null
            && !string.Equals(executionReceiptsRoot, expectedReceiptsRoot, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "beaconFinality.executionReceiptsRoot must match block.receiptsRoot.",
                nameof(finality));
        }

        var normalized = new Dictionary<string, object?>(finality, StringComparer.Ordinal)
        {
            ["executionBlockNumber"] = executionBlockNumber.ToString(System.Globalization.CultureInfo.InvariantCulture),
            ["executionBlockHash"] = executionBlockHash,
            ["executionReceiptsRoot"] = executionReceiptsRoot,
        };
        return normalized;
    }

    private static byte[] RequireNonZeroProofBytes(byte[] proofBytes, string parameterName)
    {
        ArgumentNullException.ThrowIfNull(proofBytes);
        if (proofBytes.Length == 0)
        {
            throw new ArgumentException($"{parameterName} must not be empty.", parameterName);
        }

        if (!proofBytes.Any(static value => value != 0))
        {
            throw new ArgumentException($"{parameterName} must not be all zero.", parameterName);
        }

        return proofBytes.ToArray();
    }

    private static byte[] RequireNativeRecursiveBytes(byte[] bytes, string parameterName)
    {
        ArgumentNullException.ThrowIfNull(bytes);
        if (bytes.Length == 0)
        {
            throw new ArgumentException($"{parameterName} must not be empty.", parameterName);
        }

        if (!bytes.Any(static value => value != 0))
        {
            throw new ArgumentException($"{parameterName} must not be all zero.", parameterName);
        }

        if (bytes.Length > NativeRecursiveMaxProofBytes)
        {
            throw new ArgumentException(
                $"{parameterName} must be at most {NativeRecursiveMaxProofBytes} bytes.",
                parameterName);
        }

        return bytes.ToArray();
    }

    private static byte[] RequireGroth16ProofBytes(byte[] proofBytes, string parameterName)
    {
        var proofCopy = RequireNonZeroProofBytes(proofBytes, parameterName);
        if (proofCopy.Length != Groth16Bn254ProofAbiByteLength)
        {
            throw new ArgumentException(
                $"{parameterName} must be {Groth16Bn254ProofAbiByteLength} bytes.",
                parameterName);
        }

        RequireGroth16Bn254ProofTuple(proofCopy, parameterName);
        return proofCopy;
    }

    private static byte[] RequireGroth16ProofBytesForContext(
        byte[] proofBytes,
        EthereumMainnetTransparentPublicInputs publicInputs,
        int sourceDomain,
        string parameterName)
    {
        var proofCopy = RequireGroth16ProofBytes(proofBytes, parameterName);
        var normalizedInputs = NormalizePublicInputs(publicInputs);
        if (!ProofWord(proofCopy, 1).SequenceEqual(HexToBytes(normalizedInputs.MessageId, 32)))
        {
            throw new ArgumentException(
                $"{parameterName}.messageId must match publicInputs.messageId.",
                parameterName);
        }

        if (!ProofWord(proofCopy, 3).SequenceEqual(HexToBytes(normalizedInputs.CommitmentRoot, 32)))
        {
            throw new ArgumentException(
                $"{parameterName}.commitmentRoot must match publicInputs.commitmentRoot.",
                parameterName);
        }

        if (ProofWordValue(proofCopy, 2) != sourceDomain)
        {
            throw new ArgumentException(
                $"{parameterName}.sourceDomain must match sourceDomain.",
                parameterName);
        }

        return proofCopy;
    }

    private static void RequireGroth16Bn254ProofTuple(byte[] proofBytes, string parameterName)
    {
        if (ProofWordValue(proofBytes, 0) != BigInteger.One)
        {
            throw new ArgumentException($"{parameterName}.version must be 1.", parameterName);
        }

        if (ProofWordIsZero(proofBytes, 1))
        {
            throw new ArgumentException($"{parameterName}.messageId must not be zero.", parameterName);
        }

        if (ProofWordValue(proofBytes, 2) > uint.MaxValue)
        {
            throw new ArgumentException($"{parameterName}.sourceDomain must fit u32.", parameterName);
        }

        if (ProofWordIsZero(proofBytes, 3))
        {
            throw new ArgumentException($"{parameterName}.commitmentRoot must not be zero.", parameterName);
        }

        var fields = new[] { "a.x", "a.y", "b.x0", "b.x1", "b.y0", "b.y1", "c.x", "c.y" };
        for (var index = 0; index < fields.Length; index++)
        {
            RequireGroth16BaseFieldWord(proofBytes, 4 + index, $"{parameterName}.{fields[index]}");
        }

        RequireGroth16G1Point(proofBytes, [4, 5], $"{parameterName}.a");
        RequireGroth16G2Point(proofBytes, [6, 7, 8, 9], $"{parameterName}.b");
        RequireGroth16G1Point(proofBytes, [10, 11], $"{parameterName}.c");
    }

    private static byte[] ProofWord(byte[] proofBytes, int index)
    {
        var word = new byte[32];
        proofBytes.AsSpan(index * 32, 32).CopyTo(word);
        return word;
    }

    private static BigInteger ProofWordValue(byte[] proofBytes, int index)
        => new(proofBytes.AsSpan(index * 32, 32), isUnsigned: true, isBigEndian: true);

    private static bool ProofWordIsZero(byte[] proofBytes, int index)
        => proofBytes.AsSpan(index * 32, 32).IndexOfAnyExcept((byte)0) < 0;

    private static void RequireGroth16BaseFieldWord(byte[] proofBytes, int index, string label)
    {
        if (ProofWordValue(proofBytes, index) >= Bn254BaseFieldModulus)
        {
            throw new ArgumentException($"{label} must be a BN254 base-field element.");
        }
    }

    private static void RequireGroth16NonZeroPoint(byte[] proofBytes, IReadOnlyList<int> indexes, string label)
    {
        if (indexes.All(index => ProofWordIsZero(proofBytes, index)))
        {
            throw new ArgumentException($"{label} must not be zero.");
        }
    }

    private static void RequireGroth16G1Point(byte[] proofBytes, IReadOnlyList<int> indexes, string label)
    {
        RequireGroth16NonZeroPoint(proofBytes, indexes, label);
        var x = ProofWordValue(proofBytes, indexes[0]);
        var y = ProofWordValue(proofBytes, indexes[1]);
        if (Bn254Fq(y * y) != Bn254Fq(x * x * x + 3))
        {
            throw new ArgumentException($"{label} must be a BN254 G1 point.");
        }
    }

    private static void RequireGroth16G2Point(byte[] proofBytes, IReadOnlyList<int> indexes, string label)
    {
        RequireGroth16NonZeroPoint(proofBytes, indexes, label);
        var x = new Bn254Fq2(
            ProofWordValue(proofBytes, indexes[0]),
            ProofWordValue(proofBytes, indexes[1]));
        var y = new Bn254Fq2(
            ProofWordValue(proofBytes, indexes[2]),
            ProofWordValue(proofBytes, indexes[3]));
        var left = Bn254Fq2Mul(y, y);
        var x2 = Bn254Fq2Mul(x, x);
        var right = Bn254Fq2Add(Bn254Fq2Mul(x2, x), new Bn254Fq2(Bn254G2BC0, Bn254G2BC1));
        if (!left.Equals(right) || !Bn254G2PointIsInPrimeSubgroup(x, y))
        {
            throw new ArgumentException($"{label} must be a BN254 G2 point.");
        }
    }

    private static BigInteger Bn254Fq(BigInteger value)
    {
        var reduced = value % Bn254BaseFieldModulus;
        return reduced.Sign < 0 ? reduced + Bn254BaseFieldModulus : reduced;
    }

    private static Bn254Fq2 Bn254Fq2Add(Bn254Fq2 left, Bn254Fq2 right)
        => new(Bn254Fq(left.C0 + right.C0), Bn254Fq(left.C1 + right.C1));

    private static Bn254Fq2 Bn254Fq2Sub(Bn254Fq2 left, Bn254Fq2 right)
        => new(Bn254Fq(left.C0 - right.C0), Bn254Fq(left.C1 - right.C1));

    private static Bn254Fq2 Bn254Fq2Scale(Bn254Fq2 left, BigInteger scalar)
        => new(Bn254Fq(left.C0 * scalar), Bn254Fq(left.C1 * scalar));

    private static Bn254Fq2 Bn254Fq2Mul(Bn254Fq2 left, Bn254Fq2 right)
        => new(
            Bn254Fq(left.C0 * right.C0 - left.C1 * right.C1),
            Bn254Fq(left.C0 * right.C1 + left.C1 * right.C0));

    private static bool Bn254Fq2IsZero(Bn254Fq2 value)
        => value.C0.IsZero && value.C1.IsZero;

    private static Bn254G2Projective Bn254G2Infinity()
        => new(
            new Bn254Fq2(BigInteger.Zero, BigInteger.Zero),
            new Bn254Fq2(BigInteger.One, BigInteger.Zero),
            new Bn254Fq2(BigInteger.Zero, BigInteger.Zero),
            true);

    private static Bn254G2Projective Bn254G2AffineProjective(Bn254Fq2 x, Bn254Fq2 y)
        => new(x, y, new Bn254Fq2(BigInteger.One, BigInteger.Zero), false);

    private static bool Bn254G2ProjectiveIsInfinity(Bn254G2Projective point)
        => point.Infinity || Bn254Fq2IsZero(point.Z);

    private static Bn254G2Projective Bn254G2ProjectiveDouble(Bn254G2Projective point)
    {
        if (Bn254G2ProjectiveIsInfinity(point) || Bn254Fq2IsZero(point.Y))
        {
            return Bn254G2Infinity();
        }

        var xx = Bn254Fq2Mul(point.X, point.X);
        var yy = Bn254Fq2Mul(point.Y, point.Y);
        var yyyy = Bn254Fq2Mul(yy, yy);
        var s = Bn254Fq2Scale(
            Bn254Fq2Sub(
                Bn254Fq2Sub(
                    Bn254Fq2Mul(Bn254Fq2Add(point.X, yy), Bn254Fq2Add(point.X, yy)),
                    xx),
                yyyy),
            2);
        var m = Bn254Fq2Scale(xx, 3);
        var x3 = Bn254Fq2Sub(Bn254Fq2Mul(m, m), Bn254Fq2Scale(s, 2));
        var y3 = Bn254Fq2Sub(
            Bn254Fq2Mul(m, Bn254Fq2Sub(s, x3)),
            Bn254Fq2Scale(yyyy, 8));
        var z3 = Bn254Fq2Scale(Bn254Fq2Mul(point.Y, point.Z), 2);
        return new Bn254G2Projective(x3, y3, z3, false);
    }

    private static Bn254G2Projective Bn254G2ProjectiveAddAffine(
        Bn254G2Projective point,
        Bn254Fq2 affineX,
        Bn254Fq2 affineY)
    {
        if (Bn254G2ProjectiveIsInfinity(point))
        {
            return Bn254G2AffineProjective(affineX, affineY);
        }

        var z1z1 = Bn254Fq2Mul(point.Z, point.Z);
        var u2 = Bn254Fq2Mul(affineX, z1z1);
        var s2 = Bn254Fq2Mul(affineY, Bn254Fq2Mul(point.Z, z1z1));
        var h = Bn254Fq2Sub(u2, point.X);
        if (Bn254Fq2IsZero(h))
        {
            return s2.Equals(point.Y) ? Bn254G2ProjectiveDouble(point) : Bn254G2Infinity();
        }

        var hh = Bn254Fq2Mul(h, h);
        var i = Bn254Fq2Scale(hh, 4);
        var j = Bn254Fq2Mul(h, i);
        var r = Bn254Fq2Scale(Bn254Fq2Sub(s2, point.Y), 2);
        var v = Bn254Fq2Mul(point.X, i);
        var x3 = Bn254Fq2Sub(Bn254Fq2Sub(Bn254Fq2Mul(r, r), j), Bn254Fq2Scale(v, 2));
        var y3 = Bn254Fq2Sub(
            Bn254Fq2Mul(r, Bn254Fq2Sub(v, x3)),
            Bn254Fq2Scale(Bn254Fq2Mul(point.Y, j), 2));
        var z3 = Bn254Fq2Sub(
            Bn254Fq2Sub(
                Bn254Fq2Mul(Bn254Fq2Add(point.Z, h), Bn254Fq2Add(point.Z, h)),
                z1z1),
            hh);
        return new Bn254G2Projective(x3, y3, z3, false);
    }

    private static bool Bn254G2PointIsInPrimeSubgroup(Bn254Fq2 x, Bn254Fq2 y)
    {
        var acc = Bn254G2Infinity();
        foreach (var bit in Bn254ScalarFieldBits)
        {
            acc = Bn254G2ProjectiveDouble(acc);
            if (bit == 1)
            {
                acc = Bn254G2ProjectiveAddAffine(acc, x, y);
            }
        }

        return Bn254G2ProjectiveIsInfinity(acc);
    }

    private static int[] ScalarBits(BigInteger value)
    {
        var bytes = value.ToByteArray(isUnsigned: true, isBigEndian: true);
        var bits = new List<int>(bytes.Length * 8);
        var started = false;
        foreach (var item in bytes)
        {
            for (var bit = 7; bit >= 0; bit--)
            {
                var selected = (item >> bit) & 1;
                if (selected == 1)
                {
                    started = true;
                }

                if (started)
                {
                    bits.Add(selected);
                }
            }
        }

        return bits.ToArray();
    }

    private static byte[] RequireNonEmptyBytes(byte[] bytes, string parameterName)
    {
        ArgumentNullException.ThrowIfNull(bytes);
        if (bytes.Length == 0)
        {
            throw new ArgumentException($"{parameterName} must not be empty.", parameterName);
        }

        return bytes.ToArray();
    }

    private static byte[] NormalizeOptionalNonZeroBytes(byte[]? bytes, string parameterName)
    {
        if (bytes is null || bytes.Length == 0)
        {
            return [];
        }

        if (!bytes.Any(static value => value != 0))
        {
            throw new ArgumentException($"{parameterName} must not be all zero.", parameterName);
        }

        return bytes.ToArray();
    }

    private static byte[] RpcHexToBytes(
        string value,
        string parameterName,
        int byteLength,
        bool allowZero = false)
    {
        var normalized = NormalizeRpcHex(value, parameterName, byteLength, allowZero);
        return Convert.FromHexString(normalized[2..]);
    }

    private static IReadOnlyList<byte[]> NormalizeReceiptTrieProofNodes(IReadOnlyList<byte[]> nodes)
    {
        ArgumentNullException.ThrowIfNull(nodes);
        if (nodes.Count == 0 || nodes.Count > MaxMptProofNodes)
        {
            throw new ArgumentException(
                $"receiptTrieProofNodes must contain 1..{MaxMptProofNodes} entries.",
                nameof(nodes));
        }

        var normalized = new byte[nodes.Count][];
        for (var index = 0; index < nodes.Count; index++)
        {
            var node = nodes[index] ?? throw new ArgumentException(
                $"receiptTrieProofNodes[{index}] is required.",
                nameof(nodes));
            if (node.Length == 0 || node.Length > MaxMptNodeBytes)
            {
                throw new ArgumentException(
                    $"receiptTrieProofNodes[{index}] must contain 1..{MaxMptNodeBytes} bytes.",
                    nameof(nodes));
            }

            normalized[index] = node.ToArray();
        }

        return normalized;
    }

    private static IReadOnlyList<byte[]> NormalizeReceiptInclusionBranch(
        IReadOnlyList<byte[]> branch,
        bool requireNonEmpty)
    {
        ArgumentNullException.ThrowIfNull(branch);
        if (requireNonEmpty && branch.Count == 0)
        {
            throw new ArgumentException("inclusionBranch must not be empty.", nameof(branch));
        }

        if (branch.Count > MaxSourceMerkleBranchNodes)
        {
            throw new ArgumentException(
                $"inclusionBranch must contain at most {MaxSourceMerkleBranchNodes} entries.",
                nameof(branch));
        }

        var normalized = new byte[branch.Count][];
        for (var index = 0; index < branch.Count; index++)
        {
            var sibling = branch[index] ?? throw new ArgumentException(
                $"inclusionBranch[{index}] is required.",
                nameof(branch));
            if (sibling.Length != 32)
            {
                throw new ArgumentException($"inclusionBranch[{index}] must be 32 bytes.", nameof(branch));
            }

            normalized[index] = sibling.ToArray();
        }

        return normalized;
    }

    private static EthereumMainnetOutboundProofRequest Snapshot(
        EthereumMainnetOutboundProofRequest request)
    {
        return request with
        {
            PublicInputsBytes = request.PublicInputsBytes.ToArray(),
            PublicSignalWords = request.PublicSignalWords.ToArray(),
            BundleBytes = request.BundleBytes.ToArray(),
            SourceProofBytes = request.SourceProofBytes.ToArray(),
        };
    }

    private static bool IsCanonicalRpcQuantityHex(string text)
    {
        return text == "0"
            || (text.Length > 0 && text[0] != '0' && text.All(IsLowerHex));
    }

    private static bool IsLowerHex(char character)
    {
        return character is >= '0' and <= '9' or >= 'a' and <= 'f';
    }

    private static bool IsDecimalDigit(char character)
    {
        return character is >= '0' and <= '9';
    }

    private static byte[] AbiWordU32(int value)
    {
        if (value < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(value), value, "Domain id must be u32.");
        }

        var outBytes = new byte[32];
        BinaryPrimitives.WriteUInt32BigEndian(outBytes.AsSpan(28, 4), (uint)value);
        return outBytes;
    }

    private static byte[] AbiWordU64(ulong value)
    {
        var outBytes = new byte[32];
        BinaryPrimitives.WriteUInt64BigEndian(outBytes.AsSpan(24, 8), value);
        return outBytes;
    }

    private static byte[] AbiWordU256(ulong value)
    {
        var outBytes = new byte[32];
        BinaryPrimitives.WriteUInt64BigEndian(outBytes.AsSpan(24, 8), value);
        return outBytes;
    }

    private static byte[] LeU32(int value)
    {
        if (value < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(value), value, "Domain id must be u32.");
        }

        var outBytes = new byte[4];
        BinaryPrimitives.WriteUInt32LittleEndian(outBytes, (uint)value);
        return outBytes;
    }

    private static byte[] LeU64(ulong value)
    {
        var outBytes = new byte[8];
        BinaryPrimitives.WriteUInt64LittleEndian(outBytes, value);
        return outBytes;
    }

    private static byte[] WriteBytes(byte[] value)
    {
        var length = new byte[4];
        BinaryPrimitives.WriteUInt32LittleEndian(length, checked((uint)value.Length));
        return Concat(length, value);
    }

    private static byte[] AbiWordAddress20(string value)
    {
        var address = HexToBytes(value, 20);
        var outBytes = new byte[32];
        address.CopyTo(outBytes.AsSpan(12));
        return outBytes;
    }

    private static string NormalizeNonZeroHex(string value, string parameterName, int byteLength)
    {
        if (value is null)
        {
            throw new ArgumentNullException(parameterName);
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException($"{parameterName} must be canonical hex.", parameterName);
        }

        var text = value.StartsWith("0x", StringComparison.OrdinalIgnoreCase)
            ? value[2..]
            : value;
        if (text.Length != byteLength * 2 || text.Length == 0 || text.Length % 2 != 0)
        {
            throw new ArgumentException($"{parameterName} must be {byteLength} bytes.", parameterName);
        }

        byte[] bytes;
        try
        {
            bytes = Convert.FromHexString(text);
        }
        catch (FormatException ex)
        {
            throw new ArgumentException($"{parameterName} must be hex.", parameterName, ex);
        }

        if (!bytes.Any(static value => value != 0))
        {
            throw new ArgumentException($"{parameterName} must not be zero.", parameterName);
        }

        return ToHex(bytes);
    }

    private static byte[] HexToBytes(string value, int byteLength)
    {
        var normalized = NormalizeNonZeroHex(value, nameof(value), byteLength);
        return Convert.FromHexString(normalized[2..]);
    }

    private static byte[] HexToBytes32(string value)
        => FixedHexToBytes(value, nameof(value), 32);

    private static byte[] FixedHexToBytes(string value, string parameterName, int byteLength)
    {
        if (value is null)
        {
            throw new ArgumentNullException(parameterName);
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException($"{parameterName} must be canonical hex.", parameterName);
        }

        var text = value.StartsWith("0x", StringComparison.OrdinalIgnoreCase)
            ? value[2..]
            : value;
        if (text.Length != byteLength * 2 || text.Length == 0 || text.Length % 2 != 0)
        {
            throw new ArgumentException($"{parameterName} must be {byteLength} bytes.", parameterName);
        }

        try
        {
            return Convert.FromHexString(text);
        }
        catch (FormatException ex)
        {
            throw new ArgumentException($"{parameterName} must be hex.", parameterName, ex);
        }
    }

    private static byte[] Concat(params byte[][] chunks)
    {
        var total = checked(chunks.Sum(static chunk => chunk.Length));
        var output = new byte[total];
        var offset = 0;
        foreach (var chunk in chunks)
        {
            chunk.CopyTo(output.AsSpan(offset));
            offset += chunk.Length;
        }

        return output;
    }

    private static string ToHex(ReadOnlySpan<byte> value)
    {
        return "0x" + Convert.ToHexString(value).ToLowerInvariant();
    }

    private static byte[] Keccak256(ReadOnlySpan<byte> data)
    {
        var state = new ulong[25];
        var offset = 0;
        while (offset + Keccak256Rate <= data.Length)
        {
            AbsorbKeccakBlock(state, data.Slice(offset, Keccak256Rate));
            KeccakF1600(state);
            offset += Keccak256Rate;
        }

        Span<byte> block = stackalloc byte[Keccak256Rate];
        data[offset..].CopyTo(block);
        block[data.Length - offset] ^= 0x01;
        block[Keccak256Rate - 1] ^= 0x80;
        AbsorbKeccakBlock(state, block);
        KeccakF1600(state);

        var output = new byte[32];
        var written = 0;
        Span<byte> laneBytes = stackalloc byte[8];
        for (var lane = 0; written < output.Length; lane++)
        {
            BinaryPrimitives.WriteUInt64LittleEndian(laneBytes, state[lane]);
            var count = Math.Min(8, output.Length - written);
            laneBytes[..count].CopyTo(output.AsSpan(written));
            written += count;
        }

        return output;
    }

    private static void AbsorbKeccakBlock(ulong[] state, ReadOnlySpan<byte> block)
    {
        for (var lane = 0; lane < Keccak256Rate / 8; lane++)
        {
            state[lane] ^= BinaryPrimitives.ReadUInt64LittleEndian(block.Slice(lane * 8, 8));
        }
    }

    private static void KeccakF1600(ulong[] state)
    {
        Span<ulong> c = stackalloc ulong[5];
        Span<ulong> d = stackalloc ulong[5];
        Span<ulong> b = stackalloc ulong[25];

        foreach (var roundConstant in KeccakRoundConstants)
        {
            for (var x = 0; x < 5; x++)
            {
                c[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
            }

            for (var x = 0; x < 5; x++)
            {
                d[x] = c[(x + 4) % 5] ^ RotateLeft(c[(x + 1) % 5], 1);
            }

            for (var x = 0; x < 5; x++)
            {
                for (var y = 0; y < 5; y++)
                {
                    state[x + 5 * y] ^= d[x];
                }
            }

            for (var x = 0; x < 5; x++)
            {
                for (var y = 0; y < 5; y++)
                {
                    var sourceIndex = x + 5 * y;
                    var targetIndex = y + 5 * ((2 * x + 3 * y) % 5);
                    b[targetIndex] = RotateLeft(state[sourceIndex], KeccakRhoOffsets[sourceIndex]);
                }
            }

            for (var x = 0; x < 5; x++)
            {
                for (var y = 0; y < 5; y++)
                {
                    state[x + 5 * y] =
                        b[x + 5 * y] ^ ((~b[((x + 1) % 5) + 5 * y]) & b[((x + 2) % 5) + 5 * y]);
                }
            }

            state[0] ^= roundConstant;
        }
    }

    private static ulong RotateLeft(ulong value, int amount)
    {
        return amount == 0 ? value : (value << amount) | (value >> (64 - amount));
    }
}

public sealed record EthereumMainnetSccpDestinationBinding(
    int Version,
    int SourceDomain,
    int TargetDomain,
    string NetworkId,
    string VerifierAddress,
    string BridgeAddress,
    string VerifierCodeHash,
    string VerifierKeyHash,
    string VerifierBackend,
    string ProofFamily,
    string Key,
    string BindingHash);

public sealed record EthereumMainnetLocalAdmissionSubmissionInput(
    byte[] ProofBytes,
    byte[] PublicInputsBytes,
    byte[] BundleBytes,
    byte[] EnvelopeBytes,
    string StatementHash,
    string SourceVerifierMaterialHash,
    string SourceAdapterEngineDeploymentHash,
    int SourceDomain = EthereumMainnetSccp.DomainEthereum,
    int TargetDomain = EthereumMainnetSccp.DomainSora,
    string ProofFamily = EthereumMainnetSccp.StarkFriProofFamily,
    string VerifierBackend = EthereumMainnetSccp.EvmGroth16Bn254ProofBackend,
    string EnvelopeEncoding = EthereumMainnetSccp.LocalAdmissionEnvelopeEncoding,
    string SubmissionKind = EthereumMainnetSccp.LocalAdmissionSubmissionKind,
    string VerifierEntrypoint = EthereumMainnetSccp.LocalAdmissionEntrypoint);

public sealed record EthereumMainnetLocalAdmissionPayload(
    byte[] ProofBytes,
    byte[] PublicInputsBytes,
    byte[] BundleBytes,
    string StatementHash,
    string SourceVerifierMaterialHash,
    string SourceAdapterEngineDeploymentHash)
{
    public int Version { get; } = 1;
    public string ProofBytesHex { get; } = "0x" + Convert.ToHexString(ProofBytes).ToLowerInvariant();
    public string PublicInputsBytesHex { get; } = "0x" + Convert.ToHexString(PublicInputsBytes).ToLowerInvariant();
    public string BundleBytesHex { get; } = "0x" + Convert.ToHexString(BundleBytes).ToLowerInvariant();
}

public sealed record EthereumMainnetLocalAdmissionSubmission(
    string ProofFamily,
    string VerifierBackend,
    int SourceDomain,
    int TargetDomain,
    string StatementHash,
    string SourceVerifierMaterialHash,
    string SourceAdapterEngineDeploymentHash,
    EthereumMainnetLocalAdmissionPayload LocalAdmission,
    byte[] ProofBytes,
    byte[] PublicInputsBytes,
    byte[] BundleBytes,
    byte[] EnvelopeBytes)
{
    public int Version { get; } = 1;
    public string PlatformPayload { get; } = EthereumMainnetSccp.LocalAdmissionSubmissionKind;
    public string EnvelopeEncoding { get; } = EthereumMainnetSccp.LocalAdmissionEnvelopeEncoding;
    public string SubmissionKind { get; } = EthereumMainnetSccp.LocalAdmissionSubmissionKind;
    public string VerifierEntrypoint { get; } = EthereumMainnetSccp.LocalAdmissionEntrypoint;
    public IReadOnlyList<EthereumMainnetSccpSubmissionArgument> Arguments { get; } =
        Array.Empty<EthereumMainnetSccpSubmissionArgument>();
    public string ProofBytesHex { get; } = "0x" + Convert.ToHexString(ProofBytes).ToLowerInvariant();
    public string PublicInputsBytesHex { get; } = "0x" + Convert.ToHexString(PublicInputsBytes).ToLowerInvariant();
    public string BundleBytesHex { get; } = "0x" + Convert.ToHexString(BundleBytes).ToLowerInvariant();
    public string EnvelopeHex { get; } = "0x" + Convert.ToHexString(EnvelopeBytes).ToLowerInvariant();
}

public sealed record EthereumMainnetTransparentPublicInputs(
    int Version,
    string MessageId,
    string PayloadHash,
    int TargetDomain,
    string CommitmentRoot,
    ulong FinalityHeight,
    string FinalityBlockHash);

public sealed record EthereumMainnetSccpProofContext(
    string StatementHash,
    string DestinationBindingHash);

public sealed record EthereumMainnetOutboundProofRequestInput
{
    public EthereumMainnetTransparentPublicInputs? PublicInputs { get; init; }

    public byte[] BundleBytes { get; init; } = [];

    public byte[]? SourceProofBytes { get; init; }

    public string StatementHash { get; init; } = string.Empty;

    public string? DestinationBindingHash { get; init; }

    public int SourceDomain { get; init; } = EthereumMainnetSccp.DomainSora;

    public EthereumMainnetSccpDestinationBinding? DestinationBinding { get; init; }
}

public sealed record EthereumMainnetOutboundProofRequest(
    int Version,
    string Backend,
    int SourceDomain,
    int TargetDomain,
    EthereumMainnetTransparentPublicInputs PublicInputs,
    byte[] PublicInputsBytes,
    string[] PublicSignalWords,
    byte[] BundleBytes,
    byte[] SourceProofBytes,
    EthereumMainnetSccpProofContext ProofContext,
    string StatementHash,
    string DestinationBindingHash,
    string RequestHash,
    EthereumMainnetSccpDestinationBinding DestinationBinding);

public sealed record EthereumMainnetOutboundProofResult(
    int Version,
    string Backend,
    byte[] ProofBytes,
    string ProofBase64,
    EthereumMainnetOutboundProofRequest Request,
    EthereumMainnetTransparentPublicInputs PublicInputs,
    string[] PublicSignalWords,
    string StatementHash,
    string DestinationBindingHash,
    EthereumMainnetSccpProofContext ProofContext,
    string RequestHash,
    string EnvelopeHash,
    EthereumMainnetSccpDestinationBinding DestinationBinding);

public sealed record EthereumMainnetSccpSubmissionInput(
    EthereumMainnetOutboundProofResult ProofResult);

public sealed record EthereumMainnetSccpSubmissionArgument(
    string Key,
    string Encoding,
    string Bytes);

public sealed record EthereumMainnetSccpSubmission(
    int Version,
    string ProofFamily,
    string VerifierBackend,
    string PlatformPayload,
    string EnvelopeEncoding,
    string SubmissionKind,
    string VerifierEntrypoint,
    string ContractMethod,
    string FunctionSelector,
    int SourceDomain,
    int TargetDomain,
    EthereumMainnetTransparentPublicInputs PublicInputs,
    string[] PublicInputWords,
    string[] PublicSignalWords,
    string StatementHash,
    string DestinationBindingHash,
    EthereumMainnetSccpSubmissionArgument[] Arguments,
    byte[] CallData,
    string CallDataHex,
    byte[] EnvelopeBytes,
    string EnvelopeHex,
    byte[] ProofBytes,
    byte[] PublicInputWordsBytes);

public interface IEthereumMainnetExecutionProvider
{
    ValueTask<object?> RequestAsync(
        string method,
        IReadOnlyList<object?> parameters,
        CancellationToken cancellationToken = default);
}

public interface IEthereumMainnetConsensusProvider
{
    ValueTask<IReadOnlyDictionary<string, object?>?> CollectFinalityEvidenceAsync(
        IReadOnlyDictionary<string, object?>? receipt,
        IReadOnlyDictionary<string, object?>? block,
        string? transactionHash,
        CancellationToken cancellationToken = default);
}

public interface IEthereumMainnetInboundProver
{
    ValueTask<byte[]> ProveAsync(
        EthereumMainnetInboundEvidence evidence,
        CancellationToken cancellationToken = default);
}

public interface IEthereumMainnetInboundSubmitter
{
    ValueTask<object?> SubmitAsync(
        byte[] proofBytes,
        CancellationToken cancellationToken = default);
}

public interface IEthereumMainnetOutboundProver
{
    ValueTask<byte[]> ProveAsync(
        EthereumMainnetOutboundProofRequest request,
        CancellationToken cancellationToken = default);
}

public interface IEthereumMainnetOutboundSubmitter
{
    ValueTask<object?> SubmitAsync(
        EthereumMainnetSccpSubmission submission,
        CancellationToken cancellationToken = default);
}

public sealed record EthereumMainnetBeaconFinalityEvidence(
    string ExecutionBlockNumber,
    string ExecutionBlockHash,
    string ExecutionReceiptsRoot)
{
    public IReadOnlyDictionary<string, object?> ToDictionary(
        IEnumerable<KeyValuePair<string, object?>>? additionalFields = null)
    {
        var value = additionalFields is null
            ? new Dictionary<string, object?>(StringComparer.Ordinal)
            : new Dictionary<string, object?>(additionalFields, StringComparer.Ordinal);
        value["executionBlockNumber"] = ExecutionBlockNumber;
        value["executionBlockHash"] = ExecutionBlockHash;
        value["executionReceiptsRoot"] = ExecutionReceiptsRoot;
        return value;
    }
}

public sealed record EthereumMainnetReceiptProof
{
    public int SourceDomain { get; init; } = EthereumMainnetSccp.DomainEthereum;

    public string SourceEventDigest { get; init; } = string.Empty;

    public ulong BeaconSlot { get; init; }

    public ulong ExecutionBlockNumber { get; init; }

    public string ExecutionBlockHash { get; init; } = string.Empty;

    public string ExecutionReceiptsRoot { get; init; } = string.Empty;

    public string BeaconFinalizedRoot { get; init; } = string.Empty;

    public string SyncCommitteeRoot { get; init; } = string.Empty;

    public ulong ReceiptRootIndex { get; init; }

    public IReadOnlyList<byte[]> ReceiptTrieProofNodes { get; init; } = Array.Empty<byte[]>();

    public IReadOnlyList<byte[]> InclusionBranch { get; init; } = Array.Empty<byte[]>();
}

public sealed record EthereumMainnetInboundEvidence
{
    public int SourceDomain { get; init; } = EthereumMainnetSccp.DomainEthereum;

    public int TargetDomain { get; init; } = EthereumMainnetSccp.DomainSora;

    public string? TransactionHash { get; init; }

    public IReadOnlyDictionary<string, object?>? Receipt { get; init; }

    public IReadOnlyDictionary<string, object?>? Block { get; init; }

    public IReadOnlyDictionary<string, object?>? BeaconFinality { get; init; }

    public EthereumMainnetReceiptProof? ReceiptProof { get; init; }

    public string? ReceiptProofHash { get; init; }

    public string? SourceEventDigest { get; init; }

    public string? SourceBridgeEmitterAddress { get; init; }

    public EthereumMainnetInboundEvidence WithBeaconFinalityEvidence(
        EthereumMainnetBeaconFinalityEvidence? evidence,
        IEnumerable<KeyValuePair<string, object?>>? additionalFields = null)
    {
        return this with
        {
            BeaconFinality = evidence?.ToDictionary(additionalFields),
        };
    }
}
