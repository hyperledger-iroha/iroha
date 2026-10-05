package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;

/**
 * Production native verifier for restricted atomic-private-settlement responses.
 *
 * <p>{@code connect_norito_bridge} exports these JNI entry points only for the Kotlin SDK object
 * {@link org.hyperledger.iroha.sdk.client.AtomicPrivateSettlementNativeResponseVerifierV1}; the
 * duplicate {@code org.hyperledger.iroha.android} exports are retired. This class keeps the Java
 * argument checks and delegates verification to the Kotlin owner.
 */
public final class AtomicPrivateSettlementNativeResponseVerifierV1
    implements AtomicPrivateSettlementResponseVerifierV1 {
  private static final int HASH_BYTES = 32;
  private static final int RESPONSE_MAX_BYTES = 32 * 1024 * 1024;
  private static final int APPROVAL_REQUEST_MAX_BYTES = 1024 * 1024;
  private static final int PUBLIC_KEY_MAX_BYTES = 1024;
  private static final AtomicPrivateSettlementNativeResponseVerifierV1 INSTANCE =
      new AtomicPrivateSettlementNativeResponseVerifierV1();
  private static final org.hyperledger.iroha.sdk.client.AtomicPrivateSettlementResponseVerifierV1
      NATIVE = org.hyperledger.iroha.sdk.client.AtomicPrivateSettlementNativeResponseVerifierV1
          .INSTANCE;

  private AtomicPrivateSettlementNativeResponseVerifierV1() {}

  /** Return the shared immutable production verifier. */
  public static AtomicPrivateSettlementNativeResponseVerifierV1 instance() {
    return INSTANCE;
  }

  @Override
  public void requireAvailable() {
    NATIVE.requireAvailable();
  }

  @Override
  public void verifyCommitteeProofResponse(
      final byte[] responseJson,
      final byte[] expectedNetworkId,
      final byte[] requestedPayloadDigest) {
    requireCommonInputs(responseJson, expectedNetworkId, requestedPayloadDigest);
    NATIVE.verifyCommitteeProofResponse(responseJson, expectedNetworkId, requestedPayloadDigest);
  }

  @Override
  public void verifyAuditorCapsuleResponse(
      final byte[] responseJson,
      final byte[] requestJson,
      final byte[] expectedNetworkId,
      final byte[] requestedPayloadDigest,
      final String auditorPublicKey) {
    requireCommonInputs(responseJson, expectedNetworkId, requestedPayloadDigest);
    if (requestJson == null
        || requestJson.length == 0
        || requestJson.length > APPROVAL_REQUEST_MAX_BYTES) {
      throw new IllegalArgumentException(
          "private settlement auditor capsule request is outside the native verification bound");
    }
    requireAuditorPublicKey(auditorPublicKey);
    NATIVE.verifyAuditorCapsuleResponse(
        responseJson, requestJson, expectedNetworkId, requestedPayloadDigest, auditorPublicKey);
  }

  @Override
  public void verifyAuditApprovalResponse(
      final byte[] responseJson,
      final byte[] requestJson,
      final byte[] expectedNetworkId,
      final byte[] requestedPayloadDigest,
      final String auditorPublicKey) {
    requireCommonInputs(responseJson, expectedNetworkId, requestedPayloadDigest);
    if (requestJson == null
        || requestJson.length == 0
        || requestJson.length > APPROVAL_REQUEST_MAX_BYTES) {
      throw new IllegalArgumentException(
          "private settlement approval request is outside the native verification bound");
    }
    requireAuditorPublicKey(auditorPublicKey);
    NATIVE.verifyAuditApprovalResponse(
        responseJson, requestJson, expectedNetworkId, requestedPayloadDigest, auditorPublicKey);
  }

  private static void requireCommonInputs(
      final byte[] responseJson,
      final byte[] expectedNetworkId,
      final byte[] requestedPayloadDigest) {
    if (responseJson == null
        || responseJson.length == 0
        || responseJson.length > RESPONSE_MAX_BYTES) {
      throw new IllegalArgumentException(
          "private settlement response is outside the native verification bound");
    }
    if (expectedNetworkId == null || expectedNetworkId.length != HASH_BYTES) {
      throw new IllegalArgumentException(
          "private settlement network identity must contain exactly 32 bytes");
    }
    if (requestedPayloadDigest == null || requestedPayloadDigest.length != HASH_BYTES) {
      throw new IllegalArgumentException(
          "private settlement payload digest must contain exactly 32 bytes");
    }
  }

  private static void requireAuditorPublicKey(final String value) {
    if (value == null || value.isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(
          "private settlement auditor public key must be exact and non-empty");
    }
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (character < 0x21 || character > 0x7e) {
        throw new IllegalArgumentException(
            "private settlement auditor public key must be printable ASCII");
      }
    }
    final byte[] utf8 = value.getBytes(StandardCharsets.UTF_8);
    if (utf8.length > PUBLIC_KEY_MAX_BYTES) {
      throw new IllegalArgumentException(
          "private settlement auditor public key exceeds the native verification bound");
    }
  }
}
