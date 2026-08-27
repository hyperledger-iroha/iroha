package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.FeePaymentIntent;

/** Non-mutating prepare body consuming one signed onboarding plan receipt. */
public final class AccountOnboardingPrepareRequestV1 extends AliasJsonValue {
  public static final String SCHEMA = "iroha.accounts.onboard.prepare.v1";

  private final String schema;
  private final TairaPublicResetMutationBindingV1 binding;
  private final AccountOnboardingPlanReceiptV1 receipt;
  private final FeePaymentIntent feePayment;

  /** Constructs an exact V1 prepare request. */
  public AccountOnboardingPrepareRequestV1(
      final TairaPublicResetMutationBindingV1 binding,
      final AccountOnboardingPlanReceiptV1 receipt,
      final FeePaymentIntent feePayment) {
    this(SCHEMA, binding, receipt, feePayment);
  }

  /** Constructs an explicitly schema-bound prepare request. */
  public AccountOnboardingPrepareRequestV1(
      final String schema,
      final TairaPublicResetMutationBindingV1 binding,
      final AccountOnboardingPlanReceiptV1 receipt,
      final FeePaymentIntent feePayment) {
    if (!SCHEMA.equals(schema)) {
      throw new IllegalArgumentException("unsupported onboarding prepare schema");
    }
    this.binding = Objects.requireNonNull(binding, "binding");
    if (!TairaPublicResetMutationBindingV1.ONBOARDING.equals(binding.kind())) {
      throw new IllegalArgumentException("onboarding prepare requires an onboarding binding");
    }
    this.schema = schema;
    this.receipt = Objects.requireNonNull(receipt, "receipt");
    this.feePayment = Objects.requireNonNull(feePayment, "feePayment");
  }

  public String schema() { return schema; }
  public TairaPublicResetMutationBindingV1 binding() { return binding; }
  public AccountOnboardingPlanReceiptV1 receipt() { return receipt; }
  public FeePaymentIntent feePayment() { return feePayment; }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("schema", schema);
    map.put("binding", binding.toJsonMap());
    map.put("receipt", receipt.toJsonMap());
    map.put("fee_payment", feePayment.toJsonMap());
    return map;
  }
}
