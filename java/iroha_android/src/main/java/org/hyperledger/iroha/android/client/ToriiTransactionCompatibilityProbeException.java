package org.hyperledger.iroha.android.client;

/** Raised when the fresh Torii capability advert cannot be fetched or decoded exactly. */
public final class ToriiTransactionCompatibilityProbeException
    extends ToriiTransactionCompatibilityException {
  /** Creates a capability-probe failure with its underlying cause. */
  public ToriiTransactionCompatibilityProbeException(final Throwable cause) {
    super("Failed to verify Torii transaction submission compatibility", cause);
  }
}
