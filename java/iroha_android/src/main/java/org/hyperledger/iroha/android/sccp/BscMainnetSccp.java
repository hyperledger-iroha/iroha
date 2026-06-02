package org.hyperledger.iroha.android.sccp;

import java.math.BigInteger;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/** BSC mainnet SCCP helpers for local-first Android proof generation. */
public final class BscMainnetSccp {
  public static final int DOMAIN_SORA = EvmSccpProver.DOMAIN_SORA;
  public static final int DOMAIN_BSC = EvmSccpProver.DOMAIN_BSC;
  public static final long MAINNET_CHAIN_ID = SourceSccpProofs.BSC_MAINNET_CHAIN_ID;
  public static final String MAINNET_NETWORK_ID = SourceSccpProofs.BSC_MAINNET_NETWORK_ID;
  public static final String LOCAL_ADMISSION_ENVELOPE_ENCODING_V1 =
      "norito:sccp-local-admission:v1";
  public static final String LOCAL_ADMISSION_SUBMISSION_KIND_V1 = "local_admission";
  public static final String LOCAL_ADMISSION_ENTRYPOINT_V1 = "SubmitBridgeProof";
  public static final String STARK_FRI_PROOF_FAMILY_V1 = "stark-fri-v1";
  public static final int NATIVE_RECURSIVE_MAX_PROOF_BYTES = 2 * 1024 * 1024;

  private final EvmSccpProver.WitnessProvider witnessProvider;
  private final EvmSccpProver.ProofEngine proofEngine;
  private final ExecutionProvider executionProvider;
  private final ConsensusProvider consensusProvider;
  private final InboundProver inboundProver;
  private final InboundSubmitter inboundSubmitter;
  private final OutboundSubmitter outboundSubmitter;

  public BscMainnetSccp() {
    this(null, null, null, null, null);
  }

  public BscMainnetSccp(
      final EvmSccpProver.WitnessProvider witnessProvider,
      final EvmSccpProver.ProofEngine proofEngine) {
    this(witnessProvider, proofEngine, null, null, null);
  }

  public BscMainnetSccp(
      final EvmSccpProver.WitnessProvider witnessProvider,
      final EvmSccpProver.ProofEngine proofEngine,
      final ExecutionProvider executionProvider,
      final InboundProver inboundProver,
      final InboundSubmitter inboundSubmitter) {
    this(witnessProvider, proofEngine, executionProvider, null, inboundProver, inboundSubmitter);
  }

  public BscMainnetSccp(
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

  public BscMainnetSccp(
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
      throw new IllegalArgumentException("BSC mainnet SCCP requires eth_chainId == 56");
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
      final ConsensusProvider consensusProvider) {
    Objects.requireNonNull(input, "input");
    if (input.sourceDomain() != DOMAIN_BSC) {
      throw new IllegalArgumentException("BSC mainnet inbound evidence sourceDomain must be BSC");
    }
    if (input.targetDomain() != DOMAIN_SORA) {
      throw new IllegalArgumentException("BSC mainnet inbound evidence targetDomain must be SORA");
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
          "BSC mainnet inbound evidence requires receipt, receiptProofHash, or transactionHash");
    }

    String blockHash = null;
    String receiptBlockNumber = null;
    String blockReceiptsRoot = null;
    if (receipt != null) {
      if (!"0x1".equals(receipt.get("status"))) {
        throw new IllegalArgumentException("BSC mainnet inbound receipt status must be 0x1");
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
      blockHash = normalizedBlockHash;
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

    Map<String, Object> parliaFinality = input.parliaFinality();
    if (parliaFinality == null && consensusProvider != null) {
      parliaFinality = consensusProvider.collectFinalityEvidence(receipt, block, transactionHash);
    }
    final Map<String, Object> normalizedParliaFinality =
        parliaFinality == null
            ? null
            : normalizeParliaFinality(
                parliaFinality, blockHash, receiptBlockNumber, blockReceiptsRoot);
    final String receiptProofHash =
        input.receiptProofHash() == null
            ? null
            : normalizeRpcHex(input.receiptProofHash(), "receiptProofHash", 32);
    return new InboundEvidence(
        DOMAIN_BSC,
        DOMAIN_SORA,
        transactionHash,
        receipt,
        block,
        normalizedParliaFinality,
        receiptProofHash);
  }

  public byte[] proveInboundToSora(final InboundEvidence input) {
    if (inboundProver == null) {
      throw new IllegalStateException("BSC mainnet SCCP inbound prover is not linked");
    }
    final InboundEvidence evidence = collectInboundEvidenceFromReceipt(input);
    if (evidence.parliaFinality() == null) {
      throw new IllegalArgumentException("BSC mainnet SCCP inbound proof requires parliaFinality");
    }
    final byte[] proofBytes = inboundProver.prove(evidence);
    if (proofBytes == null || proofBytes.length == 0) {
      throw new IllegalArgumentException("proofBytes must not be empty");
    }
    boolean nonzero = false;
    for (final byte value : proofBytes) {
      nonzero |= value != 0;
    }
    if (!nonzero) {
      throw new IllegalArgumentException("proofBytes must not be all zero");
    }
    return Arrays.copyOf(proofBytes, proofBytes.length);
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
      throw new IllegalStateException("BSC mainnet SCCP inbound submitter is not linked");
    }
    return inboundSubmitter.submit(Arrays.copyOf(proof, proof.length));
  }

  public static SourceSccpProofs.EvmDestinationBinding destinationBinding(
      final String verifierAddress,
      final String bridgeAddress,
      final String verifierCodeHash,
      final String verifierKeyHash) {
    return BscSccpProver.destinationBinding(
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
    return BscSccpProver.buildProofRequest(input);
  }

  public static EvmSccpProver.ProofResult wrapProofResult(
      final byte[] proofBytes, final EvmSccpProver.ProofRequest request) {
    return BscSccpProver.wrapProofResult(proofBytes, request);
  }

  public static EvmSccpProver.Submission buildSubmission(
      final EvmSccpProver.SubmissionInput input) {
    return BscSccpProver.buildSubmission(input);
  }

  public static LocalAdmissionSubmission buildLocalAdmissionSubmission(
      final LocalAdmissionSubmissionInput input) {
    Objects.requireNonNull(input, "input");
    if (input.sourceDomain() != DOMAIN_BSC || input.targetDomain() != DOMAIN_SORA) {
      throw new IllegalArgumentException(
          "BSC mainnet local-admission submissions must route BSC -> SORA");
    }
    if (!LOCAL_ADMISSION_ENVELOPE_ENCODING_V1.equals(input.envelopeEncoding())) {
      throw new IllegalArgumentException(
          "BSC mainnet local-admission envelopeEncoding is not canonical");
    }
    if (!LOCAL_ADMISSION_SUBMISSION_KIND_V1.equals(input.submissionKind())) {
      throw new IllegalArgumentException(
          "BSC mainnet local-admission submissionKind is not canonical");
    }
    if (!LOCAL_ADMISSION_ENTRYPOINT_V1.equals(input.verifierEntrypoint())) {
      throw new IllegalArgumentException(
          "BSC mainnet local-admission verifierEntrypoint is not canonical");
    }
    if (!STARK_FRI_PROOF_FAMILY_V1.equals(input.proofFamily())) {
      throw new IllegalArgumentException(
          "BSC mainnet local-admission proofFamily is not canonical");
    }
    if (!EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1.equals(input.verifierBackend())) {
      throw new IllegalArgumentException(
          "BSC mainnet local-admission verifierBackend is not canonical");
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
        DOMAIN_BSC,
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
    return buildProofRequest(resolved);
  }

  public EvmSccpProver.ProofResult proveOutboundToBsc(
      final EvmSccpProver.ProofRequestInput input) {
    final EvmSccpProver.ProofRequest request = buildOutboundProofRequest(input);
    if (proofEngine == null) {
      throw new IllegalStateException("BSC mainnet SCCP Groth16 prover is not linked");
    }
    return wrapProofResult(proofEngine.prove(EvmSccpProver.callbackRequestSnapshot(request)), request);
  }

  public EvmSccpProver.Submission buildBscCalldata(
      final EvmSccpProver.SubmissionInput input) {
    return buildSubmission(input);
  }

  public LocalAdmissionSubmission buildLocalAdmission(final LocalAdmissionSubmissionInput input) {
    return buildLocalAdmissionSubmission(input);
  }

  public Object submitOutboundToBsc(final EvmSccpProver.SubmissionInput input) {
    if (outboundSubmitter == null) {
      throw new IllegalStateException("BSC mainnet SCCP outbound submitter is not linked");
    }
    return outboundSubmitter.submit(buildBscCalldata(input));
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
    if (value instanceof BigInteger) {
      final BigInteger parsed = (BigInteger) value;
      if (parsed.signum() < 0 || parsed.bitLength() > 63) {
        throw new IllegalArgumentException("eth_chainId must fit positive i64");
      }
      return parsed.longValue();
    }
    if (value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long) {
      final long parsed = ((Number) value).longValue();
      if (parsed < 0) {
        throw new IllegalArgumentException("eth_chainId must be non-negative");
      }
      return parsed;
    }
    if (value instanceof Number) {
      throw new IllegalArgumentException("eth_chainId must be an integral JSON-RPC quantity");
    }
    if (value instanceof String) {
      final String text = (String) value;
      if (!text.trim().equals(text)) {
        throw new IllegalArgumentException("eth_chainId must be canonical");
      }
      final BigInteger parsed;
      if (text.startsWith("0x")) {
        final String hex = text.substring(2);
        if (!hex.matches("0|[1-9a-f][0-9a-f]*")) {
          throw new IllegalArgumentException(
              "eth_chainId must be a canonical JSON-RPC quantity");
        }
        parsed = new BigInteger(hex, 16);
      } else {
        if (!text.matches("0|[1-9][0-9]*")) {
          throw new IllegalArgumentException("eth_chainId must be a canonical decimal integer");
        }
        parsed = new BigInteger(text, 10);
      }
      if (parsed.bitLength() > 63) {
        throw new IllegalArgumentException("eth_chainId must fit positive i64");
      }
      return parsed.longValue();
    }
    throw new IllegalArgumentException("eth_chainId must be a JSON-RPC quantity or integer");
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

  private static Map<String, Object> normalizeParliaFinality(
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
            "parliaFinality.executionBlockNumber");
    if (executionBlockNumber == 0) {
      throw new IllegalArgumentException("parliaFinality.executionBlockNumber must be positive");
    }
    if (expectedBlockNumber != null
        && executionBlockNumber != normalizeUnsignedInteger(expectedBlockNumber, "block.number")) {
      throw new IllegalArgumentException(
          "parliaFinality.executionBlockNumber must match block.number");
    }
    final String executionBlockHash =
        normalizeRpcHex(
            firstPresent(
                finality,
                "executionBlockHash",
                "execution_block_hash",
                "finalityBlockHash",
                "finality_block_hash"),
            "parliaFinality.executionBlockHash",
            32);
    if (expectedBlockHash != null && !expectedBlockHash.equals(executionBlockHash)) {
      throw new IllegalArgumentException("parliaFinality.executionBlockHash must match block.hash");
    }
    final String executionReceiptsRoot =
        normalizeRpcHex(
            firstPresent(
                finality,
                "executionReceiptsRoot",
                "execution_receipts_root",
                "receiptsRoot",
                "receipts_root"),
            "parliaFinality.executionReceiptsRoot",
            32);
    if (expectedReceiptsRoot != null && !expectedReceiptsRoot.equals(executionReceiptsRoot)) {
      throw new IllegalArgumentException(
          "parliaFinality.executionReceiptsRoot must match block.receiptsRoot");
    }
    final Map<String, Object> normalized = new LinkedHashMap<>(finality);
    normalized.put("executionBlockNumber", Long.toString(executionBlockNumber));
    normalized.put("executionBlockHash", executionBlockHash);
    normalized.put("executionReceiptsRoot", executionReceiptsRoot);
    return normalized;
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
    if (value instanceof String) {
      final String text = (String) value;
      if (!text.trim().equals(text)) {
        throw new IllegalArgumentException(label + " must be canonical");
      }
      final BigInteger parsed;
      if (text.startsWith("0x")) {
        final String hex = text.substring(2);
        if (!hex.matches("0|[1-9a-f][0-9a-f]*")) {
          throw new IllegalArgumentException(label + " must be a canonical JSON-RPC quantity");
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

  /** App-supplied BSC JSON-RPC execution provider for native SCCP evidence collection. */
  public interface ExecutionProvider {
    Object request(String method, List<Object> params);
  }

  /** App-supplied BSC Parlia finality collector for native SCCP evidence collection. */
  public interface ConsensusProvider {
    Map<String, Object> collectFinalityEvidence(
        Map<String, Object> receipt, Map<String, Object> block, String transactionHash);
  }

  /** Typed BSC Parlia finality evidence required before inbound source proving. */
  public record ParliaFinalityEvidence(
      String executionBlockNumber,
      String executionBlockHash,
      String executionReceiptsRoot,
      Map<String, Object> additionalFields) {
    public ParliaFinalityEvidence(
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

  /** Local BSC mainnet inbound source prover linked by the application bundle. */
  public interface InboundProver {
    byte[] prove(InboundEvidence evidence);
  }

  /** App-supplied Torii submitter for locally generated BSC inbound proofs. */
  public interface InboundSubmitter {
    Object submit(byte[] proofBytes);
  }

  /** App-supplied BSC transaction submitter for locally generated outbound proof calldata. */
  public interface OutboundSubmitter {
    Object submit(EvmSccpProver.Submission submission);
  }

  /** Input for BSC -> SORA local-admission submission packaging. */
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
          DOMAIN_BSC,
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

  /** BSC local-admission payload mirrored from the core SCCP package. */
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

  /** BSC -> SORA local-admission package ready for Torii bridge-proof submission. */
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

  /** Locally collected BSC mainnet inbound evidence before source-proof generation. */
  public record InboundEvidence(
      int sourceDomain,
      int targetDomain,
      String transactionHash,
      Map<String, Object> receipt,
      Map<String, Object> block,
      Map<String, Object> parliaFinality,
      String receiptProofHash) {
    public static InboundEvidence withParliaFinalityEvidence(
        final int sourceDomain,
        final int targetDomain,
        final String transactionHash,
        final Map<String, Object> receipt,
        final Map<String, Object> block,
        final ParliaFinalityEvidence parliaFinalityEvidence,
        final String receiptProofHash) {
      return new InboundEvidence(
          sourceDomain,
          targetDomain,
          transactionHash,
          receipt,
          block,
          parliaFinalityEvidence == null ? null : parliaFinalityEvidence.toMap(),
          receiptProofHash);
    }
  }
}
