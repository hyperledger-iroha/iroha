package org.hyperledger.iroha.android.offline;

import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters;
import org.bouncycastle.crypto.signers.Ed25519Signer;

/** Ed25519 verifier for issuer-signed Offline Note key certificates. */
public final class Ed25519OfflineNoteCertificateVerifier
    implements OfflineNoteCertificateVerifier {
  private final List<byte[]> trustedIssuerPublicKeys;

  public Ed25519OfflineNoteCertificateVerifier(final Collection<byte[]> trustedIssuerPublicKeys) {
    this.trustedIssuerPublicKeys = new ArrayList<>();
    for (final byte[] root : trustedIssuerPublicKeys) {
      this.trustedIssuerPublicKeys.add(root.clone());
    }
  }

  @Override
  public boolean verifyCertificate(final OfflineNote.KeyCertificate certificate) {
    if (trustedIssuerPublicKeys.isEmpty()
        || certificate.platform().trim().isEmpty()
        || certificate.keyId().trim().isEmpty()
        || certificate.deviceId().trim().isEmpty()
        || certificate.assertionScheme().trim().isEmpty()
        || certificate.assertionKeyAlgorithm().trim().isEmpty()
        || certificate.assertionPublicKey().length == 0) {
      return false;
    }
    final byte[] message = certificate.signingBytes();
    final byte[] signature = certificate.issuerSignature();
    for (final byte[] root : trustedIssuerPublicKeys) {
      if (root.length == 32 && verifyEd25519(root, message, signature)) {
        return true;
      }
    }
    return false;
  }

  private static boolean verifyEd25519(
      final byte[] publicKey, final byte[] message, final byte[] signature) {
    try {
      final Ed25519Signer verifier = new Ed25519Signer();
      verifier.init(false, new Ed25519PublicKeyParameters(publicKey, 0));
      verifier.update(message, 0, message.length);
      return verifier.verifySignature(signature);
    } catch (final RuntimeException e) {
      return false;
    }
  }
}
