package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;

/** Production JNI-backed verifier for restricted atomic-private-settlement responses. */
public final class AtomicPrivateSettlementNativeResponseVerifierV1
    implements AtomicPrivateSettlementResponseVerifierV1 {
  private static final String LIBRARY_NAME = "connect_norito_bridge";
  private static final int REQUIRED_BRIDGE_ABI_VERSION = 23;
  private static final int HASH_BYTES = 32;
  private static final int RESPONSE_MAX_BYTES = 32 * 1024 * 1024;
  private static final int APPROVAL_REQUEST_MAX_BYTES = 1024 * 1024;
  private static final int PUBLIC_KEY_MAX_BYTES = 1024;
  private static final int PRIVATE_SETTLEMENT_REJECTED_STATUS = -507;
  private static final AtomicPrivateSettlementNativeResponseVerifierV1 INSTANCE =
      new AtomicPrivateSettlementNativeResponseVerifierV1();
  private static final boolean NATIVE_AVAILABLE;

  static {
    boolean available = false;
    try {
      System.loadLibrary(LIBRARY_NAME);
      final byte[] invalid = new byte[0];
      available =
          nativeBridgeAbiVersion() == REQUIRED_BRIDGE_ABI_VERSION
              && nativeVerifyCommitteeProofResponseV1(invalid, invalid, invalid)
                  == PRIVATE_SETTLEMENT_REJECTED_STATUS
              && nativeVerifyAuditorCapsuleResponseV1(invalid, invalid, invalid, invalid)
                  == PRIVATE_SETTLEMENT_REJECTED_STATUS
              && nativeVerifyAuditApprovalResponseV1(
                      invalid, invalid, invalid, invalid, invalid)
                  == PRIVATE_SETTLEMENT_REJECTED_STATUS;
    } catch (final RuntimeException | LinkageError ignored) {
      // Availability is reported through the fixed fail-closed exception below.
    }
    NATIVE_AVAILABLE = available;
  }

  private AtomicPrivateSettlementNativeResponseVerifierV1() {}

  /** Return the shared immutable production verifier. */
  public static AtomicPrivateSettlementNativeResponseVerifierV1 instance() {
    return INSTANCE;
  }

  @Override
  public void requireAvailable() {
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException(
          "native private settlement response verifier is unavailable");
    }
  }

  @Override
  public void verifyCommitteeProofResponse(
      final byte[] responseJson,
      final byte[] expectedNetworkId,
      final byte[] requestedPayloadDigest) {
    requireCommonInputs(responseJson, expectedNetworkId, requestedPayloadDigest);
    invokeRequiredNative(
        () ->
            nativeVerifyCommitteeProofResponseV1(
                responseJson.clone(), expectedNetworkId.clone(), requestedPayloadDigest.clone()));
  }

  @Override
  public void verifyAuditorCapsuleResponse(
      final byte[] responseJson,
      final byte[] expectedNetworkId,
      final byte[] requestedPayloadDigest,
      final String auditorPublicKey) {
    requireCommonInputs(responseJson, expectedNetworkId, requestedPayloadDigest);
    final byte[] auditorPublicKeyUtf8 = requireAuditorPublicKey(auditorPublicKey);
    invokeRequiredNative(
        () ->
            nativeVerifyAuditorCapsuleResponseV1(
                responseJson.clone(),
                expectedNetworkId.clone(),
                requestedPayloadDigest.clone(),
                auditorPublicKeyUtf8.clone()));
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
    final byte[] auditorPublicKeyUtf8 = requireAuditorPublicKey(auditorPublicKey);
    invokeRequiredNative(
        () ->
            nativeVerifyAuditApprovalResponseV1(
                responseJson.clone(),
                requestJson.clone(),
                expectedNetworkId.clone(),
                requestedPayloadDigest.clone(),
                auditorPublicKeyUtf8.clone()));
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

  private static byte[] requireAuditorPublicKey(final String value) {
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
    return utf8;
  }

  private void invokeRequiredNative(final NativeStatusCall invocation) {
    requireAvailable();
    final int status;
    try {
      status = invocation.invoke();
    } catch (final LinkageError ignored) {
      throw new IllegalStateException(
          "native private settlement response verifier is unavailable");
    }
    if (status != 0) {
      throw new IllegalStateException(
          "native private settlement response verification rejected");
    }
  }

  private interface NativeStatusCall {
    int invoke();
  }

  private static native int nativeBridgeAbiVersion();

  private static native int nativeVerifyCommitteeProofResponseV1(
      byte[] responseJson, byte[] expectedNetworkId, byte[] requestedPayloadDigest);

  private static native int nativeVerifyAuditorCapsuleResponseV1(
      byte[] responseJson,
      byte[] expectedNetworkId,
      byte[] requestedPayloadDigest,
      byte[] auditorPublicKeyUtf8);

  private static native int nativeVerifyAuditApprovalResponseV1(
      byte[] responseJson,
      byte[] requestJson,
      byte[] expectedNetworkId,
      byte[] requestedPayloadDigest,
      byte[] auditorPublicKeyUtf8);
}
