package org.hyperledger.iroha.android.client;

import org.hyperledger.iroha.android.alias.AccountAliasName;

/** One visible alias bound to an account. */
public final class AccountAliasListItem {
  private final String alias;
  private final String dataspace;
  private final String domain;
  private final boolean primary;

  /** Constructs and cross-checks one exact list item. */
  public AccountAliasListItem(
      final String alias,
      final String dataspace,
      final String domain,
      final boolean primary) {
    final AccountAliasName parsed = AccountAliasName.parse(alias);
    final AccountAliasName normalized = new AccountAliasName("item", domain, dataspace);
    if (!parsed.dataspace().equals(normalized.dataspace())
        || !java.util.Objects.equals(parsed.domain(), normalized.domain())) {
      throw new IllegalArgumentException("alias scope does not match dataspace/domain fields");
    }
    if (!parsed.canonicalText().equals(alias)) {
      throw new IllegalArgumentException("alias must use its canonical representation");
    }
    this.alias = parsed.canonicalText();
    this.dataspace = normalized.dataspace();
    this.domain = normalized.domain();
    this.primary = primary;
  }

  public String alias() { return alias; }
  public String dataspace() { return dataspace; }
  public String domain() { return domain; }
  public boolean isPrimary() { return primary; }
}
