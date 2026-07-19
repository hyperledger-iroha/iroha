package org.hyperledger.iroha.android.alias;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeSet;
import org.hyperledger.iroha.android.address.AccountIdLiteral;

/** Secret-free intent accepted by the sponsored account-onboarding planner. */
public final class AccountOnboardingPlanRequestV1 extends AliasJsonValue {
  /** Current request layout. */
  public static final int VERSION = 1;

  private final int version;
  private final String alias;
  private final String accountId;
  private final List<String> permissions;

  /** Constructs a version-one request. */
  public AccountOnboardingPlanRequestV1(
      final String alias, final String accountId, final List<String> permissions) {
    this(VERSION, alias, accountId, permissions);
  }

  /** Constructs an explicitly versioned request. */
  public AccountOnboardingPlanRequestV1(
      final int version,
      final String alias,
      final String accountId,
      final List<String> permissions) {
    if (version != VERSION) throw new IllegalArgumentException("version must be " + VERSION);
    if (permissions == null) throw new IllegalArgumentException("permissions must not be null");
    final TreeSet<String> sorted = new TreeSet<>();
    for (final String permission : permissions) {
      sorted.add(requireToken(permission, "permission"));
    }
    this.version = version;
    this.alias = AccountAliasName.parse(alias).canonicalText();
    this.accountId = AccountIdLiteral.requireCanonicalI105Address(accountId, "accountId");
    this.permissions = Collections.unmodifiableList(new ArrayList<>(sorted));
  }

  public int version() { return version; }
  public String alias() { return alias; }
  public String accountId() { return accountId; }
  public List<String> permissions() { return permissions; }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("version", version);
    map.put("alias", alias);
    map.put("account_id", accountId);
    map.put("permissions", permissions);
    return map;
  }

  static String requireOnboardingCredential(final String value) {
    if (value == null || value.length() < 32 || value.length() > 256) {
      throw new IllegalArgumentException(
          "onboarding token must contain 32..256 printable non-whitespace ASCII bytes");
    }
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (character < '!' || character > '~') {
        throw new IllegalArgumentException(
            "onboarding token must contain 32..256 printable non-whitespace ASCII bytes");
      }
    }
    return value;
  }

  private static String requireToken(final String value, final String field) {
    if (value == null || value.trim().isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(field + " must be non-blank without whitespace");
    }
    for (int index = 0; index < value.length(); index++) {
      if (Character.isWhitespace(value.charAt(index))
          || Character.isISOControl(value.charAt(index))) {
        throw new IllegalArgumentException(field + " must not contain whitespace or controls");
      }
    }
    return value;
  }
}
