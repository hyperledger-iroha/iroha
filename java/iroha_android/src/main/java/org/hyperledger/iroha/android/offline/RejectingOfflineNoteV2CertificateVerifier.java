package org.hyperledger.iroha.android.offline;

/** Fails closed until a wallet is configured with trusted issuer roots. */
public final class RejectingOfflineNoteV2CertificateVerifier
    implements OfflineNoteV2CertificateVerifier {
  @Override
  public boolean verifyCertificate(final OfflineNoteV2.KeyCertificateV2 certificate) {
    return false;
  }
}
