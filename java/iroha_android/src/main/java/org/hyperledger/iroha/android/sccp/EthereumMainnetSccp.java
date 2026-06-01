package org.hyperledger.iroha.android.sccp;

import java.math.BigInteger;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/** Ethereum mainnet SCCP Groth16 proof request helpers for local-first Android proof generation. */
public final class EthereumMainnetSccp {
  public static final int DOMAIN_SORA = EvmSccpProver.DOMAIN_SORA;
  public static final int DOMAIN_ETH = EvmSccpProver.DOMAIN_ETH;
  public static final long MAINNET_CHAIN_ID = SourceSccpProofs.ETH_MAINNET_CHAIN_ID;
  public static final String MAINNET_NETWORK_ID = SourceSccpProofs.ETH_MAINNET_NETWORK_ID;

  private final EvmSccpProver.WitnessProvider witnessProvider;
  private final EvmSccpProver.ProofEngine proofEngine;
  private final ExecutionProvider executionProvider;
  private final ConsensusProvider consensusProvider;
  private final InboundProver inboundProver;
  private final InboundSubmitter inboundSubmitter;
  private final OutboundSubmitter outboundSubmitter;

  public EthereumMainnetSccp() {
    this(null, null, null, null, null);
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
    this.witnessProvider = witnessProvider;
    this.proofEngine = proofEngine;
    this.executionProvider = executionProvider;
    this.consensusProvider = consensusProvider;
    this.inboundProver = inboundProver;
    this.inboundSubmitter = inboundSubmitter;
    this.outboundSubmitter = outboundSubmitter;
  }

  public static void requireMainnetChainId(final long chainId) {
    if (chainId != MAINNET_CHAIN_ID) {
      throw new IllegalArgumentException("Ethereum mainnet SCCP requires eth_chainId == 1");
    }
  }

  public Object validateExecutionProviderMainnet() {
    return validateExecutionProviderMainnet(executionProvider);
  }

  public Object validateExecutionProviderMainnet(final ExecutionProvider provider) {
    final ExecutionProvider selectedProvider =
        Objects.requireNonNull(provider, "executionProvider");
    final Object chainId = selectedProvider.request("eth_chainId", Collections.emptyList());
    requireMainnetChainId(normalizeMainnetChainId(chainId));
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
    if (receipt == null && transactionHash != null && provider != null) {
      receipt =
          requireMap(
              provider.request(
                  "eth_getTransactionReceipt", Collections.<Object>singletonList(transactionHash)),
              "eth_getTransactionReceipt");
    }
    if (receipt == null && input.receiptProofHash() == null) {
      throw new IllegalArgumentException(
          "Ethereum mainnet inbound evidence requires receipt, receiptProofHash, or transactionHash");
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
              firstPresent(receipt, "transactionHash", "transaction_hash"),
              "receipt.transactionHash",
              32);
      if (transactionHash != null && !transactionHash.equals(receiptTransactionHash)) {
        throw new IllegalArgumentException("receipt.transactionHash must match transactionHash");
      }
      transactionHash = receiptTransactionHash;
      blockHash =
          normalizeRpcHex(firstPresent(receipt, "blockHash", "block_hash"), "receipt.blockHash", 32);
      final Object receiptBlockNumberInput = firstPresent(receipt, "blockNumber", "block_number");
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
          firstPresent(block, "number", "blockNumber", "block_number");
      final String blockNumber = normalizePositiveRpcQuantity(blockNumberInput, "block.number");
      if (receiptBlockNumber != null && !receiptBlockNumber.equals(blockNumber)) {
        throw new IllegalArgumentException("block.number must match receipt.blockNumber");
      }
      receiptBlockNumber = blockNumber;
      blockReceiptsRoot =
          normalizeRpcHex(
              firstPresent(block, "receiptsRoot", "receipts_root"), "block.receiptsRoot", 32);
    }

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
    final String receiptProofHash =
        input.receiptProofHash() == null
            ? null
            : normalizeRpcHex(input.receiptProofHash(), "receiptProofHash", 32);
    return new InboundEvidence(
        DOMAIN_ETH,
        DOMAIN_SORA,
            transactionHash,
            receipt,
            block,
            beaconFinality,
            receiptProofHash);
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
    return inboundProver.prove(evidence);
  }

  public Object submitInboundToIroha(final byte[] proofBytes) {
    final byte[] proof = Objects.requireNonNull(proofBytes, "proofBytes");
    if (proof.length == 0) {
      throw new IllegalArgumentException("proofBytes must not be empty");
    }
    boolean nonzero = false;
    for (final byte value : proof) {
      nonzero |= value != 0;
    }
    if (!nonzero) {
      throw new IllegalArgumentException("proofBytes must not be all zero");
    }
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

  public EvmSccpProver.ProofRequest buildOutboundProofRequest(
      final EvmSccpProver.ProofRequestInput input) {
    final EvmSccpProver.ProofRequestInput resolved =
        witnessProvider == null ? input : witnessProvider.resolveWitness(inputSnapshot(input));
    return buildProofRequest(resolved);
  }

  public EvmSccpProver.ProofResult proveOutboundToEthereum(
      final EvmSccpProver.ProofRequestInput input) {
    final EvmSccpProver.ProofRequest request = buildOutboundProofRequest(input);
    if (proofEngine == null) {
      throw new IllegalStateException("Ethereum mainnet SCCP Groth16 prover is not linked");
    }
    return wrapProofResult(proofEngine.prove(request), request);
  }

  public EvmSccpProver.Submission buildEthereumCalldata(
      final EvmSccpProver.SubmissionInput input) {
    return buildSubmission(input);
  }

  public Object submitOutboundToEthereum(final EvmSccpProver.SubmissionInput input) {
    if (outboundSubmitter == null) {
      throw new IllegalStateException("Ethereum mainnet SCCP outbound submitter is not linked");
    }
    return outboundSubmitter.submit(buildEthereumCalldata(input));
  }

  private static void requireEthereumRequestInput(final EvmSccpProver.ProofRequestInput input) {
    Objects.requireNonNull(input, "input");
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
    if (request.targetDomain() != DOMAIN_ETH
        || request.publicInputs().targetDomain() != DOMAIN_ETH) {
      throw new IllegalArgumentException("Ethereum mainnet proof requests must target ETH");
    }
    requireEthereumDestinationBinding(request.destinationBinding());
  }

  private static SourceSccpProofs.EvmDestinationBinding requireEthereumDestinationBinding(
      final SourceSccpProofs.EvmDestinationBinding destinationBinding) {
    final SourceSccpProofs.EvmDestinationBinding binding =
        Objects.requireNonNull(destinationBinding, "destinationBinding");
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
        input.destinationBinding());
  }

  private static long normalizeMainnetChainId(final Object value) {
    return normalizeUnsignedInteger(value, "eth_chainId");
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

  @SuppressWarnings("unchecked")
  private static Map<String, Object> requireMap(final Object value, final String label) {
    if (!(value instanceof Map)) {
      throw new IllegalArgumentException(label + " must return an object");
    }
    return (Map<String, Object>) value;
  }

  private static Object firstPresent(final Map<String, Object> input, final String... keys) {
    for (final String key : keys) {
      if (input.containsKey(key)) {
        return input.get(key);
      }
    }
    return null;
  }

  private static String normalizeRpcHex(
      final Object value, final String label, final int byteLength) {
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
    if (!nonzero) {
      throw new IllegalArgumentException(label + " must not be zero");
    }
    return text;
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
            firstPresent(
                finality,
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
            firstPresent(
                finality,
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
            firstPresent(
                finality,
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
    normalized.put("executionBlockNumber", Long.toString(executionBlockNumber));
    normalized.put("executionBlockHash", executionBlockHash);
    normalized.put("executionReceiptsRoot", executionReceiptsRoot);
    return Collections.unmodifiableMap(normalized);
  }

  /** App-supplied Ethereum JSON-RPC execution provider for native SCCP evidence collection. */
  public interface ExecutionProvider {
    Object request(String method, List<Object> params);
  }

  /** App-supplied Ethereum Beacon REST finality collector for native SCCP evidence collection. */
  public interface ConsensusProvider {
    Map<String, Object> collectFinalityEvidence(
        Map<String, Object> receipt, Map<String, Object> block, String transactionHash);
  }

  /** Typed Ethereum beacon finality evidence required before inbound source proving. */
  public record BeaconFinalityEvidence(
      String executionBlockNumber,
      String executionBlockHash,
      String executionReceiptsRoot,
      Map<String, Object> additionalFields) {
    public BeaconFinalityEvidence(
        final String executionBlockNumber,
        final String executionBlockHash,
        final String executionReceiptsRoot) {
      this(executionBlockNumber, executionBlockHash, executionReceiptsRoot, Collections.emptyMap());
    }

    public Map<String, Object> toMap() {
      final java.util.LinkedHashMap<String, Object> value =
          new java.util.LinkedHashMap<>(
              additionalFields == null ? Collections.emptyMap() : additionalFields);
      value.put("executionBlockNumber", executionBlockNumber);
      value.put("executionBlockHash", executionBlockHash);
      value.put("executionReceiptsRoot", executionReceiptsRoot);
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

  /** Locally collected Ethereum mainnet inbound evidence before source-proof generation. */
  public record InboundEvidence(
      int sourceDomain,
      int targetDomain,
      String transactionHash,
      Map<String, Object> receipt,
      Map<String, Object> block,
      Map<String, Object> beaconFinality,
      String receiptProofHash) {
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
  }
}
