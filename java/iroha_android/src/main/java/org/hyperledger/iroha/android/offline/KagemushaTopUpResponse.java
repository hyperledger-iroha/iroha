package org.hyperledger.iroha.android.offline;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;

/** Torii response after a Kagemusha top-up request is accepted for chain submission. */
public final class KagemushaTopUpResponse {
  private final String operationId;
  private final String chainTxHash;
  private final String assetDefinitionId;
  private final String amount;
  private final List<String> topupAnchorNullifiers;
  private final List<String> outputCommitments;
  private final String rootHint;

  public KagemushaTopUpResponse(
      final String operationId,
      final String chainTxHash,
      final String assetDefinitionId,
      final String amount,
      final List<String> topupAnchorNullifiers,
      final List<String> outputCommitments,
      final String rootHint) {
    this.operationId = requireExactNonEmptyText(operationId, "operationId");
    this.chainTxHash = requireExactNonEmptyText(chainTxHash, "chainTxHash");
    this.assetDefinitionId = requireExactNonEmptyText(assetDefinitionId, "assetDefinitionId");
    this.amount = requireExactNonEmptyText(amount, "amount");
    this.topupAnchorNullifiers = immutableExactList(topupAnchorNullifiers, "topupAnchorNullifiers");
    this.outputCommitments = immutableExactList(outputCommitments, "outputCommitments");
    this.rootHint = requireExactNonEmptyText(rootHint, "rootHint");
  }

  public String operationId() {
    return operationId;
  }

  public String chainTxHash() {
    return chainTxHash;
  }

  public String assetDefinitionId() {
    return assetDefinitionId;
  }

  public String amount() {
    return amount;
  }

  public List<String> topupAnchorNullifiers() {
    return topupAnchorNullifiers;
  }

  public List<String> outputCommitments() {
    return outputCommitments;
  }

  public String rootHint() {
    return rootHint;
  }

  private static List<String> immutableExactList(final List<String> values, final String field) {
    Objects.requireNonNull(values, field);
    final List<String> copy = new ArrayList<>(values.size());
    for (int index = 0; index < values.size(); index++) {
      copy.add(requireExactNonEmptyText(values.get(index), field + "[" + index + "]"));
    }
    return Collections.unmodifiableList(copy);
  }

  private static String requireExactNonEmptyText(final String value, final String field) {
    Objects.requireNonNull(value, field);
    if (value.isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(field + " must be exact non-empty text");
    }
    return value;
  }
}
