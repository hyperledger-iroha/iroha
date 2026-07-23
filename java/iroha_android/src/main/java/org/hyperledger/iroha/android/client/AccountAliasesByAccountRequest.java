package org.hyperledger.iroha.android.client;

import java.util.LinkedHashMap;
import java.util.Map;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.alias.AccountAliasName;

/** Typed request for aliases bound to one canonical account. */
public final class AccountAliasesByAccountRequest {
  private final String accountId;
  private final String dataspace;
  private final String domain;

  /** Constructs an unfiltered account lookup. */
  public AccountAliasesByAccountRequest(final String accountId) {
    this(accountId, null, null);
  }

  /** Constructs an optionally scope-filtered account lookup. */
  public AccountAliasesByAccountRequest(
      final String accountId, final String dataspace, final String domain) {
    if (domain != null && dataspace == null) {
      throw new IllegalArgumentException("domain requires a dataspace filter");
    }
    this.accountId = AccountIdLiteral.requireCanonicalI105Address(accountId, "accountId");
    if (dataspace == null) {
      this.dataspace = null;
      this.domain = null;
    } else {
      final AccountAliasName normalized = new AccountAliasName("filter", domain, dataspace);
      this.dataspace = normalized.dataspace();
      this.domain = normalized.domain();
    }
  }

  public String accountId() { return accountId; }
  public String dataspace() { return dataspace; }
  public String domain() { return domain; }

  /** Returns the Norito-JSON-compatible request object. */
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("account_id", accountId);
    map.put("dataspace", dataspace);
    map.put("domain", domain);
    return map;
  }
}
