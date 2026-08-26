package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.model.FeePaymentIntent;

/** Authenticated exact transaction returned by onboarding preparation. */
public final class AccountOnboardingPreparedTransactionV1 extends AliasJsonValue
    implements AccountOnboardingPrepareResponseV1 {
  public static final String SCHEMA = "iroha.taira.prepared-transaction.v1";
  public static final String OPERATION = "onboarding";

  private final String schema;
  private final TairaPublicResetMutationBindingV1 binding;
  private final String operation;
  private final AccountOnboardingPlanReceiptV1 receipt;
  private final String semanticHashHex;
  private final String accountId;
  private final String alias;
  private final AliasSetupModels.AliasPlanDispositionV1 disposition;
  private final String transactionHashHex;
  private final String signedTransactionWireHex;
  private final String signedTransactionWireSha256;
  private final FeePaymentIntent feePayment;
  private final String serverSignature;

  /** Constructs an exact parsed prepared transaction. */
  public AccountOnboardingPreparedTransactionV1(
      final String schema,
      final TairaPublicResetMutationBindingV1 binding,
      final String operation,
      final AccountOnboardingPlanReceiptV1 receipt,
      final String semanticHashHex,
      final String accountId,
      final String alias,
      final AliasSetupModels.AliasPlanDispositionV1 disposition,
      final String transactionHashHex,
      final String signedTransactionWireHex,
      final String signedTransactionWireSha256,
      final FeePaymentIntent feePayment,
      final String serverSignature) {
    if (!SCHEMA.equals(schema)) throw new IllegalArgumentException("unsupported prepared transaction schema");
    if (!OPERATION.equals(operation)) throw new IllegalArgumentException("prepared onboarding operation must be onboarding");
    this.binding = Objects.requireNonNull(binding, "binding");
    if (!TairaPublicResetMutationBindingV1.ONBOARDING.equals(binding.kind())) {
      throw new IllegalArgumentException("prepared onboarding requires an onboarding binding");
    }
    this.disposition = Objects.requireNonNull(disposition, "disposition");
    if (disposition == AliasSetupModels.AliasPlanDispositionV1.CONFLICT
        || disposition == AliasSetupModels.AliasPlanDispositionV1.NO_OP) {
      throw new IllegalArgumentException("prepared onboarding requires create or repair disposition");
    }
    final String canonicalAlias = AccountAliasName.parse(alias).canonicalText();
    if (!canonicalAlias.equals(alias)) throw new IllegalArgumentException("alias must be canonical");
    this.schema = schema;
    this.operation = operation;
    this.receipt = Objects.requireNonNull(receipt, "receipt");
    this.semanticHashHex = TairaPublicResetMutationBindingV1.requireLowerHex32(semanticHashHex, "semanticHashHex");
    this.accountId = AccountIdLiteral.requireCanonicalI105Address(accountId, "accountId");
    this.alias = alias;
    this.transactionHashHex = TairaPublicResetMutationBindingV1.requireTransactionHash(transactionHashHex, "transactionHashHex");
    this.signedTransactionWireHex = TairaPublicResetMutationBindingV1.requireLowerHex(signedTransactionWireHex, "signedTransactionWireHex");
    this.signedTransactionWireSha256 = TairaPublicResetMutationBindingV1.requireLowerHex32(signedTransactionWireSha256, "signedTransactionWireSha256");
    this.feePayment = Objects.requireNonNull(feePayment, "feePayment");
    this.serverSignature = TairaPublicResetMutationBindingV1.requireHex(serverSignature, "serverSignature");
  }

  public String schema() { return schema; }
  public TairaPublicResetMutationBindingV1 binding() { return binding; }
  public String operation() { return operation; }
  public AccountOnboardingPlanReceiptV1 receipt() { return receipt; }
  public String semanticHashHex() { return semanticHashHex; }
  public String accountId() { return accountId; }
  public String alias() { return alias; }
  public AliasSetupModels.AliasPlanDispositionV1 disposition() { return disposition; }
  public String transactionHashHex() { return transactionHashHex; }
  public String signedTransactionWireHex() { return signedTransactionWireHex; }
  public String signedTransactionWireSha256() { return signedTransactionWireSha256; }
  public FeePaymentIntent feePayment() { return feePayment; }
  public String serverSignature() { return serverSignature; }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("schema", schema);
    map.put("binding", binding.toJsonMap());
    map.put("operation", operation);
    map.put("receipt", receipt.toJsonMap());
    map.put("semantic_hash_hex", semanticHashHex);
    map.put("account_id", accountId);
    map.put("alias", alias);
    map.put("disposition", disposition.toJsonMap());
    map.put("transaction_hash_hex", transactionHashHex);
    map.put("signed_transaction_wire_hex", signedTransactionWireHex);
    map.put("signed_transaction_wire_sha256", signedTransactionWireSha256);
    map.put("fee_payment", feePayment.toJsonMap());
    map.put("server_signature", serverSignature);
    return map;
  }
}
