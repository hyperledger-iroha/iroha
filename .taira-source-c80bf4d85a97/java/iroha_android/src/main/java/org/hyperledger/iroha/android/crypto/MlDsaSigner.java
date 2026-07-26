package org.hyperledger.iroha.android.crypto;

import org.hyperledger.iroha.android.SigningException;

/** ML-DSA signer backed by the native {@code connect_norito_bridge} helpers. */
public final class MlDsaSigner implements Signer {
  private final MlDsaPrivateKey privateKey;
  private final MlDsaPublicKey publicKey;

  public MlDsaSigner(final MlDsaPrivateKey privateKey) {
    this(privateKey, privateKey.publicKey());
  }

  public MlDsaSigner(final MlDsaPrivateKey privateKey, final MlDsaPublicKey publicKey) {
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
      return NativeSignerBridge.signDetached(
          SigningAlgorithm.ML_DSA, privateKey.getEncoded(), prehashed);
    } catch (final RuntimeException ex) {
      throw new SigningException("ML-DSA signing failed", ex);
    }
  }

  @Override
  public byte[] publicKey() {
    return publicKey.getEncoded();
  }

  @Override
  public String algorithm() {
    return SigningAlgorithm.ML_DSA.providerName();
  }
}
