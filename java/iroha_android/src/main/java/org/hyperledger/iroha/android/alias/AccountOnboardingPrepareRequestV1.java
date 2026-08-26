package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Non-mutating prepare body consuming one signed onboarding plan receipt. */
public final class AccountOnboardingPrepareRequestV1 extends AliasJsonValue {
  public static final String SCHEMA = "iroha.accounts.onboard.prepare.v1";

  private final String schema;
  private final TairaPublicResetMutationBindingV1 binding;
  private final AccountOnboardingPlanReceiptV1 receipt;

  /** Constructs an exact V1 prepare request. */
  public AccountOnboardingPrepareRequestV1(
      final TairaPublicResetMutationBindingV1 binding,
      final AccountOnboardingPlanReceiptV1 receipt) {
    this(SCHEMA, binding, receipt);
  }

  /** Constructs an explicitly schema-bound prepare request. */
  public AccountOnboardingPrepareRequestV1(
      final String schema,
      final TairaPublicResetMutationBindingV1 binding,
      final AccountOnboardingPlanReceiptV1 receipt) {
    if (!SCHEMA.equals(schema)) {
      throw new IllegalArgumentException("unsupported onboarding prepare schema");
    }
    this.binding = Objects.requireNonNull(binding, "binding");
    if (!TairaPublicResetMutationBindingV1.ONBOARDING.equals(binding.kind())) {
      throw new IllegalArgumentException("onboarding prepare requires an onboarding binding");
    }
    this.schema = schema;
    this.receipt = Objects.requireNonNull(receipt, "receipt");
  }

  public String schema() { return schema; }
  public TairaPublicResetMutationBindingV1 binding() { return binding; }
  public AccountOnboardingPlanReceiptV1 receipt() { return receipt; }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("schema", schema);
    map.put("binding", binding.toJsonMap());
    map.put("receipt", receipt.toJsonMap());
    return map;
  }
}
