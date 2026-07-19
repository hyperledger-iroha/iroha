package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;

/** Catalog-free textual account alias in `label@domain.dataspace` or `label@dataspace` form. */
public final class AccountAliasName extends AliasJsonValue {
  private final String label;
  private final String domain;
  private final String dataspace;

  /** Constructs and canonicalizes explicit alias segments. */
  public AccountAliasName(final String label, final String domain, final String dataspace) {
    this.label = AliasNameSupport.segment(label, "label");
    this.domain = domain == null ? null : AliasNameSupport.segment(domain, "domain");
    this.dataspace = AliasNameSupport.segment(dataspace, "dataspace");
  }

  /** Parses an alias without consulting a dataspace catalog. */
  public static AccountAliasName parse(final String literal) {
    if (literal == null || literal.isEmpty()) {
      throw new IllegalArgumentException("account alias must not be empty");
    }
    if (!literal.equals(literal.trim())) {
      throw new IllegalArgumentException(
          "account alias must not contain leading or trailing whitespace");
    }
    final int at = literal.indexOf('@');
    if (at <= 0 || at != literal.lastIndexOf('@') || at >= literal.length() - 1) {
      throw new IllegalArgumentException("account alias must contain exactly one '@' separator");
    }
    final String label = literal.substring(0, at);
    final String scope = literal.substring(at + 1);
    final int dot = scope.indexOf('.');
    if (dot < 0) {
      return new AccountAliasName(label, null, scope);
    }
    if (dot <= 0 || dot != scope.lastIndexOf('.') || dot >= scope.length() - 1) {
      throw new IllegalArgumentException(
          "account alias must contain one non-empty domain before the dataspace");
    }
    return new AccountAliasName(label, scope.substring(0, dot), scope.substring(dot + 1));
  }

  /** Returns the canonical alias label. */
  public String label() {
    return label;
  }

  /** Returns the optional canonical domain label. */
  public String domain() {
    return domain;
  }

  /** Returns the canonical textual dataspace. */
  public String dataspace() {
    return dataspace;
  }

  /** Returns the canonical external alias literal. */
  public String canonicalText() {
    return label + "@" + (domain == null ? "" : domain + ".") + dataspace;
  }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("label", label);
    map.put("domain", domain);
    map.put("dataspace", dataspace);
    return map;
  }

  @Override
  public String toString() {
    return canonicalText();
  }
}

