package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.Objects;

/** Authority-split native-verified committed transaction result. */
public final class AuthenticatedCommittedTransactionResultV2 {
  private final AuthenticatedCommittedTransactionResultV1 transaction;
  private final String queryAuthorityAccountId;

  AuthenticatedCommittedTransactionResultV2(
      final String transactionHashHex,
      final String queryAuthorityAccountId,
      final String transactionAuthorityAccountId,
      final String blockHashHex,
      final String resultHashHex,
      final boolean resultOk,
      final String rejectionMessage,
      final BigInteger committedBlockHeight) {
    this.transaction =
        new AuthenticatedCommittedTransactionResultV1(
            transactionHashHex,
            transactionAuthorityAccountId,
            blockHashHex,
            resultHashHex,
            resultOk,
            rejectionMessage,
            committedBlockHeight);
    this.queryAuthorityAccountId = requireAuthority(queryAuthorityAccountId);
  }

  public String transactionHashHex() { return transaction.transactionHashHex(); }
  public String queryAuthorityAccountId() { return queryAuthorityAccountId; }
  public String transactionAuthorityAccountId() {
    return transaction.transactionAuthorityAccountId();
  }
  public String blockHashHex() { return transaction.blockHashHex(); }
  public String resultHashHex() { return transaction.resultHashHex(); }
  public boolean resultOk() { return transaction.resultOk(); }
  public String rejectionMessage() { return transaction.rejectionMessage(); }
  public BigInteger committedBlockHeight() { return transaction.committedBlockHeight(); }

  private static String requireAuthority(final String value) {
    final String exact = Objects.requireNonNull(value, "queryAuthorityAccountId");
    if (exact.isEmpty()
        || exact.getBytes(StandardCharsets.UTF_8).length > 16 * 1024
        || !exact.equals(exact.trim())
        || exact.codePoints().anyMatch(Character::isISOControl)) {
      throw new IllegalArgumentException(
          "queryAuthorityAccountId must be canonical non-empty text");
    }
    return exact;
  }
}
