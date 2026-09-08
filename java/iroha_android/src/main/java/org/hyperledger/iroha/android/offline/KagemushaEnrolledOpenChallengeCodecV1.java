// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import java.util.Objects;
import org.hyperledger.iroha.sdk.offline.KagemushaEnrolledOpenAccountChallengeV1;
import org.hyperledger.iroha.sdk.offline.KagemushaEnrolledOpenAuthoritySourceV1;
import org.hyperledger.iroha.sdk.offline.KagemushaEnrolledOpenSelectorV1;

/** Java mirror of the sole untrusted enrolled-open challenge projection codec. */
public final class KagemushaEnrolledOpenChallengeCodecV1 {
  public static final int MAXIMUM_ARCHIVE_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaEnrolledOpenChallengeCodecV1.MAXIMUM_ARCHIVE_BYTES;

  private KagemushaEnrolledOpenChallengeCodecV1() {}

  public static byte[] encodeAccountChallengeShape(final KagemushaEnrolledOpenAccountChallengeV1 value) {
    return org.hyperledger.iroha.sdk.offline.KagemushaEnrolledOpenChallengeCodecV1.encodeAccountChallengeShape(
        Objects.requireNonNull(value, "value"));
  }

  public static KagemushaEnrolledOpenAccountChallengeV1 decodeAccountChallengeShapeExact(final byte[] bytes) {
    return org.hyperledger.iroha.sdk.offline.KagemushaEnrolledOpenChallengeCodecV1.decodeAccountChallengeShapeExact(copy(bytes));
  }

  public static byte[] encodeAuthoritySourceShape(final KagemushaEnrolledOpenAuthoritySourceV1 value) {
    return org.hyperledger.iroha.sdk.offline.KagemushaEnrolledOpenChallengeCodecV1.encodeAuthoritySourceShape(
        Objects.requireNonNull(value, "value"));
  }

  public static KagemushaEnrolledOpenAuthoritySourceV1 decodeAuthoritySourceShapeExact(final byte[] bytes) {
    return org.hyperledger.iroha.sdk.offline.KagemushaEnrolledOpenChallengeCodecV1.decodeAuthoritySourceShapeExact(copy(bytes));
  }

  /** Returns the once-hashed Rust message for direct Ed25519 signing after exact correlation. */
  public static byte[] accountSigningMessageShape(
      final KagemushaEnrolledOpenAccountChallengeV1 challenge,
      final KagemushaEnrolledOpenSelectorV1 expectedSelector,
      final byte[] expectedNonce,
      final byte[] expectedReleaseId,
      final byte[] expectedHardwarePolicyDigest,
      final byte[] expectedCoreAuthorizationKeyReference) {
    return org.hyperledger.iroha.sdk.offline.KagemushaEnrolledOpenChallengeCodecV1.accountSigningMessageShape(
        Objects.requireNonNull(challenge, "challenge"), Objects.requireNonNull(expectedSelector, "expectedSelector"),
        copy(expectedNonce), copy(expectedReleaseId), copy(expectedHardwarePolicyDigest), copy(expectedCoreAuthorizationKeyReference));
  }

  private static byte[] copy(final byte[] value) {
    return Objects.requireNonNull(value, "bytes").clone();
  }
}
