package org.hyperledger.iroha.android.offline;

import java.util.Objects;

/** Bearer Cash v1 payment token produced by a payer. */
public final class OfflineBearerCashPaymentTokenV1 {
  private final OfflineNotePaymentToken delegate;

  public OfflineBearerCashPaymentTokenV1(final OfflineNotePaymentToken delegate) {
    this.delegate = Objects.requireNonNull(delegate, "delegate");
  }

  public OfflineNotePaymentToken unwrap() {
    return delegate;
  }

  public String chainId() {
    return delegate.chainId();
  }

  public String paymentRequestId() {
    return delegate.paymentRequestId();
  }

  public String tokenIdHex() {
    return delegate.tokenIdHex();
  }

  public long createdAtMs() {
    return delegate.createdAtMs();
  }
}
