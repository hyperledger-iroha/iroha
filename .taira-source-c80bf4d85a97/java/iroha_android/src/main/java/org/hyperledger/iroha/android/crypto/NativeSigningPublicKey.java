package org.hyperledger.iroha.android.crypto;

import java.io.Serializable;
import java.security.PublicKey;
import java.util.Arrays;

/** Raw public key wrapper for native-backed Iroha signing algorithms. */
public final class NativeSigningPublicKey implements PublicKey, Serializable {
  private final SigningAlgorithm signingAlgorithm;
  private final byte[] encoded;

  public NativeSigningPublicKey(final SigningAlgorithm signingAlgorithm, final byte[] encoded) {
    if (!NativeSigningKeyMaterial.supports(signingAlgorithm)) {
      throw new IllegalArgumentException("algorithm must be a native-backed non-ML-DSA signing algorithm");
    }
    if (encoded == null || encoded.length == 0) {
      throw new IllegalArgumentException("encoded must not be empty");
    }
    this.signingAlgorithm = signingAlgorithm;
    this.encoded = Arrays.copyOf(encoded, encoded.length);
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
}
