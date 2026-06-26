using System.Buffers.Binary;
using System.Security.Cryptography;
using System.Text;

namespace Hyperledger.Iroha.Sccp;

/// <summary>
/// BSC mainnet SCCP constants, route validators, receipt collection, and destination
/// bindings for native .NET callers.
/// </summary>
public static partial class BscMainnetSccp
{
    public const int DomainSora = 0;
    public const int DomainBsc = 2;
    public const ulong MainnetChainId = 56;
    public const string EvmGroth16Bn254ProofBackend = "evm-groth16-bn254-v1";
    public const string StarkFriProofFamily = "stark-fri-v1";
    public const string LocalAdmissionEnvelopeEncoding = "norito:sccp-local-admission:v1";
    public const string LocalAdmissionSubmissionKind = "local_admission";
    public const string LocalAdmissionEntrypoint = "SubmitBridgeProof";
    public const int NativeRecursiveMaxProofBytes = 2 * 1024 * 1024;
    public const string SourceEventTopic = EthereumMainnetSccp.SourceEventTopic;
    public const string MainnetNetworkId =
        "0x0000000000000000000000000000000000000000000000000000000000000038";
    public const string SourceAdapterOpenVerifyCircuitId = "sccp-source-adapter-v1";
    public const string SourceAdapterFastPqParameterSet = "fastpq-lane-balanced";

    private const string EvmDestinationBindingLabel = "iroha:sccp:evm-destination-binding:v1";
    private const string SourceVerifierMaterialRecordPrefix =
        "sccp:source-verifier-material-record:v1";
    private const string SourceAdapterEngineDeploymentRecordPrefix =
        "sccp:source-adapter-engine-deployment:v1";
    private const string BscReceiptProofPrefix = "sccp:bsc:receipt-proof:v1";
    private const string SourceChain = "bsc";
    private const byte SourceProofPlan = 2;
    private const byte SourceFinalityModel = 2;
    private const string SourceTrustAnchorId =
        "sccp:bsc:source-trust-anchor:bsc-mainnet-validator-set:v1";
    private const string ConsensusVerifierId =
        "sccp:bsc:consensus-verifier:validator-set-seal-mainnet:v1";
    private const string MessageInclusionVerifierId =
        "sccp:bsc:message-inclusion-verifier:receipt-trie-branch-mainnet:v1";
    private const string FinalityPolicyId =
        "sccp:bsc:finality-policy:validator-set-finality-mainnet:v1";
    private const string SourceBridgeEmitterId =
        "sccp:bsc:source-bridge-emitter:bsc-mainnet:v1";
    private const int Keccak256Rate = 136;
    private const int MaxSourceMerkleBranchNodes = 64;
    private const int MaxMptProofNodes = 64;
    private const int MaxMptNodeBytes = 16 * 1024;
    private const ulong SourceAdapterFastPqTraceRoot = 0x002A_247F_81C6_F850UL;
    private const ulong SourceAdapterFastPqLdeRoot = 0x6026_3388_DBBF_9B2AUL;
    private const ulong SourceAdapterFastPqOmegaCoset = 0x6AF3_25E8_25AD_5C18UL;

    private sealed record SourceEvent(string? SourceEventDigest, string? SourceBridgeEmitterAddress);

    private sealed record NormalizedBscSourceMaterial(
        int SourceDomain,
        int TargetDomain,
        string SourceTrustAnchorHash,
        string ConsensusVerifierHash,
        string MessageInclusionVerifierHash,
        string FinalityPolicyHash,
        string BridgeAddress,
        string SourceBridgeEmitterCodeHash);

    private sealed record NormalizedBscSourceAdapterDeployment(
        int SourceDomain,
        int TargetDomain,
        string SourceTrustAnchorHash,
        string ConsensusVerifierHash,
        string MessageInclusionVerifierHash,
        string FinalityPolicyHash,
        string BridgeAddress,
        string SourceBridgeEmitterCodeHash,
        string AdapterVerifierVkHash,
        string DeploymentReceiptHash);

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

    public static void RequireMainnetChainId(ulong chainId)
    {
        if (chainId != MainnetChainId)
        {
            throw new ArgumentOutOfRangeException(
                nameof(chainId),
                chainId,
                "BSC mainnet SCCP requires eth_chainId == 56.");
        }
    }

    public static async ValueTask<object?> ValidateExecutionProviderMainnetAsync(
        IBscMainnetExecutionProvider executionProvider,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(executionProvider);

        var chainId = await executionProvider.RequestAsync(
            "eth_chainId",
            Array.Empty<object?>(),
            cancellationToken).ConfigureAwait(false);
        RequireMainnetChainId(NormalizeRpcChainId(chainId));
        return chainId;
    }

    public static async ValueTask<BscMainnetInboundEvidence> CollectInboundEvidenceFromReceiptAsync(
        BscMainnetInboundEvidence input,
        IBscMainnetExecutionProvider? executionProvider = null,
        IBscMainnetConsensusProvider? consensusProvider = null,
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
        if (receipt is null && transactionHash is not null && executionProvider is not null)
        {
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
                "BSC mainnet inbound evidence requires receipt, receiptProof, receiptProofHash, or transactionHash.",
                nameof(input));
        }

        string? blockHash = null;
        string? receiptBlockNumber = null;
        string? executionReceiptsRoot = null;
        string? sourceEventDigest = null;
        string? normalizedSourceBridgeEmitterAddress = null;
        if (receipt is not null)
        {
            if (!string.Equals(FirstPresent(receipt, "status") as string, "0x1", StringComparison.Ordinal))
            {
                throw new ArgumentException(
                    "BSC mainnet inbound receipt status must be 0x1.",
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
            var sourceEvent = NormalizeBscReceiptSourceEvent(
                receipt,
                input.SourceEventDigest,
                input.SourceBridgeEmitterAddress,
                transactionHash,
                blockHash,
                receiptBlockNumber);
            sourceEventDigest = sourceEvent.SourceEventDigest;
            normalizedSourceBridgeEmitterAddress = sourceEvent.SourceBridgeEmitterAddress;
        }
        else if (input.SourceEventDigest is not null || input.SourceBridgeEmitterAddress is not null)
        {
            throw new ArgumentException(
                "receipt.logs is required for SCCP source event validation.",
                nameof(input));
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

            blockHash = normalizedBlockHash;
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
            executionReceiptsRoot = NormalizeRpcHex(
                FirstPresent(block, "receiptsRoot", "receipts_root"),
                "block.receiptsRoot",
                32);
        }

        receipt = SnapshotDictionaryOrNull(receipt);
        block = SnapshotDictionaryOrNull(block);

        var parliaFinality = input.ParliaFinality;
        if (parliaFinality is null && consensusProvider is not null)
        {
            parliaFinality = await consensusProvider.CollectFinalityEvidenceAsync(
                SnapshotDictionaryOrNull(receipt),
                SnapshotDictionaryOrNull(block),
                transactionHash,
                cancellationToken).ConfigureAwait(false);
        }
        var normalizedParliaFinality = parliaFinality is null
            ? null
            : NormalizeParliaFinality(
                parliaFinality,
                blockHash,
                receiptBlockNumber,
                executionReceiptsRoot);
        RequireReceiptProofMatchesEvidence(
            receiptProof,
            blockHash,
            receiptBlockNumber,
            executionReceiptsRoot,
            normalizedParliaFinality,
            sourceEventDigest);

        return SnapshotInboundEvidence(input with
        {
            SourceDomain = DomainBsc,
            TargetDomain = DomainSora,
            TransactionHash = transactionHash,
            Receipt = SnapshotDictionaryOrNull(receipt),
            Block = SnapshotDictionaryOrNull(block),
            ParliaFinality = SnapshotDictionaryOrNull(normalizedParliaFinality),
            ReceiptProof = receiptProof,
            ReceiptProofHash = NormalizeReceiptProofHash(receiptProof, input.ReceiptProofHash),
            SourceEventDigest = sourceEventDigest,
            SourceBridgeEmitterAddress = normalizedSourceBridgeEmitterAddress,
        });
    }

    public static async ValueTask<byte[]> ProveInboundToSoraAsync(
        BscMainnetInboundEvidence input,
        IBscMainnetInboundProver inboundProver,
        IBscMainnetExecutionProvider? executionProvider = null,
        IBscMainnetConsensusProvider? consensusProvider = null,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(inboundProver);

        var evidence = await CollectInboundEvidenceFromReceiptAsync(
            input,
            executionProvider,
            consensusProvider,
            cancellationToken).ConfigureAwait(false);
        if (evidence.ParliaFinality is null)
        {
            throw new ArgumentException(
                "BSC mainnet SCCP inbound proof requires ParliaFinality.",
                nameof(input));
        }
        if (evidence.ReceiptProof is null)
        {
            throw new ArgumentException(
                "BSC mainnet SCCP inbound proof requires ReceiptProof.",
                nameof(input));
        }
        if (evidence.SourceEventDigest is null)
        {
            throw new ArgumentException(
                "BSC mainnet SCCP inbound proof requires receipt source event validation.",
                nameof(input));
        }
        var proofBytes = await inboundProver.ProveAsync(
            SnapshotInboundEvidence(evidence),
            cancellationToken).ConfigureAwait(false);
        return RequireNonZeroProofBytes(proofBytes, nameof(proofBytes));
    }

    public static async ValueTask<object?> SubmitInboundToIrohaAsync(
        byte[] proofBytes,
        IBscMainnetInboundSubmitter inboundSubmitter,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(inboundSubmitter);

        var proofCopy = RequireNonZeroProofBytes(proofBytes, nameof(proofBytes));
        return await inboundSubmitter.SubmitAsync(proofCopy, cancellationToken).ConfigureAwait(false);
    }

    public static BscMainnetLocalAdmissionSubmission BuildLocalAdmissionSubmission(
        BscMainnetLocalAdmissionSubmissionInput input)
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
                "BSC mainnet local-admission submission metadata is not canonical.",
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
        var payload = new BscMainnetLocalAdmissionPayload(
            ProofBytes: proofBytes,
            PublicInputsBytes: publicInputsBytes,
            BundleBytes: bundleBytes,
            StatementHash: statementHash,
            SourceVerifierMaterialHash: sourceVerifierMaterialHash,
            SourceAdapterEngineDeploymentHash: sourceAdapterEngineDeploymentHash);

        return new BscMainnetLocalAdmissionSubmission(
            ProofFamily: input.ProofFamily,
            VerifierBackend: input.VerifierBackend,
            SourceDomain: DomainBsc,
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

    public static void RequireInboundRoute(int sourceDomain, int targetDomain)
    {
        if (sourceDomain != DomainBsc || targetDomain != DomainSora)
        {
            throw new ArgumentException(
                "BSC mainnet inbound SCCP proofs must route BSC -> SORA.");
        }
    }

    public static void RequireOutboundRoute(int sourceDomain, int targetDomain)
    {
        if (sourceDomain != DomainSora || targetDomain != DomainBsc)
        {
            throw new ArgumentException(
                "BSC mainnet outbound SCCP proofs must route SORA -> BSC.");
        }
    }

    public static byte[] CanonicalBscSccpReceiptProofBytes(
        string sourceEventDigest,
        ulong validatorEpoch,
        ulong blockNumber,
        string blockHash,
        string receiptsRoot,
        string validatorSetHash,
        string commitSealHash,
        ulong receiptRootIndex,
        IReadOnlyList<byte[]> receiptTrieProofNodes,
        IReadOnlyList<byte[]> inclusionBranch,
        int sourceDomain = DomainBsc)
    {
        if (sourceDomain != DomainBsc)
        {
            throw new ArgumentException("sourceDomain must be BSC.", nameof(sourceDomain));
        }

        var nodes = NormalizeReceiptTrieProofNodes(receiptTrieProofNodes);
        var branch = NormalizeReceiptInclusionBranch(inclusionBranch, requireNonEmpty: true);
        using var payload = new MemoryStream();
        payload.WriteByte(1);
        payload.Write(LeU32(sourceDomain));
        payload.Write(RpcHexToBytes(sourceEventDigest, nameof(sourceEventDigest), 32));
        payload.Write(LeU64(validatorEpoch));
        payload.Write(LeU64(blockNumber));
        payload.Write(RpcHexToBytes(blockHash, nameof(blockHash), 32));
        payload.Write(RpcHexToBytes(receiptsRoot, nameof(receiptsRoot), 32));
        payload.Write(RpcHexToBytes(validatorSetHash, nameof(validatorSetHash), 32));
        payload.Write(RpcHexToBytes(commitSealHash, nameof(commitSealHash), 32));
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

    public static string BscSccpReceiptProofHash(
        string sourceEventDigest,
        ulong validatorEpoch,
        ulong blockNumber,
        string blockHash,
        string receiptsRoot,
        string validatorSetHash,
        string commitSealHash,
        ulong receiptRootIndex,
        IReadOnlyList<byte[]> receiptTrieProofNodes,
        IReadOnlyList<byte[]> inclusionBranch,
        int sourceDomain = DomainBsc)
        => PrefixedBlake2bHex(
            Encoding.UTF8.GetBytes(BscReceiptProofPrefix),
            CanonicalBscSccpReceiptProofBytes(
                sourceEventDigest,
                validatorEpoch,
                blockNumber,
                blockHash,
                receiptsRoot,
                validatorSetHash,
                commitSealHash,
                receiptRootIndex,
                receiptTrieProofNodes,
                inclusionBranch,
                sourceDomain));

    public static string SourceAdapterVerifierVkHash(
        int sourceDomain = DomainBsc,
        int targetDomain = DomainSora)
    {
        RequireInboundRoute(sourceDomain, targetDomain);

        using var verifier = new MemoryStream();
        verifier.WriteByte(1);
        verifier.Write(WriteBytes(Encoding.UTF8.GetBytes(SourceAdapterOpenVerifyCircuitId)));
        verifier.Write(WriteBytes(Encoding.UTF8.GetBytes(SourceChain)));
        verifier.Write(LeU32(sourceDomain));
        verifier.Write(LeU32(targetDomain));
        verifier.WriteByte(SourceProofPlan);
        verifier.WriteByte(SourceFinalityModel);
        verifier.Write(WriteBytes(Encoding.UTF8.GetBytes(SourceAdapterFastPqParameterSet)));
        verifier.Write(LeU32(128));
        verifier.Write(LeU32(23));
        verifier.Write(LeU32(16));
        verifier.Write(LeU64(SourceAdapterFastPqTraceRoot));
        verifier.Write(LeU32(19));
        verifier.Write(LeU64(SourceAdapterFastPqLdeRoot));
        verifier.Write(LeU32(65_536));
        verifier.WriteByte(1);
        verifier.Write(LeU32(19));
        verifier.Write(LeU64(SourceAdapterFastPqOmegaCoset));
        verifier.Write(WriteBytes(Encoding.UTF8.GetBytes("Goldilocks")));
        verifier.Write(WriteBytes(Encoding.UTF8.GetBytes("18446744069414584321")));
        verifier.Write(LeU32(2));
        verifier.Write(WriteBytes(Encoding.UTF8.GetBytes("Poseidon2(Goldilocks)")));
        verifier.Write(WriteBytes(Encoding.UTF8.GetBytes("SHA3-256")));
        verifier.Write(LeU32(8));
        verifier.Write(LeU32(8));
        verifier.Write(LeU32(8));
        verifier.Write(LeU32(46));

        return ToHex(SHA256.HashData(Concat(
            Encoding.UTF8.GetBytes(SourceAdapterOpenVerifyCircuitId),
            verifier.ToArray())));
    }

    public static byte[] CanonicalSourceVerifierMaterialBytes(
        BscMainnetSourceVerifierMaterialInput input)
    {
        var material = NormalizeSourceVerifierMaterial(input);
        using var payload = new MemoryStream();
        payload.WriteByte(1);
        payload.Write(LeU32(material.SourceDomain));
        payload.Write(WriteBytes(Encoding.UTF8.GetBytes(SourceChain)));
        payload.WriteByte(SourceProofPlan);
        payload.WriteByte(SourceFinalityModel);
        payload.Write(WriteBytes(Encoding.UTF8.GetBytes(SourceAdapterOpenVerifyCircuitId)));
        payload.Write(WriteBytes(Encoding.UTF8.GetBytes(SourceTrustAnchorId)));
        payload.Write(HexToBytes(material.SourceTrustAnchorHash, 32));
        payload.Write(WriteBytes(Encoding.UTF8.GetBytes(ConsensusVerifierId)));
        payload.Write(HexToBytes(material.ConsensusVerifierHash, 32));
        payload.Write(WriteBytes(Encoding.UTF8.GetBytes(MessageInclusionVerifierId)));
        payload.Write(HexToBytes(material.MessageInclusionVerifierHash, 32));
        payload.Write(WriteBytes(Encoding.UTF8.GetBytes(FinalityPolicyId)));
        payload.Write(HexToBytes(material.FinalityPolicyHash, 32));
        payload.Write(WriteBytes(Array.Empty<byte>()));
        payload.Write(new byte[32]);
        payload.Write(WriteBytes(Encoding.UTF8.GetBytes(SourceBridgeEmitterId)));
        payload.Write(WriteBytes(HexToBytes(material.BridgeAddress, 20)));
        payload.Write(HexToBytes(material.SourceBridgeEmitterCodeHash, 32));
        payload.Write(new byte[32]);
        payload.Write(WriteBytes(Array.Empty<byte>()));
        payload.Write(new byte[32]);
        payload.WriteByte(0);
        return payload.ToArray();
    }

    public static string SourceVerifierMaterialHash(BscMainnetSourceVerifierMaterialInput input)
        => PrefixedBlake2bHex(
            Encoding.UTF8.GetBytes(SourceVerifierMaterialRecordPrefix),
            CanonicalSourceVerifierMaterialBytes(input));

    public static byte[] CanonicalSourceAdapterEngineDeploymentBytes(
        BscMainnetSourceAdapterDeploymentInput input)
    {
        var deployment = NormalizeSourceAdapterDeployment(input);
        using var payload = new MemoryStream();
        payload.WriteByte(1);
        payload.Write(LeU32(deployment.SourceDomain));
        payload.Write(LeU32(deployment.TargetDomain));
        payload.Write(WriteBytes(Encoding.UTF8.GetBytes(SourceChain)));
        payload.WriteByte(SourceProofPlan);
        payload.WriteByte(SourceFinalityModel);
        payload.Write(WriteBytes(Encoding.UTF8.GetBytes(StarkFriProofFamily)));
        payload.Write(WriteBytes(Encoding.UTF8.GetBytes(SourceAdapterOpenVerifyCircuitId)));
        payload.Write(HexToBytes(deployment.AdapterVerifierVkHash, 32));
        payload.Write(WriteBytes(Encoding.UTF8.GetBytes(SourceTrustAnchorId)));
        payload.Write(HexToBytes(deployment.SourceTrustAnchorHash, 32));
        payload.Write(WriteBytes(Encoding.UTF8.GetBytes(ConsensusVerifierId)));
        payload.Write(HexToBytes(deployment.ConsensusVerifierHash, 32));
        payload.Write(WriteBytes(Encoding.UTF8.GetBytes(MessageInclusionVerifierId)));
        payload.Write(HexToBytes(deployment.MessageInclusionVerifierHash, 32));
        payload.Write(WriteBytes(Encoding.UTF8.GetBytes(FinalityPolicyId)));
        payload.Write(HexToBytes(deployment.FinalityPolicyHash, 32));
        payload.Write(WriteBytes(Array.Empty<byte>()));
        payload.Write(new byte[32]);
        payload.Write(WriteBytes(Encoding.UTF8.GetBytes(SourceBridgeEmitterId)));
        payload.Write(WriteBytes(HexToBytes(deployment.BridgeAddress, 20)));
        payload.Write(HexToBytes(deployment.SourceBridgeEmitterCodeHash, 32));
        payload.Write(new byte[32]);
        payload.Write(WriteBytes(Array.Empty<byte>()));
        payload.Write(new byte[32]);
        payload.Write(HexToBytes(deployment.DeploymentReceiptHash, 32));
        return payload.ToArray();
    }

    public static string SourceAdapterEngineDeploymentHash(
        BscMainnetSourceAdapterDeploymentInput input)
        => PrefixedBlake2bHex(
            Encoding.UTF8.GetBytes(SourceAdapterEngineDeploymentRecordPrefix),
            CanonicalSourceAdapterEngineDeploymentBytes(input));

    public static void RequireMainnetNetworkId(string networkId)
    {
        if (!string.Equals(networkId, MainnetNetworkId, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "BSC mainnet SCCP destination bindings must use the canonical chain id 56 "
                    + "bytes32 network id.",
                nameof(networkId));
        }
    }

    public static BscMainnetSccpDestinationBinding DestinationBinding(
        string verifierAddress,
        string bridgeAddress,
        string verifierCodeHash,
        string verifierKeyHash,
        string? networkId = null,
        int sourceDomain = DomainSora,
        int targetDomain = DomainBsc,
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
                "BSC mainnet destination bindings must use chain id 56.",
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
                "BSC mainnet verifierAddress must differ from bridgeAddress.",
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
                    "expectedBindingHash must match the BSC mainnet destination binding.",
                    nameof(expectedBindingHash));
            }
        }

        if (expectedKey is not null && !string.Equals(expectedKey.Trim(), key, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "expectedKey must match the BSC mainnet destination binding.",
                nameof(expectedKey));
        }

        return new BscMainnetSccpDestinationBinding(
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

    private static ulong NormalizeMainnetChainIdString(string value)
    {
        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException("eth_chainId must be canonical.", nameof(value));
        }

        if (value.StartsWith("0x", StringComparison.Ordinal))
        {
            var hex = value[2..];
            if (!IsCanonicalRpcQuantityHex(hex))
            {
                throw new ArgumentException(
                    "eth_chainId must be a canonical JSON-RPC quantity.",
                    nameof(value));
            }

            return Convert.ToUInt64(hex, 16);
        }

        if (value.Length == 0
            || (value != "0" && (value[0] == '0' || !value.All(IsDecimalDigit))))
        {
            throw new ArgumentException(
                "eth_chainId must be a canonical decimal integer.",
                nameof(value));
        }

        return ulong.Parse(value, System.Globalization.CultureInfo.InvariantCulture);
    }

    private static ulong NormalizeRpcChainId(object? value)
    {
        var quantity = NormalizeRpcQuantity(value, "eth_chainId");
        return Convert.ToUInt64(quantity[2..], 16);
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

    private static string NormalizeRpcHex(object? value, string parameterName, int byteLength)
    {
        return NormalizeRpcHex(value, parameterName, byteLength, allowZero: false);
    }

    private static string NormalizeRpcHex(
        object? value,
        string parameterName,
        int byteLength,
        bool allowZero)
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

    private static string? NormalizeReceiptProofHash(
        BscMainnetReceiptProof? receiptProof,
        string? suppliedHash)
    {
        var normalizedHash = suppliedHash is null
            ? null
            : NormalizeRpcHex(suppliedHash, nameof(BscMainnetInboundEvidence.ReceiptProofHash), 32);
        if (receiptProof is null)
        {
            return normalizedHash;
        }

        if (receiptProof.SourceDomain != DomainBsc)
        {
            throw new ArgumentException(
                "receiptProof.sourceDomain must be BSC.",
                nameof(receiptProof));
        }

        var computedHash = BscSccpReceiptProofHash(
            receiptProof.SourceEventDigest,
            receiptProof.ValidatorEpoch,
            receiptProof.BlockNumber,
            receiptProof.BlockHash,
            receiptProof.ReceiptsRoot,
            receiptProof.ValidatorSetHash,
            receiptProof.CommitSealHash,
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
        BscMainnetReceiptProof? receiptProof,
        string? blockHash,
        string? receiptBlockNumber,
        string? blockReceiptsRoot,
        IReadOnlyDictionary<string, object?>? parliaFinality,
        string? sourceEventDigest)
    {
        if (receiptProof is null)
        {
            return;
        }

        if (receiptBlockNumber is not null
            && receiptProof.BlockNumber != NormalizeUnsignedInteger(receiptBlockNumber, "block.number"))
        {
            throw new ArgumentException(
                "receiptProof.blockNumber must match block.number.",
                nameof(receiptProof));
        }

        if (parliaFinality is not null
            && receiptProof.BlockNumber != NormalizeUnsignedInteger(
                parliaFinality["executionBlockNumber"],
                "parliaFinality.executionBlockNumber"))
        {
            throw new ArgumentException(
                "receiptProof.blockNumber must match parliaFinality.executionBlockNumber.",
                nameof(receiptProof));
        }

        var proofBlockHash = NormalizeRpcHex(
            receiptProof.BlockHash,
            "receiptProof.blockHash",
            32);
        if (blockHash is not null
            && !string.Equals(proofBlockHash, blockHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "receiptProof.blockHash must match block.hash.",
                nameof(receiptProof));
        }

        if (parliaFinality is not null
            && !string.Equals(
                proofBlockHash,
                parliaFinality["executionBlockHash"] as string,
                StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "receiptProof.blockHash must match parliaFinality.executionBlockHash.",
                nameof(receiptProof));
        }

        var proofReceiptsRoot = NormalizeRpcHex(
            receiptProof.ReceiptsRoot,
            "receiptProof.receiptsRoot",
            32);
        if (blockReceiptsRoot is not null
            && !string.Equals(proofReceiptsRoot, blockReceiptsRoot, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "receiptProof.receiptsRoot must match block.receiptsRoot.",
                nameof(receiptProof));
        }

        if (parliaFinality is not null)
        {
            if (!string.Equals(
                proofReceiptsRoot,
                parliaFinality["executionReceiptsRoot"] as string,
                StringComparison.Ordinal))
            {
                throw new ArgumentException(
                    "receiptProof.receiptsRoot must match parliaFinality.executionReceiptsRoot.",
                    nameof(receiptProof));
            }

            var finalityValidatorEpochInput = FirstPresent(
                parliaFinality,
                "validatorEpoch",
                "validator_epoch");
            if (finalityValidatorEpochInput is not null
                && receiptProof.ValidatorEpoch != NormalizeUnsignedInteger(
                    finalityValidatorEpochInput,
                    "parliaFinality.validatorEpoch"))
            {
                throw new ArgumentException(
                    "receiptProof.validatorEpoch must match parliaFinality.validatorEpoch.",
                    nameof(receiptProof));
            }

            var finalityValidatorSetHashInput = FirstPresent(
                parliaFinality,
                "validatorSetHash",
                "validator_set_hash");
            if (finalityValidatorSetHashInput is not null)
            {
                var finalityValidatorSetHash = NormalizeRpcHex(
                    finalityValidatorSetHashInput,
                    "parliaFinality.validatorSetHash",
                    32);
                var proofValidatorSetHash = NormalizeRpcHex(
                    receiptProof.ValidatorSetHash,
                    "receiptProof.validatorSetHash",
                    32);
                if (!string.Equals(proofValidatorSetHash, finalityValidatorSetHash, StringComparison.Ordinal))
                {
                    throw new ArgumentException(
                        "receiptProof.validatorSetHash must match parliaFinality.validatorSetHash.",
                        nameof(receiptProof));
                }
            }

            var finalityCommitSealHashInput = FirstPresent(
                parliaFinality,
                "commitSealHash",
                "commit_seal_hash");
            if (finalityCommitSealHashInput is not null)
            {
                var finalityCommitSealHash = NormalizeRpcHex(
                    finalityCommitSealHashInput,
                    "parliaFinality.commitSealHash",
                    32);
                var proofCommitSealHash = NormalizeRpcHex(
                    receiptProof.CommitSealHash,
                    "receiptProof.commitSealHash",
                    32);
                if (!string.Equals(proofCommitSealHash, finalityCommitSealHash, StringComparison.Ordinal))
                {
                    throw new ArgumentException(
                        "receiptProof.commitSealHash must match parliaFinality.commitSealHash.",
                        nameof(receiptProof));
                }
            }
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

    private static SourceEvent NormalizeBscReceiptSourceEvent(
        IReadOnlyDictionary<string, object?>? receipt,
        string? sourceEventDigestInput,
        string? sourceBridgeEmitterAddressInput,
        string? transactionHash,
        string? blockHash,
        string? blockNumber)
    {
        var sourceEventDigest = sourceEventDigestInput is null
            ? null
            : NormalizeRpcHex(sourceEventDigestInput, nameof(BscMainnetInboundEvidence.SourceEventDigest), 32);
        var sourceBridgeEmitterAddress = sourceBridgeEmitterAddressInput is null
            ? null
            : NormalizeRpcHex(
                sourceBridgeEmitterAddressInput,
                nameof(BscMainnetInboundEvidence.SourceBridgeEmitterAddress),
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
            if (string.Equals(logAddress, sourceBridgeEmitterAddress, StringComparison.Ordinal)
                && normalizedTopics.Length > 0
                && string.Equals(normalizedTopics[0], SourceEventTopic, StringComparison.Ordinal))
            {
                if (normalizedTopics.Length != 2)
                {
                    throw new ArgumentException(
                        "SCCP source event log must contain exactly 2 topics.",
                        nameof(receipt));
                }

                var data = FirstPresent(log, "data") as string
                    ?? throw new ArgumentException(
                        $"receipt.logs[{index}].data is required.",
                        nameof(receipt));
                if (!string.Equals(data, "0x", StringComparison.Ordinal))
                {
                    throw new ArgumentException(
                        "SCCP source event log data must be 0x.",
                        nameof(receipt));
                }

                var logTransactionHash = NormalizeRpcHex(
                    FirstPresent(log, "transactionHash", "transaction_hash"),
                    $"receipt.logs[{index}].transactionHash",
                    32);
                if (transactionHash is not null
                    && !string.Equals(logTransactionHash, transactionHash, StringComparison.Ordinal))
                {
                    throw new ArgumentException(
                        "receipt.logs transactionHash must match receipt.transactionHash.",
                        nameof(receipt));
                }

                var logBlockHash = NormalizeRpcHex(
                    FirstPresent(log, "blockHash", "block_hash"),
                    $"receipt.logs[{index}].blockHash",
                    32);
                if (blockHash is not null
                    && !string.Equals(logBlockHash, blockHash, StringComparison.Ordinal))
                {
                    throw new ArgumentException(
                        "receipt.logs blockHash must match receipt.blockHash.",
                        nameof(receipt));
                }

                var logBlockNumber = NormalizePositiveRpcQuantity(
                    FirstPresent(log, "blockNumber", "block_number"),
                    $"receipt.logs[{index}].blockNumber");
                if (blockNumber is not null
                    && !string.Equals(logBlockNumber, blockNumber, StringComparison.Ordinal))
                {
                    throw new ArgumentException(
                        "receipt.logs blockNumber must match receipt.blockNumber.",
                        nameof(receipt));
                }

                var candidateDigest = normalizedTopics[1];
                if (IsZeroRpcHex(candidateDigest))
                {
                    throw new ArgumentException(
                        "SCCP source event digest must not be zero.",
                        nameof(receipt));
                }

                if (sourceEventDigest is not null
                    && !string.Equals(sourceEventDigest, candidateDigest, StringComparison.Ordinal))
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

    private static IReadOnlyList<object?> RequireList(object? value, string parameterName)
    {
        return value switch
        {
            IReadOnlyList<object?> list => list,
            IEnumerable<object?> enumerable => enumerable.ToArray(),
            _ => throw new ArgumentException(
                $"{parameterName} must be an array.",
                parameterName),
        };
    }

    private static bool IsZeroRpcHex(string text)
    {
        return text[2..].All(static character => character == '0');
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

    private static IReadOnlyDictionary<string, object?> NormalizeParliaFinality(
        IReadOnlyDictionary<string, object?> finality,
        string? expectedBlockHash,
        string? expectedBlockNumber,
        string? expectedReceiptsRoot)
    {
        ArgumentNullException.ThrowIfNull(finality);

        var executionBlockNumber = NormalizeUnsignedInteger(
            FirstPresent(
                finality,
                "executionBlockNumber",
                "execution_block_number",
                "finalityHeight",
                "finality_height"),
            "parliaFinality.executionBlockNumber");
        if (executionBlockNumber == 0)
        {
            throw new ArgumentException(
                "parliaFinality.executionBlockNumber must be positive.",
                nameof(finality));
        }

        if (expectedBlockNumber is not null
            && executionBlockNumber != NormalizeUnsignedInteger(expectedBlockNumber, "block.number"))
        {
            throw new ArgumentException(
                "parliaFinality.executionBlockNumber must match block.number.",
                nameof(finality));
        }

        var executionBlockHash = NormalizeRpcHex(
            FirstPresent(
                finality,
                "executionBlockHash",
                "execution_block_hash",
                "finalityBlockHash",
                "finality_block_hash"),
            "parliaFinality.executionBlockHash",
            32);
        if (expectedBlockHash is not null
            && !string.Equals(executionBlockHash, expectedBlockHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "parliaFinality.executionBlockHash must match block.hash.",
                nameof(finality));
        }

        var executionReceiptsRoot = NormalizeRpcHex(
            FirstPresent(
                finality,
                "executionReceiptsRoot",
                "execution_receipts_root",
                "receiptsRoot",
                "receipts_root"),
            "parliaFinality.executionReceiptsRoot",
            32);
        if (expectedReceiptsRoot is not null
            && !string.Equals(executionReceiptsRoot, expectedReceiptsRoot, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "parliaFinality.executionReceiptsRoot must match block.receiptsRoot.",
                nameof(finality));
        }

        var normalized = new Dictionary<string, object?>(finality)
        {
            ["executionBlockNumber"] = executionBlockNumber.ToString(
                System.Globalization.CultureInfo.InvariantCulture),
            ["executionBlockHash"] = executionBlockHash,
            ["executionReceiptsRoot"] = executionReceiptsRoot,
        };
        return normalized;
    }

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
                return NormalizeMainnetChainIdString(text);
            default:
                throw new ArgumentException(
                    $"{parameterName} must be an unsigned integer.",
                    parameterName);
        }
    }

    private static byte[] RpcHexToBytes(object? value, string parameterName, int byteLength)
    {
        var normalized = NormalizeRpcHex(value, parameterName, byteLength);
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

    private static BscMainnetReceiptProof? SnapshotReceiptProof(BscMainnetReceiptProof? receiptProof)
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

    private static BscMainnetInboundEvidence SnapshotInboundEvidence(
        BscMainnetInboundEvidence evidence)
    {
        return evidence with
        {
            Receipt = SnapshotDictionaryOrNull(evidence.Receipt),
            Block = SnapshotDictionaryOrNull(evidence.Block),
            ParliaFinality = SnapshotDictionaryOrNull(evidence.ParliaFinality),
        };
    }

    private static IReadOnlyDictionary<string, object?>? SnapshotDictionaryOrNull(
        IReadOnlyDictionary<string, object?>? dictionary)
        => dictionary is null ? null : SnapshotDictionary(dictionary);

    private static IReadOnlyDictionary<string, object?> SnapshotDictionary(
        IReadOnlyDictionary<string, object?> dictionary)
    {
        var snapshot = new Dictionary<string, object?>(dictionary.Count, StringComparer.Ordinal);
        foreach (var item in dictionary)
        {
            snapshot[item.Key] = SnapshotValue(item.Value);
        }

        return snapshot;
    }

    private static object? SnapshotValue(object? value)
    {
        return value switch
        {
            null => null,
            string text => text,
            byte[] bytes => bytes.ToArray(),
            IReadOnlyDictionary<string, object?> dictionary => SnapshotDictionary(dictionary),
            IReadOnlyList<object?> list => list.Select(SnapshotValue).ToArray(),
            System.Collections.IDictionary dictionary => SnapshotDictionary(dictionary),
            System.Collections.IEnumerable enumerable => SnapshotEnumerable(enumerable),
            _ => value,
        };
    }

    private static object SnapshotDictionary(System.Collections.IDictionary dictionary)
    {
        var snapshot = new Dictionary<string, object?>(dictionary.Count, StringComparer.Ordinal);
        foreach (System.Collections.DictionaryEntry item in dictionary)
        {
            if (item.Key is not string key)
            {
                return SnapshotObjectDictionary(dictionary);
            }

            snapshot[key] = SnapshotValue(item.Value);
        }

        return snapshot;
    }

    private static IReadOnlyDictionary<object, object?> SnapshotObjectDictionary(
        System.Collections.IDictionary dictionary)
    {
        var snapshot = new Dictionary<object, object?>(dictionary.Count);
        foreach (System.Collections.DictionaryEntry item in dictionary)
        {
            if (item.Key is null)
            {
                throw new ArgumentException(
                    "SCCP callback evidence dictionaries must not contain null keys.");
            }

            snapshot[item.Key] = SnapshotValue(item.Value);
        }

        return snapshot;
    }

    private static object?[] SnapshotEnumerable(System.Collections.IEnumerable enumerable)
    {
        var snapshot = new List<object?>();
        foreach (var item in enumerable)
        {
            snapshot.Add(SnapshotValue(item));
        }

        return snapshot.ToArray();
    }

    private static NormalizedBscSourceMaterial NormalizeSourceVerifierMaterial(
        BscMainnetSourceVerifierMaterialInput input)
    {
        ArgumentNullException.ThrowIfNull(input);
        RequireInboundRoute(input.SourceDomain, input.TargetDomain);

        var sourceTrustAnchorHash = NormalizeNonZeroHex(
            input.SourceTrustAnchorHash,
            nameof(input.SourceTrustAnchorHash),
            32);
        var consensusVerifierHash = NormalizeNonZeroHex(
            input.ConsensusVerifierHash,
            nameof(input.ConsensusVerifierHash),
            32);
        var messageInclusionVerifierHash = NormalizeNonZeroHex(
            input.MessageInclusionVerifierHash,
            nameof(input.MessageInclusionVerifierHash),
            32);
        var finalityPolicyHash = NormalizeNonZeroHex(
            input.FinalityPolicyHash,
            nameof(input.FinalityPolicyHash),
            32);
        var bridgeAddress = NormalizeNonZeroHex(
            input.BridgeAddress,
            nameof(input.BridgeAddress),
            20);
        var sourceBridgeEmitterCodeHash = NormalizeNonZeroHex(
            input.SourceBridgeEmitterCodeHash,
            nameof(input.SourceBridgeEmitterCodeHash),
            32);

        RequireRoleSeparated(
            "BSC mainnet source verifier material",
            (nameof(input.SourceTrustAnchorHash), sourceTrustAnchorHash),
            (nameof(input.ConsensusVerifierHash), consensusVerifierHash),
            (nameof(input.MessageInclusionVerifierHash), messageInclusionVerifierHash),
            (nameof(input.FinalityPolicyHash), finalityPolicyHash),
            (nameof(input.SourceBridgeEmitterCodeHash), sourceBridgeEmitterCodeHash));

        return new NormalizedBscSourceMaterial(
            SourceDomain: input.SourceDomain,
            TargetDomain: input.TargetDomain,
            SourceTrustAnchorHash: sourceTrustAnchorHash,
            ConsensusVerifierHash: consensusVerifierHash,
            MessageInclusionVerifierHash: messageInclusionVerifierHash,
            FinalityPolicyHash: finalityPolicyHash,
            BridgeAddress: bridgeAddress,
            SourceBridgeEmitterCodeHash: sourceBridgeEmitterCodeHash);
    }

    private static NormalizedBscSourceAdapterDeployment NormalizeSourceAdapterDeployment(
        BscMainnetSourceAdapterDeploymentInput input)
    {
        ArgumentNullException.ThrowIfNull(input);
        var material = NormalizeSourceVerifierMaterial(new BscMainnetSourceVerifierMaterialInput(
            SourceTrustAnchorHash: input.SourceTrustAnchorHash,
            ConsensusVerifierHash: input.ConsensusVerifierHash,
            MessageInclusionVerifierHash: input.MessageInclusionVerifierHash,
            FinalityPolicyHash: input.FinalityPolicyHash,
            BridgeAddress: input.BridgeAddress,
            SourceBridgeEmitterCodeHash: input.SourceBridgeEmitterCodeHash,
            SourceDomain: input.SourceDomain,
            TargetDomain: input.TargetDomain));
        var canonicalAdapterVerifierVkHash = SourceAdapterVerifierVkHash(
            material.SourceDomain,
            material.TargetDomain);
        var adapterVerifierVkHash = input.AdapterVerifierVkHash is null
            ? canonicalAdapterVerifierVkHash
            : NormalizeNonZeroHex(
                input.AdapterVerifierVkHash,
                nameof(input.AdapterVerifierVkHash),
                32);
        if (!string.Equals(adapterVerifierVkHash, canonicalAdapterVerifierVkHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "AdapterVerifierVkHash must match the canonical BSC source-adapter verifier profile.",
                nameof(input.AdapterVerifierVkHash));
        }

        var deploymentReceiptHash = NormalizeNonZeroHex(
            input.DeploymentReceiptHash,
            nameof(input.DeploymentReceiptHash),
            32);
        RequireRoleSeparated(
            "BSC mainnet source-adapter deployment",
            (nameof(input.SourceTrustAnchorHash), material.SourceTrustAnchorHash),
            (nameof(input.ConsensusVerifierHash), material.ConsensusVerifierHash),
            (nameof(input.MessageInclusionVerifierHash), material.MessageInclusionVerifierHash),
            (nameof(input.FinalityPolicyHash), material.FinalityPolicyHash),
            (nameof(input.AdapterVerifierVkHash), adapterVerifierVkHash),
            (nameof(input.SourceBridgeEmitterCodeHash), material.SourceBridgeEmitterCodeHash),
            (nameof(input.DeploymentReceiptHash), deploymentReceiptHash));

        return new NormalizedBscSourceAdapterDeployment(
            SourceDomain: material.SourceDomain,
            TargetDomain: material.TargetDomain,
            SourceTrustAnchorHash: material.SourceTrustAnchorHash,
            ConsensusVerifierHash: material.ConsensusVerifierHash,
            MessageInclusionVerifierHash: material.MessageInclusionVerifierHash,
            FinalityPolicyHash: material.FinalityPolicyHash,
            BridgeAddress: material.BridgeAddress,
            SourceBridgeEmitterCodeHash: material.SourceBridgeEmitterCodeHash,
            AdapterVerifierVkHash: adapterVerifierVkHash,
            DeploymentReceiptHash: deploymentReceiptHash);
    }

    private static void RequireRoleSeparated(
        string label,
        params (string Field, string Hash)[] roleHashes)
    {
        var seen = new Dictionary<string, string>(StringComparer.Ordinal);
        foreach (var (field, hash) in roleHashes)
        {
            if (seen.TryGetValue(hash, out var previousField))
            {
                throw new ArgumentException(
                    $"{label} hashes must be role-separated: {field} matches {previousField}.",
                    field);
            }

            seen.Add(hash, field);
        }
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

        if (byteLength != 20
            && (!value.StartsWith("0x", StringComparison.Ordinal)
                || value.Length != byteLength * 2 + 2
                || !value[2..].All(IsLowerHex)))
        {
            throw new ArgumentException(
                $"{parameterName} must be canonical lowercase 0x-prefixed {byteLength}-byte hex.",
                parameterName);
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

public sealed record BscMainnetSccpDestinationBinding(
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

public sealed record BscMainnetSourceVerifierMaterialInput(
    string SourceTrustAnchorHash,
    string ConsensusVerifierHash,
    string MessageInclusionVerifierHash,
    string FinalityPolicyHash,
    string BridgeAddress,
    string SourceBridgeEmitterCodeHash,
    int SourceDomain = BscMainnetSccp.DomainBsc,
    int TargetDomain = BscMainnetSccp.DomainSora);

public sealed record BscMainnetSourceAdapterDeploymentInput(
    string SourceTrustAnchorHash,
    string ConsensusVerifierHash,
    string MessageInclusionVerifierHash,
    string FinalityPolicyHash,
    string BridgeAddress,
    string SourceBridgeEmitterCodeHash,
    string DeploymentReceiptHash,
    string? AdapterVerifierVkHash = null,
    int SourceDomain = BscMainnetSccp.DomainBsc,
    int TargetDomain = BscMainnetSccp.DomainSora);

public sealed record BscMainnetLocalAdmissionSubmissionInput(
    byte[] ProofBytes,
    byte[] PublicInputsBytes,
    byte[] BundleBytes,
    byte[] EnvelopeBytes,
    string StatementHash,
    string SourceVerifierMaterialHash,
    string SourceAdapterEngineDeploymentHash,
    int SourceDomain = BscMainnetSccp.DomainBsc,
    int TargetDomain = BscMainnetSccp.DomainSora,
    string ProofFamily = BscMainnetSccp.StarkFriProofFamily,
    string VerifierBackend = BscMainnetSccp.EvmGroth16Bn254ProofBackend,
    string EnvelopeEncoding = BscMainnetSccp.LocalAdmissionEnvelopeEncoding,
    string SubmissionKind = BscMainnetSccp.LocalAdmissionSubmissionKind,
    string VerifierEntrypoint = BscMainnetSccp.LocalAdmissionEntrypoint);

public sealed record BscMainnetLocalAdmissionPayload(
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

public sealed record BscMainnetLocalAdmissionSubmission(
    string ProofFamily,
    string VerifierBackend,
    int SourceDomain,
    int TargetDomain,
    string StatementHash,
    string SourceVerifierMaterialHash,
    string SourceAdapterEngineDeploymentHash,
    BscMainnetLocalAdmissionPayload LocalAdmission,
    byte[] ProofBytes,
    byte[] PublicInputsBytes,
    byte[] BundleBytes,
    byte[] EnvelopeBytes)
{
    public int Version { get; } = 1;
    public string PlatformPayload { get; } = BscMainnetSccp.LocalAdmissionSubmissionKind;
    public string EnvelopeEncoding { get; } = BscMainnetSccp.LocalAdmissionEnvelopeEncoding;
    public string SubmissionKind { get; } = BscMainnetSccp.LocalAdmissionSubmissionKind;
    public string VerifierEntrypoint { get; } = BscMainnetSccp.LocalAdmissionEntrypoint;
    public IReadOnlyList<BscMainnetSccpSubmissionArgument> Arguments { get; } =
        Array.Empty<BscMainnetSccpSubmissionArgument>();
    public string ProofBytesHex { get; } = "0x" + Convert.ToHexString(ProofBytes).ToLowerInvariant();
    public string PublicInputsBytesHex { get; } = "0x" + Convert.ToHexString(PublicInputsBytes).ToLowerInvariant();
    public string BundleBytesHex { get; } = "0x" + Convert.ToHexString(BundleBytes).ToLowerInvariant();
    public string EnvelopeHex { get; } = "0x" + Convert.ToHexString(EnvelopeBytes).ToLowerInvariant();
}

public interface IBscMainnetExecutionProvider
{
    ValueTask<object?> RequestAsync(
        string method,
        IReadOnlyList<object?> parameters,
        CancellationToken cancellationToken = default);
}

public interface IBscMainnetConsensusProvider
{
    ValueTask<IReadOnlyDictionary<string, object?>> CollectFinalityEvidenceAsync(
        IReadOnlyDictionary<string, object?>? receipt,
        IReadOnlyDictionary<string, object?>? block,
        string? transactionHash,
        CancellationToken cancellationToken = default);
}

public interface IBscMainnetInboundProver
{
    ValueTask<byte[]> ProveAsync(
        BscMainnetInboundEvidence evidence,
        CancellationToken cancellationToken = default);
}

public interface IBscMainnetInboundSubmitter
{
    ValueTask<object?> SubmitAsync(
        byte[] proofBytes,
        CancellationToken cancellationToken = default);
}

public sealed record BscMainnetParliaFinalityEvidence(
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

public sealed record BscMainnetReceiptProof
{
    public int SourceDomain { get; init; } = BscMainnetSccp.DomainBsc;

    public string SourceEventDigest { get; init; } = string.Empty;

    public ulong ValidatorEpoch { get; init; }

    public ulong BlockNumber { get; init; }

    public string BlockHash { get; init; } = string.Empty;

    public string ReceiptsRoot { get; init; } = string.Empty;

    public string ValidatorSetHash { get; init; } = string.Empty;

    public string CommitSealHash { get; init; } = string.Empty;

    public ulong ReceiptRootIndex { get; init; }

    public IReadOnlyList<byte[]> ReceiptTrieProofNodes { get; init; } = Array.Empty<byte[]>();

    public IReadOnlyList<byte[]> InclusionBranch { get; init; } = Array.Empty<byte[]>();
}

public sealed record BscMainnetInboundEvidence
{
    public int SourceDomain { get; init; } = BscMainnetSccp.DomainBsc;

    public int TargetDomain { get; init; } = BscMainnetSccp.DomainSora;

    public string? TransactionHash { get; init; }

    public IReadOnlyDictionary<string, object?>? Receipt { get; init; }

    public IReadOnlyDictionary<string, object?>? Block { get; init; }

    public IReadOnlyDictionary<string, object?>? ParliaFinality { get; init; }

    public string? ReceiptProofHash { get; init; }

    public BscMainnetReceiptProof? ReceiptProof { get; init; }

    public string? SourceEventDigest { get; init; }

    public string? SourceBridgeEmitterAddress { get; init; }

    public BscMainnetInboundEvidence WithParliaFinalityEvidence(
        BscMainnetParliaFinalityEvidence? evidence,
        IEnumerable<KeyValuePair<string, object?>>? additionalFields = null)
    {
        return this with
        {
            ParliaFinality = evidence?.ToDictionary(additionalFields),
        };
    }
}
