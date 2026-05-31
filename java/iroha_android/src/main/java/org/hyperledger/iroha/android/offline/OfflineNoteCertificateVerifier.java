package org.hyperledger.iroha.android.offline;

/** Verifies issuer trust and attestation shape for Offline Note key certificates. */
public interface OfflineNoteCertificateVerifier {
  boolean verifyCertificate(OfflineNote.KeyCertificate certificate);
}
