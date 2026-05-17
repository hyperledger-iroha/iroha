package org.hyperledger.iroha.android.offline;

/** State persisted for a wallet-owned Offline Note V2 note. */
public enum OfflineNoteV2WalletNoteState {
  SPENDABLE,
  RECEIVE_PENDING,
  SPENT,
  REDEEM_PENDING,
  REDEEMED,
  CANCELLED
}
