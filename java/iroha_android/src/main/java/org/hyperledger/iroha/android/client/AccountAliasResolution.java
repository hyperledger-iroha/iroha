package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.alias.AccountAliasName;

/** Parsed payload for account alias resolution (`/v1/aliases/resolve`). */
public final class AccountAliasResolution {
  private final String alias;
  private final String accountId;
  private final BigInteger index;
  private final String source;

  public AccountAliasResolution(
      final String alias, final String accountId, final BigInteger index, final String source) {
    final String canonicalAlias = AccountAliasName.parse(alias).canonicalText();
    if (!canonicalAlias.equals(alias)) {
      throw new IllegalArgumentException("alias must use its canonical representation");
    }
    this.alias = canonicalAlias;
    this.accountId = AccountIdLiteral.requireCanonicalI105Address(accountId, "accountId");
    this.index = index == null ? null : AccountAliasUInt64.require(index, "index");
    this.source = source == null ? null : AccountAliasesByAccount.requireExactText(source, "source");
  }

  public String alias() {
    return alias;
  }

  public String accountId() {
    return accountId;
  }

  public BigInteger index() {
    return index;
  }

  public String source() {
    return source;
  }
}
