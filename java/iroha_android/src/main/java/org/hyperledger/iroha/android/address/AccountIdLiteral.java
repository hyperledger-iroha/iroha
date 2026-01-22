package org.hyperledger.iroha.android.address;

import java.util.Objects;

/** Helpers for normalizing account identifier literals. */
public final class AccountIdLiteral {

  private AccountIdLiteral() {}

  /**
   * Extracts the IH58 address portion from a {@code "<address>@<domain>"} account identifier.
   *
   * @param accountId account identifier string
   * @return IH58 address portion without the domain suffix
   */
  public static String extractIh58Address(final String accountId) {
    final String value = Objects.requireNonNull(accountId, "accountId").trim();
    if (value.isEmpty()) {
      throw new IllegalArgumentException("accountId must not be blank");
    }
    final int atIndex = value.lastIndexOf('@');
    if (atIndex <= 0 || atIndex == value.length() - 1) {
      throw new IllegalArgumentException("Invalid account ID format: " + value);
    }
    return value.substring(0, atIndex);
  }
}
