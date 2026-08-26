package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Response bound to one exact submitted prepared transaction. */
public final class PreparedTransactionSubmitResponseV1 extends AliasJsonValue {
  public static final String SCHEMA = "iroha.taira.prepared-transaction-submit.v1";

  private final String schema;
  private final TairaPublicResetMutationBindingV1 binding;
  private final String operation;
  private final String transactionHashHex;
  private final PreparedTransactionOutcomeV1 outcome;

  /** Constructs an exact submit reconciliation response. */
  public PreparedTransactionSubmitResponseV1(
      final String schema,
      final TairaPublicResetMutationBindingV1 binding,
      final String operation,
      final String transactionHashHex,
      final PreparedTransactionOutcomeV1 outcome) {
    if (!SCHEMA.equals(schema)) throw new IllegalArgumentException("unsupported prepared submit schema");
    if (!TairaPublicResetMutationBindingV1.ONBOARDING.equals(operation)
        && !TairaPublicResetMutationBindingV1.FAUCET.equals(operation)) {
      throw new IllegalArgumentException("unsupported prepared submit operation");
    }
    this.schema = schema;
    this.binding = Objects.requireNonNull(binding, "binding");
    this.operation = operation;
    this.transactionHashHex = TairaPublicResetMutationBindingV1.requireTransactionHash(transactionHashHex, "transactionHashHex");
    this.outcome = Objects.requireNonNull(outcome, "outcome");
  }

  public String schema() { return schema; }
  public TairaPublicResetMutationBindingV1 binding() { return binding; }
  public String operation() { return operation; }
  public String transactionHashHex() { return transactionHashHex; }
  public PreparedTransactionOutcomeV1 outcome() { return outcome; }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("schema", schema);
    map.put("binding", binding.toJsonMap());
    map.put("operation", operation);
    map.put("transaction_hash_hex", transactionHashHex);
    map.put("outcome", outcome.wireValue());
    return map;
  }
}
