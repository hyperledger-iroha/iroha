package org.hyperledger.iroha.android.crypto;

/** Thin JVM/JNI wrapper around {@code connect_norito_bridge} signing helpers. */
public final class NativeSignerBridge {
  private static final String LIBRARY_NAME = "connect_norito_bridge";
  private static final boolean NATIVE_AVAILABLE = loadLibrary();

  private NativeSignerBridge() {}

  public static boolean isNativeAvailable() {
    return NATIVE_AVAILABLE;
  }

  public static byte[] publicKeyFromPrivate(
      final SigningAlgorithm algorithm, final byte[] privateKey) {
    if (privateKey == null || privateKey.length == 0) {
      throw new IllegalArgumentException("privateKey must not be empty");
    }
    requireNative();
    final byte[] result = nativePublicKeyFromPrivate(algorithm.bridgeCode(), privateKey);
    if (result == null) {
      throw new IllegalStateException("nativePublicKeyFromPrivate returned null");
    }
    return result;
  }

  public static KeypairBytes keypairFromSeed(
      final SigningAlgorithm algorithm, final byte[] seed) {
    if (seed == null || seed.length == 0) {
      throw new IllegalArgumentException("seed must not be empty");
    }
    requireNative();
    final byte[][] result = nativeKeypairFromSeed(algorithm.bridgeCode(), seed);
    if (result == null || result.length != 2 || result[0] == null || result[1] == null) {
      throw new IllegalStateException("nativeKeypairFromSeed returned invalid key material");
    }
    return new KeypairBytes(result[0], result[1]);
  }

  public static byte[] signDetached(
      final SigningAlgorithm algorithm, final byte[] privateKey, final byte[] message) {
    if (privateKey == null || privateKey.length == 0) {
      throw new IllegalArgumentException("privateKey must not be empty");
    }
    if (message == null || message.length == 0) {
      throw new IllegalArgumentException("message must not be empty");
    }
    requireNative();
    final byte[] result = nativeSignDetached(algorithm.bridgeCode(), privateKey, message);
    if (result == null) {
      throw new IllegalStateException("nativeSignDetached returned null");
    }
    return result;
  }

  public static boolean verifyDetached(
      final SigningAlgorithm algorithm,
      final byte[] publicKey,
      final byte[] message,
      final byte[] signature) {
    if (publicKey == null || publicKey.length == 0) {
      throw new IllegalArgumentException("publicKey must not be empty");
    }
    if (message == null || message.length == 0) {
      throw new IllegalArgumentException("message must not be empty");
    }
    if (signature == null || signature.length == 0) {
      throw new IllegalArgumentException("signature must not be empty");
    }
    requireNative();
    return nativeVerifyDetached(algorithm.bridgeCode(), publicKey, message, signature);
  }

  private static void requireNative() {
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException(LIBRARY_NAME + " is not available in this runtime");
    }
  }

  private static boolean loadLibrary() {
    try {
      System.loadLibrary(LIBRARY_NAME);
      return true;
    } catch (final UnsatisfiedLinkError | SecurityException error) {
      return false;
    }
  }

  private static native byte[] nativePublicKeyFromPrivate(int algorithmCode, byte[] privateKey);

  private static native byte[][] nativeKeypairFromSeed(int algorithmCode, byte[] seed);

  private static native byte[] nativeSignDetached(int algorithmCode, byte[] privateKey, byte[] message);

  private static native boolean nativeVerifyDetached(
      int algorithmCode, byte[] publicKey, byte[] message, byte[] signature);

  /** Raw keypair bytes returned by the bridge. */
  public record KeypairBytes(byte[] privateKey, byte[] publicKey) {}
}
