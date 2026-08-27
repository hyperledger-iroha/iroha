package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.numeric.NumericV1;

/** Authenticated exact faucet transaction prepared by Torii. */
public final class AccountFaucetPreparedTransactionV1 extends AliasJsonValue {
  public static final String SCHEMA = "iroha.taira.prepared-transaction.v1";
  public static final String OPERATION = "faucet";

  private final String schema;
  private final TairaPublicResetMutationBindingV1 binding;
  private final String operation;
  private final AccountFaucetClaimV1 claim;
  private final String semanticHashHex;
  private final String accountId;
  private final String assetDefinitionId;
  private final String assetId;
  private final NumericV1.QuantityValue amount;
  private final String transactionHashHex;
  private final String signedTransactionWireHex;
  private final String signedTransactionWireSha256;
  private final FeePaymentIntent feePayment;
  private final String serverSignature;

  /** Constructs one exact parsed faucet prepared envelope. */
  public AccountFaucetPreparedTransactionV1(
      final String schema,
      final TairaPublicResetMutationBindingV1 binding,
      final String operation,
      final AccountFaucetClaimV1 claim,
      final String semanticHashHex,
      final String accountId,
      final String assetDefinitionId,
      final String assetId,
      final NumericV1.QuantityValue amount,
      final String transactionHashHex,
      final String signedTransactionWireHex,
      final String signedTransactionWireSha256,
      final FeePaymentIntent feePayment,
      final String serverSignature) {
    if (!SCHEMA.equals(schema)) {
      throw new IllegalArgumentException("unsupported prepared transaction schema");
    }
    if (!OPERATION.equals(operation)) {
      throw new IllegalArgumentException("prepared faucet operation must be faucet");
    }
    this.binding = Objects.requireNonNull(binding, "binding");
    if (!TairaPublicResetMutationBindingV1.FAUCET.equals(binding.kind())) {
      throw new IllegalArgumentException("prepared faucet requires a faucet binding");
    }
    this.schema = schema;
    this.operation = operation;
    this.claim = Objects.requireNonNull(claim, "claim");
    this.semanticHashHex =
        TairaPublicResetMutationBindingV1.requireLowerHex32(
            semanticHashHex, "semanticHashHex");
    this.accountId = AccountIdLiteral.requireCanonicalI105Address(accountId, "accountId");
    if (!claim.accountId().equals(this.accountId)) {
      throw new IllegalArgumentException("prepared faucet account must equal the claim account");
    }
    if (!AssetDefinitionIdEncoder.isCanonicalAddress(assetDefinitionId)) {
      throw new IllegalArgumentException(
          "assetDefinitionId must be a canonical asset-definition address");
    }
    this.assetDefinitionId = assetDefinitionId;
    this.assetId = Objects.requireNonNull(assetId, "assetId");
    if (!(assetDefinitionId + "#" + this.accountId).equals(assetId)) {
      throw new IllegalArgumentException(
          "prepared faucet asset must be the exact destination balance");
    }
    this.amount = Objects.requireNonNull(amount, "amount");
    if (amount.mantissa().signum() <= 0) {
      throw new IllegalArgumentException("prepared faucet amount must be positive");
    }
    this.transactionHashHex =
        TairaPublicResetMutationBindingV1.requireTransactionHash(
            transactionHashHex, "transactionHashHex");
    this.signedTransactionWireHex =
        TairaPublicResetMutationBindingV1.requireLowerHex(
            signedTransactionWireHex, "signedTransactionWireHex");
    this.signedTransactionWireSha256 =
        TairaPublicResetMutationBindingV1.requireLowerHex32(
            signedTransactionWireSha256, "signedTransactionWireSha256");
    this.feePayment = Objects.requireNonNull(feePayment, "feePayment");
    this.serverSignature =
        TairaPublicResetMutationBindingV1.requireHex(serverSignature, "serverSignature");
  }

  public String schema() { return schema; }
  public TairaPublicResetMutationBindingV1 binding() { return binding; }
  public String operation() { return operation; }
  public AccountFaucetClaimV1 claim() { return claim; }
  public String semanticHashHex() { return semanticHashHex; }
  public String accountId() { return accountId; }
  public String assetDefinitionId() { return assetDefinitionId; }
  public String assetId() { return assetId; }
  public NumericV1.QuantityValue amount() { return amount; }
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
    map.put("claim", claim.toJsonMap());
    map.put("semantic_hash_hex", semanticHashHex);
    map.put("account_id", accountId);
    map.put("asset_definition_id", assetDefinitionId);
    map.put("asset_id", assetId);
    map.put("amount", amount.toString());
    map.put("transaction_hash_hex", transactionHashHex);
    map.put("signed_transaction_wire_hex", signedTransactionWireHex);
    map.put("signed_transaction_wire_sha256", signedTransactionWireSha256);
    map.put("fee_payment", feePayment.toJsonMap());
    map.put("server_signature", serverSignature);
    return map;
  }
}
