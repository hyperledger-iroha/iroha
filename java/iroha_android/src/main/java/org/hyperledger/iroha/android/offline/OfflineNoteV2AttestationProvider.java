package org.hyperledger.iroha.android.offline;

/** Supplies wallet-bound Offline Note V2 key certificates. */
public interface OfflineNoteV2AttestationProvider {
  OfflineNoteV2.KeyCertificateV2 currentKeyCertificate();
}
