package org.hyperledger.iroha.android.sccp;

import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.math.BigInteger;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.JsonParser;

/** Ethereum mainnet SCCP Groth16 proof request helpers for local-first Android proof generation. */
public final class EthereumMainnetSccp {
  public static final int DOMAIN_SORA = EvmSccpProver.DOMAIN_SORA;
  public static final int DOMAIN_ETH = EvmSccpProver.DOMAIN_ETH;
  public static final long MAINNET_CHAIN_ID = SourceSccpProofs.ETH_MAINNET_CHAIN_ID;
  public static final String MAINNET_NETWORK_ID = SourceSccpProofs.ETH_MAINNET_NETWORK_ID;
  public static final String LOCAL_ADMISSION_ENVELOPE_ENCODING_V1 =
      "norito:sccp-local-admission:v1";
  public static final String LOCAL_ADMISSION_SUBMISSION_KIND_V1 = "local_admission";
  public static final String LOCAL_ADMISSION_ENTRYPOINT_V1 = "SubmitBridgeProof";
  public static final String STARK_FRI_PROOF_FAMILY_V1 = "stark-fri-v1";
  public static final String SOURCE_EVENT_ABI_V1 = "SccpSourceEvent(bytes32)";
  public static final String SOURCE_EVENT_TOPIC_V1 =
      "0x577b41c65ffbce226de59f224b464797257063747891b88ebec1bcd57af82727";
  public static final int NATIVE_RECURSIVE_MAX_PROOF_BYTES = 2 * 1024 * 1024;
  private static final int BEACON_REST_MAX_RESPONSE_BYTES = 1024 * 1024;
  private static final long ETHEREUM_MAINNET_SECONDS_PER_SLOT = 12L;
  private static final List<String> BEACON_FINALITY_ALIAS_KEYS =
      Arrays.asList(
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
          "sync_committee_participation");

  private static final class BeaconRestHeaderSummary {
    final String root;
    final long slot;

    BeaconRestHeaderSummary(final String root, final long slot) {
      this.root = root;
      this.slot = slot;
    }
  }

  private static final class BeaconRestBlockId {
    final String id;
    final Long slot;
    final String root;

    BeaconRestBlockId(final String id, final Long slot, final String root) {
      this.id = id;
      this.slot = slot;
      this.root = root;
    }
  }

  private static final class BeaconRestFinalityUpdateSummary {
    final List<String> finalityBranch;
    final String syncCommitteeBits;
    final String syncCommitteeSignature;
    final long syncCommitteeParticipation;
    final long syncSignatureSlot;

    BeaconRestFinalityUpdateSummary(
        final List<String> finalityBranch,
        final String syncCommitteeBits,
        final String syncCommitteeSignature,
        final long syncCommitteeParticipation,
        final long syncSignatureSlot) {
      this.finalityBranch = finalityBranch;
      this.syncCommitteeBits = syncCommitteeBits;
      this.syncCommitteeSignature = syncCommitteeSignature;
      this.syncCommitteeParticipation = syncCommitteeParticipation;
      this.syncSignatureSlot = syncSignatureSlot;
    }
  }

  private final EvmSccpProver.WitnessProvider witnessProvider;
  private final EvmSccpProver.ProofEngine proofEngine;
  private final ExecutionProvider executionProvider;
  private final ConsensusProvider consensusProvider;
  private final InboundProver inboundProver;
  private final InboundSubmitter inboundSubmitter;
  private final OutboundSubmitter outboundSubmitter;
  private final EvmSccpProver.EthereumMainnetNativeEvmProverBundle nativeProverBundle;
  private final EvmSccpProver.EthereumMainnetNativeEvmProverArtifacts nativeProverArtifacts;
  private final String sourceBridgeEmitterAddress;

  public EthereumMainnetSccp() {
    this(null, null, null, null, null);
  }

  public EthereumMainnetSccp(
      final EvmSccpProver.EthereumMainnetNativeEvmProverBundle nativeProverBundle) {
    this(null, null, null, null, null, null, null, nativeProverBundle, null);
  }

  public EthereumMainnetSccp(
      final EvmSccpProver.EthereumMainnetNativeEvmProverArtifacts nativeProverArtifacts) {
    this(null, null, null, null, null, null, null, null, nativeProverArtifacts, null);
  }

  public EthereumMainnetSccp(
      final EvmSccpProver.WitnessProvider witnessProvider,
      final EvmSccpProver.ProofEngine proofEngine) {
    this(witnessProvider, proofEngine, null, null, null);
  }

  public EthereumMainnetSccp(
      final EvmSccpProver.WitnessProvider witnessProvider,
      final EvmSccpProver.ProofEngine proofEngine,
      final ExecutionProvider executionProvider,
      final InboundProver inboundProver,
      final InboundSubmitter inboundSubmitter) {
    this(witnessProvider, proofEngine, executionProvider, null, inboundProver, inboundSubmitter);
  }

  public EthereumMainnetSccp(
      final EvmSccpProver.WitnessProvider witnessProvider,
      final EvmSccpProver.ProofEngine proofEngine,
      final ExecutionProvider executionProvider,
      final ConsensusProvider consensusProvider,
      final InboundProver inboundProver,
      final InboundSubmitter inboundSubmitter) {
    this(
        witnessProvider,
        proofEngine,
        executionProvider,
        consensusProvider,
        inboundProver,
        inboundSubmitter,
        null);
  }

  public EthereumMainnetSccp(
      final EvmSccpProver.WitnessProvider witnessProvider,
      final EvmSccpProver.ProofEngine proofEngine,
      final ExecutionProvider executionProvider,
      final ConsensusProvider consensusProvider,
      final InboundProver inboundProver,
      final InboundSubmitter inboundSubmitter,
      final OutboundSubmitter outboundSubmitter) {
    this(
        witnessProvider,
        proofEngine,
        executionProvider,
        consensusProvider,
        inboundProver,
        inboundSubmitter,
        outboundSubmitter,
        null);
  }

  public EthereumMainnetSccp(
      final EvmSccpProver.WitnessProvider witnessProvider,
      final EvmSccpProver.ProofEngine proofEngine,
      final ExecutionProvider executionProvider,
      final ConsensusProvider consensusProvider,
      final InboundProver inboundProver,
      final InboundSubmitter inboundSubmitter,
      final OutboundSubmitter outboundSubmitter,
      final String sourceBridgeEmitterAddress) {
    this(
        witnessProvider,
        proofEngine,
        executionProvider,
        consensusProvider,
        inboundProver,
        inboundSubmitter,
        outboundSubmitter,
        null,
        sourceBridgeEmitterAddress);
  }

  public EthereumMainnetSccp(
      final EvmSccpProver.WitnessProvider witnessProvider,
      final EvmSccpProver.ProofEngine proofEngine,
      final ExecutionProvider executionProvider,
      final ConsensusProvider consensusProvider,
      final InboundProver inboundProver,
      final InboundSubmitter inboundSubmitter,
      final OutboundSubmitter outboundSubmitter,
      final EvmSccpProver.EthereumMainnetNativeEvmProverBundle nativeProverBundle,
      final String sourceBridgeEmitterAddress) {
    this(
        witnessProvider,
        proofEngine,
        executionProvider,
        consensusProvider,
        inboundProver,
        inboundSubmitter,
        outboundSubmitter,
        nativeProverBundle,
        null,
        sourceBridgeEmitterAddress);
  }

  public EthereumMainnetSccp(
      final EvmSccpProver.WitnessProvider witnessProvider,
      final EvmSccpProver.ProofEngine proofEngine,
      final ExecutionProvider executionProvider,
      final ConsensusProvider consensusProvider,
      final InboundProver inboundProver,
      final InboundSubmitter inboundSubmitter,
      final OutboundSubmitter outboundSubmitter,
      final EvmSccpProver.EthereumMainnetNativeEvmProverBundle nativeProverBundle,
      final EvmSccpProver.EthereumMainnetNativeEvmProverArtifacts nativeProverArtifacts,
      final String sourceBridgeEmitterAddress) {
    this.witnessProvider = witnessProvider;
    this.proofEngine = proofEngine;
    this.executionProvider = executionProvider;
    this.consensusProvider = consensusProvider;
    this.inboundProver = inboundProver;
    this.inboundSubmitter = inboundSubmitter;
    this.outboundSubmitter = outboundSubmitter;
    this.nativeProverArtifacts = nativeProverArtifacts;
    this.nativeProverBundle =
        nativeProverArtifacts == null ? nativeProverBundle : nativeProverArtifacts.nativeProverBundle();
    this.sourceBridgeEmitterAddress = sourceBridgeEmitterAddress;
  }

  public static void requireMainnetChainId(final long chainId) {
    if (chainId != MAINNET_CHAIN_ID) {
      throw new IllegalArgumentException("Ethereum mainnet SCCP requires eth_chainId == 1");
    }
  }

  public static String sourceEventTopic() {
    return SOURCE_EVENT_TOPIC_V1;
  }

  public Object validateExecutionProviderMainnet() {
    return validateExecutionProviderMainnet(executionProvider);
  }

  public Object validateExecutionProviderMainnet(final ExecutionProvider provider) {
    final ExecutionProvider selectedProvider =
        Objects.requireNonNull(provider, "executionProvider");
    final Object chainId = selectedProvider.request("eth_chainId", Collections.emptyList());
    requireMainnetChainId(normalizeRpcChainId(chainId));
    return chainId;
  }

  public InboundEvidence collectInboundEvidenceFromReceipt(final InboundEvidence input) {
    return collectInboundEvidenceFromReceipt(input, executionProvider, consensusProvider);
  }

  public InboundEvidence collectInboundEvidenceFromReceipt(
      final InboundEvidence input, final ExecutionProvider provider) {
    return collectInboundEvidenceFromReceipt(input, provider, consensusProvider);
  }

  public InboundEvidence collectInboundEvidenceFromReceipt(
      final InboundEvidence input,
      final ExecutionProvider provider,
      final ConsensusProvider finalityProvider) {
    Objects.requireNonNull(input, "input");
    if (input.sourceDomain() != DOMAIN_ETH) {
      throw new IllegalArgumentException(
          "Ethereum mainnet inbound evidence sourceDomain must be ETH");
    }
    if (input.targetDomain() != DOMAIN_SORA) {
      throw new IllegalArgumentException(
          "Ethereum mainnet inbound evidence targetDomain must be SORA");
    }
    if (provider != null) {
      validateExecutionProviderMainnet(provider);
    }

    String transactionHash =
        input.transactionHash() == null
            ? null
            : normalizeRpcHex(input.transactionHash(), "transactionHash", 32);
    Map<String, Object> receipt = input.receipt();
    if (receipt == null && transactionHash != null) {
      if (provider == null) {
        throw new IllegalStateException(
            "Ethereum mainnet execution provider is not linked for transactionHash evidence collection");
      }
      receipt =
          requireMap(
              provider.request(
                  "eth_getTransactionReceipt", Collections.<Object>singletonList(transactionHash)),
              "eth_getTransactionReceipt");
    }
    if (receipt == null && input.receiptProof() == null && input.receiptProofHash() == null) {
      throw new IllegalArgumentException(
          "Ethereum mainnet inbound evidence requires receipt, receiptProof, receiptProofHash, or transactionHash");
    }

    String blockHash = null;
    String receiptBlockNumber = null;
    String blockReceiptsRoot = null;
    if (receipt != null) {
      if (!"0x1".equals(receipt.get("status"))) {
        throw new IllegalArgumentException(
            "Ethereum mainnet inbound receipt status must be 0x1");
      }
      final String receiptTransactionHash =
          normalizeRpcHex(
              strictFirstPresent(
                  receipt, "receipt.transactionHash", "transactionHash", "transaction_hash"),
              "receipt.transactionHash",
              32);
      if (transactionHash != null && !transactionHash.equals(receiptTransactionHash)) {
        throw new IllegalArgumentException("receipt.transactionHash must match transactionHash");
      }
      transactionHash = receiptTransactionHash;
      blockHash =
          normalizeRpcHex(
              strictFirstPresent(receipt, "receipt.blockHash", "blockHash", "block_hash"),
              "receipt.blockHash",
              32);
      final Object receiptBlockNumberInput =
          strictFirstPresent(receipt, "receipt.blockNumber", "blockNumber", "block_number");
      receiptBlockNumber = normalizePositiveRpcQuantity(receiptBlockNumberInput, "receipt.blockNumber");
    }

    Map<String, Object> block = input.block();
    if (block == null && blockHash != null && provider != null) {
      block =
          requireMap(
              provider.request("eth_getBlockByHash", Arrays.asList(blockHash, Boolean.FALSE)),
              "eth_getBlockByHash");
    }
    if (block != null) {
      final String normalizedBlockHash = normalizeRpcHex(block.get("hash"), "block.hash", 32);
      if (blockHash != null && !blockHash.equals(normalizedBlockHash)) {
        throw new IllegalArgumentException("block.hash must match receipt.blockHash");
      }
      final Object blockNumberInput =
          strictFirstPresent(block, "block.number", "number", "blockNumber", "block_number");
      final String blockNumber = normalizePositiveRpcQuantity(blockNumberInput, "block.number");
      if (receiptBlockNumber != null && !receiptBlockNumber.equals(blockNumber)) {
        throw new IllegalArgumentException("block.number must match receipt.blockNumber");
      }
      receiptBlockNumber = blockNumber;
      blockReceiptsRoot =
          normalizeRpcHex(
              strictFirstPresent(block, "block.receiptsRoot", "receiptsRoot", "receipts_root"),
              "block.receiptsRoot",
              32);
    }

    receipt = callbackMapSnapshot(receipt);
    block = callbackMapSnapshot(block);
    final Map<String, Object> rawBeaconFinality =
        input.beaconFinality() != null
            ? input.beaconFinality()
            : finalityProvider == null
                ? null
                : finalityProvider.collectFinalityEvidence(receipt, block, transactionHash);
    final Map<String, Object> beaconFinality =
        rawBeaconFinality == null
            ? null
            : normalizeBeaconFinality(
                rawBeaconFinality, blockHash, receiptBlockNumber, blockReceiptsRoot);
    final SourceEvent sourceEvent =
        normalizeEthereumReceiptSourceEvent(
            receipt,
            input.sourceEventDigest(),
            receipt == null
                    && input.sourceEventDigest() == null
                    && input.sourceBridgeEmitterAddress() == null
                ? null
                : resolveSourceBridgeEmitterAddress(
                    input.sourceBridgeEmitterAddress(), sourceBridgeEmitterAddress),
            transactionHash,
            blockHash,
            receiptBlockNumber);
    ReceiptProof receiptProof = input.receiptProof();
    List<Map<String, Object>> blockReceipts = input.blockReceipts();
    if (receiptProof == null
        && receipt != null
        && beaconFinality != null
        && sourceEvent.sourceEventDigest() != null
        && input.inclusionBranch() != null) {
      if (blockReceipts == null) {
        if (provider == null) {
          throw new IllegalStateException(
              "Ethereum mainnet execution provider is not linked for block receipt evidence collection");
        }
        blockReceipts =
            requireMapList(
                provider.request(
                    "eth_getBlockReceipts", Collections.<Object>singletonList(receiptBlockNumber)),
                "eth_getBlockReceipts");
      }
      final Object receiptTransactionIndex =
          strictFirstPresent(
              receipt, "receipt.transactionIndex", "transactionIndex", "transaction_index");
      final SourceSccpProofs.EvmReceiptTrieProof receiptTrieProof =
          SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(
              blockReceipts, receiptTransactionIndex);
      final String expectedReceiptsRoot =
          blockReceiptsRoot == null
              ? (String) beaconFinality.get("executionReceiptsRoot")
              : blockReceiptsRoot;
      if (!receiptTrieProof.receiptsRoot.equals(expectedReceiptsRoot)) {
        throw new IllegalArgumentException(
            "computed receipt trie root must match block.receiptsRoot");
      }
      final long targetIndex =
          normalizeUnsignedInteger(receiptTransactionIndex, "receipt.transactionIndex");
      if (targetIndex >= blockReceipts.size()) {
        throw new IllegalArgumentException(
            "receipt.transactionIndex must select an eth_getBlockReceipts entry");
      }
      final Map<String, Object> indexedReceipt = blockReceipts.get((int) targetIndex);
      final String indexedTransactionHash =
          normalizeRpcHex(
              strictFirstPresent(
                  indexedReceipt,
                  "blockReceipts.transactionHash",
                  "transactionHash",
                  "transaction_hash"),
              "blockReceipts transactionHash",
              32);
      if (!indexedTransactionHash.equals(transactionHash)) {
        throw new IllegalArgumentException(
            "eth_getBlockReceipts target receipt must match transactionHash");
      }
      final String indexedBlockHash =
          normalizeRpcHex(
              strictFirstPresent(
                  indexedReceipt, "blockReceipts.blockHash", "blockHash", "block_hash"),
              "blockReceipts blockHash",
              32);
      if (!indexedBlockHash.equals(blockHash)) {
        throw new IllegalArgumentException(
            "eth_getBlockReceipts target receipt blockHash must match receipt");
      }
      final String indexedBlockNumber =
          normalizePositiveRpcQuantity(
              strictFirstPresent(
                  indexedReceipt, "blockReceipts.blockNumber", "blockNumber", "block_number"),
              "blockReceipts blockNumber");
      if (!indexedBlockNumber.equals(receiptBlockNumber)) {
        throw new IllegalArgumentException(
            "eth_getBlockReceipts target receipt blockNumber must match receipt");
      }
      final String receiptRlp =
          "0x" + hexLower(SourceSccpProofs.canonicalEvmReceiptRlp(receipt));
      if (!receiptTrieProof.receiptRlp.equals(receiptRlp)) {
        throw new IllegalArgumentException(
            "eth_getBlockReceipts target receipt RLP must match receipt");
      }
      final String beaconSlot = (String) beaconFinality.get("beaconSlot");
      final String finalizedHeaderRoot = (String) beaconFinality.get("finalizedHeaderRoot");
      final String syncCommitteeRoot = (String) beaconFinality.get("syncCommitteeRoot");
      if (beaconSlot == null) {
        throw new IllegalArgumentException("beaconFinality.beaconSlot is required for receiptProof");
      }
      if (finalizedHeaderRoot == null) {
        throw new IllegalArgumentException(
            "beaconFinality.finalizedHeaderRoot is required for receiptProof");
      }
      if (syncCommitteeRoot == null) {
        throw new IllegalArgumentException(
            "beaconFinality.syncCommitteeRoot is required for receiptProof");
      }
      receiptProof =
          new ReceiptProof(
              DOMAIN_ETH,
              sourceEvent.sourceEventDigest(),
              beaconSlot,
              (String) beaconFinality.get("executionBlockNumber"),
              (String) beaconFinality.get("executionBlockHash"),
              (String) beaconFinality.get("executionReceiptsRoot"),
              finalizedHeaderRoot,
              syncCommitteeRoot,
              Long.toString(targetIndex),
              receiptTrieProof.receiptTrieProofNodes(),
              input.inclusionBranch());
    }
    requireReceiptProofMatchesEvidence(
        receiptProof,
        blockHash,
        receiptBlockNumber,
        blockReceiptsRoot,
        beaconFinality,
        sourceEvent.sourceEventDigest());
    final String receiptProofHash =
        normalizeReceiptProofHash(receiptProof, input.receiptProofHash());
    return callbackEvidenceSnapshot(
        new InboundEvidence(
            DOMAIN_ETH,
            DOMAIN_SORA,
            transactionHash,
            receipt,
            block,
            beaconFinality,
            receiptProof,
            receiptProofHash,
            sourceEvent.sourceEventDigest(),
            sourceEvent.sourceBridgeEmitterAddress(),
            blockReceipts,
            input.inclusionBranch()));
  }

  public byte[] proveInboundToSora(final InboundEvidence input) {
    return proveInboundToSora(input, executionProvider, consensusProvider);
  }

  public byte[] proveInboundToSora(
      final InboundEvidence input,
      final ExecutionProvider provider,
      final ConsensusProvider finalityProvider) {
    if (inboundProver == null) {
      throw new IllegalStateException("Ethereum mainnet SCCP inbound prover is not linked");
    }
    final InboundEvidence evidence =
        collectInboundEvidenceFromReceipt(input, provider, finalityProvider);
    if (evidence.beaconFinality() == null) {
      throw new IllegalArgumentException(
          "Ethereum mainnet SCCP inbound proof requires beaconFinality");
    }
    if (evidence.receiptProof() == null) {
      throw new IllegalArgumentException(
          "Ethereum mainnet SCCP inbound proof requires receiptProof");
    }
    if (evidence.sourceEventDigest() == null) {
      throw new IllegalArgumentException(
          "Ethereum mainnet SCCP inbound proof requires receipt source event validation");
    }
    if (!evidence.beaconFinality().containsKey("finalizedHeaderRoot")) {
      throw new IllegalArgumentException(
          "Ethereum mainnet SCCP inbound proof requires beaconFinality.finalizedHeaderRoot");
    }
    if (!evidence.beaconFinality().containsKey("syncCommitteeRoot")) {
      throw new IllegalArgumentException(
          "Ethereum mainnet SCCP inbound proof requires beaconFinality.syncCommitteeRoot");
    }
    if (!evidence.beaconFinality().containsKey("beaconSlot")) {
      throw new IllegalArgumentException(
          "Ethereum mainnet SCCP inbound proof requires beaconFinality.beaconSlot");
    }
    for (final String field :
        Arrays.asList(
            "finalityBranch",
            "syncCommitteeBits",
            "syncCommitteeSignature",
            "syncCommitteeParticipation",
            "syncSignatureSlot")) {
      if (!evidence.beaconFinality().containsKey(field)) {
        throw new IllegalArgumentException(
            "Ethereum mainnet SCCP inbound proof requires beaconFinality." + field);
      }
    }
    return requireNativeRecursiveBytes(
        inboundProver.prove(callbackEvidenceSnapshot(evidence)), "proofBytes");
  }

  public Object submitInboundToIroha(final byte[] proofBytes) {
    final byte[] proof = requireNativeRecursiveBytes(proofBytes, "proofBytes");
    if (inboundSubmitter == null) {
      throw new IllegalStateException("Ethereum mainnet SCCP inbound submitter is not linked");
    }
    return inboundSubmitter.submit(Arrays.copyOf(proof, proof.length));
  }

  public static SourceSccpProofs.EvmDestinationBinding destinationBinding(
      final String verifierAddress,
      final String bridgeAddress,
      final String verifierCodeHash,
      final String verifierKeyHash) {
    return SourceSccpProofs.ethereumMainnetDestinationBinding(
        verifierAddress, bridgeAddress, verifierCodeHash, verifierKeyHash);
  }

  public static String destinationBindingHash(
      final String verifierAddress,
      final String bridgeAddress,
      final String verifierCodeHash,
      final String verifierKeyHash) {
    return destinationBinding(verifierAddress, bridgeAddress, verifierCodeHash, verifierKeyHash)
        .hash;
  }

  public static EvmSccpProver.ProofRequest buildProofRequest(
      final EvmSccpProver.ProofRequestInput input) {
    requireEthereumRequestInput(input);
    final EvmSccpProver.ProofRequest request = EvmSccpProver.buildProofRequest(input);
    requireEthereumProofRequest(request);
    return request;
  }

  public static EvmSccpProver.ProofRequest buildProofRequest(
      final EvmSccpProver.ProofRequestInput input,
      final EvmSccpProver.EthereumMainnetNativeEvmProverBundle nativeProverBundle) {
    return buildProofRequest(Objects.requireNonNull(nativeProverBundle, "nativeProverBundle").applyTo(input));
  }

  public static EvmSccpProver.ProofResult wrapProofResult(
      final byte[] proofBytes, final EvmSccpProver.ProofRequest request) {
    requireEthereumProofRequest(request);
    return EvmSccpProver.wrapProofResult(proofBytes, request);
  }

  public static EvmSccpProver.Submission buildSubmission(
      final EvmSccpProver.SubmissionInput input) {
    Objects.requireNonNull(input, "input");
    if (input.publicInputs().targetDomain() != DOMAIN_ETH) {
      throw new IllegalArgumentException("Ethereum mainnet submissions must target ETH");
    }
    final EvmSccpProver.ProofResult proofResult =
        Objects.requireNonNull(
            input.proofResult(),
            "Ethereum mainnet submissions require a wrapped proofResult with destinationBinding");
    if (proofResult.publicInputs().targetDomain() != DOMAIN_ETH) {
      throw new IllegalArgumentException("Ethereum mainnet proofResult must target ETH");
    }
    final SourceSccpProofs.EvmDestinationBinding destinationBinding =
        requireEthereumDestinationBinding(proofResult.destinationBinding());
    if (!destinationBinding.hash.equals(proofResult.destinationBindingHash())) {
      throw new IllegalArgumentException(
          "Ethereum mainnet proofResult destinationBindingHash must match destinationBinding");
    }
    return EvmSccpProver.buildSubmission(input);
  }

  public static LocalAdmissionSubmission buildLocalAdmissionSubmission(
      final LocalAdmissionSubmissionInput input) {
    Objects.requireNonNull(input, "input");
    if (input.sourceDomain() != DOMAIN_ETH || input.targetDomain() != DOMAIN_SORA) {
      throw new IllegalArgumentException(
          "Ethereum mainnet local-admission submissions must route ETH -> SORA");
    }
    if (!LOCAL_ADMISSION_ENVELOPE_ENCODING_V1.equals(input.envelopeEncoding())) {
      throw new IllegalArgumentException(
          "Ethereum mainnet local-admission envelopeEncoding is not canonical");
    }
    if (!LOCAL_ADMISSION_SUBMISSION_KIND_V1.equals(input.submissionKind())) {
      throw new IllegalArgumentException(
          "Ethereum mainnet local-admission submissionKind is not canonical");
    }
    if (!LOCAL_ADMISSION_ENTRYPOINT_V1.equals(input.verifierEntrypoint())) {
      throw new IllegalArgumentException(
          "Ethereum mainnet local-admission verifierEntrypoint is not canonical");
    }
    if (!STARK_FRI_PROOF_FAMILY_V1.equals(input.proofFamily())) {
      throw new IllegalArgumentException(
          "Ethereum mainnet local-admission proofFamily is not canonical");
    }
    if (!EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1.equals(input.verifierBackend())) {
      throw new IllegalArgumentException(
          "Ethereum mainnet local-admission verifierBackend is not canonical");
    }
    final byte[] proofBytes = requireNativeRecursiveBytes(input.proofBytes(), "proofBytes");
    final byte[] publicInputsBytes =
        requireNativeRecursiveBytes(input.publicInputsBytes(), "publicInputsBytes");
    final byte[] bundleBytes = requireNativeRecursiveBytes(input.bundleBytes(), "bundleBytes");
    final byte[] envelopeBytes =
        requireNativeRecursiveBytes(input.envelopeBytes(), "envelopeBytes");
    final String statementHash = normalizeNonZeroHex32(input.statementHash(), "statementHash");
    final String sourceVerifierMaterialHash =
        normalizeNonZeroHex32(
            input.sourceVerifierMaterialHash(), "sourceVerifierMaterialHash");
    final String sourceAdapterEngineDeploymentHash =
        normalizeNonZeroHex32(
            input.sourceAdapterEngineDeploymentHash(),
            "sourceAdapterEngineDeploymentHash");
    final LocalAdmissionPayload payload =
        new LocalAdmissionPayload(
            proofBytes,
            publicInputsBytes,
            bundleBytes,
            statementHash,
            sourceVerifierMaterialHash,
            sourceAdapterEngineDeploymentHash);
    return new LocalAdmissionSubmission(
        input.proofFamily(),
        input.verifierBackend(),
        DOMAIN_ETH,
        DOMAIN_SORA,
        statementHash,
        sourceVerifierMaterialHash,
        sourceAdapterEngineDeploymentHash,
        payload,
        proofBytes,
        publicInputsBytes,
        bundleBytes,
        envelopeBytes);
  }

  public EvmSccpProver.ProofRequest buildOutboundProofRequest(
      final EvmSccpProver.ProofRequestInput input) {
    final EvmSccpProver.ProofRequestInput resolved =
        witnessProvider == null ? input : witnessProvider.resolveWitness(inputSnapshot(input));
    return buildProofRequest(nativeProverBundle == null ? resolved : nativeProverBundle.applyTo(resolved));
  }

  public EvmSccpProver.ProofResult proveOutboundToEthereum(
      final EvmSccpProver.ProofRequestInput input) {
    final EvmSccpProver.ProofRequest request = buildOutboundProofRequest(input);
    requireVerifiedNativeProverArtifacts(nativeProverArtifacts, request);
    if (proofEngine == null) {
      throw new IllegalStateException("Ethereum mainnet SCCP Groth16 prover is not linked");
    }
    return wrapProofResult(
        proofEngine.prove(EvmSccpProver.callbackRequestSnapshot(request)), request);
  }

  private static void requireVerifiedNativeProverArtifacts(
      final EvmSccpProver.EthereumMainnetNativeEvmProverArtifacts artifacts,
      final EvmSccpProver.ProofRequest request) {
    if (artifacts == null) {
      throw new IllegalArgumentException(
          "Ethereum mainnet SCCP outbound proof requires verified native EVM prover artifacts");
    }
    if (!artifacts.nativeProverBundle().destinationBindingHash().equals(request.destinationBindingHash())) {
      throw new IllegalArgumentException(
          "nativeProverArtifacts destinationBindingHash must match proof request");
    }
    if (!artifacts.proofArtifactHash().equals(request.proofArtifactHash())
        || !artifacts.provingKeyHash().equals(request.provingKeyHash())) {
      throw new IllegalArgumentException(
          "nativeProverArtifacts artifact hashes must match proof request");
    }
    if (!artifacts.verifierKeyHash().equals(artifacts.nativeProverBundle().verifierKeyHash())) {
      throw new IllegalArgumentException(
          "nativeProverArtifacts verifierKeyHash must match nativeProverBundle");
    }
    if (artifacts.sdk() == null
        || artifacts.sdk().isEmpty()
        || artifacts.implementation() == null
        || artifacts.implementation().isEmpty()
        || artifacts.implementationHash() == null
        || artifacts.implementationHash().isEmpty()) {
      throw new IllegalArgumentException(
          "nativeProverArtifacts must bind sdk implementation and implementationHash");
    }
    EvmSccpProver.EthereumMainnetNativeEvmProverBundleSdkArtifact artifact = null;
    for (final EvmSccpProver.EthereumMainnetNativeEvmProverBundleSdkArtifact row :
        artifacts.nativeProverBundle().nativeSdkArtifacts()) {
      if (artifacts.sdk().equals(row.sdk())) {
        artifact = row;
        break;
      }
    }
    if (artifact == null) {
      throw new IllegalArgumentException(
          "nativeProverBundle has no artifact row for sdk: " + artifacts.sdk());
    }
    if (!artifacts.implementation().equals(artifact.implementation())
        || !artifacts.implementationHash().equals(artifact.implementationHash())) {
      throw new IllegalArgumentException(
          "nativeProverArtifacts implementation binding must match nativeProverBundle");
    }
  }

  public EvmSccpProver.Submission buildEthereumCalldata(
      final EvmSccpProver.SubmissionInput input) {
    return buildSubmission(input);
  }

  public LocalAdmissionSubmission buildLocalAdmission(
      final LocalAdmissionSubmissionInput input) {
    return buildLocalAdmissionSubmission(input);
  }

  public Object submitOutboundToEthereum(final EvmSccpProver.SubmissionInput input) {
    if (outboundSubmitter == null) {
      throw new IllegalStateException("Ethereum mainnet SCCP outbound submitter is not linked");
    }
    if (executionProvider != null) {
      validateExecutionProviderMainnet(executionProvider);
    }
    return outboundSubmitter.submit(buildEthereumCalldata(input));
  }

  private static void requireEthereumRequestInput(final EvmSccpProver.ProofRequestInput input) {
    Objects.requireNonNull(input, "input");
    if (input.sourceDomain() != DOMAIN_SORA) {
      throw new IllegalArgumentException(
          "Ethereum mainnet proof requests must route SORA -> ETH");
    }
    if (input.publicInputs().targetDomain() != DOMAIN_ETH) {
      throw new IllegalArgumentException("Ethereum mainnet proof requests must target ETH");
    }
    final SourceSccpProofs.EvmDestinationBinding binding =
        requireEthereumDestinationBinding(input.destinationBinding());
    if (!binding.hash.equals(input.destinationBindingHash())) {
      throw new IllegalArgumentException(
          "destinationBindingHash must match Ethereum mainnet destinationBinding");
    }
  }

  private static void requireEthereumProofRequest(final EvmSccpProver.ProofRequest request) {
    Objects.requireNonNull(request, "request");
    if (request.sourceDomain() != DOMAIN_SORA) {
      throw new IllegalArgumentException(
          "Ethereum mainnet proof requests must route SORA -> ETH");
    }
    if (request.targetDomain() != DOMAIN_ETH
        || request.publicInputs().targetDomain() != DOMAIN_ETH) {
      throw new IllegalArgumentException("Ethereum mainnet proof requests must target ETH");
    }
    final SourceSccpProofs.EvmDestinationBinding binding =
        requireEthereumDestinationBinding(request.destinationBinding());
    if (!binding.hash.equals(request.destinationBindingHash())) {
      throw new IllegalArgumentException(
          "destinationBindingHash must match Ethereum mainnet destinationBinding");
    }
  }

  private static SourceSccpProofs.EvmDestinationBinding requireEthereumDestinationBinding(
      final SourceSccpProofs.EvmDestinationBinding destinationBinding) {
    final SourceSccpProofs.EvmDestinationBinding binding =
        Objects.requireNonNull(destinationBinding, "destinationBinding");
    if (binding.sourceDomain != DOMAIN_SORA) {
      throw new IllegalArgumentException(
          "Ethereum mainnet destinationBinding must start from SORA");
    }
    if (binding.targetDomain != DOMAIN_ETH) {
      throw new IllegalArgumentException("Ethereum mainnet destinationBinding must target ETH");
    }
    if (!MAINNET_NETWORK_ID.equals(binding.networkId)) {
      throw new IllegalArgumentException(
          "Ethereum mainnet destinationBinding.networkId must be chain id 1");
    }
    return binding;
  }

  private static EvmSccpProver.ProofRequestInput inputSnapshot(
      final EvmSccpProver.ProofRequestInput input) {
    final byte[] bundleBytes = Objects.requireNonNull(input.bundleBytes(), "bundleBytes");
    final byte[] sourceProofBytes =
        Objects.requireNonNull(input.sourceProofBytes(), "sourceProofBytes");
    return new EvmSccpProver.ProofRequestInput(
        input.publicInputs(),
        Arrays.copyOf(bundleBytes, bundleBytes.length),
        Arrays.copyOf(sourceProofBytes, sourceProofBytes.length),
        input.statementHash(),
        input.destinationBindingHash(),
        input.backend(),
        input.sourceDomain(),
        input.destinationBinding(),
        input.proofArtifactHash(),
        input.provingKeyHash());
  }

  private static long normalizeRpcChainId(final Object value) {
    final String quantity = normalizeRpcQuantity(value, "eth_chainId");
    final BigInteger parsed = new BigInteger(quantity.substring(2), 16);
    if (parsed.bitLength() > 63) {
      throw new IllegalArgumentException("eth_chainId must fit positive i64");
    }
    return parsed.longValue();
  }

  private static long normalizeUnsignedInteger(final Object value, final String label) {
    if (value instanceof BigInteger) {
      final BigInteger parsed = (BigInteger) value;
      if (parsed.signum() < 0 || parsed.bitLength() > 63) {
        throw new IllegalArgumentException(label + " must fit positive i64");
      }
      return parsed.longValue();
    }
    if (value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long) {
      final long parsed = ((Number) value).longValue();
      if (parsed < 0) {
        throw new IllegalArgumentException(label + " must be non-negative");
      }
      return parsed;
    }
    if (value instanceof Number) {
      throw new IllegalArgumentException(label + " must be an integral JSON-RPC quantity");
    }
    if (value instanceof String) {
      final String text = (String) value;
      if (!text.trim().equals(text)) {
        throw new IllegalArgumentException(label + " must be canonical");
      }
      final BigInteger parsed;
      if (text.startsWith("0x")) {
        final String hex = text.substring(2);
        if (!hex.matches("0|[1-9a-f][0-9a-f]*")) {
          throw new IllegalArgumentException(
              label + " must be a canonical JSON-RPC quantity");
        }
        parsed = new BigInteger(hex, 16);
      } else {
        if (!text.matches("0|[1-9][0-9]*")) {
          throw new IllegalArgumentException(label + " must be a canonical decimal integer");
        }
        parsed = new BigInteger(text, 10);
      }
      if (parsed.bitLength() > 63) {
        throw new IllegalArgumentException(label + " must fit positive i64");
      }
      return parsed.longValue();
    }
    throw new IllegalArgumentException(label + " must be a JSON-RPC quantity or integer");
  }

  private static byte[] requireNativeRecursiveBytes(final byte[] bytes, final String label) {
    final byte[] copy = Arrays.copyOf(Objects.requireNonNull(bytes, label), bytes.length);
    if (copy.length == 0) {
      throw new IllegalArgumentException(label + " must not be empty");
    }
    boolean nonzero = false;
    for (final byte value : copy) {
      nonzero |= value != 0;
    }
    if (!nonzero) {
      throw new IllegalArgumentException(label + " must not be all zero");
    }
    if (copy.length > NATIVE_RECURSIVE_MAX_PROOF_BYTES) {
      throw new IllegalArgumentException(
          label + " must be at most " + NATIVE_RECURSIVE_MAX_PROOF_BYTES + " bytes");
    }
    return copy;
  }

  private static String normalizeNonZeroHex32(final String value, final String label) {
    final String text = Objects.requireNonNull(value, label);
    if (!text.startsWith("0x") || text.length() != 66) {
      throw new IllegalArgumentException(
          label + " must be 32 bytes of canonical lowercase 0x hex");
    }
    boolean nonzero = false;
    for (int index = 2; index < text.length(); index++) {
      final char symbol = text.charAt(index);
      if (!((symbol >= '0' && symbol <= '9') || (symbol >= 'a' && symbol <= 'f'))) {
        throw new IllegalArgumentException(
            label + " must be 32 bytes of canonical lowercase 0x hex");
      }
      nonzero |= symbol != '0';
    }
    if (!nonzero) {
      throw new IllegalArgumentException(label + " must not be zero");
    }
    return text;
  }

  private static String hexLower(final byte[] bytes) {
    final char[] out = new char[bytes.length * 2];
    final char[] alphabet = "0123456789abcdef".toCharArray();
    for (int index = 0; index < bytes.length; index++) {
      final int value = bytes[index] & 0xff;
      out[index * 2] = alphabet[value >>> 4];
      out[index * 2 + 1] = alphabet[value & 0x0f];
    }
    return new String(out);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> requireMap(final Object value, final String label) {
    if (!(value instanceof Map)) {
      throw new IllegalArgumentException(label + " must return an object");
    }
    return (Map<String, Object>) value;
  }

  @SuppressWarnings("unchecked")
  private static List<Map<String, Object>> requireMapList(final Object value, final String label) {
    if (!(value instanceof List)) {
      throw new IllegalArgumentException(label + " must return an array");
    }
    final List<Object> list = (List<Object>) value;
    final List<Map<String, Object>> maps = new ArrayList<>(list.size());
    for (int index = 0; index < list.size(); index++) {
      final Object item = list.get(index);
      if (!(item instanceof Map)) {
        throw new IllegalArgumentException(label + "[" + index + "] must be an object");
      }
      maps.add((Map<String, Object>) item);
    }
    return maps;
  }

  private static Object firstPresent(final Map<String, Object> input, final String... keys) {
    for (final String key : keys) {
      if (input.containsKey(key)) {
        return input.get(key);
      }
    }
    return null;
  }

  private static Object strictFirstPresent(
      final Map<String, Object> input, final String label, final String... keys) {
    Object selected = null;
    boolean found = false;
    for (final String key : keys) {
      if (input.containsKey(key)) {
        if (found) {
          throw new IllegalArgumentException(label + " must not use multiple aliases");
        }
        selected = input.get(key);
        found = true;
      }
    }
    return selected;
  }

  private static String normalizeRpcHex(
      final Object value, final String label, final int byteLength) {
    return normalizeRpcHex(value, label, byteLength, false);
  }

  private static String normalizeRpcHex(
      final Object value,
      final String label,
      final int byteLength,
      final boolean allowZero) {
    if (!(value instanceof String)) {
      throw new IllegalArgumentException(label + " must be canonical lowercase 0x hex");
    }
    final String text = (String) value;
    if (!text.trim().equals(text) || !text.startsWith("0x")) {
      throw new IllegalArgumentException(label + " must be canonical lowercase 0x hex");
    }
    final String hex = text.substring(2);
    if (hex.length() != byteLength * 2 || !hex.matches("[0-9a-f]+")) {
      throw new IllegalArgumentException(
          label + " must be " + byteLength + " bytes canonical lowercase 0x hex");
    }
    boolean nonzero = false;
    for (int index = 0; index < hex.length(); index++) {
      nonzero |= hex.charAt(index) != '0';
    }
    if (!allowZero && !nonzero) {
      throw new IllegalArgumentException(label + " must not be zero");
    }
    return text;
  }

  private static String normalizeReceiptProofHash(
      final ReceiptProof receiptProof, final String suppliedHash) {
    String normalizedHash =
        suppliedHash == null ? null : normalizeRpcHex(suppliedHash, "receiptProofHash", 32);
    if (receiptProof == null) {
      return normalizedHash;
    }
    if (receiptProof.sourceDomain() != DOMAIN_ETH) {
      throw new IllegalArgumentException("receiptProof.sourceDomain must be ETH");
    }
    final String computedHash =
        SourceSccpProofs.evmReceiptProofHash(
            receiptProof.sourceEventDigest(),
            receiptProof.beaconSlot(),
            receiptProof.executionBlockNumber(),
            receiptProof.executionBlockHash(),
            receiptProof.executionReceiptsRoot(),
            receiptProof.beaconFinalizedRoot(),
            receiptProof.syncCommitteeRoot(),
            receiptProof.receiptRootIndex(),
            receiptProof.receiptTrieProofNodes(),
            receiptProof.inclusionBranch(),
            receiptProof.sourceDomain());
    if (normalizedHash != null && !normalizedHash.equals(computedHash)) {
      throw new IllegalArgumentException("receiptProofHash must match receiptProof");
    }
    return computedHash;
  }

  private static void requireReceiptProofMatchesEvidence(
      final ReceiptProof receiptProof,
      final String blockHash,
      final String receiptBlockNumber,
      final String blockReceiptsRoot,
      final Map<String, Object> beaconFinality,
      final String sourceEventDigest) {
    if (receiptProof == null) {
      return;
    }
    final long proofBlockNumber =
        normalizeUnsignedInteger(
            receiptProof.executionBlockNumber(), "receiptProof.executionBlockNumber");
    if (receiptBlockNumber != null
        && proofBlockNumber != normalizeUnsignedInteger(receiptBlockNumber, "block.number")) {
      throw new IllegalArgumentException(
          "receiptProof.executionBlockNumber must match block.number");
    }
    if (beaconFinality != null
        && proofBlockNumber
            != normalizeUnsignedInteger(
                beaconFinality.get("executionBlockNumber"),
                "beaconFinality.executionBlockNumber")) {
      throw new IllegalArgumentException(
          "receiptProof.executionBlockNumber must match beaconFinality.executionBlockNumber");
    }
    final String proofBlockHash =
        normalizeRpcHex(receiptProof.executionBlockHash(), "receiptProof.executionBlockHash", 32);
    if (blockHash != null && !proofBlockHash.equals(blockHash)) {
      throw new IllegalArgumentException("receiptProof.executionBlockHash must match block.hash");
    }
    if (beaconFinality != null
        && !proofBlockHash.equals(beaconFinality.get("executionBlockHash"))) {
      throw new IllegalArgumentException(
          "receiptProof.executionBlockHash must match beaconFinality.executionBlockHash");
    }
    final String proofReceiptsRoot =
        normalizeRpcHex(
            receiptProof.executionReceiptsRoot(), "receiptProof.executionReceiptsRoot", 32);
    if (blockReceiptsRoot != null && !proofReceiptsRoot.equals(blockReceiptsRoot)) {
      throw new IllegalArgumentException(
          "receiptProof.executionReceiptsRoot must match block.receiptsRoot");
    }
    if (beaconFinality != null
        && !proofReceiptsRoot.equals(beaconFinality.get("executionReceiptsRoot"))) {
      throw new IllegalArgumentException(
          "receiptProof.executionReceiptsRoot must match beaconFinality.executionReceiptsRoot");
    }
    if (beaconFinality != null) {
      final Object finalityFinalizedRootInput =
          strictFirstPresent(
              beaconFinality,
              "beaconFinality.finalizedHeaderRoot",
              "finalizedHeaderRoot",
              "finalized_header_root",
              "beaconFinalizedRoot",
              "beacon_finalized_root");
      if (finalityFinalizedRootInput != null) {
        final String finalityFinalizedRoot =
            normalizeRpcHex(
                finalityFinalizedRootInput, "beaconFinality.finalizedHeaderRoot", 32);
        final String proofFinalizedRoot =
            normalizeRpcHex(
                receiptProof.beaconFinalizedRoot(), "receiptProof.beaconFinalizedRoot", 32);
        if (!proofFinalizedRoot.equals(finalityFinalizedRoot)) {
          throw new IllegalArgumentException(
              "receiptProof.beaconFinalizedRoot must match beaconFinality.finalizedHeaderRoot");
        }
      }
      final Object finalitySyncCommitteeRootInput =
          strictFirstPresent(
              beaconFinality,
              "beaconFinality.syncCommitteeRoot",
              "syncCommitteeRoot",
              "sync_committee_root");
      if (finalitySyncCommitteeRootInput != null) {
        final String finalitySyncCommitteeRoot =
            normalizeRpcHex(
                finalitySyncCommitteeRootInput, "beaconFinality.syncCommitteeRoot", 32);
        final String proofSyncCommitteeRoot =
            normalizeRpcHex(
                receiptProof.syncCommitteeRoot(), "receiptProof.syncCommitteeRoot", 32);
        if (!proofSyncCommitteeRoot.equals(finalitySyncCommitteeRoot)) {
          throw new IllegalArgumentException(
              "receiptProof.syncCommitteeRoot must match beaconFinality.syncCommitteeRoot");
        }
      }
      final Object finalityBeaconSlotInput =
          strictFirstPresent(
              beaconFinality,
              "beaconFinality.beaconSlot",
              "beaconSlot",
              "beacon_slot",
              "finalizedSlot",
              "finalized_slot",
              "slot");
      if (finalityBeaconSlotInput != null) {
        final long finalityBeaconSlot =
            normalizeUnsignedInteger(finalityBeaconSlotInput, "beaconFinality.beaconSlot");
        final long proofBeaconSlot =
            normalizeUnsignedInteger(receiptProof.beaconSlot(), "receiptProof.beaconSlot");
        if (proofBeaconSlot != finalityBeaconSlot) {
          throw new IllegalArgumentException(
              "receiptProof.beaconSlot must match beaconFinality.beaconSlot");
        }
      }
    }
    if (sourceEventDigest != null) {
      final String proofSourceEventDigest =
          normalizeRpcHex(
              receiptProof.sourceEventDigest(), "receiptProof.sourceEventDigest", 32);
      if (!proofSourceEventDigest.equals(sourceEventDigest)) {
        throw new IllegalArgumentException(
            "receiptProof.sourceEventDigest must match receipt source event");
      }
    }
  }

  private static InboundEvidence callbackEvidenceSnapshot(final InboundEvidence evidence) {
    return new InboundEvidence(
        evidence.sourceDomain(),
        evidence.targetDomain(),
        evidence.transactionHash(),
        callbackMapSnapshot(evidence.receipt()),
        callbackMapSnapshot(evidence.block()),
        callbackMapSnapshot(evidence.beaconFinality()),
        callbackReceiptProofSnapshot(evidence.receiptProof()),
        evidence.receiptProofHash(),
        evidence.sourceEventDigest(),
        evidence.sourceBridgeEmitterAddress(),
        callbackMapListSnapshot(evidence.blockReceipts()),
        evidence.inclusionBranch() == null
            ? null
            : copyByteArrayList(evidence.inclusionBranch(), "inclusionBranch"));
  }

  private static ReceiptProof callbackReceiptProofSnapshot(final ReceiptProof receiptProof) {
    if (receiptProof == null) {
      return null;
    }
    return new ReceiptProof(
        receiptProof.sourceDomain(),
        receiptProof.sourceEventDigest(),
        receiptProof.beaconSlot(),
        receiptProof.executionBlockNumber(),
        receiptProof.executionBlockHash(),
        receiptProof.executionReceiptsRoot(),
        receiptProof.beaconFinalizedRoot(),
        receiptProof.syncCommitteeRoot(),
        receiptProof.receiptRootIndex(),
        receiptProof.receiptTrieProofNodes(),
        receiptProof.inclusionBranch());
  }

  private static List<Map<String, Object>> callbackMapListSnapshot(
      final List<Map<String, Object>> values) {
    if (values == null) {
      return null;
    }
    final ArrayList<Map<String, Object>> copy = new ArrayList<>(values.size());
    for (final Map<String, Object> value : values) {
      copy.add(callbackMapSnapshot(value));
    }
    return Collections.unmodifiableList(copy);
  }

  private static Map<String, Object> callbackMapSnapshot(final Map<String, Object> value) {
    if (value == null) {
      return null;
    }
    final LinkedHashMap<String, Object> copy = new LinkedHashMap<>();
    for (final Map.Entry<String, Object> entry : value.entrySet()) {
      copy.put(entry.getKey(), callbackAnySnapshot(entry.getValue()));
    }
    return Collections.unmodifiableMap(copy);
  }

  @SuppressWarnings("unchecked")
  private static Object callbackAnySnapshot(final Object value) {
    if (value instanceof byte[]) {
      final byte[] bytes = (byte[]) value;
      return Arrays.copyOf(bytes, bytes.length);
    }
    if (value instanceof Map) {
      final Map<?, ?> map = (Map<?, ?>) value;
      final LinkedHashMap<String, Object> copy = new LinkedHashMap<>();
      for (final Map.Entry<?, ?> entry : map.entrySet()) {
        if (entry.getKey() instanceof String) {
          copy.put((String) entry.getKey(), callbackAnySnapshot(entry.getValue()));
        }
      }
      return Collections.unmodifiableMap(copy);
    }
    if (value instanceof List) {
      final List<?> list = (List<?>) value;
      final ArrayList<Object> copy = new ArrayList<>(list.size());
      for (final Object item : list) {
        copy.add(callbackAnySnapshot(item));
      }
      return Collections.unmodifiableList(copy);
    }
    if (value instanceof Object[]) {
      final Object[] array = (Object[]) value;
      final ArrayList<Object> copy = new ArrayList<>(array.length);
      for (final Object item : array) {
        copy.add(callbackAnySnapshot(item));
      }
      return Collections.unmodifiableList(copy);
    }
    return value;
  }

  private static List<byte[]> copyByteArrayList(final List<byte[]> values, final String label) {
    Objects.requireNonNull(values, label);
    final ArrayList<byte[]> copy = new ArrayList<>(values.size());
    for (final byte[] value : values) {
      copy.add(Arrays.copyOf(Objects.requireNonNull(value, label), value.length));
    }
    return Collections.unmodifiableList(copy);
  }

  @SuppressWarnings("unchecked")
  private static SourceEvent normalizeEthereumReceiptSourceEvent(
      final Map<String, Object> receipt,
      final String sourceEventDigestInput,
      final String sourceBridgeEmitterAddressInput,
      final String transactionHash,
      final String blockHash,
      final String blockNumber) {
    final String sourceEventDigest =
        sourceEventDigestInput == null
            ? null
            : normalizeRpcHex(sourceEventDigestInput, "sourceEventDigest", 32);
    final String sourceBridgeEmitterAddress =
        sourceBridgeEmitterAddressInput == null
            ? null
            : normalizeRpcHex(sourceBridgeEmitterAddressInput, "sourceBridgeEmitterAddress", 20);
    if (sourceEventDigest == null && sourceBridgeEmitterAddress == null) {
      return new SourceEvent(null, null);
    }
    if (sourceBridgeEmitterAddress == null) {
      throw new IllegalArgumentException(
          "sourceBridgeEmitterAddress is required when validating sourceEventDigest");
    }
    if (receipt == null || !(receipt.get("logs") instanceof List)) {
      throw new IllegalArgumentException(
          "receipt.logs is required for SCCP source event validation");
    }
    final List<Object> logs = (List<Object>) receipt.get("logs");
    String matchedDigest = null;
    for (int index = 0; index < logs.size(); index++) {
      final Object logInput = logs.get(index);
      if (!(logInput instanceof Map)) {
        throw new IllegalArgumentException("receipt.logs[" + index + "] must be an object");
      }
      final Map<String, Object> log = (Map<String, Object>) logInput;
      if (Boolean.TRUE.equals(log.get("removed"))) {
        throw new IllegalArgumentException("receipt.logs must not contain removed logs");
      }
      final String logAddress =
          normalizeRpcHex(log.get("address"), "receipt.logs[" + index + "].address", 20, true);
      if (!(log.get("topics") instanceof List)) {
        throw new IllegalArgumentException(
            "receipt.logs[" + index + "].topics must be an array");
      }
      final List<Object> topics = (List<Object>) log.get("topics");
      if (topics.size() > 4) {
        throw new IllegalArgumentException(
            "receipt.logs[" + index + "].topics must contain at most 4 entries");
      }
      final java.util.ArrayList<String> normalizedTopics = new java.util.ArrayList<>(topics.size());
      for (int topicIndex = 0; topicIndex < topics.size(); topicIndex++) {
        normalizedTopics.add(
            normalizeRpcHex(
                topics.get(topicIndex),
                "receipt.logs[" + index + "].topics[" + topicIndex + "]",
                32,
                true));
      }
      if (sourceBridgeEmitterAddress.equals(logAddress)
          && !normalizedTopics.isEmpty()
          && SOURCE_EVENT_TOPIC_V1.equals(normalizedTopics.get(0))) {
        if (normalizedTopics.size() != 2) {
          throw new IllegalArgumentException(
              "SCCP source event log must contain exactly 2 topics");
        }
        if (!(log.get("data") instanceof String)) {
          throw new IllegalArgumentException("receipt.logs[" + index + "].data is required");
        }
        final Object data = log.get("data");
        if (!"0x".equals(data)) {
          throw new IllegalArgumentException("SCCP source event log data must be 0x");
        }
        final String logTransactionHash =
            normalizeRpcHex(
                strictFirstPresent(
                    log,
                    "receipt.logs[" + index + "].transactionHash",
                    "transactionHash",
                    "transaction_hash"),
                "receipt.logs[" + index + "].transactionHash",
                32);
        if (transactionHash != null && !transactionHash.equals(logTransactionHash)) {
          throw new IllegalArgumentException(
              "receipt.logs transactionHash must match receipt.transactionHash");
        }
        final String logBlockHash =
            normalizeRpcHex(
                strictFirstPresent(
                    log,
                    "receipt.logs[" + index + "].blockHash",
                    "blockHash",
                    "block_hash"),
                "receipt.logs[" + index + "].blockHash",
                32);
        if (blockHash != null && !blockHash.equals(logBlockHash)) {
          throw new IllegalArgumentException(
              "receipt.logs blockHash must match receipt.blockHash");
        }
        final String logBlockNumber =
            normalizePositiveRpcQuantity(
                strictFirstPresent(
                    log,
                    "receipt.logs[" + index + "].blockNumber",
                    "blockNumber",
                    "block_number"),
                "receipt.logs[" + index + "].blockNumber");
        if (blockNumber != null && !blockNumber.equals(logBlockNumber)) {
          throw new IllegalArgumentException(
              "receipt.logs blockNumber must match receipt.blockNumber");
        }
        final String candidateDigest = normalizedTopics.get(1);
        if (isZeroRpcHex(candidateDigest)) {
          throw new IllegalArgumentException("SCCP source event digest must not be zero");
        }
        if (sourceEventDigest != null && !sourceEventDigest.equals(candidateDigest)) {
          continue;
        }
        if (matchedDigest != null) {
          throw new IllegalArgumentException(
              "receipt.logs must contain exactly one matching SCCP source event");
        }
        matchedDigest = candidateDigest;
      }
    }
    if (matchedDigest == null) {
      throw new IllegalArgumentException(
          "receipt.logs must contain the expected SCCP source event");
    }
    return new SourceEvent(matchedDigest, sourceBridgeEmitterAddress);
  }

  private static String resolveSourceBridgeEmitterAddress(
      final String inputAddress, final String defaultAddress) {
    final String normalizedInput =
        inputAddress == null ? null : normalizeRpcHex(inputAddress, "sourceBridgeEmitterAddress", 20);
    final String normalizedDefault =
        defaultAddress == null
            ? null
            : normalizeRpcHex(defaultAddress, "sourceBridgeEmitterAddress", 20);
    if (normalizedInput != null
        && normalizedDefault != null
        && !normalizedInput.equals(normalizedDefault)) {
      throw new IllegalArgumentException("sourceBridgeEmitterAddress values must match");
    }
    return normalizedInput == null ? normalizedDefault : normalizedInput;
  }

  private static boolean isZeroRpcHex(final String text) {
    for (int index = 2; index < text.length(); index++) {
      if (text.charAt(index) != '0') {
        return false;
      }
    }
    return true;
  }

  private static String normalizeRpcQuantity(final Object value, final String label) {
    if (!(value instanceof String)) {
      throw new IllegalArgumentException(label + " must be a canonical JSON-RPC quantity");
    }
    final String text = (String) value;
    if (!text.trim().equals(text) || !text.startsWith("0x")) {
      throw new IllegalArgumentException(label + " must be a canonical JSON-RPC quantity");
    }
    final String hex = text.substring(2);
    if (!hex.matches("0|[1-9a-f][0-9a-f]*")) {
      throw new IllegalArgumentException(label + " must be a canonical JSON-RPC quantity");
    }
    return "0x" + new BigInteger(hex, 16).toString(16);
  }

  private static String normalizePositiveRpcQuantity(final Object value, final String label) {
    final String quantity = normalizeRpcQuantity(value, label);
    if ("0x0".equals(quantity)) {
      throw new IllegalArgumentException(label + " must be positive");
    }
    return quantity;
  }

  private static Map<String, Object> normalizeBeaconFinality(
      final Map<String, Object> finality,
      final String expectedBlockHash,
      final String expectedBlockNumber,
      final String expectedReceiptsRoot) {
    final long executionBlockNumber =
        normalizeUnsignedInteger(
            strictFirstPresent(
                finality,
                "beaconFinality.executionBlockNumber",
                "executionBlockNumber",
                "execution_block_number",
                "finalityHeight",
                "finality_height"),
            "beaconFinality.executionBlockNumber");
    if (executionBlockNumber == 0) {
      throw new IllegalArgumentException("beaconFinality.executionBlockNumber must be positive");
    }
    if (expectedBlockNumber != null
        && executionBlockNumber != normalizeUnsignedInteger(expectedBlockNumber, "block.number")) {
      throw new IllegalArgumentException(
          "beaconFinality.executionBlockNumber must match block.number");
    }
    final String executionBlockHash =
        normalizeRpcHex(
            strictFirstPresent(
                finality,
                "beaconFinality.executionBlockHash",
                "executionBlockHash",
                "execution_block_hash",
                "finalityBlockHash",
                "finality_block_hash"),
            "beaconFinality.executionBlockHash",
            32);
    if (expectedBlockHash != null && !expectedBlockHash.equals(executionBlockHash)) {
      throw new IllegalArgumentException("beaconFinality.executionBlockHash must match block.hash");
    }
    final String executionReceiptsRoot =
        normalizeRpcHex(
            strictFirstPresent(
                finality,
                "beaconFinality.executionReceiptsRoot",
                "executionReceiptsRoot",
                "execution_receipts_root",
                "receiptsRoot",
                "receipts_root"),
            "beaconFinality.executionReceiptsRoot",
            32);
    if (expectedReceiptsRoot != null && !expectedReceiptsRoot.equals(executionReceiptsRoot)) {
      throw new IllegalArgumentException(
          "beaconFinality.executionReceiptsRoot must match block.receiptsRoot");
    }
    final java.util.LinkedHashMap<String, Object> normalized =
        new java.util.LinkedHashMap<>(finality);
    for (final String key : BEACON_FINALITY_ALIAS_KEYS) {
      normalized.remove(key);
    }
    normalized.put("executionBlockNumber", Long.toString(executionBlockNumber));
    normalized.put("executionBlockHash", executionBlockHash);
    normalized.put("executionReceiptsRoot", executionReceiptsRoot);
    final Object finalizedHeaderRootInput =
        strictFirstPresent(
            finality,
            "beaconFinality.finalizedHeaderRoot",
            "finalizedHeaderRoot",
            "finalized_header_root",
            "beaconFinalizedRoot",
            "beacon_finalized_root");
    if (finalizedHeaderRootInput != null) {
      normalized.put(
          "finalizedHeaderRoot",
          normalizeRpcHex(
              finalizedHeaderRootInput, "beaconFinality.finalizedHeaderRoot", 32));
    }
    final Object syncCommitteeRootInput =
        strictFirstPresent(
            finality,
            "beaconFinality.syncCommitteeRoot",
            "syncCommitteeRoot",
            "sync_committee_root");
    if (syncCommitteeRootInput != null) {
      normalized.put(
          "syncCommitteeRoot",
          normalizeRpcHex(syncCommitteeRootInput, "beaconFinality.syncCommitteeRoot", 32));
    }
    final Object beaconSlotInput =
        strictFirstPresent(
            finality,
            "beaconFinality.beaconSlot",
            "beaconSlot",
            "beacon_slot",
            "finalizedSlot",
            "finalized_slot",
            "slot");
    Long normalizedBeaconSlot = null;
    if (beaconSlotInput != null) {
      normalizedBeaconSlot = normalizeUnsignedInteger(beaconSlotInput, "beaconFinality.beaconSlot");
      if (normalizedBeaconSlot == 0) {
        throw new IllegalArgumentException("beaconFinality.beaconSlot must be positive");
      }
      normalized.put("beaconSlot", Long.toString(normalizedBeaconSlot));
    }
    final Object finalityBranchInput =
        strictFirstPresent(
            finality,
            "beaconFinality.finalityBranch",
            "finalityBranch",
            "finality_branch");
    if (finalityBranchInput != null) {
      normalized.put(
          "finalityBranch",
          normalizeFinalityBranch(finalityBranchInput, "beaconFinality.finalityBranch"));
    }
    final Object syncCommitteeBitsInput =
        strictFirstPresent(
            finality,
            "beaconFinality.syncCommitteeBits",
            "syncCommitteeBits",
            "sync_committee_bits");
    String normalizedSyncCommitteeBits = null;
    if (syncCommitteeBitsInput != null) {
      normalizedSyncCommitteeBits =
          normalizeFinalitySyncCommitteeBits(
              syncCommitteeBitsInput, "beaconFinality.syncCommitteeBits");
      normalized.put(
          "syncCommitteeBits",
          normalizedSyncCommitteeBits);
    }
    final Object syncCommitteeSignatureInput =
        strictFirstPresent(
            finality,
            "beaconFinality.syncCommitteeSignature",
            "syncCommitteeSignature",
            "sync_committee_signature");
    if (syncCommitteeSignatureInput != null) {
      normalized.put(
          "syncCommitteeSignature",
          normalizeRpcHex(
              syncCommitteeSignatureInput, "beaconFinality.syncCommitteeSignature", 96));
    }
    final Object syncSignatureSlotInput =
        strictFirstPresent(
            finality,
            "beaconFinality.syncSignatureSlot",
            "syncSignatureSlot",
            "sync_signature_slot",
            "signatureSlot",
            "signature_slot");
    Long normalizedSyncSignatureSlot = null;
    if (syncSignatureSlotInput != null) {
      normalizedSyncSignatureSlot =
          normalizeUnsignedInteger(syncSignatureSlotInput, "beaconFinality.syncSignatureSlot");
      if (normalizedSyncSignatureSlot == 0) {
        throw new IllegalArgumentException("beaconFinality.syncSignatureSlot must be positive");
      }
      normalized.put("syncSignatureSlot", Long.toString(normalizedSyncSignatureSlot));
    }
    if (normalizedBeaconSlot != null
        && normalizedSyncSignatureSlot != null
        && normalizedSyncSignatureSlot < normalizedBeaconSlot) {
      throw new IllegalArgumentException(
          "beaconFinality.syncSignatureSlot must cover beaconFinality.beaconSlot");
    }
    final Object syncCommitteeParticipationInput =
        strictFirstPresent(
            finality,
            "beaconFinality.syncCommitteeParticipation",
            "syncCommitteeParticipation",
            "sync_committee_participation");
    Long normalizedSyncCommitteeParticipation = null;
    if (syncCommitteeParticipationInput != null) {
      normalizedSyncCommitteeParticipation =
          normalizeUnsignedInteger(
              syncCommitteeParticipationInput, "beaconFinality.syncCommitteeParticipation");
      if (normalizedSyncCommitteeParticipation == 0) {
        throw new IllegalArgumentException(
            "beaconFinality.syncCommitteeParticipation must be positive");
      }
      normalized.put(
          "syncCommitteeParticipation",
          Long.toString(normalizedSyncCommitteeParticipation));
    }
    if (normalizedSyncCommitteeBits != null
        && normalizedSyncCommitteeParticipation != null
        && finalitySyncCommitteeParticipation(normalizedSyncCommitteeBits)
            != normalizedSyncCommitteeParticipation) {
      throw new IllegalArgumentException(
          "beaconFinality.syncCommitteeParticipation must match syncCommitteeBits");
    }
    return Collections.unmodifiableMap(normalized);
  }

  private static String normalizeFinalitySyncCommitteeBits(
      final Object value, final String label) {
    final String bits = normalizeRpcHex(value, label, 64, true);
    final long participation = finalitySyncCommitteeParticipation(bits);
    if (participation == 0) {
      throw new IllegalArgumentException(label + " must contain at least one participant");
    }
    if (participation * 3 < 512 * 2) {
      throw new IllegalArgumentException(
          label + " must contain Ethereum sync committee supermajority");
    }
    return bits;
  }

  private static List<String> normalizeFinalityBranch(
      final Object value, final String label) {
    if (!(value instanceof List<?>)) {
      throw new IllegalArgumentException(label + " must be an array");
    }
    final List<?> input = (List<?>) value;
    if (input.size() != 6) {
      throw new IllegalArgumentException(label + " must contain 6 siblings");
    }
    final List<String> branch = new ArrayList<>();
    for (int index = 0; index < input.size(); index++) {
      branch.add(normalizeRpcHex(input.get(index), label + "[" + index + "]", 32, true));
    }
    return Collections.unmodifiableList(branch);
  }

  private static long finalitySyncCommitteeParticipation(final String bits) {
    final String text = bits.substring(2);
    long count = 0;
    for (int index = 0; index < text.length(); index += 2) {
      int value = Integer.parseInt(text.substring(index, index + 2), 16);
      while (value != 0) {
        count += value & 1;
        value >>>= 1;
      }
    }
    return count;
  }

  private static String normalizeBeaconRestEndpoint(final String endpoint) {
    if (endpoint == null || endpoint.trim().length() != endpoint.length() || endpoint.isEmpty()) {
      throw new IllegalArgumentException(
          "Ethereum mainnet Beacon REST endpoint must be a non-empty URL");
    }
    try {
      final URL url = new URL(endpoint);
      if (!"http".equals(url.getProtocol()) && !"https".equals(url.getProtocol())) {
        throw new IllegalArgumentException(
            "Ethereum mainnet Beacon REST endpoint must use http or https");
      }
      final int fragmentIndex = endpoint.indexOf('#');
      return fragmentIndex < 0 ? endpoint : endpoint.substring(0, fragmentIndex);
    } catch (final java.net.MalformedURLException ex) {
      throw new IllegalArgumentException("Ethereum mainnet Beacon REST endpoint must be a valid URL", ex);
    }
  }

  private static String beaconRestUrl(final String endpoint, final String path) {
    try {
      final URL url = new URL(endpoint);
      String basePath = url.getPath();
      while (basePath.endsWith("/")) {
        basePath = basePath.substring(0, basePath.length() - 1);
      }
      final String apiPath;
      if (basePath.matches(".*/eth/v[0-9]+") && path.matches("/eth/v[0-9]+/.*")) {
        apiPath = basePath.replaceFirst("/eth/v[0-9]+$", "") + path;
      } else {
        apiPath = basePath + path;
      }
      final String query = url.getQuery() == null ? "" : "?" + url.getQuery();
      return url.getProtocol() + "://" + url.getAuthority() + apiPath + query;
    } catch (final java.net.MalformedURLException ex) {
      throw new IllegalArgumentException("Ethereum mainnet Beacon REST endpoint must be a valid URL", ex);
    }
  }

  private static byte[] readAll(final InputStream stream) throws java.io.IOException {
    try (InputStream input = stream; ByteArrayOutputStream out = new ByteArrayOutputStream()) {
      final byte[] buffer = new byte[8192];
      int total = 0;
      int read;
      while ((read = input.read(buffer)) >= 0) {
        total += read;
        if (total > BEACON_REST_MAX_RESPONSE_BYTES) {
          throw new IllegalArgumentException(
              "Ethereum mainnet Beacon REST response body must be at most "
                  + BEACON_REST_MAX_RESPONSE_BYTES
                  + " bytes");
        }
        out.write(buffer, 0, read);
      }
      return out.toByteArray();
    }
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> expectBeaconRestObject(final Object value, final String label) {
    if (!(value instanceof Map<?, ?>)) {
      throw new IllegalArgumentException(label + " must be an object");
    }
    return (Map<String, Object>) value;
  }

  private static Object requireBeaconRestField(
      final Map<String, Object> value, final String label, final String field) {
    if (!value.containsKey(field)) {
      throw new IllegalArgumentException(label + "." + field + " is required");
    }
    return value.get(field);
  }

  private static void rejectUnsafeBeaconRestPayload(
      final Map<String, Object> payload, final String label) {
    final Boolean executionOptimistic =
        optionalBeaconRestBoolean(payload, "execution_optimistic", label);
    final Boolean executionOptimisticAlias =
        optionalBeaconRestBoolean(payload, "executionOptimistic", label);
    final Boolean finalized = optionalBeaconRestBoolean(payload, "finalized", label);
    if (Boolean.TRUE.equals(executionOptimistic)
        || Boolean.TRUE.equals(executionOptimisticAlias)) {
      throw new IllegalArgumentException(label + " must not be execution optimistic");
    }
    if (Boolean.FALSE.equals(finalized)) {
      throw new IllegalArgumentException(label + " must be finalized");
    }
  }

  private static void rejectNonBooleanBeaconRestCanonical(
      final Map<String, Object> payload, final String label) {
    if (Boolean.FALSE.equals(optionalBeaconRestBoolean(payload, "canonical", label))) {
      throw new IllegalArgumentException(label + " must be canonical");
    }
  }

  private static Boolean optionalBeaconRestBoolean(
      final Map<String, Object> payload, final String field, final String label) {
    if (!payload.containsKey(field)) {
      return null;
    }
    final Object value = payload.get(field);
    if (!(value instanceof Boolean)) {
      throw new IllegalArgumentException(label + "." + field + " must be a boolean");
    }
    return (Boolean) value;
  }

  private static String resolveBeaconRestSyncCommitteeRoot(
      final String syncCommitteeRoot, final byte[] syncCommitteePayload) {
    final String payloadRoot =
        syncCommitteePayload == null
            ? null
            : SourceSccpProofs.ethSyncCommitteeHashFromPayload(
                Arrays.copyOf(syncCommitteePayload, syncCommitteePayload.length));
    if (syncCommitteeRoot != null) {
      final String normalizedRoot = normalizeRpcHex(syncCommitteeRoot, "syncCommitteeRoot", 32);
      if (payloadRoot != null && !payloadRoot.equals(normalizedRoot)) {
        throw new IllegalArgumentException("syncCommitteeRoot must match syncCommitteePayload");
      }
      return normalizedRoot;
    }
    if (payloadRoot != null) {
      return payloadRoot;
    }
    throw new IllegalArgumentException(
        "Ethereum mainnet Beacon REST provider requires syncCommitteeRoot or syncCommitteePayload");
  }

  /** Normalized SCCP source event material recovered from a receipt log. */
  private record SourceEvent(String sourceEventDigest, String sourceBridgeEmitterAddress) {}

  /** App-supplied Ethereum JSON-RPC execution provider for native SCCP evidence collection. */
  public interface ExecutionProvider {
    Object request(String method, List<Object> params);
  }

  /** App-supplied Ethereum Beacon REST finality collector for native SCCP evidence collection. */
  public interface ConsensusProvider {
    Map<String, Object> collectFinalityEvidence(
        Map<String, Object> receipt, Map<String, Object> block, String transactionHash);
  }

  /** Minimal HTTP response used by the Ethereum Beacon REST consensus provider. */
  public record BeaconRestResponse(int statusCode, byte[] body, String statusMessage) {
    public BeaconRestResponse {
      body = body == null ? new byte[0] : Arrays.copyOf(body, body.length);
    }

    public BeaconRestResponse(final int statusCode, final byte[] body) {
      this(statusCode, body, null);
    }

    @Override
    public byte[] body() {
      return Arrays.copyOf(body, body.length);
    }
  }

  /** Injectable Beacon REST transport for tests and app-controlled networking stacks. */
  public interface BeaconRestTransport {
    BeaconRestResponse get(String url, Map<String, String> headers);
  }

  /** JDK-only Beacon REST transport for native Ethereum mainnet SCCP finality collection. */
  public static final class BeaconRestHttpTransport implements BeaconRestTransport {
    @Override
    public BeaconRestResponse get(final String url, final Map<String, String> headers) {
      try {
        final HttpURLConnection connection = (HttpURLConnection) new URL(url).openConnection();
        try {
          connection.setRequestMethod("GET");
          for (final Map.Entry<String, String> header :
              (headers == null ? Collections.<String, String>emptyMap() : headers).entrySet()) {
            connection.setRequestProperty(header.getKey(), header.getValue());
          }
          final int statusCode = connection.getResponseCode();
          final InputStream stream =
              statusCode >= 200 && statusCode <= 299
                  ? connection.getInputStream()
                  : connection.getErrorStream();
          final byte[] body = stream == null ? new byte[0] : readAll(stream);
          return new BeaconRestResponse(statusCode, body, connection.getResponseMessage());
        } finally {
          connection.disconnect();
        }
      } catch (final java.io.IOException ex) {
        throw new IllegalStateException("Ethereum mainnet Beacon REST request failed", ex);
      }
    }
  }

  /** Beacon REST-backed Ethereum mainnet finality collector for local-first SDK flows. */
  public static final class BeaconRestConsensusProvider implements ConsensusProvider {
    private final String endpoint;
    private final String syncCommitteeRoot;
    private final byte[] syncCommitteePayload;
    private final Map<String, String> headers;
    private final boolean verifyFinalityCheckpoint;
    private final BeaconRestTransport transport;

    public BeaconRestConsensusProvider(final String endpoint, final String syncCommitteeRoot) {
      this(
          endpoint,
          syncCommitteeRoot,
          null,
          Collections.emptyMap(),
          true,
          new BeaconRestHttpTransport());
    }

    public BeaconRestConsensusProvider(
        final String endpoint,
        final String syncCommitteeRoot,
        final byte[] syncCommitteePayload,
        final Map<String, String> headers,
        final boolean verifyFinalityCheckpoint,
        final BeaconRestTransport transport) {
      this.endpoint = normalizeBeaconRestEndpoint(endpoint);
      this.syncCommitteeRoot = syncCommitteeRoot;
      this.syncCommitteePayload =
          syncCommitteePayload == null
              ? null
              : Arrays.copyOf(syncCommitteePayload, syncCommitteePayload.length);
      this.headers =
          Collections.unmodifiableMap(
              new java.util.LinkedHashMap<>(
                  headers == null ? Collections.emptyMap() : headers));
      this.verifyFinalityCheckpoint = verifyFinalityCheckpoint;
      this.transport = Objects.requireNonNull(transport, "transport");
    }

    @Override
    public Map<String, Object> collectFinalityEvidence(
        final Map<String, Object> receipt,
        final Map<String, Object> block,
        final String transactionHash) {
      if (block == null) {
        throw new IllegalArgumentException(
            "Ethereum mainnet Beacon REST finality collection requires block");
      }
      final String blockHash = normalizeRpcHex(block.get("hash"), "block.hash", 32);
      final String blockNumber =
          normalizeRpcQuantity(
              strictFirstPresent(block, "block.number", "number", "blockNumber", "block_number"),
              "block.number");
      if ("0x0".equals(blockNumber)) {
        throw new IllegalArgumentException("block.number must be positive");
      }
      final String receiptsRoot =
          normalizeRpcHex(
              strictFirstPresent(block, "block.receiptsRoot", "receiptsRoot", "receipts_root"),
              "block.receiptsRoot",
              32);
      final BeaconRestBlockId targetBlockId = beaconRestBlockIdForTarget(block);
      final Map<String, Object> finalizedHeaderResponse =
          fetchJsonObject("/eth/v1/beacon/headers/finalized", "Ethereum mainnet Beacon REST finalized header");
      final BeaconRestHeaderSummary finalizedHeader =
          beaconRestHeaderSummary(
              finalizedHeaderResponse, "Ethereum mainnet Beacon REST finalized header");
      final BeaconRestHeaderSummary targetHeader;
      if ("finalized".equals(targetBlockId.id)) {
        targetHeader = finalizedHeader;
      } else {
        targetHeader =
            beaconRestHeaderSummary(
                fetchJsonObject(
                    "/eth/v1/beacon/headers/" + targetBlockId.id,
                    "Ethereum mainnet Beacon REST finalized target header"),
                "Ethereum mainnet Beacon REST finalized target header");
      }
      if (targetBlockId.slot != null && targetHeader.slot != targetBlockId.slot.longValue()) {
        throw new IllegalArgumentException(
            "Ethereum mainnet Beacon REST finalized target header slot must match beaconSlot");
      }
      if (targetBlockId.root != null && !targetHeader.root.equals(targetBlockId.root)) {
        throw new IllegalArgumentException(
            "Ethereum mainnet Beacon REST finalized target header root must match beaconBlockRoot");
      }
      if (targetHeader.slot > finalizedHeader.slot) {
        throw new IllegalArgumentException(
            "Ethereum mainnet Beacon REST target block is newer than the finalized header");
      }
      if (targetHeader.slot < finalizedHeader.slot) {
        throw new IllegalArgumentException(
            "Ethereum mainnet Beacon REST historical target blocks require an ancestry proof");
      }
      if (targetHeader.slot == finalizedHeader.slot && !targetHeader.root.equals(finalizedHeader.root)) {
        throw new IllegalArgumentException(
            "Ethereum mainnet Beacon REST target header root must match finalized header root at the same slot");
      }
      final Map<String, Object> finalizedBlockRootResponse =
          fetchJsonObject(
              "/eth/v1/beacon/blocks/" + targetBlockId.id + "/root",
              "Ethereum mainnet Beacon REST finalized block root");
      rejectUnsafeBeaconRestPayload(
          finalizedBlockRootResponse, "Ethereum mainnet Beacon REST finalized block root");
      final Map<String, Object> finalizedBlockRootData =
          expectBeaconRestObject(
              requireBeaconRestField(
                  finalizedBlockRootResponse,
                  "Ethereum mainnet Beacon REST finalized block root",
                  "data"),
              "Ethereum mainnet Beacon REST finalized block root.data");
      final String finalizedBlockRootHash =
          normalizeRpcHex(
              requireBeaconRestField(
                  finalizedBlockRootData,
                  "Ethereum mainnet Beacon REST finalized block root.data",
                  "root"),
              "finalizedBlockRoot",
              32);
      if (!finalizedBlockRootHash.equals(targetHeader.root)) {
        throw new IllegalArgumentException(
            "Ethereum mainnet Beacon REST finalized block root must match finalized header root");
      }
      final Map<String, Object> finalizedBlockRoot =
          fetchJsonObject(
              "/eth/v2/beacon/blocks/" + targetBlockId.id,
              "Ethereum mainnet Beacon REST finalized block");
      rejectUnsafeBeaconRestPayload(
          finalizedBlockRoot, "Ethereum mainnet Beacon REST finalized block");
      final Map<String, Object> finalizedBlockData =
          expectBeaconRestObject(
              requireBeaconRestField(
                  finalizedBlockRoot, "Ethereum mainnet Beacon REST finalized block", "data"),
              "Ethereum mainnet Beacon REST finalized block.data");
      final Map<String, Object> finalizedBlockMessage =
          expectBeaconRestObject(
              requireBeaconRestField(
                  finalizedBlockData,
                  "Ethereum mainnet Beacon REST finalized block.data",
                  "message"),
              "Ethereum mainnet Beacon REST finalized block.data.message");
      final long finalizedBlockSlot =
          normalizeUnsignedInteger(
              requireBeaconRestField(
                  finalizedBlockMessage,
                  "Ethereum mainnet Beacon REST finalized block.data.message",
                  "slot"),
              "Ethereum mainnet Beacon REST finalized block.data.message.slot");
      if (finalizedBlockSlot != targetHeader.slot) {
        throw new IllegalArgumentException(
            "Ethereum mainnet Beacon REST finalized block slot must match finalized header slot");
      }
      final Map<String, Object> finalizedBlockBody =
          expectBeaconRestObject(
              requireBeaconRestField(
                  finalizedBlockMessage,
                  "Ethereum mainnet Beacon REST finalized block.data.message",
                  "body"),
              "Ethereum mainnet Beacon REST finalized block.data.message.body");
      final Map<String, Object> executionPayload =
          expectBeaconRestObject(
              requireBeaconRestField(
                  finalizedBlockBody,
                  "Ethereum mainnet Beacon REST finalized block.data.message.body",
                  "execution_payload"),
              "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload");
      final String payloadBlockHash =
          normalizeRpcHex(
              requireBeaconRestField(
                  executionPayload,
                  "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload",
                  "block_hash"),
              "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload.block_hash",
              32);
      if (!payloadBlockHash.equals(blockHash)) {
        throw new IllegalArgumentException(
            "Ethereum mainnet Beacon REST execution payload block_hash must match block.hash");
      }
      final long payloadBlockNumber =
          normalizeUnsignedInteger(
              requireBeaconRestField(
                  executionPayload,
                  "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload",
                  "block_number"),
              "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload.block_number");
      if (payloadBlockNumber != normalizeUnsignedInteger(blockNumber, "block.number")) {
        throw new IllegalArgumentException(
            "Ethereum mainnet Beacon REST execution payload block_number must match block.number");
      }
      final String payloadReceiptsRoot =
          normalizeRpcHex(
              requireBeaconRestField(
                  executionPayload,
                  "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload",
                  "receipts_root"),
              "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload.receipts_root",
              32);
      if (!payloadReceiptsRoot.equals(receiptsRoot)) {
        throw new IllegalArgumentException(
            "Ethereum mainnet Beacon REST execution payload receipts_root must match block.receiptsRoot");
      }
      if (verifyFinalityCheckpoint) {
        final Map<String, Object> checkpointRoot =
            fetchJsonObject(
                "/eth/v1/beacon/states/finalized/finality_checkpoints",
                "Ethereum mainnet Beacon REST finality checkpoints");
        rejectUnsafeBeaconRestPayload(
            checkpointRoot, "Ethereum mainnet Beacon REST finality checkpoints");
        final Map<String, Object> checkpointData =
            expectBeaconRestObject(
                requireBeaconRestField(
                    checkpointRoot, "Ethereum mainnet Beacon REST finality checkpoints", "data"),
                "Ethereum mainnet Beacon REST finality checkpoints.data");
        final Map<String, Object> finalizedCheckpoint =
            expectBeaconRestObject(
                requireBeaconRestField(
                    checkpointData,
                    "Ethereum mainnet Beacon REST finality checkpoints.data",
                    "finalized"),
                "Ethereum mainnet Beacon REST finality checkpoints.data.finalized");
        final String finalizedCheckpointRoot =
            normalizeRpcHex(
                requireBeaconRestField(
                    finalizedCheckpoint,
                    "Ethereum mainnet Beacon REST finality checkpoints.data.finalized",
                    "root"),
                "finalizedCheckpointRoot",
                32);
        if (!finalizedCheckpointRoot.equals(finalizedHeader.root)) {
          throw new IllegalArgumentException(
              "Ethereum mainnet Beacon REST finality checkpoint root must match finalized header root");
        }
      }
      final BeaconRestFinalityUpdateSummary finalityUpdate =
          beaconRestFinalityUpdateSummary(
              fetchJsonObject(
                  "/eth/v1/beacon/light_client/finality_update",
                  "Ethereum mainnet Beacon REST light-client finality update"),
              finalizedHeader.slot);
      final java.util.LinkedHashMap<String, Object> evidence = new java.util.LinkedHashMap<>();
      evidence.put(
          "executionBlockNumber",
          Long.toString(normalizeUnsignedInteger(blockNumber, "block.number")));
      evidence.put("executionBlockHash", blockHash);
      evidence.put("executionReceiptsRoot", receiptsRoot);
      evidence.put("finalizedHeaderRoot", targetHeader.root);
      evidence.put(
          "syncCommitteeRoot",
          resolveBeaconRestSyncCommitteeRoot(syncCommitteeRoot, syncCommitteePayload));
      evidence.put("beaconSlot", Long.toString(targetHeader.slot));
      evidence.put("finalityBranch", finalityUpdate.finalityBranch);
      evidence.put("syncCommitteeBits", finalityUpdate.syncCommitteeBits);
      evidence.put("syncCommitteeSignature", finalityUpdate.syncCommitteeSignature);
      evidence.put(
          "syncCommitteeParticipation",
          Long.toString(finalityUpdate.syncCommitteeParticipation));
      evidence.put("syncSignatureSlot", Long.toString(finalityUpdate.syncSignatureSlot));
      return Collections.unmodifiableMap(evidence);
    }

    private BeaconRestBlockId beaconRestBlockIdForTarget(final Map<String, Object> block) {
      final Object rootInput =
          firstPresent(
              block,
              "beaconBlockRoot",
              "beacon_block_root",
              "targetBeaconBlockRoot",
              "target_beacon_block_root");
      if (rootInput != null) {
        final String root = normalizeRpcHex(rootInput, "block.beaconBlockRoot", 32);
        return new BeaconRestBlockId(root, null, root);
      }
      final Object idInput =
          firstPresent(
              block,
              "beaconBlockId",
              "beacon_block_id",
              "targetBeaconBlockId",
              "target_beacon_block_id");
      if (idInput != null) {
        return beaconRestBlockIdFromValue(idInput, "block.beaconBlockId");
      }
      final Object slotInput =
          firstPresent(block, "beaconSlot", "beacon_slot", "finalizedSlot", "finalized_slot", "slot");
      if (slotInput != null) {
        final long slot = normalizeBeaconSlot(slotInput, "block.beaconSlot");
        return new BeaconRestBlockId(Long.toString(slot), Long.valueOf(slot), null);
      }
      final Object timestampInput = firstPresent(block, "timestamp", "blockTimestamp", "block_timestamp");
      if (timestampInput != null) {
        final long timestamp = normalizeUnsignedInteger(timestampInput, "block.timestamp");
        final long genesisTime = beaconRestGenesisTime();
        if (timestamp < genesisTime) {
          throw new IllegalArgumentException("block.timestamp must not be before Beacon genesis time");
        }
        final long elapsed = timestamp - genesisTime;
        if (elapsed % ETHEREUM_MAINNET_SECONDS_PER_SLOT != 0) {
          throw new IllegalArgumentException(
              "block.timestamp must align to an Ethereum mainnet Beacon slot");
        }
        final long slot = elapsed / ETHEREUM_MAINNET_SECONDS_PER_SLOT;
        if (slot == 0) {
          throw new IllegalArgumentException("beaconFinality.beaconSlot must be positive");
        }
        return new BeaconRestBlockId(Long.toString(slot), Long.valueOf(slot), null);
      }
      return new BeaconRestBlockId("finalized", null, null);
    }

    private long beaconRestGenesisTime() {
      final Map<String, Object> genesis =
          fetchJsonObject(
              "/eth/v1/beacon/genesis",
              "Ethereum mainnet Beacon REST genesis");
      final Map<String, Object> data =
          expectBeaconRestObject(
              requireBeaconRestField(genesis, "Ethereum mainnet Beacon REST genesis", "data"),
              "Ethereum mainnet Beacon REST genesis.data");
      return normalizeUnsignedInteger(
          requireBeaconRestField(
              data,
              "Ethereum mainnet Beacon REST genesis.data",
              "genesis_time"),
          "Ethereum mainnet Beacon REST genesis.data.genesis_time");
    }

    private static BeaconRestHeaderSummary beaconRestHeaderSummary(
        final Map<String, Object> payload, final String label) {
      rejectUnsafeBeaconRestPayload(payload, label);
      final Map<String, Object> headerData =
          expectBeaconRestObject(
              requireBeaconRestField(payload, label, "data"), label + ".data");
      rejectNonBooleanBeaconRestCanonical(headerData, label);
      final String root =
          normalizeRpcHex(
              requireBeaconRestField(headerData, label + ".data", "root"),
              label.contains("target") ? "targetHeaderRoot" : "finalizedHeaderRoot",
              32);
      final Map<String, Object> header =
          expectBeaconRestObject(
              requireBeaconRestField(headerData, label + ".data", "header"),
              label + ".data.header");
      final Map<String, Object> message =
          expectBeaconRestObject(
              requireBeaconRestField(header, label + ".data.header", "message"),
              label + ".data.header.message");
      for (final String field : Arrays.asList("parent_root", "state_root", "body_root")) {
        normalizeRpcHex(
            requireBeaconRestField(message, label + ".data.header.message", field),
            label + ".data.header.message." + field,
            32);
      }
      normalizeRpcHex(
          requireBeaconRestField(header, label + ".data.header", "signature"),
          label + ".data.header.signature",
          96);
      final long slot =
          normalizeBeaconSlot(
              requireBeaconRestField(message, label + ".data.header.message", "slot"),
              "beaconFinality.beaconSlot");
      return new BeaconRestHeaderSummary(root, slot);
    }

    private static BeaconRestFinalityUpdateSummary beaconRestFinalityUpdateSummary(
        final Map<String, Object> payload, final long expectedFinalizedSlot) {
      final String label = "Ethereum mainnet Beacon REST light-client finality update";
      rejectUnsafeBeaconRestPayload(payload, label);
      final Map<String, Object> data =
          expectBeaconRestObject(requireBeaconRestField(payload, label, "data"), label + ".data");
      final Map<String, Object> finalizedHeader =
          expectBeaconRestObject(
              requireBeaconRestField(data, label + ".data", "finalized_header"),
              label + ".data.finalized_header");
      final Map<String, Object> finalizedBeacon =
          expectBeaconRestObject(
              requireBeaconRestField(
                  finalizedHeader, label + ".data.finalized_header", "beacon"),
              label + ".data.finalized_header.beacon");
      final long finalizedSlot =
          normalizeBeaconSlot(
              requireBeaconRestField(
                  finalizedBeacon, label + ".data.finalized_header.beacon", "slot"),
              label + ".data.finalized_header.beacon.slot");
      if (finalizedSlot != expectedFinalizedSlot) {
        throw new IllegalArgumentException(
            "Ethereum mainnet Beacon REST finality update finalized_header slot must match finalized header slot");
      }
      final long syncSignatureSlot =
          normalizeBeaconSlot(
              requireBeaconRestField(data, label + ".data", "signature_slot"),
              label + ".data.signature_slot");
      if (syncSignatureSlot < expectedFinalizedSlot) {
        throw new IllegalArgumentException(
            "Ethereum mainnet Beacon REST finality update signature_slot must cover finalized header slot");
      }
      final Map<String, Object> syncAggregate =
          expectBeaconRestObject(
              requireBeaconRestField(data, label + ".data", "sync_aggregate"),
              label + ".data.sync_aggregate");
      final String syncCommitteeBits =
          normalizeBeaconRestSyncCommitteeBits(
              requireBeaconRestField(
                  syncAggregate, label + ".data.sync_aggregate", "sync_committee_bits"),
              label + ".data.sync_aggregate.sync_committee_bits");
      final List<String> finalityBranch =
          normalizeBeaconRestFinalityBranch(
              requireBeaconRestField(data, label + ".data", "finality_branch"),
              label + ".data.finality_branch");
      final String syncCommitteeSignature =
          normalizeRpcHex(
              requireBeaconRestField(
                  syncAggregate, label + ".data.sync_aggregate", "sync_committee_signature"),
              label + ".data.sync_aggregate.sync_committee_signature",
              96);
      return new BeaconRestFinalityUpdateSummary(
          finalityBranch,
          syncCommitteeBits,
          syncCommitteeSignature,
          beaconRestSyncCommitteeParticipation(syncCommitteeBits),
          syncSignatureSlot);
    }

    private static String normalizeBeaconRestSyncCommitteeBits(
        final Object value, final String label) {
      final String bits = normalizeRpcHex(value, label, 64, true);
      final long participation = beaconRestSyncCommitteeParticipation(bits);
      if (participation == 0) {
        throw new IllegalArgumentException(label + " must contain at least one participant");
      }
      if (participation * 3 < 512 * 2) {
        throw new IllegalArgumentException(
            label + " must contain Ethereum sync committee supermajority");
      }
      return bits;
    }

    private static List<String> normalizeBeaconRestFinalityBranch(
        final Object value, final String label) {
      if (!(value instanceof List<?>)) {
        throw new IllegalArgumentException(label + " must be an array");
      }
      final List<?> input = (List<?>) value;
      if (input.size() != 6) {
        throw new IllegalArgumentException(label + " must contain 6 siblings");
      }
      final List<String> branch = new ArrayList<>();
      for (int index = 0; index < input.size(); index++) {
        branch.add(normalizeRpcHex(input.get(index), label + "[" + index + "]", 32, true));
      }
      return Collections.unmodifiableList(branch);
    }

    private static long beaconRestSyncCommitteeParticipation(final String bits) {
      final String text = bits.substring(2);
      long count = 0;
      for (int index = 0; index < text.length(); index += 2) {
        int value = Integer.parseInt(text.substring(index, index + 2), 16);
        while (value != 0) {
          count += value & 1;
          value >>>= 1;
        }
      }
      return count;
    }

    private static BeaconRestBlockId beaconRestBlockIdFromValue(
        final Object value, final String label) {
      if (value instanceof String) {
        final String text = (String) value;
        if (text.trim().equals(text) && text.startsWith("0x") && text.length() == 66) {
          final String root = normalizeRpcHex(text, label, 32);
          return new BeaconRestBlockId(root, null, root);
        }
      }
      final long slot = normalizeBeaconSlot(value, label);
      return new BeaconRestBlockId(Long.toString(slot), Long.valueOf(slot), null);
    }

    private static long normalizeBeaconSlot(final Object value, final String label) {
      final long slot = normalizeUnsignedInteger(value, label);
      if (slot == 0) {
        throw new IllegalArgumentException("beaconFinality.beaconSlot must be positive");
      }
      return slot;
    }

    private Map<String, Object> fetchJsonObject(final String path, final String label) {
      final BeaconRestResponse response = transport.get(beaconRestUrl(endpoint, path), headers);
      if (response.statusCode() < 200 || response.statusCode() > 299) {
        final String suffix =
            response.statusMessage() == null ? "" : " " + response.statusMessage();
        throw new IllegalArgumentException(
            label + " request failed " + response.statusCode() + suffix);
      }
      if (response.body().length > BEACON_REST_MAX_RESPONSE_BYTES) {
        throw new IllegalArgumentException(
            label + " response body must be at most " + BEACON_REST_MAX_RESPONSE_BYTES + " bytes");
      }
      final Object parsed = JsonParser.parse(new String(response.body(), StandardCharsets.UTF_8));
      return expectBeaconRestObject(parsed, label + " response JSON");
    }
  }

  /** Typed Ethereum beacon finality evidence required before inbound source proving. */
  public record BeaconFinalityEvidence(
      String executionBlockNumber,
      String executionBlockHash,
      String executionReceiptsRoot,
      String beaconSlot,
      String syncCommitteeBits,
      String syncCommitteeSignature,
      String syncCommitteeParticipation,
      String syncSignatureSlot,
      Map<String, Object> additionalFields) {
    public BeaconFinalityEvidence(
        final String executionBlockNumber,
        final String executionBlockHash,
        final String executionReceiptsRoot) {
      this(
          executionBlockNumber,
          executionBlockHash,
          executionReceiptsRoot,
          null,
          null,
          null,
          null,
          null,
          Collections.emptyMap());
    }

    public BeaconFinalityEvidence(
        final String executionBlockNumber,
        final String executionBlockHash,
        final String executionReceiptsRoot,
        final Map<String, Object> additionalFields) {
      this(
          executionBlockNumber,
          executionBlockHash,
          executionReceiptsRoot,
          null,
          null,
          null,
          null,
          null,
          additionalFields);
    }

    public Map<String, Object> toMap() {
      final java.util.LinkedHashMap<String, Object> value =
          new java.util.LinkedHashMap<>(
              additionalFields == null ? Collections.emptyMap() : additionalFields);
      value.put("executionBlockNumber", executionBlockNumber);
      value.put("executionBlockHash", executionBlockHash);
      value.put("executionReceiptsRoot", executionReceiptsRoot);
      if (beaconSlot != null) {
        value.put("beaconSlot", beaconSlot);
      }
      if (syncCommitteeBits != null) {
        value.put("syncCommitteeBits", syncCommitteeBits);
      }
      if (syncCommitteeSignature != null) {
        value.put("syncCommitteeSignature", syncCommitteeSignature);
      }
      if (syncCommitteeParticipation != null) {
        value.put("syncCommitteeParticipation", syncCommitteeParticipation);
      }
      if (syncSignatureSlot != null) {
        value.put("syncSignatureSlot", syncSignatureSlot);
      }
      return Collections.unmodifiableMap(value);
    }
  }

  /** Local Ethereum mainnet inbound source prover linked by the application bundle. */
  public interface InboundProver {
    byte[] prove(InboundEvidence evidence);
  }

  /** App-supplied Torii submitter for locally generated Ethereum inbound proofs. */
  public interface InboundSubmitter {
    Object submit(byte[] proofBytes);
  }

  /** App-supplied Ethereum transaction submitter for locally generated outbound proof calldata. */
  public interface OutboundSubmitter {
    Object submit(EvmSccpProver.Submission submission);
  }

  /** Input for Ethereum mainnet -> SORA local-admission submission packaging. */
  public record LocalAdmissionSubmissionInput(
      byte[] proofBytes,
      byte[] publicInputsBytes,
      byte[] bundleBytes,
      byte[] envelopeBytes,
      String statementHash,
      String sourceVerifierMaterialHash,
      String sourceAdapterEngineDeploymentHash,
      int sourceDomain,
      int targetDomain,
      String proofFamily,
      String verifierBackend,
      String envelopeEncoding,
      String submissionKind,
      String verifierEntrypoint) {
    public LocalAdmissionSubmissionInput(
        final byte[] proofBytes,
        final byte[] publicInputsBytes,
        final byte[] bundleBytes,
        final byte[] envelopeBytes,
        final String statementHash,
        final String sourceVerifierMaterialHash,
        final String sourceAdapterEngineDeploymentHash) {
      this(
          proofBytes,
          publicInputsBytes,
          bundleBytes,
          envelopeBytes,
          statementHash,
          sourceVerifierMaterialHash,
          sourceAdapterEngineDeploymentHash,
          DOMAIN_ETH,
          DOMAIN_SORA,
          STARK_FRI_PROOF_FAMILY_V1,
          EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
          LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
          LOCAL_ADMISSION_SUBMISSION_KIND_V1,
          LOCAL_ADMISSION_ENTRYPOINT_V1);
    }

    public LocalAdmissionSubmissionInput {
      proofBytes = Arrays.copyOf(Objects.requireNonNull(proofBytes), proofBytes.length);
      publicInputsBytes =
          Arrays.copyOf(Objects.requireNonNull(publicInputsBytes), publicInputsBytes.length);
      bundleBytes = Arrays.copyOf(Objects.requireNonNull(bundleBytes), bundleBytes.length);
      envelopeBytes = Arrays.copyOf(Objects.requireNonNull(envelopeBytes), envelopeBytes.length);
      proofFamily = Objects.requireNonNull(proofFamily, "proofFamily");
      verifierBackend = Objects.requireNonNull(verifierBackend, "verifierBackend");
      envelopeEncoding = Objects.requireNonNull(envelopeEncoding, "envelopeEncoding");
      submissionKind = Objects.requireNonNull(submissionKind, "submissionKind");
      verifierEntrypoint = Objects.requireNonNull(verifierEntrypoint, "verifierEntrypoint");
    }

    @Override
    public byte[] proofBytes() {
      return Arrays.copyOf(proofBytes, proofBytes.length);
    }

    @Override
    public byte[] publicInputsBytes() {
      return Arrays.copyOf(publicInputsBytes, publicInputsBytes.length);
    }

    @Override
    public byte[] bundleBytes() {
      return Arrays.copyOf(bundleBytes, bundleBytes.length);
    }

    @Override
    public byte[] envelopeBytes() {
      return Arrays.copyOf(envelopeBytes, envelopeBytes.length);
    }
  }

  /** Ethereum mainnet local-admission payload mirrored from the core SCCP package. */
  public record LocalAdmissionPayload(
      int version,
      byte[] proofBytes,
      byte[] publicInputsBytes,
      byte[] bundleBytes,
      String statementHash,
      String sourceVerifierMaterialHash,
      String sourceAdapterEngineDeploymentHash,
      String proofBytesHex,
      String publicInputsBytesHex,
      String bundleBytesHex) {
    public LocalAdmissionPayload(
        final byte[] proofBytes,
        final byte[] publicInputsBytes,
        final byte[] bundleBytes,
        final String statementHash,
        final String sourceVerifierMaterialHash,
        final String sourceAdapterEngineDeploymentHash) {
      this(
          1,
          proofBytes,
          publicInputsBytes,
          bundleBytes,
          statementHash,
          sourceVerifierMaterialHash,
          sourceAdapterEngineDeploymentHash,
          "0x" + hexLower(proofBytes),
          "0x" + hexLower(publicInputsBytes),
          "0x" + hexLower(bundleBytes));
    }

    public LocalAdmissionPayload {
      proofBytes = Arrays.copyOf(Objects.requireNonNull(proofBytes), proofBytes.length);
      publicInputsBytes =
          Arrays.copyOf(Objects.requireNonNull(publicInputsBytes), publicInputsBytes.length);
      bundleBytes = Arrays.copyOf(Objects.requireNonNull(bundleBytes), bundleBytes.length);
    }

    @Override
    public byte[] proofBytes() {
      return Arrays.copyOf(proofBytes, proofBytes.length);
    }

    @Override
    public byte[] publicInputsBytes() {
      return Arrays.copyOf(publicInputsBytes, publicInputsBytes.length);
    }

    @Override
    public byte[] bundleBytes() {
      return Arrays.copyOf(bundleBytes, bundleBytes.length);
    }
  }

  /** Ethereum mainnet -> SORA local-admission package ready for Torii bridge-proof submission. */
  public record LocalAdmissionSubmission(
      int version,
      String proofFamily,
      String verifierBackend,
      String platformPayload,
      String envelopeEncoding,
      String submissionKind,
      String verifierEntrypoint,
      int sourceDomain,
      int targetDomain,
      String statementHash,
      String sourceVerifierMaterialHash,
      String sourceAdapterEngineDeploymentHash,
      List<EvmSccpProver.SubmissionArgument> arguments,
      LocalAdmissionPayload localAdmission,
      byte[] proofBytes,
      byte[] publicInputsBytes,
      byte[] bundleBytes,
      byte[] envelopeBytes,
      String proofBytesHex,
      String publicInputsBytesHex,
      String bundleBytesHex,
      String envelopeHex) {
    public LocalAdmissionSubmission(
        final String proofFamily,
        final String verifierBackend,
        final int sourceDomain,
        final int targetDomain,
        final String statementHash,
        final String sourceVerifierMaterialHash,
        final String sourceAdapterEngineDeploymentHash,
        final LocalAdmissionPayload localAdmission,
        final byte[] proofBytes,
        final byte[] publicInputsBytes,
        final byte[] bundleBytes,
        final byte[] envelopeBytes) {
      this(
          1,
          proofFamily,
          verifierBackend,
          LOCAL_ADMISSION_SUBMISSION_KIND_V1,
          LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
          LOCAL_ADMISSION_SUBMISSION_KIND_V1,
          LOCAL_ADMISSION_ENTRYPOINT_V1,
          sourceDomain,
          targetDomain,
          statementHash,
          sourceVerifierMaterialHash,
          sourceAdapterEngineDeploymentHash,
          Collections.emptyList(),
          localAdmission,
          proofBytes,
          publicInputsBytes,
          bundleBytes,
          envelopeBytes,
          "0x" + hexLower(proofBytes),
          "0x" + hexLower(publicInputsBytes),
          "0x" + hexLower(bundleBytes),
          "0x" + hexLower(envelopeBytes));
    }

    public LocalAdmissionSubmission {
      arguments =
          Collections.unmodifiableList(
              arguments == null ? Collections.emptyList() : arguments);
      proofBytes = Arrays.copyOf(Objects.requireNonNull(proofBytes), proofBytes.length);
      publicInputsBytes =
          Arrays.copyOf(Objects.requireNonNull(publicInputsBytes), publicInputsBytes.length);
      bundleBytes = Arrays.copyOf(Objects.requireNonNull(bundleBytes), bundleBytes.length);
      envelopeBytes = Arrays.copyOf(Objects.requireNonNull(envelopeBytes), envelopeBytes.length);
    }

    @Override
    public byte[] proofBytes() {
      return Arrays.copyOf(proofBytes, proofBytes.length);
    }

    @Override
    public byte[] publicInputsBytes() {
      return Arrays.copyOf(publicInputsBytes, publicInputsBytes.length);
    }

    @Override
    public byte[] bundleBytes() {
      return Arrays.copyOf(bundleBytes, bundleBytes.length);
    }

    @Override
    public byte[] envelopeBytes() {
      return Arrays.copyOf(envelopeBytes, envelopeBytes.length);
    }
  }

  /** Locally collected Ethereum mainnet inbound evidence before source-proof generation. */
  public record InboundEvidence(
      int sourceDomain,
      int targetDomain,
      String transactionHash,
      Map<String, Object> receipt,
      Map<String, Object> block,
      Map<String, Object> beaconFinality,
      ReceiptProof receiptProof,
      String receiptProofHash,
      String sourceEventDigest,
      String sourceBridgeEmitterAddress,
      List<Map<String, Object>> blockReceipts,
      List<byte[]> inclusionBranch) {
    public InboundEvidence {
      blockReceipts =
          blockReceipts == null
              ? null
              : Collections.unmodifiableList(new ArrayList<>(blockReceipts));
      inclusionBranch =
          inclusionBranch == null ? null : copyByteArrayList(inclusionBranch, "inclusionBranch");
    }

    public InboundEvidence(
        final int sourceDomain,
        final int targetDomain,
        final String transactionHash,
        final Map<String, Object> receipt,
        final Map<String, Object> block,
        final Map<String, Object> beaconFinality,
        final String receiptProofHash) {
      this(
          sourceDomain,
          targetDomain,
          transactionHash,
          receipt,
          block,
          beaconFinality,
          null,
          receiptProofHash,
          null,
          null,
          null,
          null);
    }

    public InboundEvidence(
        final int sourceDomain,
        final int targetDomain,
        final String transactionHash,
        final Map<String, Object> receipt,
        final Map<String, Object> block,
        final Map<String, Object> beaconFinality,
        final String receiptProofHash,
        final String sourceEventDigest,
        final String sourceBridgeEmitterAddress) {
      this(
          sourceDomain,
          targetDomain,
          transactionHash,
          receipt,
          block,
          beaconFinality,
          null,
          receiptProofHash,
          sourceEventDigest,
          sourceBridgeEmitterAddress,
          null,
          null);
    }

    public static InboundEvidence withBeaconFinalityEvidence(
        final int sourceDomain,
        final int targetDomain,
        final String transactionHash,
        final Map<String, Object> receipt,
        final Map<String, Object> block,
        final BeaconFinalityEvidence beaconFinalityEvidence,
        final String receiptProofHash) {
      return new InboundEvidence(
          sourceDomain,
          targetDomain,
          transactionHash,
          receipt,
          block,
          beaconFinalityEvidence == null ? null : beaconFinalityEvidence.toMap(),
          receiptProofHash);
    }

    public static InboundEvidence withBeaconFinalityEvidence(
        final int sourceDomain,
        final int targetDomain,
        final String transactionHash,
        final Map<String, Object> receipt,
        final Map<String, Object> block,
        final BeaconFinalityEvidence beaconFinalityEvidence,
        final ReceiptProof receiptProof,
        final String receiptProofHash) {
      return new InboundEvidence(
          sourceDomain,
          targetDomain,
          transactionHash,
          receipt,
          block,
          beaconFinalityEvidence == null ? null : beaconFinalityEvidence.toMap(),
          receiptProof,
          receiptProofHash,
          null,
          null,
          null,
          null);
    }

    public static InboundEvidence withBeaconFinalityEvidence(
        final int sourceDomain,
        final int targetDomain,
        final String transactionHash,
        final Map<String, Object> receipt,
        final Map<String, Object> block,
        final BeaconFinalityEvidence beaconFinalityEvidence,
        final String receiptProofHash,
        final String sourceEventDigest,
        final String sourceBridgeEmitterAddress) {
      return new InboundEvidence(
          sourceDomain,
          targetDomain,
          transactionHash,
          receipt,
          block,
          beaconFinalityEvidence == null ? null : beaconFinalityEvidence.toMap(),
          null,
          receiptProofHash,
          sourceEventDigest,
          sourceBridgeEmitterAddress,
          null,
          null);
    }

    public InboundEvidence(
        final int sourceDomain,
        final int targetDomain,
        final String transactionHash,
        final Map<String, Object> receipt,
        final Map<String, Object> block,
        final Map<String, Object> beaconFinality,
        final ReceiptProof receiptProof,
        final String receiptProofHash,
        final String sourceEventDigest,
        final String sourceBridgeEmitterAddress) {
      this(
          sourceDomain,
          targetDomain,
          transactionHash,
          receipt,
          block,
          beaconFinality,
          receiptProof,
          receiptProofHash,
          sourceEventDigest,
          sourceBridgeEmitterAddress,
          null,
          null);
    }
  }

  /** Ethereum mainnet receipt-proof transcript collected from app-supplied providers. */
  public record ReceiptProof(
      int sourceDomain,
      String sourceEventDigest,
      String beaconSlot,
      String executionBlockNumber,
      String executionBlockHash,
      String executionReceiptsRoot,
      String beaconFinalizedRoot,
      String syncCommitteeRoot,
      String receiptRootIndex,
      List<byte[]> receiptTrieProofNodes,
      List<byte[]> inclusionBranch) {
    public ReceiptProof {
      receiptTrieProofNodes = copyByteArrayList(receiptTrieProofNodes, "receiptTrieProofNodes");
      inclusionBranch = copyByteArrayList(inclusionBranch, "inclusionBranch");
    }

    @Override
    public List<byte[]> receiptTrieProofNodes() {
      return copyByteArrayList(receiptTrieProofNodes, "receiptTrieProofNodes");
    }

    @Override
    public List<byte[]> inclusionBranch() {
      return copyByteArrayList(inclusionBranch, "inclusionBranch");
    }

    public ReceiptProof(
        final String sourceEventDigest,
        final String beaconSlot,
        final String executionBlockNumber,
        final String executionBlockHash,
        final String executionReceiptsRoot,
        final String beaconFinalizedRoot,
        final String syncCommitteeRoot,
        final String receiptRootIndex,
        final List<byte[]> receiptTrieProofNodes,
        final List<byte[]> inclusionBranch) {
      this(
          DOMAIN_ETH,
          sourceEventDigest,
          beaconSlot,
          executionBlockNumber,
          executionBlockHash,
          executionReceiptsRoot,
          beaconFinalizedRoot,
          syncCommitteeRoot,
          receiptRootIndex,
          receiptTrieProofNodes,
          inclusionBranch);
    }
  }
}
