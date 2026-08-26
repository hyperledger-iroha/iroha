package org.hyperledger.iroha.android.offline;

import java.io.IOException;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.hyperledger.iroha.android.test.FixtureGeneratorRunner;

/** Authoritative Rust-generated semantic fixtures for the Java Offline Cash V1 facade. */
final class OfflineCashToriiV1Fixtures {
  private static final Set<String> EXPECTED_NAMES =
      Collections.unmodifiableSet(
          new LinkedHashSet<>(
              Arrays.asList(
                  "network_id",
                  "top_up_operation_id",
                  "top_up_submitted_at_ms",
                  "top_up_request",
                  "top_up_reference",
                  "top_up_pending_status",
                  "top_up_finalized_block_height",
                  "top_up_server_time_ms",
                  "top_up_applied_status",
                  "invalid_top_up_anchor_status",
                  "invalid_top_up_proof_status",
                  "wrong_top_up_operation_status",
                  "wrong_top_up_transaction_status",
                  "wrong_top_up_height_status",
                  "wrong_top_up_proof_network_status",
                  "foreign_network_top_up_status",
                  "wrong_top_up_proof_anchor_status",
                  "wrong_top_up_proof_height_status",
                  "redeem_operation_id",
                  "redeem_submitted_at_ms",
                  "redeem_request",
                  "redeem_reference",
                  "redeem_pending_status",
                  "redeem_applied_status",
                  "rejected_status",
                  "invalid_binding_top_up_request",
                  "wrong_id_reference",
                  "wrong_kind_reference",
                  "wrong_time_reference",
                  "zero_time_reference",
                  "wrong_uri_reference",
                  "invalid_transaction_hash_reference",
                  "wrong_id_status",
                  "zero_submitted_pending_status",
                  "zero_height_status",
                  "zero_time_status",
                  "invalid_transaction_hash_status",
                  "wrong_rejection_code_status",
                  "rejection_details_status",
                  "oversized_rejection_message_status")));
  private static final Set<String> DIGEST_NAMES =
      Collections.unmodifiableSet(
          new LinkedHashSet<>(
              Arrays.asList("network_id", "top_up_operation_id", "redeem_operation_id")));
  private static final Set<String> POSITIVE_DECIMAL_NAMES =
      Collections.unmodifiableSet(
          new LinkedHashSet<>(
              Arrays.asList(
                  "top_up_submitted_at_ms",
                  "top_up_finalized_block_height",
                  "top_up_server_time_ms",
                  "redeem_submitted_at_ms")));
  private static final Map<String, String> VALUES = load();

  private OfflineCashToriiV1Fixtures() {}

  static String networkId() {
    return text("network_id");
  }

  static String topUpOperationId() {
    return text("top_up_operation_id");
  }

  static byte[] topUpRequest() {
    return bytes("top_up_request");
  }

  static byte[] topUpReference() {
    return bytes("top_up_reference");
  }

  static byte[] topUpPendingStatus() {
    return bytes("top_up_pending_status");
  }

  static long topUpFinalizedBlockHeight() {
    return Long.parseLong(text("top_up_finalized_block_height"));
  }

  static long topUpServerTimeMilliseconds() {
    return Long.parseLong(text("top_up_server_time_ms"));
  }

  static byte[] topUpAppliedStatus() {
    return bytes("top_up_applied_status");
  }

  static byte[] invalidTopUpAnchorStatus() {
    return bytes("invalid_top_up_anchor_status");
  }

  static byte[] invalidTopUpProofStatus() {
    return bytes("invalid_top_up_proof_status");
  }

  static byte[] wrongTopUpOperationStatus() {
    return bytes("wrong_top_up_operation_status");
  }

  static byte[] wrongTopUpTransactionStatus() {
    return bytes("wrong_top_up_transaction_status");
  }

  static byte[] wrongTopUpHeightStatus() {
    return bytes("wrong_top_up_height_status");
  }

  static byte[] wrongTopUpProofNetworkStatus() {
    return bytes("wrong_top_up_proof_network_status");
  }

  static byte[] foreignNetworkTopUpStatus() {
    return bytes("foreign_network_top_up_status");
  }

  static byte[] wrongTopUpProofAnchorStatus() {
    return bytes("wrong_top_up_proof_anchor_status");
  }

  static byte[] wrongTopUpProofHeightStatus() {
    return bytes("wrong_top_up_proof_height_status");
  }

  static String redeemOperationId() {
    return text("redeem_operation_id");
  }

  static byte[] redeemRequest() {
    return bytes("redeem_request");
  }

  static byte[] redeemReference() {
    return bytes("redeem_reference");
  }

  static byte[] redeemPendingStatus() {
    return bytes("redeem_pending_status");
  }

  static byte[] redeemAppliedStatus() {
    return bytes("redeem_applied_status");
  }

  static byte[] rejectedStatus() {
    return bytes("rejected_status");
  }

  static byte[] invalidBindingTopUpRequest() {
    return bytes("invalid_binding_top_up_request");
  }

  static byte[] wrongIdReference() {
    return bytes("wrong_id_reference");
  }

  static byte[] wrongKindReference() {
    return bytes("wrong_kind_reference");
  }

  static byte[] wrongTimeReference() {
    return bytes("wrong_time_reference");
  }

  static byte[] zeroTimeReference() {
    return bytes("zero_time_reference");
  }

  static byte[] wrongUriReference() {
    return bytes("wrong_uri_reference");
  }

  static byte[] invalidTransactionHashReference() {
    return bytes("invalid_transaction_hash_reference");
  }

  static byte[] wrongIdStatus() {
    return bytes("wrong_id_status");
  }

  static byte[] zeroSubmittedPendingStatus() {
    return bytes("zero_submitted_pending_status");
  }

  static byte[] zeroHeightStatus() {
    return bytes("zero_height_status");
  }

  static byte[] zeroTimeStatus() {
    return bytes("zero_time_status");
  }

  static byte[] invalidTransactionHashStatus() {
    return bytes("invalid_transaction_hash_status");
  }

  static byte[] wrongRejectionCodeStatus() {
    return bytes("wrong_rejection_code_status");
  }

  static byte[] rejectionDetailsStatus() {
    return bytes("rejection_details_status");
  }

  static byte[] oversizedRejectionMessageStatus() {
    return bytes("oversized_rejection_message_status");
  }

  private static Map<String, String> load() {
    final List<String> rows;
    try {
      rows =
          FixtureGeneratorRunner.run(
              "offline-cash-v1",
              FixtureGeneratorRunner.OFFLINE_CASH_BINARY_ENVIRONMENT_VARIABLE);
    } catch (final IOException | InterruptedException error) {
      if (error instanceof InterruptedException) Thread.currentThread().interrupt();
      throw new ExceptionInInitializerError(error);
    }
    return parseRows(rows);
  }

  static Map<String, String> parseRows(final List<String> rows) {
    final Map<String, String> values = new LinkedHashMap<>();
    for (final String row : rows) {
      final int separator = row.indexOf('=');
      if (separator <= 0 || separator == row.length() - 1) {
        throw new IllegalStateException("invalid offline-cash-v1 fixture row");
      }
      final String name = row.substring(0, separator);
      if (!EXPECTED_NAMES.contains(name)) {
        throw new IllegalStateException("unexpected offline-cash-v1 fixture row " + name);
      }
      final String value = row.substring(separator + 1);
      validateValue(name, value);
      final String previous = values.put(name, value);
      if (previous != null) {
        throw new IllegalStateException("duplicate offline-cash-v1 fixture row " + name);
      }
    }
    if (!values.keySet().equals(EXPECTED_NAMES)) {
      throw new IllegalStateException(
          "offline-cash-v1 fixture rows do not match the exact "
              + EXPECTED_NAMES.size()
              + "-row contract");
    }
    return Collections.unmodifiableMap(values);
  }

  static List<String> canonicalRowsForTest() {
    final java.util.ArrayList<String> rows = new java.util.ArrayList<>(VALUES.size());
    for (final Map.Entry<String, String> entry : VALUES.entrySet()) {
      rows.add(entry.getKey() + "=" + entry.getValue());
    }
    return Collections.unmodifiableList(rows);
  }

  private static void validateValue(final String name, final String value) {
    if (DIGEST_NAMES.contains(name)) {
      if (value.length() != 64 || isAllZero(value) || !isLowerHex(value)) {
        throw new IllegalStateException(
            name + " must be exactly 32 non-zero lowercase hexadecimal bytes");
      }
      if ("network_id".equals(name)
          && (Integer.parseInt(value.substring(value.length() - 2), 16) & 1) != 1) {
        throw new IllegalStateException(
            "network_id must contain a canonical marked Iroha hash");
      }
      return;
    }
    if (POSITIVE_DECIMAL_NAMES.contains(name)) {
      if (!isCanonicalPositiveLong(value)) {
        throw new IllegalStateException(
            name + " must be a canonical positive signed 64-bit decimal");
      }
      return;
    }
    if ((value.length() & 1) != 0 || !isLowerHex(value)) {
      throw new IllegalStateException(
          name + " must be non-empty even-length lowercase hexadecimal");
    }
  }

  private static boolean isAllZero(final String value) {
    for (int index = 0; index < value.length(); index++) {
      if (value.charAt(index) != '0') return false;
    }
    return true;
  }

  private static boolean isLowerHex(final String value) {
    if (value.isEmpty()) return false;
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (!((character >= '0' && character <= '9')
          || (character >= 'a' && character <= 'f'))) return false;
    }
    return true;
  }

  private static boolean isCanonicalPositiveLong(final String value) {
    if (value.isEmpty() || value.charAt(0) < '1' || value.charAt(0) > '9') return false;
    for (int index = 1; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (character < '0' || character > '9') return false;
    }
    try {
      return Long.parseLong(value) > 0;
    } catch (final NumberFormatException ignored) {
      return false;
    }
  }

  private static String text(final String name) {
    final String value = VALUES.get(name);
    if (value == null) throw new IllegalStateException("missing offline-cash-v1 fixture " + name);
    return value;
  }

  private static byte[] bytes(final String name) {
    final String value = text(name);
    if ((value.length() & 1) != 0) {
      throw new IllegalStateException("invalid fixture hex length for " + name);
    }
    final byte[] result = new byte[value.length() / 2];
    for (int index = 0; index < result.length; index++) {
      final int high = Character.digit(value.charAt(index * 2), 16);
      final int low = Character.digit(value.charAt(index * 2 + 1), 16);
      if (high < 0 || low < 0) {
        throw new IllegalStateException("invalid fixture hex for " + name);
      }
      result[index] = (byte) ((high << 4) | low);
    }
    return result;
  }
}
