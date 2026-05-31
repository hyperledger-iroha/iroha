package org.hyperledger.iroha.android.sccp;

import java.io.ByteArrayOutputStream;
import java.util.ArrayList;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import org.hyperledger.iroha.android.crypto.Blake2b;

/** Substrate-family SCCP runtime proof request helpers for local-first Android proof generation. */
public final class SubstrateSccpProver {
  public static final int DOMAIN_SORA = SourceSccpProofs.DOMAIN_SORA;
  public static final int DOMAIN_SORA_KUSAMA = SourceSccpProofs.DOMAIN_SORA_KUSAMA;
  public static final int DOMAIN_SORA_POLKADOT = SourceSccpProofs.DOMAIN_SORA_POLKADOT;
  public static final int DOMAIN_SORA2 = SourceSccpProofs.DOMAIN_SORA2;
  public static final String RUNTIME_PROOF_BACKEND_V1 = "substrate-runtime-v1";
  public static final String RUNTIME_CALL_SCALE_V1 = "scale_call_v1";
  public static final String SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1 =
      "SccpBridge.submit_message_proof";
  public static final String STARK_FRI_PROOF_FAMILY_V1 = "stark-fri-v1";
  public static final int NATIVE_RECURSIVE_MAX_PROOF_BYTES = 2 * 1024 * 1024;

  private static final String PROOF_REQUEST_PREFIX_V1 =
      "sccp:substrate:runtime-proof-request:v1";
  private static final String PROOF_ENVELOPE_PREFIX_V1 =
      "sccp:substrate:runtime-proof-envelope:v1";
  private static final BigInteger MAX_U64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);

  private final WitnessProvider witnessProvider;
  private final ProofEngine proofEngine;

  public SubstrateSccpProver() {
    this(null, null);
  }

  public SubstrateSccpProver(final WitnessProvider witnessProvider, final ProofEngine proofEngine) {
    this.witnessProvider = witnessProvider;
    this.proofEngine = proofEngine;
  }

  public ProofRequest buildRequest(final ProofRequestInput input) {
    final ProofRequestInput resolved =
        witnessProvider == null
            ? input
            : witnessProvider.resolveWitness(witnessProviderInputSnapshot(input));
    return buildProofRequest(resolved);
  }

  public ProofResult prove(final ProofRequestInput input) {
    final ProofRequest request = buildRequest(input);
    if (proofEngine == null) {
      throw new IllegalStateException("Substrate SCCP runtime prover is not linked");
    }
    requireProductionProofRequest(request);
    return wrapProofResult(proofEngine.prove(callbackRequestSnapshot(request)), request);
  }

  private static ProofRequestInput witnessProviderInputSnapshot(final ProofRequestInput input) {
    final byte[] bundleBytes = Objects.requireNonNull(input.bundleBytes(), "bundleBytes");
    final byte[] sourceProofBytes =
        Objects.requireNonNull(input.sourceProofBytes(), "sourceProofBytes");
    return new ProofRequestInput(
        input.publicInputs(),
        Arrays.copyOf(bundleBytes, bundleBytes.length),
        Arrays.copyOf(sourceProofBytes, sourceProofBytes.length),
        input.statementHash(),
        input.destinationBindingHash(),
        input.backend(),
        input.sourceDomain());
  }

  static ProofRequest callbackRequestSnapshot(final ProofRequest request) {
    return new ProofRequest(
        request.version(),
        request.backend(),
        request.sourceDomain(),
        request.targetDomain(),
        request.publicInputs(),
        request.publicInputsBytes(),
        request.bundleBytes(),
        request.sourceProofBytes(),
        request.proofContext(),
        request.statementHash(),
        request.destinationBindingHash(),
        request.requestHash());
  }

  public static byte[] canonicalPublicInputsBytes(final PublicInputsInput input) {
    Objects.requireNonNull(input, "input");
    if (input.version() != 1) {
      throw new IllegalArgumentException("publicInputs.version must be 1");
    }
    if (!targetDomainIsSupported(input.targetDomain())) {
      throw new IllegalArgumentException(
          "publicInputs.targetDomain must be a Substrate-family SCCP domain");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(input.version());
    write(out, nonZeroHex32Bytes(input.messageId(), "messageId"));
    write(out, nonZeroHex32Bytes(input.payloadHash(), "payloadHash"));
    writeU32Le(out, input.targetDomain());
    write(out, nonZeroHex32Bytes(input.commitmentRoot(), "commitmentRoot"));
    writeU64Le(out, normalizeU64(input.finalityHeight(), "finalityHeight"));
    write(out, nonZeroHex32Bytes(input.finalityBlockHash(), "finalityBlockHash"));
    return out.toByteArray();
  }

  public static ProofRequest buildProofRequest(final ProofRequestInput input) {
    Objects.requireNonNull(input, "input");
    if (!RUNTIME_PROOF_BACKEND_V1.equals(input.backend())) {
      throw new IllegalArgumentException("backend must be substrate-runtime-v1");
    }
    final byte[] bundleBytes = Arrays.copyOf(input.bundleBytes(), input.bundleBytes().length);
    final byte[] sourceProofBytes =
        Arrays.copyOf(input.sourceProofBytes(), input.sourceProofBytes().length);
    if (bundleBytes.length == 0) {
      throw new IllegalArgumentException("bundleBytes must not be empty");
    }
    requireOptionalNonZeroBytes(sourceProofBytes, "sourceProofBytes");
    if (input.sourceDomain() != DOMAIN_SORA) {
      throw new IllegalArgumentException("sourceDomain must be SORA");
    }
    if (input.sourceDomain() == input.publicInputs().targetDomain()) {
      throw new IllegalArgumentException("sourceDomain and publicInputs.targetDomain must differ");
    }
    final byte[] publicInputsBytes = canonicalPublicInputsBytes(input.publicInputs());
    final ProofContext proofContext =
        normalizeProofContext(input.statementHash(), input.destinationBindingHash());
    final ByteArrayOutputStream preimage = new ByteArrayOutputStream();
    writeU32Le(preimage, input.sourceDomain());
    write(preimage, publicInputsBytes);
    writeU32Le(preimage, bundleBytes.length);
    write(preimage, bundleBytes);
    writeU32Le(preimage, sourceProofBytes.length);
    write(preimage, sourceProofBytes);
    write(preimage, hex32Bytes(proofContext.statementHash(), "statementHash"));
    write(preimage, hex32Bytes(proofContext.destinationBindingHash(), "destinationBindingHash"));
    return new ProofRequest(
        1,
        input.backend(),
        input.sourceDomain(),
        input.publicInputs().targetDomain(),
        input.publicInputs(),
        publicInputsBytes,
        bundleBytes,
        sourceProofBytes,
        proofContext,
        proofContext.statementHash(),
        proofContext.destinationBindingHash(),
        hashHex(PROOF_REQUEST_PREFIX_V1, preimage.toByteArray()));
  }

  public static ProofResult wrapProofResult(final byte[] proofBytes, final ProofRequest request) {
    Objects.requireNonNull(request, "request");
    if (!RUNTIME_PROOF_BACKEND_V1.equals(request.backend())) {
      throw new IllegalArgumentException("backend must be substrate-runtime-v1");
    }
    requireNonZeroProofBytes(proofBytes);
    requireProductionProofRequest(request);
    final ByteArrayOutputStream envelopePayload = new ByteArrayOutputStream();
    write(envelopePayload, hex32Bytes(request.requestHash(), "requestHash"));
    write(envelopePayload, proofBytes);
    return new ProofResult(
        1,
        request.backend(),
        Arrays.copyOf(proofBytes, proofBytes.length),
        Base64.getEncoder().encodeToString(proofBytes),
        request.publicInputs(),
        request.bundleBytes(),
        request.sourceProofBytes(),
        request.proofContext(),
        request.statementHash(),
        request.destinationBindingHash(),
        request.requestHash(),
        hashHex(PROOF_ENVELOPE_PREFIX_V1, envelopePayload.toByteArray()));
  }

  public static Submission buildSubmission(final SubmissionInput input) {
    Objects.requireNonNull(input, "input");
    if (input.sourceDomain() != DOMAIN_SORA) {
      throw new IllegalArgumentException("sourceDomain must be SORA");
    }
    final byte[] proofBytes = input.proofBytes();
    requireNonZeroProofBytes(proofBytes);
    final byte[] bundleBytes = input.bundleBytes();
    final byte[] sourceProofBytes = input.sourceProofBytes();
    final ProofRequest request =
        buildProofRequest(
            new ProofRequestInput(
                input.publicInputs(),
                bundleBytes,
                sourceProofBytes,
                input.statementHash(),
                input.destinationBindingHash(),
                RUNTIME_PROOF_BACKEND_V1,
                input.sourceDomain()));
    if (input.proofResult() != null) {
      final ProofResult proofResult = input.proofResult();
      if (!RUNTIME_PROOF_BACKEND_V1.equals(proofResult.backend())) {
        throw new IllegalArgumentException("proofResult.backend must be substrate-runtime-v1");
      }
      if (!Objects.equals(proofResult.publicInputs(), input.publicInputs())) {
        throw new IllegalArgumentException("proofResult.publicInputs must match publicInputs");
      }
      if (!Arrays.equals(proofResult.bundleBytes(), bundleBytes)) {
        throw new IllegalArgumentException("bundleBytes must match proofResult.bundleBytes");
      }
      if (!Arrays.equals(proofResult.sourceProofBytes(), sourceProofBytes)) {
        throw new IllegalArgumentException(
            "sourceProofBytes must match proofResult.sourceProofBytes");
      }
      final ProofResult expectedResult = wrapProofResult(proofResult.proofBytes(), request);
      if (expectedResult.version() != proofResult.version()
          || !Objects.equals(expectedResult.backend(), proofResult.backend())
          || !Arrays.equals(expectedResult.proofBytes(), proofResult.proofBytes())
          || !Objects.equals(expectedResult.proofBase64(), proofResult.proofBase64())
          || !Objects.equals(expectedResult.publicInputs(), proofResult.publicInputs())
          || !Arrays.equals(expectedResult.bundleBytes(), proofResult.bundleBytes())
          || !Arrays.equals(expectedResult.sourceProofBytes(), proofResult.sourceProofBytes())
          || !Objects.equals(expectedResult.proofContext(), proofResult.proofContext())
          || !Objects.equals(expectedResult.statementHash(), proofResult.statementHash())
          || !Objects.equals(
              expectedResult.destinationBindingHash(), proofResult.destinationBindingHash())
          || !Objects.equals(expectedResult.requestHash(), proofResult.requestHash())
          || !Objects.equals(expectedResult.envelopeHash(), proofResult.envelopeHash())) {
        throw new IllegalArgumentException("proofResult must match request");
      }
      if (!Arrays.equals(proofResult.proofBytes(), proofBytes)) {
        throw new IllegalArgumentException("proofBytes must match proofResult.proofBytes");
      }
    }
    final List<SubmissionArgument> arguments = new ArrayList<>();
    arguments.add(
        new SubmissionArgument("proof_bytes", "raw_bytes", "0x" + hexLower(proofBytes)));
    arguments.add(
        new SubmissionArgument(
            "public_inputs", "raw_bytes", "0x" + hexLower(request.publicInputsBytes())));
    arguments.add(
        new SubmissionArgument("bundle_bytes", "raw_bytes", "0x" + hexLower(bundleBytes)));
    final byte[] runtimeCall =
        encodeSubstrateRuntimeCall(
            Arrays.asList(proofBytes, request.publicInputsBytes(), bundleBytes));
    return new Submission(
        1,
        STARK_FRI_PROOF_FAMILY_V1,
        RUNTIME_PROOF_BACKEND_V1,
        "substrate_runtime_call",
        RUNTIME_CALL_SCALE_V1,
        "runtime_call",
        SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1,
        input.sourceDomain(),
        input.publicInputs().targetDomain(),
        input.publicInputs(),
        request.proofContext(),
        request.statementHash(),
        request.destinationBindingHash(),
        request.requestHash(),
        proofBytes,
        request.publicInputsBytes(),
        bundleBytes,
        arguments,
        runtimeCall,
        "0x" + hexLower(runtimeCall),
        runtimeCall,
        "0x" + hexLower(runtimeCall));
  }

  private static void requireNonZeroProofBytes(final byte[] proofBytes) {
    if (proofBytes == null || proofBytes.length == 0) {
      throw new IllegalArgumentException("proofBytes must not be empty");
    }
    if (proofBytes.length > NATIVE_RECURSIVE_MAX_PROOF_BYTES) {
      throw new IllegalArgumentException(
          "proofBytes must be at most " + NATIVE_RECURSIVE_MAX_PROOF_BYTES + " bytes");
    }
    for (final byte value : proofBytes) {
      if (value != 0) {
        return;
      }
    }
    throw new IllegalArgumentException("proofBytes must not be all zero");
  }

  private static void requireOptionalNonZeroBytes(final byte[] bytes, final String label) {
    if (bytes.length == 0) {
      return;
    }
    for (final byte value : bytes) {
      if (value != 0) {
        return;
      }
    }
    throw new IllegalArgumentException(label + " must not be all zero");
  }

  private static void requireNonEmptyNonZeroBytes(final byte[] bytes, final String label) {
    if (bytes.length == 0) {
      throw new IllegalArgumentException(label + " must not be empty");
    }
    requireOptionalNonZeroBytes(bytes, label);
  }

  private static void requireCanonicalProofRequest(final ProofRequest request) {
    final ProofRequest expected =
        buildProofRequest(
            new ProofRequestInput(
                request.publicInputs(),
                request.bundleBytes(),
                request.sourceProofBytes(),
                request.statementHash(),
                request.destinationBindingHash(),
                request.backend(),
                request.sourceDomain()));
    if (request.version() != expected.version()
        || !Objects.equals(request.backend(), expected.backend())
        || request.sourceDomain() != expected.sourceDomain()
        || request.targetDomain() != expected.targetDomain()
        || !Objects.equals(request.publicInputs(), expected.publicInputs())
        || !Arrays.equals(request.publicInputsBytes(), expected.publicInputsBytes())
        || !Arrays.equals(request.bundleBytes(), expected.bundleBytes())
        || !Arrays.equals(request.sourceProofBytes(), expected.sourceProofBytes())
        || !Objects.equals(request.proofContext(), expected.proofContext())
        || !Objects.equals(request.statementHash(), expected.statementHash())
        || !Objects.equals(request.destinationBindingHash(), expected.destinationBindingHash())
        || !Objects.equals(request.requestHash(), expected.requestHash())) {
      throw new IllegalArgumentException("proof request must be canonical");
    }
  }

  private static void requireProductionProofRequest(final ProofRequest request) {
    requireCanonicalProofRequest(request);
    if (request.version() != 1) {
      throw new IllegalArgumentException("proof request version must be 1");
    }
    if (!RUNTIME_PROOF_BACKEND_V1.equals(request.backend())) {
      throw new IllegalArgumentException(
          "Substrate SCCP proof request backend must be substrate-runtime-v1");
    }
    if (request.sourceDomain() != DOMAIN_SORA) {
      throw new IllegalArgumentException("Substrate SCCP production proofs must start from SORA");
    }
    if (request.targetDomain() != request.publicInputs().targetDomain()
        || !targetDomainIsSupported(request.targetDomain())) {
      throw new IllegalArgumentException(
          "Substrate SCCP production proofs must target a Substrate-family domain");
    }
    if (request.bundleBytes().length == 0) {
      throw new IllegalArgumentException(
          "Substrate SCCP proof request bundleBytes must not be empty");
    }
    requireOptionalNonZeroBytes(
        request.sourceProofBytes(), "Substrate SCCP proof request sourceProofBytes");
  }

  private static boolean targetDomainIsSupported(final int value) {
    return value == DOMAIN_SORA_KUSAMA || value == DOMAIN_SORA_POLKADOT || value == DOMAIN_SORA2;
  }

  private static String hashHex(final String prefix, final byte[] payload) {
    final byte[] prefixBytes = prefix.getBytes(StandardCharsets.UTF_8);
    final byte[] preimage = new byte[prefixBytes.length + payload.length];
    System.arraycopy(prefixBytes, 0, preimage, 0, prefixBytes.length);
    System.arraycopy(payload, 0, preimage, prefixBytes.length, payload.length);
    return "0x" + hexLower(Blake2b.digest256(preimage));
  }

  private static byte[] hex32Bytes(final String value, final String field) {
    String body = Objects.requireNonNull(value, field);
    if (!body.trim().equals(body)) {
      throw new IllegalArgumentException(field + " must be canonical hex");
    }
    if (body.regionMatches(true, 0, "0x", 0, 2)) {
      body = body.substring(2);
    }
    if (body.length() != 64) {
      throw new IllegalArgumentException(field + " must be 32 bytes");
    }
    final byte[] out = new byte[32];
    for (int i = 0; i < out.length; i++) {
      final int hi = Character.digit(body.charAt(i * 2), 16);
      final int lo = Character.digit(body.charAt(i * 2 + 1), 16);
      if (hi < 0 || lo < 0) {
        throw new IllegalArgumentException(field + " must be canonical hex");
      }
      out[i] = (byte) ((hi << 4) | lo);
    }
    return out;
  }

  private static byte[] nonZeroHex32Bytes(final String value, final String field) {
    final byte[] out = hex32Bytes(value, field);
    for (final byte b : out) {
      if (b != 0) {
        return out;
      }
    }
    throw new IllegalArgumentException(field + " must not be zero");
  }

  private static String normalizeHex32(final String value, final String field) {
    return "0x" + hexLower(nonZeroHex32Bytes(value, field));
  }

  private static ProofContext normalizeProofContext(
      final String statementHash, final String destinationBindingHash) {
    return new ProofContext(
        1,
        normalizeHex32(statementHash, "statementHash"),
        normalizeHex32(destinationBindingHash, "destinationBindingHash"));
  }

  private static BigInteger normalizeU64(final String value, final String field) {
    final String text = Objects.requireNonNull(value, field);
    if (!isCanonicalDecimalText(text)) {
      throw new IllegalArgumentException(field + " must be an unsigned integer");
    }
    final BigInteger numeric = new BigInteger(text);
    if (numeric.compareTo(MAX_U64) > 0) {
      throw new IllegalArgumentException(field + " must fit u64");
    }
    if (numeric.signum() == 0) {
      throw new IllegalArgumentException(field + " must not be zero");
    }
    return numeric;
  }

  private static boolean isCanonicalDecimalText(final String value) {
    if ("0".equals(value)) {
      return true;
    }
    if (value.isEmpty() || value.charAt(0) < '1' || value.charAt(0) > '9') {
      return false;
    }
    for (int i = 0; i < value.length(); i++) {
      final char ch = value.charAt(i);
      if (ch < '0' || ch > '9') {
        return false;
      }
    }
    return true;
  }

  private static void writeU32Le(final ByteArrayOutputStream out, final int value) {
    if (value < 0) {
      throw new IllegalArgumentException("u32 value must not be negative");
    }
    out.write(value & 0xff);
    out.write((value >>> 8) & 0xff);
    out.write((value >>> 16) & 0xff);
    out.write((value >>> 24) & 0xff);
  }

  private static void writeU64Le(final ByteArrayOutputStream out, final BigInteger value) {
    BigInteger working = value;
    for (int i = 0; i < 8; i++) {
      out.write(working.and(BigInteger.valueOf(0xffL)).intValue());
      working = working.shiftRight(8);
    }
  }

  private static byte[] scaleCompactU32(final int value, final String label) {
    if (value < 0) {
      throw new IllegalArgumentException(label + " length must fit u32");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    if (value < (1 << 6)) {
      out.write(value << 2);
    } else if (value < (1 << 14)) {
      final int encoded = (value << 2) | 0b01;
      out.write(encoded & 0xff);
      out.write((encoded >>> 8) & 0xff);
    } else if (value < (1 << 30)) {
      final int encoded = (value << 2) | 0b10;
      out.write(encoded & 0xff);
      out.write((encoded >>> 8) & 0xff);
      out.write((encoded >>> 16) & 0xff);
      out.write((encoded >>> 24) & 0xff);
    } else {
      out.write(0b11);
      writeU32Le(out, value);
    }
    return out.toByteArray();
  }

  private static byte[] scaleVec(final byte[] bytes, final String label) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    write(out, scaleCompactU32(bytes.length, label));
    write(out, bytes);
    return out.toByteArray();
  }

  private static byte[] encodeSubstrateRuntimeCall(final List<byte[]> argumentBytes) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    write(
        out,
        scaleVec(
            SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1.getBytes(StandardCharsets.UTF_8),
            "verifierEntrypoint"));
    for (int i = 0; i < argumentBytes.size(); i++) {
      write(out, scaleVec(argumentBytes.get(i), "arguments[" + i + "]"));
    }
    return out.toByteArray();
  }

  private static void write(final ByteArrayOutputStream out, final byte[] bytes) {
    out.write(bytes, 0, bytes.length);
  }

  private static String hexLower(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      builder.append(String.format("%02x", b & 0xff));
    }
    return builder.toString();
  }

  public interface WitnessProvider {
    ProofRequestInput resolveWitness(ProofRequestInput input);
  }

  public interface ProofEngine {
    byte[] prove(ProofRequest request);
  }

  public record PublicInputsInput(
      int version,
      String messageId,
      String payloadHash,
      int targetDomain,
      String commitmentRoot,
      String finalityHeight,
      String finalityBlockHash) {
    public PublicInputsInput(
        final String messageId,
        final String payloadHash,
        final String commitmentRoot,
        final String finalityHeight,
        final String finalityBlockHash) {
      this(
          1,
          messageId,
          payloadHash,
          DOMAIN_SORA2,
          commitmentRoot,
          finalityHeight,
          finalityBlockHash);
    }
  }

  public record ProofRequestInput(
      PublicInputsInput publicInputs,
      byte[] bundleBytes,
      byte[] sourceProofBytes,
      String statementHash,
      String destinationBindingHash,
      String backend,
      int sourceDomain) {
    public ProofRequestInput(
        final PublicInputsInput publicInputs,
        final byte[] bundleBytes,
        final String statementHash,
        final String destinationBindingHash) {
      this(
          publicInputs,
          bundleBytes,
          new byte[0],
          statementHash,
          destinationBindingHash,
          RUNTIME_PROOF_BACKEND_V1,
          DOMAIN_SORA);
    }
  }

  public record ProofContext(int version, String statementHash, String destinationBindingHash) {}

  public record ProofRequest(
      int version,
      String backend,
      int sourceDomain,
      int targetDomain,
      PublicInputsInput publicInputs,
      byte[] publicInputsBytes,
      byte[] bundleBytes,
      byte[] sourceProofBytes,
      ProofContext proofContext,
      String statementHash,
      String destinationBindingHash,
      String requestHash) {
    public ProofRequest {
      publicInputsBytes =
          Arrays.copyOf(
              Objects.requireNonNull(publicInputsBytes, "publicInputsBytes"),
              publicInputsBytes.length);
      bundleBytes =
          Arrays.copyOf(Objects.requireNonNull(bundleBytes, "bundleBytes"), bundleBytes.length);
      sourceProofBytes =
          Arrays.copyOf(
              Objects.requireNonNull(sourceProofBytes, "sourceProofBytes"),
              sourceProofBytes.length);
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
    public byte[] sourceProofBytes() {
      return Arrays.copyOf(sourceProofBytes, sourceProofBytes.length);
    }
  }

  public record ProofResult(
      int version,
      String backend,
      byte[] proofBytes,
      String proofBase64,
      PublicInputsInput publicInputs,
      byte[] bundleBytes,
      byte[] sourceProofBytes,
      ProofContext proofContext,
      String statementHash,
      String destinationBindingHash,
      String requestHash,
      String envelopeHash) {
    public ProofResult {
      proofBytes = Arrays.copyOf(Objects.requireNonNull(proofBytes, "proofBytes"), proofBytes.length);
      bundleBytes =
          Arrays.copyOf(Objects.requireNonNull(bundleBytes, "bundleBytes"), bundleBytes.length);
      sourceProofBytes =
          Arrays.copyOf(
              Objects.requireNonNull(sourceProofBytes, "sourceProofBytes"),
              sourceProofBytes.length);
    }

    @Override
    public byte[] proofBytes() {
      return Arrays.copyOf(proofBytes, proofBytes.length);
    }

    @Override
    public byte[] bundleBytes() {
      return Arrays.copyOf(bundleBytes, bundleBytes.length);
    }

    @Override
    public byte[] sourceProofBytes() {
      return Arrays.copyOf(sourceProofBytes, sourceProofBytes.length);
    }
  }

  public record SubmissionInput(
      PublicInputsInput publicInputs,
      byte[] proofBytes,
      byte[] bundleBytes,
      byte[] sourceProofBytes,
      String statementHash,
      String destinationBindingHash,
      int sourceDomain,
      ProofResult proofResult) {
    public SubmissionInput {
      proofBytes = Arrays.copyOf(Objects.requireNonNull(proofBytes, "proofBytes"), proofBytes.length);
      bundleBytes =
          Arrays.copyOf(Objects.requireNonNull(bundleBytes, "bundleBytes"), bundleBytes.length);
      sourceProofBytes =
          Arrays.copyOf(
              Objects.requireNonNull(sourceProofBytes, "sourceProofBytes"),
              sourceProofBytes.length);
    }

    public SubmissionInput(final ProofResult proofResult) {
      this(proofResult, DOMAIN_SORA);
    }

    public SubmissionInput(final ProofResult proofResult, final int sourceDomain) {
      this(
          Objects.requireNonNull(proofResult, "proofResult").publicInputs(),
          proofResult.proofBytes(),
          proofResult.bundleBytes(),
          proofResult.sourceProofBytes(),
          proofResult.statementHash(),
          proofResult.destinationBindingHash(),
          sourceDomain,
          proofResult);
    }

    @Override
    public byte[] proofBytes() {
      return Arrays.copyOf(proofBytes, proofBytes.length);
    }

    @Override
    public byte[] bundleBytes() {
      return Arrays.copyOf(bundleBytes, bundleBytes.length);
    }

    @Override
    public byte[] sourceProofBytes() {
      return Arrays.copyOf(sourceProofBytes, sourceProofBytes.length);
    }
  }

  public record SubmissionArgument(String key, String encoding, String bytesHex) {}

  public record Submission(
      int version,
      String proofFamily,
      String verifierBackend,
      String platformPayload,
      String envelopeEncoding,
      String submissionKind,
      String verifierEntrypoint,
      int sourceDomain,
      int targetDomain,
      PublicInputsInput publicInputs,
      ProofContext proofContext,
      String statementHash,
      String destinationBindingHash,
      String requestHash,
      byte[] proofBytes,
      byte[] publicInputsBytes,
      byte[] bundleBytes,
      List<SubmissionArgument> arguments,
      byte[] runtimeCall,
      String runtimeCallHex,
      byte[] envelopeBytes,
      String envelopeHex) {
    public Submission {
      proofBytes = Arrays.copyOf(Objects.requireNonNull(proofBytes, "proofBytes"), proofBytes.length);
      publicInputsBytes =
          Arrays.copyOf(
              Objects.requireNonNull(publicInputsBytes, "publicInputsBytes"),
              publicInputsBytes.length);
      bundleBytes =
          Arrays.copyOf(Objects.requireNonNull(bundleBytes, "bundleBytes"), bundleBytes.length);
      arguments =
          Collections.unmodifiableList(
              new ArrayList<>(Objects.requireNonNull(arguments, "arguments")));
      runtimeCall =
          Arrays.copyOf(Objects.requireNonNull(runtimeCall, "runtimeCall"), runtimeCall.length);
      envelopeBytes =
          Arrays.copyOf(Objects.requireNonNull(envelopeBytes, "envelopeBytes"), envelopeBytes.length);
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
    public byte[] runtimeCall() {
      return Arrays.copyOf(runtimeCall, runtimeCall.length);
    }

    @Override
    public byte[] envelopeBytes() {
      return Arrays.copyOf(envelopeBytes, envelopeBytes.length);
    }
  }
}
