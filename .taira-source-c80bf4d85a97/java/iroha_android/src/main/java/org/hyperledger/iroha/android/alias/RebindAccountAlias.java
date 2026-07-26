package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;
import org.hyperledger.iroha.android.address.AccountIdLiteral;

/** Target-CAS account-alias rebind instruction; lease state is not accepted. */
public final class RebindAccountAlias extends AliasJsonValue {
  /** Stable instruction registry identifier. */
  public static final String WIRE_ID = "iroha.account.alias.rebind";

  private final ResolvedAccountAliasV1 alias;
  private final String expectedTargetAccount;
  private final String newTargetAccount;

  /** Constructs an exact target-account compare-and-set rebind. */
  public RebindAccountAlias(
      final ResolvedAccountAliasV1 alias,
      final String expectedTargetAccount,
      final String newTargetAccount) {
    if (alias == null) throw new IllegalArgumentException("alias must not be null");
    this.alias = alias;
    this.expectedTargetAccount =
        AccountIdLiteral.requireCanonicalI105Address(
            expectedTargetAccount, "expectedTargetAccount");
    this.newTargetAccount =
        AccountIdLiteral.requireCanonicalI105Address(newTargetAccount, "newTargetAccount");
  }

  /** Returns the exact resolved alias. */
  public ResolvedAccountAliasV1 alias() { return alias; }

  /** Returns the account that must currently be bound. */
  public String expectedTargetAccount() { return expectedTargetAccount; }

  /** Returns the account to bind. */
  public String newTargetAccount() { return newTargetAccount; }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("alias", alias.toJsonMap());
    map.put("expected_target_account", expectedTargetAccount);
    map.put("new_target_account", newTargetAccount);
    return map;
  }
}
