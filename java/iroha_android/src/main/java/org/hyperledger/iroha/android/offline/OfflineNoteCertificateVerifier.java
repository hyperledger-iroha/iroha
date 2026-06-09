package org.hyperledger.iroha.android.offline;

/** Verifies trust and attestation shape for Offline Note key certificates. */
public interface OfflineNoteCertificateVerifier {
  /** Verifies a certificate signed by a trusted issuer for topup/issue paths. */
  boolean verifyIssuerCertificate(OfflineNote.KeyCertificate certificate);

  /** Verifies a certificate self-signed by the account named in its accountId for P2P paths. */
  boolean verifyOwnerCertificate(OfflineNote.KeyCertificate certificate);
}
