package org.hyperledger.iroha.android.offline;

import java.util.Objects;

/** Resolution returned by a wallet sync resolver for one pending Offline Note V2 note. */
public final class OfflineNoteV2SyncResolution {
  private final OfflineNoteV2WalletNoteState state;
  private final String transactionHashHex;

  public OfflineNoteV2SyncResolution(final OfflineNoteV2WalletNoteState state) {
    this(state, null);
  }

  public OfflineNoteV2SyncResolution(
      final OfflineNoteV2WalletNoteState state, final String transactionHashHex) {
    this.state = Objects.requireNonNull(state, "state");
    this.transactionHashHex = transactionHashHex;
  }

  public OfflineNoteV2WalletNoteState state() {
    return state;
  }

  public String transactionHashHex() {
    return transactionHashHex;
  }
}
