package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.FeePaymentIntent;

/** Non-mutating prepare body consuming one exact faucet claim. */
public final class AccountFaucetPrepareRequestV1 extends AliasJsonValue {
  public static final String SCHEMA = "iroha.accounts.faucet.prepare.v1";

  private final String schema;
  private final TairaPublicResetMutationBindingV1 binding;
  private final AccountFaucetClaimV1 claim;
  private final FeePaymentIntent feePayment;

  /** Constructs an exact V1 faucet prepare request. */
  public AccountFaucetPrepareRequestV1(
      final TairaPublicResetMutationBindingV1 binding,
      final AccountFaucetClaimV1 claim,
      final FeePaymentIntent feePayment) {
    this(SCHEMA, binding, claim, feePayment);
  }

  /** Constructs an explicitly schema-bound faucet prepare request. */
  public AccountFaucetPrepareRequestV1(
      final String schema,
      final TairaPublicResetMutationBindingV1 binding,
      final AccountFaucetClaimV1 claim,
      final FeePaymentIntent feePayment) {
    if (!SCHEMA.equals(schema)) {
      throw new IllegalArgumentException("unsupported faucet prepare schema");
    }
    this.binding = Objects.requireNonNull(binding, "binding");
    if (!TairaPublicResetMutationBindingV1.FAUCET.equals(binding.kind())) {
      throw new IllegalArgumentException("faucet prepare requires a faucet binding");
    }
    this.schema = schema;
    this.claim = Objects.requireNonNull(claim, "claim");
    this.feePayment = Objects.requireNonNull(feePayment, "feePayment");
  }

  public String schema() { return schema; }
  public TairaPublicResetMutationBindingV1 binding() { return binding; }
  public AccountFaucetClaimV1 claim() { return claim; }
  public FeePaymentIntent feePayment() { return feePayment; }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("schema", schema);
    map.put("binding", binding.toJsonMap());
    map.put("claim", claim.toJsonMap());
    map.put("fee_payment", feePayment.toJsonMap());
    return map;
  }
}
