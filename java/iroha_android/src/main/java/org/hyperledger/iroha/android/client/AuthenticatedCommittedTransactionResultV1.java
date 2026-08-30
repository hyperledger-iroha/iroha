package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.Objects;

/**
 * Native-verified success or rejection from an authenticated committed-state query.
 *
 * <p>This authenticates Torii's committed-state answer; it is not a signed block or QC and does
 * not independently prove finality. Independent finality requires exact-block verification.
 */
public final class AuthenticatedCommittedTransactionResultV1 {
  private final String transactionHashHex;
  private final String transactionAuthorityAccountId;
  private final String blockHashHex;
  private final String resultHashHex;
  private final boolean resultOk;
  private final String rejectionMessage;
  private final BigInteger committedBlockHeight;

  public AuthenticatedCommittedTransactionResultV1(
      final String transactionHashHex,
      final String transactionAuthorityAccountId,
      final String blockHashHex,
      final String resultHashHex,
      final boolean resultOk,
      final String rejectionMessage,
      final BigInteger committedBlockHeight) {
    this.transactionHashHex = requireHash(transactionHashHex, "transactionHashHex");
    this.transactionAuthorityAccountId =
        requireText(transactionAuthorityAccountId, "transactionAuthorityAccountId", 16 * 1024);
    this.blockHashHex = requireHash(blockHashHex, "blockHashHex");
    this.resultHashHex = requireHash(resultHashHex, "resultHashHex");
    this.resultOk = resultOk;
    if (resultOk) {
      if (rejectionMessage != null) {
        throw new IllegalArgumentException(
            "successful committed results must not carry a rejection message");
      }
      this.rejectionMessage = null;
    } else {
      this.rejectionMessage = requireText(rejectionMessage, "rejectionMessage", 1_024);
    }
    this.committedBlockHeight =
        Objects.requireNonNull(committedBlockHeight, "committedBlockHeight");
    if (committedBlockHeight.signum() <= 0 || committedBlockHeight.bitLength() > 64) {
      throw new IllegalArgumentException("committedBlockHeight must be a positive u64");
    }
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

  public boolean resultOk() {
    return resultOk;
  }

  public String rejectionMessage() {
    return rejectionMessage;
  }

  public BigInteger committedBlockHeight() {
    return committedBlockHeight;
  }

  private static String requireHash(final String value, final String field) {
    if (value == null || !value.matches("[0-9a-f]{64}")) {
      throw new IllegalArgumentException(field + " must be an exact lowercase 32-byte hash");
    }
    return value;
  }

  private static String requireText(
      final String value, final String field, final int maximumUtf8Bytes) {
    if (value == null
        || value.isEmpty()
        || value.getBytes(StandardCharsets.UTF_8).length > maximumUtf8Bytes
        || hasBoundaryWhitespace(value)
        || value.codePoints().anyMatch(Character::isISOControl)) {
      throw new IllegalArgumentException(field + " violates its closed UTF-8 text bound");
    }
    return value;
  }

  private static boolean hasBoundaryWhitespace(final String value) {
    final int first = value.codePointAt(0);
    final int last = value.codePointBefore(value.length());
    return Character.isWhitespace(first)
        || Character.isSpaceChar(first)
        || Character.isWhitespace(last)
        || Character.isSpaceChar(last);
  }
}
