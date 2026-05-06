package org.hyperledger.iroha.android.offline;

/** Generates wallet-local request and operation identifiers. */
public interface OfflineNoteV2IdGenerator {
  String nextId(String prefix);
}
