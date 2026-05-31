package org.hyperledger.iroha.android.sccp;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import org.bouncycastle.crypto.digests.KeccakDigest;
import org.hyperledger.iroha.android.crypto.Blake2b;

/** TRON SCCP Groth16 proof request helpers for local-first Android proof generation. */
public final class TronSccpProver {
  public static final int DOMAIN_SORA = SourceSccpProofs.DOMAIN_SORA;
  public static final int DOMAIN_TRON = SourceSccpProofs.DOMAIN_TRON;
  public static final String GROTH16_BN254_PROOF_BACKEND_V1 = "tron-groth16-bn254-v1";
  public static final int GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1 = 384;
  public static final String CONTRACT_CALL_ABI_TUPLE_V1 = "tron_abi_tuple_v1";
  public static final String SUBMIT_MESSAGE_PROOF_ABI_V1 =
      "submitSccpMessageProof(bytes,bytes32[6],bytes32)";
  public static final String SUBMIT_MESSAGE_PROOF_SELECTOR_V1 = "0xbd57826c";

  private static final String PROOF_REQUEST_PREFIX_V1 = "sccp:tron:groth16-proof-request:v1";
  private static final String PROOF_ENVELOPE_PREFIX_V1 = "sccp:tron:groth16-proof-envelope:v1";
  private static final String STARK_FRI_PROOF_FAMILY_V1 = "stark-fri-v1";
  private static final String SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1 =
      "submitSccpMessageProof(bytes proof_bytes, bytes32[6] public_inputs, bytes32 statement_hash)";
  private static final byte[] SUBMIT_MESSAGE_PROOF_SELECTOR_BYTES_V1 =
      new byte[] {(byte) 0xbd, 0x57, (byte) 0x82, 0x6c};
  private static final BigInteger MAX_U64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);
  private static final BigInteger BN254_BASE_FIELD_MODULUS =
      new BigInteger("30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47", 16);
  private static final BigInteger BN254_SCALAR_FIELD_MODULUS =
      new BigInteger("30644e72e131a029b85045b68181585d2833e84879b9709143e1f593f0000001", 16);
  private static final BigInteger BN254_G2_B_C0 =
      new BigInteger(
          "2b149d40ceb8aaae81be18991be06ac3b5b4c5e559dbefa33267e6dc24a138e5", 16);
  private static final BigInteger BN254_G2_B_C1 =
      new BigInteger(
          "009713b03af0fed4cd2cafadeed8fdf4a74fa084e52d1852e4a2bd0685c315d2", 16);
  private static final List<String> SIGNAL_LABELS =
      Arrays.asList(
          "sccp:groth16-bn254:signal:message-id:v1",
          "sccp:groth16-bn254:signal:payload-hash:v1",
          "sccp:groth16-bn254:signal:target-domain:v1",
          "sccp:groth16-bn254:signal:commitment-root:v1",
          "sccp:groth16-bn254:signal:finality-height:v1",
          "sccp:groth16-bn254:signal:finality-block-hash:v1",
          "sccp:groth16-bn254:signal:source-domain:v1",
          "sccp:groth16-bn254:signal:statement-hash:v1",
          "sccp:groth16-bn254:signal:destination-binding-hash:v1");

  private final WitnessProvider witnessProvider;
  private final ProofEngine proofEngine;

  public TronSccpProver() {
    this(null, null);
  }

  public TronSccpProver(final WitnessProvider witnessProvider, final ProofEngine proofEngine) {
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
      throw new IllegalStateException("TRON SCCP Groth16 prover is not linked");
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
        input.sourceDomain(),
        input.destinationBinding());
  }

  static ProofRequest callbackRequestSnapshot(final ProofRequest request) {
    return new ProofRequest(
        request.version(),
        request.backend(),
        request.sourceDomain(),
        request.targetDomain(),
        request.publicInputs(),
        request.publicInputsBytes(),
        request.publicSignalWords(),
        request.bundleBytes(),
        request.sourceProofBytes(),
        request.proofContext(),
        request.statementHash(),
        request.destinationBindingHash(),
        request.requestHash(),
        request.destinationBinding());
  }

  public static byte[] canonicalPublicInputsBytes(final PublicInputsInput input) {
    Objects.requireNonNull(input, "input");
    if (input.version() != 1) {
      throw new IllegalArgumentException("publicInputs.version must be 1");
    }
    if (input.targetDomain() == 0) {
      throw new IllegalArgumentException("publicInputs.targetDomain must not be zero");
    }
    if (input.targetDomain() != DOMAIN_TRON) {
      throw new IllegalArgumentException("publicInputs.targetDomain must be TRON");
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

  public static List<String> groth16Bn254PublicSignalWords(
      final PublicInputsInput publicInputs,
      final int sourceDomain,
      final String statementHash,
      final String destinationBindingHash) {
    if (publicInputs.version() != 1) {
      throw new IllegalArgumentException("publicInputs.version must be 1");
    }
    if (publicInputs.targetDomain() == 0) {
      throw new IllegalArgumentException("publicInputs.targetDomain must not be zero");
    }
    if (publicInputs.targetDomain() != DOMAIN_TRON) {
      throw new IllegalArgumentException("publicInputs.targetDomain must be TRON");
    }
    if (sourceDomain != DOMAIN_SORA) {
      throw new IllegalArgumentException("sourceDomain must be SORA");
    }
    if (sourceDomain == publicInputs.targetDomain()) {
      throw new IllegalArgumentException("sourceDomain and publicInputs.targetDomain must differ");
    }
    final byte[][] values = {
      nonZeroHex32Bytes(publicInputs.messageId(), "messageId"),
      nonZeroHex32Bytes(publicInputs.payloadHash(), "payloadHash"),
      abiWordU32(publicInputs.targetDomain()),
      nonZeroHex32Bytes(publicInputs.commitmentRoot(), "commitmentRoot"),
      abiWordU64(normalizeU64(publicInputs.finalityHeight(), "finalityHeight")),
      nonZeroHex32Bytes(publicInputs.finalityBlockHash(), "finalityBlockHash"),
      abiWordU32(sourceDomain),
      nonZeroHex32Bytes(statementHash, "statementHash"),
      nonZeroHex32Bytes(destinationBindingHash, "destinationBindingHash")
    };
    final java.util.ArrayList<String> out = new java.util.ArrayList<>(values.length);
    for (int i = 0; i < values.length; i++) {
      out.add(groth16SignalWord(SIGNAL_LABELS.get(i), values[i]));
    }
    return out;
  }

  public static ProofRequest buildProofRequest(final ProofRequestInput input) {
    Objects.requireNonNull(input, "input");
    if (!GROTH16_BN254_PROOF_BACKEND_V1.equals(input.backend())) {
      throw new IllegalArgumentException("backend must be tron-groth16-bn254-v1");
    }
    final byte[] publicInputsBytes = canonicalPublicInputsBytes(input.publicInputs());
    final ProofContext proofContext =
        normalizeProofContext(input.statementHash(), input.destinationBindingHash());
    final List<String> publicSignalWords =
        groth16Bn254PublicSignalWords(
            input.publicInputs(),
            input.sourceDomain(),
            proofContext.statementHash(),
            proofContext.destinationBindingHash());
    final byte[] bundleBytes = Arrays.copyOf(input.bundleBytes(), input.bundleBytes().length);
    if (bundleBytes.length == 0) {
      throw new IllegalArgumentException("bundleBytes must not be empty");
    }
    final byte[] sourceProofBytes =
        Arrays.copyOf(input.sourceProofBytes(), input.sourceProofBytes().length);
    requireOptionalNonZeroBytes(sourceProofBytes, "sourceProofBytes");
    final ByteArrayOutputStream preimage = new ByteArrayOutputStream();
    write(preimage, publicInputsBytes);
    writeU32Le(preimage, bundleBytes.length);
    write(preimage, bundleBytes);
    writeU32Le(preimage, sourceProofBytes.length);
    write(preimage, sourceProofBytes);
    write(preimage, hex32Bytes(proofContext.statementHash(), "statementHash"));
    write(preimage, hex32Bytes(proofContext.destinationBindingHash(), "destinationBindingHash"));
    for (final String signal : publicSignalWords) {
      write(preimage, hex32Bytes(signal, "publicSignalWords"));
    }
    return new ProofRequest(
        1,
        input.backend(),
        input.sourceDomain(),
        input.publicInputs().targetDomain(),
        input.publicInputs(),
        publicInputsBytes,
        publicSignalWords,
        bundleBytes,
        sourceProofBytes,
        proofContext,
        proofContext.statementHash(),
        proofContext.destinationBindingHash(),
        hashHex(PROOF_REQUEST_PREFIX_V1, preimage.toByteArray()),
        input.destinationBinding());
  }

  public static ProofResult wrapProofResult(final byte[] proofBytes, final ProofRequest request) {
    Objects.requireNonNull(request, "request");
    if (!GROTH16_BN254_PROOF_BACKEND_V1.equals(request.backend())) {
      throw new IllegalArgumentException("backend must be tron-groth16-bn254-v1");
    }
    requireProductionProofRequest(request);
    requireProofBytesForContext(proofBytes, request.publicInputs(), request.sourceDomain());
    final ByteArrayOutputStream envelopePayload = new ByteArrayOutputStream();
    write(envelopePayload, hex32Bytes(request.requestHash(), "requestHash"));
    write(envelopePayload, proofBytes);
    return new ProofResult(
        1,
        request.backend(),
        Arrays.copyOf(proofBytes, proofBytes.length),
        Base64.getEncoder().encodeToString(proofBytes),
        request.publicInputs(),
        request.publicSignalWords(),
        request.bundleBytes(),
        request.sourceProofBytes(),
        request.proofContext(),
        request.statementHash(),
        request.destinationBindingHash(),
        request.requestHash(),
        hashHex(PROOF_ENVELOPE_PREFIX_V1, envelopePayload.toByteArray()),
        request.destinationBinding());
  }

  private static ProofResult requireWrappedProofResultForSubmission(final ProofResult proofResult) {
    Objects.requireNonNull(proofResult, "proofResult");
    if (!GROTH16_BN254_PROOF_BACKEND_V1.equals(proofResult.backend())) {
      throw new IllegalArgumentException("proofResult.backend must be tron-groth16-bn254-v1");
    }
    final ProofContext proofContext =
        Objects.requireNonNull(proofResult.proofContext(), "proofResult.proofContext");
    final ProofContext expectedProofContext =
        normalizeProofContext(proofResult.statementHash(), proofResult.destinationBindingHash());
    if (!proofContext.equals(expectedProofContext)) {
      throw new IllegalArgumentException(
          "proofResult.proofContext must match statementHash and destinationBindingHash");
    }
    final byte[] proofBytes = proofResult.proofBytes();
    requireProofBytesForPublicInputs(proofBytes, proofResult.publicInputs());
    if (!Base64.getEncoder().encodeToString(proofBytes).equals(proofResult.proofBase64())) {
      throw new IllegalArgumentException(
          "proofResult.proofBase64 must match proofResult.proofBytes");
    }
    final String requestHash = normalizeHex32(proofResult.requestHash(), "proofResult.requestHash");
    final String envelopeHash =
        normalizeHex32(proofResult.envelopeHash(), "proofResult.envelopeHash");
    final ByteArrayOutputStream envelopePayload = new ByteArrayOutputStream();
    write(envelopePayload, hex32Bytes(requestHash, "proofResult.requestHash"));
    write(envelopePayload, proofBytes);
    if (!envelopeHash.equals(hashHex(PROOF_ENVELOPE_PREFIX_V1, envelopePayload.toByteArray()))) {
      throw new IllegalArgumentException(
          "proofResult.envelopeHash must match wrapped proof bytes");
    }
    requireOptionalNonZeroBytes(proofResult.sourceProofBytes(), "proofResult.sourceProofBytes");
    final ProofRequest expectedRequest =
        buildProofRequest(
            new ProofRequestInput(
                proofResult.publicInputs(),
                proofResult.bundleBytes(),
                proofResult.sourceProofBytes(),
                proofResult.statementHash(),
                proofResult.destinationBindingHash(),
                proofResult.backend(),
                DOMAIN_SORA,
                proofResult.destinationBinding()));
    if (!Objects.equals(expectedRequest.requestHash(), requestHash)) {
      throw new IllegalArgumentException(
          "proofResult.requestHash must match bundleBytes and sourceProofBytes");
    }
    return proofResult;
  }

  public static List<String> messageTransparentPublicInputAbiWords(
      final PublicInputsInput publicInputs) {
    final byte[][] words = messageTransparentPublicInputAbiWordBytes(publicInputs);
    final List<String> out = new ArrayList<>(words.length);
    for (final byte[] word : words) {
      out.add("0x" + hexLower(word));
    }
    return Collections.unmodifiableList(out);
  }

  public static byte[] submitMessageProofCallData(
      final byte[] proofBytes, final PublicInputsInput publicInputs, final String statementHash) {
    return submitMessageProofCallData(proofBytes, publicInputs, statementHash, DOMAIN_SORA);
  }

  public static byte[] submitMessageProofCallData(
      final byte[] proofBytes,
      final PublicInputsInput publicInputs,
      final String statementHash,
      final int sourceDomain) {
    if (sourceDomain != DOMAIN_SORA) {
      throw new IllegalArgumentException("sourceDomain must be SORA");
    }
    requireProofBytesForContext(proofBytes, publicInputs, sourceDomain);
    final byte[][] publicInputWords = messageTransparentPublicInputAbiWordBytes(publicInputs);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    write(out, SUBMIT_MESSAGE_PROOF_SELECTOR_BYTES_V1);
    write(out, abiWordU256(32 * 8));
    for (final byte[] word : publicInputWords) {
      write(out, word);
    }
    write(out, nonZeroHex32Bytes(statementHash, "statementHash"));
    write(out, abiWordU256(proofBytes.length));
    write(out, proofBytes);
    final int padding = (32 - (proofBytes.length % 32)) % 32;
    if (padding > 0) {
      write(out, new byte[padding]);
    }
    return out.toByteArray();
  }

  public static Submission buildSubmission(final SubmissionInput input) {
    Objects.requireNonNull(input, "input");
    if (input.sourceDomain() != DOMAIN_SORA) {
      throw new IllegalArgumentException("sourceDomain must be SORA");
    }
    final PublicInputsInput publicInputs = input.publicInputs();
    canonicalPublicInputsBytes(publicInputs);
    final byte[] proofBytes = input.proofBytes();
    requireProofBytesForContext(proofBytes, publicInputs, input.sourceDomain());
    final String statementHash = normalizeHex32(input.statementHash(), "statementHash");
    final String destinationBindingHash =
        normalizeHex32(input.destinationBindingHash(), "destinationBindingHash");
    ProofResult proofResult = input.proofResult();
    if (proofResult != null) {
      proofResult = requireWrappedProofResultForSubmission(proofResult);
    }
    if (proofResult != null) {
      if (!GROTH16_BN254_PROOF_BACKEND_V1.equals(proofResult.backend())) {
        throw new IllegalArgumentException("proofResult.backend must be tron-groth16-bn254-v1");
      }
      if (!Objects.equals(proofResult.publicInputs(), publicInputs)) {
        throw new IllegalArgumentException("publicInputs must match proofResult.publicInputs");
      }
      if (!Arrays.equals(proofResult.proofBytes(), proofBytes)) {
        throw new IllegalArgumentException("proofBytes must match proofResult.proofBytes");
      }
      if (!Objects.equals(proofResult.statementHash(), statementHash)) {
        throw new IllegalArgumentException("statementHash must match proofResult.statementHash");
      }
      if (!Objects.equals(proofResult.destinationBindingHash(), destinationBindingHash)) {
        throw new IllegalArgumentException(
            "destinationBindingHash must match proofResult.destinationBindingHash");
      }
    }
    final List<String> publicSignalWords =
        groth16Bn254PublicSignalWords(
            publicInputs, input.sourceDomain(), statementHash, destinationBindingHash);
    final List<String> suppliedPublicSignalWords =
        input.publicSignalWords() != null
            ? input.publicSignalWords()
            : (proofResult == null ? null : proofResult.publicSignalWords());
    if (suppliedPublicSignalWords != null) {
      if (suppliedPublicSignalWords.size() != 9) {
        throw new IllegalArgumentException("publicSignalWords must contain 9 words");
      }
      final List<String> normalizedSignals = new ArrayList<>(suppliedPublicSignalWords.size());
      for (int index = 0; index < suppliedPublicSignalWords.size(); index++) {
        normalizedSignals.add(
            normalizeOptionalHex32(suppliedPublicSignalWords.get(index), "publicSignalWords[" + index + "]"));
      }
      if (!normalizedSignals.equals(publicSignalWords)) {
        throw new IllegalArgumentException(
            "publicSignalWords must match publicInputs and proof context");
      }
    }
    final List<String> publicInputWords = messageTransparentPublicInputAbiWords(publicInputs);
    final ByteArrayOutputStream publicInputWordsBytesOut = new ByteArrayOutputStream();
    for (final byte[] word : messageTransparentPublicInputAbiWordBytes(publicInputs)) {
      write(publicInputWordsBytesOut, word);
    }
    final byte[] publicInputWordsBytes = publicInputWordsBytesOut.toByteArray();
    final byte[] callData =
        submitMessageProofCallData(proofBytes, publicInputs, statementHash, input.sourceDomain());
    final List<SubmissionArgument> arguments =
        Arrays.asList(
            new SubmissionArgument("proof_bytes", "raw_bytes", "0x" + hexLower(proofBytes)),
            new SubmissionArgument(
                "public_inputs", "abi_bytes32x6", "0x" + hexLower(publicInputWordsBytes)),
            new SubmissionArgument("statement_hash", "abi_bytes32", statementHash));
    return new Submission(
        1,
        STARK_FRI_PROOF_FAMILY_V1,
        GROTH16_BN254_PROOF_BACKEND_V1,
        "tron_contract_call",
        CONTRACT_CALL_ABI_TUPLE_V1,
        "contract_call",
        SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1,
        SUBMIT_MESSAGE_PROOF_ABI_V1,
        SUBMIT_MESSAGE_PROOF_SELECTOR_V1,
        input.sourceDomain(),
        publicInputs.targetDomain(),
        publicInputs,
        publicInputWords,
        publicSignalWords,
        statementHash,
        destinationBindingHash,
        arguments,
        "0x" + hexLower(callData),
        "0x" + hexLower(callData),
        proofBytes,
        publicInputWordsBytes,
        callData);
  }

  private static void requireNonZeroProofBytes(final byte[] proofBytes) {
    if (proofBytes == null || proofBytes.length == 0) {
      throw new IllegalArgumentException("proofBytes must not be empty");
    }
    for (final byte value : proofBytes) {
      if (value != 0) {
        if (proofBytes.length != GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1) {
          throw new IllegalArgumentException(
              "proofBytes must be " + GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1 + " bytes");
        }
        requireGroth16ProofTuple(proofBytes, "proofBytes");
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

  private static void requireProofBytesForPublicInputs(
      final byte[] proofBytes, final PublicInputsInput publicInputs) {
    requireNonZeroProofBytes(proofBytes);
    if (!Arrays.equals(
        proofWord(proofBytes, 1),
        nonZeroHex32Bytes(publicInputs.messageId(), "publicInputs.messageId"))) {
      throw new IllegalArgumentException("proofBytes.messageId must match publicInputs.messageId");
    }
    if (!Arrays.equals(
        proofWord(proofBytes, 3),
        nonZeroHex32Bytes(publicInputs.commitmentRoot(), "publicInputs.commitmentRoot"))) {
      throw new IllegalArgumentException(
          "proofBytes.commitmentRoot must match publicInputs.commitmentRoot");
    }
  }

  private static void requireProofBytesForContext(
      final byte[] proofBytes, final PublicInputsInput publicInputs, final int sourceDomain) {
    requireProofBytesForPublicInputs(proofBytes, publicInputs);
    if (!proofWordValue(proofBytes, 2).equals(BigInteger.valueOf(sourceDomain))) {
      throw new IllegalArgumentException("proofBytes.sourceDomain must match sourceDomain");
    }
  }

  private static byte[] proofWord(final byte[] proofBytes, final int index) {
    return Arrays.copyOfRange(proofBytes, index * 32, (index + 1) * 32);
  }

  private static BigInteger proofWordValue(final byte[] proofBytes, final int index) {
    return new BigInteger(1, proofWord(proofBytes, index));
  }

  private static boolean proofWordIsZero(final byte[] proofBytes, final int index) {
    for (final byte value : proofWord(proofBytes, index)) {
      if (value != 0) {
        return false;
      }
    }
    return true;
  }

  private static void requireBaseFieldWord(
      final byte[] proofBytes, final int index, final String label) {
    if (proofWordValue(proofBytes, index).compareTo(BN254_BASE_FIELD_MODULUS) >= 0) {
      throw new IllegalArgumentException(label + " must be a BN254 base-field element");
    }
  }

  private static void requireNonZeroPoint(
      final byte[] proofBytes, final int[] indexes, final String label) {
    for (final int index : indexes) {
      if (!proofWordIsZero(proofBytes, index)) {
        return;
      }
    }
    throw new IllegalArgumentException(label + " must not be zero");
  }

  private static BigInteger fq(final BigInteger value) {
    return value.mod(BN254_BASE_FIELD_MODULUS);
  }

  private static final class G2ProjectivePoint {
    private final BigInteger[] x;
    private final BigInteger[] y;
    private final BigInteger[] z;
    private final boolean infinity;

    private G2ProjectivePoint(
        final BigInteger[] x,
        final BigInteger[] y,
        final BigInteger[] z,
        final boolean infinity) {
      this.x = x;
      this.y = y;
      this.z = z;
      this.infinity = infinity;
    }
  }

  private static BigInteger[] fq2Add(final BigInteger[] left, final BigInteger[] right) {
    return new BigInteger[] {fq(left[0].add(right[0])), fq(left[1].add(right[1]))};
  }

  private static BigInteger[] fq2Sub(final BigInteger[] left, final BigInteger[] right) {
    return new BigInteger[] {fq(left[0].subtract(right[0])), fq(left[1].subtract(right[1]))};
  }

  private static BigInteger[] fq2Scale(final BigInteger[] left, final long scalar) {
    final BigInteger value = BigInteger.valueOf(scalar);
    return new BigInteger[] {fq(left[0].multiply(value)), fq(left[1].multiply(value))};
  }

  private static BigInteger[] fq2Mul(final BigInteger[] left, final BigInteger[] right) {
    return new BigInteger[] {
      fq(left[0].multiply(right[0]).subtract(left[1].multiply(right[1]))),
      fq(left[0].multiply(right[1]).add(left[1].multiply(right[0])))
    };
  }

  private static boolean fq2IsZero(final BigInteger[] value) {
    return BigInteger.ZERO.equals(value[0]) && BigInteger.ZERO.equals(value[1]);
  }

  private static G2ProjectivePoint g2Infinity() {
    return new G2ProjectivePoint(
        new BigInteger[] {BigInteger.ZERO, BigInteger.ZERO},
        new BigInteger[] {BigInteger.ONE, BigInteger.ZERO},
        new BigInteger[] {BigInteger.ZERO, BigInteger.ZERO},
        true);
  }

  private static G2ProjectivePoint g2AffineProjective(
      final BigInteger[] x, final BigInteger[] y) {
    return new G2ProjectivePoint(
        x, y, new BigInteger[] {BigInteger.ONE, BigInteger.ZERO}, false);
  }

  private static boolean g2ProjectiveIsInfinity(final G2ProjectivePoint point) {
    return point.infinity || fq2IsZero(point.z);
  }

  private static G2ProjectivePoint g2ProjectiveDouble(final G2ProjectivePoint point) {
    if (g2ProjectiveIsInfinity(point) || fq2IsZero(point.y)) {
      return g2Infinity();
    }
    final BigInteger[] xx = fq2Mul(point.x, point.x);
    final BigInteger[] yy = fq2Mul(point.y, point.y);
    final BigInteger[] yyyy = fq2Mul(yy, yy);
    final BigInteger[] s =
        fq2Scale(
            fq2Sub(fq2Sub(fq2Mul(fq2Add(point.x, yy), fq2Add(point.x, yy)), xx), yyyy),
            2);
    final BigInteger[] m = fq2Scale(xx, 3);
    final BigInteger[] x3 = fq2Sub(fq2Mul(m, m), fq2Scale(s, 2));
    final BigInteger[] y3 =
        fq2Sub(fq2Mul(m, fq2Sub(s, x3)), fq2Scale(yyyy, 8));
    final BigInteger[] z3 = fq2Scale(fq2Mul(point.y, point.z), 2);
    return new G2ProjectivePoint(x3, y3, z3, false);
  }

  private static G2ProjectivePoint g2ProjectiveAddAffine(
      final G2ProjectivePoint point, final BigInteger[] affineX, final BigInteger[] affineY) {
    if (g2ProjectiveIsInfinity(point)) {
      return g2AffineProjective(affineX, affineY);
    }
    final BigInteger[] z1z1 = fq2Mul(point.z, point.z);
    final BigInteger[] u2 = fq2Mul(affineX, z1z1);
    final BigInteger[] s2 = fq2Mul(affineY, fq2Mul(point.z, z1z1));
    final BigInteger[] h = fq2Sub(u2, point.x);
    if (fq2IsZero(h)) {
      return Arrays.equals(s2, point.y) ? g2ProjectiveDouble(point) : g2Infinity();
    }
    final BigInteger[] hh = fq2Mul(h, h);
    final BigInteger[] i = fq2Scale(hh, 4);
    final BigInteger[] j = fq2Mul(h, i);
    final BigInteger[] r = fq2Scale(fq2Sub(s2, point.y), 2);
    final BigInteger[] v = fq2Mul(point.x, i);
    final BigInteger[] x3 = fq2Sub(fq2Sub(fq2Mul(r, r), j), fq2Scale(v, 2));
    final BigInteger[] y3 =
        fq2Sub(fq2Mul(r, fq2Sub(v, x3)), fq2Scale(fq2Mul(point.y, j), 2));
    final BigInteger[] z3 =
        fq2Sub(fq2Sub(fq2Mul(fq2Add(point.z, h), fq2Add(point.z, h)), z1z1), hh);
    return new G2ProjectivePoint(x3, y3, z3, false);
  }

  private static boolean g2PointIsInPrimeSubgroup(
      final BigInteger[] x, final BigInteger[] y) {
    G2ProjectivePoint acc = g2Infinity();
    for (int index = BN254_SCALAR_FIELD_MODULUS.bitLength() - 1; index >= 0; index--) {
      acc = g2ProjectiveDouble(acc);
      if (BN254_SCALAR_FIELD_MODULUS.testBit(index)) {
        acc = g2ProjectiveAddAffine(acc, x, y);
      }
    }
    return g2ProjectiveIsInfinity(acc);
  }

  private static void requireG1Point(
      final byte[] proofBytes, final int xIndex, final int yIndex, final String label) {
    requireNonZeroPoint(proofBytes, new int[] {xIndex, yIndex}, label);
    final BigInteger x = proofWordValue(proofBytes, xIndex);
    final BigInteger y = proofWordValue(proofBytes, yIndex);
    final BigInteger left = fq(y.multiply(y));
    final BigInteger right = fq(x.multiply(x).multiply(x).add(BigInteger.valueOf(3)));
    if (!left.equals(right)) {
      throw new IllegalArgumentException(label + " must be a BN254 G1 point");
    }
  }

  private static void requireG2Point(
      final byte[] proofBytes,
      final int x0Index,
      final int x1Index,
      final int y0Index,
      final int y1Index,
      final String label) {
    requireNonZeroPoint(proofBytes, new int[] {x0Index, x1Index, y0Index, y1Index}, label);
    final BigInteger[] x =
        new BigInteger[] {proofWordValue(proofBytes, x0Index), proofWordValue(proofBytes, x1Index)};
    final BigInteger[] y =
        new BigInteger[] {proofWordValue(proofBytes, y0Index), proofWordValue(proofBytes, y1Index)};
    final BigInteger[] left = fq2Mul(y, y);
    final BigInteger[] x2 = fq2Mul(x, x);
    final BigInteger[] x3 = fq2Mul(x2, x);
    final BigInteger[] right =
        new BigInteger[] {fq(x3[0].add(BN254_G2_B_C0)), fq(x3[1].add(BN254_G2_B_C1))};
    if (!left[0].equals(right[0]) || !left[1].equals(right[1]) || !g2PointIsInPrimeSubgroup(x, y)) {
      throw new IllegalArgumentException(label + " must be a BN254 G2 point");
    }
  }

  private static void requireGroth16ProofTuple(final byte[] proofBytes, final String label) {
    if (!BigInteger.ONE.equals(proofWordValue(proofBytes, 0))) {
      throw new IllegalArgumentException(label + ".version must be 1");
    }
    if (proofWordIsZero(proofBytes, 1)) {
      throw new IllegalArgumentException(label + ".messageId must not be zero");
    }
    if (proofWordValue(proofBytes, 2).compareTo(BigInteger.valueOf(0xffff_ffffL)) > 0) {
      throw new IllegalArgumentException(label + ".sourceDomain must fit u32");
    }
    if (proofWordIsZero(proofBytes, 3)) {
      throw new IllegalArgumentException(label + ".commitmentRoot must not be zero");
    }
    final String[] fields = {"a.x", "a.y", "b.x0", "b.x1", "b.y0", "b.y1", "c.x", "c.y"};
    for (int offset = 0; offset < fields.length; offset++) {
      requireBaseFieldWord(proofBytes, 4 + offset, label + "." + fields[offset]);
    }
    requireG1Point(proofBytes, 4, 5, label + ".a");
    requireG2Point(proofBytes, 6, 7, 8, 9, label + ".b");
    requireG1Point(proofBytes, 10, 11, label + ".c");
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
                request.sourceDomain(),
                request.destinationBinding()));
    if (request.version() != expected.version()
        || !Objects.equals(request.backend(), expected.backend())
        || request.sourceDomain() != expected.sourceDomain()
        || request.targetDomain() != expected.targetDomain()
        || !Objects.equals(request.publicInputs(), expected.publicInputs())
        || !Arrays.equals(request.publicInputsBytes(), expected.publicInputsBytes())
        || !Objects.equals(request.publicSignalWords(), expected.publicSignalWords())
        || !Arrays.equals(request.bundleBytes(), expected.bundleBytes())
        || !Arrays.equals(request.sourceProofBytes(), expected.sourceProofBytes())
        || !Objects.equals(request.proofContext(), expected.proofContext())
        || !Objects.equals(request.statementHash(), expected.statementHash())
        || !Objects.equals(request.destinationBindingHash(), expected.destinationBindingHash())
        || !Objects.equals(request.requestHash(), expected.requestHash())
        || !Objects.equals(request.destinationBinding(), expected.destinationBinding())) {
      throw new IllegalArgumentException("proof request must be canonical");
    }
  }

  private static void requireProductionProofRequest(final ProofRequest request) {
    requireCanonicalProofRequest(request);
    if (request.version() != 1) {
      throw new IllegalArgumentException("proof request version must be 1");
    }
    if (!GROTH16_BN254_PROOF_BACKEND_V1.equals(request.backend())) {
      throw new IllegalArgumentException(
          "TRON SCCP proof request backend must be tron-groth16-bn254-v1");
    }
    if (request.sourceDomain() != DOMAIN_SORA) {
      throw new IllegalArgumentException("TRON SCCP production proofs must start from SORA");
    }
    if (request.targetDomain() != request.publicInputs().targetDomain()
        || request.targetDomain() != DOMAIN_TRON) {
      throw new IllegalArgumentException("TRON SCCP production proofs must target TRON");
    }
    if (request.bundleBytes().length == 0) {
      throw new IllegalArgumentException("TRON SCCP proof request bundleBytes must not be empty");
    }
    requireOptionalNonZeroBytes(
        request.sourceProofBytes(), "TRON SCCP proof request sourceProofBytes");
    requireProductionDestinationBinding(request);
  }

  private static void requireProductionDestinationBinding(final ProofRequest request) {
    final SourceSccpProofs.TronDestinationBinding destinationBinding = request.destinationBinding();
    if (destinationBinding == null) {
      throw new IllegalArgumentException(
          "TRON SCCP production proof request destinationBinding must include deployment material");
    }
    final String destinationBindingHash =
        requireDestinationBindingHash(
            request.publicInputs(), destinationBinding, request.backend(), request.sourceDomain());
    if (!Objects.equals(request.destinationBindingHash(), destinationBindingHash)) {
      throw new IllegalArgumentException(
          "destinationBindingHash must match destinationBinding deployment material");
    }
    if (!Objects.equals(request.proofContext().destinationBindingHash(), destinationBindingHash)) {
      throw new IllegalArgumentException(
          "proofContext.destinationBindingHash must match destinationBinding deployment material");
    }
  }

  private static String requireDestinationBindingHash(
      final PublicInputsInput publicInputs,
      final SourceSccpProofs.TronDestinationBinding destinationBinding,
      final String backend,
      final int sourceDomain) {
    final SourceSccpProofs.TronDestinationBinding binding =
        Objects.requireNonNull(destinationBinding, "destinationBinding");
    if (binding.version != 1) {
      throw new IllegalArgumentException("destinationBinding.version must be 1");
    }
    if (binding.sourceDomain != sourceDomain) {
      throw new IllegalArgumentException("destinationBinding.sourceDomain must match sourceDomain");
    }
    if (binding.targetDomain != publicInputs.targetDomain()) {
      throw new IllegalArgumentException(
          "destinationBinding.targetDomain must match publicInputs.targetDomain");
    }
    if (!binding.verifierBackend.equals(backend)) {
      throw new IllegalArgumentException("destinationBinding.verifierBackend must match backend");
    }
    if (!STARK_FRI_PROOF_FAMILY_V1.equals(binding.proofFamily)) {
      throw new IllegalArgumentException("destinationBinding.proofFamily must be stark-fri-v1");
    }
    final SourceSccpProofs.TronDestinationBinding expected =
        SourceSccpProofs.tronDestinationBinding(
            sourceDomain,
            publicInputs.targetDomain(),
            binding.networkId,
            binding.verifierAddress,
            binding.verifierCodeHash,
            binding.verifierKeyHash);
    if (!tronDestinationBindingsEqual(binding, expected)) {
      throw new IllegalArgumentException("destinationBinding must match deployment material");
    }
    return expected.hash;
  }

  private static boolean tronDestinationBindingsEqual(
      final SourceSccpProofs.TronDestinationBinding left,
      final SourceSccpProofs.TronDestinationBinding right) {
    return left.version == right.version
        && left.sourceDomain == right.sourceDomain
        && left.targetDomain == right.targetDomain
        && Objects.equals(left.networkId, right.networkId)
        && Objects.equals(left.verifierAddress, right.verifierAddress)
        && Objects.equals(left.verifierCodeHash, right.verifierCodeHash)
        && Objects.equals(left.verifierKeyHash, right.verifierKeyHash)
        && Objects.equals(left.verifierBackend, right.verifierBackend)
        && Objects.equals(left.proofFamily, right.proofFamily)
        && Objects.equals(left.key, right.key)
        && Objects.equals(left.hash, right.hash);
  }

  private static String groth16SignalWord(final String label, final byte[] value) {
    final byte[] labelHash = keccak256(label.getBytes(StandardCharsets.UTF_8));
    final byte[] payload = new byte[labelHash.length + value.length];
    System.arraycopy(labelHash, 0, payload, 0, labelHash.length);
    System.arraycopy(value, 0, payload, labelHash.length, value.length);
    final BigInteger reduced = new BigInteger(1, keccak256(payload)).mod(BN254_SCALAR_FIELD_MODULUS);
    return "0x" + hexLower(toFixedBytes(reduced, 32));
  }

  private static byte[] keccak256(final byte[] input) {
    final KeccakDigest digest = new KeccakDigest(256);
    digest.update(input, 0, input.length);
    final byte[] out = new byte[32];
    digest.doFinal(out, 0);
    return out;
  }

  private static byte[] abiWordU32(final int value) {
    if (value < 0) {
      throw new IllegalArgumentException("domain id must not be negative");
    }
    final byte[] out = new byte[32];
    out[28] = (byte) ((value >>> 24) & 0xff);
    out[29] = (byte) ((value >>> 16) & 0xff);
    out[30] = (byte) ((value >>> 8) & 0xff);
    out[31] = (byte) (value & 0xff);
    return out;
  }

  private static byte[] abiWordU64(final BigInteger value) {
    BigInteger working = value;
    final byte[] out = new byte[32];
    for (int index = 31; index >= 24; index--) {
      out[index] = working.and(BigInteger.valueOf(0xffL)).byteValue();
      working = working.shiftRight(8);
    }
    return out;
  }

  private static byte[] abiWordU256(final int value) {
    if (value < 0) {
      throw new IllegalArgumentException("u256 value must not be negative");
    }
    int working = value;
    final byte[] out = new byte[32];
    for (int index = 31; index >= 0; index--) {
      out[index] = (byte) (working & 0xff);
      working >>>= 8;
      if (working == 0) {
        break;
      }
    }
    return out;
  }

  private static byte[][] messageTransparentPublicInputAbiWordBytes(
      final PublicInputsInput publicInputs) {
    canonicalPublicInputsBytes(publicInputs);
    return new byte[][] {
      nonZeroHex32Bytes(publicInputs.messageId(), "messageId"),
      nonZeroHex32Bytes(publicInputs.payloadHash(), "payloadHash"),
      abiWordU32(publicInputs.targetDomain()),
      nonZeroHex32Bytes(publicInputs.commitmentRoot(), "commitmentRoot"),
      abiWordU64(normalizeU64(publicInputs.finalityHeight(), "finalityHeight")),
      nonZeroHex32Bytes(publicInputs.finalityBlockHash(), "finalityBlockHash")
    };
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

  private static String normalizeOptionalHex32(final String value, final String field) {
    return "0x" + hexLower(hex32Bytes(value, field));
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
    for (int i = 1; i < value.length(); i++) {
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

  private static byte[] toFixedBytes(final BigInteger value, final int length) {
    final byte[] raw = value.toByteArray();
    final byte[] out = new byte[length];
    final int copyLength = Math.min(raw.length, length);
    System.arraycopy(raw, raw.length - copyLength, out, length - copyLength, copyLength);
    return out;
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
          DOMAIN_TRON,
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
      int sourceDomain,
      SourceSccpProofs.TronDestinationBinding destinationBinding) {
    public ProofRequestInput(
        final PublicInputsInput publicInputs,
        final byte[] bundleBytes,
        final byte[] sourceProofBytes,
        final String statementHash,
        final String destinationBindingHash,
        final String backend,
        final int sourceDomain) {
      this(
          publicInputs,
          bundleBytes,
          sourceProofBytes,
          statementHash,
          destinationBindingHash,
          backend,
          sourceDomain,
          null);
    }

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
          GROTH16_BN254_PROOF_BACKEND_V1,
          DOMAIN_SORA,
          null);
    }

    public ProofRequestInput(
        final PublicInputsInput publicInputs,
        final byte[] bundleBytes,
        final byte[] sourceProofBytes,
        final String statementHash,
        final SourceSccpProofs.TronDestinationBinding destinationBinding,
        final String backend,
        final int sourceDomain) {
      this(
          publicInputs,
          bundleBytes,
          sourceProofBytes,
          statementHash,
          requireDestinationBindingHash(publicInputs, destinationBinding, backend, sourceDomain),
          backend,
          sourceDomain,
          destinationBinding);
    }

    public ProofRequestInput(
        final PublicInputsInput publicInputs,
        final byte[] bundleBytes,
        final byte[] sourceProofBytes,
        final String statementHash,
        final SourceSccpProofs.TronDestinationBinding destinationBinding) {
      this(
          publicInputs,
          bundleBytes,
          sourceProofBytes,
          statementHash,
          destinationBinding,
          GROTH16_BN254_PROOF_BACKEND_V1,
          DOMAIN_SORA);
    }

    public ProofRequestInput(
        final PublicInputsInput publicInputs,
        final byte[] bundleBytes,
        final String statementHash,
        final SourceSccpProofs.TronDestinationBinding destinationBinding) {
      this(
          publicInputs,
          bundleBytes,
          new byte[0],
          statementHash,
          destinationBinding,
          GROTH16_BN254_PROOF_BACKEND_V1,
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
      List<String> publicSignalWords,
      byte[] bundleBytes,
      byte[] sourceProofBytes,
      ProofContext proofContext,
      String statementHash,
      String destinationBindingHash,
      String requestHash,
      SourceSccpProofs.TronDestinationBinding destinationBinding) {
    public ProofRequest(
        final int version,
        final String backend,
        final int sourceDomain,
        final int targetDomain,
        final PublicInputsInput publicInputs,
        final byte[] publicInputsBytes,
        final List<String> publicSignalWords,
        final byte[] bundleBytes,
        final byte[] sourceProofBytes,
        final ProofContext proofContext,
        final String statementHash,
        final String destinationBindingHash,
        final String requestHash) {
      this(
          version,
          backend,
          sourceDomain,
          targetDomain,
          publicInputs,
          publicInputsBytes,
          publicSignalWords,
          bundleBytes,
          sourceProofBytes,
          proofContext,
          statementHash,
          destinationBindingHash,
          requestHash,
          null);
    }

    public ProofRequest {
      publicInputsBytes =
          Arrays.copyOf(
              Objects.requireNonNull(publicInputsBytes, "publicInputsBytes"),
              publicInputsBytes.length);
      publicSignalWords =
          Collections.unmodifiableList(
              new ArrayList<>(Objects.requireNonNull(publicSignalWords, "publicSignalWords")));
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
      List<String> publicSignalWords,
      byte[] bundleBytes,
      byte[] sourceProofBytes,
      ProofContext proofContext,
      String statementHash,
      String destinationBindingHash,
      String requestHash,
      String envelopeHash,
      SourceSccpProofs.TronDestinationBinding destinationBinding) {
    public ProofResult(
        final int version,
        final String backend,
        final byte[] proofBytes,
        final String proofBase64,
        final PublicInputsInput publicInputs,
        final List<String> publicSignalWords,
        final byte[] bundleBytes,
        final byte[] sourceProofBytes,
        final ProofContext proofContext,
        final String statementHash,
        final String destinationBindingHash,
        final String requestHash,
        final String envelopeHash) {
      this(
          version,
          backend,
          proofBytes,
          proofBase64,
          publicInputs,
          publicSignalWords,
          bundleBytes,
          sourceProofBytes,
          proofContext,
          statementHash,
          destinationBindingHash,
          requestHash,
          envelopeHash,
          null);
    }

    public ProofResult {
      proofBytes = Arrays.copyOf(Objects.requireNonNull(proofBytes, "proofBytes"), proofBytes.length);
      publicSignalWords =
          Collections.unmodifiableList(
              new ArrayList<>(Objects.requireNonNull(publicSignalWords, "publicSignalWords")));
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
      String statementHash,
      String destinationBindingHash,
      int sourceDomain,
      ProofResult proofResult,
      List<String> publicSignalWords) {
    public SubmissionInput {
      proofBytes = Arrays.copyOf(Objects.requireNonNull(proofBytes, "proofBytes"), proofBytes.length);
      if (publicSignalWords != null) {
        publicSignalWords = Collections.unmodifiableList(new ArrayList<>(publicSignalWords));
      }
    }

    public SubmissionInput(final ProofResult proofResult) {
      this(
          Objects.requireNonNull(proofResult, "proofResult").publicInputs(),
          proofResult.proofBytes(),
          proofResult.statementHash(),
          proofResult.destinationBindingHash(),
          DOMAIN_SORA,
          proofResult,
          proofResult.publicSignalWords());
    }

    public SubmissionInput(
        final PublicInputsInput publicInputs,
        final byte[] proofBytes,
        final String statementHash,
        final String destinationBindingHash) {
      this(publicInputs, proofBytes, statementHash, destinationBindingHash, DOMAIN_SORA, null, null);
    }

    public SubmissionInput(
        final PublicInputsInput publicInputs,
        final byte[] proofBytes,
        final String statementHash,
        final SourceSccpProofs.TronDestinationBinding destinationBinding,
        final int sourceDomain,
        final ProofResult proofResult,
        final List<String> publicSignalWords) {
      this(
          publicInputs,
          proofBytes,
          statementHash,
          requireDestinationBindingHash(
              publicInputs, destinationBinding, GROTH16_BN254_PROOF_BACKEND_V1, sourceDomain),
          sourceDomain,
          proofResult,
          publicSignalWords);
    }

    public SubmissionInput(
        final PublicInputsInput publicInputs,
        final byte[] proofBytes,
        final String statementHash,
        final SourceSccpProofs.TronDestinationBinding destinationBinding) {
      this(publicInputs, proofBytes, statementHash, destinationBinding, DOMAIN_SORA, null, null);
    }

    @Override
    public byte[] proofBytes() {
      return Arrays.copyOf(proofBytes, proofBytes.length);
    }
  }

  public record SubmissionArgument(String key, String encoding, String bytes) {}

  public record Submission(
      int version,
      String proofFamily,
      String verifierBackend,
      String platformPayload,
      String envelopeEncoding,
      String submissionKind,
      String verifierEntrypoint,
      String contractMethod,
      String functionSelector,
      int sourceDomain,
      int targetDomain,
      PublicInputsInput publicInputs,
      List<String> publicInputWords,
      List<String> publicSignalWords,
      String statementHash,
      String destinationBindingHash,
      List<SubmissionArgument> arguments,
      String callDataHex,
      String envelopeHex,
      byte[] proofBytes,
      byte[] publicInputWordsBytes,
      byte[] callData) {
    public Submission {
      publicInputWords =
          Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(publicInputWords)));
      publicSignalWords =
          Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(publicSignalWords)));
      arguments = Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(arguments)));
      proofBytes = Arrays.copyOf(Objects.requireNonNull(proofBytes, "proofBytes"), proofBytes.length);
      publicInputWordsBytes =
          Arrays.copyOf(
              Objects.requireNonNull(publicInputWordsBytes, "publicInputWordsBytes"),
              publicInputWordsBytes.length);
      callData = Arrays.copyOf(Objects.requireNonNull(callData, "callData"), callData.length);
    }

    @Override
    public byte[] proofBytes() {
      return Arrays.copyOf(proofBytes, proofBytes.length);
    }

    @Override
    public byte[] publicInputWordsBytes() {
      return Arrays.copyOf(publicInputWordsBytes, publicInputWordsBytes.length);
    }

    @Override
    public byte[] callData() {
      return Arrays.copyOf(callData, callData.length);
    }

    public byte[] envelopeBytes() {
      return Arrays.copyOf(callData, callData.length);
    }
  }
}
