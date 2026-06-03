using System.Buffers.Binary;
using System.Net.Http;
using System.Numerics;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
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
    public const string SourceAdapterOpenVerifyCircuitId = "sccp-source-adapter-v1";
    public const string SourceAdapterFastPqParameterSet = "fastpq-lane-balanced";

    private const string EvmDestinationBindingLabel = "iroha:sccp:evm-destination-binding:v1";
    private const string EthSourceBridgeConfigLabel = "iroha:sccp:eth-source-bridge-config:v1";
    private const string SourceVerifierMaterialRecordPrefix =
        "sccp:source-verifier-material-record:v1";
    private const string SourceAdapterEngineDeploymentRecordPrefix =
        "sccp:source-adapter-engine-deployment:v1";
    private const string EvmReceiptProofPrefix = "sccp:evm:receipt-proof:v1";
    private const string EthSyncCommitteePrefix = "sccp:eth:sync-committee:v1";
    private const string ProofRequestPrefix = "sccp:evm:groth16-proof-request:v1";
    private const string ProofEnvelopePrefix = "sccp:evm:groth16-proof-envelope:v1";
    private const int Groth16Bn254ProofAbiByteLength = 384;
    private const int EthMaxSyncCommitteeAuthorities = 512;
    private const int EthSyncCommitteePublicKeyBytes = 48;
    private const int EthMaxSyncCommitteePublicKeyBytes = 96;
    private const int EthSyncCommitteePopBytes = 96;
    private const int EthMaxSyncCommitteePopBytes = 256;
    private const int EthMaxSyncCommitteePayloadBytes = 1 + 4
        + EthMaxSyncCommitteeAuthorities
        * (4 + EthMaxSyncCommitteePublicKeyBytes + 8 + 4 + EthMaxSyncCommitteePopBytes);
    private const int Keccak256Rate = 136;
    private const int MaxSourceMerkleBranchNodes = 64;
    private const int MaxMptProofNodes = 64;
    private const int MaxMptNodeBytes = 16 * 1024;
    private const int EvmMaxBlockReceipts = 4096;
    private const ulong SourceAdapterFastPqTraceRoot = 0x002A_247F_81C6_F850UL;
    private const ulong SourceAdapterFastPqLdeRoot = 0x6026_3388_DBBF_9B2AUL;
    private const ulong SourceAdapterFastPqOmegaCoset = 0x6AF3_25E8_25AD_5C18UL;
    private const string SourceChain = "eth";
    private const byte SourceProofPlan = 1;
    private const byte SourceFinalityModel = 1;
    private const string SourceTrustAnchorId =
        "sccp:eth:source-trust-anchor:ethereum-mainnet-beacon-finalized-checkpoint:v1";
    private const string ConsensusVerifierId =
        "sccp:eth:consensus-verifier:beacon-sync-committee-execution-header-mainnet:v1";
    private const string MessageInclusionVerifierId =
        "sccp:eth:message-inclusion-verifier:execution-receipt-trie-branch-mainnet:v1";
    private const string FinalityPolicyId =
        "sccp:eth:finality-policy:beacon-finalized-checkpoint-mainnet:v1";
    private const string SourceBridgeEmitterId =
        "sccp:eth:source-bridge-emitter:ethereum-mainnet:v1";
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

    private readonly record struct NormalizedEthereumSourceMaterial(
        int SourceDomain,
        int TargetDomain,
        string SourceTrustAnchorHash,
        string ConsensusVerifierHash,
        string MessageInclusionVerifierHash,
        string FinalityPolicyHash,
        string BridgeAddress,
        string SourceBridgeEmitterCodeHash,
        string NetworkId,
        string SourceBridgeConfigHash);

    private readonly record struct NormalizedEthereumSourceAdapterDeployment(
        int SourceDomain,
        int TargetDomain,
        string SourceTrustAnchorHash,
        string ConsensusVerifierHash,
        string MessageInclusionVerifierHash,
        string FinalityPolicyHash,
        string BridgeAddress,
        string SourceBridgeEmitterCodeHash,
        string NetworkId,
        string SourceBridgeConfigHash,
        string AdapterVerifierVkHash,
        string DeploymentReceiptHash);

    private readonly record struct Bn254Fq2(BigInteger C0, BigInteger C1);

    private readonly record struct Bn254G2Projective(
        Bn254Fq2 X,
        Bn254Fq2 Y,
        Bn254Fq2 Z,
        bool Infinity);

    private sealed record EvmTrieItem(IReadOnlyList<int> Path, byte[] Value);

    private abstract class EvmTrieNode
    {
        public byte[]? Rlp { get; set; }
    }

    private sealed class EvmTrieLeaf(IReadOnlyList<int> path, byte[] value) : EvmTrieNode
    {
        public IReadOnlyList<int> Path { get; } = path;

        public byte[] Value { get; } = value;
    }

    private sealed class EvmTrieExtension(IReadOnlyList<int> path, EvmTrieNode child) : EvmTrieNode
    {
        public IReadOnlyList<int> Path { get; } = path;

        public EvmTrieNode Child { get; } = child;
    }

    private sealed class EvmTrieBranch(IReadOnlyList<EvmTrieNode?> children, byte[] value) : EvmTrieNode
    {
        public IReadOnlyList<EvmTrieNode?> Children { get; } = children;

        public byte[] Value { get; } = value;
    }

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
        RequireMainnetChainId(NormalizeRpcChainId(chainId));
        return chainId;
    }

    public static byte[] CanonicalEvmReceiptRlp(IReadOnlyDictionary<string, object?> receipt)
    {
        ArgumentNullException.ThrowIfNull(receipt);

        var status = RequireEthereumRpcQuantity(FirstPresent(receipt, "status"), "receipt.status");
        if (status is not 0UL and not 1UL)
        {
            throw new ArgumentException("receipt.status must be 0x0 or 0x1.", nameof(receipt));
        }

        var fields = new[]
        {
            RlpBytes(MinimalBigEndianBytes(status)),
            RlpBytes(MinimalBigEndianBytes(RequireEthereumRpcQuantity(
                FirstPresent(receipt, "cumulativeGasUsed", "cumulative_gas_used"),
                "receipt.cumulativeGasUsed"))),
            RlpBytes(EthereumRpcHexBytes(
                FirstPresent(receipt, "logsBloom", "logs_bloom"),
                "receipt.logsBloom",
                byteLength: 256,
                nonZero: false,
                allowEmpty: false)),
            RlpList(EvmReceiptLogsForRlp(receipt)),
        };
        var payload = RlpList(fields);
        var receiptType = EvmReceiptType(receipt);
        if (receiptType is null)
        {
            return payload;
        }

        return Concat(new[] { receiptType.Value }, payload);
    }

    public static string EvmReceiptTrieKey(object? transactionIndex)
    {
        var index = NormalizeUnsignedInteger(transactionIndex, "transactionIndex");
        return ToHex(RlpBytes(MinimalBigEndianBytes(index)));
    }

    public static EvmReceiptTrieProof BuildEvmReceiptTrieProofFromReceipts(
        IReadOnlyList<IReadOnlyDictionary<string, object?>> receipts,
        object? transactionIndex)
    {
        ArgumentNullException.ThrowIfNull(receipts);
        if (receipts.Count == 0 || receipts.Count > EvmMaxBlockReceipts)
        {
            throw new ArgumentException(
                $"blockReceipts must contain 1..{EvmMaxBlockReceipts} entries.",
                nameof(receipts));
        }

        var targetIndex = NormalizeUnsignedInteger(transactionIndex, "transactionIndex");
        if (targetIndex >= (ulong)receipts.Count)
        {
            throw new ArgumentOutOfRangeException(
                nameof(transactionIndex),
                transactionIndex,
                "transactionIndex must select a block receipt.");
        }

        var items = new List<EvmTrieItem>(receipts.Count);
        byte[]? targetReceiptRlp = null;
        for (var index = 0; index < receipts.Count; index++)
        {
            var receipt = receipts[index]
                ?? throw new ArgumentException($"blockReceipts[{index}] is required.", nameof(receipts));
            var receiptIndex = RequireEthereumRpcQuantity(
                FirstPresent(receipt, "transactionIndex", "transaction_index"),
                $"blockReceipts[{index}].transactionIndex");
            if (receiptIndex != (ulong)index)
            {
                throw new ArgumentException(
                    "block receipt transactionIndex must match receipt order.",
                    nameof(receipts));
            }

            var encodedReceipt = CanonicalEvmReceiptRlp(receipt);
            if (receiptIndex == targetIndex)
            {
                targetReceiptRlp = encodedReceipt;
            }

            var key = RlpBytes(MinimalBigEndianBytes((ulong)index));
            items.Add(new EvmTrieItem(BytesToNibbles(key), encodedReceipt));
        }

        var root = BuildEvmTrieNode(items);
        var receiptsRoot = ToHex(Keccak256(EncodeEvmTrieNode(root)));
        var receiptTrieKey = RlpBytes(MinimalBigEndianBytes(targetIndex));
        var proofNodes = CollectEvmTrieProofNodes(root, BytesToNibbles(receiptTrieKey));
        _ = NormalizeReceiptTrieProofNodes(proofNodes);
        if (targetReceiptRlp is null)
        {
            throw new ArgumentException(
                "transactionIndex must select a block receipt.",
                nameof(transactionIndex));
        }

        return new EvmReceiptTrieProof(
            receiptsRoot,
            ToHex(targetReceiptRlp),
            ToHex(receiptTrieKey),
            CopyByteArrays(proofNodes));
    }

    public static async ValueTask<EthereumMainnetInboundEvidence> CollectInboundEvidenceFromReceiptAsync(
        EthereumMainnetInboundEvidence input,
        IEthereumMainnetExecutionProvider? executionProvider = null,
        IEthereumMainnetConsensusProvider? consensusProvider = null,
        CancellationToken cancellationToken = default,
        string? sourceBridgeEmitterAddress = null)
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
            receipt is null
                && input.SourceEventDigest is null
                && input.SourceBridgeEmitterAddress is null
                    ? null
                    : ResolveSourceBridgeEmitterAddress(
                        input.SourceBridgeEmitterAddress,
                        sourceBridgeEmitterAddress),
            transactionHash,
            blockHash,
            receiptBlockNumber);

        var blockReceipts = input.BlockReceipts;
        if (receiptProof is null
            && receipt is not null
            && beaconFinality is not null
            && sourceEvent.SourceEventDigest is not null
            && input.InclusionBranch is not null)
        {
            if (blockReceipts is null)
            {
                if (executionProvider is null)
                {
                    throw new InvalidOperationException(
                        "Ethereum mainnet execution provider is not linked for block receipt evidence collection.");
                }

                if (receiptBlockNumber is null)
                {
                    throw new ArgumentException("receipt.blockNumber is required.", nameof(input));
                }

                blockReceipts = RequireDictionaryList(
                    await executionProvider.RequestAsync(
                        "eth_getBlockReceipts",
                        new object?[] { receiptBlockNumber },
                        cancellationToken).ConfigureAwait(false),
                    "eth_getBlockReceipts");
            }

            var receiptTransactionIndex = FirstPresent(receipt, "transactionIndex", "transaction_index");
            var receiptTrieProof = BuildEvmReceiptTrieProofFromReceipts(
                blockReceipts,
                receiptTransactionIndex);
            var expectedReceiptsRoot = blockReceiptsRoot
                ?? FirstPresent(beaconFinality, "executionReceiptsRoot", "execution_receipts_root") as string;
            if (expectedReceiptsRoot is null
                || !string.Equals(receiptTrieProof.ReceiptsRoot, expectedReceiptsRoot, StringComparison.Ordinal))
            {
                throw new ArgumentException(
                    "receiptProof.executionReceiptsRoot must match computed receipt trie root.",
                    nameof(input));
            }

            var targetIndex = NormalizeUnsignedInteger(
                receiptTransactionIndex,
                "receipt.transactionIndex");
            if (targetIndex >= (ulong)blockReceipts.Count)
            {
                throw new ArgumentException(
                    "receipt.transactionIndex must select an eth_getBlockReceipts entry.",
                    nameof(input));
            }

            var indexedReceipt = blockReceipts[(int)targetIndex];
            var indexedTransactionHash = NormalizeRpcHex(
                FirstPresent(indexedReceipt, "transactionHash", "transaction_hash"),
                "blockReceipts.transactionHash",
                32);
            if (transactionHash is null
                || !string.Equals(indexedTransactionHash, transactionHash, StringComparison.Ordinal))
            {
                throw new ArgumentException(
                    "blockReceipts.transactionHash must match transactionHash.",
                    nameof(input));
            }

            var indexedBlockHash = NormalizeRpcHex(
                FirstPresent(indexedReceipt, "blockHash", "block_hash"),
                "blockReceipts.blockHash",
                32);
            if (!string.Equals(indexedBlockHash, blockHash, StringComparison.Ordinal))
            {
                throw new ArgumentException(
                    "blockReceipts.blockHash must match receipt.",
                    nameof(input));
            }

            var indexedBlockNumber = NormalizePositiveRpcQuantity(
                FirstPresent(indexedReceipt, "blockNumber", "block_number"),
                "blockReceipts.blockNumber");
            if (!string.Equals(indexedBlockNumber, receiptBlockNumber, StringComparison.Ordinal))
            {
                throw new ArgumentException(
                    "blockReceipts.blockNumber must match receipt.",
                    nameof(input));
            }

            var receiptRlp = ToHex(CanonicalEvmReceiptRlp(receipt));
            if (!string.Equals(receiptTrieProof.ReceiptRlp, receiptRlp, StringComparison.Ordinal))
            {
                throw new ArgumentException(
                    "blockReceipts.receiptRlp must match receipt.",
                    nameof(input));
            }

            receiptProof = new EthereumMainnetReceiptProof
            {
                SourceEventDigest = sourceEvent.SourceEventDigest,
                BeaconSlot = NormalizeUnsignedInteger(
                    FirstPresent(
                        beaconFinality,
                        "beaconSlot",
                        "beacon_slot",
                        "finalizedSlot",
                        "finalized_slot",
                        "slot"),
                    "beaconFinality.beaconSlot"),
                ExecutionBlockNumber = NormalizeUnsignedInteger(
                    FirstPresent(beaconFinality, "executionBlockNumber", "execution_block_number"),
                    "beaconFinality.executionBlockNumber"),
                ExecutionBlockHash = NormalizeRpcHex(
                    FirstPresent(beaconFinality, "executionBlockHash", "execution_block_hash"),
                    "beaconFinality.executionBlockHash",
                    32),
                ExecutionReceiptsRoot = NormalizeRpcHex(
                    FirstPresent(beaconFinality, "executionReceiptsRoot", "execution_receipts_root"),
                    "beaconFinality.executionReceiptsRoot",
                    32),
                BeaconFinalizedRoot = NormalizeRpcHex(
                    FirstPresent(
                        beaconFinality,
                        "finalizedHeaderRoot",
                        "finalized_header_root",
                        "beaconFinalizedRoot",
                        "beacon_finalized_root"),
                    "beaconFinality.finalizedHeaderRoot",
                    32),
                SyncCommitteeRoot = NormalizeRpcHex(
                    FirstPresent(beaconFinality, "syncCommitteeRoot", "sync_committee_root"),
                    "beaconFinality.syncCommitteeRoot",
                    32),
                ReceiptRootIndex = targetIndex,
                ReceiptTrieProofNodes = receiptTrieProof.ReceiptTrieProofNodes,
                InclusionBranch = NormalizeReceiptInclusionBranch(input.InclusionBranch, requireNonEmpty: true),
            };
        }

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
            BlockReceipts = blockReceipts,
            InclusionBranch = input.InclusionBranch is null
                ? null
                : CopyByteArrays(input.InclusionBranch),
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
        CancellationToken cancellationToken = default,
        string? sourceBridgeEmitterAddress = null)
    {
        ArgumentNullException.ThrowIfNull(inboundProver);

        var evidence = await CollectInboundEvidenceFromReceiptAsync(
            input,
            executionProvider,
            consensusProvider,
            cancellationToken,
            sourceBridgeEmitterAddress).ConfigureAwait(false);
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
        if (evidence.Receipt is not null && evidence.SourceEventDigest is null)
        {
            throw new ArgumentException(
                "Ethereum mainnet SCCP inbound proof requires receipt source event validation.",
                nameof(input));
        }
        if (!evidence.BeaconFinality.ContainsKey("finalizedHeaderRoot"))
        {
            throw new ArgumentException(
                "Ethereum mainnet SCCP inbound proof requires beaconFinality.finalizedHeaderRoot.",
                nameof(input));
        }

        if (!evidence.BeaconFinality.ContainsKey("syncCommitteeRoot"))
        {
            throw new ArgumentException(
                "Ethereum mainnet SCCP inbound proof requires beaconFinality.syncCommitteeRoot.",
                nameof(input));
        }

        if (!evidence.BeaconFinality.ContainsKey("beaconSlot"))
        {
            throw new ArgumentException(
                "Ethereum mainnet SCCP inbound proof requires beaconFinality.beaconSlot.",
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

    public static string EthSyncCommitteeHashFromPayload(byte[] syncCommitteePayload)
    {
        ArgumentNullException.ThrowIfNull(syncCommitteePayload);
        ValidateEthSyncCommitteePayload(syncCommitteePayload);
        return PrefixedBlake2bHex(Encoding.UTF8.GetBytes(EthSyncCommitteePrefix), syncCommitteePayload);
    }

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
        => await SubmitOutboundToEthereumAsync(
            input,
            outboundSubmitter,
            executionProvider: null,
            cancellationToken).ConfigureAwait(false);

    public static async ValueTask<object?> SubmitOutboundToEthereumAsync(
        EthereumMainnetSccpSubmissionInput input,
        IEthereumMainnetOutboundSubmitter outboundSubmitter,
        IEthereumMainnetExecutionProvider? executionProvider,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(outboundSubmitter);

        var submission = BuildEthereumCalldata(input);
        if (executionProvider is not null)
        {
            _ = await ValidateExecutionProviderMainnetAsync(
                executionProvider,
                cancellationToken).ConfigureAwait(false);
        }
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

    public static string SourceBridgeConfigHash(
        string bridgeAddress,
        string sourceBridgeEmitterCodeHash,
        string? networkId = null,
        int sourceDomain = DomainEthereum,
        int targetDomain = DomainSora)
    {
        RequireInboundRoute(sourceDomain, targetDomain);
        var canonicalNetworkId = NormalizeEthereumMainnetNetworkId(networkId);
        var canonicalBridgeAddress = NormalizeNonZeroHex(
            bridgeAddress,
            nameof(bridgeAddress),
            20);
        var canonicalCodeHash = NormalizeNonZeroHex(
            sourceBridgeEmitterCodeHash,
            nameof(sourceBridgeEmitterCodeHash),
            32);

        using var payload = new MemoryStream();
        payload.Write(Keccak256(Encoding.UTF8.GetBytes(EthSourceBridgeConfigLabel)));
        payload.Write(AbiWordAddress20(canonicalBridgeAddress));
        payload.Write(HexToBytes(canonicalNetworkId, 32));
        payload.Write(AbiWordU32(sourceDomain));
        payload.Write(AbiWordU32(targetDomain));
        payload.Write(HexToBytes(canonicalCodeHash, 32));
        return ToHex(Keccak256(payload.ToArray()));
    }

    public static string SourceAdapterVerifierVkHash(
        int sourceDomain = DomainEthereum,
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
        EthereumMainnetSourceVerifierMaterialInput input)
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
        payload.Write(HexToBytes(material.NetworkId, 32));
        payload.Write(WriteBytes(Array.Empty<byte>()));
        payload.Write(HexToBytes(material.SourceBridgeConfigHash, 32));
        payload.WriteByte(0);
        return payload.ToArray();
    }

    public static string SourceVerifierMaterialHash(EthereumMainnetSourceVerifierMaterialInput input)
        => PrefixedBlake2bHex(
            Encoding.UTF8.GetBytes(SourceVerifierMaterialRecordPrefix),
            CanonicalSourceVerifierMaterialBytes(input));

    public static byte[] CanonicalSourceAdapterEngineDeploymentBytes(
        EthereumMainnetSourceAdapterDeploymentInput input)
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
        payload.Write(HexToBytes(deployment.NetworkId, 32));
        payload.Write(WriteBytes(Array.Empty<byte>()));
        payload.Write(HexToBytes(deployment.SourceBridgeConfigHash, 32));
        payload.Write(HexToBytes(deployment.DeploymentReceiptHash, 32));
        return payload.ToArray();
    }

    public static string SourceAdapterEngineDeploymentHash(
        EthereumMainnetSourceAdapterDeploymentInput input)
        => PrefixedBlake2bHex(
            Encoding.UTF8.GetBytes(SourceAdapterEngineDeploymentRecordPrefix),
            CanonicalSourceAdapterEngineDeploymentBytes(input));

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

    private static void ValidateEthSyncCommitteePayload(byte[] payload)
    {
        if (payload.Length > EthMaxSyncCommitteePayloadBytes)
        {
            throw new ArgumentException(
                $"syncCommitteePayload must be at most {EthMaxSyncCommitteePayloadBytes} bytes.",
                nameof(payload));
        }

        var cursor = 0;
        if (payload.Length < 5 || payload[cursor] != 1)
        {
            throw new ArgumentException("syncCommitteePayload must have version 1.", nameof(payload));
        }

        cursor++;
        var count = ReadU32Le(payload, ref cursor, "syncCommitteePayload");
        if (count == 0)
        {
            throw new ArgumentException("syncCommitteePayload must not be empty.", nameof(payload));
        }
        if (count > EthMaxSyncCommitteeAuthorities)
        {
            throw new ArgumentException(
                $"syncCommitteePayload must contain at most {EthMaxSyncCommitteeAuthorities} entries.",
                nameof(payload));
        }

        var seenPublicKeys = new HashSet<string>(StringComparer.Ordinal);
        for (var index = 0; index < count; index++)
        {
            var publicKeyLength = ReadU32Le(payload, ref cursor, $"syncCommitteePublicKeys[{index}]");
            if (publicKeyLength != EthSyncCommitteePublicKeyBytes
                || cursor + publicKeyLength > payload.Length)
            {
                throw new ArgumentException(
                    $"syncCommitteePublicKeys[{index}] is invalid.",
                    nameof(payload));
            }
            var publicKey = payload.AsSpan(cursor, publicKeyLength).ToArray();
            cursor += publicKeyLength;
            if (IsAllZero(publicKey))
            {
                throw new ArgumentException(
                    $"syncCommitteePublicKeys[{index}] must not be zero.",
                    nameof(payload));
            }
            if (!seenPublicKeys.Add(Convert.ToHexString(publicKey).ToLowerInvariant()))
            {
                throw new ArgumentException(
                    $"syncCommitteePublicKeys[{index}] must be unique.",
                    nameof(payload));
            }

            var weight = ReadU64Le(payload, ref cursor, $"syncCommitteeWeights[{index}]");
            if (weight == 0)
            {
                throw new ArgumentException(
                    $"syncCommitteeWeights[{index}] must not be zero.",
                    nameof(payload));
            }

            var popLength = ReadU32Le(payload, ref cursor, $"syncCommitteePops[{index}]");
            if (popLength != EthSyncCommitteePopBytes || cursor + popLength > payload.Length)
            {
                throw new ArgumentException(
                    $"syncCommitteePops[{index}] is invalid.",
                    nameof(payload));
            }
            var pop = payload.AsSpan(cursor, popLength);
            if (IsAllZero(pop))
            {
                throw new ArgumentException(
                    $"syncCommitteePops[{index}] must not be zero.",
                    nameof(payload));
            }
            cursor += popLength;
        }

        if (cursor != payload.Length)
        {
            throw new ArgumentException("syncCommitteePayload has trailing bytes.", nameof(payload));
        }
    }

    private static int ReadU32Le(byte[] payload, ref int cursor, string label)
    {
        if (cursor + 4 > payload.Length)
        {
            throw new ArgumentException($"{label} is truncated.", nameof(payload));
        }
        var value = BinaryPrimitives.ReadUInt32LittleEndian(payload.AsSpan(cursor, 4));
        cursor += 4;
        if (value > int.MaxValue)
        {
            throw new ArgumentException($"{label} is too large.", nameof(payload));
        }
        return (int)value;
    }

    private static ulong ReadU64Le(byte[] payload, ref int cursor, string label)
    {
        if (cursor + 8 > payload.Length)
        {
            throw new ArgumentException($"{label} is truncated.", nameof(payload));
        }
        var value = BinaryPrimitives.ReadUInt64LittleEndian(payload.AsSpan(cursor, 8));
        cursor += 8;
        return value;
    }

    private static bool IsAllZero(ReadOnlySpan<byte> value)
    {
        foreach (var item in value)
        {
            if (item != 0)
            {
                return false;
            }
        }
        return true;
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

    private static NormalizedEthereumSourceMaterial NormalizeSourceVerifierMaterial(
        EthereumMainnetSourceVerifierMaterialInput input)
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
        var networkId = NormalizeEthereumMainnetNetworkId(input.NetworkId);
        var expectedConfigHash = SourceBridgeConfigHash(
            bridgeAddress,
            sourceBridgeEmitterCodeHash,
            networkId,
            input.SourceDomain,
            input.TargetDomain);
        var sourceBridgeConfigHash = input.SourceBridgeConfigHash is null
            ? expectedConfigHash
            : NormalizeNonZeroHex(
                input.SourceBridgeConfigHash,
                nameof(input.SourceBridgeConfigHash),
                32);
        if (!string.Equals(sourceBridgeConfigHash, expectedConfigHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "SourceBridgeConfigHash must match the Ethereum mainnet source bridge config fields.",
                nameof(input.SourceBridgeConfigHash));
        }

        RequireRoleSeparated(
            "Ethereum mainnet source verifier material",
            (nameof(input.SourceTrustAnchorHash), sourceTrustAnchorHash),
            (nameof(input.ConsensusVerifierHash), consensusVerifierHash),
            (nameof(input.MessageInclusionVerifierHash), messageInclusionVerifierHash),
            (nameof(input.FinalityPolicyHash), finalityPolicyHash),
            (nameof(input.SourceBridgeEmitterCodeHash), sourceBridgeEmitterCodeHash),
            (nameof(input.NetworkId), networkId),
            (nameof(input.SourceBridgeConfigHash), sourceBridgeConfigHash));

        return new NormalizedEthereumSourceMaterial(
            SourceDomain: input.SourceDomain,
            TargetDomain: input.TargetDomain,
            SourceTrustAnchorHash: sourceTrustAnchorHash,
            ConsensusVerifierHash: consensusVerifierHash,
            MessageInclusionVerifierHash: messageInclusionVerifierHash,
            FinalityPolicyHash: finalityPolicyHash,
            BridgeAddress: bridgeAddress,
            SourceBridgeEmitterCodeHash: sourceBridgeEmitterCodeHash,
            NetworkId: networkId,
            SourceBridgeConfigHash: sourceBridgeConfigHash);
    }

    private static NormalizedEthereumSourceAdapterDeployment NormalizeSourceAdapterDeployment(
        EthereumMainnetSourceAdapterDeploymentInput input)
    {
        ArgumentNullException.ThrowIfNull(input);
        var material = NormalizeSourceVerifierMaterial(new EthereumMainnetSourceVerifierMaterialInput(
            SourceTrustAnchorHash: input.SourceTrustAnchorHash,
            ConsensusVerifierHash: input.ConsensusVerifierHash,
            MessageInclusionVerifierHash: input.MessageInclusionVerifierHash,
            FinalityPolicyHash: input.FinalityPolicyHash,
            BridgeAddress: input.BridgeAddress,
            SourceBridgeEmitterCodeHash: input.SourceBridgeEmitterCodeHash,
            NetworkId: input.NetworkId,
            SourceBridgeConfigHash: input.SourceBridgeConfigHash,
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
                "AdapterVerifierVkHash must match the canonical Ethereum mainnet source-adapter verifier profile.",
                nameof(input.AdapterVerifierVkHash));
        }

        var deploymentReceiptHash = NormalizeNonZeroHex(
            input.DeploymentReceiptHash,
            nameof(input.DeploymentReceiptHash),
            32);
        RequireRoleSeparated(
            "Ethereum mainnet source-adapter deployment",
            (nameof(input.SourceTrustAnchorHash), material.SourceTrustAnchorHash),
            (nameof(input.ConsensusVerifierHash), material.ConsensusVerifierHash),
            (nameof(input.MessageInclusionVerifierHash), material.MessageInclusionVerifierHash),
            (nameof(input.FinalityPolicyHash), material.FinalityPolicyHash),
            (nameof(input.AdapterVerifierVkHash), adapterVerifierVkHash),
            (nameof(input.SourceBridgeEmitterCodeHash), material.SourceBridgeEmitterCodeHash),
            (nameof(input.NetworkId), material.NetworkId),
            (nameof(input.SourceBridgeConfigHash), material.SourceBridgeConfigHash),
            (nameof(input.DeploymentReceiptHash), deploymentReceiptHash));

        return new NormalizedEthereumSourceAdapterDeployment(
            SourceDomain: material.SourceDomain,
            TargetDomain: material.TargetDomain,
            SourceTrustAnchorHash: material.SourceTrustAnchorHash,
            ConsensusVerifierHash: material.ConsensusVerifierHash,
            MessageInclusionVerifierHash: material.MessageInclusionVerifierHash,
            FinalityPolicyHash: material.FinalityPolicyHash,
            BridgeAddress: material.BridgeAddress,
            SourceBridgeEmitterCodeHash: material.SourceBridgeEmitterCodeHash,
            NetworkId: material.NetworkId,
            SourceBridgeConfigHash: material.SourceBridgeConfigHash,
            AdapterVerifierVkHash: adapterVerifierVkHash,
            DeploymentReceiptHash: deploymentReceiptHash);
    }

    private static string NormalizeEthereumMainnetNetworkId(string? networkId)
    {
        var canonicalNetworkId = NormalizeNonZeroHex(
            networkId ?? MainnetNetworkId,
            nameof(networkId),
            32);
        if (!string.Equals(canonicalNetworkId, MainnetNetworkId, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "Ethereum mainnet source material must use chain id 1.",
                nameof(networkId));
        }

        return canonicalNetworkId;
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

    private static ulong NormalizeRpcChainId(object? value)
    {
        var quantity = NormalizeRpcQuantity(value, "eth_chainId");
        return Convert.ToUInt64(quantity[2..], 16);
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

    private static IReadOnlyList<IReadOnlyDictionary<string, object?>> RequireDictionaryList(
        object? value,
        string label)
    {
        if (value is IReadOnlyList<IReadOnlyDictionary<string, object?>> dictionaries)
        {
            return dictionaries;
        }

        if (value is System.Collections.IEnumerable enumerable && value is not string)
        {
            var output = new List<IReadOnlyDictionary<string, object?>>();
            var index = 0;
            foreach (var item in enumerable)
            {
                if (item is not IReadOnlyDictionary<string, object?> dictionary)
                {
                    throw new ArgumentException($"{label}[{index}] must be an object.", nameof(value));
                }

                output.Add(dictionary);
                index++;
            }

            return output;
        }

        throw new ArgumentException($"{label} must return an array.", nameof(value));
    }

    private static byte? EvmReceiptType(IReadOnlyDictionary<string, object?> receipt)
    {
        if (!receipt.TryGetValue("type", out var typeInput) || typeInput is null)
        {
            return null;
        }

        var receiptType = RequireEthereumRpcQuantity(typeInput, "receipt.type");
        if (receiptType == 0)
        {
            return null;
        }

        if (receiptType > 0x7f)
        {
            throw new ArgumentException(
                "typed receipt type must fit one byte below 0x80.",
                nameof(receipt));
        }

        if (receiptType < 1 || receiptType > 4)
        {
            throw new ArgumentException(
                "typed receipt type is not supported for Ethereum mainnet receipt proofs.",
                nameof(receipt));
        }

        return (byte)receiptType;
    }

    private static IReadOnlyList<byte[]> EvmReceiptLogsForRlp(
        IReadOnlyDictionary<string, object?> receipt)
    {
        var logs = RequireList(FirstPresent(receipt, "logs"), "receipt.logs");
        var encodedLogs = new byte[logs.Count][];
        for (var index = 0; index < logs.Count; index++)
        {
            if (logs[index] is not IReadOnlyDictionary<string, object?> log)
            {
                throw new ArgumentException($"receipt.logs[{index}] must be an object.", nameof(receipt));
            }

            if (FirstPresent(log, "removed") is true)
            {
                throw new ArgumentException($"receipt.logs[{index}] must not be removed.", nameof(receipt));
            }

            var topics = RequireList(FirstPresent(log, "topics"), $"receipt.logs[{index}].topics");
            if (topics.Count > 4)
            {
                throw new ArgumentException(
                    $"receipt.logs[{index}].topics must contain at most 4 entries.",
                    nameof(receipt));
            }

            var topicFields = topics
                .Select((topic, topicIndex) => RlpBytes(EthereumRpcHexBytes(
                    topic,
                    $"receipt.logs[{index}].topics[{topicIndex}]",
                    byteLength: 32,
                    nonZero: true,
                    allowEmpty: false)))
                .ToArray();
            encodedLogs[index] = RlpList([
                RlpBytes(EthereumRpcHexBytes(
                    FirstPresent(log, "address"),
                    $"receipt.logs[{index}].address",
                    byteLength: 20,
                    nonZero: true,
                    allowEmpty: false)),
                RlpList(topicFields),
                RlpBytes(EthereumRpcHexBytes(
                    FirstPresent(log, "data"),
                    $"receipt.logs[{index}].data",
                    byteLength: null,
                    nonZero: false,
                    allowEmpty: true)),
            ]);
        }

        return encodedLogs;
    }

    private static EvmTrieNode BuildEvmTrieNode(IReadOnlyList<EvmTrieItem> items)
    {
        if (items.Count == 0)
        {
            throw new ArgumentException("cannot build an empty trie node.", nameof(items));
        }

        if (items.Count == 1)
        {
            return new EvmTrieLeaf(items[0].Path, items[0].Value);
        }

        var prefix = LongestCommonNibblePrefix(items.Select(static item => item.Path).ToArray());
        if (prefix.Count > 0)
        {
            var stripped = items
                .Select(item => new EvmTrieItem(item.Path.Skip(prefix.Count).ToArray(), item.Value))
                .ToArray();
            return new EvmTrieExtension(prefix, BuildEvmTrieNode(stripped));
        }

        var grouped = Enumerable.Range(0, 16)
            .Select(static _ => new List<EvmTrieItem>())
            .ToArray();
        var branchValue = Array.Empty<byte>();
        foreach (var item in items)
        {
            if (item.Path.Count == 0)
            {
                branchValue = item.Value;
            }
            else
            {
                grouped[item.Path[0]].Add(new EvmTrieItem(item.Path.Skip(1).ToArray(), item.Value));
            }
        }

        var children = grouped
            .Select(group => group.Count == 0 ? null : BuildEvmTrieNode(group))
            .ToArray();
        return new EvmTrieBranch(children, branchValue);
    }

    private static byte[] EncodeEvmTrieNode(EvmTrieNode node)
    {
        if (node.Rlp is not null)
        {
            return node.Rlp;
        }

        byte[] encoded = node switch
        {
            EvmTrieLeaf leaf => RlpList([
                RlpBytes(EncodeEvmTrieCompactPath(leaf.Path, leaf: true)),
                RlpBytes(leaf.Value),
            ]),
            EvmTrieExtension extension => RlpList([
                RlpBytes(EncodeEvmTrieCompactPath(extension.Path, leaf: false)),
                RlpBytes(EvmTrieNodeReference(extension.Child)),
            ]),
            EvmTrieBranch branch => RlpList(branch.Children
                .Select(child => RlpBytes(child is null ? Array.Empty<byte>() : EvmTrieNodeReference(child)))
                .Append(RlpBytes(branch.Value))
                .ToArray()),
            _ => throw new ArgumentException("unknown trie node kind.", nameof(node)),
        };
        node.Rlp = encoded;
        return encoded;
    }

    private static byte[] EvmTrieNodeReference(EvmTrieNode node)
    {
        var rlp = EncodeEvmTrieNode(node);
        return rlp.Length < 32 ? rlp : Keccak256(rlp);
    }

    private static IReadOnlyList<byte[]> CollectEvmTrieProofNodes(
        EvmTrieNode node,
        IReadOnlyList<int> path)
    {
        var proof = new List<byte[]> { EncodeEvmTrieNode(node) };
        switch (node)
        {
            case EvmTrieLeaf leaf:
                if (!leaf.Path.SequenceEqual(path))
                {
                    throw new ArgumentException(
                        "receipt trie proof path does not end at requested receipt.",
                        nameof(path));
                }
                break;
            case EvmTrieExtension extension:
                if (path.Count < extension.Path.Count
                    || !path.Take(extension.Path.Count).SequenceEqual(extension.Path))
                {
                    throw new ArgumentException(
                        "receipt trie proof path does not match extension.",
                        nameof(path));
                }
                proof.AddRange(CollectEvmTrieProofNodes(
                    extension.Child,
                    path.Skip(extension.Path.Count).ToArray()));
                break;
            case EvmTrieBranch branch:
                if (path.Count == 0)
                {
                    if (branch.Value.Length == 0)
                    {
                        throw new ArgumentException(
                            "receipt trie branch has no value for requested receipt.",
                            nameof(path));
                    }
                }
                else
                {
                    var child = branch.Children[path[0]];
                    if (child is null)
                    {
                        throw new ArgumentException(
                            "receipt trie proof path is missing child.",
                            nameof(path));
                    }
                    proof.AddRange(CollectEvmTrieProofNodes(child, path.Skip(1).ToArray()));
                }
                break;
            default:
                throw new ArgumentException("unknown trie node kind.", nameof(node));
        }

        return proof;
    }

    private static byte[] EncodeEvmTrieCompactPath(IReadOnlyList<int> nibbles, bool leaf)
    {
        foreach (var nibble in nibbles)
        {
            if (nibble is < 0 or > 15)
            {
                throw new ArgumentException("trie path nibble out of range.", nameof(nibbles));
            }
        }

        var flags = leaf ? 2 : 0;
        var output = new List<byte>(1 + ((nibbles.Count + 1) / 2));
        var start = 0;
        if (nibbles.Count % 2 == 1)
        {
            output.Add((byte)(((flags + 1) << 4) | nibbles[0]));
            start = 1;
        }
        else
        {
            output.Add((byte)(flags << 4));
        }

        for (var index = start; index < nibbles.Count; index += 2)
        {
            output.Add((byte)((nibbles[index] << 4) | nibbles[index + 1]));
        }

        return output.ToArray();
    }

    private static int[] BytesToNibbles(byte[] bytes)
    {
        var nibbles = new int[bytes.Length * 2];
        for (var index = 0; index < bytes.Length; index++)
        {
            nibbles[index * 2] = (bytes[index] >> 4) & 0x0f;
            nibbles[index * 2 + 1] = bytes[index] & 0x0f;
        }

        return nibbles;
    }

    private static IReadOnlyList<int> LongestCommonNibblePrefix(IReadOnlyList<int>[] paths)
    {
        if (paths.Length == 0)
        {
            return Array.Empty<int>();
        }

        var prefix = paths[0].ToList();
        foreach (var path in paths.Skip(1))
        {
            var index = 0;
            var limit = Math.Min(prefix.Count, path.Count);
            while (index < limit && prefix[index] == path[index])
            {
                index++;
            }

            if (index < prefix.Count)
            {
                prefix.RemoveRange(index, prefix.Count - index);
            }

            if (prefix.Count == 0)
            {
                break;
            }
        }

        return prefix.ToArray();
    }

    private static ulong RequireEthereumRpcQuantity(object? value, string label)
    {
        if (value is not string text
            || !string.Equals(text.Trim(), text, StringComparison.Ordinal)
            || !text.StartsWith("0x", StringComparison.Ordinal))
        {
            throw new ArgumentException($"{label} must be a canonical JSON-RPC quantity.", label);
        }

        var hex = text[2..];
        if (!IsCanonicalRpcQuantityHex(hex))
        {
            throw new ArgumentException($"{label} must be a canonical JSON-RPC quantity.", label);
        }

        return Convert.ToUInt64(hex, 16);
    }

    private static byte[] EthereumRpcHexBytes(
        object? value,
        string label,
        int? byteLength,
        bool nonZero,
        bool allowEmpty)
    {
        if (value is not string text
            || !string.Equals(text.Trim(), text, StringComparison.Ordinal)
            || !text.StartsWith("0x", StringComparison.Ordinal))
        {
            throw new ArgumentException($"{label} must be canonical lowercase 0x hex.", label);
        }

        var hex = text[2..];
        if ((!allowEmpty && hex.Length == 0)
            || hex.Length % 2 != 0
            || !hex.All(IsLowerHex))
        {
            throw new ArgumentException($"{label} must be canonical lowercase 0x hex.", label);
        }

        if (byteLength is not null && hex.Length != byteLength.Value * 2)
        {
            throw new ArgumentException($"{label} must be {byteLength.Value} bytes.", label);
        }

        var bytes = Convert.FromHexString(hex);
        if (nonZero && !bytes.Any(static value => value != 0))
        {
            throw new ArgumentException($"{label} must not be zero.", label);
        }

        return bytes;
    }

    private static byte[] MinimalBigEndianBytes(ulong value)
    {
        if (value == 0)
        {
            return [];
        }

        Span<byte> buffer = stackalloc byte[8];
        BinaryPrimitives.WriteUInt64BigEndian(buffer, value);
        var start = 0;
        while (start < buffer.Length && buffer[start] == 0)
        {
            start++;
        }

        return buffer[start..].ToArray();
    }

    private static byte[] RlpBytes(byte[] value)
    {
        if (value.Length == 1 && value[0] < 0x80)
        {
            return value.ToArray();
        }

        return Concat(RlpLengthPrefix(value.Length, 0x80, 0xb7), value);
    }

    private static byte[] RlpList(IEnumerable<byte[]> fields)
    {
        var payload = Concat(fields.ToArray());
        return Concat(RlpLengthPrefix(payload.Length, 0xc0, 0xf7), payload);
    }

    private static byte[] RlpLengthPrefix(int length, int shortOffset, int longOffset)
    {
        if (length < 56)
        {
            return new[] { (byte)(shortOffset + length) };
        }

        var remaining = length;
        var lengthBytes = new List<byte>();
        while (remaining > 0)
        {
            lengthBytes.Insert(0, (byte)(remaining & 0xff));
            remaining >>= 8;
        }

        return Concat(new[] { (byte)(longOffset + lengthBytes.Count) }, lengthBytes.ToArray());
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
        string? sourceBridgeEmitterAddressInput,
        string? transactionHash,
        string? blockHash,
        string? blockNumber)
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

    private static string? ResolveSourceBridgeEmitterAddress(
        string? inputAddress,
        string? defaultAddress)
    {
        var normalizedInput = inputAddress is null
            ? null
            : NormalizeRpcHex(
                inputAddress,
                nameof(EthereumMainnetInboundEvidence.SourceBridgeEmitterAddress),
                20);
        var normalizedDefault = defaultAddress is null
            ? null
            : NormalizeRpcHex(
                defaultAddress,
                nameof(EthereumMainnetInboundEvidence.SourceBridgeEmitterAddress),
                20);
        if (normalizedInput is not null
            && normalizedDefault is not null
            && !string.Equals(normalizedInput, normalizedDefault, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "sourceBridgeEmitterAddress values must match.",
                nameof(defaultAddress));
        }

        return normalizedInput ?? normalizedDefault;
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

        if (beaconFinality is not null)
        {
            var finalityFinalizedRootInput = FirstPresent(
                beaconFinality,
                "finalizedHeaderRoot",
                "finalized_header_root",
                "beaconFinalizedRoot",
                "beacon_finalized_root");
            if (finalityFinalizedRootInput is not null)
            {
                var finalityFinalizedRoot = NormalizeRpcHex(
                    finalityFinalizedRootInput,
                    "beaconFinality.finalizedHeaderRoot",
                    32);
                var proofFinalizedRoot = NormalizeRpcHex(
                    receiptProof.BeaconFinalizedRoot,
                    "receiptProof.beaconFinalizedRoot",
                    32);
                if (!string.Equals(proofFinalizedRoot, finalityFinalizedRoot, StringComparison.Ordinal))
                {
                    throw new ArgumentException(
                        "receiptProof.beaconFinalizedRoot must match beaconFinality.finalizedHeaderRoot.",
                        nameof(receiptProof));
                }
            }

            var finalitySyncCommitteeRootInput = FirstPresent(
                beaconFinality,
                "syncCommitteeRoot",
                "sync_committee_root");
            if (finalitySyncCommitteeRootInput is not null)
            {
                var finalitySyncCommitteeRoot = NormalizeRpcHex(
                    finalitySyncCommitteeRootInput,
                    "beaconFinality.syncCommitteeRoot",
                    32);
                var proofSyncCommitteeRoot = NormalizeRpcHex(
                    receiptProof.SyncCommitteeRoot,
                    "receiptProof.syncCommitteeRoot",
                    32);
                if (!string.Equals(proofSyncCommitteeRoot, finalitySyncCommitteeRoot, StringComparison.Ordinal))
                {
                    throw new ArgumentException(
                        "receiptProof.syncCommitteeRoot must match beaconFinality.syncCommitteeRoot.",
                        nameof(receiptProof));
                }
            }

            var finalityBeaconSlotInput = FirstPresent(
                beaconFinality,
                "beaconSlot",
                "beacon_slot",
                "finalizedSlot",
                "finalized_slot",
                "slot");
            if (finalityBeaconSlotInput is not null)
            {
                var finalityBeaconSlot = NormalizeUnsignedInteger(
                    finalityBeaconSlotInput,
                    "beaconFinality.beaconSlot");
                if (receiptProof.BeaconSlot != finalityBeaconSlot)
                {
                    throw new ArgumentException(
                        "receiptProof.beaconSlot must match beaconFinality.beaconSlot.",
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
        var finalizedHeaderRootInput = FirstPresent(
            finality,
            "finalizedHeaderRoot",
            "finalized_header_root",
            "beaconFinalizedRoot",
            "beacon_finalized_root");
        if (finalizedHeaderRootInput is not null)
        {
            normalized["finalizedHeaderRoot"] = NormalizeRpcHex(
                finalizedHeaderRootInput,
                "beaconFinality.finalizedHeaderRoot",
                32);
        }

        var syncCommitteeRootInput = FirstPresent(
            finality,
            "syncCommitteeRoot",
            "sync_committee_root");
        if (syncCommitteeRootInput is not null)
        {
            normalized["syncCommitteeRoot"] = NormalizeRpcHex(
                syncCommitteeRootInput,
                "beaconFinality.syncCommitteeRoot",
                32);
        }
        var beaconSlotInput = FirstPresent(
            finality,
            "beaconSlot",
            "beacon_slot",
            "finalizedSlot",
            "finalized_slot",
            "slot");
        if (beaconSlotInput is not null)
        {
            var beaconSlot = NormalizeUnsignedInteger(
                beaconSlotInput,
                "beaconFinality.beaconSlot");
            if (beaconSlot == 0)
            {
                throw new ArgumentException(
                    "beaconFinality.beaconSlot must be positive.",
                    nameof(finality));
            }

            normalized["beaconSlot"] = beaconSlot.ToString(System.Globalization.CultureInfo.InvariantCulture);
        }
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

public sealed record EthereumMainnetSourceVerifierMaterialInput(
    string SourceTrustAnchorHash,
    string ConsensusVerifierHash,
    string MessageInclusionVerifierHash,
    string FinalityPolicyHash,
    string BridgeAddress,
    string SourceBridgeEmitterCodeHash,
    string? NetworkId = null,
    string? SourceBridgeConfigHash = null,
    int SourceDomain = EthereumMainnetSccp.DomainEthereum,
    int TargetDomain = EthereumMainnetSccp.DomainSora);

public sealed record EthereumMainnetSourceAdapterDeploymentInput(
    string SourceTrustAnchorHash,
    string ConsensusVerifierHash,
    string MessageInclusionVerifierHash,
    string FinalityPolicyHash,
    string BridgeAddress,
    string SourceBridgeEmitterCodeHash,
    string DeploymentReceiptHash,
    string? NetworkId = null,
    string? SourceBridgeConfigHash = null,
    string? AdapterVerifierVkHash = null,
    int SourceDomain = EthereumMainnetSccp.DomainEthereum,
    int TargetDomain = EthereumMainnetSccp.DomainSora);

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

public sealed record EthereumMainnetBeaconRestResponse(
    int StatusCode,
    byte[] Body,
    string? StatusMessage = null)
{
    public byte[] Body { get; init; } = Body.ToArray();
}

public interface IEthereumMainnetBeaconRestTransport
{
    ValueTask<EthereumMainnetBeaconRestResponse> GetAsync(
        string url,
        IReadOnlyDictionary<string, string> headers,
        CancellationToken cancellationToken = default);
}

public sealed class EthereumMainnetBeaconRestHttpTransport(HttpClient? httpClient = null)
    : IEthereumMainnetBeaconRestTransport
{
    private readonly HttpClient httpClient = httpClient ?? new HttpClient();

    public async ValueTask<EthereumMainnetBeaconRestResponse> GetAsync(
        string url,
        IReadOnlyDictionary<string, string> headers,
        CancellationToken cancellationToken = default)
    {
        using var request = new HttpRequestMessage(HttpMethod.Get, url);
        foreach (var (name, value) in headers)
        {
            request.Headers.TryAddWithoutValidation(name, value);
        }
        using var response = await httpClient.SendAsync(request, cancellationToken).ConfigureAwait(false);
        var body = await response.Content.ReadAsByteArrayAsync(cancellationToken).ConfigureAwait(false);
        return new EthereumMainnetBeaconRestResponse(
            (int)response.StatusCode,
            body,
            response.ReasonPhrase);
    }
}

public sealed class EthereumMainnetBeaconRestConsensusProvider : IEthereumMainnetConsensusProvider
{
    private readonly Uri endpoint;
    private readonly string syncCommitteeRoot;
    private readonly IReadOnlyDictionary<string, string> headers;
    private readonly bool verifyFinalityCheckpoint;
    private readonly IEthereumMainnetBeaconRestTransport transport;

    public EthereumMainnetBeaconRestConsensusProvider(
        string endpoint,
        string syncCommitteeRoot,
        IReadOnlyDictionary<string, string>? headers = null,
        bool verifyFinalityCheckpoint = true,
        IEthereumMainnetBeaconRestTransport? transport = null)
        : this(endpoint, syncCommitteeRoot, null, headers, verifyFinalityCheckpoint, transport)
    {
    }

    public EthereumMainnetBeaconRestConsensusProvider(
        string endpoint,
        string? syncCommitteeRoot,
        byte[]? syncCommitteePayload,
        IReadOnlyDictionary<string, string>? headers = null,
        bool verifyFinalityCheckpoint = true,
        IEthereumMainnetBeaconRestTransport? transport = null)
    {
        this.endpoint = NormalizeBeaconRestEndpoint(endpoint);
        this.syncCommitteeRoot = ResolveSyncCommitteeRoot(syncCommitteeRoot, syncCommitteePayload);
        this.headers = new Dictionary<string, string>(headers ?? new Dictionary<string, string>(), StringComparer.Ordinal);
        this.verifyFinalityCheckpoint = verifyFinalityCheckpoint;
        this.transport = transport ?? new EthereumMainnetBeaconRestHttpTransport();
    }

    public async ValueTask<IReadOnlyDictionary<string, object?>?> CollectFinalityEvidenceAsync(
        IReadOnlyDictionary<string, object?>? receipt,
        IReadOnlyDictionary<string, object?>? block,
        string? transactionHash,
        CancellationToken cancellationToken = default)
    {
        if (block is null)
        {
            throw new ArgumentException(
                "Ethereum mainnet Beacon REST finality collection requires block",
                nameof(block));
        }
        var blockHash = NormalizeRpcHex(RequiredBlockValue(block, "hash"), "block.hash", 32);
        var blockNumber = NormalizeRpcQuantity(
            FirstPresent(block, "number", "blockNumber", "block_number"),
            "block.number");
        if (blockNumber == "0x0")
        {
            throw new ArgumentException("block.number must be positive", nameof(block));
        }
        var receiptsRoot = NormalizeRpcHex(
            FirstPresent(block, "receiptsRoot", "receipts_root"),
            "block.receiptsRoot",
            32);

        using var headerDocument = await FetchJsonDocumentAsync(
            "/eth/v1/beacon/headers/finalized",
            "Ethereum mainnet Beacon REST finalized header",
            cancellationToken).ConfigureAwait(false);
        var headerRoot = headerDocument.RootElement;
        RejectUnsafeBeaconRestPayload(headerRoot, "Ethereum mainnet Beacon REST finalized header");
        var headerData = RequireObject(
            RequireProperty(headerRoot, "Ethereum mainnet Beacon REST finalized header", "data"),
            "Ethereum mainnet Beacon REST finalized header.data");
        if (headerData.TryGetProperty("canonical", out var canonical)
            && canonical.ValueKind == JsonValueKind.False)
        {
            throw new ArgumentException(
                "Ethereum mainnet Beacon REST finalized header must be canonical");
        }
        var finalizedHeaderRoot = NormalizeRpcHex(
            RequireString(
                RequireProperty(headerData, "Ethereum mainnet Beacon REST finalized header.data", "root"),
                "finalizedHeaderRoot"),
            "finalizedHeaderRoot",
            32);
        var header = RequireObject(
            RequireProperty(headerData, "Ethereum mainnet Beacon REST finalized header.data", "header"),
            "Ethereum mainnet Beacon REST finalized header.data.header");
        var message = RequireObject(
            RequireProperty(header, "Ethereum mainnet Beacon REST finalized header.data.header", "message"),
            "Ethereum mainnet Beacon REST finalized header.data.header.message");
        var beaconSlot = NormalizeUnsignedInteger(
            RequireString(
                RequireProperty(
                    message,
                    "Ethereum mainnet Beacon REST finalized header.data.header.message",
                    "slot"),
                "beaconFinality.beaconSlot"),
            "beaconFinality.beaconSlot");
        if (beaconSlot == 0)
        {
            throw new ArgumentException("beaconFinality.beaconSlot must be positive");
        }

        if (verifyFinalityCheckpoint)
        {
            using var checkpointDocument = await FetchJsonDocumentAsync(
                "/eth/v1/beacon/states/finalized/finality_checkpoints",
                "Ethereum mainnet Beacon REST finality checkpoints",
                cancellationToken).ConfigureAwait(false);
            var checkpointRoot = checkpointDocument.RootElement;
            RejectUnsafeBeaconRestPayload(
                checkpointRoot,
                "Ethereum mainnet Beacon REST finality checkpoints");
            var checkpointData = RequireObject(
                RequireProperty(
                    checkpointRoot,
                    "Ethereum mainnet Beacon REST finality checkpoints",
                    "data"),
                "Ethereum mainnet Beacon REST finality checkpoints.data");
            var finalizedCheckpoint = RequireObject(
                RequireProperty(
                    checkpointData,
                    "Ethereum mainnet Beacon REST finality checkpoints.data",
                    "finalized"),
                "Ethereum mainnet Beacon REST finality checkpoints.data.finalized");
            var finalizedCheckpointRoot = NormalizeRpcHex(
                RequireString(
                    RequireProperty(
                        finalizedCheckpoint,
                        "Ethereum mainnet Beacon REST finality checkpoints.data.finalized",
                        "root"),
                    "finalizedCheckpointRoot"),
                "finalizedCheckpointRoot",
                32);
            if (!string.Equals(finalizedCheckpointRoot, finalizedHeaderRoot, StringComparison.Ordinal))
            {
                throw new ArgumentException(
                    "Ethereum mainnet Beacon REST finality checkpoint root must match finalized header root");
            }
        }

        return new Dictionary<string, object?>(StringComparer.Ordinal)
        {
            ["executionBlockNumber"] = NormalizeUnsignedInteger(blockNumber, "block.number").ToString(),
            ["executionBlockHash"] = blockHash,
            ["executionReceiptsRoot"] = receiptsRoot,
            ["finalizedHeaderRoot"] = finalizedHeaderRoot,
            ["syncCommitteeRoot"] = syncCommitteeRoot,
            ["beaconSlot"] = beaconSlot.ToString(),
        };
    }

    private async ValueTask<JsonDocument> FetchJsonDocumentAsync(
        string path,
        string label,
        CancellationToken cancellationToken)
    {
        var response = await transport.GetAsync(
            BeaconRestUrl(endpoint, path),
            headers,
            cancellationToken).ConfigureAwait(false);
        if (response.StatusCode is < 200 or > 299)
        {
            var suffix = string.IsNullOrEmpty(response.StatusMessage) ? string.Empty : $" {response.StatusMessage}";
            throw new ArgumentException($"{label} request failed {response.StatusCode}{suffix}");
        }
        try
        {
            return JsonDocument.Parse(response.Body);
        }
        catch (JsonException ex)
        {
            throw new ArgumentException($"{label} response JSON must be an object", ex);
        }
    }

    private static Uri NormalizeBeaconRestEndpoint(string endpoint)
    {
        if (string.IsNullOrEmpty(endpoint) || endpoint.Trim() != endpoint)
        {
            throw new ArgumentException(
                "Ethereum mainnet Beacon REST endpoint must be a non-empty URL",
                nameof(endpoint));
        }
        if (!Uri.TryCreate(endpoint, UriKind.Absolute, out var uri)
            || (uri.Scheme != Uri.UriSchemeHttp && uri.Scheme != Uri.UriSchemeHttps))
        {
            throw new ArgumentException(
                "Ethereum mainnet Beacon REST endpoint must use http or https",
                nameof(endpoint));
        }
        return new UriBuilder(uri) { Fragment = string.Empty }.Uri;
    }

    private static string ResolveSyncCommitteeRoot(string? syncCommitteeRoot, byte[]? syncCommitteePayload)
    {
        string? payloadRoot = null;
        if (syncCommitteePayload is not null)
        {
            payloadRoot = EthereumMainnetSccp.EthSyncCommitteeHashFromPayload(syncCommitteePayload.ToArray());
        }

        if (syncCommitteeRoot is not null)
        {
            var normalizedRoot = NormalizeRpcHex(syncCommitteeRoot, nameof(syncCommitteeRoot), 32);
            if (payloadRoot is not null
                && !string.Equals(normalizedRoot, payloadRoot, StringComparison.Ordinal))
            {
                throw new ArgumentException(
                    "syncCommitteeRoot must match syncCommitteePayload",
                    nameof(syncCommitteePayload));
            }
            return normalizedRoot;
        }

        if (payloadRoot is not null)
        {
            return payloadRoot;
        }

        throw new ArgumentException(
            "Ethereum mainnet Beacon REST provider requires syncCommitteeRoot or syncCommitteePayload",
            nameof(syncCommitteeRoot));
    }

    private static string BeaconRestUrl(Uri endpoint, string path)
    {
        var builder = new UriBuilder(endpoint) { Fragment = string.Empty };
        var basePath = builder.Path.TrimEnd('/');
        var apiPath = basePath.EndsWith("/eth/v1", StringComparison.Ordinal) && path.StartsWith("/eth/v1/", StringComparison.Ordinal)
            ? path["/eth/v1".Length..]
            : path;
        builder.Path = basePath + apiPath;
        return builder.Uri.ToString();
    }

    private static JsonElement RequireProperty(JsonElement value, string label, string property)
    {
        if (!value.TryGetProperty(property, out var propertyValue))
        {
            throw new ArgumentException($"{label}.{property} is required");
        }
        return propertyValue;
    }

    private static JsonElement RequireObject(JsonElement value, string label)
    {
        if (value.ValueKind != JsonValueKind.Object)
        {
            throw new ArgumentException($"{label} must be an object");
        }
        return value;
    }

    private static string RequireString(JsonElement value, string label)
    {
        if (value.ValueKind != JsonValueKind.String)
        {
            throw new ArgumentException($"{label} must be a string");
        }
        return value.GetString()!;
    }

    private static void RejectUnsafeBeaconRestPayload(JsonElement payload, string label)
    {
        if ((payload.TryGetProperty("execution_optimistic", out var optimistic)
                && optimistic.ValueKind == JsonValueKind.True)
            || (payload.TryGetProperty("executionOptimistic", out var optimisticAlias)
                && optimisticAlias.ValueKind == JsonValueKind.True))
        {
            throw new ArgumentException($"{label} must not be execution optimistic");
        }
        if (payload.TryGetProperty("finalized", out var finalized)
            && finalized.ValueKind == JsonValueKind.False)
        {
            throw new ArgumentException($"{label} must be finalized");
        }
    }

    private static object RequiredBlockValue(IReadOnlyDictionary<string, object?> block, string key)
    {
        if (!block.TryGetValue(key, out var value) || value is null)
        {
            throw new ArgumentException($"block.{key} is required", nameof(block));
        }
        return value;
    }

    private static object FirstPresent(IReadOnlyDictionary<string, object?> value, params string[] names)
    {
        foreach (var name in names)
        {
            if (value.TryGetValue(name, out var item) && item is not null)
            {
                return item;
            }
        }
        throw new ArgumentException($"{names[0]} is required");
    }

    private static string NormalizeRpcHex(object? value, string parameterName, int byteLength)
    {
        if (value is not string text || text.Trim() != text || !text.StartsWith("0x", StringComparison.Ordinal))
        {
            throw new ArgumentException($"{parameterName} must be canonical lowercase 0x hex", parameterName);
        }
        var hex = text[2..];
        if (hex.Length != byteLength * 2 || !IsLowerHex(hex))
        {
            throw new ArgumentException($"{parameterName} must be {byteLength} bytes canonical lowercase 0x hex", parameterName);
        }
        if (hex.All(static ch => ch == '0'))
        {
            throw new ArgumentException($"{parameterName} must not be zero", parameterName);
        }
        return text;
    }

    private static string NormalizeRpcQuantity(object? value, string parameterName)
    {
        if (value is not string text || text.Trim() != text || !text.StartsWith("0x", StringComparison.Ordinal))
        {
            throw new ArgumentException($"{parameterName} must be a canonical JSON-RPC quantity", parameterName);
        }
        var hex = text[2..];
        if (hex.Length == 0
            || !IsLowerHex(hex)
            || (hex.Length > 1 && hex[0] == '0'))
        {
            throw new ArgumentException($"{parameterName} must be a canonical JSON-RPC quantity", parameterName);
        }
        return "0x" + BigInteger.Parse("0" + hex, System.Globalization.NumberStyles.HexNumber).ToString("x");
    }

    private static bool IsLowerHex(string text)
        => text.All(static ch => (ch >= '0' && ch <= '9') || (ch >= 'a' && ch <= 'f'));

    private static ulong NormalizeUnsignedInteger(object? value, string parameterName)
        => value switch
        {
            string text when text.StartsWith("0x", StringComparison.Ordinal)
                => ulong.Parse(NormalizeRpcQuantity(text, parameterName)[2..], System.Globalization.NumberStyles.HexNumber),
            string text when text.Trim() == text && text.All(char.IsDigit)
                => ulong.Parse(text, System.Globalization.CultureInfo.InvariantCulture),
            ulong item => item,
            long item when item >= 0 => (ulong)item,
            int item when item >= 0 => (ulong)item,
            _ => throw new ArgumentException($"{parameterName} must be an unsigned integer", parameterName),
        };
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
    string ExecutionReceiptsRoot,
    string? BeaconSlot = null)
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
        if (BeaconSlot is not null)
        {
            value["beaconSlot"] = BeaconSlot;
        }
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

public sealed record EvmReceiptTrieProof(
    string ReceiptsRoot,
    string ReceiptRlp,
    string ReceiptTrieKey,
    IReadOnlyList<byte[]> ReceiptTrieProofNodes);

public sealed record EthereumMainnetInboundEvidence
{
    public int SourceDomain { get; init; } = EthereumMainnetSccp.DomainEthereum;

    public int TargetDomain { get; init; } = EthereumMainnetSccp.DomainSora;

    public string? TransactionHash { get; init; }

    public IReadOnlyDictionary<string, object?>? Receipt { get; init; }

    public IReadOnlyDictionary<string, object?>? Block { get; init; }

    public IReadOnlyDictionary<string, object?>? BeaconFinality { get; init; }

    public IReadOnlyList<IReadOnlyDictionary<string, object?>>? BlockReceipts { get; init; }

    public IReadOnlyList<byte[]>? InclusionBranch { get; init; }

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
