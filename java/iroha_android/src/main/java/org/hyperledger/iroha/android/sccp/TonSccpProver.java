package org.hyperledger.iroha.android.sccp;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.List;
import java.util.Objects;
import org.hyperledger.iroha.android.crypto.Blake2b;

/** TON SCCP proof request and internal-message helpers for local-first Android proof generation. */
public final class TonSccpProver {
  public static final int DOMAIN_TON = 4;
  public static final String CONTRACT_PROOF_BACKEND_V1 = "ton-contract-v1";
  public static final String MESSAGE_BODY_BOC_V1 = "ton_message_body_boc_v1";

  private static final long SUBMIT_OP_V1 = 0x53434350L;
  private static final int MESSAGE_SCHEMA_VERSION_V1 = 1;
  private static final int MAX_CELL_DATA_BYTES = 127;
  private static final int MAX_REFS = 4;
  private static final byte[] BOC_MAGIC = {
    (byte) 0xb5, (byte) 0xee, (byte) 0x9c, 0x72
  };
  private static final BigInteger MAX_U64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);

  private final WitnessProvider witnessProvider;
  private final ProofEngine proofEngine;

  public TonSccpProver() {
    this(null, null);
  }

  public TonSccpProver(final WitnessProvider witnessProvider, final ProofEngine proofEngine) {
    this.witnessProvider = witnessProvider;
    this.proofEngine = proofEngine;
  }

  public ProofRequest buildRequest(final ProofRequestInput input) {
    final ProofRequestInput resolved =
        witnessProvider == null ? input : witnessProvider.resolveWitness(input);
    return buildProofRequest(resolved);
  }

  public ProofResult prove(final ProofRequestInput input) {
    final ProofRequest request = buildRequest(input);
    if (proofEngine == null) {
      throw new IllegalStateException("TON SCCP local prover is not linked");
    }
    return wrapProofResult(proofEngine.prove(request), request);
  }

  public static byte[] canonicalPublicInputsBytes(final PublicInputsInput input) {
    Objects.requireNonNull(input, "input");
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(input.version());
    write(out, hex32Bytes(input.messageId(), "messageId"));
    write(out, hex32Bytes(input.payloadHash(), "payloadHash"));
    writeU32Le(out, input.targetDomain());
    write(out, hex32Bytes(input.commitmentRoot(), "commitmentRoot"));
    writeU64Le(out, normalizeU64(input.finalityHeight(), "finalityHeight"));
    write(out, hex32Bytes(input.finalityBlockHash(), "finalityBlockHash"));
    return out.toByteArray();
  }

  public static String submissionQueryId(final PublicInputsInput publicInputs) {
    final byte[] messageId = hex32Bytes(publicInputs.messageId(), "messageId");
    BigInteger value = BigInteger.ZERO;
    for (int i = 0; i < 8; i++) {
      value = value.shiftLeft(8).or(BigInteger.valueOf(messageId[i] & 0xffL));
    }
    return value.toString();
  }

  public static byte[] buildMessageBodyBoc(final MessageBodyInput input) {
    Objects.requireNonNull(input, "input");
    final byte[] publicInputsBytes = canonicalPublicInputsBytes(input.publicInputs());
    final byte[] statementHash = hex32Bytes(input.statementHash(), "statementHash");
    final byte[] destinationBindingHash =
        hex32Bytes(input.destinationBindingHash(), "destinationBindingHash");
    final ByteArrayOutputStream rootData = new ByteArrayOutputStream();
    writeU32Be(rootData, SUBMIT_OP_V1);
    writeU64Be(
        rootData,
        normalizeU64(
            input.queryId() == null ? submissionQueryId(input.publicInputs()) : input.queryId(),
            "queryId"));
    writeU16Be(rootData, MESSAGE_SCHEMA_VERSION_V1);
    write(rootData, statementHash);
    write(rootData, destinationBindingHash);

    final List<TonCell> cells = new ArrayList<>();
    cells.add(new TonCell(rootData.toByteArray(), new ArrayList<>()));
    final int publicInputsRoot = pushSnakeCells(cells, publicInputsBytes);
    final int proofRoot = pushSnakeCells(cells, input.proofBytes());
    final int bundleRoot = pushSnakeCells(cells, input.bundleBytes());
    final int metadataRoot = pushSnakeCells(cells, input.metadataBytes());
    cells.get(0).refs.add(publicInputsRoot);
    cells.get(0).refs.add(proofRoot);
    cells.get(0).refs.add(bundleRoot);
    cells.get(0).refs.add(metadataRoot);
    return encodeBocSingleRoot(cells, 0);
  }

  public static Submission buildSubmission(final MessageBodyInput input) {
    final byte[] messageBodyBoc = buildMessageBodyBoc(input);
    return new Submission(
        MESSAGE_BODY_BOC_V1, Arrays.copyOf(messageBodyBoc, messageBodyBoc.length), "0x" + hexLower(messageBodyBoc));
  }

  public static ProofRequest buildProofRequest(final ProofRequestInput input) {
    Objects.requireNonNull(input, "input");
    final byte[] publicInputsBytes = canonicalPublicInputsBytes(input.publicInputs());
    final ByteArrayOutputStream preimage = new ByteArrayOutputStream();
    write(preimage, publicInputsBytes);
    write(preimage, input.bundleBytes());
    write(preimage, input.sourceProofBytes());
    return new ProofRequest(
        1,
        input.backend(),
        input.sourceDomain(),
        input.publicInputs().targetDomain(),
        input.publicInputs(),
        publicInputsBytes,
        Arrays.copyOf(input.bundleBytes(), input.bundleBytes().length),
        Arrays.copyOf(input.sourceProofBytes(), input.sourceProofBytes().length),
        hashHex("sccp:ton:proof-request:v1", preimage.toByteArray()));
  }

  public static ProofResult wrapProofResult(final byte[] proofBytes, final ProofRequest request) {
    Objects.requireNonNull(request, "request");
    if (proofBytes == null || proofBytes.length == 0) {
      throw new IllegalArgumentException("proofBytes must not be empty");
    }
    return new ProofResult(
        1,
        request.backend(),
        Arrays.copyOf(proofBytes, proofBytes.length),
        Base64.getEncoder().encodeToString(proofBytes),
        request.publicInputs(),
        request.requestHash());
  }

  private static int pushSnakeCells(final List<TonCell> cells, final byte[] bytes) {
    final int start = cells.size();
    final byte[] data = bytes == null ? new byte[0] : bytes;
    if (data.length == 0) {
      cells.add(new TonCell(new byte[0], new ArrayList<>()));
      return start;
    }
    final int chunkCount = (data.length + MAX_CELL_DATA_BYTES - 1) / MAX_CELL_DATA_BYTES;
    for (int index = 0; index < chunkCount; index++) {
      final int chunkStart = index * MAX_CELL_DATA_BYTES;
      final int chunkEnd = Math.min(chunkStart + MAX_CELL_DATA_BYTES, data.length);
      final byte[] chunk = Arrays.copyOfRange(data, chunkStart, chunkEnd);
      final ArrayList<Integer> refs = new ArrayList<>();
      if (index + 1 != chunkCount) {
        refs.add(start + index + 1);
      }
      cells.add(new TonCell(chunk, refs));
    }
    return start;
  }

  private static byte[] encodeBocSingleRoot(final List<TonCell> cells, final int rootIndex) {
    if (cells.isEmpty() || rootIndex < 0 || rootIndex >= cells.size()) {
      throw new IllegalArgumentException("invalid TON BOC root");
    }
    final int sizeBytes = minSizeBytes(Math.max(cells.size(), rootIndex));
    final byte[] cellsBytes = serializeCells(cells, sizeBytes);
    final int offsetBytes = minSizeBytes(cellsBytes.length);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    write(out, BOC_MAGIC);
    out.write(sizeBytes);
    out.write(offsetBytes);
    write(out, sizedUInt(cells.size(), sizeBytes));
    write(out, sizedUInt(1, sizeBytes));
    write(out, sizedUInt(0, sizeBytes));
    write(out, sizedUInt(cellsBytes.length, offsetBytes));
    write(out, sizedUInt(rootIndex, sizeBytes));
    write(out, cellsBytes);
    return out.toByteArray();
  }

  private static byte[] serializeCells(final List<TonCell> cells, final int sizeBytes) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    for (int index = 0; index < cells.size(); index++) {
      final TonCell cell = cells.get(index);
      if (cell.data.length > MAX_CELL_DATA_BYTES || cell.refs.size() > MAX_REFS) {
        throw new IllegalArgumentException("invalid TON cell at index " + index);
      }
      out.write(cell.refs.size());
      out.write(cell.data.length * 2);
      write(out, cell.data);
      for (final int ref : cell.refs) {
        if (ref < 0 || ref >= cells.size()) {
          throw new IllegalArgumentException("invalid TON cell ref");
        }
        write(out, sizedUInt(ref, sizeBytes));
      }
    }
    return out.toByteArray();
  }

  private static int minSizeBytes(final int value) {
    final BigInteger numeric = BigInteger.valueOf(value);
    for (int size = 1; size <= 7; size++) {
      if (numeric.compareTo(BigInteger.ONE.shiftLeft(size * 8).subtract(BigInteger.ONE)) <= 0) {
        return size;
      }
    }
    throw new IllegalArgumentException("TON sized integer is too large");
  }

  private static byte[] sizedUInt(final int value, final int size) {
    BigInteger working = BigInteger.valueOf(value);
    final byte[] out = new byte[size];
    for (int index = size - 1; index >= 0; index--) {
      out[index] = working.and(BigInteger.valueOf(0xffL)).byteValue();
      working = working.shiftRight(8);
    }
    if (!BigInteger.ZERO.equals(working)) {
      throw new IllegalArgumentException("TON sized integer overflows");
    }
    return out;
  }

  private static String hashHex(final String prefix, final byte[] payload) {
    final byte[] prefixBytes = prefix.getBytes(java.nio.charset.StandardCharsets.UTF_8);
    final byte[] preimage = new byte[prefixBytes.length + payload.length];
    System.arraycopy(prefixBytes, 0, preimage, 0, prefixBytes.length);
    System.arraycopy(payload, 0, preimage, prefixBytes.length, payload.length);
    return "0x" + hexLower(Blake2b.digest256(preimage));
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

  private static void writeU16Be(final ByteArrayOutputStream out, final int value) {
    out.write((value >>> 8) & 0xff);
    out.write(value & 0xff);
  }

  private static void writeU32Le(final ByteArrayOutputStream out, final int value) {
    out.write(value & 0xff);
    out.write((value >>> 8) & 0xff);
    out.write((value >>> 16) & 0xff);
    out.write((value >>> 24) & 0xff);
  }

  private static void writeU32Be(final ByteArrayOutputStream out, final long value) {
    out.write((int) ((value >>> 24) & 0xff));
    out.write((int) ((value >>> 16) & 0xff));
    out.write((int) ((value >>> 8) & 0xff));
    out.write((int) (value & 0xff));
  }

  private static void writeU64Le(final ByteArrayOutputStream out, final BigInteger value) {
    BigInteger working = value;
    for (int i = 0; i < 8; i++) {
      out.write(working.and(BigInteger.valueOf(0xffL)).intValue());
      working = working.shiftRight(8);
    }
  }

  private static void writeU64Be(final ByteArrayOutputStream out, final BigInteger value) {
    BigInteger working = value;
    final byte[] bytes = new byte[8];
    for (int index = 7; index >= 0; index--) {
      bytes[index] = working.and(BigInteger.valueOf(0xffL)).byteValue();
      working = working.shiftRight(8);
    }
    write(out, bytes);
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

  private static final class TonCell {
    private final byte[] data;
    private final ArrayList<Integer> refs;

    private TonCell(final byte[] data, final ArrayList<Integer> refs) {
      this.data = data;
      this.refs = refs;
    }
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
      this(1, messageId, payloadHash, DOMAIN_TON, commitmentRoot, finalityHeight, finalityBlockHash);
    }
  }

  public record MessageBodyInput(
      PublicInputsInput publicInputs,
      byte[] proofBytes,
      byte[] bundleBytes,
      String statementHash,
      String destinationBindingHash,
      byte[] metadataBytes,
      String queryId) {
    public MessageBodyInput(
        final PublicInputsInput publicInputs,
        final byte[] proofBytes,
        final byte[] bundleBytes,
        final String statementHash,
        final String destinationBindingHash,
        final byte[] metadataBytes) {
      this(publicInputs, proofBytes, bundleBytes, statementHash, destinationBindingHash, metadataBytes, null);
    }
  }

  public record Submission(String envelopeEncoding, byte[] messageBodyBoc, String messageBodyBocHex) {}

  public record ProofRequestInput(
      PublicInputsInput publicInputs,
      byte[] bundleBytes,
      byte[] sourceProofBytes,
      String backend,
      int sourceDomain) {
    public ProofRequestInput(final PublicInputsInput publicInputs, final byte[] bundleBytes) {
      this(publicInputs, bundleBytes, new byte[0], CONTRACT_PROOF_BACKEND_V1, DOMAIN_TON);
    }
  }

  public record ProofRequest(
      int version,
      String backend,
      int sourceDomain,
      int targetDomain,
      PublicInputsInput publicInputs,
      byte[] publicInputsBytes,
      byte[] bundleBytes,
      byte[] sourceProofBytes,
      String requestHash) {}

  public record ProofResult(
      int version,
      String backend,
      byte[] proofBytes,
      String proofBase64,
      PublicInputsInput publicInputs,
      String requestHash) {}
}
