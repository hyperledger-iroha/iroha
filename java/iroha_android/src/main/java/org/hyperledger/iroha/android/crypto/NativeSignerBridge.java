package org.hyperledger.iroha.android.crypto;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import kotlin.Pair;
import org.hyperledger.iroha.android.model.FeeChargeLimit;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.model.instructions.RegisterZkAssetInstruction;

/**
 * Java view of the {@code connect_norito_bridge} signing helpers.
 *
 * <p>The bridge exports its JNI entry points only for the Kotlin SDK class {@link
 * org.hyperledger.iroha.sdk.crypto.NativeSignerBridge}; the duplicate {@code
 * org.hyperledger.iroha.android} exports are retired. This class keeps the Java argument checks
 * and exceptions, then delegates every native call to the Kotlin owner.
 */
public final class NativeSignerBridge {
  private static final String LIBRARY_NAME = "connect_norito_bridge";
  public static final int REQUIRED_BRIDGE_ABI_VERSION =
      org.hyperledger.iroha.sdk.crypto.NativeSignerBridge.REQUIRED_BRIDGE_ABI_VERSION;
  public static final int REQUIRED_NATIVE_SIGNER_CONTRACT_REVISION =
      org.hyperledger.iroha.sdk.crypto.NativeSignerBridge.REQUIRED_NATIVE_SIGNER_CONTRACT_REVISION;

  private NativeSignerBridge() {}

  public static boolean isNativeAvailable() {
    return org.hyperledger.iroha.sdk.crypto.NativeSignerBridge.isNativeAvailable();
  }

  public static byte[] publicKeyFromPrivate(
      final SigningAlgorithm algorithm, final byte[] privateKey) {
    if (privateKey == null || privateKey.length == 0) {
      throw new IllegalArgumentException("privateKey must not be empty");
    }
    requireNative();
    return org.hyperledger.iroha.sdk.crypto.NativeSignerBridge.publicKeyFromPrivate(
        kotlinAlgorithm(algorithm), privateKey);
  }

  public static KeypairBytes keypairFromSeed(
      final SigningAlgorithm algorithm, final byte[] seed) {
    if (seed == null || seed.length == 0) {
      throw new IllegalArgumentException("seed must not be empty");
    }
    requireNative();
    final Pair<byte[], byte[]> result =
        org.hyperledger.iroha.sdk.crypto.NativeSignerBridge.keypairFromSeed(
            kotlinAlgorithm(algorithm), seed);
    return new KeypairBytes(result.getFirst(), result.getSecond());
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
    return org.hyperledger.iroha.sdk.crypto.NativeSignerBridge.signDetached(
        kotlinAlgorithm(algorithm), privateKey, message);
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
    return org.hyperledger.iroha.sdk.crypto.NativeSignerBridge.verifyDetached(
        kotlinAlgorithm(algorithm), publicKey, message, signature);
  }

  public static NativeSignedTransaction encodeRegisterZkAssetSignedTransaction(
      final SigningAlgorithm algorithm,
      final NetworkId networkId,
      final int chainDiscriminant,
      final String authority,
      final long creationTimeMs,
      final RegisterZkAssetInstruction instruction,
      final byte[] privateKey,
      final FeePaymentIntent feePayment) {
    return encodeRegisterZkAssetSignedTransaction(
        algorithm,
        networkId,
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
      final NetworkId networkId,
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
    final byte[] networkIdBytes =
        Objects.requireNonNull(networkId, "networkId").bytes();
    textBytes(authority, "authority");
    textBytes(instruction.asset(), "asset");
    Objects.requireNonNull(feePayment, "feePayment");
    ttlValue(ttlMs);
    requireNative();
    final org.hyperledger.iroha.sdk.crypto.NativeSignedTransaction signed =
        org.hyperledger.iroha.sdk.crypto.NativeSignerBridge.encodeRegisterZkAssetSignedTransaction(
            kotlinAlgorithm(algorithm),
            org.hyperledger.iroha.sdk.core.model.NetworkId.fromBytes(networkIdBytes),
            validatedChainDiscriminant,
            authority,
            creationTimeMs,
            ttlMs,
            org.hyperledger.iroha.sdk.core.model.instructions.RegisterZkAssetInstruction.builder()
                .setAsset(instruction.asset())
                .setUnshieldVerifyingKey(instruction.unshieldVerifyingKey())
                .build(),
            key,
            kotlinFeePayment(feePayment));
    return new NativeSignedTransaction(
        signed.versionedSignedTransactionBytes(), signed.transactionHashBytes());
  }

  private static void requireNative() {
    if (!isNativeAvailable()) {
      throw new IllegalStateException(LIBRARY_NAME + " is not available in this runtime");
    }
  }

  private static org.hyperledger.iroha.sdk.crypto.SigningAlgorithm kotlinAlgorithm(
      final SigningAlgorithm algorithm) {
    return org.hyperledger.iroha.sdk.crypto.SigningAlgorithm.valueOf(
        Objects.requireNonNull(algorithm, "algorithm").name());
  }

  /** The same fee intent in the Kotlin model; both encode the identical Norito JSON. */
  static org.hyperledger.iroha.sdk.core.model.FeePaymentIntent kotlinFeePayment(
      final FeePaymentIntent value) {
    final List<org.hyperledger.iroha.sdk.core.model.FeeChargeLimit> limits = new ArrayList<>();
    for (final FeeChargeLimit limit : value.chargeLimits()) {
      limits.add(
          new org.hyperledger.iroha.sdk.core.model.FeeChargeLimit(
              org.hyperledger.iroha.sdk.core.model.FeeChargeKind.valueOf(limit.kind().name()),
              limit.assetDefinitionId(),
              limit.maxAmount()));
    }
    if (value instanceof FeePaymentIntent.Sponsor sponsor) {
      return org.hyperledger.iroha.sdk.core.model.FeePaymentIntent.sponsor(
          org.hyperledger.iroha.sdk.core.model.FeeSponsorProgramId.parse(
              sponsor.programId().literal()),
          sponsor.programRevision(),
          limits,
          value.gasLimit());
    }
    return org.hyperledger.iroha.sdk.core.model.FeePaymentIntent.authority(
        limits, value.gasLimit());
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

  /** Raw keypair bytes returned by the bridge. */
  public record KeypairBytes(byte[] privateKey, byte[] publicKey) {}
}
