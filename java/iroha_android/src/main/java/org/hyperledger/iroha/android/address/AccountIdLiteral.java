package org.hyperledger.iroha.android.address;

import java.util.Objects;

/** Helpers for normalizing account identifier literals. */
public final class AccountIdLiteral {

  private AccountIdLiteral() {}

  /**
   * Normalizes an encoded account identifier.
   *
   * <p>Accepted format is canonical I105. Legacy account identifiers with an {@code @domain}
   * suffix are rejected.
   *
   * @param accountId account identifier string
   * @return normalized encoded account identifier
   */
  public static String extractI105Address(final String accountId) {
    final String value = Objects.requireNonNull(accountId, "accountId").trim();
    if (value.isEmpty()) {
      throw new IllegalArgumentException("accountId must not be blank");
    }
    if (value.indexOf('@') >= 0) {
      throw new IllegalArgumentException("accountId must not include @domain suffix");
    }
    try {
      AccountAddress.parseEncoded(value, null);
      return value;
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalArgumentException("accountId must be canonical I105 encoded", ex);
    }
  }
}
