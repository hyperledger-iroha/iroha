package org.hyperledger.iroha.android.offline;

/** Supplies wallet-bound Offline Note key certificates. */
public interface OfflineNoteAttestationProvider {
  OfflineNote.KeyCertificate currentKeyCertificate();
}
