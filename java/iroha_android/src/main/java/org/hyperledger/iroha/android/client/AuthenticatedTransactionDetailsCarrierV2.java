package org.hyperledger.iroha.android.client;

/**
 * Exact signed query plus its structurally native-checked transaction-details response.
 *
 * <p>The height and result fields are routing hints only. They are not terminal evidence until
 * {@code projectFinalizedKagemushaOutcomeV1} authenticates the corresponding finality page and
 * executed block wire.
 */
public final class AuthenticatedTransactionDetailsCarrierV2 {
  private final AuthenticatedTransactionDetailsNativeBridge.SignedQueryV2 signedQuery;
  private final byte[] responseNorito;
  private final long committedBlockHeightHint;
  private final boolean resultOkHint;

  AuthenticatedTransactionDetailsCarrierV2(
      final AuthenticatedTransactionDetailsNativeBridge.SignedQueryV2 signedQuery,
      final byte[] responseNorito,
      final long committedBlockHeightHint,
      final boolean resultOkHint) {
    if (signedQuery == null || responseNorito == null || responseNorito.length == 0) {
      throw new IllegalArgumentException("transaction-details carrier inputs must be nonempty");
    }
    if (committedBlockHeightHint <= 0) {
      throw new IllegalArgumentException("committedBlockHeightHint must be positive");
    }
    this.signedQuery = signedQuery;
    this.responseNorito = responseNorito.clone();
    this.committedBlockHeightHint = committedBlockHeightHint;
    this.resultOkHint = resultOkHint;
  }

  /** Exact response bytes suitable for a private content-addressed evidence cache. */
  public byte[] responseNorito() { return responseNorito.clone(); }

  /** Untrusted routing hint; never consume or release value from this field. */
  public long committedBlockHeightHint() { return committedBlockHeightHint; }

  /** Untrusted routing hint; never consume or release value from this field. */
  public boolean resultOkHint() { return resultOkHint; }

  AuthenticatedTransactionDetailsNativeBridge.SignedQueryV2 signedQuery() { return signedQuery; }
}
