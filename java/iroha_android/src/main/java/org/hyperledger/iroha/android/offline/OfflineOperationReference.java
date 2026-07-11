package org.hyperledger.iroha.android.offline;

import java.math.BigInteger;
import java.util.Objects;

/** Reference returned after Torii accepts an asynchronous Offline operation. */
public final class OfflineOperationReference {
  private static final BigInteger U64_MAX = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);
  private final String operationId;
  private final OfflineOperationKind kind;
  private final OfflineOperationState state;
  private final String transactionHash;
  private final String statusUri;
  private final BigInteger submittedAtMs;

  public OfflineOperationReference(
      final String operationId,
      final OfflineOperationKind kind,
      final OfflineOperationState state,
      final String transactionHash,
      final String statusUri,
      final BigInteger submittedAtMs) {
    this.operationId = OfflineOperationCodec.requireOperationId(operationId);
    this.kind = Objects.requireNonNull(kind, "kind");
    this.state = Objects.requireNonNull(state, "state");
    this.transactionHash =
        OfflineOperationCodec.requireTransactionHash(transactionHash, "transactionHash");
    this.statusUri = OfflineOperationCodec.requireOperationStatusUri(statusUri, this.operationId);
    this.submittedAtMs = Objects.requireNonNull(submittedAtMs, "submittedAtMs");
    if (submittedAtMs.signum() < 0 || submittedAtMs.compareTo(U64_MAX) > 0) {
      throw new IllegalArgumentException("submittedAtMs must fit in an unsigned 64-bit integer");
    }
  }

  public String operationId() {
    return operationId;
  }

  public OfflineOperationKind kind() {
    return kind;
  }

  public OfflineOperationState state() {
    return state;
  }

  public String transactionHash() {
    return transactionHash;
  }

  public String statusUri() {
    return statusUri;
  }

  public BigInteger submittedAtMs() {
    return submittedAtMs;
  }

  @Override
  public boolean equals(final Object other) {
    if (this == other) {
      return true;
    }
    if (!(other instanceof OfflineOperationReference)) {
      return false;
    }
    final OfflineOperationReference that = (OfflineOperationReference) other;
    return operationId.equals(that.operationId)
        && kind == that.kind
        && state == that.state
        && transactionHash.equals(that.transactionHash)
        && statusUri.equals(that.statusUri)
        && submittedAtMs.equals(that.submittedAtMs);
  }

  @Override
  public int hashCode() {
    return Objects.hash(operationId, kind, state, transactionHash, statusUri, submittedAtMs);
  }

}
