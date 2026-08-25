package org.hyperledger.iroha.android.norito;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.client.MultisigProposeRequest;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.TransactionAdmissionIntent;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.util.HashLiteral;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/**
 * Norito codec adapter that delegates to the shared JVM Norito implementation bundled with the
 * workspace. This ensures Android tooling stays in lockstep with the canonical Rust codecs and
 * schema hashes.
 */
public final class NoritoJavaCodecAdapter implements NoritoCodecAdapter {

  private static final String DEFAULT_SCHEMA = "iroha.android.transaction.Payload.v1";

  private final int chainDiscriminant;
  private final String schemaName;
  private final TypeAdapter<TransactionPayload> adapter;

  public NoritoJavaCodecAdapter(final int chainDiscriminant) {
    this(chainDiscriminant, DEFAULT_SCHEMA);
  }

  public NoritoJavaCodecAdapter(final int chainDiscriminant, final String schemaName) {
    if (chainDiscriminant < 0 || chainDiscriminant > 0xffff) {
      throw new IllegalArgumentException("chainDiscriminant must fit in u16");
    }
    this.chainDiscriminant = chainDiscriminant;
    this.schemaName = schemaName;
    this.adapter = TransactionPayloadAdapter.forChain(chainDiscriminant);
  }

  @Override
  public byte[] encodeTransaction(final TransactionPayload payload) throws NoritoException {
    try {
      return NoritoCodec.encodeAdaptive(payload, adapter).payload();
    } catch (final Exception ex) {
      throw new NoritoException("Failed to encode Norito transaction payload", ex);
    }
  }

  @Override
  public TransactionPayload decodeTransaction(final byte[] encoded) throws NoritoException {
    try {
      if (hasHeader(encoded)) {
        return NoritoCodec.decode(encoded, adapter, schemaName);
      }
      return NoritoCodec.decodeAdaptive(encoded, adapter);
    } catch (final Exception ex) {
      throw new NoritoException("Failed to decode Norito transaction payload", ex);
    }
  }

  /** Returns the target chain's required I105 discriminant. */
  public int chainDiscriminant() {
    return chainDiscriminant;
  }

  @Override
  public String schemaName() {
    return schemaName;
  }

  public static byte[] encodeInstructionBox(final InstructionBox instruction) throws NoritoException {
    try {
      return TransactionPayloadAdapter.encodeInstructionBox(instruction);
    } catch (final Exception ex) {
      throw new NoritoException("Failed to encode Norito instruction box", ex);
    }
  }

  /**
   * Returns Rust-compatible {@code HashOf<Vec<InstructionBox>>} bytes for the exact canonical
   * instruction boxes supplied to a multisig proposal.
   *
   * <p>The vector is encoded as a bare default-flags Norito value (COMPACT_LEN), then hashed with
   * Iroha's marked BLAKE2b-256 prehash. Individual elements must be canonical wire-framed
   * {@link InstructionBox} values.
   */
  public static byte[] hashCanonicalInstructionBoxes(final List<byte[]> encodedInstructions)
      throws NoritoException {
    byte[] preimage = null;
    try {
      preimage = TransactionPayloadAdapter.encodeCanonicalInstructionBoxes(encodedInstructions);
      return IrohaHash.prehash(preimage);
    } catch (final Exception ex) {
      throw new NoritoException("Failed to hash canonical Norito instruction boxes", ex);
    } finally {
      if (preimage != null) {
        Arrays.fill(preimage, (byte) 0);
      }
    }
  }

  /**
   * Verifies the exact outer action built by {@code POST /v1/multisig/propose}.
   *
   * <p>The transaction must contain exactly one canonical {@code MultisigPropose} custom
   * instruction, optionally followed by the quorum-one {@code MultisigApprove}. The proposal's
   * account and every embedded canonical instruction frame must equal the caller's request. When
   * present, the approval must target the same account and the locally computed instruction-vector
   * hash. The returned bytes are the Rust-compatible proposal/instructions hash and belong to the
   * caller.
   */
  public static byte[] verifyCanonicalMultisigProposeExecutable(
      final TransactionPayload transactionPayload,
      final String expectedMultisigAccountId,
      final List<byte[]> expectedInstructionBoxes)
      throws NoritoException {
    final List<byte[]> expected = snapshotInstructionBoxes(expectedInstructionBoxes);
    byte[] instructionsHash = null;
    try {
      final String canonicalAccount =
          AccountIdLiteral.requireCanonicalI105Address(
              expectedMultisigAccountId, "expectedMultisigAccountId");
      if (transactionPayload == null
          || !transactionPayload.executable().isInstructions()) {
        throw new IllegalArgumentException(
            "multisig proposal transaction must contain an instruction executable");
      }
      final List<InstructionBox> outer = transactionPayload.executable().instructions();
      if (outer.size() < 1 || outer.size() > 2) {
        throw new IllegalArgumentException(
            "multisig proposal transaction must contain only propose and optional approve");
      }

      instructionsHash = hashCanonicalInstructionBoxes(expected);
      verifyMultisigProposeJson(
          TransactionPayloadAdapter.decodeCanonicalCustomInstructionJson(outer.get(0)),
          canonicalAccount,
          expected);
      if (outer.size() == 2) {
        verifyMultisigApproveJson(
            TransactionPayloadAdapter.decodeCanonicalCustomInstructionJson(outer.get(1)),
            canonicalAccount,
            HashLiteral.canonicalize(instructionsHash));
      }
      final byte[] result = instructionsHash;
      instructionsHash = null;
      return result;
    } catch (final Exception ex) {
      throw new NoritoException("Invalid canonical multisig propose executable", ex);
    } finally {
      if (instructionsHash != null) {
        Arrays.fill(instructionsHash, (byte) 0);
      }
      for (final byte[] instruction : expected) {
        Arrays.fill(instruction, (byte) 0);
      }
    }
  }

  private static List<byte[]> snapshotInstructionBoxes(final List<byte[]> encodedInstructions)
      throws NoritoException {
    if (encodedInstructions == null || encodedInstructions.isEmpty()) {
      throw new NoritoException("Expected multisig instruction boxes must not be empty");
    }
    final List<byte[]> snapshot = new ArrayList<>(encodedInstructions.size());
    for (int index = 0; index < encodedInstructions.size(); index++) {
      final byte[] instruction = encodedInstructions.get(index);
      if (instruction == null || instruction.length == 0) {
        for (final byte[] retained : snapshot) {
          Arrays.fill(retained, (byte) 0);
        }
        throw new NoritoException(
            "Expected multisig instruction box " + index + " must not be empty");
      }
      snapshot.add(instruction.clone());
    }
    return snapshot;
  }

  private static void verifyMultisigProposeJson(
      final String json,
      final String expectedAccount,
      final List<byte[]> expectedInstructions) {
    final Map<String, Object> root = requireJsonObject(JsonParser.parse(json), "multisig custom");
    requireExactKeys(root, Set.of("Propose"), "multisig custom");
    final Map<String, Object> propose =
        requireJsonObject(root.get("Propose"), "multisig custom.Propose");
    requireExactKeys(
        propose,
        Set.of("account", "instructions", "transaction_ttl_ms"),
        "multisig custom.Propose");
    requireExactString(propose.get("account"), expectedAccount, "multisig custom.Propose.account");
    if (propose.get("transaction_ttl_ms") != null) {
      throw new IllegalArgumentException(
          "multisig custom.Propose.transaction_ttl_ms must be null");
    }
    final Object instructionValue = propose.get("instructions");
    if (!(instructionValue instanceof List<?>)) {
      throw new IllegalArgumentException("multisig custom.Propose.instructions must be an array");
    }
    final List<?> embedded = (List<?>) instructionValue;
    if (embedded.size() != expectedInstructions.size()) {
      throw new IllegalArgumentException(
          "multisig custom.Propose.instructions changed instruction count");
    }
    for (int index = 0; index < embedded.size(); index++) {
      final String expectedBase64 =
          Base64.getEncoder().encodeToString(expectedInstructions.get(index));
      requireExactString(
          embedded.get(index),
          expectedBase64,
          "multisig custom.Propose.instructions[" + index + "]");
    }
  }

  private static void verifyMultisigApproveJson(
      final String json, final String expectedAccount, final String expectedHashLiteral) {
    final Map<String, Object> root = requireJsonObject(JsonParser.parse(json), "multisig custom");
    requireExactKeys(root, Set.of("Approve"), "multisig custom");
    final Map<String, Object> approve =
        requireJsonObject(root.get("Approve"), "multisig custom.Approve");
    requireExactKeys(
        approve, Set.of("account", "instructions_hash"), "multisig custom.Approve");
    requireExactString(approve.get("account"), expectedAccount, "multisig custom.Approve.account");
    requireExactString(
        approve.get("instructions_hash"),
        expectedHashLiteral,
        "multisig custom.Approve.instructions_hash");
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> requireJsonObject(final Object value, final String field) {
    if (!(value instanceof Map<?, ?>)) {
      throw new IllegalArgumentException(field + " must be an object");
    }
    final Map<?, ?> candidate = (Map<?, ?>) value;
    for (final Object key : candidate.keySet()) {
      if (!(key instanceof String)) {
        throw new IllegalArgumentException(field + " keys must be strings");
      }
    }
    return (Map<String, Object>) candidate;
  }

  private static void requireExactKeys(
      final Map<String, Object> value, final Set<String> expected, final String field) {
    if (!value.keySet().equals(expected)) {
      throw new IllegalArgumentException(field + " has unexpected fields");
    }
  }

  private static void requireExactString(
      final Object value, final String expected, final String field) {
    if (!(value instanceof String) || !expected.equals(value)) {
      throw new IllegalArgumentException(field + " changed");
    }
  }

  /** Rejects transaction payload bytes that are not the exact canonical Norito encoding. */
  public static void validateCanonicalTransactionPayload(final byte[] encoded)
      throws NoritoException {
    decodeCanonicalTransactionPayload(encoded, null);
  }

  /** Rejects non-canonical payloads and payloads with a different admission intent. */
  public static void validateCanonicalTransactionPayload(
      final byte[] encoded, final TransactionAdmissionIntent expectedAdmissionIntent)
      throws NoritoException {
    decodeCanonicalTransactionPayload(encoded, expectedAdmissionIntent);
  }

  /** Decodes one exact canonical payload so callers can verify signature-bound fields. */
  public static TransactionPayload decodeCanonicalTransactionPayload(final byte[] encoded)
      throws NoritoException {
    return decodeCanonicalTransactionPayload(encoded, null);
  }

  /** Decodes one exact canonical payload and enforces its admission intent when supplied. */
  public static TransactionPayload decodeCanonicalTransactionPayload(
      final byte[] encoded, final TransactionAdmissionIntent expectedAdmissionIntent)
      throws NoritoException {
    try {
      final TransactionPayload payload =
          TransactionPayloadAdapter.validateCanonicalPayloadBytes(encoded);
      if (expectedAdmissionIntent != null && payload.admissionIntent() != expectedAdmissionIntent) {
        throw new IllegalArgumentException(
            "transaction payload admission intent must be " + expectedAdmissionIntent);
      }
      return payload;
    } catch (final Exception ex) {
      throw new NoritoException("Invalid canonical Norito transaction payload", ex);
    }
  }

  public static byte[] encodeMultisigProposeRequest(
      final MultisigProposeRequest request, final int chainDiscriminant) throws NoritoException {
    try {
      return TransactionPayloadAdapter.encodeMultisigProposeRequest(
          request, chainDiscriminant);
    } catch (final Exception ex) {
      throw new NoritoException("Failed to encode Norito multisig propose request", ex);
    }
  }

  private static boolean hasHeader(final byte[] encoded) {
    if (encoded == null || encoded.length < NoritoHeader.HEADER_LENGTH) {
      return false;
    }
    return encoded[0] == 'N'
        && encoded[1] == 'R'
        && encoded[2] == 'T'
        && encoded[3] == '0';
  }
}
