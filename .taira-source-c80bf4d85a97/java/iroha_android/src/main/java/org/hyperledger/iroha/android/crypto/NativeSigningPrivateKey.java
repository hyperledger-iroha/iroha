package org.hyperledger.iroha.android.crypto;

import java.io.Serializable;
import java.security.PrivateKey;
import java.util.Arrays;

/** Raw private key wrapper for native-backed Iroha signing algorithms. */
public final class NativeSigningPrivateKey implements PrivateKey, Serializable {
  private final SigningAlgorithm signingAlgorithm;
  private final byte[] encoded;
  private final byte[] cachedPublicKey;

  public NativeSigningPrivateKey(final SigningAlgorithm signingAlgorithm, final byte[] encoded) {
    this(signingAlgorithm, encoded, null);
  }

  public NativeSigningPrivateKey(
      final SigningAlgorithm signingAlgorithm, final byte[] encoded, final byte[] publicKey) {
    if (!NativeSigningKeyMaterial.supports(signingAlgorithm)) {
      throw new IllegalArgumentException("algorithm must be a native-backed non-ML-DSA signing algorithm");
    }
    if (encoded == null || encoded.length == 0) {
      throw new IllegalArgumentException("encoded must not be empty");
    }
    this.signingAlgorithm = signingAlgorithm;
    this.encoded = Arrays.copyOf(encoded, encoded.length);
    this.cachedPublicKey = publicKey == null ? null : Arrays.copyOf(publicKey, publicKey.length);
  }

  public SigningAlgorithm signingAlgorithm() {
    return signingAlgorithm;
  }

  @Override
  public String getAlgorithm() {
    return signingAlgorithm.providerName();
  }

  @Override
  public String getFormat() {
    return "RAW";
  }

  @Override
  public byte[] getEncoded() {
    return Arrays.copyOf(encoded, encoded.length);
  }

  public NativeSigningPublicKey publicKey() {
    final byte[] bytes =
        cachedPublicKey != null
            ? Arrays.copyOf(cachedPublicKey, cachedPublicKey.length)
            : NativeSignerBridge.publicKeyFromPrivate(signingAlgorithm, encoded);
    return new NativeSigningPublicKey(signingAlgorithm, bytes);
  }
}
