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
 * multi-credit-inbox/journal/outbox support is a normal {@link Availability#ONLINE_ONLY} result;
 * no KeyMint-only or software implementation is accepted as a fallback.
 */
public final class OfflineCashDeviceLifecycleBridgeV1 {
  /** Whether the complete secure-device contract is present. */
  public enum Availability {
    ONLINE_ONLY,
    AVAILABLE,
  }

  /**
   * Exact operations shared with Core's sealed reservation, transition, and recovery flow.
   *
   * <p>The service reserves bytes before accepting authority, commits a verified candidate exactly
   * once, and recovers every terminal certificate and installed envelope byte-identically.
   */
  public enum Operation {
    READ_ACTIVE_HARDWARE_CREDENTIAL,
    PREPARE_ACCEPTANCE_INTENT_AUTHORIZATION,
    RECOVER_ACCEPTANCE_INTENT_AUTHORIZATION,
    VERIFY_AUTHORIZATION_RESERVE_INBOX_AND_ISSUE_ACCEPTANCE_TICKET,
    RECOVER_ACCEPTANCE_TICKET,
    STAGE_INBOUND_PAYMENT,
    RECOVER_STAGED_INBOUND_PAYMENT,
    RECOVER_INBOUND_INBOX_PAGE,
    PREPARE_EXACT_NEXT_TRANSITION,
    RECOVER_PREPARED_TRANSITION,
    ABANDON_UNCOMMITTED_PREPARED_TRANSITION,
    COMMIT_VERIFIED_CANDIDATE,
    RECOVER_TERMINAL_COMMIT_CERTIFICATE,
    INSTALL_FINAL_COMMIT_WRAPPER,
    RECOVER_INSTALLED_ENVELOPE_OR_STATE_PROOF,
    SIGN_RECEIVE_ACKNOWLEDGEMENT,
    RELEASE_OUTBOX_ENTRY,
    READ_TRUSTED_TIME_OR_LEASE,
    PREPARE_MINT_AUTHORIZATION,
    RECOVER_MINT_AUTHORIZATION,
    VERIFY_AUTHORIZATION_AND_STAGE_MINT_CREDIT,
    FOLD_RECEIVE,
    READ_PENDING_CREDIT_WATERMARK,
    ROTATE_HARDWARE_EPOCH,
  }

  /** Exact secure-backend capabilities shared with the Kotlin bridge. */
  public enum Capability {
    EXACT_NEXT_PREDECESSOR_CONSUMPTION(1 << 0),
    ONE_USE_SUCCESSOR_AUTHORIZATION(1 << 1),
    ROLLBACK_RESISTANT_COUNTER_AND_JOURNAL(1 << 2),
    SEALED_TRANSITION_RECOVERY(1 << 3),
    ONE_USE_ACCEPTANCE_TICKETS(1 << 4),
    DURABLE_INBOX_RESERVATION(1 << 5),
    AUTHENTICATED_INBOUND_STAGING(1 << 6),
    AUTHORITATIVE_REPLAY_ROOT_RECOVERY(1 << 7),
    SENDER_OUTBOX_RESERVATION(1 << 8),
    AUTHENTICATED_DURABLE_RETRY_OUTBOX(1 << 9),
    ATOMIC_VERIFIED_CANDIDATE_COMMIT(1 << 10),
    RECOVERABLE_TERMINAL_COMMIT_CERTIFICATE(1 << 11),
    TRUSTED_TIME_OR_LEASE(1 << 12),
    OFFLINE_HARDWARE_EPOCH_ROTATION(1 << 13),
    ROLLBACK_SAFE_COUNTER_ROLLOVER(1 << 14),
    NO_SOFTWARE_FALLBACK(1 << 15);

    private final int mask;

    Capability(final int mask) {
      this.mask = mask;
    }

    /** Return the canonical bit in the native capability frame. */
    public int mask() {
      return mask;
    }
  }

  /** Stable result status. */
  public enum Status {
    SUCCESS,
    UNAVAILABLE,
    STALE_OR_CONCURRENT,
    BINDING_MISMATCH,
    TRUSTED_TIME_REJECTED,
    REJECTED,
    MISSING,
    CONFLICT,
    CORRUPT,
    MALFORMED_REQUEST,
    RECOVERY_REQUIRED,
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
