package org.hyperledger.iroha.android.offline;

/** Fails closed until a wallet is configured with trusted issuer roots. */
public final class RejectingOfflineNoteCertificateVerifier
    implements OfflineNoteCertificateVerifier {
  @Override
  public boolean verifyIssuerCertificate(final OfflineNote.KeyCertificate certificate) {
    return false;
  }

  @Override
  public boolean verifyOwnerCertificate(final OfflineNote.KeyCertificate certificate) {
    return false;
  }
}
