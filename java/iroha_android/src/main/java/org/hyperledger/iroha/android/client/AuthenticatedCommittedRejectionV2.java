package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.Objects;

/** Authority-split native-verified committed rejection. */
public final class AuthenticatedCommittedRejectionV2 {
  private final AuthenticatedCommittedRejectionV1 transaction;
  private final String queryAuthorityAccountId;

  AuthenticatedCommittedRejectionV2(
      final String transactionHashHex,
      final String queryAuthorityAccountId,
      final String transactionAuthorityAccountId,
      final String blockHashHex,
      final String resultHashHex,
      final String rejectionCode,
      final String rejectionMessage,
      final long committedBlockHeight) {
    this.transaction =
        new AuthenticatedCommittedRejectionV1(
            transactionHashHex,
            transactionAuthorityAccountId,
            blockHashHex,
            resultHashHex,
            rejectionCode,
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
  public String rejectionCode() { return transaction.rejectionCode(); }
  public String rejectionMessage() { return transaction.rejectionMessage(); }
  public long committedBlockHeight() { return transaction.committedBlockHeight(); }

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
