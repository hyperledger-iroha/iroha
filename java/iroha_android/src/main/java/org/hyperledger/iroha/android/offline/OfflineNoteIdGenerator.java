package org.hyperledger.iroha.android.offline;

/** Generates wallet-local request and operation identifiers. */
public interface OfflineNoteIdGenerator {
  String nextId(String prefix);
}
