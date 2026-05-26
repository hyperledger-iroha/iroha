package org.hyperledger.iroha.android.offline;

/** Fails closed until a wallet is configured with trusted issuer roots. */
public final class RejectingOfflineNoteCertificateVerifier
    implements OfflineNoteCertificateVerifier {
  @Override
  public boolean verifyCertificate(final OfflineNote.KeyCertificate certificate) {
    return false;
  }
}
