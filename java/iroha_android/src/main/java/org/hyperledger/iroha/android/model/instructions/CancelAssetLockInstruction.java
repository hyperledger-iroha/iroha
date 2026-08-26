package org.hyperledger.iroha.android.model.instructions;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.regex.Pattern;
import org.hyperledger.iroha.android.crypto.Blake2b;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.numeric.NumericV1;
import org.hyperledger.iroha.android.util.HashLiteral;

/**
 * Strict typed representation of the native V1 {@code CancelAssetLock} instruction.
 *
 * <p>The builder accepts the exact application lock identifier and derives the native escrow hash
 * with Blake2b-256. Use {@link #fromEscrowId(String, String)} only when rebuilding an already
 * committed canonical instruction.
 */
public final class CancelAssetLockInstruction implements InstructionTemplate {

  /** Canonical native instruction wire identifier. */
  public static final String WIRE_NAME =
      "iroha.instruction.v1::escrow::CancelAssetLock";

  /** Concrete Norito schema path used only for the typed payload. */
  static final String SCHEMA_NAME = "iroha_data_model::isi::escrow::CancelAssetLock";

  /** Maximum UTF-8 bytes accepted for the lock-id preimage in V1. */
  public static final int MAX_LOCK_ID_UTF8_BYTES_V1 = 4_096;

  private static final String ESCROW_ID = "escrow_id";
  private static final String EXPECTED_REMAINING_AMOUNT = "expected_remaining_amount";
  private static final Set<String> CANONICAL_FIELDS =
      Collections.unmodifiableSet(
          new HashSet<>(Arrays.asList(ESCROW_ID, EXPECTED_REMAINING_AMOUNT)));
  private static final Pattern CANONICAL_HASH_LITERAL =
      Pattern.compile("^hash:[0-9A-F]{64}#[0-9A-F]{4}$");

  private final String escrowId;
  private final NumericV1.QuantityValue expectedRemainingAmount;
  private final Map<String, String> arguments;

  private CancelAssetLockInstruction(
      final String escrowId, final NumericV1.QuantityValue expectedRemainingAmount) {
    this.escrowId = requireCanonicalEscrowId(escrowId);
    this.expectedRemainingAmount = requirePositiveQuantity(expectedRemainingAmount);
    final Map<String, String> canonical = new LinkedHashMap<>();
    canonical.put(ESCROW_ID, this.escrowId);
    canonical.put(EXPECTED_REMAINING_AMOUNT, this.expectedRemainingAmount.toString());
    this.arguments = Collections.unmodifiableMap(canonical);
  }

  /** Return the canonical native escrow hash literal. */
  public String escrowId() {
    return escrowId;
  }

  /** Return the exact positive remaining-amount precondition. */
  public NumericV1.QuantityValue expectedRemainingAmount() {
    return expectedRemainingAmount;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  /**
   * Return a wire-framed instruction box.
   *
   * <p>This deliberately bypasses the local custom argument-map representation: native
   * {@code CancelAssetLock} is submitted only under its registered Norito wire identifier.
   */
  @Override
  public InstructionBox toInstructionBox() {
    return CancelAssetLockWirePayloadEncoder.encode(this);
  }

  /**
   * Start a strict compare-and-cancel builder.
   *
   * <p>The lock identifier must be well-formed UTF-16 so its UTF-8 hash preimage cannot depend on
   * replacement-character behavior.
   */
  public static Builder builder() {
    return new Builder();
  }

  /**
   * Rebuild from the exact native JSON field surface.
   *
   * <p>Missing fields, legacy aliases, and unknown fields are all rejected.
   */
  public static CancelAssetLockInstruction fromCanonicalFields(
      final Map<String, String> fields) {
    Objects.requireNonNull(fields, "fields");
    if (!fields.keySet().equals(CANONICAL_FIELDS)) {
      throw new IllegalArgumentException(
          "CancelAssetLock must contain exactly escrow_id and expected_remaining_amount");
    }
    if (fields.get(ESCROW_ID) == null
        || fields.get(EXPECTED_REMAINING_AMOUNT) == null) {
      throw new IllegalArgumentException(
          "CancelAssetLock canonical fields must not be null");
    }
    return fromEscrowId(
        fields.get(ESCROW_ID), fields.get(EXPECTED_REMAINING_AMOUNT));
  }

  /** Rebuild from an exact canonical native escrow hash literal. */
  public static CancelAssetLockInstruction fromEscrowId(
      final String escrowId, final String expectedRemainingAmount) {
    return new CancelAssetLockInstruction(
        escrowId, requirePositiveQuantity(expectedRemainingAmount));
  }

  /** Rebuild from an exact canonical native escrow id and lossless quantity. */
  public static CancelAssetLockInstruction fromEscrowId(
      final String escrowId, final NumericV1.QuantityValue expectedRemainingAmount) {
    return new CancelAssetLockInstruction(escrowId, expectedRemainingAmount);
  }

  /** Decode a canonical native Norito payload and reject legacy or trailing layouts. */
  public static CancelAssetLockInstruction fromWirePayload(final byte[] payload) {
    return CancelAssetLockWirePayloadEncoder.decodePayload(payload);
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof CancelAssetLockInstruction)) {
      return false;
    }
    final CancelAssetLockInstruction other = (CancelAssetLockInstruction) obj;
    return escrowId.equals(other.escrowId)
        && expectedRemainingAmount.equals(other.expectedRemainingAmount);
  }

  @Override
  public int hashCode() {
    return Objects.hash(escrowId, expectedRemainingAmount);
  }

  /** Strict builder accepting only the ergonomic lock id and exact remaining amount. */
  public static final class Builder {
    private String lockId;
    private NumericV1.QuantityValue expectedRemainingAmount;

    private Builder() {}

    /** Set the exact application lock identifier used to derive the native escrow hash. */
    public Builder setLockId(final String lockId) {
      this.lockId = requireExactLockId(lockId);
      return this;
    }

    /** Set the positive canonical quantity spelling observed in finalized ledger state. */
    public Builder setExpectedRemainingAmount(final String value) {
      this.expectedRemainingAmount = requirePositiveQuantity(value);
      return this;
    }

    /** Set an already validated lossless positive quantity. */
    public Builder setExpectedRemainingAmount(final NumericV1.QuantityValue value) {
      this.expectedRemainingAmount = requirePositiveQuantity(value);
      return this;
    }

    /** Build the canonical two-field native instruction. */
    public CancelAssetLockInstruction build() {
      if (lockId == null) {
        throw new IllegalArgumentException("lockId is required");
      }
      if (expectedRemainingAmount == null) {
        throw new IllegalArgumentException("expectedRemainingAmount is required");
      }
      return new CancelAssetLockInstruction(
          deriveEscrowId(lockId), expectedRemainingAmount);
    }
  }

  private static String deriveEscrowId(final String value) {
    final String lockId = requireExactLockId(value);
    return HashLiteral.canonicalize(
        Blake2b.digest256(lockId.getBytes(StandardCharsets.UTF_8)));
  }

  private static String requireExactLockId(final String value) {
    if (value == null || value.isEmpty() || isAllAssetLockWhitespace(value)) {
      throw new IllegalArgumentException("lockId must be an exact non-empty string");
    }
    if (isAssetLockWhitespace(value.charAt(0))
        || isAssetLockWhitespace(value.charAt(value.length() - 1))) {
      throw new IllegalArgumentException("lockId must not contain surrounding whitespace");
    }
    requireWellFormedUtf16(value);
    if (value.getBytes(StandardCharsets.UTF_8).length > MAX_LOCK_ID_UTF8_BYTES_V1) {
      throw new IllegalArgumentException(
          "lockId must be at most " + MAX_LOCK_ID_UTF8_BYTES_V1 + " UTF-8 bytes");
    }
    return value;
  }

  private static String requireCanonicalEscrowId(final String value) {
    if (value == null || !CANONICAL_HASH_LITERAL.matcher(value).matches()) {
      throw new IllegalArgumentException(
          "escrow_id must be a canonical uppercase native hash literal");
    }
    final byte[] bytes = HashLiteral.decode(value);
    if ((bytes[bytes.length - 1] & 1) == 0) {
      throw new IllegalArgumentException(
          "escrow_id must use a native hash with its marker bit set");
    }
    if (!HashLiteral.canonicalize(bytes).equals(value)) {
      throw new IllegalArgumentException(
          "escrow_id must be a canonical uppercase native hash literal");
    }
    return value;
  }

  private static NumericV1.QuantityValue requirePositiveQuantity(final String value) {
    if (value == null) {
      throw new IllegalArgumentException(
          "expected_remaining_amount must be a positive canonical Quantity");
    }
    try {
      return requirePositiveQuantity(NumericV1.QuantityValue.parseCanonical(value));
    } catch (final IllegalArgumentException error) {
      throw new IllegalArgumentException(
          "expected_remaining_amount must be a positive canonical Quantity", error);
    }
  }

  private static NumericV1.QuantityValue requirePositiveQuantity(
      final NumericV1.QuantityValue value) {
    final NumericV1.QuantityValue nonNull =
        Objects.requireNonNull(value, "expectedRemainingAmount");
    if (nonNull.mantissa().signum() <= 0) {
      throw new IllegalArgumentException(
          "expected_remaining_amount must be greater than zero");
    }
    return nonNull;
  }

  private static boolean isAllAssetLockWhitespace(final String value) {
    for (int index = 0; index < value.length(); index++) {
      if (!isAssetLockWhitespace(value.charAt(index))) {
        return false;
      }
    }
    return true;
  }

  private static boolean isAssetLockWhitespace(final char value) {
    return Character.isWhitespace(value)
        || Character.isSpaceChar(value)
        || value == '\uFEFF';
  }

  private static void requireWellFormedUtf16(final String value) {
    for (int index = 0; index < value.length(); index++) {
      final char current = value.charAt(index);
      if (Character.isHighSurrogate(current)) {
        if (index + 1 >= value.length()
            || !Character.isLowSurrogate(value.charAt(index + 1))) {
          throw new IllegalArgumentException(
              "lockId must not contain unpaired UTF-16 surrogates");
        }
        index++;
      } else if (Character.isLowSurrogate(current)) {
        throw new IllegalArgumentException(
            "lockId must not contain unpaired UTF-16 surrogates");
      }
    }
  }
}
