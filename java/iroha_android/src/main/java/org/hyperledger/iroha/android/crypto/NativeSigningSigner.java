package org.hyperledger.iroha.android.crypto;

import org.hyperledger.iroha.android.SigningException;

/** Signer backed by native {@code connect_norito_bridge} helpers for non-Ed25519 algorithms. */
public final class NativeSigningSigner implements Signer {
  private final SigningAlgorithm signingAlgorithm;
  private final NativeSigningPrivateKey privateKey;
  private final NativeSigningPublicKey publicKey;

  public NativeSigningSigner(
      final SigningAlgorithm signingAlgorithm, final NativeSigningPrivateKey privateKey) {
    this(signingAlgorithm, privateKey, privateKey.publicKey());
  }

  public NativeSigningSigner(
      final SigningAlgorithm signingAlgorithm,
      final NativeSigningPrivateKey privateKey,
      final NativeSigningPublicKey publicKey) {
    if (!NativeSigningKeyMaterial.supports(signingAlgorithm)) {
      throw new IllegalArgumentException("Unsupported native signing algorithm: " + signingAlgorithm);
    }
    if (privateKey == null || privateKey.signingAlgorithm() != signingAlgorithm) {
      throw new IllegalArgumentException("private key algorithm does not match signer algorithm");
    }
    if (publicKey == null || publicKey.signingAlgorithm() != signingAlgorithm) {
      throw new IllegalArgumentException("public key algorithm does not match signer algorithm");
    }
    this.signingAlgorithm = signingAlgorithm;
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
      return NativeSignerBridge.signDetached(signingAlgorithm, privateKey.getEncoded(), prehashed);
    } catch (final RuntimeException ex) {
      throw new SigningException(signingAlgorithm.providerName() + " signing failed", ex);
    }
  }

  @Override
  public byte[] publicKey() {
    return publicKey.getEncoded();
  }

  @Override
  public String algorithm() {
    return signingAlgorithm.providerName();
  }
}
