package org.hyperledger.iroha.android.crypto;

import java.nio.charset.StandardCharsets;
import java.util.Objects;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.instructions.RegisterZkAssetInstruction;

/** Thin JVM/JNI wrapper around {@code connect_norito_bridge} signing helpers. */
public final class NativeSignerBridge {
  private static final String LIBRARY_NAME = "connect_norito_bridge";
  public static final int REQUIRED_BRIDGE_ABI_VERSION = 21;
  public static final int REQUIRED_NATIVE_SIGNER_CONTRACT_REVISION = 3;
  private static final int HASH_BYTES = 32;
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

  public static NativeSignedTransaction encodeRegisterZkAssetSignedTransaction(
      final SigningAlgorithm algorithm,
      final String chainId,
      final int chainDiscriminant,
      final String authority,
      final long creationTimeMs,
      final RegisterZkAssetInstruction instruction,
      final byte[] privateKey,
      final FeePaymentIntent feePayment) {
    return encodeRegisterZkAssetSignedTransaction(
        algorithm,
        chainId,
        chainDiscriminant,
        authority,
        creationTimeMs,
        null,
        instruction,
        privateKey,
        feePayment);
  }

  public static NativeSignedTransaction encodeRegisterZkAssetSignedTransaction(
      final SigningAlgorithm algorithm,
      final String chainId,
      final int chainDiscriminant,
      final String authority,
      final long creationTimeMs,
      final Long ttlMs,
      final RegisterZkAssetInstruction instruction,
      final byte[] privateKey,
      final FeePaymentIntent feePayment) {
    requireCreationTime(creationTimeMs);
    final int validatedChainDiscriminant = requireChainDiscriminant(chainDiscriminant);
    if (instruction == null) {
      throw new IllegalArgumentException("instruction must be provided");
    }
    final byte[] key = requirePrivateKey(privateKey);
    final byte[] chainBytes = textBytes(chainId, "chainId");
    final byte[] authorityBytes = textBytes(authority, "authority");
    final byte[] assetBytes = textBytes(instruction.asset(), "asset");
    final byte[] unshieldBytes = optionalTextBytes(instruction.unshieldVerifyingKey());
    final byte[] shieldBytes = optionalTextBytes(instruction.shieldVerifyingKey());
    final byte[] feePaymentJson = feePaymentJson(feePayment);
    final long ttl = ttlValue(ttlMs);
    final boolean hasTtl = ttlMs != null;
    requireNative();
    return requireNativeSignedOutput(
        nativeEncodeRegisterZkAssetSignedTransaction(
            algorithm.bridgeCode(),
            chainBytes,
            validatedChainDiscriminant,
            authorityBytes,
            creationTimeMs,
            ttl,
            hasTtl,
            assetBytes,
            instruction.mode().bridgeCode(),
            instruction.allowShield(),
            instruction.allowUnshield(),
            unshieldBytes,
            instruction.unshieldVerifyingKey() != null,
            shieldBytes,
            instruction.shieldVerifyingKey() != null,
            key,
            feePaymentJson),
        "encodeRegisterZkAssetSignedTransaction");
  }

  private static void requireNative() {
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException(LIBRARY_NAME + " is not available in this runtime");
    }
  }

  private static boolean loadLibrary() {
    try {
      System.loadLibrary(LIBRARY_NAME);
      return nativeBridgeAbiVersion() == REQUIRED_BRIDGE_ABI_VERSION
          && nativeSignerContractRevision() == REQUIRED_NATIVE_SIGNER_CONTRACT_REVISION;
    } catch (final UnsatisfiedLinkError | SecurityException error) {
      return false;
    }
  }

  private static NativeSignedTransaction requireNativeSignedOutput(
      final byte[][] output, final String context) {
    if (output == null || output.length != 2) {
      throw new IllegalArgumentException(context + " returned invalid output");
    }
    if (output[0] == null || output[0].length == 0) {
      throw new IllegalArgumentException(context + " returned empty transaction bytes");
    }
    if (output[1] == null || output[1].length != HASH_BYTES) {
      throw new IllegalArgumentException(context + " returned invalid hash bytes");
    }
    return new NativeSignedTransaction(output[0], output[1]);
  }

  private static byte[] textBytes(final String value, final String name) {
    if (value == null) {
      throw new IllegalArgumentException(name + " must be provided");
    }
    if (value.trim().isEmpty()) {
      throw new IllegalArgumentException(name + " must not be blank");
    }
    if (!value.trim().equals(value)) {
      throw new IllegalArgumentException(name + " must not contain surrounding whitespace");
    }
    if (value.indexOf('\0') >= 0) {
      throw new IllegalArgumentException(name + " must not contain NUL");
    }
    return value.getBytes(StandardCharsets.UTF_8);
  }

  private static byte[] optionalTextBytes(final String value) {
    return value == null ? new byte[0] : value.getBytes(StandardCharsets.UTF_8);
  }

  private static byte[] feePaymentJson(final FeePaymentIntent value) {
    return JsonEncoder.encode(Objects.requireNonNull(value, "feePayment").toJsonMap())
        .getBytes(StandardCharsets.UTF_8);
  }

  private static void requireCreationTime(final long creationTimeMs) {
    if (creationTimeMs < 0) {
      throw new IllegalArgumentException("creationTimeMs must be non-negative");
    }
  }

  private static long ttlValue(final Long ttlMs) {
    if (ttlMs == null) {
      return 0L;
    }
    if (ttlMs <= 0) {
      throw new IllegalArgumentException("ttlMs must be positive when provided");
    }
    return ttlMs;
  }

  private static int requireChainDiscriminant(final int value) {
    if (value < 0 || value > 0xffff) {
      throw new IllegalArgumentException("chainDiscriminant must fit in u16");
    }
    return value;
  }

  private static byte[] requirePrivateKey(final byte[] privateKey) {
    if (privateKey == null || privateKey.length == 0) {
      throw new IllegalArgumentException("privateKey must not be empty");
    }
    return privateKey.clone();
  }

  private static native int nativeBridgeAbiVersion();

  private static native int nativeSignerContractRevision();

  private static native byte[] nativePublicKeyFromPrivate(int algorithmCode, byte[] privateKey);

  private static native byte[][] nativeKeypairFromSeed(int algorithmCode, byte[] seed);

  private static native byte[] nativeSignDetached(int algorithmCode, byte[] privateKey, byte[] message);

  private static native boolean nativeVerifyDetached(
      int algorithmCode, byte[] publicKey, byte[] message, byte[] signature);

  private static native byte[][] nativeEncodeRegisterZkAssetSignedTransaction(
      int algorithmCode,
      byte[] chainId,
      int chainDiscriminant,
      byte[] authority,
      long creationTimeMs,
      long ttlMs,
      boolean ttlPresent,
      byte[] asset,
      int modeCode,
      boolean allowShield,
      boolean allowUnshield,
      byte[] unshieldVerifyingKey,
      boolean unshieldVerifyingKeyPresent,
      byte[] shieldVerifyingKey,
      boolean shieldVerifyingKeyPresent,
      byte[] privateKey,
      byte[] feePaymentJson);

  /** Raw keypair bytes returned by the bridge. */
  public record KeypairBytes(byte[] privateKey, byte[] publicKey) {}
}
