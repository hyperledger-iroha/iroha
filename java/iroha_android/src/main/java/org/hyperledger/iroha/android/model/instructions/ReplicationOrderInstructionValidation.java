package org.hyperledger.iroha.android.model.instructions;

import java.util.Base64;
import java.util.LinkedHashSet;
import java.util.Map;
import java.util.Objects;
import java.util.Set;

/** Shared canonical-input checks for SoraFS replication-order instructions. */
final class ReplicationOrderInstructionValidation {

  private static final int ORDER_ID_BYTES = 32;
  private static final int MAX_ORDER_PAYLOAD_BYTES = 1024 * 1024;
  private static final char[] HEX = "0123456789abcdef".toCharArray();

  private ReplicationOrderInstructionValidation() {}

  static String requireOrderId(final String value) {
    Objects.requireNonNull(value, "orderIdHex");
    if (!value.matches("[0-9a-f]{64}")) {
      throw new IllegalArgumentException(
          "orderIdHex must contain exactly 64 lowercase hexadecimal characters");
    }
    boolean nonzero = false;
    for (int i = 0; i < value.length(); i++) {
      nonzero |= value.charAt(i) != '0';
    }
    if (!nonzero) {
      throw new IllegalArgumentException("orderIdHex must not be the zero identifier");
    }
    return value;
  }

  static String encodeOrderId(final byte[] value) {
    Objects.requireNonNull(value, "orderId");
    if (value.length != ORDER_ID_BYTES) {
      throw new IllegalArgumentException(
          "orderId must contain exactly "
              + ORDER_ID_BYTES
              + " bytes, found "
              + value.length);
    }
    boolean nonzero = false;
    final char[] encoded = new char[value.length * 2];
    for (int i = 0; i < value.length; i++) {
      final int unsigned = value[i] & 0xff;
      nonzero |= unsigned != 0;
      encoded[i * 2] = HEX[unsigned >>> 4];
      encoded[i * 2 + 1] = HEX[unsigned & 0x0f];
    }
    if (!nonzero) {
      throw new IllegalArgumentException("orderId must not be the zero identifier");
    }
    return new String(encoded);
  }

  static String requireCanonicalPayload(final String value) {
    Objects.requireNonNull(value, "orderPayloadBase64");
    if (value.isEmpty()) {
      throw new IllegalArgumentException("orderPayloadBase64 must not be empty");
    }
    final byte[] decoded;
    try {
      decoded = Base64.getDecoder().decode(value);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException("orderPayloadBase64 must be canonical base64", ex);
    }
    if (decoded.length == 0) {
      throw new IllegalArgumentException("orderPayloadBase64 must decode to non-empty bytes");
    }
    if (decoded.length > MAX_ORDER_PAYLOAD_BYTES) {
      throw new IllegalArgumentException(
          "orderPayloadBase64 decodes to "
              + decoded.length
              + " bytes; maximum is "
              + MAX_ORDER_PAYLOAD_BYTES);
    }
    if (!Base64.getEncoder().encodeToString(decoded).equals(value)) {
      throw new IllegalArgumentException("orderPayloadBase64 must use canonical base64");
    }
    return value;
  }

  static long requireEpoch(final long value, final String fieldName) {
    if (value < 0) {
      throw new IllegalArgumentException(fieldName + " must be non-negative");
    }
    return value;
  }

  static void requireWindow(final long issuedEpoch, final long deadlineEpoch) {
    if (deadlineEpoch <= issuedEpoch) {
      throw new IllegalArgumentException("deadlineEpoch must be greater than issuedEpoch");
    }
  }

  static void requireArguments(
      final Map<String, String> arguments, final String action, final String... fields) {
    Objects.requireNonNull(arguments, "arguments");
    final Set<String> expected = new LinkedHashSet<>();
    expected.add("action");
    for (final String field : fields) {
      expected.add(field);
    }
    if (!arguments.keySet().equals(expected)) {
      throw new IllegalArgumentException(
          "Instruction arguments must contain exactly " + expected);
    }
    if (!action.equals(arguments.get("action"))) {
      throw new IllegalArgumentException("Instruction argument 'action' must be '" + action + "'");
    }
  }
}
