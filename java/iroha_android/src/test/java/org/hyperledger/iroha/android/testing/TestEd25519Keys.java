package org.hyperledger.iroha.android.testing;

import java.util.Arrays;
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters;

/** Deterministic valid Ed25519 keys for SDK tests that do not exercise key admission. */
public final class TestEd25519Keys {

  private TestEd25519Keys() {}

  /** Derives a valid public key from a 32-byte seed filled with {@code seedByte}. */
  public static byte[] publicKey(final int seedByte) {
    final byte[] seed = new byte[32];
    Arrays.fill(seed, (byte) seedByte);
    return new Ed25519PrivateKeyParameters(seed, 0).generatePublicKey().getEncoded();
  }

  /** Derives a valid public key and returns its canonical lowercase hex encoding. */
  public static String publicKeyHex(final int seedByte) {
    final byte[] publicKey = publicKey(seedByte);
    final char[] encoded = new char[publicKey.length * 2];
    final char[] alphabet = "0123456789abcdef".toCharArray();
    for (int index = 0; index < publicKey.length; index++) {
      final int value = publicKey[index] & 0xFF;
      encoded[index * 2] = alphabet[value >>> 4];
      encoded[index * 2 + 1] = alphabet[value & 0x0F];
    }
    return new String(encoded);
  }
}
