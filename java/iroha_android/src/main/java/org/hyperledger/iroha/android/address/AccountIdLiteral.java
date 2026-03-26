package org.hyperledger.iroha.android.address;

import java.util.Objects;

/** Helpers for normalizing account identifier literals. */
public final class AccountIdLiteral {

  private AccountIdLiteral() {}

  /**
   * Requires a canonical encoded account identifier without a trailing {@code @domain} suffix.
   *
   * @param accountId account identifier string
   * @param field field name used in validation messages
   * @return canonical encoded account identifier
   */
  public static String requireCanonicalI105Address(final String accountId, final String field) {
    Objects.requireNonNull(field, "field");
    final String value = Objects.requireNonNull(accountId, field).trim();
    if (value.isEmpty()) {
      throw new IllegalArgumentException(field + " must not be blank");
    }
    if (value.indexOf('@') >= 0) {
      throw new IllegalArgumentException(
          field + " must use canonical I105 encoded account without @domain");
    }
    final AccountAddress.ParseResult parsed;
    try {
      parsed = AccountAddress.parseEncoded(value, null);
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalArgumentException(
          field + " must use a canonical I105 encoded account literal", ex);
    }
    if (parsed.format != AccountAddress.Format.I105) {
      throw new IllegalArgumentException(
          field + " must use a canonical I105 encoded account literal");
    }
    final Integer discriminant = AccountAddress.detectI105Discriminant(value);
    if (discriminant == null) {
      throw new IllegalArgumentException(
          field + " must use a canonical I105 encoded account literal");
    }
    final String canonical;
    try {
      canonical = parsed.address.toI105(discriminant.intValue());
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalArgumentException(
          field + " must use a canonical I105 encoded account literal", ex);
    }
    if (!value.equals(canonical)) {
      throw new IllegalArgumentException(
          field + " must use a canonical I105 encoded account literal");
    }
    return canonical;
  }
}
