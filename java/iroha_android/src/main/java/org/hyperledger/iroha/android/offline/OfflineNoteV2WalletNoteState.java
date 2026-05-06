package org.hyperledger.iroha.android.offline;

/** State persisted for a wallet-owned Offline Note V2 note. */
public enum OfflineNoteV2WalletNoteState {
  SPENDABLE,
  RECEIVE_PENDING,
  CHANGE_PENDING,
  SPEND_PENDING,
  REDEEM_PENDING,
  REDEEMED
}
