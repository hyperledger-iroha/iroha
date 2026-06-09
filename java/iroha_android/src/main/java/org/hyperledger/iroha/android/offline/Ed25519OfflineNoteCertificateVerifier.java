package org.hyperledger.iroha.android.offline;

import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.Optional;
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters;
import org.bouncycastle.crypto.signers.Ed25519Signer;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AccountAddress.AccountAddressException;

/** Ed25519 verifier for issuer-signed and owner-self-signed Offline Note key certificates. */
public final class Ed25519OfflineNoteCertificateVerifier
    implements OfflineNoteCertificateVerifier {
  private static final int ED25519_CURVE_ID = 0x01;

  private final List<byte[]> trustedIssuerPublicKeys;

  public Ed25519OfflineNoteCertificateVerifier(final Collection<byte[]> trustedIssuerPublicKeys) {
    this.trustedIssuerPublicKeys = new ArrayList<>();
    for (final byte[] root : trustedIssuerPublicKeys) {
      this.trustedIssuerPublicKeys.add(root.clone());
    }
  }

  @Override
  public boolean verifyIssuerCertificate(final OfflineNote.KeyCertificate certificate) {
    if (trustedIssuerPublicKeys.isEmpty() || !hasValidAttestationShape(certificate)) {
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

  @Override
  public boolean verifyOwnerCertificate(final OfflineNote.KeyCertificate certificate) {
    if (!hasValidAttestationShape(certificate)) {
      return false;
    }
    final byte[] ownerKey = ownerSignatoryKey(certificate.accountId());
    return ownerKey != null
        && verifyEd25519(ownerKey, certificate.signingBytes(), certificate.issuerSignature());
  }

  private static boolean hasValidAttestationShape(final OfflineNote.KeyCertificate certificate) {
    return !certificate.platform().trim().isEmpty()
        && !certificate.keyId().trim().isEmpty()
        && !certificate.deviceId().trim().isEmpty()
        && !certificate.assertionScheme().trim().isEmpty()
        && !certificate.assertionKeyAlgorithm().trim().isEmpty()
        && certificate.assertionPublicKey().length != 0;
  }

  private static byte[] ownerSignatoryKey(final String accountId) {
    final Optional<AccountAddress.SingleKeyPayload> single;
    try {
      single =
          AccountAddress.parseEncodedIgnoringCurveSupport(accountId, null)
              .address
              .singleKeyPayloadIgnoringCurveSupport();
    } catch (final AccountAddressException e) {
      return null;
    }
    if (!single.isPresent()) {
      return null;
    }
    final AccountAddress.SingleKeyPayload payload = single.get();
    final byte[] publicKey = payload.publicKey();
    if (payload.curveId() != ED25519_CURVE_ID || publicKey.length != 32) {
      return null;
    }
    return publicKey;
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
