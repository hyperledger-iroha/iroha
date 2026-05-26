package org.hyperledger.iroha.android.offline;

import java.util.Objects;

/** Resolution returned by a wallet sync resolver for one pending Offline Note note. */
public final class OfflineNoteSyncResolution {
  private final OfflineNoteWalletNoteState state;
  private final String transactionHashHex;

  public OfflineNoteSyncResolution(final OfflineNoteWalletNoteState state) {
    this(state, null);
  }

  public OfflineNoteSyncResolution(
      final OfflineNoteWalletNoteState state, final String transactionHashHex) {
    this.state = Objects.requireNonNull(state, "state");
    this.transactionHashHex = transactionHashHex;
  }

  public OfflineNoteWalletNoteState state() {
    return state;
  }

  public String transactionHashHex() {
    return transactionHashHex;
  }
}
