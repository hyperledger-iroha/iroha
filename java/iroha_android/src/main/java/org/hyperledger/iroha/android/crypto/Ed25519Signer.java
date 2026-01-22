package org.hyperledger.iroha.android.crypto;

import java.security.GeneralSecurityException;
import java.security.PrivateKey;
import java.security.PublicKey;
import java.security.Signature;
import java.util.Arrays;
import org.hyperledger.iroha.android.SigningException;

/**
 * Ed25519 signer that relies on the JCA provider available on the runtime.
 *
 * <p>Implements Iroha's signing convention: the payload is pre-hashed with Blake2b-256 and the
 * least-significant bit is set to {@code 1} before signing.
 */
public final class Ed25519Signer implements Signer {

  private final PrivateKey privateKey;
  private final PublicKey publicKey;

  public Ed25519Signer(final PrivateKey privateKey, final PublicKey publicKey) {
    if (privateKey == null) {
      throw new IllegalArgumentException("privateKey must not be null");
    }
    if (publicKey == null) {
      throw new IllegalArgumentException("publicKey must not be null");
    }
    this.privateKey = privateKey;
    this.publicKey = publicKey;
  }

  @Override
  public byte[] sign(final byte[] message) throws SigningException {
    if (message == null) {
      throw new SigningException("message must not be null");
    }
    try {
      final byte[] prehashed = IrohaHash.prehash(message);
      final Signature signature = Signature.getInstance("Ed25519");
      signature.initSign(privateKey);
      signature.update(prehashed);
      return signature.sign();
    } catch (final GeneralSecurityException ex) {
      throw new SigningException("Ed25519 signing failed", ex);
    }
  }

  @Override
  public byte[] publicKey() {
    final byte[] encoded = publicKey.getEncoded();
    return encoded != null ? Arrays.copyOf(encoded, encoded.length) : new byte[0];
  }

  @Override
  public String algorithm() {
    return "Ed25519";
  }

  public PublicKey jcaPublicKey() {
    return publicKey;
  }
}
