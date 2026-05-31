package org.hyperledger.iroha.android.offline;

import java.util.Objects;

/** App-facing Offline Bearer Cash note wrapper over the ZK note engine. */
public final class OfflineBearerCashNote {
  private final OfflineNoteWalletNote delegate;

  public OfflineBearerCashNote(final OfflineNoteWalletNote delegate) {
    this.delegate = Objects.requireNonNull(delegate, "delegate");
  }

  public OfflineNoteWalletNote unwrap() {
    return delegate;
  }

  public String chainId() {
    return delegate.chainId();
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

  public String noteCommitmentHex() {
    return delegate.noteCommitmentHex();
  }
}
