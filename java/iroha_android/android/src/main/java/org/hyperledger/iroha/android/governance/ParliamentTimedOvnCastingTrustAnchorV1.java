// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.governance;

import java.math.BigInteger;
import java.util.Arrays;
import java.util.Objects;
import org.hyperledger.iroha.android.client.ParliamentApiV1;

/** Immutable external trust anchor for one Parliament timed-OVN casting proof. */
public final class ParliamentTimedOvnCastingTrustAnchorV1 {
  private static final int HASH_BYTES = 32;

  private final byte[] networkId;
  private final BigInteger trustedCheckpointHeight;
  private final byte[] trustedCheckpointContextId;
  private final byte[] expectedBallotAttemptId;

  /** Create an exact network/checkpoint/ballot trust anchor across the complete u64 height domain. */
  public ParliamentTimedOvnCastingTrustAnchorV1(
      final byte[] networkId,
      final BigInteger trustedCheckpointHeight,
      final byte[] trustedCheckpointContextId,
      final byte[] expectedBallotAttemptId) {
    this.networkId = exactBytes("networkId", networkId);
    final BigInteger requiredHeight =
        Objects.requireNonNull(trustedCheckpointHeight, "trustedCheckpointHeight");
    if (requiredHeight.signum() <= 0 || requiredHeight.bitLength() > 64) {
      throw new IllegalArgumentException("trustedCheckpointHeight must be a positive u64");
    }
    this.trustedCheckpointHeight = requiredHeight;
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

  /** Convenience constructor for positive checkpoint heights representable by {@code long}. */
  public ParliamentTimedOvnCastingTrustAnchorV1(
      final byte[] networkId,
      final long trustedCheckpointHeight,
      final byte[] trustedCheckpointContextId,
      final byte[] expectedBallotAttemptId) {
    this(
        networkId,
        BigInteger.valueOf(trustedCheckpointHeight),
        trustedCheckpointContextId,
        expectedBallotAttemptId);
  }

  /** Return a defensive copy of the raw genesis-derived NetworkId. */
  public byte[] networkId() {
    return networkId.clone();
  }

  /** Return the exact nonzero trusted checkpoint height in the u64 protocol domain. */
  public BigInteger trustedCheckpointHeight() {
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

  /** Returns a new immutable anchor promoted by one native-authenticated page result. */
  public ParliamentTimedOvnCastingTrustAnchorV1 promoted(
      final ParliamentApiV1.TimedOvnCastingProofPageVerification verification) {
    final ParliamentApiV1.TimedOvnCastingProofPageVerification required =
        Objects.requireNonNull(verification, "verification");
    return new ParliamentTimedOvnCastingTrustAnchorV1(
        networkId,
        required.evaluatedBlockHeight,
        required.evaluatedContextId(),
        expectedBallotAttemptId);
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
    return trustedCheckpointHeight.equals(anchor.trustedCheckpointHeight)
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
