package org.hyperledger.iroha.android.offline;

/** State persisted for a wallet-owned Offline Note note. */
public enum OfflineNoteWalletNoteState {
  SPENDABLE,
  RECEIVE_PENDING,
  SPENT,
  REDEEM_PENDING,
  REDEEMED,
  CANCELLED
}
