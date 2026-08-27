package org.hyperledger.iroha.android.crypto;

import java.io.Serializable;
import java.security.PublicKey;
import java.util.Arrays;

/** Raw ML-DSA public key wrapper used by the Java/Android SDK software provider. */
public final class MlDsaPublicKey implements PublicKey, Serializable {
  private final byte[] encoded;

  public MlDsaPublicKey(final byte[] encoded) {
    if (!MlDsaPublicKeyAdmission.isValid(encoded)) {
      throw new IllegalArgumentException(
          "invalid ML-DSA-65 public key: expected 1952 bytes with nonzero material");
    }
    this.encoded = Arrays.copyOf(encoded, encoded.length);
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
}
