// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.governance;

import java.util.Arrays;
import java.util.Objects;

/** Immutable external trust anchor for one Parliament timed-OVN casting proof. */
public final class ParliamentTimedOvnCastingTrustAnchorV1 {
  private static final int HASH_BYTES = 32;

  private final byte[] networkId;
  private final long trustedCheckpointHeight;
  private final byte[] trustedCheckpointContextId;
  private final byte[] expectedBallotAttemptId;

  /** Create an exact network/checkpoint/ballot trust anchor; no field has a default. */
  public ParliamentTimedOvnCastingTrustAnchorV1(
      final byte[] networkId,
      final long trustedCheckpointHeight,
      final byte[] trustedCheckpointContextId,
      final byte[] expectedBallotAttemptId) {
    this.networkId = exactBytes("networkId", networkId);
    if (trustedCheckpointHeight <= 0) {
      throw new IllegalArgumentException("trustedCheckpointHeight must be positive");
    }
    this.trustedCheckpointHeight = trustedCheckpointHeight;
    this.trustedCheckpointContextId =
        exactBytes("trustedCheckpointContextId", trustedCheckpointContextId);
    this.expectedBallotAttemptId =
        exactBytes("expectedBallotAttemptId", expectedBallotAttemptId);
    boolean nonzeroBallot = false;
    for (final byte value : this.expectedBallotAttemptId) {
      nonzeroBallot |= value != 0;
    }
    if (!nonzeroBallot) {
      throw new IllegalArgumentException("expectedBallotAttemptId must be nonzero");
    }
  }

  /** Return a defensive copy of the raw genesis-derived NetworkId. */
  public byte[] networkId() {
    return networkId.clone();
  }

  /** Return the exact nonzero trusted checkpoint height. */
  public long trustedCheckpointHeight() {
    return trustedCheckpointHeight;
  }

  /** Return a defensive copy of the exact trusted HeightContextId. */
  public byte[] trustedCheckpointContextId() {
    return trustedCheckpointContextId.clone();
  }

  /** Return a defensive copy of the exact expected BallotAttemptId. */
  public byte[] expectedBallotAttemptId() {
    return expectedBallotAttemptId.clone();
  }

  @Override
  public String toString() {
    return "ParliamentTimedOvnCastingTrustAnchorV1(redacted)";
  }

  @Override
  public boolean equals(final Object other) {
    if (!(other instanceof ParliamentTimedOvnCastingTrustAnchorV1)) {
      return false;
    }
    final ParliamentTimedOvnCastingTrustAnchorV1 anchor =
        (ParliamentTimedOvnCastingTrustAnchorV1) other;
    return trustedCheckpointHeight == anchor.trustedCheckpointHeight
        && Arrays.equals(networkId, anchor.networkId)
        && Arrays.equals(trustedCheckpointContextId, anchor.trustedCheckpointContextId)
        && Arrays.equals(expectedBallotAttemptId, anchor.expectedBallotAttemptId);
  }

  @Override
  public int hashCode() {
    int result = Objects.hash(trustedCheckpointHeight);
    result = 31 * result + Arrays.hashCode(networkId);
    result = 31 * result + Arrays.hashCode(trustedCheckpointContextId);
    return 31 * result + Arrays.hashCode(expectedBallotAttemptId);
  }

  private static byte[] exactBytes(final String label, final byte[] value) {
    final byte[] required = Objects.requireNonNull(value, label);
    if (required.length != HASH_BYTES) {
      throw new IllegalArgumentException(label + " must contain exactly 32 bytes");
    }
    return required.clone();
  }
}
