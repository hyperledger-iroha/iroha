using System.Buffers.Binary;
using System.Globalization;
using System.IO;
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
    public const string NativeEvmProverBundleSchemaV1 =
        "sccp-native-evm-groth16-prover-bundle-v1";
    public const string EthNativeEvmProverParityFixtureSchemaV1 =
        "sccp-ethereum-mainnet-native-evm-cross-sdk-fixture-parity-v1";
    public const string EthNativeEvmProverSelfTestSchemaV1 =
        "sccp-ethereum-mainnet-native-evm-prover-self-test-v1";
    public const string EthNativeEvmProverBundleIdV1 =
        "sccp:eth:native-evm-groth16-prover:ethereum-mainnet:v1";
    public const string NativeEvmProverArtifactHashAlgorithmV1 = "sha256";
    internal const int NativeEvmProverMinArtifactBytesV1 = 256;
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

    public static readonly IReadOnlyDictionary<string, string>
        EthNativeEvmProverRequiredImplementationsV1 =
            new Dictionary<string, string>(StringComparer.Ordinal)
            {
                ["javascript"] = "pure-typescript",
                ["swift"] = "native-swift",
                ["kotlin"] = "native-kotlin",
                ["java-android"] = "native-java",
                ["dotnet"] = "native-csharp",
            };

    public static readonly IReadOnlyList<string> EthNativeEvmProverRequiredAuditHashesV1 =
        new[]
        {
            "circuit_security_audit",
            "native_implementation_audit",
            "reproducible_build_attestation",
            "cross_sdk_fixture_parity",
            "native_prover_self_test",
            "no_wasm_no_remote_scan",
        };

    internal static string NormalizeNativeEvmProverBundleHex32(string value, string name)
    {
        var normalized = NormalizeNonZeroHex(value, name, 32);
        if (!string.Equals(value, normalized, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                $"{name} must be canonical lowercase 0x-prefixed 32-byte hex.",
                name);
        }

        return normalized;
    }

    internal static string NormalizeNativeEvmProverArtifactPath(string value, string name)
    {
        if (string.IsNullOrEmpty(value))
        {
            throw new ArgumentException($"{name} must be a non-empty relative POSIX path.", name);
        }

        if (value.Any(character => character < 0x20 || character == 0x7f))
        {
            throw new ArgumentException($"{name} must not contain control characters.", name);
        }

        if (value.StartsWith("/", StringComparison.Ordinal) || value.Contains('\\'))
        {
            throw new ArgumentException($"{name} must be a relative POSIX path.", name);
        }

        var segments = value.Split('/');
        if (segments.Length == 0 || segments.Any(segment => segment.Length == 0 || segment == "." || segment == ".."))
        {
            throw new ArgumentException($"{name} must stay under the manifest directory.", name);
        }

        return value;
    }

    internal static string NormalizeNativeEvmProverParityHex32(string value, string name)
    {
        var normalized = ToHex(FixedHexToBytes(value, name, 32));
        if (!string.Equals(value, normalized, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                $"{name} must be canonical lowercase 0x-prefixed 32-byte hex.",
                name);
        }

        return normalized;
    }

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
    private const int EthMainnetSyncCommitteeAuthorities = 512;
    private const int EthMaxSyncCommitteeAuthorities = EthMainnetSyncCommitteeAuthorities;
    private const int EthSyncCommitteePublicKeyBytes = 48;
    private const int EthSyncCommitteePopBytes = 96;
    private const int EthMaxSyncCommitteePayloadBytes = 1 + 4
        + EthMaxSyncCommitteeAuthorities
        * (4 + EthSyncCommitteePublicKeyBytes + 8 + 4 + EthSyncCommitteePopBytes);
    private const int Keccak256Rate = 136;
    private const int MaxSourceMerkleBranchNodes = 64;
    private const int MaxMptProofNodes = 64;
    private const int MaxMptNodeBytes = 16 * 1024;
    private const int EvmMaxBlockReceipts = 4096;
    private const int EthExecutionPayloadBodyFieldIndex = 9;
    private const int EthExecutionPayloadBodyBranchDepth = 4;
    private const ulong SourceAdapterFastPqTraceRoot = 0x002A_247F_81C6_F850UL;
    private const ulong SourceAdapterFastPqLdeRoot = 0x6026_3388_DBBF_9B2AUL;
    private const ulong SourceAdapterFastPqOmegaCoset = 0x6AF3_25E8_25AD_5C18UL;
    private const string SourceChain = "eth";
    private const byte SourceProofPlan = 1;
    private const byte SourceFinalityModel = 1;
    private sealed record Groth16ProverArtifacts(string ProofArtifactHash, string ProvingKeyHash);
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
    private static readonly string[] BeaconFinalityAliasKeys =
    [
        "executionBlockNumber",
        "execution_block_number",
        "finalityHeight",
        "finality_height",
        "executionBlockHash",
        "execution_block_hash",
        "finalityBlockHash",
        "finality_block_hash",
        "executionReceiptsRoot",
        "execution_receipts_root",
        "receiptsRoot",
        "receipts_root",
        "finalizedHeaderRoot",
        "finalized_header_root",
        "beaconFinalizedRoot",
        "beacon_finalized_root",
        "syncCommitteeRoot",
        "sync_committee_root",
        "beaconSlot",
        "beacon_slot",
        "finalizedSlot",
        "finalized_slot",
        "slot",
        "finalityBranch",
        "finality_branch",
        "syncCommitteeBits",
        "sync_committee_bits",
        "syncCommitteeSignature",
        "sync_committee_signature",
        "syncSignatureSlot",
        "sync_signature_slot",
        "signatureSlot",
        "signature_slot",
        "syncCommitteeParticipation",
        "sync_committee_participation",
    ];
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

    private readonly record struct RlpDecodedItem(
        int PayloadOffset,
        int PayloadLength,
        int NextOffset,
        bool IsList);

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
                StrictFirstPresent(
                    receipt,
                    "receipt.cumulativeGasUsed",
                    "cumulativeGasUsed",
                    "cumulative_gas_used"),
                "receipt.cumulativeGasUsed"))),
            RlpBytes(EthereumRpcHexBytes(
                StrictFirstPresent(receipt, "receipt.logsBloom", "logsBloom", "logs_bloom"),
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
        var seenTransactionHashes = new HashSet<string>(StringComparer.Ordinal);
        byte[]? targetReceiptRlp = null;
        for (var index = 0; index < receipts.Count; index++)
        {
            var receipt = receipts[index]
                ?? throw new ArgumentException($"blockReceipts[{index}] is required.", nameof(receipts));
            var receiptIndex = RequireEthereumRpcQuantity(
                StrictFirstPresent(
                    receipt,
                    $"blockReceipts[{index}].transactionIndex",
                    "transactionIndex",
                    "transaction_index"),
                $"blockReceipts[{index}].transactionIndex");
            if (receiptIndex != (ulong)index)
            {
                throw new ArgumentException(
                    "block receipt transactionIndex must match receipt order.",
                    nameof(receipts));
            }

            var transactionHash = NormalizeRpcHex(
                StrictFirstPresent(
                    receipt,
                    $"blockReceipts[{index}].transactionHash",
                    "transactionHash",
                    "transaction_hash"),
                $"blockReceipts[{index}].transactionHash",
                32);
            if (!seenTransactionHashes.Add(transactionHash))
            {
                throw new ArgumentException(
                    "block receipt transactionHash values must be unique.",
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

    /// <summary>
    /// Derives the SSZ ExecutionPayloadHeader root from a Deneb/Fulu Ethereum execution header RLP.
    /// </summary>
    public static string EthExecutionPayloadHeaderRootFromRlp(byte[] headerRlp)
    {
        ArgumentNullException.ThrowIfNull(headerRlp);

        var fields = RlpListByteFields(headerRlp);
        if (fields.Count < 19)
        {
            throw new ArgumentException(
                "headerRlp must include Deneb/Fulu execution payload fields.",
                nameof(headerRlp));
        }

        return ToHex(SszMerkleizeChunks(
        [
            SszByteVectorRoot(fields[0], 32, "parentHash"),
            SszByteVectorRoot(fields[2], 20, "feeRecipient"),
            SszByteVectorRoot(fields[3], 32, "stateRoot"),
            SszByteVectorRoot(fields[5], 32, "receiptsRoot"),
            SszByteVectorRoot(fields[6], 256, "logsBloom"),
            SszByteVectorRoot(fields[13], 32, "prevRandao"),
            SszU64ChunkFromRlp(fields[8], "blockNumber"),
            SszU64ChunkFromRlp(fields[9], "gasLimit"),
            SszU64ChunkFromRlp(fields[10], "gasUsed"),
            SszU64ChunkFromRlp(fields[11], "timestamp"),
            SszByteListRoot(fields[12], 32, "extraData"),
            SszU256ChunkFromRlp(fields[15], "baseFeePerGas"),
            Keccak256(headerRlp),
            SszByteVectorRoot(fields[4], 32, "transactionsRoot"),
            SszByteVectorRoot(fields[16], 32, "withdrawalsRoot"),
            SszU64ChunkFromRlp(fields[17], "blobGasUsed"),
            SszU64ChunkFromRlp(fields[18], "excessBlobGas"),
        ]));
    }

    /// <summary>
    /// Derives the BeaconBlockBody root from an execution-payload header root and body branch.
    /// </summary>
    public static string EthBeaconBodyRootFromExecutionPayloadBranch(
        string executionPayloadHeaderRoot,
        IReadOnlyList<byte[]> executionPayloadBranch)
    {
        ArgumentNullException.ThrowIfNull(executionPayloadBranch);
        if (executionPayloadBranch.Count != EthExecutionPayloadBodyBranchDepth)
        {
            throw new ArgumentException(
                $"executionPayloadBranch must contain {EthExecutionPayloadBodyBranchDepth} siblings.",
                nameof(executionPayloadBranch));
        }

        return ToHex(SszMerkleRootFromBranch(
            FixedHexToBytes(executionPayloadHeaderRoot, nameof(executionPayloadHeaderRoot), 32),
            EthExecutionPayloadBodyFieldIndex,
            executionPayloadBranch,
            "executionPayloadBranch"));
    }

    /// <summary>
    /// Derives the SSZ BeaconBlockHeader root from local Ethereum mainnet witness material.
    /// </summary>
    public static string EthBeaconBlockHeaderRoot(
        ulong beaconSlot,
        ulong beaconProposerIndex,
        string beaconParentRoot,
        string beaconStateRoot,
        string beaconBodyRoot)
        => ToHex(SszMerkleizeChunks(
        [
            SszU64Chunk(beaconSlot),
            SszU64Chunk(beaconProposerIndex),
            FixedHexToBytes(beaconParentRoot, nameof(beaconParentRoot), 32),
            FixedHexToBytes(beaconStateRoot, nameof(beaconStateRoot), 32),
            FixedHexToBytes(beaconBodyRoot, nameof(beaconBodyRoot), 32),
        ]));

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
                StrictFirstPresent(
                    receipt,
                    "receipt.transactionHash",
                    "transactionHash",
                    "transaction_hash"),
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
                StrictFirstPresent(receipt, "receipt.blockHash", "blockHash", "block_hash"),
                "receipt.blockHash",
                32);
            var receiptBlockNumberValue =
                StrictFirstPresent(receipt, "receipt.blockNumber", "blockNumber", "block_number");
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

            var blockNumberValue = StrictFirstPresent(
                block,
                "block.number",
                "number",
                "blockNumber",
                "block_number");
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
                StrictFirstPresent(block, "block.receiptsRoot", "receiptsRoot", "receipts_root"),
                "block.receiptsRoot",
                32);
        }

        receipt = SnapshotDictionaryOrNull(receipt);
        block = SnapshotDictionaryOrNull(block);

        var beaconFinality = input.BeaconFinality;
        if (beaconFinality is null && consensusProvider is not null)
        {
            beaconFinality = await consensusProvider.CollectFinalityEvidenceAsync(
                SnapshotDictionaryOrNull(receipt),
                SnapshotDictionaryOrNull(block),
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

            var receiptTransactionIndex = StrictFirstPresent(
                receipt,
                "receipt.transactionIndex",
                "transactionIndex",
                "transaction_index");
            var receiptTrieProof = BuildEvmReceiptTrieProofFromReceipts(
                blockReceipts,
                receiptTransactionIndex);
            var expectedReceiptsRoot = blockReceiptsRoot
                ?? (StrictFirstPresent(
                    beaconFinality,
                    "beaconFinality.executionReceiptsRoot",
                    "executionReceiptsRoot",
                    "execution_receipts_root") as string);
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
                StrictFirstPresent(
                    indexedReceipt,
                    "blockReceipts.transactionHash",
                    "transactionHash",
                    "transaction_hash"),
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
                StrictFirstPresent(indexedReceipt, "blockReceipts.blockHash", "blockHash", "block_hash"),
                "blockReceipts.blockHash",
                32);
            if (!string.Equals(indexedBlockHash, blockHash, StringComparison.Ordinal))
            {
                throw new ArgumentException(
                    "blockReceipts.blockHash must match receipt.",
                    nameof(input));
            }

            var indexedBlockNumber = NormalizePositiveRpcQuantity(
                StrictFirstPresent(
                    indexedReceipt,
                    "blockReceipts.blockNumber",
                    "blockNumber",
                    "block_number"),
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
                    StrictFirstPresent(
                        beaconFinality,
                        "beaconFinality.beaconSlot",
                        "beaconSlot",
                        "beacon_slot",
                        "finalizedSlot",
                        "finalized_slot",
                        "slot"),
                    "beaconFinality.beaconSlot"),
                ExecutionBlockNumber = NormalizeUnsignedInteger(
                    StrictFirstPresent(
                        beaconFinality,
                        "beaconFinality.executionBlockNumber",
                        "executionBlockNumber",
                        "execution_block_number"),
                    "beaconFinality.executionBlockNumber"),
                ExecutionBlockHash = NormalizeRpcHex(
                    StrictFirstPresent(
                        beaconFinality,
                        "beaconFinality.executionBlockHash",
                        "executionBlockHash",
                        "execution_block_hash"),
                    "beaconFinality.executionBlockHash",
                    32),
                ExecutionReceiptsRoot = NormalizeRpcHex(
                    StrictFirstPresent(
                        beaconFinality,
                        "beaconFinality.executionReceiptsRoot",
                        "executionReceiptsRoot",
                        "execution_receipts_root"),
                    "beaconFinality.executionReceiptsRoot",
                    32),
                BeaconFinalizedRoot = NormalizeRpcHex(
                    StrictFirstPresent(
                        beaconFinality,
                        "beaconFinality.finalizedHeaderRoot",
                        "finalizedHeaderRoot",
                        "finalized_header_root",
                        "beaconFinalizedRoot",
                        "beacon_finalized_root"),
                    "beaconFinality.finalizedHeaderRoot",
                    32),
                SyncCommitteeRoot = NormalizeRpcHex(
                    StrictFirstPresent(
                        beaconFinality,
                        "beaconFinality.syncCommitteeRoot",
                        "syncCommitteeRoot",
                        "sync_committee_root"),
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

        return SnapshotInboundEvidence(input with
        {
            SourceDomain = DomainEthereum,
            TargetDomain = DomainSora,
            TransactionHash = transactionHash,
            Receipt = SnapshotDictionaryOrNull(receipt),
            Block = SnapshotDictionaryOrNull(block),
            BeaconFinality = SnapshotDictionaryOrNull(beaconFinality),
            BlockReceipts = blockReceipts?.Select(SnapshotDictionary).ToArray(),
            InclusionBranch = input.InclusionBranch is null
                ? null
                : CopyByteArrays(input.InclusionBranch),
            ReceiptProof = receiptProof,
            ReceiptProofHash = NormalizeReceiptProofHash(receiptProof, input.ReceiptProofHash),
            SourceEventDigest = sourceEvent.SourceEventDigest,
            SourceBridgeEmitterAddress = sourceEvent.SourceBridgeEmitterAddress,
        });
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
        if (evidence.SourceEventDigest is null)
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

        foreach (var field in new[]
                 {
                     "finalityBranch",
                     "syncCommitteeBits",
                     "syncCommitteeSignature",
                     "syncCommitteeParticipation",
                     "syncSignatureSlot",
                 })
        {
            if (!evidence.BeaconFinality.ContainsKey(field))
            {
                throw new ArgumentException(
                    $"Ethereum mainnet SCCP inbound proof requires beaconFinality.{field}.",
                    nameof(input));
            }
        }

        var proofBytes = await inboundProver.ProveAsync(
            SnapshotInboundEvidence(evidence),
            cancellationToken).ConfigureAwait(false);
        return RequireNativeRecursiveBytes(proofBytes, nameof(proofBytes));
    }

    public static async ValueTask<object?> SubmitInboundToIrohaAsync(
        byte[] proofBytes,
        IEthereumMainnetInboundSubmitter inboundSubmitter,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(inboundSubmitter);

        var proofCopy = RequireNativeRecursiveBytes(proofBytes, nameof(proofBytes));
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
        payload.Write(RpcHexToBytes(executionBlockHash, nameof(executionBlockHash), 32));
        payload.Write(RpcHexToBytes(executionReceiptsRoot, nameof(executionReceiptsRoot), 32));
        payload.Write(RpcHexToBytes(beaconFinalizedRoot, nameof(beaconFinalizedRoot), 32));
        payload.Write(RpcHexToBytes(syncCommitteeRoot, nameof(syncCommitteeRoot), 32));
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
        var proverArtifacts = NormalizeOptionalGroth16ProverArtifacts(
            input.ProofArtifactHash,
            input.ProvingKeyHash);
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
            publicSignalWords,
            proverArtifacts?.ProofArtifactHash,
            proverArtifacts?.ProvingKeyHash);

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
            ProofArtifactHash: proverArtifacts?.ProofArtifactHash,
            ProvingKeyHash: proverArtifacts?.ProvingKeyHash,
            RequestHash: requestHash,
            DestinationBinding: destinationBinding);
    }

    public static EthereumMainnetOutboundProofRequest BuildOutboundProofRequest(
        EthereumMainnetOutboundProofRequestInput input,
        EthereumMainnetNativeEvmProverBundle nativeProverBundle)
    {
        ArgumentNullException.ThrowIfNull(nativeProverBundle);
        return BuildOutboundProofRequest(nativeProverBundle.ApplyTo(input));
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

    public static async ValueTask<EthereumMainnetOutboundProofResult> ProveOutboundToEthereumAsync(
        EthereumMainnetOutboundProofRequestInput input,
        IEthereumMainnetOutboundProver outboundProver,
        EthereumMainnetNativeEvmProverArtifacts nativeProverArtifacts,
        CancellationToken cancellationToken = default)
        => await ProveOutboundToEthereumAsync(
            input,
            outboundProver,
            nativeProverArtifacts,
            nativeProverSelfTest: null,
            cancellationToken).ConfigureAwait(false);

    public static async ValueTask<EthereumMainnetOutboundProofResult> ProveOutboundToEthereumAsync(
        EthereumMainnetOutboundProofRequestInput input,
        IEthereumMainnetOutboundProver outboundProver,
        EthereumMainnetNativeEvmProverArtifacts nativeProverArtifacts,
        IEthereumMainnetNativeProverSelfTest? nativeProverSelfTest,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(outboundProver);
        ArgumentNullException.ThrowIfNull(nativeProverArtifacts);

        var request = BuildOutboundProofRequest(input, nativeProverArtifacts.NativeProverBundle);
        RequireVerifiedNativeProverArtifacts(nativeProverArtifacts, request);
        _ = await RequireNativeProverSelfTestAsync(
            nativeProverArtifacts,
            nativeProverSelfTest,
            cancellationToken).ConfigureAwait(false);
        var proofBytes = await outboundProver.ProveAsync(
            Snapshot(request),
            cancellationToken).ConfigureAwait(false);
        return WrapOutboundProofResult(proofBytes, request);
    }

    public static async ValueTask<EthereumMainnetNativeEvmProverSelfTestSdkResult>
        RunNativeProverSelfTestAsync(
            EthereumMainnetNativeEvmProverArtifacts nativeProverArtifacts,
            IEthereumMainnetNativeProverSelfTest nativeProverSelfTest,
            CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(nativeProverArtifacts);
        ArgumentNullException.ThrowIfNull(nativeProverSelfTest);
        return await RequireNativeProverSelfTestAsync(
            nativeProverArtifacts,
            nativeProverSelfTest,
            cancellationToken).ConfigureAwait(false);
    }

    public static ValueTask<EthereumMainnetOutboundProofResult> ProveOutboundToEthereumFromNativeProverBundleAsync(
        EthereumMainnetOutboundProofRequestInput input,
        IEthereumMainnetOutboundProver outboundProver,
        EthereumMainnetNativeEvmProverBundle nativeProverBundle,
        string sdk,
        Func<string, byte[]> artifactResolver,
        CancellationToken cancellationToken = default)
        => ProveOutboundToEthereumFromNativeProverBundleAsync(
            input,
            outboundProver,
            nativeProverBundle,
            sdk,
            artifactResolver,
            nativeProverSelfTest: null,
            cancellationToken);

    public static ValueTask<EthereumMainnetOutboundProofResult> ProveOutboundToEthereumFromNativeProverBundleAsync(
        EthereumMainnetOutboundProofRequestInput input,
        IEthereumMainnetOutboundProver outboundProver,
        EthereumMainnetNativeEvmProverBundle nativeProverBundle,
        string sdk,
        Func<string, byte[]> artifactResolver,
        IEthereumMainnetNativeProverSelfTest? nativeProverSelfTest,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(nativeProverBundle);
        var verifiedArtifacts = nativeProverBundle.VerifiedArtifacts(sdk, artifactResolver);
        return ProveOutboundToEthereumAsync(
            input,
            outboundProver,
            verifiedArtifacts,
            nativeProverSelfTest,
            cancellationToken);
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
            ProofArtifactHash: request.ProofArtifactHash,
            ProvingKeyHash: request.ProvingKeyHash,
            RequestHash: request.RequestHash,
            EnvelopeHash: envelopeHash,
            DestinationBinding: request.DestinationBinding);
    }

    public static EthereumMainnetSccpSubmission BuildEthereumCalldata(
        EthereumMainnetSccpSubmissionInput input)
    {
        _ = BuildEthereumCalldataUnchecked(input);
        throw new ArgumentException(
            "Ethereum mainnet calldata requires verified native EVM prover artifacts.",
            nameof(input));
    }

    public static EthereumMainnetSccpSubmission BuildEthereumCalldata(
        EthereumMainnetSccpSubmissionInput input,
        EthereumMainnetNativeEvmProverArtifacts nativeProverArtifacts)
    {
        var submission = BuildEthereumCalldataUnchecked(input);
        RequireVerifiedNativeProverArtifacts(nativeProverArtifacts, input.ProofResult!);
        return submission;
    }

    public static EthereumMainnetSccpSubmission BuildEthereumCalldataFromNativeProverBundle(
        EthereumMainnetSccpSubmissionInput input,
        EthereumMainnetNativeEvmProverBundle nativeProverBundle,
        string sdk,
        Func<string, byte[]> artifactResolver)
    {
        ArgumentNullException.ThrowIfNull(nativeProverBundle);
        var verifiedArtifacts = nativeProverBundle.VerifiedArtifacts(sdk, artifactResolver);
        return BuildEthereumCalldata(input, verifiedArtifacts);
    }

    private static EthereumMainnetSccpSubmission BuildEthereumCalldataUnchecked(
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
        EthereumMainnetNativeEvmProverArtifacts nativeProverArtifacts,
        CancellationToken cancellationToken = default)
        => await SubmitOutboundToEthereumAsync(
            input,
            outboundSubmitter,
            nativeProverArtifacts,
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

    public static async ValueTask<object?> SubmitOutboundToEthereumAsync(
        EthereumMainnetSccpSubmissionInput input,
        IEthereumMainnetOutboundSubmitter outboundSubmitter,
        EthereumMainnetNativeEvmProverArtifacts nativeProverArtifacts,
        IEthereumMainnetExecutionProvider? executionProvider,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(outboundSubmitter);

        var submission = BuildEthereumCalldata(input, nativeProverArtifacts);
        if (executionProvider is not null)
        {
            _ = await ValidateExecutionProviderMainnetAsync(
                executionProvider,
                cancellationToken).ConfigureAwait(false);
        }
        return await outboundSubmitter.SubmitAsync(submission, cancellationToken).ConfigureAwait(false);
    }

    public static async ValueTask<object?> SubmitOutboundToEthereumFromNativeProverBundleAsync(
        EthereumMainnetSccpSubmissionInput input,
        IEthereumMainnetOutboundSubmitter outboundSubmitter,
        EthereumMainnetNativeEvmProverBundle nativeProverBundle,
        string sdk,
        Func<string, byte[]> artifactResolver,
        CancellationToken cancellationToken = default)
        => await SubmitOutboundToEthereumFromNativeProverBundleAsync(
            input,
            outboundSubmitter,
            nativeProverBundle,
            sdk,
            artifactResolver,
            executionProvider: null,
            cancellationToken).ConfigureAwait(false);

    public static async ValueTask<object?> SubmitOutboundToEthereumFromNativeProverBundleAsync(
        EthereumMainnetSccpSubmissionInput input,
        IEthereumMainnetOutboundSubmitter outboundSubmitter,
        EthereumMainnetNativeEvmProverBundle nativeProverBundle,
        string sdk,
        Func<string, byte[]> artifactResolver,
        IEthereumMainnetExecutionProvider? executionProvider,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(outboundSubmitter);
        ArgumentNullException.ThrowIfNull(nativeProverBundle);

        var verifiedArtifacts = nativeProverBundle.VerifiedArtifacts(sdk, artifactResolver);
        var submission = BuildEthereumCalldata(input, verifiedArtifacts);
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
            request.PublicSignalWords,
            request.ProofArtifactHash,
            request.ProvingKeyHash);
    }

    private static string ComputeProofRequestHash(
        byte[] publicInputsBytes,
        byte[] bundleBytes,
        byte[] sourceProofBytes,
        string statementHash,
        string destinationBindingHash,
        IReadOnlyList<string> publicSignalWords,
        string? proofArtifactHash = null,
        string? provingKeyHash = null)
    {
        if (publicSignalWords.Count != 9)
        {
            throw new ArgumentException(
                "Ethereum mainnet SCCP publicSignalWords must contain 9 words.",
                nameof(publicSignalWords));
        }

        var proverArtifacts = NormalizeOptionalGroth16ProverArtifacts(proofArtifactHash, provingKeyHash);
        using var payload = new MemoryStream();
        payload.Write(publicInputsBytes);
        payload.Write(WriteBytes(bundleBytes));
        payload.Write(WriteBytes(sourceProofBytes));
        payload.Write(HexToBytes(statementHash, 32));
        payload.Write(HexToBytes(destinationBindingHash, 32));
        if (proverArtifacts is not null)
        {
            payload.Write(HexToBytes(proverArtifacts.ProofArtifactHash, 32));
            payload.Write(HexToBytes(proverArtifacts.ProvingKeyHash, 32));
        }

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
        if (count != EthMainnetSyncCommitteeAuthorities)
        {
            throw new ArgumentException(
                $"syncCommitteePayload must contain exactly {EthMainnetSyncCommitteeAuthorities} entries.",
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
            if (weight != 1)
            {
                throw new ArgumentException(
                    $"syncCommitteeWeights[{index}] must be 1 for Ethereum mainnet.",
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

        if (!string.Equals(
                proofResult.ProofArtifactHash,
                proofResult.Request.ProofArtifactHash,
                StringComparison.Ordinal)
            || !string.Equals(
                proofResult.ProvingKeyHash,
                proofResult.Request.ProvingKeyHash,
                StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "proofResult proofArtifactHash and provingKeyHash must match request.");
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

    private static async ValueTask<EthereumMainnetNativeEvmProverSelfTestSdkResult>
        RequireNativeProverSelfTestAsync(
        EthereumMainnetNativeEvmProverArtifacts artifacts,
        IEthereumMainnetNativeProverSelfTest? nativeProverSelfTest,
        CancellationToken cancellationToken)
    {
        if (artifacts.NativeProverSelfTest is null)
        {
            throw new ArgumentException(
                "nativeProverArtifacts nativeProverSelfTest is required.",
                nameof(artifacts));
        }

        if (artifacts.Sdk is null)
        {
            throw new ArgumentException("nativeProverArtifacts sdk is required.", nameof(artifacts));
        }

        if (!artifacts.NativeProverSelfTest.SdkResults.TryGetValue(artifacts.Sdk, out var expectedResult))
        {
            throw new ArgumentException(
                $"nativeProverSelfTest sdkResults must include {artifacts.Sdk}.",
                nameof(artifacts));
        }

        if (nativeProverSelfTest is null)
        {
            throw new ArgumentException(
                "nativeProverSelfTest runner is required.",
                nameof(nativeProverSelfTest));
        }

        var result = await nativeProverSelfTest.RunAsync(
            artifacts.NativeProverSelfTest,
            expectedResult,
            artifacts,
            cancellationToken).ConfigureAwait(false);
        if (!NativeProverSelfTestResultEquals(result, expectedResult))
        {
            throw new ArgumentException(
                "nativeProverSelfTest result must match nativeProverBundle fixture.",
                nameof(nativeProverSelfTest));
        }

        return result;
    }

    private static bool NativeProverSelfTestResultEquals(
        EthereumMainnetNativeEvmProverSelfTestSdkResult left,
        EthereumMainnetNativeEvmProverSelfTestSdkResult right)
        => string.Equals(left.RequestHash, right.RequestHash, StringComparison.Ordinal)
            && string.Equals(left.WitnessHash, right.WitnessHash, StringComparison.Ordinal)
            && string.Equals(left.SourceProofHash, right.SourceProofHash, StringComparison.Ordinal)
            && string.Equals(left.ProofHash, right.ProofHash, StringComparison.Ordinal)
            && left.PublicSignalWords.SequenceEqual(right.PublicSignalWords)
            && string.Equals(left.CalldataHash, right.CalldataHash, StringComparison.Ordinal)
            && string.Equals(left.ToriiSubmitPayloadHash, right.ToriiSubmitPayloadHash, StringComparison.Ordinal);

    private static void RequireVerifiedNativeProverArtifacts(
        EthereumMainnetNativeEvmProverArtifacts artifacts,
        EthereumMainnetOutboundProofRequest request)
    {
        if (!string.Equals(
                artifacts.NativeProverBundle.DestinationBindingHash,
                request.DestinationBindingHash,
                StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverArtifacts destinationBindingHash must match proof request.",
                nameof(artifacts));
        }

        if (!string.Equals(artifacts.ProofArtifactHash, request.ProofArtifactHash, StringComparison.Ordinal)
            || !string.Equals(artifacts.ProvingKeyHash, request.ProvingKeyHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverArtifacts artifact hashes must match proof request.",
                nameof(artifacts));
        }

        if (!string.Equals(artifacts.VerifierKeyHash, artifacts.NativeProverBundle.VerifierKeyHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverArtifacts verifierKeyHash must match nativeProverBundle.",
                nameof(artifacts));
        }

        if (!string.Equals(
                artifacts.CrossSdkFixtureParityHash,
                artifacts.NativeProverBundle.AuditHashes["cross_sdk_fixture_parity"],
                StringComparison.Ordinal)
            || artifacts.CrossSdkFixtureParity is null
            || !string.Equals(
                artifacts.CrossSdkFixtureParity.DestinationBindingHash,
                artifacts.NativeProverBundle.DestinationBindingHash,
                StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverArtifacts crossSdkFixtureParityHash must match nativeProverBundle.",
                nameof(artifacts));
        }

        if (!string.Equals(
                artifacts.NativeProverSelfTestHash,
                artifacts.NativeProverBundle.AuditHashes["native_prover_self_test"],
                StringComparison.Ordinal)
            || artifacts.NativeProverSelfTest is null
            || !string.Equals(
                artifacts.NativeProverSelfTest.DestinationBindingHash,
                artifacts.NativeProverBundle.DestinationBindingHash,
                StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverArtifacts nativeProverSelfTestHash must match nativeProverBundle.",
                nameof(artifacts));
        }

        if (string.IsNullOrEmpty(artifacts.Sdk)
            || string.IsNullOrEmpty(artifacts.Implementation)
            || string.IsNullOrEmpty(artifacts.ImplementationHash))
        {
            throw new ArgumentException(
                "nativeProverArtifacts must bind sdk implementation and implementationHash.",
                nameof(artifacts));
        }

        var artifact = artifacts.NativeProverBundle.NativeSdkArtifacts
            .FirstOrDefault(row => string.Equals(row.Sdk, artifacts.Sdk, StringComparison.Ordinal));
        if (artifact is null)
        {
            throw new ArgumentException(
                $"nativeProverBundle has no artifact row for sdk: {artifacts.Sdk}.",
                nameof(artifacts));
        }

        if (!string.Equals(artifacts.Implementation, artifact.Implementation, StringComparison.Ordinal)
            || !string.Equals(artifacts.ImplementationHash, artifact.ImplementationHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverArtifacts implementation binding must match nativeProverBundle.",
                nameof(artifacts));
        }
    }

    private static void RequireVerifiedNativeProverArtifacts(
        EthereumMainnetNativeEvmProverArtifacts? artifacts,
        EthereumMainnetOutboundProofResult proofResult)
    {
        ArgumentNullException.ThrowIfNull(proofResult);
        if (artifacts is null)
        {
            throw new ArgumentException(
                "Ethereum mainnet SCCP submission requires verified native EVM prover artifacts.",
                nameof(artifacts));
        }

        if (!string.Equals(
                artifacts.NativeProverBundle.DestinationBindingHash,
                proofResult.DestinationBindingHash,
                StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverArtifacts destinationBindingHash must match proofResult.",
                nameof(artifacts));
        }

        if (!string.Equals(artifacts.ProofArtifactHash, proofResult.ProofArtifactHash, StringComparison.Ordinal)
            || !string.Equals(artifacts.ProvingKeyHash, proofResult.ProvingKeyHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverArtifacts artifact hashes must match proofResult.",
                nameof(artifacts));
        }

        if (!string.Equals(artifacts.VerifierKeyHash, artifacts.NativeProverBundle.VerifierKeyHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverArtifacts verifierKeyHash must match nativeProverBundle.",
                nameof(artifacts));
        }

        if (!string.Equals(
                artifacts.CrossSdkFixtureParityHash,
                artifacts.NativeProverBundle.AuditHashes["cross_sdk_fixture_parity"],
                StringComparison.Ordinal)
            || artifacts.CrossSdkFixtureParity is null
            || !string.Equals(
                artifacts.CrossSdkFixtureParity.DestinationBindingHash,
                artifacts.NativeProverBundle.DestinationBindingHash,
                StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverArtifacts crossSdkFixtureParityHash must match nativeProverBundle.",
                nameof(artifacts));
        }

        if (!string.Equals(
                artifacts.NativeProverSelfTestHash,
                artifacts.NativeProverBundle.AuditHashes["native_prover_self_test"],
                StringComparison.Ordinal)
            || artifacts.NativeProverSelfTest is null
            || !string.Equals(
                artifacts.NativeProverSelfTest.DestinationBindingHash,
                artifacts.NativeProverBundle.DestinationBindingHash,
                StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverArtifacts nativeProverSelfTestHash must match nativeProverBundle.",
                nameof(artifacts));
        }

        if (string.IsNullOrEmpty(artifacts.Sdk)
            || string.IsNullOrEmpty(artifacts.Implementation)
            || string.IsNullOrEmpty(artifacts.ImplementationHash))
        {
            throw new ArgumentException(
                "nativeProverArtifacts must bind sdk implementation and implementationHash.",
                nameof(artifacts));
        }

        var artifact = artifacts.NativeProverBundle.NativeSdkArtifacts
            .FirstOrDefault(row => string.Equals(row.Sdk, artifacts.Sdk, StringComparison.Ordinal));
        if (artifact is null)
        {
            throw new ArgumentException(
                $"nativeProverBundle has no artifact row for sdk: {artifacts.Sdk}.",
                nameof(artifacts));
        }

        if (!string.Equals(artifacts.Implementation, artifact.Implementation, StringComparison.Ordinal)
            || !string.Equals(artifacts.ImplementationHash, artifact.ImplementationHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverArtifacts implementation binding must match nativeProverBundle.",
                nameof(artifacts));
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
            request.PublicSignalWords,
            request.ProofArtifactHash,
            request.ProvingKeyHash);
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
        NormalizeOptionalGroth16ProverArtifacts(request.ProofArtifactHash, request.ProvingKeyHash);
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

    private static object? StrictFirstPresent(
        IReadOnlyDictionary<string, object?> input,
        string parameterName,
        params string[] keys)
    {
        object? selected = null;
        var found = false;
        foreach (var key in keys)
        {
            if (input.TryGetValue(key, out var value))
            {
                if (found)
                {
                    throw new ArgumentException(
                        $"{parameterName} must not use multiple aliases.",
                        parameterName);
                }
                selected = value;
                found = true;
            }
        }

        return selected;
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
                    nonZero: false,
                    allowEmpty: false)))
                .ToArray();
            encodedLogs[index] = RlpList([
                RlpBytes(EthereumRpcHexBytes(
                    FirstPresent(log, "address"),
                    $"receipt.logs[{index}].address",
                    byteLength: 20,
                    nonZero: false,
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

    private static IReadOnlyList<byte[]> RlpListByteFields(byte[] bytes)
    {
        var outer = ReadRlpItem(bytes, 0);
        if (!outer.IsList || outer.NextOffset != bytes.Length)
        {
            throw new ArgumentException("headerRlp must be an RLP list.", nameof(bytes));
        }

        var fields = new List<byte[]>();
        var cursor = outer.PayloadOffset;
        var end = outer.PayloadOffset + outer.PayloadLength;
        while (cursor < end)
        {
            var item = ReadRlpItem(bytes, cursor);
            if (item.IsList)
            {
                throw new ArgumentException(
                    "headerRlp must contain only RLP byte fields.",
                    nameof(bytes));
            }

            fields.Add(bytes.AsSpan(item.PayloadOffset, item.PayloadLength).ToArray());
            cursor = item.NextOffset;
        }

        return fields;
    }

    private static RlpDecodedItem ReadRlpItem(byte[] bytes, int offset)
    {
        if (offset < 0 || offset >= bytes.Length)
        {
            throw new ArgumentException("RLP item offset is out of bounds.", nameof(offset));
        }

        var prefix = bytes[offset];
        if (prefix <= 0x7f)
        {
            return new RlpDecodedItem(offset, 1, offset + 1, false);
        }

        if (prefix <= 0xb7)
        {
            var length = prefix - 0x80;
            var payloadOffset = offset + 1;
            RequireRlpBounds(bytes, payloadOffset, length);
            if (length == 1 && bytes[payloadOffset] < 0x80)
            {
                throw new ArgumentException("RLP byte field is not canonical.", nameof(bytes));
            }

            return new RlpDecodedItem(payloadOffset, length, payloadOffset + length, false);
        }

        if (prefix <= 0xbf)
        {
            var lengthOfLength = prefix - 0xb7;
            var payloadLength = ReadRlpLength(bytes, offset + 1, lengthOfLength);
            if (payloadLength < 56)
            {
                throw new ArgumentException("RLP long byte field is not canonical.", nameof(bytes));
            }

            var payloadOffset = offset + 1 + lengthOfLength;
            RequireRlpBounds(bytes, payloadOffset, payloadLength);
            return new RlpDecodedItem(payloadOffset, payloadLength, payloadOffset + payloadLength, false);
        }

        if (prefix <= 0xf7)
        {
            var length = prefix - 0xc0;
            var payloadOffset = offset + 1;
            RequireRlpBounds(bytes, payloadOffset, length);
            return new RlpDecodedItem(payloadOffset, length, payloadOffset + length, true);
        }

        var listLengthOfLength = prefix - 0xf7;
        var listPayloadLength = ReadRlpLength(bytes, offset + 1, listLengthOfLength);
        if (listPayloadLength < 56)
        {
            throw new ArgumentException("RLP long list is not canonical.", nameof(bytes));
        }

        var listPayloadOffset = offset + 1 + listLengthOfLength;
        RequireRlpBounds(bytes, listPayloadOffset, listPayloadLength);
        return new RlpDecodedItem(
            listPayloadOffset,
            listPayloadLength,
            listPayloadOffset + listPayloadLength,
            true);
    }

    private static int ReadRlpLength(byte[] bytes, int offset, int lengthOfLength)
    {
        if (lengthOfLength <= 0 || lengthOfLength > 4)
        {
            throw new ArgumentException("RLP length-of-length is out of range.", nameof(bytes));
        }

        RequireRlpBounds(bytes, offset, lengthOfLength);
        if (bytes[offset] == 0)
        {
            throw new ArgumentException("RLP length must be minimally encoded.", nameof(bytes));
        }

        var length = 0;
        for (var index = 0; index < lengthOfLength; index++)
        {
            length = checked((length << 8) | bytes[offset + index]);
        }

        return length;
    }

    private static void RequireRlpBounds(byte[] bytes, int offset, int length)
    {
        if (offset < 0 || length < 0 || offset > bytes.Length - length)
        {
            throw new ArgumentException("RLP item length exceeds input.", nameof(bytes));
        }
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
                    StrictFirstPresent(
                        log,
                        $"receipt.logs[{index}].transactionHash",
                        "transactionHash",
                        "transaction_hash"),
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
                    StrictFirstPresent(
                        log,
                        $"receipt.logs[{index}].blockHash",
                        "blockHash",
                        "block_hash"),
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
                    StrictFirstPresent(
                        log,
                        $"receipt.logs[{index}].blockNumber",
                        "blockNumber",
                        "block_number"),
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
            var finalityFinalizedRootInput = StrictFirstPresent(
                beaconFinality,
                "beaconFinality.finalizedHeaderRoot",
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

            var finalitySyncCommitteeRootInput = StrictFirstPresent(
                beaconFinality,
                "beaconFinality.syncCommitteeRoot",
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

            var finalityBeaconSlotInput = StrictFirstPresent(
                beaconFinality,
                "beaconFinality.beaconSlot",
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
            StrictFirstPresent(
                finality,
                "beaconFinality.executionBlockNumber",
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
            StrictFirstPresent(
                finality,
                "beaconFinality.executionBlockHash",
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
            StrictFirstPresent(
                finality,
                "beaconFinality.executionReceiptsRoot",
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

        var normalized = new Dictionary<string, object?>(finality, StringComparer.Ordinal);
        foreach (var key in BeaconFinalityAliasKeys)
        {
            normalized.Remove(key);
        }
        normalized["executionBlockNumber"] = executionBlockNumber.ToString(System.Globalization.CultureInfo.InvariantCulture);
        normalized["executionBlockHash"] = executionBlockHash;
        normalized["executionReceiptsRoot"] = executionReceiptsRoot;
        var finalizedHeaderRootInput = StrictFirstPresent(
            finality,
            "beaconFinality.finalizedHeaderRoot",
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

        var syncCommitteeRootInput = StrictFirstPresent(
            finality,
            "beaconFinality.syncCommitteeRoot",
            "syncCommitteeRoot",
            "sync_committee_root");
        if (syncCommitteeRootInput is not null)
        {
            normalized["syncCommitteeRoot"] = NormalizeRpcHex(
                syncCommitteeRootInput,
                "beaconFinality.syncCommitteeRoot",
                32);
        }
        var beaconSlotInput = StrictFirstPresent(
            finality,
            "beaconFinality.beaconSlot",
            "beaconSlot",
            "beacon_slot",
            "finalizedSlot",
            "finalized_slot",
            "slot");
        ulong? normalizedBeaconSlot = null;
        if (beaconSlotInput is not null)
        {
            normalizedBeaconSlot = NormalizeUnsignedInteger(
                beaconSlotInput,
                "beaconFinality.beaconSlot");
            if (normalizedBeaconSlot.Value == 0)
            {
                throw new ArgumentException(
                    "beaconFinality.beaconSlot must be positive.",
                    nameof(finality));
            }

            normalized["beaconSlot"] = normalizedBeaconSlot.Value.ToString(System.Globalization.CultureInfo.InvariantCulture);
        }
        var finalityBranchInput = StrictFirstPresent(
            finality,
            "beaconFinality.finalityBranch",
            "finalityBranch",
            "finality_branch");
        if (finalityBranchInput is not null)
        {
            normalized["finalityBranch"] = NormalizeFinalityBranch(
                finalityBranchInput,
                "beaconFinality.finalityBranch");
        }
        var syncCommitteeBitsInput = StrictFirstPresent(
            finality,
            "beaconFinality.syncCommitteeBits",
            "syncCommitteeBits",
            "sync_committee_bits");
        string? normalizedSyncCommitteeBits = null;
        if (syncCommitteeBitsInput is not null)
        {
            normalizedSyncCommitteeBits = NormalizeFinalitySyncCommitteeBits(
                syncCommitteeBitsInput,
                "beaconFinality.syncCommitteeBits");
            normalized["syncCommitteeBits"] = normalizedSyncCommitteeBits;
        }

        var syncCommitteeSignatureInput = StrictFirstPresent(
            finality,
            "beaconFinality.syncCommitteeSignature",
            "syncCommitteeSignature",
            "sync_committee_signature");
        if (syncCommitteeSignatureInput is not null)
        {
            normalized["syncCommitteeSignature"] = NormalizeRpcHex(
                syncCommitteeSignatureInput,
                "beaconFinality.syncCommitteeSignature",
                96);
        }

        var syncSignatureSlotInput = StrictFirstPresent(
            finality,
            "beaconFinality.syncSignatureSlot",
            "syncSignatureSlot",
            "sync_signature_slot",
            "signatureSlot",
            "signature_slot");
        ulong? normalizedSyncSignatureSlot = null;
        if (syncSignatureSlotInput is not null)
        {
            normalizedSyncSignatureSlot = NormalizeUnsignedInteger(
                syncSignatureSlotInput,
                "beaconFinality.syncSignatureSlot");
            if (normalizedSyncSignatureSlot.Value == 0)
            {
                throw new ArgumentException(
                    "beaconFinality.syncSignatureSlot must be positive.",
                    nameof(finality));
            }

            normalized["syncSignatureSlot"] = normalizedSyncSignatureSlot.Value.ToString(System.Globalization.CultureInfo.InvariantCulture);
        }
        if (normalizedBeaconSlot is not null
            && normalizedSyncSignatureSlot is not null
            && normalizedSyncSignatureSlot.Value < normalizedBeaconSlot.Value)
        {
            throw new ArgumentException(
                "beaconFinality.syncSignatureSlot must cover beaconFinality.beaconSlot.",
                nameof(finality));
        }

        var syncCommitteeParticipationInput = StrictFirstPresent(
            finality,
            "beaconFinality.syncCommitteeParticipation",
            "syncCommitteeParticipation",
            "sync_committee_participation");
        ulong? normalizedSyncCommitteeParticipation = null;
        if (syncCommitteeParticipationInput is not null)
        {
            normalizedSyncCommitteeParticipation = NormalizeUnsignedInteger(
                syncCommitteeParticipationInput,
                "beaconFinality.syncCommitteeParticipation");
            if (normalizedSyncCommitteeParticipation.Value == 0)
            {
                throw new ArgumentException(
                    "beaconFinality.syncCommitteeParticipation must be positive.",
                    nameof(finality));
            }

            normalized["syncCommitteeParticipation"] =
                normalizedSyncCommitteeParticipation.Value.ToString(System.Globalization.CultureInfo.InvariantCulture);
        }
        if (normalizedSyncCommitteeBits is not null
            && normalizedSyncCommitteeParticipation is not null
            && FinalitySyncCommitteeParticipation(normalizedSyncCommitteeBits) != normalizedSyncCommitteeParticipation.Value)
        {
            throw new ArgumentException(
                "beaconFinality.syncCommitteeParticipation must match syncCommitteeBits.",
                nameof(finality));
        }
        return normalized;
    }

    private static string NormalizeFinalitySyncCommitteeBits(object? value, string parameterName)
    {
        var bits = NormalizeRpcHex(value, parameterName, 64, allowZero: true);
        var participation = FinalitySyncCommitteeParticipation(bits);
        if (participation == 0)
        {
            throw new ArgumentException(
                $"{parameterName} must contain at least one participant.",
                parameterName);
        }

        if (participation * 3 < 512 * 2)
        {
            throw new ArgumentException(
                $"{parameterName} must contain Ethereum sync committee supermajority.",
                parameterName);
        }

        return bits;
    }

    private static IReadOnlyList<string> NormalizeFinalityBranch(object? value, string parameterName)
    {
        if (value is string || value is not System.Collections.IEnumerable values)
        {
            throw new ArgumentException($"{parameterName} must be an array.", parameterName);
        }
        var branch = values.Cast<object?>().Select((sibling, index) => NormalizeRpcHex(
                sibling,
                $"{parameterName}[{index}]",
                32,
                allowZero: true))
            .ToArray();
        if (branch.Length != 6)
        {
            throw new ArgumentException($"{parameterName} must contain 6 siblings.", parameterName);
        }
        return Array.AsReadOnly(branch);
    }

    private static ulong FinalitySyncCommitteeParticipation(string bits)
    {
        var text = bits[2..];
        ulong count = 0;
        for (var index = 0; index < text.Length; index += 2)
        {
            var value = Convert.ToByte(text[index..(index + 2)], 16);
            while (value != 0)
            {
                count += (ulong)(value & 1);
                value >>= 1;
            }
        }

        return count;
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

    private static EthereumMainnetInboundEvidence SnapshotInboundEvidence(
        EthereumMainnetInboundEvidence evidence)
    {
        return evidence with
        {
            Receipt = SnapshotDictionaryOrNull(evidence.Receipt),
            Block = SnapshotDictionaryOrNull(evidence.Block),
            BeaconFinality = SnapshotDictionaryOrNull(evidence.BeaconFinality),
            BlockReceipts = evidence.BlockReceipts?.Select(SnapshotDictionary).ToArray(),
            InclusionBranch = evidence.InclusionBranch is null
                ? null
                : CopyByteArrays(evidence.InclusionBranch),
            ReceiptProof = SnapshotReceiptProof(evidence.ReceiptProof),
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
            IReadOnlyList<string> list => list.ToArray(),
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

    private static byte[] SszHashNode(byte[] left, byte[] right)
    {
        if (left.Length != 32 || right.Length != 32)
        {
            throw new ArgumentException("SSZ node inputs must be 32 bytes.");
        }

        return SHA256.HashData(Concat(left, right));
    }

    private static byte[] SszMerkleizeChunks(IReadOnlyList<byte[]> inputChunks)
    {
        if (inputChunks.Count == 0)
        {
            return new byte[32];
        }

        var chunks = new List<byte[]>(inputChunks.Count);
        foreach (var chunk in inputChunks)
        {
            if (chunk.Length != 32)
            {
                throw new ArgumentException("SSZ chunk must be 32 bytes.", nameof(inputChunks));
            }

            chunks.Add(chunk.ToArray());
        }

        var paddedLength = 1;
        while (paddedLength < chunks.Count)
        {
            paddedLength <<= 1;
        }

        while (chunks.Count < paddedLength)
        {
            chunks.Add(new byte[32]);
        }

        while (chunks.Count > 1)
        {
            var next = new List<byte[]>(chunks.Count / 2);
            for (var index = 0; index < chunks.Count; index += 2)
            {
                next.Add(SszHashNode(chunks[index], chunks[index + 1]));
            }

            chunks = next;
        }

        return chunks[0];
    }

    private static byte[] SszU64Chunk(ulong value)
    {
        var outBytes = new byte[32];
        BinaryPrimitives.WriteUInt64LittleEndian(outBytes, value);
        return outBytes;
    }

    private static byte[] SszU64ChunkFromRlp(byte[] bytes, string field)
    {
        if (bytes.Length > 8 || (bytes.Length > 1 && bytes[0] == 0))
        {
            throw new ArgumentException($"{field} must be a canonical RLP u64.", field);
        }

        ulong value = 0;
        foreach (var item in bytes)
        {
            value = (value << 8) | item;
        }

        return SszU64Chunk(value);
    }

    private static byte[] SszU256ChunkFromRlp(byte[] bytes, string field)
    {
        if (bytes.Length > 32 || (bytes.Length > 1 && bytes[0] == 0))
        {
            throw new ArgumentException($"{field} must be a canonical RLP uint256.", field);
        }

        var outBytes = new byte[32];
        for (var index = 0; index < bytes.Length; index++)
        {
            outBytes[index] = bytes[bytes.Length - 1 - index];
        }

        return outBytes;
    }

    private static byte[] SszByteVectorRoot(byte[] bytes, int expectedLength, string field)
    {
        if (bytes.Length != expectedLength)
        {
            throw new ArgumentException($"{field} must be {expectedLength} bytes.", field);
        }

        var chunks = new List<byte[]>();
        for (var offset = 0; offset < bytes.Length; offset += 32)
        {
            var chunk = new byte[32];
            var length = Math.Min(32, bytes.Length - offset);
            bytes.AsSpan(offset, length).CopyTo(chunk);
            chunks.Add(chunk);
        }

        return SszMerkleizeChunks(chunks);
    }

    private static byte[] SszByteListRoot(byte[] bytes, int maxLength, string field)
    {
        if (bytes.Length > maxLength)
        {
            throw new ArgumentException($"{field} must be at most {maxLength} bytes.", field);
        }

        var limitChunks = Math.Max(1, (maxLength + 31) / 32);
        var chunks = new List<byte[]>();
        for (var offset = 0; offset < bytes.Length; offset += 32)
        {
            var chunk = new byte[32];
            var length = Math.Min(32, bytes.Length - offset);
            bytes.AsSpan(offset, length).CopyTo(chunk);
            chunks.Add(chunk);
        }

        while (chunks.Count < limitChunks)
        {
            chunks.Add(new byte[32]);
        }

        return SszHashNode(SszMerkleizeChunks(chunks), SszU64Chunk((ulong)bytes.Length));
    }

    private static byte[] SszMerkleRootFromBranch(
        byte[] leaf,
        int leafIndex,
        IReadOnlyList<byte[]> branch,
        string field)
    {
        if (leaf.Length != 32)
        {
            throw new ArgumentException($"{field} leaf must be 32 bytes.", field);
        }

        var current = leaf.ToArray();
        var index = leafIndex;
        for (var branchIndex = 0; branchIndex < branch.Count; branchIndex++)
        {
            var sibling = branch[branchIndex]
                ?? throw new ArgumentException($"{field}[{branchIndex}] is required.", field);
            if (sibling.Length != 32)
            {
                throw new ArgumentException($"{field}[{branchIndex}] must be 32 bytes.", field);
            }

            current = (index & 1) == 1
                ? SszHashNode(sibling, current)
                : SszHashNode(current, sibling);
            index >>= 1;
        }

        return current;
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

    private static Groth16ProverArtifacts? NormalizeOptionalGroth16ProverArtifacts(
        string? proofArtifactHash,
        string? provingKeyHash)
    {
        if ((proofArtifactHash is null) != (provingKeyHash is null))
        {
            throw new ArgumentException(
                "proofArtifactHash and provingKeyHash must be supplied together.");
        }

        if (proofArtifactHash is null || provingKeyHash is null)
        {
            return null;
        }

        return new Groth16ProverArtifacts(
            NormalizeNonZeroHex(proofArtifactHash, nameof(proofArtifactHash), 32),
            NormalizeNonZeroHex(provingKeyHash, nameof(provingKeyHash), 32));
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

public sealed record EthereumMainnetNativeEvmProverBundleSdkArtifact
{
    public EthereumMainnetNativeEvmProverBundleSdkArtifact(
        string sdk,
        string implementation,
        string proofArtifactHash,
        string provingKeyHash,
        string implementationHash,
        string? implementationArtifact = null)
    {
        if (string.IsNullOrEmpty(sdk))
        {
            throw new ArgumentException("nativeSdkArtifacts.sdk must be non-empty.", nameof(sdk));
        }

        if (!EthereumMainnetSccp.EthNativeEvmProverRequiredImplementationsV1.TryGetValue(
                sdk,
                out var expectedImplementation)
            || !string.Equals(expectedImplementation, implementation, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                $"{sdk} implementation must match Ethereum native EVM prover bundle profile.",
                nameof(implementation));
        }

        Sdk = sdk;
        Implementation = implementation;
        ProofArtifactHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            proofArtifactHash,
            nameof(proofArtifactHash));
        ProvingKeyHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            provingKeyHash,
            nameof(provingKeyHash));
        ImplementationArtifact = implementationArtifact is null
            ? null
            : EthereumMainnetSccp.NormalizeNativeEvmProverArtifactPath(
                implementationArtifact,
                nameof(implementationArtifact));
        ImplementationHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            implementationHash,
            nameof(implementationHash));
    }

    public string Sdk { get; }

    public string Implementation { get; }

    public string ProofArtifactHash { get; }

    public string ProvingKeyHash { get; }

    public string? ImplementationArtifact { get; }

    public string ImplementationHash { get; }
}

public sealed record EthereumMainnetNativeEvmProverBundle
{
    public EthereumMainnetNativeEvmProverBundle(
        string proofArtifactHash,
        string provingKeyHash,
        string verifierKeyHash,
        string destinationBindingHash,
        IReadOnlyList<EthereumMainnetNativeEvmProverBundleSdkArtifact> nativeSdkArtifacts,
        IReadOnlyDictionary<string, string> auditHashes,
        string schema = EthereumMainnetSccp.NativeEvmProverBundleSchemaV1,
        string bundleId = EthereumMainnetSccp.EthNativeEvmProverBundleIdV1,
        int domain = EthereumMainnetSccp.DomainEthereum,
        string chain = "eth",
        string proofBackend = EthereumMainnetSccp.EvmGroth16Bn254ProofBackend,
        bool noWasm = true,
        bool remoteProverRequired = false,
        string browserImplementation = "pure-typescript",
        string? expectedDestinationBindingHash = null,
        string? proofArtifact = null,
        string? provingKey = null,
        string? verifierKey = null,
        string? crossSdkFixtureParityArtifact = null,
        string? nativeProverSelfTestArtifact = null)
    {
        ArgumentNullException.ThrowIfNull(nativeSdkArtifacts);
        ArgumentNullException.ThrowIfNull(auditHashes);

        if (!string.Equals(schema, EthereumMainnetSccp.NativeEvmProverBundleSchemaV1, StringComparison.Ordinal))
        {
            throw new ArgumentException("nativeProverBundle.schema is not supported.", nameof(schema));
        }

        if (!string.Equals(bundleId, EthereumMainnetSccp.EthNativeEvmProverBundleIdV1, StringComparison.Ordinal))
        {
            throw new ArgumentException("nativeProverBundle.bundleId is not supported.", nameof(bundleId));
        }

        if (domain != EthereumMainnetSccp.DomainEthereum)
        {
            throw new ArgumentException("nativeProverBundle.domain must be ETH.", nameof(domain));
        }

        if (!string.Equals(chain, "eth", StringComparison.Ordinal))
        {
            throw new ArgumentException("nativeProverBundle.chain must be eth.", nameof(chain));
        }

        if (!string.Equals(proofBackend, EthereumMainnetSccp.EvmGroth16Bn254ProofBackend, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverBundle.proofBackend must be evm-groth16-bn254-v1.",
                nameof(proofBackend));
        }

        if (!noWasm)
        {
            throw new ArgumentException("nativeProverBundle.noWasm must be true.", nameof(noWasm));
        }

        if (remoteProverRequired)
        {
            throw new ArgumentException(
                "nativeProverBundle.remoteProverRequired must be false.",
                nameof(remoteProverRequired));
        }

        if (!string.Equals(browserImplementation, "pure-typescript", StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverBundle.browserImplementation must be pure-typescript.",
                nameof(browserImplementation));
        }

        ProofArtifactHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            proofArtifactHash,
            nameof(proofArtifactHash));
        ProvingKeyHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            provingKeyHash,
            nameof(provingKeyHash));
        VerifierKeyHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            verifierKeyHash,
            nameof(verifierKeyHash));
        DestinationBindingHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            destinationBindingHash,
            nameof(destinationBindingHash));
        if (expectedDestinationBindingHash is not null
            && !string.Equals(
                EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
                    expectedDestinationBindingHash,
                    nameof(expectedDestinationBindingHash)),
                DestinationBindingHash,
                StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverBundle.destinationBindingHash must match destinationBinding.",
                nameof(expectedDestinationBindingHash));
        }

        if (auditHashes.Count == 0)
        {
            throw new ArgumentException("nativeProverBundle.auditHashes must be non-empty.", nameof(auditHashes));
        }

        foreach (var key in auditHashes.Keys)
        {
            if (!EthereumMainnetSccp.EthNativeEvmProverRequiredAuditHashesV1.Contains(key, StringComparer.Ordinal))
            {
                throw new ArgumentException($"auditHashes.{key} is not expected.", nameof(auditHashes));
            }
        }

        var normalizedAuditHashes = new Dictionary<string, string>(StringComparer.Ordinal);
        foreach (var key in EthereumMainnetSccp.EthNativeEvmProverRequiredAuditHashesV1)
        {
            if (!auditHashes.TryGetValue(key, out var value))
            {
                throw new ArgumentException($"auditHashes.{key} is required.", nameof(auditHashes));
            }

            normalizedAuditHashes[key] = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
                value,
                $"auditHashes.{key}");
        }

        AuditHashes = normalizedAuditHashes;
        var artifactsBySdk = new Dictionary<string, EthereumMainnetNativeEvmProverBundleSdkArtifact>(
            StringComparer.Ordinal);
        foreach (var artifact in nativeSdkArtifacts)
        {
            if (!artifactsBySdk.TryAdd(artifact.Sdk, artifact))
            {
                throw new ArgumentException(
                    $"nativeSdkArtifacts contains duplicate sdk: {artifact.Sdk}.",
                    nameof(nativeSdkArtifacts));
            }

            if (!string.Equals(artifact.ProofArtifactHash, ProofArtifactHash, StringComparison.Ordinal)
                || !string.Equals(artifact.ProvingKeyHash, ProvingKeyHash, StringComparison.Ordinal))
            {
                throw new ArgumentException(
                    $"{artifact.Sdk} artifact hashes must match bundle.",
                    nameof(nativeSdkArtifacts));
            }
        }

        foreach (var sdk in EthereumMainnetSccp.EthNativeEvmProverRequiredImplementationsV1.Keys)
        {
            if (!artifactsBySdk.ContainsKey(sdk))
            {
                throw new ArgumentException(
                    $"nativeSdkArtifacts missing sdk: {sdk}.",
                    nameof(nativeSdkArtifacts));
            }
        }

        var sortedArtifacts = artifactsBySdk.Values
            .OrderBy(artifact => artifact.Sdk, StringComparer.Ordinal)
            .ToArray();
        var hashRoles = new List<KeyValuePair<string, string>>
        {
            new("proofArtifactHash", ProofArtifactHash),
            new("provingKeyHash", ProvingKeyHash),
            new("verifierKeyHash", VerifierKeyHash),
            new("destinationBindingHash", DestinationBindingHash),
        };
        hashRoles.AddRange(sortedArtifacts.Select(artifact =>
            new KeyValuePair<string, string>(
                $"nativeSdkArtifacts[{artifact.Sdk}].implementationHash",
                artifact.ImplementationHash)));
        hashRoles.AddRange(AuditHashes
            .OrderBy(row => row.Key, StringComparer.Ordinal)
            .Select(row => new KeyValuePair<string, string>($"auditHashes.{row.Key}", row.Value)));
        RequireNativeEvmProverBundleHashRoleSeparation(hashRoles);

        NativeSdkArtifacts = sortedArtifacts;
        Schema = schema;
        BundleId = bundleId;
        Domain = domain;
        Chain = chain;
        ProofBackend = proofBackend;
        ProofArtifact = proofArtifact is null
            ? null
            : EthereumMainnetSccp.NormalizeNativeEvmProverArtifactPath(
                proofArtifact,
                nameof(proofArtifact));
        ProvingKey = provingKey is null
            ? null
            : EthereumMainnetSccp.NormalizeNativeEvmProverArtifactPath(
                provingKey,
                nameof(provingKey));
        VerifierKey = verifierKey is null
            ? null
            : EthereumMainnetSccp.NormalizeNativeEvmProverArtifactPath(
                verifierKey,
                nameof(verifierKey));
        NoWasm = noWasm;
        RemoteProverRequired = remoteProverRequired;
        BrowserImplementation = browserImplementation;
        CrossSdkFixtureParityArtifact = crossSdkFixtureParityArtifact is null
            ? null
            : EthereumMainnetSccp.NormalizeNativeEvmProverArtifactPath(
                crossSdkFixtureParityArtifact,
                nameof(crossSdkFixtureParityArtifact));
        NativeProverSelfTestArtifact = nativeProverSelfTestArtifact is null
            ? null
            : EthereumMainnetSccp.NormalizeNativeEvmProverArtifactPath(
                nativeProverSelfTestArtifact,
                nameof(nativeProverSelfTestArtifact));
    }

    private static void RequireNativeEvmProverBundleHashRoleSeparation(
        IEnumerable<KeyValuePair<string, string>> roles)
    {
        var seen = new Dictionary<string, string>(StringComparer.Ordinal);
        foreach (var role in roles)
        {
            if (seen.TryGetValue(role.Value, out var previous))
            {
                throw new ArgumentException(
                    $"nativeProverBundle hashes must be role-separated: {role.Key} matches {previous}.");
            }

            seen.Add(role.Value, role.Key);
        }
    }

    public EthereumMainnetNativeEvmProverArtifacts VerifiedArtifacts(
        byte[] proofArtifactBytes,
        byte[] provingKeyBytes,
        byte[] verifierKeyBytes,
        string? sdk = null,
        byte[]? implementationBytes = null,
        byte[]? crossSdkFixtureParityBytes = null,
        byte[]? nativeProverSelfTestBytes = null)
    {
        ArgumentNullException.ThrowIfNull(proofArtifactBytes);
        ArgumentNullException.ThrowIfNull(provingKeyBytes);
        ArgumentNullException.ThrowIfNull(verifierKeyBytes);

        var proofArtifactHash = Sha256Hex(proofArtifactBytes);
        if (!string.Equals(proofArtifactHash, ProofArtifactHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "proofArtifactBytes sha256 must match nativeProverBundle.proofArtifactHash.",
                nameof(proofArtifactBytes));
        }

        var provingKeyHash = Sha256Hex(provingKeyBytes);
        if (!string.Equals(provingKeyHash, ProvingKeyHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "provingKeyBytes sha256 must match nativeProverBundle.provingKeyHash.",
                nameof(provingKeyBytes));
        }

        var verifierKeyHash = Sha256Hex(verifierKeyBytes);
        if (!string.Equals(verifierKeyHash, VerifierKeyHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "verifierKeyBytes sha256 must match nativeProverBundle.verifierKeyHash.",
                nameof(verifierKeyBytes));
        }

        if (crossSdkFixtureParityBytes is null)
        {
            throw new ArgumentException(
                "crossSdkFixtureParityBytes are required for nativeProverBundle parity binding.",
                nameof(crossSdkFixtureParityBytes));
        }

        var crossSdkFixtureParityHash = Sha256Hex(crossSdkFixtureParityBytes);
        if (!string.Equals(
                crossSdkFixtureParityHash,
                AuditHashes["cross_sdk_fixture_parity"],
                StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "crossSdkFixtureParityBytes sha256 must match nativeProverBundle.auditHashes.cross_sdk_fixture_parity.",
                nameof(crossSdkFixtureParityBytes));
        }

        if (nativeProverSelfTestBytes is null)
        {
            throw new ArgumentException(
                "nativeProverSelfTestBytes are required for nativeProverBundle self-test binding.",
                nameof(nativeProverSelfTestBytes));
        }

        var nativeProverSelfTestHash = Sha256Hex(nativeProverSelfTestBytes);
        if (!string.Equals(
                nativeProverSelfTestHash,
                AuditHashes["native_prover_self_test"],
                StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverSelfTestBytes sha256 must match nativeProverBundle.auditHashes.native_prover_self_test.",
                nameof(nativeProverSelfTestBytes));
        }

        RequireNativeEvmProverProductionArtifactSize(proofArtifactBytes, nameof(proofArtifactBytes));
        RequireNativeEvmProverProductionArtifactSize(provingKeyBytes, nameof(provingKeyBytes));
        RequireNativeEvmProverProductionArtifactSize(verifierKeyBytes, nameof(verifierKeyBytes));
        RejectNativeEvmProverForbiddenArtifactMarkers(proofArtifactBytes, nameof(proofArtifactBytes));
        RejectNativeEvmProverForbiddenArtifactMarkers(provingKeyBytes, nameof(provingKeyBytes));
        RejectNativeEvmProverForbiddenArtifactMarkers(verifierKeyBytes, nameof(verifierKeyBytes));
        RejectNativeEvmProverForbiddenArtifactMarkers(
            crossSdkFixtureParityBytes,
            nameof(crossSdkFixtureParityBytes));
        RejectNativeEvmProverForbiddenArtifactMarkers(
            nativeProverSelfTestBytes,
            nameof(nativeProverSelfTestBytes));
        var crossSdkFixtureParity = EthereumMainnetNativeEvmProverParityFixture.FromJsonBytes(
            crossSdkFixtureParityBytes,
            this);
        var nativeProverSelfTest = EthereumMainnetNativeEvmProverSelfTestFixture.FromJsonBytes(
            nativeProverSelfTestBytes,
            this);

        if (string.IsNullOrEmpty(sdk))
        {
            throw new ArgumentException(
                "sdk must be a non-empty string for nativeProverBundle implementation binding.",
                nameof(sdk));
        }

        if (implementationBytes is null)
        {
            throw new ArgumentException(
                "implementationBytes are required for nativeProverBundle implementation binding.",
                nameof(implementationBytes));
        }

        var artifact = NativeSdkArtifacts.FirstOrDefault(row => string.Equals(row.Sdk, sdk, StringComparison.Ordinal));
        if (artifact is null)
        {
            throw new ArgumentException($"nativeProverBundle has no artifact row for sdk: {sdk}.", nameof(sdk));
        }

        var implementationHash = Sha256Hex(implementationBytes);
        if (!string.Equals(implementationHash, artifact.ImplementationHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "implementationBytes sha256 must match nativeProverBundle implementationHash.",
                nameof(implementationBytes));
        }

        RequireNativeEvmProverProductionArtifactSize(
            implementationBytes,
            nameof(implementationBytes));
        RejectNativeEvmProverForbiddenArtifactMarkers(
            implementationBytes,
            nameof(implementationBytes));

        var implementation = artifact.Implementation;

        return new EthereumMainnetNativeEvmProverArtifacts(
            EthereumMainnetSccp.NativeEvmProverArtifactHashAlgorithmV1,
            this,
            proofArtifactHash,
            provingKeyHash,
            verifierKeyHash,
            crossSdkFixtureParityHash,
            crossSdkFixtureParity,
            nativeProverSelfTestHash,
            nativeProverSelfTest,
            sdk,
            implementation,
            implementationHash);
    }

    public EthereumMainnetNativeEvmProverArtifacts VerifiedArtifacts(
        string sdk,
        Func<string, byte[]> artifactResolver)
    {
        ArgumentNullException.ThrowIfNull(artifactResolver);
        if (ProofArtifact is null)
        {
            throw new ArgumentException("proofArtifact is required.", nameof(ProofArtifact));
        }

        if (ProvingKey is null)
        {
            throw new ArgumentException("provingKey is required.", nameof(ProvingKey));
        }

        if (VerifierKey is null)
        {
            throw new ArgumentException("verifierKey is required.", nameof(VerifierKey));
        }

        if (CrossSdkFixtureParityArtifact is null)
        {
            throw new ArgumentException(
                "crossSdkFixtureParityArtifact is required.",
                nameof(CrossSdkFixtureParityArtifact));
        }

        if (NativeProverSelfTestArtifact is null)
        {
            throw new ArgumentException(
                "nativeProverSelfTestArtifact is required.",
                nameof(NativeProverSelfTestArtifact));
        }

        var artifact = NativeSdkArtifacts.FirstOrDefault(row => string.Equals(row.Sdk, sdk, StringComparison.Ordinal));
        if (artifact is null)
        {
            throw new ArgumentException($"nativeProverBundle has no artifact row for sdk: {sdk}.", nameof(sdk));
        }

        if (artifact.ImplementationArtifact is null)
        {
            throw new ArgumentException(
                "implementationArtifact is required.",
                nameof(artifact.ImplementationArtifact));
        }

        return VerifiedArtifacts(
            artifactResolver(ProofArtifact),
            artifactResolver(ProvingKey),
            artifactResolver(VerifierKey),
            sdk,
            artifactResolver(artifact.ImplementationArtifact),
            artifactResolver(CrossSdkFixtureParityArtifact),
            artifactResolver(NativeProverSelfTestArtifact));
    }

    public static EthereumMainnetNativeEvmProverBundle FromJson(
        string json,
        string? expectedDestinationBindingHash = null)
    {
        ArgumentNullException.ThrowIfNull(json);
        using var document = JsonDocument.Parse(json);
        return FromJsonElement(document.RootElement, expectedDestinationBindingHash);
    }

    public static EthereumMainnetNativeEvmProverBundle FromJsonBytes(
        byte[] payload,
        string? expectedDestinationBindingHash = null)
    {
        ArgumentNullException.ThrowIfNull(payload);
        return FromJson(Encoding.UTF8.GetString(payload), expectedDestinationBindingHash);
    }

    public static EthereumMainnetNativeEvmProverBundle FromJsonElement(
        JsonElement manifest,
        string? expectedDestinationBindingHash = null)
    {
        RequireManifestObject(manifest, "nativeProverBundle");
        RequireManifestKeys(
            manifest,
            "nativeProverBundle",
            NativeEvmProverBundleManifestKeys);
        var proofArtifactHash = ManifestString(
            ManifestProperty(
                manifest,
                "proofArtifactHash",
                "proofArtifactHash",
                "proof_artifact_hash",
                "proverArtifactHash",
                "prover_artifact_hash",
                "circuitArtifactHash",
                "circuit_artifact_hash"),
            "proofArtifactHash");
        var provingKeyHash = ManifestString(
            ManifestProperty(manifest, "provingKeyHash", "provingKeyHash", "proving_key_hash"),
            "provingKeyHash");
        return new EthereumMainnetNativeEvmProverBundle(
            proofArtifactHash,
            provingKeyHash,
            ManifestString(
                ManifestProperty(manifest, "verifierKeyHash", "verifierKeyHash", "verifier_key_hash"),
                "verifierKeyHash"),
            ManifestString(
                ManifestProperty(
                    manifest,
                    "destinationBindingHash",
                    "destinationBindingHash",
                    "destination_binding_hash"),
                "destinationBindingHash"),
            ManifestSdkArtifacts(
                ManifestProperty(
                    manifest,
                    "nativeSdkArtifacts",
                    "nativeSdkArtifacts",
                    "native_sdk_artifacts",
                    "sdkArtifacts",
                    "sdk_artifacts")),
            ManifestStringMap(
                ManifestProperty(manifest, "auditHashes", "auditHashes", "audit_hashes"),
                "auditHashes"),
            ManifestString(ManifestProperty(manifest, "schema", "schema"), "schema"),
            ManifestString(ManifestProperty(manifest, "bundleId", "bundleId", "bundle_id"), "bundleId"),
            ManifestDomain(ManifestProperty(manifest, "domain", "domain"), "domain"),
            ManifestString(ManifestProperty(manifest, "chain", "chain"), "chain"),
            ManifestString(
                ManifestProperty(manifest, "proofBackend", "proofBackend", "proof_backend", "backend"),
                "proofBackend"),
            ManifestBool(ManifestProperty(manifest, "noWasm", "noWasm", "no_wasm"), "noWasm"),
            ManifestBool(
                ManifestProperty(
                    manifest,
                    "remoteProverRequired",
                    "remoteProverRequired",
                    "remote_prover_required"),
                "remoteProverRequired"),
            ManifestString(
                ManifestProperty(
                    manifest,
                    "browserImplementation",
                    "browserImplementation",
                    "browser_implementation"),
                "browserImplementation"),
            expectedDestinationBindingHash,
            proofArtifact: ManifestString(
                ManifestProperty(
                    manifest,
                    "proofArtifact",
                    "proofArtifact",
                    "proof_artifact",
                    "proverArtifact",
                    "prover_artifact",
                    "circuitArtifact",
                    "circuit_artifact"),
                "proofArtifact"),
            provingKey: ManifestString(
                ManifestProperty(manifest, "provingKey", "provingKey", "proving_key"),
                "provingKey"),
            verifierKey: ManifestString(
                ManifestProperty(manifest, "verifierKey", "verifierKey", "verifier_key"),
                "verifierKey"),
            crossSdkFixtureParityArtifact: ManifestString(
                ManifestProperty(
                    manifest,
                    "crossSdkFixtureParityArtifact",
                    "crossSdkFixtureParityArtifact",
                    "cross_sdk_fixture_parity_artifact"),
                "crossSdkFixtureParityArtifact"),
            nativeProverSelfTestArtifact: ManifestString(
                ManifestProperty(
                    manifest,
                    "nativeProverSelfTestArtifact",
                    "nativeProverSelfTestArtifact",
                    "native_prover_self_test_artifact",
                    "selfTestArtifact",
                    "self_test_artifact"),
                "nativeProverSelfTestArtifact"));
    }

    public string Schema { get; }

    public string BundleId { get; }

    public int Domain { get; }

    public string Chain { get; }

    public string ProofBackend { get; }

    public string ProofArtifactHash { get; }

    public string? ProofArtifact { get; }

    public string ProvingKeyHash { get; }

    public string? ProvingKey { get; }

    public string VerifierKeyHash { get; }

    public string? VerifierKey { get; }

    public string DestinationBindingHash { get; }

    public bool NoWasm { get; }

    public bool RemoteProverRequired { get; }

    public string BrowserImplementation { get; }

    public IReadOnlyList<EthereumMainnetNativeEvmProverBundleSdkArtifact> NativeSdkArtifacts { get; }

    public string? CrossSdkFixtureParityArtifact { get; }

    public string? NativeProverSelfTestArtifact { get; }

    public IReadOnlyDictionary<string, string> AuditHashes { get; }

    private static string Sha256Hex(byte[] value) => "0x" + Convert.ToHexString(SHA256.HashData(value)).ToLowerInvariant();

    private static readonly byte[][] NativeEvmProverForbiddenArtifactMarkers =
    {
        new byte[] { 0x77, 0x65, 0x62, 0x61, 0x73, 0x73, 0x65, 0x6D, 0x62, 0x6C, 0x79 },
        new byte[] { 0x77, 0x61, 0x73, 0x6D },
        new byte[] { 0x73, 0x6E, 0x61, 0x72, 0x6B, 0x6A, 0x73 },
        new byte[] { 0x72, 0x65, 0x6D, 0x6F, 0x74, 0x65, 0x70, 0x72, 0x6F, 0x76, 0x65, 0x72 },
        new byte[] { 0x72, 0x65, 0x6D, 0x6F, 0x74, 0x65, 0x20, 0x70, 0x72, 0x6F, 0x76, 0x65, 0x72 },
        new byte[] { 0x72, 0x65, 0x6D, 0x6F, 0x74, 0x65, 0x5F, 0x70, 0x72, 0x6F, 0x76, 0x65, 0x72 },
        new byte[] { 0x70, 0x72, 0x6F, 0x76, 0x65, 0x72, 0x5F, 0x75, 0x72, 0x6C },
        new byte[] { 0x70, 0x72, 0x6F, 0x76, 0x65, 0x72, 0x2D, 0x75, 0x72, 0x6C },
        new byte[] { 0x70, 0x72, 0x6F, 0x76, 0x65, 0x72, 0x65, 0x6E, 0x64, 0x70, 0x6F, 0x69, 0x6E, 0x74 },
        new byte[] { 0x70, 0x72, 0x6F, 0x76, 0x65, 0x72, 0x20, 0x65, 0x6E, 0x64, 0x70, 0x6F, 0x69, 0x6E, 0x74 },
    };

    private static int LowerAsciiByte(byte value) =>
        value >= 0x41 && value <= 0x5A ? value + 0x20 : value;

    private static bool ContainsNativeEvmProverMarker(
        ReadOnlySpan<byte> bytes,
        ReadOnlySpan<byte> marker)
    {
        if (marker.Length > bytes.Length)
        {
            return false;
        }

        for (var offset = 0; offset <= bytes.Length - marker.Length; offset++)
        {
            var matched = true;
            for (var index = 0; index < marker.Length; index++)
            {
                if (LowerAsciiByte(bytes[offset + index]) != marker[index])
                {
                    matched = false;
                    break;
                }
            }

            if (matched)
            {
                return true;
            }
        }

        return false;
    }

    private static void RejectNativeEvmProverForbiddenArtifactMarkers(
        byte[] bytes,
        string parameterName)
    {
        foreach (var marker in NativeEvmProverForbiddenArtifactMarkers)
        {
            if (ContainsNativeEvmProverMarker(bytes, marker))
            {
                throw new ArgumentException(
                    $"{parameterName} contains forbidden prover dependency marker.",
                    parameterName);
            }
        }
    }

    private static void RequireNativeEvmProverProductionArtifactSize(
        byte[] bytes,
        string parameterName)
    {
        if (bytes.Length < EthereumMainnetSccp.NativeEvmProverMinArtifactBytesV1)
        {
            throw new ArgumentException(
                $"{parameterName} must be at least {EthereumMainnetSccp.NativeEvmProverMinArtifactBytesV1} bytes.",
                parameterName);
        }
    }

    public EthereumMainnetOutboundProofRequestInput ApplyTo(EthereumMainnetOutboundProofRequestInput input)
    {
        ArgumentNullException.ThrowIfNull(input);
        var destinationBindingHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            input.DestinationBindingHash ?? input.DestinationBinding?.BindingHash
                ?? throw new ArgumentException("destinationBindingHash is required.", nameof(input)),
            nameof(input.DestinationBindingHash));
        if (!string.Equals(destinationBindingHash, DestinationBindingHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverBundle.destinationBindingHash must match destinationBinding.",
                nameof(input));
        }

        if (input.DestinationBinding is not null
            && !string.Equals(
                EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
                    input.DestinationBinding.VerifierKeyHash,
                    nameof(input.DestinationBinding.VerifierKeyHash)),
                VerifierKeyHash,
                StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverBundle.verifierKeyHash must match destinationBinding.",
                nameof(input));
        }

        if (input.ProofArtifactHash is not null
            && !string.Equals(
                EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
                    input.ProofArtifactHash,
                    nameof(input.ProofArtifactHash)),
                ProofArtifactHash,
                StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverBundle.proofArtifactHash must match proof request.",
                nameof(input));
        }

        if (input.ProvingKeyHash is not null
            && !string.Equals(
                EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
                    input.ProvingKeyHash,
                    nameof(input.ProvingKeyHash)),
                ProvingKeyHash,
                StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverBundle.provingKeyHash must match proof request.",
                nameof(input));
        }

        if ((input.ProofArtifactHash is null) != (input.ProvingKeyHash is null))
        {
            throw new ArgumentException(
                "proofArtifactHash and provingKeyHash must be supplied together.",
                nameof(input));
        }

        return input with
        {
            DestinationBindingHash = DestinationBindingHash,
            ProofArtifactHash = ProofArtifactHash,
            ProvingKeyHash = ProvingKeyHash,
        };
    }

    internal static void RequireManifestObject(JsonElement value, string label)
    {
        if (value.ValueKind != JsonValueKind.Object)
        {
            throw new ArgumentException($"{label} must be an object.");
        }
    }

    private static readonly HashSet<string> NativeEvmProverBundleManifestKeys =
        new(StringComparer.Ordinal)
        {
            "schema",
            "bundleId",
            "bundle_id",
            "domain",
            "chain",
            "proofBackend",
            "proof_backend",
            "backend",
            "proofArtifact",
            "proof_artifact",
            "proverArtifact",
            "prover_artifact",
            "circuitArtifact",
            "circuit_artifact",
            "proofArtifactHash",
            "proof_artifact_hash",
            "proverArtifactHash",
            "prover_artifact_hash",
            "circuitArtifactHash",
            "circuit_artifact_hash",
            "provingKey",
            "proving_key",
            "provingKeyHash",
            "proving_key_hash",
            "verifierKey",
            "verifier_key",
            "verifierKeyHash",
            "verifier_key_hash",
            "destinationBindingHash",
            "destination_binding_hash",
            "noWasm",
            "no_wasm",
            "remoteProverRequired",
            "remote_prover_required",
            "browserImplementation",
            "browser_implementation",
            "nativeSdkArtifacts",
            "native_sdk_artifacts",
            "sdkArtifacts",
            "sdk_artifacts",
            "crossSdkFixtureParityArtifact",
            "cross_sdk_fixture_parity_artifact",
            "nativeProverSelfTestArtifact",
            "native_prover_self_test_artifact",
            "selfTestArtifact",
            "self_test_artifact",
            "auditHashes",
            "audit_hashes",
        };

    internal static readonly HashSet<string> NativeEvmProverParityFixtureKeys =
        new(StringComparer.Ordinal)
        {
            "schema",
            "domain",
            "chain",
            "proofBackend",
            "proof_backend",
            "backend",
            "proofArtifactHash",
            "proof_artifact_hash",
            "proverArtifactHash",
            "prover_artifact_hash",
            "circuitArtifactHash",
            "circuit_artifact_hash",
            "provingKeyHash",
            "proving_key_hash",
            "verifierKeyHash",
            "verifier_key_hash",
            "destinationBindingHash",
            "destination_binding_hash",
            "receiptProofHash",
            "receipt_proof_hash",
            "sourceProofHash",
            "source_proof_hash",
            "publicSignalWords",
            "public_signal_words",
            "calldataHash",
            "calldata_hash",
            "toriiSubmitPayloadHash",
            "torii_submit_payload_hash",
            "sdkResults",
            "sdk_results",
        };

    internal static readonly HashSet<string> NativeEvmProverParitySdkResultKeys =
        new(StringComparer.Ordinal)
        {
            "receiptProofHash",
            "receipt_proof_hash",
            "sourceProofHash",
            "source_proof_hash",
            "destinationBindingHash",
            "destination_binding_hash",
            "publicSignalWords",
            "public_signal_words",
            "calldataHash",
            "calldata_hash",
            "toriiSubmitPayloadHash",
            "torii_submit_payload_hash",
        };

    internal static readonly HashSet<string> NativeEvmProverSelfTestFixtureKeys =
        new(StringComparer.Ordinal)
        {
            "schema",
            "domain",
            "chain",
            "proofBackend",
            "proof_backend",
            "backend",
            "proofArtifactHash",
            "proof_artifact_hash",
            "proverArtifactHash",
            "prover_artifact_hash",
            "circuitArtifactHash",
            "circuit_artifact_hash",
            "provingKeyHash",
            "proving_key_hash",
            "verifierKeyHash",
            "verifier_key_hash",
            "destinationBindingHash",
            "destination_binding_hash",
            "requestHash",
            "request_hash",
            "witnessHash",
            "witness_hash",
            "sourceProofHash",
            "source_proof_hash",
            "proofHash",
            "proof_hash",
            "publicSignalWords",
            "public_signal_words",
            "calldataHash",
            "calldata_hash",
            "toriiSubmitPayloadHash",
            "torii_submit_payload_hash",
            "sdkResults",
            "sdk_results",
        };

    internal static readonly HashSet<string> NativeEvmProverSelfTestSdkResultKeys =
        new(StringComparer.Ordinal)
        {
            "requestHash",
            "request_hash",
            "witnessHash",
            "witness_hash",
            "sourceProofHash",
            "source_proof_hash",
            "proofHash",
            "proof_hash",
            "publicSignalWords",
            "public_signal_words",
            "calldataHash",
            "calldata_hash",
            "toriiSubmitPayloadHash",
            "torii_submit_payload_hash",
        };

    private static readonly HashSet<string> NativeEvmProverBundleSdkArtifactKeys =
        new(StringComparer.Ordinal)
        {
            "sdk",
            "implementation",
            "proofArtifactHash",
            "proof_artifact_hash",
            "proverArtifactHash",
            "prover_artifact_hash",
            "provingKeyHash",
            "proving_key_hash",
            "implementationArtifact",
            "implementation_artifact",
            "implementationPath",
            "implementation_path",
            "implementationHash",
            "implementation_hash",
        };

    internal static void RequireManifestKeys(
        JsonElement value,
        string label,
        IReadOnlySet<string> allowedKeys)
    {
        var seen = new HashSet<string>(StringComparer.Ordinal);
        foreach (var property in value.EnumerateObject())
        {
            if (!seen.Add(property.Name))
            {
                throw new ArgumentException($"{label} contains duplicate JSON key: {property.Name}.");
            }

            if (!allowedKeys.Contains(property.Name))
            {
                throw new ArgumentException($"{label} contains unknown field: {property.Name}.");
            }
        }
    }

    internal static JsonElement ManifestProperty(JsonElement value, string label, params string[] aliases)
    {
        RequireManifestObject(value, label);
        string? present = null;
        JsonElement selected = default;
        foreach (var alias in aliases)
        {
            if (value.TryGetProperty(alias, out var property))
            {
                if (present is not null)
                {
                    throw new ArgumentException($"{label} must not use multiple aliases.");
                }

                present = alias;
                selected = property;
            }
        }

        if (present is not null)
        {
            return selected;
        }

        throw new ArgumentException($"{label} is required.");
    }

    internal static string ManifestString(JsonElement value, string label)
    {
        if (value.ValueKind != JsonValueKind.String)
        {
            throw new ArgumentException($"{label} must be a string.");
        }

        return value.GetString()!;
    }

    internal static IReadOnlyList<string> ManifestStringList(JsonElement value, string label)
    {
        if (value.ValueKind != JsonValueKind.Array)
        {
            throw new ArgumentException($"{label} must be an array.");
        }

        return value.EnumerateArray()
            .Select((item, index) => ManifestString(item, $"{label}[{index}]"))
            .ToArray();
    }

    private static bool ManifestBool(JsonElement value, string label)
    {
        return value.ValueKind switch
        {
            JsonValueKind.True => true,
            JsonValueKind.False => false,
            _ => throw new ArgumentException($"{label} must be a boolean."),
        };
    }

    internal static int ManifestDomain(JsonElement value, string label)
    {
        if (value.ValueKind == JsonValueKind.Number && value.TryGetInt32(out var numeric))
        {
            return numeric;
        }

        if (value.ValueKind == JsonValueKind.String)
        {
            var text = value.GetString()!;
            if (text.Length == 0
                || (text != "0" && (text[0] == '0' || !text.All(static character => character is >= '0' and <= '9')))
                || !int.TryParse(text, NumberStyles.None, CultureInfo.InvariantCulture, out var parsed))
            {
                throw new ArgumentException($"{label} must be a canonical decimal integer.");
            }

            return parsed;
        }

        throw new ArgumentException($"{label} must be an integer.");
    }

    private static IReadOnlyDictionary<string, string> ManifestStringMap(JsonElement value, string label)
    {
        if (value.ValueKind != JsonValueKind.Object)
        {
            throw new ArgumentException($"{label} must be an object.");
        }

        var values = new Dictionary<string, string>(StringComparer.Ordinal);
        foreach (var item in value.EnumerateObject().OrderBy(item => item.Name, StringComparer.Ordinal))
        {
            values[item.Name] = ManifestString(item.Value, $"{label}.{item.Name}");
        }

        if (values.Count == 0)
        {
            throw new ArgumentException($"{label} must be non-empty.");
        }

        return values;
    }

    private static IReadOnlyList<EthereumMainnetNativeEvmProverBundleSdkArtifact> ManifestSdkArtifacts(
        JsonElement value)
    {
        if (value.ValueKind != JsonValueKind.Array)
        {
            throw new ArgumentException("nativeSdkArtifacts must be an array.");
        }

        var artifacts = value.EnumerateArray()
            .Select((item, index) =>
            {
                RequireManifestObject(item, $"nativeSdkArtifacts[{index}]");
                RequireManifestKeys(
                    item,
                    $"nativeSdkArtifacts[{index}]",
                    NativeEvmProverBundleSdkArtifactKeys);
                return new EthereumMainnetNativeEvmProverBundleSdkArtifact(
                    ManifestString(
                        ManifestProperty(item, $"nativeSdkArtifacts[{index}].sdk", "sdk"),
                        $"nativeSdkArtifacts[{index}].sdk"),
                    ManifestString(
                        ManifestProperty(
                            item,
                            $"nativeSdkArtifacts[{index}].implementation",
                            "implementation"),
                        $"nativeSdkArtifacts[{index}].implementation"),
                    ManifestString(
                        ManifestProperty(
                            item,
                            $"nativeSdkArtifacts[{index}].proofArtifactHash",
                            "proofArtifactHash",
                            "proof_artifact_hash",
                            "proverArtifactHash",
                            "prover_artifact_hash"),
                        $"nativeSdkArtifacts[{index}].proofArtifactHash"),
                    ManifestString(
                        ManifestProperty(
                            item,
                            $"nativeSdkArtifacts[{index}].provingKeyHash",
                            "provingKeyHash",
                            "proving_key_hash"),
                        $"nativeSdkArtifacts[{index}].provingKeyHash"),
                    ManifestString(
                        ManifestProperty(
                            item,
                            $"nativeSdkArtifacts[{index}].implementationHash",
                            "implementationHash",
                            "implementation_hash"),
                        $"nativeSdkArtifacts[{index}].implementationHash"),
                    implementationArtifact: ManifestString(
                        ManifestProperty(
                            item,
                            $"nativeSdkArtifacts[{index}].implementationArtifact",
                            "implementationArtifact",
                            "implementation_artifact",
                            "implementationPath",
                            "implementation_path"),
                        $"nativeSdkArtifacts[{index}].implementationArtifact"));
            })
            .ToArray();
        if (artifacts.Length == 0)
        {
            throw new ArgumentException("nativeSdkArtifacts must be non-empty.");
        }

        return artifacts;
    }
}

public sealed record EthereumMainnetNativeEvmProverParitySdkResult
{
    public EthereumMainnetNativeEvmProverParitySdkResult(
        string receiptProofHash,
        string sourceProofHash,
        string destinationBindingHash,
        IReadOnlyList<string> publicSignalWords,
        string calldataHash,
        string toriiSubmitPayloadHash)
    {
        ReceiptProofHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            receiptProofHash,
            nameof(receiptProofHash));
        SourceProofHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            sourceProofHash,
            nameof(sourceProofHash));
        DestinationBindingHash = EthereumMainnetSccp.NormalizeNativeEvmProverParityHex32(
            destinationBindingHash,
            nameof(destinationBindingHash));
        ArgumentNullException.ThrowIfNull(publicSignalWords);
        if (publicSignalWords.Count != 9)
        {
            throw new ArgumentException("publicSignalWords must contain 9 words.", nameof(publicSignalWords));
        }

        PublicSignalWords = publicSignalWords
            .Select((word, index) => EthereumMainnetSccp.NormalizeNativeEvmProverParityHex32(
                word,
                $"publicSignalWords[{index}]"))
            .ToArray();
        CalldataHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            calldataHash,
            nameof(calldataHash));
        ToriiSubmitPayloadHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            toriiSubmitPayloadHash,
            nameof(toriiSubmitPayloadHash));
    }

    public string ReceiptProofHash { get; }

    public string SourceProofHash { get; }

    public string DestinationBindingHash { get; }

    public IReadOnlyList<string> PublicSignalWords { get; }

    public string CalldataHash { get; }

    public string ToriiSubmitPayloadHash { get; }
}

public sealed record EthereumMainnetNativeEvmProverParityFixture
{
    public EthereumMainnetNativeEvmProverParityFixture(
        EthereumMainnetNativeEvmProverBundle nativeProverBundle,
        string proofArtifactHash,
        string provingKeyHash,
        string verifierKeyHash,
        string destinationBindingHash,
        string receiptProofHash,
        string sourceProofHash,
        IReadOnlyList<string> publicSignalWords,
        string calldataHash,
        string toriiSubmitPayloadHash,
        IReadOnlyDictionary<string, EthereumMainnetNativeEvmProverParitySdkResult> sdkResults,
        string schema = EthereumMainnetSccp.EthNativeEvmProverParityFixtureSchemaV1,
        int domain = EthereumMainnetSccp.DomainEthereum,
        string chain = "eth",
        string proofBackend = EthereumMainnetSccp.EvmGroth16Bn254ProofBackend)
    {
        ArgumentNullException.ThrowIfNull(nativeProverBundle);
        ArgumentNullException.ThrowIfNull(sdkResults);
        if (!string.Equals(schema, EthereumMainnetSccp.EthNativeEvmProverParityFixtureSchemaV1, StringComparison.Ordinal))
        {
            throw new ArgumentException("nativeProverParityFixture.schema is not supported.", nameof(schema));
        }

        if (domain != EthereumMainnetSccp.DomainEthereum || domain != nativeProverBundle.Domain)
        {
            throw new ArgumentException(
                "nativeProverParityFixture.domain must match nativeProverBundle.",
                nameof(domain));
        }

        if (!string.Equals(chain, nativeProverBundle.Chain, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverParityFixture.chain must match nativeProverBundle.",
                nameof(chain));
        }

        if (!string.Equals(proofBackend, nativeProverBundle.ProofBackend, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverParityFixture.proofBackend must match nativeProverBundle.",
                nameof(proofBackend));
        }

        ProofArtifactHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            proofArtifactHash,
            nameof(proofArtifactHash));
        ProvingKeyHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            provingKeyHash,
            nameof(provingKeyHash));
        VerifierKeyHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            verifierKeyHash,
            nameof(verifierKeyHash));
        DestinationBindingHash = EthereumMainnetSccp.NormalizeNativeEvmProverParityHex32(
            destinationBindingHash,
            nameof(destinationBindingHash));
        if (!string.Equals(ProofArtifactHash, nativeProverBundle.ProofArtifactHash, StringComparison.Ordinal)
            || !string.Equals(ProvingKeyHash, nativeProverBundle.ProvingKeyHash, StringComparison.Ordinal)
            || !string.Equals(VerifierKeyHash, nativeProverBundle.VerifierKeyHash, StringComparison.Ordinal)
            || !string.Equals(DestinationBindingHash, nativeProverBundle.DestinationBindingHash, StringComparison.Ordinal))
        {
            throw new ArgumentException("nativeProverParityFixture hashes must match nativeProverBundle.");
        }

        ReceiptProofHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            receiptProofHash,
            nameof(receiptProofHash));
        SourceProofHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            sourceProofHash,
            nameof(sourceProofHash));
        ArgumentNullException.ThrowIfNull(publicSignalWords);
        if (publicSignalWords.Count != 9)
        {
            throw new ArgumentException("publicSignalWords must contain 9 words.", nameof(publicSignalWords));
        }

        PublicSignalWords = publicSignalWords
            .Select((word, index) => EthereumMainnetSccp.NormalizeNativeEvmProverParityHex32(
                word,
                $"publicSignalWords[{index}]"))
            .ToArray();
        CalldataHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            calldataHash,
            nameof(calldataHash));
        ToriiSubmitPayloadHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            toriiSubmitPayloadHash,
            nameof(toriiSubmitPayloadHash));

        var requiredSdks = EthereumMainnetSccp.EthNativeEvmProverRequiredImplementationsV1.Keys
            .ToHashSet(StringComparer.Ordinal);
        if (!sdkResults.Keys.ToHashSet(StringComparer.Ordinal).SetEquals(requiredSdks))
        {
            throw new ArgumentException("sdkResults must contain exactly the required SDKs.", nameof(sdkResults));
        }

        var normalizedResults = new Dictionary<string, EthereumMainnetNativeEvmProverParitySdkResult>(
            StringComparer.Ordinal);
        foreach (var sdk in requiredSdks.OrderBy(static sdk => sdk, StringComparer.Ordinal))
        {
            var result = sdkResults[sdk];
            if (!string.Equals(result.ReceiptProofHash, ReceiptProofHash, StringComparison.Ordinal)
                || !string.Equals(result.SourceProofHash, SourceProofHash, StringComparison.Ordinal)
                || !string.Equals(result.DestinationBindingHash, DestinationBindingHash, StringComparison.Ordinal)
                || !result.PublicSignalWords.SequenceEqual(PublicSignalWords)
                || !string.Equals(result.CalldataHash, CalldataHash, StringComparison.Ordinal)
                || !string.Equals(result.ToriiSubmitPayloadHash, ToriiSubmitPayloadHash, StringComparison.Ordinal))
            {
                throw new ArgumentException($"sdkResults.{sdk} must match top-level parity fixture hashes.", nameof(sdkResults));
            }

            normalizedResults[sdk] = result;
        }

        Schema = schema;
        Domain = domain;
        Chain = chain;
        ProofBackend = proofBackend;
        SdkResults = normalizedResults;
    }

    public string Schema { get; }

    public int Domain { get; }

    public string Chain { get; }

    public string ProofBackend { get; }

    public string ProofArtifactHash { get; }

    public string ProvingKeyHash { get; }

    public string VerifierKeyHash { get; }

    public string DestinationBindingHash { get; }

    public string ReceiptProofHash { get; }

    public string SourceProofHash { get; }

    public IReadOnlyList<string> PublicSignalWords { get; }

    public string CalldataHash { get; }

    public string ToriiSubmitPayloadHash { get; }

    public IReadOnlyDictionary<string, EthereumMainnetNativeEvmProverParitySdkResult> SdkResults { get; }

    public static EthereumMainnetNativeEvmProverParityFixture FromJson(
        string json,
        EthereumMainnetNativeEvmProverBundle nativeProverBundle)
    {
        ArgumentNullException.ThrowIfNull(json);
        using var document = JsonDocument.Parse(json);
        return FromJsonElement(document.RootElement, nativeProverBundle);
    }

    public static EthereumMainnetNativeEvmProverParityFixture FromJsonBytes(
        byte[] payload,
        EthereumMainnetNativeEvmProverBundle nativeProverBundle)
    {
        ArgumentNullException.ThrowIfNull(payload);
        return FromJson(Encoding.UTF8.GetString(payload), nativeProverBundle);
    }

    public static EthereumMainnetNativeEvmProverParityFixture FromJsonElement(
        JsonElement fixture,
        EthereumMainnetNativeEvmProverBundle nativeProverBundle)
    {
        EthereumMainnetNativeEvmProverBundle.RequireManifestObject(
            fixture,
            "nativeProverParityFixture");
        EthereumMainnetNativeEvmProverBundle.RequireManifestKeys(
            fixture,
            "nativeProverParityFixture",
            EthereumMainnetNativeEvmProverBundle.NativeEvmProverParityFixtureKeys);
        var publicSignalWords = EthereumMainnetNativeEvmProverBundle.ManifestStringList(
            EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                fixture,
                "publicSignalWords",
                "publicSignalWords",
                "public_signal_words"),
            "publicSignalWords");
        var sdkResultsElement = EthereumMainnetNativeEvmProverBundle.ManifestProperty(
            fixture,
            "sdkResults",
            "sdkResults",
            "sdk_results");
        EthereumMainnetNativeEvmProverBundle.RequireManifestObject(sdkResultsElement, "sdkResults");
        var sdkResults = new Dictionary<string, EthereumMainnetNativeEvmProverParitySdkResult>(
            StringComparer.Ordinal);
        foreach (var sdkResult in sdkResultsElement.EnumerateObject())
        {
            EthereumMainnetNativeEvmProverBundle.RequireManifestObject(
                sdkResult.Value,
                $"sdkResults.{sdkResult.Name}");
            EthereumMainnetNativeEvmProverBundle.RequireManifestKeys(
                sdkResult.Value,
                $"sdkResults.{sdkResult.Name}",
                EthereumMainnetNativeEvmProverBundle.NativeEvmProverParitySdkResultKeys);
            sdkResults[sdkResult.Name] = new EthereumMainnetNativeEvmProverParitySdkResult(
                EthereumMainnetNativeEvmProverBundle.ManifestString(
                    EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                        sdkResult.Value,
                        $"sdkResults.{sdkResult.Name}.receiptProofHash",
                        "receiptProofHash",
                        "receipt_proof_hash"),
                    $"sdkResults.{sdkResult.Name}.receiptProofHash"),
                EthereumMainnetNativeEvmProverBundle.ManifestString(
                    EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                        sdkResult.Value,
                        $"sdkResults.{sdkResult.Name}.sourceProofHash",
                        "sourceProofHash",
                        "source_proof_hash"),
                    $"sdkResults.{sdkResult.Name}.sourceProofHash"),
                EthereumMainnetNativeEvmProverBundle.ManifestString(
                    EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                        sdkResult.Value,
                        $"sdkResults.{sdkResult.Name}.destinationBindingHash",
                        "destinationBindingHash",
                        "destination_binding_hash"),
                    $"sdkResults.{sdkResult.Name}.destinationBindingHash"),
                EthereumMainnetNativeEvmProverBundle.ManifestStringList(
                    EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                        sdkResult.Value,
                        $"sdkResults.{sdkResult.Name}.publicSignalWords",
                        "publicSignalWords",
                        "public_signal_words"),
                    $"sdkResults.{sdkResult.Name}.publicSignalWords"),
                EthereumMainnetNativeEvmProverBundle.ManifestString(
                    EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                        sdkResult.Value,
                        $"sdkResults.{sdkResult.Name}.calldataHash",
                        "calldataHash",
                        "calldata_hash"),
                    $"sdkResults.{sdkResult.Name}.calldataHash"),
                EthereumMainnetNativeEvmProverBundle.ManifestString(
                    EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                        sdkResult.Value,
                        $"sdkResults.{sdkResult.Name}.toriiSubmitPayloadHash",
                        "toriiSubmitPayloadHash",
                        "torii_submit_payload_hash"),
                    $"sdkResults.{sdkResult.Name}.toriiSubmitPayloadHash"));
        }

        return new EthereumMainnetNativeEvmProverParityFixture(
            nativeProverBundle,
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                    fixture,
                    "proofArtifactHash",
                    "proofArtifactHash",
                    "proof_artifact_hash",
                    "proverArtifactHash",
                    "prover_artifact_hash",
                    "circuitArtifactHash",
                    "circuit_artifact_hash"),
                "proofArtifactHash"),
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                    fixture,
                    "provingKeyHash",
                    "provingKeyHash",
                    "proving_key_hash"),
                "provingKeyHash"),
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                    fixture,
                    "verifierKeyHash",
                    "verifierKeyHash",
                    "verifier_key_hash"),
                "verifierKeyHash"),
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                    fixture,
                    "destinationBindingHash",
                    "destinationBindingHash",
                    "destination_binding_hash"),
                "destinationBindingHash"),
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                    fixture,
                    "receiptProofHash",
                    "receiptProofHash",
                    "receipt_proof_hash"),
                "receiptProofHash"),
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                    fixture,
                    "sourceProofHash",
                    "sourceProofHash",
                    "source_proof_hash"),
                "sourceProofHash"),
            publicSignalWords,
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                    fixture,
                    "calldataHash",
                    "calldataHash",
                    "calldata_hash"),
                "calldataHash"),
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                    fixture,
                    "toriiSubmitPayloadHash",
                    "toriiSubmitPayloadHash",
                    "torii_submit_payload_hash"),
                "toriiSubmitPayloadHash"),
            sdkResults,
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(fixture, "schema", "schema"),
                "schema"),
            EthereumMainnetNativeEvmProverBundle.ManifestDomain(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(fixture, "domain", "domain"),
                "domain"),
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(fixture, "chain", "chain"),
                "chain"),
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                    fixture,
                    "proofBackend",
                    "proofBackend",
                    "proof_backend",
                    "backend"),
                "proofBackend"));
    }
}

public sealed record EthereumMainnetNativeEvmProverSelfTestSdkResult
{
    public EthereumMainnetNativeEvmProverSelfTestSdkResult(
        string requestHash,
        string witnessHash,
        string sourceProofHash,
        string proofHash,
        IReadOnlyList<string> publicSignalWords,
        string calldataHash,
        string toriiSubmitPayloadHash)
    {
        RequestHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            requestHash,
            nameof(requestHash));
        WitnessHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            witnessHash,
            nameof(witnessHash));
        SourceProofHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            sourceProofHash,
            nameof(sourceProofHash));
        ProofHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            proofHash,
            nameof(proofHash));
        ArgumentNullException.ThrowIfNull(publicSignalWords);
        if (publicSignalWords.Count != 9)
        {
            throw new ArgumentException("publicSignalWords must contain 9 words.", nameof(publicSignalWords));
        }

        PublicSignalWords = publicSignalWords
            .Select((word, index) => EthereumMainnetSccp.NormalizeNativeEvmProverParityHex32(
                word,
                $"publicSignalWords[{index}]"))
            .ToArray();
        CalldataHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            calldataHash,
            nameof(calldataHash));
        ToriiSubmitPayloadHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            toriiSubmitPayloadHash,
            nameof(toriiSubmitPayloadHash));
    }

    public string RequestHash { get; }

    public string WitnessHash { get; }

    public string SourceProofHash { get; }

    public string ProofHash { get; }

    public IReadOnlyList<string> PublicSignalWords { get; }

    public string CalldataHash { get; }

    public string ToriiSubmitPayloadHash { get; }
}

public sealed record EthereumMainnetNativeEvmProverSelfTestFixture
{
    public EthereumMainnetNativeEvmProverSelfTestFixture(
        EthereumMainnetNativeEvmProverBundle nativeProverBundle,
        string proofArtifactHash,
        string provingKeyHash,
        string verifierKeyHash,
        string destinationBindingHash,
        string requestHash,
        string witnessHash,
        string sourceProofHash,
        string proofHash,
        IReadOnlyList<string> publicSignalWords,
        string calldataHash,
        string toriiSubmitPayloadHash,
        IReadOnlyDictionary<string, EthereumMainnetNativeEvmProverSelfTestSdkResult> sdkResults,
        string schema = EthereumMainnetSccp.EthNativeEvmProverSelfTestSchemaV1,
        int domain = EthereumMainnetSccp.DomainEthereum,
        string chain = "eth",
        string proofBackend = EthereumMainnetSccp.EvmGroth16Bn254ProofBackend)
    {
        ArgumentNullException.ThrowIfNull(nativeProverBundle);
        ArgumentNullException.ThrowIfNull(sdkResults);
        if (!string.Equals(schema, EthereumMainnetSccp.EthNativeEvmProverSelfTestSchemaV1, StringComparison.Ordinal))
        {
            throw new ArgumentException("nativeProverSelfTestFixture.schema is not supported.", nameof(schema));
        }

        if (domain != EthereumMainnetSccp.DomainEthereum || domain != nativeProverBundle.Domain)
        {
            throw new ArgumentException(
                "nativeProverSelfTestFixture.domain must match nativeProverBundle.",
                nameof(domain));
        }

        if (!string.Equals(chain, nativeProverBundle.Chain, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverSelfTestFixture.chain must match nativeProverBundle.",
                nameof(chain));
        }

        if (!string.Equals(proofBackend, nativeProverBundle.ProofBackend, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "nativeProverSelfTestFixture.proofBackend must match nativeProverBundle.",
                nameof(proofBackend));
        }

        ProofArtifactHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            proofArtifactHash,
            nameof(proofArtifactHash));
        ProvingKeyHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            provingKeyHash,
            nameof(provingKeyHash));
        VerifierKeyHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            verifierKeyHash,
            nameof(verifierKeyHash));
        DestinationBindingHash = EthereumMainnetSccp.NormalizeNativeEvmProverParityHex32(
            destinationBindingHash,
            nameof(destinationBindingHash));
        if (!string.Equals(ProofArtifactHash, nativeProverBundle.ProofArtifactHash, StringComparison.Ordinal)
            || !string.Equals(ProvingKeyHash, nativeProverBundle.ProvingKeyHash, StringComparison.Ordinal)
            || !string.Equals(VerifierKeyHash, nativeProverBundle.VerifierKeyHash, StringComparison.Ordinal)
            || !string.Equals(DestinationBindingHash, nativeProverBundle.DestinationBindingHash, StringComparison.Ordinal))
        {
            throw new ArgumentException("nativeProverSelfTestFixture hashes must match nativeProverBundle.");
        }

        RequestHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            requestHash,
            nameof(requestHash));
        WitnessHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            witnessHash,
            nameof(witnessHash));
        SourceProofHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            sourceProofHash,
            nameof(sourceProofHash));
        ProofHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            proofHash,
            nameof(proofHash));
        ArgumentNullException.ThrowIfNull(publicSignalWords);
        if (publicSignalWords.Count != 9)
        {
            throw new ArgumentException("publicSignalWords must contain 9 words.", nameof(publicSignalWords));
        }

        PublicSignalWords = publicSignalWords
            .Select((word, index) => EthereumMainnetSccp.NormalizeNativeEvmProverParityHex32(
                word,
                $"publicSignalWords[{index}]"))
            .ToArray();
        CalldataHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            calldataHash,
            nameof(calldataHash));
        ToriiSubmitPayloadHash = EthereumMainnetSccp.NormalizeNativeEvmProverBundleHex32(
            toriiSubmitPayloadHash,
            nameof(toriiSubmitPayloadHash));

        var requiredSdks = EthereumMainnetSccp.EthNativeEvmProverRequiredImplementationsV1.Keys
            .ToHashSet(StringComparer.Ordinal);
        if (!sdkResults.Keys.ToHashSet(StringComparer.Ordinal).SetEquals(requiredSdks))
        {
            throw new ArgumentException("sdkResults must contain exactly the required SDKs.", nameof(sdkResults));
        }

        var normalizedResults = new Dictionary<string, EthereumMainnetNativeEvmProverSelfTestSdkResult>(
            StringComparer.Ordinal);
        foreach (var sdk in requiredSdks.OrderBy(static sdk => sdk, StringComparer.Ordinal))
        {
            var result = sdkResults[sdk];
            if (!string.Equals(result.RequestHash, RequestHash, StringComparison.Ordinal)
                || !string.Equals(result.WitnessHash, WitnessHash, StringComparison.Ordinal)
                || !string.Equals(result.SourceProofHash, SourceProofHash, StringComparison.Ordinal)
                || !string.Equals(result.ProofHash, ProofHash, StringComparison.Ordinal)
                || !result.PublicSignalWords.SequenceEqual(PublicSignalWords)
                || !string.Equals(result.CalldataHash, CalldataHash, StringComparison.Ordinal)
                || !string.Equals(result.ToriiSubmitPayloadHash, ToriiSubmitPayloadHash, StringComparison.Ordinal))
            {
                throw new ArgumentException($"sdkResults.{sdk} must match top-level self-test fixture hashes.", nameof(sdkResults));
            }

            normalizedResults[sdk] = result;
        }

        Schema = schema;
        Domain = domain;
        Chain = chain;
        ProofBackend = proofBackend;
        SdkResults = normalizedResults;
    }

    public string Schema { get; }

    public int Domain { get; }

    public string Chain { get; }

    public string ProofBackend { get; }

    public string ProofArtifactHash { get; }

    public string ProvingKeyHash { get; }

    public string VerifierKeyHash { get; }

    public string DestinationBindingHash { get; }

    public string RequestHash { get; }

    public string WitnessHash { get; }

    public string SourceProofHash { get; }

    public string ProofHash { get; }

    public IReadOnlyList<string> PublicSignalWords { get; }

    public string CalldataHash { get; }

    public string ToriiSubmitPayloadHash { get; }

    public IReadOnlyDictionary<string, EthereumMainnetNativeEvmProverSelfTestSdkResult> SdkResults { get; }

    public static EthereumMainnetNativeEvmProverSelfTestFixture FromJson(
        string json,
        EthereumMainnetNativeEvmProverBundle nativeProverBundle)
    {
        ArgumentNullException.ThrowIfNull(json);
        using var document = JsonDocument.Parse(json);
        return FromJsonElement(document.RootElement, nativeProverBundle);
    }

    public static EthereumMainnetNativeEvmProverSelfTestFixture FromJsonBytes(
        byte[] payload,
        EthereumMainnetNativeEvmProverBundle nativeProverBundle)
    {
        ArgumentNullException.ThrowIfNull(payload);
        return FromJson(Encoding.UTF8.GetString(payload), nativeProverBundle);
    }

    public static EthereumMainnetNativeEvmProverSelfTestFixture FromJsonElement(
        JsonElement fixture,
        EthereumMainnetNativeEvmProverBundle nativeProverBundle)
    {
        EthereumMainnetNativeEvmProverBundle.RequireManifestObject(
            fixture,
            "nativeProverSelfTestFixture");
        EthereumMainnetNativeEvmProverBundle.RequireManifestKeys(
            fixture,
            "nativeProverSelfTestFixture",
            EthereumMainnetNativeEvmProverBundle.NativeEvmProverSelfTestFixtureKeys);
        var publicSignalWords = EthereumMainnetNativeEvmProverBundle.ManifestStringList(
            EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                fixture,
                "publicSignalWords",
                "publicSignalWords",
                "public_signal_words"),
            "publicSignalWords");
        var sdkResultsElement = EthereumMainnetNativeEvmProverBundle.ManifestProperty(
            fixture,
            "sdkResults",
            "sdkResults",
            "sdk_results");
        EthereumMainnetNativeEvmProverBundle.RequireManifestObject(sdkResultsElement, "sdkResults");
        var sdkResults = new Dictionary<string, EthereumMainnetNativeEvmProverSelfTestSdkResult>(
            StringComparer.Ordinal);
        foreach (var sdkResult in sdkResultsElement.EnumerateObject())
        {
            EthereumMainnetNativeEvmProverBundle.RequireManifestObject(
                sdkResult.Value,
                $"sdkResults.{sdkResult.Name}");
            EthereumMainnetNativeEvmProverBundle.RequireManifestKeys(
                sdkResult.Value,
                $"sdkResults.{sdkResult.Name}",
                EthereumMainnetNativeEvmProverBundle.NativeEvmProverSelfTestSdkResultKeys);
            sdkResults[sdkResult.Name] = new EthereumMainnetNativeEvmProverSelfTestSdkResult(
                EthereumMainnetNativeEvmProverBundle.ManifestString(
                    EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                        sdkResult.Value,
                        $"sdkResults.{sdkResult.Name}.requestHash",
                        "requestHash",
                        "request_hash"),
                    $"sdkResults.{sdkResult.Name}.requestHash"),
                EthereumMainnetNativeEvmProverBundle.ManifestString(
                    EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                        sdkResult.Value,
                        $"sdkResults.{sdkResult.Name}.witnessHash",
                        "witnessHash",
                        "witness_hash"),
                    $"sdkResults.{sdkResult.Name}.witnessHash"),
                EthereumMainnetNativeEvmProverBundle.ManifestString(
                    EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                        sdkResult.Value,
                        $"sdkResults.{sdkResult.Name}.sourceProofHash",
                        "sourceProofHash",
                        "source_proof_hash"),
                    $"sdkResults.{sdkResult.Name}.sourceProofHash"),
                EthereumMainnetNativeEvmProverBundle.ManifestString(
                    EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                        sdkResult.Value,
                        $"sdkResults.{sdkResult.Name}.proofHash",
                        "proofHash",
                        "proof_hash"),
                    $"sdkResults.{sdkResult.Name}.proofHash"),
                EthereumMainnetNativeEvmProverBundle.ManifestStringList(
                    EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                        sdkResult.Value,
                        $"sdkResults.{sdkResult.Name}.publicSignalWords",
                        "publicSignalWords",
                        "public_signal_words"),
                    $"sdkResults.{sdkResult.Name}.publicSignalWords"),
                EthereumMainnetNativeEvmProverBundle.ManifestString(
                    EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                        sdkResult.Value,
                        $"sdkResults.{sdkResult.Name}.calldataHash",
                        "calldataHash",
                        "calldata_hash"),
                    $"sdkResults.{sdkResult.Name}.calldataHash"),
                EthereumMainnetNativeEvmProverBundle.ManifestString(
                    EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                        sdkResult.Value,
                        $"sdkResults.{sdkResult.Name}.toriiSubmitPayloadHash",
                        "toriiSubmitPayloadHash",
                        "torii_submit_payload_hash"),
                    $"sdkResults.{sdkResult.Name}.toriiSubmitPayloadHash"));
        }

        return new EthereumMainnetNativeEvmProverSelfTestFixture(
            nativeProverBundle,
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                    fixture,
                    "proofArtifactHash",
                    "proofArtifactHash",
                    "proof_artifact_hash",
                    "proverArtifactHash",
                    "prover_artifact_hash",
                    "circuitArtifactHash",
                    "circuit_artifact_hash"),
                "proofArtifactHash"),
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                    fixture,
                    "provingKeyHash",
                    "provingKeyHash",
                    "proving_key_hash"),
                "provingKeyHash"),
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                    fixture,
                    "verifierKeyHash",
                    "verifierKeyHash",
                    "verifier_key_hash"),
                "verifierKeyHash"),
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                    fixture,
                    "destinationBindingHash",
                    "destinationBindingHash",
                    "destination_binding_hash"),
                "destinationBindingHash"),
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                    fixture,
                    "requestHash",
                    "requestHash",
                    "request_hash"),
                "requestHash"),
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                    fixture,
                    "witnessHash",
                    "witnessHash",
                    "witness_hash"),
                "witnessHash"),
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                    fixture,
                    "sourceProofHash",
                    "sourceProofHash",
                    "source_proof_hash"),
                "sourceProofHash"),
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                    fixture,
                    "proofHash",
                    "proofHash",
                    "proof_hash"),
                "proofHash"),
            publicSignalWords,
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                    fixture,
                    "calldataHash",
                    "calldataHash",
                    "calldata_hash"),
                "calldataHash"),
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                    fixture,
                    "toriiSubmitPayloadHash",
                    "toriiSubmitPayloadHash",
                    "torii_submit_payload_hash"),
                "toriiSubmitPayloadHash"),
            sdkResults,
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(fixture, "schema", "schema"),
                "schema"),
            EthereumMainnetNativeEvmProverBundle.ManifestDomain(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(fixture, "domain", "domain"),
                "domain"),
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(fixture, "chain", "chain"),
                "chain"),
            EthereumMainnetNativeEvmProverBundle.ManifestString(
                EthereumMainnetNativeEvmProverBundle.ManifestProperty(
                    fixture,
                    "proofBackend",
                    "proofBackend",
                    "proof_backend",
                    "backend"),
                "proofBackend"));
    }
}

public sealed record EthereumMainnetNativeEvmProverArtifacts(
    string HashAlgorithm,
    EthereumMainnetNativeEvmProverBundle NativeProverBundle,
    string ProofArtifactHash,
    string ProvingKeyHash,
    string VerifierKeyHash,
    string? CrossSdkFixtureParityHash,
    EthereumMainnetNativeEvmProverParityFixture? CrossSdkFixtureParity,
    string? NativeProverSelfTestHash,
    EthereumMainnetNativeEvmProverSelfTestFixture? NativeProverSelfTest,
    string? Sdk,
    string? Implementation,
    string? ImplementationHash);

public sealed record EthereumMainnetOutboundProofRequestInput
{
    public EthereumMainnetTransparentPublicInputs? PublicInputs { get; init; }

    public byte[] BundleBytes { get; init; } = [];

    public byte[]? SourceProofBytes { get; init; }

    public string StatementHash { get; init; } = string.Empty;

    public string? DestinationBindingHash { get; init; }

    public string? ProofArtifactHash { get; init; }

    public string? ProvingKeyHash { get; init; }

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
    string? ProofArtifactHash,
    string? ProvingKeyHash,
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
    string? ProofArtifactHash,
    string? ProvingKeyHash,
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
    private const int BeaconRestMaxResponseBytes = 1024 * 1024;

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
        using var response = await httpClient
            .SendAsync(request, HttpCompletionOption.ResponseHeadersRead, cancellationToken)
            .ConfigureAwait(false);
        if (response.Content.Headers.ContentLength is > BeaconRestMaxResponseBytes)
        {
            throw new ArgumentException(
                $"Ethereum mainnet Beacon REST response body must be at most {BeaconRestMaxResponseBytes} bytes");
        }
        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken).ConfigureAwait(false);
        var body = await ReadBodyAsync(stream, cancellationToken).ConfigureAwait(false);
        return new EthereumMainnetBeaconRestResponse(
            (int)response.StatusCode,
            body,
            response.ReasonPhrase);
    }

    private static async ValueTask<byte[]> ReadBodyAsync(Stream stream, CancellationToken cancellationToken)
    {
        using var outStream = new MemoryStream();
        var buffer = new byte[8192];
        while (true)
        {
            var read = await stream.ReadAsync(buffer.AsMemory(0, buffer.Length), cancellationToken)
                .ConfigureAwait(false);
            if (read == 0)
            {
                break;
            }
            if (outStream.Length + read > BeaconRestMaxResponseBytes)
            {
                throw new ArgumentException(
                    $"Ethereum mainnet Beacon REST response body must be at most {BeaconRestMaxResponseBytes} bytes");
            }
            outStream.Write(buffer, 0, read);
        }
        return outStream.ToArray();
    }
}

public sealed class EthereumMainnetBeaconRestConsensusProvider : IEthereumMainnetConsensusProvider
{
    private const int BeaconRestMaxResponseBytes = 1024 * 1024;
    private const ulong EthereumMainnetSecondsPerSlot = 12;

    private readonly record struct BeaconRestHeaderSummary(string Root, ulong Slot);

    private readonly record struct BeaconRestBlockId(string Id, ulong? Slot = null, string? Root = null);

    private readonly record struct BeaconRestFinalityUpdateSummary(
        string FinalizedHeaderRoot,
        ulong BeaconSlot,
        IReadOnlyList<string> FinalityBranch,
        string SyncCommitteeBits,
        string SyncCommitteeSignature,
        ulong SyncCommitteeParticipation,
        ulong SyncSignatureSlot);

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
            StrictFirstPresent(block, "block.number", "number", "blockNumber", "block_number"),
            "block.number");
        if (blockNumber == "0x0")
        {
            throw new ArgumentException("block.number must be positive", nameof(block));
        }
        var receiptsRoot = NormalizeRpcHex(
            StrictFirstPresent(block, "block.receiptsRoot", "receiptsRoot", "receipts_root"),
            "block.receiptsRoot",
            32);
        var targetBlockId = await BeaconRestBlockIdForTargetAsync(
            block,
            cancellationToken).ConfigureAwait(false);

        var finalizedHeader = await FetchHeaderSummaryAsync(
            "/eth/v1/beacon/headers/finalized",
            "Ethereum mainnet Beacon REST finalized header",
            cancellationToken).ConfigureAwait(false);
        var targetHeader = targetBlockId.Id == "finalized"
            ? finalizedHeader
            : await FetchHeaderSummaryAsync(
                $"/eth/v1/beacon/headers/{targetBlockId.Id}",
                "Ethereum mainnet Beacon REST finalized target header",
                cancellationToken).ConfigureAwait(false);
        if (targetBlockId.Slot is { } expectedSlot && targetHeader.Slot != expectedSlot)
        {
            throw new ArgumentException(
                "Ethereum mainnet Beacon REST finalized target header slot must match beaconSlot");
        }
        if (targetBlockId.Root is { } expectedRoot
            && !string.Equals(targetHeader.Root, expectedRoot, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "Ethereum mainnet Beacon REST finalized target header root must match beaconBlockRoot");
        }
        if (targetHeader.Slot > finalizedHeader.Slot)
        {
            throw new ArgumentException(
                "Ethereum mainnet Beacon REST target block is newer than the finalized header");
        }
        if (targetHeader.Slot < finalizedHeader.Slot)
        {
            throw new ArgumentException(
                "Ethereum mainnet Beacon REST historical target blocks require an ancestry proof");
        }
        if (targetHeader.Slot == finalizedHeader.Slot
            && !string.Equals(targetHeader.Root, finalizedHeader.Root, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "Ethereum mainnet Beacon REST target header root must match finalized header root at the same slot");
        }

        using var finalizedBlockRootDocument = await FetchJsonDocumentAsync(
            $"/eth/v1/beacon/blocks/{targetBlockId.Id}/root",
            "Ethereum mainnet Beacon REST finalized block root",
            cancellationToken).ConfigureAwait(false);
        var finalizedBlockRootResponse = finalizedBlockRootDocument.RootElement;
        RejectUnsafeBeaconRestPayload(
            finalizedBlockRootResponse,
            "Ethereum mainnet Beacon REST finalized block root");
        var finalizedBlockRootData = RequireObject(
            RequireProperty(
                finalizedBlockRootResponse,
                "Ethereum mainnet Beacon REST finalized block root",
                "data"),
            "Ethereum mainnet Beacon REST finalized block root.data");
        var finalizedBlockRootHash = NormalizeRpcHex(
            RequireString(
                RequireProperty(
                    finalizedBlockRootData,
                    "Ethereum mainnet Beacon REST finalized block root.data",
                    "root"),
                "finalizedBlockRoot"),
            "finalizedBlockRoot",
            32);
        if (!string.Equals(finalizedBlockRootHash, targetHeader.Root, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "Ethereum mainnet Beacon REST finalized block root must match finalized header root");
        }

        using var finalizedBlockDocument = await FetchJsonDocumentAsync(
            $"/eth/v2/beacon/blocks/{targetBlockId.Id}",
            "Ethereum mainnet Beacon REST finalized block",
            cancellationToken).ConfigureAwait(false);
        var finalizedBlockRoot = finalizedBlockDocument.RootElement;
        RejectUnsafeBeaconRestPayload(
            finalizedBlockRoot,
            "Ethereum mainnet Beacon REST finalized block");
        var finalizedBlockData = RequireObject(
            RequireProperty(
                finalizedBlockRoot,
                "Ethereum mainnet Beacon REST finalized block",
                "data"),
            "Ethereum mainnet Beacon REST finalized block.data");
        var finalizedBlockMessage = RequireObject(
            RequireProperty(
                finalizedBlockData,
                "Ethereum mainnet Beacon REST finalized block.data",
                "message"),
            "Ethereum mainnet Beacon REST finalized block.data.message");
        var finalizedBlockSlot = NormalizeUnsignedInteger(
            RequireString(
                RequireProperty(
                    finalizedBlockMessage,
                    "Ethereum mainnet Beacon REST finalized block.data.message",
                    "slot"),
                "Ethereum mainnet Beacon REST finalized block.data.message.slot"),
            "Ethereum mainnet Beacon REST finalized block.data.message.slot");
        if (finalizedBlockSlot != targetHeader.Slot)
        {
            throw new ArgumentException(
                "Ethereum mainnet Beacon REST finalized block slot must match finalized header slot");
        }
        var finalizedBlockBody = RequireObject(
            RequireProperty(
                finalizedBlockMessage,
                "Ethereum mainnet Beacon REST finalized block.data.message",
                "body"),
            "Ethereum mainnet Beacon REST finalized block.data.message.body");
        var executionPayload = RequireObject(
            RequireProperty(
                finalizedBlockBody,
                "Ethereum mainnet Beacon REST finalized block.data.message.body",
                "execution_payload"),
            "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload");
        var payloadBlockHash = NormalizeRpcHex(
            RequireString(
                RequireProperty(
                    executionPayload,
                    "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload",
                    "block_hash"),
                "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload.block_hash"),
            "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload.block_hash",
            32);
        if (!string.Equals(payloadBlockHash, blockHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "Ethereum mainnet Beacon REST execution payload block_hash must match block.hash");
        }
        var payloadBlockNumber = NormalizeUnsignedInteger(
            RequireString(
                RequireProperty(
                    executionPayload,
                    "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload",
                    "block_number"),
                "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload.block_number"),
            "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload.block_number");
        if (payloadBlockNumber != NormalizeUnsignedInteger(blockNumber, "block.number"))
        {
            throw new ArgumentException(
                "Ethereum mainnet Beacon REST execution payload block_number must match block.number");
        }
        var payloadReceiptsRoot = NormalizeRpcHex(
            RequireString(
                RequireProperty(
                    executionPayload,
                    "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload",
                    "receipts_root"),
                "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload.receipts_root"),
            "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload.receipts_root",
            32);
        if (!string.Equals(payloadReceiptsRoot, receiptsRoot, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "Ethereum mainnet Beacon REST execution payload receipts_root must match block.receiptsRoot");
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
            if (!string.Equals(finalizedCheckpointRoot, finalizedHeader.Root, StringComparison.Ordinal))
            {
                throw new ArgumentException(
                    "Ethereum mainnet Beacon REST finality checkpoint root must match finalized header root");
            }
        }

        var finalityUpdate = await FetchFinalityUpdateSummaryAsync(
            finalizedHeader.Slot,
            finalizedHeader.Root,
            cancellationToken).ConfigureAwait(false);

        return new Dictionary<string, object?>(StringComparer.Ordinal)
        {
            ["executionBlockNumber"] = NormalizeUnsignedInteger(blockNumber, "block.number").ToString(),
            ["executionBlockHash"] = blockHash,
            ["executionReceiptsRoot"] = receiptsRoot,
            ["finalizedHeaderRoot"] = finalityUpdate.FinalizedHeaderRoot,
            ["syncCommitteeRoot"] = syncCommitteeRoot,
            ["beaconSlot"] = finalityUpdate.BeaconSlot.ToString(),
            ["finalityBranch"] = finalityUpdate.FinalityBranch,
            ["syncCommitteeBits"] = finalityUpdate.SyncCommitteeBits,
            ["syncCommitteeSignature"] = finalityUpdate.SyncCommitteeSignature,
            ["syncCommitteeParticipation"] = finalityUpdate.SyncCommitteeParticipation.ToString(),
            ["syncSignatureSlot"] = finalityUpdate.SyncSignatureSlot.ToString(),
        };
    }

    private async ValueTask<BeaconRestBlockId> BeaconRestBlockIdForTargetAsync(
        IReadOnlyDictionary<string, object?> block,
        CancellationToken cancellationToken)
    {
        if (FirstPresent(
                block,
                "beaconBlockRoot",
                "beacon_block_root",
                "targetBeaconBlockRoot",
                "target_beacon_block_root") is { } rootInput)
        {
            var root = NormalizeRpcHex(rootInput, "block.beaconBlockRoot", 32);
            return new BeaconRestBlockId(root, Root: root);
        }
        if (FirstPresent(
                block,
                "beaconBlockId",
                "beacon_block_id",
                "targetBeaconBlockId",
                "target_beacon_block_id") is { } idInput)
        {
            return BeaconRestBlockIdFromValue(idInput, "block.beaconBlockId");
        }
        if (FirstPresent(block, "beaconSlot", "beacon_slot", "finalizedSlot", "finalized_slot", "slot")
            is { } slotInput)
        {
            var slot = NormalizeBeaconSlot(slotInput, "block.beaconSlot");
            return new BeaconRestBlockId(slot.ToString(), Slot: slot);
        }
        if (FirstPresent(block, "timestamp", "blockTimestamp", "block_timestamp") is { } timestampInput)
        {
            var timestamp = NormalizeUnsignedInteger(timestampInput, "block.timestamp");
            var genesisTime = await BeaconRestGenesisTimeAsync(cancellationToken).ConfigureAwait(false);
            if (timestamp < genesisTime)
            {
                throw new ArgumentException("block.timestamp must not be before Beacon genesis time");
            }
            var elapsed = timestamp - genesisTime;
            if (elapsed % EthereumMainnetSecondsPerSlot != 0)
            {
                throw new ArgumentException(
                    "block.timestamp must align to an Ethereum mainnet Beacon slot");
            }
            var slot = elapsed / EthereumMainnetSecondsPerSlot;
            if (slot == 0)
            {
                throw new ArgumentException("beaconFinality.beaconSlot must be positive");
            }
            return new BeaconRestBlockId(slot.ToString(), Slot: slot);
        }
        return new BeaconRestBlockId("finalized");
    }

    private async ValueTask<ulong> BeaconRestGenesisTimeAsync(CancellationToken cancellationToken)
    {
        using var genesisDocument = await FetchJsonDocumentAsync(
            "/eth/v1/beacon/genesis",
            "Ethereum mainnet Beacon REST genesis",
            cancellationToken).ConfigureAwait(false);
        var data = RequireObject(
            RequireProperty(
                genesisDocument.RootElement,
                "Ethereum mainnet Beacon REST genesis",
                "data"),
            "Ethereum mainnet Beacon REST genesis.data");
        return NormalizeUnsignedInteger(
            RequireString(
                RequireProperty(
                    data,
                    "Ethereum mainnet Beacon REST genesis.data",
                    "genesis_time"),
                "Ethereum mainnet Beacon REST genesis.data.genesis_time"),
            "Ethereum mainnet Beacon REST genesis.data.genesis_time");
    }

    private async ValueTask<BeaconRestHeaderSummary> FetchHeaderSummaryAsync(
        string path,
        string label,
        CancellationToken cancellationToken)
    {
        using var document = await FetchJsonDocumentAsync(path, label, cancellationToken).ConfigureAwait(false);
        return BeaconRestHeaderSummaryFromPayload(document.RootElement, label);
    }

    private static BeaconRestHeaderSummary BeaconRestHeaderSummaryFromPayload(
        JsonElement payload,
        string label)
    {
        RejectUnsafeBeaconRestPayload(payload, label);
        var headerData = RequireObject(
            RequireProperty(payload, label, "data"),
            $"{label}.data");
        RejectNonBooleanBeaconRestCanonical(headerData, label);
        var rootLabel = label.Contains("target", StringComparison.Ordinal)
            ? "targetHeaderRoot"
            : "finalizedHeaderRoot";
        var root = NormalizeRpcHex(
            RequireString(
                RequireProperty(headerData, $"{label}.data", "root"),
                rootLabel),
            rootLabel,
            32);
        var header = RequireObject(
            RequireProperty(headerData, $"{label}.data", "header"),
            $"{label}.data.header");
        var message = RequireObject(
            RequireProperty(header, $"{label}.data.header", "message"),
            $"{label}.data.header.message");
        foreach (var field in new[] { "parent_root", "state_root", "body_root" })
        {
            NormalizeRpcHex(
                RequireString(
                    RequireProperty(message, $"{label}.data.header.message", field),
                    $"{label}.data.header.message.{field}"),
                $"{label}.data.header.message.{field}",
                32);
        }
        NormalizeRpcHex(
            RequireString(
                RequireProperty(header, $"{label}.data.header", "signature"),
                $"{label}.data.header.signature"),
            $"{label}.data.header.signature",
            96);
        var slot = NormalizeBeaconSlot(
            RequireString(
                RequireProperty(message, $"{label}.data.header.message", "slot"),
                "beaconFinality.beaconSlot"),
            "beaconFinality.beaconSlot");
        return new BeaconRestHeaderSummary(root, slot);
    }

    private async ValueTask<BeaconRestFinalityUpdateSummary> FetchFinalityUpdateSummaryAsync(
        ulong expectedFinalizedSlot,
        string expectedFinalizedRoot,
        CancellationToken cancellationToken)
    {
        using var document = await FetchJsonDocumentAsync(
            "/eth/v1/beacon/light_client/finality_update",
            "Ethereum mainnet Beacon REST light-client finality update",
            cancellationToken).ConfigureAwait(false);
        return BeaconRestFinalityUpdateSummaryFromPayload(
            document.RootElement,
            expectedFinalizedSlot,
            expectedFinalizedRoot);
    }

    private static BeaconRestFinalityUpdateSummary BeaconRestFinalityUpdateSummaryFromPayload(
        JsonElement payload,
        ulong expectedFinalizedSlot,
        string expectedFinalizedRoot)
    {
        const string Label = "Ethereum mainnet Beacon REST light-client finality update";
        RejectUnsafeBeaconRestPayload(payload, Label);
        var data = RequireObject(RequireProperty(payload, Label, "data"), $"{Label}.data");
        var finalizedHeader = RequireObject(
            RequireProperty(data, $"{Label}.data", "finalized_header"),
            $"{Label}.data.finalized_header");
        var finalizedBeacon = RequireObject(
            RequireProperty(finalizedHeader, $"{Label}.data.finalized_header", "beacon"),
            $"{Label}.data.finalized_header.beacon");
        var finalizedSlot = NormalizeBeaconSlot(
            RequireString(
                RequireProperty(finalizedBeacon, $"{Label}.data.finalized_header.beacon", "slot"),
                $"{Label}.data.finalized_header.beacon.slot"),
            $"{Label}.data.finalized_header.beacon.slot");
        if (finalizedSlot != expectedFinalizedSlot)
        {
            throw new ArgumentException(
                "Ethereum mainnet Beacon REST finality update finalized_header slot must match finalized header slot");
        }
        var finalizedHeaderRoot = BeaconBlockHeaderRoot(
            finalizedSlot,
            NormalizeUnsignedInteger(
                RequireString(
                    RequireProperty(finalizedBeacon, $"{Label}.data.finalized_header.beacon", "proposer_index"),
                    $"{Label}.data.finalized_header.beacon.proposer_index"),
                $"{Label}.data.finalized_header.beacon.proposer_index"),
            NormalizeRpcHex(
                RequireString(
                    RequireProperty(finalizedBeacon, $"{Label}.data.finalized_header.beacon", "parent_root"),
                    $"{Label}.data.finalized_header.beacon.parent_root"),
                $"{Label}.data.finalized_header.beacon.parent_root",
                32),
            NormalizeRpcHex(
                RequireString(
                    RequireProperty(finalizedBeacon, $"{Label}.data.finalized_header.beacon", "state_root"),
                    $"{Label}.data.finalized_header.beacon.state_root"),
                $"{Label}.data.finalized_header.beacon.state_root",
                32),
            NormalizeRpcHex(
                RequireString(
                    RequireProperty(finalizedBeacon, $"{Label}.data.finalized_header.beacon", "body_root"),
                    $"{Label}.data.finalized_header.beacon.body_root"),
                $"{Label}.data.finalized_header.beacon.body_root",
                32));
        if (!string.Equals(finalizedHeaderRoot, expectedFinalizedRoot, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "Ethereum mainnet Beacon REST finality update finalized_header root must match finalized header root");
        }
        var signatureSlot = NormalizeBeaconSlot(
            RequireString(RequireProperty(data, $"{Label}.data", "signature_slot"), $"{Label}.data.signature_slot"),
            $"{Label}.data.signature_slot");
        if (signatureSlot < expectedFinalizedSlot)
        {
            throw new ArgumentException(
                "Ethereum mainnet Beacon REST finality update signature_slot must cover finalized header slot");
        }
        var syncAggregate = RequireObject(
            RequireProperty(data, $"{Label}.data", "sync_aggregate"),
            $"{Label}.data.sync_aggregate");
        var syncCommitteeBits = NormalizeSyncCommitteeBits(
            RequireString(
                RequireProperty(syncAggregate, $"{Label}.data.sync_aggregate", "sync_committee_bits"),
                $"{Label}.data.sync_aggregate.sync_committee_bits"),
            $"{Label}.data.sync_aggregate.sync_committee_bits");
        var finalityBranch = NormalizeFinalityBranch(
            RequireProperty(data, $"{Label}.data", "finality_branch"),
            $"{Label}.data.finality_branch");
        var syncCommitteeSignature = NormalizeRpcHex(
            RequireString(
                RequireProperty(syncAggregate, $"{Label}.data.sync_aggregate", "sync_committee_signature"),
                $"{Label}.data.sync_aggregate.sync_committee_signature"),
            $"{Label}.data.sync_aggregate.sync_committee_signature",
                96);
        return new BeaconRestFinalityUpdateSummary(
            finalizedHeaderRoot,
            finalizedSlot,
            finalityBranch,
            syncCommitteeBits,
            syncCommitteeSignature,
            SyncCommitteeParticipation(syncCommitteeBits),
            signatureSlot);
    }

    private static BeaconRestBlockId BeaconRestBlockIdFromValue(object? value, string label)
    {
        if (value is string text
            && text.Trim() == text
            && text.StartsWith("0x", StringComparison.Ordinal)
            && text.Length == 66)
        {
            var root = NormalizeRpcHex(text, label, 32);
            return new BeaconRestBlockId(root, Root: root);
        }
        var slot = NormalizeBeaconSlot(value, label);
        return new BeaconRestBlockId(slot.ToString(), Slot: slot);
    }

    private static ulong NormalizeBeaconSlot(object? value, string label)
    {
        var slot = NormalizeUnsignedInteger(value, label);
        if (slot == 0)
        {
            throw new ArgumentException("beaconFinality.beaconSlot must be positive");
        }
        return slot;
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
        if (response.Body.Length > BeaconRestMaxResponseBytes)
        {
            throw new ArgumentException(
                $"{label} response body must be at most {BeaconRestMaxResponseBytes} bytes");
        }
        try
        {
            var document = JsonDocument.Parse(response.Body);
            if (document.RootElement.ValueKind != JsonValueKind.Object)
            {
                document.Dispose();
                throw new ArgumentException($"{label} response JSON must be an object");
            }
            return document;
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
        var apiPath =
            System.Text.RegularExpressions.Regex.IsMatch(basePath, "/eth/v[0-9]+$")
            && System.Text.RegularExpressions.Regex.IsMatch(path, "^/eth/v[0-9]+/")
                ? System.Text.RegularExpressions.Regex.Replace(basePath, "/eth/v[0-9]+$", string.Empty) + path
                : basePath + path;
        builder.Path = apiPath;
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
        var optimistic = OptionalBeaconRestBoolean(payload, "execution_optimistic", label);
        var optimisticAlias = OptionalBeaconRestBoolean(payload, "executionOptimistic", label);
        var finalized = OptionalBeaconRestBoolean(payload, "finalized", label);
        if (optimistic == true || optimisticAlias == true)
        {
            throw new ArgumentException($"{label} must not be execution optimistic");
        }
        if (finalized == false)
        {
            throw new ArgumentException($"{label} must be finalized");
        }
    }

    private static void RejectNonBooleanBeaconRestCanonical(JsonElement payload, string label)
    {
        if (OptionalBeaconRestBoolean(payload, "canonical", label) == false)
        {
            throw new ArgumentException($"{label} must be canonical");
        }
    }

    private static bool? OptionalBeaconRestBoolean(JsonElement payload, string field, string label)
    {
        if (!payload.TryGetProperty(field, out var value))
        {
            return null;
        }
        return value.ValueKind switch
        {
            JsonValueKind.True => true,
            JsonValueKind.False => false,
            _ => throw new ArgumentException($"{label}.{field} must be a boolean"),
        };
    }

    private static object RequiredBlockValue(IReadOnlyDictionary<string, object?> block, string key)
    {
        if (!block.TryGetValue(key, out var value) || value is null)
        {
            throw new ArgumentException($"block.{key} is required", nameof(block));
        }
        return value;
    }

    private static object? FirstPresent(IReadOnlyDictionary<string, object?> value, params string[] names)
    {
        foreach (var name in names)
        {
            if (value.TryGetValue(name, out var item) && item is not null)
            {
                return item;
            }
        }
        return null;
    }

    private static object? StrictFirstPresent(
        IReadOnlyDictionary<string, object?> value,
        string parameterName,
        params string[] names)
    {
        object? selected = null;
        var found = false;
        foreach (var name in names)
        {
            if (value.TryGetValue(name, out var item))
            {
                if (found)
                {
                    throw new ArgumentException(
                        $"{parameterName} must not use multiple aliases.",
                        parameterName);
                }

                selected = item;
                found = true;
            }
        }

        if (!found)
        {
            throw new ArgumentException($"{parameterName} is required.", parameterName);
        }

        return selected;
    }

    private static string NormalizeRpcHex(
        object? value,
        string parameterName,
        int byteLength,
        bool allowZero = false)
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
        if (!allowZero && hex.All(static ch => ch == '0'))
        {
            throw new ArgumentException($"{parameterName} must not be zero", parameterName);
        }
        return text;
    }

    private static string NormalizeSyncCommitteeBits(object? value, string parameterName)
    {
        var bits = NormalizeRpcHex(value, parameterName, 64, allowZero: true);
        var participation = SyncCommitteeParticipation(bits);
        if (participation == 0)
        {
            throw new ArgumentException($"{parameterName} must contain at least one participant", parameterName);
        }
        if (participation * 3 < 512 * 2)
        {
            throw new ArgumentException(
                $"{parameterName} must contain Ethereum sync committee supermajority",
                parameterName);
        }
        return bits;
    }

    private static IReadOnlyList<string> NormalizeFinalityBranch(JsonElement value, string parameterName)
    {
        if (value.ValueKind != JsonValueKind.Array)
        {
            throw new ArgumentException($"{parameterName} must be an array", parameterName);
        }
        var branch = value.EnumerateArray()
            .Select((sibling, index) => NormalizeRpcHex(
                RequireString(sibling, $"{parameterName}[{index}]"),
                $"{parameterName}[{index}]",
                32,
                allowZero: true))
            .ToArray();
        if (branch.Length != 6)
        {
            throw new ArgumentException($"{parameterName} must contain 6 siblings", parameterName);
        }
        return Array.AsReadOnly(branch);
    }

    private static ulong SyncCommitteeParticipation(string bits)
    {
        var hex = bits[2..];
        ulong count = 0;
        for (var index = 0; index < hex.Length; index += 2)
        {
            var value = Convert.ToByte(hex.Substring(index, 2), 16);
            while (value != 0)
            {
                count += (ulong)(value & 1);
                value >>= 1;
            }
        }
        return count;
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

    private static string BeaconBlockHeaderRoot(
        ulong slot,
        ulong proposerIndex,
        string parentRoot,
        string stateRoot,
        string bodyRoot)
        => "0x" + Convert.ToHexString(SszMerkleizeChunks([
            SszU64Chunk(slot),
            SszU64Chunk(proposerIndex),
            BeaconRestHexBytes(parentRoot),
            BeaconRestHexBytes(stateRoot),
            BeaconRestHexBytes(bodyRoot),
        ])).ToLowerInvariant();

    private static byte[] BeaconRestHexBytes(string normalizedHex)
        => Convert.FromHexString(normalizedHex[2..]);

    private static byte[] SszU64Chunk(ulong value)
    {
        var chunk = new byte[32];
        BinaryPrimitives.WriteUInt64LittleEndian(chunk.AsSpan(0, 8), value);
        return chunk;
    }

    private static byte[] SszMerkleizeChunks(IReadOnlyList<byte[]> inputChunks)
    {
        if (inputChunks.Count == 0 || inputChunks.Any(static chunk => chunk.Length != 32))
        {
            throw new ArgumentException("SSZ chunks must be non-empty 32-byte values.");
        }

        var chunks = inputChunks.Select(static chunk => chunk.ToArray()).ToList();
        var width = 1;
        while (width < chunks.Count)
        {
            width <<= 1;
        }
        while (chunks.Count < width)
        {
            chunks.Add(new byte[32]);
        }

        while (chunks.Count > 1)
        {
            var next = new List<byte[]>(chunks.Count / 2);
            for (var index = 0; index < chunks.Count; index += 2)
            {
                var pair = new byte[64];
                chunks[index].CopyTo(pair.AsSpan(0, 32));
                chunks[index + 1].CopyTo(pair.AsSpan(32, 32));
                next.Add(SHA256.HashData(pair));
            }
            chunks = next;
        }

        return chunks[0];
    }
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

public interface IEthereumMainnetNativeProverSelfTest
{
    ValueTask<EthereumMainnetNativeEvmProverSelfTestSdkResult> RunAsync(
        EthereumMainnetNativeEvmProverSelfTestFixture fixture,
        EthereumMainnetNativeEvmProverSelfTestSdkResult expectedResult,
        EthereumMainnetNativeEvmProverArtifacts artifacts,
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
    string? BeaconSlot = null,
    string? SyncCommitteeBits = null,
    string? SyncCommitteeSignature = null,
    string? SyncCommitteeParticipation = null,
    string? SyncSignatureSlot = null)
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
        if (SyncCommitteeBits is not null)
        {
            value["syncCommitteeBits"] = SyncCommitteeBits;
        }
        if (SyncCommitteeSignature is not null)
        {
            value["syncCommitteeSignature"] = SyncCommitteeSignature;
        }
        if (SyncCommitteeParticipation is not null)
        {
            value["syncCommitteeParticipation"] = SyncCommitteeParticipation;
        }
        if (SyncSignatureSlot is not null)
        {
            value["syncSignatureSlot"] = SyncSignatureSlot;
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
