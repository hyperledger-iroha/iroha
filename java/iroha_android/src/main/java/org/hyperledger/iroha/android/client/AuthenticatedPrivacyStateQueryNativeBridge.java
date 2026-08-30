// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.client;

import java.security.SecureRandom;
import java.util.Arrays;
import java.util.Objects;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.sdk.privacy.PrivacyFinalizedStateProjectionV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyFinalizedStateRequestV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyFinalizedStateViewV1;

/** ABI-22 native codec and verifier for authenticated finalized privacy-state IDs 97 through 104. */
public final class AuthenticatedPrivacyStateQueryNativeBridge {
  public static final int REQUIRED_BRIDGE_ABI_VERSION = 22;
  public static final long RESPONSE_MAX_BYTES = 256L * 1024L;
  private static final int DIGEST_BYTES = 32;
  private static final int NONCE_BYTES = 32;
  private static final int PREPARATION_MAX_BYTES = 64 * 1024;
  private static final int SIGNATURE_MAX_BYTES = 16 * 1024;
  private static final int SIGNED_QUERY_MAX_BYTES = 64 * 1024;
  private static final SecureRandom NONCE_RANDOM = new SecureRandom();

  private AuthenticatedPrivacyStateQueryNativeBridge() {}

  /** Native-bound signed ID97-104 body plus the private preparation used to verify its result. */
  public static final class SignedQueryV1 {
    private final byte[] preparation;
    private final byte[] requestBody;
    private final PrivacyFinalizedStateRequestV1 request;
    private final org.hyperledger.iroha.sdk.core.model.NetworkId sdkNetworkId;

    private SignedQueryV1(
        final byte[] preparation,
        final byte[] requestBody,
        final PrivacyFinalizedStateRequestV1 request,
        final org.hyperledger.iroha.sdk.core.model.NetworkId sdkNetworkId) {
      this.preparation = preparation.clone();
      this.requestBody = requestBody.clone();
      this.request = Objects.requireNonNull(request, "request");
      this.sdkNetworkId = Objects.requireNonNull(sdkNetworkId, "sdkNetworkId");
    }

    /** Canonical versioned {@code SignedQuery} bytes for exact {@code POST /v1/query}. */
    public byte[] requestBody() {
      return requestBody.clone();
    }

    private byte[] preparation() {
      return preparation.clone();
    }
  }

  /** Builds and opaquely signs one fresh member of the closed ID97-104 query union. */
  public static SignedQueryV1 buildSignedPrivacyStateQueryV1(
      final PrivacyFinalizedStateRequestV1 request,
      final NetworkId networkId,
      final String authorityAccountId,
      final IrohaQuerySignatureProvider signer) {
    final byte[] nonce = new byte[NONCE_BYTES];
    for (int attempt = 0; attempt < 16 && allZero(nonce); attempt++) {
      NONCE_RANDOM.nextBytes(nonce);
    }
    if (allZero(nonce)) {
      throw new IllegalStateException(
          "secure privacy state-query nonce generator repeatedly returned zero");
    }
    return buildSignedPrivacyStateQueryAtV1(
        request,
        networkId,
        authorityAccountId,
        signer,
        System.currentTimeMillis(),
        nonce);
  }

  static SignedQueryV1 buildSignedPrivacyStateQueryAtV1(
      final PrivacyFinalizedStateRequestV1 request,
      final NetworkId networkId,
      final String authorityAccountId,
      final IrohaQuerySignatureProvider signer,
      final long creationTimeMs,
      final byte[] nonce) {
    requireNative();
    final PrivacyFinalizedStateRequestV1 exactRequest =
        Objects.requireNonNull(request, "request");
    final NetworkId exactNetwork = Objects.requireNonNull(networkId, "networkId");
    Objects.requireNonNull(signer, "signer");
    if (creationTimeMs <= 0) {
      throw new IllegalArgumentException("creationTimeMs must be positive");
    }
    if (nonce == null || nonce.length != NONCE_BYTES || allZero(nonce)) {
      throw new IllegalArgumentException("nonce must contain exactly 32 nonzero bytes");
    }
    final byte[] binding = exactRequest.requestBinding();
    if (binding == null || binding.length == 0 || binding.length > 128) {
      throw new IllegalArgumentException("request binding violates the closed native bound");
    }
    final byte[][] prepared;
    try {
      prepared =
          nativePreparePrivacyStateQueryV1(
              exactNetwork.bytes(),
              utf8(authorityAccountId, "authorityAccountId"),
              exactRequest.getQueryId(),
              exactRequest.getProtocolIndex(),
              binding.clone(),
              creationTimeMs,
              nonce.clone());
    } finally {
      Arrays.fill(binding, (byte) 0);
    }
    if (prepared == null
        || prepared.length != 2
        || prepared[0] == null
        || prepared[0].length == 0
        || prepared[0].length > PREPARATION_MAX_BYTES
        || prepared[1] == null
        || prepared[1].length != DIGEST_BYTES) {
      throw new IllegalStateException(
          "native privacy state-query preparation returned an invalid shape");
    }
    final byte[] digest = prepared[1].clone();
    final byte[] signature;
    try {
      final byte[] provided = signer.signQueryDigest(digest.clone());
      if (provided == null) {
        throw new IllegalArgumentException("opaque query signer returned null");
      }
      signature = provided.clone();
    } finally {
      Arrays.fill(digest, (byte) 0);
      Arrays.fill(prepared[1], (byte) 0);
    }
    if (signature.length == 0 || signature.length > SIGNATURE_MAX_BYTES) {
      Arrays.fill(signature, (byte) 0);
      throw new IllegalArgumentException("opaque query signer returned invalid signature bytes");
    }
    final byte[] requestBody;
    try {
      requestBody =
          nativeFinalizePrivacyStateQueryV1(prepared[0].clone(), signature.clone());
    } finally {
      Arrays.fill(signature, (byte) 0);
    }
    if (requestBody == null
        || requestBody.length == 0
        || requestBody.length > SIGNED_QUERY_MAX_BYTES) {
      throw new IllegalStateException(
          "native privacy state-query finalizer violated the request byte bound");
    }
    final org.hyperledger.iroha.sdk.core.model.NetworkId sdkNetworkId =
        org.hyperledger.iroha.sdk.core.model.NetworkId.fromBytes(exactNetwork.bytes());
    return new SignedQueryV1(prepared[0], requestBody, exactRequest, sdkNetworkId);
  }

  /** Natively verifies and projects the exact finalized response bound to {@code signedQuery}. */
  public static PrivacyFinalizedStateViewV1 projectPrivacyStateQueryV1(
      final SignedQueryV1 signedQuery, final byte[] responseNorito) {
    requireNative();
    final SignedQueryV1 exact = Objects.requireNonNull(signedQuery, "signedQuery");
    if (responseNorito == null
        || responseNorito.length == 0
        || (long) responseNorito.length > RESPONSE_MAX_BYTES) {
      throw new IllegalArgumentException("responseNorito violates its closed byte bound");
    }
    final byte[] projection =
        nativeProjectPrivacyStateQueryV1(exact.preparation(), responseNorito.clone());
    if (projection == null
        || projection.length == 0
        || projection.length > PrivacyFinalizedStateProjectionV1.MAX_PROJECTION_BYTES) {
      throw new IllegalStateException(
          "native privacy state-query projection violated its byte bound");
    }
    return PrivacyFinalizedStateProjectionV1.parse(
        projection, exact.request, exact.sdkNetworkId);
  }

  private static byte[] utf8(final String value, final String field) {
    final String exact = Objects.requireNonNull(value, field);
    final byte[] encoded = exact.getBytes(java.nio.charset.StandardCharsets.UTF_8);
    if (encoded.length == 0 || !exact.equals(exact.trim())) {
      throw new IllegalArgumentException(field + " must be canonical nonempty UTF-8");
    }
    return encoded;
  }

  private static boolean allZero(final byte[] value) {
    int aggregate = 0;
    for (final byte item : value) {
      aggregate |= item;
    }
    return aggregate == 0;
  }

  private static void requireNative() {
    NativeHolder.requireLoaded();
  }

  private static final class NativeHolder {
    private static final RuntimeException FAILURE = load();

    private static RuntimeException load() {
      try {
        System.loadLibrary("connect_norito_bridge");
        final int actual = nativeBridgeAbiVersion();
        if (actual != REQUIRED_BRIDGE_ABI_VERSION) {
          return new IllegalStateException(
              "native finalized privacy-state ABI mismatch: expected "
                  + REQUIRED_BRIDGE_ABI_VERSION
                  + ", found "
                  + actual);
        }
        return null;
      } catch (final RuntimeException error) {
        return error;
      } catch (final LinkageError error) {
        return new IllegalStateException(
            "native authenticated finalized privacy-state bridge is unavailable", error);
      }
    }

    private static void requireLoaded() {
      if (FAILURE != null) {
        throw FAILURE;
      }
    }
  }

  private static native int nativeBridgeAbiVersion();

  private static native byte[][] nativePreparePrivacyStateQueryV1(
      byte[] networkId,
      byte[] authorityAccountId,
      int queryId,
      int protocolIndex,
      byte[] requestBinding,
      long creationTimeMs,
      byte[] nonce);

  private static native byte[] nativeFinalizePrivacyStateQueryV1(
      byte[] preparation, byte[] signature);

  private static native byte[] nativeProjectPrivacyStateQueryV1(
      byte[] preparation, byte[] responseNorito);
}
