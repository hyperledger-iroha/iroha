package org.hyperledger.iroha.android.crypto;

import java.io.Serializable;
import java.security.PrivateKey;
import java.util.Arrays;

/** Raw ML-DSA private key wrapper used by the Java/Android SDK software provider. */
public final class MlDsaPrivateKey implements PrivateKey, Serializable {
  private final byte[] encoded;
  private final byte[] cachedPublicKey;

  public MlDsaPrivateKey(final byte[] encoded) {
    this(encoded, null);
  }

  public MlDsaPrivateKey(final byte[] encoded, final byte[] publicKey) {
    if (encoded == null || encoded.length == 0) {
      throw new IllegalArgumentException("encoded must not be empty");
    }
    this.encoded = Arrays.copyOf(encoded, encoded.length);
    this.cachedPublicKey = publicKey == null ? null : Arrays.copyOf(publicKey, publicKey.length);
  }

  @Override
  public String getAlgorithm() {
    return SigningAlgorithm.ML_DSA.providerName();
  }

  @Override
  public String getFormat() {
    return "RAW";
  }

  @Override
  public byte[] getEncoded() {
    return Arrays.copyOf(encoded, encoded.length);
  }

  public MlDsaPublicKey publicKey() {
    final byte[] bytes =
        cachedPublicKey != null
            ? Arrays.copyOf(cachedPublicKey, cachedPublicKey.length)
            : NativeSignerBridge.publicKeyFromPrivate(SigningAlgorithm.ML_DSA, encoded);
    return new MlDsaPublicKey(bytes);
  }
}
