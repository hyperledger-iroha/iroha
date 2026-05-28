package org.hyperledger.iroha.android.offline;

import java.util.Objects;

/** Bearer Cash v1 receipt ACK returned after accepting a payment token. */
public final class OfflineBearerCashAckV1 {
  private final OfflineNoteReceiptAck delegate;

  public OfflineBearerCashAckV1(final OfflineNoteReceiptAck delegate) {
    this.delegate = Objects.requireNonNull(delegate, "delegate");
  }

  public OfflineNoteReceiptAck unwrap() {
    return delegate;
  }

  public String paymentRequestId() {
    return delegate.paymentRequestId();
  }

  public String tokenIdHex() {
    return delegate.tokenIdHex();
  }

  public String recipientAccountId() {
    return delegate.recipientAccountId();
  }
}
