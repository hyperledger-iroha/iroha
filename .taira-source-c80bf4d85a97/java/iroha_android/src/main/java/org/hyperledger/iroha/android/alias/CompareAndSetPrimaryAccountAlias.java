package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;
import org.hyperledger.iroha.android.address.AccountIdLiteral;

/** Primary-alias compare-and-set instruction; lease state is not accepted. */
public final class CompareAndSetPrimaryAccountAlias extends AliasJsonValue {
  /** Stable instruction registry identifier. */
  public static final String WIRE_ID = "iroha.account.alias.primary.compare_and_set";

  private final String account;
  private final ResolvedAccountAliasV1 expectedAlias;
  private final ResolvedAccountAliasV1 newAlias;

  /** Constructs an exact primary-alias compare-and-set. */
  public CompareAndSetPrimaryAccountAlias(
      final String account,
      final ResolvedAccountAliasV1 expectedAlias,
      final ResolvedAccountAliasV1 newAlias) {
    this.account = AccountIdLiteral.requireCanonicalI105Address(account, "account");
    this.expectedAlias = expectedAlias;
    this.newAlias = newAlias;
  }

  /** Returns the account whose primary alias changes. */
  public String account() { return account; }

  /** Returns the expected current alias, or null when no primary is expected. */
  public ResolvedAccountAliasV1 expectedAlias() { return expectedAlias; }

  /** Returns the new primary alias, or null when clearing it. */
  public ResolvedAccountAliasV1 newAlias() { return newAlias; }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("account", account);
    map.put("expected_alias", expectedAlias == null ? null : expectedAlias.toJsonMap());
    map.put("new_alias", newAlias == null ? null : newAlias.toJsonMap());
    return map;
  }
}
