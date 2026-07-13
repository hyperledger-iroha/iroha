package org.hyperledger.iroha.android.sorafs;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;

/** Thin JVM/JNI wrapper around the SoraFS reference validators in {@code connect_norito_bridge}. */
public final class SorafsReferenceValidators {
  private static final String LIBRARY_NAME = "connect_norito_bridge";
  public static final int REQUIRED_BRIDGE_ABI_VERSION = 19;
  private static final boolean NATIVE_AVAILABLE = loadLibrary();

  private SorafsReferenceValidators() {}

  /** Returns true when the native bridge is present and new enough for SoraFS validation. */
  public static boolean isNativeAvailable() {
    return NATIVE_AVAILABLE;
  }

  static boolean isBridgeAbiSupported(final int abiVersion) {
    return abiVersion >= REQUIRED_BRIDGE_ABI_VERSION;
  }

  public static String validateOrderbookPayloadJson(
      final SorafsOrderbookPayloadKind kind, final byte[] noritoBytes) {
    return validateOrderbookPayloadJson(kind, noritoBytes, null, currentEpochSeconds());
  }

  public static String validateOrderbookPayloadJson(
      final SorafsOrderbookPayloadKind kind, final byte[] noritoBytes, final String label) {
    return validateOrderbookPayloadJson(kind, noritoBytes, label, currentEpochSeconds());
  }

  public static String validateOrderbookPayloadJson(
      final SorafsOrderbookPayloadKind kind,
      final byte[] noritoBytes,
      final String label,
      final long generatedAtUnix) {
    requireGeneratedAt(generatedAtUnix);
    final SorafsOrderbookPayloadKind selected = requireKind(kind, "kind");
    final byte[] payload = requirePayload(noritoBytes, "noritoBytes");
    final byte[] labelPayload = labelBytes(label, selected.defaultLabel());
    requireNative();
    return requireJsonOutput(
        nativeValidateOrderbookPayloadJson(
            selected.bridgeCode(),
            payload,
            labelPayload,
            generatedAtUnix),
        "SoraFS orderbook validation");
  }

  public static String validatePopPayloadJson(
      final SorafsPopPayloadKind kind, final byte[] noritoBytes) {
    return validatePopPayloadJson(kind, noritoBytes, null, currentEpochSeconds());
  }

  public static String validatePopPayloadJson(
      final SorafsPopPayloadKind kind, final byte[] noritoBytes, final String label) {
    return validatePopPayloadJson(kind, noritoBytes, label, currentEpochSeconds());
  }

  public static String validatePopPayloadJson(
      final SorafsPopPayloadKind kind,
      final byte[] noritoBytes,
      final String label,
      final long generatedAtUnix) {
    requireGeneratedAt(generatedAtUnix);
    final SorafsPopPayloadKind selected = requireKind(kind, "kind");
    final byte[] payload = requirePayload(noritoBytes, "noritoBytes");
    final byte[] labelPayload = labelBytes(label, selected.defaultLabel());
    requireNative();
    return requireJsonOutput(
        nativeValidatePopPayloadJson(
            selected.bridgeCode(),
            payload,
            labelPayload,
            generatedAtUnix),
        "SoraFS PoP validation");
  }

  public static String validateHedgingPayloadJson(
      final SorafsHedgingPayloadKind kind, final byte[] noritoBytes) {
    return validateHedgingPayloadJson(kind, noritoBytes, null, currentEpochSeconds());
  }

  public static String validateHedgingPayloadJson(
      final SorafsHedgingPayloadKind kind, final byte[] noritoBytes, final String label) {
    return validateHedgingPayloadJson(kind, noritoBytes, label, currentEpochSeconds());
  }

  public static String validateHedgingPayloadJson(
      final SorafsHedgingPayloadKind kind,
      final byte[] noritoBytes,
      final String label,
      final long generatedAtUnix) {
    requireGeneratedAt(generatedAtUnix);
    final SorafsHedgingPayloadKind selected = requireKind(kind, "kind");
    final byte[] payload = requirePayload(noritoBytes, "noritoBytes");
    final byte[] labelPayload = labelBytes(label, selected.defaultLabel());
    requireNative();
    return requireJsonOutput(
        nativeValidateHedgingPayloadJson(
            selected.bridgeCode(),
            payload,
            labelPayload,
            generatedAtUnix),
        "SoraFS hedging validation");
  }

  public static byte[] signOrderbookPayload(
      final SorafsOrderbookPayloadKind kind, final byte[] noritoBytes, final byte[] privateKey) {
    final SorafsOrderbookPayloadKind selected = requireUserSignedOrderbookKind(kind);
    final byte[] payload = requirePayload(noritoBytes, "noritoBytes");
    final byte[] key = requirePrivateKey(privateKey);
    try {
      requireNative();
      return requireBytesOutput(
          nativeSignOrderbookPayload(selected.bridgeCode(), payload, key),
          "SoraFS orderbook signing");
    } finally {
      Arrays.fill(key, (byte) 0);
    }
  }

  /** Derive the canonical V1 order id from owner-account bytes and nonce. */
  public static byte[] deriveOrderbookOrderId(final byte[] ownerAccount, final long nonce) {
    final byte[] ownerBytes = requireNonEmptyBytes(ownerAccount, "ownerAccount");
    requirePositive(nonce, "nonce");
    requireNative();
    final byte[] orderId =
        requireBytesOutput(
            nativeDeriveOrderbookOrderId(ownerBytes, nonce),
            "SoraFS orderbook order id derivation");
    if (orderId.length != 32) {
      throw new IllegalStateException(
          "SoraFS orderbook order id derivation returned a non-32-byte identifier");
    }
    return orderId;
  }

  public static byte[] buildSignedOrderbookOrderRequest(
      final SorafsOrderbookSide side,
      final SorafsOrderbookTier tier,
      final String pricePerGib,
      final long quantityGib,
      final byte[] ownerAccount,
      final long expiryUnix,
      final long nonce,
      final int makerFeeBps,
      final int takerFeeBps,
      final byte[] privateKey) {
    return buildSignedOrderbookOrderRequest(
        side,
        tier,
        pricePerGib,
        quantityGib,
        quantityGib,
        ownerAccount,
        expiryUnix,
        nonce,
        makerFeeBps,
        takerFeeBps,
        privateKey);
  }

  public static byte[] buildSignedOrderbookOrderRequest(
      final SorafsOrderbookSide side,
      final SorafsOrderbookTier tier,
      final String pricePerGib,
      final long quantityGib,
      final long remainingGib,
      final byte[] ownerAccount,
      final long expiryUnix,
      final long nonce,
      final int makerFeeBps,
      final int takerFeeBps,
      final byte[] privateKey) {
    return buildSignedOrderbookOrderRequest(
        deriveOrderbookOrderId(ownerAccount, nonce),
        side,
        tier,
        pricePerGib,
        quantityGib,
        remainingGib,
        ownerAccount,
        expiryUnix,
        nonce,
        makerFeeBps,
        takerFeeBps,
        privateKey);
  }

  public static byte[] buildSignedOrderbookOrderRequest(
      final byte[] orderId,
      final SorafsOrderbookSide side,
      final SorafsOrderbookTier tier,
      final String pricePerGib,
      final long quantityGib,
      final byte[] ownerAccount,
      final long expiryUnix,
      final long nonce,
      final int makerFeeBps,
      final int takerFeeBps,
      final byte[] privateKey) {
    return buildSignedOrderbookOrderRequest(
        orderId,
        side,
        tier,
        pricePerGib,
        quantityGib,
        quantityGib,
        ownerAccount,
        expiryUnix,
        nonce,
        makerFeeBps,
        takerFeeBps,
        privateKey);
  }

  public static byte[] buildSignedOrderbookOrderRequest(
      final byte[] orderId,
      final SorafsOrderbookSide side,
      final SorafsOrderbookTier tier,
      final String pricePerGib,
      final long quantityGib,
      final long remainingGib,
      final byte[] ownerAccount,
      final long expiryUnix,
      final long nonce,
      final int makerFeeBps,
      final int takerFeeBps,
      final byte[] privateKey) {
    final byte[] orderIdBytes = requireFixed32(orderId, "orderId");
    final SorafsOrderbookSide selectedSide = requireKind(side, "side");
    final SorafsOrderbookTier selectedTier = requireKind(tier, "tier");
    final byte[] priceBytes =
        xorQuantityBytes(pricePerGib, "pricePerGib", true);
    requirePositive(quantityGib, "quantityGib");
    requirePositive(remainingGib, "remainingGib");
    final byte[] ownerBytes = requireNonEmptyBytes(ownerAccount, "ownerAccount");
    requirePositive(expiryUnix, "expiryUnix");
    requirePositive(nonce, "nonce");
    final byte[] canonicalOrderId = deriveOrderbookOrderId(ownerBytes, nonce);
    if (!Arrays.equals(orderIdBytes, canonicalOrderId)) {
      throw new IllegalArgumentException(
          "orderId must equal the canonical owner-and-nonce derivation");
    }
    final int makerFee = requireFeeBps(makerFeeBps, "makerFeeBps");
    final int takerFee = requireFeeBps(takerFeeBps, "takerFeeBps");
    final byte[] key = requirePrivateKey(privateKey);
    try {
      requireNative();
      return requireBytesOutput(
          nativeBuildSignedOrderbookOrderRequest(
              orderIdBytes,
              selectedSide.bridgeCode(),
              selectedTier.bridgeCode(),
              priceBytes,
              quantityGib,
              remainingGib,
              ownerBytes,
              expiryUnix,
              nonce,
              makerFee,
              takerFee,
              key),
          "SoraFS orderbook order request builder");
    } finally {
      Arrays.fill(key, (byte) 0);
    }
  }

  public static byte[] buildSignedOrderbookOrderCancel(
      final byte[] orderId,
      final byte[] ownerAccount,
      final SorafsOrderbookCancelReason reason,
      final long nonce,
      final byte[] privateKey) {
    final byte[] orderIdBytes = requireFixed32(orderId, "orderId");
    final byte[] ownerBytes = requireNonEmptyBytes(ownerAccount, "ownerAccount");
    final SorafsOrderbookCancelReason selectedReason = requireKind(reason, "reason");
    requirePositive(nonce, "nonce");
    final byte[] key = requirePrivateKey(privateKey);
    try {
      requireNative();
      return requireBytesOutput(
          nativeBuildSignedOrderbookOrderCancel(
              orderIdBytes, ownerBytes, selectedReason.bridgeCode(), nonce, key),
          "SoraFS orderbook cancel builder");
    } finally {
      Arrays.fill(key, (byte) 0);
    }
  }

  public static byte[] buildSignedOrderbookSettlementReceipt(
      final byte[] receiptId,
      final byte[] channelId,
      final byte[] tradeId,
      final long rangeStart,
      final long rangeEnd,
      final byte[] chunkHash,
      final long bytesDelivered,
      final String xorDebited,
      final String providerCredit,
      final String feeAmount,
      final long issuedAtUnix,
      final byte[] privateKey) {
    final byte[] receiptIdBytes = requireFixed32(receiptId, "receiptId");
    final byte[] channelIdBytes = requireFixed32(channelId, "channelId");
    final byte[] tradeIdBytes = requireFixed32(tradeId, "tradeId");
    requireNonNegative(rangeStart, "rangeStart");
    requirePositive(rangeEnd, "rangeEnd");
    final byte[] chunkHashBytes = requireFixed32(chunkHash, "chunkHash");
    requirePositive(bytesDelivered, "bytesDelivered");
    final byte[] debitBytes = xorQuantityBytes(xorDebited, "xorDebited", true);
    final byte[] creditBytes =
        xorQuantityBytes(providerCredit, "providerCredit", false);
    final byte[] feeBytes = xorQuantityBytes(feeAmount, "feeAmount", false);
    requirePositive(issuedAtUnix, "issuedAtUnix");
    final byte[] key = requirePrivateKey(privateKey);
    try {
      requireNative();
      return requireBytesOutput(
          nativeBuildSignedOrderbookSettlementReceipt(
              receiptIdBytes,
              channelIdBytes,
              tradeIdBytes,
              rangeStart,
              rangeEnd,
              chunkHashBytes,
              bytesDelivered,
              debitBytes,
              creditBytes,
              feeBytes,
              issuedAtUnix,
              key),
          "SoraFS orderbook settlement receipt builder");
    } finally {
      Arrays.fill(key, (byte) 0);
    }
  }

  public static String validatePdpPayloadJson(
      final SorafsPdpPayloadKind kind, final byte[] noritoBytes) {
    return validatePdpPayloadJson(kind, noritoBytes, null, currentEpochSeconds());
  }

  public static String validatePdpPayloadJson(
      final SorafsPdpPayloadKind kind, final byte[] noritoBytes, final String label) {
    return validatePdpPayloadJson(kind, noritoBytes, label, currentEpochSeconds());
  }

  public static String validatePdpPayloadJson(
      final SorafsPdpPayloadKind kind,
      final byte[] noritoBytes,
      final String label,
      final long generatedAtUnix) {
    requireGeneratedAt(generatedAtUnix);
    final SorafsPdpPayloadKind selected = requireKind(kind, "kind");
    final byte[] payload = requirePayload(noritoBytes, "noritoBytes");
    final byte[] labelPayload = labelBytes(label, selected.defaultLabel());
    requireNative();
    return requireJsonOutput(
        nativeValidatePdpPayloadJson(
            selected.bridgeCode(),
            payload,
            labelPayload,
            generatedAtUnix),
        "SoraFS PDP validation");
  }

  public static String validatePdpCommitmentChallengeJson(
      final byte[] commitment, final byte[] challenge) {
    return validatePdpCommitmentChallengeJson(
        commitment, challenge, null, null, currentEpochSeconds());
  }

  public static String validatePdpCommitmentChallengeJson(
      final byte[] commitment,
      final byte[] challenge,
      final String commitmentLabel,
      final String challengeLabel,
      final long generatedAtUnix) {
    requireGeneratedAt(generatedAtUnix);
    final byte[] commitmentPayload = requirePayload(commitment, "commitment");
    final byte[] commitmentLabelPayload =
        labelBytes(commitmentLabel, SorafsPdpPayloadKind.COMMITMENT.defaultLabel());
    final byte[] challengePayload = requirePayload(challenge, "challenge");
    final byte[] challengeLabelPayload =
        labelBytes(challengeLabel, SorafsPdpPayloadKind.CHALLENGE.defaultLabel());
    requireNative();
    return requireJsonOutput(
        nativeValidatePdpCommitmentChallengeJson(
            commitmentPayload,
            commitmentLabelPayload,
            challengePayload,
            challengeLabelPayload,
            generatedAtUnix),
        "SoraFS PDP commitment/challenge validation");
  }

  public static String validatePdpChallengeProofJson(
      final byte[] challenge, final byte[] proof) {
    return validatePdpChallengeProofJson(challenge, proof, null, null, currentEpochSeconds());
  }

  public static String validatePdpChallengeProofJson(
      final byte[] challenge,
      final byte[] proof,
      final String challengeLabel,
      final String proofLabel,
      final long generatedAtUnix) {
    requireGeneratedAt(generatedAtUnix);
    final byte[] challengePayload = requirePayload(challenge, "challenge");
    final byte[] challengeLabelPayload =
        labelBytes(challengeLabel, SorafsPdpPayloadKind.CHALLENGE.defaultLabel());
    final byte[] proofPayload = requirePayload(proof, "proof");
    final byte[] proofLabelPayload =
        labelBytes(proofLabel, SorafsPdpPayloadKind.PROOF.defaultLabel());
    requireNative();
    return requireJsonOutput(
        nativeValidatePdpChallengeProofJson(
            challengePayload,
            challengeLabelPayload,
            proofPayload,
            proofLabelPayload,
            generatedAtUnix),
        "SoraFS PDP challenge/proof validation");
  }

  public static String validatePdpBundleJson(
      final byte[] commitment, final byte[] challenge, final byte[] proof) {
    return validatePdpBundleJson(
        commitment, challenge, proof, null, null, null, currentEpochSeconds());
  }

  public static String validatePdpBundleJson(
      final byte[] commitment,
      final byte[] challenge,
      final byte[] proof,
      final String commitmentLabel,
      final String challengeLabel,
      final String proofLabel,
      final long generatedAtUnix) {
    requireGeneratedAt(generatedAtUnix);
    final byte[] commitmentPayload = requirePayload(commitment, "commitment");
    final byte[] commitmentLabelPayload =
        labelBytes(commitmentLabel, SorafsPdpPayloadKind.COMMITMENT.defaultLabel());
    final byte[] challengePayload = requirePayload(challenge, "challenge");
    final byte[] challengeLabelPayload =
        labelBytes(challengeLabel, SorafsPdpPayloadKind.CHALLENGE.defaultLabel());
    final byte[] proofPayload = requirePayload(proof, "proof");
    final byte[] proofLabelPayload =
        labelBytes(proofLabel, SorafsPdpPayloadKind.PROOF.defaultLabel());
    requireNative();
    return requireJsonOutput(
        nativeValidatePdpBundleJson(
            commitmentPayload,
            commitmentLabelPayload,
            challengePayload,
            challengeLabelPayload,
            proofPayload,
            proofLabelPayload,
            generatedAtUnix),
        "SoraFS PDP bundle validation");
  }

  private static long currentEpochSeconds() {
    return System.currentTimeMillis() / 1000L;
  }

  private static void requireGeneratedAt(final long generatedAtUnix) {
    if (generatedAtUnix < 0L) {
      throw new IllegalArgumentException("generatedAtUnix must be non-negative");
    }
  }

  private static void requireNative() {
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException(LIBRARY_NAME + " is not available in this runtime");
    }
  }

  private static <T> T requireKind(final T kind, final String field) {
    if (kind == null) {
      throw new IllegalArgumentException(field + " must be provided");
    }
    return kind;
  }

  private static SorafsOrderbookPayloadKind requireUserSignedOrderbookKind(
      final SorafsOrderbookPayloadKind kind) {
    final SorafsOrderbookPayloadKind selected = requireKind(kind, "kind");
    if (!selected.isUserSignedPayload()) {
      throw new IllegalArgumentException(
          "orderbook payload kind " + selected.name() + " cannot be signed");
    }
    return selected;
  }

  private static byte[] requirePayload(final byte[] payload, final String field) {
    if (payload == null) {
      throw new IllegalArgumentException(field + " must be provided");
    }
    return payload.clone();
  }

  private static byte[] requirePrivateKey(final byte[] privateKey) {
    if (privateKey == null) {
      throw new IllegalArgumentException("privateKey must be provided");
    }
    if (privateKey.length != 32) {
      throw new IllegalArgumentException("privateKey must be 32 bytes");
    }
    boolean nonZero = false;
    for (final byte value : privateKey) {
      if (value != 0) {
        nonZero = true;
        break;
      }
    }
    if (!nonZero) {
      throw new IllegalArgumentException("privateKey must not be all zero");
    }
    return privateKey.clone();
  }

  private static byte[] requireFixed32(final byte[] payload, final String field) {
    if (payload == null) {
      throw new IllegalArgumentException(field + " must be provided");
    }
    if (payload.length != 32) {
      throw new IllegalArgumentException(field + " must be 32 bytes");
    }
    return payload.clone();
  }

  private static byte[] requireNonEmptyBytes(final byte[] payload, final String field) {
    if (payload == null) {
      throw new IllegalArgumentException(field + " must be provided");
    }
    if (payload.length == 0) {
      throw new IllegalArgumentException(field + " must not be empty");
    }
    return payload.clone();
  }

  private static void requireNonNegative(final long value, final String field) {
    if (value < 0L) {
      throw new IllegalArgumentException(field + " must be non-negative");
    }
  }

  private static void requirePositive(final long value, final String field) {
    if (value <= 0L) {
      throw new IllegalArgumentException(field + " must be greater than zero");
    }
  }

  private static int requireFeeBps(final int value, final String field) {
    if (value < 0 || value > 0xFFFF) {
      throw new IllegalArgumentException(field + " must fit in u16 basis points");
    }
    return value;
  }

  private static byte[] xorQuantityBytes(
      final String value, final String field, final boolean positive) {
    if (value == null) {
      throw new IllegalArgumentException(field + " must be provided");
    }
    if (value.length() > 155) {
      throw new IllegalArgumentException(field + " exceeds the canonical XOR quantity text bound");
    }
    if (!value.matches("(0|[1-9][0-9]*)(?:\\.([0-9]*[1-9]))?")) {
      throw new IllegalArgumentException(
          field + " must be a canonical non-negative XOR quantity");
    }
    final int decimalIndex = value.indexOf('.');
    final String integer = decimalIndex < 0 ? value : value.substring(0, decimalIndex);
    final String fractional = decimalIndex < 0 ? "" : value.substring(decimalIndex + 1);
    if (fractional.length() > 9) {
      throw new IllegalArgumentException(
          field + " must have at most 9 fractional decimal places");
    }
    final BigInteger mantissa = new BigInteger(integer + fractional);
    final BigInteger maximum = BigInteger.ONE.shiftLeft(511).subtract(BigInteger.ONE);
    if (mantissa.compareTo(maximum) > 0) {
      throw new IllegalArgumentException(field + " exceeds the 512-bit signed quantity domain");
    }
    if (positive && mantissa.signum() == 0) {
      throw new IllegalArgumentException(field + " must be greater than zero");
    }
    return value.getBytes(StandardCharsets.UTF_8);
  }

  private static byte[] labelBytes(final String label, final String fallback) {
    final String value = label == null ? fallback : label;
    if (value.trim().isEmpty()) {
      throw new IllegalArgumentException("label must not be blank");
    }
    if (!value.trim().equals(value)) {
      throw new IllegalArgumentException("label must not contain surrounding whitespace");
    }
    if (value.indexOf('\0') >= 0) {
      throw new IllegalArgumentException("label must not contain NUL");
    }
    return value.getBytes(StandardCharsets.UTF_8);
  }

  private static String requireJsonOutput(final byte[] output, final String context) {
    if (output == null) {
      throw new IllegalStateException(context + " returned no outcome JSON");
    }
    if (output.length == 0) {
      throw new IllegalStateException(context + " returned empty outcome JSON");
    }
    final String json = new String(output, StandardCharsets.UTF_8);
    if (!json.trim().startsWith("{")) {
      throw new IllegalStateException(context + " returned malformed outcome JSON");
    }
    return json;
  }

  private static byte[] requireBytesOutput(final byte[] output, final String context) {
    if (output == null) {
      throw new IllegalStateException(context + " returned no bytes");
    }
    if (output.length == 0) {
      throw new IllegalStateException(context + " returned empty bytes");
    }
    return output.clone();
  }

  private static boolean loadLibrary() {
    try {
      System.loadLibrary(LIBRARY_NAME);
      return isBridgeAbiSupported(nativeBridgeAbiVersion());
    } catch (final UnsatisfiedLinkError | SecurityException error) {
      return false;
    }
  }

  private static native int nativeBridgeAbiVersion();

  private static native byte[] nativeValidateOrderbookPayloadJson(
      int kind, byte[] payload, byte[] label, long generatedAtUnix);

  private static native byte[] nativeValidatePopPayloadJson(
      int kind, byte[] payload, byte[] label, long generatedAtUnix);

  private static native byte[] nativeValidateHedgingPayloadJson(
      int kind, byte[] payload, byte[] label, long generatedAtUnix);

  private static native byte[] nativeSignOrderbookPayload(
      int kind, byte[] payload, byte[] privateKey);

  private static native byte[] nativeDeriveOrderbookOrderId(byte[] ownerAccount, long nonce);

  private static native byte[] nativeBuildSignedOrderbookOrderRequest(
      byte[] orderId,
      int side,
      int tier,
      byte[] pricePerGib,
      long quantityGib,
      long remainingGib,
      byte[] ownerAccount,
      long expiryUnix,
      long nonce,
      int makerFeeBps,
      int takerFeeBps,
      byte[] privateKey);

  private static native byte[] nativeBuildSignedOrderbookOrderCancel(
      byte[] orderId, byte[] ownerAccount, int reason, long nonce, byte[] privateKey);

  private static native byte[] nativeBuildSignedOrderbookSettlementReceipt(
      byte[] receiptId,
      byte[] channelId,
      byte[] tradeId,
      long rangeStart,
      long rangeEnd,
      byte[] chunkHash,
      long bytesDelivered,
      byte[] xorDebited,
      byte[] providerCredit,
      byte[] feeAmount,
      long issuedAtUnix,
      byte[] privateKey);

  private static native byte[] nativeValidatePdpPayloadJson(
      int kind, byte[] payload, byte[] label, long generatedAtUnix);

  private static native byte[] nativeValidatePdpCommitmentChallengeJson(
      byte[] commitment,
      byte[] commitmentLabel,
      byte[] challenge,
      byte[] challengeLabel,
      long generatedAtUnix);

  private static native byte[] nativeValidatePdpChallengeProofJson(
      byte[] challenge, byte[] challengeLabel, byte[] proof, byte[] proofLabel, long generatedAtUnix);

  private static native byte[] nativeValidatePdpBundleJson(
      byte[] commitment,
      byte[] commitmentLabel,
      byte[] challenge,
      byte[] challengeLabel,
      byte[] proof,
      byte[] proofLabel,
      long generatedAtUnix);
}
