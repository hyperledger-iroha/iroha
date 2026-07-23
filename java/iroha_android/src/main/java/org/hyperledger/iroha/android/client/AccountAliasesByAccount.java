package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import org.hyperledger.iroha.android.address.AccountIdLiteral;

/** Visibility-filtered aliases bound to one account. */
public final class AccountAliasesByAccount {
  private final String accountId;
  private final BigInteger total;
  private final List<AccountAliasListItem> items;
  private final String source;

  /** Constructs one complete visibility-filtered response. */
  public AccountAliasesByAccount(
      final String accountId,
      final BigInteger total,
      final List<AccountAliasListItem> items,
      final String source) {
    final BigInteger checkedTotal = AccountAliasUInt64.require(total, "total");
    if (items == null
        || items.contains(null)
        || !checkedTotal.equals(BigInteger.valueOf(items.size()))) {
      throw new IllegalArgumentException("total must match the non-null visible item count");
    }
    for (int index = 1; index < items.size(); index++) {
      if (items.get(index - 1).alias().compareTo(items.get(index).alias()) > 0) {
        throw new IllegalArgumentException("items must be sorted by canonical alias");
      }
    }
    this.accountId = AccountIdLiteral.requireCanonicalI105Address(accountId, "accountId");
    this.total = checkedTotal;
    this.items = Collections.unmodifiableList(new ArrayList<>(items));
    this.source = source == null ? null : requireExactText(source, "source");
  }

  public String accountId() { return accountId; }
  public BigInteger total() { return total; }
  public List<AccountAliasListItem> items() { return items; }
  public String source() { return source; }

  static String requireExactText(final String value, final String field) {
    if (value.trim().isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(
          field + " must be non-blank without surrounding whitespace");
    }
    for (int index = 0; index < value.length(); index++) {
      if (Character.isISOControl(value.charAt(index))) {
        throw new IllegalArgumentException(field + " must not contain controls");
      }
    }
    return value;
  }
}
