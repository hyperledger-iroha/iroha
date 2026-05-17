package org.hyperledger.iroha.android.offline;

/** Verifies issuer trust and attestation shape for Offline Note V2 key certificates. */
public interface OfflineNoteV2CertificateVerifier {
  boolean verifyCertificate(OfflineNoteV2.KeyCertificateV2 certificate);
}
