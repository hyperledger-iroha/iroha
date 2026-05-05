package org.hyperledger.iroha.android.crypto;

import java.security.KeyPair;
import java.security.SecureRandom;
import java.util.Arrays;

/** Key material helpers for native-backed non-Ed25519 Iroha signing algorithms. */
public final class NativeSigningKeyMaterial {
  private static final int NATIVE_SIGNING_SEED_LENGTH_BYTES = 32;

  private NativeSigningKeyMaterial() {}

  public static boolean supports(final SigningAlgorithm algorithm) {
    return algorithm != null && algorithm.isNativeBacked() && algorithm != SigningAlgorithm.ML_DSA;
  }

  public static KeyPair generate(final SigningAlgorithm algorithm, final SecureRandom secureRandom) {
    final byte[] seed = new byte[NATIVE_SIGNING_SEED_LENGTH_BYTES];
    secureRandom.nextBytes(seed);
    try {
      return fromSeed(algorithm, seed);
    } finally {
      Arrays.fill(seed, (byte) 0);
    }
  }

  public static KeyPair fromSeed(final SigningAlgorithm algorithm, final byte[] seed) {
    if (!supports(algorithm)) {
      throw new IllegalArgumentException("Unsupported native signing algorithm: " + algorithm);
    }
    final NativeSignerBridge.KeypairBytes pair = NativeSignerBridge.keypairFromSeed(algorithm, seed);
    return fromRaw(algorithm, pair.privateKey(), pair.publicKey());
  }

  public static KeyPair fromRaw(
      final SigningAlgorithm algorithm, final byte[] privateKey, final byte[] publicKey) {
    if (!supports(algorithm)) {
      throw new IllegalArgumentException("Unsupported native signing algorithm: " + algorithm);
    }
    final byte[] expected = NativeSignerBridge.publicKeyFromPrivate(algorithm, privateKey);
    if (!Arrays.equals(expected, publicKey)) {
      throw new IllegalArgumentException(
          algorithm.providerName() + " public key does not match private key");
    }
    return new KeyPair(
        new NativeSigningPublicKey(algorithm, publicKey),
        new NativeSigningPrivateKey(algorithm, privateKey, publicKey));
  }
}
