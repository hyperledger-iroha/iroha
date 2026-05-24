package org.hyperledger.iroha.android.sccp;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Base64;
import java.util.Objects;
import org.hyperledger.iroha.android.crypto.Blake2b;

/** Solana SCCP proof request helpers for local-first Android proof generation. */
public final class SolanaSccpProver {
  public static final int DOMAIN_SORA = 0;
  public static final int DOMAIN_SOLANA = 3;
  public static final String RECURSIVE_PROOF_BACKEND_V1 =
      "sccp-solana-recursive-mainnet-v1";
  public static final String MAINNET_GENESIS_HASH = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp";

  private static final BigInteger MAX_U64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);

  private final WitnessProvider witnessProvider;
  private final ProofEngine proofEngine;

  public SolanaSccpProver() {
    this(null, null);
  }

  public SolanaSccpProver(final WitnessProvider witnessProvider, final ProofEngine proofEngine) {
    this.witnessProvider = witnessProvider;
    this.proofEngine = proofEngine;
  }

  public ProofRequest buildRequest(final WitnessInput input) {
    final WitnessInput resolved = witnessProvider == null ? input : witnessProvider.resolveWitness(input);
    return buildProofRequest(resolved);
  }

  public ProofResult prove(final WitnessInput input) {
    final ProofRequest request = buildRequest(input);
    if (proofEngine == null) {
      throw new IllegalStateException("Solana SCCP local prover is not linked");
    }
    return wrapProofResult(proofEngine.prove(request), request);
  }

  public static Witness normalizeWitness(final WitnessInput input) {
    Objects.requireNonNull(input, "input");
    return new Witness(
        1,
        DOMAIN_SOLANA,
        input.targetDomain(),
        normalizeNonEmpty(input.mainnetGenesisHash(), "mainnetGenesisHash"),
        normalizeU64(input.finalizedSlot(), "finalizedSlot").toString(),
        normalizeNonEmpty(input.blockhash(), "blockhash"),
        normalizeHex32(input.bankHash(), "bankHash"),
        normalizeHex32(input.transactionStatusRoot(), "transactionStatusRoot"),
        normalizeHex32(input.messageProofHash(), "messageProofHash"),
        normalizeNonEmpty(input.transactionSignature(), "transactionSignature"),
        normalizeNonEmpty(input.emitterProgramId(), "emitterProgramId"),
        normalizeHex32(input.messageId(), "messageId"),
        normalizeHex32(input.payloadHash(), "payloadHash"),
        normalizeHex32(input.commitmentRoot(), "commitmentRoot"),
        normalizeHex32(input.sourceEventDigest(), "sourceEventDigest"));
  }

  public static byte[] canonicalWitnessBytes(final WitnessInput input) {
    return canonicalWitnessBytes(normalizeWitness(input));
  }

  public static byte[] canonicalWitnessBytes(final Witness witness) {
    Objects.requireNonNull(witness, "witness");
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(witness.version());
    writeU32Le(out, witness.sourceDomain());
    writeU32Le(out, witness.targetDomain());
    writeString(out, witness.mainnetGenesisHash(), "mainnetGenesisHash");
    writeU64Le(out, normalizeU64(witness.finalizedSlot(), "finalizedSlot"));
    writeString(out, witness.blockhash(), "blockhash");
    writeString(out, witness.transactionSignature(), "transactionSignature");
    writeString(out, witness.emitterProgramId(), "emitterProgramId");
    write(out, hex32Bytes(witness.bankHash(), "bankHash"));
    write(out, hex32Bytes(witness.transactionStatusRoot(), "transactionStatusRoot"));
    write(out, hex32Bytes(witness.messageProofHash(), "messageProofHash"));
    write(out, hex32Bytes(witness.messageId(), "messageId"));
    write(out, hex32Bytes(witness.payloadHash(), "payloadHash"));
    write(out, hex32Bytes(witness.commitmentRoot(), "commitmentRoot"));
    write(out, hex32Bytes(witness.sourceEventDigest(), "sourceEventDigest"));
    return out.toByteArray();
  }

  public static byte[] canonicalMessageProofBytes(
      final String sourceEventDigest,
      final String transactionStatusRoot,
      final byte[][] inclusionBranch) {
    Objects.requireNonNull(inclusionBranch, "inclusionBranch");
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    write(out, hex32Bytes(sourceEventDigest, "sourceEventDigest"));
    write(out, hex32Bytes(transactionStatusRoot, "transactionStatusRoot"));
    writeU32Le(out, inclusionBranch.length);
    for (int i = 0; i < inclusionBranch.length; i++) {
      final byte[] sibling = Objects.requireNonNull(inclusionBranch[i], "inclusionBranch[" + i + "]");
      if (sibling.length != 32) {
        throw new IllegalArgumentException("inclusionBranch[" + i + "] must be 32 bytes");
      }
      write(out, sibling);
    }
    return out.toByteArray();
  }

  public static String messageProofHash(
      final String sourceEventDigest,
      final String transactionStatusRoot,
      final byte[][] inclusionBranch) {
    return hashHex(
        "sccp:solana:message-proof:v1",
        canonicalMessageProofBytes(sourceEventDigest, transactionStatusRoot, inclusionBranch));
  }

  public static ProofRequest buildProofRequest(final WitnessInput input) {
    final Witness witness = normalizeWitness(input);
    final String witnessHash = hashHex("sccp:solana:witness:v1", canonicalWitnessBytes(witness));
    return new ProofRequest(
        1,
        RECURSIVE_PROOF_BACKEND_V1,
        DOMAIN_SOLANA,
        witness.targetDomain(),
        witness.mainnetGenesisHash(),
        witnessHash,
        new PublicInputs(
            witness.messageId(),
            witness.payloadHash(),
            witness.commitmentRoot(),
            witness.finalizedSlot(),
            witness.blockhash(),
            witness.sourceEventDigest()),
        witness);
  }

  public static ProofResult wrapProofResult(final byte[] proofBytes, final ProofRequest request) {
    Objects.requireNonNull(request, "request");
    if (proofBytes == null || proofBytes.length == 0) {
      throw new IllegalArgumentException("proofBytes must not be empty");
    }
    final ByteArrayOutputStream payload = new ByteArrayOutputStream();
    write(payload, hex32Bytes(request.witnessHash(), "witnessHash"));
    write(payload, proofBytes);
    return new ProofResult(
        1,
        request.backend(),
        Arrays.copyOf(proofBytes, proofBytes.length),
        Base64.getEncoder().encodeToString(proofBytes),
        request.publicInputs(),
        request.witnessHash(),
        hashHex("sccp:solana:proof-envelope:v1", payload.toByteArray()));
  }

  private static String hashHex(final String prefix, final byte[] payload) {
    final byte[] prefixBytes = prefix.getBytes(StandardCharsets.UTF_8);
    final byte[] preimage = new byte[prefixBytes.length + payload.length];
    System.arraycopy(prefixBytes, 0, preimage, 0, prefixBytes.length);
    System.arraycopy(payload, 0, preimage, prefixBytes.length, payload.length);
    return "0x" + hexLower(Blake2b.digest256(preimage));
  }

  private static String normalizeNonEmpty(final String value, final String field) {
    final String trimmed = Objects.requireNonNull(value, field).trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException(field + " must be non-empty");
    }
    return trimmed;
  }

  private static String normalizeHex32(final String value, final String field) {
    return "0x" + hexLower(hex32Bytes(value, field));
  }

  private static byte[] hex32Bytes(final String value, final String field) {
    String body = Objects.requireNonNull(value, field).trim();
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

  private static BigInteger normalizeU64(final String value, final String field) {
    final String trimmed = Objects.requireNonNull(value, field).trim();
    if (!trimmed.matches("[0-9]+")) {
      throw new IllegalArgumentException(field + " must be an unsigned integer");
    }
    final BigInteger numeric = new BigInteger(trimmed);
    if (numeric.compareTo(MAX_U64) > 0) {
      throw new IllegalArgumentException(field + " must fit u64");
    }
    return numeric;
  }

  private static void writeString(
      final ByteArrayOutputStream out, final String value, final String field) {
    final byte[] bytes = normalizeNonEmpty(value, field).getBytes(StandardCharsets.UTF_8);
    writeU32Le(out, bytes.length);
    write(out, bytes);
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

  /** Optional witness resolver backed by app-controlled Solana RPC calls. */
  public interface WitnessProvider {
    WitnessInput resolveWitness(WitnessInput input);
  }

  /** Local proof engine linked by the application bundle. */
  public interface ProofEngine {
    byte[] prove(ProofRequest request);
  }

  /** Raw Solana SCCP witness data collected by portal or mobile UI code. */
  public record WitnessInput(
      int targetDomain,
      String mainnetGenesisHash,
      String finalizedSlot,
      String blockhash,
      String bankHash,
      String transactionStatusRoot,
      String messageProofHash,
      String transactionSignature,
      String emitterProgramId,
      String messageId,
      String payloadHash,
      String commitmentRoot,
      String sourceEventDigest) {
    public WitnessInput(
        final String finalizedSlot,
        final String blockhash,
        final String bankHash,
        final String transactionStatusRoot,
        final String messageProofHash,
        final String transactionSignature,
        final String emitterProgramId,
        final String messageId,
        final String payloadHash,
        final String commitmentRoot,
        final String sourceEventDigest) {
      this(
          DOMAIN_SORA,
          MAINNET_GENESIS_HASH,
          finalizedSlot,
          blockhash,
          bankHash,
          transactionStatusRoot,
          messageProofHash,
          transactionSignature,
          emitterProgramId,
          messageId,
          payloadHash,
          commitmentRoot,
          sourceEventDigest);
    }
  }

  /** Canonical Solana SCCP witness passed into local proof generation. */
  public record Witness(
      int version,
      int sourceDomain,
      int targetDomain,
      String mainnetGenesisHash,
      String finalizedSlot,
      String blockhash,
      String bankHash,
      String transactionStatusRoot,
      String messageProofHash,
      String transactionSignature,
      String emitterProgramId,
      String messageId,
      String payloadHash,
      String commitmentRoot,
      String sourceEventDigest) {}

  /** Public inputs exposed by the Solana SCCP proof request. */
  public record PublicInputs(
      String messageId,
      String payloadHash,
      String commitmentRoot,
      String finalizedSlot,
      String blockhash,
      String sourceEventDigest) {}

  /** Request passed to a linked local Solana SCCP prover. */
  public record ProofRequest(
      int version,
      String backend,
      int sourceDomain,
      int targetDomain,
      String mainnetGenesisHash,
      String witnessHash,
      PublicInputs publicInputs,
      Witness witness) {}

  /** Proof envelope returned after local Solana SCCP proof generation. */
  public record ProofResult(
      int version,
      String backend,
      byte[] proofBytes,
      String proofBase64,
      PublicInputs publicInputs,
      String witnessHash,
      String envelopeHash) {
    public ProofResult {
      proofBytes = Arrays.copyOf(proofBytes, proofBytes.length);
    }

    @Override
    public byte[] proofBytes() {
      return Arrays.copyOf(proofBytes, proofBytes.length);
    }
  }
}
