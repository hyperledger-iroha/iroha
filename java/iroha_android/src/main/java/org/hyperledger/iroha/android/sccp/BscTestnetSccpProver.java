package org.hyperledger.iroha.android.sccp;

import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Objects;

/** BSC testnet SCCP Groth16 proof request helpers for local-first Android proof generation. */
public final class BscTestnetSccpProver {
  public static final int DOMAIN_SORA = EvmSccpProver.DOMAIN_SORA;
  public static final int DOMAIN_BSC = EvmSccpProver.DOMAIN_BSC;
  public static final long TESTNET_CHAIN_ID = SourceSccpProofs.BSC_TESTNET_CHAIN_ID;
  public static final String TESTNET_NETWORK_ID = SourceSccpProofs.BSC_TESTNET_NETWORK_ID;
  public static final String LOCAL_ADMISSION_ENVELOPE_ENCODING_V1 =
      "norito:sccp-local-admission:v1";
  public static final String LOCAL_ADMISSION_SUBMISSION_KIND_V1 = "local_admission";
  public static final String LOCAL_ADMISSION_ENTRYPOINT_V1 = "SubmitBridgeProof";
  public static final String STARK_FRI_PROOF_FAMILY_V1 = "stark-fri-v1";
  public static final int NATIVE_RECURSIVE_MAX_PROOF_BYTES = 2 * 1024 * 1024;

  private final EvmSccpProver.WitnessProvider witnessProvider;
  private final EvmSccpProver.ProofEngine proofEngine;

  public BscTestnetSccpProver() {
    this(null, null);
  }

  public BscTestnetSccpProver(
      final EvmSccpProver.WitnessProvider witnessProvider,
      final EvmSccpProver.ProofEngine proofEngine) {
    this.witnessProvider = witnessProvider;
    this.proofEngine = proofEngine;
  }

  public static SourceSccpProofs.EvmDestinationBinding destinationBinding(
      final String verifierAddress,
      final String bridgeAddress,
      final String verifierCodeHash,
      final String verifierKeyHash) {
    return SourceSccpProofs.bscTestnetDestinationBinding(
        verifierAddress, bridgeAddress, verifierCodeHash, verifierKeyHash);
  }

  public static SourceSccpProofs.EvmDestinationBinding destinationBinding(
      final String verifierAddress,
      final String bridgeAddress,
      final String verifierCodeHash,
      final String verifierKeyHash,
      final String networkId) {
    return SourceSccpProofs.bscTestnetDestinationBinding(
        verifierAddress, bridgeAddress, verifierCodeHash, verifierKeyHash, networkId);
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
    requireBscTestnetRequestInput(input);
    final EvmSccpProver.ProofRequest request = EvmSccpProver.buildProofRequest(input);
    requireBscTestnetProofRequest(request);
    return request;
  }

  public static EvmSccpProver.ProofResult wrapProofResult(
      final byte[] proofBytes, final EvmSccpProver.ProofRequest request) {
    requireBscTestnetProofRequest(request);
    return EvmSccpProver.wrapProofResult(proofBytes, request);
  }

  public static EvmSccpProver.Submission buildSubmission(
      final EvmSccpProver.SubmissionInput input) {
    Objects.requireNonNull(input, "input");
    if (input.publicInputs().targetDomain() != DOMAIN_BSC) {
      throw new IllegalArgumentException("BSC testnet submissions must target BSC");
    }
    final EvmSccpProver.ProofResult proofResult = input.proofResult();
    if (proofResult == null) {
      throw new IllegalArgumentException(
          "BSC testnet submissions require a wrapped proofResult with destinationBinding");
    }
    if (proofResult.publicInputs().targetDomain() != DOMAIN_BSC) {
      throw new IllegalArgumentException("BSC testnet proofResult must target BSC");
    }
    final SourceSccpProofs.EvmDestinationBinding destinationBinding =
        requireBscTestnetDestinationBinding(proofResult.destinationBinding());
    if (!destinationBinding.hash.equals(proofResult.destinationBindingHash())) {
      throw new IllegalArgumentException(
          "BSC testnet proofResult destinationBindingHash must match destinationBinding");
    }
    return EvmSccpProver.buildSubmission(input);
  }

  public static LocalAdmissionSubmission buildLocalAdmissionSubmission(
      final LocalAdmissionSubmissionInput input) {
    Objects.requireNonNull(input, "input");
    if (input.sourceDomain() != DOMAIN_BSC || input.targetDomain() != DOMAIN_SORA) {
      throw new IllegalArgumentException(
          "BSC testnet local-admission submissions must route BSC -> SORA");
    }
    if (!LOCAL_ADMISSION_ENVELOPE_ENCODING_V1.equals(input.envelopeEncoding())
        || !LOCAL_ADMISSION_SUBMISSION_KIND_V1.equals(input.submissionKind())
        || !LOCAL_ADMISSION_ENTRYPOINT_V1.equals(input.verifierEntrypoint())
        || !STARK_FRI_PROOF_FAMILY_V1.equals(input.proofFamily())
        || !EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1.equals(input.verifierBackend())) {
      throw new IllegalArgumentException(
          "BSC testnet local-admission submission metadata is not canonical");
    }
    final byte[] proofBytes = requireNativeRecursiveBytes(input.proofBytes(), "proofBytes");
    final byte[] publicInputsBytes =
        requireNativeRecursiveBytes(input.publicInputsBytes(), "publicInputsBytes");
    final byte[] bundleBytes = requireNativeRecursiveBytes(input.bundleBytes(), "bundleBytes");
    final byte[] envelopeBytes =
        requireNativeRecursiveBytes(input.envelopeBytes(), "envelopeBytes");
    final String statementHash = normalizeNonZeroHex32(input.statementHash(), "statementHash");
    final String sourceVerifierMaterialHash =
        normalizeNonZeroHex32(input.sourceVerifierMaterialHash(), "sourceVerifierMaterialHash");
    final String sourceAdapterEngineDeploymentHash =
        normalizeNonZeroHex32(
            input.sourceAdapterEngineDeploymentHash(), "sourceAdapterEngineDeploymentHash");
    final LocalAdmissionPayload localAdmission =
        new LocalAdmissionPayload(
            proofBytes,
            publicInputsBytes,
            bundleBytes,
            statementHash,
            sourceVerifierMaterialHash,
            sourceAdapterEngineDeploymentHash);
    return new LocalAdmissionSubmission(
        1,
        input.proofFamily(),
        input.verifierBackend(),
        LOCAL_ADMISSION_SUBMISSION_KIND_V1,
        LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
        LOCAL_ADMISSION_SUBMISSION_KIND_V1,
        LOCAL_ADMISSION_ENTRYPOINT_V1,
        DOMAIN_BSC,
        DOMAIN_SORA,
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

  public EvmSccpProver.ProofRequest buildRequest(final EvmSccpProver.ProofRequestInput input) {
    final EvmSccpProver.ProofRequestInput resolved =
        witnessProvider == null ? input : witnessProvider.resolveWitness(inputSnapshot(input));
    return buildProofRequest(resolved);
  }

  public EvmSccpProver.ProofResult prove(final EvmSccpProver.ProofRequestInput input) {
    final EvmSccpProver.ProofRequest request = buildRequest(input);
    if (proofEngine == null) {
      throw new IllegalStateException("BSC testnet SCCP Groth16 prover is not linked");
    }
    return wrapProofResult(
        proofEngine.prove(EvmSccpProver.callbackRequestSnapshot(request)), request);
  }

  public LocalAdmissionSubmission buildLocalAdmission(final LocalAdmissionSubmissionInput input) {
    return buildLocalAdmissionSubmission(input);
  }

  private static void requireBscTestnetRequestInput(
      final EvmSccpProver.ProofRequestInput input) {
    Objects.requireNonNull(input, "input");
    if (input.publicInputs().targetDomain() != DOMAIN_BSC) {
      throw new IllegalArgumentException("BSC testnet proof requests must target BSC");
    }
    final SourceSccpProofs.EvmDestinationBinding binding =
        requireBscTestnetDestinationBinding(input.destinationBinding());
    if (!binding.hash.equals(input.destinationBindingHash())) {
      throw new IllegalArgumentException(
          "destinationBindingHash must match BSC testnet destinationBinding");
    }
  }

  private static void requireBscTestnetProofRequest(
      final EvmSccpProver.ProofRequest request) {
    Objects.requireNonNull(request, "request");
    if (request.targetDomain() != DOMAIN_BSC
        || request.publicInputs().targetDomain() != DOMAIN_BSC) {
      throw new IllegalArgumentException("BSC testnet proof requests must target BSC");
    }
    final SourceSccpProofs.EvmDestinationBinding binding =
        requireBscTestnetDestinationBinding(request.destinationBinding());
    if (!binding.hash.equals(request.destinationBindingHash())) {
      throw new IllegalArgumentException(
          "destinationBindingHash must match BSC testnet destinationBinding");
    }
  }

  private static SourceSccpProofs.EvmDestinationBinding requireBscTestnetDestinationBinding(
      final SourceSccpProofs.EvmDestinationBinding destinationBinding) {
    final SourceSccpProofs.EvmDestinationBinding binding =
        Objects.requireNonNull(destinationBinding, "destinationBinding");
    if (binding.targetDomain != DOMAIN_BSC) {
      throw new IllegalArgumentException("BSC testnet destinationBinding must target BSC");
    }
    if (!TESTNET_NETWORK_ID.equals(binding.networkId)) {
      throw new IllegalArgumentException(
          "BSC testnet destinationBinding.networkId must be chain id 97");
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

  private static byte[] requireNativeRecursiveBytes(final byte[] bytes, final String label) {
    final byte[] copy = Arrays.copyOf(Objects.requireNonNull(bytes, label), bytes.length);
    if (copy.length == 0) {
      throw new IllegalArgumentException(label + " must not be empty");
    }
    boolean nonZero = false;
    for (final byte value : copy) {
      if (value != 0) {
        nonZero = true;
        break;
      }
    }
    if (!nonZero) {
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
      throw new IllegalArgumentException(label + " must be 32 bytes of canonical lowercase 0x hex");
    }
    boolean nonZero = false;
    for (int i = 2; i < text.length(); i++) {
      final char c = text.charAt(i);
      if (!((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f'))) {
        throw new IllegalArgumentException(label + " must be 32 bytes of canonical lowercase 0x hex");
      }
      if (c != '0') {
        nonZero = true;
      }
    }
    if (!nonZero) {
      throw new IllegalArgumentException(label + " must not be zero");
    }
    return text;
  }

  private static String hexLower(final byte[] bytes) {
    final StringBuilder out = new StringBuilder(bytes.length * 2);
    for (final byte value : bytes) {
      out.append(String.format("%02x", value & 0xff));
    }
    return out.toString();
  }

  /** Input for BSC testnet -> SORA local-admission submission packaging. */
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

  /** BSC testnet local-admission payload mirrored from the core SCCP package. */
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

  /** BSC testnet -> SORA local-admission package ready for Torii bridge-proof submission. */
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
}
