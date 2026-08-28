package org.hyperledger.iroha.android.validationfee;

import java.nio.charset.StandardCharsets;
import java.util.Objects;

/**
 * Native Norito boundary for the live Hijiri validation-fee quote route.
 *
 * <p>No managed JSON encoder or decoder participates in the wire exchange. The native bridge
 * creates the exact request and validates canonical response encoding, request echoes, live
 * next-height semantics, all hashes, and fee arithmetic before returning a typed projection.
 */
public final class ValidationFeeHijiriQuoteBridge {
  private static final String LIBRARY_NAME = "connect_norito_bridge";
  private static final int REQUIRED_BRIDGE_ABI_VERSION = 23;

  private static boolean loadAttempted;
  private static Throwable loadFailure;

  private ValidationFeeHijiriQuoteBridge() {}

  /** Encodes an exact canonical bare-Norito V1 request. */
  public static byte[] encodeRequestV1(final ValidationFeeHijiriQuoteRequestV1 request) {
    final ValidationFeeHijiriQuoteRequestV1 exact = Objects.requireNonNull(request, "request");
    requireNative();
    final byte[] encoded =
        invokeRequiredQuoteNative(
            "nativeEncodeRequestV1",
            () ->
                nativeEncodeRequestV1(
                    exact.accountId().getBytes(StandardCharsets.UTF_8),
                    exact.qualifyingTransferCount()));
    if (encoded == null
        || encoded.length == 0
        || encoded.length > ValidationFeeHijiriQuoteRequestV1.MAX_REQUEST_BYTES) {
      throw new IllegalStateException(
          "native Hijiri quote encoder returned an invalid request size");
    }
    return encoded.clone();
  }

  /**
   * Verifies a canonical Norito response against the exact request bytes sent to Torii.
   *
   * <p>The returned object is constructed only after native semantic validation succeeds.
   */
  public static ValidationFeeHijiriQuoteV1 verifyResponseV1(
      final byte[] responseNorito, final byte[] requestNorito) {
    final byte[] response = Objects.requireNonNull(responseNorito, "responseNorito");
    final byte[] request = Objects.requireNonNull(requestNorito, "requestNorito");
    if (response.length == 0 || response.length > ValidationFeeHijiriQuoteV1.MAX_RESPONSE_BYTES) {
      throw new IllegalArgumentException(
          "responseNorito must contain 1.."
              + ValidationFeeHijiriQuoteV1.MAX_RESPONSE_BYTES
              + " bytes");
    }
    if (request.length == 0
        || request.length > ValidationFeeHijiriQuoteRequestV1.MAX_REQUEST_BYTES) {
      throw new IllegalArgumentException(
          "requestNorito must contain 1.."
              + ValidationFeeHijiriQuoteRequestV1.MAX_REQUEST_BYTES
              + " bytes");
    }
    requireNative();
    final byte[] projection =
        invokeRequiredQuoteNative(
            "nativeVerifyResponseV1",
            () -> nativeVerifyResponseV1(response.clone(), request.clone()));
    if (projection == null) {
      throw new IllegalStateException("native Hijiri quote verifier returned null");
    }
    return ValidationFeeHijiriQuoteV1.parseVerifiedProjection(projection.clone());
  }

  @FunctionalInterface
  interface RequiredQuoteNativeCall {
    byte[] invoke();
  }

  static byte[] invokeRequiredQuoteNative(
      final String method, final RequiredQuoteNativeCall invocation) {
    try {
      return invocation.invoke();
    } catch (final UnsatisfiedLinkError failure) {
      throw new IllegalStateException(
          "native Hijiri validation-fee quote bridge is unavailable: required ABI-23 method "
              + method
              + " is missing",
          failure);
    }
  }

  private static synchronized void requireNative() {
    if (!loadAttempted) {
      loadAttempted = true;
      try {
        System.loadLibrary(LIBRARY_NAME);
        final int actualAbi = nativeBridgeAbiVersion();
        if (actualAbi != REQUIRED_BRIDGE_ABI_VERSION) {
          throw new IllegalStateException(
              "native Hijiri quote bridge ABI mismatch: expected "
                  + REQUIRED_BRIDGE_ABI_VERSION
                  + ", found "
                  + actualAbi);
        }
      } catch (final Throwable failure) {
        loadFailure = failure;
      }
    }
    if (loadFailure != null) {
      throw new IllegalStateException(
          "native Hijiri validation-fee quote bridge is unavailable", loadFailure);
    }
  }

  private static native int nativeBridgeAbiVersion();

  private static native byte[] nativeEncodeRequestV1(
      byte[] accountIdUtf8, int qualifyingTransferCount);

  private static native byte[] nativeVerifyResponseV1(
      byte[] responseNorito, byte[] requestNorito);
}
