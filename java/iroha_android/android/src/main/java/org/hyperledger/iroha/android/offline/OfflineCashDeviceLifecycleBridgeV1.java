// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import java.util.Arrays;
import java.util.Objects;

/**
 * Java Android facade for the exact Offline Cash V1 secure-device lifecycle bridge.
 *
 * <p>The canonical implementation lives in the default Kotlin Android SDK. This facade preserves
 * the Java migration surface without adding a second codec or backend. Missing native secure
 * journal/outbox support is a normal {@link Availability#ONLINE_ONLY} result; no KeyMint-only or
 * software implementation is accepted as a fallback.
 */
public final class OfflineCashDeviceLifecycleBridgeV1 {
  /** Whether the complete secure-device contract is present. */
  public enum Availability {
    ONLINE_ONLY,
    AVAILABLE,
  }

  /** Exact operations shared with the sealed Core journal/outbox boundary. */
  public enum Operation {
    RESERVE_RECEIVE_INTENT_AND_SIGN,
    RECOVER_RECEIVE_INTENT_AND_SIGNATURE,
    BIND_RECEIVE_REQUEST_DIGEST,
    PUBLISH_SEND_PAYMENT,
    RECOVER_ACTIVE_INTENT,
    CANCEL_EXPIRED_RECEIVE,
    COMMIT_INTENT_EXACT_NEXT,
    RECOVER_TERMINAL,
    RECOVER_RECEIVE_TERMINAL,
    SIGN_RECEIVE_ACKNOWLEDGEMENT,
    STAGE_PAYMENT,
    RECOVER_STAGED_PAYMENT_DIGEST,
    PUBLISH_STAGED_PAYMENT,
    RECOVER_PUBLISHED_PAYMENT,
  }

  /** Stable result status. */
  public enum Status {
    SUCCESS,
    UNAVAILABLE,
    STALE_OR_CONCURRENT,
    INTENT_MISMATCH,
    TRUSTED_TIME_REJECTED,
    REJECTED,
    MISSING,
    CONFLICT,
    CORRUPT,
    MALFORMED_REQUEST,
  }

  /** Defensively copied native secure-backend identity. */
  public static final class Capabilities {
    private final byte[] hardwarePolicyId;
    private final byte[] attestationDigest;

    private Capabilities(
        final org.hyperledger.iroha.sdk.offline.OfflineCashDeviceLifecycleBridgeV1.Capabilities
            source) {
      hardwarePolicyId = source.hardwarePolicyId();
      attestationDigest = source.attestationDigest();
    }

    public byte[] hardwarePolicyId() {
      return hardwarePolicyId.clone();
    }

    public byte[] attestationDigest() {
      return attestationDigest.clone();
    }
  }

  /** Defensively copied secure-backend result. */
  public static final class Result {
    private final Operation operation;
    private final Status status;
    private final byte[] payload;
    private final byte[] authenticator;

    private Result(
        final org.hyperledger.iroha.sdk.offline.OfflineCashDeviceLifecycleBridgeV1.Result source) {
      operation = Operation.valueOf(source.getOperation().name());
      status = Status.valueOf(source.getStatus().name());
      payload = source.payload();
      authenticator = source.authenticator();
    }

    public Operation operation() {
      return operation;
    }

    public Status status() {
      return status;
    }

    public byte[] payload() {
      return payload.clone();
    }

    public byte[] authenticator() {
      return authenticator.clone();
    }
  }

  private final org.hyperledger.iroha.sdk.offline.OfflineCashDeviceLifecycleBridgeV1 delegate;

  private OfflineCashDeviceLifecycleBridgeV1(
      final org.hyperledger.iroha.sdk.offline.OfflineCashDeviceLifecycleBridgeV1 delegate) {
    this.delegate = Objects.requireNonNull(delegate, "delegate");
  }

  /** Discover the optional native backend. Missing or partial support remains online-only. */
  public static OfflineCashDeviceLifecycleBridgeV1 production() {
    return new OfflineCashDeviceLifecycleBridgeV1(
        org.hyperledger.iroha.sdk.offline.OfflineCashDeviceLifecycleBridgeV1.production());
  }

  /** Construct an explicit online-only facade with no execution backend. */
  public static OfflineCashDeviceLifecycleBridgeV1 onlineOnly() {
    return new OfflineCashDeviceLifecycleBridgeV1(
        org.hyperledger.iroha.sdk.offline.OfflineCashDeviceLifecycleBridgeV1.onlineOnly());
  }

  public Availability availability() {
    return Availability.valueOf(delegate.getAvailability().name());
  }

  public Capabilities capabilities() {
    final org.hyperledger.iroha.sdk.offline.OfflineCashDeviceLifecycleBridgeV1.Capabilities source =
        delegate.capabilities();
    return source == null ? null : new Capabilities(source);
  }

  /** Execute one bounded canonical Offline Cash V1 command through the qualifying backend. */
  public Result execute(
      final Operation operation,
      final byte[] requestId,
      final byte[] canonicalCommand) {
    final Operation requiredOperation = Objects.requireNonNull(operation, "operation");
    final byte[] requestIdCopy = Objects.requireNonNull(requestId, "requestId").clone();
    final byte[] commandCopy =
        Objects.requireNonNull(canonicalCommand, "canonicalCommand").clone();
    try {
      return new Result(
          delegate.execute(
              org.hyperledger.iroha.sdk.offline.OfflineCashDeviceLifecycleBridgeV1.Operation
                  .valueOf(requiredOperation.name()),
              requestIdCopy,
              commandCopy));
    } finally {
      Arrays.fill(requestIdCopy, (byte) 0);
      Arrays.fill(commandCopy, (byte) 0);
    }
  }
}
