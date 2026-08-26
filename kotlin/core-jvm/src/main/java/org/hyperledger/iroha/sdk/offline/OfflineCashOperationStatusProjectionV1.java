package org.hyperledger.iroha.sdk.offline;

import java.util.Arrays;
import java.util.Objects;
import org.jetbrains.annotations.NotNull;
import org.jetbrains.annotations.Nullable;

/** Strict public projection of a decoded Offline Cash V1 operation status. */
public final class OfflineCashOperationStatusProjectionV1 {
  private final OfflineCashOperationStateV1 state;
  private final OfflineCashOperationKindV1 kind;
  private final byte[] operationId;
  private final byte[] transactionHash;
  private final Long submittedAtMilliseconds;
  private final Long finalizedBlockHeight;
  private final Long serverTimeMilliseconds;
  private final OfflineCashFinalizedTopUpV1 finalizedTopUp;
  private final OfflineCashOperationRejectionV1 rejection;

  private OfflineCashOperationStatusProjectionV1(
      @NotNull final OfflineCashOperationStateV1 state,
      @NotNull final OfflineCashOperationKindV1 kind,
      @NotNull final byte[] operationId,
      @NotNull final byte[] transactionHash,
      @Nullable final Long submittedAtMilliseconds,
      @Nullable final Long finalizedBlockHeight,
      @Nullable final Long serverTimeMilliseconds,
      @Nullable final OfflineCashFinalizedTopUpV1 finalizedTopUp,
      @Nullable final OfflineCashOperationRejectionV1 rejection) {
    this.state = Objects.requireNonNull(state, "state");
    this.kind = Objects.requireNonNull(kind, "kind");
    this.operationId = requireOfflineCashDigest(operationId, "operationId");
    this.transactionHash = requireOfflineCashDigest(transactionHash, "transactionHash");
    this.submittedAtMilliseconds = submittedAtMilliseconds;
    this.finalizedBlockHeight = finalizedBlockHeight;
    this.serverTimeMilliseconds = serverTimeMilliseconds;
    this.finalizedTopUp = finalizedTopUp;
    this.rejection = rejection;
    validateState();
  }

  @NotNull
  static OfflineCashOperationStatusProjectionV1 fromValidatedProjection(
      @NotNull final OfflineCashOperationStateV1 state,
      @NotNull final OfflineCashOperationKindV1 kind,
      @NotNull final byte[] operationId,
      @NotNull final byte[] transactionHash,
      @Nullable final Long submittedAtMilliseconds,
      @Nullable final Long finalizedBlockHeight,
      @Nullable final Long serverTimeMilliseconds,
      @Nullable final OfflineCashFinalizedTopUpV1 finalizedTopUp,
      @Nullable final OfflineCashOperationRejectionV1 rejection) {
    return new OfflineCashOperationStatusProjectionV1(
        state,
        kind,
        operationId,
        transactionHash,
        submittedAtMilliseconds,
        finalizedBlockHeight,
        serverTimeMilliseconds,
        finalizedTopUp,
        rejection);
  }

  @NotNull
  public OfflineCashOperationStateV1 getState() {
    return state;
  }

  @NotNull
  public OfflineCashOperationKindV1 getKind() {
    return kind;
  }

  @Nullable
  public Long getSubmittedAtMilliseconds() {
    return submittedAtMilliseconds;
  }

  @Nullable
  public Long getFinalizedBlockHeight() {
    return finalizedBlockHeight;
  }

  @Nullable
  public Long getServerTimeMilliseconds() {
    return serverTimeMilliseconds;
  }

  @Nullable
  public OfflineCashFinalizedTopUpV1 getFinalizedTopUp() {
    return finalizedTopUp;
  }

  @Nullable
  public OfflineCashOperationRejectionV1 getRejection() {
    return rejection;
  }

  @NotNull
  public byte[] operationId() {
    return Arrays.copyOf(operationId, operationId.length);
  }

  @NotNull
  public byte[] transactionHash() {
    return Arrays.copyOf(transactionHash, transactionHash.length);
  }

  private void validateState() {
    switch (state) {
      case PENDING:
        if (submittedAtMilliseconds == null
            || submittedAtMilliseconds <= 0
            || finalizedBlockHeight != null
            || serverTimeMilliseconds != null
            || finalizedTopUp != null
            || rejection != null) {
          throw new IllegalArgumentException(
              "pending operation projection requires a positive submission time and no terminal fields");
        }
        return;
      case APPLIED:
        if (submittedAtMilliseconds != null
            || finalizedBlockHeight == null
            || serverTimeMilliseconds == null
            || rejection != null) {
          throw new IllegalArgumentException("applied operation projection is incomplete");
        }
        if (finalizedBlockHeight <= 0 || serverTimeMilliseconds <= 0) {
          throw new IllegalArgumentException(
              "applied operation projection requires positive finality height and time");
        }
        if ((kind == OfflineCashOperationKindV1.TOP_UP) != (finalizedTopUp != null)) {
          throw new IllegalArgumentException(
              "applied top-up projection must contain only its opaque finalized evidence");
        }
        if (finalizedTopUp != null
            && (finalizedTopUp.getFinalizedBlockHeight() != finalizedBlockHeight
                || finalizedTopUp.getServerTimeMilliseconds() != serverTimeMilliseconds)) {
          throw new IllegalArgumentException(
              "finalized top-up projection changed its terminal height or time");
        }
        return;
      case REJECTED:
        if (submittedAtMilliseconds != null
            || finalizedBlockHeight != null
            || serverTimeMilliseconds != null
            || finalizedTopUp != null
            || rejection == null) {
          throw new IllegalArgumentException(
              "rejected operation projection must contain only its rejection");
        }
        return;
      default:
        throw new IllegalArgumentException("unsupported operation state: " + state);
    }
  }

  private static byte[] requireOfflineCashDigest(
      @NotNull final byte[] value, @NotNull final String name) {
    Objects.requireNonNull(value, name);
    boolean containsNonzeroByte = false;
    for (final byte current : value) {
      if (current != 0) {
        containsNonzeroByte = true;
        break;
      }
    }
    if (value.length != 32 || !containsNonzeroByte) {
      throw new IllegalArgumentException(
          name + " must contain exactly 32 bytes and must not be all-zero");
    }
    return Arrays.copyOf(value, value.length);
  }
}
