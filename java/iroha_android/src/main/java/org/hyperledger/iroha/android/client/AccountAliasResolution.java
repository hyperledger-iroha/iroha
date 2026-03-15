package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Parsed payload for account alias resolution (`/v1/aliases/resolve`). */
public final class AccountAliasResolution {
  private final String alias;
  private final String accountId;
  private final Long index;
  private final String source;

  public AccountAliasResolution(
      final String alias, final String accountId, final Long index, final String source) {
    this.alias = Objects.requireNonNull(alias, "alias");
    this.accountId = Objects.requireNonNull(accountId, "accountId");
    this.index = index;
    this.source = source;
  }

  public String alias() {
    return alias;
  }

  public String accountId() {
    return accountId;
  }

  public Long index() {
    return index;
  }

  public String source() {
    return source;
  }
}
