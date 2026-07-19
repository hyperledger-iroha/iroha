package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.alias.AccountAliasName;

/** Typed result from resolving a numeric alias index. */
public final class AccountAliasIndexResolution {
  private final BigInteger index;
  private final String alias;
  private final String accountId;
  private final String source;

  /** Constructs one exact index-resolution response. */
  public AccountAliasIndexResolution(
      final BigInteger index, final String alias, final String accountId, final String source) {
    this.index = AccountAliasUInt64.require(index, "index");
    this.alias = AccountAliasName.parse(alias).canonicalText();
    this.accountId = AccountIdLiteral.requireCanonicalI105Address(accountId, "accountId");
    this.source = source == null ? null : AccountAliasesByAccount.requireExactText(source, "source");
  }

  public BigInteger index() { return index; }
  public String alias() { return alias; }
  public String accountId() { return accountId; }
  public String source() { return source; }
}
