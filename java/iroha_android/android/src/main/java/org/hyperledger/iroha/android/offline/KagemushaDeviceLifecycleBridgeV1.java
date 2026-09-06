// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import java.util.Arrays;
import java.util.Objects;

/**
 * Java Android facade for the exact KAGEMUSHA V1 secure-device lifecycle bridge.
 *
 * <p>The canonical implementation lives in the default Kotlin Android SDK. This facade exposes
 * the same contract without adding a second codec or backend. Missing native secure
 * multi-credit-inbox/journal/outbox support is a normal {@link Availability#ONLINE_ONLY} result;
 * no KeyMint-only or software implementation is accepted as a fallback.
 */
public final class KagemushaDeviceLifecycleBridgeV1 {
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
    READ_ACTIVE_HARDWARE_CREDENTIAL(1),
    STAGE_INBOUND_PAYMENT(2),
    RECOVER_STAGED_INBOUND_PAYMENT(3),
    RECOVER_INBOUND_INBOX_PAGE(4),
    PREPARE_EXACT_NEXT_TRANSITION(5),
    RECOVER_PREPARED_TRANSITION(6),
    COMMIT_VERIFIED_CANDIDATE_AND_SIGN_TERMINAL(7),
    RECOVER_TERMINAL_OUTCOME(8),
    INSTALL_TERMINAL_ENVELOPE(9),
    RECOVER_INSTALLED_ENVELOPE_OR_STATE_PROOF(10),
    SIGN_RECEIVE_ACKNOWLEDGEMENT(11),
    RELEASE_OUTBOX_ENTRY(12),
    READ_TRUSTED_TIME_OR_LEASE(13),
    PREPARE_MINT_AUTHORIZATION(14),
    RECOVER_MINT_AUTHORIZATION(15),
    VERIFY_AUTHORIZATION_AND_STAGE_MINT_CREDIT(16),
    FOLD_RECEIVE_CREDIT(17),
    READ_PENDING_CREDIT_WATERMARK(18),
    ROTATE_HARDWARE_EPOCH(19),
    BOOTSTRAP_AGGREGATE_STATE(20),
    RECOVER_WALLET_SNAPSHOT(21),
    CREATE_SIGNED_PAYMENT_REQUEST(22);

    private final int code;

    Operation(final int code) {
      this.code = code;
    }

    public int code() {
      return code;
    }
  }

  /** Exact secure-backend capabilities shared with the Kotlin bridge. */
  public enum Capability {
    EXACT_NEXT_PREDECESSOR_CONSUMPTION(1 << 0),
    ONE_USE_SUCCESSOR_AUTHORIZATION(1 << 1),
    ROLLBACK_RESISTANT_COUNTER_AND_JOURNAL(1 << 2),
    SEALED_TRANSITION_RECOVERY(1 << 3),
    RECEIVER_BOUND_CREDIT_COMMIT(1 << 4),
    ROLLBACK_RESISTANT_ACCEPTED_CREDIT_INBOX(1 << 5),
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
    private final byte[] qualificationReportDigest;

    private Capabilities(
        final org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1.Capabilities
            source) {
      hardwarePolicyId = source.hardwarePolicyId();
      qualificationReportDigest = source.qualificationReportDigest();
    }

    public byte[] hardwarePolicyId() {
      return hardwarePolicyId.clone();
    }

    public byte[] qualificationReportDigest() {
      return qualificationReportDigest.clone();
    }
  }

  /** Defensively copied secure-backend result. */
  public static final class Result {
    private final Operation operation;
    private final Status status;
    private final byte[] payload;
    private final byte[] authenticator;

    private Result(
        final org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1.Result source) {
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

  private final org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1 delegate;

  public static final int PROTOCOL_VERSION =
      org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1.PROTOCOL_VERSION;
  public static final int MAXIMUM_COMMAND_PAYLOAD_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1
          .MAXIMUM_COMMAND_PAYLOAD_BYTES;
  public static final int MAXIMUM_RESPONSE_PAYLOAD_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1
          .MAXIMUM_RESPONSE_PAYLOAD_BYTES;
  public static final int MAXIMUM_AUTHENTICATOR_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1.MAXIMUM_AUTHENTICATOR_BYTES;
  public static final int MAXIMUM_NATIVE_CONTRACT_VECTOR_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1
          .MAXIMUM_NATIVE_CONTRACT_VECTOR_BYTES;

  private KagemushaDeviceLifecycleBridgeV1(
      final org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1 delegate) {
    this.delegate = Objects.requireNonNull(delegate, "delegate");
  }

  /** Discover the optional native backend. Missing or partial support remains online-only. */
  public static KagemushaDeviceLifecycleBridgeV1 production() {
    return new KagemushaDeviceLifecycleBridgeV1(
        org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1.production());
  }

  /** Construct an explicit online-only facade with no execution backend. */
  public static KagemushaDeviceLifecycleBridgeV1 onlineOnly() {
    return new KagemushaDeviceLifecycleBridgeV1(
        org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1.onlineOnly());
  }

  /**
   * Return the canonical Norito contract vector compiled into the native bridge, when linked.
   *
   * <p>Its domain-separated digest is an ABI/tamper pin only and never grants monetary authority.
   */
  public static byte[] nativeContractVector() {
    final byte[] vector =
        org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1
            .nativeContractVector();
    return vector == null ? null : vector.clone();
  }

  public Availability availability() {
    return Availability.valueOf(delegate.getAvailability().name());
  }

  public Capabilities capabilities() {
    final org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1.Capabilities source =
        delegate.capabilities();
    return source == null ? null : new Capabilities(source);
  }

  /** Execute with the operation-1 bootstrapped response key required for operations 2 through 22. */
  public Result executeAuthenticated(
      final Operation operation,
      final byte[] requestId,
      final byte[] canonicalCommand,
      final byte[] acceptedDevicePublicKey) {
    final Operation requiredOperation = Objects.requireNonNull(operation, "operation");
    final byte[] requestIdCopy = Objects.requireNonNull(requestId, "requestId").clone();
    final byte[] commandCopy =
        Objects.requireNonNull(canonicalCommand, "canonicalCommand").clone();
    final byte[] publicKeyCopy =
        acceptedDevicePublicKey == null ? null : acceptedDevicePublicKey.clone();
    try {
      return new Result(
          delegate.executeAuthenticated(
              org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1.Operation
                  .valueOf(requiredOperation.name()),
              requestIdCopy,
              commandCopy,
              publicKeyCopy));
    } finally {
      Arrays.fill(requestIdCopy, (byte) 0);
      Arrays.fill(commandCopy, (byte) 0);
      if (publicKeyCopy != null) {
        Arrays.fill(publicKeyCopy, (byte) 0);
      }
    }
  }
}
