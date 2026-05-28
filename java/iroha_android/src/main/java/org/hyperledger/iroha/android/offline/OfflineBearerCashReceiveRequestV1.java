package org.hyperledger.iroha.android.offline;

import java.util.Objects;

/** Bearer Cash v1 receive request handed to a payer. */
public final class OfflineBearerCashReceiveRequestV1 {
  private final OfflineNoteReceiveRequest delegate;

  public OfflineBearerCashReceiveRequestV1(final OfflineNoteReceiveRequest delegate) {
    this.delegate = Objects.requireNonNull(delegate, "delegate");
  }

  public OfflineNoteReceiveRequest unwrap() {
    return delegate;
  }

  public String paymentRequestId() {
    return delegate.paymentRequestId();
  }

  public String accountId() {
    return delegate.accountId();
  }

  public String assetId() {
    return delegate.assetId();
  }

  public String canonicalAmount() {
    return delegate.canonicalAmount();
  }

  public String outputCommitmentHex() {
    return delegate.outputCommitmentHex();
  }
}
