package org.hyperledger.iroha.android.offline;

/** Supplies deterministic random material in tests and secure random material in production. */
public interface OfflineNoteV2RandomSource {
  byte[] nextBytes(int length);
}
