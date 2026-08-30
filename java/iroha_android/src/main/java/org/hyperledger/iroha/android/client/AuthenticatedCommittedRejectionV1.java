package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.Objects;

/**
 * Native-checked terminal rejection from an authenticated Torii committed-state query.
 *
 * <p>The projection verifies the external transaction signature and every hash/result/proof-index
 * binding carried by the endpoint. Because that response does not contain a signed block header
 * or finality certificate, TLS authenticates the Torii response; independent finality requires
 * separately fetching and verifying the exact finalized block.
 */
public final class AuthenticatedCommittedRejectionV1 {
  private final String transactionHashHex;
  private final String transactionAuthorityAccountId;
  private final String blockHashHex;
  private final String resultHashHex;
  private final String rejectionCode;
  private final String rejectionMessage;
  private final long committedBlockHeight;

  AuthenticatedCommittedRejectionV1(
      final String transactionHashHex,
      final String transactionAuthorityAccountId,
      final String blockHashHex,
      final String resultHashHex,
      final String rejectionCode,
      final String rejectionMessage,
      final long committedBlockHeight) {
    this.transactionHashHex = requireHash(transactionHashHex, "transactionHashHex");
    this.transactionAuthorityAccountId =
        requireText(transactionAuthorityAccountId, "transactionAuthorityAccountId", 16 * 1024);
    this.blockHashHex = requireHash(blockHashHex, "blockHashHex");
    this.resultHashHex = requireHash(resultHashHex, "resultHashHex");
    this.rejectionCode = requireRejectionCode(rejectionCode);
    this.rejectionMessage = requireText(rejectionMessage, "rejectionMessage", 1_024);
    if (committedBlockHeight <= 0) {
      throw new IllegalArgumentException("committedBlockHeight must be positive");
    }
    this.committedBlockHeight = committedBlockHeight;
  }

  public String transactionHashHex() {
    return transactionHashHex;
  }

  public String transactionAuthorityAccountId() {
    return transactionAuthorityAccountId;
  }

  public String blockHashHex() {
    return blockHashHex;
  }

  public String resultHashHex() {
    return resultHashHex;
  }

  public String rejectionCode() {
    return rejectionCode;
  }

  public String rejectionMessage() {
    return rejectionMessage;
  }

  public long committedBlockHeight() {
    return committedBlockHeight;
  }

  private static String requireHash(final String value, final String field) {
    final String exact = Objects.requireNonNull(value, field);
    if (exact.length() != 64) {
      throw new IllegalArgumentException(field + " must be an exact lowercase 32-byte hash");
    }
    for (int index = 0; index < exact.length(); index++) {
      final char character = exact.charAt(index);
      if (!((character >= '0' && character <= '9')
          || (character >= 'a' && character <= 'f'))) {
        throw new IllegalArgumentException(field + " must be an exact lowercase 32-byte hash");
      }
    }
    return exact;
  }

  private static String requireText(
      final String value, final String field, final int maximumUtf8Bytes) {
    final String exact = Objects.requireNonNull(value, field);
    if (exact.isEmpty()
        || exact.getBytes(StandardCharsets.UTF_8).length > maximumUtf8Bytes
        || hasBoundaryWhitespace(exact)) {
      throw new IllegalArgumentException(field + " violates its closed UTF-8 text bound");
    }
    for (int index = 0; index < exact.length(); index++) {
      if (Character.isISOControl(exact.charAt(index))) {
        throw new IllegalArgumentException(field + " must not contain control characters");
      }
    }
    return exact;
  }

  private static boolean hasBoundaryWhitespace(final String value) {
    final int first = value.codePointAt(0);
    final int last = value.codePointBefore(value.length());
    return Character.isWhitespace(first)
        || Character.isSpaceChar(first)
        || Character.isWhitespace(last)
        || Character.isSpaceChar(last);
  }

  private static String requireRejectionCode(final String value) {
    final String exact = requireText(value, "rejectionCode", 128);
    switch (exact) {
      case "account_does_not_exist":
      case "limit_check":
      case "validation":
      case "instruction_execution":
      case "ivm_execution":
      case "trigger_execution":
        return exact;
      default:
        throw new IllegalArgumentException(
            "rejectionCode is not one of the six ABI-22 terminal rejection kinds");
    }
  }
}
