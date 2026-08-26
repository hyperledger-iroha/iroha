package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.address.AccountIdLiteral;

/** Authenticated nonterminal result requiring one fresh atomic account-and-alias observation. */
public final class AccountOnboardingProofRequiredPrepareResponseV1 extends AliasJsonValue
    implements AccountOnboardingPrepareResponseV1 {
  public static final String SCHEMA = "iroha.accounts.onboard.prepare-proof-required.v1";
  public static final String OUTCOME = "ProofRequired";
  public static final String PROOF_KIND = "account_alias_current_state";

  private final String schema;
  private final TairaPublicResetMutationBindingV1 binding;
  private final String operation;
  private final String outcome;
  private final String proofKind;
  private final String semanticHashHex;
  private final String accountId;
  private final String alias;
  private final AliasSetupModels.AliasPlanDispositionV1 disposition;
  private final String serverSignature;

  /** Constructs an exact authenticated no-op response. */
  public AccountOnboardingProofRequiredPrepareResponseV1(
      final String schema,
      final TairaPublicResetMutationBindingV1 binding,
      final String operation,
      final String outcome,
      final String proofKind,
      final String semanticHashHex,
      final String accountId,
      final String alias,
      final AliasSetupModels.AliasPlanDispositionV1 disposition,
      final String serverSignature) {
    if (!SCHEMA.equals(schema)) {
      throw new IllegalArgumentException("unsupported onboarding proof-required schema");
    }
    if (!AccountOnboardingPreparedTransactionV1.OPERATION.equals(operation)) {
      throw new IllegalArgumentException("proof-required operation must be onboarding");
    }
    if (!OUTCOME.equals(outcome)) {
      throw new IllegalArgumentException("outcome must be ProofRequired");
    }
    if (!PROOF_KIND.equals(proofKind)) {
      throw new IllegalArgumentException(
          "proofKind must require current account and alias state");
    }
    this.binding = Objects.requireNonNull(binding, "binding");
    if (!TairaPublicResetMutationBindingV1.ONBOARDING.equals(binding.kind())) {
      throw new IllegalArgumentException("proof-required onboarding requires an onboarding binding");
    }
    this.disposition = Objects.requireNonNull(disposition, "disposition");
    if (disposition != AliasSetupModels.AliasPlanDispositionV1.NO_OP) {
      throw new IllegalArgumentException("proof-required onboarding must report no-op");
    }
    final String canonicalAlias = AccountAliasName.parse(alias).canonicalText();
    if (!canonicalAlias.equals(alias)) throw new IllegalArgumentException("alias must be canonical");
    this.schema = schema;
    this.operation = operation;
    this.outcome = outcome;
    this.proofKind = proofKind;
    this.semanticHashHex = TairaPublicResetMutationBindingV1.requireLowerHex32(semanticHashHex, "semanticHashHex");
    this.accountId = AccountIdLiteral.requireCanonicalI105Address(accountId, "accountId");
    this.alias = alias;
    this.serverSignature = TairaPublicResetMutationBindingV1.requireHex(serverSignature, "serverSignature");
  }

  public String schema() { return schema; }
  public TairaPublicResetMutationBindingV1 binding() { return binding; }
  public String operation() { return operation; }
  public String outcome() { return outcome; }
  public String proofKind() { return proofKind; }
  public String semanticHashHex() { return semanticHashHex; }
  public String accountId() { return accountId; }
  public String alias() { return alias; }
  public AliasSetupModels.AliasPlanDispositionV1 disposition() { return disposition; }
  public String serverSignature() { return serverSignature; }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("schema", schema);
    map.put("binding", binding.toJsonMap());
    map.put("operation", operation);
    map.put("outcome", outcome);
    map.put("proof_kind", proofKind);
    map.put("semantic_hash_hex", semanticHashHex);
    map.put("account_id", accountId);
    map.put("alias", alias);
    map.put("disposition", disposition.toJsonMap());
    map.put("server_signature", serverSignature);
    return map;
  }
}
