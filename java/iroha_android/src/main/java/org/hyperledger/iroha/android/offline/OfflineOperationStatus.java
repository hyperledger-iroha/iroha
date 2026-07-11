package org.hyperledger.iroha.android.offline;

import java.math.BigInteger;
import java.util.Arrays;
import java.util.Objects;

/** Pollable state of a Torii Offline operation. */
public abstract class OfflineOperationStatus {
  private static final BigInteger U64_MAX = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);
  private final String operationId;

  private OfflineOperationStatus(final String operationId) {
    this.operationId = OfflineOperationCodec.requireOperationId(operationId);
  }

  public String operationId() {
    return operationId;
  }

  /** The transaction is queued or awaiting finality. */
  public static final class Pending extends OfflineOperationStatus {
    private final OfflineOperationKind kind;
    private final String transactionHash;
    private final BigInteger submittedAtMs;

    public Pending(
        final String operationId,
        final OfflineOperationKind kind,
        final String transactionHash,
        final BigInteger submittedAtMs) {
      super(operationId);
      this.kind = Objects.requireNonNull(kind, "kind");
      this.transactionHash =
          OfflineOperationCodec.requireTransactionHash(transactionHash, "transactionHash");
      this.submittedAtMs = requireU64(submittedAtMs, "submittedAtMs");
    }

    public OfflineOperationKind kind() {
      return kind;
    }

    public String transactionHash() {
      return transactionHash;
    }

    public BigInteger submittedAtMs() {
      return submittedAtMs;
    }
  }

  /** The transaction was applied and finalized. */
  public static final class Applied extends OfflineOperationStatus {
    private final Result result;

    public Applied(final String operationId, final Result result) {
      super(operationId);
      this.result = Objects.requireNonNull(result, "result");
    }

    public Result result() {
      return result;
    }
  }

  /** The transaction reached a terminal rejection. */
  public static final class Rejected extends OfflineOperationStatus {
    private final OfflineOperationKind kind;
    private final String transactionHash;
    private final Error error;

    public Rejected(
        final String operationId,
        final OfflineOperationKind kind,
        final String transactionHash,
        final Error error) {
      super(operationId);
      this.kind = Objects.requireNonNull(kind, "kind");
      this.transactionHash =
          OfflineOperationCodec.requireTransactionHash(transactionHash, "transactionHash");
      this.error = Objects.requireNonNull(error, "error");
    }

    public OfflineOperationKind kind() {
      return kind;
    }

    public String transactionHash() {
      return transactionHash;
    }

    public Error error() {
      return error;
    }
  }

  /** Operation-specific result of an applied command. */
  public abstract static class Result {
    private Result() {}

    /** Applied top-up result. */
    public static final class TopUp extends Result {
      private final TopUpResult value;

      public TopUp(final TopUpResult value) {
        this.value = Objects.requireNonNull(value, "value");
      }

      public TopUpResult value() {
        return value;
      }
    }

    /** Applied redemption result. */
    public static final class Redeem extends Result {
      private final RedeemResult value;

      public Redeem(final RedeemResult value) {
        this.value = Objects.requireNonNull(value, "value");
      }

      public RedeemResult value() {
        return value;
      }
    }
  }

  /** Finalized top-up result. */
  public static final class TopUpResult {
    private final String transactionHash;
    private final BigInteger finalizedBlockHeight;
    private final BigInteger serverTimeMs;
    private final TopUpAnchor anchor;

    public TopUpResult(
        final String transactionHash,
        final BigInteger finalizedBlockHeight,
        final BigInteger serverTimeMs,
        final TopUpAnchor anchor) {
      this.transactionHash =
          OfflineOperationCodec.requireTransactionHash(transactionHash, "transactionHash");
      this.finalizedBlockHeight =
          requirePositiveU64(finalizedBlockHeight, "finalizedBlockHeight");
      this.serverTimeMs = requirePositiveU64(serverTimeMs, "serverTimeMs");
      this.anchor = Objects.requireNonNull(anchor, "anchor");
    }

    public String transactionHash() {
      return transactionHash;
    }

    public BigInteger finalizedBlockHeight() {
      return finalizedBlockHeight;
    }

    public BigInteger serverTimeMs() {
      return serverTimeMs;
    }

    public TopUpAnchor anchor() {
      return anchor;
    }
  }

  /** Finalized redemption result. */
  public static final class RedeemResult {
    private final String transactionHash;
    private final BigInteger finalizedBlockHeight;
    private final BigInteger serverTimeMs;

    public RedeemResult(
        final String transactionHash,
        final BigInteger finalizedBlockHeight,
        final BigInteger serverTimeMs) {
      this.transactionHash =
          OfflineOperationCodec.requireTransactionHash(transactionHash, "transactionHash");
      this.finalizedBlockHeight =
          requirePositiveU64(finalizedBlockHeight, "finalizedBlockHeight");
      this.serverTimeMs = requirePositiveU64(serverTimeMs, "serverTimeMs");
    }

    public String transactionHash() {
      return transactionHash;
    }

    public BigInteger finalizedBlockHeight() {
      return finalizedBlockHeight;
    }

    public BigInteger serverTimeMs() {
      return serverTimeMs;
    }
  }

  /**
   * Schema-bound top-up anchor archive.
   *
   * <p>The internal consensus anchor remains wire-versioned. This wrapper
   * keeps the canonical, schema-validated archive behind a current public name
   * instead of exposing the internal wire type through the operation status.
   */
  public static final class TopUpAnchor {
    private final byte[] archive;

    TopUpAnchor(final byte[] archive) {
      this.archive = Arrays.copyOf(archive, archive.length);
    }

    public byte[] noritoArchive() {
      return Arrays.copyOf(archive, archive.length);
    }
  }

  /** Stable typed Torii rejection. */
  public static final class Error {
    private final String code;
    private final String message;
    private final ErrorDetails details;

    public Error(final String code, final String message, final ErrorDetails details) {
      this.code = OfflineOperationCodec.requireStableErrorCode(code, "error.code");
      this.message = requireExactText(message, "error.message");
      this.details = details;
    }

    public String code() {
      return code;
    }

    public String message() {
      return message;
    }

    public ErrorDetails details() {
      return details;
    }
  }

  /** Structured optional metadata attached to a rejection. */
  public static final class ErrorDetails {
    public final String layer;
    public final String rejectCode;
    public final QueueErrorSnapshot queue;
    public final BigInteger retryAfterSeconds;
    public final String endpoint;
    public final String field;
    public final String expected;
    public final String actual;
    public final String profile;
    public final Integer chainDiscriminant;
    public final String transactionHash;
    public final String lastStatus;
    public final String hint;
    public final AxtErrorDetails axt;

    public ErrorDetails(
        final String layer,
        final String rejectCode,
        final QueueErrorSnapshot queue,
        final BigInteger retryAfterSeconds,
        final String endpoint,
        final String field,
        final String expected,
        final String actual,
        final String profile,
        final Integer chainDiscriminant,
        final String transactionHash,
        final String lastStatus,
        final String hint,
        final AxtErrorDetails axt) {
      this.layer = layer;
      this.rejectCode = rejectCode;
      this.queue = queue;
      this.retryAfterSeconds =
          retryAfterSeconds == null ? null : requireU64(retryAfterSeconds, "retryAfterSeconds");
      this.endpoint = endpoint;
      this.field = field;
      this.expected = expected;
      this.actual = actual;
      this.profile = profile;
      if (chainDiscriminant != null
          && (chainDiscriminant.intValue() < 0 || chainDiscriminant.intValue() > 0xffff)) {
        throw new IllegalArgumentException(
            "chainDiscriminant must fit in an unsigned 16-bit integer");
      }
      this.chainDiscriminant = chainDiscriminant;
      this.transactionHash = transactionHash;
      this.lastStatus = lastStatus;
      this.hint = hint;
      this.axt = axt;
    }
  }

  /** Queue pressure snapshot attached to a rejection. */
  public static final class QueueErrorSnapshot {
    public final String state;
    public final BigInteger queued;
    public final BigInteger capacity;
    public final boolean saturated;

    public QueueErrorSnapshot(
        final String state,
        final BigInteger queued,
        final BigInteger capacity,
        final boolean saturated) {
      this.state = requireExactText(state, "queue.state");
      this.queued = requireU64(queued, "queue.queued");
      this.capacity = requireU64(capacity, "queue.capacity");
      this.saturated = saturated;
    }
  }

  /** AXT rejection metadata attached to a validation failure. */
  public static final class AxtErrorDetails {
    public final String code;
    public final String reason;
    public final BigInteger snapshotVersion;
    public final BigInteger dataspace;
    public final Long lane;
    public final BigInteger nextMinHandleEra;
    public final BigInteger nextMinSubNonce;

    public AxtErrorDetails(
        final String code,
        final String reason,
        final BigInteger snapshotVersion,
        final BigInteger dataspace,
        final Long lane,
        final BigInteger nextMinHandleEra,
        final BigInteger nextMinSubNonce) {
      this.code = code;
      this.reason = reason;
      this.snapshotVersion =
          snapshotVersion == null ? null : requireU64(snapshotVersion, "axt.snapshotVersion");
      this.dataspace = dataspace == null ? null : requireU64(dataspace, "axt.dataspace");
      if (lane != null && (lane.longValue() < 0 || lane.longValue() > 0xffff_ffffL)) {
        throw new IllegalArgumentException("axt.lane must fit in an unsigned 32-bit integer");
      }
      this.lane = lane;
      this.nextMinHandleEra =
          nextMinHandleEra == null ? null : requireU64(nextMinHandleEra, "axt.nextMinHandleEra");
      this.nextMinSubNonce =
          nextMinSubNonce == null ? null : requireU64(nextMinSubNonce, "axt.nextMinSubNonce");
    }
  }

  private static String requireExactText(final String value, final String field) {
    Objects.requireNonNull(value, field);
    if (value.isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(field + " must be exact non-empty text");
    }
    return value;
  }

  private static BigInteger requireU64(final BigInteger value, final String field) {
    Objects.requireNonNull(value, field);
    if (value.signum() < 0 || value.compareTo(U64_MAX) > 0) {
      throw new IllegalArgumentException(field + " must fit in an unsigned 64-bit integer");
    }
    return value;
  }

  private static BigInteger requirePositiveU64(final BigInteger value, final String field) {
    final BigInteger checked = requireU64(value, field);
    if (checked.signum() == 0) {
      throw new IllegalArgumentException(field + " must be at least 1");
    }
    return checked;
  }
}
