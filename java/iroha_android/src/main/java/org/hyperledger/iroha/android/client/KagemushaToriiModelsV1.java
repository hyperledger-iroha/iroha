// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.client;

import java.util.Arrays;
import java.util.Objects;
import java.util.function.BiFunction;

/** Java-facing KAGEMUSHA V1 Torii models that preserve the finality-verification boundary. */
public final class KagemushaToriiModelsV1 {
  private KagemushaToriiModelsV1() {}

  /** Exact asset-neutral readiness projection. */
  public record Readiness(
      String kagemushaHandoffCapability,
      int wireVersion,
      int deviceLifecycleVersion,
      boolean ready) {}

  /** Closed reserve-operation catalog. */
  public enum OperationKind {
    TOP_UP,
    REDEMPTION
  }

  /** Pollable reserve-operation state. */
  public enum OperationState {
    PENDING,
    APPLIED,
    REJECTED
  }

  /** Exact terminal rejection catalog. */
  public enum RejectionCode {
    INVALID_REQUEST,
    UNAUTHORIZED,
    INSUFFICIENT_ONLINE_BALANCE,
    INVALID_PROOF,
    HARDWARE_POLICY_REJECTED,
    IDENTITY_CONFLICT,
    RESERVE_UNDERFLOW,
    ARITHMETIC_OVERFLOW,
    INTERNAL_FAILURE
  }

  /** Public rejection metadata that carries no monetary result. */
  public static final class Rejection {
    private final RejectionCode code;
    private final byte[] detailDigest;

    Rejection(final org.hyperledger.iroha.sdk.client.KagemushaOperationRejectionV1 value) {
      code = RejectionCode.valueOf(value.code.name());
      detailDigest = value.detailDigest();
    }

    public RejectionCode code() {
      return code;
    }

    public byte[] detailDigest() {
      return detailDigest.clone();
    }
  }

  /** Structurally validated metadata that withholds every unverified applied result. */
  public static final class UnverifiedOperationStatus {
    private final org.hyperledger.iroha.sdk.client.UnverifiedKagemushaOperationStatusV1 delegate;

    UnverifiedOperationStatus(
        final org.hyperledger.iroha.sdk.client.UnverifiedKagemushaOperationStatusV1 delegate) {
      this.delegate = Objects.requireNonNull(delegate, "delegate");
    }

    public byte[] operationId() {
      return delegate.operationId();
    }

    public OperationKind kind() {
      return OperationKind.valueOf(delegate.kind.name());
    }

    public OperationState state() {
      return OperationState.valueOf(delegate.state.name());
    }

    public Rejection rejection() {
      return delegate.rejection == null ? null : new Rejection(delegate.rejection);
    }

    /** Release the complete response only through a caller-pinned finality verifier. */
    public <A, R> R verifyAgainst(
        final A trustAnchor, final BiFunction<byte[], A, R> verifier) {
      Objects.requireNonNull(trustAnchor, "trustAnchor");
      Objects.requireNonNull(verifier, "verifier");
      return delegate.verifyAgainst(
          trustAnchor, (canonicalJson, anchor) -> verifier.apply(canonicalJson, anchor));
    }

    @Override
    public String toString() {
      return "UnverifiedKagemushaOperationStatusV1(kind="
          + kind()
          + ", state="
          + state()
          + ", result=[WITHHELD])";
    }

    @Override
    public boolean equals(final Object other) {
      if (!(other instanceof UnverifiedOperationStatus status)) return false;
      return Arrays.equals(operationId(), status.operationId())
          && kind() == status.kind()
          && state() == status.state();
    }

    @Override
    public int hashCode() {
      return 31 * Arrays.hashCode(operationId()) + kind().hashCode();
    }
  }
}
