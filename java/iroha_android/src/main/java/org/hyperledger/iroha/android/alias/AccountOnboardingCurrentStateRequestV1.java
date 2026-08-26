package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;
import org.hyperledger.iroha.android.address.AccountIdLiteral;

/** Exact closed request for one atomic account-onboarding state observation. */
public final class AccountOnboardingCurrentStateRequestV1 extends AliasJsonValue {
  /** Current and only first-release layout. */
  public static final int VERSION = 1;

  private final int version;
  private final String accountId;
  private final String alias;

  /** Constructs the first-release request. */
  public AccountOnboardingCurrentStateRequestV1(
      final String accountId, final String alias) {
    final String canonicalAlias = AccountAliasName.parse(alias).canonicalText();
    if (!canonicalAlias.equals(alias)) throw new IllegalArgumentException("alias must be canonical");
    this.version = VERSION;
    this.accountId = AccountIdLiteral.requireCanonicalI105Address(accountId, "accountId");
    this.alias = alias;
  }

  public int version() {
    return version;
  }

  public String accountId() {
    return accountId;
  }

  public String alias() {
    return alias;
  }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("version", Integer.valueOf(version));
    map.put("account_id", accountId);
    map.put("alias", alias);
    return map;
  }
}
